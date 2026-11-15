//! NetworkManager via nmcli (monitor for changes, terse output for state).

use crate::app::Msg;
use crate::data::{AccessPoint, Net, NetKind};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

/// Splits nmcli terse output respecting `\:` escapes.
pub fn split_terse(line: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut esc = false;
    for c in line.chars() {
        if esc {
            out.last_mut().unwrap().push(c);
            esc = false;
        } else if c == '\\' {
            esc = true;
        } else if c == ':' {
            out.push(String::new());
        } else {
            out.last_mut().unwrap().push(c);
        }
    }
    out
}

pub fn state() -> Net {
    let mut net = Net::default();
    let Some(radio) = super::output("nmcli", &["-t", "-f", "WIFI", "radio"]) else { return net };
    net.available = true;
    net.wifi_enabled = radio.trim() == "enabled";
    if let Some(devs) = super::output("nmcli", &["-t", "-f", "TYPE,STATE,CONNECTION", "device"]) {
        for l in devs.lines() {
            let f = split_terse(l);
            if f.len() < 3 || f[1] != "connected" {
                continue;
            }
            match f[0].as_str() {
                "ethernet" if net.kind == NetKind::None => {
                    net.kind = NetKind::Ethernet;
                    net.name = f[2].clone();
                }
                "wifi" => {
                    net.kind = NetKind::Wifi;
                    net.name = f[2].clone();
                }
                _ => {}
            }
        }
    }
    if net.kind == NetKind::Wifi {
        if let Some(list) = super::output("nmcli", &["-t", "-f", "IN-USE,SIGNAL,SSID", "device", "wifi", "list", "--rescan", "no"]) {
            for l in list.lines() {
                let f = split_terse(l);
                if f.len() >= 3 && f[0] == "*" {
                    net.signal = f[1].parse().unwrap_or(0);
                    if !f[2].is_empty() {
                        net.name = f[2].clone();
                    }
                }
            }
        }
    }
    net
}

pub fn parse_aps(list: &str) -> Vec<AccessPoint> {
    let mut aps: Vec<AccessPoint> = vec![];
    for l in list.lines() {
        let f = split_terse(l);
        if f.len() < 4 || f[2].is_empty() {
            continue;
        }
        let ap = AccessPoint { active: f[0] == "*", signal: f[1].parse().unwrap_or(0), ssid: f[2].clone(), secure: !f[3].is_empty() && f[3] != "--" };
        match aps.iter_mut().find(|a| a.ssid == ap.ssid) {
            Some(existing) => {
                if ap.signal > existing.signal || ap.active {
                    existing.signal = existing.signal.max(ap.signal);
                    existing.active |= ap.active;
                }
            }
            None => aps.push(ap),
        }
    }
    aps.sort_by(|a, b| b.active.cmp(&a.active).then(b.signal.cmp(&a.signal)));
    aps
}

pub fn scan(tx: Sender<Msg>) {
    if !super::have("nmcli") {
        return;
    }
    std::thread::spawn(move || {
        if let Some(l) = super::output("nmcli", &["-t", "-f", "IN-USE,SIGNAL,SSID,SECURITY", "device", "wifi", "list", "--rescan", "no"]) {
            let _ = tx.send(Msg::Aps(parse_aps(&l)));
        }
        if let Some(l) = super::output("nmcli", &["-t", "-f", "IN-USE,SIGNAL,SSID,SECURITY", "device", "wifi", "list", "--rescan", "yes"]) {
            let _ = tx.send(Msg::Aps(parse_aps(&l)));
        }
    });
}

pub fn known(ssid: &str) -> bool {
    super::output("nmcli", &["-t", "-f", "NAME", "connection", "show"]).map(|l| l.lines().any(|n| split_terse(n).first().map(|s| s == ssid).unwrap_or(false))).unwrap_or(false)
}

pub fn connect(ssid: &str, password: Option<&str>, tx: &Sender<Msg>) {
    let ssid = ssid.to_string();
    let pw = password.map(str::to_string);
    let tx = tx.clone();
    std::thread::spawn(move || {
        let mut c = Command::new("nmcli");
        if known(&ssid) {
            c.args(["connection", "up", "id", &ssid]);
        } else {
            c.args(["device", "wifi", "connect", &ssid]);
            if let Some(p) = &pw {
                c.args(["password", p]);
            }
        }
        let ok = c.stdout(Stdio::null()).stderr(Stdio::null()).status().map(|s| s.success()).unwrap_or(false);
        if !ok {
            super::spawn_shell(&format!("notify-send -a Creed -i network-wireless-symbolic {} {}", super::shell_quote("Could not connect"), super::shell_quote(&ssid)));
        }
        let _ = tx.send(Msg::Net(state()));
    });
}

pub fn toggle_wifi(on: bool, tx: &Sender<Msg>) {
    let tx = tx.clone();
    std::thread::spawn(move || {
        let _ = Command::new("nmcli").args(["radio", "wifi", if on { "on" } else { "off" }]).status();
        let _ = tx.send(Msg::Net(state()));
    });
}

pub fn spawn(tx: Sender<Msg>) {
    if !super::have("nmcli") {
        let _ = tx.send(Msg::Net(Net::default()));
        return;
    }
    std::thread::Builder::new()
        .name("network".into())
        .stack_size(128 * 1024)
        .spawn(move || {
            if tx.send(Msg::Net(state())).is_err() {
                return;
            }
            loop {
                let child = Command::new("nmcli").arg("monitor").stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
                let Ok(mut child) = child else { return };
                let Some(out) = child.stdout.take() else { return };
                let mut last = std::time::Instant::now() - std::time::Duration::from_secs(5);
                for _line in BufReader::new(out).lines().map_while(Result::ok) {
                    // Debounce bursts of events.
                    if last.elapsed().as_millis() < 500 {
                        continue;
                    }
                    last = std::time::Instant::now();
                    std::thread::sleep(std::time::Duration::from_millis(300));
                    if tx.send(Msg::Net(state())).is_err() {
                        return;
                    }
                }
                let _ = child.wait();
                std::thread::sleep(std::time::Duration::from_secs(5));
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn terse_and_aps() {
        assert_eq!(split_terse(r"a\:b:c"), vec!["a:b", "c"]);
        let aps = parse_aps("*:80:Home:WPA2\n :40:Cafe:\n :90:Home:WPA2\n :10::WPA2\n");
        assert_eq!(aps.len(), 2);
        assert_eq!(aps[0].ssid, "Home");
        assert!(aps[0].active && aps[0].secure && aps[0].signal == 90);
        assert!(!aps[1].secure);
    }
}
