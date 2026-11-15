//! Emoji picker with categories, search and recents. Data: Unicode emoji-test.txt (see data/NOTICE).

use super::{open_amount, Views};
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};

pub const COLS: usize = 9;
const DATA: &str = include_str!("../../data/emoji.tsv");

pub const CATEGORIES: &[(&str, &str)] = &[
    ("Recent", "document-open-recent-symbolic"),
    ("Smileys & Emotion", "face-smile-symbolic"),
    ("People & Body", "system-users-symbolic"),
    ("Animals & Nature", "weather-clear-symbolic"),
    ("Food & Drink", "emoji-food-symbolic"),
    ("Travel & Places", "mark-location-symbolic"),
    ("Activities", "applications-games-symbolic"),
    ("Objects", "emoji-objects-symbolic"),
    ("Symbols", "emoji-symbols-symbolic"),
    ("Flags", "emoji-flags-symbolic"),
];

pub fn all() -> impl Iterator<Item = (&'static str, &'static str, &'static str)> {
    DATA.lines().filter_map(|l| {
        let mut it = l.splitn(3, '\t');
        Some((it.next()?, it.next()?, it.next()?))
    })
}

/// (emoji, name) for the current query/category.
pub fn filtered(q: &str, cat: usize, recent: &[String]) -> Vec<(&'static str, &'static str)> {
    let q = q.trim().to_lowercase();
    if !q.is_empty() {
        let words: Vec<&str> = q.split_whitespace().collect();
        return all().filter(|(_, _, n)| words.iter().all(|w| n.contains(w))).map(|(_, e, n)| (e, n)).take(400).collect();
    }
    if cat == 0 {
        if recent.is_empty() {
            return all().take(COLS * 4).map(|(_, e, n)| (e, n)).collect();
        }
        return recent.iter().filter_map(|r| all().find(|(_, e, _)| *e == r.as_str()).map(|(_, e, n)| (e, n))).collect();
    }
    let name = CATEGORIES.get(cat).map(|c| c.0).unwrap_or("");
    all().filter(|(g, _, _)| *g == name).map(|(_, e, n)| (e, n)).collect()
}

pub fn draw(ui: &mut Ui, closing: bool, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Emoji));
    let th = ui.th;
    let (sw, sh) = ui.size;
    let cell = 46.0;
    let w = COLS as f64 * cell + 24.0;
    let panel = Rect::new(((sw - w) / 2.0).round(), (sh * 0.18).round(), w, 470.0);
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, panel, t, true);
    th.panel(ui.cr, panel, th.radius() + 4.0, "launcher", ui.pointer);
    ui.record("emoji.panel", panel, None);
    ui.hit(ui::id("emoji.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(12.0);
    let (inp, rest) = inner.take_top(42.0, 8.0);
    let pre = views.preedit.clone();
    ui.input(ui::id("emoji.input"), inp, &views.query, "Search emoji", true, &pre);
    let (tabs, grid) = rest.take_top(34.0, 6.0);
    let tw = tabs.w / CATEGORIES.len() as f64;
    for (i, (name, icon)) in CATEGORIES.iter().enumerate() {
        let r = Rect::new(tabs.x + i as f64 * tw, tabs.y, tw, tabs.h);
        ui.icon_button(ui::idn("emoji.tab", i), Rect::centered(r.cx(), r.cy(), 32.0, 32.0), icon, name, Action::Page(i as u32), views.emoji_cat == i && views.query.is_empty());
    }
    let items = filtered(&views.query, views.emoji_cat, &views.emoji_recent);
    let (grid, footer) = grid.take_bottom(24.0, 4.0);
    let (grid, footer) = (footer, grid);
    if items.is_empty() {
        super::empty(ui, grid, "face-uncertain-symbolic", "No emoji found");
    } else {
        let rows = (items.len() + COLS - 1) / COLS;
        let sel_row = views.sel / COLS;
        let total = rows as f64 * cell;
        let sel_top = sel_row as f64 * cell;
        if sel_top < views.scroll {
            views.scroll = sel_top;
        } else if sel_top + cell > views.scroll + grid.h {
            views.scroll = sel_top + cell - grid.h;
        }
        views.scroll = views.scroll.clamp(0.0, (total - grid.h).max(0.0));
        let from = ui.hits.len();
        let scroll = views.scroll;
        let sel = views.sel;
        super::with_clip(ui, grid, |ui| {
            for (i, (e, n)) in items.iter().enumerate() {
                let r = Rect::new(grid.x + (i % COLS) as f64 * cell, grid.y + (i / COLS) as f64 * cell - scroll, cell, cell);
                if r.bottom() < grid.y || r.y > grid.bottom() {
                    continue;
                }
                let k = ui::idn("emoji.e", e);
                let hv = ui.hover_amt(k);
                if sel == i {
                    gfx::fill_rounded(ui.cr, r.inset(3.0), 10.0, ui.th.pal.accent.with_alpha(0.25));
                }
                ui.th.hover_bg(ui.cr, r.inset(3.0), 10.0, hv);
                ui.text(r, e, &Text::new(26.0, ui.th.pal.text).center());
                ui.hit(k, r, Action::EmojiPick(e.to_string()), Role::Button, n.to_string());
            }
        });
        super::clip_hits(ui, from, grid);
        let name = items.get(views.sel).map(|x| x.1).unwrap_or("");
        ui.text(footer, name, &Text::new(th.fs * 0.9, th.pal.subtext).center());
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn data_loads_and_searches() {
        assert!(all().count() > 1500);
        let r = filtered("grinning", 0, &[]);
        assert!(r.iter().any(|(e, _)| *e == "😀"));
        assert!(filtered("", 1, &[]).len() > 50);
        assert_eq!(filtered("", 0, &["😀".into()]), vec![("😀", "grinning face")]);
        assert!(filtered("zzzzqq", 0, &[]).is_empty());
    }
}
