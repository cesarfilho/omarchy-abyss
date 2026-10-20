//! Swimming routes that are a pure function of the clock. A route is a heading that wanders
//! (a few slow sines plus a quick wiggle, periodic over `PERIOD`), integrated once into a table;
//! `pos(t)` then reads the table with a cubic, so every monitor gets the same fish without any
//! per-frame state. Because the heading is periodic, one period moves the fish by a fixed
//! displacement and the path simply repeats shifted by it.

use std::f64::consts::TAU;

use crate::world::Gen;

/// Half an hour: long enough that nobody notices the wandering repeating.
pub const PERIOD: f64 = 1800.0;
const STEP: f64 = 0.5;
const N: usize = (PERIOD / STEP) as usize;
/// A side view: vertical motion is squeezed so fish cruise mostly level.
const VY_K: f64 = 0.45;
/// Start heading stays within this cone of horizontal so no fish hovers in one column.
const CONE: f64 = 65.0 * TAU / 360.0;

pub struct Path {
    x: Vec<f64>,
    u: Vec<f64>,
    dx: f64,
    du: f64,
    x0: f64,
    t0: f64,
    ymid: f64,
    r: f64,
}

fn cubic(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    0.5 * (2.0 * p1 + (p2 - p0) * t + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t * t + (3.0 * p1 - p0 - 3.0 * p2 + p3) * t * t * t)
}

/// Sample `j` of a table that continues past its end shifted by `d` per period.
fn sample(v: &[f64], d: f64, j: i64) -> f64 {
    let n = N as i64;
    v[j.rem_euclid(n) as usize] + j.div_euclid(n) as f64 * d
}

impl Path {
    /// `speed` in pixels per second; the fish stays within `ymid ± r` vertically.
    pub fn new(g: &mut Gen, speed: f64, ymid: f64, r: f64) -> Self {
        Self::build(g, speed, ymid, r, 1.0, CONE)
    }

    /// A long, stiff animal (a shark): gentle turns that never fold the body onto itself, and a
    /// start heading close to level.
    pub fn calm(g: &mut Gen, speed: f64, ymid: f64, r: f64) -> Self {
        Self::build(g, speed, ymid, r, 0.3, 25.0 * TAU / 360.0)
    }

    fn build(g: &mut Gen, speed: f64, ymid: f64, r: f64, agility: f64, cone: f64) -> Self {
        let mut f = |lo: f64, hi: f64| g.range(lo as f32, hi as f32) as f64;
        let side = if f(0.0, 1.0) < 0.5 { 0.0 } else { std::f64::consts::PI };
        let theta0 = side + f(-cone, cone);
        let (a1, a2, a3) = (f(0.4, 1.4) * agility, f(0.2, 0.9) * agility, f(0.1, 0.35) * agility * agility);
        let (k1, k2, k3, k4) = (f(1.0, 3.99).floor(), f(3.0, 7.99).floor(), f(12.0, 30.99).floor(), f(4.0, 9.99).floor());
        let (p1, p2, p3, p4) = (f(0.0, TAU), f(0.0, TAU), f(0.0, TAU), f(0.0, TAU));
        let x0 = f(0.0, 1.0e6);
        let t0 = f(0.0, PERIOD);

        let (mut x, mut u) = (Vec::with_capacity(N), Vec::with_capacity(N));
        let (mut ax, mut au) = (0.0, 0.0);
        for i in 0..N {
            x.push(ax);
            u.push(au);
            let tau = (i as f64 + 0.5) * STEP / PERIOD * TAU;
            let theta = theta0 + a1 * (k1 * tau + p1).sin() + a2 * (k2 * tau + p2).sin() + a3 * (k3 * tau + p3).sin();
            let s = speed * (1.0 + 0.2 * (k4 * tau + p4).sin());
            ax += s * theta.cos() * STEP;
            au += s * VY_K * theta.sin() * STEP;
        }
        Self { x, u, dx: ax, du: au, x0, t0, ymid, r: r.max(1.0) }
    }

    /// Position at wall-clock time `t`: x is unbounded (wrap it into the panorama), y is within the band.
    pub fn pos(&self, t: f64) -> (f64, f64) {
        let t = t + self.t0;
        let n = (t / PERIOD).floor();
        let f = (t - n * PERIOD) / STEP;
        let i = f.floor();
        let fr = f - i;
        let i = i as i64;
        let cx = cubic(sample(&self.x, self.dx, i - 1), sample(&self.x, self.dx, i), sample(&self.x, self.dx, i + 1), sample(&self.x, self.dx, i + 2), fr);
        let cu = cubic(sample(&self.u, self.du, i - 1), sample(&self.u, self.du, i), sample(&self.u, self.du, i + 1), sample(&self.u, self.du, i + 2), fr);
        let u = n * self.du + cu;
        (self.x0 + n * self.dx + cx, self.ymid + self.r * (u / self.r).sin())
    }

    /// Velocity in pixels per second (central difference).
    pub fn vel(&self, t: f64) -> (f64, f64) {
        let (a, b) = (self.pos(t - 0.25), self.pos(t + 0.25));
        ((b.0 - a.0) / 0.5, (b.1 - a.1) / 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(seed: u64) -> Path {
        Path::new(&mut Gen(crate::world::mix(7, seed)), 20.0, 100.0, 40.0)
    }

    #[test]
    fn the_route_is_continuous_across_period_boundaries() {
        let p = path(1);
        for k in [0.0, 1.0, 5.0, 1_000_000.0] {
            let edge = (k + 1.0) * PERIOD - p.t0;
            let (a, b) = (p.pos(edge - 0.01), p.pos(edge + 0.01));
            assert!((a.0 - b.0).abs() < 1.0 && (a.1 - b.1).abs() < 1.0, "jump at period {k}: {a:?} -> {b:?}");
            let (va, vb) = (p.vel(edge - 0.3), p.vel(edge + 0.3));
            assert!((va.0 - vb.0).abs() < 4.0, "velocity jump at period {k}");
        }
    }

    #[test]
    fn one_period_shifts_x_by_a_fixed_displacement() {
        let p = path(2);
        let d0 = p.pos(1234.5 + PERIOD).0 - p.pos(1234.5).0;
        let d1 = p.pos(9_999_999.0 + PERIOD).0 - p.pos(9_999_999.0).0;
        assert!((d0 - d1).abs() < 1e-3 && (d0 - p.dx).abs() < 1e-3);
    }

    #[test]
    fn height_stays_inside_the_band_and_speed_is_sane() {
        for seed in 0..50 {
            let p = path(seed);
            for k in 0..400 {
                let t = 1.7e9 + k as f64 * 7.3;
                let (_, y) = p.pos(t);
                assert!((60.0 - 1e-6..=140.0 + 1e-6).contains(&y), "seed {seed}: y = {y}");
                let (vx, vy) = p.vel(t);
                assert!(vx.hypot(vy) < 40.0, "seed {seed}: speed {}", vx.hypot(vy));
            }
        }
    }

    #[test]
    fn fish_head_every_which_way() {
        let (mut right, mut left, mut up, mut down) = (0, 0, 0, 0);
        for seed in 0..200 {
            let p = path(seed);
            let (vx, vy) = p.vel(1.7e9);
            if vx > 0.0 { right += 1 } else { left += 1 }
            if vy > 1.0 { down += 1 }
            if vy < -1.0 { up += 1 }
        }
        assert!((60..=140).contains(&right) && (60..=140).contains(&left), "{right} right / {left} left");
        assert!(up > 20 && down > 20, "{up} up / {down} down");
    }

    #[test]
    fn same_seed_same_route() {
        let (a, b) = (path(3), path(3));
        assert_eq!(a.pos(1.7e9), b.pos(1.7e9));
        assert_ne!(path(4).pos(1.7e9), a.pos(1.7e9));
    }
}
