//! Dashboard / Widgets board: clock, calendar, weather, system stats, media.

use super::{calendar, open_amount, popup_anchor, popup_transform, Views};
use crate::data::Data;
use crate::gfx::{self, Align, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::Color;
use std::f64::consts::PI;

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Dashboard));
    let th = ui.th;
    let variant = th.cfg.style.dashboard.clone();
    if variant == "aqua" {
        ui.cr.save().ok();
        ui.cr.push_group();
        super::mac::aqua_dashboard(ui, t, data, views);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        return;
    }
    let (sw, sh) = ui.size;
    let bar = if th.cfg.bar.enabled { (th.cfg.bar.size + th.cfg.bar.margin) as f64 } else { 0.0 };
    let panel = match variant.as_str() {
        "widgets11" => Rect::new(12.0, 12.0, 760.0f64.min(sw - 24.0), sh - bar - 24.0),
        "mac" => Rect::new(sw - 372.0, bar + 8.0, 360.0, (sh - bar - 16.0).min(720.0)),
        _ => popup_anchor(ui, 760.0f64.min(sw - 24.0), 440.0, Align::Center),
    };
    ui.cr.save().ok();
    ui.cr.push_group();
    popup_transform(ui, panel, t, th.cfg.bar.position == creed_core::config::Edge::Top);
    if variant == "widgets11" {
        let t = super::fluent::tok(th);
        super::fluent::flyout(ui.cr, &t, panel, super::fluent::OVERLAY_RADIUS);
    } else {
        th.panel(ui.cr, panel, th.radius() + 2.0, "dashboard", ui.pointer);
    }
    ui.record("dashboard.panel", panel, None);
    ui.hit(ui::id("dashboard.panel"), panel, Action::None, Role::Group, "");
    let mut inner = panel.inset(18.0);
    if variant == "widgets11" {
        // Header: time on the left, "Search the web" in the middle, account on the right.
        let t = super::fluent::tok(th);
        let hdr = Rect::new(inner.x, inner.y, inner.w, 36.0);
        ui.text(Rect::new(hdr.x + 4.0, hdr.y, 120.0, hdr.h), &time::format(time::now_secs(), &th.cfg.bar.clock_format), &Text::new(super::fluent::BODY, t.text1).bold());
        let sb = Rect::centered(hdr.cx(), hdr.cy(), 320.0, 32.0);
        gfx::fill_rounded(ui.cr, sb, 16.0, t.ctl_fill);
        gfx::stroke_rounded(ui.cr, sb, 16.0, 1.0, t.ctl_stroke);
        ui.icon(Rect::new(sb.x + 12.0, sb.y, 16.0, sb.h), "system-search-symbolic", 16.0, Some(t.text2));
        ui.text(Rect::new(sb.x + 38.0, sb.y, sb.w - 50.0, sb.h), "Search the web", &Text::new(super::fluent::BODY, t.text2));
        ui.hit(ui::id("w11.widgets.search"), sb, Action::Exec(format!("{} https://duckduckgo.com", th.cfg.general.browser)), Role::Button, "Search the web");
        let av = Rect::new(hdr.right() - 32.0, hdr.cy() - 16.0, 32.0, 32.0);
        gfx::fill_rounded(ui.cr, av, 16.0, t.ctl_fill2);
        ui.icon(av, "avatar-default-symbolic", 16.0, Some(t.text1));
        inner = Rect::new(inner.x, hdr.bottom() + 16.0, inner.w, inner.h - 52.0);
    }
    match variant.as_str() {
        "widgets11" | "mac" => column_layout(ui, inner, data, views, variant == "widgets11"),
        _ => grid_layout(ui, inner, data, views),
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

fn card(ui: &mut Ui, r: Rect) {
    if ui.th.cfg.style.dashboard == "widgets11" {
        let t = super::fluent::tok(ui.th);
        gfx::fill_rounded(ui.cr, r, super::fluent::OVERLAY_RADIUS, t.card);
        gfx::stroke_rounded(ui.cr, r, super::fluent::OVERLAY_RADIUS, 1.0, t.card_stroke);
        return;
    }
    let c = if ui.th.dark { Color::rgba(1.0, 1.0, 1.0, 0.05) } else { Color::rgba(0.0, 0.0, 0.0, 0.035) };
    gfx::fill_rounded(ui.cr, r, ui.th.radius_small() + 6.0, c);
}

fn grid_layout(ui: &mut Ui, inner: Rect, data: &Data, views: &mut Views) {
    let cols = inner.columns(3, 14.0);
    // Column 1: greeting + clock + weather
    let (clock_r, weather_r) = cols[0].take_top(cols[0].h * 0.55, 14.0);
    card(ui, clock_r);
    clock_card(ui, clock_r.inset(16.0), data);
    card(ui, weather_r);
    weather_card(ui, weather_r.inset(14.0), data);
    // Column 2: calendar
    card(ui, cols[1]);
    calendar::draw_month(ui, cols[1].inset(12.0), views.calendar_offset);
    // Column 3: system + media
    let (sys_r, media_r) = cols[2].take_top(if data.media.is_some() { cols[2].h - 96.0 } else { cols[2].h }, 14.0);
    card(ui, sys_r);
    sys_card(ui, sys_r.inset(14.0), data);
    if let Some(m) = &data.media {
        super::control::media_card(ui, media_r, m);
    }
}

fn column_layout(ui: &mut Ui, inner: Rect, data: &Data, views: &mut Views, two_cols: bool) {
    if two_cols {
        let cols = inner.columns(2, 14.0);
        let (a, b) = cols[0].take_top(170.0, 14.0);
        card(ui, a);
        clock_card(ui, a.inset(16.0), data);
        let (b1, b2) = b.take_top(150.0, 14.0);
        card(ui, b1);
        weather_card(ui, b1.inset(14.0), data);
        card(ui, b2);
        sys_card(ui, b2.inset(14.0), data);
        let (c1, c2) = cols[1].take_top(320.0, 14.0);
        card(ui, c1);
        calendar::draw_month(ui, c1.inset(12.0), views.calendar_offset);
        if let Some(m) = &data.media {
            super::control::media_card(ui, Rect::new(c2.x, c2.y, c2.w, 72.0), m);
        }
    } else {
        let (a, rest) = inner.take_top(150.0, 12.0);
        card(ui, a);
        weather_card(ui, a.inset(14.0), data);
        let (b, rest) = rest.take_top(300.0, 12.0);
        card(ui, b);
        calendar::draw_month(ui, b.inset(12.0), views.calendar_offset);
        let (c, rest) = rest.take_top(170.0f64.min(rest.h), 12.0);
        card(ui, c);
        sys_card(ui, c.inset(14.0), data);
        if let Some(m) = &data.media {
            if rest.h >= 72.0 {
                super::control::media_card(ui, Rect::new(rest.x, rest.y, rest.w, 72.0), m);
            }
        }
    }
}

fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Good morning",
        12..=17 => "Good afternoon",
        18..=22 => "Good evening",
        _ => "Up late",
    }
}

fn clock_card(ui: &mut Ui, r: Rect, data: &Data) {
    let th = ui.th;
    let now = time::now_secs();
    let tm = time::local(now);
    let (g, rest) = r.take_top(24.0, 6.0);
    let name = if data.user.is_empty() { String::new() } else { format!(", {}", data.user) };
    ui.text(g, &format!("{}{name}", greeting(tm.hour)), &Text::new(th.fs, th.pal.subtext));
    let (c, rest) = rest.take_top(th.fs * 4.2, 4.0);
    ui.text(c, &time::format(now, &th.cfg.bar.clock_format), &Text::new(th.fs * 3.6, th.pal.text).heavy());
    let (d, rest) = rest.take_top(24.0, 8.0);
    ui.text(d, &time::format(now, "%A, %d %B"), &Text::new(th.fs * 1.05, th.pal.accent).medium());
    if rest.h > 20.0 && !data.uptime.is_empty() {
        ui.text(Rect::new(rest.x, rest.bottom() - 20.0, rest.w, 20.0), &format!("Up {}", data.uptime), &Text::new(th.fs * 0.85, th.pal.muted));
    }
}

fn weather_card(ui: &mut Ui, r: Rect, data: &Data) {
    let th = ui.th;
    match &data.weather {
        Some(w) => {
            let (ic, rest) = r.take_left(r.h.min(64.0), 12.0);
            ui.icon(ic, &w.icon, ic.w.min(56.0), Some(th.pal.accent));
            let (a, rest) = rest.take_top(rest.h * 0.5, 0.0);
            ui.text(a, &w.temp, &Text::new(th.fs * 2.0, th.pal.text).bold());
            let (b, c) = rest.take_top(rest.h / 2.0, 0.0);
            ui.text(b, &w.desc, &Text::new(th.fs, th.pal.subtext));
            ui.text(c, &format!("{}{}", w.place, if w.hi_lo.is_empty() { String::new() } else { format!(" · {}", w.hi_lo) }), &Text::new(th.fs * 0.85, th.pal.muted));
        }
        None => {
            let msg = if th.cfg.weather.enabled { "Weather unavailable (offline?)" } else { "Weather is turned off" };
            super::empty(ui, r, "weather-overcast-symbolic", msg);
        }
    }
}

fn ring(ui: &mut Ui, r: Rect, frac: f64, label: &str, value: &str, key: ui::Id) {
    let th = ui.th;
    let f = ui.spring(key, frac.clamp(0.0, 1.0), frac.clamp(0.0, 1.0));
    let cr = ui.cr;
    let rad = (r.w.min(r.h - 24.0) / 2.0 - 5.0).max(10.0);
    let (cx, cy) = (r.cx(), r.y + rad + 5.0);
    cr.new_path();
    cr.set_line_width((rad * 0.16).clamp(4.0, 8.0));
    cr.set_line_cap(cairo::LineCap::Round);
    gfx::set(cr, th.pal.text.with_alpha(0.1));
    cr.arc(cx, cy, rad, 0.0, 2.0 * PI);
    let _ = cr.stroke();
    cr.new_path();
    let col = if frac > 0.85 { th.pal.error } else if frac > 0.65 { th.pal.warning } else { th.pal.accent };
    gfx::set(cr, col);
    cr.arc(cx, cy, rad, -PI / 2.0, -PI / 2.0 + 2.0 * PI * f.max(0.001));
    let _ = cr.stroke();
    ui.text(Rect::centered(cx, cy, rad * 2.0, 20.0), value, &Text::new(th.fs * 0.95, th.pal.text).bold().center());
    ui.text(Rect::new(r.x, r.bottom() - 18.0, r.w, 18.0), label, &Text::new(th.fs * 0.82, th.pal.subtext).center());
    ui.hit(key, r, Action::None, Role::Label, format!("{label} {value}"));
}

fn sys_card(ui: &mut Ui, r: Rect, data: &Data) {
    let s = &data.sys;
    // Two rows of two rings when there is room vertically, otherwise one row.
    let n = if s.temp_c.is_some() { 4 } else { 3 };
    let two_rows = r.h > r.w * 0.9;
    let cells: Vec<Rect> = if two_rows {
        let rows = r.rows(2, 10.0);
        let mut v = rows[0].columns(2, 8.0);
        v.extend(rows[1].columns(2, 8.0));
        v
    } else {
        r.columns(n, 8.0)
    };
    let cols = cells;
    let row = |c: &Rect| {
        let side = c.w.min(c.h).min(132.0);
        Rect::new(c.x, c.y + (c.h - side) / 2.0, c.w, side)
    };
    let mem = if s.mem_total_mb > 0 { s.mem_used_mb as f64 / s.mem_total_mb as f64 } else { 0.0 };
    let disk = if s.disk_total_gb > 0.0 { s.disk_used_gb / s.disk_total_gb } else { 0.0 };
    ring(ui, row(&cols[0]), s.cpu / 100.0, "CPU", &format!("{:.0}%", s.cpu), ui::id("dash.cpu"));
    ring(ui, row(&cols[1]), mem, "Memory", &format!("{:.1}G", s.mem_used_mb as f64 / 1024.0), ui::id("dash.mem"));
    ring(ui, row(&cols[2]), disk, "Disk", &format!("{:.0}%", disk * 100.0), ui::id("dash.disk"));
    if let Some(t) = s.temp_c {
        ring(ui, row(&cols[3]), (t / 100.0).min(1.0), "Temp", &format!("{t:.0}°"), ui::id("dash.temp"));
    }
}
