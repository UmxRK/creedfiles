//! The bar. One layout engine, several looks:
//! `creed` (floating / edge / islands pills), `win11` (centred taskbar), `win10` (left taskbar),
//! `mac` (menu bar), `aqua` (glossy menu bar).

use super::Spec;
use crate::data::{Data, NetKind};
use crate::gfx::{self, Align, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::config::{BarStyle, Edge};
use creed_core::Color;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

pub fn spec(th: &gfx::Theme, _ow: u32, _oh: u32) -> Spec {
    let b = &th.cfg.bar;
    let floating = b.style != BarStyle::Edge;
    let m = if floating { b.margin } else { 0 };
    let thick = b.size + m;
    // Material bar: rounded screen corners hang below (or above) it; they take no input.
    let corners = screen_corners(th);
    let input = match (corners, b.position) {
        (c, Edge::Top) if c > 0 => Some((0, 0, 1 << 20, thick as i32)),
        (c, Edge::Bottom) if c > 0 => Some((0, c as i32, 1 << 20, thick as i32)),
        _ => None,
    };
    let (anchor, size) = match b.position {
        Edge::Top => (Anchor::TOP | Anchor::LEFT | Anchor::RIGHT, (0, thick + corners)),
        Edge::Bottom => (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT, (0, thick + corners)),
        Edge::Left => (Anchor::LEFT | Anchor::TOP | Anchor::BOTTOM, (thick, 0)),
        Edge::Right => (Anchor::RIGHT | Anchor::TOP | Anchor::BOTTOM, (thick, 0)),
    };
    Spec {
        layer: Layer::Top,
        anchor,
        size,
        margin: (0, 0, 0, 0),
        exclusive: thick as i32,
        keyboard: KeyboardInteractivity::None,
        namespace: "creed-bar",
        click_through: false,
        input,
    }
}

/// Radius of the rounded screen corners drawn under a flush Material bar (0 = none).
pub fn screen_corners(th: &gfx::Theme) -> u32 {
    let b = &th.cfg.bar;
    if th.cfg.style.bar == "material" && b.style == BarStyle::Edge && matches!(b.position, Edge::Top | Edge::Bottom) {
        th.cfg.style.radius.round().clamp(0.0, 40.0) as u32
    } else {
        0
    }
}

fn vertical(ui: &Ui) -> bool {
    matches!(ui.th.cfg.bar.position, Edge::Left | Edge::Right)
}

pub fn status_icons_pub(data: &Data) -> (String, String, Option<String>, Option<String>) {
    status_icons(data)
}

fn status_icons(data: &Data) -> (String, String, Option<String>, Option<String>) {
    let net = match data.net.kind {
        NetKind::Ethernet => "network-wired-symbolic".to_string(),
        NetKind::Wifi => {
            let s = data.net.signal;
            let lvl = if s > 75 { "excellent" } else if s > 50 { "good" } else if s > 25 { "ok" } else { "weak" };
            format!("network-wireless-signal-{lvl}-symbolic")
        }
        NetKind::None => {
            if data.net.available && !data.net.wifi_enabled {
                "network-wireless-disabled-symbolic".into()
            } else {
                "network-offline-symbolic".into()
            }
        }
    };
    let vol = match &data.audio {
        Some(a) if a.muted || a.volume <= 0.001 => "audio-volume-muted-symbolic",
        Some(a) if a.volume < 0.34 => "audio-volume-low-symbolic",
        Some(a) if a.volume < 0.67 => "audio-volume-medium-symbolic",
        Some(_) => "audio-volume-high-symbolic",
        None => "audio-volume-muted-symbolic",
    }
    .to_string();
    let bat = data.battery.as_ref().map(|b| battery_icon(b.percent, b.charging));
    let bt = data.bt.as_ref().map(|b| {
        if !b.powered {
            "bluetooth-disabled-symbolic"
        } else {
            "bluetooth-active-symbolic"
        }
        .to_string()
    });
    (net, vol, bat, bt)
}

pub fn battery_icon(pct: f64, charging: bool) -> String {
    let lvl = ((pct / 10.0).round() as i32 * 10).clamp(0, 100);
    format!("battery-level-{lvl}{}-symbolic", if charging { "-charging" } else { "" })
}

/// Main-axis length a module wants.
fn module_len(ui: &mut Ui, name: &str, data: &Data, h: f64) -> f64 {
    match ui.th.cfg.style.bar.as_str() {
        "win11" => {
            if let Some(l) = super::win11::module_len(ui, name, data) {
                return l;
            }
        }
        "win10" => {
            if let Some(l) = super::win10::module_len(ui, name, data) {
                return l;
            }
        }
        "mac" | "aqua" => {
            if let Some(l) = super::mac::module_len(ui, name, data) {
                return l;
            }
        }
        "material" => {
            if let Some(l) = super::material::module_len(ui, name, data) {
                return l;
            }
        }
        _ => {}
    }
    if matches!(name, "resources" | "utilities" | "status") {
        return super::material::module_len(ui, name, data).unwrap_or(0.0);
    }
    let th = ui.th;
    let fs = th.fs;
    let variant = th.cfg.style.bar.as_str();
    let vert = vertical(ui);
    let icon_slot = (h * 0.82).round();
    match name {
        "launcher" => icon_slot,
        "workspaces" => {
            let n = ws_list(data, th.cfg.bar.workspaces).len() as f64;
            if variant == "creed" {
                let dot = (h * 0.22).round();
                n * (dot + 6.0) + dot * 1.6 + 12.0
            } else {
                n * (h * 0.62) + 8.0
            }
        }
        "window" | "appmenu" => {
            if vert {
                return 0.0;
            }
            let title = window_label(data, name == "appmenu");
            if title.is_empty() {
                return if name == "appmenu" { ui.measure("Desktop", &Text::new(fs, th.pal.text).bold()).0 + 20.0 } else { 0.0 };
            }
            let t = if name == "appmenu" { Text::new(fs, th.pal.text).bold() } else { Text::new(fs, th.pal.text) };
            (ui.measure(&title, &t).0 + 20.0).min(360.0)
        }
        "clock" => {
            if vert {
                return h * 1.1;
            }
            match variant {
                "win11" | "win10" => {
                    let a = ui.measure(&clock_text(ui), &Text::new(fs * 0.9, th.pal.text)).0;
                    let b = ui.measure(&time::format(time::now_secs(), "%d/%m/%Y"), &Text::new(fs * 0.9, th.pal.text)).0;
                    a.max(b) + 20.0
                }
                "mac" | "aqua" => {
                    let s = format!("{}  {}", time::format(time::now_secs(), &th.cfg.bar.date_format), clock_text(ui));
                    ui.measure(&s, &Text::new(fs, th.pal.text).medium()).0 + 16.0
                }
                _ => ui.measure(&clock_text(ui), &Text::new(fs * 1.05, th.pal.text).bold()).0 + 22.0,
            }
        }
        "date" => {
            if vert {
                return 0.0;
            }
            ui.measure(&time::format(time::now_secs(), &th.cfg.bar.date_format), &Text::new(fs, th.pal.text)).0 + 16.0
        }
        "tray" => {
            if data.tray.is_empty() {
                0.0
            } else {
                data.tray.len() as f64 * (h * 0.6) + 8.0
            }
        }
        "media" => match &data.media {
            Some(m) if !vert && !m.title.is_empty() => {
                let s = media_label(m);
                (ui.measure(&s, &Text::new(fs * 0.95, th.pal.text)).0 + h * 0.5 + 24.0).min(260.0)
            }
            _ => 0.0,
        },
        "network" | "volume" => icon_slot * 0.9,
        "bluetooth" => {
            if data.bt.is_some() {
                icon_slot * 0.9
            } else {
                0.0
            }
        }
        "battery" => match &data.battery {
            Some(b) if !vert => ui.measure(&format!("{:.0}%", b.percent), &Text::new(fs * 0.9, th.pal.text)).0 + icon_slot * 0.9,
            Some(_) => icon_slot,
            None => 0.0,
        },
        "sysstat" => {
            if vert {
                return 0.0;
            }
            ui.measure(&sys_label(data), &Text::new(fs * 0.9, th.pal.text).mono()).0 + 20.0
        }
        "weather" => match &data.weather {
            Some(w) if !vert => ui.measure(&w.temp, &Text::new(fs, th.pal.text)).0 + icon_slot + 8.0,
            _ => 0.0,
        },
        "keyboard" => {
            if data.kb_layout.is_empty() || vert {
                0.0
            } else {
                ui.measure(&kb_short(&data.kb_layout), &Text::new(fs * 0.9, th.pal.text).bold()).0 + 18.0
            }
        }
        "gamemode" | "notifications" | "control" | "power" | "taskview" | "widgets" => icon_slot,
        "search" => match variant {
            "win11" => (h * 4.6).round(),
            "win10" => (h * 6.6).round(),
            _ => icon_slot,
        },
        "taskbar" => {
            let n = taskbar_items(ui, data).len() as f64;
            n * (h * 0.92).round()
        }
        "spacer" => 8.0,
        _ => 0.0,
    }
}

pub fn clock_text(ui: &Ui) -> String {
    time::format(time::now_secs(), &ui.th.cfg.bar.clock_format)
}

fn window_label(data: &Data, app_name: bool) -> String {
    match data.active_client() {
        Some(c) if app_name => Data::pretty_class(&c.class),
        Some(c) => c.title.clone(),
        None => String::new(),
    }
}

fn media_label(m: &crate::data::Media) -> String {
    if m.artist.is_empty() {
        m.title.clone()
    } else {
        format!("{} - {}", m.title, m.artist)
    }
}

fn sys_label(d: &Data) -> String {
    format!("CPU {:>2.0}%  RAM {:.1}G", d.sys.cpu, d.sys.mem_used_mb as f64 / 1024.0)
}

fn kb_short(l: &str) -> String {
    l.chars().take(2).collect::<String>().to_uppercase()
}

/// Workspaces to show: 1..=N always, plus any existing ones beyond N (no special workspaces).
pub fn ws_list(data: &Data, n: u32) -> Vec<i64> {
    let mut v: Vec<i64> = (1..=n as i64).collect();
    for w in &data.workspaces {
        if w.id > 0 && !v.contains(&w.id) {
            v.push(w.id);
        }
    }
    v.sort();
    v
}

/// (desktop id or class, icon, running windows) for taskbar-style bars and docks.
pub fn taskbar_items(ui: &mut Ui, data: &Data) -> Vec<(String, String, Vec<String>)> {
    let pinned = ui.th.cfg.dock.pinned.clone();
    let mut items: Vec<(String, String, Vec<String>)> = vec![];
    for p in &pinned {
        let icon = data.apps.iter().find(|a| &a.id == p).map(|a| a.icon.clone()).unwrap_or_else(|| p.clone());
        items.push((p.clone(), icon, vec![]));
    }
    for c in data.clients.iter().filter(|c| Data::is_app_window(c)) {
        let class_l = c.class.to_lowercase();
        let pos = items.iter().position(|(id, _, _)| {
            let idl = id.to_lowercase();
            idl == class_l || idl.ends_with(&format!(".{class_l}")) || class_l.ends_with(&format!(".{idl}"))
        });
        match pos {
            Some(i) => items[i].2.push(c.address.clone()),
            None => {
                let icon = crate::services::apps::icon_for_class(&data.apps, &c.class);
                items.push((c.class.clone(), icon, vec![c.address.clone()]));
            }
        }
    }
    items
}

pub fn draw(ui: &mut Ui, data: &Data, _views: &mut super::Views, cfg_err: Option<&str>) {
    let th = ui.th;
    let b = &th.cfg.bar;
    let (sw, sh) = ui.size;
    let floating = b.style != BarStyle::Edge;
    let m = if floating { b.margin as f64 } else { 0.0 };
    let vert = vertical(ui);
    let full = match b.position {
        Edge::Top => Rect::new(m, m, sw - 2.0 * m, b.size as f64),
        Edge::Bottom => Rect::new(m, sh - m - b.size as f64, sw - 2.0 * m, b.size as f64),
        Edge::Left => Rect::new(m, m, b.size as f64, sh - 2.0 * m),
        Edge::Right => Rect::new(0.0, m, b.size as f64, sh - 2.0 * m),
    };
    ui.record("bar", full, None);
    let h = b.size as f64;
    let radius = if floating { b.radius.min(h / 2.0) } else { 0.0 };
    let os_variant = matches!(th.cfg.style.bar.as_str(), "win11" | "win10" | "mac" | "aqua");
    let pad = if os_variant { if th.cfg.style.bar.starts_with("win") { 4.0 } else { 10.0 } } else { (h * 0.16).round() };
    let gap = if os_variant { 0.0 } else { 2.0 };

    // Measure sections.
    let sections: [(&str, Vec<String>); 3] = [("left", b.left.clone()), ("center", b.center.clone()), ("right", b.right.clone())];
    let mut measured: Vec<(&str, Vec<(String, f64)>)> = vec![];
    for (name, mods) in &sections {
        let mut v = vec![];
        for mname in mods {
            let len = module_len(ui, mname, data, h).round();
            if len > 0.0 {
                v.push((mname.clone(), len));
            }
        }
        measured.push((name, v));
    }
    let total = |v: &Vec<(String, f64)>| -> f64 {
        if v.is_empty() {
            0.0
        } else {
            v.iter().map(|x| x.1).sum::<f64>() + gap * (v.len() as f64 - 1.0) + pad * 2.0
        }
    };
    let main_len = if vert { full.h } else { full.w };
    let lens: Vec<f64> = measured.iter().map(|(_, v)| total(v)).collect();
    // Section start offsets along the main axis.
    let starts = [0.0, ((main_len - lens[1]) / 2.0).round(), main_len - lens[2]];

    let sect_rect = |i: usize| -> Rect {
        let off = starts[i];
        let len = lens[i];
        if vert {
            Rect::new(full.x, full.y + off, full.w, len)
        } else {
            Rect::new(full.x + off, full.y, len, full.h)
        }
    };

    // Background.
    match th.cfg.style.bar.as_str() {
        "win11" => super::win11::bar_background(ui, full),
        "win10" => super::win10::bar_background(ui, full),
        "mac" | "aqua" => super::mac::bar_background(ui, full),
        "material" => super::material::bar_background(ui, full, screen_corners(th) as f64),
        _ => draw_creed_background(ui, full, radius, floating, &sect_rect, &lens),
    }
    fn draw_creed_background(ui: &mut Ui, full: Rect, radius: f64, floating: bool, sect_rect: &dyn Fn(usize) -> Rect, lens: &[f64]) {
    let th = ui.th;
    let b = &th.cfg.bar;
    match b.style {
        BarStyle::Islands => {
            for i in 0..3 {
                if lens[i] > 0.0 {
                    th.panel(ui.cr, sect_rect(i), radius, "bar", ui.pointer);
                }
            }
        }
        _ => {
            th.panel(ui.cr, full, radius, "bar", ui.pointer);
            if !floating && th.cfg.style.bar != "mac" && th.cfg.style.bar != "aqua" {
                // Hairline separator against the desktop.
                let line = if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.07) } else { Color::rgba(0.0, 0.0, 0.0, 0.08) };
                let lr = match b.position {
                    Edge::Top => Rect::new(full.x, full.bottom() - 1.0, full.w, 1.0),
                    Edge::Bottom => Rect::new(full.x, full.y, full.w, 1.0),
                    Edge::Left => Rect::new(full.right() - 1.0, full.y, 1.0, full.h),
                    Edge::Right => Rect::new(full.x, full.y, 1.0, full.h),
                };
                gfx::fill_rounded(ui.cr, lr, 0.0, line);
            }
            if th.cfg.style.bar == "aqua" {
                // Glossy top half.
                let g = cairo::LinearGradient::new(0.0, full.y, 0.0, full.bottom());
                g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, if th.dark { 0.10 } else { 0.55 });
                g.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.0);
                ui.cr.rectangle(full.x, full.y, full.w, full.h);
                let _ = ui.cr.set_source(&g);
                let _ = ui.cr.fill();
            }
        }
    }

    }

    for (si, (sname, mods)) in measured.iter().enumerate() {
        let sr = sect_rect(si);
        if lens[si] > 0.0 {
            ui.record(&format!("bar.{sname}"), sr, Some("bar"));
        }
        let mut off = pad;
        for (mname, len) in mods {
            let r = if vert {
                Rect::new(sr.x, sr.y + off, sr.w, *len)
            } else {
                Rect::new(sr.x + off, sr.y, *len, sr.h)
            };
            ui.record(&format!("bar.mod.{mname}"), r, Some(&format!("bar.{sname}")));
            draw_module(ui, mname, r, data);
            off += len + gap;
        }
    }

    if let Some(e) = cfg_err {
        // Small warning dot at the far end so a broken config is never silent.
        let r = Rect::new(full.right() - 10.0, full.y + 4.0, 6.0, 6.0);
        gfx::fill_rounded(ui.cr, r, 3.0, th.pal.warning);
        ui.hit(ui::id("bar.cfgerr"), r.inset(-6.0), Action::Exec(format!("notify-send 'Creed config error' {}", shell_q(e))), Role::Button, "Config error");
    }
}

fn shell_q(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn draw_module(ui: &mut Ui, name: &str, r: Rect, data: &Data) {
    let handled = match ui.th.cfg.style.bar.as_str() {
        "win11" => super::win11::draw_module(ui, name, r, data),
        "win10" => super::win10::draw_module(ui, name, r, data),
        "mac" | "aqua" => super::mac::draw_module(ui, name, r, data),
        "material" => super::material::draw_module(ui, name, r, data),
        _ => matches!(name, "resources" | "utilities" | "status") && super::material::draw_module(ui, name, r, data),
    };
    if handled {
        return;
    }
    let th = ui.th;
    let p = &th.pal;
    let fs = th.fs;
    let variant = th.cfg.style.bar.clone();
    let vert = vertical(ui);
    let h = th.cfg.bar.size as f64;
    let slot = Rect::centered(r.cx(), r.cy(), if vert { r.w * 0.8 } else { r.w }, if vert { r.h } else { h * 0.78 });
    let key = ui::id(&format!("bar.{name}"));
    match name {
        "launcher" => {
            let k = Kind::Launcher;
            let open = false;
            let hv = ui.hover_amt(key);
            let rr = Rect::centered(r.cx(), r.cy(), slot.h, slot.h);
            ui.th.hover_bg(ui.cr, rr, rr.h / 2.0, hv);
            draw_logo(ui, rr.inset(rr.h * 0.22), &variant);
            if ui.focus == Some(key) {
                ui.th.focus_ring(ui.cr, rr, rr.h / 2.0);
            }
            let label = match variant.as_str() {
                "win11" | "win10" => "Start",
                "mac" | "aqua" => "Applications",
                _ => "Apps",
            };
            let _ = open;
            ui.hit(key, r, Action::Toggle(k), Role::Button, label);
        }
        "workspaces" => draw_workspaces(ui, r, data, &variant),
        "window" | "appmenu" => {
            let title = window_label(data, name == "appmenu");
            let title = if title.is_empty() && name == "appmenu" { "Desktop".to_string() } else { title };
            let t = if name == "appmenu" { Text::new(fs, p.text).bold() } else { Text::new(fs, p.subtext) };
            let hv = ui.hover_amt(key);
            if name == "appmenu" {
                ui.th.hover_bg(ui.cr, slot, th.radius_small(), hv);
            }
            ui.text(r.inset_xy(10.0, 0.0), &title, &t);
            if name == "appmenu" {
                ui.hit(key, r, Action::Toggle(Kind::Snap), Role::Button, format!("{title} window menu"));
            }
        }
        "clock" => {
            let hv = ui.hover_amt(key);
            ui.th.hover_bg(ui.cr, slot, th.radius_small().min(slot.h / 2.0), hv);
            let now = time::now_secs();
            if vert {
                let hh = time::format(now, "%H");
                let mm = time::format(now, "%M");
                let (a, bb) = r.take_top(r.h / 2.0, 0.0);
                ui.text(Rect::new(a.x, a.y + 4.0, a.w, a.h - 4.0), &hh, &Text::new(fs * 1.05, p.text).bold().center());
                ui.text(Rect::new(bb.x, bb.y, bb.w, bb.h - 4.0), &mm, &Text::new(fs * 1.05, p.subtext).bold().center());
            } else {
                match variant.as_str() {
                    "win11" | "win10" => {
                        let (a, bb) = slot.take_top(slot.h / 2.0, 0.0);
                        ui.text(Rect::new(a.x, a.y + 1.0, a.w - 10.0, a.h), &clock_text(ui), &Text::new(fs * 0.9, p.text).end());
                        ui.text(Rect::new(bb.x, bb.y - 1.0, bb.w - 10.0, bb.h), &time::format(now, "%d/%m/%Y"), &Text::new(fs * 0.9, p.text).end());
                    }
                    "mac" | "aqua" => {
                        let s = format!("{}  {}", time::format(now, &th.cfg.bar.date_format), clock_text(ui));
                        ui.text(r, &s, &Text::new(fs, p.text).medium().center());
                    }
                    _ => {
                        ui.text(r, &clock_text(ui), &Text::new(fs * 1.05, p.text).bold().center());
                    }
                }
            }
            let target = if matches!(variant.as_str(), "win11" | "win10" | "mac" | "aqua") { Kind::NotifCenter } else { Kind::Dashboard };
            let label = format!("Clock, {}", time::format(now, "%A %d %B %H:%M"));
            ui.hit(key, r, Action::Toggle(target), Role::Button, label);
        }
        "date" => {
            ui.text(r, &time::format(time::now_secs(), &th.cfg.bar.date_format), &Text::new(fs, p.subtext).center());
            ui.hit(key, r, Action::Toggle(Kind::Calendar), Role::Button, "Calendar");
        }
        "tray" => {
            let n = data.tray.len();
            if n == 0 {
                return;
            }
            let cell = if vert { r.h / n as f64 } else { r.w / n as f64 };
            for (i, item) in data.tray.iter().enumerate() {
                let c = if vert {
                    Rect::new(r.x, r.y + i as f64 * cell, r.w, cell)
                } else {
                    Rect::new(r.x + 4.0 + i as f64 * ((r.w - 8.0) / n as f64), r.y, (r.w - 8.0) / n as f64, r.h)
                };
                let k = ui::idn("bar.tray", &item.key);
                let hv = ui.hover_amt(k);
                let cc = Rect::centered(c.cx(), c.cy(), c.w.min(slot.h), slot.h * 0.86);
                ui.th.hover_bg(ui.cr, cc, th.radius_small().min(cc.h / 2.0), hv);
                let isz = (h * 0.42).round();
                if let Some((pw, ph, px)) = &item.pixmap {
                    draw_pixmap(ui.cr, Rect::centered(c.cx(), c.cy(), isz, isz), *pw, *ph, px);
                } else {
                    let icon = if item.icon.is_empty() { item.id.clone() } else { item.icon.clone() };
                    ui.icon(c, &icon, isz, Some(p.text));
                }
                let label = if item.title.is_empty() { item.id.clone() } else { item.title.clone() };
                let primary = if item.item_is_menu { Action::TrayMenu(item.key.clone()) } else { Action::TrayActivate(item.key.clone()) };
                let hit = ui.hit(k, c, primary, Role::Button, label);
                hit.secondary = Some(Action::TrayMenu(item.key.clone()));
            }
        }
        "media" => {
            let Some(m) = &data.media else { return };
            let hv = ui.hover_amt(key);
            ui.th.hover_bg(ui.cr, slot, slot.h / 2.0, hv);
            let (ic, rest) = slot.inset_xy(10.0, 0.0).take_left(h * 0.42, 6.0);
            ui.icon(ic, if m.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, (h * 0.38).round(), Some(p.accent));
            ui.text(rest, &media_label(m), &Text::new(fs * 0.95, p.text));
            let hit = ui.hit(key, r, Action::MediaPlayPause, Role::Button, format!("Media: {}", media_label(m)));
            hit.secondary = Some(Action::MediaNext);
        }
        "network" | "volume" | "bluetooth" => {
            let (net, vol, _, bt) = status_icons(data);
            let (icon, label, action, scroll): (String, String, Action, Option<fn(f64) -> Action>) = match name {
                "network" => {
                    let l = match data.net.kind {
                        NetKind::Wifi => format!("Wi-Fi: {} ({}%)", data.net.name, data.net.signal),
                        NetKind::Ethernet => "Wired network connected".into(),
                        NetKind::None => "Offline".into(),
                    };
                    (net, l, Action::Toggle(Kind::Control), None)
                }
                "volume" => {
                    let l = match &data.audio {
                        Some(a) => format!("Volume {:.0}%{}", a.volume * 100.0, if a.muted { ", muted" } else { "" }),
                        None => "No audio device".into(),
                    };
                    (vol, l, Action::Toggle(Kind::Control), Some(|d| Action::VolumeStep(d * 0.05)))
                }
                _ => {
                    let l = match &data.bt {
                        Some(b) if !b.powered => "Bluetooth off".into(),
                        Some(b) if b.connected.is_empty() => "Bluetooth on".into(),
                        Some(b) => format!("Bluetooth: {}", b.connected.join(", ")),
                        None => "Bluetooth".into(),
                    };
                    (bt.unwrap_or_default(), l, Action::Toggle(Kind::Control), None)
                }
            };
            let hv = ui.hover_amt(key);
            let cc = Rect::centered(r.cx(), r.cy(), r.w.min(slot.h), slot.h);
            ui.th.hover_bg(ui.cr, cc, cc.h / 2.0, hv);
            ui.icon(r, &icon, (h * 0.44).round(), Some(p.text));
            let hit = ui.hit(key, r, action, Role::Button, label);
            hit.scroll = scroll;
            if name == "volume" {
                hit.secondary = Some(Action::ToggleMute);
            }
        }
        "battery" => {
            let Some(bat) = &data.battery else { return };
            let icon = battery_icon(bat.percent, bat.charging);
            let low = bat.percent <= 15.0 && !bat.charging;
            let c = if low { p.error } else { p.text };
            let hv = ui.hover_amt(key);
            ui.th.hover_bg(ui.cr, slot, slot.h / 2.0, hv);
            if vert {
                ui.icon(r, &icon, (h * 0.44).round(), Some(c));
            } else {
                let (ic, rest) = r.take_left(h * 0.7, 0.0);
                ui.icon(ic, &icon, (h * 0.44).round(), Some(c));
                ui.text(rest, &format!("{:.0}%", bat.percent), &Text::new(fs * 0.9, c));
            }
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, format!("Battery {:.0}%{}", bat.percent, if bat.charging { ", charging" } else { "" }));
        }
        "sysstat" => {
            ui.text(r, &sys_label(data), &Text::new(fs * 0.9, p.subtext).mono().center());
            ui.hit(key, r, Action::Toggle(Kind::Dashboard), Role::Button, sys_label(data));
        }
        "weather" => {
            let Some(w) = &data.weather else { return };
            let (ic, rest) = r.take_left(h * 0.82, 0.0);
            ui.icon(ic, &w.icon, (h * 0.46).round(), Some(p.text));
            ui.text(rest, &w.temp, &Text::new(fs, p.text));
            ui.hit(key, r, Action::Toggle(Kind::Dashboard), Role::Button, format!("Weather {} {}", w.temp, w.desc));
        }
        "keyboard" => {
            ui.text(r, &kb_short(&data.kb_layout), &Text::new(fs * 0.9, p.subtext).bold().center());
            ui.hit(key, r, Action::Hypr("hl.dsp.exec_cmd(\"hyprctl switchxkblayout all next\")".into()), Role::Button, format!("Keyboard layout {}", data.kb_layout));
        }
        "gamemode" => {
            ui.icon_button(key, slot, "input-gaming-symbolic", if data.game { "Game Mode on" } else { "Game Mode off" }, Action::ToggleGame, data.game);
        }
        "notifications" => {
            let icon = if data.dnd { "notifications-disabled-symbolic" } else { "preferences-system-notifications-symbolic" };
            ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), icon, &format!("{} notifications", data.history.len()), Action::Toggle(Kind::NotifCenter), false);
            if !data.history.is_empty() && !data.dnd {
                let d = Rect::centered(r.cx() + slot.h * 0.22, r.cy() - slot.h * 0.22, 7.0, 7.0);
                gfx::fill_rounded(ui.cr, d, 3.5, p.accent);
            }
        }
        "control" => {
            let icon = match variant.as_str() {
                "mac" | "aqua" => "view-more-horizontal-symbolic",
                _ => "emblem-system-symbolic",
            };
            ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), icon, "Control center", Action::Toggle(Kind::Control), false);
        }
        "power" => {
            ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), "system-shutdown-symbolic", "Power menu", Action::Toggle(Kind::Power), false);
        }
        "taskview" => {
            let icon = if variant == "mac" || variant == "aqua" { "view-grid-symbolic" } else { "view-dual-symbolic" };
            ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), icon, "Task view", Action::Toggle(Kind::Overview), false);
        }
        "widgets" => {
            ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), "weather-few-clouds-symbolic", "Widgets", Action::Toggle(Kind::Dashboard), false);
        }
        "search" => match variant.as_str() {
            "win11" | "win10" => {
                let hv = ui.hover_amt(key);
                let box_h = if variant == "win11" { slot.h * 0.86 } else { h };
                let sr = Rect::centered(r.cx(), r.cy(), r.w - 8.0, box_h);
                let radius = if variant == "win11" { sr.h / 2.0 } else { 0.0 };
                let bg = if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.06 + 0.04 * hv) } else { Color::rgba(1.0, 1.0, 1.0, 0.65 + 0.2 * hv) };
                gfx::fill_rounded(ui.cr, sr, radius, bg);
                if variant == "win11" {
                    gfx::stroke_rounded(ui.cr, sr, radius, 1.0, if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.06) } else { Color::rgba(0.0, 0.0, 0.0, 0.08) });
                }
                let (ic, rest) = sr.inset_xy(12.0, 0.0).take_left(h * 0.36, 10.0);
                ui.icon(ic, "system-search-symbolic", (h * 0.34).round(), Some(p.subtext));
                ui.text(rest, if variant == "win11" { "Search" } else { "Type here to search" }, &Text::new(fs * 0.95, p.subtext));
                ui.hit(key, r, Action::Toggle(Kind::Launcher), Role::Button, "Search");
            }
            _ => {
                ui.icon_button(key, Rect::centered(r.cx(), r.cy(), slot.h, slot.h), "system-search-symbolic", "Search", Action::Toggle(Kind::Launcher), false);
            }
        },
        "taskbar" => draw_taskbar(ui, r, data, &variant),
        _ => {}
    }
}

/// Creed's own mark: a soft four-petal knot. Windows/mac modes use a neutral grid / dot mark
/// (no vendor logos).
fn draw_logo(ui: &mut Ui, r: Rect, variant: &str) {
    let cr = ui.cr;
    let p = &ui.th.pal;
    match variant {
        "win11" | "win10" => {
            // Neutral 2×2 rounded tiles in the accent colour.
            let g = r.w * 0.1;
            let s = (r.w - g) / 2.0;
            for (i, j) in [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)] {
                gfx::fill_rounded(cr, Rect::new(r.x + i * (s + g), r.y + j * (s + g), s, s), s * 0.22, p.accent);
            }
        }
        "mac" | "aqua" => {
            // A simple filled circle with a soft bite - clearly not any vendor's mark.
            gfx::fill_rounded(cr, Rect::centered(r.cx(), r.cy(), r.w * 0.78, r.w * 0.78), r.w, p.text);
            gfx::fill_rounded(cr, Rect::centered(r.cx() + r.w * 0.2, r.cy() - r.w * 0.2, r.w * 0.26, r.w * 0.26), r.w, p.accent);
        }
        _ => {
            // Creed knot: four overlapping petals.
            let c = (r.cx(), r.cy());
            let pr = r.w * 0.27;
            for (k, (dx, dy)) in [(0.0, -1.0), (1.0, 0.0), (0.0, 1.0), (-1.0, 0.0)].iter().enumerate() {
                let col = if k % 2 == 0 { p.accent } else { p.accent_alt };
                gfx::fill_rounded(cr, Rect::centered(c.0 + dx * pr * 0.8, c.1 + dy * pr * 0.8, pr * 1.55, pr * 1.55), pr, col.with_alpha(0.92));
            }
            gfx::fill_rounded(cr, Rect::centered(c.0, c.1, pr * 0.7, pr * 0.7), pr, p.on_accent.with_alpha(0.85));
        }
    }
}

fn draw_workspaces(ui: &mut Ui, r: Rect, data: &Data, variant: &str) {
    let th = ui.th;
    let p = th.pal.clone();
    let h = th.cfg.bar.size as f64;
    let list = ws_list(data, th.cfg.bar.workspaces);
    let occupied = |id: i64| data.workspaces.iter().any(|w| w.id == id && w.windows > 0);
    let vert = vertical(ui);
    if variant == "creed" {
        let dot = (h * 0.22).round();
        let mut off = 6.0;
        for id in &list {
            let active = *id == data.active_ws;
            let k = ui::idn("bar.ws", id);
            let w = ui.spring(k, if active { dot * 2.6 } else { dot }, if active { dot * 2.6 } else { dot });
            let hv = ui.hover_amt(k);
            let rr = if vert {
                Rect::new(r.cx() - dot / 2.0, r.y + off, dot, w)
            } else {
                Rect::new(r.x + off, r.cy() - dot / 2.0, w, dot)
            };
            let col = if active {
                p.accent
            } else if occupied(*id) {
                p.text.with_alpha(0.55 + 0.25 * hv)
            } else {
                p.text.with_alpha(0.22 + 0.25 * hv)
            };
            gfx::fill_rounded(ui.cr, rr, dot / 2.0, col);
            let hitr = if vert { Rect::new(r.x, rr.y - 3.0, r.w, rr.h + 6.0) } else { Rect::new(rr.x - 3.0, r.y, rr.w + 6.0, r.h) };
            if ui.focus == Some(k) {
                ui.th.focus_ring(ui.cr, rr, dot / 2.0);
            }
            let hit = ui.hit(k, hitr, Action::Workspace(*id), Role::Button, format!("Workspace {id}{}", if active { ", current" } else { "" }));
            hit.scroll = Some(|d| Action::WorkspaceScroll(if d > 0.0 { 1 } else { -1 }));
            off += w + 6.0;
        }
    } else {
        // Numbered chips (Windows/mac flavours rarely show workspaces, but users may add them).
        let cell = h * 0.62;
        for (i, id) in list.iter().enumerate() {
            let active = *id == data.active_ws;
            let k = ui::idn("bar.ws", id);
            let hv = ui.hover_amt(k);
            let c = Rect::new(r.x + 4.0 + i as f64 * cell, r.cy() - cell / 2.0, cell - 4.0, cell);
            if active {
                gfx::fill_rounded(ui.cr, c, th.radius_small().min(c.h / 2.0), p.accent.with_alpha(0.9));
            } else {
                ui.th.hover_bg(ui.cr, c, th.radius_small().min(c.h / 2.0), hv);
            }
            let fg = if active { p.on_accent } else if occupied(*id) { p.text } else { p.muted };
            ui.text(c, &id.to_string(), &Text::new(th.fs * 0.9, fg).bold().center());
            ui.hit(k, c, Action::Workspace(*id), Role::Button, format!("Workspace {id}"));
        }
    }
}

fn draw_taskbar(ui: &mut Ui, r: Rect, data: &Data, variant: &str) {
    let items = taskbar_items(ui, data);
    let th = ui.th;
    let p = th.pal.clone();
    let h = th.cfg.bar.size as f64;
    let cell = (h * 0.92).round();
    for (i, (id, icon, wins)) in items.iter().enumerate() {
        let c = Rect::new(r.x + i as f64 * cell, r.y, cell, r.h);
        let k = ui::idn("bar.task", id);
        let hv = ui.hover_amt(k);
        let active = wins.iter().any(|a| *a == data.active_addr);
        let box_r = Rect::centered(c.cx(), c.cy(), cell - 6.0, if variant == "win10" { r.h } else { cell - 6.0 });
        let radius = if variant == "win10" { 0.0 } else { th.radius_small().min(6.0) };
        if active {
            gfx::fill_rounded(ui.cr, box_r, radius, if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.09) } else { Color::rgba(1.0, 1.0, 1.0, 0.6) });
        }
        ui.th.hover_bg(ui.cr, box_r, radius, hv);
        let press = if ui.is_pressed(k) { 0.86 } else { 1.0 };
        ui.icon(c, icon, (h * 0.5 * press).round(), None);
        if !wins.is_empty() {
            // Running indicator: Win11 = short pill (wider when active), Win10 = full underline.
            let (iw, ih) = if variant == "win10" { (box_r.w, 2.0) } else { (if active { 16.0 } else { 6.0 }, 3.0) };
            let iw = ui.spring(k ^ 0x99, iw, iw);
            let ir = Rect::centered(c.cx(), box_r.bottom() - ih - if variant == "win10" { 0.0 } else { 1.0 }, iw, ih);
            gfx::fill_rounded(ui.cr, ir, ih / 2.0, if active { p.accent } else { p.muted });
        }
        let label = Data::pretty_class(id);
        let action = match wins.as_slice() {
            [] => Action::Launch(id.clone()),
            [one] if *one != data.active_addr => Action::FocusWindow(one.clone()),
            [one] => Action::Hypr(format!("hl.dsp.window.move({{ workspace = \"special:minimized\", window = \"address:{one}\" }})")),
            many => {
                // Cycle through this app's windows.
                let idx = many.iter().position(|a| *a == data.active_addr).map(|i| (i + 1) % many.len()).unwrap_or(0);
                Action::FocusWindow(many[idx].clone())
            }
        };
        if ui.focus == Some(k) {
            ui.th.focus_ring(ui.cr, box_r, radius);
        }
        let hit = ui.hit(k, c, action, Role::Button, format!("{label}{}", if wins.is_empty() { "" } else { ", running" }));
        hit.secondary = Some(Action::Launch(id.clone()));
    }
}

/// Draws an SNI ARGB32 (network byte order) pixmap.
pub fn draw_pixmap(cr: &cairo::Context, r: Rect, w: i32, h: i32, px: &[u8]) {
    if w <= 0 || h <= 0 || px.len() < (w * h * 4) as usize {
        return;
    }
    let Ok(mut surf) = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h) else { return };
    let stride = surf.stride() as usize;
    if let Ok(mut d) = surf.data() {
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = (y * w as usize + x) * 4;
                let (a, rr, g, b) = (px[i] as u32, px[i + 1] as u32, px[i + 2] as u32, px[i + 3] as u32);
                let o = y * stride + x * 4;
                d[o] = (b * a / 255) as u8;
                d[o + 1] = (g * a / 255) as u8;
                d[o + 2] = (rr * a / 255) as u8;
                d[o + 3] = a as u8;
            }
        }
    }
    cr.save().ok();
    cr.translate(r.x, r.y);
    cr.scale(r.w / w as f64, r.h / h as f64);
    let _ = cr.set_source_surface(&surf, 0.0, 0.0);
    cr.source().set_filter(cairo::Filter::Good);
    let _ = cr.paint();
    cr.restore().ok();
}

#[allow(dead_code)]
pub fn align_for(variant: &str) -> Align {
    match variant {
        "win11" => Align::Center,
        _ => Align::Start,
    }
}
