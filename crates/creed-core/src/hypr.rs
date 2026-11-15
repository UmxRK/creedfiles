//! Hyprland IPC: request socket (`.socket.sock`) and event socket (`.socket2.sock`).
//! Hyprland ≥ 0.57 takes Lua dispatchers: `dispatch hl.dsp.focus({ workspace = 3 })`.

use serde::Deserialize;
use std::env;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Hypr {
    pub dir: PathBuf,
}

impl Hypr {
    pub fn from_env() -> Option<Hypr> {
        let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").ok()?;
        let rt = env::var("XDG_RUNTIME_DIR").ok()?;
        let dir = PathBuf::from(rt).join("hypr").join(sig);
        dir.join(".socket.sock").exists().then_some(Hypr { dir })
    }

    pub fn request(&self, cmd: &str) -> std::io::Result<String> {
        let mut s = UnixStream::connect(self.dir.join(".socket.sock"))?;
        s.set_read_timeout(Some(Duration::from_secs(2)))?;
        s.set_write_timeout(Some(Duration::from_secs(2)))?;
        s.write_all(cmd.as_bytes())?;
        let mut out = String::new();
        s.read_to_string(&mut out)?;
        Ok(out)
    }

    pub fn json<T: for<'de> Deserialize<'de>>(&self, what: &str) -> Option<T> {
        let txt = self.request(&format!("j/{what}")).ok()?;
        serde_json::from_str(&txt).ok()
    }

    /// Runs a Lua dispatcher expression, e.g. `hl.dsp.focus({ workspace = 2 })`.
    pub fn dispatch(&self, lua: &str) -> std::io::Result<String> {
        self.request(&format!("dispatch {lua}"))
    }

    pub fn eval(&self, lua: &str) -> std::io::Result<String> {
        self.request(&format!("eval {lua}"))
    }

    pub fn event_socket(&self) -> std::io::Result<UnixStream> {
        UnixStream::connect(self.dir.join(".socket2.sock"))
    }

    pub fn workspaces(&self) -> Vec<Workspace> {
        self.json("workspaces").unwrap_or_default()
    }
    pub fn clients(&self) -> Vec<Client> {
        self.json("clients").unwrap_or_default()
    }
    pub fn monitors(&self) -> Vec<Monitor> {
        self.json("monitors").unwrap_or_default()
    }
    pub fn active_window(&self) -> Option<Client> {
        self.json::<Client>("activewindow").filter(|c| !c.address.is_empty())
    }
}

/// Quotes a string as a Lua literal.
pub fn lua_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '"' => o.push_str("\\\""),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\{}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct WsRef {
    pub id: i64,
    pub name: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Workspace {
    pub id: i64,
    pub name: String,
    pub monitor: String,
    pub windows: u32,
    pub hasfullscreen: bool,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Client {
    pub address: String,
    pub mapped: bool,
    pub hidden: bool,
    pub at: [i32; 2],
    pub size: [i32; 2],
    pub workspace: WsRef,
    pub floating: bool,
    pub monitor: i64,
    pub class: String,
    pub title: String,
    pub initial_class: String,
    pub initial_title: String,
    pub pid: i64,
    pub xwayland: bool,
    pub pinned: bool,
    pub fullscreen: i64,
    #[serde(rename = "focusHistoryID")]
    pub focus_history_id: i64,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Monitor {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub width: i32,
    pub height: i32,
    pub refresh_rate: f64,
    pub x: i32,
    pub y: i32,
    pub scale: f64,
    pub focused: bool,
    pub active_workspace: WsRef,
    pub vrr: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum HyprEvent {
    Workspace { id: i64, name: String },
    FocusedMonitor { monitor: String, workspace: i64 },
    ActiveWindow { address: String },
    OpenWindow { address: String, workspace: String, class: String, title: String },
    CloseWindow { address: String },
    MoveWindow { address: String, workspace: i64 },
    WindowTitle { address: String, title: String },
    Fullscreen(bool),
    WorkspaceCreated { id: i64 },
    WorkspaceDestroyed { id: i64 },
    MonitorAdded(String),
    MonitorRemoved(String),
    SpecialWorkspace { name: String, monitor: String },
    Urgent { address: String },
    ConfigReloaded,
    Other { name: String, data: String },
}

impl HyprEvent {
    pub fn parse(line: &str) -> Option<HyprEvent> {
        let (name, data) = line.trim_end_matches('\n').split_once(">>")?;
        let parts = |n: usize| data.splitn(n, ',').map(str::to_string).collect::<Vec<_>>();
        let addr = |s: &str| if s.starts_with("0x") { s.to_string() } else { format!("0x{s}") };
        let num = |s: &str| s.trim().parse::<i64>().ok();
        Some(match name {
            "workspacev2" => {
                let p = parts(2);
                HyprEvent::Workspace { id: num(&p[0])?, name: p.get(1).cloned().unwrap_or_default() }
            }
            "focusedmonv2" => {
                let p = parts(2);
                HyprEvent::FocusedMonitor { monitor: p[0].clone(), workspace: num(p.get(1)?)? }
            }
            "activewindowv2" => HyprEvent::ActiveWindow { address: if data.is_empty() { String::new() } else { addr(data) } },
            "openwindow" => {
                let p = parts(4);
                if p.len() < 4 {
                    return None;
                }
                HyprEvent::OpenWindow { address: addr(&p[0]), workspace: p[1].clone(), class: p[2].clone(), title: p[3].clone() }
            }
            "closewindow" => HyprEvent::CloseWindow { address: addr(data) },
            "movewindowv2" => {
                let p = parts(3);
                HyprEvent::MoveWindow { address: addr(&p[0]), workspace: num(p.get(1)?)? }
            }
            "windowtitlev2" => {
                let p = parts(2);
                HyprEvent::WindowTitle { address: addr(&p[0]), title: p.get(1).cloned().unwrap_or_default() }
            }
            "fullscreen" => HyprEvent::Fullscreen(data.trim() == "1"),
            "createworkspacev2" => HyprEvent::WorkspaceCreated { id: num(&parts(2)[0])? },
            "destroyworkspacev2" => HyprEvent::WorkspaceDestroyed { id: num(&parts(2)[0])? },
            "monitoraddedv2" => HyprEvent::MonitorAdded(parts(3).get(1).cloned().unwrap_or_default()),
            "monitorremovedv2" => HyprEvent::MonitorRemoved(parts(3).get(1).cloned().unwrap_or_default()),
            "monitoradded" => HyprEvent::MonitorAdded(data.to_string()),
            "monitorremoved" => HyprEvent::MonitorRemoved(data.to_string()),
            "activespecialv2" => {
                let p = parts(3);
                HyprEvent::SpecialWorkspace { name: p.get(1).cloned().unwrap_or_default(), monitor: p.get(2).cloned().unwrap_or_default() }
            }
            "urgent" => HyprEvent::Urgent { address: addr(data) },
            "configreloaded" => HyprEvent::ConfigReloaded,
            _ => HyprEvent::Other { name: name.to_string(), data: data.to_string() },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_events() {
        assert_eq!(HyprEvent::parse("workspacev2>>3,3"), Some(HyprEvent::Workspace { id: 3, name: "3".into() }));
        assert_eq!(
            HyprEvent::parse("openwindow>>55d1a2b0,2,foot,user@host: ~, a,b"),
            Some(HyprEvent::OpenWindow {
                address: "0x55d1a2b0".into(),
                workspace: "2".into(),
                class: "foot".into(),
                title: "user@host: ~, a,b".into()
            })
        );
        assert_eq!(HyprEvent::parse("activewindowv2>>"), Some(HyprEvent::ActiveWindow { address: String::new() }));
        assert_eq!(HyprEvent::parse("fullscreen>>1"), Some(HyprEvent::Fullscreen(true)));
        assert_eq!(HyprEvent::parse("movewindowv2>>abc,4,4"), Some(HyprEvent::MoveWindow { address: "0xabc".into(), workspace: 4 }));
        assert_eq!(HyprEvent::parse("windowtitlev2>>abc,hi, there"), Some(HyprEvent::WindowTitle { address: "0xabc".into(), title: "hi, there".into() }));
        assert_eq!(HyprEvent::parse("focusedmonv2>>DP-1,2"), Some(HyprEvent::FocusedMonitor { monitor: "DP-1".into(), workspace: 2 }));
        assert_eq!(HyprEvent::parse("monitoraddedv2>>1,HDMI-A-1,desc"), Some(HyprEvent::MonitorAdded("HDMI-A-1".into())));
        assert_eq!(HyprEvent::parse("configreloaded>>"), Some(HyprEvent::ConfigReloaded));
        assert!(matches!(HyprEvent::parse("submap>>resize"), Some(HyprEvent::Other { .. })));
        assert_eq!(HyprEvent::parse("garbage"), None);
        assert_eq!(HyprEvent::parse("workspacev2>>x,y"), None);
    }

    #[test]
    fn parse_clients_json() {
        let j = r#"[{"address":"0x1","mapped":true,"hidden":false,"at":[10,20],"size":[800,600],
            "workspace":{"id":2,"name":"2"},"floating":false,"monitor":0,"class":"foot","title":"t",
            "initialClass":"foot","initialTitle":"t","pid":42,"xwayland":false,"pinned":false,
            "fullscreen":0,"focusHistoryID":0,"unknownField":5}]"#;
        let c: Vec<Client> = serde_json::from_str(j).unwrap();
        assert_eq!(c[0].workspace.id, 2);
        assert_eq!(c[0].initial_class, "foot");
        assert_eq!(c[0].size, [800, 600]);
    }

    #[test]
    fn lua_quoting() {
        assert_eq!(lua_str("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }
}
