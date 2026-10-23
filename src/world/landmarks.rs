//! Rare set pieces on the far planes: wrecks, a liner, ruins, bones, vents. Each is a
//! procedural silhouette placed like the other scenery (deterministic, scrolling with its plane),
//! and each is a bestiary discovery the first time it comes into view.

use super::scenery::{Spot, disc, hf, poly, rock, shade, thick};
use super::*;

/// (id, weights by zone 0..5, size in units of the plane's scale)
pub(super) const KINDS: [(&str, [u32; 6], f32); 10] = [
    ("wreck", [4, 4, 3, 2, 0, 0], 1.5),
    ("titanic", [0, 0, 0, 0, 5, 2], 0.8),
    ("sub", [0, 0, 2, 3, 0, 0], 1.8),
    ("plane", [0, 0, 2, 3, 2, 0], 1.8),
    ("ruins", [0, 3, 3, 1, 0, 0], 1.5),
    ("anchor", [3, 2, 2, 1, 0, 0], 1.4),
    ("treasure", [3, 3, 1, 0, 0, 0], 1.3),
    ("amphorae", [0, 2, 1, 0, 0, 0], 1.4),
    ("whalefall", [0, 0, 0, 3, 3, 3], 1.4),
    ("vents", [0, 0, 0, 0, 0, 5], 1.4),
];

/// Local drawing frame: x along the object (flipped by `dir`), y down, in art units, rotated by `a`.
struct Xf {
    x: f32,
    y: f32,
    u: f32,
    cos: f32,
    sin: f32,
    dir: f32,
}

impl Xf {
    fn new(x: f32, y: f32, u: f32, a: f32, dir: f32) -> Self {
        Self { x, y, u, cos: a.cos(), sin: a.sin(), dir }
    }

    fn p(&self, lx: f32, ly: f32) -> (f32, f32) {
        let (lx, ly) = (lx * self.dir * self.u, ly * self.u);
        (self.x + lx * self.cos - ly * self.sin, self.y + lx * self.sin + ly * self.cos)
    }

    fn pts(&self, v: &[(f32, f32)]) -> Vec<(f32, f32)> {
        v.iter().map(|&(a, b)| self.p(a, b)).collect()
    }
}

fn kind_at(zone: usize, r: f32) -> Option<usize> {
    let total: u32 = KINDS.iter().map(|k| k.1[zone.min(5)]).sum();
    if total == 0 {
        return None;
    }
    let mut k = (r * total as f32) as u32;
    for (i, kind) in KINDS.iter().enumerate() {
        let w = kind.1[zone.min(5)];
        if k < w {
            return Some(i);
        }
        k -= w;
    }
    None
}

impl World {
    /// Landmarks of one far plane: (kind index, screen x of the centre, seed).
    pub(super) fn landmark_spots(&self, slot: usize) -> Vec<(usize, f32, u64)> {
        if slot != 1 && slot != 2 {
            return Vec::new();
        }
        let unit = self.unit() as f64;
        let spacing = 250.0 * unit;
        let speed = PLANES[slot].4 as f64;
        let reach = 300.0 * unit;
        let left = (self.cam64() * speed - reach) / spacing;
        let right = (self.cam64() * speed + self.pano as f64 + reach) / spacing;
        let mut out = Vec::new();
        for i in left.floor() as i64..=right.ceil() as i64 {
            let seed = mix(90_000 + slot as u64, i as u64);
            if self.force_landmark.is_none() && hf(seed, 1) > 0.7 {
                continue;
            }
            let Some(kind) = self.force_landmark.or_else(|| kind_at(self.tz, hf(seed, 2))) else { continue };
            let wx = (i as f64 + 0.15 + 0.7 * hf(seed, 3) as f64) * spacing;
            let sx = (wx - self.cam64() * speed - self.off as f64) as f32;
            out.push((kind, sx, seed));
        }
        out
    }

    /// Landmarks at least partly on screen anywhere in the panorama (for bestiary sightings).
    pub(super) fn landmarks_in_view(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        for slot in [1, 2] {
            for (kind, sx, _) in self.landmark_spots(slot) {
                let reach = 60.0 * self.unit();
                // `sx` is relative to this screen; anything inside the whole panorama counts.
                let pano_x = sx + self.off;
                if pano_x > -reach && pano_x < self.pano + reach {
                    out.push(KINDS[kind].0);
                }
            }
        }
        out
    }

    pub(super) fn draw_landmarks(&self, f: &mut Frame, slot: usize, bot: Rgb) {
        let z = slot as f32 / 4.0;
        let cs = scale_z(z);
        let base_u = self.unit() * cs;
        let sc = ((self.px_scale as f32 * cs).round() as i32).max(1);
        for (kind, sx, seed) in self.landmark_spots(slot) {
            if sx < -140.0 * base_u * KINDS[kind].2 || sx > self.w as f32 + 140.0 * base_u * KINDS[kind].2 {
                continue;
            }
            let u = base_u * KINDS[kind].2;
            let base = self.floor_y(slot, sx) + 2.0;
            let s = Spot { x: sx, base, u, sc, fog: self.fog_z(z) * 0.6, bot, t: self.t * 0.5 };
            let dir = if hf(seed, 4) < 0.5 { 1.0 } else { -1.0 };
            match KINDS[kind].0 {
                "wreck" => self.lm_wreck(f, &s, dir),
                "titanic" => self.lm_titanic(f, &s, dir),
                "sub" => self.lm_sub(f, &s, dir),
                "plane" => self.lm_plane(f, &s, dir),
                "ruins" => self.lm_ruins(f, &s, seed),
                "anchor" => self.lm_anchor(f, &s, dir),
                "treasure" => self.lm_treasure(f, &s, dir),
                "amphorae" => self.lm_amphorae(f, &s, dir),
                "whalefall" => self.lm_whalefall(f, &s, dir),
                _ => self.lm_vents(f, &s, seed),
            }
        }
    }

    /// Sand heaped around the foot of a wreck so it looks half buried.
    fn bury(&self, f: &mut Frame, s: &Spot, half_w: f32, h: f32) {
        let c = s.col(shade(self.sand(), 0.75));
        disc(f, s.x, s.base + 1.0, half_w * s.u, h * s.u, c);
    }

    fn lm_wreck(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base - 3.0 * s.u, s.u, -0.16, dir);
        let wood = s.col([86, 60, 38]);
        let dark = s.col([50, 34, 24]);
        let hull = x.pts(&[(-26.0, 3.0), (-23.0, -9.0), (-6.0, -11.0), (18.0, -9.0), (30.0, -19.0), (27.0, -3.0), (12.0, 3.0)]);
        poly(f, &hull, wood);
        // Planks, then the broken top edge showing the ribs.
        for ly in [-7.0f32, -3.5, 0.0] {
            let (a, b) = (x.p(-24.0 + ly.abs() * 0.4, ly), x.p(26.0 - ly.abs(), ly));
            thick(f, a, b, s.sc, dark);
        }
        for k in 0..11 {
            let lx = -22.0 + k as f32 * 4.4;
            let h = 2.0 + 5.0 * hf(k as u64, 77);
            let top = if lx > 14.0 { -9.0 - (lx - 14.0) * 0.5 } else { -10.0 };
            thick(f, x.p(lx, top + 1.0), x.p(lx, top - h), s.sc, dark);
        }
        // Gun ports.
        for k in 0..5 {
            let p = x.p(-12.0 + k as f32 * 6.5, -5.0);
            f.dot(p.0 as i32, p.1 as i32, s.sc + 1, s.col([22, 16, 12]), 1.0);
        }
        // Broken mast, yard and a torn sail.
        let (m0, m1) = (x.p(-2.0, -11.0), x.p(-5.0, -33.0));
        thick(f, m0, m1, s.sc + 1, dark);
        thick(f, x.p(-12.0, -26.0), x.p(5.0, -28.0), s.sc, dark);
        poly(f, &x.pts(&[(-11.0, -26.0), (4.0, -27.5), (2.0, -18.0), (-1.0, -20.0), (-5.0, -14.0), (-10.0, -17.0)]), s.col([150, 140, 118]));
        // Algae on the rail.
        let alg = s.col([70, 110, 60]);
        for k in 0..14 {
            let p = x.p(-22.0 + k as f32 * 3.4, -10.5 - hf(k as u64, 5) * 1.5);
            f.dot(p.0 as i32, p.1 as i32, s.sc, alg, 0.9);
        }
        self.bury(f, s, 24.0, 3.0);
    }

    fn lm_titanic(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let hull_c = s.col([34, 38, 44]);
        let rust = s.col([92, 56, 40]);
        let deck_c = s.col([118, 120, 122]);
        let line_c = s.col([62, 64, 66]);
        // Bow section, nose-down and half buried.
        let b = Xf::new(s.x - dir * 52.0 * s.u, s.base - 8.0 * s.u, s.u, 0.2, dir);
        poly(f, &b.pts(&[(-30.0, 2.0), (-30.0, -14.0), (22.0, -14.0), (34.0, -19.0), (27.0, -2.0), (16.0, 3.0)]), hull_c);
        poly(f, &b.pts(&[(-30.0, 2.0), (-30.0, -3.0), (27.0, -3.0), (24.0, 2.0)]), rust);
        poly(f, &b.pts(&[(-26.0, -14.0), (18.0, -14.0), (18.0, -22.0), (-26.0, -22.0)]), deck_c);
        poly(f, &b.pts(&[(-21.0, -22.0), (10.0, -22.0), (10.0, -28.0), (-21.0, -28.0)]), shade(deck_c, 0.9));
        for row in [-18.0, -25.0] {
            for k in 0..26 {
                let lx = -25.0 + k as f32 * 1.7;
                if (row == -25.0 && !(-20.0..9.0).contains(&lx)) || hf(k as u64, row as u64) < 0.15 {
                    continue;
                }
                let p = b.p(lx, row);
                // Faint lit portholes, glowing through the dark.
                f.dot(p.0 as i32, p.1 as i32, s.sc, lerp([230, 190, 100], s.bot, 0.55 + 0.2 * s.fog), 0.7);
            }
        }
        // Four funnels, raked back, buff with a black top.
        for k in 0..4 {
            let lx = -17.0 + k as f32 * 9.5;
            poly(f, &b.pts(&[(lx - 2.6, -28.0), (lx + 2.6, -28.0), (lx + 2.0 - 1.6, -43.0), (lx - 3.0 - 1.6, -43.0)]), s.col([176, 138, 88]));
            poly(f, &b.pts(&[(lx - 3.0 - 1.6, -43.0), (lx + 2.0 - 1.6, -43.0), (lx + 2.1 - 1.6, -40.0), (lx - 3.1 - 1.6, -40.0)]), s.col([20, 20, 22]));
        }
        for lx in [-28.0, 20.0] {
            thick(f, b.p(lx, -14.0), b.p(lx - 1.0, -40.0), s.sc, line_c);
        }
        for ly in [-9.0, -6.0] {
            thick(f, b.p(-29.0, ly), b.p(27.0 - (ly + 9.0).abs() * 1.5, ly), s.sc, line_c);
        }
        // Stern section, torn off and tilted steeply up.
        let t = Xf::new(s.x + dir * 40.0 * s.u, s.base - 3.0 * s.u, s.u, -0.42, dir);
        poly(f, &t.pts(&[(-18.0, 3.0), (-22.0, -5.0), (-17.0, -9.0), (-20.0, -14.0), (-14.0, -17.0), (20.0, -14.0), (22.0, 0.0), (14.0, 3.0)]), hull_c);
        poly(f, &t.pts(&[(-14.0, -17.0), (18.0, -14.0), (18.0, -21.0), (-10.0, -24.0)]), deck_c);
        for k in 0..8 {
            let p = t.p(-8.0 + k as f32 * 3.0, -18.0 - k as f32 * 0.1);
            f.dot(p.0 as i32, p.1 as i32, s.sc, lerp([230, 190, 100], s.bot, 0.6 + 0.2 * s.fog), 0.6);
        }
        thick(f, t.p(18.0, -2.0), t.p(25.0, 6.0), s.sc + 1, line_c); // rudder
        thick(f, t.p(8.0, -21.0), t.p(8.0, -36.0), s.sc, line_c); // mast
        // Debris field between the halves: crates, plates, a boiler.
        for k in 0..16 {
            let dx = (k as f32 - 8.0) * 2.6 + hf(k as u64, 3) * 2.0;
            let (px, py) = (s.x + dir * dx * s.u * 0.5 - 6.0 * dir * s.u, s.base - hf(k as u64, 9) * 1.2 * s.u);
            let w = (0.8 + hf(k as u64, 4) * 1.6) * s.u;
            poly(f, &[(px, py), (px + w, py - w * 0.2), (px + w * 0.8, py - w * 0.7), (px - w * 0.1, py - w * 0.5)], if k % 3 == 0 { rust } else { line_c });
        }
        disc(f, s.x + dir * 4.0 * s.u, s.base - 1.2 * s.u, 3.2 * s.u, 1.5 * s.u, shade(rust, 0.8));
        self.bury(f, s, 56.0, 3.0);
    }

    fn lm_sub(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base - 4.5 * s.u, s.u, 0.14, dir);
        let hull = s.col([74, 86, 78]);
        let rust = s.col([112, 70, 44]);
        let c = x.p(0.0, 0.0);
        disc(f, c.0, c.1, 17.0 * s.u, 3.8 * s.u, hull);
        // The sail (conning tower) with periscope and a hatch.
        poly(f, &x.pts(&[(-3.0, -3.0), (-2.0, -9.0), (5.0, -9.0), (6.0, -3.0)]), shade(hull, 1.1));
        thick(f, x.p(1.0, -9.0), x.p(1.0, -14.0), s.sc, shade(hull, 0.8));
        thick(f, x.p(1.0, -14.0), x.p(3.5, -14.0), s.sc, shade(hull, 0.8));
        let h = x.p(1.5, -6.0);
        f.dot(h.0 as i32, h.1 as i32, s.sc + 1, s.col([206, 176, 70]), 1.0);
        // Rust streaks and a deck rail.
        for k in 0..8 {
            let p = x.p(-12.0 + k as f32 * 3.2, 1.5 + hf(k as u64, 8) * 1.5);
            f.dot(p.0 as i32, p.1 as i32, s.sc + 1, rust, 0.7);
        }
        thick(f, x.p(-14.0, -3.0), x.p(13.0, -3.2), s.sc, shade(hull, 0.7));
        // Propeller and rudder at the stern.
        let tail = x.p(-17.0, 0.0);
        for a in [-0.9f32, 0.0, 0.9] {
            thick(f, tail, (tail.0 - dir * 2.0 * s.u * 0.5, tail.1 + a * 3.0 * s.u), s.sc, shade(hull, 0.6));
        }
        poly(f, &x.pts(&[(-18.0, -2.0), (-21.0, -5.0), (-21.0, 5.0), (-18.0, 2.0)]), shade(hull, 0.8));
        // A few bubbles still seeping out of the hatch.
        for k in 0..4 {
            let ph = (s.t * 0.35 + k as f32 * 0.25).fract();
            f.dot((h.0 + (ph * 9.0).sin() * 2.0) as i32, (h.1 - ph * 26.0 * s.u * 0.5) as i32, s.sc, [196, 226, 240], (1.0 - ph) * 0.8);
        }
        self.bury(f, s, 15.0, 2.5);
    }

    fn lm_plane(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base - 4.0 * s.u, s.u, -0.26, dir);
        let body = s.col([108, 116, 120]);
        let dark = s.col([62, 68, 72]);
        let c = x.p(0.0, 0.0);
        disc(f, c.0, c.1, 14.0 * s.u, 2.6 * s.u, body);
        poly(f, &x.pts(&[(-12.0, -1.5), (-17.0, -9.0), (-13.5, -9.0), (-8.0, -1.5)]), shade(body, 0.9)); // tail fin
        poly(f, &x.pts(&[(-13.0, 0.0), (-17.0, 3.0), (-12.0, 2.0)]), shade(body, 0.8)); // elevator
        // Wing, one intact and one snapped off, with roundels.
        poly(f, &x.pts(&[(-3.0, 1.0), (2.0, 1.0), (4.5, 10.0), (-1.0, 11.0)]), shade(body, 1.05));
        poly(f, &x.pts(&[(-2.0, -0.5), (3.0, -0.5), (2.0, -5.0), (-1.0, -3.5)]), dark);
        let r = x.p(1.5, 6.0);
        disc(f, r.0, r.1, 1.4 * s.u * 0.5, 1.4 * s.u * 0.5, s.col([150, 60, 60]));
        // Canopy, engine cowling, bent propeller.
        let k = x.p(3.0, -2.2);
        disc(f, k.0, k.1, 2.8 * s.u * 0.5, 1.5 * s.u * 0.5, s.col([70, 96, 108]));
        poly(f, &x.pts(&[(10.0, -2.5), (14.0, -2.0), (14.0, 2.0), (10.0, 2.5)]), dark);
        thick(f, x.p(14.5, 0.0), x.p(15.5, 6.0), s.sc + 1, shade(dark, 0.7));
        thick(f, x.p(14.5, 0.0), x.p(15.0, -4.0), s.sc + 1, shade(dark, 0.7));
        self.bury(f, s, 13.0, 2.0);
    }

    fn lm_ruins(&self, f: &mut Frame, s: &Spot, seed: u64) {
        let stone = s.col([176, 172, 156]);
        let shadow = s.col([120, 118, 108]);
        poly(f, &[(s.x - 24.0 * s.u, s.base), (s.x - 23.0 * s.u, s.base - 2.2 * s.u), (s.x + 23.0 * s.u, s.base - 2.2 * s.u), (s.x + 24.0 * s.u, s.base)], shadow);
        for k in 0..5 {
            let cx = s.x + (k as f32 - 2.0) * 9.0 * s.u;
            let h = if hf(seed, 10 + k) < 0.4 { 6.0 + 5.0 * hf(seed, 20 + k) } else { 14.0 + 3.0 * hf(seed, 30 + k) };
            let w = 2.2 * s.u;
            let top = s.base - 2.2 * s.u - h * s.u;
            poly(f, &[(cx - w * 0.5, s.base - 2.2 * s.u), (cx - w * 0.42, top), (cx + w * 0.42, top), (cx + w * 0.5, s.base - 2.2 * s.u)], stone);
            // Fluting: darker vertical stripes on the shaded side.
            for g in 0..3 {
                let gx = cx - w * 0.3 + g as f32 * w * 0.3;
                thick(f, (gx, s.base - 2.4 * s.u), (gx, top + 1.0), s.sc, shade(stone, 0.86));
            }
            if h > 13.0 {
                poly(f, &[(cx - w * 0.75, top), (cx + w * 0.75, top), (cx + w * 0.7, top - 1.1 * s.u), (cx - w * 0.7, top - 1.1 * s.u)], shade(stone, 1.08));
            } else {
                // Broken: jagged top and a drum on the ground.
                for j in 0..3 {
                    f.dot((cx - w * 0.4 + j as f32 * w * 0.4) as i32, (top - j as f32 % 2.0 * s.u * 0.4) as i32, s.sc + 1, stone, 1.0);
                }
                rock(f, cx + w * 1.3, s.base - 0.7 * s.u, w * 0.6, 0.8 * s.u, stone);
            }
        }
        // Part of the architrave still stands, and algae creeps up the bases.
        poly(f, &[(s.x - 19.0 * s.u, s.base - 18.5 * s.u), (s.x - 1.0 * s.u, s.base - 18.5 * s.u), (s.x - 1.0 * s.u, s.base - 20.5 * s.u), (s.x - 19.0 * s.u, s.base - 20.5 * s.u)], shade(stone, 1.05));
        for k in 0..16 {
            f.dot((s.x - 22.0 * s.u + k as f32 * 2.9 * s.u) as i32, (s.base - 2.6 * s.u - hf(seed, 40 + k) * 3.0 * s.u) as i32, s.sc, s.col([74, 116, 64]), 0.8);
        }
    }

    fn lm_anchor(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base - 1.0 * s.u, s.u, -0.1, dir);
        let iron = s.col([96, 70, 52]);
        thick(f, x.p(0.0, -13.0), x.p(0.0, 0.0), s.sc + 1, iron); // shaft
        thick(f, x.p(-4.0, -11.0), x.p(4.0, -11.0), s.sc + 1, iron); // stock
        let ring = x.p(0.0, -15.0);
        disc(f, ring.0, ring.1, 1.8 * s.u * 0.5, 1.8 * s.u * 0.5, iron);
        disc(f, ring.0, ring.1, 1.0 * s.u * 0.5, 1.0 * s.u * 0.5, s.col([40, 30, 24]));
        // Arms and flukes.
        for sgn in [-1.0f32, 1.0] {
            thick(f, x.p(0.0, 0.0), x.p(sgn * 5.5, -2.5), s.sc + 1, iron);
            poly(f, &x.pts(&[(sgn * 5.5, -2.5), (sgn * 7.5, -5.5), (sgn * 6.0, -0.5)]), iron);
        }
        // A chain trailing across the sand.
        let mut prev = x.p(0.0, -15.0);
        for k in 1..14 {
            let q = k as f32;
            let p = x.p(-q * 1.7, -15.0 + (q * 1.7).min(15.0) + ((q * 0.9).sin()) * 0.3);
            let p = (p.0, p.1.min(s.base));
            thick(f, prev, p, s.sc, if k % 2 == 0 { iron } else { shade(iron, 0.75) });
            prev = p;
        }
        self.bury(f, s, 7.0, 1.4);
    }

    fn lm_treasure(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base, s.u, 0.05, dir);
        let wood = s.col([110, 70, 36]);
        let band = s.col([190, 160, 70]);
        poly(f, &x.pts(&[(-5.0, 0.0), (-5.0, -5.0), (5.0, -5.0), (5.0, 0.0)]), wood);
        for lx in [-4.0, 0.0, 4.0] {
            thick(f, x.p(lx, 0.0), x.p(lx, -5.0), s.sc, band);
        }
        // The lid, flung open.
        poly(f, &x.pts(&[(-5.0, -5.0), (5.0, -5.0), (6.0, -12.0), (-4.0, -12.5)]), shade(wood, 0.85));
        thick(f, x.p(-4.0, -12.0), x.p(6.0, -11.5), s.sc, band);
        // Gold heaped inside and spilling out, with a pulsing glint (it ignores the fog).
        disc(f, x.p(0.0, -5.0).0, x.p(0.0, -5.2).1, 4.8 * s.u * 0.5, 1.6 * s.u * 0.5, [226, 184, 60]);
        for k in 0..9 {
            let p = x.p(5.0 + k as f32 * 0.9, -0.4 - (k % 2) as f32 * 0.4);
            f.dot(p.0 as i32, p.1 as i32, s.sc, [222, 178, 56], 1.0);
        }
        for k in 0..3 {
            let a = (s.t * 1.7 + k as f32 * 2.1).sin() * 0.5 + 0.5;
            let p = x.p(-2.0 + k as f32 * 2.6, -7.0 - k as f32 * 0.8);
            f.dot(p.0 as i32 - 1, p.1 as i32, s.sc + 2, [255, 244, 190], a * 0.9);
            f.dot(p.0 as i32, p.1 as i32 - 1, s.sc, [255, 255, 230], a);
        }
        self.bury(f, s, 7.0, 1.2);
    }

    fn lm_amphorae(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let clay = s.col([168, 90, 52]);
        for (k, (dx, tilt, h)) in [(-7.0f32, 0.0f32, 9.0f32), (0.0, 1.15, 8.0), (6.5, -0.25, 10.0)].into_iter().enumerate() {
            let x = Xf::new(s.x + dx * dir * s.u, s.base - 0.5 * s.u, s.u, tilt, dir);
            // foot, belly, neck and the two handles
            let v = [(-1.2, 0.0), (1.2, 0.0), (2.6, -h * 0.35), (2.0, -h * 0.75), (1.0, -h * 0.85), (1.0, -h), (-1.0, -h), (-1.0, -h * 0.85), (-2.0, -h * 0.75), (-2.6, -h * 0.35)];
            poly(f, &x.pts(&v), if k == 1 { shade(clay, 0.9) } else { clay });
            for sgn in [-1.0f32, 1.0] {
                thick(f, x.p(sgn * 1.0, -h * 0.9), x.p(sgn * 2.4, -h * 0.7), s.sc, shade(clay, 0.8));
            }
            thick(f, x.p(-2.4, -h * 0.5), x.p(2.4, -h * 0.5), s.sc, shade(clay, 0.75)); // painted band
        }
        // Shards.
        for k in 0..6 {
            let (px, py) = (s.x + (k as f32 - 3.0) * 2.2 * s.u, s.base - 0.4 * s.u);
            poly(f, &[(px, py), (px + 1.2 * s.u, py - 0.3 * s.u), (px + 0.4 * s.u, py - 1.0 * s.u)], shade(clay, 0.85));
        }
        self.bury(f, s, 11.0, 1.2);
    }

    fn lm_whalefall(&self, f: &mut Frame, s: &Spot, dir: f32) {
        let x = Xf::new(s.x, s.base - 1.0 * s.u, s.u, 0.0, dir);
        let bone = s.col([208, 202, 184]);
        let dark = s.col([140, 134, 120]);
        // Spine: a row of vertebrae sagging slightly.
        let spine = |lx: f32| -> (f32, f32) { x.p(lx, -3.0 + (lx / 30.0).powi(2) * 2.0) };
        for k in 0..22 {
            let lx = -26.0 + k as f32 * 2.1;
            let p = spine(lx);
            disc(f, p.0, p.1, 0.9 * s.u, 0.9 * s.u, if k % 2 == 0 { bone } else { dark });
        }
        // Ribs arching up from the spine, shorter toward the tail.
        for k in 0..9 {
            let lx = -4.0 + k as f32 * 2.4;
            let r = 6.5 - k as f32 * 0.45;
            let mut prev = spine(lx);
            for j in 1..=8 {
                let a = j as f32 / 8.0 * 2.0;
                let p = x.p(lx + (a * 0.9).sin() * 0.6, -3.0 - (a * 1.4).sin() * r + (1.0 - (a * 0.5).cos()).max(0.0) * r * 0.2);
                thick(f, prev, p, s.sc + 1, bone);
                prev = p;
            }
        }
        // Skull: a long wedge with the lower jaw.
        poly(f, &x.pts(&[(-26.0, -4.5), (-26.0, -1.0), (-40.0, 0.0), (-44.0, -1.5), (-40.0, -5.0)]), bone);
        poly(f, &x.pts(&[(-26.0, -0.5), (-43.0, 0.5), (-44.0, 2.0), (-27.0, 1.5)]), dark);
        let e = x.p(-28.0, -3.0);
        f.dot(e.0 as i32, e.1 as i32, s.sc + 1, s.col([40, 36, 30]), 1.0);
        // Red worm tufts feeding on the bones.
        for k in 0..10 {
            let p = spine(-24.0 + k as f32 * 5.0);
            let sway = (s.t * 1.5 + k as f32).sin() * 1.2;
            thick(f, (p.0, p.1 - s.u * 0.5), (p.0 + sway, p.1 - s.u * 1.6), s.sc, s.col([196, 60, 56]));
        }
        self.bury(f, s, 36.0, 2.0);
    }

    fn lm_vents(&self, f: &mut Frame, s: &Spot, seed: u64) {
        let rock_c = s.col([40, 34, 36]);
        let crust = s.col([84, 70, 60]);
        for k in 0..3 {
            let cx = s.x + (k as f32 - 1.0) * 10.0 * s.u + (hf(seed, 50 + k) - 0.5) * 3.0 * s.u;
            let h = (12.0 + 9.0 * hf(seed, 60 + k)) * s.u;
            let w = 3.0 * s.u;
            poly(f, &[(cx - w * 1.2, s.base), (cx - w * 0.45, s.base - h), (cx + w * 0.45, s.base - h), (cx + w * 1.2, s.base)], rock_c);
            // Mineral crust on one side, glowing cracks at the base (these ignore the fog).
            thick(f, (cx + w * 0.3, s.base - h), (cx + w * 0.9, s.base), s.sc + 1, crust);
            let glow = 0.6 + 0.4 * (s.t * 1.3 + k as f32 * 2.0).sin();
            thick(f, (cx - w * 0.5, s.base), (cx - w * 0.2, s.base - h * 0.3), s.sc, lerp([120, 30, 10], [255, 140, 50], glow));
            disc(f, cx, s.base - h, w * 0.5, w * 0.22, lerp([180, 60, 20], [255, 170, 70], glow));
            // The plume: dark particles rising and fanning out.
            for j in 0..22 {
                let ph = (s.t * 0.22 + j as f32 / 22.0 + k as f32 * 0.31).fract();
                let rise = ph * 34.0 * s.u;
                let (px, py) = (cx + (ph * 9.0 + j as f32).sin() * (1.0 + ph * 4.0) * s.u * 0.5, s.base - h - rise);
                f.dot(px as i32, py as i32, s.sc + (ph * 2.0) as i32, lerp([70, 64, 66], s.bot, ph * 0.8 + 0.1), (1.0 - ph) * 0.8);
            }
        }
        // A tube worm colony around the foot.
        self.prop_tubeworms(f, &Spot { x: s.x - 14.0 * s.u, ..Spot { x: s.x, base: s.base, u: s.u, sc: s.sc, fog: s.fog, bot: s.bot, t: s.t } }, 0.5, 0.8);
        self.prop_tubeworms(f, &Spot { x: s.x + 15.0 * s.u, ..Spot { x: s.x, base: s.base, u: s.u, sc: s.sc, fog: s.fog, bot: s.bot, t: s.t } }, 0.3, 0.4);
    }
}
