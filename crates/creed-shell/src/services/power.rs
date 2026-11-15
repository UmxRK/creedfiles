//! Battery (sysfs), backlight (sysfs + brightnessctl), power profiles, session actions.

use crate::app::Msg;
use crate::data::Battery;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

fn read(p: PathBuf) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

pub fn battery() -> Option<Battery> {
    let mut total = 0.0;
    let mut n = 0.0;
    let mut charging = false;
    let mut full = true;
    for e in fs::read_dir("/sys/class/power_supply").ok()?.flatten() {
        let p = e.path();
        if read(p.join("type")).as_deref() != Some("Battery") || read(p.join("scope")).as_deref() == Some("Device") {
            continue;
        }
        let cap: f64 = read(p.join("capacity")).and_then(|v| v.parse().ok())?;
        total += cap;
        n += 1.0;
        match read(p.join("status")).as_deref() {
            Some("Charging") => {
                charging = true;
                full = false;
            }
            Some("Full") => {}
            _ => full = false,
        }
    }
    (n > 0.0).then(|| Battery { percent: total / n, charging, full })
}

fn backlight_dir() -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir("/sys/class/backlight").ok()?.flatten().map(|e| e.path()).collect();
    v.sort();
    v.into_iter().next()
}

pub fn brightness() -> Option<f64> {
    let d = backlight_dir()?;
    let cur: f64 = read(d.join("brightness"))?.parse().ok()?;
    let max: f64 = read(d.join("max_brightness"))?.parse().ok()?;
    (max > 0.0).then(|| (cur / max).clamp(0.0, 1.0))
}

pub fn refresh_brightness(tx: &Sender<Msg>) {
    let _ = tx.send(Msg::Brightness(brightness()));
}

pub fn refresh_profile(tx: &Sender<Msg>) {
    if super::have("powerprofilesctl") {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Msg::PowerProfile(super::output("powerprofilesctl", &["get"])));
        });
    }
}

pub fn spawn(tx: Sender<Msg>) {
    refresh_brightness(&tx);
    refresh_profile(&tx);
    let b = battery();
    let has_battery = b.is_some();
    let _ = tx.send(Msg::Battery(b));
    if !has_battery {
        return;
    }
    std::thread::Builder::new()
        .name("battery".into())
        .stack_size(64 * 1024)
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(30));
            if tx.send(Msg::Battery(battery())).is_err() {
                return;
            }
        })
        .ok();
}

pub fn set_brightness(v: f64) {
    super::spawn_shell(&format!("brightnessctl -q set {}%", (v.clamp(0.01, 1.0) * 100.0).round()));
}

pub fn step_brightness(d: f64) {
    let pct = (d.abs() * 100.0).round() as i32;
    super::spawn_shell(&format!("brightnessctl -q set {pct}%{}", if d > 0.0 { "+" } else { "-" }));
}

pub fn set_profile(p: &str, tx: &Sender<Msg>) {
    if !["power-saver", "balanced", "performance"].contains(&p) {
        return;
    }
    let p = p.to_string();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let _ = std::process::Command::new("powerprofilesctl").args(["set", &p]).status();
        let _ = tx.send(Msg::PowerProfile(super::output("powerprofilesctl", &["get"])));
    });
}

/// lock / suspend / logout / reboot / poweroff
pub fn session(what: &str) {
    let cmd = match what {
        "lock" => "loginctl lock-session",
        "suspend" => "systemctl suspend",
        "logout" => "hyprctl dispatch 'hl.dsp.exit()'",
        "reboot" => "systemctl reboot",
        "poweroff" => "systemctl poweroff",
        _ => return,
    };
    super::spawn_shell(cmd);
}
