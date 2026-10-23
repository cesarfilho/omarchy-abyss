//! Simulated, clock-driven tidal current shared by every monitor.

pub const PERIOD_SECS: f64 = 12.0 * 3600.0 + 25.0 * 60.0;
pub(super) const MAX_KNOTS: f64 = 1.5;
pub(super) const CAMERA_UNIT_SPEED: f64 = 2.5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    East,
    West,
    Still,
}

#[derive(Clone, Copy, Debug)]
pub struct Tide {
    /// Observer position in the panorama's unbounded world coordinates.
    pub camera_x: f64,
    pub speed_knots: f64,
    pub direction: Direction,
}

impl Tide {
    pub fn at(epoch_secs: f64, unit: f64) -> Self {
        let omega = std::f64::consts::TAU / PERIOD_SECS;
        let phase = epoch_secs.rem_euclid(PERIOD_SECS) * omega;
        let max_speed = CAMERA_UNIT_SPEED * unit;
        let flow = phase.sin();
        let direction = if flow > 0.03 {
            Direction::East
        } else if flow < -0.03 {
            Direction::West
        } else {
            Direction::Still
        };
        Self {
            camera_x: max_speed / omega * (1.0 - phase.cos()),
            speed_knots: MAX_KNOTS * flow.abs(),
            direction,
        }
    }
}

/// Fictional local water temperature, interpolated across the game's depth range.
pub fn temperature_c(depth_m: u32) -> f32 {
    const POINTS: [(u32, f32); 5] = [(0, 24.0), (200, 17.0), (1000, 8.0), (4000, 4.0), (6000, 2.0)];
    for pair in POINTS.windows(2) {
        let (lo_depth, lo_temp) = pair[0];
        let (hi_depth, hi_temp) = pair[1];
        if depth_m <= hi_depth {
            let t = (depth_m.saturating_sub(lo_depth)) as f32 / (hi_depth - lo_depth) as f32;
            return lo_temp + (hi_temp - lo_temp) * t;
        }
    }
    POINTS.last().unwrap().1
}
