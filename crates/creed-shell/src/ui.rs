//! Immediate-mode UI: views draw every frame and register hit regions with actions. Input is
//! resolved against the previous frame's hits; animations are springs keyed by widget id.

use crate::gfx::{self, Rect, Text, Theme};
use crate::icons::Icons;
use cairo::Context;
use creed_core::Color;
use serde::Serialize;
use std::collections::HashMap;

pub type Id = u64;

pub fn id(s: &str) -> Id {
    // FNV-1a
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn idn(s: &str, n: impl std::fmt::Display) -> Id {
    id(&format!("{s}#{n}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    Wallpaper,
    Bar,
    Dock,
    Launcher,
    Control,
    Dashboard,
    Notifications,
    NotifCenter,
    Overview,
    Osd,
    Clipboard,
    Emoji,
    Power,
    Snap,
    Menu,
    Calendar,
    /// Desktop widgets (under windows).
    Desktop,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "wallpaper" => Kind::Wallpaper,
            "bar" => Kind::Bar,
            "dock" => Kind::Dock,
            "launcher" | "start" | "spotlight" => Kind::Launcher,
            "control" | "control-center" | "quick-settings" => Kind::Control,
            "dashboard" | "widgets" => Kind::Dashboard,
            "notifications" => Kind::Notifications,
            "notif-center" | "notification-center" => Kind::NotifCenter,
            "overview" | "taskview" | "mission-control" | "launchpad" => Kind::Overview,
            "osd" => Kind::Osd,
            "clipboard" => Kind::Clipboard,
            "emoji" => Kind::Emoji,
            "power" => Kind::Power,
            "snap" | "snap-layouts" => Kind::Snap,
            "menu" => Kind::Menu,
            "calendar" => Kind::Calendar,
            "desktop" | "desktop-widgets" => Kind::Desktop,
            _ => return None,
        })
    }
    pub fn name(&self) -> &'static str {
        match self {
            Kind::Wallpaper => "wallpaper",
            Kind::Bar => "bar",
            Kind::Dock => "dock",
            Kind::Launcher => "launcher",
            Kind::Control => "control",
            Kind::Dashboard => "dashboard",
            Kind::Notifications => "notifications",
            Kind::NotifCenter => "notif-center",
            Kind::Overview => "overview",
            Kind::Osd => "osd",
            Kind::Clipboard => "clipboard",
            Kind::Emoji => "emoji",
            Kind::Power => "power",
            Kind::Snap => "snap",
            Kind::Menu => "menu",
            Kind::Calendar => "calendar",
            Kind::Desktop => "desktop",
        }
    }
    /// Popups close each other and take keyboard focus.
    pub fn is_popup(&self) -> bool {
        !matches!(self, Kind::Wallpaper | Kind::Bar | Kind::Dock | Kind::Notifications | Kind::Osd | Kind::Desktop)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Slider {
    Volume,
    Mic,
    Brightness,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    None,
    Toggle(Kind),
    Open(Kind),
    Close(Kind),
    /// Run a shell command (detached).
    Exec(String),
    /// Launch an application by desktop-file id.
    Launch(String),
    /// Lua dispatcher expression for Hyprland.
    Hypr(String),
    FocusWindow(String),
    CloseWindow(String),
    Workspace(i64),
    WorkspaceScroll(i64),
    Slider(Slider),
    ToggleMute,
    ToggleMicMute,
    ToggleWifi,
    ToggleBluetooth,
    ToggleDnd,
    ToggleGame,
    ToggleNightLight,
    PowerProfile(String),
    MediaPlayPause,
    MediaNext,
    MediaPrev,
    NotifDismiss(u32),
    NotifInvoke(u32, String),
    NotifClear,
    TrayActivate(String),
    TrayMenu(String),
    MenuItem(i32),
    /// Open a macOS-style app menu (title, x position of its title in the bar).
    AppMenu(String, i32),
    ClipboardPick(String),
    EmojiPick(String),
    LauncherPick(usize),
    Page(u32),
    WifiConnect(String),
    Power(&'static str),
    Snap(u32, u32),
    Calendar(i32),
    VolumeStep(f64),
    /// Material sidebar rail: 0 calendar, 1 to-do.
    SideTab(u8),
    TodoAdd,
    TodoToggle(usize),
    TodoDelete(usize),
    BrightnessStep(f64),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    Button,
    Toggle,
    Slider,
    Item,
    Input,
    Label,
    Group,
    Backdrop,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub id: Id,
    pub rect: Rect,
    pub action: Action,
    pub role: Role,
    pub label: String,
    pub focusable: bool,
    /// Value for sliders / toggles (0..1 or 0/1).
    pub value: Option<f64>,
    /// Scroll handler (positive = up).
    pub scroll: Option<fn(f64) -> Action>,
    pub secondary: Option<Action>,
}

/// Named layout boxes recorded for `dump-layout` geometry tests.
#[derive(Clone, Debug, Serialize)]
pub struct LayoutBox {
    pub name: String,
    pub rect: [f64; 4],
    pub parent: Option<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub value: f64,
    pub vel: f64,
    pub target: f64,
}

#[derive(Default)]
pub struct Anims {
    pub springs: HashMap<Id, Spring>,
    pub seen: Vec<Id>,
}

impl Anims {
    pub fn gc(&mut self) {
        let seen: std::collections::HashSet<Id> = self.seen.drain(..).collect();
        self.springs.retain(|k, _| seen.contains(k));
    }
}

pub struct Ui<'a> {
    pub cr: &'a Context,
    pub th: &'a Theme,
    pub icons: &'a mut Icons,
    pub anims: &'a mut Anims,
    pub hits: Vec<Hit>,
    pub boxes: Vec<LayoutBox>,
    pub hover: Option<Id>,
    pub pressed: Option<Id>,
    pub focus: Option<Id>,
    pub pointer: Option<(f64, f64)>,
    pub dt: f64,
    pub animating: bool,
    pub size: (f64, f64),
}

impl<'a> Ui<'a> {
    /// Springs a value toward `target`. First call snaps to `initial`.
    pub fn spring(&mut self, key: Id, target: f64, initial: f64) -> f64 {
        self.anims.seen.push(key);
        let params = gfx::spring_params(self.th.motion());
        let s = self.anims.springs.entry(key).or_insert(Spring { value: initial, vel: 0.0, target });
        s.target = target;
        let Some((k, c)) = params else {
            s.value = target;
            s.vel = 0.0;
            return target;
        };
        let mut dt = self.dt.clamp(0.0, 0.05);
        // Sub-step for stability with stiff springs.
        while dt > 0.0 {
            let h = dt.min(1.0 / 240.0);
            let a = -k * (s.value - s.target) - c * s.vel;
            s.vel += a * h;
            s.value += s.vel * h;
            dt -= h;
        }
        if (s.value - s.target).abs() < 0.0015 && s.vel.abs() < 0.01 {
            s.value = s.target;
            s.vel = 0.0;
        } else {
            self.animating = true;
        }
        s.value
    }

    /// 0→1 hover amount for a widget.
    pub fn hover_amt(&mut self, key: Id) -> f64 {
        let on = if self.hover == Some(key) { 1.0 } else { 0.0 };
        self.spring(key ^ 0x5151, on, on)
    }

    pub fn is_pressed(&self, key: Id) -> bool {
        self.pressed == Some(key) && self.hover == Some(key)
    }

    pub fn hit(&mut self, key: Id, rect: Rect, action: Action, role: Role, label: impl Into<String>) -> &mut Hit {
        self.hits.push(Hit {
            id: key,
            rect,
            action,
            role,
            label: label.into(),
            focusable: !matches!(role, Role::Backdrop | Role::Label | Role::Group),
            value: None,
            scroll: None,
            secondary: None,
        });
        self.hits.last_mut().unwrap()
    }

    pub fn backdrop(&mut self, action: Action) {
        let (w, h) = self.size;
        self.hit(id("__backdrop"), Rect::new(0.0, 0.0, w, h), action, Role::Backdrop, "");
    }

    pub fn record(&mut self, name: &str, rect: Rect, parent: Option<&str>) {
        self.boxes.push(LayoutBox {
            name: name.to_string(),
            rect: [rect.x, rect.y, rect.w, rect.h],
            parent: parent.map(str::to_string),
        });
    }

    pub fn text(&self, r: Rect, s: &str, t: &Text) -> Rect {
        gfx::text(self.cr, self.th, r, s, t)
    }

    pub fn measure(&self, s: &str, t: &Text) -> (f64, f64) {
        gfx::measure(self.cr, self.th, s, t)
    }

    pub fn t(&self) -> Text {
        Text::new(self.th.fs, self.th.pal.text)
    }
    pub fn t_sub(&self) -> Text {
        Text::new(self.th.fs * 0.9, self.th.pal.subtext)
    }

    /// Draws a named icon (theme icon or symbolic glyph) centred in `r`.
    pub fn icon(&mut self, r: Rect, name: &str, size: f64, tint: Option<Color>) {
        let cr = self.cr;
        self.icons.draw(cr, name, Rect::centered(r.cx(), r.cy(), size, size), tint, self.th);
    }

    /// Flat icon button with hover background; returns (hover, rect).
    pub fn icon_button(&mut self, key: Id, r: Rect, icon: &str, label: &str, action: Action, active: bool) -> f64 {
        let h = self.hover_amt(key);
        let press = if self.is_pressed(key) { 0.94 } else { 1.0 };
        let rr = Rect::centered(r.cx(), r.cy(), r.w * press, r.h * press);
        let radius = rr.h.min(rr.w) / 2.0;
        if active {
            self.th.chip(self.cr, rr, radius, h, true);
        } else {
            self.th.hover_bg(self.cr, rr, radius, h);
        }
        let tint = if active { self.th.pal.on_accent } else { self.th.pal.text };
        let isz = (r.h * 0.5).round().max(12.0);
        self.icon(rr, icon, isz, Some(tint));
        if self.focus == Some(key) {
            self.th.focus_ring(self.cr, rr, radius);
        }
        self.hit(key, r, action, Role::Button, label);
        h
    }

    /// Pill toggle tile used by control centers.
    pub fn toggle_tile(&mut self, key: Id, r: Rect, icon: &str, title: &str, sub: &str, on: bool, action: Action) {
        let h = self.hover_amt(key);
        let on_amt = self.spring(key ^ 0x7777, if on { 1.0 } else { 0.0 }, if on { 1.0 } else { 0.0 });
        let radius = self.th.radius_small().max(r.h * 0.3);
        let p = &self.th.pal;
        let off_bg = if self.th.dark { Color::rgba(1.0, 1.0, 1.0, 0.07) } else { Color::rgba(0.0, 0.0, 0.0, 0.06) };
        let bg = off_bg.mix(p.accent, on_amt);
        gfx::fill_rounded(self.cr, r, radius, bg);
        self.th.hover_bg(self.cr, r, radius, h * 0.7);
        let fg = p.text.mix(p.on_accent, on_amt);
        let sub_c = p.subtext.mix(p.on_accent.with_alpha(0.8), on_amt);
        let pad = (r.h * 0.28).round();
        let (ic, rest) = r.inset_xy(pad, 0.0).take_left(r.h * 0.42, pad * 0.7);
        self.icon(ic, icon, (r.h * 0.38).round(), Some(fg));
        if sub.is_empty() {
            self.text(rest, title, &Text::new(self.th.fs, fg).medium());
        } else {
            let lh = self.th.fs * 1.25;
            let top = Rect::new(rest.x, r.cy() - lh, rest.w, lh);
            let bot = Rect::new(rest.x, r.cy(), rest.w, lh);
            self.text(top, title, &Text::new(self.th.fs, fg).medium());
            self.text(bot, sub, &Text::new(self.th.fs * 0.85, sub_c));
        }
        if self.focus == Some(key) {
            self.th.focus_ring(self.cr, r, radius);
        }
        let label = format!("{title}{}", if sub.is_empty() { String::new() } else { format!(", {sub}") });
        let hit = self.hit(key, r, action, Role::Toggle, label);
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }

    /// Horizontal slider with icon. `value` 0..1.
    pub fn slider(&mut self, key: Id, r: Rect, icon: &str, label: &str, value: f64, which: Slider, icon_action: Action) {
        let v = self.spring(key ^ 0x3131, value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
        let p = &self.th.pal;
        let radius = r.h / 2.0;
        let track = if self.th.dark { Color::rgba(1.0, 1.0, 1.0, 0.08) } else { Color::rgba(0.0, 0.0, 0.0, 0.07) };
        gfx::fill_rounded(self.cr, r, radius, track);
        let fill_w = (r.h + (r.w - r.h) * v).min(r.w);
        gfx::fill_rounded(self.cr, Rect::new(r.x, r.y, fill_w, r.h), radius, p.accent);
        // Knob: subtle inner dot at the end of the fill.
        let kx = r.x + fill_w - r.h / 2.0;
        gfx::fill_rounded(self.cr, Rect::centered(kx, r.cy(), r.h * 0.42, r.h * 0.42), r.h, p.on_accent.with_alpha(0.9));
        let ic = Rect::new(r.x, r.y, r.h, r.h);
        self.icon(ic, icon, (r.h * 0.5).round(), Some(p.on_accent));
        let icon_key = key ^ 0xabab;
        self.hit(icon_key, ic, icon_action, Role::Button, format!("{label} mute"));
        if self.focus == Some(key) {
            self.th.focus_ring(self.cr, r, radius);
        }
        let scroll: Option<fn(f64) -> Action> = match which {
            Slider::Volume => Some(|d| Action::VolumeStep(d * 0.05)),
            Slider::Brightness => Some(|d| Action::BrightnessStep(d * 0.05)),
            Slider::Mic => None,
        };
        let hit = self.hit(key, r, Action::Slider(which), Role::Slider, label);
        hit.value = Some(value);
        hit.scroll = scroll;
        // Icon hit must win over the slider: move it after.
        let n = self.hits.len();
        self.hits.swap(n - 1, n - 2);
    }

    /// List row with optional icon, title, subtitle; returns hover amount.
    pub fn row(&mut self, key: Id, r: Rect, icon: Option<&str>, title: &str, sub: &str, selected: bool, action: Action) -> f64 {
        let h = self.hover_amt(key);
        let radius = self.th.radius_small();
        if selected {
            gfx::fill_rounded(self.cr, r, radius, self.th.pal.accent.with_alpha(if self.th.dark { 0.22 } else { 0.16 }));
        }
        self.th.hover_bg(self.cr, r, radius, h);
        let pad = 10.0;
        let mut rest = r.inset_xy(pad, 0.0);
        if let Some(ic) = icon {
            let isz = (r.h * 0.62).round().min(40.0);
            let (icr, rr) = rest.take_left(isz, 10.0);
            self.icon(icr, ic, isz, None);
            rest = rr;
        }
        if sub.is_empty() {
            self.text(rest, title, &self.t());
        } else {
            let lh = self.th.fs * 1.3;
            self.text(Rect::new(rest.x, r.cy() - lh + 1.0, rest.w, lh), title, &self.t().medium());
            self.text(Rect::new(rest.x, r.cy() + 1.0, rest.w, lh), sub, &self.t_sub());
        }
        if self.focus == Some(key) {
            self.th.focus_ring(self.cr, r, radius);
        }
        self.hit(key, r, action, Role::Item, title.to_string());
        h
    }

    /// Text input field (display only; editing handled by the surface).
    pub fn input(&mut self, key: Id, r: Rect, text: &str, placeholder: &str, focused: bool, preedit: &str) {
        let p = &self.th.pal;
        let radius = (r.h / 2.0).min(self.th.radius());
        let bg = if self.th.dark { Color::rgba(1.0, 1.0, 1.0, 0.07) } else { Color::rgba(0.0, 0.0, 0.0, 0.05) };
        gfx::fill_rounded(self.cr, r, radius, bg);
        let inner = r.inset_xy(r.h * 0.42, 0.0);
        let (ic, rest) = inner.take_left(r.h * 0.42, 10.0);
        self.icon(ic, "system-search-symbolic", (r.h * 0.4).round(), Some(p.subtext));
        let t = Text::new(self.th.fs * 1.12, p.text);
        if text.is_empty() && preedit.is_empty() {
            self.text(rest, placeholder, &Text::new(self.th.fs * 1.12, p.muted));
        }
        let shown = format!("{text}{preedit}");
        let b = if shown.is_empty() { Rect::new(rest.x, rest.y, 0.0, rest.h) } else { self.text(rest, &shown, &t) };
        if !preedit.is_empty() {
            let (pw, _) = self.measure(preedit, &t);
            let ux = (b.right() - pw).max(rest.x);
            gfx::fill_rounded(self.cr, Rect::new(ux, b.bottom() - 1.0, pw, 1.5), 0.0, p.accent);
        }
        if focused {
            // caret
            let cx = if shown.is_empty() { rest.x } else { b.right() + 1.0 };
            let ch = self.th.fs * 1.35;
            gfx::fill_rounded(self.cr, Rect::new(cx.min(rest.right()), r.cy() - ch / 2.0, 1.6, ch), 0.8, p.accent);
        }
        self.hit(key, r, Action::None, Role::Input, placeholder.to_string());
    }
}
