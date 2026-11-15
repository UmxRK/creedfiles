//! Clipboard history (cliphist).

use super::{open_amount, Views};
use crate::data::Data;
use crate::gfx::{Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};

pub fn filtered<'a>(data: &'a Data, q: &str) -> Vec<&'a (String, String)> {
    let q = q.to_lowercase();
    data.clips.iter().filter(|(_, t)| q.is_empty() || t.to_lowercase().contains(&q)).collect()
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Clipboard));
    let th = ui.th;
    let (sw, sh) = ui.size;
    let panel = Rect::new(((sw - 560.0) / 2.0).round(), (sh * 0.16).round(), 560.0, 520.0f64.min(sh * 0.7).round());
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, panel, t, true);
    th.panel(ui.cr, panel, th.radius() + 4.0, "launcher", ui.pointer);
    ui.record("clipboard.panel", panel, None);
    ui.hit(ui::id("clip.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(12.0);
    let (inp, list) = inner.take_top(46.0, 8.0);
    let pre = views.preedit.clone();
    ui.input(ui::id("clip.input"), inp, &views.query, "Search clipboard history", true, &pre);
    let items = filtered(data, &views.query);
    if items.is_empty() {
        let msg = if data.clips.is_empty() { "Clipboard history is empty (is cliphist running?)" } else { "No matches" };
        super::empty(ui, list, "edit-paste-symbolic", msg);
    } else {
        let from = ui.hits.len();
        let rows = super::list_layout(views, list, items.len(), 44.0, 2.0);
        let sel = views.sel;
        super::with_clip(ui, list, |ui| {
            for (i, r) in rows {
                let (id, text) = items[i];
                let one_line: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let is_image = one_line.starts_with("[[ binary data");
                let label = if is_image { "Image".to_string() } else { one_line };
                ui.row(ui::idn("clip.row", id), r, Some(if is_image { "image-x-generic-symbolic" } else { "edit-paste-symbolic" }), &label, "", sel == i, Action::ClipboardPick(id.clone()));
            }
        });
        super::clip_hits(ui, from, list);
    }
    let _ = th;
    let _ = Text::new(0.0, creed_core::Color::BLACK);
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}
