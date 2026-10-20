use crossterm::event::{self, Event, KeyEventKind};
use std::{
    fs,
    io::{self, Write},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use crate::state::{self, Save, Session, ZONE_NAMES};

/// Centers each line against the widest one so ttfx (which left-aligns a text block) looks centered.
pub fn banner(lines: &[String]) -> String {
    let width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
    lines
        .iter()
        .map(|l| {
            let pad = (width - l.chars().count()) / 2;
            format!("{}{}", " ".repeat(pad), l)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn spaced(s: &str) -> String {
    s.to_uppercase().chars().map(String::from).collect::<Vec<_>>().join(" ")
}

pub fn intro_text(save: &Save) -> String {
    banner(&[
        spaced("abyss"),
        String::new(),
        format!("{} · {} m", ZONE_NAMES[save.zone()], save.depth_m()),
        format!("nível {}", save.level()),
    ])
}

fn fmt_dur(secs: f64) -> String {
    let s = secs as u64;
    if s >= 3600 {
        format!("{} h {:02} min", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{} min", s / 60)
    } else {
        format!("{s} s")
    }
}

pub fn summary_text(sess: &Session) -> String {
    let mut lines = vec![
        spaced("enquanto você estava fora"),
        String::new(),
        format!("{} no fundo do mar", fmt_dur(sess.secs)),
        format!("+{:.0} XP · nível {} · {} m", sess.xp_gain, sess.level, sess.depth_m),
    ];
    if sess.levels_gained > 0 {
        lines.push(format!("{} nível(is) conquistado(s)", sess.levels_gained));
    }
    if sess.whales > 0 {
        lines.push(format!("{} baleia(s) avistada(s)", sess.whales));
    }
    if sess.reef_gain > 0 {
        lines.push(format!("{} coral(is) novo(s) no recife", sess.reef_gain));
    }
    for id in &sess.discoveries {
        if let Some(c) = state::creature(id) {
            lines.push(format!("◆ {}", c.name));
        }
    }
    for id in &sess.quests_done {
        if let Some(q) = state::quest(id) {
            lines.push(format!("✔ {}", q.name));
        }
    }
    for id in &sess.trophies {
        if let Some(t) = state::trophy(id) {
            lines.push(format!("★ {}", t.name));
        }
    }
    lines.push(String::new());
    lines.push(format!("{} pontos · série de {} dia(s)", sess.points, sess.streak));
    lines.push(format!("{}/{} troféus · {}/{} criaturas", sess.trophies_total, state::TROPHIES.len(), sess.bestiary_total, state::BESTIARY.len()));
    banner(&lines)
}

/// Longest a cutscene may run. If the terminal vanishes under it (the window was closed) ttfx
/// and this loop would otherwise spin forever.
const MAX_SECS: u64 = 25;

/// Plays one ttfx effect over the whole terminal. Returns true when a key was pressed
/// (the effect is killed and the caller should wake up). `stop` is the SIGTERM/SIGHUP flag.
pub fn play(text: &str, effect: &str, stop: &AtomicBool) -> io::Result<bool> {
    let dir = state::state_dir();
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("cutscene.{}.txt", std::process::id()));
    fs::write(&path, text)?;

    let mut out = io::stdout();
    out.write_all(b"\x1b[0m\x1b[2J\x1b[H")?;
    out.flush()?;

    let child = Command::new("ttfx")
        .args(["-i"])
        .arg(&path)
        .args([
            "--frame-rate", "60", "--canvas-width", "0", "--canvas-height", "0", "--reuse-canvas",
            "--anchor-canvas", "c", "--anchor-text", "c", "--no-eol", "--no-restore-cursor", effect,
        ])
        .stdin(Stdio::null())
        .spawn();
    let Ok(mut child) = child else {
        let _ = fs::remove_file(&path);
        return Ok(false);
    };
    let mut woke = false;
    let deadline = Instant::now() + Duration::from_secs(MAX_SECS);
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if stop.load(Ordering::Relaxed) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break;
        }
        let polled = Instant::now();
        if event::poll(Duration::from_millis(40))?
            && let Event::Key(k) = event::read()?
                && k.kind == KeyEventKind::Press {
                    woke = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
        // A hung-up terminal makes poll return at once; do not turn that into a busy loop.
        if polled.elapsed() < Duration::from_millis(5) {
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    let _ = fs::remove_file(&path);
    out.write_all(b"\x1b[0m\x1b[2J\x1b[H")?;
    out.flush()?;
    Ok(woke)
}
