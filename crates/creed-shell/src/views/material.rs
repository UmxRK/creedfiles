//! Creed's Material You layout, modelled on end-4's "illogical-impulse" Hyprland setup:
//! a flush top bar (window title · resources · media · workspaces · clock · utilities ·
//! status), full-height left and right sidebars, a top-centre OSD pill. Every colour is a
//! Material 3 role from the wallpaper-generated TonalSpot scheme; shapes are M3 pills and
//! 20–24 px containers; depth comes from surface tone, never glow.

use super::Views;
use crate::data::{Data, NetKind, OsdKind};
use crate::gfx::{self, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Slider, Ui};
use creed_core::palette::M3;
use creed_core::Color;

/// Material roles for the current palette (synthesised from the basic palette when the mode
/// uses a fixed system palette, so the layout still works everywhere).
pub fn roles(th: &gfx::Theme) -> M3 {
    if let Some(m) = th.pal.m3 {
        return m;
    }
    let p = &th.pal;
    M3 {
        primary: p.accent,
        on_primary: p.on_accent,
        primary_container: p.accent.with_alpha(0.3),
        on_primary_container: p.text,
        secondary: p.accent_alt,
        secondary_container: p.overlay,
        on_secondary_container: p.text,
        tertiary: p.accent_alt,
        tertiary_container: p.overlay,
        on_tertiary_container: p.text,
        surface: p.bg,
        surface_container_lowest: p.bg,
        surface_container_low: p.surface,
        surface_container: p.surface,
        surface_container_high: p.surface_alt,
        surface_container_highest: p.overlay,
        on_surface: p.text,
        on_surface_variant: p.subtext,
        outline: p.muted,
        outline_variant: p.border,
        inverse_surface: p.text,
        inverse_on_surface: p.bg,
    }
}

/// M3 state layer: on-colour overlay at 8 % hover / 12 % press.
fn state(ui: &Ui, r: Rect, radius: f64, on: Color, hover: f64, pressed: bool) {
    let a = if pressed { 0.12 } else { 0.08 * hover };
    if a > 0.001 {
        gfx::fill_rounded(ui.cr, r, radius, on.with_alpha(a));
    }
}

/// Round icon button. `filled`: primary container style (toggle on).
fn icon_btn(ui: &mut Ui, key: ui::Id, r: Rect, icon: &str, label: &str, action: Action, filled: Option<bool>) {
    let m = roles(ui.th);
    let hv = ui.hover_amt(key);
    let pressed = ui.is_pressed(key);
    let (bg, fg) = match filled {
        Some(true) => (Some(m.primary), m.on_primary),
        Some(false) => (Some(m.surface_container_high), m.on_surface_variant),
        None => (None, m.on_surface),
    };
    let radius = r.h.min(r.w) / 2.0;
    if let Some(bg) = bg {
        gfx::fill_rounded(ui.cr, r, radius, bg);
    }
    state(ui, r, radius, fg, hv, pressed);
    ui.icon(r, icon, (r.h * 0.5).round().clamp(14.0, 24.0), Some(fg));
    if ui.focus == Some(key) {
        ui.th.focus_ring(ui.cr, r, radius);
    }
    let role = if filled.is_some() { Role::Toggle } else { Role::Button };
    let hit = ui.hit(key, r, action, role, label);
    if let Some(on) = filled {
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
}

fn bar_h(ui: &Ui) -> f64 {
    let b = &ui.th.cfg.bar;
    if b.enabled && b.position == creed_core::config::Edge::Top {
        (b.size + if b.style == creed_core::config::BarStyle::Edge { 0 } else { b.margin }) as f64
    } else {
        0.0
    }
}

// ───────────────────────────── bar ─────────────────────────────

pub fn bar_background(ui: &mut Ui, full: Rect, corners: f64) {
    let m = roles(ui.th);
    let alpha = if ui.th.cfg.glass.enabled { (ui.th.cfg.glass.frost + 0.12).clamp(0.6, 1.0) } else { 1.0 };
    let col = m.surface.with_alpha(alpha);
    gfx::fill_rounded(ui.cr, full, 0.0, col);
    if corners > 0.0 {
        // Concave screen corners continuing the bar, so the desktop reads as a rounded screen.
        let top = ui.th.cfg.bar.position == creed_core::config::Edge::Top;
        let y = if top { full.bottom() } else { full.y - corners };
        let cr = ui.cr;
        let c = corners;
        for left in [true, false] {
            let x = if left { full.x } else { full.right() - c };
            cr.new_path();
            // Corner square minus a quarter circle centred on the inner point.
            let (cx, cy) = (if left { x + c } else { x }, if top { y + c } else { y });
            let (ax, ay) = (if left { x } else { x + c }, if top { y } else { y + c });
            cr.move_to(ax, ay);
            cr.line_to(cx, ay);
            let (a0, a1) = match (left, top) {
                (true, true) => (1.5 * std::f64::consts::PI, std::f64::consts::PI),
                (false, true) => (1.5 * std::f64::consts::PI, 2.0 * std::f64::consts::PI),
                (true, false) => (0.5 * std::f64::consts::PI, std::f64::consts::PI),
                (false, false) => (0.5 * std::f64::consts::PI, 0.0),
            };
            if (left && top) || (!left && !top) {
                cr.arc_negative(cx, cy, c, a0, a1);
            } else {
                cr.arc(cx, cy, c, a0, a1);
            }
            cr.close_path();
            gfx::set(cr, col);
            let _ = cr.fill();
        }
    }
}

/// Groups that sit on a tonal pill (resources, media, workspaces, utilities, status).
fn group_pill(ui: &mut Ui, r: Rect) {
    let m = roles(ui.th);
    let h = (ui.th.cfg.bar.size as f64 * 0.74).round();
    let pr = Rect::centered(r.cx(), r.cy(), r.w - 4.0, h);
    gfx::fill_rounded(ui.cr, pr, h / 2.0, m.surface_container_low);
}

const WS_CELL: f64 = 26.0;

fn resources(data: &Data) -> [(&'static str, f64, &'static str); 3] {
    let ram = if data.sys.mem_total_mb > 0 { data.sys.mem_used_mb as f64 / data.sys.mem_total_mb as f64 } else { 0.0 };
    let disk = if data.sys.disk_total_gb > 0.0 { data.sys.disk_used_gb / data.sys.disk_total_gb } else { 0.0 };
    [("memory-symbolic", ram, "Memory"), ("drive-harddisk-symbolic", disk, "Disk"), ("cpu-symbolic", data.sys.cpu / 100.0, "CPU")]
}

fn pct(v: f64) -> String {
    format!("{:.0}%", (v * 100.0).clamp(0.0, 100.0))
}

fn status_list(data: &Data) -> Vec<(String, String)> {
    let mut v = vec![];
    if let Some(mic) = &data.mic {
        if mic.muted {
            v.push(("microphone-disabled-symbolic".to_string(), "Microphone muted".to_string()));
        }
    }
    let (net, vol, bat, bt) = super::bar::status_icons_pub(data);
    let vol_label = match &data.audio {
        Some(a) => format!("Volume {:.0}%{}", a.volume * 100.0, if a.muted { ", muted" } else { "" }),
        None => "No audio device".into(),
    };
    v.push((vol, vol_label));
    let net_label = match data.net.kind {
        NetKind::Wifi => format!("Wi-Fi: {} ({}%)", data.net.name, data.net.signal),
        NetKind::Ethernet => "Wired network connected".into(),
        NetKind::None => "Offline".into(),
    };
    v.push((net, net_label));
    if let Some(bt) = bt {
        v.push((bt, "Bluetooth".into()));
    }
    if let (Some(b), Some(icon)) = (&data.battery, bat) {
        v.push((icon, format!("Battery {:.0}%{}", b.percent, if b.charging { ", charging" } else { "" })));
    }
    v
}

fn clock_label(ui: &Ui) -> String {
    let now = time::now_secs();
    format!("{}  •  {}", time::format(now, &ui.th.cfg.bar.clock_format), time::format(now, "%A, %d/%m"))
}

pub fn module_len(ui: &mut Ui, name: &str, data: &Data) -> Option<f64> {
    let th = ui.th;
    let fs = th.fs;
    let h = th.cfg.bar.size as f64;
    Some(match name {
        "window" => {
            let (a, b) = window_lines(data);
            let wa = ui.measure(&a, &Text::new(fs * 0.78, th.pal.text)).0;
            let wb = ui.measure(&b, &Text::new(fs * 0.95, th.pal.text)).0;
            (wa.max(wb) + 20.0).clamp(120.0, 340.0)
        }
        "resources" => {
            let tw = ui.measure("100%", &Text::new(fs * 0.9, th.pal.text)).0;
            3.0 * (h * 0.62 + 6.0 + tw + 10.0) + 12.0
        }
        "media" => {
            let s = media_text(data);
            (ui.measure(&s, &Text::new(fs * 0.92, th.pal.text)).0 + h * 0.62 + 32.0).clamp(130.0, 300.0)
        }
        "workspaces" => th.cfg.bar.workspaces.max(1) as f64 * WS_CELL + 16.0,
        "clock" => ui.measure(&clock_label(ui), &Text::new(fs, th.pal.text).medium()).0 + 28.0,
        "utilities" => 3.0 * (h * 0.74) + 12.0,
        "status" => status_list(data).len() as f64 * (h * 0.62) + 20.0,
        "keyboard" => {
            if data.kb_layout.is_empty() {
                0.0
            } else {
                ui.measure("US", &Text::new(fs * 0.9, th.pal.text).medium()).0 + 20.0
            }
        }
        "sidebar-left" | "sidebar-right" => h,
        _ => return None,
    })
}

fn window_lines(data: &Data) -> (String, String) {
    match data.active_client() {
        Some(c) if !c.class.is_empty() => (c.class.clone(), if c.title.is_empty() { Data::pretty_class(&c.class) } else { c.title.clone() }),
        _ => ("Desktop".into(), format!("Workspace {}", data.active_ws.max(1))),
    }
}

fn media_text(data: &Data) -> String {
    match &data.media {
        Some(m) if !m.title.is_empty() => {
            if m.artist.is_empty() {
                m.title.clone()
            } else {
                format!("{} • {}", m.title, m.artist)
            }
        }
        _ => "No media".into(),
    }
}

/// Small progress ring with an icon inside (bar resources / media).
fn ring(ui: &mut Ui, c: (f64, f64), rad: f64, frac: f64, icon: &str) {
    let m = roles(ui.th);
    let cr = ui.cr;
    cr.set_line_width(2.4);
    cr.new_path();
    cr.arc(c.0, c.1, rad, 0.0, std::f64::consts::TAU);
    gfx::set(cr, m.secondary_container);
    let _ = cr.stroke();
    if frac > 0.003 {
        cr.new_path();
        let a0 = -std::f64::consts::FRAC_PI_2;
        cr.arc(c.0, c.1, rad, a0, a0 + std::f64::consts::TAU * frac.clamp(0.0, 1.0));
        cr.set_line_cap(cairo::LineCap::Round);
        gfx::set(cr, m.primary);
        let _ = cr.stroke();
    }
    ui.icon(Rect::centered(c.0, c.1, rad * 2.0, rad * 2.0), icon, (rad * 1.15).round(), Some(m.on_surface));
}

pub fn draw_module(ui: &mut Ui, name: &str, r: Rect, data: &Data) -> bool {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let h = th.cfg.bar.size as f64;
    let key = ui::id(&format!("bar.{name}"));
    match name {
        "window" => {
            let (a, b) = window_lines(data);
            let hv = ui.hover_amt(key);
            let inner = r.inset_xy(10.0, 0.0);
            state(ui, Rect::centered(r.cx(), r.cy(), r.w, h * 0.86), 12.0, m.on_surface, hv, false);
            let lh = fs * 1.2;
            ui.text(Rect::new(inner.x, r.cy() - lh + 1.0, inner.w, lh), &a, &Text::new(fs * 0.78, m.on_surface_variant));
            ui.text(Rect::new(inner.x, r.cy() - 1.0, inner.w, lh + 2.0), &b, &Text::new(fs * 0.95, m.on_surface));
            ui.hit(key, r, Action::Toggle(Kind::Dashboard), Role::Button, format!("{b} - open left sidebar"));
        }
        "resources" => {
            group_pill(ui, r);
            let tw = ui.measure("100%", &Text::new(fs * 0.9, m.on_surface)).0;
            let rad = (h * 0.62 - 6.0) / 2.0;
            let mut x = r.x + 12.0;
            let mut label = vec![];
            for (icon, v, what) in resources(data) {
                ring(ui, (x + rad + 3.0, r.cy()), rad, v, icon);
                let tx = x + rad * 2.0 + 10.0;
                ui.text(Rect::new(tx, r.y, tw + 4.0, r.h), &pct(v), &Text::new(fs * 0.9, m.on_surface));
                x = tx + tw + 10.0;
                label.push(format!("{what} {}", pct(v)));
            }
            ui.hit(key, r, Action::Exec("btop-launch".into()), Role::Label, label.join(", "));
            ui.hits.last_mut().map(|h| h.action = Action::Toggle(Kind::Dashboard));
        }
        "media" => {
            group_pill(ui, r);
            let hv = ui.hover_amt(key);
            let pr = Rect::centered(r.cx(), r.cy(), r.w - 4.0, (h * 0.74).round());
            state(ui, pr, pr.h / 2.0, m.on_surface, hv, ui.is_pressed(key));
            let rad = (h * 0.62 - 6.0) / 2.0;
            let playing = data.media.as_ref().map(|x| x.playing).unwrap_or(false);
            ring(ui, (r.x + 12.0 + rad + 3.0, r.cy()), rad, if data.media.is_some() { 1.0 } else { 0.0 }, if playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" });
            let tx = r.x + 12.0 + rad * 2.0 + 12.0;
            ui.text(Rect::new(tx, r.y, r.right() - tx - 12.0, r.h), &media_text(data), &Text::new(fs * 0.92, m.on_surface).center());
            let hit = ui.hit(key, r, if data.media.is_some() { Action::MediaPlayPause } else { Action::None }, Role::Button, format!("Media: {}", media_text(data)));
            hit.secondary = Some(Action::MediaNext);
        }
        "workspaces" => {
            group_pill(ui, r);
            let n = th.cfg.bar.workspaces.max(1) as i64;
            let active_ws = data.active_ws.clamp(1, n);
            let x0 = r.x + 8.0;
            let occupied: Vec<i64> = data.workspaces.iter().filter(|w| w.windows > 0).map(|w| w.id).collect();
            // Occupied runs share one tonal pill, like the reference bar.
            let mut i = 1;
            while i <= n {
                if occupied.contains(&i) && i != active_ws {
                    let start = i;
                    while i < n && occupied.contains(&(i + 1)) && i + 1 != active_ws {
                        i += 1;
                    }
                    let a = x0 + (start - 1) as f64 * WS_CELL;
                    let b = x0 + i as f64 * WS_CELL;
                    let pr = Rect::new(a + 2.0, r.cy() - WS_CELL / 2.0 + 2.0, b - a - 4.0, WS_CELL - 4.0);
                    gfx::fill_rounded(ui.cr, pr, pr.h / 2.0, m.secondary_container.with_alpha(0.7));
                }
                i += 1;
            }
            // Active indicator slides between workspaces.
            let act = ui.spring(ui::id("ws.active.x"), (active_ws - 1) as f64, (active_ws - 1) as f64);
            let ar = Rect::new(x0 + act * WS_CELL + 1.0, r.cy() - WS_CELL / 2.0 + 1.0, WS_CELL - 2.0, WS_CELL - 2.0);
            gfx::fill_rounded(ui.cr, ar, ar.h / 2.0, m.primary);
            for ws in 1..=n {
                let cell = Rect::new(x0 + (ws - 1) as f64 * WS_CELL, r.cy() - WS_CELL / 2.0, WS_CELL, WS_CELL);
                let k = ui::idn("bar.ws", ws);
                let hv = ui.hover_amt(k);
                state(ui, cell.inset(1.0), WS_CELL / 2.0, m.on_surface, hv, false);
                let active = ws == active_ws;
                let col = if active {
                    m.on_primary
                } else if occupied.contains(&ws) {
                    m.on_secondary_container
                } else {
                    m.on_surface_variant.with_alpha(0.55)
                };
                ui.text(cell, &ws.to_string(), &Text::new(fs * 0.85, col).medium().center());
                ui.hit(k, cell, Action::Workspace(ws), Role::Button, format!("Workspace {ws}{}", if active { " (current)" } else { "" }));
            }
            let hit = ui.hit(key, r, Action::None, Role::Group, "Workspaces");
            hit.scroll = Some(|d| Action::WorkspaceScroll(if d > 0.0 { -1 } else { 1 }));
            // Group must not steal clicks from the buttons above.
            let n_hits = ui.hits.len();
            let g = ui.hits.remove(n_hits - 1);
            ui.hits.insert(n_hits - 1 - n as usize, g);
        }
        "clock" => {
            let hv = ui.hover_amt(key);
            let pr = Rect::centered(r.cx(), r.cy(), r.w - 4.0, (h * 0.74).round());
            state(ui, pr, pr.h / 2.0, m.on_surface, hv, ui.is_pressed(key));
            ui.text(r, &clock_label(ui), &Text::new(fs, m.on_surface).medium().center());
            let now = time::now_secs();
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, format!("Clock, {} - open right sidebar", time::format(now, "%A %d %B %H:%M")));
        }
        "utilities" => {
            group_pill(ui, r);
            let s = (h * 0.74).round();
            let items: [(&str, &str, Action); 3] = [
                ("screenshot-symbolic", "Screenshot region", Action::Exec("creed screenshot region".into())),
                ("color-select-symbolic", "Colour picker", Action::Exec("hyprpicker -a".into())),
                ("edit-paste-symbolic", "Clipboard history", Action::Toggle(Kind::Clipboard)),
            ];
            for (i, (icon, label, action)) in items.into_iter().enumerate() {
                let c = Rect::new(r.x + 6.0 + i as f64 * s, r.cy() - s / 2.0, s, s).inset(2.0);
                icon_btn(ui, ui::idn("bar.util", i), c, icon, label, action, None);
            }
        }
        "status" => {
            group_pill(ui, r);
            let hv = ui.hover_amt(key);
            let pr = Rect::centered(r.cx(), r.cy(), r.w - 4.0, (h * 0.74).round());
            state(ui, pr, pr.h / 2.0, m.on_surface, hv, ui.is_pressed(key));
            let list = status_list(data);
            let cell = h * 0.62;
            for (i, (icon, _)) in list.iter().enumerate() {
                let c = Rect::new(r.x + 10.0 + i as f64 * cell, r.y, cell, r.h);
                ui.icon(c, icon, (fs * 1.25).round(), Some(m.on_surface));
            }
            let label = list.iter().map(|(_, l)| l.clone()).collect::<Vec<_>>().join(", ");
            let hit = ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, label);
            hit.scroll = Some(|d| Action::VolumeStep(d * 0.05));
        }
        "keyboard" => {
            let short: String = data.kb_layout.chars().take(2).collect::<String>().to_uppercase();
            ui.text(r, &short, &Text::new(fs * 0.9, m.on_surface).medium().center());
            ui.hit(key, r, Action::None, Role::Label, format!("Keyboard layout {}", data.kb_layout));
        }
        _ => return false,
    }
    true
}

// ───────────────────────────── sidebars ─────────────────────────────

pub const SIDEBAR_W: f64 = 420.0;
const GAP: f64 = 8.0;
const R_PANEL: f64 = 24.0;

fn sidebar_rect(ui: &Ui, right: bool) -> Rect {
    let (sw, sh) = ui.size;
    let top = bar_h(ui) + GAP;
    let w = SIDEBAR_W.min(sw - 2.0 * GAP);
    let x = if right { sw - w - GAP } else { GAP };
    Rect::new(x, top, w, sh - top - GAP)
}

fn panel(ui: &mut Ui, r: Rect, name: &str) {
    let m = roles(ui.th);
    let alpha = if ui.th.cfg.glass.enabled { (ui.th.cfg.glass.frost + 0.14).clamp(0.72, 1.0) } else { 1.0 };
    gfx::fill_rounded(ui.cr, r, R_PANEL, m.surface_container_low.with_alpha(alpha));
    gfx::stroke_rounded(ui.cr, r, R_PANEL, 1.0, m.outline_variant.with_alpha(0.35));
    ui.record(name, r, None);
    ui.hit(ui::id(name), r, Action::None, Role::Group, "");
}

/// Slides a sidebar in from its edge.
fn slide(ui: &Ui, t: f64, right: bool) {
    let d = (1.0 - t.clamp(0.0, 1.0)) * 48.0;
    ui.cr.translate((if right { d } else { -d }).round(), 0.0);
}

pub fn right_sidebar(ui: &mut Ui, t: f64, data: &Data, views: &mut Views) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let p = sidebar_rect(ui, true);
    slide(ui, t, true);
    panel(ui, p, "sidebar.right");
    let inner = p.inset(14.0);

    // Header: logo + uptime, then reload / settings / power.
    let (hdr, rest) = inner.take_top(40.0, 12.0);
    let (logo, hrest) = hdr.take_left(32.0, 10.0);
    let logo_icon = distro_logo(ui);
    ui.icon(logo, &logo_icon, 26.0, Some(m.on_surface));
    let (btns, label) = hrest.take_right(3.0 * 40.0, 8.0);
    let _ = &data.uptime;
    ui.text(label, &format!("Uptime: {}", uptime_short()), &Text::new(fs * 1.15, m.on_surface).medium());
    let acts: [(&str, &str, Action); 3] = [
        ("view-refresh-symbolic", "Reload Creed", Action::Exec("creed theme apply".into())),
        ("settings-symbolic", "Settings", Action::Exec("creed-settings".into())),
        ("power-symbolic", "Power menu", Action::Open(Kind::Power)),
    ];
    for (i, (icon, l, a)) in acts.into_iter().enumerate() {
        let c = Rect::new(btns.x + i as f64 * 40.0, btns.y, 40.0, 40.0).inset(2.0);
        icon_btn(ui, ui::idn("rs.hdr", i), c, icon, l, a, None);
    }

    // Quick toggles: round buttons, filled when on.
    let (row, rest) = rest.take_top(48.0, 14.0);
    let wifi_on = data.net.wifi_enabled;
    let bt_on = data.bt.as_ref().map(|b| b.powered).unwrap_or(false);
    let toggles: Vec<(&str, &str, Action, bool)> = vec![
        (if wifi_on { "network-wireless-symbolic" } else { "network-wireless-disabled-symbolic" }, "Wi-Fi", Action::ToggleWifi, wifi_on),
        (if bt_on { "bluetooth-active-symbolic" } else { "bluetooth-disabled-symbolic" }, "Bluetooth", Action::ToggleBluetooth, bt_on),
        ("night-light-symbolic", "Night Light", Action::ToggleNightLight, data.night_light),
        ("input-gaming-symbolic", "Game Mode", Action::ToggleGame, data.game),
        (if data.dnd { "notifications-disabled-symbolic" } else { "notifications-symbolic" }, "Do Not Disturb", Action::ToggleDnd, data.dnd),
        ("power-profile-power-saver-symbolic", "Power saver", Action::PowerProfile(if data.power_profile.as_deref() == Some("power-saver") { "balanced".into() } else { "power-saver".into() }), data.power_profile.as_deref() == Some("power-saver")),
    ];
    let n = toggles.len() as f64;
    let s = 48.0;
    let gap = ((row.w - n * s) / (n - 1.0)).clamp(6.0, 24.0);
    let x0 = row.x + (row.w - (n * s + (n - 1.0) * gap)) / 2.0;
    for (i, (icon, l, a, on)) in toggles.into_iter().enumerate() {
        let c = Rect::new(x0 + i as f64 * (s + gap), row.y, s, s);
        icon_btn(ui, ui::idn("rs.tog", i), c, icon, l, a, Some(on));
    }

    // Calendar block at the bottom.
    let (cal, mid) = rest.take_bottom(318.0, 12.0);
    calendar_block(ui, cal, views);

    // Tabs, then the active page.
    let (tabs, body) = mid.take_top(40.0, 10.0);
    let tab_list: [(&str, &str); 4] = [
        ("notifications-symbolic", "Notifications"),
        ("audio-card-symbolic", "Audio"),
        ("bluetooth-active-symbolic", "Bluetooth"),
        ("network-wireless-symbolic", "Wi-Fi"),
    ];
    let page = (views.page as usize).min(tab_list.len() - 1);
    let active_w = 40.0 + ui.measure(tab_list[page].1, &Text::new(fs * 0.95, m.on_surface).medium()).0 + 22.0;
    let total = active_w + 44.0 * (tab_list.len() as f64 - 1.0);
    let mut x = tabs.x + (tabs.w - total) / 2.0;
    for (i, (icon, label)) in tab_list.iter().enumerate() {
        let on = i == page;
        let w = if on { active_w } else { 44.0 };
        let c = Rect::new(x, tabs.y + 2.0, w - 4.0, tabs.h - 4.0);
        let k = ui::idn("rs.tab", i);
        let hv = ui.hover_amt(k);
        if on {
            gfx::fill_rounded(ui.cr, c, c.h / 2.0, m.secondary_container);
        }
        state(ui, c, c.h / 2.0, m.on_surface, hv, ui.is_pressed(k));
        let fg = if on { m.on_secondary_container } else { m.on_surface_variant };
        if on {
            let (ic, tr) = c.inset_xy(12.0, 0.0).take_left(20.0, 8.0);
            ui.icon(ic, icon, 18.0, Some(fg));
            ui.text(tr, label, &Text::new(fs * 0.95, fg).medium());
        } else {
            ui.icon(c, icon, 18.0, Some(fg));
        }
        let hit = ui.hit(k, c, Action::Page(i as u32), Role::Toggle, *label);
        hit.value = Some(if on { 1.0 } else { 0.0 });
        x += w;
    }
    ui.record("sidebar.right.body", body, Some("sidebar.right"));
    match page {
        1 => audio_page(ui, body, data),
        2 => super::control::bt_page_pub(ui, body, data),
        3 => super::control::wifi_page_pub(ui, body, data, views),
        _ => notifications_page(ui, body, data, views),
    }
}

/// "2m", "13h, 44m", "3d, 2h" - the compact form used by the reference sidebar.
fn uptime_short() -> String {
    let secs = std::fs::read_to_string("/proc/uptime").ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    match (d, h) {
        (0, 0) => format!("{m}m"),
        (0, _) => format!("{h}h, {m}m"),
        _ => format!("{d}d, {h}h"),
    }
}

fn distro_logo(ui: &mut Ui) -> String {
    let logo = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| s.lines().find_map(|l| l.strip_prefix("LOGO=").map(|v| v.trim_matches('"').to_string())))
        .unwrap_or_else(|| "archlinux-logo".into());
    for cand in [format!("{logo}-symbolic"), logo.clone(), "distributor-logo-archlinux".into(), "start-here-symbolic".into()] {
        if ui.icons.exists(&cand) {
            return cand;
        }
    }
    "start-here-symbolic".into()
}

fn notifications_page(ui: &mut Ui, body: Rect, data: &Data, views: &mut Views) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let (list, footer) = {
        let (f, l) = body.take_bottom(36.0, 8.0);
        (l, f)
    };
    // Footer: count · Silence · Clear
    let count = data.history.len();
    let (left, right) = footer.take_left(footer.w * 0.45, 0.0);
    ui.text(left.inset_xy(6.0, 0.0), &format!("{count} notification{}", if count == 1 { "" } else { "s" }), &Text::new(fs * 0.95, m.on_surface_variant));
    let buttons: [(&str, &str, Action, bool); 2] = [
        (if data.dnd { "notifications-disabled-symbolic" } else { "notifications-symbolic" }, "Silence", Action::ToggleDnd, data.dnd),
        ("edit-clear-all-symbolic", "Clear", Action::NotifClear, false),
    ];
    let mut x = right.right();
    for (i, (icon, label, a, on)) in buttons.into_iter().enumerate().rev() {
        let w = ui.measure(label, &Text::new(fs * 0.92, m.on_surface).medium()).0 + 58.0;
        x -= w;
        let c = Rect::new(x, right.y + 2.0, w - 4.0, right.h - 4.0);
        let k = ui::idn("rs.nbtn", i);
        let hv = ui.hover_amt(k);
        if on {
            gfx::fill_rounded(ui.cr, c, c.h / 2.0, m.secondary_container);
        }
        state(ui, c, c.h / 2.0, m.on_surface, hv, ui.is_pressed(k));
        let fg = if on { m.on_secondary_container } else { m.on_surface_variant };
        let (ic, tr) = c.inset_xy(12.0, 0.0).take_left(18.0, 6.0);
        ui.icon(ic, icon, 16.0, Some(fg));
        ui.text(tr, label, &Text::new(fs * 0.92, fg).medium());
        let hit = ui.hit(k, c, a, if i == 0 { Role::Toggle } else { Role::Button }, label);
        if i == 0 {
            hit.value = Some(if on { 1.0 } else { 0.0 });
        }
    }
    if data.history.is_empty() {
        let c = Rect::centered(list.cx(), list.cy() - 10.0, 64.0, 64.0);
        ui.icon(c, "notifications-symbolic", 44.0, Some(m.outline));
        ui.text(Rect::new(list.x, c.bottom() + 4.0, list.w, 24.0), "No notifications", &Text::new(fs * 1.0, m.outline).center());
        return;
    }
    // Group by app (newest first), like the reference.
    let mut groups: Vec<(String, Vec<&crate::data::Notif>)> = vec![];
    for n in data.history.iter().rev() {
        match groups.iter_mut().find(|(a, _)| *a == n.app) {
            Some((_, v)) => v.push(n),
            None => groups.push((n.app.clone(), vec![n])),
        }
    }
    let start = ui.hits.len();
    super::with_clip(ui, list, |ui| {
        let mut y = list.y - views.scroll;
        for (app, items) in &groups {
            let first = items[0];
            let lines = items.len().min(2);
            let h = 56.0 + lines as f64 * 0.0 + if lines > 1 { 20.0 } else { 0.0 };
            let card = Rect::new(list.x, y, list.w, h);
            y += h + 6.0;
            if card.bottom() < list.y || card.y > list.bottom() {
                continue;
            }
            let k = ui::idn("rs.notif", first.id);
            let hv = ui.hover_amt(k);
            gfx::fill_rounded(ui.cr, card, 18.0, m.surface_container);
            state(ui, card, 18.0, m.on_surface, hv, false);
            let ic = Rect::new(card.x + 10.0, card.y + 10.0, 36.0, 36.0);
            gfx::fill_rounded(ui.cr, ic, 18.0, m.primary_container);
            let icon = if first.icon.is_empty() { "preferences-system-notifications-symbolic".to_string() } else { first.icon.clone() };
            ui.icon(ic, &icon, 20.0, if icon.ends_with("-symbolic") { Some(m.on_primary_container) } else { None });
            let tx = ic.right() + 12.0;
            let tw = card.right() - tx - 12.0;
            let (meta_r, _) = Rect::new(tx, card.y + 8.0, tw, 16.0).take_right(84.0, 0.0);
            ui.text(Rect::new(tx, card.y + 8.0, tw - 84.0, 16.0), app, &Text::new(fs * 0.78, m.on_surface_variant));
            let meta = if items.len() > 1 { format!("{}   {}", super::notif::ago(first.arrived), items.len()) } else { super::notif::ago(first.arrived) };
            ui.text(meta_r, &meta, &Text::new(fs * 0.78, m.on_surface_variant).end());
            for (li, n) in items.iter().take(2).enumerate() {
                let ly = card.y + 25.0 + li as f64 * 20.0;
                let s = super::notif::strip_markup(&n.summary);
                let (sw, _) = ui.measure(&s, &Text::new(fs * 0.95, m.on_surface).medium());
                let sw = sw.min(tw * 0.55);
                ui.text(Rect::new(tx, ly, sw, 20.0), &s, &Text::new(fs * 0.95, m.on_surface).medium());
                let body = super::notif::strip_markup(&n.body).replace('\n', " ");
                ui.text(Rect::new(tx + sw + 6.0, ly, tw - sw - 6.0, 20.0), &body, &Text::new(fs * 0.92, m.on_surface_variant));
            }
            ui.hit(k, card, Action::NotifDismiss(first.id), Role::Item, format!("{app}: {} - click to dismiss", first.summary));
        }
    });
    super::clip_hits(ui, start, list);
}

fn audio_page(ui: &mut Ui, body: Rect, data: &Data) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let mut y = body.y + 6.0;
    let row = |y: f64| Rect::new(body.x + 6.0, y, body.w - 12.0, 40.0);
    let label = |ui: &mut Ui, y: f64, s: &str| {
        ui.text(Rect::new(body.x + 8.0, y, body.w, 20.0), s, &Text::new(fs * 0.85, m.on_surface_variant).medium());
    };
    if let Some(a) = &data.audio {
        label(ui, y, "Output");
        y += 22.0;
        let v = if a.muted { 0.0 } else { a.volume.min(1.0) };
        ui.slider(ui::id("rs.vol"), row(y), if a.muted { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" }, &format!("Volume {:.0}%", v * 100.0), v, Slider::Volume, Action::ToggleMute);
        y += 54.0;
    }
    if let Some(a) = &data.mic {
        label(ui, y, "Input");
        y += 22.0;
        let v = if a.muted { 0.0 } else { a.volume.min(1.0) };
        ui.slider(ui::id("rs.mic"), row(y), if a.muted { "microphone-disabled-symbolic" } else { "audio-input-microphone-symbolic" }, &format!("Microphone {:.0}%", v * 100.0), v, Slider::Mic, Action::ToggleMicMute);
        y += 54.0;
    }
    if let Some(b) = data.brightness {
        label(ui, y, "Display");
        y += 22.0;
        ui.slider(ui::id("rs.bri"), row(y), "display-brightness-symbolic", &format!("Brightness {:.0}%", b * 100.0), b, Slider::Brightness, Action::None);
        y += 54.0;
    }
    if data.audio.is_none() && data.mic.is_none() && data.brightness.is_none() {
        super::empty(ui, body, "audio-volume-muted-symbolic", "No audio devices");
        return;
    }
    if let Some(cur) = &data.power_profile {
        label(ui, y, "Power mode");
        y += 24.0;
        let opts = [("power-saver", "Saver"), ("balanced", "Balanced"), ("performance", "Performance")];
        let cols = Rect::new(body.x + 6.0, y, body.w - 12.0, 38.0).columns(3, 6.0);
        for (i, ((id, l), c)) in opts.iter().zip(cols).enumerate() {
            let on = cur == id;
            let k = ui::idn("rs.pp", i);
            let hv = ui.hover_amt(k);
            gfx::fill_rounded(ui.cr, c, c.h / 2.0, if on { m.secondary_container } else { m.surface_container_high });
            state(ui, c, c.h / 2.0, m.on_surface, hv, ui.is_pressed(k));
            let fg = if on { m.on_secondary_container } else { m.on_surface_variant };
            let label = if on { format!("✓ {l}") } else { l.to_string() };
            ui.text(c, &label, &Text::new(fs * 0.92, fg).medium().center());
            let hit = ui.hit(k, c, Action::PowerProfile(id.to_string()), Role::Toggle, format!("Power mode {l}"));
            hit.value = Some(if on { 1.0 } else { 0.0 });
        }
    }
}

/// Calendar with the Calendar / To Do rail (bottom of the right sidebar).
fn calendar_block(ui: &mut Ui, r: Rect, views: &mut Views) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    gfx::fill_rounded(ui.cr, r, 20.0, m.surface_container);
    ui.record("sidebar.right.calendar", r, Some("sidebar.right"));
    let (rail, main) = r.inset(10.0).take_left(70.0, 6.0);
    let items = [("calendar-symbolic", "Calendar"), ("task-due-symbolic", "To Do")];
    for (i, (icon, label)) in items.iter().enumerate() {
        let on = views.side_tab as usize == i;
        let c = Rect::new(rail.x, rail.y + 40.0 + i as f64 * 64.0, rail.w, 58.0);
        let k = ui::idn("rs.rail", i);
        let hv = ui.hover_amt(k);
        let pill = Rect::centered(c.cx(), c.y + 16.0, 52.0, 30.0);
        if on {
            gfx::fill_rounded(ui.cr, pill, 15.0, m.secondary_container);
        }
        state(ui, pill, 15.0, m.on_surface, hv, ui.is_pressed(k));
        ui.icon(pill, icon, 18.0, Some(if on { m.on_secondary_container } else { m.on_surface_variant }));
        ui.text(Rect::new(c.x, c.y + 33.0, c.w, 20.0), label, &Text::new(fs * 0.82, m.on_surface).medium().center());
        let hit = ui.hit(k, c, Action::SideTab(i as u8), Role::Toggle, *label);
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
    if views.side_tab == 1 {
        todo_list(ui, main, views);
    } else {
        month(ui, main, views.calendar_offset);
    }
}

/// Material month grid: today is a filled primary circle, other months' days are dimmed.
pub fn month(ui: &mut Ui, r: Rect, offset: i32) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let now = time::local(time::now_secs());
    let (year, month) = time::add_months(now.year, now.month, offset);
    let (hdr, rest) = r.take_top(36.0, 4.0);
    let (nav, title) = hdr.take_right(76.0, 4.0);
    let k = ui::id("cal.title");
    let hv = ui.hover_amt(k);
    state(ui, title, 12.0, m.on_surface, hv * if offset != 0 { 1.0 } else { 0.0 }, false);
    ui.text(title.inset_xy(10.0, 0.0), &format!("{} {}", time::MONTHS[(month - 1) as usize], year), &Text::new(fs * 1.2, m.on_surface).medium());
    ui.hit(k, title, Action::Calendar(0), Role::Button, "Back to today");
    let (prev, next) = nav.take_left(36.0, 4.0);
    icon_btn(ui, ui::id("cal.prev"), Rect::centered(prev.cx(), prev.cy(), 32.0, 32.0), "go-previous-symbolic", "Previous month", Action::Calendar(-1), None);
    icon_btn(ui, ui::id("cal.next"), Rect::centered(next.cx(), next.cy(), 32.0, 32.0), "go-next-symbolic", "Next month", Action::Calendar(1), None);
    let (dow, grid) = rest.take_top(24.0, 2.0);
    let names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cw = dow.w / 7.0;
    for (i, n) in names.iter().enumerate() {
        ui.text(Rect::new(dow.x + i as f64 * cw, dow.y, cw, dow.h), n, &Text::new(fs * 0.85, m.on_surface).medium().center());
    }
    let first = ((time::weekday(year, month, 1) + 6) % 7) as i64; // 0 = Monday
    let days = time::days_in_month(year, month) as i64;
    let (py, pm) = time::add_months(year, month, -1);
    let prev_days = time::days_in_month(py, pm) as i64;
    let ch = (grid.h / 6.0).min(cw);
    for cell_i in 0..42i64 {
        let (cx, cy) = (cell_i % 7, cell_i / 7);
        let cell = Rect::new(grid.x + cx as f64 * cw, grid.y + cy as f64 * ch, cw, ch);
        let d = cell_i - first + 1;
        let (label, this_month) = if d < 1 {
            (prev_days + d, false)
        } else if d > days {
            (d - days, false)
        } else {
            (d, true)
        };
        let today = this_month && offset == 0 && d == now.day as i64;
        if today {
            let s = ch.min(cw) - 4.0;
            gfx::fill_rounded(ui.cr, Rect::centered(cell.cx(), cell.cy(), s, s), s / 2.0, m.primary);
        }
        let col = if today {
            m.on_primary
        } else if this_month {
            m.on_surface
        } else {
            m.outline.with_alpha(0.6)
        };
        let t = Text::new(fs * 0.92, col).center();
        ui.text(cell, &label.to_string(), &if today { t.medium() } else { t });
    }
}

fn todo_list(ui: &mut Ui, r: Rect, views: &mut Views) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let (input, list) = r.take_top(40.0, 8.0);
    ui.input(ui::id("todo.input"), input, &views.query, "Add a task, then Enter", true, &views.preedit);
    let todos = super::todo::load();
    if todos.is_empty() {
        ui.text(list, "Nothing to do", &Text::new(fs, m.outline).center());
        return;
    }
    let start = ui.hits.len();
    super::with_clip(ui, list, |ui| {
        for (i, (text, done)) in todos.iter().enumerate() {
            let row = Rect::new(list.x, list.y + i as f64 * 34.0 - views.scroll, list.w, 32.0);
            if row.bottom() < list.y || row.y > list.bottom() {
                continue;
            }
            let k = ui::idn("todo", i);
            let hv = ui.hover_amt(k);
            state(ui, row, 12.0, m.on_surface, hv, false);
            let (cb, rest) = row.inset_xy(6.0, 0.0).take_left(24.0, 8.0);
            ui.icon(cb, if *done { "checkbox-checked-symbolic" } else { "checkbox-symbolic" }, 20.0, Some(if *done { m.primary } else { m.on_surface_variant }));
            let (del, txt) = rest.take_right(28.0, 4.0);
            ui.text(txt, text, &Text::new(fs * 0.95, if *done { m.outline } else { m.on_surface }));
            let hit = ui.hit(k, Rect::new(row.x, row.y, row.w - 34.0, row.h), Action::TodoToggle(i), Role::Toggle, text.clone());
            hit.value = Some(if *done { 1.0 } else { 0.0 });
            icon_btn(ui, ui::idn("todo.del", i), Rect::centered(del.cx(), del.cy(), 28.0, 28.0), "edit-delete-symbolic", &format!("Delete {text}"), Action::TodoDelete(i), None);
        }
    });
    super::clip_hits(ui, start, list);
}

pub fn left_sidebar(ui: &mut Ui, t: f64, data: &Data, views: &mut Views) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let p = sidebar_rect(ui, false);
    slide(ui, t, false);
    panel(ui, p, "sidebar.left");
    let inner = p.inset(14.0);
    // Underline tabs.
    let (tabs, body) = inner.take_top(44.0, 14.0);
    let names: [(&str, &str); 2] = [("widgets-symbolic", "Widgets"), ("build-symbolic", "Tools")];
    let page = (views.page as usize).min(names.len() - 1);
    let cols = tabs.columns(names.len(), 0.0);
    for (i, ((icon, label), c)) in names.iter().zip(cols.iter()).enumerate() {
        let on = i == page;
        let k = ui::idn("ls.tab", i);
        let hv = ui.hover_amt(k);
        state(ui, *c, 12.0, m.on_surface, hv, ui.is_pressed(k));
        let fg = if on { m.primary } else { m.on_surface_variant };
        let tw = ui.measure(label, &Text::new(fs * 0.98, fg).medium()).0;
        let wtot = 20.0 + 8.0 + tw;
        let ic = Rect::new(c.cx() - wtot / 2.0, c.y, 20.0, c.h - 4.0);
        ui.icon(ic, icon, 18.0, Some(fg));
        ui.text(Rect::new(ic.right() + 8.0, c.y, tw + 2.0, c.h - 4.0), label, &Text::new(fs * 0.98, fg).medium());
        if on {
            gfx::fill_rounded(ui.cr, Rect::new(c.cx() - wtot / 2.0 - 4.0, c.bottom() - 3.0, wtot + 8.0, 3.0), 1.5, m.primary);
        }
        let hit = ui.hit(k, *c, Action::Page(i as u32), Role::Toggle, *label);
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
    gfx::fill_rounded(ui.cr, Rect::new(tabs.x, tabs.bottom(), tabs.w, 1.0), 0.0, m.outline_variant.with_alpha(0.6));
    ui.record("sidebar.left.body", body, Some("sidebar.left"));
    if page == 1 {
        tools_page(ui, body);
    } else {
        widgets_page(ui, body, data);
    }
}

fn widgets_page(ui: &mut Ui, body: Rect, data: &Data) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let now = time::local(time::now_secs());
    // Greeting + big clock.
    let (top, rest) = body.take_top(112.0, 12.0);
    gfx::fill_rounded(ui.cr, top, 20.0, m.surface_container);
    let greet = match now.hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        _ => "Good evening",
    };
    let it = top.inset_xy(18.0, 14.0);
    ui.text(Rect::new(it.x, it.y, it.w, 20.0), &format!("{greet}, {}", data.user), &Text::new(fs * 0.95, m.on_surface_variant));
    ui.text(Rect::new(it.x, it.y + 20.0, it.w, 48.0), &time::format(time::now_secs(), &th.cfg.bar.clock_format), &Text::new(fs * 3.2, m.on_surface).medium());
    ui.text(Rect::new(it.x, it.y + 66.0, it.w, 20.0), &time::format(time::now_secs(), "%A, %-d %B"), &Text::new(fs * 0.95, m.primary).medium());

    // Media card with the wavy progress line.
    let (media, rest) = rest.take_top(132.0, 12.0);
    media_card(ui, media, data);

    // Weather + resources side by side.
    let (row, rest) = rest.take_top(150.0, 12.0);
    quick_actions(ui, rest, data);
    let (wcard, scard) = row.take_left((row.w - 12.0) / 2.0, 12.0);
    gfx::fill_rounded(ui.cr, wcard, 20.0, m.surface_container);
    let wi = wcard.inset(16.0);
    match &data.weather {
        Some(w) => {
            ui.icon(Rect::new(wi.x, wi.y, 40.0, 40.0), &w.icon, 36.0, Some(m.primary));
            ui.text(Rect::new(wi.x + 48.0, wi.y, wi.w - 48.0, 40.0), &w.temp, &Text::new(fs * 2.0, m.on_surface).medium());
            ui.text(Rect::new(wi.x, wi.y + 52.0, wi.w, 20.0), &w.desc, &Text::new(fs * 0.95, m.on_surface));
            ui.text(Rect::new(wi.x, wi.y + 74.0, wi.w, 20.0), &w.place, &Text::new(fs * 0.85, m.on_surface_variant));
            ui.text(Rect::new(wi.x, wi.y + 94.0, wi.w, 20.0), &w.hi_lo, &Text::new(fs * 0.85, m.on_surface_variant));
        }
        None => {
            ui.icon(Rect::new(wi.x, wi.y, 40.0, 40.0), "weather-few-clouds-symbolic", 34.0, Some(m.outline));
            ui.text(Rect::new(wi.x, wi.y + 52.0, wi.w, 20.0), "Weather unavailable", &Text::new(fs * 0.92, m.on_surface_variant));
        }
    }
    gfx::fill_rounded(ui.cr, scard, 20.0, m.surface_container);
    let si = scard.inset(16.0);
    for (i, (icon, v, what)) in resources(data).iter().enumerate() {
        let y = si.y + i as f64 * 40.0;
        ui.icon(Rect::new(si.x, y, 20.0, 20.0), icon, 18.0, Some(m.on_surface_variant));
        ui.text(Rect::new(si.x + 26.0, y, si.w - 26.0, 20.0), what, &Text::new(fs * 0.9, m.on_surface));
        ui.text(Rect::new(si.x + 26.0, y, si.w - 26.0, 20.0), &pct(*v), &Text::new(fs * 0.9, m.on_surface_variant).end());
        let tr = Rect::new(si.x, y + 24.0, si.w, 6.0);
        gfx::fill_rounded(ui.cr, tr, 3.0, m.secondary_container);
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, tr.y, (tr.w * v.clamp(0.0, 1.0)).max(6.0), 6.0), 3.0, m.primary);
    }
}

/// Quick actions (Files, Terminal, Settings, Wallpaper, Screenshot, Session).
fn quick_actions(ui: &mut Ui, r: Rect, _data: &Data) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let h = 208.0f64.min(r.h);
    if h < 120.0 {
        return;
    }
    let card = Rect::new(r.x, r.y, r.w, h);
    gfx::fill_rounded(ui.cr, card, 20.0, m.surface_container);
    let (title, grid) = card.inset(14.0).take_top(22.0, 6.0);
    ui.text(title.inset_xy(4.0, 0.0), "Quick actions", &Text::new(fs * 1.0, m.on_surface).medium());
    let term = th.cfg.general.terminal.clone();
    let files = th.cfg.general.file_manager.clone();
    let items: [(&str, &str, Action); 6] = [
        ("folder-documents-symbolic", "Files", Action::Exec(files)),
        ("keyboard-symbolic", "Terminal", Action::Exec(term)),
        ("settings-symbolic", "Settings", Action::Exec("creed-settings".into())),
        ("view-grid-symbolic", "Wallpaper", Action::Exec("creed-settings --page appearance".into())),
        ("screenshot-symbolic", "Screenshot", Action::Exec("creed screenshot region".into())),
        ("power-symbolic", "Session", Action::Open(Kind::Power)),
    ];
    let rows = grid.rows(2, 6.0);
    for (i, (icon, label, a)) in items.into_iter().enumerate() {
        let c = rows[i / 3].columns(3, 6.0)[i % 3];
        let k = ui::idn("ls.qa", i);
        let hv = ui.hover_amt(k);
        state(ui, c, 16.0, m.on_surface, hv, ui.is_pressed(k));
        ui.icon(Rect::new(c.x, c.y + 6.0, c.w, c.h * 0.55), icon, 22.0, Some(m.on_surface));
        ui.text(Rect::new(c.x, c.y + c.h * 0.58, c.w, c.h * 0.36), label, &Text::new(fs * 0.85, m.on_surface_variant).center());
        if ui.focus == Some(k) {
            ui.th.focus_ring(ui.cr, c, 16.0);
        }
        ui.hit(k, c, a, Role::Button, label);
    }
}

/// Media card: art placeholder, title/artist, wavy progress, big rounded-square play button.
fn media_card(ui: &mut Ui, r: Rect, data: &Data) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    gfx::fill_rounded(ui.cr, r, 20.0, m.surface_container);
    let (art, rest) = r.inset(12.0).take_left(r.h - 24.0, 14.0);
    gfx::fill_rounded(ui.cr, art, 14.0, m.secondary_container);
    ui.icon(art, "music-note-symbolic", 36.0, Some(m.on_secondary_container));
    let (btn, info) = rest.take_right(56.0, 10.0);
    let (title, artist, playing, has) = match &data.media {
        Some(md) => (md.title.clone(), md.artist.clone(), md.playing, true),
        None => ("Nothing playing".into(), "Play something to see it here".into(), false, false),
    };
    ui.text(Rect::new(info.x, info.y + 4.0, info.w, 22.0), &title, &Text::new(fs * 1.05, m.on_surface).medium());
    ui.text(Rect::new(info.x, info.y + 26.0, info.w, 18.0), &artist, &Text::new(fs * 0.85, m.on_surface_variant));
    // Controls row + wavy line (Material 3 expressive progress).
    let wy = info.bottom() - 22.0;
    let (prev, rest2) = Rect::new(info.x, wy - 12.0, info.w, 24.0).take_left(24.0, 6.0);
    let (next, wave_r) = rest2.take_right(24.0, 6.0);
    icon_btn(ui, ui::id("lm.prev"), prev, "media-skip-backward-symbolic", "Previous track", Action::MediaPrev, None);
    icon_btn(ui, ui::id("lm.next"), next, "media-skip-forward-symbolic", "Next track", Action::MediaNext, None);
    let cr = ui.cr;
    cr.set_line_width(3.0);
    cr.set_line_cap(cairo::LineCap::Round);
    let split = wave_r.x + wave_r.w * if has { 0.45 } else { 0.0 };
    if has {
        cr.new_path();
        let mut x = wave_r.x;
        cr.move_to(x, wave_r.cy());
        while x <= split {
            cr.line_to(x, wave_r.cy() + ((x - wave_r.x) / 5.0).sin() * 3.0);
            x += 1.0;
        }
        gfx::set(cr, m.primary);
        let _ = cr.stroke();
    }
    cr.new_path();
    cr.move_to(split + 6.0, wave_r.cy());
    cr.line_to(wave_r.right(), wave_r.cy());
    gfx::set(cr, m.secondary_container);
    let _ = cr.stroke();
    let k = ui::id("lm.play");
    let hv = ui.hover_amt(k);
    let b = Rect::centered(btn.cx(), btn.cy(), 56.0, 56.0);
    gfx::fill_rounded(ui.cr, b, 16.0, m.primary_container);
    state(ui, b, 16.0, m.on_primary_container, hv, ui.is_pressed(k));
    ui.icon(b, if playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, 26.0, Some(m.on_primary_container));
    ui.hit(k, b, if has { Action::MediaPlayPause } else { Action::None }, Role::Button, if playing { "Pause" } else { "Play" });
}

fn tools_page(ui: &mut Ui, body: Rect) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let tools: [(&str, &str, &str, Action); 6] = [
        ("screenshot-symbolic", "Screenshot", "Select a region", Action::Exec("creed screenshot region".into())),
        ("view-fullscreen-symbolic", "Full screen", "Capture the whole screen", Action::Exec("creed screenshot screen".into())),
        ("color-select-symbolic", "Colour picker", "Copy a colour to the clipboard", Action::Exec("hyprpicker -a".into())),
        ("edit-paste-symbolic", "Clipboard", "History of copied items", Action::Toggle(Kind::Clipboard)),
        ("face-smile-symbolic", "Emoji", "Search and copy emoji", Action::Toggle(Kind::Emoji)),
        ("tune-symbolic", "Settings", "Everything in Creed", Action::Exec("creed-settings".into())),
    ];
    for (i, (icon, title, sub, a)) in tools.into_iter().enumerate() {
        let r = Rect::new(body.x, body.y + i as f64 * 64.0, body.w, 58.0);
        let k = ui::idn("ls.tool", i);
        let hv = ui.hover_amt(k);
        gfx::fill_rounded(ui.cr, r, 18.0, m.surface_container);
        state(ui, r, 18.0, m.on_surface, hv, ui.is_pressed(k));
        let ic = Rect::new(r.x + 10.0, r.y + 9.0, 40.0, 40.0);
        gfx::fill_rounded(ui.cr, ic, 20.0, m.secondary_container);
        ui.icon(ic, icon, 20.0, Some(m.on_secondary_container));
        ui.text(Rect::new(ic.right() + 14.0, r.y + 9.0, r.w - 80.0, 20.0), title, &Text::new(fs, m.on_surface).medium());
        ui.text(Rect::new(ic.right() + 14.0, r.y + 29.0, r.w - 80.0, 20.0), sub, &Text::new(fs * 0.85, m.on_surface_variant));
        if ui.focus == Some(k) {
            ui.th.focus_ring(ui.cr, r, 18.0);
        }
        ui.hit(k, r, a, Role::Button, format!("{title}: {sub}"));
    }
}

// ───────────────────────────── OSD ─────────────────────────────

pub const OSD_SIZE: (u32, u32) = (232, 64);

pub fn osd(ui: &mut Ui, kind: OsdKind, value: f64) {
    let th = ui.th;
    let m = roles(th);
    let fs = th.fs;
    let (w, h) = ui.size;
    let r = Rect::new(4.0, 4.0, w - 8.0, h - 8.0);
    gfx::fill_rounded(ui.cr, r, r.h / 2.0, m.surface_container);
    gfx::stroke_rounded(ui.cr, r, r.h / 2.0, 1.0, m.outline_variant.with_alpha(0.4));
    let (icon, label) = match kind {
        OsdKind::Volume => (if value <= 0.0 { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" }, "Volume"),
        OsdKind::Mic => (if value <= 0.0 { "microphone-disabled-symbolic" } else { "audio-input-microphone-symbolic" }, "Microphone"),
        OsdKind::Brightness => ("display-brightness-symbolic", "Brightness"),
        OsdKind::Game => ("input-gaming-symbolic", "Game Mode"),
    };
    let (ic, rest) = r.inset_xy(18.0, 0.0).take_left(28.0, 14.0);
    ui.icon(ic, icon, 26.0, Some(m.on_surface));
    if kind == OsdKind::Game {
        ui.text(rest, if value > 0.5 { "Game Mode on" } else { "Game Mode off" }, &Text::new(fs * 1.05, m.on_surface).medium());
        return;
    }
    let v = ui.spring(ui::id("m.osd"), value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    let top = Rect::new(rest.x, r.cy() - 17.0, rest.w, 22.0);
    ui.text(top, label, &Text::new(fs * 1.0, m.on_surface).medium());
    ui.text(top, &format!("{:.0}", v * 100.0), &Text::new(fs * 1.0, m.on_surface).medium().end());
    let track = Rect::new(rest.x, r.cy() + 9.0, rest.w, 4.0);
    let fw = (track.w * v).round();
    if fw > 0.0 {
        gfx::fill_rounded(ui.cr, Rect::new(track.x, track.y, fw, 4.0), 2.0, m.primary);
    }
    if fw + 4.0 < track.w {
        gfx::fill_rounded(ui.cr, Rect::new(track.x + fw + 4.0, track.y, track.w - fw - 4.0, 4.0), 2.0, m.secondary_container);
    }
}
