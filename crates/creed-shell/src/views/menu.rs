//! Tray context menus (DBusMenu).

use super::{open_amount, Views};
use crate::data::Data;
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Menu));
    if let Some((_, x)) = views.app_menu.clone() {
        ui.cr.save().ok();
        ui.cr.push_group();
        super::mac::draw_app_menu(ui, t, data, x);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        return;
    }
    let th = ui.th;
    let row_h = 32.0;
    let entries = &data.menu;
    let h = if entries.is_empty() { 60.0 } else { entries.iter().map(|e| if e.separator { 9.0 } else { row_h }).sum::<f64>() + 12.0 };
    let w = 260.0;
    // Under the tray: right side of the bar.
    let panel = super::popup_anchor(ui, w, h, gfx::Align::End);
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, panel, t, th.cfg.bar.position == creed_core::config::Edge::Top);
    th.panel(ui.cr, panel, th.radius_small() + 4.0, "menu", ui.pointer);
    ui.record("menu.panel", panel, None);
    ui.hit(ui::id("menu.panel"), panel, Action::None, Role::Group, "");
    let mut y = panel.y + 6.0;
    if entries.is_empty() {
        ui.text(panel, "Loading…", &Text::new(th.fs, th.pal.muted).center());
    }
    for e in entries {
        if e.separator {
            gfx::fill_rounded(ui.cr, Rect::new(panel.x + 10.0, y + 4.0, panel.w - 20.0, 1.0), 0.0, th.pal.text.with_alpha(0.1));
            y += 9.0;
            continue;
        }
        let r = Rect::new(panel.x + 6.0, y, panel.w - 12.0, row_h);
        let k = ui::idn("menu.item", e.id);
        let hv = if e.enabled { ui.hover_amt(k) } else { 0.0 };
        th.hover_bg(ui.cr, r, th.radius_small(), hv);
        let mut tr = r.inset_xy(10.0, 0.0);
        if let Some(on) = e.toggle {
            let (c, rest) = tr.take_left(18.0, 8.0);
            if on {
                ui.icon(c, "object-select-symbolic", 14.0, Some(th.pal.accent));
            }
            tr = rest;
        }
        let col = if e.enabled { th.pal.text } else { th.pal.muted };
        ui.text(tr, &e.label.replace('_', ""), &Text::new(th.fs, col));
        if e.enabled {
            ui.hit(k, r, Action::MenuItem(e.id), Role::Item, e.label.replace('_', ""));
        }
        y += row_h;
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}
