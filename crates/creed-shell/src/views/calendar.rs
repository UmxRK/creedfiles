//! Month calendar (used in the dashboard and as a standalone popup).

use super::{open_amount, popup_anchor, popup_transform, Views};
use crate::gfx::{self, Align, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Ui};

pub fn draw_month(ui: &mut Ui, r: Rect, offset: i32) {
    let th = ui.th;
    let now = time::local(time::now_secs());
    let (year, month) = time::add_months(now.year, now.month, offset);
    let (hdr, rest) = r.take_top(34.0, 6.0);
    let (prev, hdr) = hdr.take_left(32.0, 4.0);
    let (next, title) = hdr.take_right(32.0, 4.0);
    ui.icon_button(ui::id("cal.prev"), prev, "go-previous-symbolic", "Previous month", Action::Calendar(-1), false);
    ui.icon_button(ui::id("cal.next"), next, "go-next-symbolic", "Next month", Action::Calendar(1), false);
    let k = ui::id("cal.title");
    let hv = ui.hover_amt(k);
    th.hover_bg(ui.cr, title, th.radius_small(), hv * if offset != 0 { 1.0 } else { 0.0 });
    ui.text(title, &format!("{} {}", time::MONTHS[(month - 1) as usize], year), &Text::new(th.fs * 1.05, th.pal.text).bold().center());
    ui.hit(k, title, Action::Calendar(0), Role::Button, "Back to today");
    let (dow, grid) = rest.take_top(22.0, 2.0);
    // Monday-first week.
    let names = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
    let cw = dow.w / 7.0;
    for (i, n) in names.iter().enumerate() {
        ui.text(Rect::new(dow.x + i as f64 * cw, dow.y, cw, dow.h), n, &Text::new(th.fs * 0.82, th.pal.muted).bold().center());
    }
    let first = (time::weekday(year, month, 1) + 6) % 7; // 0 = Monday
    let days = time::days_in_month(year, month);
    let rows = ((first + days) as f64 / 7.0).ceil() as usize;
    let ch = (grid.h / rows.max(5) as f64).min(cw);
    for d in 1..=days {
        let idx = first + d - 1;
        let (cx, cy) = (idx % 7, idx / 7);
        let cell = Rect::new(grid.x + cx as f64 * cw, grid.y + cy as f64 * ch, cw, ch);
        let today = offset == 0 && d == now.day;
        let weekend = cx >= 5;
        if today {
            let s = ch.min(cw) - 4.0;
            gfx::fill_rounded(ui.cr, Rect::centered(cell.cx(), cell.cy(), s, s), s / 2.0, th.pal.accent);
        }
        let col = if today { th.pal.on_accent } else if weekend { th.pal.subtext } else { th.pal.text };
        let t = if today { Text::new(th.fs * 0.92, col).bold().center() } else { Text::new(th.fs * 0.92, col).center() };
        ui.text(cell, &d.to_string(), &t);
    }
}

pub fn draw_popup(ui: &mut Ui, closing: bool, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Calendar));
    let th = ui.th;
    let panel = popup_anchor(ui, 320.0, 320.0, Align::Center);
    ui.cr.save().ok();
    ui.cr.push_group();
    popup_transform(ui, panel, t, th.cfg.bar.position == creed_core::config::Edge::Top);
    th.panel(ui.cr, panel, th.radius(), "calendar", ui.pointer);
    ui.record("calendar.panel", panel, None);
    ui.hit(ui::id("calendar.panel"), panel, Action::None, Role::Group, "");
    draw_month(ui, panel.inset(14.0), views.calendar_offset);
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}
