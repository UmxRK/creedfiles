//! Overview: Creed workspace map, Windows Task View, macOS Mission Control, and Launchpad.

use super::{launcher, open_amount, Views};
use crate::data::Data;
use crate::gfx::{self, Rect, Text};
use crate::services::apps::icon_for_class;
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::hypr::Client;
use creed_core::Color;

/// Windows in a stable order: current workspace first, then by workspace and recency.
pub fn windows(data: &Data) -> Vec<&Client> {
    let mut v: Vec<&Client> = data.clients.iter().filter(|c| c.workspace.id > 0 && c.mapped && !c.hidden).collect();
    v.sort_by_key(|c| (c.workspace.id != data.active_ws, c.workspace.id, c.focus_history_id));
    v
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    let (sw, sh) = ui.size;
    // Dimmed backdrop.
    let dim = if ui.th.dark { 0.45 } else { 0.3 };
    gfx::fill_rounded(ui.cr, Rect::new(0.0, 0.0, sw, sh), 0.0, Color::rgba(0.0, 0.0, 0.0, dim * t));
    ui.backdrop(Action::Close(Kind::Overview));
    ui.cr.save().ok();
    ui.cr.push_group();
    if views.overview_launchpad {
        launchpad(ui, data, views);
    } else {
        match ui.th.cfg.style.overview.as_str() {
            "taskview" => super::win11::task_view(ui, data, views),
            "mission" => cards(ui, data, views, true),
            _ => workspace_map(ui, data, views),
        }
    }
    let _ = ui.cr.pop_group_to_source();
    let s = 0.96 + 0.04 * t;
    ui.cr.save().ok();
    ui.cr.translate(sw / 2.0, sh / 2.0);
    ui.cr.scale(s, s);
    ui.cr.translate(-sw / 2.0, -sh / 2.0);
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
    ui.cr.restore().ok();
}

fn window_card(ui: &mut Ui, r: Rect, c: &Client, data: &Data, selected: bool, idx: usize) {
    let th = ui.th;
    let k = ui::idn("ov.win", &c.address);
    let hv = ui.hover_amt(k);
    let lift = ui.spring(k ^ 0x1, if hv > 0.5 || selected { 1.0 } else { 0.0 }, 0.0);
    let r = Rect::centered(r.cx(), r.cy(), r.w * (1.0 + 0.03 * lift), r.h * (1.0 + 0.03 * lift));
    gfx::shadow(ui.cr, r, th.radius_small() + 4.0, 12.0, 4.0, 0.35);
    gfx::fill_rounded(ui.cr, r, th.radius_small() + 4.0, th.pal.surface);
    let (title_bar, body) = r.take_top(34.0, 0.0);
    let icon = icon_for_class(&data.apps, &c.class);
    let (ic, rest) = title_bar.inset_xy(10.0, 0.0).take_left(20.0, 8.0);
    ui.icon(ic, &icon, 18.0, None);
    ui.text(rest, &c.title, &Text::new(th.fs * 0.9, th.pal.text).medium());
    // Body: large app icon on a tinted field (live thumbnails need compositor capture).
    let field = body.inset(6.0);
    gfx::fill_rounded(ui.cr, field, th.radius_small(), th.pal.surface_alt);
    ui.icon(field, &icon, (field.h.min(field.w) * 0.42).min(96.0).round(), None);
    if selected || hv > 0.0 {
        gfx::stroke_rounded(ui.cr, r.inset(-2.0), th.radius_small() + 6.0, 2.5, th.pal.accent.with_alpha(if selected { 1.0 } else { hv }));
    }
    let close = Rect::new(r.right() - 30.0, r.y + 5.0, 24.0, 24.0);
    let hit = ui.hit(k, r, Action::FocusWindow(c.address.clone()), Role::Item, format!("{}: {}", Data::pretty_class(&c.class), c.title));
    hit.secondary = Some(Action::CloseWindow(c.address.clone()));
    if hv > 0.05 {
        ui.icon_button(ui::idn("ov.close", &c.address), close, "window-close-symbolic", "Close window", Action::CloseWindow(c.address.clone()), false);
    }
    let _ = idx;
}

/// Grid layout that keeps cards at roughly 16:10 and centres the last row.
pub fn grid_pub(area: Rect, n: usize, aspect: f64, gap: f64) -> Vec<Rect> {
    grid(area, n, aspect, gap)
}

fn grid(area: Rect, n: usize, aspect: f64, gap: f64) -> Vec<Rect> {
    if n == 0 {
        return vec![];
    }
    let mut best = (1usize, 0.0f64);
    for cols in 1..=n {
        let rows = (n + cols - 1) / cols;
        let cw = (area.w - gap * (cols as f64 - 1.0)) / cols as f64;
        let ch = (area.h - gap * (rows as f64 - 1.0)) / rows as f64;
        let w = cw.min(ch * aspect);
        if w > best.1 {
            best = (cols, w);
        }
    }
    let cols = best.0;
    let rows = (n + cols - 1) / cols;
    let w = best.1.min(460.0);
    let h = w / aspect;
    let total_h = rows as f64 * h + (rows as f64 - 1.0) * gap;
    let y0 = area.y + (area.h - total_h) / 2.0;
    let mut out = vec![];
    for i in 0..n {
        let row = i / cols;
        let in_row = if row == rows - 1 { n - row * cols } else { cols };
        let row_w = in_row as f64 * w + (in_row as f64 - 1.0) * gap;
        let x0 = area.x + (area.w - row_w) / 2.0;
        out.push(Rect::new(x0 + (i % cols) as f64 * (w + gap), y0 + row as f64 * (h + gap), w, h).round());
    }
    out
}

fn desktops_strip(ui: &mut Ui, r: Rect, data: &Data) {
    let th = ui.th;
    let list = super::bar::ws_list(data, th.cfg.bar.workspaces);
    let cw = 150.0;
    let total = list.len() as f64 * (cw + 12.0) - 12.0;
    let x0 = r.cx() - total / 2.0;
    for (i, id) in list.iter().enumerate() {
        let c = Rect::new(x0 + i as f64 * (cw + 12.0), r.y, cw, r.h);
        let k = ui::idn("ov.ws", id);
        let hv = ui.hover_amt(k);
        let active = *id == data.active_ws;
        gfx::fill_rounded(ui.cr, c, th.radius_small(), th.pal.surface.with_alpha(0.85));
        th.hover_bg(ui.cr, c, th.radius_small(), hv);
        if active {
            gfx::stroke_rounded(ui.cr, c, th.radius_small(), 2.0, th.pal.accent);
        }
        let n = data.clients.iter().filter(|cl| cl.workspace.id == *id).count();
        ui.text(c.inset_xy(10.0, 0.0), &format!("Desktop {id}"), &Text::new(th.fs * 0.9, th.pal.text).medium());
        if n > 0 {
            ui.text(c.inset_xy(10.0, 0.0), &format!("{n}"), &Text::new(th.fs * 0.85, th.pal.muted).end());
        }
        ui.hit(k, c, Action::Workspace(*id), Role::Button, format!("Desktop {id}"));
    }
}

fn cards(ui: &mut Ui, data: &Data, views: &mut Views, mission: bool) {
    let (sw, sh) = ui.size;
    let wins = windows(data);
    let strip_h = 64.0;
    let (strip, area) = if mission {
        let (s, a) = Rect::new(0.0, 0.0, sw, sh).inset(40.0).take_top(strip_h, 30.0);
        (s, a)
    } else {
        let (s, a) = Rect::new(0.0, 0.0, sw, sh).inset(40.0).take_bottom(strip_h, 30.0);
        (s, a)
    };
    ui.record("overview.area", area, None);
    desktops_strip(ui, strip, data);
    if wins.is_empty() {
        super::empty(ui, area, "focus-windows-symbolic", "No open windows");
        return;
    }
    let rects = grid(area, wins.len(), 1.45, 28.0);
    let sel = views.sel.min(wins.len() - 1);
    for (i, (c, r)) in wins.iter().zip(rects).enumerate() {
        window_card(ui, r, c, data, i == sel, i);
    }
}

fn workspace_map(ui: &mut Ui, data: &Data, views: &mut Views) {
    let th = ui.th;
    let (sw, sh) = ui.size;
    let list = super::bar::ws_list(data, th.cfg.bar.workspaces);
    let mon = data.monitors.iter().find(|m| m.focused).or(data.monitors.first());
    let (mw, mh, mx, my) = mon.map(|m| ((m.width as f64 / m.scale.max(0.1)), (m.height as f64 / m.scale.max(0.1)), m.x as f64, m.y as f64)).unwrap_or((sw, sh, 0.0, 0.0));
    let area = Rect::new(0.0, 0.0, sw, sh).inset_xy(60.0, 80.0);
    let rects = grid(area, list.len(), mw / mh, 26.0);
    let wins = windows(data);
    let sel_addr = wins.get(views.sel).map(|c| c.address.clone());
    for (id, r) in list.iter().zip(rects) {
        let k = ui::idn("ov.map", id);
        let hv = ui.hover_amt(k);
        let active = *id == data.active_ws;
        gfx::shadow(ui.cr, r, th.radius(), 14.0, 4.0, 0.3);
        th.panel(ui.cr, r, th.radius(), "overview", ui.pointer);
        th.hover_bg(ui.cr, r, th.radius(), hv * 0.6);
        if active {
            gfx::stroke_rounded(ui.cr, r.inset(-3.0), th.radius() + 3.0, 2.5, th.pal.accent);
        }
        ui.hit(k, r, Action::Workspace(*id), Role::Button, format!("Workspace {id}"));
        ui.text(Rect::new(r.x + 12.0, r.y + 8.0, 80.0, 22.0), &id.to_string(), &Text::new(th.fs * 1.1, th.pal.subtext).bold());
        // Windows as proportional tiles.
        let s = (r.w / mw).min(r.h / mh);
        for c in data.clients.iter().filter(|c| c.workspace.id == *id) {
            let wr = Rect::new(r.x + (c.at[0] as f64 - mx) * s, r.y + (c.at[1] as f64 - my) * s, c.size[0] as f64 * s, c.size[1] as f64 * s).inset(2.0);
            if wr.w < 8.0 || wr.h < 8.0 {
                continue;
            }
            let wk = ui::idn("ov.mapwin", &c.address);
            let whv = ui.hover_amt(wk);
            gfx::fill_rounded(ui.cr, wr, th.radius_small(), th.pal.surface_alt.mix(th.pal.overlay, whv));
            let selected = sel_addr.as_deref() == Some(c.address.as_str());
            if selected || whv > 0.0 {
                gfx::stroke_rounded(ui.cr, wr, th.radius_small(), 2.0, th.pal.accent.with_alpha(if selected { 1.0 } else { whv }));
            }
            let icon = icon_for_class(&data.apps, &c.class);
            ui.icon(wr, &icon, (wr.w.min(wr.h) * 0.4).clamp(12.0, 64.0).round(), None);
            let hit = ui.hit(wk, wr, Action::FocusWindow(c.address.clone()), Role::Item, format!("{}: {}", Data::pretty_class(&c.class), c.title));
            hit.secondary = Some(Action::CloseWindow(c.address.clone()));
        }
    }
}

fn launchpad(ui: &mut Ui, data: &Data, views: &mut Views) {
    let th = ui.th;
    let (sw, sh) = ui.size;
    let input = Rect::centered(sw / 2.0, 70.0, 280.0, 36.0);
    let pre = views.preedit.clone();
    ui.input(ui::id("launchpad.input"), input, &views.query, "Search", true, &pre);
    let res = launcher::search(&data.apps, &views.query);
    let cols = 7usize;
    let rows_per_page = 5usize;
    let per_page = cols * rows_per_page;
    let page = (views.sel / per_page).min(res.len().saturating_sub(1) / per_page.max(1));
    let area = Rect::new(0.0, 120.0, sw, sh - 200.0).inset_xy((sw * 0.12).round(), 10.0);
    let cw = area.w / cols as f64;
    let ch = area.h / rows_per_page as f64;
    let icon = (cw.min(ch) * 0.5).clamp(48.0, 96.0).round();
    for (slot, &ai) in res.iter().skip(page * per_page).take(per_page).enumerate() {
        let a = &data.apps[ai];
        let r = Rect::new(area.x + (slot % cols) as f64 * cw, area.y + (slot / cols) as f64 * ch, cw, ch);
        let k = ui::idn("lp.app", &a.id);
        let hv = ui.hover_amt(k);
        let selected = page * per_page + slot == views.sel;
        let press = if ui.is_pressed(k) { 0.9 } else { 1.0 };
        let ir = Rect::centered(r.cx(), r.cy() - 12.0, icon * press, icon * press);
        if selected {
            gfx::fill_rounded(ui.cr, ir.inset(-10.0), 22.0, Color::rgba(1.0, 1.0, 1.0, 0.14));
        }
        ui.icon(ir, &a.icon, ir.w.round(), None);
        let _ = hv;
        ui.text(Rect::new(r.x + 6.0, ir.bottom() + 8.0, r.w - 12.0, 20.0), &a.name, &Text::new(th.fs * 0.92, Color::rgba(1.0, 1.0, 1.0, 0.95)).center());
        ui.hit(k, r, Action::Launch(a.id.clone()), Role::Button, a.name.clone());
    }
    // Page dots.
    let pages = (res.len() + per_page - 1) / per_page.max(1);
    if pages > 1 {
        let y = sh - 60.0;
        let total = pages as f64 * 16.0;
        for p in 0..pages {
            let c = Rect::centered(sw / 2.0 - total / 2.0 + p as f64 * 16.0 + 8.0, y, 8.0, 8.0);
            gfx::fill_rounded(ui.cr, c, 4.0, Color::rgba(1.0, 1.0, 1.0, if p == page { 0.9 } else { 0.35 }));
            ui.hit(ui::idn("lp.page", p), c.inset(-4.0), Action::Page((p * per_page) as u32), Role::Button, format!("Page {}", p + 1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn grid_fits_and_centres() {
        let area = Rect::new(0.0, 0.0, 1000.0, 600.0);
        for n in 1..12 {
            let g = grid(area, n, 1.5, 20.0);
            assert_eq!(g.len(), n);
            for r in &g {
                assert!(r.x >= -0.5 && r.right() <= 1000.5 && r.y >= -0.5 && r.bottom() <= 600.5, "{n}: {r:?}");
            }
            // first row centred
            let row: Vec<&Rect> = g.iter().filter(|r| (r.y - g[0].y).abs() < 1.0).collect();
            let left = row.first().unwrap().x - area.x;
            let right = area.right() - row.last().unwrap().right();
            assert!((left - right).abs() <= 1.5, "{n}: {left} vs {right}");
        }
    }
}
