//! Dock with optional macOS-style magnification and launch bounce.

use super::{Spec, Views};
use crate::data::Data;
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::config::Edge;
use creed_core::Color;
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

fn metrics(th: &gfx::Theme) -> (f64, f64, f64) {
    let icon = th.cfg.dock.icon_size as f64;
    let pad = (icon * 0.22).round();
    let gap = (icon * 0.14).round();
    (icon, pad, gap)
}

fn item_count(th: &gfx::Theme, data: &Data) -> usize {
    // pinned ∪ running (+ launcher button)
    let mut n = th.cfg.dock.pinned.len();
    let pinned: Vec<String> = th.cfg.dock.pinned.iter().map(|p| p.to_lowercase()).collect();
    let mut seen: Vec<String> = vec![];
    if th.cfg.dock.show_running {
        for c in data.clients.iter().filter(|c| c.workspace.id > 0 && !c.class.is_empty()) {
            let cl = c.class.to_lowercase();
            if !pinned.iter().any(|p| *p == cl || p.ends_with(&format!(".{cl}")) || cl.ends_with(&format!(".{p}"))) && !seen.contains(&cl) {
                seen.push(cl);
                n += 1;
            }
        }
    }
    n + 1
}

pub fn spec(th: &gfx::Theme, data: &Data, views: &Views, ow: u32, oh: u32) -> Spec {
    let d = &th.cfg.dock;
    let (icon, pad, gap) = metrics(th);
    let n = item_count(th, data) as f64;
    let base_len = n * icon + (n - 1.0).max(0.0) * gap + pad * 2.0 + 12.0;
    let headroom = icon * (d.magnify - 1.0).max(0.0) + 12.0;
    let thick = icon + pad * 2.0;
    let margin = d.margin as f64;
    let hidden = d.autohide && views.dock_pointer.is_none();
    let vertical = matches!(d.position, Edge::Left | Edge::Right);
    let max_len = if vertical { oh as f64 } else { ow as f64 };
    let len = (base_len + icon * (d.magnify - 1.0) * 2.0).min(max_len - 2.0 * margin).max(thick);
    let depth = if hidden { 4.0 } else { thick + margin + headroom };
    let (anchor, size) = match d.position {
        Edge::Bottom => (Anchor::BOTTOM, (len, depth)),
        Edge::Top => (Anchor::TOP, (len, depth)),
        Edge::Left => (Anchor::LEFT, (depth, len)),
        Edge::Right => (Anchor::RIGHT, (depth, len)),
    };
    // The bar shares an edge with the dock in some layouts: the compositor stacks exclusive
    // zones, so a bottom dock under a bottom bar sits above it automatically.
    Spec {
        layer: Layer::Top,
        anchor,
        size: (size.0.round() as u32, size.1.round() as u32),
        margin: (0, 0, 0, 0),
        exclusive: if d.autohide { 0 } else { (thick + margin) as i32 },
        keyboard: KeyboardInteractivity::None,
        namespace: "creed-dock",
        click_through: false,
    }
}

pub fn draw(ui: &mut Ui, data: &Data, views: &mut Views) {
    let th = ui.th;
    let d = &th.cfg.dock;
    let (sw, sh) = ui.size;
    let vertical = matches!(d.position, Edge::Left | Edge::Right);
    if d.autohide && views.dock_pointer.is_none() {
        // Thin hot edge - invisible but hoverable.
        ui.hit(ui::id("dock.edge"), Rect::new(0.0, 0.0, sw, sh), Action::None, Role::Group, "Dock");
        return;
    }
    let (icon, pad, gap) = metrics(th);
    let margin = d.margin as f64;
    let thick = icon + pad * 2.0;
    let variant = th.cfg.style.dock.clone();
    let mut items = super::bar::taskbar_items(ui, data);
    let n_pinned = th.cfg.dock.pinned.len();
    if !d.show_running {
        items.truncate(n_pinned);
    }
    // Leading "apps" button.
    let mut entries: Vec<(String, String, Vec<String>, bool)> = vec![("__apps".into(), "view-app-grid-symbolic".into(), vec![], false)];
    for (i, (id, ic, w)) in items.into_iter().enumerate() {
        entries.push((id, ic, w, i == n_pinned && i > 0));
    }

    // Magnification: scale per item from pointer distance along the main axis.
    let pointer_main = views.dock_pointer.map(|(x, y)| if vertical { y } else { x });
    let n = entries.len();
    let base_len = n as f64 * icon + (n as f64 - 1.0) * gap;
    let main_extent = if vertical { sh } else { sw };
    let start = (main_extent - base_len) / 2.0;
    let mut scales = vec![1.0; n];
    if d.magnify > 1.0 {
        if let Some(pm) = pointer_main {
            for (i, s) in scales.iter_mut().enumerate() {
                let center = start + i as f64 * (icon + gap) + icon / 2.0;
                let dist = (pm - center) / (icon * 2.4);
                *s = 1.0 + (d.magnify - 1.0) * (-dist * dist * 2.2).exp();
            }
        }
    }
    let scales: Vec<f64> = scales.iter().enumerate().map(|(i, &s)| ui.spring(ui::idn("dock.mag", i), s, 1.0)).collect();
    let total: f64 = scales.iter().map(|s| s * icon).sum::<f64>() + gap * (n as f64 - 1.0);
    let panel_len = total + pad * 2.0;

    // Panel rect along the docking edge.
    let panel = match d.position {
        Edge::Bottom => Rect::new((sw - panel_len) / 2.0, sh - margin - thick, panel_len, thick),
        Edge::Top => Rect::new((sw - panel_len) / 2.0, margin, panel_len, thick),
        Edge::Left => Rect::new(margin, (sh - panel_len) / 2.0, thick, panel_len),
        Edge::Right => Rect::new(sw - margin - thick, (sh - panel_len) / 2.0, thick, panel_len),
    };
    ui.record("dock", panel, None);
    let radius = match variant.as_str() {
        "mac" => (thick * 0.36).round(),
        "aqua" => 6.0,
        _ => th.radius().min(thick / 2.0),
    };
    if variant == "aqua" {
        draw_aqua_shelf(ui, panel);
    } else {
        th.panel(ui.cr, panel, radius, "dock", ui.pointer);
    }

    let mut off = pad;
    for (i, (id, ic, wins, sep_before)) in entries.iter().enumerate() {
        let s = scales[i];
        let sz = icon * s;
        if *sep_before {
            let sep = if vertical {
                Rect::new(panel.x + pad, panel.y + off - gap / 2.0 - 0.5, thick - 2.0 * pad, 1.0)
            } else {
                Rect::new(panel.x + off - gap / 2.0 - 0.5, panel.y + pad, 1.0, thick - 2.0 * pad)
            };
            gfx::fill_rounded(ui.cr, sep, 0.0, th.pal.text.with_alpha(0.18));
        }
        // Icons grow away from the edge.
        let r = match d.position {
            Edge::Bottom => Rect::new(panel.x + off, panel.bottom() - pad - sz, sz, sz),
            Edge::Top => Rect::new(panel.x + off, panel.y + pad, sz, sz),
            Edge::Left => Rect::new(panel.x + pad, panel.y + off, sz, sz),
            Edge::Right => Rect::new(panel.right() - pad - sz, panel.y + off, sz, sz),
        };
        let k = ui::idn("dock.item", id);
        let hv = ui.hover_amt(k);
        let press = if ui.is_pressed(k) { 0.9 } else { 1.0 };
        let dr = Rect::centered(r.cx(), r.cy(), r.w * press, r.h * press);
        if d.magnify <= 1.0 {
            ui.th.hover_bg(ui.cr, dr.inset(-4.0), th.radius_small(), hv);
        }
        if id == "__apps" {
            let tile = dr.inset(sz * 0.06);
            gfx::fill_rounded(ui.cr, tile, tile.w * 0.26, th.pal.accent.with_alpha(0.18));
            ui.icon(tile, ic, (sz * 0.5).round(), Some(th.pal.accent));
            let target = if variant == "mac" || variant == "aqua" { Kind::Overview } else { Kind::Launcher };
            ui.hit(k, r, if target == Kind::Overview { Action::Exec("creed toggle launchpad".into()) } else { Action::Toggle(target) }, Role::Button, "Applications");
        } else {
            ui.icon(dr, ic, dr.w.round(), None);
            let active = wins.iter().any(|a| *a == data.active_addr);
            if !wins.is_empty() {
                let dot = (icon * 0.09).round().max(4.0);
                let len = if variant == "creed" && active { dot * 2.6 } else { dot };
                let len = ui.spring(k ^ 0x44, len, len);
                let c = if active { th.pal.accent } else { th.pal.text.with_alpha(0.55) };
                let dr2 = match d.position {
                    Edge::Bottom => Rect::centered(r.cx(), panel.bottom() - pad * 0.45, len, dot),
                    Edge::Top => Rect::centered(r.cx(), panel.y + pad * 0.45, len, dot),
                    Edge::Left => Rect::centered(panel.x + pad * 0.45, r.cy(), dot, len),
                    Edge::Right => Rect::centered(panel.right() - pad * 0.45, r.cy(), dot, len),
                };
                gfx::fill_rounded(ui.cr, dr2, dot / 2.0, c);
            }
            // Tooltip-style label above the hovered icon.
            if hv > 0.05 && !vertical {
                let name = data.apps.iter().find(|a| &a.id == id).map(|a| a.name.clone()).unwrap_or_else(|| Data::pretty_class(id));
                let t = Text::new(th.fs * 0.9, th.pal.text).medium();
                let (tw, thh) = ui.measure(&name, &t);
                let lr = Rect::centered(r.cx(), if d.position == Edge::Bottom { r.y - thh - 10.0 } else { r.bottom() + thh + 10.0 }, tw + 18.0, thh + 8.0);
                let lr = Rect::new(lr.x.clamp(2.0, sw - lr.w - 2.0), lr.y.max(1.0), lr.w, lr.h);
                ui.cr.save().ok();
                ui.cr.push_group();
                gfx::fill_rounded(ui.cr, lr, lr.h / 2.0, th.pal.surface_alt);
                ui.text(lr, &name, &t.center());
                let _ = ui.cr.pop_group_to_source();
                let _ = ui.cr.paint_with_alpha(hv);
                ui.cr.restore().ok();
            }
            let action = match wins.as_slice() {
                [] => Action::Launch(id.clone()),
                many => {
                    let idx = many.iter().position(|a| *a == data.active_addr).map(|i| (i + 1) % many.len()).unwrap_or(0);
                    Action::FocusWindow(many[idx].clone())
                }
            };
            if ui.focus == Some(k) {
                ui.th.focus_ring(ui.cr, dr, th.radius_small());
            }
            let label = data.apps.iter().find(|a| &a.id == id).map(|a| a.name.clone()).unwrap_or_else(|| Data::pretty_class(id));
            let hit = ui.hit(k, r, action, Role::Button, format!("{label}{}", if wins.is_empty() { "" } else { ", running" }));
            hit.secondary = Some(Action::Launch(id.clone()));
        }
        off += sz + gap;
    }
}

/// Aqua-era shelf: translucent slab with a bright top edge and reflective floor.
fn draw_aqua_shelf(ui: &mut Ui, r: Rect) {
    let cr = ui.cr;
    let slab = Rect::new(r.x, r.y + r.h * 0.35, r.w, r.h * 0.65);
    gfx::shadow(cr, slab, 6.0, 8.0, 2.0, 0.25);
    let g = cairo::LinearGradient::new(0.0, slab.y, 0.0, slab.bottom());
    g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.55);
    g.add_color_stop_rgba(0.15, 0.85, 0.88, 0.92, 0.45);
    g.add_color_stop_rgba(1.0, 0.55, 0.6, 0.68, 0.55);
    gfx::rounded(cr, slab, 6.0);
    let _ = cr.set_source(&g);
    let _ = cr.fill();
    gfx::fill_rounded(cr, Rect::new(slab.x, slab.y, slab.w, 1.5), 0.0, Color::rgba(1.0, 1.0, 1.0, 0.9));
}
