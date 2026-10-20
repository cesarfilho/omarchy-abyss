mod app;
mod cutscene;
mod gfx;
mod sprites;
mod state;
mod toast;
mod world;

use gfx::{Frame, Overlay, Presenter};
use state::{Save, TROPHIES, ZONE_NAMES};

const USAGE: &str = "abyss: screensaver do fundo do mar

uso:
  abyss run [--screensaver] [--no-intro]   roda a cena
  abyss status [--json]                    nível, XP, série e troféus
  abyss mode [abyss|default|toggle]        escolhe qual screensaver o plugin dispara
  abyss quality [low|medium|high|ultra|max] resolução do screensaver (padrão: ultra)
  abyss wake                               acorda o screensaver (mostra o resumo)
  abyss snapshot [zona 0-5] [reef 0-30]    imprime um quadro ANSI e sai";

fn status_json() {
    let s = Save::load();
    println!("{}", s.stats_json(state::Session::load_last().as_ref()));
}

fn mode_path() -> std::path::PathBuf {
    state::state_dir().join("mode")
}

fn current_mode() -> String {
    match std::fs::read_to_string(mode_path()).map(|m| m.trim().to_string()).as_deref() {
        Ok("default") => "default".into(),
        _ => "abyss".into(),
    }
}

const QUALITIES: [(&str, f32); 5] = [("low", 18.0), ("medium", 14.0), ("high", 12.0), ("ultra", 10.5), ("max", 9.0)];

fn current_quality() -> (&'static str, f32) {
    let saved = std::fs::read_to_string(state::state_dir().join("quality")).unwrap_or_default();
    QUALITIES.iter().copied().find(|(n, _)| *n == saved.trim()).unwrap_or(QUALITIES[3])
}

/// `abyss quality [low|medium|high|ultra|max]`: screensaver resolution (terminal font size; smaller = finer grid).
fn quality(args: &[String]) {
    if let Some(&(name, _)) = args.first().and_then(|a| QUALITIES.iter().find(|(n, _)| n == a)) {
        let _ = std::fs::create_dir_all(state::state_dir());
        let _ = std::fs::write(state::state_dir().join("quality"), name);
    }
    println!("{}", current_quality().0);
}

/// `abyss mode [abyss|default|toggle]`: which screensaver the idle plugin launches.
fn mode(args: &[String]) {
    let new = match args.first().map(String::as_str) {
        Some("abyss") => Some("abyss"),
        Some("default") => Some("default"),
        Some("toggle") => Some(if current_mode() == "abyss" { "default" } else { "abyss" }),
        _ => None,
    };
    if let Some(m) = new {
        let _ = std::fs::create_dir_all(state::state_dir());
        let _ = std::fs::write(mode_path(), m);
    }
    println!("{}", current_mode());
}

fn status() {
    let s = Save::load();
    let l = s.level();
    println!("Nível {} · {} · {} m", l, ZONE_NAMES[s.zone()], s.depth_m());
    println!("XP {:.0}/{:.0} ({:.0}%)", s.xp, state::xp_at(l + 1), s.progress() * 100.0);
    println!("{} pontos · série de {} dia(s) · recife {}/{} · {} baleia(s)", s.points, s.streak, s.reef, state::REEF_MAX, s.whales);
    println!("Tempo no mar: {:.1} h · sessões: {}", s.total_secs / 3600.0, s.sessions);
    println!("\nTroféus {}/{}", s.trophies.len(), TROPHIES.len());
    for t in TROPHIES {
        println!("  {} {:<24} {}", if s.has(t.id) { "★" } else { "·" }, t.name, t.desc);
    }
}

fn snapshot(args: &[String]) {
    let zone: f32 = args.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let reef: u32 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(12);
    let (cols, rows) = (140usize, 40usize);
    let mut world = world::World::new(cols * 2, rows * 4, world::Layout::single(cols * 2), zone, reef, false);
    for _ in 0..200 {
        world.update(0.1, zone, reef, false);
    }
    let mut frame = Frame::new(cols * 2, rows * 4);
    world.draw(&mut frame);
    let mut out = std::io::stdout().lock();
    let _ = Presenter::new(cols, rows).present(&frame, &Overlay::new(cols, rows), &mut out);
    println!();
}

fn main() -> std::io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("run") => {
            let flag = |f: &str| args.iter().any(|a| a == f);
            app::run(app::Options {
                screensaver: flag("--screensaver"),
                // ttfx paints on a black canvas for several seconds; as a screensaver the scene must show at once.
                monitor: args.iter().position(|a| a == "--monitor").and_then(|i| args.get(i + 1)).cloned(),
                skip_intro: flag("--no-intro") || flag("--screensaver") || std::env::var_os("ABYSS_NO_INTRO").is_some(),
            })
        }
        Some("status") => {
            if args.iter().any(|a| a == "--json") { status_json() } else { status() }
            Ok(())
        }
        Some("quality") => {
            quality(&args[1..]);
            Ok(())
        }
        Some("font-size") => {
            println!("{}", current_quality().1);
            Ok(())
        }
        Some("mode") => {
            mode(&args[1..]);
            Ok(())
        }
        Some("wake") => {
            let _ = std::process::Command::new("pkill").args(["-USR1", "-f", "^abyss run"]).status();
            Ok(())
        }
        Some("snapshot") => {
            snapshot(&args[1..]);
            Ok(())
        }
        _ => {
            println!("{USAGE}");
            Ok(())
        }
    }
}
