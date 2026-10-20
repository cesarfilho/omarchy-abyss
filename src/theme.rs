//! Colours for the sea and the HUD. "classic" is the original look; "theme" nudges the water,
//! plankton glow and HUD text toward the active Omarchy theme (`colors.toml`).

use std::{collections::HashMap, path::PathBuf};

use crate::gfx::{Rgb, lerp};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    /// Colours the water is pulled toward (shallow end, deep end) and how strongly.
    pub water: Option<(Rgb, Rgb)>,
    pub glow: (Rgb, Rgb),
    pub text: Rgb,
    pub dim: Rgb,
    pub gold: Rgb,
    pub cyan: Rgb,
}

impl Palette {
    pub const fn classic() -> Self {
        Self {
            water: None,
            glow: ([70, 255, 200], [90, 160, 255]),
            text: [235, 240, 235],
            dim: [160, 190, 210],
            gold: [255, 220, 120],
            cyan: [140, 230, 246],
        }
    }

    /// Builds a palette from theme colours (`name -> #rrggbb`). Missing keys fall back to classic.
    pub fn from_colors(c: &HashMap<String, Rgb>) -> Self {
        let base = Self::classic();
        let get = |keys: &[&str]| keys.iter().find_map(|k| c.get(*k).copied());
        let fg = get(&["foreground"]);
        let bg = get(&["dark_background", "background"]);
        let blue = get(&["bright_blue", "blue"]);
        let cyan = get(&["bright_cyan", "cyan"]);
        let gold = get(&["accent", "yellow", "bright_yellow"]);
        let water = match (blue, bg) {
            (Some(b), Some(d)) => Some((b, d)),
            _ => None,
        };
        let text = fg.map(|f| lerp(f, [255, 255, 255], 0.35)).unwrap_or(base.text);
        Self {
            water,
            glow: (cyan.map(|c| lerp(base.glow.0, c, 0.6)).unwrap_or(base.glow.0), blue.map(|b| lerp(base.glow.1, b, 0.6)).unwrap_or(base.glow.1)),
            text,
            dim: fg.map(|f| lerp(f, [120, 150, 180], 0.5)).unwrap_or(base.dim),
            gold: gold.map(|g| lerp(g, [255, 235, 160], 0.35)).unwrap_or(base.gold),
            cyan: cyan.map(|c| lerp(c, [255, 255, 255], 0.2)).unwrap_or(base.cyan),
        }
    }

    pub fn load(kind: &str) -> Self {
        if kind == "classic" {
            return Self::classic();
        }
        let path = std::env::var_os("ABYSS_THEME_FILE").map(PathBuf::from).unwrap_or_else(|| {
            let base = std::env::var("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state"));
            base.join("omarchy/current/theme/colors.toml")
        });
        match std::fs::read_to_string(path) {
            Ok(t) => Self::from_colors(&parse_colors(&t)),
            Err(_) => Self::classic(),
        }
    }
}

/// Reads `name = "#rrggbb"` lines; everything else in the file is ignored.
pub fn parse_colors(text: &str) -> HashMap<String, Rgb> {
    let mut out = HashMap::new();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else { continue };
        let v = v.trim().trim_matches(|c| c == '"' || c == '\'').trim_start_matches('#');
        if v.len() != 6 {
            continue;
        }
        if let Ok(n) = u32::from_str_radix(v, 16) {
            out.insert(k.trim().to_string(), [(n >> 16) as u8, (n >> 8) as u8, n as u8]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hex_colours_and_skips_the_rest() {
        let c = parse_colors("mode = \"dark\"\nblue = \"#2563eb\"\n# x = \"#fff\"\nbad = \"#12\"\n");
        assert_eq!(c.get("blue"), Some(&[0x25, 0x63, 0xeb]));
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn theme_changes_the_look_but_empty_theme_is_classic() {
        assert_eq!(Palette::from_colors(&HashMap::new()), Palette::classic());
        let c = parse_colors("blue = \"#ff0000\"\ndark_background = \"#000010\"\nforeground = \"#ffffff\"\n");
        let p = Palette::from_colors(&c);
        assert_eq!(p.water, Some(([255, 0, 0], [0, 0, 16])));
        assert_ne!(p.glow, Palette::classic().glow);
    }
}
