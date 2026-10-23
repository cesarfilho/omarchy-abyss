#![allow(clippy::too_many_arguments)] // sprite drawing passes frame, geometry and style explicitly
mod app;
mod config;
mod cutscene;
mod doctor;
mod gfx;
mod ipc;
mod path;
mod power;
mod shark;
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
  abyss idle [segundos]                    tempo de inatividade até iniciar o screensaver (padrão: 150)
  abyss cosmetic buy|select ID             compra ou seleciona uma paleta cosmética
  abyss start                              inicia o screensaver agora
  abyss reset                              apaga estatísticas, XP e conquistas
  abyss doctor [--fix]                     confere a instalação (e limpa restos de uma falha)
  abyss config                             mostra a configuração (~/.config/abyss/config.json)
  abyss snapshot [zona 0-5] [reef 0-30] [--ppm ARQ] [--t SEG] [--size LxA] [--season ID] [--landmark ID] [--region ID]
                                           desenha um quadro (ANSI, ou imagem PPM sem tocar a tela)";

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

/// `abyss idle [SECONDS]`: inactivity time before the idle service launches the screensaver.
fn idle(args: &[String]) {
    let mut cfg = Config::load();
    if let Some(n) = args.first().and_then(|a| a.parse::<u32>().ok()) {
        cfg.idle_secs = n;
        cfg = cfg.sanitized();
        cfg.store();
    }
    println!("{}", cfg.idle_secs);
}

/// `abyss reset`: stops a running screensaver, then starts the game over (XP, trophies, history).
fn reset() {
    if !ipc::send_once(&ipc::sock_path(), &serde_json::json!({ "t": "stop" })) {
        let _ = std::process::Command::new("pkill").args(["-TERM", "-f", "^abyss run"]).status();
    }
    std::thread::sleep(std::time::Duration::from_millis(600));
    state::Session::clear_last();
    Save::default().store();
    println!("estatísticas e conquistas apagadas");
}

fn cosmetic(args: &[String]) {
    let (Some(action), Some(id)) = (args.first().map(String::as_str), args.get(1)) else {
        eprintln!("uso: abyss cosmetic buy|select ID");
        return;
    };
    let req = match action {
        "buy" => serde_json::json!({ "t": "cosmetic_buy", "id": id }),
        "select" => serde_json::json!({ "t": "cosmetic_select", "id": id }),
        _ => { eprintln!("ação inválida"); return; }
    };
    if !ipc::send_once(&ipc::sock_path(), &req) {
        let mut save = Save::load();
        let changed = match action { "buy" => save.buy_cosmetic(id), "select" => save.select_cosmetic(id), _ => false };
        if changed { save.store(); }
        println!("{}", if changed { "ok" } else { "indisponível: saldo insuficiente ou paleta bloqueada" });
    } else {
        println!("pedido enviado");
    }
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
        println!("\nEvento visual: {}", se.name);
    }
    println!("\nTroféus {}/{}", s.trophies.len(), TROPHIES.len());
    for t in TROPHIES {
        println!("  {} {:<24} {}", if s.has(t.id) { "★" } else { "·" }, t.name, t.desc);
    }
}

/// `abyss snapshot [zone] [reef] [--ppm FILE] [--t SECONDS] [--size WxH] [--season ID] [--region ID]`: renders one
/// frame without a terminal. Without `--ppm` it prints the frame as ANSI; with it, a PPM image
/// (convert with `magick FILE out.png`), which never touches the screen.
fn snapshot(args: &[String]) {
    let opt = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    let nums: Vec<&String> = {
        let mut skip = false;
        args.iter()
            .filter(|a| {
                if skip {
                    skip = false;
                    return false;
                }
                if a.starts_with("--") {
                    skip = true;
                    return false;
                }
                true
            })
            .collect()
    };
    let zone: f32 = nums.first().and_then(|v| v.parse().ok()).unwrap_or(1.0);
    let reef: u32 = nums.get(1).and_then(|v| v.parse().ok()).unwrap_or(12);
    let t: f64 = opt("--t").and_then(|v| v.parse().ok()).unwrap_or(1_700_000_000.0);
    let season = opt("--season").and_then(|id| state::season_info(&id)).map(|s| s.id);
    let ppm = opt("--ppm");
    let (w, h) = match (&ppm, opt("--size").and_then(|s| s.split_once('x').map(|(a, b)| (a.parse().ok(), b.parse().ok())))) {
        (_, Some((Some(w), Some(h)))) => (w, h),
        (Some(_), _) => (1280usize, 720usize),
        _ => (280, 160),
    };
    let mut world = world::World::new(w, h, world::Layout::single(w), zone, reef, false).at(t);
    let depths = [0.0, 50.0, 200.0, 1000.0, 4000.0, 6000.0, 11000.0];
    let zi = (zone.floor() as usize).min(5);
    world.set_depth((depths[zi] + (depths[zi + 1] - depths[zi]) * (zone - zi as f32).clamp(0.0, 1.0) as f64) as u32);
    if let Some(region) = opt("--region").and_then(|id| world::region_by_id(&id)) {
        world.set_journey(region, (t / 3600.0).floor() * 3600.0);
    }
    world.set_palette(theme::Palette::classic());
    world.set_season(season);
    if let Some(id) = opt("--landmark") {
        world.force_landmark(&id);
    }
    world.update(0.0, zone, reef, false);
    let mut frame = Frame::new(w, h);
    world.draw(&mut frame);
    if let Some(path) = ppm {
        let mut data = format!("P6\n{w} {h}\n255\n").into_bytes();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&frame.at(x, y));
            }
        }
        if let Err(e) = std::fs::write(&path, data) {
            eprintln!("abyss: cannot write {path}: {e}");
            std::process::exit(1);
        }
        return;
    }
    let (cols, rows) = (w / 2, h / 4);
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
        Some("idle") => {
            idle(&args[1..]);
            Ok(())
        }
        Some("cosmetic") => { cosmetic(&args[1..]); Ok(()) }
        Some("start") => {
            let _ = std::process::Command::new("abyss-screensaver").spawn();
            Ok(())
        }
        Some("reset") => {
            reset();
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
