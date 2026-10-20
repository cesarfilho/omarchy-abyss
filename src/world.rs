use crate::gfx::{Frame, Rgb, lerp};
use crate::sprites::{self, SPECIES};

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
const SEED: u64 = 0xAB155_0CEA;
const WHALE_WINDOW: f64 = 300.0;
const BLOOM_WINDOW: f64 = 240.0;

#[derive(Debug, PartialEq)]
pub enum WorldEvent {
    WhalePassed,
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

fn mix(seed: u64, k: u64) -> u64 {
    let mut z = seed.wrapping_add(k.wrapping_mul(0x9E37_79B9_7F4A_7C15));
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic stream of floats in [0, 1) for world generation.
struct Gen(u64);

impl Gen {
    fn f(&mut self) -> f32 {
        self.0 = mix(self.0, 1);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f()
    }

    fn below(&mut self, n: u32) -> u32 {
        (self.f() * n as f32) as u32 % n.max(1)
    }

    fn chance(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

fn now_secs() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0)
}

/// Position on a looping track of length `len` (panorama plus margins), minus the margin.
fn track(x0: f64, v: f64, tt: f64, len: f64, margin: f64) -> f32 {
    (((x0 + v * tt) % len + len) % len - margin) as f32
}

struct Fish {
    species: usize,
    z: f32,
    x0: f64,
    base_y: f32, // fraction of the height
    vx: f32,
    amp: f32,
    freq: f32,
    phase: f32,
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
    x0: f64,
    base_y: f32,
    vx: f32,
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
    bubbles: Vec<Bubble>,
    snow: Vec<Snow>,
    jellies: Vec<Jelly>,
    glows: Vec<Glow>,
    whale_active: bool,
    gen_zone: usize,
    gen_reef: u32,
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
            bubbles: Vec::new(),
            snow: Vec::new(),
            jellies: Vec::new(),
            glows: Vec::new(),
            whale_active: false,
            gen_zone: usize::MAX,
            gen_reef: u32::MAX,
        };
        world.px_scale = scale_for(world.refw);
        world.t = (world.tt % 100_000.0) as f32;
        world.populate();
        world
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
        self.gen_zone = self.zone();
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
        let len_track = self.track_len();
        while !ok.is_empty() && self.fishes.len() < target {
            let species = ok[g.below(ok.len() as u32) as usize];
            let sp = &SPECIES[species];
            let dir = if g.chance(0.5) { 1.0 } else { -1.0 };
            let school_z = g.range(0.0, 1.0);
            let speed = g.range(sp.speed.0, sp.speed.1) * dir * self.unit() * scale_z(school_z);
            let sprite_w = self.refw * sp.len * scale_z(school_z);
            let x0 = g.range(0.0, len_track as f32) as f64;
            let base_y = g.range(0.12, 0.68);
            let freq = g.range(0.6, 1.4);
            for i in 0..sp.school {
                self.fishes.push(Fish {
                    species,
                    z: (school_z + g.range(-0.04, 0.04)).clamp(0.0, 1.0),
                    x0: x0 - dir as f64 * i as f64 * (sprite_w as f64 * 1.5) + g.range(-3.0, 3.0) as f64,
                    base_y: (base_y + g.range(-0.04, 0.04)).clamp(0.08, 0.7),
                    vx: speed * g.range(0.95, 1.05),
                    amp: g.range(1.0, 3.5) * scale_z(school_z),
                    freq,
                    phase: g.range(0.0, 6.28),
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
                phase: g.range(0.0, 6.28),
                big: g.chance(0.2),
            })
            .collect();
        let n_snow = ((self.pano / 6.0) as usize).min(220);
        self.snow = (0..n_snow)
            .map(|_| Snow { x: g.range(0.0, self.pano), y0: g.range(0.0, self.h as f32), vy: g.range(1.5, 5.0) * self.unit() })
            .collect();
        self.jellies = (0..5)
            .map(|_| Jelly {
                z: g.range(0.0, 0.9),
                x0: g.range(0.0, len_track as f32) as f64,
                base_y: g.range(0.15, 0.6),
                vx: g.range(-4.0, 4.0) * self.unit(),
                phase: g.range(0.0, 6.28),
            })
            .collect();
        let n_glows = (70.0 * self.pano / self.refw) as usize;
        self.glows = (0..n_glows)
            .map(|_| Glow { x: g.range(0.0, self.pano), y: g.range(0.0, 0.9), phase: g.range(0.0, 6.28), hue: g.f() })
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
    }

    /// The whale schedule is a pure function of the clock: every 5-minute window may hold one pass.
    fn whale_state(&self, tt: f64) -> Option<Whale> {
        if self.zf >= 4.0 {
            return None;
        }
        let k = (tt / WHALE_WINDOW).floor();
        let r = mix(SEED, 300 + k as u64);
        if r % 100 >= 40 {
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
        self.tt = now_secs();
        self.t = (self.tt % 100_000.0) as f32;
        self.zf += (zf - self.zf) * 0.02;
        self.reef = reef;
        self.night = night;
        if self.zone() != self.gen_zone || self.reef != self.gen_reef {
            self.populate();
        }
        let active = self.whale_state(self.tt).is_some();
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
        (
            lerp(ZONE_WATER[i].0, ZONE_WATER[j].0, t),
            lerp(ZONE_WATER[i].1, ZONE_WATER[j].1, t),
        )
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
        self.draw_snow(f);
        self.draw_bubbles(f, true);
        // Far to near: each plane of seabed, then what swims in front of it.
        for slot in (0..5).rev() {
            self.draw_seabed(f, slot, bot);
            if slot <= 1 {
                self.draw_corals(f, bot, slot);
            }
            self.draw_shadows(f, bot, slot);
            self.draw_jellies(f, bot, slot);
            if slot == 3 {
                self.draw_whale(f, bot);
            }
            self.draw_fish(f, bot, slot);
            if slot == 2 {
                self.draw_bloom(f);
            }
        }
        self.draw_foreground(f, bot);
        self.draw_bubbles(f, false);
    }

    fn draw_water(&self, f: &mut Frame, top: Rgb, bot: Rgb) {
        let ray_strength = (1.0 - self.zf / 3.0).max(0.0) * 0.8;
        let dim = 1.0 - 0.35 * if self.night { 1.0 } else { 0.0 } * (1.0 - (self.zf / 3.0).min(1.0));
        let h = self.h as f32;
        for y in 0..self.h {
            let base = lerp(top, bot, y as f32 / h);
            let fall = 1.0 - y as f32 / h;
            for x in 0..self.w {
                let mut c = [base[0] as f32 * dim, base[1] as f32 * dim, base[2] as f32 * dim];
                if ray_strength > 0.02 {
                    let k = self.px_scale as f32;
                    let u = ((x as f32 + self.off) + y as f32 * 0.45) / k - self.t * 2.5;
                    let v = (u * 0.045).sin() * 0.5 + (u * 0.11 + 1.3).sin() * 0.5;
                    if v > 0.35 {
                        let add = (v - 0.35) * 70.0 * ray_strength * fall * dim;
                        c[0] += add * 0.6;
                        c[1] += add * 0.9;
                        c[2] += add;
                    }
                }
                let th = (BAYER[y & 3][(x + self.off as usize) & 3] as f32 / 16.0 - 0.5) * DITHER_STEP;
                let q = |v: f32| (((v + th) / DITHER_STEP).round() * DITHER_STEP).clamp(0.0, 255.0) as u8;
                f.set(x as i32, y as i32, [q(c[0]), q(c[1]), q(c[2])]);
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

    fn draw_seabed(&self, f: &mut Frame, slot: usize, bot: Rgb) {
        let color = lerp(bot, self.sand(), PLANES[slot].5);
        let edge = lerp(color, [255, 255, 255], 0.12);
        for x in 0..self.w {
            let fy = self.floor_y(slot, x as f32) as i32;
            for y in fy.max(0)..self.h as i32 {
                f.set(x as i32, y, if y == fy && slot == 0 { edge } else { color });
            }
        }
    }

    fn fish_x(&self, fish: &Fish) -> f32 {
        track(fish.x0, fish.vx as f64, self.tt, self.track_len(), self.margin()) - self.off
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
            let fx = self.fish_x(fish);
            if fx < -len || fx > self.w as f32 {
                continue;
            }
            let cx = fx + len * 0.5;
            let floor = self.floor_y(slot, cx.clamp(0.0, self.w as f32 - 1.0));
            let y = fish.base_y * self.h as f32 + (self.t * fish.freq + fish.phase).sin() * fish.amp;
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
            let fx = self.fish_x(fish);
            if fx < -(len as f32) || fx > self.w as f32 {
                continue;
            }
            let fog = (bot, self.fog_z(fish.z));
            let floor = self.floor_y(slot, (fx + len as f32 * 0.5).clamp(0.0, self.w as f32 - 1.0));
            let y = (fish.base_y * self.h as f32 + (self.t * fish.freq + fish.phase).sin() * fish.amp).min(floor - len as f32 * 0.55);
            // Distant fish lose their dark outline: at that size it only muddies them.
            let outline = len >= 14 && fish.z < 0.65;
            sprites::draw_fish(f, sp.kind, fx as i32, y as i32, len, fish.vx < 0.0, self.t * fish.freq + fish.phase, fog, outline);
        }
    }

    fn draw_jellies(&self, f: &mut Frame, bot: Rgb, slot: usize) {
        let target = if self.zone() >= 2 { 2 + self.zone().saturating_sub(2) } else { 0 };
        for j in self.jellies.iter().take(target).filter(|j| slot_of(j.z) == slot) {
            let fog = (bot, self.fog_z(j.z) * 0.6);
            let len = (self.refw * 0.036 * scale_z(j.z)) as i32;
            let x = track(j.x0, j.vx as f64, self.tt, self.track_len(), self.margin()) - self.off;
            if x < -(len as f32) || x > self.w as f32 {
                continue;
            }
            let y = j.base_y * self.h as f32 + (self.t * 0.7 + j.phase).sin() * 6.0;
            sprites::draw_jelly(f, x as i32, y as i32, len, self.t + j.phase, fog);
        }
    }

    fn draw_bloom(&self, f: &mut Frame) {
        let deep = (self.zf / 3.0).min(1.0);
        let base = if self.night { 0.25 } else { 0.0 } + deep * 0.2;
        let a = (self.bloom_strength() + base).min(1.0);
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
            let col = lerp([70, 255, 200], [90, 160, 255], g.hue);
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
