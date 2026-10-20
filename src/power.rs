//! What the screensaver should cost: pause drawing while the monitor is asleep and drop the
//! frame rate on battery.

use std::{
    fs,
    process::{Command, Stdio},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Power {
    pub display_on: bool,
    pub on_battery: bool,
}

impl Default for Power {
    fn default() -> Self {
        Self { display_on: true, on_battery: false }
    }
}

/// True when any battery reports "Discharging".
pub fn on_battery() -> bool {
    let Ok(dir) = fs::read_dir("/sys/class/power_supply") else { return false };
    dir.flatten().any(|e| {
        let p = e.path();
        fs::read_to_string(p.join("type")).is_ok_and(|t| t.trim() == "Battery")
            && fs::read_to_string(p.join("status")).is_ok_and(|s| s.trim() == "Discharging")
    })
}

/// Reads `dpmsStatus` of `monitor` (or of every monitor when unknown) from `hyprctl monitors -j`.
/// Anything unreadable counts as "on", so a failure never blanks the screen.
pub fn display_on(monitor: &Option<String>) -> bool {
    if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_none() {
        return true;
    }
    let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).stderr(Stdio::null()).output() else { return true };
    let Ok(list) = serde_json::from_slice::<Vec<serde_json::Value>>(&out.stdout) else { return true };
    parse_display_on(&list, monitor)
}

pub fn parse_display_on(list: &[serde_json::Value], monitor: &Option<String>) -> bool {
    let mut it = list.iter().filter(|m| monitor.as_deref().is_none_or(|n| m["name"].as_str() == Some(n)));
    let mut any = false;
    for m in &mut it {
        any = true;
        if m["dpmsStatus"].as_bool().unwrap_or(true) {
            return true;
        }
    }
    !any
}

impl Power {
    pub fn read(monitor: &Option<String>) -> Self {
        Self { display_on: display_on(monitor), on_battery: on_battery() }
    }

    pub fn fps(&self, ac: u32, battery: u32) -> u32 {
        if self.on_battery { battery } else { ac }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn dpms_off_only_when_every_matching_monitor_is_off() {
        let list = vec![json!({"name":"A","dpmsStatus":false}), json!({"name":"B","dpmsStatus":true})];
        assert!(!parse_display_on(&list, &Some("A".into())));
        assert!(parse_display_on(&list, &Some("B".into())));
        assert!(parse_display_on(&list, &None), "any monitor on keeps drawing");
        assert!(parse_display_on(&list, &Some("missing".into())), "unknown monitor never blanks");
        assert!(parse_display_on(&[json!({"name":"A"})], &None), "missing field counts as on");
    }

    #[test]
    fn battery_lowers_fps() {
        let p = Power { display_on: true, on_battery: true };
        assert_eq!(p.fps(30, 15), 15);
        assert_eq!(Power::default().fps(30, 15), 30);
    }
}
