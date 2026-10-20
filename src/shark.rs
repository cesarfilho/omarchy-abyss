//! A shark that patrols the open water now and then. Like everything in the world it is a pure
//! function of the clock: a window of `WINDOW` seconds may hold one visit whose route is a `Path`
//! seeded from the window number. Its body is a chain of circles laid along the route it has
//! just swum (so it bends and undulates when it turns), and fish near its head are pushed away
//! (`push`), which opens a school as the shark passes and lets it close again afterwards.

use crate::{
    gfx::{Frame, Rgb, lerp},
    path::Path,
    world::{Gen, SEED, mix},
};

pub const WINDOW: f64 = 240.0;
const CHANCE: u64 = 45;
const SEGMENTS: usize = 26;

pub struct SharkRun {
    path: Path,
    shift_x: f64,
    start: f64,
    dur: f64,
    pub z: f32,
    len: f32,
    /// Fish closer than this to the head are pushed away.
    reach: f32,
}

fn smooth(x: f32) -> f32 {
    let t = x.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

impl SharkRun {
    /// The visit planned for the window containing `tt`, if there is one. `len` is the shark's
    /// length at depth z = 0 (screen pixels); `unit` the world's speed unit.
    pub fn plan(tt: f64, pano: f32, h: f32, refw: f32, unit: f32) -> Option<Self> {
        let k = (tt / WINDOW).floor() as u64;
        if mix(SEED, 500 + k) % 100 >= CHANCE {
            return None;
        }
        let mut g = Gen(mix(SEED, 600 + k));
        let z = g.range(0.08, 0.45);
        let len = refw * 0.2 * (1.2 - 0.7 * z);
        let speed = g.range(6.5, 9.5) * unit;
        let ymid = g.range(0.3, 0.45) * h;
        let r = (g.range(0.06, 0.14) * h).min(ymid - 0.12 * h).max(2.0);
        let path = Path::calm(&mut g, speed as f64, ymid as f64, r as f64);
        let start = k as f64 * WINDOW + g.range(4.0, 40.0) as f64;
        // Enter from the edge it is heading away from, then leave when it has crossed and gone.
        let heading_right = path.vel(start).0 >= 0.0;
        let entry = if heading_right { -1.6 * len } else { pano + 1.6 * len } as f64;
        let shift_x = entry - path.pos(start).0;
        let (lo, hi) = (-1.8 * len as f64, pano as f64 + 1.8 * len as f64);
        let mut dur = 190.0;
        for s in 10..190 {
            let x = path.pos(start + s as f64).0 + shift_x;
            if x < lo || x > hi {
                dur = s as f64;
                break;
            }
        }
        Some(Self { path, shift_x, start, dur, z, len, reach: 0.45 * h })
    }

    /// 0 outside the visit, 1 while it is fully there, easing over the first and last seconds.
    pub fn envelope(&self, t: f64) -> f32 {
        let el = (t - self.start) as f32;
        if el < 0.0 || el > self.dur as f32 {
            return 0.0;
        }
        smooth(el / 4.0).min(smooth((self.dur as f32 - el) / 4.0))
    }

    /// Head position in panorama coordinates (0 is the left edge of the first screen).
    pub fn head(&self, t: f64) -> (f32, f32) {
        let (x, y) = self.path.pos(t);
        ((x + self.shift_x) as f32, y as f32)
    }

    /// How far a fish at (x, y) is shoved away from the head, as a vector.
    pub fn push(&self, t: f64, x: f32, y: f32) -> (f32, f32) {
        let env = self.envelope(t);
        if env <= 0.0 {
            return (0.0, 0.0);
        }
        let (hx, hy) = self.head(t);
        let (dx, dy) = (x - hx, y - hy);
        let d = dx.hypot(dy);
        if d >= self.reach {
            return (0.0, 0.0);
        }
        let (ux, uy) = if d > 1e-3 { (dx / d, dy / d) } else { (1.0, 0.0) };
        let k = env * (1.0 - d / self.reach).powi(2) * self.reach * 0.9;
        (ux * k, uy * k * 0.7)
    }

    /// Points along the body from the head, evenly spaced by arc length.
    fn spine(&self, t: f64) -> Vec<(f32, f32)> {
        let step = self.len / SEGMENTS as f32;
        let mut out = vec![self.head(t)];
        let mut prev = out[0];
        let mut acc = 0.0f32;
        let mut j = 1;
        while out.len() <= SEGMENTS && j < 400 {
            let p = self.head(t - j as f64 * 0.15);
            let seg = (p.0 - prev.0).hypot(p.1 - prev.1);
            // A point of the chain falls inside this stretch of route: interpolate to it.
            while out.len() <= SEGMENTS && acc + seg >= out.len() as f32 * step && seg > 1e-6 {
                let f = (out.len() as f32 * step - acc) / seg;
                out.push((prev.0 + (p.0 - prev.0) * f, prev.1 + (p.1 - prev.1) * f));
            }
            acc += seg;
            prev = p;
            j += 1;
        }
        // A shark that has not swum its own length yet (just entered): trail straight behind.
        while out.len() <= SEGMENTS {
            let last = *out.last().unwrap();
            out.push((last.0 - step * self.dir_x(t), last.1));
        }
        out
    }

    fn dir_x(&self, t: f64) -> f32 {
        if self.path.vel(t).0 >= 0.0 { 1.0 } else { -1.0 }
    }

    /// Draws the shark. `off` is this screen's offset in the panorama.
    pub fn draw(&self, f: &mut Frame, t: f64, off: f32, fog: (Rgb, f32), clock: f32) {
        let env = self.envelope(t);
        if env <= 0.0 {
            return;
        }
        let n = SEGMENTS;
        let l = self.len;
        let mut sp = self.spine(t);
        // Lateral undulation, growing toward the tail.
        let raw = sp.clone();
        for i in 0..=n {
            let (a, b) = (raw[i.saturating_sub(1)], raw[(i + 1).min(n)]);
            let (tx, ty) = (a.0 - b.0, a.1 - b.1);
            let m = tx.hypot(ty).max(1e-3);
            let (px, py) = (-ty / m, tx / m);
            let s = i as f32 / n as f32;
            let amp = l * 0.05 * s.powf(1.4) * (clock * 3.0 - s * 5.0).sin();
            sp[i] = (raw[i].0 + px * amp - off, raw[i].1 + py * amp);
        }
        let (min_x, max_x) = sp.iter().fold((f32::MAX, f32::MIN), |a, p| (a.0.min(p.0), a.1.max(p.0)));
        if max_x < -l * 0.3 || min_x > f.w as f32 + l * 0.3 {
            return;
        }
        let tan = |i: usize| {
            let (a, b) = (sp[i.saturating_sub(1)], sp[(i + 1).min(n)]);
            let (tx, ty) = (b.0 - a.0, b.1 - a.1); // head to tail
            let m = tx.hypot(ty).max(1e-3);
            (tx / m, ty / m)
        };
        // Normal pointing up the screen, so the back is always on top.
        let up = |i: usize| {
            let (tx, ty) = tan(i);
            let (nx, ny) = (ty, -tx);
            if ny <= 0.0 { (nx, ny) } else { (-nx, -ny) }
        };
        let maxr = l * 0.105;
        let radius = |s: f32| -> f32 {
            let r = if s < 0.28 { (s / 0.28).sqrt() * 0.95 + 0.05 } else { (1.0 - 0.82 * ((s - 0.28) / 0.72).powf(1.2)).max(0.12) };
            r * maxr
        };
        let top = [74, 98, 124];
        let belly = [214, 224, 230];
        let fin = [58, 78, 102];
        let dark = [20, 28, 40];
        let put = |f: &mut Frame, x: i32, y: i32, c: Rgb| {
            let c = lerp(c, fog.0, fog.1);
            if env >= 0.99 { f.set(x, y, c) } else { f.blend(x, y, c, env) }
        };
        let tri = |f: &mut Frame, a: (f32, f32), b: (f32, f32), c: (f32, f32), col: Rgb| {
            let (x0, x1) = (a.0.min(b.0).min(c.0).floor() as i32, a.0.max(b.0).max(c.0).ceil() as i32);
            let (y0, y1) = (a.1.min(b.1).min(c.1).floor() as i32, a.1.max(b.1).max(c.1).ceil() as i32);
            let d = (b.1 - c.1) * (a.0 - c.0) + (c.0 - b.0) * (a.1 - c.1);
            if d.abs() < 1e-3 {
                return;
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                    let w1 = ((b.1 - c.1) * (px - c.0) + (c.0 - b.0) * (py - c.1)) / d;
                    let w2 = ((c.1 - a.1) * (px - c.0) + (a.0 - c.0) * (py - c.1)) / d;
                    if w1 >= 0.0 && w2 >= 0.0 && w1 + w2 <= 1.0 {
                        put(f, x, y, col);
                    }
                }
            }
        };
        let at = |i: usize, side: f32, k: f32| -> (f32, f32) {
            // A point at node i, `side` body radii up (+) or down (-), `k` pixels toward the tail.
            let s = i as f32 / n as f32;
            let (nx, ny) = up(i);
            let (tx, ty) = tan(i);
            (sp[i].0 + nx * radius(s) * side + tx * k, sp[i].1 + ny * radius(s) * side + ty * k)
        };
        let idx = |s: f32| ((s * n as f32).round() as usize).min(n);

        // Fins behind the body: second dorsal, pectoral on the far side, tail.
        let tail = sp[n];
        let (tx, ty) = tan(n);
        let (nx, ny) = up(n);
        let base = at(idx(0.9), 0.0, 0.0);
        tri(f, base, (tail.0 + tx * l * 0.07 + nx * l * 0.17, tail.1 + ty * l * 0.07 + ny * l * 0.17), tail, fin);
        tri(f, base, (tail.0 + tx * l * 0.05 - nx * l * 0.1, tail.1 + ty * l * 0.05 - ny * l * 0.1), tail, lerp(fin, dark, 0.2));

        // Body: circles from tail to head so the head ends up on top.
        for i in (0..=n).rev() {
            let s = i as f32 / n as f32;
            let r = radius(s).max(1.0);
            let (nx, ny) = up(i);
            let (cx, cy) = sp[i];
            let (x0, x1, y0, y1) = ((cx - r).floor() as i32, (cx + r).ceil() as i32, (cy - r).floor() as i32, (cy + r).ceil() as i32);
            if x1 < 0 || x0 >= f.w as i32 {
                continue;
            }
            for y in y0..=y1 {
                for x in x0..=x1 {
                    let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                    let d2 = dx * dx + dy * dy;
                    if d2 > r * r {
                        continue;
                    }
                    let side = (dx * nx + dy * ny) / r; // +1 back, -1 belly
                    let mut c = lerp(belly, top, smooth((side + 0.15) / 0.75));
                    if d2 > r * r * 0.93 {
                        c = lerp(c, dark, 0.18);
                    }
                    put(f, x, y, c);
                }
            }
        }

        // Dorsal and pectoral fins on top of the body, swept toward the tail.
        let (i0, i1) = (idx(0.30), idx(0.42));
        let d_tip = {
            let (nx, ny) = up(idx(0.36));
            let (tx, ty) = tan(idx(0.36));
            (sp[idx(0.36)].0 + nx * (radius(0.36) + l * 0.13) + tx * l * 0.08, sp[idx(0.36)].1 + ny * (radius(0.36) + l * 0.13) + ty * l * 0.08)
        };
        tri(f, at(i0, 0.9, 0.0), at(i1, 0.9, 0.0), d_tip, fin);
        let p_tip = {
            let (nx, ny) = up(idx(0.27));
            let (tx, ty) = tan(idx(0.27));
            (sp[idx(0.27)].0 - nx * (radius(0.27) + l * 0.1) + tx * l * 0.1, sp[idx(0.27)].1 - ny * (radius(0.27) + l * 0.1) + ty * l * 0.1)
        };
        tri(f, at(idx(0.24), -0.85, 0.0), at(idx(0.32), -0.85, 0.0), p_tip, lerp(fin, belly, 0.2));

        // Eye, gill slits and mouth.
        let e = at(idx(0.07), 0.38, 0.0);
        put(f, e.0 as i32, e.1 as i32, dark);
        if l > 60.0 {
            put(f, e.0 as i32 + 1, e.1 as i32, dark);
        }
        for s in [0.22, 0.25, 0.28] {
            let a = at(idx(s), 0.55, 0.0);
            let b = at(idx(s), -0.2, 0.0);
            let steps = ((a.0 - b.0).abs().max((a.1 - b.1).abs())) as i32 + 1;
            for k in 0..=steps {
                let u = k as f32 / steps as f32;
                put(f, (a.0 + (b.0 - a.0) * u) as i32, (a.1 + (b.1 - a.1) * u) as i32, lerp(dark, top, 0.4));
            }
        }
        for s in [0.04, 0.07, 0.10, 0.13] {
            let m = at(idx(s), -0.45, 0.0);
            put(f, m.0 as i32, m.1 as i32, dark);
        }
    }

}

#[cfg(test)]
impl SharkRun {
    fn len(&self) -> f32 {
        self.len
    }
}

#[cfg(test)]
pub(crate) fn shark_reach(s: &SharkRun) -> f32 {
    s.reach
}

#[cfg(test)]
mod tests {
    use super::*;

    const PANO: f32 = 640.0;
    const H: f32 = 360.0;

    fn run() -> SharkRun {
        let mut k = 0.0;
        loop {
            if let Some(s) = SharkRun::plan(k * WINDOW, PANO, H, H * 16.0 / 9.0, 3.0) {
                return s;
            }
            k += 1.0;
        }
    }

    #[test]
    fn some_windows_have_a_shark_and_the_plan_is_deterministic() {
        let hits = (0..400).filter(|k| SharkRun::plan(*k as f64 * WINDOW + 1.0, PANO, H, 640.0, 3.0).is_some()).count();
        assert!((100..260).contains(&hits), "{hits} of 400 windows");
        let (a, b) = (run(), run());
        assert_eq!(a.head(a.start + 20.0), b.head(b.start + 20.0));
    }

    #[test]
    fn it_swims_in_from_off_screen_and_fades_in_and_out() {
        let s = run();
        let (x0, _) = s.head(s.start);
        assert!(x0 < -s.len() || x0 > PANO + s.len(), "enters from outside the panorama: {x0}");
        assert_eq!(s.envelope(s.start - 1.0), 0.0);
        assert!(s.envelope(s.start + 2.0) < 1.0 && s.envelope(s.start + 10.0) > 0.99);
        assert_eq!(s.envelope(s.start + s.dur + 1.0), 0.0);
        // It does cross the panorama while it is there.
        let xs: Vec<f32> = (0..s.dur as i32).map(|k| s.head(s.start + k as f64).0).collect();
        assert!(xs.iter().any(|x| (0.0..PANO).contains(x)) || s.dur < 15.0);
    }

    #[test]
    fn fish_are_pushed_away_only_when_close_and_only_while_it_is_there() {
        let s = run();
        let t = s.start + 12.0;
        let (hx, hy) = s.head(t);
        let near = s.push(t, hx + s.len() * 0.3, hy);
        assert!(near.0 > 5.0, "pushed along +x, away from the head: {near:?}");
        let far = s.push(t, hx + s.reach * 1.2, hy);
        assert_eq!(far, (0.0, 0.0));
        let closer = s.push(t, hx + s.len() * 0.1, hy);
        assert!(closer.0 > near.0, "stronger when closer");
        assert_eq!(s.push(s.start + s.dur + 5.0, hx, hy), (0.0, 0.0), "gone: the school closes again");
    }

    #[test]
    fn the_body_keeps_its_length_through_turns() {
        let s = run();
        for k in 8..(s.dur as i32 - 2) {
            let sp = s.spine(s.start + k as f64);
            assert_eq!(sp.len(), SEGMENTS + 1);
            let total: f32 = sp.windows(2).map(|w| (w[1].0 - w[0].0).hypot(w[1].1 - w[0].1)).sum();
            assert!(total > s.len() * 0.85 && total < s.len() * 1.1, "t+{k}: body {total} vs {}", s.len());
        }
    }

    #[test]
    fn it_draws_something_near_its_head_and_nothing_when_absent() {
        let s = run();
        let mut f = Frame::new(PANO as usize, H as usize);
        s.draw(&mut f, s.start + s.dur + 30.0, 0.0, ([0, 0, 0], 0.0), 0.0);
        assert!((0..f.h).all(|y| (0..f.w).all(|x| f.at(x, y) == [0, 0, 0])));
        // Find a moment when it is on screen.
        let t = (10..s.dur as i32).map(|k| s.start + k as f64).find(|t| (40.0..PANO - 40.0).contains(&s.head(*t).0)).expect("on screen at some point");
        s.draw(&mut f, t, 0.0, ([0, 0, 0], 0.0), 0.0);
        let painted = (0..f.h).flat_map(|y| (0..f.w).map(move |x| (x, y))).filter(|(x, y)| f.at(*x, *y) != [0, 0, 0]).count();
        assert!(painted as f32 > s.len() * s.len() * 0.03, "{painted} px painted");
    }
}

#[cfg(test)]
mod probe {
    use super::*;

    /// `cargo test --release shark_windows -- --ignored --nocapture`: when does a visit start (for screenshots)?
    #[test]
    #[ignore]
    fn shark_windows() {
        let base = 1_700_000_000.0;
        for k in 0..12 {
            let t = base + k as f64 * WINDOW;
            if let Some(s) = SharkRun::plan(t, 960.0, 540.0, 960.0, 4.36) {
                println!("visit: start {:.0} dur {:.0} (use --t {:.0})", s.start, s.dur, s.start + 25.0);
            }
        }
    }
}
