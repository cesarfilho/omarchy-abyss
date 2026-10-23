//! Illustrative, deterministic sea conditions for a journey through real current regions.
use super::{
    mix,
    tide::{temperature_c, Tide, PERIOD_SECS, MAX_KNOTS, CAMERA_UNIT_SPEED},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Region {
    pub id: &'static str,
    pub name: &'static str,
    pub ocean: &'static str,
    pub bearing_deg: f32,
    pub strength_knots: f64,
    pub surface_c: f32,
    pub salinity_psu: f32,
    pub upwelling: bool,
}

pub const REGIONS: [Region; 12] = [
    Region {
        id: "gulf-stream",
        name: "Corrente do Golfo",
        ocean: "Atlântico Norte",
        bearing_deg: 45.0,
        strength_knots: 1.0,
        surface_c: 25.0,
        salinity_psu: 36.0,
        upwelling: false,
    },
    Region {
        id: "labrador",
        name: "Corrente do Labrador",
        ocean: "Atlântico Norte",
        bearing_deg: 180.0,
        strength_knots: 0.6,
        surface_c: 6.0,
        salinity_psu: 33.5,
        upwelling: false,
    },
    Region {
        id: "brazil",
        name: "Corrente do Brasil",
        ocean: "Atlântico Sul",
        bearing_deg: 225.0,
        strength_knots: 0.6,
        surface_c: 27.0,
        salinity_psu: 36.0,
        upwelling: false,
    },
    Region {
        id: "benguela",
        name: "Corrente de Benguela",
        ocean: "Atlântico Sul",
        bearing_deg: 345.0,
        strength_knots: 0.6,
        surface_c: 16.0,
        salinity_psu: 35.1,
        upwelling: true,
    },
    Region {
        id: "kuroshio",
        name: "Corrente de Kuroshio",
        ocean: "Pacífico Norte",
        bearing_deg: 45.0,
        strength_knots: 1.0,
        surface_c: 26.0,
        salinity_psu: 34.8,
        upwelling: false,
    },
    Region {
        id: "oyashio",
        name: "Corrente de Oyashio",
        ocean: "Pacífico Norte",
        bearing_deg: 225.0,
        strength_knots: 0.6,
        surface_c: 7.0,
        salinity_psu: 33.3,
        upwelling: false,
    },
    Region {
        id: "california",
        name: "Corrente da Califórnia",
        ocean: "Pacífico Norte",
        bearing_deg: 180.0,
        strength_knots: 0.3,
        surface_c: 17.0,
        salinity_psu: 33.7,
        upwelling: true,
    },
    Region {
        id: "humboldt",
        name: "Corrente de Humboldt",
        ocean: "Pacífico Sul",
        bearing_deg: 345.0,
        strength_knots: 0.6,
        surface_c: 16.0,
        salinity_psu: 34.7,
        upwelling: true,
    },
    Region {
        id: "agulhas",
        name: "Corrente das Agulhas",
        ocean: "Índico",
        bearing_deg: 225.0,
        strength_knots: 1.0,
        surface_c: 25.0,
        salinity_psu: 35.5,
        upwelling: false,
    },
    Region {
        id: "west-australian",
        name: "Corrente da Austrália Ocidental",
        ocean: "Índico",
        bearing_deg: 345.0,
        strength_knots: 0.3,
        surface_c: 20.0,
        salinity_psu: 35.3,
        upwelling: true,
    },
    Region {
        id: "east-australian",
        name: "Corrente da Austrália Oriental",
        ocean: "Pacífico Sul",
        bearing_deg: 180.0,
        strength_knots: 1.0,
        surface_c: 25.0,
        salinity_psu: 35.2,
        upwelling: false,
    },
    Region {
        id: "antarctic",
        name: "Corrente Circumpolar Antártica",
        ocean: "Oceano Austral",
        bearing_deg: 90.0,
        strength_knots: 1.0,
        surface_c: 2.0,
        salinity_psu: 34.5,
        upwelling: false,
    },
];

pub fn region_by_id(id: &str) -> Option<Region> {
    REGIONS.iter().copied().find(|region| region.id == id)
}

pub fn region_for_session(started_at: f64) -> Region {
    let seed = mix(0x0CEA_2026, started_at.floor() as u64);
    REGIONS[(seed % REGIONS.len() as u64) as usize]
}

pub fn compass_name(degrees: f32) -> &'static str {
    const ROSE: [&str; 8] = ["N", "NE", "L", "SE", "S", "SO", "O", "NO"];
    ROSE[((degrees.rem_euclid(360.0) + 22.5) / 45.0) as usize % 8]
}

fn event_envelope(region: Region, t: f64, period: f64, salt: u64, odds: u64) -> f32 {
    let k = (t / period).floor() as u64;
    let r = mix(
        mix(0x0CEA_2026, region.id.bytes().map(u64::from).sum()),
        k ^ salt,
    );
    if r % 100 >= odds {
        return 0.0;
    }
    let phase = t.rem_euclid(period) / period;
    let center = 0.25 + ((r >> 8) % 50) as f64 / 100.0;
    (1.0 - ((phase - center).abs() / 0.2)).clamp(0.0, 1.0) as f32
}

#[derive(Clone, Copy, Debug)]
pub struct SeaState {
    pub region: Region,
    pub tide: Tide,
    pub camera_x: f64,
    pub flow_knots: f64,
    pub bearing_deg: Option<f32>,
    pub wind_knots: f32,
    pub wave_m: f32,
    pub temperature_c: f32,
    pub salinity_psu: f32,
    pub pressure_bar: f32,
    pub visibility_m: f32,
    pub storm: f32,
    pub upwelling: f32,
}

impl SeaState {
    pub fn at(region: Region, started_at: f64, t: f64, depth_m: u32, unit: f64) -> Self {
        let tide = Tide::at(t, unit);
        let tide_signed =
            1.5 * (t.rem_euclid(PERIOD_SECS) * std::f64::consts::TAU / PERIOD_SECS).sin();
        let flow_knots = region.strength_knots + tide_signed;
        let bearing_deg = if flow_knots.abs() < 0.03 {
            None
        } else if flow_knots > 0.0 {
            Some(region.bearing_deg)
        } else {
            Some((region.bearing_deg + 180.0) % 360.0)
        };
        let camera_x =
            tide.camera_x + (t - started_at).max(0.0) * (CAMERA_UNIT_SPEED * unit / MAX_KNOTS) * region.strength_knots;
        let shallow = (1.0 - depth_m as f32 / 1000.0).clamp(0.0, 1.0);
        let storm = event_envelope(region, t, 7200.0, 900, 12) * shallow;
        let upwelling = if region.upwelling {
            event_envelope(region, t, 5400.0, 901, 40) * shallow
        } else {
            0.0
        };
        let wind_knots =
            (7.0 + 3.0 * (t / 3000.0 + region.bearing_deg as f64).sin() as f32 + 20.0 * storm)
                .max(0.0);
        let wave_m = (0.4 + 0.05 * wind_knots + 2.5 * storm).min(5.0);
        let depth_temp = temperature_c(depth_m);
        let surface_delta = region.surface_c - 24.0;
        let temperature_c = (depth_temp
            + surface_delta * (1.0 - depth_m as f32 / 1000.0).clamp(0.0, 1.0))
        .min(region.surface_c)
            - 3.0 * upwelling;
        let visibility_m =
            (28.0 - depth_m as f32 * 0.004 - 14.0 * storm - 8.0 * upwelling).clamp(2.0, 30.0);
        Self {
            region,
            tide,
            camera_x,
            flow_knots,
            bearing_deg,
            wind_knots,
            wave_m,
            temperature_c,
            salinity_psu: region.salinity_psu + 0.1 * (depth_m as f32 / 1000.0).min(2.0),
            pressure_bar: 1.0 + depth_m as f32 / 10.0,
            visibility_m,
            storm,
            upwelling,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn regions_are_distinct_and_addressable() {
        assert_eq!(REGIONS.len(), 12);
        for region in REGIONS {
            assert_eq!(region_by_id(region.id).unwrap().id, region.id);
            assert!((0.0..360.0).contains(&region.bearing_deg));
            assert!(region.strength_knots > 0.0);
        }
        assert!(region_by_id("unknown").is_none());
        assert_eq!(
            region_for_session(1_700_000_000.0),
            region_for_session(1_700_000_000.0)
        );
    }

    #[test]
    fn current_and_tide_share_a_stable_camera_and_compass() {
        let region = region_by_id("gulf-stream").unwrap();
        let start = 1_700_000_000.0;
        let a = SeaState::at(region, start, start, 50, 2.0);
        let b = SeaState::at(region, start, start + 60.0, 50, 2.0);
        assert!(b.camera_x > a.camera_x);
        assert!((b.camera_x - a.camera_x) > 0.0);
        assert!(b.flow_knots.abs() <= region.strength_knots + 1.5);
        assert!(compass_name(45.0) == "NE" && compass_name(225.0) == "SO");
    }

    #[test]
    fn combined_flow_has_no_jump_at_the_tide_direction_threshold() {
        let region = region_by_id("california").unwrap();
        let crossing = (0.03_f64).asin() * super::super::tide::PERIOD_SECS / std::f64::consts::TAU;
        let before = SeaState::at(region, 0.0, crossing - 0.01, 0, 1.0);
        let after = SeaState::at(region, 0.0, crossing + 0.01, 0, 1.0);
        assert!((before.flow_knots - after.flow_knots).abs() < 0.001);
    }

    #[test]
    fn deep_water_is_colder_darker_and_more_pressurized() {
        let region = region_by_id("benguela").unwrap();
        let surface = SeaState::at(region, 1000.0, 1200.0, 0, 1.0);
        let deep = SeaState::at(region, 1000.0, 1200.0, 5000, 1.0);
        assert!(deep.temperature_c < surface.temperature_c);
        assert!(deep.pressure_bar > surface.pressure_bar);
        assert!(deep.visibility_m < surface.visibility_m);
        assert_eq!(surface.pressure_bar, 1.0);
        let polar = region_by_id("antarctic").unwrap();
        assert!(SeaState::at(polar, 0.0, 1200.0, 2000, 1.0).temperature_c <= polar.surface_c);
    }

    #[test]
    fn all_compass_points_and_flow_reversal_are_available() {
        assert_eq!(
            [0, 45, 90, 135, 180, 225, 270, 315].map(|d| compass_name(d as f32)),
            ["N", "NE", "L", "SE", "S", "SO", "O", "NO"]
        );
        let region = region_by_id("california").unwrap();
        let west_tide = super::super::tide::PERIOD_SECS * 3.0 / 4.0;
        let sea = SeaState::at(region, 0.0, west_tide, 20, 1.0);
        assert!(sea.flow_knots < 0.0);
        assert_eq!(sea.bearing_deg, Some(0.0));
    }

    #[test]
    fn rare_events_are_deterministic_and_only_reach_shallow_water() {
        let region = region_by_id("humboldt").unwrap();
        let found = (0..40_000)
            .step_by(60)
            .find_map(|t| {
                let sea = SeaState::at(region, 0.0, t as f64, 10, 1.0);
                (sea.upwelling > 0.5).then_some(t as f64)
            })
            .unwrap();
        let a = SeaState::at(region, 0.0, found, 10, 1.0);
        let b = SeaState::at(region, 0.0, found, 10, 1.0);
        let deep = SeaState::at(region, 0.0, found, 2000, 1.0);
        assert_eq!(a.upwelling, b.upwelling);
        assert_eq!(deep.upwelling, 0.0);
    }
}
