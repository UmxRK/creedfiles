//! Desktop widgets (macOS 26 style): glass tiles that live on the desktop, under windows.
//! A mode turns them on with `style.desktop_widgets = ["weather", "calendar", "battery", "shortcuts"]`.
//! Sizes follow macOS widget families: small 158 × 158, medium 338 × 158 (logical px).

use super::Spec;
use crate::data::Data;
use crate::gfx::{self, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::Color;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

const SMALL: f64 = 158.0;
const GAP: f64 = 16.0;
const MEDIUM: f64 = SMALL * 2.0 + GAP + 6.0;
const R: f64 = 22.0;
const PAD: f64 = 16.0;

/// Rows of tiles: each row is either one medium tile or up to two small ones.
fn rows(th: &gfx::Theme, data: &Data) -> Vec<Vec<&'static str>> {
    let want = &th.cfg.style.desktop_widgets;
    let mut rows: Vec<Vec<&'static str>> = vec![];
    let mut smalls: Vec<&'static str> = vec![];
    for w in want {
        match w.as_str() {
            "weather" => rows.push(vec!["weather"]),
            "shortcuts" => rows.push(vec!["shortcuts"]),
            "calendar" => smalls.push("calendar"),
            "battery" => smalls.push(if data.battery.is_some() { "battery" } else { "system" }),
            "system" => smalls.push("system"),
            _ => {}
        }
        if smalls.len() == 2 {
            rows.push(std::mem::take(&mut smalls));
        }
    }
    if !smalls.is_empty() {
        rows.push(smalls);
    }
    rows
}

pub fn spec(th: &gfx::Theme, data: &Data) -> Spec {
    let n = rows(th, data).len().max(1) as f64;
    let top = if th.cfg.bar.enabled && th.cfg.bar.position == creed_core::config::Edge::Top { th.cfg.bar.size + th.cfg.bar.margin } else { 0 };
    Spec {
        layer: Layer::Bottom,
        anchor: Anchor::TOP | Anchor::LEFT,
        size: ((MEDIUM + 24.0) as u32, (n * (SMALL + GAP) + 24.0) as u32),
        margin: (top as i32 + 8, 0, 0, 16),
        exclusive: -1,
        keyboard: KeyboardInteractivity::None,
        namespace: "creed-widgets",
        click_through: false,
        input: None,
    }
}

struct Pal {
    label: Color,
    label2: Color,
    accent: Color,
}

fn pal(th: &gfx::Theme) -> Pal {
    if th.dark {
        Pal { label: Color::rgba(1.0, 1.0, 1.0, 0.92), label2: Color::rgba(1.0, 1.0, 1.0, 0.6), accent: th.pal.accent }
    } else {
        Pal { label: Color::rgba(0.0, 0.0, 0.0, 0.85), label2: Color::rgba(0.0, 0.0, 0.0, 0.5), accent: th.pal.accent }
    }
}

pub fn draw(ui: &mut Ui, data: &Data) {
    let th = ui.th;
    let mut y = 12.0;
    for row in rows(th, data) {
        let mut x = 12.0;
        for w in row {
            let wide = matches!(w, "weather" | "shortcuts");
            let r = Rect::new(x, y, if wide { MEDIUM } else { SMALL }, SMALL);
            th.panel(ui.cr, r, R, "widgets", ui.pointer);
            ui.record(&format!("widget.{w}"), r, None);
            match w {
                "weather" => weather(ui, r, data),
                "calendar" => calendar(ui, r),
                "battery" => battery(ui, r, data),
                "system" => system(ui, r, data),
                "shortcuts" => shortcuts(ui, r),
                _ => {}
            }
            x += r.w + GAP + if wide { 0.0 } else { 6.0 };
        }
        y += SMALL + GAP;
    }
}

fn hour_label(h: u32, twelve: bool) -> String {
    if twelve {
        let (n, ap) = match h {
            0 => (12, "AM"),
            1..=11 => (h, "AM"),
            12 => (12, "PM"),
            _ => (h - 12, "PM"),
        };
        format!("{n}{ap}")
    } else {
        format!("{h:02}")
    }
}

fn weather(ui: &mut Ui, r: Rect, data: &Data) {
    let p = pal(ui.th);
    let i = r.inset(PAD);
    let Some(w) = &data.weather else {
        ui.icon(Rect::new(i.x, i.y, 34.0, 34.0), "weather-few-clouds-symbolic", 30.0, Some(p.label2));
        ui.text(Rect::new(i.x, i.y + 44.0, i.w, 20.0), "Weather", &Text::new(15.0, p.label).bold());
        ui.text(Rect::new(i.x, i.y + 66.0, i.w, 18.0), "Unavailable - check your connection", &Text::new(12.0, p.label2));
        ui.hit(ui::id("widget.weather"), r, Action::None, Role::Label, "Weather unavailable");
        return;
    };
    // Left: place, big temperature. Right: condition icon, description, H/L.
    let place = if w.place.is_empty() { "Weather".to_string() } else { w.place.clone() };
    ui.text(Rect::new(i.x, i.y - 2.0, i.w * 0.6, 18.0), &format!("{place} ➤"), &Text::new(13.0, p.label).bold());
    ui.text(Rect::new(i.x, i.y + 14.0, i.w * 0.5, 42.0), &w.temp, &Text::new(36.0, p.label));
    let rr = Rect::new(i.x + i.w * 0.5, i.y, i.w * 0.5, 56.0);
    ui.icon(Rect::new(rr.right() - 20.0, rr.y - 2.0, 20.0, 20.0), &w.icon, 18.0, Some(p.label));
    ui.text(Rect::new(rr.x, rr.y + 20.0, rr.w, 16.0), &w.desc, &Text::new(12.0, p.label).bold().end());
    ui.text(Rect::new(rr.x, rr.y + 36.0, rr.w, 16.0), &w.hi_lo.replace("H ", "H:").replace(" L ", " L:"), &Text::new(12.0, p.label).end());
    // Hourly forecast.
    let twelve = ui.th.cfg.bar.clock_format.contains("%I") || ui.th.cfg.bar.clock_format.contains("%l");
    let hr = Rect::new(i.x, i.y + 66.0, i.w, i.bottom() - i.y - 66.0);
    if !w.hourly.is_empty() {
        let cols = hr.columns(6, 4.0);
        for (k, (h, t, icon)) in w.hourly.iter().enumerate().take(6) {
            let c = cols[k];
            ui.text(Rect::new(c.x, c.y, c.w, 16.0), &hour_label(*h, twelve), &Text::new(11.0, p.label2).bold().center());
            ui.icon(Rect::new(c.x, c.y + 18.0, c.w, 20.0), icon, 18.0, Some(p.label));
            ui.text(Rect::new(c.x, c.y + 40.0, c.w, 16.0), t, &Text::new(12.0, p.label).bold().center());
        }
    }
    let label = format!("Weather in {place}: {}, {}. {}", w.temp, w.desc, w.hi_lo);
    ui.hit(ui::id("widget.weather"), r, Action::None, Role::Label, label);
}

fn calendar(ui: &mut Ui, r: Rect) {
    let p = pal(ui.th);
    let i = r.inset(PAD);
    let now = time::local(time::now_secs());
    let day = time::format(time::now_secs(), "%A").to_uppercase();
    ui.text(Rect::new(i.x, i.y - 2.0, i.w, 16.0), &day, &Text::new(11.0, Color::parse("#ff3b30").unwrap_or(p.accent)).bold());
    ui.text(Rect::new(i.x, i.y + 12.0, i.w, 50.0), &now.day.to_string(), &Text::new(44.0, p.label));
    ui.text(Rect::new(i.x, i.bottom() - 34.0, i.w, 16.0), "No events today", &Text::new(12.0, p.label2));
    ui.text(Rect::new(i.x, i.bottom() - 18.0, i.w, 16.0), &time::format(time::now_secs(), "%B"), &Text::new(12.0, p.label2));
    ui.hit(ui::id("widget.calendar"), r, Action::Toggle(Kind::Calendar), Role::Button, format!("Calendar, {}", time::format(time::now_secs(), "%A %-d %B")));
}

fn ring(ui: &mut Ui, c: (f64, f64), rad: f64, frac: f64, col: Color, track: Color) {
    let cr = ui.cr;
    cr.set_line_width(6.0);
    cr.new_path();
    cr.arc(c.0, c.1, rad, 0.0, std::f64::consts::TAU);
    gfx::set(cr, track);
    let _ = cr.stroke();
    cr.new_path();
    let a0 = -std::f64::consts::FRAC_PI_2;
    cr.arc(c.0, c.1, rad, a0, a0 + std::f64::consts::TAU * frac.clamp(0.0, 1.0));
    cr.set_line_cap(cairo::LineCap::Round);
    gfx::set(cr, col);
    let _ = cr.stroke();
}

fn battery(ui: &mut Ui, r: Rect, data: &Data) {
    let p = pal(ui.th);
    let Some(b) = &data.battery else { return };
    let i = r.inset(PAD);
    let col = if b.percent <= 20.0 && !b.charging { Color::parse("#ff3b30").unwrap() } else { Color::parse("#34c759").unwrap() };
    let c = (i.cx(), i.y + 50.0);
    ring(ui, c, 38.0, b.percent / 100.0, col, p.label2.with_alpha(0.18));
    ui.icon(Rect::centered(c.0, c.1, 30.0, 30.0), if b.charging { "battery-level-100-charging-symbolic" } else { "computer-symbolic" }, 24.0, Some(p.label));
    ui.text(Rect::new(i.x, i.bottom() - 18.0, i.w, 18.0), &format!("{:.0}%", b.percent), &Text::new(15.0, p.label).bold().center());
    ui.hit(ui::id("widget.battery"), r, Action::None, Role::Label, format!("Battery {:.0}%{}", b.percent, if b.charging { ", charging" } else { "" }));
}

fn system(ui: &mut Ui, r: Rect, data: &Data) {
    let p = pal(ui.th);
    let i = r.inset(PAD);
    ui.text(Rect::new(i.x, i.y - 2.0, i.w, 16.0), "SYSTEM", &Text::new(11.0, p.label2).bold());
    let mem = if data.sys.mem_total_mb > 0 { data.sys.mem_used_mb as f64 / data.sys.mem_total_mb as f64 } else { 0.0 };
    let rows = [("CPU", data.sys.cpu / 100.0), ("Memory", mem), ("Disk", if data.sys.disk_total_gb > 0.0 { data.sys.disk_used_gb / data.sys.disk_total_gb } else { 0.0 })];
    for (k, (name, v)) in rows.iter().enumerate() {
        let y = i.y + 22.0 + k as f64 * 36.0;
        ui.text(Rect::new(i.x, y, i.w, 16.0), name, &Text::new(12.0, p.label));
        ui.text(Rect::new(i.x, y, i.w, 16.0), &format!("{:.0}%", v * 100.0), &Text::new(12.0, p.label).bold().end());
        let tr = Rect::new(i.x, y + 19.0, i.w, 6.0);
        gfx::fill_rounded(ui.cr, tr, 3.0, p.label2.with_alpha(0.18));
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, tr.y, (tr.w * v.clamp(0.0, 1.0)).max(6.0), 6.0), 3.0, p.accent);
    }
    ui.hit(ui::id("widget.system"), r, Action::None, Role::Label, format!("CPU {:.0}%, memory {:.0}%", data.sys.cpu, mem * 100.0));
}

fn shortcuts(ui: &mut Ui, r: Rect) {
    let p = pal(ui.th);
    let th = ui.th;
    let items: [(&str, &str, Action); 4] = [
        ("folder-documents-symbolic", "Files", Action::Exec(th.cfg.general.file_manager.clone())),
        ("utilities-terminal-symbolic", "Terminal", Action::Exec(th.cfg.general.terminal.clone())),
        ("view-fullscreen-symbolic", "Screenshot", Action::Exec("creed screenshot region".into())),
        ("emblem-system-symbolic", "Settings", Action::Exec("creed-settings".into())),
    ];
    let i = r.inset(10.0);
    let grid = i.rows(2, 8.0);
    for (k, (icon, label, a)) in items.into_iter().enumerate() {
        let c = grid[k / 2].columns(2, 8.0)[k % 2];
        let key = ui::idn("widget.sc", k);
        let hv = ui.hover_amt(key);
        let base = if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.10 + 0.06 * hv) } else { Color::rgba(1.0, 1.0, 1.0, 0.35 + 0.15 * hv) };
        gfx::fill_rounded(ui.cr, c, 14.0, base);
        ui.icon(Rect::new(c.x + 10.0, c.y + 8.0, 20.0, 20.0), icon, 18.0, Some(p.label));
        ui.text(Rect::new(c.x + 10.0, c.bottom() - 26.0, c.w - 16.0, 18.0), label, &Text::new(12.5, p.label).bold());
        if ui.focus == Some(key) {
            ui.th.focus_ring(ui.cr, c, 14.0);
        }
        ui.hit(key, c, a, Role::Button, label);
    }
}
