//! Notification popups and the notification center.

use super::{calendar, open_amount, popup_anchor, popup_transform, Spec, Views};
use crate::data::{Data, Notif};
use crate::gfx::{self, Align, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::config::{Corner, Edge};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

const W: f64 = 380.0;
const GAP: f64 = 10.0;

fn card_height(th: &gfx::Theme, n: &Notif) -> f64 {
    let fs = th.fs;
    let chars_per_line = ((W - 90.0) / (fs * 0.5)).max(10.0);
    let lines = if n.body.is_empty() { 0.0 } else { (n.body.chars().count() as f64 / chars_per_line).ceil().clamp(1.0, 3.0) };
    let actions = n.actions.iter().any(|(k, _)| k != "default");
    (16.0 * 2.0 + fs * 1.4 + lines * fs * 1.35 + if actions { 40.0 } else { 0.0 }).max(72.0).round()
}

pub fn spec(th: &gfx::Theme, data: &Data, _ow: u32, _oh: u32) -> Spec {
    let variant = th.cfg.style.notifications.as_str();
    let (cw, h) = match variant {
        "win11" => (364.0 + 24.0, data.notifs.iter().map(|n| super::win11::toast_height(n) + 12.0).sum::<f64>() + 12.0),
        "win10" => (364.0 + 24.0, data.notifs.iter().map(|n| super::win10::toast_height(n) + 12.0).sum::<f64>() + 12.0),
        "mac" => (344.0 + 24.0, data.notifs.iter().map(|n| super::mac::banner_height(n) + 10.0).sum::<f64>() + 12.0),
        _ => (W + 24.0, data.notifs.iter().map(|n| card_height(th, n) + GAP).sum::<f64>() + 16.0),
    };
    let corner = th.cfg.notifications.position;
    let anchor = match corner {
        Corner::TopRight => Anchor::TOP | Anchor::RIGHT,
        Corner::TopLeft => Anchor::TOP | Anchor::LEFT,
        Corner::BottomRight => Anchor::BOTTOM | Anchor::RIGHT,
        Corner::BottomLeft => Anchor::BOTTOM | Anchor::LEFT,
        Corner::TopCenter => Anchor::TOP,
    };
    Spec {
        layer: Layer::Overlay,
        anchor,
        size: (cw as u32, h.max(1.0) as u32),
        margin: (0, 0, 0, 0),
        exclusive: 0,
        keyboard: KeyboardInteractivity::None,
        namespace: "creed-notifications",
        click_through: false,
    }
}

fn card(ui: &mut Ui, r: Rect, n: &Notif, in_center: bool) {
    let th = ui.th;
    let p = &th.pal;
    let k = ui::idn("notif", n.id);
    let hv = ui.hover_amt(k);
    if in_center {
        let c = if th.dark { creed_core::Color::rgba(1.0, 1.0, 1.0, 0.05 + 0.03 * hv) } else { creed_core::Color::rgba(0.0, 0.0, 0.0, 0.035 + 0.03 * hv) };
        gfx::fill_rounded(ui.cr, r, th.radius_small() + 4.0, c);
    } else {
        th.panel(ui.cr, r, th.radius(), "notifications", ui.pointer);
        if n.urgency >= 2 {
            gfx::fill_rounded(ui.cr, Rect::new(r.x, r.y + 14.0, 3.0, r.h - 28.0), 1.5, p.error);
        }
    }
    let inner = r.inset(14.0);
    let (ic, rest) = inner.take_left(40.0, 12.0);
    let icon = if n.icon.is_empty() { "dialog-information-symbolic".to_string() } else { n.icon.clone() };
    let tint = if icon.ends_with("-symbolic") { Some(p.accent) } else { None };
    ui.icon(Rect::new(ic.x, ic.y, 40.0, 40.0), &icon, 36.0, tint);
    let lh = th.fs * 1.4;
    let (head, body) = rest.take_top(lh, 2.0);
    let ago = ago(n.arrived);
    let (time_r, title_r) = head.take_right(ui.measure(&ago, &Text::new(th.fs * 0.8, p.muted)).0 + 4.0, 6.0);
    ui.text(time_r, &ago, &Text::new(th.fs * 0.8, p.muted).end());
    ui.text(title_r, &n.summary, &Text::new(th.fs, p.text).bold());
    let has_actions = n.actions.iter().any(|(k, _)| k != "default");
    let body_r = if has_actions { Rect::new(body.x, body.y, body.w, body.h - 40.0) } else { body };
    if !n.body.is_empty() {
        let clean = strip_markup(&n.body);
        ui.text(body_r, &clean, &Text::new(th.fs * 0.95, p.subtext).lines(3));
    }
    let default_action = n.actions.iter().find(|(k, _)| k == "default").map(|(k, _)| Action::NotifAction(n.id, k.clone())).unwrap_or(Action::NotifDismiss(n.id));
    ui.hit(k, r, default_action, Role::Item, format!("Notification from {}: {}. {}", n.app, n.summary, strip_markup(&n.body)));
    if has_actions {
        let acts: Vec<&(String, String)> = n.actions.iter().filter(|(k, _)| k != "default").take(3).collect();
        let row = Rect::new(rest.x, r.bottom() - 14.0 - 32.0, rest.w, 32.0);
        let cols = row.columns(acts.len(), 6.0);
        for (i, (key, label)) in acts.iter().enumerate() {
            let ak = ui::idn(&format!("notif.act.{}", n.id), i);
            let ahv = ui.hover_amt(ak);
            th.chip(ui.cr, cols[i], 16.0, ahv, false);
            ui.text(cols[i], label, &Text::new(th.fs * 0.9, p.text).medium().center());
            ui.hit(ak, cols[i], Action::NotifAction(n.id, key.clone()), Role::Button, label.clone());
        }
    }
    // Close button appears on hover.
    if hv > 0.05 {
        let cb = Rect::new(r.right() - 30.0, r.y + 6.0, 24.0, 24.0);
        ui.cr.save().ok();
        ui.cr.push_group();
        ui.icon_button(ui::idn("notif.close", n.id), cb, "window-close-symbolic", "Dismiss", Action::NotifDismiss(n.id), false);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(hv);
        ui.cr.restore().ok();
    }
}

pub fn strip_markup(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        // Only `<` followed by a letter or `/` starts a tag; a lone `<` is literal text.
        if c == '<' && chars.peek().map(|n| n.is_ascii_alphabetic() || *n == '/').unwrap_or(false) {
            for t in chars.by_ref() {
                if t == '>' {
                    break;
                }
            }
            continue;
        }
        out.push(c);
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

pub fn ago(t: i64) -> String {
    let d = (time::now_secs() - t).max(0);
    if d < 60 {
        "now".into()
    } else if d < 3600 {
        format!("{}m", d / 60)
    } else if d < 86400 {
        format!("{}h", d / 3600)
    } else {
        format!("{}d", d / 86400)
    }
}

pub fn draw_popups(ui: &mut Ui, data: &Data) {
    match ui.th.cfg.style.notifications.as_str() {
        "win11" => return super::win11::toasts(ui, data),
        "win10" => return super::win10::toasts(ui, data),
        "mac" => return super::mac::banners(ui, data),
        _ => {}
    }
    let th = ui.th;
    let (sw, _) = ui.size;
    let top = matches!(th.cfg.notifications.position, Corner::TopLeft | Corner::TopRight | Corner::TopCenter);
    let mut y = if top { 8.0 } else { ui.size.1 - 8.0 };
    for n in &data.notifs {
        let h = card_height(th, n);
        let k = ui::idn("notif.in", n.id);
        let t = ui.spring(k, 1.0, 0.0);
        let r = if top { Rect::new((sw - W) / 2.0, y, W, h) } else { Rect::new((sw - W) / 2.0, y - h, W, h) };
        ui.cr.save().ok();
        ui.cr.push_group();
        ui.cr.translate((1.0 - t) * 40.0, 0.0);
        card(ui, r, n, false);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        ui.record(&format!("notif.{}", n.id), r, None);
        if top {
            y += h + GAP;
        } else {
            y -= h + GAP;
        }
    }
}

pub fn draw_center(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::NotifCenter));
    let v = ui.th.cfg.style.notifications.clone();
    if matches!(v.as_str(), "win11" | "win10" | "mac") {
        ui.cr.save().ok();
        ui.cr.push_group();
        match v.as_str() {
            "win11" => super::win11::notif_center(ui, t, data, views),
            "win10" => super::win10::action_center(ui, t, data, views),
            _ => super::mac::notif_center(ui, t, data, views),
        }
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        return;
    }
    let th = ui.th;
    let variant = th.cfg.style.notifications.clone();
    let (sw, sh) = ui.size;
    let bar = if th.cfg.bar.enabled { (th.cfg.bar.size + th.cfg.bar.margin) as f64 } else { 0.0 };
    let with_cal = variant == "win11";
    let panel = match variant.as_str() {
        "win11" => {
            let h = (sh - bar - 24.0).min(820.0);
            let y = if th.cfg.bar.position == Edge::Bottom { sh - bar - 12.0 - h } else { bar + 12.0 };
            Rect::new(sw - 400.0, y, 388.0, h)
        }
        "mac" => Rect::new(sw - 372.0, bar + 8.0, 360.0, (sh - bar - 16.0).min(760.0)),
        _ => popup_anchor(ui, 400.0, (sh - bar - 40.0).min(620.0), Align::End),
    };
    ui.cr.save().ok();
    ui.cr.push_group();
    popup_transform(ui, panel, t, th.cfg.bar.position == Edge::Top);
    th.panel(ui.cr, panel, th.radius() + 2.0, "notifications", ui.pointer);
    ui.record("notif-center.panel", panel, None);
    ui.hit(ui::id("nc.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(16.0);
    let (list_area, cal_area) = if with_cal { inner.take_top(inner.h - 330.0, 12.0) } else { (inner, Rect::default()) };
    let (hdr, list) = list_area.take_top(32.0, 10.0);
    let (title, btns) = hdr.take_left(hdr.w - 150.0, 4.0);
    ui.text(title, "Notifications", &Text::new(th.fs * 1.15, th.pal.text).bold());
    let b = btns.columns(2, 6.0);
    ui.icon_button(ui::id("nc.dnd"), Rect::centered(b[0].cx(), b[0].cy(), 32.0, 32.0), "notifications-disabled-symbolic", if data.dnd { "Do Not Disturb on" } else { "Do Not Disturb off" }, Action::ToggleDnd, data.dnd);
    if !data.history.is_empty() {
        let k = ui::id("nc.clear");
        let hv = ui.hover_amt(k);
        let cr = Rect::centered(b[1].cx(), b[1].cy(), b[1].w, 28.0);
        th.chip(ui.cr, cr, 14.0, hv, false);
        ui.text(cr, "Clear all", &Text::new(th.fs * 0.88, th.pal.text).center());
        ui.hit(k, cr, Action::NotifClear, Role::Button, "Clear all notifications");
    }
    if data.history.is_empty() {
        super::empty(ui, list, "notifications-symbolic", "You're all caught up");
    } else {
        let from = ui.hits.len();
        // Variable-height list: compute offsets manually.
        let heights: Vec<f64> = data.history.iter().map(|n| card_height(th, n)).collect();
        let total: f64 = heights.iter().sum::<f64>() + GAP * heights.len() as f64;
        views.scroll = views.scroll.clamp(0.0, (total - list.h).max(0.0));
        let scroll = views.scroll;
        super::with_clip(ui, list, |ui| {
            let mut y = list.y - scroll;
            for (n, h) in data.history.iter().zip(&heights) {
                if y + h >= list.y && y <= list.bottom() {
                    card(ui, Rect::new(list.x, y, list.w, *h), n, true);
                }
                y += h + GAP;
            }
        });
        super::clip_hits(ui, from, list);
    }
    if with_cal {
        let c = if th.dark { creed_core::Color::rgba(1.0, 1.0, 1.0, 0.04) } else { creed_core::Color::rgba(0.0, 0.0, 0.0, 0.03) };
        gfx::fill_rounded(ui.cr, cal_area, th.radius_small() + 4.0, c);
        calendar::draw_month(ui, cal_area.inset(12.0), views.calendar_offset);
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

#[cfg(test)]
mod tests {
    #[test]
    fn markup() {
        assert_eq!(super::strip_markup("<b>Hi</b> &amp; <i>bye</i>"), "Hi & bye");
        assert_eq!(super::strip_markup("a < b"), "a < b");
        assert_eq!(super::strip_markup("&amp;lt;"), "&lt;");
    }
}
