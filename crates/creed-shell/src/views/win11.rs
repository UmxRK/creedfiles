//! Windows 11 (23H2/24H2) surfaces, built to the shipping layout:
//! taskbar 48 px (40 px buttons, 24 px icons, 3 px running pill 6/16 px), Start 642×726,
//! Quick Settings 360 wide, Notification Center + calendar, toasts 364 wide, volume pill,
//! Task View. Colours, radii and type ramp come from `fluent`.

use super::fluent::{self, tok, Tok, BODY, CAPTION, CONTROL_RADIUS, OVERLAY_RADIUS};
use super::Views;
use crate::data::{Data, NetKind};
use crate::gfx::{self, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Slider, Ui};
use creed_core::Color;

const SLOT: f64 = 44.0;
const BTN: f64 = 40.0;

// ───────────────────────────── taskbar ─────────────────────────────

pub fn bar_background(ui: &mut Ui, full: Rect) {
    let t = tok(ui.th);
    gfx::fill_rounded(ui.cr, full, 0.0, t.mica);
    gfx::fill_rounded(ui.cr, Rect::new(full.x, full.y, full.w, 1.0), 0.0, if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.08) } else { Color::rgba(0.0, 0.0, 0.0, 0.08) });
}

fn status_icons(data: &Data) -> Vec<String> {
    let mut v = vec![];
    v.push(match data.net.kind {
        NetKind::Wifi => {
            let s = data.net.signal;
            format!("network-wireless-signal-{}-symbolic", if s > 75 { "excellent" } else if s > 50 { "good" } else if s > 25 { "ok" } else { "weak" })
        }
        NetKind::Ethernet => "network-wired-symbolic".into(),
        NetKind::None => "network-offline-symbolic".into(),
    });
    v.push(match &data.audio {
        Some(a) if a.muted || a.volume <= 0.001 => "audio-volume-muted-symbolic".into(),
        Some(a) if a.volume < 0.34 => "audio-volume-low-symbolic".into(),
        Some(a) if a.volume < 0.67 => "audio-volume-medium-symbolic".into(),
        Some(_) => "audio-volume-high-symbolic".into(),
        None => "audio-volume-muted-symbolic".into(),
    });
    if let Some(b) = &data.battery {
        v.push(super::bar::battery_icon(b.percent, b.charging));
    }
    v
}

pub fn module_len(ui: &mut Ui, name: &str, data: &Data) -> Option<f64> {
    let t = tok(ui.th);
    Some(match name {
        "launcher" | "taskview" | "power" => SLOT,
        "search" => 212.0,
        "widgets" => match &data.weather {
            Some(w) => {
                let a = ui.measure(&w.temp, &Text::new(CAPTION, t.text1)).0;
                let b = ui.measure(&w.desc, &Text::new(CAPTION, t.text2)).0;
                (24.0 + 8.0 + a.max(b) + 16.0).min(160.0)
            }
            None => SLOT,
        },
        "network" => 8.0 + status_icons(data).len() as f64 * 24.0 + 8.0,
        "volume" | "battery" | "bluetooth" => 0.0,
        "clock" => {
            let a = ui.measure(&super::bar::clock_text(ui), &Text::new(CAPTION, t.text1)).0;
            let b = ui.measure(&time::format(time::now_secs(), &ui.th.cfg.bar.date_format), &Text::new(CAPTION, t.text1)).0;
            a.max(b) + 24.0
        }
        "notifications" => 36.0,
        "keyboard" => {
            if data.kb_layout.is_empty() {
                0.0
            } else {
                let (a, b) = lang_lines(&data.kb_layout);
                ui.measure(&a, &Text::new(CAPTION, t.text1)).0.max(ui.measure(&b, &Text::new(CAPTION, t.text1)).0) + 20.0
            }
        }
        "tray" => {
            if data.tray.is_empty() {
                0.0
            } else {
                SLOT * 0.8 + data.tray.len() as f64 * 32.0
            }
        }
        "taskbar" => super::bar::taskbar_items(ui, data).len() as f64 * SLOT,
        _ => return None,
    })
}

/// Windows 11 calendar flyout grid: month title on the left, ▲ ▼ on the right, six full weeks
/// with the neighbouring months' days dimmed and today in an accent circle.
fn month(ui: &mut Ui, t: &Tok, r: Rect, offset: i32) {
    let now = time::local(time::now_secs());
    let (year, mon) = time::add_months(now.year, now.month, offset);
    let (hdr, rest) = r.take_top(36.0, 6.0);
    let tk = ui::id("cal.title");
    let hv = ui.hover_amt(tk);
    let title = Rect::new(hdr.x, hdr.y + 2.0, hdr.w - 80.0, hdr.h - 4.0);
    fluent::subtle(ui.cr, t, title, CONTROL_RADIUS, hv * if offset != 0 { 1.0 } else { 0.0 }, false);
    ui.text(title.inset_xy(8.0, 0.0), &format!("{} {}", time::MONTHS[(mon - 1) as usize], year), &Text::new(BODY, t.text1).bold());
    ui.hit(tk, title, Action::Calendar(0), Role::Button, "Back to today");
    for (i, (icon, label, d)) in [("go-up-symbolic", "Previous month", -1), ("go-down-symbolic", "Next month", 1)].into_iter().enumerate() {
        let b = Rect::new(hdr.right() - 76.0 + i as f64 * 38.0, hdr.cy() - 16.0, 36.0, 32.0);
        let k = ui::id(if d < 0 { "cal.prev" } else { "cal.next" });
        let hv = ui.hover_amt(k);
        fluent::subtle(ui.cr, t, b, CONTROL_RADIUS, hv, ui.is_pressed(k));
        ui.icon(b, icon, 14.0, Some(t.text1));
        ui.hit(k, b, Action::Calendar(d), Role::Button, label);
    }
    let (dow, grid) = rest.take_top(26.0, 2.0);
    let names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cw = dow.w / 7.0;
    for (i, n) in names.iter().enumerate() {
        ui.text(Rect::new(dow.x + i as f64 * cw, dow.y, cw, dow.h), n, &Text::new(CAPTION, t.text1).center());
    }
    let first = ((time::weekday(year, mon, 1) + 6) % 7) as i64;
    let days = time::days_in_month(year, mon) as i64;
    let (py, pm) = time::add_months(year, mon, -1);
    let prev_days = time::days_in_month(py, pm) as i64;
    let ch = (grid.h / 6.0).min(cw);
    for i in 0..42i64 {
        let cell = Rect::new(grid.x + (i % 7) as f64 * cw, grid.y + (i / 7) as f64 * ch, cw, ch);
        let d = i - first + 1;
        let (label, inside) = if d < 1 { (prev_days + d, false) } else if d > days { (d - days, false) } else { (d, true) };
        let today = inside && offset == 0 && d == now.day as i64;
        if today {
            let s = (ch.min(cw) - 4.0).min(36.0);
            gfx::fill_rounded(ui.cr, Rect::centered(cell.cx(), cell.cy(), s, s), s / 2.0, t.accent);
        }
        let col = if today { t.on_accent } else if inside { t.text1 } else { t.text3 };
        ui.text(cell, &label.to_string(), &Text::new(CAPTION, col).center());
    }
}

/// Windows' two-line input indicator from a layout name: "English (UK)" → ("ENG", "UK"),
/// "us" → ("ENG", "US"), "German" → ("DEU", "").
pub fn lang_lines(layout: &str) -> (String, String) {
    const LANGS: &[(&str, &str)] = &[
        ("english", "ENG"), ("german", "DEU"), ("french", "FRA"), ("spanish", "ESP"), ("italian", "ITA"),
        ("portuguese", "POR"), ("russian", "РУС"), ("polish", "POL"), ("dutch", "NLD"), ("swedish", "SWE"),
        ("norwegian", "NOR"), ("danish", "DAN"), ("finnish", "FIN"), ("turkish", "TRK"), ("japanese", "JPN"),
        ("korean", "KOR"), ("chinese", "CHN"),
    ];
    let l = layout.trim();
    let lower = l.to_lowercase();
    let region = l.split_once('(').and_then(|(_, r)| r.split(')').next()).map(|r| r.trim().to_uppercase()).filter(|r| r.len() <= 4).unwrap_or_default();
    if let Some((_, code)) = LANGS.iter().find(|(k, _)| lower.starts_with(k)) {
        return (code.to_string(), region);
    }
    // Bare xkb codes.
    match lower.as_str() {
        "us" => ("ENG".into(), "US".into()),
        "gb" | "uk" => ("ENG".into(), "UK".into()),
        _ => (l.chars().take(3).collect::<String>().to_uppercase(), region),
    }
}

#[cfg(test)]
mod lang_tests {
    #[test]
    fn indicator() {
        assert_eq!(super::lang_lines("English (UK)"), ("ENG".into(), "UK".into()));
        assert_eq!(super::lang_lines("English (US)"), ("ENG".into(), "US".into()));
        assert_eq!(super::lang_lines("us"), ("ENG".into(), "US".into()));
        assert_eq!(super::lang_lines("German"), ("DEU".into(), "".into()));
        assert_eq!(super::lang_lines("Czech"), ("CZE".into(), "".into()));
    }
}

/// Draws one Windows 11 taskbar module; returns false if the module is not a Win11 one.
pub fn draw_module(ui: &mut Ui, name: &str, r: Rect, data: &Data) -> bool {
    let t = tok(ui.th);
    let key = ui::id(&format!("w11.{name}"));
    let btn = |r: Rect| Rect::centered(r.cx(), r.cy(), r.w - 4.0, BTN);
    match name {
        "launcher" | "taskview" | "power" => {
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            let (icon, label, action, tint) = match name {
                "launcher" => ("view-app-grid-symbolic", "Start", Action::Toggle(Kind::Launcher), t.accent),
                "taskview" => ("view-dual-symbolic", "Task View", Action::Toggle(Kind::Overview), t.text1),
                _ => ("system-shutdown-symbolic", "Power", Action::Toggle(Kind::Power), t.text1),
            };
            let s = if ui.is_pressed(key) { 20.0 } else { 24.0 };
            ui.icon(b, icon, s, Some(tint));
            ui.hit(key, r, action, Role::Button, label);
        }
        "search" => {
            let b = Rect::centered(r.cx(), r.cy(), r.w - 8.0, 32.0);
            let hv = ui.hover_amt(key);
            gfx::fill_rounded(ui.cr, b, 16.0, if hv > 0.5 { t.ctl_fill2 } else { t.ctl_fill });
            gfx::stroke_rounded(ui.cr, b, 16.0, 1.0, t.ctl_stroke);
            ui.icon(Rect::new(b.x + 10.0, b.y, 20.0, b.h), "system-search-symbolic", 16.0, Some(t.text1));
            ui.text(Rect::new(b.x + 38.0, b.y, b.w - 46.0, b.h), "Search", &Text::new(BODY, t.text2));
            ui.hit(key, r, Action::Toggle(Kind::Launcher), Role::Button, "Search");
        }
        "widgets" => {
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            match &data.weather {
                Some(w) => {
                    ui.icon(Rect::new(b.x + 6.0, b.y, 24.0, b.h), &w.icon, 24.0, Some(t.accent));
                    let tx = Rect::new(b.x + 38.0, b.y + 3.0, b.w - 42.0, 17.0);
                    ui.text(tx, &w.temp, &Text::new(CAPTION, t.text1));
                    ui.text(Rect::new(tx.x, tx.y + 16.0, tx.w, 17.0), &w.desc, &Text::new(CAPTION, t.text2));
                }
                None => ui.icon(b, "weather-few-clouds-symbolic", 24.0, Some(t.accent)),
            }
            ui.hit(key, r, Action::Toggle(Kind::Dashboard), Role::Button, "Widgets");
        }
        "network" => {
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            for (i, icon) in status_icons(data).iter().enumerate() {
                ui.icon(Rect::new(b.x + 6.0 + i as f64 * 24.0, b.y, 20.0, b.h), icon, 16.0, Some(t.text1));
            }
            let hit = ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, "Quick settings");
            hit.scroll = Some(|d| Action::VolumeStep(d * 0.02));
        }
        "clock" => {
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            let now = time::now_secs();
            let tr = Rect::new(b.x, b.y + 3.0, b.w - 12.0, 17.0);
            ui.text(tr, &super::bar::clock_text(ui), &Text::new(CAPTION, t.text1).end());
            ui.text(Rect::new(tr.x, tr.y + 17.0, tr.w, 17.0), &time::format(now, &ui.th.cfg.bar.date_format), &Text::new(CAPTION, t.text1).end());
            ui.hit(key, r, Action::Toggle(Kind::NotifCenter), Role::Button, format!("Clock {}", time::format(now, "%A %d %B %H:%M")));
        }
        "keyboard" => {
            if data.kb_layout.is_empty() {
                return true;
            }
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            let (a, l2) = lang_lines(&data.kb_layout);
            ui.text(Rect::new(b.x, b.y + 3.0, b.w, 17.0), &a, &Text::new(CAPTION, t.text1).center());
            ui.text(Rect::new(b.x, b.y + 20.0, b.w, 17.0), &l2, &Text::new(CAPTION, t.text1).center());
            ui.hit(key, r, Action::Exec("hyprctl switchxkblayout all next".into()), Role::Button, format!("Input language {}", data.kb_layout));
        }
        "notifications" => {
            let b = btn(r);
            let hv = ui.hover_amt(key);
            fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(key));
            let icon = if data.dnd { "notifications-disabled-symbolic" } else { "preferences-system-notifications-symbolic" };
            ui.icon(b, icon, 16.0, Some(t.text1));
            if !data.history.is_empty() && !data.dnd {
                let n = data.history.len().min(99).to_string();
                let badge = Rect::new(b.right() - 15.0, b.y + 5.0, 14.0_f64.max(ui.measure(&n, &Text::new(10.0, t.on_accent)).0 + 6.0), 14.0);
                gfx::fill_rounded(ui.cr, badge, 7.0, t.accent);
                ui.text(badge, &n, &Text::new(10.0, t.on_accent).bold().center());
            }
            ui.hit(key, r, Action::Toggle(Kind::NotifCenter), Role::Button, "Notifications");
        }
        "tray" => {
            if data.tray.is_empty() {
                return true;
            }
            let chev = Rect::new(r.x, r.y, SLOT * 0.8, r.h);
            let ck = ui::id("w11.tray.chevron");
            let hv = ui.hover_amt(ck);
            fluent::subtle(ui.cr, &t, btn(chev), CONTROL_RADIUS, hv, false);
            ui.icon(chev, "go-up-symbolic", 16.0, Some(t.text1));
            ui.hit(ck, chev, Action::None, Role::Button, "Show hidden icons");
            for (i, item) in data.tray.iter().enumerate() {
                let c = Rect::new(chev.right() + i as f64 * 32.0, r.y, 32.0, r.h);
                let k = ui::idn("w11.tray", &item.key);
                let hv = ui.hover_amt(k);
                fluent::subtle(ui.cr, &t, Rect::centered(c.cx(), c.cy(), 30.0, BTN), CONTROL_RADIUS, hv, false);
                if let Some((pw, ph, px)) = &item.pixmap {
                    super::bar::draw_pixmap(ui.cr, Rect::centered(c.cx(), c.cy(), 16.0, 16.0), *pw, *ph, px);
                } else {
                    let icon = if item.icon.is_empty() { item.id.clone() } else { item.icon.clone() };
                    ui.icon(c, &icon, 16.0, Some(t.text1));
                }
                let primary = if item.item_is_menu { Action::TrayMenu(item.key.clone()) } else { Action::TrayActivate(item.key.clone()) };
                let hit = ui.hit(k, c, primary, Role::Button, if item.title.is_empty() { item.id.clone() } else { item.title.clone() });
                hit.secondary = Some(Action::TrayMenu(item.key.clone()));
            }
        }
        "taskbar" => {
            let items = super::bar::taskbar_items(ui, data);
            for (i, (id, icon, wins)) in items.iter().enumerate() {
                let c = Rect::new(r.x + i as f64 * SLOT, r.y, SLOT, r.h);
                let b = btn(c);
                let k = ui::idn("w11.task", id);
                let hv = ui.hover_amt(k);
                let active = wins.iter().any(|a| *a == data.active_addr);
                if active {
                    gfx::fill_rounded(ui.cr, b, CONTROL_RADIUS, t.ctl_fill2);
                    gfx::stroke_rounded(ui.cr, b, CONTROL_RADIUS, 1.0, t.ctl_stroke);
                } else {
                    fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(k));
                }
                let s = if ui.is_pressed(k) { 20.0 } else { 24.0 };
                ui.icon(b, icon, s, None);
                if !wins.is_empty() {
                    let w = ui.spring(k ^ 0x99, if active { 16.0 } else { 6.0 }, if active { 16.0 } else { 6.0 });
                    let ind = Rect::centered(b.cx(), b.bottom() - 2.5, w, 3.0);
                    gfx::fill_rounded(ui.cr, ind, 1.5, if active { t.accent } else { t.text3 });
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
                let label = Data::pretty_class(id);
                let hit = ui.hit(k, c, action, Role::Button, format!("{label}{}", if wins.is_empty() { "" } else { ", running" }));
                hit.secondary = Some(Action::Launch(id.clone()));
            }
        }
        _ => return false,
    }
    true
}

// ───────────────────────────── Start ─────────────────────────────

fn placement_above_taskbar(ui: &Ui, w: f64, h: f64, align_right: bool) -> Rect {
    let (sw, sh) = ui.size;
    let bar = if ui.th.cfg.bar.enabled { ui.th.cfg.bar.size as f64 } else { 0.0 };
    let h = h.min(sh - bar - 24.0);
    let x = if align_right { sw - w - 12.0 } else { ((sw - w) / 2.0).round() };
    Rect::new(x, sh - bar - 12.0 - h, w, h)
}

pub fn start(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let panel = placement_above_taskbar(ui, 642.0, 726.0, false);
    super::popup_transform(ui, panel, t_open, false);
    fluent::flyout(ui.cr, &t, panel, OVERLAY_RADIUS);
    ui.record("launcher.panel", panel, None);
    ui.hit(ui::id("launcher.panel"), panel, Action::None, Role::Group, "");
    // Search box
    let search = Rect::new(panel.x + 32.0, panel.y + 32.0, panel.w - 64.0, 36.0);
    gfx::fill_rounded(ui.cr, search, 18.0, if t.dark { Color::rgba(0.0, 0.0, 0.0, 0.3) } else { Color::rgba(1.0, 1.0, 1.0, 0.7) });
    gfx::stroke_rounded(ui.cr, search, 18.0, 1.0, t.ctl_stroke);
    gfx::fill_rounded(ui.cr, Rect::new(search.x + 18.0, search.bottom() - 1.0, search.w - 36.0, 1.0), 0.0, t.accent.with_alpha(0.0));
    ui.icon(Rect::new(search.x + 14.0, search.y, 16.0, search.h), "system-search-symbolic", 16.0, Some(t.text2));
    let q = format!("{}{}", views.query, views.preedit);
    let tr = Rect::new(search.x + 42.0, search.y, search.w - 56.0, search.h);
    if q.is_empty() {
        ui.text(Rect::new(tr.x + 4.0, tr.y, tr.w - 4.0, tr.h), "Search for apps, settings, and documents", &Text::new(BODY, t.text3));
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, search.cy() - 8.0, 1.2, 16.0), 0.0, t.text1);
    } else {
        let b = ui.text(tr, &q, &Text::new(BODY, t.text1));
        gfx::fill_rounded(ui.cr, Rect::new(b.right() + 1.0, search.cy() - 8.0, 1.2, 16.0), 0.0, t.text1);
    }
    ui.hit(ui::id("launcher.input"), search, Action::None, Role::Input, "Search for apps, settings, and documents");
    let footer_h = 64.0;
    let body = Rect::new(panel.x + 32.0, search.bottom() + 28.0, panel.w - 64.0, panel.bottom() - footer_h - search.bottom() - 28.0);
    if views.query.is_empty() {
        pinned_and_recommended(ui, &t, body, data);
    } else {
        super::launcher::draw_results_list_pub(ui, body, data, views);
    }
    // Footer: account + power on a slightly darker band.
    let foot = Rect::new(panel.x, panel.bottom() - footer_h, panel.w, footer_h);
    ui.cr.save().ok();
    gfx::rounded(ui.cr, panel, OVERLAY_RADIUS);
    ui.cr.clip();
    gfx::fill_rounded(ui.cr, foot, 0.0, if t.dark { Color::rgba(0.0, 0.0, 0.0, 0.18) } else { Color::rgba(0.0, 0.0, 0.0, 0.035) });
    ui.cr.restore().ok();
    gfx::fill_rounded(ui.cr, Rect::new(foot.x, foot.y, foot.w, 1.0), 0.0, t.divider);
    let user = Rect::new(foot.x + 52.0, foot.y + 12.0, 220.0, 40.0);
    let uk = ui::id("start.user");
    let hv = ui.hover_amt(uk);
    fluent::subtle(ui.cr, &t, Rect::new(user.x - 12.0, user.y, user.w, user.h), CONTROL_RADIUS, hv, false);
    let av = Rect::new(user.x, user.cy() - 16.0, 32.0, 32.0);
    gfx::fill_rounded(ui.cr, av, 16.0, t.ctl_fill2);
    ui.icon(av, "avatar-default-symbolic", 16.0, Some(t.text1));
    ui.text(Rect::new(av.right() + 12.0, user.y, user.w - 56.0, user.h), &data.user, &Text::new(CAPTION, t.text1));
    ui.hit(uk, user, Action::Exec("creed-settings".into()), Role::Button, format!("Account {}", data.user));
    // Folder shortcuts beside Power (Start → Personalization → Folders defaults on).
    let folders: [(&str, &str, &str); 5] = [
        ("text-x-generic-symbolic", "Documents", "xdg-open \"$(xdg-user-dir DOCUMENTS)\""),
        ("folder-download-symbolic", "Downloads", "xdg-open \"$(xdg-user-dir DOWNLOAD)\""),
        ("folder-pictures-symbolic", "Pictures", "xdg-open \"$(xdg-user-dir PICTURES)\""),
        ("system-file-manager-symbolic", "File Explorer", ""),
        ("emblem-system-symbolic", "Settings", "creed-settings"),
    ];
    for (i, (icon, label, cmd)) in folders.iter().enumerate() {
        let fr = Rect::new(foot.right() - 52.0 - 40.0 - (folders.len() - i) as f64 * 40.0, foot.cy() - 20.0, 40.0, 40.0);
        let fk = ui::idn("start.folder", i);
        let hv = ui.hover_amt(fk);
        fluent::subtle(ui.cr, &t, fr, CONTROL_RADIUS, hv, ui.is_pressed(fk));
        ui.icon(fr, icon, 16.0, Some(t.text1));
        let cmd = if cmd.is_empty() { ui.th.cfg.general.file_manager.clone() } else { cmd.to_string() };
        ui.hit(fk, fr, Action::Exec(cmd), Role::Button, *label);
    }
    let pw = Rect::new(foot.right() - 52.0 - 40.0, foot.cy() - 20.0, 40.0, 40.0);
    let pk = ui::id("start.power");
    let hv = ui.hover_amt(pk);
    fluent::subtle(ui.cr, &t, pw, CONTROL_RADIUS, hv, ui.is_pressed(pk));
    ui.icon(pw, "system-shutdown-symbolic", 16.0, Some(t.text1));
    ui.hit(pk, pw, Action::Open(Kind::Power), Role::Button, "Power");
}

fn section_header(ui: &mut Ui, t: &Tok, r: Rect, title: &str, button: &str, action: Action, key: &str) {
    ui.text(Rect::new(r.x + 18.0, r.y, r.w / 2.0, r.h), title, &Text::new(BODY, t.text1).bold());
    let bw = ui.measure(button, &Text::new(CAPTION, t.text1)).0 + 36.0;
    let b = Rect::new(r.right() - 18.0 - bw, r.cy() - 12.0, bw, 24.0);
    let k = ui::id(key);
    let hv = ui.hover_amt(k);
    fluent::button(ui.cr, t, b, hv, ui.is_pressed(k), false);
    ui.text(Rect::new(b.x + 10.0, b.y, b.w - 30.0, b.h), button, &Text::new(CAPTION, t.text1));
    ui.icon(Rect::new(b.right() - 22.0, b.y, 12.0, b.h), "go-next-symbolic", 10.0, Some(t.text1));
    ui.hit(k, b, action, Role::Button, button);
}

fn pinned_and_recommended(ui: &mut Ui, t: &Tok, body: Rect, data: &Data) {
    let (hdr, rest) = body.take_top(28.0, 12.0);
    section_header(ui, t, hdr, "Pinned", "All apps", Action::Exec("creed toggle launchpad".into()), "start.allapps");
    let cols = 6usize;
    let cell = Rect::new(rest.x, rest.y, (rest.w / cols as f64).floor(), 84.0);
    let mut pinned: Vec<&crate::data::AppEntry> = ui.th.cfg.dock.pinned.iter().filter_map(|p| data.apps.iter().find(|a| &a.id == p)).collect();
    for a in &data.apps {
        if pinned.len() >= 18 {
            break;
        }
        if !pinned.iter().any(|x| x.id == a.id) {
            pinned.push(a);
        }
    }
    for (i, a) in pinned.iter().enumerate().take(18) {
        let r = Rect::new(rest.x + (i % cols) as f64 * cell.w, rest.y + (i / cols) as f64 * cell.h, cell.w, cell.h).inset(2.0);
        let k = ui::idn("start.pin", &a.id);
        let hv = ui.hover_amt(k);
        fluent::subtle(ui.cr, t, r, CONTROL_RADIUS, hv, ui.is_pressed(k));
        ui.icon(Rect::new(r.x, r.y + 12.0, r.w, 32.0), &a.icon, 32.0, None);
        ui.text(Rect::new(r.x + 4.0, r.y + 52.0, r.w - 8.0, 18.0), &a.name, &Text::new(CAPTION, t.text1).center());
        ui.hit(k, r, Action::Launch(a.id.clone()), Role::Button, a.name.clone());
    }
    let rec_y = rest.y + 3.0 * cell.h + 24.0;
    let rec = Rect::new(body.x, rec_y, body.w, body.bottom() - rec_y);
    if rec.h < 60.0 {
        return;
    }
    let (hdr, list) = rec.take_top(28.0, 12.0);
    section_header(ui, t, hdr, "Recommended", "More", Action::Exec("xdg-open ~".into()), "start.more");
    if data.recent.is_empty() {
        ui.text(Rect::new(list.x + 18.0, list.y, list.w - 36.0, 40.0), "The more you use your device, the more we'll show your new apps and recent files here.", &Text::new(CAPTION, t.text2).lines(2));
        return;
    }
    let colw = (list.w / 2.0).floor();
    for (i, (path, name, when)) in data.recent.iter().take(6).enumerate() {
        let r = Rect::new(list.x + (i % 2) as f64 * colw, list.y + (i / 2) as f64 * 56.0, colw - 4.0, 52.0);
        if r.bottom() > list.bottom() {
            break;
        }
        let k = ui::idn("start.rec", i);
        let hv = ui.hover_amt(k);
        fluent::subtle(ui.cr, t, r, CONTROL_RADIUS, hv, ui.is_pressed(k));
        ui.icon(Rect::new(r.x + 14.0, r.y, 32.0, r.h), mime_icon(name), 32.0, None);
        ui.text(Rect::new(r.x + 58.0, r.y + 8.0, r.w - 66.0, 18.0), name, &Text::new(CAPTION, t.text1));
        ui.text(Rect::new(r.x + 58.0, r.y + 26.0, r.w - 66.0, 18.0), when, &Text::new(CAPTION, t.text2));
        ui.hit(k, r, Action::Exec(format!("xdg-open {}", crate::services::shell_quote(path))), Role::Item, name.clone());
    }
}

pub fn mime_icon(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("").to_lowercase().as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "svg" => "image-x-generic",
        "pdf" => "application-pdf",
        "mp3" | "flac" | "ogg" | "wav" => "audio-x-generic",
        "mp4" | "mkv" | "webm" | "mov" => "video-x-generic",
        "zip" | "tar" | "gz" | "xz" | "7z" => "package-x-generic",
        "odt" | "doc" | "docx" => "x-office-document",
        "ods" | "xls" | "xlsx" | "csv" => "x-office-spreadsheet",
        _ => "text-x-generic",
    }
}

// ───────────────────────────── Quick Settings ─────────────────────────────

pub fn quick_settings(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let rows = 2usize;
    let sliders = data.brightness.is_some() as usize + data.audio.is_some() as usize;
    let top_h = 24.0 + rows as f64 * 84.0 + 8.0 + sliders as f64 * 48.0 + 12.0;
    let h = top_h + 48.0;
    let panel = placement_above_taskbar(ui, 360.0, h, true);
    super::popup_transform(ui, panel, t_open, false);
    fluent::flyout(ui.cr, &t, panel, OVERLAY_RADIUS);
    ui.record("control.panel", panel, None);
    ui.hit(ui::id("control.panel"), panel, Action::None, Role::Group, "");
    if views.page == 1 {
        super::control::wifi_page_pub(ui, panel.inset(16.0), data, views);
        return;
    }
    // Button grid: 3 columns of 96×48, 12 px gutters, 24 px margins; label 8 px below.
    let wifi_on = data.net.wifi_enabled && data.net.available;
    let bt_on = data.bt.as_ref().map(|b| b.powered).unwrap_or(false);
    let tiles: Vec<(&str, &str, bool, Action, Option<Action>)> = vec![
        ("network-wireless-symbolic", if data.net.kind == NetKind::Wifi { data.net.name.as_str() } else { "Wi-Fi" }, wifi_on, Action::ToggleWifi, Some(Action::Page(1))),
        ("bluetooth-active-symbolic", "Bluetooth", bt_on, Action::ToggleBluetooth, None),
        ("notifications-disabled-symbolic", "Do not disturb", data.dnd, Action::ToggleDnd, None),
        ("night-light-symbolic", "Night light", data.night_light, Action::ToggleNightLight, None),
        ("input-gaming-symbolic", "Game Mode", data.game, Action::ToggleGame, None),
        ("power-profile-power-saver-symbolic", "Energy saver", data.power_profile.as_deref() == Some("power-saver"), Action::PowerProfile(if data.power_profile.as_deref() == Some("power-saver") { "balanced".into() } else { "power-saver".into() }), None),
    ];
    for (i, (icon, label, on, action, more)) in tiles.into_iter().enumerate() {
        let col = (i % 3) as f64;
        let row = (i / 3) as f64;
        let b = Rect::new(panel.x + 24.0 + col * (96.0 + 12.0), panel.y + 24.0 + row * 84.0, 96.0, 48.0);
        let k = ui::idn("w11.qs", i);
        let hv = ui.hover_amt(k);
        let split = more.is_some();
        let main = if split { Rect::new(b.x, b.y, 64.0, b.h) } else { b };
        if on {
            gfx::fill_rounded(ui.cr, b, CONTROL_RADIUS, t.accent.mix(if t.dark { Color::BLACK } else { Color::WHITE }, 0.08 * hv));
        } else {
            fluent::button(ui.cr, &t, b, hv, ui.is_pressed(k), false);
        }
        let fg = if on { t.on_accent } else { t.text1 };
        ui.icon(main, icon, 16.0, Some(fg));
        if let Some(m) = more {
            let mr = Rect::new(b.x + 64.0, b.y, 32.0, b.h);
            gfx::fill_rounded(ui.cr, Rect::new(mr.x, mr.y + 12.0, 1.0, mr.h - 24.0), 0.0, if on { t.on_accent.with_alpha(0.3) } else { t.divider });
            ui.icon(mr, "go-next-symbolic", 12.0, Some(fg));
            let mk = ui::idn("w11.qs.more", i);
            ui.hit(mk, mr, m, Role::Button, format!("{label} settings"));
        }
        ui.text(Rect::new(b.x - 6.0, b.bottom() + 6.0, b.w + 12.0, 18.0), label, &Text::new(CAPTION, t.text1).center());
        let hit = ui.hit(k, main, action, Role::Toggle, label.to_string());
        hit.value = Some(if on { 1.0 } else { 0.0 });
        // keep the split-button arrow on top
        let n = ui.hits.len();
        if split && n >= 2 {
            ui.hits.swap(n - 1, n - 2);
        }
    }
    let mut y = panel.y + 24.0 + rows as f64 * 84.0 + 8.0;
    let slider_row = |ui: &mut Ui, y: f64, icon: &str, value: f64, which: Slider, label: &str, key: &str| {
        let ic = Rect::new(panel.x + 22.0, y, 28.0, 40.0);
        ui.icon(ic, icon, 16.0, Some(t.text1));
        let sr = Rect::new(panel.x + 52.0, y, panel.w - 52.0 - 24.0, 40.0);
        let k = ui::id(key);
        let hv = ui.hover_amt(k);
        let v = ui.spring(k ^ 0x31, value, value);
        fluent::slider(ui.cr, &t, sr, v, hv);
        let hit = ui.hit(k, Rect::new(sr.x - 10.0, sr.y, sr.w + 20.0, sr.h), Action::Slider(which), Role::Slider, label.to_string());
        hit.value = Some(value);
    };
    if let Some(b) = data.brightness {
        slider_row(ui, y, "display-brightness-symbolic", b, Slider::Brightness, "Brightness", "w11.bright");
        y += 48.0;
    }
    if let Some(a) = &data.audio {
        let v = if a.muted { 0.0 } else { a.volume.min(1.0) };
        let icon = if v <= 0.0 { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" };
        slider_row(ui, y, icon, v, Slider::Volume, "Volume", "w11.vol");
        let mk = ui::id("w11.vol.mute");
        ui.hit(mk, Rect::new(panel.x + 18.0, y + 4.0, 32.0, 32.0), Action::ToggleMute, Role::Button, "Mute");
    }
    // Footer band.
    let foot = Rect::new(panel.x, panel.bottom() - 48.0, panel.w, 48.0);
    ui.cr.save().ok();
    gfx::rounded(ui.cr, panel, OVERLAY_RADIUS);
    ui.cr.clip();
    gfx::fill_rounded(ui.cr, foot, 0.0, if t.dark { Color::rgba(0.0, 0.0, 0.0, 0.18) } else { Color::rgba(0.0, 0.0, 0.0, 0.035) });
    ui.cr.restore().ok();
    gfx::fill_rounded(ui.cr, Rect::new(foot.x, foot.y, foot.w, 1.0), 0.0, t.divider);
    if let Some(b) = &data.battery {
        ui.icon(Rect::new(foot.x + 20.0, foot.y, 20.0, foot.h), &super::bar::battery_icon(b.percent, b.charging), 16.0, Some(t.text1));
        ui.text(Rect::new(foot.x + 46.0, foot.y, 80.0, foot.h), &format!("{:.0}%", b.percent), &Text::new(CAPTION, t.text1));
    }
    for (i, (icon, label, action)) in [("emblem-system-symbolic", "All settings", Action::Exec("creed-settings".into())), ("system-shutdown-symbolic", "Power", Action::Open(Kind::Power))].into_iter().enumerate() {
        let b = Rect::new(foot.right() - 20.0 - 32.0 * (2 - i) as f64 - 4.0 * (1 - i) as f64, foot.cy() - 16.0, 32.0, 32.0);
        let k = ui::idn("w11.qs.foot", i);
        let hv = ui.hover_amt(k);
        fluent::subtle(ui.cr, &t, b, CONTROL_RADIUS, hv, ui.is_pressed(k));
        ui.icon(b, icon, 16.0, Some(t.text1));
        ui.hit(k, b, action, Role::Button, label);
    }
}

// ───────────────────────────── Notification Center + calendar ─────────────────────────────

pub fn notif_center(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let (sw, sh) = ui.size;
    let bar = ui.th.cfg.bar.size as f64;
    let w = 360.0;
    let cal_h = 382.0;
    let x = sw - w - 12.0;
    let cal = Rect::new(x, sh - bar - 12.0 - cal_h, w, cal_h);
    let avail = cal.y - 12.0 - 12.0;
    let list_h = if data.history.is_empty() { 120.0 } else { (68.0 + data.history.iter().map(|n| toast_height(n) + 8.0).sum::<f64>()).min(avail) };
    let notes = Rect::new(x, cal.y - 12.0 - list_h, w, list_h);
    super::popup_transform(ui, cal, t_open, false);
    // Notifications card
    fluent::flyout(ui.cr, &t, notes, OVERLAY_RADIUS);
    ui.record("notif-center.panel", notes, None);
    ui.hit(ui::id("nc.panel"), notes, Action::None, Role::Group, "");
    let hdr = Rect::new(notes.x + 16.0, notes.y + 12.0, notes.w - 32.0, 32.0);
    ui.text(hdr, "Notifications", &Text::new(BODY, t.text1).bold());
    if !data.history.is_empty() {
        let bw = ui.measure("Clear all", &Text::new(CAPTION, t.text1)).0 + 24.0;
        let b = Rect::new(hdr.right() - bw, hdr.cy() - 12.0, bw, 24.0);
        let k = ui::id("nc.clear");
        let hv = ui.hover_amt(k);
        fluent::button(ui.cr, &t, b, hv, ui.is_pressed(k), false);
        ui.text(b, "Clear all", &Text::new(CAPTION, t.text1).center());
        ui.hit(k, b, Action::NotifClear, Role::Button, "Clear all notifications");
        let list = Rect::new(notes.x + 8.0, hdr.bottom() + 8.0, notes.w - 16.0, notes.bottom() - hdr.bottom() - 16.0);
        let from = ui.hits.len();
        let scroll = views.scroll;
        super::with_clip(ui, list, |ui| {
            let mut y = list.y - scroll;
            for n in &data.history {
                let h = toast_height(n);
                if y + h >= list.y && y <= list.bottom() {
                    toast_card(ui, &t, Rect::new(list.x, y, list.w, h), n, true);
                }
                y += h + 8.0;
            }
        });
        super::clip_hits(ui, from, list);
    } else {
        ui.text(Rect::new(notes.x, hdr.bottom(), notes.w, notes.bottom() - hdr.bottom() - 12.0), "No new notifications", &Text::new(BODY, t.text2).center());
    }
    // Calendar card
    fluent::flyout(ui.cr, &t, cal, OVERLAY_RADIUS);
    ui.hit(ui::id("nc.cal"), cal, Action::None, Role::Group, "");
    let now = time::now_secs();
    let head = Rect::new(cal.x + 16.0, cal.y, cal.w - 32.0, 48.0);
    ui.text(head, &time::format(now, "%A, %-d %B"), &Text::new(BODY, t.text1).bold());
    gfx::fill_rounded(ui.cr, Rect::new(cal.x, head.bottom(), cal.w, 1.0), 0.0, t.divider);
    month(ui, &t, Rect::new(cal.x + 12.0, head.bottom() + 10.0, cal.w - 24.0, cal.bottom() - head.bottom() - 70.0), views.calendar_offset);
    // Focus row
    let f = Rect::new(cal.x, cal.bottom() - 52.0, cal.w, 52.0);
    gfx::fill_rounded(ui.cr, Rect::new(f.x, f.y, f.w, 1.0), 0.0, t.divider);
    let fb = Rect::new(f.right() - 16.0 - 72.0, f.cy() - 16.0, 72.0, 32.0);
    let k = ui::id("nc.focus");
    let hv = ui.hover_amt(k);
    fluent::button(ui.cr, &t, fb, hv, ui.is_pressed(k), data.dnd);
    ui.text(fb, "Focus", &Text::new(CAPTION, if data.dnd { t.on_accent } else { t.text1 }).center());
    ui.text(Rect::new(f.x + 16.0, f.y, 160.0, f.h), if data.dnd { "Focus is on" } else { "Focus" }, &Text::new(BODY, t.text1));
    ui.hit(k, fb, Action::ToggleDnd, Role::Toggle, "Focus");
}

// ───────────────────────────── toasts ─────────────────────────────

pub fn toast_height(n: &crate::data::Notif) -> f64 {
    let body_lines = if n.body.is_empty() { 0.0 } else { ((n.body.chars().count() as f64) / 44.0).ceil().clamp(1.0, 3.0) };
    let actions = n.actions.iter().any(|(k, _)| k != "default");
    (44.0 + 20.0 + body_lines * 20.0 + 16.0 + if actions { 44.0 } else { 0.0 }).round()
}

pub fn toast_card(ui: &mut Ui, t: &Tok, r: Rect, n: &crate::data::Notif, in_center: bool) {
    let k = ui::idn("w11.toast", n.id);
    let hv = ui.hover_amt(k);
    if in_center {
        gfx::fill_rounded(ui.cr, r, OVERLAY_RADIUS, t.card);
        gfx::stroke_rounded(ui.cr, r, OVERLAY_RADIUS, 1.0, t.card_stroke);
    } else {
        fluent::flyout(ui.cr, t, r, OVERLAY_RADIUS);
    }
    // Header: app icon 16 + app name (caption), close on the right.
    let icon = if n.icon.is_empty() { "dialog-information-symbolic".to_string() } else { n.icon.clone() };
    ui.icon(Rect::new(r.x + 16.0, r.y + 12.0, 16.0, 20.0), &icon, 16.0, if icon.ends_with("-symbolic") { Some(t.accent) } else { None });
    ui.text(Rect::new(r.x + 40.0, r.y + 12.0, r.w - 100.0, 20.0), &n.app, &Text::new(CAPTION, t.text1));
    let close = Rect::new(r.right() - 40.0, r.y + 8.0, 28.0, 28.0);
    if hv > 0.05 {
        let ck = ui::idn("w11.toast.x", n.id);
        let chv = ui.hover_amt(ck);
        fluent::subtle(ui.cr, t, close, CONTROL_RADIUS, chv, false);
        ui.icon(close, "window-close-symbolic", 12.0, Some(t.text1.with_alpha(t.text1.a * hv)));
    }
    let content = Rect::new(r.x + 16.0, r.y + 44.0, r.w - 32.0, r.h - 52.0);
    ui.text(Rect::new(content.x, content.y, content.w, 20.0), &n.summary, &Text::new(BODY, t.text1).bold());
    if !n.body.is_empty() {
        let body = super::notif::strip_markup(&n.body);
        ui.text(Rect::new(content.x, content.y + 22.0, content.w, 60.0), &body, &Text::new(BODY, t.text2).lines(3));
    }
    let default = n.actions.iter().find(|(k, _)| k == "default").map(|(k, _)| Action::NotifAction(n.id, k.clone())).unwrap_or(Action::NotifDismiss(n.id));
    ui.hit(k, r, default, Role::Item, format!("Notification from {}: {}", n.app, n.summary));
    if hv > 0.05 {
        ui.hit(ui::idn("w11.toast.x", n.id), close, Action::NotifDismiss(n.id), Role::Button, "Dismiss");
    }
    let acts: Vec<&(String, String)> = n.actions.iter().filter(|(k, _)| k != "default").take(3).collect();
    if !acts.is_empty() {
        let row = Rect::new(r.x + 16.0, r.bottom() - 16.0 - 32.0, r.w - 32.0, 32.0);
        let cols = row.columns(acts.len(), 8.0);
        for (i, (key, label)) in acts.iter().enumerate() {
            let ak = ui::idn(&format!("w11.toast.act.{}", n.id), i);
            let ahv = ui.hover_amt(ak);
            fluent::button(ui.cr, t, cols[i], ahv, ui.is_pressed(ak), false);
            ui.text(cols[i], label, &Text::new(BODY, t.text1).center());
            ui.hit(ak, cols[i], Action::NotifAction(n.id, key.clone()), Role::Button, label.clone());
        }
    }
}

pub fn toasts(ui: &mut Ui, data: &Data) {
    let t = tok(ui.th);
    let (sw, sh) = ui.size;
    let mut y = sh - 12.0;
    for n in &data.notifs {
        let h = toast_height(n);
        let k = ui::idn("w11.toast.in", n.id);
        let a = ui.spring(k, 1.0, 0.0);
        let r = Rect::new(sw - 364.0 - 12.0, y - h, 364.0, h);
        ui.cr.save().ok();
        ui.cr.push_group();
        ui.cr.translate(((1.0 - a) * 60.0).round(), 0.0);
        toast_card(ui, &t, r, n, false);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(a.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        ui.record(&format!("notif.{}", n.id), r, None);
        y -= h + 12.0;
    }
}

// ───────────────────────────── volume / brightness flyout ─────────────────────────────

/// 24H2-style pill: icon, bar, value. Centred above the taskbar.
pub fn osd(ui: &mut Ui, kind: crate::data::OsdKind, value: f64) {
    let t = tok(ui.th);
    let (w, h) = ui.size;
    let r = Rect::new(4.0, 4.0, w - 8.0, h - 8.0);
    fluent::flyout(ui.cr, &t, r, r.h / 2.0);
    let icon = match kind {
        crate::data::OsdKind::Brightness => "display-brightness-symbolic",
        crate::data::OsdKind::Game => "input-gaming-symbolic",
        crate::data::OsdKind::Mic if value <= 0.0 => "microphone-disabled-symbolic",
        crate::data::OsdKind::Mic => "audio-input-microphone-symbolic",
        _ if value <= 0.0 => "audio-volume-muted-symbolic",
        _ => "audio-volume-high-symbolic",
    };
    ui.icon(Rect::new(r.x + 16.0, r.y, 20.0, r.h), icon, 16.0, Some(t.text1));
    if kind == crate::data::OsdKind::Game {
        let label = if value > 0.5 { "Game Mode on" } else { "Game Mode off" };
        ui.text(Rect::new(r.x + 48.0, r.y, r.w - 64.0, r.h), label, &Text::new(BODY, t.text1));
        return;
    }
    let v = ui.spring(ui::id("w11.osd"), value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    let track = Rect::new(r.x + 48.0, r.cy() - 2.0, r.w - 48.0 - 48.0, 4.0);
    gfx::fill_rounded(ui.cr, track, 2.0, if t.dark { Color::rgba(1.0, 1.0, 1.0, 0.55) } else { Color::rgba(0.0, 0.0, 0.0, 0.45) });
    gfx::fill_rounded(ui.cr, Rect::new(track.x, track.y, track.w * v, 4.0), 2.0, t.accent);
    ui.text(Rect::new(r.right() - 44.0, r.y, 30.0, r.h), &format!("{:.0}", v * 100.0), &Text::new(CAPTION, t.text1).end());
}

pub const OSD_SIZE: (u32, u32) = (200, 56);

// ───────────────────────────── Task View ─────────────────────────────

pub fn task_view(ui: &mut Ui, data: &Data, views: &mut Views) {
    let t = tok(ui.th);
    let (sw, sh) = ui.size;
    let bar = ui.th.cfg.bar.size as f64;
    let strip = Rect::new(0.0, sh - bar - 150.0, sw, 150.0);
    let area = Rect::new(60.0, 60.0, sw - 120.0, strip.y - 90.0);
    ui.record("overview.area", area, None);
    let wins = super::overview::windows(data);
    if wins.is_empty() {
        ui.text(area, "No open windows on this desktop", &Text::new(BODY, Color::rgba(1.0, 1.0, 1.0, 0.8)).center());
    }
    let rects = super::overview::grid_pub(area, wins.len(), 1.6, 40.0);
    let sel = views.sel.min(wins.len().saturating_sub(1));
    for (i, (c, r)) in wins.iter().zip(rects).enumerate() {
        let k = ui::idn("tv.win", &c.address);
        let hv = ui.hover_amt(k);
        let icon = crate::services::apps::icon_for_class(&data.apps, &c.class);
        // Title row above the thumbnail, as in Windows.
        let title = Rect::new(r.x, r.y - 30.0, r.w, 28.0);
        ui.icon(Rect::new(title.x, title.y, 20.0, title.h), &icon, 16.0, None);
        ui.text(Rect::new(title.x + 26.0, title.y, title.w - 60.0, title.h), &c.title, &Text::new(CAPTION, Color::rgba(1.0, 1.0, 1.0, 0.95)));
        gfx::shadow(ui.cr, r, OVERLAY_RADIUS, 16.0, 6.0, 0.45);
        gfx::fill_rounded(ui.cr, r, OVERLAY_RADIUS, t.solid_base);
        if !super::overview::draw_thumb(ui, data, &c.address, r, OVERLAY_RADIUS) {
            ui.icon(r, &icon, (r.h * 0.35).clamp(32.0, 96.0).round(), None);
        }
        if hv > 0.0 || i == sel {
            gfx::stroke_rounded(ui.cr, r.inset(-3.0), OVERLAY_RADIUS + 3.0, 3.0, Color::rgba(1.0, 1.0, 1.0, if i == sel { 0.95 } else { 0.6 * hv }));
        }
        let hit = ui.hit(k, Rect::new(r.x, title.y, r.w, r.bottom() - title.y), Action::FocusWindow(c.address.clone()), Role::Item, format!("{}: {}", Data::pretty_class(&c.class), c.title));
        hit.secondary = Some(Action::CloseWindow(c.address.clone()));
        if hv > 0.05 {
            let x = Rect::new(r.right() - 32.0, title.y, 32.0, 28.0);
            let xk = ui::idn("tv.close", &c.address);
            let xhv = ui.hover_amt(xk);
            gfx::fill_rounded(ui.cr, x, CONTROL_RADIUS, Color::rgba(0.77, 0.17, 0.11, xhv));
            ui.icon(x, "window-close-symbolic", 12.0, Some(Color::WHITE));
            ui.hit(xk, x, Action::CloseWindow(c.address.clone()), Role::Button, "Close");
        }
    }
    // Desktops bar
    gfx::fill_rounded(ui.cr, strip, 0.0, if t.dark { Color::rgba(0.0, 0.0, 0.0, 0.25) } else { Color::rgba(1.0, 1.0, 1.0, 0.25) });
    let list = super::bar::ws_list(data, ui.th.cfg.bar.workspaces);
    let (tw, th_) = (160.0, 90.0);
    let total = (list.len() + 1) as f64 * (tw + 16.0) - 16.0;
    let mut x = (sw - total) / 2.0;
    for id in &list {
        let thumb = Rect::new(x, strip.y + 34.0, tw, th_);
        let k = ui::idn("tv.ws", id);
        let hv = ui.hover_amt(k);
        let active = *id == data.active_ws;
        ui.text(Rect::new(thumb.x, strip.y + 8.0, tw, 22.0), &format!("Desktop {id}"), &Text::new(CAPTION, Color::rgba(1.0, 1.0, 1.0, 0.95)));
        gfx::fill_rounded(ui.cr, thumb, OVERLAY_RADIUS, Color::rgba(1.0, 1.0, 1.0, 0.12 + 0.08 * hv));
        if active {
            gfx::stroke_rounded(ui.cr, thumb.inset(-2.0), OVERLAY_RADIUS + 2.0, 2.0, Color::rgba(1.0, 1.0, 1.0, 0.9));
        }
        ui.hit(k, thumb, Action::Workspace(*id), Role::Button, format!("Desktop {id}"));
        x += tw + 16.0;
    }
    let add = Rect::new(x, strip.y + 34.0, tw, th_);
    let ak = ui::id("tv.new");
    let hv = ui.hover_amt(ak);
    gfx::fill_rounded(ui.cr, add, OVERLAY_RADIUS, Color::rgba(1.0, 1.0, 1.0, 0.08 + 0.08 * hv));
    ui.text(Rect::new(add.x, strip.y + 8.0, tw, 22.0), "New desktop", &Text::new(CAPTION, Color::rgba(1.0, 1.0, 1.0, 0.95)));
    ui.text(add, "+", &Text::new(28.0, Color::rgba(1.0, 1.0, 1.0, 0.9)).center());
    ui.hit(ak, add, Action::Hypr("hl.dsp.focus({ workspace = \"empty\" })".into()), Role::Button, "New desktop");
}
