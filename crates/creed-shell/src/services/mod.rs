//! Background services. Each one is optional: if its backend is missing it reports `None`
//! once and stays quiet, and the matching UI hides itself.

pub mod apps;
pub mod audio;
pub mod bluetooth;
pub mod clipboard;
pub mod game;
pub mod hypr;
pub mod ipc;
pub mod mpris;
pub mod network;
pub mod notify;
pub mod power;
pub mod sysstat;
pub mod tray;
pub mod weather;

use crate::app::Msg;
use crate::ui::Kind;
use creed_core::Config;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::process::{Command, Stdio};
use std::sync::OnceLock;

/// Shared session-bus connection for short calls (MPRIS, tray).
pub fn session_bus() -> Option<&'static zbus::blocking::Connection> {
    static BUS: OnceLock<Option<zbus::blocking::Connection>> = OnceLock::new();
    BUS.get_or_init(|| zbus::blocking::Connection::session().ok()).as_ref()
}

pub fn have(cmd: &str) -> bool {
    std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file()))
        .unwrap_or(false)
}

/// Runs a command and returns trimmed stdout if it succeeded.
pub fn output(cmd: &str, args: &[&str]) -> Option<String> {
    let o = Command::new(cmd).args(args).stdin(Stdio::null()).stderr(Stdio::null()).output().ok()?;
    o.status.success().then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Fire-and-forget shell command, fully detached so it outlives the shell.
pub fn spawn_shell(cmd: &str) {
    let _ = Command::new("sh")
        .arg("-c")
        .arg(format!("setsid -f sh -c {} >/dev/null 2>&1 </dev/null", shell_quote(cmd)))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|mut c| std::thread::spawn(move || c.wait()));
}

pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn start_all(tx: &Sender<Msg>, cfg: &Config) {
    hypr::spawn(tx.clone());
    ipc::spawn(tx.clone());
    audio::spawn(tx.clone());
    power::spawn(tx.clone());
    network::spawn(tx.clone());
    bluetooth::spawn(tx.clone());
    mpris::spawn(tx.clone());
    tray::spawn(tx.clone());
    apps::refresh(tx.clone());
    sysstat::spawn(tx.clone());
    if cfg.bar.left.iter().chain(&cfg.bar.center).chain(&cfg.bar.right).any(|m| m == "sysstat") {
        sysstat::set_active("bar", true);
    }
    if cfg.weather.enabled {
        weather::spawn(tx.clone(), cfg.weather.location.clone(), cfg.weather.metric);
    }
}

/// Things to refresh right before a surface opens.
pub fn on_open(kind: Kind, tx: &Sender<Msg>, _cfg: &Config) {
    match kind {
        Kind::Launcher | Kind::Overview => {
            apps::refresh(tx.clone());
            apps::refresh_recent(tx.clone());
        }
        Kind::Control => {
            network::scan(tx.clone());
            power::refresh_profile(tx);
            bluetooth::refresh(tx.clone());
        }
        Kind::Clipboard => clipboard::refresh(tx.clone()),
        Kind::Dashboard => {
            sysstat::set_active("dashboard", true);
            sysstat::sample_now(tx.clone());
        }
        _ => {}
    }
    if kind != Kind::Dashboard {
        sysstat::set_active("dashboard", false);
    }
}
