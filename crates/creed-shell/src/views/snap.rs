//! Snap Layouts (Windows 11 style, available in every mode via Super+Z): pick a layout and a
//! zone; the active window floats into that zone. In macOS modes the same popup doubles as
//! the window menu (app name in the bar).

use super::{open_amount, Views};
use crate::data::Data;
use crate::gfx::{self, Rect, Text, Theme};
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::hypr::lua_str;

/// Layouts as lists of zones in unit coordinates (x, y, w, h).
pub const LAYOUTS: &[&[(f64, f64, f64, f64)]] = &[
    &[(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 0.5, 1.0)],
    &[(0.0, 0.0, 0.66, 1.0), (0.66, 0.0, 0.34, 1.0)],
    &[(0.0, 0.0, 0.333, 1.0), (0.333, 0.0, 0.334, 1.0), (0.667, 0.0, 0.333, 1.0)],
    &[(0.0, 0.0, 0.5, 1.0), (0.5, 0.0, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)],
    &[(0.0, 0.0, 0.5, 0.5), (0.5, 0.0, 0.5, 0.5), (0.0, 0.5, 0.5, 0.5), (0.5, 0.5, 0.5, 0.5)],
    &[(0.25, 0.0, 0.5, 1.0), (0.0, 0.0, 0.25, 1.0), (0.75, 0.0, 0.25, 1.0)],
];

/// Lua dispatchers that place `addr` into the zone on the focused monitor.
pub fn dispatch_for(layout: u32, zone: u32, addr: &str, data: &Data, th: &Theme) -> Option<Vec<String>> {
    let z = LAYOUTS.get(layout as usize)?.get(zone as usize)?;
    let mon = data.monitors.iter().find(|m| m.focused).or(data.monitors.first())?;
    let scale = mon.scale.max(0.1);
    let (mw, mh) = (mon.width as f64 / scale, mon.height as f64 / scale);
    // Usable area: respect bar exclusive zone and gaps.
    let gap = th.cfg.style.window_gaps_out as f64;
    let inner = th.cfg.style.window_gaps_in as f64;
    let bar = if th.cfg.bar.enabled { (th.cfg.bar.size + th.cfg.bar.margin) as f64 } else { 0.0 };
    let (mut ax, mut ay, mut aw, mut ah) = (mon.x as f64 + gap, mon.y as f64 + gap, mw - 2.0 * gap, mh - 2.0 * gap);
    match th.cfg.bar.position {
        creed_core::config::Edge::Top => {
            ay += bar;
            ah -= bar;
        }
        creed_core::config::Edge::Bottom => ah -= bar,
        creed_core::config::Edge::Left => {
            ax += bar;
            aw -= bar;
        }
        creed_core::config::Edge::Right => aw -= bar,
    }
    let x = (ax + z.0 * aw + if z.0 > 0.0 { inner } else { 0.0 }).round();
    let y = (ay + z.1 * ah + if z.1 > 0.0 { inner } else { 0.0 }).round();
    let w = (z.2 * aw - if z.0 > 0.0 { inner } else { 0.0 } - if z.0 + z.2 < 0.999 { inner } else { 0.0 }).round();
    let h = (z.3 * ah - if z.1 > 0.0 { inner } else { 0.0 } - if z.1 + z.3 < 0.999 { inner } else { 0.0 }).round();
    let win = lua_str(&format!("address:{addr}"));
    let mut v = vec![];
    if !addr.is_empty() {
        v.push(format!("hl.dsp.focus({{ window = {win} }})"));
    }
    v.push("hl.dsp.window.float({ action = \"set\" })".to_string());
    v.push(format!("hl.dsp.window.resize({{ x = {w}, y = {h} }})"));
    v.push(format!("hl.dsp.window.move({{ x = {x}, y = {y} }})"));
    Some(v)
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Snap));
    let th = ui.th;
    let (sw, _) = ui.size;
    let cell_w = 120.0;
    let cell_h = 74.0;
    let cols = 3;
    let rows = (LAYOUTS.len() + cols - 1) / cols;
    let target = data.clients.iter().find(|c| c.address == views.snap_target);
    let extra = 4.0 * 40.0 + 30.0; // window actions row
    let w = cols as f64 * (cell_w + 12.0) + 24.0;
    let h = rows as f64 * (cell_h + 12.0) + 24.0 + 30.0 + extra;
    let bar = if th.cfg.bar.enabled { (th.cfg.bar.size + th.cfg.bar.margin) as f64 } else { 0.0 };
    let y = if th.cfg.bar.position == creed_core::config::Edge::Top { bar + 10.0 } else { 14.0 };
    let panel = Rect::new(((sw - w) / 2.0).round(), y, w, h);
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, panel, t, true);
    th.panel(ui.cr, panel, th.radius() + 2.0, "snap", ui.pointer);
    ui.record("snap.panel", panel, None);
    ui.hit(ui::id("snap.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(12.0);
    let (hdr, rest) = inner.take_top(26.0, 4.0);
    let title = target.map(|c| format!("Snap “{}”", Data::pretty_class(&c.class))).unwrap_or_else(|| "No window focused".into());
    ui.text(hdr, &title, &Text::new(th.fs, th.pal.subtext).medium());
    let (grid, actions) = rest.take_top(rows as f64 * (cell_h + 12.0), 8.0);
    for (li, zones) in LAYOUTS.iter().enumerate() {
        let c = Rect::new(grid.x + (li % cols) as f64 * (cell_w + 12.0), grid.y + (li / cols) as f64 * (cell_h + 12.0), cell_w, cell_h);
        gfx::fill_rounded(ui.cr, c, th.radius_small(), th.pal.text.with_alpha(0.05));
        for (zi, z) in zones.iter().enumerate() {
            let zr = Rect::new(c.x + 6.0 + z.0 * (c.w - 12.0), c.y + 6.0 + z.1 * (c.h - 12.0), z.2 * (c.w - 12.0), z.3 * (c.h - 12.0)).inset(2.0);
            let k = ui::id(&format!("snap.{li}.{zi}"));
            let hv = ui.hover_amt(k);
            let col = th.pal.text.with_alpha(0.16).mix(th.pal.accent, hv);
            gfx::fill_rounded(ui.cr, zr, 5.0, col);
            if ui.focus == Some(k) {
                th.focus_ring(ui.cr, zr, 5.0);
            }
            if target.is_some() {
                ui.hit(k, zr, Action::Snap(li as u32, zi as u32), Role::Button, format!("Layout {} zone {}", li + 1, zi + 1));
            }
        }
    }
    // Window actions.
    if let Some(c) = target {
        let win = lua_str(&format!("address:{}", c.address));
        let acts: [(&str, &str, Action); 4] = [
            ("view-fullscreen-symbolic", "Full screen", Action::Hypr("hl.dsp.window.fullscreen({ mode = \"fullscreen\" })".into())),
            ("window-pin-symbolic", "Keep on top", Action::Hypr("hl.dsp.window.pin()".into())),
            ("focus-windows-symbolic", "Center", Action::Hypr("hl.dsp.window.center()".into())),
            ("window-close-symbolic", "Close", Action::Hypr(format!("hl.dsp.window.close({{ window = {win} }})"))),
        ];
        let cols4 = actions.take_top(36.0, 0.0).0.columns(4, 6.0);
        for (i, (icon, label, a)) in acts.into_iter().enumerate() {
            let k = ui::idn("snap.act", i);
            let r = cols4[i];
            let hv = ui.hover_amt(k);
            th.chip(ui.cr, r, 18.0, hv, false);
            let (ic, tr) = r.inset_xy(10.0, 0.0).take_left(18.0, 6.0);
            ui.icon(ic, icon, 16.0, Some(th.pal.text));
            ui.text(tr, label, &Text::new(th.fs * 0.85, th.pal.text));
            ui.hit(k, r, a, Role::Button, label);
        }
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

/// All dispatcher strings this module can emit (used by tests to validate with Hyprland).
pub fn sample_dispatches() -> Vec<String> {
    let mut v = vec![
        "hl.dsp.window.fullscreen({ mode = \"fullscreen\" })".to_string(),
        "hl.dsp.window.pin()".to_string(),
        "hl.dsp.window.center()".to_string(),
        format!("hl.dsp.window.close({{ window = {} }})", lua_str("address:0x1")),
    ];
    v.push(format!("hl.dsp.focus({{ window = {} }})", lua_str("address:0x1")));
    v.push("hl.dsp.window.float({ action = \"set\" })".into());
    v.push("hl.dsp.window.resize({ x = 800, y = 600 })".into());
    v.push("hl.dsp.window.move({ x = 10, y = 20 })".into());
    v
}
