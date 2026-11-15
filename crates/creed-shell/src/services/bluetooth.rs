//! Bluetooth via bluetoothctl (polled slowly; refreshed on demand).

use crate::app::Msg;
use crate::data::Bluetooth;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::time::Duration;

pub fn state() -> Option<Bluetooth> {
    let show = super::output("bluetoothctl", &["show"])?;
    if !show.contains("Controller") {
        return None;
    }
    let powered = show.lines().any(|l| l.trim() == "Powered: yes");
    let mut connected = vec![];
    if powered {
        if let Some(devs) = super::output("bluetoothctl", &["devices", "Connected"]) {
            for l in devs.lines() {
                // "Device AA:BB:CC:DD:EE:FF Name with spaces"
                let mut it = l.splitn(3, ' ');
                if it.next() == Some("Device") {
                    let _mac = it.next();
                    if let Some(name) = it.next() {
                        connected.push(name.to_string());
                    }
                }
            }
        }
    }
    Some(Bluetooth { powered, connected })
}

pub fn refresh(tx: Sender<Msg>) {
    if !super::have("bluetoothctl") {
        return;
    }
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Bt(state()));
    });
}

pub fn spawn(tx: Sender<Msg>) {
    if !super::have("bluetoothctl") {
        return;
    }
    std::thread::Builder::new()
        .name("bluetooth".into())
        .stack_size(64 * 1024)
        .spawn(move || loop {
            let s = state();
            let present = s.is_some();
            if tx.send(Msg::Bt(s)).is_err() || !present {
                return;
            }
            std::thread::sleep(Duration::from_secs(20));
        })
        .ok();
}

pub fn set_power(on: bool, tx: &Sender<Msg>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        if on {
            let _ = std::process::Command::new("rfkill").args(["unblock", "bluetooth"]).status();
        }
        let _ = std::process::Command::new("bluetoothctl").args(["power", if on { "on" } else { "off" }]).status();
        let _ = tx.send(Msg::Bt(state()));
    });
}
