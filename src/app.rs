use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyEventKind},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use signal_hook::{consts::signal, flag};
use std::{
    fs::{File, OpenOptions},
    io::{self, BufWriter, Write},
    process::{Command, Stdio},
    collections::VecDeque,
    sync::{Arc, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::{
    config::Config,
    cutscene,
    gfx::{Frame, Overlay, Presenter, Rgb},
    ipc::{self, Client, Request, Server},
    power::Power,
    state::{self, GameEvent, Save, Session, ZONE_NAMES},
    theme::Palette,
    toast::Toasts,
    world::{Direction, Layout, SeaState, World, WorldEvent, compass_name, region_by_id, region_for_session},
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
fn layout_from_monitors(name: &str, frame_w: usize, monitors: &[serde_json::Value]) -> Option<Layout> {
    let me = monitors.iter().find(|m| m["name"].as_str() == Some(name))?;
    let own_width = me["width"].as_f64()?;
    if own_width <= 0.0 || frame_w < 2 {
        return None;
    }
    // Both terminals use the same font. Recover its physical cell width from this terminal,
    // then predict the other terminals' integer column counts. Height-based scaling can put
    // the next monitor dozens of frame pixels inside the previous one on wide displays.
    let cell_px = (own_width / (frame_w / 2) as f64).round().max(1.0);
    let mut sorted: Vec<_> = monitors.iter().filter(|m| m["x"].as_f64().is_some()).collect();
    sorted.sort_by(|a, b| a["x"].as_f64().unwrap().total_cmp(&b["x"].as_f64().unwrap()));
    let mut off = 0usize;
    let mut mine = None;
    let mut previous_right: Option<f64> = None;
    for m in sorted {
        let x = m["x"].as_f64()?;
        let scale = m["scale"].as_f64().unwrap_or(1.0).max(0.1);
        let width = m["width"].as_f64()?;
        if let Some(right) = previous_right {
            off += (((x - right).max(0.0) * scale / cell_px).round() as usize) * 2;
        }
        if m["name"].as_str() == Some(name) {
            mine = Some(off);
        }
        let cols = if m["name"].as_str() == Some(name) {
            frame_w / 2
        } else {
            (width / cell_px).floor() as usize
        };
        off += cols * 2;
        previous_right = Some(x + width / scale);
    }
    Some(Layout { pano_w: off, off: mine? })
}

fn layout(monitor: &Option<String>, frame_w: usize) -> Layout {
    let single = Layout::single(frame_w);
    let Some(name) = monitor else { return single };
    let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).stderr(Stdio::null()).output() else { return single };
    let Ok(list) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else { return single };
    layout_from_monitors(name, frame_w, &list).unwrap_or(single)
}

fn local_hm() -> String {
    Command::new("date")
        .arg("+%H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "--:--".to_owned())
}

fn epoch_secs() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

fn elapsed_hms(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    format!("{:02}:{:02}:{:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

fn hud_lines(save: &Save, pal: &Palette, cols: usize, clock: &str, elapsed: Duration, sea: SeaState) -> Vec<(String, Rgb)> {
    let zone = save.zone();
    let bar_len = 14;
    let filled = (save.progress() * bar_len as f64).round() as usize;
    let bar: String = (0..bar_len).map(|i| if i < filled { '█' } else { '░' }).collect();
    let phase = match sea.tide.direction { Direction::East => "enchendo", Direction::West => "vazando", Direction::Still => "parada" };
    let bearing = sea.bearing_deg.map(compass_name).unwrap_or("·");
    let arrow = match bearing { "N" => "↑", "NE" => "↗", "L" => "→", "SE" => "↘", "S" => "↓", "SO" => "↙", "O" => "←", "NO" => "↖", _ => "·" };
    let compass = format!("O {arrow} L");
    let speed = format!("{:.1}", sea.flow_knots.abs()).replace('.', ",");
    let tide_speed = format!("{:.1}", sea.tide.speed_knots).replace('.', ",");
    let temp = format!("{:.1}", sea.temperature_c).replace('.', ",");
    let details = if (elapsed.as_secs() / 8).is_multiple_of(2) {
        format!("Vento {:.0} nós · ondas {:.1} m · visib. {:.0} m", sea.wind_knots, sea.wave_m, sea.visibility_m)
    } else {
        format!("Água {temp} °C · salinidade {:.1} PSU · pressão {:.0} bar", sea.salinity_psu, sea.pressure_bar)
    }.replace('.', ",");
    let event = if sea.storm > 0.5 { "tempestade · " } else if sea.upwelling > 0.5 { "ressurgência · " } else { "" };
    let details = format!("{event}{details}");
    let lines = if cols >= 64 {
        vec![
            (format!("        ABYSS  ·  {clock}  ·  imersão {}", elapsed_hms(elapsed)), pal.text),
            (format!("  N   │ {}  ·  {}", sea.region.name, sea.region.ocean), pal.dim),
            (format!("{compass} │ Nv {} · {} · {} m · {bar} {:.0}/{:.0} XP", save.level(), ZONE_NAMES[zone], save.depth_m(), save.xp, state::xp_at(save.level() + 1)), pal.text),
            (format!("  S   │ ★ {}/{}  ·  {} pts  ·  série {} d", save.trophies.len(), state::TROPHIES.len(), save.points, save.streak), pal.gold),
            (format!("        Fluxo {bearing} {speed} nós · Maré {phase} {tide_speed} nós"), pal.dim),
            (format!("        {details}"), pal.dim),
        ]
    } else {
        let short_bar: String = bar.chars().take(8).collect();
        vec![
            (format!("        {clock}  ·  {}", elapsed_hms(elapsed)), pal.text),
            (format!("  N   │ {}", sea.region.name), pal.dim),
            (format!("{compass} │ Nv {} · {} {:.0} XP", save.level(), short_bar, save.xp), pal.text),
            (format!("  S   │ {} pts  ·  {} m", save.points, save.depth_m()), pal.gold),
            (format!("        {bearing} {speed} nós · Maré {phase}"), pal.dim),
            (format!("        {details}"), pal.dim),
        ]
    };
    lines.into_iter().map(|(line, color)| (line.chars().take(cols).collect(), color)).collect()
}

fn hud(overlay: &mut Overlay, save: &Save, pal: &Palette, cols: usize, clock: &str, elapsed: Duration, sea: SeaState) {
    let lines = hud_lines(save, pal, cols, clock, elapsed, sea);
    let panel_w = lines.iter().map(|(text, _)| text.chars().count()).max().unwrap_or(0).min(cols);
    let left = cols.saturating_sub(panel_w) / 2;
    for (row, (text, color)) in lines.iter().enumerate() {
        let trimmed: String = text.chars().take(panel_w).collect();
        let pad = panel_w.saturating_sub(trimmed.chars().count());
        let padded = format!("{}{}", trimmed, " ".repeat(pad));
        overlay.text(left, row, &padded, *color);
        if (1..=3).contains(&row) {
            let rose: String = padded.chars().take(5).collect();
            overlay.emphasized_text(left, row, &rose, pal.text);
        }
    }
    if panel_w >= 3 {
        let arrow = match sea.bearing_deg.map(compass_name) { Some("N") => "↑", Some("NE") => "↗", Some("L") => "→", Some("SE") => "↘", Some("S") => "↓", Some("SO") => "↙", Some("O") => "←", Some("NO") => "↖", _ => "·" };
        overlay.emphasized_text(left + 2, 2, arrow, pal.cyan);
    }
}

fn wait_for_primary_release(lock: &File, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if lock.try_lock().is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    false
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

fn state_message(save: &Save, started_at: f64) -> serde_json::Value {
    serde_json::json!({ "t": "state", "save": save, "started_at": started_at, "region": region_for_session(started_at).id })
}

fn send_state(server: &mut Option<Server>, save: &Save, started_at: f64) {
    if let Some(sv) = server
        && sv.peers() > 0 {
            sv.broadcast(&state_message(save, started_at));
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
    let started_at = epoch_secs();
    let mut journey_started_at = started_at;
    let mut session_started = Instant::now();
    let mut hud_clock = local_hm();
    let mut hud_clock_acc = 0.0f64;
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
    toasts.push_resume(&save);
    let season_of = |day: i64| if cfg.seasons { state::season_at(day) } else { None };
    let mut season = season_of(today);
    if let Some(se) = season {
        toasts.push_season(se);
    }

    let (mut cols, mut rows, mut pw, mut ph) = pixel_size();
    let mut world = World::new(pw, ph, layout(&opts.monitor, pw), save.zone_f(), save.reef, state::is_night(hour));
    world.set_journey(region_for_session(started_at), started_at);
    world.set_depth(save.depth_m());
    let mut active_pal = Palette::cosmetic(pal, &save.selected_cosmetic);
    world.set_palette(active_pal);
    toasts.set_palette(active_pal);
    world.set_season(season.map(|s| s.id));
    let mut frame = Frame::new(pw, ph);
    let mut presenter = Presenter::new(cols, rows);

    let quit: Quit;
    let mut pending: VecDeque<(String, &'static str)> = VecDeque::new();
    if !opts.skip_intro {
        pending.push_back((cutscene::intro_text(&save), "bubbles"));
    }

    let mut power = Power::read(&opts.monitor);
    let (mut last, mut sec_acc, mut save_acc, mut clock_acc, mut reload_acc, mut state_acc, mut power_acc, mut ipc_retry_acc) =
        (Instant::now(), 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64, 0.0f64);
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
                    Request::Joined => send_state(&mut server, &save, started_at),
                    Request::Wake => {
                        quit = Quit::Wake;
                        break 'main;
                    }
                    Request::Stop => {
                        quit = Quit::Signal;
                        break 'main;
                    }
                    Request::CosmeticBuy(id) => {
                        if save.buy_cosmetic(&id) {
                            save.store();
                            send_state(&mut server, &save, started_at);
                        }
                    }
                    Request::CosmeticSelect(id) => {
                        if save.select_cosmetic(&id) {
                            active_pal = Palette::cosmetic(pal, &save.selected_cosmetic);
                            world.set_palette(active_pal);
                            toasts.set_palette(active_pal);
                            save.store();
                            send_state(&mut server, &save, started_at);
                        }
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
                        if let Some(epoch) = m["started_at"].as_f64()
                            && epoch.is_finite() {
                                journey_started_at = epoch;
                                let elapsed = Duration::from_secs_f64((epoch_secs() - epoch).max(0.0));
                                session_started = Instant::now().checked_sub(elapsed).unwrap_or_else(Instant::now);
                            }
                        let region = m["region"].as_str().and_then(region_by_id).unwrap_or_else(|| region_for_session(journey_started_at));
                        world.set_journey(region, journey_started_at);
                        if let Ok(s) = serde_json::from_value::<Save>(m["save"].clone()) {
                            save = s;
                            save.no_seasons = !cfg.seasons;
                            active_pal = Palette::cosmetic(pal, &save.selected_cosmetic);
                            world.set_palette(active_pal);
                            toasts.set_palette(active_pal);
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
        // A secondary can start before the primary has bound its socket. Keep retrying instead of
        // permanently losing the shared shutdown signal after the initial connection timeout.
        if !primary && client.is_none() && ipc_retry_acc >= 1.0 {
            client = Client::connect(&sock, Duration::from_millis(100));
            ipc_retry_acc = 0.0;
        }

        let night = state::is_night(hour);
        world.set_depth(save.depth_m());
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
        ipc_retry_acc += dt;
        hud_clock_acc += dt;
        if hud_clock_acc >= 10.0 {
            hud_clock = local_hm();
            hud_clock_acc = 0.0;
        }
        if sec_acc >= 1.0 {
            if primary {
                let (zone, reef) = (save.zone(), save.reef);
                let events = save.tick(sec_acc, &mut sess, night, today);
                announce(&mut toasts, &mut server, events);
                // Level, zone and reef shape the scene: push them to the others right away.
                if save.zone() != zone || save.reef != reef {
                    send_state(&mut server, &save, started_at);
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
            send_state(&mut server, &save, started_at);
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
            hud(&mut overlay, &save, &pal, cols, &hud_clock, session_started.elapsed(), world.sea());
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
    // Farewell: persist and tell the other monitors before closing.
    if primary {
        save.snapshot(&mut sess);
        save.store();
        sess.store_last();
        if let Some(sv) = &mut server {
            let msg = if matches!(quit, Quit::Wake) { "wake" } else { "stop" };
            sv.broadcast(&serde_json::json!({ "t": msg }));
        }
    } else if matches!(quit, Quit::Key) {
        let msg = serde_json::json!({ "t": "stop" });
        if let Some(c) = &mut client {
            c.send(&msg);
        } else {
            // The server may have appeared just after a reconnect attempt timed out.
            let _ = ipc::send_once(&sock, &msg);
        }
    }
    if !primary && matches!(quit, Quit::Key | Quit::Wake) {
        // Keep the last underwater frame visible until the primary has saved the session and
        // released the lock. This prevents a blank monitor during the shutdown handoff.
        let _ = wait_for_primary_release(&lock, Duration::from_secs(5));
    }
    if matches!(quit, Quit::Wake)
        && let Some(s) = Session::load_last()
        && s.secs >= 5.0
    {
        let _ = cutscene::play(&cutscene::summary_text(&s), "waves", &stop);
    }
    drop(server);
    drop(out);
    drop(term);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn shared_state_carries_the_primary_journey() {
        let save = Save::default();
        let message = state_message(&save, 1_700_000_000.0);
        assert_eq!(message["region"], region_for_session(1_700_000_000.0).id);
        assert_eq!(message["started_at"], 1_700_000_000.0);
    }

    #[test]
    fn hud_names_active_storm_or_upwelling() {
        let save = Save::default();
        let pal = Palette::classic();
        let region = region_by_id("humboldt").unwrap();
        let event = (0..50_000).step_by(60).map(|t| SeaState::at(region, 0.0, t as f64, 10, 1.0))
            .find(|sea| sea.storm > 0.5 || sea.upwelling > 0.5).unwrap();
        let lines = hud_lines(&save, &pal, 100, "08:13", Duration::from_secs(8), event);
        assert!(lines.iter().any(|(line, _)| line.contains("tempestade") || line.contains("ressurgência")));
    }

    #[test]
    fn hud_shows_region_combined_flow_and_alternating_conditions() {
        let save = Save::default();
        let pal = Palette::classic();
        let region = crate::world::region_by_id("gulf-stream").unwrap();
        let sea = SeaState::at(region, 0.0, 11_175.0, save.depth_m(), 2.0);
        let east = hud_lines(&save, &pal, 100, "08:13", Duration::from_secs(64), sea);
        let content = east.iter().map(|(s, _)| s.as_str()).collect::<Vec<_>>().join(" ");
        assert!(content.contains("08:13") && content.contains("00:01:04"));
        assert!(content.contains("Corrente do Golfo"));
        assert!(content.contains("Fluxo") && content.contains("NE") && content.contains("Maré enchendo"));
        assert!(east[5].0.contains("Vento") && east[5].0.contains("ondas"));
        let details = hud_lines(&save, &pal, 100, "08:13", Duration::from_secs(72), sea);
        assert!(details[5].0.contains("Água") && details[5].0.contains("salinidade") && details[5].0.contains("pressão"));
        let narrow = hud_lines(&save, &pal, 40, "08:13", Duration::ZERO, sea);
        assert!(narrow.iter().all(|(s, _)| s.chars().count() <= 40));
    }

    #[test]
    fn panorama_geometry_is_shared_across_different_monitor_widths() {
        let monitors = [
            json!({"name":"DP-1", "width":1920, "height":1080, "scale":1.0, "x":-2426}),
            json!({"name":"HDMI-A-1", "width":2560, "height":1080, "scale":1.0, "x":-506}),
        ];
        let left = layout_from_monitors("DP-1", 548, &monitors).unwrap();
        let right = layout_from_monitors("HDMI-A-1", 730, &monitors).unwrap();
        assert_eq!(left.pano_w, right.pano_w);
        assert_eq!(left.pano_w, 1278);
        assert_eq!(left.off, 0);
        assert_eq!(right.off, 548);
    }

    #[test]
    fn secondary_keeps_waiting_until_primary_releases_the_lock() {
        let path = std::env::temp_dir().join(format!("abyss-lock-wait-{}", std::process::id()));
        let primary = OpenOptions::new().create(true).truncate(false).write(true).open(&path).unwrap();
        primary.try_lock().unwrap();
        let secondary = OpenOptions::new().create(true).truncate(false).write(true).open(&path).unwrap();
        let release = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            drop(primary);
        });
        let began = Instant::now();
        assert!(wait_for_primary_release(&secondary, Duration::from_secs(2)));
        assert!(began.elapsed() >= Duration::from_millis(75));
        release.join().unwrap();
        let _ = std::fs::remove_file(path);
    }
}
