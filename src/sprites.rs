//! Procedural creatures. They are drawn straight into screen pixels from their length, so a
//! finer terminal grid means more detail at the same on-screen size (shading, fins with rays,
//! eyes, patterns, outlines) instead of the same blocky art scaled up.

use crate::gfx::{Frame, Rgb, lerp};

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Clown,
    Tang,
    Silver,
    Angler,
    Lantern,
    Viper,
}

/// A species and the zones where it can show up. `len` is the body length as a fraction of the screen width.
pub struct Species {
    pub id: &'static str,
    pub kind: Kind,
    pub len: f32,
    pub zones: (usize, usize),
    pub speed: (f32, f32),
    pub school: u32,
}

pub const SPECIES: &[Species] = &[
    Species { id: "clown", kind: Kind::Clown, len: 0.038, zones: (0, 2), speed: (10.0, 18.0), school: 1 },
    Species { id: "tang", kind: Kind::Tang, len: 0.04, zones: (0, 1), speed: (12.0, 22.0), school: 4 },
    Species { id: "silver", kind: Kind::Silver, len: 0.056, zones: (1, 3), speed: (18.0, 30.0), school: 3 },
    Species { id: "lantern", kind: Kind::Lantern, len: 0.034, zones: (2, 4), speed: (9.0, 15.0), school: 4 },
    Species { id: "angler", kind: Kind::Angler, len: 0.06, zones: (4, 5), speed: (6.0, 10.0), school: 1 },
    Species { id: "viper", kind: Kind::Viper, len: 0.07, zones: (3, 5), speed: (8.0, 13.0), school: 1 },
];

struct Style {
    body_h: f32, // max half-height, as a fraction of the length
    body_c: f32, // body centre along the length (0 = tail tip, 1 = nose)
    body_r: f32, // body half-length
    tail: f32,   // tail fin length
    flare: f32,  // how much the tail fans out compared with the body height
    dorsal: f32, // dorsal fin height (fraction of length)
    top: Rgb,
    belly: Rgb,
    fin: Rgb,
}

fn style(kind: Kind) -> Style {
    match kind {
        Kind::Clown => Style { body_h: 0.20, body_c: 0.62, body_r: 0.34, tail: 0.26, flare: 1.15, dorsal: 0.07, top: [250, 118, 22], belly: [255, 168, 70], fin: [232, 96, 18] },
        Kind::Tang => Style { body_h: 0.26, body_c: 0.60, body_r: 0.37, tail: 0.27, flare: 0.95, dorsal: 0.12, top: [28, 82, 196], belly: [76, 150, 246], fin: [250, 206, 40] },
        Kind::Silver => Style { body_h: 0.15, body_c: 0.58, body_r: 0.40, tail: 0.24, flare: 1.5, dorsal: 0.06, top: [92, 118, 146], belly: [226, 236, 242], fin: [150, 168, 184] },
        Kind::Lantern => Style { body_h: 0.22, body_c: 0.60, body_r: 0.36, tail: 0.24, flare: 1.0, dorsal: 0.07, top: [26, 66, 88], belly: [92, 168, 178], fin: [58, 118, 140] },
        Kind::Viper => Style { body_h: 0.13, body_c: 0.58, body_r: 0.40, tail: 0.22, flare: 0.6, dorsal: 0.09, top: [20, 24, 40], belly: [62, 72, 98], fin: [30, 34, 58] },
        Kind::Angler => Style { body_h: 0.34, body_c: 0.60, body_r: 0.36, tail: 0.20, flare: 0.8, dorsal: 0.05, top: [44, 36, 60], belly: [86, 72, 100], fin: [58, 48, 76] },
    }
}

type Cell = Option<(Rgb, f32, bool)>; // colour, alpha, glows (ignores depth fog)

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn shade(c: Rgb, k: f32) -> Rgb {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

/// Half-height of the body at `s`, or None outside it.
fn body_half(st: &Style, s: f32) -> Option<f32> {
    let t = (s - st.body_c) / st.body_r;
    if t.abs() > 1.0 {
        return None;
    }
    // Longer taper toward the tail, rounder nose.
    let p = if t < 0.0 { 1.7 } else { 2.3 };
    Some(st.body_h * (1.0 - t.abs().powf(p)).max(0.0).powf(1.0 / p))
}

/// Colour at (s, yy) for the fish in local length units, with the tail sway already applied to yy.
fn fish_cell(kind: Kind, st: &Style, s: f32, yy: f32, px: i32, py: i32) -> Cell {
    let ped = st.body_h * 0.2;
    let body = body_half(st, s).map(|h| h.max(if s > st.tail * 0.8 && s < st.body_c { ped } else { 0.0 }));

    // Tail fin: fans out toward the tip, V notch, radial rays.
    if s < st.tail * 1.05 {
        let u = (st.tail - s) / st.tail; // 0 at the peduncle, 1 at the tip
        let hf = ped + u.max(0.0) * st.body_h * st.flare;
        if yy.abs() <= hf && !(s < st.tail * 0.5 * (1.0 - yy.abs() / hf.max(1e-4)) && u > 0.5) {
            let mut c = st.fin;
            if ((yy / hf.max(1e-4)) * 6.0).fract().abs() < 0.16 {
                c = shade(c, 0.78);
            }
            if kind == Kind::Clown && (hf - yy.abs()) < 0.012 {
                c = [24, 18, 18];
            }
            return Some((lerp(c, st.belly, 0.1 * u), 1.0, false));
        }
    }

    // Dorsal fin: rises over the back, swept toward the tail.
    let a = st.body_c - st.body_r * 0.6;
    let b = st.body_c + st.body_r * 0.35;
    if let Some(hb) = body_half(st, s) {
        let lift = st.dorsal * smooth(a, a + (b - a) * 0.35, s) * (1.0 - smooth(a + (b - a) * 0.6, b, s));
        if s > a && s < b && yy < 0.0 && -yy <= hb + lift && -yy > hb * 0.8 {
            let edge = hb + lift + yy;
            let mut c = if kind == Kind::Clown && edge < 0.012 { [24, 18, 18] } else { shade(st.fin, 0.9) };
            if ((s * 90.0) as i32) % 3 == 0 {
                c = shade(c, 0.85);
            }
            return Some((c, 1.0, false));
        }
    }

    let hb = body?;
    if yy.abs() > hb {
        // Pectoral fin: small translucent flap behind the gill.
        let (fx, fy) = (st.body_c + st.body_r * 0.2, st.body_h * 0.35);
        let d = ((s - fx) / 0.07).powi(2) + ((yy - fy) / 0.045).powi(2);
        return if d <= 1.0 && yy.abs() < hb + 0.02 { Some((lerp(st.fin, st.belly, 0.4), 0.85, false)) } else { None };
    }

    let v = yy / hb.max(1e-4); // -1 back .. +1 belly
    let mut c = lerp(st.top, st.belly, smooth(-0.35, 1.0, v));
    // Scales: faint checker; highlight stripe along the back; darker belly edge.
    if ((px / 3) + (py / 3)) % 2 == 0 {
        c = shade(c, 0.94);
    }
    let hl = 1.0 - ((v + 0.58).abs() / 0.17).min(1.0);
    c = lerp(c, [255, 255, 255], 0.2 * hl);
    c = shade(c, 1.0 - 0.18 * smooth(0.7, 1.0, v));

    match kind {
        Kind::Clown => {
            for (lo, hi) in [(0.33, 0.39), (0.58, 0.66), (0.84, 0.90)] {
                if s > lo - 0.012 && s < hi + 0.012 {
                    c = if s > lo && s < hi { [250, 248, 238] } else { [24, 18, 18] };
                }
            }
        }
        Kind::Tang => {
            let band = yy - 0.03 + 0.2 * (s - 0.6);
            if s > 0.36 && s < 0.74 && band.abs() < 0.022 + 0.03 * (0.74 - s) {
                c = [10, 16, 52];
            }
            let from = st.body_c - st.body_r * 0.55;
            if s < from {
                c = lerp(c, st.fin, 0.8 * smooth(from, st.body_c - st.body_r * 0.8, s));
            }
        }
        Kind::Silver => {
            if yy.abs() < 0.008 && s > 0.3 && s < 0.88 {
                c = shade(c, 0.7);
            }
        }
        Kind::Lantern => {
            // Row of glowing photophores along the belly.
            if v > 0.4 && v < 0.85 && s > 0.3 && s < 0.85 && px % 4 == 0 && py % 2 == 0 {
                return Some(([130, 255, 235], 1.0, true));
            }
        }
        Kind::Viper => {
            if v > 0.55 && v < 0.8 && s > 0.25 && s < 0.9 && px % 5 == 0 {
                return Some(([120, 255, 200], 1.0, true));
            }
            if s > 0.76 && (yy - 0.03).abs() < 0.02 {
                c = [10, 8, 16];
            } else if s > 0.76 && s < 0.99 && (yy - 0.03).abs() < 0.055 && (px % 4) < 2 {
                c = [236, 236, 226];
            }
        }
        Kind::Angler => {
            // Gaping jaw with teeth.
            if s > 0.7 && (yy - 0.09).abs() < 0.03 {
                c = [14, 10, 20];
            } else if s > 0.7 && s < 0.99 && (yy - 0.09).abs() < 0.075 && (px % 5) < 2 {
                c = [236, 236, 226];
            }
        }
    }

    // Eye: ring, pupil, glint.
    let (ex, ey) = (st.body_c + st.body_r * 0.62, -st.body_h * 0.18);
    let r = st.body_h * if kind == Kind::Angler { 0.2 } else { 0.24 };
    let d = ((s - ex).powi(2) + (yy - ey).powi(2)).sqrt();
    if d < r {
        c = if kind == Kind::Angler { [255, 70, 60] } else { [250, 250, 244] };
        if d < r * 0.62 {
            c = [8, 8, 14];
        }
        if (s - (ex - r * 0.25)).powi(2) + (yy - (ey - r * 0.25)).powi(2) < (r * 0.2).powi(2) {
            c = [255, 255, 255];
        }
    } else if d < r * 1.18 && kind != Kind::Angler {
        c = shade(c, 0.5);
    }
    // Gill line.
    if (s - (st.body_c + st.body_r * 0.38)).abs() < 0.006 && yy.abs() < hb * 0.7 {
        c = shade(c, 0.78);
    }
    Some((c, 1.0, false))
}

/// Draws one fish. `len` is its body length in screen pixels; (x, y) is the top-left of its box.
pub fn draw_fish(f: &mut Frame, kind: Kind, x: i32, y: i32, len: i32, flip: bool, t: f32, fog: (Rgb, f32), outline: bool) {
    let st = style(kind);
    let lenf = len as f32;
    let h = (lenf * if kind == Kind::Angler { 1.0 } else { 0.75 }) as i32;
    let cy = if kind == Kind::Angler { h as f32 * 0.62 } else { h as f32 * 0.5 };
    let w = len;
    if w < 4 || h < 3 {
        return;
    }
    let mut buf: Vec<Cell> = vec![None; (w * h) as usize];

    for py in 0..h {
        for px in 0..w {
            let s = (px as f32 + 0.5) / lenf;
            let yy = (py as f32 + 0.5 - cy) / lenf;
            // The tail sways more than the head.
            let sway = 0.035 * (t * 7.0 - s * 4.0).sin() * (1.0 - s).powi(2);
            buf[(py * w + px) as usize] = fish_cell(kind, &st, s, yy - sway, px, py);
        }
    }

    // Angler lure: curved stalk and pulsing glowing bulb.
    if kind == Kind::Angler {
        let stalk = |sx: f32| -> (f32, f32) {
            let k = ((sx - 0.68) / 0.2).clamp(0.0, 1.0);
            (sx, -st.body_h * 0.95 - 0.3 * (k * (2.0 - k)))
        };
        let steps = (len / 2).max(8);
        for i in 0..steps {
            let (s, yy) = stalk(0.66 + 0.26 * i as f32 / steps as f32);
            let (px, py) = ((s * lenf) as i32, (cy + yy * lenf) as i32);
            if px >= 0 && px < w && py >= 0 && py < h {
                buf[(py * w + px) as usize] = Some(([70, 60, 90], 1.0, false));
            }
        }
        let (bs, by) = stalk(0.92);
        let (bx, byy) = (bs * lenf, cy + by * lenf);
        let br = (lenf * 0.045).max(1.5);
        let pulse = 0.6 + 0.4 * (t * 3.0).sin();
        for py in 0..h {
            for px in 0..w {
                let d = ((px as f32 - bx).powi(2) + (py as f32 - byy).powi(2)).sqrt() / br;
                let i = (py * w + px) as usize;
                if d < 1.0 {
                    buf[i] = Some((lerp([255, 255, 200], [255, 220, 90], d), 1.0, true));
                } else if d < 2.6 && buf[i].is_none() {
                    buf[i] = Some(([255, 230, 120], (1.0 - (d - 1.0) / 1.6) * 0.45 * pulse, true));
                }
            }
        }
    }

    blit(f, &buf, w, h, x, y, flip, fog, outline);
}

/// Writes a cell buffer to the frame, optionally rimmed with a darker outline.
fn blit(f: &mut Frame, buf: &[Cell], w: i32, h: i32, x: i32, y: i32, flip: bool, fog: (Rgb, f32), outline: bool) {
    let at = |px: i32, py: i32| -> Cell {
        if px < 0 || py < 0 || px >= w || py >= h { None } else { buf[(py * w + px) as usize] }
    };
    for py in 0..h {
        for px in 0..w {
            let Some((col, alpha, glow)) = at(px, py) else { continue };
            let mut c = col;
            if outline && alpha >= 0.99 && !glow {
                let open = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| at(px + dx, py + dy).is_none_or(|n| n.1 < 0.5));
                if open {
                    c = shade(c, 0.5);
                }
            }
            let c = if glow { c } else { lerp(c, fog.0, fog.1) };
            let sx = if flip { w - 1 - px } else { px };
            if alpha >= 0.99 {
                f.set(x + sx, y + py, c);
            } else {
                f.blend(x + sx, y + py, c, alpha);
            }
        }
    }
}

/// Jellyfish: translucent dome with a bright rim, radial canals, oral arms and trailing tentacles.
pub fn draw_jelly(f: &mut Frame, x: i32, y: i32, len: i32, t: f32, fog: (Rgb, f32)) {
    let lenf = len as f32;
    let w = len;
    let h = (lenf * 1.5) as i32;
    if w < 4 || h < 3 {
        return;
    }
    let mut buf: Vec<Cell> = vec![None; (w * h) as usize];
    let pulse = 1.0 + 0.07 * (t * 2.2).sin();
    let (cx, cyb) = (0.5, 0.34);
    let (rx, ry) = (0.46 * (2.0 - pulse), 0.30 * pulse);
    let body = [228, 132, 246];
    let deep = [150, 78, 206];
    for py in 0..h {
        for px in 0..w {
            let s = (px as f32 + 0.5) / lenf;
            let yy = (py as f32 + 0.5) / lenf;
            let dx = (s - cx) / rx;
            let dy = (yy - cyb) / ry;
            let scallop = 0.03 * (s * 26.0).sin().abs();
            if dx.abs() <= 1.0 && yy <= cyb + scallop && dx * dx + dy * dy <= 1.0 {
                let depth = (dx * dx + dy * dy).sqrt();
                let mut c = lerp(lerp(body, [255, 220, 255], 0.35 * (1.0 - depth)), deep, smooth(0.55, 1.0, depth));
                if (dx * 7.0).fract().abs() < 0.1 && dy > -0.9 {
                    c = lerp(c, deep, 0.5);
                }
                let rim = smooth(0.86, 1.0, depth);
                c = lerp(c, [255, 235, 255], 0.55 * rim);
                buf[(py * w + px) as usize] = Some((c, 0.78 + 0.18 * rim, true));
            }
        }
    }
    let strands = 7;
    for i in 0..strands {
        let s0 = 0.18 + 0.64 * i as f32 / (strands - 1) as f32;
        let arm = i % 2 == 1;
        let steps = (lenf * if arm { 0.85 } else { 1.1 }) as i32;
        for k in 0..steps {
            let yy = cyb + 0.02 + k as f32 / lenf;
            let sx = s0 + 0.035 * (yy * 11.0 - t * 3.0 + i as f32).sin() * (0.4 + yy);
            let (px, py) = ((sx * lenf) as i32, (yy * lenf) as i32);
            let fade = 1.0 - k as f32 / steps as f32;
            for tx in 0..(if arm { 2 } else { 1 }) {
                if px + tx >= 0 && px + tx < w && py >= 0 && py < h {
                    buf[(py * w + px + tx) as usize] = Some((lerp(deep, body, 0.3), 0.25 + 0.55 * fade, true));
                }
            }
        }
    }
    blit(f, &buf, w, h, x, y, false, fog, false);
}
