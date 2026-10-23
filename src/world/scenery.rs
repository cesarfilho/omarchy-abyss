//! Life on the seabed, the texture of the ground, light playing on the sand, the surface and
//! far-away schools. Everything is placed like the corals: by plane, from a world x that scrolls
//! with the plane's parallax, so it is deterministic and identical on every monitor.

use super::*;

/// Screen-space scale for tiny fish at the far edge of visibility.
pub(super) const FAR_SCHOOL_PROJECTION: f64 = 0.1;

pub(super) fn shade(c: Rgb, k: f32) -> Rgb {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

/// Stable hash in [0, 1) of (a, b).
pub(super) fn hf(a: u64, b: u64) -> f32 {
    ((mix(SEED ^ 0x5CE_4E27, a.wrapping_mul(0x1F3D_5B79).wrapping_add(b)) >> 40) as f32) / (1u64 << 24) as f32
}

pub(super) fn disc(f: &mut Frame, cx: f32, cy: f32, rx: f32, ry: f32, c: Rgb) {
    let (ry, rx) = (ry.max(0.5), rx.max(0.5));
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        let dy = (y as f32 + 0.5 - cy) / ry;
        if dy.abs() > 1.0 {
            continue;
        }
        let span = rx * (1.0 - dy * dy).sqrt();
        for x in (cx - span).round() as i32..=(cx + span).round() as i32 {
            f.set(x, y, c);
        }
    }
}

/// A lit blob (rock, boulder): light from the upper left, a dark rim underneath.
pub(super) fn rock(f: &mut Frame, cx: f32, cy: f32, rx: f32, ry: f32, base: Rgb) {
    let (ry, rx) = (ry.max(1.0), rx.max(1.0));
    for y in (cy - ry).floor() as i32..=(cy + ry).ceil() as i32 {
        for x in (cx - rx).floor() as i32..=(cx + rx).ceil() as i32 {
            let (nx, ny) = ((x as f32 + 0.5 - cx) / rx, (y as f32 + 0.5 - cy) / ry);
            let d2 = nx * nx + ny * ny;
            if d2 > 1.0 {
                continue;
            }
            let nz = (1.0 - d2).sqrt();
            let lit = (-0.55 * nx - 0.65 * ny + 0.55 * nz).clamp(0.0, 1.0);
            let speckle = if ((x * 7 + y * 13) & 7) == 0 { 0.9 } else { 1.0 };
            f.set(x, y, shade(base, (0.45 + 0.75 * lit) * speckle));
        }
    }
}

/// Even-odd scanline fill of a polygon.
pub(super) fn poly(f: &mut Frame, pts: &[(f32, f32)], c: Rgb) {
    if pts.len() < 3 {
        return;
    }
    let (y0, y1) = pts.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
    let mut xs: Vec<f32> = Vec::with_capacity(8);
    for y in y0.floor() as i32..=y1.ceil() as i32 {
        let sy = y as f32 + 0.5;
        xs.clear();
        for i in 0..pts.len() {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            if (a.1 <= sy) != (b.1 <= sy) {
                xs.push(a.0 + (sy - a.1) / (b.1 - a.1) * (b.0 - a.0));
            }
        }
        xs.sort_by(|p, q| p.total_cmp(q));
        for pair in xs.chunks(2).filter(|p| p.len() == 2) {
            for x in pair[0].round() as i32..pair[1].round() as i32 {
                f.set(x, y, c);
            }
        }
    }
}

/// A line `w` pixels thick.
pub(super) fn thick(f: &mut Frame, a: (f32, f32), b: (f32, f32), w: i32, c: Rgb) {
    let steps = (b.0 - a.0).abs().max((b.1 - a.1).abs()).ceil().max(1.0) as i32;
    for i in 0..=steps {
        let k = i as f32 / steps as f32;
        f.dot((a.0 + (b.0 - a.0) * k) as i32 - w / 2, (a.1 + (b.1 - a.1) * k) as i32 - w / 2, w.max(1), c, 1.0);
    }
}

/// Everything a prop needs to know about where and how big it is drawn.
pub(super) struct Spot {
    pub x: f32,
    pub base: f32,
    /// Pixels per art unit at this depth.
    pub u: f32,
    pub sc: i32,
    pub fog: f32,
    pub bot: Rgb,
    pub t: f32,
}

impl Spot {
    pub fn col(&self, c: Rgb) -> Rgb {
        lerp(c, self.bot, self.fog)
    }
}

/// (rock, seagrass, kelp, fan, anemone, starfish, urchin, sponge, tubeworms, sea pen) by zone.
const WEIGHTS: [[u32; 10]; 6] = [
    [3, 5, 2, 2, 2, 2, 1, 2, 0, 0],
    [3, 4, 3, 2, 1, 2, 1, 2, 0, 0],
    [4, 3, 3, 1, 0, 1, 2, 1, 0, 0],
    [5, 0, 1, 0, 0, 0, 2, 0, 3, 3],
    [5, 0, 0, 0, 0, 0, 1, 0, 3, 4],
    [5, 0, 0, 0, 0, 0, 0, 0, 4, 4],
];

fn pick(zone: usize, small_ok: bool, r: f32) -> usize {
    let w = WEIGHTS[zone.min(5)];
    let total: u32 = w.iter().enumerate().map(|(i, v)| if !small_ok && matches!(i, 4..=6) { 0 } else { *v }).sum();
    let mut k = (r * total as f32) as u32;
    for (i, v) in w.iter().enumerate() {
        let v = if !small_ok && matches!(i, 4..=6) { 0 } else { *v };
        if k < v {
            return i;
        }
        k -= v;
    }
    0
}

#[cfg(test)]
pub(super) fn pick_for_test(zone: usize, small_ok: bool, r: f32) -> usize {
    pick(zone, small_ok, r)
}

impl World {
    /// Props, ground texture and light for one seabed plane (called right after `draw_seabed`).
    pub(super) fn draw_scenery(&self, f: &mut Frame, slot: usize, bot: Rgb, floor: &[f32]) {
        self.draw_ground_texture(f, slot, bot, floor);
        self.draw_props(f, slot, bot);
        self.draw_lost_nets(f, slot, bot);
        self.draw_crabs(f, slot, bot);
    }

    /// Torn fishing nets caught on the seabed. Their sparse, seeded placement is shared by all
    /// monitors and fades out in the deep zones where old fishing gear is less plausible.
    fn draw_lost_nets(&self, f: &mut Frame, slot: usize, bot: Rgb) {
        if slot > 2 || self.tz > 3 {
            return;
        }
        let unit = self.unit() as f64;
        let spacing = 300.0 * unit;
        let speed = PLANES[slot].4 as f64;
        let z = slot as f32 / 4.0;
        let sc = ((self.px_scale as f32 * scale_z(z)).round() as i32).max(1);
        let u = self.unit() * scale_z(z);
        let fog = self.fog_z(z);
        for i in self.visible_cells(spacing, speed, 50.0 * unit) {
            let seed = mix(84_000 + slot as u64, i as u64);
            if hf(seed, 1) > 0.36 {
                continue;
            }
            let wx = (i as f64 + 0.15 + hf(seed, 2) as f64 * 0.7) * spacing;
            let sx = ((wx - self.cam64() * speed).round() as i32 - self.off as i32) as f32;
            if sx < -50.0 * u || sx > self.w as f32 + 50.0 * u {
                continue;
            }
            let base = self.floor_y(slot, sx);
            let net = lerp([92, 116, 104], bot, fog + 0.12);
            let rope = lerp([70, 78, 66], bot, fog);
            let width = (18.0 + 8.0 * hf(seed, 3)) * u;
            let height = (7.0 + 4.0 * hf(seed, 4)) * u;
            // A sagging net with a few broken, trailing strands.
            for k in 0..=8 {
                let x = sx - width + 2.0 * width * k as f32 / 8.0;
                let y = base - height * (1.0 - (2.0 * k as f32 / 8.0 - 1.0).abs()) * 0.5;
                thick(f, (x, base), (x, y - height * 0.45), sc, net);
            }
            for k in 0..=4 {
                let x = sx - width + 2.0 * width * k as f32 / 4.0;
                let y = base - height * (1.0 - (2.0 * k as f32 / 4.0 - 1.0).abs()) * 0.5;
                thick(f, (sx - width, base - height * 0.45), (x, y), sc, net);
                thick(f, (sx + width, base - height * 0.45), (x, y), sc, net);
            }
            thick(f, (sx - width * 1.1, base), (sx + width * 1.1, base), sc + 1, rope);
            thick(f, (sx - width, base - height * 0.45), (sx + width, base - height * 0.45), sc, rope);
            // Floats and a snagged fragment of rope make the net readable at a glance.
            for side in [-1.0f32, 1.0] {
                disc(f, sx + side * width, base - height * 0.5, 1.5 * u, 1.1 * u, lerp([196, 154, 72], bot, fog));
            }
            thick(f, (sx + width * 0.7, base - height * 0.45), (sx + width * 1.25, base - height * 1.4), sc, rope);
        }
    }

    /// Floor height of a plane for every screen column (computed once per plane and frame).
    pub(super) fn floor_row(&self, slot: usize) -> Vec<f32> {
        // Same curve as `floor_y`, with the huge scrolling phase reduced once in f64 so the
        // per-column work can run in f32.
        let (base, amp, f1, f2, speed, _) = PLANES[slot.min(4)];
        let sc = self.px_scale as f64;
        let phase = (self.off as f64 + self.cam64() * speed as f64) / sc + slot as f64 * 17.0;
        let a1 = (phase * f1 as f64).rem_euclid(std::f64::consts::TAU) as f32;
        let a2 = (phase * f2 as f64 + 1.0).rem_euclid(std::f64::consts::TAU) as f32;
        let (d1, d2) = (f1 / sc as f32, f2 / sc as f32);
        let h = self.h as f32;
        (0..self.w).map(|x| h - h * (base + amp * (a1 + x as f32 * d1).sin() + amp * 0.5 * (a2 + x as f32 * d2).sin())).collect()
    }

    pub(super) fn draw_props(&self, f: &mut Frame, slot: usize, bot: Rgb) {
        let z = slot as f32 / 4.0;
        let cs = scale_z(z);
        let unit = self.unit() as f64;
        let u = self.unit() * cs;
        let spacing = 24.0 * unit;
        let speed = PLANES[slot].4 as f64;
        let sc = ((self.px_scale as f32 * cs).round() as i32).max(1);
        let fog = self.fog_z(z);
        for i in self.visible_cells(spacing, speed, 60.0 * unit) {
            let seed = mix(1_000 + slot as u64, i as u64);
            if hf(seed, 1) < 0.12 {
                continue;
            }
            let wx = (i as f64 + hf(seed, 2) as f64 * 0.8) * spacing;
            let sx = ((wx - self.cam64() * speed).round() as i32 - self.off as i32) as f32;
            if sx < -60.0 * unit as f32 || sx > self.w as f32 + 60.0 * unit as f32 {
                continue;
            }
            let base = self.floor_y(slot, sx) + 1.0;
            let sp = Spot { x: sx, base, u, sc, fog, bot, t: self.t * 0.5 };
            let (a, b, c) = (hf(seed, 3), hf(seed, 4), hf(seed, 5));
            match pick(self.tz, slot <= 2, hf(seed, 6)) {
                0 => self.prop_rock(f, &sp, a, b),
                1 => self.prop_seagrass(f, &sp, a, b),
                2 => self.prop_kelp(f, &sp, a, b, c),
                3 => self.prop_fan(f, &sp, a, b),
                4 => self.prop_anemone(f, &sp, a),
                5 => self.prop_starfish(f, &sp, a, b),
                6 => self.prop_urchin(f, &sp, a),
                7 => self.prop_sponge(f, &sp, a, b),
                8 => self.prop_tubeworms(f, &sp, a, b),
                _ => self.prop_seapen(f, &sp, a, b),
            }
        }
    }

    fn prop_rock(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let r = (3.0 + 6.0 * a) * s.u;
        let tone = lerp([112, 116, 120], [78, 86, 96], b);
        let tone = lerp(tone, self.sand(), 0.25);
        let c = s.col(tone);
        let (rx, ry) = (r * (0.9 + 0.5 * b), r * 0.62);
        // A contact shadow, the main body, a smaller one leaning on it.
        disc(f, s.x, s.base - 0.1 * ry, rx * 1.1, ry * 0.25, s.col(shade(self.sand(), 0.35)));
        rock(f, s.x, s.base - ry * 0.75, rx, ry, c);
        rock(f, s.x + rx * 0.7, s.base - ry * 0.4, rx * 0.5, ry * 0.5, shade(c, 0.92));
        // Moss on the shallow ones.
        if self.tz <= 1 {
            let moss = s.col([84, 124, 58]);
            for k in 0..(rx as i32 / 2).max(2) {
                let mx = s.x - rx * 0.6 + k as f32 * rx * 1.1 / (rx / 2.0).max(2.0);
                f.dot(mx as i32, (s.base - ry * 1.5 + (k % 3) as f32) as i32, s.sc, moss, 0.85);
            }
        }
    }

    fn prop_seagrass(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let h = (5.0 + 8.0 * a) * s.u;
        let blades = 4 + (b * 4.0) as i32;
        let (dark, light) = (lerp([36, 98, 52], [30, 92, 90], (self.zf / 2.0).min(1.0)), lerp([112, 178, 84], [90, 170, 150], (self.zf / 2.0).min(1.0)));
        for k in 0..blades {
            let lean = (k as f32 - blades as f32 / 2.0) * 0.12 * h * 0.25;
            let ph = k as f32 * 1.7 + a * 6.0;
            let hh = h * (0.7 + 0.3 * hf(k as u64, (a * 1000.0) as u64));
            for j in 0..hh as i32 {
                let q = j as f32 / hh;
                let sway = (s.t * 1.1 + ph + j as f32 * 0.05).sin() * 2.5 * s.u * 0.4 * q * q;
                let x = s.x + lean * q + sway;
                f.dot(x as i32, (s.base - j as f32) as i32, s.sc, s.col(lerp(dark, light, q)), 1.0);
            }
        }
    }

    fn prop_kelp(&self, f: &mut Frame, s: &Spot, a: f32, b: f32, c: f32) {
        let h = (22.0 + 26.0 * a) * s.u;
        let stalk = s.col(lerp([88, 96, 40], [60, 80, 56], (self.zf / 2.5).min(1.0)));
        let leaf = s.col(lerp([128, 140, 52], [86, 124, 78], (self.zf / 2.5).min(1.0)));
        let sway = |q: f32| (s.t * 0.7 + b * std::f32::consts::TAU + q * 3.0).sin() * 5.0 * s.u * 0.5 * q * q;
        let mut prev = (s.x, s.base);
        for j in 1..=(h as i32 / 3) {
            let q = (j * 3) as f32 / h;
            let p = (s.x + sway(q), s.base - j as f32 * 3.0);
            thick(f, prev, p, s.sc, stalk);
            if j % 3 == 0 {
                let side = if (j / 3) % 2 == 0 { 1.0 } else { -1.0 };
                let len = (3.0 + 4.0 * (1.0 - q)) * s.u * 0.6;
                disc(f, p.0 + side * len * 0.55, p.1 - len * 0.15, len * 0.6, len * 0.22 + 0.5, leaf);
                // a gas bladder at the base of the frond
                f.dot(p.0 as i32, p.1 as i32 - 1, s.sc + 1, lerp(leaf, [210, 200, 120], 0.35), 1.0);
            }
            prev = p;
        }
        // Tiny fry circling the top of some stalks.
        if c < 0.4 && self.tz <= 2 {
            let top = prev;
            for k in 0..5 {
                let ph = s.t * (0.8 + 0.1 * k as f32) + k as f32 * 1.26;
                let (fx, fy) = (top.0 + ph.cos() * 6.0 * s.u * 0.6, top.1 + 6.0 * s.u * 0.4 + ph.sin() * 3.0 * s.u * 0.4);
                f.dot(fx as i32, fy as i32, s.sc.max(1), s.col([160, 182, 196]), 0.9);
            }
        }
    }

    fn prop_fan(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let h = (7.0 + 7.0 * a) * s.u;
        let base = if b < 0.5 { [200, 70, 120] } else { [230, 130, 60] };
        let c = s.col(base);
        let tip = s.col(lerp(base, [255, 255, 255], 0.4));
        let sway = (s.t * 0.8 + a * 6.0).sin() * 1.5 * s.u * 0.3;
        for k in -3..=3 {
            let ang = k as f32 * 0.28;
            let top = (s.x + ang.sin() * h * 0.9 + sway, s.base - ang.cos() * h);
            thick(f, (s.x, s.base), top, s.sc, c);
            // side twigs
            for q in [0.45, 0.7] {
                let mid = (s.x + (top.0 - s.x) * q, s.base + (top.1 - s.base) * q);
                for sgn in [-1.0f32, 1.0] {
                    f.line(mid.0 as i32, mid.1 as i32, (mid.0 + sgn * h * 0.18) as i32, (mid.1 - h * 0.2) as i32, c);
                }
            }
            f.dot(top.0 as i32, top.1 as i32 - s.sc, s.sc, tip, 1.0);
        }
    }

    fn prop_anemone(&self, f: &mut Frame, s: &Spot, a: f32) {
        let r = (2.0 + 1.5 * a) * s.u;
        disc(f, s.x, s.base - r * 0.35, r * 0.9, r * 0.45, s.col([140, 60, 90]));
        for k in -5..=5 {
            let ang = k as f32 * 0.24;
            let wob = (s.t * 1.6 + k as f32 * 0.8).sin() * 0.25;
            let tip = (s.x + (ang + wob).sin() * r * 1.5, s.base - r * 0.5 - (ang + wob).cos() * r * 1.5);
            thick(f, (s.x + ang * r * 0.6, s.base - r * 0.5), tip, s.sc, s.col([232, 110, 150]));
            f.dot(tip.0 as i32, tip.1 as i32, s.sc, s.col([255, 226, 236]), 1.0);
        }
    }

    fn prop_starfish(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let r = (1.8 + 1.4 * a) * s.u;
        let c = s.col(if b < 0.5 { [232, 110, 50] } else { [200, 60, 70] });
        let (cx, cy) = (s.x, s.base - r * 0.3);
        let rot = b * std::f32::consts::TAU;
        for k in 0..5 {
            let ang = rot + k as f32 * 1.2566;
            let tip = (cx + ang.cos() * r, cy + ang.sin() * r * 0.4);
            let (l, rr) = (ang - 0.5, ang + 0.5);
            poly(f, &[(cx + l.cos() * r * 0.25, cy + l.sin() * r * 0.1), tip, (cx + rr.cos() * r * 0.25, cy + rr.sin() * r * 0.1)], c);
        }
        disc(f, cx, cy, r * 0.3, r * 0.14, s.col([255, 200, 140]));
    }

    fn prop_urchin(&self, f: &mut Frame, s: &Spot, a: f32) {
        let r = (1.2 + 1.0 * a) * s.u;
        let c = s.col([60, 34, 86]);
        let (cx, cy) = (s.x, s.base - r);
        for k in 0..16 {
            let ang = k as f32 * std::f32::consts::FRAC_PI_8 - std::f32::consts::FRAC_PI_2;
            if ang.sin() > 0.4 {
                continue;
            }
            f.line(cx as i32, cy as i32, (cx + ang.cos() * r * 1.9) as i32, (cy + ang.sin() * r * 1.9) as i32, c);
        }
        disc(f, cx, cy, r * 0.8, r * 0.8, shade(c, 1.3));
    }

    fn prop_sponge(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let base = if b < 0.5 { [226, 190, 70] } else { [170, 90, 170] };
        for (dx, hh) in [(-1.2, 0.7), (0.0, 1.0), (1.3, 0.55)] {
            let (w, h) = (1.1 * s.u, (3.0 + 4.0 * a) * s.u * hh);
            let x = s.x + dx * s.u * 1.2;
            poly(f, &[(x - w * 0.5, s.base), (x - w * 0.7, s.base - h), (x + w * 0.7, s.base - h), (x + w * 0.5, s.base)], s.col(base));
            disc(f, x, s.base - h, w * 0.7, w * 0.25, s.col(shade(base, 0.45)));
        }
    }

    pub(super) fn prop_tubeworms(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let n = 5 + (b * 4.0) as i32;
        for k in 0..n {
            let x = s.x + (k as f32 - n as f32 / 2.0) * s.u * 0.55;
            let h = (3.0 + 5.0 * hf(k as u64, (a * 999.0) as u64)) * s.u;
            let lean = (k as f32 - n as f32 / 2.0) * 0.06 * h;
            thick(f, (x, s.base), (x + lean, s.base - h), s.sc + 1, s.col([226, 224, 214]));
            let tip = (x + lean, s.base - h);
            let pulse = 0.6 + 0.4 * (s.t * 1.4 + k as f32).sin();
            // The red plume glows: it ignores the fog.
            f.dot(tip.0 as i32 - 1, tip.1 as i32 - s.sc, s.sc + 1, lerp([200, 40, 50], [255, 120, 100], pulse), 1.0);
        }
    }

    fn prop_seapen(&self, f: &mut Frame, s: &Spot, a: f32, b: f32) {
        let h = (6.0 + 6.0 * a) * s.u;
        let sway = (s.t * 0.9 + b * 6.0).sin() * 1.2 * s.u * 0.3;
        thick(f, (s.x, s.base), (s.x + sway, s.base - h), s.sc, s.col([96, 110, 132]));
        // Feathery polyps along the stalk, longest at the bottom; the tips glow and ripple.
        for k in 1..=7 {
            let q = k as f32 / 7.0;
            let (px, py) = (s.x + sway * q * q, s.base - h * (0.25 + 0.75 * q));
            let w = (1.0 - q * 0.7) * 2.6 * s.u * 0.5 + 1.0;
            let pulse = 0.35 + 0.65 * ((s.t * 1.8 - q * 5.0 + b * 8.0).sin() * 0.5 + 0.5);
            let glow = lerp([40, 150, 160], [120, 255, 220], pulse);
            for sgn in [-1.0f32, 1.0] {
                thick(f, (px, py), (px + sgn * w, py - w * 0.35), s.sc, s.col([70, 130, 150]));
                f.dot((px + sgn * w) as i32, (py - w * 0.35) as i32, s.sc, glow, 1.0);
            }
        }
        f.dot((s.x + sway) as i32 - 1, (s.base - h) as i32 - 2, s.sc + 1, [150, 255, 230], 1.0);
    }

    /// Crabs scuttling sideways along the sand of the two nearest planes (shallow zones).
    pub(super) fn draw_crabs(&self, f: &mut Frame, slot: usize, bot: Rgb) {
        if slot > 1 || self.tz > 2 {
            return;
        }
        let z = slot as f32 / 4.0;
        let cs = scale_z(z);
        let unit = self.unit() as f64;
        let u = self.unit() * cs;
        let spacing = 300.0 * unit;
        let speed = PLANES[slot].4 as f64;
        let sc = ((self.px_scale as f32 * cs).round() as i32).max(1);
        for i in self.visible_cells(spacing, speed, 60.0 * unit) {
            let seed = mix(70_000 + slot as u64, i as u64);
            if hf(seed, 1) > 0.45 {
                continue;
            }
            let wx = (i as f64 + 0.1 + 0.8 * hf(seed, 2) as f64) * spacing;
            // Walks back and forth over a stretch of sand, pausing at each end.
            let phase = self.tt * 0.09 + hf(seed, 3) as f64 * std::f64::consts::TAU;
            let walk = (phase.sin() * 1.6).clamp(-1.0, 1.0) * 22.0 * unit;
            let facing = if phase.cos() >= 0.0 { 1.0f32 } else { -1.0 };
            let moving = (phase.sin() * 1.6).abs() < 1.0;
            let sx = (wx - self.cam64() * speed + walk - self.off as f64) as f32;
            if sx < -20.0 * u || sx > self.w as f32 + 20.0 * u {
                continue;
            }
            let base = self.floor_y(slot, sx) + 1.0;
            let fog = self.fog_z(z);
            let col = |c: Rgb| lerp(c, bot, fog);
            let body = col([210, 84, 52]);
            let dark = col([140, 50, 36]);
            let (cx, cy) = (sx, base - 1.3 * u);
            // legs, three per side, alternating while walking
            for k in 0..3 {
                for sgn in [-1.0f32, 1.0] {
                    let step = if moving { (self.t * 4.5 + k as f32 * 2.1 + if sgn > 0.0 { std::f32::consts::PI } else { 0.0 }).sin() } else { 0.0 };
                    let root = (cx + sgn * (0.7 + 0.5 * k as f32) * u * 0.7, cy + 0.2 * u);
                    let foot = (root.0 + sgn * (1.3 + step * 0.5) * u * 0.7, base - step.max(0.0) * 0.5 * u * 0.4);
                    thick(f, root, (root.0 + sgn * 0.8 * u * 0.7, root.1 - 0.5 * u * 0.4), sc, dark);
                    thick(f, (root.0 + sgn * 0.8 * u * 0.7, root.1 - 0.5 * u * 0.4), foot, sc, dark);
                }
            }
            disc(f, cx, cy, 2.2 * u * 0.75, 1.3 * u * 0.75, body);
            disc(f, cx - 0.3 * u * 0.4, cy - 0.3 * u * 0.4, 1.4 * u * 0.75, 0.7 * u * 0.75, lerp(body, [255, 190, 150], 0.3));
            // claws held up in front, eyes on stalks
            for sgn in [-1.0f32, 1.0] {
                let arm = (cx + facing * 2.0 * u * 0.75, cy - 0.2 * u + sgn * 0.0);
                let claw = (arm.0 + facing * 1.2 * u * 0.75, arm.1 - (1.2 + sgn * 0.4 + (self.t * 1.5 + sgn).sin() * 0.3) * u * 0.75);
                thick(f, arm, claw, sc, body);
                disc(f, claw.0, claw.1, 0.9 * u * 0.6, 0.7 * u * 0.6, body);
                let eye = (cx + facing * (0.6 + sgn * 0.5) * u * 0.75, cy - 1.7 * u * 0.75);
                thick(f, (eye.0, cy - 0.8 * u * 0.75), eye, sc, dark);
                f.dot(eye.0 as i32, eye.1 as i32 - sc, sc + 1, col([250, 250, 240]), 1.0);
            }
        }
    }

    /// Sand ripples and pebbles along each plane, plus caustics near the surface.
    pub(super) fn draw_ground_texture(&self, f: &mut Frame, slot: usize, bot: Rgb, floor: &[f32]) {
        let color = lerp(bot, self.sand(), PLANES[slot].5);
        let (light, dark) = (lerp(color, [255, 250, 220], 0.14), shade(color, 0.86));
        let fog = self.fog_z(slot as f32 / 4.0) * 0.6;
        let sc = self.px_scale.max(1);
        let near = slot <= 2;
        // Ripple lines that follow the contour of the floor.
        if near {
            for (k, (off, c, a)) in [(5, light, 0.35), (9, dark, 0.45), (14, light, 0.2)].into_iter().enumerate() {
                for (x, &fy) in floor.iter().enumerate() {
                    let wob = ((x as f32 + self.off) * 0.09 / sc as f32 + k as f32 * 2.0).sin() * sc as f32 * 0.8;
                    let y = fy + (off * sc) as f32 + wob;
                    if y < self.h as f32 - 1.0 {
                        f.blend(x as i32, y as i32, lerp(c, bot, fog), a);
                    }
                }
            }
        }
        // Pebbles and shell glints, from a hash of the world position.
        let cell = (4 * sc) as usize;
        let gx = self.off as usize;
        for (x, &fy) in floor.iter().enumerate().step_by(2) {
            let fy = fy as i32;
            let wx = (x + gx + (self.cam64() * PLANES[slot].4 as f64) as usize) / cell;
            for dy in (3..(self.h as i32 - fy).min(40 * sc)).step_by(cell) {
                let r = hf(slot as u64 * 7919 + wx as u64, dy as u64 / cell as u64);
                if r < 0.07 {
                    f.dot(x as i32, fy + dy, sc, lerp(shade(color, 0.7 + r * 4.0), bot, fog), 1.0);
                } else if r > 0.985 {
                    f.dot(x as i32, fy + dy, sc, lerp([236, 226, 200], bot, fog), 0.9);
                }
            }
        }
        // Caustics: bright net on the sand while the sun reaches it.
        let strength = (1.0 - self.zf / 1.6).max(0.0) * if near { 1.0 } else { 0.0 };
        if strength > 0.02 && slot <= 1 {
            let tx: Vec<f32> = (0..self.w).map(|x| ((x as f32 + self.off) * 0.055 / sc as f32 + self.t * 0.3).sin()).collect();
            let tx2: Vec<f32> = (0..self.w).map(|x| ((x as f32 + self.off) * 0.083 / sc as f32 - self.t * 0.225 + 1.3).sin()).collect();
            for x in 0..self.w {
                let fy = (floor[x] as i32).max(0);
                for y in fy..self.h as i32 {
                    let ty = (y as f32 * 0.07 / sc as f32 + self.t * 0.175).sin();
                    let ty2 = (y as f32 * 0.11 / sc as f32 - self.t * 0.25).sin();
                    let g = (tx[x] * ty + tx2[x] * ty2).abs();
                    if g < 0.22 {
                        f.blend(x as i32, y, [255, 252, 220], (1.0 - g / 0.22) * 0.3 * strength);
                    }
                }
            }
        }
    }

    /// The same wave height drives the bright surface and anything floating on it.
    fn surface_height(&self, panorama_x: f32) -> f32 {
        let sc = self.px_scale as f32;
        let waves = if self.journey.is_some() { (self.sea.wave_m / 0.8).clamp(0.6, 3.0) } else { 1.0 };
        sc * (1.5 + 1.2 * waves)
            + ((panorama_x * 0.03 + self.t * 0.9).sin()
                + 0.5 * (panorama_x * 0.071 - self.t * 1.3).sin()) * sc * waves
    }

    /// The water surface seen from below (shallow zones): a wavy bright edge with glints.
    pub(super) fn draw_surface(&self, f: &mut Frame) {
        let strength = (1.0 - self.zf / 1.5).max(0.0);
        if strength < 0.02 {
            return;
        }
        let sc = self.px_scale as f32;
        for x in 0..self.w {
            let gx = x as f32 + self.off;
            let y = self.surface_height(gx);
            for k in 0..(3.0 * sc) as i32 {
                let a = (1.0 - k as f32 / (3.0 * sc)) * 0.55 * strength;
                f.blend(x as i32, y as i32 + k, [235, 250, 255], a);
            }
            if hf((gx / sc) as u64, (self.t * 3.0) as u64) > 0.992 {
                f.dot(x as i32, y as i32 + sc as i32, self.px_scale, [255, 255, 255], 0.9 * strength);
            }
        }
        self.draw_fishing_line(f, strength);
    }

    /// A stationary boat above the surface: its buoy lifts the line and hook with the waves.
    fn draw_fishing_line(&self, f: &mut Frame, strength: f32) {
        if self.tz > 2 {
            return;
        }
        let fade = strength * (1.0 - self.zf / 3.2).clamp(0.0, 1.0);
        if fade <= 0.02 {
            return;
        }
        let unit = self.unit();
        let sc = self.px_scale.max(1);
        let visibility = 0.55 + 0.35 * (1.0 - fade);
        let line = lerp([72, 84, 88], self.water().0, visibility);
        let spacing = self.pano as f64 * 1.2;
        for i in self.visible_cells(spacing, 1.0, 8.0 * unit as f64) {
            let seed = mix(85_501, i as u64);
            let world_x = (i as f64 + 0.35 + 0.3 * hf(seed, 1) as f64) * spacing;
            let x = (world_x - self.cam64() - self.off as f64) as f32;
            if x < -8.0 * unit || x > self.w as f32 + 8.0 * unit { continue; }
            let surface_y = self.surface_height(x + self.off);
            let drift = if self.journey.is_some() { self.sea.flow_knots as f32 * unit * 0.65 } else { 0.0 };
            let hook_x = x + drift;
            let end = surface_y + (19.0 + 5.0 * hf(seed, 2)) * unit;
            // The buoy marks where the unseen boat holds the line. Both the rope and hook follow it.
            disc(f, x, surface_y, 2.3 * unit, 1.35 * unit, lerp([226, 82, 52], self.water().0, 0.22));
            disc(f, x - 0.45 * unit, surface_y - 0.45 * unit, 0.7 * unit, 0.45 * unit, [255, 224, 170]);
            thick(f, (x, surface_y + unit), (hook_x, end), sc, line);
            thick(f, (hook_x, end), (hook_x + 1.0 * unit, end + 1.3 * unit), sc, line);
            thick(f, (hook_x + 1.0 * unit, end + 1.3 * unit), (hook_x + 1.8 * unit, end + 0.8 * unit), sc, line);
            disc(f, hook_x, end, 0.7 * unit, 0.7 * unit, lerp([226, 168, 76], self.water().1, 0.55 + 0.3 * (1.0 - fade)));
        }
    }

    /// Faint schools far away, projected from the lens-close cruise pace.
    pub(super) fn far_school_x(&self, school: u64) -> f64 {
        let period = self.pano as f64 + 60.0 * self.unit() as f64;
        let dir = if hf(school, 1) < 0.5 { 1.0 } else { -1.0 };
        let speed = self.paths.first().map_or(0.0, |path| path.speed) * FAR_SCHOOL_PROJECTION * dir;
        (hf(school, 3) as f64 * period + speed * self.tt).rem_euclid(period) - 30.0 * self.unit() as f64 - self.off as f64
    }

    pub(super) fn draw_far_schools(&self, f: &mut Frame, bot: Rgb) {
        let n = 6 + if self.journey.is_some() { (self.sea.upwelling * 5.0).round() as u64 } else { 0 };
        for s in 0..n {
            let dir = if hf(s, 1) < 0.5 { 1.0 } else { -1.0 };
            let cx = self.far_school_x(s);
            let cy = (0.2 + 0.35 * hf(s, 4)) * self.h as f32;
            let c = lerp([40, 70, 110], bot, 0.55 + 0.25 * (self.zf / 5.0).min(1.0));
            let count = if s == 1 || s == 4 { 34 } else { 18 };
            for k in 0..count {
                // Scattered through an elongated blob, each fish with its own slow drift.
                let (ang, rad) = (hf(s, 20 + k as u64) * std::f32::consts::TAU, hf(s, 50 + k as u64).powf(0.7));
                let spread = if count > 18 { 18.0 } else { 11.0 };
                let x = cx as f32 + (ang.cos() * rad * spread + (self.t * 0.075 + ang * 3.0).sin() * 1.5) * self.unit() * 0.7;
                let y = cy + (ang.sin() * rad * if count > 18 { 5.0 } else { 3.5 } + (self.t * 0.06 + ang * 2.0).cos() * 0.8) * self.unit() * 0.5;
                f.dot(x as i32, y as i32, self.px_scale, c, 0.8);
                f.dot(x as i32 - dir as i32 * self.px_scale, y as i32, self.px_scale, c, 0.45);
            }
        }
    }
}
