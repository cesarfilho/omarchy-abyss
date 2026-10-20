#![allow(clippy::too_many_arguments)] // sprite drawing passes frame, geometry and style explicitly
mod app;
mod config;
mod cutscene;
mod doctor;
mod gfx;
mod ipc;
mod power;
mod sprites;
mod state;
mod theme;
mod toast;
mod world;

use config::{Config, QUALITIES};
use gfx::{Frame, Overlay, Presenter};
use state::{Save, TROPHIES, ZONE_NAMES};

const USAGE: &str = "abyss: screensaver do fundo do mar

uso:
  abyss run [--screensaver] [--no-intro]   roda a cena
  abyss status [--json]                    nível, XP, série e troféus
  abyss mode [abyss|default|toggle]        escolhe qual screensaver o plugin dispara
  abyss quality [low|medium|high|ultra|max] resolução do screensaver (padrão: ultra)
  abyss wake                               acorda o screensaver (mostra o resumo)
  abyss stop                               encerra o screensaver sem resumo
  abyss doctor [--fix]                     confere a instalação (e limpa restos de uma falha)
  abyss config                             mostra a configuração (~/.config/abyss/config.json)
  abyss snapshot [zona 0-5] [reef 0-30]    imprime um quadro ANSI e sai";

fn status_json() {
    let s = Save::load();
    println!("{}", s.stats_json(state::Session::load_last().as_ref()));
}

/// `abyss quality [low|medium|high|ultra|max]`: screensaver resolution (terminal font size; smaller = finer grid).
fn quality(args: &[String]) {
    let mut cfg = Config::load();
    if let Some(&(name, _)) = args.first().and_then(|a| QUALITIES.iter().find(|(n, _)| n == a)) {
        cfg.quality = name.into();
        cfg.store();
    }
    println!("{}", cfg.quality);
}

/// `abyss mode [abyss|default|toggle]`: which screensaver the idle plugin launches.
fn mode(args: &[String]) {
    let mut cfg = Config::load();
    let new = match args.first().map(String::as_str) {
        Some("abyss") => Some("abyss"),
        Some("default") => Some("default"),
        Some("toggle") => Some(if cfg.mode == "abyss" { "default" } else { "abyss" }),
        _ => None,
    };
    if let Some(m) = new {
        cfg.mode = m.into();
        cfg.store();
    }
    println!("{}", cfg.mode);
}

fn status() {
    let s = Save::load();
    let l = s.level();
    println!("Nível {} · {} · {} m", l, ZONE_NAMES[s.zone()], s.depth_m());
    println!("XP {:.0}/{:.0} ({:.0}%)", s.xp, state::xp_at(l + 1), s.progress() * 100.0);
    println!("{} pontos · série de {} dia(s) · recife {}/{} · {} baleia(s)", s.points, s.streak, s.reef, state::REEF_MAX, s.whales);
    println!("Tempo no mar: {:.1} h · sessões: {}", s.total_secs / 3600.0, s.sessions);
    println!("\nBestiário {}/{}", s.bestiary.len(), state::BESTIARY.len());
    for c in state::BESTIARY {
        println!("  {} {}", if s.bestiary.iter().any(|b| b == c.id) { "◆" } else { "·" }, c.name);
    }
    if !s.quests.is_empty() {
        println!("\nMissões de hoje");
        for id in &s.quests {
            if let Some(q) = state::quest(id) {
                println!("  {} {:<18} {}", if s.quest_done.contains(id) { "✔" } else { "·" }, q.name, q.desc);
            }
        }
    }
    if let Some(se) = state::season_at(s.last_day) {
        println!("\nEvento: {} (XP ×{})", se.name, state::SEASON_XP_BOOST);
    }
    println!("\nTroféus {}/{}", s.trophies.len(), TROPHIES.len());
    for t in TROPHIES {
        println!("  {} {:<24} {}", if s.has(t.id) { "★" } else { "·" }, t.name, t.desc);
    }
}

fn snapshot(args: &[String]) {
    let zone: f32 = args.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let reef: u32 = args.get(1).and_then(|v| v.parse().ok()).unwrap_or(12);
    let (cols, rows) = (140usize, 40usize);
    let mut world = world::World::new(cols * 2, rows * 4, world::Layout::single(cols * 2), zone, reef, false).at(1_700_000_000.0);
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
            println!("{}", Config::load().font_size());
            Ok(())
        }
        Some("mode") => {
            mode(&args[1..]);
            Ok(())
        }
        Some("wake") => {
            // The socket reaches every monitor; SIGUSR1 is the fallback for an instance that has none.
            if !ipc::send_once(&ipc::sock_path(), &serde_json::json!({ "t": "wake" })) {
                let _ = std::process::Command::new("pkill").args(["-USR1", "-f", "^abyss run"]).status();
            }
            Ok(())
        }
        Some("stop") => {
            if !ipc::send_once(&ipc::sock_path(), &serde_json::json!({ "t": "stop" })) {
                let _ = std::process::Command::new("pkill").args(["-TERM", "-f", "^abyss run"]).status();
            }
            Ok(())
        }
        Some("doctor") => {
            if args.iter().any(|a| a == "--fix") {
                for line in doctor::fix() {
                    println!("• {line}");
                }
            }
            if !doctor::print(&doctor::run_checks()) {
                std::process::exit(1);
            }
            Ok(())
        }
        Some("config") => {
            println!("{}", serde_json::to_string_pretty(&Config::load()).unwrap_or_default());
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
