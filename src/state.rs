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
    /// Save format version; see `Save::migrate`.
    pub schema: u32,
    /// Creature ids seen so far.
    pub bestiary: Vec<String>,
    /// Today's quests, what has been done, and the counters they read (reset each day).
    pub quest_day: i64,
    pub quests: Vec<String>,
    pub quest_done: Vec<String>,
    pub qd_whales: u32,
    pub qd_discoveries: u32,
    pub qd_levels: u32,
    pub quests_total: u32,
    pub season_secs: Vec<SeasonRec>,
    /// Not persisted: set by the app when seasonal events are turned off in the config.
    #[serde(skip)]
    pub no_seasons: bool,
}

pub const SCHEMA: u32 = 2;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SeasonRec {
    pub id: String,
    pub secs: f64,
}

pub struct Creature {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub zones: (usize, usize),
}

pub const BESTIARY: &[Creature] = &[
    Creature { id: "clown", name: "Peixe-palhaço", desc: "Listrado e curioso, vive nos recifes rasos", zones: (0, 2) },
    Creature { id: "tang", name: "Cirurgião-azul", desc: "Cardume azul com cauda amarela", zones: (0, 1) },
    Creature { id: "silver", name: "Sardinha-prata", desc: "Corre em bandos pela luz do meio-dia", zones: (1, 3) },
    Creature { id: "lantern", name: "Peixe-lanterna", desc: "Carrega luzes no ventre para se achar no escuro", zones: (2, 4) },
    Creature { id: "viper", name: "Peixe-víbora", desc: "Dentes longos e fotóforos verdes", zones: (3, 5) },
    Creature { id: "angler", name: "Tamboril-abissal", desc: "Pesca com uma isca luminosa", zones: (4, 5) },
    Creature { id: "jelly", name: "Água-viva", desc: "Pulsa em silêncio no crepúsculo", zones: (2, 5) },
    Creature { id: "shark", name: "Tubarão", desc: "Patrulha o mar aberto; os cardumes se abrem à sua passagem", zones: (0, 3) },
    Creature { id: "whale", name: "Baleia", desc: "Cruza o mar a cada poucos minutos", zones: (0, 3) },
    Creature { id: "wreck", name: "Veleiro naufragado", desc: "Madeira, algas e um mastro partido", zones: (0, 3) },
    Creature { id: "anchor", name: "Âncora e corrente", desc: "Ferro enferrujado na areia", zones: (0, 3) },
    Creature { id: "treasure", name: "Baú do tesouro", desc: "Ainda brilha, depois de tanto tempo", zones: (0, 2) },
    Creature { id: "amphorae", name: "Ânforas antigas", desc: "Cerâmica de uma carga perdida", zones: (1, 2) },
    Creature { id: "ruins", name: "Ruínas submersas", desc: "Colunas de um templo engolido pelo mar", zones: (1, 3) },
    Creature { id: "sub", name: "Submarino afundado", desc: "Ainda solta bolhas pela escotilha", zones: (2, 3) },
    Creature { id: "plane", name: "Avião naufragado", desc: "Uma asa só e a hélice torta", zones: (2, 4) },
    Creature { id: "whalefall", name: "Esqueleto de baleia", desc: "Um banquete lento para o fundo do mar", zones: (3, 5) },
    Creature { id: "titanic", name: "Transatlântico partido", desc: "Proa e popa separadas por um campo de destroços", zones: (4, 5) },
    Creature { id: "vents", name: "Fumarolas hidrotermais", desc: "Chaminés quentes cercadas de vermes-tubo", zones: (5, 5) },
    Creature { id: "bloom", name: "Maré bioluminescente", desc: "Plâncton que brilha à noite e no fundo", zones: (0, 5) },
];

pub fn creature(id: &str) -> Option<&'static Creature> {
    BESTIARY.iter().find(|c| c.id == id)
}

pub struct Quest {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

pub const QUESTS: &[Quest] = &[
    Quest { id: "time10", name: "Mergulho curto", desc: "10 minutos no mar hoje" },
    Quest { id: "time30", name: "Mergulho longo", desc: "30 minutos no mar hoje" },
    Quest { id: "whale", name: "Caça-baleias", desc: "Veja uma baleia hoje" },
    Quest { id: "discover", name: "Naturalista", desc: "Descubra uma criatura nova hoje" },
    Quest { id: "level", name: "Rumo ao fundo", desc: "Suba um nível hoje" },
    Quest { id: "calm", name: "Calmaria", desc: "15 minutos numa só sessão" },
];

pub const QUEST_POINTS: u64 = 60;
pub const DISCOVERY_POINTS: u64 = 25;

pub fn quest(id: &str) -> Option<&'static Quest> {
    QUESTS.iter().find(|q| q.id == id)
}

pub struct Season {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    /// (month, day) first and last day, inclusive; the range may wrap over the new year.
    pub from: (i64, i64),
    pub to: (i64, i64),
}

pub const SEASONS: &[Season] = &[
    Season { id: "halloween", name: "Noite das Águas-vivas", desc: "Águas-vivas e plâncton laranja em todas as zonas", from: (10, 24), to: (11, 2) },
    Season { id: "xmas", name: "Luzes do Natal", desc: "O plâncton acende em vermelho, verde e dourado", from: (12, 20), to: (1, 2) },
    Season { id: "oceans", name: "Semana do Oceano", desc: "Baleias aparecem com mais frequência", from: (6, 5), to: (6, 12) },
];

pub const SEASON_XP_BOOST: f64 = 1.5;
const SEASON_TROPHY_SECS: f64 = 300.0;

pub fn season_info(id: &str) -> Option<&'static Season> {
    SEASONS.iter().find(|s| s.id == id)
}

/// Seasonal event active on `day` (days since epoch). `ABYSS_SEASON=<id>` forces one for testing.
pub fn season_at(day: i64) -> Option<&'static Season> {
    if let Ok(id) = std::env::var("ABYSS_SEASON") {
        return season_info(&id);
    }
    let (_, m, d) = civil_from_days(day);
    SEASONS.iter().find(|s| {
        let key = (m, d);
        if s.from <= s.to { key >= s.from && key <= s.to } else { key >= s.from || key <= s.to }
    })
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
    pub discoveries: Vec<String>,
    pub quests_done: Vec<String>,
    pub trophies_total: usize,
    pub bestiary_total: usize,
}

#[derive(Debug, PartialEq)]
pub enum GameEvent {
    LevelUp(u32),
    Trophy(&'static str),
    Discovery(&'static str),
    Quest(&'static str),
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
    Trophy { id: "naturalist", name: "Naturalista", desc: "Descubra todas as criaturas", prog: |s, _| s.bestiary.len() as f64 / BESTIARY.len() as f64 },
    Trophy { id: "quest_10", name: "Missão Cumprida", desc: "Complete 10 missões diárias", prog: |s, _| s.quests_total as f64 / 10.0 },
    Trophy { id: "daily_3", name: "Dia Perfeito", desc: "Complete as 3 missões no mesmo dia", prog: |s, _| s.quest_done.len() as f64 / 3.0 },
    Trophy { id: "season_halloween", name: "Noite das Águas-vivas", desc: "5 minutos no mar durante o evento", prog: |s, _| s.season_time("halloween") / SEASON_TROPHY_SECS },
    Trophy { id: "season_xmas", name: "Luzes do Natal", desc: "5 minutos no mar durante o evento", prog: |s, _| s.season_time("xmas") / SEASON_TROPHY_SECS },
    Trophy { id: "season_oceans", name: "Dia do Oceano", desc: "5 minutos no mar durante o evento", prog: |s, _| s.season_time("oceans") / SEASON_TROPHY_SECS },
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

    pub fn season_time(&self, id: &str) -> f64 {
        self.season_secs.iter().find(|r| r.id == id).map(|r| r.secs).unwrap_or(0.0)
    }

    fn season_boost(&self, today: i64) -> f64 {
        if !self.no_seasons && season_at(today).is_some() { SEASON_XP_BOOST } else { 1.0 }
    }

    pub fn today_secs(&self) -> f64 {
        self.daily.last().filter(|d| d.day == self.last_day).map(|d| d.secs).unwrap_or(0.0)
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
        self.roll_quests(today);
        away
    }

    /// Picks three quests for a new day (stable for a given day and save state).
    pub fn roll_quests(&mut self, today: i64) {
        if self.quest_day == today && !self.quests.is_empty() {
            return;
        }
        self.quest_day = today;
        self.quest_done.clear();
        self.qd_whales = 0;
        self.qd_discoveries = 0;
        self.qd_levels = 0;
        let mut pool: Vec<&Quest> = QUESTS
            .iter()
            .filter(|q| match q.id {
                "whale" => self.zone() < 4,
                "discover" => self.bestiary.len() < BESTIARY.len(),
                _ => true,
            })
            .collect();
        pool.sort_by_key(|q| {
            let h = q.id.bytes().fold(today as u64 ^ 0x9E37_79B9_7F4A_7C15, |a, b| (a ^ b as u64).wrapping_mul(0x0100_0000_01B3));
            h ^ (h >> 29)
        });
        self.quests = pool.iter().take(3).map(|q| q.id.to_string()).collect();
    }

    pub fn quest_progress(&self, id: &str, sess: &Session) -> f64 {
        let p = match id {
            "time10" => self.today_secs() / 600.0,
            "time30" => self.today_secs() / 1800.0,
            "whale" => self.qd_whales as f64,
            "discover" => self.qd_discoveries as f64,
            "level" => self.qd_levels as f64,
            "calm" => sess.secs / 900.0,
            _ => 0.0,
        };
        p.clamp(0.0, 1.0)
    }

    pub fn check_quests(&mut self, sess: &mut Session) -> Vec<GameEvent> {
        let mut events = Vec::new();
        for id in self.quests.clone() {
            if self.quest_done.contains(&id) || self.quest_progress(&id, sess) < 1.0 {
                continue;
            }
            let Some(q) = quest(&id) else { continue };
            sess.quests_done.push(id.clone());
            self.quest_done.push(id);
            self.quests_total += 1;
            self.points += QUEST_POINTS;
            events.push(GameEvent::Quest(q.id));
        }
        events
    }

    /// A creature came into view. The first time it is a discovery.
    pub fn on_spotted(&mut self, id: &str, sess: &mut Session, today: i64) -> Vec<GameEvent> {
        let Some(c) = creature(id) else { return Vec::new() };
        if self.bestiary.iter().any(|b| b == id) {
            return Vec::new();
        }
        self.bestiary.push(id.to_string());
        sess.discoveries.push(id.to_string());
        self.qd_discoveries += 1;
        self.points += DISCOVERY_POINTS;
        let mut events = vec![GameEvent::Discovery(c.id)];
        events.extend(self.check_quests(sess));
        events.extend(self.check_trophies(sess, today));
        events
    }

    pub fn tick(&mut self, dt: f64, sess: &mut Session, night: bool, today: i64) -> Vec<GameEvent> {
        let mut events = Vec::new();
        let level_before = self.level();
        self.roll_quests(today);
        let gain = self.xp_rate() * self.season_boost(today) * dt;
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
        if !self.no_seasons
            && let Some(se) = season_at(today) {
                match self.season_secs.iter_mut().find(|r| r.id == se.id) {
                    Some(r) => r.secs += dt,
                    None => self.season_secs.push(SeasonRec { id: se.id.to_string(), secs: dt }),
                }
            }
        let level = self.level();
        if level > level_before {
            sess.levels_gained += level - level_before;
            self.qd_levels += level - level_before;
            events.push(GameEvent::LevelUp(level));
        }
        events.extend(self.check_quests(sess));
        events.extend(self.check_trophies(sess, today));
        events
    }

    pub fn on_whale(&mut self, sess: &mut Session, today: i64) -> Vec<GameEvent> {
        self.whales += 1;
        sess.whales += 1;
        self.points += 50;
        self.qd_whales += 1;
        let mut events = self.check_quests(sess);
        events.extend(self.check_trophies(sess, today));
        events
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
        sess.trophies_total = self.trophies.len();
        sess.bestiary_total = self.bestiary.len();
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
        let bestiary: Vec<_> = BESTIARY
            .iter()
            .map(|c| serde_json::json!({
                "id": c.id, "name": c.name, "desc": c.desc, "zones": [c.zones.0, c.zones.1],
                "found": self.bestiary.iter().any(|b| b == c.id),
            }))
            .collect();
        let quests: Vec<_> = self
            .quests
            .iter()
            .filter_map(|id| quest(id))
            .map(|q| serde_json::json!({
                "id": q.id, "name": q.name, "desc": q.desc,
                "done": self.quest_done.iter().any(|d| d == q.id),
                "progress": if self.quest_done.iter().any(|d| d == q.id) { 1.0 } else { self.quest_progress(q.id, &none) },
            }))
            .collect();
        let season = if self.no_seasons { None } else { season_at(self.last_day) };
        serde_json::json!({
            "schema": SCHEMA, "bestiary": bestiary, "quests": quests, "questsTotal": self.quests_total,
            "season": season.map(|x| serde_json::json!({ "id": x.id, "name": x.name, "desc": x.desc, "boost": SEASON_XP_BOOST })),
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
        Self::load_from(&state_dir().join("save.json"))
    }

    /// Reads a save. A file that exists but cannot be parsed is moved aside (never silently
    /// overwritten), so a bad edit or a future format cannot wipe the player's progress.
    pub fn load_from(path: &std::path::Path) -> Self {
        let Ok(text) = fs::read_to_string(path) else { return Self::default() };
        match serde_json::from_str::<Save>(&text) {
            Ok(mut s) => {
                s.migrate();
                s
            }
            Err(_) => {
                let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                let _ = fs::rename(path, path.with_extension(format!("json.corrupt-{stamp}")));
                Self::default()
            }
        }
    }

    /// Brings an older save up to the current format.
    pub fn migrate(&mut self) {
        if self.schema < 2 {
            // Version 1 had no bestiary: credit what a player at this depth has plausibly seen.
            if self.sessions > 0 {
                let zone = self.zone();
                for c in BESTIARY.iter().filter(|c| c.zones.0 <= zone && c.id != "bloom") {
                    if c.id == "whale" && self.whales == 0 {
                        continue;
                    }
                    self.bestiary.push(c.id.to_string());
                }
            }
        }
        self.schema = SCHEMA;
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

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + (m <= 2) as i64, m, d)
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
#[allow(clippy::field_reassign_with_default)]
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
    fn civil_round_trip() {
        for day in [-1, 0, 11017, 20_000, 20_746, 25_000] {
            let (y, m, d) = civil_from_days(day);
            assert_eq!(days_from_civil(y, m, d), day);
        }
        assert_eq!(civil_from_days(20_746), (2026, 10, 20));
    }

    #[test]
    fn seasons_follow_the_calendar_and_wrap_the_new_year() {
        let day = |y, m, d| days_from_civil(y, m, d);
        assert_eq!(season_at(day(2026, 10, 31)).map(|s| s.id), Some("halloween"));
        assert_eq!(season_at(day(2026, 12, 25)).map(|s| s.id), Some("xmas"));
        assert_eq!(season_at(day(2027, 1, 1)).map(|s| s.id), Some("xmas"));
        assert_eq!(season_at(day(2027, 1, 3)).map(|s| s.id), None);
        assert_eq!(season_at(day(2026, 6, 8)).map(|s| s.id), Some("oceans"));
        assert_eq!(season_at(day(2026, 3, 15)).map(|s| s.id), None);
    }

    #[test]
    fn season_boosts_xp_and_unlocks_its_trophy() {
        let day = days_from_civil(2026, 10, 31);
        let (mut a, mut b) = (Save::default(), Save { no_seasons: true, ..Default::default() });
        let (mut sa, mut sb) = (Session::default(), Session::default());
        let mut events = Vec::new();
        for _ in 0..301 {
            events.extend(a.tick(1.0, &mut sa, false, day));
            b.tick(1.0, &mut sb, false, day);
        }
        assert!((a.xp / b.xp - SEASON_XP_BOOST).abs() < 1e-6);
        assert!(events.contains(&GameEvent::Trophy("season_halloween")));
        assert!(!b.has("season_halloween"), "seasons can be turned off");
    }

    #[test]
    fn first_sighting_is_a_discovery_once() {
        let mut s = Save::default();
        let mut sess = Session::default();
        let ev = s.on_spotted("clown", &mut sess, 1);
        assert!(ev.contains(&GameEvent::Discovery("clown")));
        assert_eq!(s.points, DISCOVERY_POINTS + 100 * s.trophies.len() as u64 + QUEST_POINTS * s.quests_total as u64);
        assert!(s.on_spotted("clown", &mut sess, 1).is_empty());
        assert!(s.on_spotted("no-such-thing", &mut sess, 1).is_empty());
        assert_eq!(sess.discoveries, vec!["clown"]);
    }

    #[test]
    fn all_creatures_unlock_the_naturalist_trophy() {
        let mut s = Save::default();
        let mut sess = Session::default();
        let mut got = false;
        for c in BESTIARY {
            got |= s.on_spotted(c.id, &mut sess, 1).contains(&GameEvent::Trophy("naturalist"));
        }
        assert!(got);
    }

    #[test]
    fn quests_are_three_stable_and_completable() {
        let mut s = Save::default();
        s.begin_session(500);
        assert_eq!(s.quests.len(), 3);
        let mut other = Save::default();
        other.begin_session(500);
        assert_eq!(s.quests, other.quests, "same day, same quests");
        assert!(s.quests.iter().all(|q| quest(q).is_some()));
        let mut ids = s.quests.clone();
        ids.dedup();
        assert_eq!(ids.len(), 3);

        s.quests = vec!["time10".into(), "whale".into(), "level".into()];
        let mut sess = Session::default();
        let mut events = Vec::new();
        for _ in 0..600 {
            events.extend(s.tick(1.0, &mut sess, false, 500));
        }
        assert!(events.contains(&GameEvent::Quest("time10")));
        events = s.on_whale(&mut sess, 500);
        assert!(events.contains(&GameEvent::Quest("whale")));
        assert!(s.quest_done.contains(&"whale".to_string()));
        assert_eq!(s.quests_total as usize, s.quest_done.len());
        assert!(s.points >= 2 * QUEST_POINTS);
    }

    #[test]
    fn quests_reset_on_a_new_day_and_skip_impossible_ones() {
        let mut s = Save { xp: xp_at(13), ..Default::default() }; // zone 4: no whales
        for day in 0..40 {
            s.roll_quests(1000 + day);
            assert!(!s.quests.contains(&"whale".to_string()), "no whale quest below zone 4");
        }
        s.quest_done.push("x".into());
        s.qd_whales = 3;
        s.roll_quests(2000);
        assert!(s.quest_done.is_empty() && s.qd_whales == 0);
        s.bestiary = BESTIARY.iter().map(|c| c.id.to_string()).collect();
        for day in 0..40 {
            s.roll_quests(3000 + day);
            assert!(!s.quests.contains(&"discover".to_string()), "nothing left to discover");
        }
    }

    #[test]
    fn old_saves_are_migrated_and_keep_their_progress() {
        let old = r#"{"xp": 1000.0, "points": 321, "sessions": 4, "whales": 2, "reef": 7,
                      "trophies": [{"id":"first_dive","day":5}], "daily": [{"day":5,"secs":90.0}]}"#;
        let dir = std::env::temp_dir().join(format!("abyss-mig-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.json");
        std::fs::write(&path, old).unwrap();
        let s = Save::load_from(&path);
        assert_eq!(s.schema, SCHEMA);
        assert_eq!((s.points, s.reef, s.whales), (321, 7, 2));
        assert!(s.has("first_dive"));
        let zone = s.zone();
        for c in BESTIARY.iter().filter(|c| c.zones.0 <= zone && c.id != "bloom") {
            assert!(s.bestiary.iter().any(|b| b == c.id), "{} credited", c.id);
        }
        assert!(!s.bestiary.iter().any(|b| b == "angler"), "creatures below the player's zone stay unseen");
    }

    #[test]
    fn an_unreadable_save_is_set_aside_not_overwritten() {
        let dir = std::env::temp_dir().join(format!("abyss-corrupt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save.json");
        std::fs::write(&path, "{ not json").unwrap();
        let s = Save::load_from(&path);
        assert_eq!(s.xp, 0.0);
        assert!(!path.exists());
        let kept = std::fs::read_dir(&dir).unwrap().flatten().any(|e| e.file_name().to_string_lossy().contains("corrupt-"));
        assert!(kept, "the bad file is kept next to the new save");
    }

    #[test]
    fn stats_json_carries_the_new_sections() {
        let mut s = Save::default();
        s.begin_session(days_from_civil(2026, 12, 24));
        let j = s.stats_json(None);
        assert_eq!(j["bestiary"].as_array().unwrap().len(), BESTIARY.len());
        assert_eq!(j["quests"].as_array().unwrap().len(), 3);
        assert_eq!(j["season"]["id"], "xmas");
        assert_eq!(j["schema"], SCHEMA);
    }

    #[test]
    fn civil_days() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11017);
    }
}
