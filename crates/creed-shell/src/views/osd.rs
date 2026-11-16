//! On-screen display for volume / brightness / Game Mode changes. Click-through.

use super::Spec;
use crate::data::{Data, OsdKind};
use crate::gfx::{self, Rect, Text};
use crate::ui::{self, Ui};
use smithay_client_toolkit::shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer};

pub fn spec(th: &gfx::Theme, _ow: u32, _oh: u32) -> Spec {
    let _oh = if _oh == 0 { 1080 } else { _oh };
    let bar = (th.cfg.bar.size + th.cfg.bar.margin) as i32;
    match th.cfg.style.osd.as_str() {
        "win11" => {
            let (w, h) = super::win11::OSD_SIZE;
            return Spec { layer: Layer::Overlay, anchor: Anchor::BOTTOM, size: (w, h), margin: (0, 0, 24, 0), exclusive: 0, keyboard: KeyboardInteractivity::None, namespace: "creed-osd", click_through: true, input: None };
        }
        "win10" => {
            let (w, h) = super::win10::OSD_SIZE;
            return Spec { layer: Layer::Overlay, anchor: Anchor::TOP | Anchor::LEFT, size: (w, h), margin: (56, 0, 0, 56), exclusive: -1, keyboard: KeyboardInteractivity::None, namespace: "creed-osd", click_through: true, input: None };
        }
        "material" => {
            let (w, h) = super::material::OSD_SIZE;
            return Spec { layer: Layer::Overlay, anchor: Anchor::TOP, size: (w, h), margin: (bar + 8, 0, 0, 0), exclusive: -1, keyboard: KeyboardInteractivity::None, namespace: "creed-osd", click_through: true, input: None };
        }
        "mac-hud" => {
            // Big Sur → Ventura and Aqua: the square HUD in the lower middle of the screen.
            let (w, h) = super::mac::HUD_SIZE;
            return Spec { layer: Layer::Overlay, anchor: Anchor::BOTTOM, size: (w, h), margin: (0, 0, (_oh as f64 * 0.14) as i32, 0), exclusive: -1, keyboard: KeyboardInteractivity::None, namespace: "creed-osd", click_through: true, input: None };
        }
        "mac" => {
            let (w, h) = super::mac::OSD_SIZE;
            return Spec { layer: Layer::Overlay, anchor: Anchor::TOP | Anchor::RIGHT, size: (w, h), margin: (bar + 4, 6, 0, 0), exclusive: -1, keyboard: KeyboardInteractivity::None, namespace: "creed-osd", click_through: true, input: None };
        }
        _ => {}
    }
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
        click_through: true, input: None,
    }
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data) {
    let t = super::open_amount(ui, closing);
    let Some((kind, value, _)) = data.osd else { return };
    let variant = ui.th.cfg.style.osd.clone();
    if matches!(variant.as_str(), "win11" | "win10" | "mac" | "mac-hud" | "material") {
        ui.cr.save().ok();
        ui.cr.push_group();
        match variant.as_str() {
            "win11" => super::win11::osd(ui, kind, value),
            "win10" => super::win10::osd(ui, kind, value),
            "material" => super::material::osd(ui, kind, value),
            "mac-hud" => super::mac::osd_hud(ui, kind, value),
            _ => super::mac::osd(ui, kind, value),
        }
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        return;
    }
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
        OsdKind::Mic => (if value <= 0.0 { "microphone-disabled-symbolic" } else { "audio-input-microphone-symbolic" }, Some(value)),
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
