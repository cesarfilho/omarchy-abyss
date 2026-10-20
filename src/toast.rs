//! Achievement banners in the same style as the HUD: terminal text on a dark plate. The plate
//! slides down from the top row by row, the text types itself in, then it slides back up.

use std::collections::VecDeque;

use crate::gfx::{Overlay, Rgb};
use crate::state::{self, GameEvent, ZONE_NAMES};

const SLIDE_IN: f32 = 0.5;
const TYPE: f32 = 0.9;
const HOLD: f32 = 9.0;
const SLIDE_OUT: f32 = 0.5;
const TOTAL: f32 = SLIDE_IN + TYPE + HOLD + SLIDE_OUT;

/// Rows of the plate: a blank row above and below the four text lines.
const PLATE_ROWS: i32 = 6;
const REST_ROW: i32 = 1;

const GOLD: Rgb = [255, 220, 120];
const CYAN: Rgb = [140, 230, 246];
const CREAM: Rgb = [235, 240, 235];
const DIM: Rgb = [160, 190, 210];

struct Toast {
    colors: [Rgb; 4],
    lines: [String; 4],
    t: f32,
}

pub struct Toasts {
    queue: VecDeque<Toast>,
    current: Option<Toast>,
}

fn ease_out(p: f32) -> f32 {
    1.0 - (1.0 - p).powi(3)
}

impl Toasts {
    pub fn new() -> Self {
        Self { queue: VecDeque::new(), current: None }
    }

    pub fn push(&mut self, ev: &GameEvent) {
        let toast = match ev {
            GameEvent::Trophy(id) => {
                let Some(t) = state::trophy(id) else { return };
                Toast {
                    colors: [GOLD, CREAM, DIM, GOLD],
                    lines: ["★ TROFÉU DESBLOQUEADO ★".into(), t.name.to_uppercase(), t.desc.to_string(), "+100 PONTOS".into()],
                    t: 0.0,
                }
            }
            GameEvent::LevelUp(l) => {
                let zone = ((*l as usize) / 3).min(5);
                let depth = state::Save { xp: state::xp_at(*l), ..Default::default() }.depth_m();
                Toast {
                    colors: [CYAN, CREAM, DIM, CYAN],
                    lines: ["▲ SUBIU DE NÍVEL ▲".into(), format!("NÍVEL {l}"), ZONE_NAMES[zone].to_uppercase(), format!("{depth} M DE PROFUNDIDADE")],
                    t: 0.0,
                }
            }
        };
        self.queue.push_back(toast);
    }

    pub fn update(&mut self, dt: f32) {
        if self.current.is_none() {
            self.current = self.queue.pop_front();
        }
        if let Some(c) = &mut self.current {
            c.t += dt;
            if c.t >= TOTAL {
                self.current = None;
            }
        }
    }

    pub fn draw(&self, o: &mut Overlay, cols: usize) {
        let Some(c) = &self.current else { return };
        let widest = c.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let width = widest + 6;
        let left = cols.saturating_sub(width) / 2;

        // Plate position in rows: eases down, rests, eases back up.
        let top = if c.t < SLIDE_IN {
            let p = ease_out((c.t / SLIDE_IN).clamp(0.0, 1.0));
            (-PLATE_ROWS as f32 + (REST_ROW + PLATE_ROWS) as f32 * p).round() as i32
        } else if c.t > TOTAL - SLIDE_OUT {
            let p = ((c.t - (TOTAL - SLIDE_OUT)) / SLIDE_OUT).clamp(0.0, 1.0);
            (REST_ROW as f32 - (REST_ROW + PLATE_ROWS) as f32 * p * p).round() as i32
        } else {
            REST_ROW
        };

        let typed = ((c.t - SLIDE_IN) / TYPE).clamp(0.0, 1.0);
        let total: usize = c.lines.iter().map(|l| l.chars().count()).sum();
        let mut budget = (typed * total as f32).ceil() as usize;
        let blank = " ".repeat(width);
        let row_at = |i: i32| -> Option<usize> { usize::try_from(top + i).ok() };
        for i in [0, PLATE_ROWS - 1] {
            if let Some(r) = row_at(i) {
                o.text(left, r, &blank, CREAM);
            }
        }
        for (i, line) in c.lines.iter().enumerate() {
            let n = line.chars().count().min(budget);
            budget -= n;
            let shown: String = line.chars().take(n).collect();
            // Centre against the widest line; unrevealed letters stay as plate.
            let pad_left = (widest - line.chars().count()) / 2;
            let text = format!(
                "{}{}{}",
                " ".repeat(3 + pad_left),
                shown,
                " ".repeat(width - 3 - pad_left - n)
            );
            if let Some(r) = row_at(i as i32 + 1) {
                o.text(left, r, &text, c.colors[i]);
            }
        }
    }
}

/// Shares events between the screensaver instances (one per monitor): only the first instance
/// earns XP, so it publishes what happened and the others replay it as banners.
fn events_path() -> std::path::PathBuf {
    state::state_dir().join("events.json")
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

pub struct Feed {
    last_seq: u64,
}

impl Feed {
    pub fn new() -> Self {
        // Accept events from just before this instance started (the session-start trophy).
        Self { last_seq: now_ms().saturating_sub(3000) }
    }

    pub fn publish(&mut self, ev: &GameEvent) {
        let mut list: Vec<serde_json::Value> =
            std::fs::read_to_string(events_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let seq = now_ms().max(self.last_seq + 1);
        self.last_seq = seq;
        let (kind, value) = match ev {
            GameEvent::Trophy(id) => ("trophy", id.to_string()),
            GameEvent::LevelUp(l) => ("level", l.to_string()),
        };
        list.push(serde_json::json!({ "seq": seq, "kind": kind, "value": value }));
        let keep = list.len().saturating_sub(20);
        list.drain(..keep);
        let _ = std::fs::create_dir_all(state::state_dir());
        let tmp = state::state_dir().join(format!("events.tmp{}", std::process::id()));
        if std::fs::write(&tmp, serde_json::Value::Array(list).to_string()).is_ok() {
            let _ = std::fs::rename(&tmp, events_path());
        }
    }

    /// New events published by another instance since the last poll.
    pub fn poll(&mut self) -> Vec<GameEvent> {
        let list: Vec<serde_json::Value> =
            std::fs::read_to_string(events_path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let mut out = Vec::new();
        for e in list {
            let seq = e["seq"].as_u64().unwrap_or(0);
            if seq <= self.last_seq {
                continue;
            }
            self.last_seq = seq;
            let value = e["value"].as_str().unwrap_or("");
            match e["kind"].as_str() {
                Some("trophy") => {
                    if let Some(t) = state::trophy(value) {
                        out.push(GameEvent::Trophy(t.id));
                    }
                }
                Some("level") => {
                    if let Ok(l) = value.parse() {
                        out.push(GameEvent::LevelUp(l));
                    }
                }
                _ => {}
            }
        }
        out
    }
}
