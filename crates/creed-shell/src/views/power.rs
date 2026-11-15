//! Power menu.

use super::{open_amount, Views};
use crate::data::Data;
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::Color;

pub const ITEMS: &[(&str, &str, &str)] = &[
    ("lock", "Lock", "system-lock-screen-symbolic"),
    ("suspend", "Sleep", "weather-clear-night-symbolic"),
    ("logout", "Log out", "system-log-out-symbolic"),
    ("reboot", "Restart", "system-reboot-symbolic"),
    ("poweroff", "Shut down", "system-shutdown-symbolic"),
];

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    let (sw, sh) = ui.size;
    gfx::fill_rounded(ui.cr, Rect::new(0.0, 0.0, sw, sh), 0.0, Color::rgba(0.0, 0.0, 0.0, 0.35 * t));
    ui.backdrop(Action::Close(Kind::Power));
    let th = ui.th;
    let cell = 112.0;
    let w = ITEMS.len() as f64 * cell + 32.0;
    let panel = Rect::centered(sw / 2.0, sh / 2.0, w, 170.0);
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, panel, t, false);
    th.panel(ui.cr, panel, th.radius() + 6.0, "power", ui.pointer);
    ui.record("power.panel", panel, None);
    ui.hit(ui::id("power.panel"), panel, Action::None, Role::Group, "");
    let (hdr, row) = panel.inset(16.0).take_top(24.0, 8.0);
    let who = if data.user.is_empty() { "Power".to_string() } else { format!("{}@{}", data.user, data.host) };
    ui.text(hdr, &who, &Text::new(th.fs, th.pal.subtext).center());
    let cols = row.columns(ITEMS.len(), 0.0);
    for (i, (id, label, icon)) in ITEMS.iter().enumerate() {
        let c = cols[i].inset(6.0);
        let k = ui::idn("power.item", id);
        let hv = ui.hover_amt(k);
        let selected = views.sel == i;
        let circle = Rect::centered(c.cx(), c.y + 34.0, 60.0, 60.0);
        let danger = matches!(*id, "poweroff" | "reboot");
        let base = if selected { th.pal.accent } else if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.07) } else { Color::rgba(0.0, 0.0, 0.0, 0.06) };
        let hover_c = if danger { th.pal.error } else { th.pal.accent };
        gfx::fill_rounded(ui.cr, circle, 30.0, base.mix(hover_c, hv * 0.85));
        let fg = if selected || hv > 0.5 { th.pal.on_accent } else { th.pal.text };
        ui.icon(circle, icon, 24.0, Some(fg));
        ui.text(Rect::new(c.x, circle.bottom() + 8.0, c.w, 20.0), label, &Text::new(th.fs * 0.95, th.pal.text).center());
        if ui.focus == Some(k) {
            th.focus_ring(ui.cr, circle, 30.0);
        }
        ui.hit(k, c, Action::Power(id), Role::Button, *label);
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}
