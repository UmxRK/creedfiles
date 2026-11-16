//! Shared state the services feed and the views read.

use creed_core::hypr::{Client, Monitor, Workspace};
use std::time::Instant;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Audio {
    pub volume: f64,
    pub muted: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Battery {
    pub percent: f64,
    pub charging: bool,
    pub full: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum NetKind {
    #[default]
    None,
    Wifi,
    Ethernet,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Net {
    pub kind: NetKind,
    pub name: String,
    pub signal: u8,
    pub wifi_enabled: bool,
    pub available: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessPoint {
    pub ssid: String,
    pub signal: u8,
    pub secure: bool,
    pub active: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Bluetooth {
    pub powered: bool,
    pub connected: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Media {
    pub bus: String,
    pub player: String,
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub can_next: bool,
    pub can_prev: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Notif {
    pub id: u32,
    pub app: String,
    pub icon: String,
    pub summary: String,
    pub body: String,
    pub actions: Vec<(String, String)>,
    pub urgency: u8,
    pub timeout_ms: i32,
    pub arrived: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrayItem {
    pub key: String,
    pub id: String,
    pub title: String,
    pub icon: String,
    pub pixmap: Option<(i32, i32, Vec<u8>)>,
    pub menu: Option<String>,
    pub item_is_menu: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuEntry {
    pub id: i32,
    /// Keyboard shortcut shown on the right (app menus only).
    pub accel: String,
    pub label: String,
    pub enabled: bool,
    pub separator: bool,
    pub toggle: Option<bool>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AppEntry {
    pub id: String,
    pub name: String,
    pub generic: String,
    pub icon: String,
    pub exec: String,
    pub keywords: String,
    pub terminal: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SysStat {
    pub cpu: f64,
    pub mem_used_mb: u64,
    pub mem_total_mb: u64,
    pub temp_c: Option<f64>,
    pub disk_used_gb: f64,
    pub disk_total_gb: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherNow {
    pub temp: String,
    pub desc: String,
    pub icon: String,
    pub place: String,
    pub hi_lo: String,
    /// Next forecast slots: (hour of day 0–23, temperature "12°", icon).
    pub hourly: Vec<(u32, String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OsdKind {
    Volume,
    Mic,
    Brightness,
    Game,
}

#[derive(Default)]
pub struct Data {
    pub workspaces: Vec<Workspace>,
    pub active_ws: i64,
    pub clients: Vec<Client>,
    pub active_addr: String,
    pub monitors: Vec<Monitor>,
    pub focused_monitor: String,
    pub kb_layout: String,
    pub hypr_ok: bool,

    pub audio: Option<Audio>,
    pub mic: Option<Audio>,
    pub brightness: Option<f64>,
    pub battery: Option<Battery>,
    pub power_profile: Option<String>,
    pub net: Net,
    pub aps: Vec<AccessPoint>,
    pub bt: Option<Bluetooth>,
    pub media: Option<Media>,
    pub notifs: Vec<Notif>,
    pub history: Vec<Notif>,
    pub dnd: bool,
    pub tray: Vec<TrayItem>,
    pub menu: Vec<MenuEntry>,
    pub menu_owner: String,
    pub apps: Vec<AppEntry>,
    /// Recently used files (path, display name, when) from recently-used.xbel.
    pub recent: Vec<(String, String, String)>,
    /// Live window previews (address → small image), only while the overview is open.
    pub thumbs: std::collections::HashMap<String, cairo::ImageSurface>,
    pub clips: Vec<(String, String)>,
    pub sys: SysStat,
    pub weather: Option<WeatherNow>,
    pub game: bool,
    pub night_light: bool,
    pub osd: Option<(OsdKind, f64, Instant)>,
    pub uptime: String,
    pub user: String,
    pub host: String,
}

impl Data {
    /// Windows that belong on a taskbar/dock: normal workspaces plus minimised ones.
    pub fn is_app_window(c: &Client) -> bool {
        !c.class.is_empty() && (c.workspace.id > 0 || c.workspace.name == "special:minimized")
    }

    pub fn is_minimized(&self, addr: &str) -> bool {
        self.clients.iter().any(|c| c.address == addr && c.workspace.name == "special:minimized")
    }

    pub fn active_client(&self) -> Option<&Client> {
        self.clients.iter().find(|c| c.address == self.active_addr)
    }

    /// Pretty application name from a window class (`org.gnome.Nautilus` → `Nautilus`).
    pub fn pretty_class(class: &str) -> String {
        let last = class.rsplit('.').next().unwrap_or(class);
        let mut s: String = last.replace(['-', '_'], " ");
        if let Some(f) = s.get_mut(0..1) {
            f.make_ascii_uppercase();
        }
        s
    }
}
