//! Per-surface views. Each surface kind has a layer spec (size/anchor) and a draw function;
//! drawing branches on the mode's style variant (creed / win11 / win10 / mac / aqua).

pub mod bar;
pub mod calendar;
pub mod clipboard;
pub mod control;
pub mod dashboard;
pub mod dock;
pub mod emoji;
pub mod fluent;
pub mod mac;
pub mod material;
pub mod todo;
pub mod win10;
pub mod win11;

pub use notif::ago as notif_ago;
pub mod launcher;
pub mod menu;
pub mod notif;
pub mod osd;
pub mod overview;
pub mod power;
pub mod snap;
pub mod wallpaper;

use crate::data::{AppEntry, Data};
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Action, Hit, Kind, Ui};
use creed_core::config::Edge;
use creed_core::Paths;
use smithay_client_toolkit::seat::keyboard::{Keysym, Modifiers};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    pub layer: Layer,
    pub anchor: Anchor,
    pub size: (u32, u32),
    pub margin: (i32, i32, i32, i32),
    pub exclusive: i32,
    pub keyboard: KeyboardInteractivity,
    pub namespace: &'static str,
    pub click_through: bool,
    /// Input region (x, y, w, h) in surface coordinates; None = the whole surface.
    pub input: Option<(i32, i32, i32, i32)>,
}

impl Spec {
    pub fn popup_full(ns: &'static str) -> Spec {
        Spec {
            layer: Layer::Overlay,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            margin: (0, 0, 0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::Exclusive,
            namespace: ns,
            click_through: false, input: None,
        }
    }
}

#[derive(Default)]
pub struct Views {
    pub query: String,
    pub preedit: String,
    pub sel: usize,
    pub scroll: f64,
    pub page: u32,
    pub results: Vec<usize>,
    pub calendar_offset: i32,
    pub emoji_cat: usize,
    pub emoji_recent: Vec<String>,
    pub snap_target: String,
    pub dock_pointer: Option<(f64, f64)>,
    pub wifi_password_for: Option<String>,
    pub password: String,
    pub overview_launchpad: bool,
    /// Open macOS app menu: (menu title, x of its title in the bar).
    pub app_menu: Option<(String, f64)>,
    /// Material sidebar: 0 calendar, 1 to-do.
    pub side_tab: u8,
}

impl Views {
    pub fn on_open(&mut self, kind: Kind, data: &mut Data) {
        if kind != Kind::Menu {
            self.app_menu = None;
        }
        if kind != Kind::Overview {
            self.overview_launchpad = false;
        }
        self.query.clear();
        self.preedit.clear();
        self.sel = 0;
        self.scroll = 0.0;
        if kind != Kind::Menu {
            self.page = 0;
        }
        self.wifi_password_for = None;
        self.password.clear();
        if kind == Kind::Calendar {
            self.calendar_offset = 0;
        }
        if matches!(kind, Kind::Launcher | Kind::Overview) {
            self.update_results(data);
        }
    }

    pub fn on_all_closed(&mut self) {
        self.results = vec![];
        self.results.shrink_to_fit();
    }

    pub fn set_page(&mut self, p: u32) {
        if self.overview_launchpad {
            // Launchpad page dots carry the first index of the page.
            self.sel = p as usize;
            return;
        }
        self.page = p;
        self.sel = 0;
        self.scroll = 0.0;
        self.emoji_cat = p as usize;
    }

    /// Escape handling inside a view; returns true if consumed (e.g. clearing a query).
    pub fn escape(&mut self, kind: Kind) -> bool {
        if self.wifi_password_for.is_some() {
            self.wifi_password_for = None;
            self.password.clear();
            return true;
        }
        if matches!(kind, Kind::Control) && self.page != 0 {
            self.page = 0;
            return true;
        }
        if has_text_input(kind) && !self.query.is_empty() {
            self.query.clear();
            self.sel = 0;
            return true;
        }
        false
    }

    pub fn type_text(&mut self, kind: Kind, s: &str) {
        if self.wifi_password_for.is_some() {
            self.password.push_str(s);
            return;
        }
        if has_text_input(kind) {
            self.query.push_str(s);
            self.sel = 0;
            self.scroll = 0.0;
        }
    }

    pub fn emoji_recent(&mut self, e: &str) {
        self.emoji_recent.retain(|x| x != e);
        self.emoji_recent.insert(0, e.to_string());
        self.emoji_recent.truncate(16);
    }

    pub fn update_results(&mut self, data: &Data) {
        self.results = launcher::search(&data.apps, &self.query);
    }

    pub fn launcher_result(&self, data: &Data, i: usize) -> Option<Action> {
        if let Some(a) = launcher::special(&self.query) {
            if i == 0 {
                return Some(a);
            }
        }
        let off = if launcher::special(&self.query).is_some() { 1 } else { 0 };
        let r = launcher::search(&data.apps, &self.query);
        r.get(i.checked_sub(off)?).and_then(|&k| data.apps.get(k)).map(|a: &AppEntry| Action::Launch(a.id.clone()))
    }

    pub fn scroll(&mut self, kind: Kind, d: f64) -> bool {
        match kind {
            Kind::Launcher | Kind::Clipboard | Kind::Emoji | Kind::NotifCenter | Kind::Control | Kind::Overview => {
                self.scroll = (self.scroll - d * 48.0).max(0.0);
                true
            }
            Kind::Calendar => {
                self.calendar_offset += if d > 0.0 { -1 } else { 1 };
                true
            }
            _ => false,
        }
    }

    /// Keyboard handling for a view; returns an action to perform.
    pub fn key(&mut self, kind: Kind, data: &Data, key: Keysym, utf8: Option<&str>, mods: &Modifiers, focused: Option<&Hit>) -> Option<Action> {
        let text_field = has_text_input(kind) || self.wifi_password_for.is_some();
        let count = match kind {
            Kind::Launcher => launcher::search(&data.apps, &self.query).len() + launcher::special(&self.query).is_some() as usize,
            Kind::Clipboard => clipboard::filtered(data, &self.query).len(),
            Kind::Emoji => emoji::filtered(&self.query, self.emoji_cat, &self.emoji_recent).len(),
            Kind::Overview => {
                if self.overview_launchpad {
                    launcher::search(&data.apps, &self.query).len()
                } else {
                    overview::windows(data).len()
                }
            }
            Kind::Power => power::ITEMS.len(),
            _ => 0,
        };
        let cols = match kind {
            Kind::Emoji => emoji::COLS,
            Kind::Overview if self.overview_launchpad => 7,
            Kind::Power => power::ITEMS.len(),
            _ => 1,
        };
        match key {
            Keysym::Down if count > 0 && focused.is_none() => {
                self.sel = (self.sel + cols).min(count - 1);
                return Some(Action::None);
            }
            Keysym::Up if count > 0 && focused.is_none() => {
                self.sel = self.sel.saturating_sub(cols);
                return Some(Action::None);
            }
            Keysym::Right if cols > 1 && count > 0 && (!text_field || self.query.is_empty()) && focused.is_none() => {
                self.sel = (self.sel + 1).min(count - 1);
                return Some(Action::None);
            }
            Keysym::Left if cols > 1 && count > 0 && (!text_field || self.query.is_empty()) && focused.is_none() => {
                self.sel = self.sel.saturating_sub(1);
                return Some(Action::None);
            }
            Keysym::Return | Keysym::KP_Enter if focused.is_none() || focused.map(|h| h.role == ui::Role::Input).unwrap_or(false) => {
                if let Some(ssid) = self.wifi_password_for.clone() {
                    return Some(Action::WifiConnect(ssid));
                }
                return match kind {
                    Kind::Launcher => self.launcher_result(data, self.sel).or(Some(Action::None)),
                    Kind::Clipboard => clipboard::filtered(data, &self.query).get(self.sel).map(|(id, _)| Action::ClipboardPick(id.clone())),
                    Kind::Emoji => emoji::filtered(&self.query, self.emoji_cat, &self.emoji_recent).get(self.sel).map(|e| Action::EmojiPick(e.0.to_string())),
                    Kind::Overview => {
                        if self.overview_launchpad {
                            launcher::search(&data.apps, &self.query).get(self.sel).and_then(|&i| data.apps.get(i)).map(|a| Action::Launch(a.id.clone()))
                        } else {
                            overview::windows(data).get(self.sel).map(|c| Action::FocusWindow(c.address.clone()))
                        }
                    }
                    Kind::Power => power::ITEMS.get(self.sel).map(|i| Action::Power(i.0)),
                    Kind::Control if self.side_tab == 1 && !self.query.trim().is_empty() => Some(Action::TodoAdd),
                    _ => None,
                };
            }
            Keysym::BackSpace if text_field => {
                if self.wifi_password_for.is_some() {
                    self.password.pop();
                } else if mods.ctrl {
                    self.query.clear();
                } else {
                    self.query.pop();
                }
                self.sel = 0;
                self.scroll = 0.0;
                return Some(Action::None);
            }
            _ => {}
        }
        if text_field && !mods.ctrl && !mods.alt && !mods.logo {
            if let Some(t) = utf8 {
                if !t.is_empty() && t.chars().all(|c| !c.is_control()) {
                    self.type_text(kind, t);
                    return Some(Action::None);
                }
            }
        }
        None
    }
}

pub fn has_text_input(kind: Kind) -> bool {
    matches!(kind, Kind::Launcher | Kind::Clipboard | Kind::Emoji | Kind::Overview | Kind::Control)
}

/// Layer specification for a surface kind.
pub fn spec(kind: Kind, th: &gfx::Theme, data: &Data, views: &Views, ow: u32, oh: u32) -> Spec {
    let cfg = &th.cfg;
    match kind {
        Kind::Wallpaper => Spec {
            layer: Layer::Background,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            size: (0, 0),
            margin: (0, 0, 0, 0),
            exclusive: -1,
            keyboard: KeyboardInteractivity::None,
            namespace: "creed-wallpaper",
            click_through: false, input: None,
        },
        Kind::Bar => bar::spec(th, ow, oh),
        Kind::Dock => dock::spec(th, data, views, ow, oh),
        Kind::Notifications => notif::spec(th, data, ow, oh),
        Kind::Osd => osd::spec(th, ow, oh),
        Kind::Menu => Spec { keyboard: KeyboardInteractivity::OnDemand, ..Spec::popup_full("creed-menu") },
        Kind::Launcher => Spec { ..Spec::popup_full("creed-launcher") },
        Kind::Control => Spec { keyboard: KeyboardInteractivity::OnDemand, ..Spec::popup_full("creed-control") },
        Kind::Dashboard => Spec { keyboard: KeyboardInteractivity::OnDemand, ..Spec::popup_full("creed-dashboard") },
        Kind::NotifCenter => Spec { keyboard: KeyboardInteractivity::OnDemand, ..Spec::popup_full("creed-notif-center") },
        Kind::Overview => Spec { ..Spec::popup_full("creed-overview") },
        Kind::Clipboard => Spec { ..Spec::popup_full("creed-clipboard") },
        Kind::Emoji => Spec { ..Spec::popup_full("creed-emoji") },
        Kind::Power => Spec { ..Spec::popup_full("creed-power") },
        Kind::Snap => Spec { ..Spec::popup_full("creed-snap") },
        Kind::Calendar => Spec { keyboard: KeyboardInteractivity::OnDemand, ..Spec::popup_full("creed-calendar") },
    }
    .with_cfg(cfg)
}

impl Spec {
    fn with_cfg(self, _cfg: &creed_core::Config) -> Spec {
        self
    }
}

/// Where a popup panel should sit, given the bar position (popups hang off the bar).
pub fn popup_anchor(ui: &Ui, w: f64, h: f64, align: gfx::Align) -> Rect {
    let cfg = &ui.th.cfg;
    let (sw, sh) = ui.size;
    let gap = 8.0;
    let bar_extent = if cfg.bar.enabled { (cfg.bar.size + cfg.bar.margin) as f64 } else { 0.0 };
    let edge_m = cfg.bar.margin.max(8) as f64;
    match cfg.bar.position {
        Edge::Top | Edge::Bottom => {
            let x = match align {
                gfx::Align::Start => edge_m,
                gfx::Align::Center => (sw - w) / 2.0,
                gfx::Align::End => sw - w - edge_m,
            };
            let y = if cfg.bar.position == Edge::Top { bar_extent + gap } else { sh - bar_extent - gap - h };
            Rect::new(x.round(), y.round(), w, h)
        }
        Edge::Left | Edge::Right => {
            let y = match align {
                gfx::Align::Start => edge_m,
                gfx::Align::Center => (sh - h) / 2.0,
                gfx::Align::End => sh - h - edge_m,
            };
            let x = if cfg.bar.position == Edge::Left { bar_extent + gap } else { sw - bar_extent - gap - w };
            Rect::new(x.round(), y.round(), w, h)
        }
    }
}

/// Open/close animation amount for popups (0 → 1).
pub fn open_amount(ui: &mut Ui, closing: bool) -> f64 {
    ui.spring(ui::id("__open"), if closing { 0.0 } else { 1.0 }, 0.0)
}

/// Applies the standard popup entrance: fade + slight rise/scale around `r`.
/// Note: only whole-pixel translation - scaling the CTM would make cairo create (and keep)
/// a new scaled font for every animation frame, growing memory with each popup.
pub fn popup_transform(ui: &Ui, _r: Rect, t: f64, from_top: bool) {
    let dy = ((1.0 - t) * if from_top { -12.0 } else { 12.0 }).round();
    ui.cr.translate(0.0, dy);
}

pub fn draw(ui: &mut Ui, kind: Kind, closing: bool, data: &Data, views: &mut Views, paths: &Paths, cfg_err: Option<&str>) {
    match kind {
        Kind::Wallpaper => wallpaper::draw(ui, paths),
        Kind::Bar => bar::draw(ui, data, views, cfg_err),
        Kind::Dock => dock::draw(ui, data, views),
        Kind::Launcher => launcher::draw(ui, closing, data, views),
        Kind::Control if th_is(ui, "control", "sidebar") => sidebar(ui, closing, data, views, true),
        Kind::Control => control::draw(ui, closing, data, views),
        Kind::Dashboard if th_is(ui, "dashboard", "sidebar") => sidebar(ui, closing, data, views, false),
        Kind::Dashboard => dashboard::draw(ui, closing, data, views),
        Kind::Notifications => notif::draw_popups(ui, data),
        Kind::NotifCenter if th_is(ui, "notifications", "material") => {
            // Material: notifications live in the right sidebar.
            views.page = 0;
            sidebar(ui, closing, data, views, true)
        }
        Kind::NotifCenter => notif::draw_center(ui, closing, data, views),
        Kind::Overview => overview::draw(ui, closing, data, views),
        Kind::Osd => osd::draw(ui, closing, data),
        Kind::Clipboard => clipboard::draw(ui, closing, data, views),
        Kind::Emoji => emoji::draw(ui, closing, views),
        Kind::Power => power::draw(ui, closing, data, views),
        Kind::Snap => snap::draw(ui, closing, data, views),
        Kind::Menu => menu::draw(ui, closing, data, views),
        Kind::Calendar => calendar::draw_popup(ui, closing, views),
    }
}

/// Empty-state message centred in `r`.
fn th_is(ui: &Ui, which: &str, v: &str) -> bool {
    ui.th.variant(which) == v
}

/// Material sidebars (right: control + notifications + calendar, left: widgets + tools).
fn sidebar(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views, right: bool) {
    let t = open_amount(ui, closing);
    let kind = if right { Kind::Control } else { Kind::Dashboard };
    ui.backdrop(Action::Close(kind));
    ui.cr.save().ok();
    ui.cr.push_group();
    if right {
        material::right_sidebar(ui, t, data, views);
    } else {
        material::left_sidebar(ui, t, data, views);
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

pub fn empty(ui: &mut Ui, r: Rect, icon: &str, msg: &str) {
    let (ic, rest) = Rect::new(r.x, r.cy() - 40.0, r.w, 80.0).take_top(40.0, 6.0);
    ui.icon(ic, icon, 32.0, Some(ui.th.pal.muted));
    ui.text(rest, msg, &Text::new(ui.th.fs, ui.th.pal.muted).center());
}

/// Vertically scrollable list helper: returns visible item indices with their rects and
/// clamps `views.scroll`. Scrolling also keeps the selection visible.
pub fn list_layout(views: &mut Views, area: Rect, count: usize, row_h: f64, gap: f64) -> Vec<(usize, Rect)> {
    let total = count as f64 * (row_h + gap) - gap;
    let max_scroll = (total - area.h).max(0.0);
    // keep selection visible
    let sel_top = views.sel as f64 * (row_h + gap);
    if sel_top < views.scroll {
        views.scroll = sel_top;
    } else if sel_top + row_h > views.scroll + area.h {
        views.scroll = sel_top + row_h - area.h;
    }
    views.scroll = views.scroll.clamp(0.0, max_scroll);
    let mut out = vec![];
    for i in 0..count {
        let y = area.y + i as f64 * (row_h + gap) - views.scroll;
        if y + row_h < area.y || y > area.bottom() {
            continue;
        }
        out.push((i, Rect::new(area.x, y, area.w, row_h)));
    }
    out
}

/// Clip to `r` while drawing a scrolled list.
pub fn with_clip<F: FnOnce(&mut Ui)>(ui: &mut Ui, r: Rect, f: F) {
    ui.cr.save().ok();
    ui.cr.rectangle(r.x, r.y, r.w, r.h);
    ui.cr.clip();
    f(ui);
    ui.cr.restore().ok();
}

/// Hit regions outside a clip rect must not be clickable.
pub fn clip_hits(ui: &mut Ui, from: usize, r: Rect) {
    for h in ui.hits[from..].iter_mut() {
        let x0 = h.rect.x.max(r.x);
        let y0 = h.rect.y.max(r.y);
        let x1 = h.rect.right().min(r.right());
        let y1 = h.rect.bottom().min(r.bottom());
        h.rect = Rect::new(x0, y0, (x1 - x0).max(0.0), (y1 - y0).max(0.0));
    }
    ui.hits.retain(|h| h.rect.w > 0.0 && h.rect.h > 0.0);
}
