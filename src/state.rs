use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

pub const ZONE_NAMES: [&str; 6] =
    ["Superfície", "Zona de Luz", "Crepúsculo", "Meia-noite", "Abissal", "Hadal"];
const ZONE_START_M: [f64; 6] = [0.0, 50.0, 200.0, 1000.0, 4000.0, 6000.0];
const ZONE_END_M: [f64; 6] = [50.0, 200.0, 1000.0, 4000.0, 6000.0, 11000.0];
const ZONE_MULT: [f64; 6] = [1.0, 1.2, 1.5, 1.8, 2.2, 2.6];
const LEVELS_PER_ZONE: f64 = 3.0;
const XP_PER_SEC: f64 = 0.2;
const REEF_SECS: f64 = 900.0;
pub const REEF_MAX: u32 = 30;
pub const WILT_GRACE_DAYS: i64 = 3;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrophyRec {
    pub id: String,
    pub day: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Save {
    pub xp: f64,
    pub points: u64,
    pub total_secs: f64,
    pub sessions: u32,
    pub best_session_secs: f64,
    pub streak: u32,
    pub last_day: i64,
    pub reef: u32,
    pub reef_secs: f64,
    pub whales: u32,
    pub night_secs: f64,
    pub trophies: Vec<TrophyRec>,
    pub daily: Vec<DayRec>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DayRec {
    pub day: i64,
    pub secs: f64,
}

/// What happened since this screensaver started. Shown in the farewell summary.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Session {
    pub secs: f64,
    pub xp_gain: f64,
    pub levels_gained: u32,
    pub trophies: Vec<String>,
    pub whales: u32,
    pub reef_gain: u32,
    pub night_secs: f64,
    pub level: u32,
    pub zone: usize,
    pub depth_m: u32,
    pub points: u64,
    pub streak: u32,
    pub days_away: i64,
}

#[derive(Debug, PartialEq)]
pub enum GameEvent {
    LevelUp(u32),
    Trophy(&'static str),
}

pub struct Trophy {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    prog: fn(&Save, &Session) -> f64,
}

pub const TROPHIES: &[Trophy] = &[
    Trophy { id: "first_dive", name: "Primeiro Mergulho", desc: "Deixe o oceano vigiar por você", prog: |s, _| s.sessions as f64 },
    Trophy { id: "calm_5", name: "Respiro Longo", desc: "5 minutos de calmaria numa só sessão", prog: |s, x| s.best_session_secs.max(x.secs) / 300.0 },
    Trophy { id: "calm_60", name: "Maré Calma", desc: "1 hora de calmaria numa só sessão", prog: |s, x| s.best_session_secs.max(x.secs) / 3600.0 },
    Trophy { id: "streak_3", name: "Maré Constante", desc: "3 dias seguidos no mar", prog: |s, _| s.streak as f64 / 3.0 },
    Trophy { id: "streak_7", name: "Semana no Mar", desc: "7 dias seguidos no mar", prog: |s, _| s.streak as f64 / 7.0 },
    Trophy { id: "zone_1", name: "Zona de Luz", desc: "Alcance a zona de luz", prog: |s, _| (s.zone() >= 1) as u8 as f64 },
    Trophy { id: "zone_2", name: "Entre Luz e Sombra", desc: "Alcance o crepúsculo", prog: |s, _| (s.zone() >= 2) as u8 as f64 },
    Trophy { id: "zone_3", name: "Meia-noite", desc: "Alcance a zona da meia-noite", prog: |s, _| (s.zone() >= 3) as u8 as f64 },
    Trophy { id: "zone_4", name: "Abismo", desc: "Alcance a zona abissal", prog: |s, _| (s.zone() >= 4) as u8 as f64 },
    Trophy { id: "zone_5", name: "Fim do Mundo", desc: "Alcance a fossa hadal", prog: |s, _| (s.zone() >= 5) as u8 as f64 },
    Trophy { id: "whale_1", name: "Canto da Baleia", desc: "Veja uma baleia passar", prog: |s, _| s.whales as f64 },
    Trophy { id: "whale_5", name: "Observador de Baleias", desc: "Veja 5 baleias passarem", prog: |s, _| s.whales as f64 / 5.0 },
    Trophy { id: "reef_10", name: "Jardineiro de Corais", desc: "Cultive 10 corais", prog: |s, _| s.reef as f64 / 10.0 },
    Trophy { id: "reef_30", name: "Recife Vivo", desc: "Cultive o recife completo", prog: |s, _| s.reef as f64 / REEF_MAX as f64 },
    Trophy { id: "night_owl", name: "Coruja do Mar", desc: "10 minutos de calmaria de madrugada", prog: |s, _| s.night_secs / 600.0 },
    Trophy { id: "hundred_h", name: "Cem Horas", desc: "100 horas de oceano acumuladas", prog: |s, _| s.total_secs / 360_000.0 },
];

impl Trophy {
    pub fn progress(&self, s: &Save, x: &Session) -> f64 {
        (self.prog)(s, x).clamp(0.0, 1.0)
    }
}

pub fn xp_at(level: u32) -> f64 {
    50.0 * (level as f64).powi(2)
}

impl Save {
    pub fn level(&self) -> u32 {
        (self.xp / 50.0).sqrt().floor() as u32
    }

    /// Progress to the next level, 0..1.
    pub fn progress(&self) -> f64 {
        let l = self.level();
        ((self.xp - xp_at(l)) / (xp_at(l + 1) - xp_at(l))).clamp(0.0, 1.0)
    }

    pub fn zone(&self) -> usize {
        ((self.level() as f64 / LEVELS_PER_ZONE) as usize).min(5)
    }

    /// Continuous depth in zone units (0.0 .. 5.99), used for smooth colour blending.
    pub fn zone_f(&self) -> f32 {
        ((self.level() as f64 + self.progress()) / LEVELS_PER_ZONE).min(5.99) as f32
    }

    pub fn depth_m(&self) -> u32 {
        let z = self.zone();
        let within = if z == 5 {
            ((self.level() as f64 + self.progress() - 15.0) / 6.0).min(1.0)
        } else {
            ((self.level() as f64 + self.progress()) / LEVELS_PER_ZONE - z as f64).clamp(0.0, 1.0)
        };
        (ZONE_START_M[z] + (ZONE_END_M[z] - ZONE_START_M[z]) * within) as u32
    }

    pub fn xp_rate(&self) -> f64 {
        let mult: f64 = std::env::var("ABYSS_XP_MULT").ok().and_then(|v| v.parse().ok()).unwrap_or(1.0);
        XP_PER_SEC * ZONE_MULT[self.zone()] * (1.0 + 0.1 * self.streak.min(10) as f64) * mult
    }

    pub fn has(&self, id: &str) -> bool {
        self.trophies.iter().any(|t| t.id == id)
    }

    /// Registers the start of a session: streak, wilting after long absence.
    pub fn begin_session(&mut self, today: i64) -> i64 {
        let away = if self.last_day == 0 { 0 } else { today - self.last_day };
        match away {
            0 if self.streak > 0 => {}
            1 => self.streak += 1,
            _ => self.streak = 1,
        }
        if away > WILT_GRACE_DAYS {
            self.reef = self.reef.saturating_sub((away - WILT_GRACE_DAYS) as u32);
            self.reef_secs = 0.0;
        }
        self.last_day = today;
        self.sessions += 1;
        away
    }

    pub fn tick(&mut self, dt: f64, sess: &mut Session, night: bool, today: i64) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let level_before = self.level();
        let gain = self.xp_rate() * dt;
        self.xp += gain;
        self.points += gain.round() as u64;
        sess.xp_gain += gain;
        self.total_secs += dt;
        sess.secs += dt;
        self.best_session_secs = self.best_session_secs.max(sess.secs);
        if night {
            self.night_secs += dt;
            sess.night_secs += dt;
        }
        match self.daily.last_mut() {
            Some(d) if d.day == today => d.secs += dt,
            _ => {
                self.daily.push(DayRec { day: today, secs: dt });
                let excess = self.daily.len().saturating_sub(30);
                self.daily.drain(..excess);
            }
        }
        self.reef_secs += dt;
        if self.reef_secs >= REEF_SECS {
            self.reef_secs -= REEF_SECS;
            if self.reef < REEF_MAX {
                self.reef += 1;
                sess.reef_gain += 1;
            }
        }
        let level = self.level();
        if level > level_before {
            sess.levels_gained += level - level_before;
            events.push(GameEvent::LevelUp(level));
        }
        events.extend(self.check_trophies(sess, today));
        events
    }

    pub fn on_whale(&mut self, sess: &mut Session, today: i64) -> Vec<GameEvent> {
        self.whales += 1;
        sess.whales += 1;
        self.points += 50;
        self.check_trophies(sess, today)
    }

    pub fn check_trophies(&mut self, sess: &mut Session, today: i64) -> Vec<GameEvent> {
        let mut events = Vec::new();
        for t in TROPHIES {
            if !self.has(t.id) && t.progress(self, sess) >= 1.0 {
                self.trophies.push(TrophyRec { id: t.id.to_string(), day: today });
                self.points += 100;
                sess.trophies.push(t.id.to_string());
                events.push(GameEvent::Trophy(t.id));
            }
        }
        events
    }

    pub fn snapshot(&self, sess: &mut Session) {
        sess.level = self.level();
        sess.zone = self.zone();
        sess.depth_m = self.depth_m();
        sess.points = self.points;
        sess.streak = self.streak;
    }
}

impl Save {
    /// Snapshot consumed by the shell plugin (watched file) and `abyss status --json`.
    pub fn stats_json(&self, last: Option<&Session>) -> serde_json::Value {
        let l = self.level();
        let none = Session::default();
        let trophies: Vec<_> = TROPHIES
            .iter()
            .map(|t| {
                let rec = self.trophies.iter().find(|r| r.id == t.id);
                serde_json::json!({
                    "id": t.id, "name": t.name, "desc": t.desc,
                    "unlocked": rec.is_some(), "day": rec.map(|r| r.day),
                    "progress": if rec.is_some() { 1.0 } else { t.progress(self, &none) },
                })
            })
            .collect();
        serde_json::json!({
            "level": l, "zone": self.zone(), "zoneF": self.zone_f(), "zoneName": ZONE_NAMES[self.zone()],
            "depth": self.depth_m(), "xp": self.xp, "xpLevelStart": xp_at(l), "xpNext": xp_at(l + 1),
            "progress": self.progress(), "points": self.points, "streak": self.streak,
            "totalSecs": self.total_secs, "sessions": self.sessions, "bestSessionSecs": self.best_session_secs,
            "reef": self.reef, "reefMax": REEF_MAX, "whales": self.whales, "nightSecs": self.night_secs,
            "today": self.last_day, "daily": self.daily, "trophies": trophies, "lastSession": last,
        })
    }
}

pub fn trophy(id: &str) -> Option<&'static Trophy> {
    TROPHIES.iter().find(|t| t.id == id)
}

pub fn state_dir() -> PathBuf {
    if let Ok(d) = std::env::var("ABYSS_STATE_DIR") {
        return PathBuf::from(d);
    }
    let base = std::env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".local/state"));
    base.join("abyss")
}

fn write_atomic(name: &str, body: &str) {
    let dir = state_dir();
    let _ = fs::create_dir_all(&dir);
    let tmp = dir.join(format!("{name}.tmp{}", std::process::id()));
    if fs::write(&tmp, body).is_ok() {
        let _ = fs::rename(&tmp, dir.join(name));
    }
}

impl Save {
    pub fn load() -> Self {
        fs::read_to_string(state_dir().join("save.json"))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn store(&self) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            write_atomic("save.json", &s);
        }
        let last = Session::load_last();
        write_atomic("stats.json", &self.stats_json(last.as_ref()).to_string());
    }
}

impl Session {
    pub fn load_last() -> Option<Self> {
        let s = fs::read_to_string(state_dir().join("last_session.json")).ok()?;
        serde_json::from_str(&s).ok()
    }

    pub fn store_last(&self) {
        if let Ok(s) = serde_json::to_string(self) {
            write_atomic("last_session.json", &s);
        }
    }

    pub fn clear_last() {
        let _ = fs::remove_file(state_dir().join("last_session.json"));
    }
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = (if y >= 0 { y } else { y - 399 }) / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Local calendar day (days since epoch) and hour. Falls back to UTC if `date` is missing.
pub fn local_clock() -> (i64, u32) {
    let parsed = Command::new("date").arg("+%Y-%m-%d %H").output().ok().and_then(|o| {
        let s = String::from_utf8(o.stdout).ok()?;
        let (date, hour) = s.trim().split_once(' ')?;
        let mut p = date.split('-').map(|n| n.parse::<i64>());
        let (y, m, d) = (p.next()?.ok()?, p.next()?.ok()?, p.next()?.ok()?);
        Some((days_from_civil(y, m, d), hour.parse::<u32>().ok()?))
    });
    parsed.unwrap_or_else(|| {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        (secs / 86_400, ((secs / 3600) % 24) as u32)
    })
}

pub fn is_night(hour: u32) -> bool {
    !(6..19).contains(&hour)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_curve() {
        let mut s = Save::default();
        assert_eq!(s.level(), 0);
        s.xp = 50.0;
        assert_eq!(s.level(), 1);
        s.xp = 199.0;
        assert_eq!(s.level(), 1);
        s.xp = 200.0;
        assert_eq!(s.level(), 2);
    }

    #[test]
    fn zones_follow_levels() {
        let mut s = Save::default();
        s.xp = xp_at(3);
        assert_eq!(s.zone(), 1);
        s.xp = xp_at(15);
        assert_eq!(s.zone(), 5);
        s.xp = xp_at(40);
        assert_eq!(s.zone(), 5);
        assert!(s.zone_f() < 6.0);
    }

    #[test]
    fn depth_grows_with_xp() {
        let mut s = Save::default();
        let mut last = 0;
        for lvl in 0..25 {
            s.xp = xp_at(lvl) + 1.0;
            let d = s.depth_m();
            assert!(d >= last, "depth must not shrink at level {lvl}");
            last = d;
        }
        assert!(last <= 11000);
    }

    #[test]
    fn streak_and_wilt() {
        let mut s = Save::default();
        s.begin_session(100);
        assert_eq!(s.streak, 1);
        s.begin_session(100);
        assert_eq!(s.streak, 1, "same day keeps streak");
        s.begin_session(101);
        assert_eq!(s.streak, 2);
        s.reef = 10;
        let away = s.begin_session(106);
        assert_eq!(away, 5);
        assert_eq!(s.streak, 1);
        assert_eq!(s.reef, 8, "2 days past grace wilts 2 corals");
    }

    #[test]
    fn trophies_unlock_once() {
        let mut s = Save::default();
        let mut sess = Session::default();
        s.begin_session(10);
        let ev = s.check_trophies(&mut sess, 10);
        assert!(ev.contains(&GameEvent::Trophy("first_dive")));
        assert!(s.check_trophies(&mut sess, 10).is_empty());
        assert_eq!(s.points, 100);
    }

    #[test]
    fn tick_levels_up_and_grows_reef() {
        let mut s = Save::default();
        let mut sess = Session::default();
        let mut ups = 0;
        for _ in 0..1000 {
            ups += s.tick(1.0, &mut sess, false, 1).iter().filter(|e| matches!(e, GameEvent::LevelUp(_))).count();
        }
        assert!(ups >= 1);
        assert_eq!(s.reef, 1, "1000s crosses one 900s reef step");
        assert!(sess.xp_gain > 150.0);
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }
}
