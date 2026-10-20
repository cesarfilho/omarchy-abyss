//! User settings in `~/.config/abyss/config.json` (`ABYSS_CONFIG_DIR` overrides the folder).
//! The shell plugin watches the same file, so `abyss mode|quality` and the panel stay in sync.

use serde::{Deserialize, Serialize};
use std::{fs, path::{Path, PathBuf}};

use crate::state;

/// (name, terminal font size). A smaller font means a finer pixel grid.
pub const QUALITIES: [(&str, f32); 5] = [("low", 18.0), ("medium", 14.0), ("high", 12.0), ("ultra", 10.5), ("max", 9.0)];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    /// Which screensaver the idle plugin launches: "abyss" or "default".
    pub mode: String,
    pub quality: String,
    /// Frame rate on AC power and while discharging.
    pub fps: u32,
    pub battery_fps: u32,
    /// "theme" tints the sea and the HUD with the active Omarchy theme; "classic" keeps the original colours.
    pub palette: String,
    /// Seasonal events (Halloween, Christmas, Oceans Day).
    pub seasons: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { mode: "abyss".into(), quality: "ultra".into(), fps: 30, battery_fps: 15, palette: "theme".into(), seasons: true }
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ABYSS_CONFIG_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".config"));
    base.join("abyss")
}

fn path() -> PathBuf {
    config_dir().join("config.json")
}

impl Config {
    pub fn sanitized(mut self) -> Self {
        if self.mode != "default" {
            self.mode = "abyss".into();
        }
        if !QUALITIES.iter().any(|(n, _)| *n == self.quality) {
            self.quality = "ultra".into();
        }
        self.fps = self.fps.clamp(5, 60);
        self.battery_fps = self.battery_fps.clamp(5, 60);
        if self.palette != "classic" {
            self.palette = "theme".into();
        }
        self
    }

    /// Reads the config; the older one-value files (`mode`, `quality` in the state folder) are
    /// migrated the first time and then removed.
    pub fn load() -> Self {
        Self::load_from(&path(), &state::state_dir())
    }

    pub fn load_from(file: &Path, legacy_dir: &Path) -> Self {
        if let Some(c) = fs::read_to_string(file).ok().and_then(|t| serde_json::from_str::<Config>(&t).ok()) {
            return c.sanitized();
        }
        let mut c = Config::default();
        let mut migrated = false;
        for key in ["mode", "quality"] {
            let old = legacy_dir.join(key);
            if let Ok(v) = fs::read_to_string(&old) {
                match key {
                    "mode" => c.mode = v.trim().into(),
                    _ => c.quality = v.trim().into(),
                }
                let _ = fs::remove_file(old);
                migrated = true;
            }
        }
        let c = c.sanitized();
        if migrated {
            c.store_to(file);
        }
        c
    }

    pub fn store(&self) {
        self.store_to(&path());
    }

    pub fn store_to(&self, file: &Path) {
        if let Some(dir) = file.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let tmp = file.with_extension(format!("tmp{}", std::process::id()));
        if let Ok(s) = serde_json::to_string_pretty(self)
            && fs::write(&tmp, s + "\n").is_ok() {
                let _ = fs::rename(&tmp, file);
            }
    }

    pub fn font_size(&self) -> f32 {
        QUALITIES.iter().find(|(n, _)| *n == self.quality).map(|q| q.1).unwrap_or(10.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("abyss-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn defaults_when_missing() {
        let d = tmp("cfg-missing");
        assert_eq!(Config::load_from(&d.join("config.json"), &d), Config::default());
    }

    #[test]
    fn migrates_legacy_files() {
        let d = tmp("cfg-legacy");
        fs::write(d.join("mode"), "default\n").unwrap();
        fs::write(d.join("quality"), "max").unwrap();
        let file = d.join("config.json");
        let c = Config::load_from(&file, &d);
        assert_eq!((c.mode.as_str(), c.quality.as_str()), ("default", "max"));
        assert!(file.exists(), "migrated config is written");
        assert!(!d.join("mode").exists() && !d.join("quality").exists(), "legacy files are removed");
        assert_eq!(Config::load_from(&file, &d), c);
    }

    #[test]
    fn invalid_values_fall_back() {
        let d = tmp("cfg-invalid");
        let file = d.join("config.json");
        fs::write(&file, r#"{"mode":"x","quality":"huge","fps":500,"palette":"neon"}"#).unwrap();
        let c = Config::load_from(&file, &d);
        assert_eq!((c.mode.as_str(), c.quality.as_str(), c.fps, c.palette.as_str()), ("abyss", "ultra", 60, "theme"));
        assert_eq!(c.font_size(), 10.5);
    }
}
