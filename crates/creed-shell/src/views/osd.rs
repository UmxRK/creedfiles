//! On-screen display for volume / brightness / Game Mode changes. Click-through.

use super::Spec;
use crate::data::{Data, OsdKind};
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Ui};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

pub fn spec(th: &gfx::Theme, _ow: u32, _oh: u32) -> Spec {
    let bottom_bar = th.cfg.bar.position == creed_core::config::Edge::Bottom;
    let dock = if th.cfg.dock.enabled && th.cfg.style.dock != "none" { th.cfg.dock.icon_size + 40 } else { 0 };
    let off = if bottom_bar { th.cfg.bar.size + th.cfg.bar.margin } else { 0 } + dock + 24;
    Spec {
        layer: Layer::Overlay,
        anchor: Anchor::BOTTOM,
        size: (320, 64),
        margin: (0, 0, off as i32, 0),
        exclusive: -1,
        keyboard: KeyboardInteractivity::None,
        namespace: "creed-osd",
        click_through: true,
    }
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data) {
    let t = super::open_amount(ui, closing);
    let Some((kind, value, _)) = data.osd else { return };
    let th = ui.th;
    let (w, h) = ui.size;
    let panel = Rect::new(4.0, 4.0, w - 8.0, h - 8.0);
    ui.cr.save().ok();
    ui.cr.push_group();
    th.panel(ui.cr, panel, panel.h / 2.0, "osd", None);
    let inner = panel.inset_xy(20.0, 0.0);
    let (icon, rest) = inner.take_left(24.0, 14.0);
    let (name, pct) = match kind {
        OsdKind::Volume => (if value <= 0.0 { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" }, Some(value)),
        OsdKind::Mic => ("audio-input-microphone-symbolic", Some(value)),
        OsdKind::Brightness => ("display-brightness-symbolic", Some(value)),
        OsdKind::Game => ("input-gaming-symbolic", None),
    };
    ui.icon(icon, name, 22.0, Some(th.pal.text));
    match pct {
        Some(v) => {
            let (bar, label) = rest.take_right(44.0, 12.0);
            let v = ui.spring(ui::id("osd.v"), v.clamp(0.0, 1.0), v.clamp(0.0, 1.0));
            let track = Rect::centered(bar.cx(), bar.cy(), bar.w, 6.0);
            gfx::fill_rounded(ui.cr, track, 3.0, th.pal.text.with_alpha(0.15));
            gfx::fill_rounded(ui.cr, Rect::new(track.x, track.y, (track.w * v).max(6.0), 6.0), 3.0, th.pal.accent);
            ui.text(label, &format!("{:.0}", v * 100.0), &Text::new(th.fs, th.pal.text).medium().end());
        }
        None => {
            ui.text(rest, if value > 0.5 { "Game Mode on - effects paused" } else { "Game Mode off" }, &Text::new(th.fs, th.pal.text).medium());
        }
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}
