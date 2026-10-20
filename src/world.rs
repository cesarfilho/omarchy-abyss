mod landmarks;
mod scenery;

use crate::gfx::{Frame, Rgb, lerp};
use crate::path::Path;
use crate::shark::{self, SharkRun};
#[cfg(test)]
use crate::shark::shark_reach;
use crate::sprites::{self, SPECIES};
use crate::theme::Palette;

/// (top, bottom) water colour per zone.
const ZONE_WATER: [(Rgb, Rgb); 6] = [
    ([110, 215, 235], [30, 130, 190]),
    ([40, 140, 200], [15, 80, 150]),
    ([20, 80, 140], [10, 40, 90]),
    ([12, 40, 80], [6, 16, 40]),
    ([8, 20, 45], [3, 8, 20]),
    ([4, 10, 24], [1, 3, 10]),
];

/// Pixels per "art pixel", from the reference width (height-based, so every monitor agrees).
fn scale_for(refw: f32) -> i32 {
    ((refw / 220.0).round() as i32).clamp(1, 6)
}

/// Seabed planes, 0 = nearest .. 4 = farthest: (base height, amplitude, freq 1, freq 2, parallax speed, sand mix).
const PLANES: [(f32, f32, f32, f32, f32, f32); 5] = [
    (0.07, 0.030, 0.047, 0.113, 1.00, 0.80),
    (0.10, 0.035, 0.040, 0.095, 0.78, 0.62),
    (0.13, 0.040, 0.034, 0.081, 0.55, 0.45),
    (0.16, 0.045, 0.027, 0.066, 0.38, 0.30),
    (0.19, 0.050, 0.021, 0.057, 0.25, 0.20),
];

/// Depth z runs 0 (nearest) .. 1 (farthest). Size and speed shrink with distance.
fn scale_z(z: f32) -> f32 {
    1.2 - 0.7 * z
}

fn slot_of(z: f32) -> usize {
    (z.clamp(0.0, 1.0) * 4.0).round() as usize
}

const BAYER: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
const DITHER_STEP: f32 = 16.0;
pub(crate) const SEED: u64 = 0x000A_B155_0CEA;
const WHALE_WINDOW: f64 = 300.0;
const BLOOM_WINDOW: f64 = 240.0;

#[derive(Debug, PartialEq)]
pub enum WorldEvent {
    WhalePassed,
    /// A creature (bestiary id) is on screen.
    Spotted(&'static str),
}

/// Where this screen sits in the shared panorama, in frame pixels. Every monitor runs the same
/// world from the same clock and draws its own slice, so a creature leaving one screen enters the next.
#[derive(Clone, Copy)]
pub struct Layout {
    pub pano_w: usize,
    pub off: usize,
}

impl Layout {
    pub fn single(w: usize) -> Self {
        Self { pano_w: w, off: 0 }
    }
}

pub(crate) fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic stream of floats in [0, 1) for world generation.
pub(crate) struct Gen(pub(crate) u64);

impl Gen {
    pub(crate) fn f(&mut self) -> f32 {
        self.0 = mix(self.0, 1);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    pub(crate) fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }

    pub(crate) fn below(&mut self, n: u32) -> u32 {
        (self.f() * n as f32) as u32 % n.max(1)
    }

    pub(crate) fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

struct Fish {
    species: usize,
    z: f32,
    /// Route shared by the whole school (a loner has its own); followers trail the leader.
    path: usize,
    delay: f32,
    ox: f32,
    oy: f32,
    amp: f32,
    freq: f32,
    phase: f32,
}

/// Where a fish is on this screen and which way it is heading.
struct Pose {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

struct Bubble {
    z: f32,
    x: f32,
    y0: f32,
    vy: f32,
    phase: f32,
    big: bool,
}

struct Snow {
    x: f32,
    y0: f32,
    vy: f32,
}

struct Jelly {
    z: f32,
    path: Path,
    phase: f32,
}

struct Whale {
    x: f32,
    y: f32,
    dir: f32,
    len: f32,
}

struct Glow {
    x: f32,
    y: f32, // fraction of the height
    phase: f32,
    hue: f32,
}

pub struct World {
    pub w: usize,
    pub h: usize,
    pano: f32,
    off: f32,
    refw: f32,
    px_scale: i32,
    tt: f64,
    t: f32,
    zf: f32,
    reef: u32,
    night: bool,
    fishes: Vec<Fish>,
    paths: Vec<Path>,
    shark: Option<SharkRun>,
    /// Snapshot/test aid: show this landmark (index into `KINDS`) on every candidate spot.
    force_landmark: Option<usize>,
    shark_key: Option<(u64, usize, usize, bool)>,
    bubbles: Vec<Bubble>,
    snow: Vec<Snow>,
    jellies: Vec<Jelly>,
    glows: Vec<Glow>,
    whale_active: bool,
    gen_zone: usize,
    gen_reef: u32,
    /// Target zone from the save (the world builds its population from this, not from the eased `zf`).
    tz: usize,
    /// Fixed clock for tests and snapshots; the wall clock otherwise.
    fixed: Option<f64>,
    pal: Palette,
    season: Option<&'static str>,
    present: Vec<&'static str>,
    announced: Vec<&'static str>,
}

impl World {
    pub fn new(w: usize, h: usize, layout: Layout, zf: f32, reef: u32, night: bool) -> Self {
        let mut world = Self {
            w,
            h,
            pano: layout.pano_w.max(w) as f32,
            off: layout.off as f32,
            refw: h as f32 * 16.0 / 9.0,
            px_scale: 1,
            tt: now_secs(),
            t: 0.0,
            zf,
            reef,
            night,
            fishes: Vec::new(),
            paths: Vec::new(),
            shark: None,
            force_landmark: None,
            shark_key: None,
            bubbles: Vec::new(),
            snow: Vec::new(),
            jellies: Vec::new(),
            glows: Vec::new(),
            whale_active: false,
            gen_zone: usize::MAX,
            gen_reef: u32::MAX,
            tz: (zf as usize).min(5),
            fixed: None,
            pal: Palette::classic(),
            season: None,
            present: Vec::new(),
            announced: Vec::new(),
        };
        world.px_scale = scale_for(world.refw);
        world.t = (world.tt % 100_000.0) as f32;
        world.populate();
        world
    }

    /// Freezes the world at wall-clock second `t` (tests and snapshots).
    pub fn at(mut self, t: f64) -> Self {
        self.fixed = Some(t);
        self.tt = t;
        self.t = (t % 100_000.0) as f32;
        self.populate();
        self.refresh_shark();
        self
    }

    fn now(&self) -> f64 {
        self.fixed.unwrap_or_else(now_secs)
    }

    /// Plans the shark's visit for the current window (or none). Cached until the window, the
    /// screen size or the zone changes.
    fn refresh_shark(&mut self) {
        let k = (self.tt / shark::WINDOW).floor() as u64;
        let key = (k, self.pano as usize, self.h, self.tz <= 3);
        if self.shark_key == Some(key) {
            return;
        }
        self.shark_key = Some(key);
        self.shark = if self.tz <= 3 { SharkRun::plan(self.tt, self.pano, self.h as f32, self.refw, self.unit()) } else { None };
    }

    /// Shows one landmark kind on every spot (used by `abyss snapshot --landmark`).
    pub fn force_landmark(&mut self, id: &str) {
        self.force_landmark = landmarks::KINDS.iter().position(|k| k.0 == id);
    }

    pub fn set_palette(&mut self, pal: Palette) {
        self.pal = pal;
    }

    pub fn set_season(&mut self, season: Option<&'static str>) {
        if self.season != season {
            self.season = season;
            self.populate();
        }
    }

    fn is_season(&self, id: &str) -> bool {
        self.season == Some(id)
    }

    /// Screen-proportional speed unit: movement scales with the frame like the creatures do.
    fn unit(&self) -> f32 {
        self.refw / 220.0
    }

    fn zone(&self) -> usize {
        (self.zf as usize).min(5)
    }

    fn margin(&self) -> f64 {
        0.1 * self.refw as f64
    }

    fn track_len(&self) -> f64 {
        self.pano as f64 + 2.0 * self.margin()
    }

    fn cam64(&self) -> f64 {
        self.tt * 5.0 * self.unit() as f64
    }

    /// Builds every moving thing from fixed seeds, so all monitors get the identical world.
    fn populate(&mut self) {
        self.gen_zone = self.tz;
        self.gen_reef = self.reef;
        self.fishes.clear();
        let mut g = Gen(mix(SEED, 100 + self.gen_zone as u64));
        let target = 8 + self.reef as usize;
        let ok: Vec<usize> = SPECIES
            .iter()
            .enumerate()
            .filter(|(_, s)| (s.zones.0..=s.zones.1).contains(&self.gen_zone))
            .map(|(i, _)| i)
            .collect();
        self.present = ok.iter().map(|&i| SPECIES[i].id).collect();
        self.paths.clear();
        let h = self.h as f32;
        while !ok.is_empty() && self.fishes.len() < target {
            let species = ok[g.below(ok.len() as u32) as usize];
            let sp = &SPECIES[species];
            let school_z = g.range(0.0, 1.0);
            let speed = g.range(sp.speed.0, sp.speed.1) * self.unit() * scale_z(school_z);
            let sprite_w = self.refw * sp.len * scale_z(school_z);
            // Every school (or loner) picks its own heading and wanders in a band of the water column.
            let ymid = g.range(0.25, 0.5) * h;
            let r = g.range(0.05, 0.2) * h;
            let r = r.min(ymid - 0.1 * h).min(0.7 * h - ymid).max(2.0);
            let group = self.paths.len();
            self.paths.push(Path::new(&mut g, speed as f64, ymid as f64, r as f64));
            let freq = g.range(0.6, 1.4);
            for i in 0..sp.school {
                self.fishes.push(Fish {
                    species,
                    z: (school_z + g.range(-0.04, 0.04)).clamp(0.0, 1.0),
                    path: group,
                    // Followers trail the leader along its own route, so the school turns together.
                    delay: i as f32 * sprite_w * 1.5 / speed.max(1.0) + g.range(-0.5, 0.5),
                    ox: g.range(-0.4, 0.4) * sprite_w,
                    oy: g.range(-0.5, 0.5) * sprite_w,
                    amp: g.range(1.0, 3.5) * scale_z(school_z),
                    freq,
                    phase: g.range(0.0, std::f32::consts::TAU),
                });
            }
        }

        let mut g = Gen(mix(SEED, 200));
        let n_bubbles = ((self.pano / 4.0) as usize).clamp(20, 160);
        self.bubbles = (0..n_bubbles)
            .map(|_| Bubble {
                z: g.range(0.0, 1.0),
                x: g.range(0.0, self.pano),
                y0: g.range(0.0, self.h as f32 * 1.1),
                vy: g.range(6.0, 16.0) * self.unit(),
                phase: g.range(0.0, std::f32::consts::TAU),
                big: g.chance(0.2),
            })
            .collect();
        let n_snow = ((self.pano / 6.0) as usize).min(220);
        self.snow = (0..n_snow)
            .map(|_| Snow { x: g.range(0.0, self.pano), y0: g.range(0.0, self.h as f32), vy: g.range(1.5, 5.0) * self.unit() })
            .collect();
        self.jellies = (0..5)
            .map(|_| {
                let z = g.range(0.0, 0.9);
                let ymid = g.range(0.25, 0.5) * h;
                let r = (g.range(0.05, 0.15) * h).min(ymid - 0.12 * h).max(2.0);
                let speed = g.range(2.0, 5.0) * self.unit();
                Jelly { z, path: Path::new(&mut g, speed as f64, ymid as f64, r as f64), phase: g.range(0.0, std::f32::consts::TAU) }
            })
            .collect();
        let n_glows = (70.0 * self.pano / self.refw) as usize;
        self.glows = (0..n_glows)
            .map(|_| Glow { x: g.range(0.0, self.pano), y: g.range(0.0, 0.9), phase: g.range(0.0, std::f32::consts::TAU), hue: g.f() })
            .collect();
    }

    pub fn resize(&mut self, w: usize, h: usize, layout: Layout) {
        self.w = w;
        self.h = h;
        self.pano = layout.pano_w.max(w) as f32;
        self.off = layout.off as f32;
        self.refw = h as f32 * 16.0 / 9.0;
        self.px_scale = scale_for(self.refw);
        self.populate();
        self.refresh_shark();
    }

    /// The whale schedule is a pure function of the clock: every 5-minute window may hold one pass.
    fn whale_state(&self, tt: f64) -> Option<Whale> {
        if self.zf >= 4.0 {
            return None;
        }
        let odds = if self.is_season("oceans") { 90 } else { 40 };
        let k = (tt / WHALE_WINDOW).floor();
        let r = mix(SEED, 300 + k as u64);
        if r % 100 >= odds {
            return None;
        }
        let len = (self.refw * 0.2).clamp(34.0, 130.0);
        let speed = 7.0 * self.unit();
        let dur = ((self.pano + 3.0 * len) / speed) as f64;
        let start = ((r >> 8) % 1000) as f64 / 1000.0 * (WHALE_WINDOW - dur).max(1.0);
        let el = tt - k * WHALE_WINDOW - start;
        if el < 0.0 || el >= dur {
            return None;
        }
        let dir = if (r >> 20) & 1 == 0 { 1.0 } else { -1.0 };
        let x = if dir > 0.0 { -1.5 * len + speed * el as f32 } else { self.pano + 1.5 * len - speed * el as f32 };
        Some(Whale { x, y: (0.18 + ((r >> 24) % 100) as f32 / 100.0 * 0.24) * self.h as f32, dir, len })
    }

    /// Bioluminescent bloom strength 0..1 (a pure function of the clock).
    fn bloom_strength(&self) -> f32 {
        if !(self.night || self.zf >= 3.0) {
            return 0.0;
        }
        let k = (self.tt / BLOOM_WINDOW).floor();
        let r = mix(SEED, 400 + k as u64);
        if r % 100 >= 35 {
            return 0.0;
        }
        let el = (self.tt - k * BLOOM_WINDOW - (r >> 8) as f64 % 150.0) as f32;
        if !(0.0..25.0).contains(&el) {
            return 0.0;
        }
        (el.min(3.0) / 3.0).min((25.0 - el).min(3.0) / 3.0)
    }

    pub fn update(&mut self, _dt: f32, zf: f32, reef: u32, night: bool) -> Vec<WorldEvent> {
        let mut events = Vec::new();
        self.tt = self.now();
        self.t = (self.tt % 100_000.0) as f32;
        self.zf += (zf - self.zf) * 0.02;
        self.reef = reef;
        self.night = night;
        self.tz = (zf as usize).min(5);
        self.refresh_shark();
        if self.tz != self.gen_zone || self.reef != self.gen_reef {
            self.populate();
        }
        let mut spotted: Vec<&'static str> = self.present.clone();
        if self.jelly_count() > 0 {
            spotted.push("jelly");
        }
        if self.bloom_level() > 0.35 {
            spotted.push("bloom");
        }
        spotted.extend(self.landmarks_in_view());
        if self.shark.as_ref().is_some_and(|s| s.envelope(self.tt) > 0.2) {
            spotted.push("shark");
        }
        let active = self.whale_state(self.tt).is_some();
        if active {
            spotted.push("whale");
        }
        for id in spotted {
            if !self.announced.contains(&id) {
                self.announced.push(id);
                events.push(WorldEvent::Spotted(id));
            }
        }
        if self.whale_active && !active {
            events.push(WorldEvent::WhalePassed);
        }
        self.whale_active = active;
        events
    }

    fn water(&self) -> (Rgb, Rgb) {
        let i = self.zone();
        let j = (i + 1).min(5);
        let t = self.zf - i as f32;
        let (top, bot) = (lerp(ZONE_WATER[i].0, ZONE_WATER[j].0, t), lerp(ZONE_WATER[i].1, ZONE_WATER[j].1, t));
        match self.pal.water {
            // The theme colours the shallows lightly and the deep more, since the deep is nearly black anyway.
            Some((t_top, t_bot)) => (lerp(top, t_top, 0.18), lerp(bot, t_bot, 0.3)),
            None => (top, bot),
        }
    }

    fn jelly_count(&self) -> usize {
        if self.is_season("halloween") {
            5
        } else if self.zone() >= 2 {
            self.zone()
        } else {
            0
        }
    }

    /// Bioluminescence: seasonal and night/deep baseline plus the scheduled bloom.
    fn bloom_level(&self) -> f32 {
        let deep = (self.zf / 3.0).min(1.0);
        let mut base = if self.night { 0.25 } else { 0.0 } + deep * 0.2;
        if self.is_season("halloween") || self.is_season("xmas") {
            base = base.max(0.55);
        }
        (self.bloom_strength() + base).min(1.0)
    }

    fn glow_color(&self, hue: f32) -> Rgb {
        match self.season {
            Some("halloween") => lerp([255, 140, 30], [170, 70, 230], hue),
            Some("xmas") => [[255, 70, 70], [80, 230, 110], [255, 215, 90]][(hue * 3.0) as usize % 3],
            _ => lerp(self.pal.glow.0, self.pal.glow.1, hue),
        }
    }

    fn fog(&self) -> f32 {
        (0.12 * self.zf).min(0.65)
    }

    /// Haze toward the water colour: depth of the zone plus distance from the camera.
    fn fog_z(&self, z: f32) -> f32 {
        (self.fog() + 0.45 * z).min(0.82)
    }

    pub fn draw(&self, f: &mut Frame) {
        let (top, bot) = self.water();
        self.draw_water(f, top, bot);
        self.draw_surface(f);
        self.draw_far_schools(f, bot);
        self.draw_snow(f);
        self.draw_bubbles(f, true);
        // Far to near: each plane of seabed, then what swims in front of it.
        for slot in (0..5).rev() {
            let floor = self.floor_row(slot);
            self.draw_seabed(f, slot, bot, &floor);
            self.draw_scenery(f, slot, bot, &floor);
            self.draw_landmarks(f, slot, bot);
            if slot <= 1 {
                self.draw_corals(f, bot, slot);
            }
            self.draw_shadows(f, bot, slot);
            self.draw_jellies(f, bot, slot);
            if slot == 3 {
                self.draw_whale(f, bot);
            }
            self.draw_fish(f, bot, slot);
            if let Some(sh) = self.shark.as_ref().filter(|sh| slot_of(sh.z) == slot) {
                sh.draw(f, self.tt, self.off, (bot, self.fog_z(sh.z)), self.t);
            }
            if slot == 2 {
                self.draw_bloom(f);
            }
        }
        self.draw_foreground(f, bot);
        self.draw_bubbles(f, false);
    }

    /// Vertical gradient, light rays and ordered dithering. Done a row at a time: the gradient is
    /// constant along a row and the ray pattern depends only on `x + 0.45 y`, so it comes from a
    /// table (sampled every 1/20 pixel, where `0.45 y` lands exactly on whole table steps).
    fn draw_water(&self, f: &mut Frame, top: Rgb, bot: Rgb) {
        const STEP: usize = 20;
        let ray_strength = (1.0 - self.zf / 3.0).max(0.0) * 0.8;
        let dim = 1.0 - 0.35 * if self.night { 1.0 } else { 0.0 } * (1.0 - (self.zf / 3.0).min(1.0));
        let h = self.h as f32;
        let k = self.px_scale as f32;
        let rays = ray_strength > 0.02;
        let table: Vec<f32> = if rays {
            (0..=STEP * self.w + 9 * self.h)
                .map(|i| {
                    let u = (i as f32 / STEP as f32 + self.off) / k - self.t * 2.5;
                    (u * 0.045).sin() * 0.5 + (u * 0.11 + 1.3).sin() * 0.5
                })
                .collect()
        } else {
            Vec::new()
        };
        let bayer: [[f32; 4]; 4] = BAYER.map(|r| r.map(|v| (v as f32 / 16.0 - 0.5) * DITHER_STEP));
        let inv = 1.0 / DITHER_STEP;
        let off = self.off as usize;
        for y in 0..self.h {
            let base = lerp(top, bot, y as f32 / h);
            let base = [base[0] as f32 * dim, base[1] as f32 * dim, base[2] as f32 * dim];
            let fall = 1.0 - y as f32 / h;
            let gain = 70.0 * ray_strength * fall * dim;
            let th_row = &bayer[y & 3];
            for (x, px) in f.row_mut(y).iter_mut().enumerate() {
                let mut c = base;
                if rays {
                    let v = table[STEP * x + 9 * y];
                    if v > 0.35 {
                        let add = (v - 0.35) * gain;
                        c[0] += add * 0.6;
                        c[1] += add * 0.9;
                        c[2] += add;
                    }
                }
                let th = th_row[(x + off) & 3];
                // `as i32` truncates, so +0.5 makes it a round for the non-negative range that matters.
                let q = |v: f32| ((((v + th) * inv + 0.5) as i32) * DITHER_STEP as i32).clamp(0, 255) as u8;
                *px = [q(c[0]), q(c[1]), q(c[2])];
            }
        }
    }

    fn draw_snow(&self, f: &mut Frame) {
        if self.zf < 1.0 {
            return;
        }
        let a = (self.zf / 3.0).min(0.7);
        let hh = self.h as f64;
        for s in &self.snow {
            let y = ((s.y0 as f64 + s.vy as f64 * self.tt) % hh) as i32;
            let x = (s.x - self.off) as i32;
            if x >= -4 && x < self.w as i32 + 4 {
                f.dot(x, y, self.px_scale, [200, 215, 230], a * 0.6);
            }
        }
    }

    fn floor_y(&self, slot: usize, x: f32) -> f32 {
        let h = self.h as f64;
        let (base, amp, f1, f2, speed, _) = PLANES[slot.min(4)];
        let gx = x as f64 + self.off as f64;
        let wx = (gx + self.cam64() * speed as f64) / self.px_scale as f64;
        let off = slot as f64 * 17.0;
        (h - h * (base as f64 + amp as f64 * ((wx + off) * f1 as f64).sin() + amp as f64 * 0.5 * ((wx + off) * f2 as f64 + 1.0).sin())) as f32
    }

    fn sand(&self) -> Rgb {
        lerp([185, 165, 115], [58, 54, 66], (self.zf / 2.5).min(1.0))
    }

    fn draw_seabed(&self, f: &mut Frame, slot: usize, bot: Rgb, floor: &[f32]) {
        let color = lerp(bot, self.sand(), PLANES[slot].5);
        let edge = lerp(color, [255, 255, 255], 0.12);
        for (x, &fy) in floor.iter().enumerate() {
            let fy = fy as i32;
            for y in fy.max(0)..self.h as i32 {
                f.set(x as i32, y, if y == fy && slot == 0 { edge } else { color });
            }
        }
    }

    /// Panorama x (wrapped, minus the margin) to this screen's x.
    fn screen_x(&self, x: f64) -> f32 {
        let len = self.track_len();
        (x.rem_euclid(len) - self.margin()) as f32 - self.off
    }

    /// Where the fish is at time `t`, in panorama x (unwrapped) and y: its route plus offset, shoved
    /// away from the shark when one is near.
    fn fish_xy(&self, fish: &Fish, t: f64) -> (f64, f32) {
        let (x, y) = self.paths[fish.path].pos(t - fish.delay as f64);
        let (mut x, mut y) = (x + fish.ox as f64, y as f32 + fish.oy);
        if let Some(sh) = &self.shark {
            let px = (x.rem_euclid(self.track_len()) - self.margin()) as f32;
            let (dx, dy) = sh.push(t, px, y);
            x += dx as f64;
            y += dy;
        }
        (x, y)
    }

    fn fish_pose(&self, fish: &Fish) -> Pose {
        let t = self.tt;
        let (x, y) = self.fish_xy(fish, t);
        // Heading from where the fish will be a moment from now, so a fish bolting from the shark faces away.
        let (a, b) = (self.fish_xy(fish, t - 0.25), self.fish_xy(fish, t + 0.25));
        let bob = (self.t * fish.freq + fish.phase).sin() * fish.amp;
        Pose { x: self.screen_x(x), y: y + bob, vx: ((b.0 - a.0) / 0.5) as f32, vy: (b.1 - a.1) / 0.5 }
    }

    /// Soft elliptical shadows on the seabed under fish swimming close to it.
    fn draw_shadows(&self, f: &mut Frame, bot: Rgb, slot: usize) {
        if slot == 4 {
            return;
        }
        let reach = self.h as f32 * 0.3;
        let dark = lerp(bot, [0, 0, 0], 0.55);
        for fish in self.fishes.iter().filter(|fi| slot_of(fi.z) == slot) {
            let len = self.refw * SPECIES[fish.species].len * scale_z(fish.z);
            let pose = self.fish_pose(fish);
            let fx = pose.x;
            if fx < -len || fx > self.w as f32 {
                continue;
            }
            let cx = fx + len * 0.5;
            let floor = self.floor_y(slot, cx.clamp(0.0, self.w as f32 - 1.0));
            let y = pose.y;
            let dist = floor - (y + len * 0.35);
            if dist <= 0.0 || dist > reach {
                continue;
            }
            let a = 0.3 * (1.0 - dist / reach) * (1.0 - 0.5 * fish.z);
            let (rx, ry) = (len * 0.45, (len * 0.07).max(1.0));
            for yy in (-ry as i32)..=(ry as i32) {
                let span = rx * (1.0 - (yy as f32 / ry).powi(2)).max(0.0).sqrt();
                for xx in (-span as i32)..=(span as i32) {
                    f.blend(cx as i32 + xx, floor as i32 + yy, dark, a);
                }
            }
        }
    }

    /// Dark rocks and kelp in front of everything, drifting faster than any other plane.
    fn draw_foreground(&self, f: &mut Frame, bot: Rgb) {
        let h = self.h as f32;
        let color = lerp(lerp(bot, self.sand(), 0.5), [0, 0, 0], 0.5);
        let speed = 1.7;
        let sc = self.px_scale as f64;
        for x in 0..self.w {
            let wx = ((x as f64 + self.off as f64) + self.cam64() * speed) / sc;
            let top = h - h * (0.035 + 0.022 * (wx * 0.06).sin() as f32 + 0.012 * (wx * 0.17 + 2.0).sin() as f32);
            for y in top as i32..self.h as i32 {
                f.set(x as i32, y, color);
            }
        }
        // A few kelp blades swaying from the rocks.
        let unit = self.unit();
        let period = (self.pano * 1.4) as f64;
        for i in 0..5 {
            let wx = i as f64 * period / 5.0 + (i * 37 % 11) as f64 * unit as f64;
            let sx = ((wx - self.cam64() * speed).rem_euclid(period) - period * 0.1) as f32 - self.off;
            let hh = h * (0.13 + 0.03 * (i % 3) as f32);
            let base = h - h * 0.03;
            for k in 0..(hh as i32) {
                let sway = (self.t * 0.9 + k as f32 * 0.06 + i as f32).sin() * (k as f32 / hh) * 3.0 * unit;
                let px = sx + sway;
                let wdt = ((1.0 - k as f32 / hh) * 4.0 * unit).max(2.0) as i32;
                for dx in 0..wdt {
                    f.set(px as i32 + dx, (base - k as f32) as i32, color);
                }
            }
        }
    }

    fn draw_corals(&self, f: &mut Frame, bot: Rgb, slot: usize) {
        let count = (self.reef as usize).min(30);
        let z = slot as f32 / 4.0;
        let cs = scale_z(z);
        let sc = (self.unit() * cs).round().max(1.0) as i32;
        let period = (23.0 * self.unit() as f64 * 30.0).max(self.pano as f64 + 40.0 * self.unit() as f64);
        let spacing = period / 30.0;
        let fog = self.fog_z(z);
        let speed = PLANES[slot].4 as f64;
        for i in (0..count).filter(|i| i % 2 == slot) {
            let jitter = ((i * 2654435761usize) % 11) as f64 * sc as f64;
            let wx = i as f64 * spacing + jitter;
            let mut sx = (wx - self.cam64() * speed).rem_euclid(period) - self.off as f64;
            if sx > self.w as f64 + 30.0 {
                sx -= period;
            }
            if sx < -30.0 {
                continue;
            }
            let sx = sx as f32;
            let x = sx as i32;
            let fy = self.floor_y(slot, sx.clamp(0.0, self.w as f32 - 1.0)) as i32 + 1;
            let h = ((5 + (i % 4) as i32 * 2) as f32 * self.unit() * cs) as i32;
            let sway = ((self.t * 0.8 + i as f32).sin() * 1.2).round() as i32;
            let kind = i % 3;
            let base_col = [[232, 92, 142], [242, 142, 60], [152, 92, 222]][kind];
            let col = lerp(base_col, bot, fog);
            let tip = lerp(base_col, [255, 255, 255], 0.35);
            let tip = lerp(tip, bot, fog);
            match kind {
                0 => {
                    f.line(x, fy, x + sway, fy - h, col);
                    f.line(x + sway / 2, fy - h / 2, x - h / 3 + sway, fy - h * 3 / 4, col);
                    f.line(x + sway / 2, fy - h / 2, x + h / 3 + sway, fy - h * 3 / 4, col);
                    f.set(x + sway, fy - h - 1, tip);
                    f.set(x - h / 3 + sway, fy - h * 3 / 4 - 1, tip);
                    f.set(x + h / 3 + sway, fy - h * 3 / 4 - 1, tip);
                }
                1 => {
                    for a in -3..=3 {
                        f.line(x, fy, x + a * h / 4 + sway, fy - h, col);
                    }
                    for a in -3..=3 {
                        f.set(x + a * h / 4 + sway, fy - h - 1, tip);
                    }
                }
                _ => {
                    for (dx, hh) in [(-2 * sc, h * 2 / 3), (0, h), (2 * sc, h / 2)] {
                        for y in 0..hh {
                            f.set(x + dx + sway * y / hh.max(1), fy - y, col);
                            f.set(x + dx + 1 + sway * y / hh.max(1), fy - y, col);
                        }
                        f.set(x + dx + sway, fy - hh - 1, tip);
                    }
                }
            }
        }
    }

    fn draw_bubbles(&self, f: &mut Frame, far: bool) {
        let hh = self.h as f64 * 1.1;
        for b in self.bubbles.iter().filter(|b| (b.z > 0.5) == far) {
            let k = ((self.px_scale as f32 * scale_z(b.z) + 1.0) / 2.0).max(1.0) as i32;
            let y = (self.h as f64 * 1.02 - (b.y0 as f64 + b.vy as f64 * self.tt) % hh) as i32;
            let x = (b.x - self.off + (self.t * 1.6 + b.phase).sin() * 2.0 * k as f32) as i32;
            if x < -6 || x > self.w as i32 + 6 {
                continue;
            }
            let c = lerp([200, 235, 255], [90, 150, 190], b.z * 0.5);
            if b.big {
                for (dx, dy) in [(0, -1), (-1, 0), (1, 0), (0, 1)] {
                    f.dot(x + dx * k, y + dy * k, k, c, 0.75);
                }
            } else {
                f.dot(x, y, k, c, 0.8);
            }
        }
    }

    fn draw_fish(&self, f: &mut Frame, bot: Rgb, slot: usize) {
        for fish in self.fishes.iter().filter(|fi| slot_of(fi.z) == slot) {
            let sp = &SPECIES[fish.species];
            let len = (self.refw * sp.len * scale_z(fish.z)) as i32;
            let pose = self.fish_pose(fish);
            let fx = pose.x;
            if fx < -(len as f32) || fx > self.w as f32 {
                continue;
            }
            let fog = (bot, self.fog_z(fish.z));
            let floor = self.floor_y(slot, (fx + len as f32 * 0.5).clamp(0.0, self.w as f32 - 1.0));
            let y = pose.y.min(floor - len as f32 * 0.55);
            // Nose up or down with the heading (a vertical shear in the sprite), capped so a turn never rolls the fish.
            let pitch = pose.vy.atan2(pose.vx.abs().max(0.001)).clamp(-0.5, 0.5);
            // Distant fish lose their dark outline: at that size it only muddies them.
            let outline = len >= 14 && fish.z < 0.65;
            sprites::draw_fish(f, sp.kind, fx as i32, y as i32, len, pose.vx < 0.0, pitch, self.t * fish.freq + fish.phase, fog, outline);
        }
    }

    fn draw_jellies(&self, f: &mut Frame, bot: Rgb, slot: usize) {
        let target = self.jelly_count();
        for j in self.jellies.iter().take(target).filter(|j| slot_of(j.z) == slot) {
            let fog = (bot, self.fog_z(j.z) * 0.6);
            let len = (self.refw * 0.036 * scale_z(j.z)) as i32;
            let (jx, jy) = j.path.pos(self.tt);
            let x = self.screen_x(jx);
            if x < -(len as f32) || x > self.w as f32 {
                continue;
            }
            let y = jy as f32 + (self.t * 0.7 + j.phase).sin() * 6.0;
            sprites::draw_jelly(f, x as i32, y as i32, len, self.t + j.phase, fog);
        }
    }

    fn draw_bloom(&self, f: &mut Frame) {
        let a = self.bloom_level();
        if a <= 0.01 {
            return;
        }
        for g in &self.glows {
            let pulse = ((self.t * 1.3 + g.phase).sin() * 0.5 + 0.5).powi(2);
            let x = g.x - self.off + (self.t * 0.4 + g.phase).sin() * 4.0;
            if x < -6.0 || x > self.w as f32 + 6.0 {
                continue;
            }
            let y = g.y * self.h as f32 + (self.t * 0.3 + g.phase * 2.0).cos() * 3.0;
            let col = self.glow_color(g.hue);
            let k = a * pulse;
            let sc = self.px_scale;
            let (x, y) = (x as i32, y as i32);
            f.dot(x, y, sc, col, k);
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                f.dot(x + dx * sc, y + dy * sc, sc, col, k * 0.5);
            }
        }
    }

    fn draw_whale(&self, f: &mut Frame, bot: Rgb) {
        let Some(mut wh) = self.whale_state(self.tt) else { return };
        wh.x -= self.off;
        let wh = &wh;
        let fog = self.fog_z(0.75);
        let a = wh.len / 2.0;
        let b = a * 0.28;
        let y0 = wh.y + (self.t * 0.5).sin() * 2.0;
        let back = -wh.dir; // direction pointing to the tail
        let back_col = lerp([62, 88, 124], bot, fog);
        let belly_col = lerp([150, 172, 196], bot, fog);
        let dark = lerp([20, 28, 44], bot, fog);
        let (x_min, x_max) = ((wh.x - a * 1.5) as i32, (wh.x + a * 1.5) as i32);
        for xi in x_min..=x_max {
            let dx = xi as f32 - wh.x;
            let along = dx * back; // >0 toward the tail
            for yi in (y0 - b * 2.2) as i32..=(y0 + b * 1.6) as i32 {
                let dy = yi as f32 - y0;
                let nx = dx / a;
                let ny = dy / b;
                let mut col = None;
                if nx * nx + ny * ny <= 1.0 {
                    col = Some(if ny > 0.25 { belly_col } else { back_col });
                } else if along > a * 0.8 && along < a * 1.35 {
                    // tapering tail stock with fluke at the end
                    let t = (along - a * 0.8) / (a * 0.55);
                    let half = b * (0.55 - 0.35 * t) + if t > 0.7 { (t - 0.7) * b * 5.0 } else { 0.0 };
                    if (dy + b * 0.5 * t).abs() <= half {
                        col = Some(back_col);
                    }
                }
                if let Some(c) = col {
                    f.set(xi, yi, c);
                }
            }
        }
        // pectoral fin and eye
        let fin_x = wh.x - back * a * 0.2;
        f.line(fin_x as i32, (y0 + b * 0.5) as i32, (fin_x + back * a * 0.25) as i32, (y0 + b * 1.4) as i32, back_col);
        f.set((wh.x - back * a * 0.62) as i32, (y0 + b * 0.05) as i32, dark);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: f64 = 1_700_000_123.0;

    fn frame_of(w: usize, h: usize, layout: Layout, zone: f32) -> Frame {
        let world = World::new(w, h, layout, zone, 10, false).at(T);
        let mut f = Frame::new(w, h);
        world.draw(&mut f);
        f
    }

    fn same(a: &Frame, b: &Frame) -> bool {
        a.w == b.w && a.h == b.h && (0..a.h).all(|y| (0..a.w).all(|x| a.at(x, y) == b.at(x, y)))
    }

    #[test]
    fn same_clock_same_picture() {
        for zone in [0.5, 2.5, 4.5] {
            let a = frame_of(160, 90, Layout::single(160), zone);
            let b = frame_of(160, 90, Layout::single(160), zone);
            assert!(same(&a, &b), "zone {zone}: two instances must draw identical frames");
        }
    }

    #[test]
    fn the_picture_moves_with_the_clock() {
        let w1 = World::new(160, 90, Layout::single(160), 1.0, 10, false).at(T);
        let w2 = World::new(160, 90, Layout::single(160), 1.0, 10, false).at(T + 30.0);
        let (mut a, mut b) = (Frame::new(160, 90), Frame::new(160, 90));
        w1.draw(&mut a);
        w2.draw(&mut b);
        assert!(!same(&a, &b));
    }

    /// Two monitors side by side must show the same sea as one monitor twice as wide.
    #[test]
    fn two_monitors_form_one_panorama() {
        let (w, h) = (160, 90);
        for zone in [0.5, 2.5] {
            let wide = frame_of(2 * w, h, Layout::single(2 * w), zone);
            let left = frame_of(w, h, Layout { pano_w: 2 * w, off: 0 }, zone);
            let right = frame_of(w, h, Layout { pano_w: 2 * w, off: w }, zone);
            let total = (2 * w * h) as f64;
            let mut diff = 0usize;
            for y in 0..h {
                for x in 0..w {
                    diff += (left.at(x, y) != wide.at(x, y)) as usize;
                    diff += (right.at(x, y) != wide.at(x + w, y)) as usize;
                }
            }
            // Sprites cut at the seam may differ by a few pixels of edge clamping; the scene must not.
            assert!((diff as f64) / total < 0.01, "zone {zone}: {diff} of {total} pixels differ across the seam");
        }
    }

    #[test]
    fn population_follows_the_target_zone_not_the_eased_one() {
        let mut w = World::new(160, 90, Layout::single(160), 0.2, 0, false).at(T);
        assert!(w.present.contains(&"clown") && !w.present.contains(&"angler"));
        w.update(0.03, 4.5, 0, false);
        assert!(w.present.contains(&"angler"), "a new target zone repopulates at once");
    }

    #[test]
    fn creatures_are_announced_once() {
        let mut w = World::new(160, 90, Layout::single(160), 1.5, 5, false).at(T);
        let first: Vec<_> = w.update(0.03, 1.5, 5, false);
        assert!(first.contains(&WorldEvent::Spotted("silver")));
        assert!(w.update(0.03, 1.5, 5, false).iter().all(|e| !matches!(e, WorldEvent::Spotted(_))));
    }

    #[test]
    fn every_species_has_a_bestiary_entry_inside_its_zones() {
        for sp in SPECIES {
            let c = crate::state::creature(sp.id).unwrap_or_else(|| panic!("{} missing from the bestiary", sp.id));
            assert_eq!(c.zones, sp.zones, "{}", sp.id);
        }
    }

    #[test]
    fn schools_and_loners_head_in_different_directions() {
        let w = World::new(320, 180, Layout::single(320), 1.2, 25, false).at(T);
        let (mut left, mut right, mut rising, mut sinking) = (0, 0, 0, 0);
        let mut seen = Vec::new();
        for f in &w.fishes {
            if seen.contains(&f.path) {
                continue;
            }
            seen.push(f.path);
            let (vx, vy) = w.paths[f.path].vel(T);
            if vx < 0.0 { left += 1 } else { right += 1 }
            if vy < -0.5 { rising += 1 }
            if vy > 0.5 { sinking += 1 }
        }
        assert!(seen.len() >= 8, "{} groups", seen.len());
        assert!(left >= 2 && right >= 2, "{left} left / {right} right: groups must not share one direction");
        assert!(rising + sinking >= 2, "some groups swim diagonally");
    }

    #[test]
    fn followers_retrace_the_leaders_route_and_stay_in_the_water() {
        let w = World::new(320, 180, Layout::single(320), 0.5, 25, false).at(T);
        let school = w.fishes.iter().filter(|f| SPECIES[f.species].school > 1).map(|f| f.path).next().expect("a school");
        let members: Vec<_> = w.fishes.iter().filter(|f| f.path == school).collect();
        assert!(members.len() >= 3);
        let p = &w.paths[school];
        for m in &members {
            // The follower is where the route was `delay` seconds ago (plus its own offset).
            let (x, y) = p.pos(w.tt - m.delay as f64);
            let pose = w.fish_pose(m);
            assert!((pose.x - w.screen_x(x + m.ox as f64)).abs() < 1e-3, "x follows the route");
            assert!((pose.y - (y as f32 + m.oy)).abs() <= m.amp + 1e-3, "y follows the route within its bob");
            assert!(pose.y > 0.0 && pose.y < 180.0 * 0.85, "y = {}", pose.y);
        }
    }

    /// A world standing in the middle of a shark visit.
    fn with_shark() -> World {
        let first = (1_700_000_000.0 / shark::WINDOW).floor();
        for k in 0..40 {
            let t = (first + k as f64) * shark::WINDOW + 1.0;
            let w = World::new(640, 360, Layout::single(640), 1.0, 25, false).at(t);
            let Some(sh) = &w.shark else { continue };
            let mid = (0..shark::WINDOW as i32).map(|s| t + s as f64).find(|t| sh.envelope(*t) >= 1.0).expect("a visit reaches full strength");
            return World::new(640, 360, Layout::single(640), 1.0, 25, false).at(mid + 3.0);
        }
        panic!("no shark in 40 windows");
    }

    #[test]
    fn a_shark_scatters_nearby_fish_and_is_announced_once() {
        let mut w = with_shark();
        let with: Vec<_> = w.fishes.iter().map(|f| w.fish_xy(f, w.tt)).collect();
        let shark = w.shark.take().unwrap();
        let without: Vec<_> = w.fishes.iter().map(|f| w.fish_xy(f, w.tt)).collect();
        let (hx, hy) = shark.head(w.tt);
        let (mut moved, mut still) = (0, 0);
        for ((a, b), _f) in with.iter().zip(&without).zip(&w.fishes) {
            let shift = ((a.0 - b.0) as f32).hypot(a.1 - b.1);
            let px = (b.0.rem_euclid(w.track_len()) - w.margin()) as f32;
            let d = (px - hx).hypot(b.1 - hy);
            if d < shark_reach(&shark) * 0.6 {
                assert!(shift > 1.0, "a fish {d:.0}px from the head should flee, moved {shift:.2}");
                moved += 1;
            } else if d > shark_reach(&shark) {
                assert!(shift < 1e-3, "a fish {d:.0}px away must not move");
                still += 1;
            }
        }
        assert!(moved + still > 0);
        w.shark = Some(shark);
        let mut seen = 0;
        for _ in 0..3 {
            seen += w.update(0.03, 1.0, 25, false).iter().filter(|e| **e == WorldEvent::Spotted("shark")).count();
        }
        assert_eq!(seen, 1);
    }

    #[test]
    fn no_shark_in_the_deep() {
        let w = World::new(640, 360, Layout::single(640), 4.5, 5, false).at(1_700_000_000.0);
        assert!(w.shark.is_none());
    }

    #[test]
    fn the_fast_floor_row_matches_floor_y() {
        for (zone, t) in [(0.5, 1_700_000_000.0), (3.5, 1_712_345_678.9)] {
            let w = World::new(320, 180, Layout { pano_w: 640, off: 320 }, zone, 5, false).at(t);
            for slot in 0..5 {
                let row = w.floor_row(slot);
                for x in (0..320).step_by(7) {
                    assert!((row[x] - w.floor_y(slot, x as f32)).abs() < 0.02, "slot {slot} x {x}: {} vs {}", row[x], w.floor_y(slot, x as f32));
                }
            }
        }
    }

    #[test]
    fn every_landmark_is_a_bestiary_entry_and_only_appears_in_its_zones() {
        for (id, weights, size) in landmarks::KINDS {
            let c = crate::state::creature(id).unwrap_or_else(|| panic!("{id} missing from the bestiary"));
            assert!(size > 0.0);
            for (zone, w) in weights.iter().enumerate() {
                assert_eq!(*w > 0, (c.zones.0..=c.zones.1).contains(&zone), "{id} zone {zone}: weight {w} vs bestiary {:?}", c.zones);
            }
        }
    }

    #[test]
    fn every_landmark_draws_inside_the_frame_at_any_time() {
        for (id, _, _) in landmarks::KINDS {
            for (zone, t) in [(2.0, 1_700_000_500.0), (4.5, 1_711_111_111.0), (5.5, 1_722_222_222.0)] {
                for off in [0usize, 200] {
                    let mut w = World::new(200, 120, Layout { pano_w: 400, off }, zone, 4, false).at(t);
                    w.force_landmark(id);
                    let mut f = Frame::new(200, 120);
                    w.draw(&mut f); // must not panic, whatever sticks out of the frame
                    assert_eq!(f.w, 200);
                }
            }
        }
    }

    #[test]
    fn a_landmark_in_view_is_announced_once() {
        let mut w = World::new(640, 360, Layout::single(640), 2.0, 5, false).at(T);
        w.force_landmark("ruins");
        assert!(w.landmarks_in_view().contains(&"ruins"));
        let seen = |w: &mut World| w.update(0.03, 2.0, 5, false).iter().filter(|e| **e == WorldEvent::Spotted("ruins")).count();
        assert_eq!(seen(&mut w), 1);
        assert_eq!(seen(&mut w), 0);
    }

    #[test]
    fn seabed_life_matches_the_depth() {
        use scenery::hf;
        let kinds = |zone: usize| -> Vec<usize> { (0..400).map(|i| scenery::pick_for_test(zone, true, hf(1, i))).collect() };
        assert!(kinds(0).iter().all(|k| *k != 8 && *k != 9), "no tube worms or sea pens in the sunlight");
        assert!(kinds(5).iter().all(|k| !matches!(*k, 1 | 2 | 3 | 4 | 5 | 7)), "only rocks, urchins, worms and pens in the hadal trench");
        assert!(kinds(0).contains(&2) && kinds(3).contains(&9));
        let v: Vec<f32> = (0..1000).map(|i| hf(3, i)).collect();
        assert!(v.iter().all(|x| (0.0..1.0).contains(x)) && v.iter().sum::<f32>() / 1000.0 > 0.4);
    }

    #[test]
    fn the_first_level_is_not_a_bare_seabed() {
        // No reef yet (level 0): there must still be plants, rocks and critters in the lowest part of the frame.
        let w = World::new(320, 180, Layout::single(320), 0.3, 0, false).at(T);
        let mut f = Frame::new(320, 180);
        w.draw(&mut f);
        let distinct: std::collections::HashSet<Rgb> = (100..180).flat_map(|y| (0..320).map(move |x| (x, y))).map(|(x, y)| f.at(x, y)).collect();
        assert!(distinct.len() > 60, "{} distinct colours in the bottom strip", distinct.len());
    }

    #[test]
    fn seasons_change_the_scene() {
        let plain = frame_of(160, 90, Layout::single(160), 1.0);
        let mut w = World::new(160, 90, Layout::single(160), 1.0, 10, false).at(T);
        w.set_season(Some("halloween"));
        let mut f = Frame::new(160, 90);
        w.draw(&mut f);
        assert!(!same(&plain, &f), "halloween adds jellies and orange plankton even in the shallows");
        assert_eq!(w.jelly_count(), 5);
    }
}

#[cfg(test)]
mod bench {
    use super::*;
    use crate::gfx::{Overlay, Presenter};

    /// `cargo test --release bench_frame -- --ignored --nocapture`: where does a frame's time go?
    #[test]
    #[ignore]
    fn bench_frame() {
        let (cols, rows) = (320usize, 85usize);
        let (w, h) = (cols * 2, rows * 4);
        let mut world = World::new(w, h, Layout::single(w), 2.5, 20, false).at(1_700_000_000.0);
        let mut f = Frame::new(w, h);
        let mut p = Presenter::new(cols, rows);
        let o = Overlay::new(cols, rows);
        let (mut td, mut tp) = (0.0, 0.0);
        let n = 60;
        for i in 0..n {
            world.fixed = Some(1_700_000_000.0 + i as f64 / 30.0);
            world.update(0.033, 2.5, 20, false);
            let t = std::time::Instant::now();
            world.draw(&mut f);
            td += t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            p.present(&f, &o, &mut std::io::sink()).unwrap();
            tp += t.elapsed().as_secs_f64();
        }
        let bot = world.water().1;
        let top = world.water().0;
        macro_rules! t { ($name:expr, $e:expr) => {{ let t = std::time::Instant::now(); for _ in 0..n { $e; } println!("  {:<10} {:.2} ms", $name, t.elapsed().as_secs_f64() / n as f64 * 1e3); }}; }
        t!("water", world.draw_water(&mut f, top, bot));
        t!("snow", world.draw_snow(&mut f));
        t!("bubbles", { world.draw_bubbles(&mut f, true); world.draw_bubbles(&mut f, false) });
        let rows: Vec<Vec<f32>> = (0..5).map(|s| world.floor_row(s)).collect();
        t!("floor_row", for s in 0..5 { world.floor_row(s); });
        t!("seabed", for (s, r) in rows.iter().enumerate() { world.draw_seabed(&mut f, s, bot, r) });
        t!("ground", for (s, r) in rows.iter().enumerate() { world.draw_ground_texture(&mut f, s, bot, r) });
        t!("props", for s in 0..5 { world.draw_props(&mut f, s, bot) });
        t!("surface", world.draw_surface(&mut f));
        t!("farfish", world.draw_far_schools(&mut f, bot));
        t!("corals", for s in 0..2 { world.draw_corals(&mut f, bot, s) });
        t!("shadows", for s in 0..5 { world.draw_shadows(&mut f, bot, s) });
        t!("jellies", for s in 0..5 { world.draw_jellies(&mut f, bot, s) });
        t!("fish", for s in 0..5 { world.draw_fish(&mut f, bot, s) });
        t!("bloom", world.draw_bloom(&mut f));
        t!("foreground", world.draw_foreground(&mut f, bot));
        println!("{w}x{h}: draw {:.2} ms, present {:.2} ms per frame", td / n as f64 * 1e3, tp / n as f64 * 1e3);
    }
}
