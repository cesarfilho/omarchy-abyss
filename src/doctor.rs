//! `abyss doctor`: checks the pieces Abyss depends on and says what to do about the ones that are off.

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use crate::{config::Config, ipc, state};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Level {
    Good,
    Warn,
    Fail,
}

pub struct Check {
    pub level: Level,
    pub name: &'static str,
    pub detail: String,
}

fn check(level: Level, name: &'static str, detail: impl Into<String>) -> Check {
    Check { level, name, detail: detail.into() }
}

fn home() -> PathBuf {
    PathBuf::from(std::env::var("HOME").unwrap_or_default())
}

pub fn which(cmd: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join(cmd)).find(|p| p.is_file())
}

fn sha256(path: &Path) -> Option<String> {
    let out = Command::new("sha256sum").arg(path).stderr(Stdio::null()).output().ok()?;
    String::from_utf8(out.stdout).ok()?.split_whitespace().next().map(String::from)
}

/// The stock launcher `abyss-launch` was derived from; `abyss-setup` records its hash.
pub fn upstream_launcher() -> Option<PathBuf> {
    which("omarchy-launch-screensaver")
}

pub fn upstream_record() -> PathBuf {
    crate::config::config_dir().join("upstream.sha256")
}

fn json_at(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

pub fn run_checks() -> Vec<Check> {
    use Level::{Fail, Good, Warn};
    let mut out = Vec::new();

    match which("abyss") {
        Some(p) => out.push(check(Good, "binário", format!("{} (v{})", p.display(), env!("CARGO_PKG_VERSION")))),
        None => out.push(check(Fail, "binário", "`abyss` não está no PATH (rode ./install.sh)")),
    }
    for l in ["abyss-launch", "abyss-screensaver"] {
        out.push(match which(l) {
            Some(p) => check(Good, "lançador", format!("{l}: {}", p.display())),
            None => check(Fail, "lançador", format!("{l} não está no PATH (rode ./install.sh)")),
        });
    }

    // abyss-launch is a copy of the stock launcher with Abyss hooks; flag when the stock one moved on.
    out.push(match (upstream_launcher().and_then(|p| sha256(&p)), fs::read_to_string(upstream_record()).ok()) {
        (Some(now), Some(then)) if then.trim() == now => check(Good, "lançador padrão", "igual ao que o abyss-launch copiou"),
        (Some(_), Some(_)) => check(
            Warn,
            "lançador padrão",
            "omarchy-launch-screensaver mudou desde a cópia: compare com plugin/abyss-launch (scripts/diff-launch.sh) e rode abyss-setup record-upstream",
        ),
        (Some(_), None) => check(Warn, "lançador padrão", "sem registro da versão copiada: rode abyss-setup record-upstream"),
        (None, _) => check(Warn, "lançador padrão", "omarchy-launch-screensaver não encontrado"),
    });

    for tool in ["hyprctl", "ttfx", "socat", "jq", "xdg-terminal-exec"] {
        out.push(match which(tool) {
            Some(_) => check(Good, "dependência", tool),
            None => check(if tool == "ttfx" { Warn } else { Fail }, "dependência", format!("{tool} não encontrado")),
        });
    }
    if let Ok(o) = Command::new("xdg-terminal-exec").arg("--print-id").stderr(Stdio::null()).output() {
        let id = String::from_utf8_lossy(&o.stdout).to_string();
        let ok = ["Alacritty", "ghostty", "foot", "kitty"].iter().any(|t| id.contains(t));
        out.push(check(if ok { Good } else { Fail }, "terminal", format!("{} {}", id.trim(), if ok { "" } else { "(só Alacritty, Foot, Ghostty e Kitty)" })));
    }
    let truecolor = std::env::var("COLORTERM").map(|v| v == "truecolor" || v == "24bit").unwrap_or(false);
    if !truecolor {
        out.push(check(Warn, "cores", "COLORTERM não indica 24 bits (normal fora de um terminal; o screensaver usa truecolor)"));
    }

    // Omarchy integration.
    let shell = home().join(".config/omarchy/shell.json");
    out.push(match json_at(&shell).map(|j| j["idle"]["screensaver"].as_i64()) {
        Some(Some(0)) => check(Good, "ociosidade", "idle.screensaver = 0 (o plugin dispara o screensaver)"),
        Some(v) => check(Warn, "ociosidade", format!("idle.screensaver = {v:?}: o do Omarchy dispara junto; rode abyss-setup enable")),
        None => check(Warn, "ociosidade", format!("não consegui ler {}", shell.display())),
    });
    let menu = home().join(".config/omarchy/extensions/omarchy-menu.jsonc");
    out.push(if fs::read_to_string(&menu).is_ok_and(|t| t.contains("abyss-menu-start")) {
        check(Good, "menu", "item Screensaver aponta para o Abyss")
    } else {
        check(Warn, "menu", "o item Screensaver do menu abre o padrão; rode abyss-setup enable")
    });
    let plugin = home().join(".config/omarchy/plugins/io.github.cesarfilho.abyss");
    out.push(if plugin.join("manifest.json").is_file() {
        check(Good, "plugin", plugin.display().to_string())
    } else {
        check(Fail, "plugin", "plugin não instalado (rode ./install.sh)")
    });
    if home().join(".local/state/omarchy/indicators/stay-awake").exists() {
        out.push(check(Warn, "stay-awake", "ligado: o screensaver nunca dispara por ociosidade"));
    }

    // Abyss's own files.
    let dir = state::state_dir();
    let writable = fs::create_dir_all(&dir).is_ok() && fs::write(dir.join(".doctor"), "").is_ok();
    let _ = fs::remove_file(dir.join(".doctor"));
    out.push(if writable { check(Good, "dados", dir.display().to_string()) } else { check(Fail, "dados", format!("{} não é gravável", dir.display())) });
    let save = dir.join("save.json");
    if save.exists() {
        out.push(match json_at(&save) {
            Some(_) => check(Good, "save", "save.json legível"),
            None => check(Fail, "save", "save.json ilegível: o Abyss o separa como save.json.corrupt-* ao abrir"),
        });
    }
    let corrupt = fs::read_dir(&dir).map(|d| d.flatten().filter(|e| e.file_name().to_string_lossy().contains(".corrupt-")).count()).unwrap_or(0);
    if corrupt > 0 {
        out.push(check(Warn, "save", format!("{corrupt} save(s) corrompido(s) guardados em {}", dir.display())));
    }
    let cfg = Config::load();
    out.push(check(Good, "config", format!("modo {} · qualidade {} · {} fps ({} na bateria) · paleta {}", cfg.mode, cfg.quality, cfg.fps, cfg.battery_fps, cfg.palette)));
    if dir.join("crash.log").exists() {
        out.push(check(Warn, "crash.log", format!("há falhas registradas em {}", dir.join("crash.log").display())));
    }

    let sock = ipc::sock_path();
    out.push(if ipc::send_once(&sock, &serde_json::json!({ "t": "ping" })) {
        check(Good, "instância", "um screensaver Abyss está rodando")
    } else if sock.exists() {
        check(Warn, "instância", "socket sobrando sem processo (abyss doctor --fix remove)")
    } else {
        check(Good, "instância", "nenhum screensaver rodando")
    });
    out
}

/// Cleans up what a crash can leave behind. Returns what it did.
pub fn fix() -> Vec<String> {
    let mut done = Vec::new();
    // Only when nobody holds the lock is the socket (and a hidden pointer) really orphaned.
    let lock = fs::OpenOptions::new().create(true).write(true).truncate(false).open(state::state_dir().join("lock"));
    if let Ok(lock) = lock {
        if lock.try_lock().is_ok() {
            let sock = ipc::sock_path();
            if sock.exists() && fs::remove_file(&sock).is_ok() {
                done.push(format!("removido {}", sock.display()));
            }
            if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
                let ok = Command::new("hyprctl")
                    .args(["eval", "hl.config({ cursor = { invisible = false } })"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .is_ok_and(|s| s.success())
                    || Command::new("hyprctl")
                        .args(["keyword", "cursor:invisible", "false"])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()
                        .is_ok_and(|s| s.success());
                if ok {
                    done.push("ponteiro do mouse visível".into());
                }
            }
        } else {
            done.push("um screensaver está rodando: nada a limpar".into());
        }
    }
    done
}

pub fn print(checks: &[Check]) -> bool {
    let mut bad = false;
    for c in checks {
        let mark = match c.level {
            Level::Good => "✓",
            Level::Warn => "!",
            Level::Fail => {
                bad = true;
                "✗"
            }
        };
        println!("{mark} {:<16} {}", c.name, c.detail);
    }
    !bad
}
