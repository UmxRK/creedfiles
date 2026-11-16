//! Windows 10 (1903–22H2) surfaces: 40 px taskbar with 48 px buttons and the full-width
//! search box, the Start menu with rail + app list + live-tile grid, the full-height
//! Action Center with quick-action tiles, square toasts and the top-left volume OSD.
//! Square corners everywhere, Acrylic surfaces.

use super::fluent::{self, tok, Tok, BODY, CAPTION};
use super::Views;
use crate::data::{Data, NetKind};
use crate::gfx::{self, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Slider, Ui};
use creed_core::Color;

const W: f64 = 48.0;

fn bg(t: &Tok) -> Color {
    if t.dark {
        Color::rgba(0.063, 0.063, 0.063, 0.88)
    } else {
        Color::rgba(0.93, 0.93, 0.93, 0.92)
    }
}

fn surface(t: &Tok) -> Color {
    if t.dark {
        Color::rgba(0.122, 0.122, 0.122, 0.94)
    } else {
        Color::rgba(0.9, 0.9, 0.9, 0.94)
    }
}

fn hover(t: &Tok) -> Color {
    if t.dark {
        Color::rgba(1.0, 1.0, 1.0, 0.1)
    } else {
        Color::rgba(0.0, 0.0, 0.0, 0.08)
    }
}

pub fn bar_background(ui: &mut Ui, full: Rect) {
    let t = tok(ui.th);
    gfx::fill_rounded(ui.cr, full, 0.0, bg(&t));
}

fn flat(ui: &mut Ui, t: &Tok, r: Rect, key: ui::Id) -> f64 {
    let hv = ui.hover_amt(key);
    if hv > 0.01 {
        gfx::fill_rounded(ui.cr, r, 0.0, hover(t).with_alpha(hover(t).a * hv));
    }
    if ui.is_pressed(key) {
        gfx::fill_rounded(ui.cr, r, 0.0, hover(t));
    }
    hv
}

pub fn module_len(ui: &mut Ui, name: &str, data: &Data) -> Option<f64> {
    let t = tok(ui.th);
    Some(match name {
        "launcher" | "taskview" | "notifications" => W,
        "search" => 344.0,
        "network" | "volume" => 24.0,
        "battery" => {
            if data.battery.is_some() {
                24.0
            } else {
                0.0
            }
        }
        "bluetooth" => 0.0,
        "keyboard" => {
            if data.kb_layout.is_empty() {
                0.0
            } else {
                40.0
            }
        }
        "tray" => {
            if data.tray.is_empty() {
                0.0
            } else {
                24.0 + data.tray.len() as f64 * 24.0
            }
        }
        "clock" => {
            let a = ui.measure(&super::bar::clock_text(ui), &Text::new(CAPTION, t.text1)).0;
            let b = ui.measure(&time::format(time::now_secs(), &ui.th.cfg.bar.date_format), &Text::new(CAPTION, t.text1)).0;
            a.max(b) + 20.0
        }
        "taskbar" => super::bar::taskbar_items(ui, data).len() as f64 * W,
        _ => return None,
    })
}

pub fn draw_module(ui: &mut Ui, name: &str, r: Rect, data: &Data) -> bool {
    let t = tok(ui.th);
    let key = ui::id(&format!("w10.{name}"));
    match name {
        "launcher" => {
            flat(ui, &t, r, key);
            ui.icon(r, "view-app-grid-symbolic", 16.0, Some(t.text1));
            ui.hit(key, r, Action::Toggle(Kind::Launcher), Role::Button, "Start");
        }
        "search" => {
            let hv = ui.hover_amt(key);
            let fill = if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.10 + 0.05 * hv) } else { Color::rgba(1.0, 1.0, 1.0, 0.85 + 0.15 * hv) };
            gfx::fill_rounded(ui.cr, r, 0.0, fill);
            if !t.dark {
                gfx::stroke_rounded(ui.cr, r, 0.0, 1.0, Color::rgba(0.0, 0.0, 0.0, 0.12));
            }
            ui.icon(Rect::new(r.x + 12.0, r.y, 16.0, r.h), "system-search-symbolic", 16.0, Some(t.text1));
            ui.text(Rect::new(r.x + 40.0, r.y, r.w - 48.0, r.h), "Type here to search", &Text::new(BODY, t.text2));
            ui.hit(key, r, Action::Toggle(Kind::Launcher), Role::Button, "Search");
        }
        "taskview" => {
            flat(ui, &t, r, key);
            ui.icon(r, "view-dual-symbolic", 16.0, Some(t.text1));
            ui.hit(key, r, Action::Toggle(Kind::Overview), Role::Button, "Task View");
        }
        "network" | "volume" | "battery" => {
            flat(ui, &t, r, key);
            let icon = match name {
                "network" => match data.net.kind {
                    NetKind::Wifi => "network-wireless-signal-excellent-symbolic".to_string(),
                    NetKind::Ethernet => "network-wired-symbolic".into(),
                    NetKind::None => "network-offline-symbolic".into(),
                },
                "volume" => match &data.audio {
                    Some(a) if !a.muted && a.volume > 0.0 => "audio-volume-high-symbolic".into(),
                    _ => "audio-volume-muted-symbolic".into(),
                },
                _ => data.battery.as_ref().map(|b| super::bar::battery_icon(b.percent, b.charging)).unwrap_or_default(),
            };
            ui.icon(r, &icon, 16.0, Some(t.text1));
            let action = Action::Toggle(Kind::Control);
            let hit = ui.hit(key, r, action, Role::Button, name.to_string());
            if name == "volume" {
                hit.scroll = Some(|d| Action::VolumeStep(d * 0.02));
            }
        }
        "keyboard" => {
            flat(ui, &t, r, key);
            ui.text(r, &data.kb_layout.chars().take(3).collect::<String>().to_uppercase(), &Text::new(CAPTION, t.text1).center());
            ui.hit(key, r, Action::Hypr("hl.dsp.exec_cmd(\"hyprctl switchxkblayout all next\")".into()), Role::Button, "Input language");
        }
        "tray" => {
            let chev = Rect::new(r.x, r.y, 24.0, r.h);
            let ck = ui::id("w10.chev");
            flat(ui, &t, chev, ck);
            ui.icon(chev, "go-up-symbolic", 12.0, Some(t.text1));
            ui.hit(ck, chev, Action::None, Role::Button, "Show hidden icons");
            for (i, item) in data.tray.iter().enumerate() {
                let c = Rect::new(r.x + 24.0 + i as f64 * 24.0, r.y, 24.0, r.h);
                let k = ui::idn("w10.tray", &item.key);
                flat(ui, &t, c, k);
                if let Some((pw, ph, px)) = &item.pixmap {
                    super::bar::draw_pixmap(ui.cr, Rect::centered(c.cx(), c.cy(), 16.0, 16.0), *pw, *ph, px);
                } else {
                    ui.icon(c, if item.icon.is_empty() { &item.id } else { &item.icon }, 16.0, Some(t.text1));
                }
                let primary = if item.item_is_menu { Action::TrayMenu(item.key.clone()) } else { Action::TrayActivate(item.key.clone()) };
                let hit = ui.hit(k, c, primary, Role::Button, item.title.clone());
                hit.secondary = Some(Action::TrayMenu(item.key.clone()));
            }
        }
        "clock" => {
            flat(ui, &t, r, key);
            let now = time::now_secs();
            ui.text(Rect::new(r.x, r.y + 3.0, r.w, 17.0), &super::bar::clock_text(ui), &Text::new(CAPTION, t.text1).center());
            ui.text(Rect::new(r.x, r.y + 20.0, r.w, 17.0), &time::format(now, &ui.th.cfg.bar.date_format), &Text::new(CAPTION, t.text1).center());
            ui.hit(key, r, Action::Toggle(Kind::Calendar), Role::Button, format!("Clock {}", time::format(now, "%A %d %B %H:%M")));
        }
        "notifications" => {
            flat(ui, &t, r, key);
            let icon = if data.history.is_empty() { "notifications-symbolic" } else { "preferences-system-notifications-symbolic" };
            ui.icon(r, icon, 16.0, Some(t.text1));
            if !data.history.is_empty() {
                let n = data.history.len().min(9).to_string();
                ui.text(Rect::new(r.cx() + 2.0, r.y + 8.0, 14.0, 12.0), &n, &Text::new(9.0, t.text1).bold());
            }
            ui.hit(key, r, Action::Toggle(Kind::NotifCenter), Role::Button, "Action Center");
            // Show-desktop sliver.
            gfx::fill_rounded(ui.cr, Rect::new(r.right() - 1.0, r.y + 8.0, 1.0, r.h - 16.0), 0.0, t.text3);
        }
        "taskbar" => {
            let items = super::bar::taskbar_items(ui, data);
            for (i, (id, icon, wins)) in items.iter().enumerate() {
                let c = Rect::new(r.x + i as f64 * W, r.y, W, r.h);
                let k = ui::idn("w10.task", id);
                let active = wins.iter().any(|a| *a == data.active_addr);
                if active {
                    gfx::fill_rounded(ui.cr, c, 0.0, if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.15) } else { Color::rgba(1.0, 1.0, 1.0, 0.7) });
                }
                flat(ui, &t, c, k);
                ui.icon(c, icon, 24.0, None);
                if !wins.is_empty() {
                    let line = if active { c.inset_xy(0.0, 0.0) } else { c.inset_xy(4.0, 0.0) };
                    gfx::fill_rounded(ui.cr, Rect::new(line.x, c.bottom() - 2.0, line.w, 2.0), 0.0, t.accent.mix(Color::WHITE, 0.35));
                }
                let action = match wins.as_slice() {
                    [] => Action::Launch(id.clone()),
                    [one] if *one != data.active_addr => Action::FocusWindow(one.clone()),
                    [one] => Action::Hypr(format!("hl.dsp.window.move({{ workspace = \"special:minimized\", window = \"address:{one}\" }})")),
                    many => {
                        let idx = many.iter().position(|a| *a == data.active_addr).map(|i| (i + 1) % many.len()).unwrap_or(0);
                        Action::FocusWindow(many[idx].clone())
                    }
                };
                let hit = ui.hit(k, c, action, Role::Button, Data::pretty_class(id));
                hit.secondary = Some(Action::Launch(id.clone()));
            }
        }
        _ => return false,
    }
    true
}

// ───────────────────────────── Start ─────────────────────────────

pub fn start(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let (_, sh) = ui.size;
    let bar = ui.th.cfg.bar.size as f64;
    let h = 640.0f64.min(sh - bar);
    let panel = Rect::new(0.0, sh - bar - h, 704.0, h);
    super::popup_transform(ui, panel, t_open, false);
    gfx::shadow(ui.cr, panel, 0.0, 12.0, 0.0, 0.35);
    gfx::fill_rounded(ui.cr, panel, 0.0, surface(&t));
    ui.record("launcher.panel", panel, None);
    ui.hit(ui::id("launcher.panel"), panel, Action::None, Role::Group, "");
    // Rail
    let rail = Rect::new(panel.x, panel.y, W, panel.h);
    let rail_top = [("view-more-horizontal-symbolic", "Expand", Action::None)];
    let rail_bottom: [(&str, &str, Action); 5] = [
        ("avatar-default-symbolic", "Account", Action::Exec("creed-settings".into())),
        ("folder-documents-symbolic", "Documents", Action::Exec(format!("{} ~/Documents", ui.th.cfg.general.file_manager))),
        ("image-x-generic-symbolic", "Pictures", Action::Exec(format!("{} ~/Pictures", ui.th.cfg.general.file_manager))),
        ("emblem-system-symbolic", "Settings", Action::Exec("creed-settings".into())),
        ("system-shutdown-symbolic", "Power", Action::Open(Kind::Power)),
    ];
    for (i, (icon, label, a)) in rail_top.into_iter().enumerate() {
        let r = Rect::new(rail.x, rail.y + i as f64 * W, W, W);
        let k = ui::idn("w10.rail.t", i);
        flat(ui, &t, r, k);
        ui.text(r, "≡", &Text::new(18.0, t.text1).center());
        let _ = icon;
        ui.hit(k, r, a, Role::Button, label);
    }
    for (i, (icon, label, a)) in rail_bottom.into_iter().enumerate() {
        let r = Rect::new(rail.x, rail.bottom() - (5 - i) as f64 * W, W, W);
        let k = ui::idn("w10.rail", i);
        flat(ui, &t, r, k);
        ui.icon(r, icon, 16.0, Some(t.text1));
        ui.hit(k, r, a, Role::Button, label);
    }
    // App list (alphabetical with letter headers) or search results.
    let list = Rect::new(panel.x + W + 4.0, panel.y + 8.0, 260.0, panel.h - 16.0);
    if !views.query.is_empty() {
        super::launcher::draw_results_list_pub(ui, list, data, views);
    } else {
        let from = ui.hits.len();
        let mut rows: Vec<(Option<char>, usize)> = vec![];
        let mut last = None;
        for (i, a) in data.apps.iter().enumerate() {
            let l = a.name.chars().next().map(|c| c.to_ascii_uppercase()).filter(|c| c.is_ascii_alphabetic()).unwrap_or('#');
            if Some(l) != last {
                rows.push((Some(l), i));
                last = Some(l);
            }
            rows.push((None, i));
        }
        let row_h = 36.0;
        let total = rows.len() as f64 * row_h;
        views.scroll = views.scroll.clamp(0.0, (total - list.h).max(0.0));
        let scroll = views.scroll;
        super::with_clip(ui, list, |ui| {
            for (n, (hdr, ai)) in rows.iter().enumerate() {
                let r = Rect::new(list.x, list.y + n as f64 * row_h - scroll, list.w, row_h);
                if r.bottom() < list.y || r.y > list.bottom() {
                    continue;
                }
                match hdr {
                    Some(l) => {
                        ui.text(Rect::new(r.x + 12.0, r.y, r.w - 12.0, r.h), &l.to_string(), &Text::new(BODY, t.text1));
                    }
                    None => {
                        let a = &data.apps[*ai];
                        let k = ui::idn("w10.app", &a.id);
                        flat(ui, &t, r, k);
                        ui.icon(Rect::new(r.x + 8.0, r.y, 24.0, r.h), &a.icon, 24.0, None);
                        ui.text(Rect::new(r.x + 44.0, r.y, r.w - 52.0, r.h), &a.name, &Text::new(BODY, t.text1));
                        ui.hit(k, r, Action::Launch(a.id.clone()), Role::Item, a.name.clone());
                    }
                }
            }
        });
        super::clip_hits(ui, from, list);
    }
    // Live tiles: medium tiles 100×100, 4 px gutters, three per row.
    let tiles = Rect::new(list.right() + 24.0, panel.y + 12.0, panel.right() - list.right() - 36.0, panel.h - 24.0);
    ui.text(Rect::new(tiles.x, tiles.y, tiles.w, 32.0), "Productivity", &Text::new(CAPTION, t.text1));
    let mut pinned: Vec<&crate::data::AppEntry> = ui.th.cfg.dock.pinned.iter().filter_map(|p| data.apps.iter().find(|a| &a.id == p)).collect();
    for a in &data.apps {
        if pinned.len() >= 9 {
            break;
        }
        if !pinned.iter().any(|x| x.id == a.id) {
            pinned.push(a);
        }
    }
    for (i, a) in pinned.iter().enumerate().take(9) {
        let r = Rect::new(tiles.x + (i % 3) as f64 * 104.0, tiles.y + 36.0 + (i / 3) as f64 * 104.0, 100.0, 100.0);
        let k = ui::idn("w10.tile", &a.id);
        let hv = ui.hover_amt(k);
        gfx::fill_rounded(ui.cr, r, 0.0, t.accent);
        if hv > 0.01 {
            gfx::stroke_rounded(ui.cr, r, 0.0, 2.0, Color::rgba(1.0, 1.0, 1.0, 0.5 * hv));
        }
        ui.icon(Rect::new(r.x, r.y, r.w, r.h - 16.0), &a.icon, 32.0, None);
        ui.text(Rect::new(r.x + 8.0, r.bottom() - 24.0, r.w - 16.0, 18.0), &a.name, &Text::new(CAPTION, Color::WHITE));
        ui.hit(k, r, Action::Launch(a.id.clone()), Role::Button, a.name.clone());
    }
}

// ───────────────────────────── Action Center ─────────────────────────────

pub fn action_center(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let (sw, sh) = ui.size;
    let bar = ui.th.cfg.bar.size as f64;
    let panel = Rect::new(sw - 396.0, 0.0, 396.0, sh - bar);
    super::popup_transform(ui, panel, t_open, false);
    gfx::fill_rounded(ui.cr, panel, 0.0, surface(&t));
    gfx::fill_rounded(ui.cr, Rect::new(panel.x, panel.y, 1.0, panel.h), 0.0, hover(&t));
    ui.record("control.panel", panel, None);
    ui.hit(ui::id("ac.panel"), panel, Action::None, Role::Group, "");
    // Header link
    let mk = ui::id("ac.manage");
    let ml = Rect::new(panel.right() - 180.0, panel.y + 12.0, 164.0, 20.0);
    ui.text(ml, "Manage notifications", &Text::new(CAPTION, t.accent.mix(Color::WHITE, 0.3)).end());
    ui.hit(mk, ml, Action::Exec("creed-settings --page desktop".into()), Role::Button, "Manage notifications");
    // Bottom: expand link, 4×2 quick actions, brightness.
    let tiles: Vec<(&str, &str, bool, Action)> = vec![
        ("network-wireless-symbolic", "Network", data.net.wifi_enabled, Action::ToggleWifi),
        ("bluetooth-active-symbolic", "Bluetooth", data.bt.as_ref().map(|b| b.powered).unwrap_or(false), Action::ToggleBluetooth),
        ("notifications-disabled-symbolic", "Focus assist", data.dnd, Action::ToggleDnd),
        ("night-light-symbolic", "Night light", data.night_light, Action::ToggleNightLight),
        ("input-gaming-symbolic", "Game Mode", data.game, Action::ToggleGame),
        ("power-profile-power-saver-symbolic", "Battery saver", data.power_profile.as_deref() == Some("power-saver"), Action::PowerProfile(if data.power_profile.as_deref() == Some("power-saver") { "balanced".into() } else { "power-saver".into() })),
        ("view-dual-symbolic", "Snap", false, Action::Open(Kind::Snap)),
        ("emblem-system-symbolic", "All settings", false, Action::Exec("creed-settings".into())),
    ];
    let tw = ((panel.w - 24.0 - 6.0) / 4.0).floor();
    let th_ = 60.0;
    let slider_h = if data.brightness.is_some() { 48.0 } else { 0.0 };
    let grid_top = panel.bottom() - 16.0 - slider_h - 2.0 * th_ - 2.0;
    for (i, (icon, label, on, action)) in tiles.into_iter().enumerate() {
        let r = Rect::new(panel.x + 12.0 + (i % 4) as f64 * (tw + 2.0), grid_top + (i / 4) as f64 * (th_ + 2.0), tw, th_);
        let k = ui::idn("ac.tile", i);
        let hv = ui.hover_amt(k);
        let base = if on { t.accent } else if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.12) } else { Color::rgba(0.0, 0.0, 0.0, 0.08) };
        gfx::fill_rounded(ui.cr, r, 0.0, base);
        if hv > 0.01 {
            gfx::stroke_rounded(ui.cr, r, 0.0, 2.0, if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.4 * hv) } else { Color::rgba(0.0, 0.0, 0.0, 0.3 * hv) });
        }
        let fg = if on { Color::WHITE } else { t.text1 };
        ui.icon(Rect::new(r.x + 10.0, r.y + 8.0, 16.0, 16.0), icon, 16.0, Some(fg));
        ui.text(Rect::new(r.x + 10.0, r.bottom() - 26.0, r.w - 14.0, 20.0), label, &Text::new(CAPTION, fg));
        let hit = ui.hit(k, r, action, Role::Toggle, label.to_string());
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
    let link = Rect::new(panel.x + 12.0, grid_top - 28.0, panel.w - 24.0, 20.0);
    ui.text(link, "Collapse", &Text::new(CAPTION, t.text1));
    if let Some(b) = data.brightness {
        let sr = Rect::new(panel.x + 44.0, panel.bottom() - 16.0 - slider_h + 4.0, panel.w - 64.0, 40.0);
        ui.icon(Rect::new(panel.x + 12.0, sr.y, 24.0, sr.h), "display-brightness-symbolic", 16.0, Some(t.text1));
        let k = ui::id("ac.bright");
        let hv = ui.hover_amt(k);
        fluent::slider(ui.cr, &t, sr, b, hv);
        let hit = ui.hit(k, sr, Action::Slider(Slider::Brightness), Role::Slider, "Brightness");
        hit.value = Some(b);
    }
    // Notifications above
    let list = Rect::new(panel.x + 12.0, panel.y + 44.0, panel.w - 24.0, link.y - panel.y - 56.0);
    if data.history.is_empty() {
        ui.text(Rect::new(list.x, list.y + 20.0, list.w, 24.0), "No new notifications", &Text::new(BODY, t.text1).center());
        return;
    }
    let from = ui.hits.len();
    let scroll = views.scroll;
    super::with_clip(ui, list, |ui| {
        let mut y = list.y - scroll;
        for n in &data.history {
            let h = toast_height(n);
            toast_card(ui, &t, Rect::new(list.x, y, list.w, h), n, true);
            y += h + 2.0;
        }
    });
    super::clip_hits(ui, from, list);
    let ck = ui::id("ac.clear");
    let cl = Rect::new(panel.right() - 200.0, link.y, 188.0, 20.0);
    ui.text(cl, "Clear all notifications", &Text::new(CAPTION, t.accent.mix(Color::WHITE, 0.3)).end());
    ui.hit(ck, cl, Action::NotifClear, Role::Button, "Clear all notifications");
}

// ───────────────────────────── toasts ─────────────────────────────

pub fn toast_height(n: &crate::data::Notif) -> f64 {
    let lines = if n.body.is_empty() { 0.0 } else { ((n.body.chars().count() as f64) / 42.0).ceil().clamp(1.0, 3.0) };
    (36.0 + 20.0 + lines * 20.0 + 14.0).round()
}

fn toast_card(ui: &mut Ui, t: &Tok, r: Rect, n: &crate::data::Notif, in_center: bool) {
    let k = ui::idn("w10.toast", n.id);
    let hv = ui.hover_amt(k);
    if !in_center {
        gfx::shadow(ui.cr, r, 0.0, 10.0, 2.0, 0.35);
    }
    gfx::fill_rounded(ui.cr, r, 0.0, if in_center { if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.06 + 0.04 * hv) } else { Color::rgba(1.0, 1.0, 1.0, 0.6) } } else { surface(t) });
    let icon = if n.icon.is_empty() { "dialog-information-symbolic".to_string() } else { n.icon.clone() };
    ui.icon(Rect::new(r.x + 12.0, r.y + 10.0, 16.0, 16.0), &icon, 16.0, if icon.ends_with("-symbolic") { Some(t.accent) } else { None });
    ui.text(Rect::new(r.x + 36.0, r.y + 8.0, r.w - 80.0, 20.0), &n.app, &Text::new(CAPTION, t.text2));
    ui.text(Rect::new(r.x + 12.0, r.y + 36.0, r.w - 24.0, 20.0), &n.summary, &Text::new(BODY, t.text1).bold());
    if !n.body.is_empty() {
        ui.text(Rect::new(r.x + 12.0, r.y + 58.0, r.w - 24.0, 60.0), &super::notif::strip_markup(&n.body), &Text::new(BODY, t.text2).lines(3));
    }
    let default = n.actions.iter().find(|(k, _)| k == "default").map(|(k, _)| Action::NotifAction(n.id, k.clone())).unwrap_or(Action::NotifDismiss(n.id));
    ui.hit(k, r, default, Role::Item, format!("Notification from {}: {}", n.app, n.summary));
    if hv > 0.05 {
        let x = Rect::new(r.right() - 32.0, r.y + 4.0, 28.0, 28.0);
        ui.icon(x, "window-close-symbolic", 12.0, Some(t.text1.with_alpha(t.text1.a * hv)));
        ui.hit(ui::idn("w10.toast.x", n.id), x, Action::NotifDismiss(n.id), Role::Button, "Dismiss");
    }
}

pub fn toasts(ui: &mut Ui, data: &Data) {
    let t = tok(ui.th);
    let (sw, sh) = ui.size;
    let mut y = sh - 12.0;
    for n in &data.notifs {
        let h = toast_height(n);
        let a = ui.spring(ui::idn("w10.toast.in", n.id), 1.0, 0.0);
        let r = Rect::new(sw - 364.0 - 12.0, y - h, 364.0, h);
        ui.cr.save().ok();
        ui.cr.push_group();
        ui.cr.translate(((1.0 - a) * 380.0).round(), 0.0);
        toast_card(ui, &t, r, n, false);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint();
        ui.cr.restore().ok();
        ui.record(&format!("notif.{}", n.id), r, None);
        y -= h + 12.0;
    }
}

// ───────────────────────────── OSD (top-left, vertical) ─────────────────────────────

pub const OSD_SIZE: (u32, u32) = (65, 140);

pub fn osd(ui: &mut Ui, kind: crate::data::OsdKind, value: f64) {
    let t = tok(ui.th);
    let (w, h) = ui.size;
    let r = Rect::new(0.0, 0.0, w, h);
    gfx::fill_rounded(ui.cr, r, 0.0, surface(&t));
    if kind == crate::data::OsdKind::Game {
        ui.icon(Rect::new(r.x, r.y + 30.0, r.w, 40.0), "input-gaming-symbolic", 24.0, Some(t.text1));
        ui.text(Rect::new(r.x, r.y + 80.0, r.w, 20.0), if value > 0.5 { "On" } else { "Off" }, &Text::new(BODY, t.text1).center());
        return;
    }
    let v = ui.spring(ui::id("w10.osd"), value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    ui.text(Rect::new(r.x, r.y + 8.0, r.w, 20.0), &format!("{:.0}", v * 100.0), &Text::new(BODY, t.text1).center());
    let track = Rect::new(r.cx() - 3.0, r.y + 36.0, 6.0, r.h - 80.0);
    gfx::fill_rounded(ui.cr, track, 0.0, if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.3) } else { Color::rgba(0.0, 0.0, 0.0, 0.25) });
    let fh = track.h * v;
    gfx::fill_rounded(ui.cr, Rect::new(track.x, track.bottom() - fh, 6.0, fh), 0.0, t.accent);
    gfx::fill_rounded(ui.cr, Rect::centered(r.cx(), track.bottom() - fh, 18.0, 6.0), 0.0, t.text1);
    let icon = match kind {
        crate::data::OsdKind::Brightness => "display-brightness-symbolic",
        crate::data::OsdKind::Game => "input-gaming-symbolic",
        crate::data::OsdKind::Mic if value <= 0.0 => "microphone-disabled-symbolic",
        crate::data::OsdKind::Mic => "audio-input-microphone-symbolic",
        _ if value <= 0.0 => "audio-volume-muted-symbolic",
        _ => "audio-volume-high-symbolic",
    };
    ui.icon(Rect::new(r.x, r.bottom() - 36.0, r.w, 28.0), icon, 16.0, Some(t.text1));
}
