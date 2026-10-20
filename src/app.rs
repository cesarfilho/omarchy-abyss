use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use signal_hook::{consts::signal, flag};
use std::{
    fs::OpenOptions,
    io::{self, BufWriter, Write},
    process::{Command, Stdio},
    collections::VecDeque,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant},
};

use crate::{
    cutscene,
    toast::{Feed, Toasts},
    gfx::{Frame, Overlay, Presenter},
    state::{self, Save, Session, ZONE_NAMES},
    world::{Layout, World, WorldEvent},
};

const FRAME: Duration = Duration::from_millis(33);
const SCREENSAVER_CLASS: &str = "org.omarchy.screensaver";

pub struct Options {
    /// Running as the Omarchy screensaver: hide the pointer and quit when the window loses focus.
    pub screensaver: bool,
    pub skip_intro: bool,
    /// Hyprland monitor name this window is on; used to place it in the shared panorama.
    pub monitor: Option<String>,
}

enum Quit {
    Key,
    Signal,
    Focus,
    Wake,
}

fn hypr_cursor(visible: bool) {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return;
    }
    let lua = format!("hl.config({{ cursor = {{ invisible = {} }} }})", !visible);
    let ok = Command::new("hyprctl")
        .args(["eval", &lua])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let _ = Command::new("hyprctl")
            .args(["keyword", "cursor:invisible", if visible { "false" } else { "true" }])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

fn screensaver_focused() -> bool {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return true;
    }
    Command::new("hyprctl")
        .args(["activewindow", "-j"])
        .stderr(Stdio::null())
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains(SCREENSAVER_CLASS))
        .unwrap_or(true)
}

/// Terminals start at 80x24 and only resize once the compositor sizes the window.
fn wait_for_resize() {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline && terminal::size().map(|s| s == (80, 24)).unwrap_or(false) {
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Restores the terminal on every exit path, including panics.
struct Terminal {
    pointer_hidden: bool,
}

impl Terminal {
    fn enter(hide_pointer: bool) -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(out, EnterAlternateScreen, Hide)?;
        out.write_all(b"\x1b[?7l\x1b[2J")?;
        out.flush()?;
        if hide_pointer {
            hypr_cursor(false);
        }
        Ok(Self { pointer_hidden: hide_pointer })
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[0m\x1b[?7h");
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        if self.pointer_hidden {
            hypr_cursor(true);
        }
    }
}

/// (terminal columns, rows, frame width, frame height). The frame has 2x4 square pixels per cell.
fn pixel_size() -> (usize, usize, usize, usize) {
    let (cols, rows) = terminal::size().unwrap_or((120, 40));
    (cols as usize, rows as usize, cols as usize * 2, rows as usize * 4)
}

/// Where this monitor sits among all monitors (side by side by their x position), in frame pixels.
/// Every instance builds the same world from the same clock and draws only its own slice, so a
/// fish that swims off one screen appears on the next.
fn layout(monitor: &Option<String>, frame_w: usize) -> Layout {
    let single = Layout::single(frame_w);
    let Some(name) = monitor else { return single };
    let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).stderr(Stdio::null()).output() else { return single };
    let Ok(list) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else { return single };
    let logical = |m: &serde_json::Value| m["width"].as_f64().unwrap_or(0.0) / m["scale"].as_f64().unwrap_or(1.0).max(0.1);
    let Some(me) = list.iter().find(|m| m["name"].as_str() == Some(name)) else { return single };
    let lw = logical(me);
    if lw <= 0.0 {
        return single;
    }
    let k = frame_w as f64 / lw;
    let min_x = list.iter().filter_map(|m| m["x"].as_f64()).fold(f64::MAX, f64::min);
    let total: f64 = list.iter().map(logical).sum();
    Layout { pano_w: (total * k) as usize, off: ((me["x"].as_f64().unwrap_or(0.0) - min_x) * k).max(0.0) as usize }
}

fn hud(overlay: &mut Overlay, save: &Save) {
    let zone = save.zone();
    let bar_len = 14;
    let filled = (save.progress() * bar_len as f64).round() as usize;
    let bar: String = (0..bar_len).map(|i| if i < filled { '█' } else { '░' }).collect();
    let cream = [235, 240, 235];
    let dim = [160, 190, 210];
    let gold = [255, 220, 120];
    overlay.text(1, 0, &format!(" Nv {} · {} · {} m ", save.level(), ZONE_NAMES[zone], save.depth_m()), cream);
    overlay.text(1, 1, &format!(" {} {:.0}/{:.0} XP ", bar, save.xp, state::xp_at(save.level() + 1)), dim);
    overlay.text(
        1,
        2,
        &format!(" ★ {}/{} · {} pts · série {} d ", save.trophies.len(), state::TROPHIES.len(), save.points, save.streak),
        gold,
    );
}

fn other_instances() -> Vec<u32> {
    let me = std::process::id();
    Command::new("pgrep")
        .args(["-x", "abyss"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter_map(|l| l.trim().parse::<u32>().ok())
                .filter(|&p| p != me)
                .collect()
        })
        .unwrap_or_default()
}

pub fn run(opts: Options) -> io::Result<()> {
    wait_for_resize();
    let term = Terminal::enter(opts.screensaver)?;
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout());

    let stop = Arc::new(AtomicBool::new(false));
    let wake = Arc::new(AtomicBool::new(false));
    let _ = flag::register(signal::SIGUSR1, Arc::clone(&wake));
    for sig in [signal::SIGTERM, signal::SIGHUP, signal::SIGINT, signal::SIGQUIT] {
        let _ = flag::register(sig, Arc::clone(&stop));
    }

    // Only one instance (one monitor) earns XP; the rest render the same state read-only.
    let _ = std::fs::create_dir_all(state::state_dir());
    let lock = OpenOptions::new().create(true).write(true).truncate(false).open(state::state_dir().join("lock"))?;
    let primary = lock.try_lock().is_ok();

    let mut toasts = Toasts::new();
    let mut feed = Feed::new();
    let (mut today, mut hour) = state::local_clock();
    let mut save = Save::load();
    let mut sess = Session::default();
    if primary {
        Session::clear_last();
        sess.days_away = save.begin_session(today);
        for g in save.check_trophies(&mut sess, today) {
            toasts.push(&g);
            feed.publish(&g);
        }
        save.store();
    }

    let (mut cols, mut rows, mut pw, mut ph) = pixel_size();
    let mut world = World::new(pw, ph, layout(&opts.monitor, pw), save.zone_f(), save.reef, state::is_night(hour));
    let mut frame = Frame::new(pw, ph);
    let mut presenter = Presenter::new(cols, rows);

    let quit: Option<Quit>;
    let mut pending: VecDeque<(String, &'static str)> = VecDeque::new();
    if !opts.skip_intro {
        pending.push_back((cutscene::intro_text(&save), "bubbles"));
    }

    let (mut last, mut sec_acc, mut save_acc, mut clock_acc, mut reload_acc, mut poll_acc) =
        (Instant::now(), 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);

    'main: loop {
        if stop.load(Ordering::Relaxed) {
            quit = Some(Quit::Signal);
            break;
        }
        if wake.load(Ordering::Relaxed) {
            quit = Some(Quit::Wake);
            break;
        }

        // Cutscenes (ttfx) run between frames and replace the scene for a few seconds.
        if let Some((text, effect)) = pending.pop_front() {
            if cutscene::play(&text, effect)? {
                quit = Some(Quit::Key);
                break;
            }
            presenter.invalidate();
            last = Instant::now();
            continue;
        }

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        let night = state::is_night(hour);
        for ev in world.update(dt as f32, save.zone_f(), save.reef, night) {
            if ev == WorldEvent::WhalePassed && primary {
                for g in save.on_whale(&mut sess, today) {
                    toasts.push(&g);
                    feed.publish(&g);
                }
            }
        }

        sec_acc += dt;
        save_acc += dt;
        clock_acc += dt;
        reload_acc += dt;
        if sec_acc >= 1.0 {
            if primary {
                for g in save.tick(sec_acc, &mut sess, night, today) {
                    toasts.push(&g);
                    feed.publish(&g);
                }
            }
            if opts.screensaver && !screensaver_focused() {
                quit = Some(Quit::Focus);
                break;
            }
            sec_acc = 0.0;
        }
        if clock_acc >= 60.0 {
            (today, hour) = state::local_clock();
            clock_acc = 0.0;
        }
        if primary && save_acc >= 10.0 {
            save.store();
            save_acc = 0.0;
        }
        poll_acc += dt;
        if !primary && poll_acc >= 0.5 {
            poll_acc = 0.0;
            for g in feed.poll() {
                toasts.push(&g);
            }
        }
        if !primary && reload_acc >= 5.0 {
            save = Save::load();
            reload_acc = 0.0;
        }
        world.draw(&mut frame);
        let mut overlay = Overlay::new(cols, rows);
        hud(&mut overlay, &save);
        toasts.update(dt as f32);
        toasts.draw(&mut overlay, cols);
        presenter.present(&frame, &overlay, &mut out)?;

        // Sleep until the next frame, but wake on any key.
        let spent = Instant::now().duration_since(now);
        let wait = FRAME.saturating_sub(spent);
        if event::poll(wait)? {
            loop {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        quit = Some(Quit::Key);
                        break 'main;
                    }
                    Event::Resize(..) => {
                        (cols, rows, pw, ph) = pixel_size();
                        world.resize(pw, ph, layout(&opts.monitor, pw));
                        frame = Frame::new(pw, ph);
                        presenter = Presenter::new(cols, rows);
                        let _ = execute!(io::stdout(), terminal::Clear(terminal::ClearType::All));
                    }
                    _ => {}
                }
                if !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }
    }

    // Farewell: persist, then show the summary on the instance that received the key.
    if primary {
        save.snapshot(&mut sess);
        save.store();
        sess.store_last();
    }
    // A key press on any monitor wakes them all; an external wake (USR1) already reached every instance.
    if matches!(quit, Some(Quit::Key | Quit::Wake)) {
        if !primary && matches!(quit, Some(Quit::Key)) {
            for pid in other_instances() {
                let _ = Command::new("kill").arg(pid.to_string()).status();
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            while lock.try_lock().is_err() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        if !primary && matches!(quit, Some(Quit::Wake)) {
            let deadline = Instant::now() + Duration::from_secs(2);
            while lock.try_lock().is_err() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        if primary || matches!(quit, Some(Quit::Key | Quit::Wake)) {
          if let Some(s) = Session::load_last() {
            if s.secs >= 5.0 {
                let _ = cutscene::play(&cutscene::summary_text(&s), "waves");
            }
          }
        }
    }
    drop(out);
    drop(term);
    Ok(())
}
