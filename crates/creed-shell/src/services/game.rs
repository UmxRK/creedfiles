//! Game Mode: detection and effects toggling.

use creed_core::hypr::{Client, Hypr};
use creed_core::Config;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

static AUTO: AtomicBool = AtomicBool::new(false);
static PID: AtomicI64 = AtomicI64::new(0);

pub fn auto_started() -> bool {
    AUTO.load(Ordering::SeqCst)
}

const LAUNCHER_ENV: &[&str] = &["SteamGameId=", "SteamAppId=", "LUTRIS_GAME_UUID=", "HEROIC_APP_NAME=", "GAMEID=", "STEAM_COMPAT_APP_ID="];

fn env_says_game(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    let Ok(env) = std::fs::read(format!("/proc/{pid}/environ")) else { return false };
    env.split(|&b| b == 0).any(|kv| {
        let s = String::from_utf8_lossy(kv);
        LAUNCHER_ENV.iter().any(|p| s.starts_with(p) && s.len() > p.len() && !s.ends_with("=0"))
    })
}

pub fn is_game(c: &Client, extra: &[String]) -> bool {
    let class = c.class.to_lowercase();
    if class.starts_with("steam_app_") && class != "steam_app_0" {
        return true;
    }
    if class == "gamescope" || extra.iter().any(|e| e.to_lowercase() == class) {
        return true;
    }
    c.fullscreen > 0 && env_says_game(c.pid)
}

/// Returns the PID of a running game window, if any.
pub fn detect(clients: &[Client], extra: &[String]) -> Option<i64> {
    clients.iter().find(|c| is_game(c, extra)).map(|c| c.pid.max(1))
}

fn gamemode(method: &str, pid: i64) {
    if pid <= 0 {
        return;
    }
    let method = method.to_string();
    std::thread::spawn(move || {
        if let Some(conn) = super::session_bus() {
            let _ = conn.call_method(Some("com.feralinteractive.GameMode"), "/com/feralinteractive/GameMode", Some("com.feralinteractive.GameMode"), method.as_str(), &(pid as i32));
        }
    });
}

pub fn apply(on: bool, pid: Option<i64>, cfg: &Config, hypr: Option<&Hypr>) {
    AUTO.store(on && pid.is_some(), Ordering::SeqCst);
    if on {
        if let Some(p) = pid {
            PID.store(p, Ordering::SeqCst);
            if cfg.game.performance_governor {
                gamemode("RegisterGame", p);
            }
        }
        if cfg.game.disable_effects {
            if let Some(h) = hypr {
                let _ = h.eval(EFFECTS_OFF);
            }
        }
    } else {
        let p = PID.swap(0, Ordering::SeqCst);
        if cfg.game.performance_governor {
            gamemode("UnregisterGame", p);
        }
        if cfg.game.disable_effects {
            if let Some(h) = hypr {
                // Re-running the config restores blur, shadows and animations exactly.
                let _ = h.request("reload config-only");
            }
        }
    }
}

pub const EFFECTS_OFF: &str = "hl.config({ decoration = { blur = { enabled = false }, shadow = { enabled = false } }, animations = { enabled = false }, general = { allow_tearing = true } })";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn detection() {
        let mut c = Client { class: "steam_app_570".into(), ..Default::default() };
        assert!(is_game(&c, &[]));
        c.class = "firefox".into();
        assert!(!is_game(&c, &[]));
        assert!(is_game(&c, &["Firefox".into()]));
        c.class = "gamescope".into();
        assert!(is_game(&c, &[]));
        c.class = "foot".into();
        c.fullscreen = 2;
        c.pid = std::process::id() as i64;
        // Our own environment has no launcher variables.
        assert!(!is_game(&c, &[]));
    }
}
