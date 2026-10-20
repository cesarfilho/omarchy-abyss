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
    config::Config,
    cutscene,
    gfx::{Frame, Overlay, Presenter},
    ipc::{self, Client, Request, Server},
    power::Power,
    state::{self, GameEvent, Save, Session, ZONE_NAMES},
    theme::Palette,
    toast::Toasts,
    world::{Layout, World, WorldEvent},
};

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

fn hud(overlay: &mut Overlay, save: &Save, pal: &Palette) {
    let zone = save.zone();
    let bar_len = 14;
    let filled = (save.progress() * bar_len as f64).round() as usize;
    let bar: String = (0..bar_len).map(|i| if i < filled { '█' } else { '░' }).collect();
    overlay.text(1, 0, &format!(" Nv {} · {} · {} m ", save.level(), ZONE_NAMES[zone], save.depth_m()), pal.text);
    overlay.text(1, 1, &format!(" {} {:.0}/{:.0} XP ", bar, save.xp, state::xp_at(save.level() + 1)), pal.dim);
    overlay.text(
        1,
        2,
        &format!(" ★ {}/{} · {} pts · série {} d ", save.trophies.len(), state::TROPHIES.len(), save.points, save.streak),
        pal.gold,
    );
}

/// Restores the terminal before the default message prints (it would be lost on the alternate
/// screen) and leaves a note in `crash.log`, so a crash never leaves a hidden pointer or a raw tty.
fn install_panic_hook(hide_pointer: bool) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let mut out = io::stdout();
        let _ = out.write_all(b"\x1b[0m\x1b[?7h");
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        if hide_pointer {
            hypr_cursor(true);
        }
        let _ = std::fs::create_dir_all(state::state_dir());
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(state::state_dir().join("crash.log")) {
            let _ = writeln!(f, "{info}");
        }
        default(info);
    }));
}

/// Shows achievements here and tells the other monitors.
fn announce(toasts: &mut Toasts, server: &mut Option<Server>, events: Vec<GameEvent>) {
    for g in events {
        toasts.push(&g);
        if let Some(sv) = server {
            sv.broadcast(&ipc::encode(&g));
        }
    }
}

fn send_state(server: &mut Option<Server>, save: &Save) {
    if let Some(sv) = server
        && sv.peers() > 0 {
            sv.broadcast(&serde_json::json!({ "t": "state", "save": save }));
        }
}

pub fn run(opts: Options) -> io::Result<()> {
    let cfg = Config::load();
    let pal = Palette::load(&cfg.palette);
    install_panic_hook(opts.screensaver);
    wait_for_resize();
    let term = Terminal::enter(opts.screensaver)?;
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout());

    let stop = Arc::new(AtomicBool::new(false));
    let wake = Arc::new(AtomicBool::new(false));
    let _ = flag::register(signal::SIGUSR1, Arc::clone(&wake));
    for sig in [signal::SIGTERM, signal::SIGHUP, signal::SIGINT, signal::SIGQUIT] {
        let _ = flag::register(sig, Arc::clone(&stop));
    }

    // Only one instance (one monitor) earns XP and serves the others; the kernel drops the lock
    // when that process dies, however it dies.
    let _ = std::fs::create_dir_all(state::state_dir());
    let lock = OpenOptions::new().create(true).write(true).truncate(false).open(state::state_dir().join("lock"))?;
    let primary = lock.try_lock().is_ok();
    let sock = ipc::sock_path();

    // crossterm spins forever inside `poll`/`read` once the terminal has hung up (it ignores the
    // EIO), so a closed window would leave this process at 100% CPU. The signal handlers above
    // only set a flag; this thread is the backstop that ends the process if the loop never sees it.
    let (watch, sock_path, hidden) = (Arc::clone(&stop), ipc::sock_path(), opts.screensaver);
    std::thread::spawn(move || {
        while !watch.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(100));
        }
        std::thread::sleep(Duration::from_millis(1500));
        // Still here: the normal exit path is stuck. Undo what the process changed, then leave.
        if hidden {
            hypr_cursor(true);
        }
        if primary {
            let _ = std::fs::remove_file(&sock_path);
        }
        let _ = std::fs::remove_file(state::state_dir().join(format!("cutscene.{}.txt", std::process::id())));
        std::process::exit(0);
    });

    let mut server: Option<Server> = if primary { Server::bind(&sock).ok() } else { None };
    let mut client: Option<Client> = if primary { None } else { Client::connect(&sock, Duration::from_secs(3)) };

    let mut toasts = Toasts::new(pal);
    let (mut today, mut hour) = state::local_clock();
    let mut save = Save::load();
    save.no_seasons = !cfg.seasons;
    let mut sess = Session::default();
    if primary {
        Session::clear_last();
        sess.days_away = save.begin_session(today);
        let events = save.check_trophies(&mut sess, today);
        announce(&mut toasts, &mut server, events);
        save.store();
    }
    let season_of = |day: i64| if cfg.seasons { state::season_at(day) } else { None };
    let mut season = season_of(today);
    if let Some(se) = season {
        toasts.push_season(se);
    }

    let (mut cols, mut rows, mut pw, mut ph) = pixel_size();
    let mut world = World::new(pw, ph, layout(&opts.monitor, pw), save.zone_f(), save.reef, state::is_night(hour));
    world.set_palette(pal);
    world.set_season(season.map(|s| s.id));
    let mut frame = Frame::new(pw, ph);
    let mut presenter = Presenter::new(cols, rows);

    let quit: Quit;
    let mut pending: VecDeque<(String, &'static str)> = VecDeque::new();
    if !opts.skip_intro {
        pending.push_back((cutscene::intro_text(&save), "bubbles"));
    }

    let mut power = Power::read(&opts.monitor);
    let (mut last, mut sec_acc, mut save_acc, mut clock_acc, mut reload_acc, mut state_acc, mut power_acc) =
        (Instant::now(), 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let mut display_was_on = true;

    'main: loop {
        if stop.load(Ordering::Relaxed) {
            quit = Quit::Signal;
            break;
        }
        if wake.load(Ordering::Relaxed) {
            quit = Quit::Wake;
            break;
        }

        // Cutscenes (ttfx) run between frames and replace the scene for a few seconds.
        if let Some((text, effect)) = pending.pop_front() {
            if cutscene::play(&text, effect, &stop)? {
                quit = Quit::Key;
                break;
            }
            presenter.invalidate();
            last = Instant::now();
            continue;
        }

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(0.1);
        last = now;

        // The primary serves the other monitors and the CLI.
        if let Some(sv) = &mut server {
            for req in sv.poll() {
                match req {
                    Request::Joined => send_state(&mut server, &save),
                    Request::Wake => {
                        quit = Quit::Wake;
                        break 'main;
                    }
                    Request::Stop => {
                        quit = Quit::Signal;
                        break 'main;
                    }
                }
            }
        }
        // A secondary mirrors the primary: state, achievements, and when to stop.
        if let Some(c) = &mut client {
            let (msgs, alive) = c.poll();
            for m in msgs {
                match m["t"].as_str() {
                    Some("state") => {
                        if let Ok(s) = serde_json::from_value::<Save>(m["save"].clone()) {
                            save = s;
                            save.no_seasons = !cfg.seasons;
                        }
                    }
                    Some("event") => {
                        if let Some(g) = ipc::decode(&m) {
                            toasts.push(&g);
                        }
                    }
                    Some("wake") => {
                        quit = Quit::Wake;
                        break 'main;
                    }
                    Some("stop") => {
                        quit = Quit::Signal;
                        break 'main;
                    }
                    _ => {}
                }
            }
            if !alive {
                quit = Quit::Signal;
                break;
            }
        }

        let night = state::is_night(hour);
        for ev in world.update(dt as f32, save.zone_f(), save.reef, night) {
            if !primary {
                continue;
            }
            let events = match ev {
                WorldEvent::WhalePassed => save.on_whale(&mut sess, today),
                WorldEvent::Spotted(id) => save.on_spotted(id, &mut sess, today),
            };
            announce(&mut toasts, &mut server, events);
        }

        sec_acc += dt;
        save_acc += dt;
        clock_acc += dt;
        reload_acc += dt;
        state_acc += dt;
        power_acc += dt;
        if sec_acc >= 1.0 {
            if primary {
                let (zone, reef) = (save.zone(), save.reef);
                let events = save.tick(sec_acc, &mut sess, night, today);
                announce(&mut toasts, &mut server, events);
                // Level, zone and reef shape the scene: push them to the others right away.
                if save.zone() != zone || save.reef != reef {
                    send_state(&mut server, &save);
                    state_acc = 0.0;
                }
            }
            if opts.screensaver && !screensaver_focused() {
                quit = Quit::Focus;
                break;
            }
            sec_acc = 0.0;
        }
        if clock_acc >= 60.0 {
            (today, hour) = state::local_clock();
            season = season_of(today);
            world.set_season(season.map(|s| s.id));
            clock_acc = 0.0;
        }
        if primary && save_acc >= 10.0 {
            save.store();
            save_acc = 0.0;
        }
        if primary && state_acc >= 2.0 {
            send_state(&mut server, &save);
            state_acc = 0.0;
        }
        // Without a link to the primary, fall back to re-reading the save file.
        if !primary && client.is_none() && reload_acc >= 5.0 {
            save = Save::load();
            save.no_seasons = !cfg.seasons;
            reload_acc = 0.0;
        }
        if power_acc >= 5.0 {
            power = Power::read(&opts.monitor);
            power_acc = 0.0;
        }

        toasts.update(dt as f32);
        if power.display_on {
            if !display_was_on {
                presenter.invalidate();
            }
            world.draw(&mut frame);
            let mut overlay = Overlay::new(cols, rows);
            hud(&mut overlay, &save, &pal);
            toasts.draw(&mut overlay, cols);
            presenter.present(&frame, &overlay, &mut out)?;
        }
        display_was_on = power.display_on;

        // Sleep until the next frame (a slow tick while the monitor is off), but wake on any key.
        let frame_time = if power.display_on {
            Duration::from_micros(1_000_000 / power.fps(cfg.fps, cfg.battery_fps).max(1) as u64)
        } else {
            Duration::from_millis(500)
        };
        let spent = Instant::now().duration_since(now);
        let wait = frame_time.saturating_sub(spent);
        if event::poll(wait)? {
            loop {
                match event::read()? {
                    Event::Key(k) if k.kind == KeyEventKind::Press => {
                        quit = Quit::Key;
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
    // Farewell: persist, tell the others, then show the summary where the user acted.
    if primary {
        save.snapshot(&mut sess);
        save.store();
        sess.store_last();
        if let Some(sv) = &mut server {
            let msg = if matches!(quit, Quit::Wake) { "wake" } else { "stop" };
            sv.broadcast(&serde_json::json!({ "t": msg }));
        }
    } else if matches!(quit, Quit::Key)
        && let Some(c) = &mut client {
            c.send(&serde_json::json!({ "t": "stop" }));
        }
    if matches!(quit, Quit::Key | Quit::Wake) {
        if !primary {
            // The summary is written by the primary as it exits.
            let deadline = Instant::now() + Duration::from_secs(2);
            while lock.try_lock().is_err() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        if let Some(s) = Session::load_last()
            && s.secs >= 5.0 {
                let _ = cutscene::play(&cutscene::summary_text(&s), "waves", &stop);
            }
    }
    drop(server);
    drop(out);
    drop(term);
    Ok(())
}
