//! Achievement banners in the same style as the HUD: terminal text on a dark plate. The plate
//! slides below the HUD, the text types itself in, then it slides back up.

use std::collections::VecDeque;

use crate::gfx::{Overlay, Rgb};
use crate::state::{self, GameEvent, Season, ZONE_NAMES};
use crate::theme::Palette;

const SLIDE_IN: f32 = 0.5;
const TYPE: f32 = 0.9;
const HOLD: f32 = 9.0;
const SLIDE_OUT: f32 = 0.5;
const TOTAL: f32 = SLIDE_IN + TYPE + HOLD + SLIDE_OUT;

/// Rows of the plate: a blank row above and below the four text lines.
const PLATE_ROWS: i32 = 6;
const START_ROW: i32 = 6;
const REST_ROW: i32 = 8;

struct Toast {
    colors: [Rgb; 4],
    lines: [String; 4],
    t: f32,
}

pub struct Toasts {
    queue: VecDeque<Toast>,
    current: Option<Toast>,
    pal: Palette,
}

fn ease_out(p: f32) -> f32 {
    1.0 - (1.0 - p).powi(3)
}

fn plate_top(t: f32) -> i32 {
    if t < SLIDE_IN {
        let p = ease_out((t / SLIDE_IN).clamp(0.0, 1.0));
        (START_ROW as f32 + (REST_ROW - START_ROW) as f32 * p).round() as i32
    } else if t > TOTAL - SLIDE_OUT {
        let p = ((t - (TOTAL - SLIDE_OUT)) / SLIDE_OUT).clamp(0.0, 1.0);
        (REST_ROW as f32 - (REST_ROW - START_ROW) as f32 * p * p).round() as i32
    } else {
        REST_ROW
    }
}

impl Toasts {
    pub fn new(pal: Palette) -> Self {
        Self { queue: VecDeque::new(), current: None, pal }
    }

    pub fn set_palette(&mut self, pal: Palette) { self.pal = pal; }

    pub fn push_season(&mut self, s: &Season) {
        let p = self.pal;
        self.queue.push_back(Toast {
            colors: [p.cyan, p.text, p.dim, p.gold],
            lines: ["≈ EVENTO DE TEMPORADA ≈".into(), s.name.to_uppercase(), s.desc.to_string(), "Evento visual · troféu sazonal ativo".into()],
            t: 0.0,
        });
    }

    /// Shown when the screensaver starts: where the saved game stands.
    pub fn push_resume(&mut self, save: &state::Save) {
        let p = self.pal;
        self.queue.push_back(Toast {
            colors: [p.cyan, p.text, p.dim, p.gold],
            lines: [
                "≈ RETOMANDO A JORNADA ≈".into(),
                format!("NÍVEL {} · {} PONTOS", save.level(), save.points),
                format!("{} · {} M · SÉRIE DE {} DIA(S)", ZONE_NAMES[save.zone()], save.depth_m(), save.streak),
                format!("{}/{} TROFÉUS", save.trophies.len(), state::TROPHIES.len()),
            ],
            t: 0.0,
        });
    }

    pub fn push(&mut self, ev: &GameEvent) {
        let (gold, cyan, cream, dim) = (self.pal.gold, self.pal.cyan, self.pal.text, self.pal.dim);
        let toast = match ev {
            GameEvent::Discovery(id) => {
                let Some(c) = state::creature(id) else { return };
                Toast {
                    colors: [cyan, cream, dim, cyan],
                    lines: ["◆ NOVA CRIATURA ◆".into(), c.name.to_uppercase(), c.desc.to_string(), format!("+{} PONTOS", state::DISCOVERY_POINTS)],
                    t: 0.0,
                }
            }
            GameEvent::Quest(id) => {
                let Some(q) = state::quest(id) else { return };
                Toast {
                    colors: [gold, cream, dim, gold],
                    lines: ["✔ MISSÃO CUMPRIDA ✔".into(), q.name.to_uppercase(), q.desc.to_string(), format!("+{} PONTOS", state::QUEST_POINTS)],
                    t: 0.0,
                }
            }
            GameEvent::Trophy(id) => {
                let Some(t) = state::trophy(id) else { return };
                Toast {
                    colors: [gold, cream, dim, gold],
                    lines: ["★ TROFÉU DESBLOQUEADO ★".into(), t.name.to_uppercase(), t.desc.to_string(), "+100 PONTOS".into()],
                    t: 0.0,
                }
            }
            GameEvent::LevelUp(l) => {
                let zone = ((*l as usize) / 3).min(5);
                let depth = state::Save { xp: state::xp_at(*l), ..Default::default() }.depth_m();
                Toast {
                    colors: [cyan, cream, dim, cyan],
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
        let cream = self.pal.text;
        let widest = c.lines.iter().map(|l| l.chars().count()).max().unwrap_or(0);
        let width = widest + 6;
        let left = cols.saturating_sub(width) / 2;

        // Enter below the HUD, so notifications never obscure its instruments.
        let top = plate_top(c.t);

        let typed = ((c.t - SLIDE_IN) / TYPE).clamp(0.0, 1.0);
        let total: usize = c.lines.iter().map(|l| l.chars().count()).sum();
        let mut budget = (typed * total as f32).ceil() as usize;
        let blank = " ".repeat(width);
        let row_at = |i: i32| -> Option<usize> { usize::try_from(top + i).ok() };
        for i in [0, PLATE_ROWS - 1] {
            if let Some(r) = row_at(i) {
                o.text(left, r, &blank, cream);
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

#[cfg(test)]
mod position_tests {
    use super::*;

    #[test]
    fn notification_never_overlaps_the_hud() {
        for step in 0..=100 {
            let t = TOTAL * step as f32 / 100.0;
            assert!(plate_top(t) >= START_ROW);
        }
    }
}
