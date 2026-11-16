//! macOS surfaces (Big Sur → Tahoe, plus Aqua): menu bar, Control Center, Spotlight,
//! Notification Center + banners and the volume HUD, laid out like the real system.
//! Colours are Apple's label/system colours; type is SF Pro metrics (13 pt menus/body,
//! 11 pt secondary). Status glyphs are drawn as vectors (SF Symbols cannot be shipped).

use super::Views;
use crate::data::{Data, NetKind};
use crate::gfx::{self, Rect, Text};
use crate::time;
use crate::ui::{self, Action, Kind, Role, Slider, Ui};
use creed_core::config::Material;
use creed_core::Color;
use std::f64::consts::PI;

struct M {
    dark: bool,
    label: Color,
    label2: Color,
    label3: Color,
    sel: Color,
    module: Color,
    panel: Color,
    stroke: Color,
    accent: Color,
    tahoe: bool,
    aqua: bool,
}

fn m(ui: &Ui) -> M {
    let th = ui.th;
    let d = th.dark;
    let tahoe = th.cfg.style.material == Material::Glass;
    let aqua = th.cfg.style.material == Material::Aqua;
    let frost = th.cfg.glass.frost;
    M {
        dark: d,
        label: if d { Color::rgba(1.0, 1.0, 1.0, 0.85) } else { Color::rgba(0.0, 0.0, 0.0, 0.85) },
        label2: if d { Color::rgba(1.0, 1.0, 1.0, 0.55) } else { Color::rgba(0.0, 0.0, 0.0, 0.5) },
        label3: if d { Color::rgba(1.0, 1.0, 1.0, 0.25) } else { Color::rgba(0.0, 0.0, 0.0, 0.25) },
        sel: if d { Color::rgba(1.0, 1.0, 1.0, 0.2) } else { Color::rgba(0.0, 0.0, 0.0, 0.1) },
        module: if d { Color::rgba(1.0, 1.0, 1.0, if tahoe { 0.1 } else { 0.08 }) } else { Color::rgba(1.0, 1.0, 1.0, if tahoe { 0.35 } else { 0.55 }) },
        panel: if d { Color::rgba(0.16, 0.16, 0.16, if tahoe { frost.max(0.25) } else { 0.78 }) } else { Color::rgba(0.93, 0.93, 0.93, if tahoe { frost.max(0.3) } else { 0.78 }) },
        stroke: if d { Color::rgba(1.0, 1.0, 1.0, 0.16) } else { Color::rgba(0.0, 0.0, 0.0, 0.12) },
        accent: th.pal.accent,
        tahoe,
        aqua,
    }
}

fn panel_radius(m: &M, base: f64) -> f64 {
    if m.tahoe {
        base + 10.0
    } else if m.aqua {
        6.0
    } else {
        base
    }
}

/// Window-style panel (Control Center, Spotlight, notifications).
fn panel(ui: &mut Ui, mm: &M, r: Rect, radius: f64, element: &str) {
    if mm.tahoe {
        ui.th.panel(ui.cr, r, radius, element, ui.pointer);
        return;
    }
    gfx::shadow(ui.cr, r, radius, 22.0, 10.0, if mm.dark { 0.5 } else { 0.22 });
    gfx::fill_rounded(ui.cr, r, radius, mm.panel);
    gfx::stroke_rounded(ui.cr, r, radius, 1.0, mm.stroke);
    if mm.dark {
        // Hairline outer edge in dark mode, like AppKit's vibrant windows.
        gfx::stroke_rounded(ui.cr, Rect::new(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0), radius + 1.0, 1.0, Color::rgba(0.0, 0.0, 0.0, 0.5));
    }
}

// ───────────────────────────── vector glyphs ─────────────────────────────

fn glyph_wifi(cr: &cairo::Context, c: (f64, f64), s: f64, col: Color, level: u8) {
    // Three arcs + dot, fanned upwards from the bottom point.
    let (cx, cy) = (c.0, c.1 + s * 0.38);
    cr.set_line_cap(cairo::LineCap::Round);
    for i in 0..3u8 {
        let rad = s * (0.28 + i as f64 * 0.24);
        let a = if 3 - i <= level.max(1) { col } else { col.with_alpha(col.a * 0.3) };
        gfx::set(cr, a);
        cr.set_line_width(s * 0.12);
        cr.new_path();
        cr.arc(cx, cy, rad, -PI * 0.75, -PI * 0.25);
        let _ = cr.stroke();
    }
    gfx::set(cr, col);
    cr.new_path();
    cr.arc(cx, cy - s * 0.02, s * 0.08, 0.0, 2.0 * PI);
    let _ = cr.fill();
}

fn glyph_battery(cr: &cairo::Context, r: Rect, col: Color, pct: f64, charging: bool) {
    // 25×12 body + 1.5×4 nub, 1 px outline, rounded 3.5.
    let body = Rect::new(r.x, r.cy() - 6.0, 24.0, 12.0);
    gfx::stroke_rounded(cr, body, 3.5, 1.0, col.with_alpha(col.a * 0.45));
    gfx::fill_rounded(cr, Rect::new(body.right() + 1.0, body.cy() - 2.0, 1.5, 4.0), 0.75, col.with_alpha(col.a * 0.45));
    let fill_w = (body.w - 4.0) * (pct / 100.0).clamp(0.04, 1.0);
    let fc = if pct <= 20.0 && !charging { Color::rgba(1.0, 0.27, 0.23, 1.0) } else { col };
    gfx::fill_rounded(cr, Rect::new(body.x + 2.0, body.y + 2.0, fill_w, body.h - 4.0), 1.8, fc);
    if charging {
        cr.new_path();
        cr.move_to(body.cx() + 1.0, body.y + 1.5);
        cr.line_to(body.cx() - 3.0, body.cy() + 0.5);
        cr.line_to(body.cx(), body.cy() + 0.5);
        cr.line_to(body.cx() - 1.0, body.bottom() - 1.5);
        cr.line_to(body.cx() + 3.0, body.cy() - 0.5);
        cr.line_to(body.cx(), body.cy() - 0.5);
        cr.close_path();
        gfx::set(cr, Color::WHITE);
        let _ = cr.fill_preserve();
        gfx::set(cr, Color::rgba(0.0, 0.0, 0.0, 0.6));
        cr.set_line_width(0.5);
        let _ = cr.stroke();
    }
}

fn glyph_control_center(cr: &cairo::Context, c: (f64, f64), col: Color) {
    // Two stacked toggles: knob left on top, knob right on bottom.
    for (i, left) in [(0.0, true), (1.0, false)] {
        let y = c.1 - 4.0 + i * 8.0;
        let pill = Rect::new(c.0 - 8.0, y - 3.0, 16.0, 6.0);
        gfx::stroke_rounded(cr, pill, 3.0, 1.2, col);
        let kx = if left { pill.x + 3.0 } else { pill.right() - 3.0 };
        cr.new_path();
        cr.arc(kx, y, 2.2, 0.0, 2.0 * PI);
        gfx::set(cr, col);
        let _ = cr.fill();
    }
}

fn glyph_search(cr: &cairo::Context, c: (f64, f64), s: f64, col: Color) {
    gfx::set(cr, col);
    cr.set_line_width(s * 0.11);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.new_path();
    cr.arc(c.0 - s * 0.08, c.1 - s * 0.08, s * 0.3, 0.0, 2.0 * PI);
    let _ = cr.stroke();
    cr.move_to(c.0 + s * 0.14, c.1 + s * 0.14);
    cr.line_to(c.0 + s * 0.38, c.1 + s * 0.38);
    let _ = cr.stroke();
}

fn glyph_mic(cr: &cairo::Context, c: (f64, f64), s: f64, col: Color, muted: bool) {
    gfx::set(cr, col);
    // Capsule head, cradle arc, stem and base - the SF "mic" silhouette, drawn from scratch.
    gfx::fill_rounded(cr, Rect::new(c.0 - s * 0.16, c.1 - s * 0.46, s * 0.32, s * 0.56), s * 0.16, col);
    cr.set_line_width(s * 0.08);
    cr.set_line_cap(cairo::LineCap::Round);
    cr.new_path();
    cr.arc(c.0, c.1 - s * 0.06, s * 0.3, 0.0, std::f64::consts::PI);
    cr.move_to(c.0, c.1 + s * 0.24);
    cr.line_to(c.0, c.1 + s * 0.42);
    cr.move_to(c.0 - s * 0.16, c.1 + s * 0.42);
    cr.line_to(c.0 + s * 0.16, c.1 + s * 0.42);
    if muted {
        cr.move_to(c.0 - s * 0.4, c.1 - s * 0.46);
        cr.line_to(c.0 + s * 0.4, c.1 + s * 0.42);
    }
    let _ = cr.stroke();
}

fn glyph_speaker(cr: &cairo::Context, c: (f64, f64), s: f64, col: Color, level: f64, muted: bool) {
    gfx::set(cr, col);
    cr.new_path();
    cr.move_to(c.0 - s * 0.42, c.1 - s * 0.14);
    cr.line_to(c.0 - s * 0.24, c.1 - s * 0.14);
    cr.line_to(c.0 - s * 0.02, c.1 - s * 0.36);
    cr.line_to(c.0 - s * 0.02, c.1 + s * 0.36);
    cr.line_to(c.0 - s * 0.24, c.1 + s * 0.14);
    cr.line_to(c.0 - s * 0.42, c.1 + s * 0.14);
    cr.close_path();
    let _ = cr.fill();
    cr.set_line_width(s * 0.08);
    cr.set_line_cap(cairo::LineCap::Round);
    if muted {
        cr.move_to(c.0 + s * 0.12, c.1 - s * 0.14);
        cr.line_to(c.0 + s * 0.4, c.1 + s * 0.14);
        cr.move_to(c.0 + s * 0.4, c.1 - s * 0.14);
        cr.line_to(c.0 + s * 0.12, c.1 + s * 0.14);
        let _ = cr.stroke();
        return;
    }
    for (i, th) in [0.34, 0.67, 1.0].iter().enumerate() {
        let rad = s * (0.14 + i as f64 * 0.13);
        let a = if level >= *th - 0.33 { col } else { col.with_alpha(col.a * 0.3) };
        gfx::set(cr, a);
        cr.new_path();
        cr.arc(c.0 + s * 0.02, c.1, rad, -PI / 4.0, PI / 4.0);
        let _ = cr.stroke();
    }
}

fn glyph_sun(cr: &cairo::Context, c: (f64, f64), s: f64, col: Color) {
    gfx::set(cr, col);
    cr.new_path();
    cr.arc(c.0, c.1, s * 0.2, 0.0, 2.0 * PI);
    let _ = cr.fill();
    cr.set_line_width(s * 0.08);
    cr.set_line_cap(cairo::LineCap::Round);
    for i in 0..8 {
        let a = i as f64 * PI / 4.0;
        cr.move_to(c.0 + a.cos() * s * 0.32, c.1 + a.sin() * s * 0.32);
        cr.line_to(c.0 + a.cos() * s * 0.44, c.1 + a.sin() * s * 0.44);
    }
    let _ = cr.stroke();
}

fn glyph_logo(cr: &cairo::Context, c: (f64, f64), col: Color) {
    // Neutral stand-in for the vendor menu: a soft four-dot knot (not any company's mark).
    for (dx, dy) in [(0.0, -3.4), (3.4, 0.0), (0.0, 3.4), (-3.4, 0.0)] {
        cr.new_path();
        cr.arc(c.0 + dx, c.1 + dy, 2.6, 0.0, 2.0 * PI);
        gfx::set(cr, col);
        let _ = cr.fill();
    }
}

// ───────────────────────────── menu bar ─────────────────────────────

pub fn bar_background(ui: &mut Ui, full: Rect) {
    let mm = m(ui);
    let cr = ui.cr;
    if mm.aqua {
        let g = cairo::LinearGradient::new(0.0, full.y, 0.0, full.bottom());
        g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.98);
        g.add_color_stop_rgba(0.5, 0.95, 0.95, 0.95, 0.98);
        g.add_color_stop_rgba(0.51, 0.91, 0.91, 0.91, 0.98);
        g.add_color_stop_rgba(1.0, 0.86, 0.86, 0.86, 0.98);
        cr.rectangle(full.x, full.y, full.w, full.h);
        let _ = cr.set_source(&g);
        let _ = cr.fill();
        gfx::fill_rounded(cr, Rect::new(full.x, full.bottom() - 1.0, full.w, 1.0), 0.0, Color::rgba(0.0, 0.0, 0.0, 0.35));
        return;
    }
    if mm.tahoe {
        // Tahoe's menu bar is transparent; a very soft top scrim keeps labels legible.
        let g = cairo::LinearGradient::new(0.0, full.y, 0.0, full.bottom());
        g.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, if mm.dark { 0.18 } else { 0.06 });
        g.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
        cr.rectangle(full.x, full.y, full.w, full.h);
        let _ = cr.set_source(&g);
        let _ = cr.fill();
        return;
    }
    let c = if mm.dark { Color::rgba(0.11, 0.11, 0.12, 0.62) } else { Color::rgba(0.97, 0.97, 0.97, 0.72) };
    gfx::fill_rounded(cr, full, 0.0, c);
}

fn item_hover(ui: &mut Ui, mm: &M, r: Rect, key: ui::Id) {
    let hv = ui.hover_amt(key);
    let open = ui.is_pressed(key);
    if hv > 0.01 || open {
        let a = if open { 1.0 } else { hv };
        let rr = Rect::centered(r.cx(), r.cy(), r.w, (r.h - 4.0).max(16.0));
        gfx::fill_rounded(ui.cr, rr, if mm.aqua { 0.0 } else { 4.0 }, mm.sel.with_alpha(mm.sel.a * a));
    }
}

fn bar_text(ui: &Ui, mm: &M) -> Color {
    if mm.aqua {
        Color::BLACK
    } else if mm.tahoe {
        if ui.th.dark {
            Color::WHITE
        } else {
            Color::rgba(0.0, 0.0, 0.0, 0.88)
        }
    } else {
        mm.label
    }
}

pub fn module_len(ui: &mut Ui, name: &str, data: &Data) -> Option<f64> {
    let mm = m(ui);
    let fs = 13.0;
    let col = bar_text(ui, &mm);
    Some(match name {
        "launcher" => 34.0,
        "appmenu" => {
            let app = data.active_client().map(|c| Data::pretty_class(&c.class)).unwrap_or_else(|| "Finder".into());
            let titles: f64 = MENU_TITLES.iter().map(|t| ui.measure(t, &Text::new(fs, col)).0 + 20.0).sum();
            ui.measure(&app, &Text::new(fs, col).bold()).0 + 20.0 + titles
        }
        "window" => 0.0,
        "battery" => {
            if data.battery.is_some() {
                ui.measure("100%", &Text::new(fs, col)).0 + 44.0
            } else {
                0.0
            }
        }
        "network" | "control" | "search" | "volume" | "bluetooth" => {
            if name == "bluetooth" && data.bt.is_none() {
                0.0
            } else {
                34.0
            }
        }
        "media" => match &data.media {
            Some(m2) if !m2.title.is_empty() => 30.0,
            _ => 0.0,
        },
        "tray" => data.tray.len() as f64 * 30.0,
        "clock" => {
            let s = format!("{}  {}", time::format(time::now_secs(), &ui.th.cfg.bar.date_format), super::bar::clock_text(ui));
            ui.measure(&s, &Text::new(fs, col)).0 + 20.0
        }
        _ => return None,
    })
}

pub fn draw_module(ui: &mut Ui, name: &str, r: Rect, data: &Data) -> bool {
    let mm = m(ui);
    let col = bar_text(ui, &mm);
    let key = ui::id(&format!("mac.{name}"));
    let fs = 13.0;
    match name {
        "launcher" => {
            item_hover(ui, &mm, r, key);
            // With the downloaded MacTahoe/WhiteSur theme installed, its own start-here glyph is
            // used (that asset comes from the theme, not from Creed); otherwise a neutral mark.
            let theme = ui.th.cfg.style.icon_theme.to_lowercase();
            if (theme.starts_with("mactahoe") || theme.starts_with("whitesur")) && ui.icons.exists("start-here-symbolic") {
                ui.icon(r, "start-here-symbolic", 16.0, Some(col));
            } else {
                glyph_logo(ui.cr, (r.cx(), r.cy()), col);
            }
            ui.hit(key, r, Action::Toggle(Kind::Power), Role::Button, "System menu");
        }
        "appmenu" => {
            let app = data.active_client().map(|c| Data::pretty_class(&c.class)).unwrap_or_else(|| "Finder".into());
            let aw = ui.measure(&app, &Text::new(fs, col).bold()).0 + 20.0;
            let ar = Rect::new(r.x, r.y, aw, r.h);
            item_hover(ui, &mm, ar, key);
            ui.text(ar, &app, &Text::new(fs, col).bold().center());
            ui.hit(key, ar, Action::AppMenu("app".into(), ar.x as i32), Role::Button, format!("{app} menu"));
            let mut x = ar.right();
            for t in MENU_TITLES {
                let tw = ui.measure(t, &Text::new(fs, col)).0 + 20.0;
                let tr = Rect::new(x, r.y, tw, r.h);
                let tk = ui::idn("mac.menutitle", t);
                item_hover(ui, &mm, tr, tk);
                ui.text(tr, t, &Text::new(fs, col).center());
                ui.hit(tk, tr, Action::AppMenu(t.to_string(), tr.x as i32), Role::Button, format!("{t} menu"));
                x += tw;
            }
        }
        "battery" => {
            let Some(b) = &data.battery else { return true };
            item_hover(ui, &mm, r, key);
            let pt = format!("{:.0}%", b.percent);
            let tw = ui.measure(&pt, &Text::new(fs, col)).0;
            ui.text(Rect::new(r.x + 8.0, r.y, tw, r.h), &pt, &Text::new(fs, col));
            glyph_battery(ui.cr, Rect::new(r.x + 14.0 + tw, r.y, 27.0, r.h), col, b.percent, b.charging);
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, format!("Battery {pt}"));
        }
        "network" => {
            item_hover(ui, &mm, r, key);
            let level = match data.net.kind {
                NetKind::Wifi => (data.net.signal / 34 + 1).min(3),
                NetKind::Ethernet => 3,
                NetKind::None => 0,
            };
            if data.net.kind == NetKind::Ethernet {
                ui.icon(r, "network-wired-symbolic", 15.0, Some(col));
            } else {
                glyph_wifi(ui.cr, (r.cx(), r.cy() - 1.0), 15.0, if level == 0 { col.with_alpha(col.a * 0.35) } else { col }, level);
            }
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, "Wi-Fi");
        }
        "volume" => {
            item_hover(ui, &mm, r, key);
            let (v, mu) = data.audio.as_ref().map(|a| (a.volume, a.muted)).unwrap_or((0.0, true));
            glyph_speaker(ui.cr, (r.cx(), r.cy()), 16.0, col, v, mu || v <= 0.0);
            let hit = ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, "Sound");
            hit.scroll = Some(|d| Action::VolumeStep(d * 0.0625));
        }
        "bluetooth" => {
            item_hover(ui, &mm, r, key);
            ui.icon(r, "bluetooth-active-symbolic", 15.0, Some(col));
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, "Bluetooth");
        }
        "control" => {
            item_hover(ui, &mm, r, key);
            glyph_control_center(ui.cr, (r.cx(), r.cy()), col);
            ui.hit(key, r, Action::Toggle(Kind::Control), Role::Button, "Control Center");
        }
        "search" => {
            if mm.aqua {
                // Tiger: a blue glossy tile with a white magnifier at the far right.
                let b = Rect::centered(r.cx(), r.cy(), 20.0, 18.0);
                let g = cairo::LinearGradient::new(0.0, b.y, 0.0, b.bottom());
                g.add_color_stop_rgb(0.0, 0.45, 0.68, 0.98);
                g.add_color_stop_rgb(0.5, 0.2, 0.45, 0.9);
                g.add_color_stop_rgb(1.0, 0.25, 0.55, 0.96);
                gfx::rounded(ui.cr, b, 4.0);
                let _ = ui.cr.set_source(&g);
                let _ = ui.cr.fill();
                glyph_search(ui.cr, (r.cx(), r.cy()), 12.0, Color::WHITE);
            } else {
                item_hover(ui, &mm, r, key);
                glyph_search(ui.cr, (r.cx(), r.cy()), 15.0, col);
            }
            ui.hit(key, r, Action::Toggle(Kind::Launcher), Role::Button, "Spotlight");
        }
        "media" => {
            let Some(md) = &data.media else { return true };
            item_hover(ui, &mm, r, key);
            ui.icon(r, if md.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, 14.0, Some(col));
            ui.hit(key, r, Action::MediaPlayPause, Role::Button, format!("Now Playing: {}", md.title));
        }
        "tray" => {
            for (i, item) in data.tray.iter().enumerate() {
                let c = Rect::new(r.x + i as f64 * 30.0, r.y, 30.0, r.h);
                let k = ui::idn("mac.tray", &item.key);
                item_hover(ui, &mm, c, k);
                if let Some((pw, ph, px)) = &item.pixmap {
                    super::bar::draw_pixmap(ui.cr, Rect::centered(c.cx(), c.cy(), 16.0, 16.0), *pw, *ph, px);
                } else {
                    ui.icon(c, if item.icon.is_empty() { &item.id } else { &item.icon }, 16.0, Some(col));
                }
                let primary = Action::TrayMenu(item.key.clone());
                ui.hit(k, c, primary, Role::Button, item.title.clone());
            }
        }
        "clock" => {
            item_hover(ui, &mm, r, key);
            let s = format!("{}  {}", time::format(time::now_secs(), &ui.th.cfg.bar.date_format), super::bar::clock_text(ui));
            ui.text(r, &s, &Text::new(fs, col).center());
            ui.hit(key, r, Action::Toggle(Kind::NotifCenter), Role::Button, format!("Clock {s}"));
        }
        _ => return false,
    }
    true
}

// ───────────────────────────── Control Center ─────────────────────────────

fn below_bar(ui: &Ui, w: f64) -> Rect {
    let (sw, _) = ui.size;
    let bar = if ui.th.cfg.bar.enabled { (ui.th.cfg.bar.size + ui.th.cfg.bar.margin) as f64 } else { 0.0 };
    Rect::new(sw - w - 8.0, bar + 6.0, w, 0.0)
}

fn circle_toggle(ui: &mut Ui, mm: &M, c: (f64, f64), on: bool, icon: &str) {
    let r = Rect::centered(c.0, c.1, 26.0, 26.0);
    gfx::fill_rounded(ui.cr, r, 13.0, if on { mm.accent } else if mm.dark { Color::rgba(1.0, 1.0, 1.0, 0.15) } else { Color::rgba(0.0, 0.0, 0.0, 0.1) });
    ui.icon(r, icon, 14.0, Some(if on { Color::WHITE } else { mm.label }));
}

fn module(ui: &mut Ui, mm: &M, r: Rect) {
    gfx::fill_rounded(ui.cr, r, panel_radius(mm, 12.0), mm.module);
}

/// macOS slider: 22 px track, white fill, white knob; glyph inside the track's left end.
fn big_slider(ui: &mut Ui, mm: &M, r: Rect, v: f64, glyph: &dyn Fn(&cairo::Context, (f64, f64), Color)) {
    let track = Rect::new(r.x, r.cy() - 11.0, r.w, 22.0);
    gfx::fill_rounded(ui.cr, track, 11.0, if mm.dark { Color::rgba(1.0, 1.0, 1.0, 0.15) } else { Color::rgba(0.0, 0.0, 0.0, 0.1) });
    let fw = 22.0 + (track.w - 22.0) * v.clamp(0.0, 1.0);
    gfx::fill_rounded(ui.cr, Rect::new(track.x, track.y, fw, 22.0), 11.0, Color::rgba(1.0, 1.0, 1.0, 0.95));
    let knob = Rect::new(track.x + fw - 22.0, track.y, 22.0, 22.0);
    gfx::shadow(ui.cr, knob, 11.0, 4.0, 1.0, 0.35);
    gfx::fill_rounded(ui.cr, knob, 11.0, Color::WHITE);
    glyph(ui.cr, (track.x + 11.0, track.cy()), Color::rgba(0.0, 0.0, 0.0, 0.6));
}

pub fn control_center(ui: &mut Ui, t_open: f64, data: &Data, _views: &mut Views) {
    let mm = m(ui);
    let pad = 10.0;
    let w = 320.0;
    let col_w = (w - pad * 3.0) / 2.0;
    let has_media = data.media.is_some();
    let h = pad + 136.0 + pad + 68.0 + pad + 68.0 + if has_media { pad + 68.0 } else { 0.0 } + pad;
    let mut panel = below_bar(ui, w);
    panel.h = h;
    super::popup_transform(ui, panel, t_open, true);
    panel_draw(ui, &mm, panel, 16.0, "control");
    ui.record("control.panel", panel, None);
    ui.hit(ui::id("control.panel"), panel, Action::None, Role::Group, "");
    // Left: connectivity module (Wi-Fi, Bluetooth, Game Mode in place of AirDrop).
    let left = Rect::new(panel.x + pad, panel.y + pad, col_w, 136.0);
    module(ui, &mm, left);
    let wifi_sub = match data.net.kind {
        NetKind::Wifi => data.net.name.clone(),
        NetKind::Ethernet => "Ethernet".into(),
        NetKind::None if !data.net.available => "Unavailable".into(),
        NetKind::None => if data.net.wifi_enabled { "Not Connected".into() } else { "Off".into() },
    };
    let rows: [(&str, &str, String, bool, Action); 3] = [
        ("network-wireless-symbolic", "Wi-Fi", wifi_sub, data.net.wifi_enabled, Action::ToggleWifi),
        ("bluetooth-active-symbolic", "Bluetooth", match &data.bt { Some(b) if b.powered => "On".into(), Some(_) => "Off".into(), None => "Unavailable".into() }, data.bt.as_ref().map(|b| b.powered).unwrap_or(false), Action::ToggleBluetooth),
        ("input-gaming-symbolic", "Game Mode", if data.game { "On".into() } else { "Off".into() }, data.game, Action::ToggleGame),
    ];
    for (i, (icon, title, sub, on, action)) in rows.into_iter().enumerate() {
        let rr = Rect::new(left.x + 10.0, left.y + 8.0 + i as f64 * 40.0, left.w - 20.0, 40.0);
        let k = ui::idn("mac.cc.row", i);
        circle_toggle(ui, &mm, (rr.x + 13.0, rr.cy()), on, icon);
        ui.text(Rect::new(rr.x + 34.0, rr.y + 3.0, rr.w - 34.0, 18.0), title, &Text::new(13.0, mm.label).bold());
        ui.text(Rect::new(rr.x + 34.0, rr.y + 20.0, rr.w - 34.0, 16.0), &sub, &Text::new(11.0, mm.label2));
        let hit = ui.hit(k, rr, action, Role::Toggle, format!("{title}, {sub}"));
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
    // Right: Focus module + two square modules.
    let focus = Rect::new(left.right() + pad, left.y, col_w, 62.0);
    module(ui, &mm, focus);
    circle_toggle(ui, &mm, (focus.x + 23.0, focus.cy()), data.dnd, "weather-clear-night-symbolic");
    ui.text(Rect::new(focus.x + 44.0, focus.y, focus.w - 50.0, focus.h), "Focus", &Text::new(13.0, mm.label).bold());
    let fk = ui::id("mac.cc.focus");
    let hit = ui.hit(fk, focus, Action::ToggleDnd, Role::Toggle, "Focus");
    hit.value = Some(if data.dnd { 1.0 } else { 0.0 });
    let sq_w = (col_w - pad) / 2.0;
    let squares: [(&str, &str, bool, Action); 2] = [
        ("night-light-symbolic", "Night Shift", data.night_light, Action::ToggleNightLight),
        ("view-dual-symbolic", "Layouts", false, Action::Open(Kind::Snap)),
    ];
    for (i, (icon, label, on, action)) in squares.into_iter().enumerate() {
        let sq = Rect::new(focus.x + i as f64 * (sq_w + pad), focus.bottom() + pad, sq_w, 136.0 - 62.0 - pad);
        module(ui, &mm, sq);
        circle_toggle(ui, &mm, (sq.cx(), sq.y + 24.0), on, icon);
        ui.text(Rect::new(sq.x + 2.0, sq.bottom() - 24.0, sq.w - 4.0, 18.0), label, &Text::new(11.0, mm.label).center());
        let k = ui::idn("mac.cc.sq", i);
        let hit = ui.hit(k, sq, action, Role::Toggle, label);
        hit.value = Some(if on { 1.0 } else { 0.0 });
    }
    // Display + Sound modules.
    let mut y = left.bottom() + pad;
    let slider_module = |ui: &mut Ui, y: f64, title: &str, v: Option<f64>, which: Slider, key: &str, glyph: &dyn Fn(&cairo::Context, (f64, f64), Color)| {
        let md = Rect::new(panel.x + pad, y, w - pad * 2.0, 68.0);
        module(ui, &mm, md);
        ui.text(Rect::new(md.x + 12.0, md.y + 8.0, md.w - 24.0, 18.0), title, &Text::new(13.0, mm.label).bold());
        let sr = Rect::new(md.x + 12.0, md.y + 30.0, md.w - 24.0, 30.0);
        match v {
            Some(v) => {
                let k = ui::id(key);
                let vv = ui.spring(k ^ 0x5, v, v);
                big_slider(ui, &mm, sr, vv, glyph);
                let hit = ui.hit(k, sr, Action::Slider(which), Role::Slider, title.to_string());
                hit.value = Some(v);
            }
            None => {
                ui.text(sr, "Unavailable", &Text::new(11.0, mm.label2));
            }
        }
    };
    slider_module(ui, y, "Display", data.brightness, Slider::Brightness, "mac.cc.display", &|cr, c, col| glyph_sun(cr, c, 14.0, col));
    y += 68.0 + pad;
    let vol = data.audio.as_ref().map(|a| if a.muted { 0.0 } else { a.volume.min(1.0) });
    slider_module(ui, y, "Sound", vol, Slider::Volume, "mac.cc.sound", &|cr, c, col| glyph_speaker(cr, c, 13.0, col, 1.0, false));
    y += 68.0 + pad;
    if let Some(md) = &data.media {
        let r = Rect::new(panel.x + pad, y, w - pad * 2.0, 68.0);
        module(ui, &mm, r);
        let art = Rect::new(r.x + 12.0, r.y + 12.0, 44.0, 44.0);
        gfx::fill_rounded(ui.cr, art, 8.0, mm.accent.with_alpha(0.3));
        ui.icon(art, "audio-x-generic-symbolic", 20.0, Some(mm.label));
        ui.text(Rect::new(art.right() + 10.0, r.y + 14.0, r.w - 140.0, 18.0), &md.title, &Text::new(13.0, mm.label).bold());
        ui.text(Rect::new(art.right() + 10.0, r.y + 34.0, r.w - 140.0, 18.0), if md.artist.is_empty() { &md.player } else { &md.artist }, &Text::new(11.0, mm.label2));
        for (i, (icon, label, a)) in [(if md.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, "Play/Pause", Action::MediaPlayPause), ("media-skip-forward-symbolic", "Next", Action::MediaNext)].into_iter().enumerate() {
            let b = Rect::new(r.right() - 76.0 + i as f64 * 34.0, r.cy() - 15.0, 30.0, 30.0);
            let k = ui::idn("mac.cc.media", i);
            ui.icon(b, icon, 16.0, Some(mm.label));
            ui.hit(k, b, a, Role::Button, label);
        }
    }
}

fn panel_draw(ui: &mut Ui, mm: &M, r: Rect, base_radius: f64, element: &str) {
    panel(ui, mm, r, panel_radius(mm, base_radius), element);
}

// ───────────────────────────── Spotlight ─────────────────────────────

pub fn spotlight(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let mm = m(ui);
    if mm.tahoe {
        return spotlight_tahoe(ui, t_open, data, views, &mm);
    }
    let (sw, sh) = ui.size;
    let results = super::launcher::search(&data.apps, &views.query);
    let special = super::launcher::special_label(&views.query);
    let n = results.len() + special.is_some() as usize;
    let show = !views.query.is_empty();
    let rows = n.min(9);
    let field_h = if mm.tahoe { 56.0 } else { 52.0 };
    let w = if mm.tahoe { 640.0 } else { 680.0 };
    let list_h = if show { if rows == 0 { 60.0 } else { rows as f64 * 30.0 + 16.0 } } else { 0.0 };
    let x = ((sw - w) / 2.0).round();
    let y = (sh * 0.22).round();
    let field = Rect::new(x, y, w, field_h);
    super::popup_transform(ui, field, t_open, true);
    let radius = if mm.tahoe { field_h / 2.0 } else if mm.aqua { 8.0 } else { 12.0 };
    let whole = if mm.tahoe { field } else { Rect::new(x, y, w, field_h + list_h) };
    panel(ui, &mm, whole, radius, "launcher");
    ui.record("launcher.panel", whole, None);
    ui.hit(ui::id("launcher.panel"), whole, Action::None, Role::Group, "");
    glyph_search(ui.cr, (field.x + 26.0, field.cy()), 20.0, mm.label2);
    let q = format!("{}{}", views.query, views.preedit);
    let tr = Rect::new(field.x + 48.0, field.y, field.w - 64.0, field.h);
    let big = Text::new(if mm.tahoe { 20.0 } else { 22.0 }, mm.label);
    if q.is_empty() {
        ui.text(Rect::new(tr.x + 4.0, tr.y, tr.w - 4.0, tr.h), "Spotlight Search", &Text::new(big.size, mm.label3));
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, field.cy() - 12.0, 2.0, 24.0), 1.0, mm.accent);
    } else {
        let b = ui.text(tr, &q, &big);
        gfx::fill_rounded(ui.cr, Rect::new(b.right() + 1.0, field.cy() - 12.0, 2.0, 24.0), 1.0, mm.accent);
    }
    ui.hit(ui::id("launcher.input"), field, Action::None, Role::Input, "Spotlight Search");
    if mm.tahoe {
        // Tahoe: four glass shortcut buttons beside the field.
        let btns: [(&str, &str, Action); 4] = [
            ("view-app-grid-symbolic", "Applications", Action::Exec("creed toggle launchpad".into())),
            ("folder-documents-symbolic", "Files", Action::Exec(ui.th.cfg.general.file_manager.clone())),
            ("system-run-symbolic", "Shortcuts", Action::Open(Kind::Snap)),
            ("edit-paste-symbolic", "Clipboard", Action::Open(Kind::Clipboard)),
        ];
        for (i, (icon, label, a)) in btns.into_iter().enumerate() {
            let b = Rect::new(field.right() + 10.0 + i as f64 * 54.0, field.y + 4.0, 48.0, 48.0);
            ui.th.panel(ui.cr, b, 24.0, "launcher", ui.pointer);
            ui.icon(b, icon, 20.0, Some(mm.label));
            ui.hit(ui::idn("mac.sp.btn", i), b, a, Role::Button, label);
        }
    }
    if !show {
        return;
    }
    let list = if mm.tahoe {
        let l = Rect::new(x, field.bottom() + 10.0, w, list_h);
        panel(ui, &mm, l, 24.0, "launcher");
        l.inset_xy(8.0, 8.0)
    } else {
        gfx::fill_rounded(ui.cr, Rect::new(x, field.bottom(), w, 1.0), 0.0, mm.stroke);
        Rect::new(x + 8.0, field.bottom() + 8.0, w - 16.0, list_h - 16.0)
    };
    if n == 0 {
        ui.text(list, "No Results", &Text::new(13.0, mm.label2).center());
        return;
    }
    for i in 0..rows {
        let r = Rect::new(list.x, list.y + i as f64 * 30.0, list.w, 28.0);
        let sel = views.sel == i;
        let k = ui::idn("mac.sp.row", i);
        let hv = ui.hover_amt(k);
        if sel {
            gfx::fill_rounded(ui.cr, r, 6.0, mm.accent);
        } else if hv > 0.01 {
            gfx::fill_rounded(ui.cr, r, 6.0, mm.sel.with_alpha(mm.sel.a * hv));
        }
        let fg = if sel { Color::WHITE } else { mm.label };
        let (icon, title) = match (i, special.as_ref()) {
            (0, Some((t, _, ic))) => (ic.to_string(), t.clone()),
            _ => {
                let a = &data.apps[results[i - special.is_some() as usize]];
                (a.icon.clone(), a.name.clone())
            }
        };
        ui.icon(Rect::new(r.x + 8.0, r.y, 20.0, r.h), &icon, 20.0, if icon.ends_with("-symbolic") { Some(fg) } else { None });
        ui.text(Rect::new(r.x + 36.0, r.y, r.w - 44.0, r.h), &title, &Text::new(13.0, fg));
        ui.hit(k, r, Action::LauncherPick(i), Role::Item, title);
    }
}


/// macOS 26 Spotlight: one Liquid Glass capsule that grows into a rounded panel when there are
/// results (search row, hairline, list), with the Apps / Files / Actions / Clipboard buttons
/// revealed beside it on hover.
fn spotlight_tahoe(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views, mm: &M) {
    let (sw, sh) = ui.size;
    let results = super::launcher::search(&data.apps, &views.query);
    let special = super::launcher::special_label(&views.query);
    let n = results.len() + special.is_some() as usize;
    let show = !views.query.is_empty();
    let rows = n.min(8);
    let field_h = 52.0;
    let row_h = 40.0;
    let w = 640.0f64.min(sw - 300.0).max(360.0);
    let list_h = if show { if rows == 0 { 56.0 } else { rows as f64 * row_h + 14.0 } } else { 0.0 };
    let x = ((sw - w) / 2.0).round();
    let y = (sh * 0.2).round();
    let field = Rect::new(x, y, w, field_h);
    super::popup_transform(ui, field, t_open, true);
    // Container: capsule when empty, 26 px rounded panel when it holds results.
    let grow = ui.spring(ui::id("tahoe.sp.grow"), list_h, list_h);
    let whole = Rect::new(x, y, w, field_h + grow);
    let radius = if grow < 1.0 { field_h / 2.0 } else { 26.0f64.max(field_h / 2.0 - grow * 0.4) };
    panel(ui, mm, whole, radius, "launcher");
    ui.record("launcher.panel", whole, None);
    ui.hit(ui::id("launcher.panel"), whole, Action::None, Role::Group, "");
    glyph_search(ui.cr, (field.x + 26.0, field.cy()), 18.0, mm.label2);
    let q = format!("{}{}", views.query, views.preedit);
    let tr = Rect::new(field.x + 48.0, field.y, field.w - 64.0, field.h);
    let big = Text::new(19.0, mm.label);
    if q.is_empty() {
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, field.cy() - 11.0, 2.0, 22.0), 1.0, mm.accent);
        ui.text(Rect::new(tr.x + 5.0, tr.y, tr.w - 5.0, tr.h), "Spotlight Search", &Text::new(big.size, mm.label3));
    } else {
        let b = ui.text(tr, &q, &big);
        gfx::fill_rounded(ui.cr, Rect::new(b.right() + 1.0, field.cy() - 11.0, 2.0, 22.0), 1.0, mm.accent);
    }
    ui.hit(ui::id("launcher.input"), field, Action::None, Role::Input, "Spotlight Search");
    // Shortcut buttons fade in while the pointer is near the search row (or one has focus).
    let btns: [(&str, &str, Action); 4] = [
        ("view-app-grid-symbolic", "Applications", Action::Exec("creed toggle launchpad".into())),
        ("folder-documents-symbolic", "Files", Action::Exec(ui.th.cfg.general.file_manager.clone())),
        ("system-run-symbolic", "Actions", Action::Open(Kind::Snap)),
        ("edit-paste-symbolic", "Clipboard", Action::Open(Kind::Clipboard)),
    ];
    let band = Rect::new(field.x, field.y - 12.0, field.w + 10.0 + 4.0 * 52.0 + 12.0, field.h + 24.0);
    let keys: Vec<ui::Id> = (0..4).map(|i| ui::idn("mac.sp.btn", i)).collect();
    let near = ui.pointer.map(|(px, py)| band.contains(px, py)).unwrap_or(false) || keys.iter().any(|k| ui.focus == Some(*k));
    let reveal = ui.spring(ui::id("tahoe.sp.reveal"), if near { 1.0 } else { 0.0 }, 0.0);
    for (i, (icon, label, a)) in btns.into_iter().enumerate() {
        let b = Rect::new(field.right() + 10.0 + i as f64 * 52.0, field.y + 3.0, 46.0, 46.0);
        if reveal > 0.01 {
            ui.cr.save().ok();
            ui.cr.push_group();
            ui.th.panel(ui.cr, b, 23.0, "launcher", ui.pointer);
            let hv = ui.hover_amt(keys[i]);
            if hv > 0.01 {
                gfx::fill_rounded(ui.cr, b, 23.0, mm.sel.with_alpha(mm.sel.a * hv));
            }
            ui.icon(b, icon, 19.0, Some(mm.label));
            if ui.focus == Some(keys[i]) {
                ui.th.focus_ring(ui.cr, b, 23.0);
            }
            let _ = ui.cr.pop_group_to_source();
            let _ = ui.cr.paint_with_alpha(reveal);
            ui.cr.restore().ok();
        }
        ui.hit(keys[i], b, a, Role::Button, label);
    }
    if !show || grow < field_h * 0.5 {
        return;
    }
    gfx::fill_rounded(ui.cr, Rect::new(x + 16.0, field.bottom(), w - 32.0, 1.0), 0.0, mm.stroke);
    let list = Rect::new(x + 8.0, field.bottom() + 7.0, w - 16.0, (grow - 14.0).max(0.0));
    if n == 0 {
        ui.text(list, "No Results", &Text::new(13.0, mm.label2).center());
        return;
    }
    super::with_clip(ui, list, |ui| {
        for i in 0..rows {
            let r = Rect::new(list.x, list.y + i as f64 * row_h, list.w, row_h - 2.0);
            let sel = views.sel == i;
            let k = ui::idn("mac.sp.row", i);
            let hv = ui.hover_amt(k);
            // Tahoe selection is a soft neutral capsule, not an accent fill.
            let a = if sel { 1.0 } else { hv * 0.6 };
            if a > 0.01 {
                gfx::fill_rounded(ui.cr, r, 12.0, mm.sel.with_alpha((mm.sel.a * 1.4).min(0.5) * a));
            }
            let (icon, title, sub) = match (i, special.as_ref()) {
                (0, Some((t, sub, ic))) => (ic.to_string(), t.clone(), sub.clone()),
                _ => {
                    let a = &data.apps[results[i - special.is_some() as usize]];
                    (a.icon.clone(), a.name.clone(), "Application".to_string())
                }
            };
            ui.icon(Rect::new(r.x + 10.0, r.y, 26.0, r.h), &icon, 26.0, if icon.ends_with("-symbolic") { Some(mm.label) } else { None });
            ui.text(Rect::new(r.x + 48.0, r.y, r.w - 160.0, r.h), &title, &Text::new(14.0, mm.label));
            ui.text(Rect::new(r.right() - 120.0, r.y, 108.0, r.h), &sub, &Text::new(12.0, mm.label3).end());
            ui.hit(k, r, Action::LauncherPick(i), Role::Item, title);
        }
    });
}

/// macOS 26 Spotlight clipboard history: the Spotlight panel with a "Clipboard" field, items with
/// a document glyph, "Text" caption and a copy button. Returns false outside Tahoe.
pub fn clipboard_tahoe(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) -> bool {
    let mm = m(ui);
    if !mm.tahoe || ui.th.cfg.style.launcher != "spotlight" {
        return false;
    }
    let (sw, sh) = ui.size;
    let w = 640.0f64.min(sw - 40.0);
    let x = ((sw - w) / 2.0).round();
    let y = (sh * 0.2).round();
    let h = 460.0f64.min(sh * 0.6).round();
    let whole = Rect::new(x, y, w, h);
    ui.cr.save().ok();
    ui.cr.push_group();
    super::popup_transform(ui, whole, t_open, true);
    panel(ui, &mm, whole, 26.0, "launcher");
    ui.record("clipboard.panel", whole, None);
    ui.hit(ui::id("clip.panel"), whole, Action::None, Role::Group, "");
    let field = Rect::new(x, y, w, 52.0);
    ui.icon(Rect::new(field.x + 14.0, field.y, 24.0, field.h), "edit-paste-symbolic", 20.0, Some(mm.label2));
    let q = format!("{}{}", views.query, views.preedit);
    let tr = Rect::new(field.x + 48.0, field.y, field.w - 100.0, field.h);
    if q.is_empty() {
        gfx::fill_rounded(ui.cr, Rect::new(tr.x, field.cy() - 11.0, 2.0, 22.0), 1.0, mm.accent);
        ui.text(Rect::new(tr.x + 5.0, tr.y, tr.w, tr.h), "Clipboard", &Text::new(19.0, mm.label3));
    } else {
        let b = ui.text(tr, &q, &Text::new(19.0, mm.label));
        gfx::fill_rounded(ui.cr, Rect::new(b.right() + 1.0, field.cy() - 11.0, 2.0, 22.0), 1.0, mm.accent);
    }
    ui.hit(ui::id("clip.input"), field, Action::None, Role::Input, "Search clipboard history");
    // "More" (⋯) button: clear history.
    let more = Rect::new(field.right() - 42.0, field.cy() - 14.0, 28.0, 28.0);
    let km = ui::id("clip.more");
    let hv = ui.hover_amt(km);
    gfx::stroke_rounded(ui.cr, more, 14.0, 1.2, mm.label2);
    if hv > 0.01 {
        gfx::fill_rounded(ui.cr, more, 14.0, mm.sel.with_alpha(mm.sel.a * hv));
    }
    for k in 0..3 {
        gfx::fill_rounded(ui.cr, Rect::centered(more.cx() - 6.0 + k as f64 * 6.0, more.cy(), 2.6, 2.6), 1.3, mm.label2);
    }
    ui.hit(km, more, Action::Exec("cliphist wipe".into()), Role::Button, "Clear clipboard history");
    gfx::fill_rounded(ui.cr, Rect::new(x + 16.0, field.bottom(), w - 32.0, 1.0), 0.0, mm.stroke);
    let list = Rect::new(x + 8.0, field.bottom() + 7.0, w - 16.0, h - field.h - 14.0);
    let items = super::clipboard::filtered(data, &views.query);
    if items.is_empty() {
        let msg = if data.clips.is_empty() { "Nothing copied yet" } else { "No Results" };
        ui.text(list, msg, &Text::new(13.0, mm.label2).center());
    } else {
        let from = ui.hits.len();
        let rows = super::list_layout(views, list, items.len(), 46.0, 2.0);
        let sel = views.sel;
        super::with_clip(ui, list, |ui| {
            for (i, r) in rows {
                let (id, text) = items[i];
                let one: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
                let image = one.starts_with("[[ binary data");
                let k = ui::idn("clip.row", id);
                let hv = ui.hover_amt(k);
                let a = if sel == i { 1.0 } else { hv * 0.6 };
                if a > 0.01 {
                    gfx::fill_rounded(ui.cr, r, 12.0, mm.sel.with_alpha((mm.sel.a * 1.4).min(0.5) * a));
                }
                // Document glyph with the little blue badge.
                let doc = Rect::new(r.x + 12.0, r.cy() - 13.0, 20.0, 26.0);
                gfx::fill_rounded(ui.cr, doc, 3.0, Color::WHITE);
                gfx::stroke_rounded(ui.cr, doc, 3.0, 1.0, Color::rgba(0.0, 0.0, 0.0, 0.18));
                for l in 0..4 {
                    gfx::fill_rounded(ui.cr, Rect::new(doc.x + 4.0, doc.y + 6.0 + l as f64 * 4.0, 12.0, 1.4), 0.7, Color::rgba(0.0, 0.0, 0.0, 0.25));
                }
                gfx::fill_rounded(ui.cr, Rect::new(doc.right() - 6.0, doc.bottom() - 7.0, 9.0, 9.0), 4.5, mm.accent);
                let tx = r.x + 46.0;
                let label = if image { "Image".to_string() } else { one };
                ui.text(Rect::new(tx, r.y + 4.0, r.w - 100.0, 20.0), &label, &Text::new(13.5, mm.label));
                ui.text(Rect::new(tx, r.y + 23.0, r.w - 100.0, 18.0), if image { "Image · Copied" } else { "Text · Copied" }, &Text::new(11.5, mm.label3));
                let cb = Rect::new(r.right() - 36.0, r.cy() - 11.0, 22.0, 22.0);
                gfx::fill_rounded(ui.cr, cb, 11.0, mm.sel);
                ui.icon(cb, "edit-copy-symbolic", 12.0, Some(mm.label));
                ui.hit(k, r, Action::ClipboardPick(id.clone()), Role::Item, label);
            }
        });
        super::clip_hits(ui, from, list);
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t_open.clamp(0.0, 1.0));
    ui.cr.restore().ok();
    true
}

// ───────────────────────────── notifications ─────────────────────────────

pub fn banner_height(n: &crate::data::Notif) -> f64 {
    let lines = if n.body.is_empty() { 0.0 } else { ((n.body.chars().count() as f64) / 46.0).ceil().clamp(1.0, 2.0) };
    (14.0 + 18.0 + lines * 17.0 + 14.0).max(64.0).round()
}

fn banner(ui: &mut Ui, mm: &M, r: Rect, n: &crate::data::Notif) {
    let k = ui::idn("mac.banner", n.id);
    let hv = ui.hover_amt(k);
    panel(ui, mm, r, panel_radius(mm, 16.0), "notifications");
    let icon = if n.icon.is_empty() { "dialog-information-symbolic".to_string() } else { n.icon.clone() };
    ui.icon(Rect::new(r.x + 12.0, r.cy() - 16.0, 32.0, 32.0), &icon, 32.0, if icon.ends_with("-symbolic") { Some(mm.accent) } else { None });
    let tx = r.x + 54.0;
    ui.text(Rect::new(tx, r.y + 12.0, r.w - 54.0 - 50.0, 18.0), &n.summary, &Text::new(13.0, mm.label).bold());
    ui.text(Rect::new(r.right() - 48.0, r.y + 12.0, 36.0, 18.0), &super::notif_ago(n.arrived), &Text::new(11.0, mm.label2).end());
    if !n.body.is_empty() {
        ui.text(Rect::new(tx, r.y + 31.0, r.w - 54.0 - 14.0, 34.0), &super::notif::strip_markup(&n.body), &Text::new(13.0, mm.label).lines(2));
    }
    let default = n.actions.iter().find(|(k, _)| k == "default").map(|(k, _)| Action::NotifInvoke(n.id, k.clone())).unwrap_or(Action::NotifDismiss(n.id));
    ui.hit(k, r, default, Role::Item, format!("{}: {}", n.app, n.summary));
    if hv > 0.05 {
        // Close button on the top-left corner, as on macOS.
        let x = Rect::centered(r.x + 2.0, r.y + 2.0, 20.0, 20.0);
        gfx::fill_rounded(ui.cr, x, 10.0, if mm.dark { Color::rgba(0.3, 0.3, 0.3, hv) } else { Color::rgba(0.95, 0.95, 0.95, hv) });
        gfx::stroke_rounded(ui.cr, x, 10.0, 0.5, mm.stroke);
        ui.icon(x, "window-close-symbolic", 9.0, Some(mm.label.with_alpha(mm.label.a * hv)));
        ui.hit(ui::idn("mac.banner.x", n.id), x, Action::NotifDismiss(n.id), Role::Button, "Close");
    }
}

pub fn banners(ui: &mut Ui, data: &Data) {
    let mm = m(ui);
    let (sw, _) = ui.size;
    let mut y = 10.0;
    for n in &data.notifs {
        let h = banner_height(n);
        let a = ui.spring(ui::idn("mac.banner.in", n.id), 1.0, 0.0);
        let r = Rect::new(sw - 344.0 - 12.0, y, 344.0, h);
        ui.cr.save().ok();
        ui.cr.push_group();
        ui.cr.translate(((1.0 - a) * 360.0).round(), 0.0);
        banner(ui, &mm, r, n);
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint();
        ui.cr.restore().ok();
        ui.record(&format!("notif.{}", n.id), r, None);
        y += h + 10.0;
    }
}

pub fn notif_center(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let mm = m(ui);
    let (_, sh) = ui.size;
    let col = below_bar(ui, 344.0);
    let area = Rect::new(col.x, col.y, 344.0, sh - col.y - 12.0);
    super::popup_transform(ui, area, t_open, true);
    ui.record("notif-center.panel", area, None);
    let mut y = area.y;
    if !data.history.is_empty() {
        let hdr = Rect::new(area.x + 4.0, y, area.w - 8.0, 28.0);
        ui.text(hdr, "Notification Center", &Text::new(15.0, Color::WHITE).bold());
        let ck = ui::id("mac.nc.clear");
        let cr = Rect::new(hdr.right() - 80.0, hdr.y + 2.0, 80.0, 24.0);
        let hv = ui.hover_amt(ck);
        gfx::fill_rounded(ui.cr, cr, 12.0, Color::rgba(0.3, 0.3, 0.3, 0.6 + 0.2 * hv));
        ui.text(cr, "Clear", &Text::new(11.0, Color::WHITE).center());
        ui.hit(ck, cr, Action::NotifClear, Role::Button, "Clear all notifications");
        y = hdr.bottom() + 8.0;
        for n in data.history.iter().take(6) {
            let h = banner_height(n);
            banner(ui, &mm, Rect::new(area.x, y, area.w, h), n);
            y += h + 8.0;
        }
        y += 6.0;
    }
    // Widgets: calendar (medium) + weather (small) like the default widget stack.
    let wr = panel_radius(&mm, 20.0);
    let cal = Rect::new(area.x, y, area.w, 248.0);
    panel(ui, &mm, cal, wr, "dashboard");
    ui.hit(ui::id("mac.nc.cal"), cal, Action::None, Role::Group, "");
    super::calendar::draw_month(ui, cal.inset(10.0), views.calendar_offset);
    y = cal.bottom() + 10.0;
    if let Some(w) = &data.weather {
        let wbox = Rect::new(area.x, y, (area.w - 10.0) / 2.0, 164.0);
        let g = cairo::LinearGradient::new(0.0, wbox.y, 0.0, wbox.bottom());
        g.add_color_stop_rgba(0.0, 0.29, 0.53, 0.86, 0.95);
        g.add_color_stop_rgba(1.0, 0.18, 0.36, 0.7, 0.95);
        gfx::rounded(ui.cr, wbox, wr);
        let _ = ui.cr.set_source(&g);
        let _ = ui.cr.fill();
        ui.text(Rect::new(wbox.x + 14.0, wbox.y + 12.0, wbox.w - 28.0, 18.0), &w.place, &Text::new(13.0, Color::WHITE).bold());
        ui.text(Rect::new(wbox.x + 14.0, wbox.y + 30.0, wbox.w - 28.0, 44.0), &w.temp, &Text::new(38.0, Color::WHITE));
        ui.icon(Rect::new(wbox.x + 14.0, wbox.bottom() - 58.0, 18.0, 18.0), &w.icon, 16.0, Some(Color::WHITE));
        ui.text(Rect::new(wbox.x + 14.0, wbox.bottom() - 40.0, wbox.w - 28.0, 16.0), &w.desc, &Text::new(11.0, Color::WHITE));
        ui.text(Rect::new(wbox.x + 14.0, wbox.bottom() - 24.0, wbox.w - 28.0, 16.0), &w.hi_lo, &Text::new(11.0, Color::WHITE));
        ui.hit(ui::id("mac.nc.weather"), wbox, Action::None, Role::Label, format!("Weather {} {}", w.temp, w.desc));
    }
}

// ───────────────────────────── volume HUD ─────────────────────────────

pub const OSD_SIZE: (u32, u32) = (300, 64);

pub const HUD_SIZE: (u32, u32) = (200, 200);

/// The square volume/brightness HUD of Big Sur → Ventura (vibrancy) and Aqua (dark smoke):
/// a big glyph over a row of 16 segments.
pub fn osd_hud(ui: &mut Ui, kind: crate::data::OsdKind, value: f64) {
    let mm = m(ui);
    let (w, h) = ui.size;
    let r = Rect::new(0.0, 0.0, w, h);
    let (bg, fg, off) = if mm.aqua {
        (Color::rgba(0.0, 0.0, 0.0, 0.32), Color::rgba(1.0, 1.0, 1.0, 0.95), Color::rgba(1.0, 1.0, 1.0, 0.18))
    } else if mm.dark {
        (Color::rgba(0.17, 0.17, 0.17, 0.82), Color::rgba(1.0, 1.0, 1.0, 0.86), Color::rgba(1.0, 1.0, 1.0, 0.16))
    } else {
        (Color::rgba(0.94, 0.94, 0.94, 0.86), Color::rgba(0.0, 0.0, 0.0, 0.72), Color::rgba(0.0, 0.0, 0.0, 0.12))
    };
    gfx::fill_rounded(ui.cr, r, if mm.aqua { 22.0 } else { 18.0 }, bg);
    if !mm.aqua {
        gfx::stroke_rounded(ui.cr, r, 18.0, 1.0, mm.stroke);
    }
    let c = (r.cx(), r.y + 84.0);
    let muted = value <= 0.0;
    match kind {
        crate::data::OsdKind::Brightness => glyph_sun(ui.cr, c, 62.0, fg),
        crate::data::OsdKind::Mic => glyph_mic(ui.cr, c, 70.0, fg, muted),
        crate::data::OsdKind::Game => ui.icon(Rect::centered(c.0, c.1, 80.0, 80.0), "input-gaming-symbolic", 64.0, Some(fg)),
        crate::data::OsdKind::Volume => glyph_speaker(ui.cr, (c.0 - 6.0, c.1), 84.0, fg, value, muted),
    }
    if kind == crate::data::OsdKind::Game {
        ui.text(Rect::new(r.x, r.bottom() - 46.0, r.w, 20.0), if value > 0.5 { "Game Mode On" } else { "Game Mode Off" }, &Text::new(13.0, fg).bold().center());
        return;
    }
    let v = ui.spring(ui::id("mac.hud"), value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    let lit = (v * 16.0).round() as i32;
    let (sw, gap) = (7.0, 2.0);
    let x0 = r.cx() - (16.0 * sw + 15.0 * gap) / 2.0;
    for i in 0..16 {
        let seg = Rect::new(x0 + i as f64 * (sw + gap), r.bottom() - 38.0, sw, 7.0);
        gfx::fill_rounded(ui.cr, seg, if mm.aqua { 0.5 } else { 1.5 }, if i < lit { fg } else { off });
    }
}

pub fn osd(ui: &mut Ui, kind: crate::data::OsdKind, value: f64) {
    let mm = m(ui);
    let (w, h) = ui.size;
    let r = Rect::new(4.0, 4.0, w - 8.0, h - 8.0);
    panel(ui, &mm, r, panel_radius(&mm, 14.0), "osd");
    let title = match kind {
        crate::data::OsdKind::Brightness => "Display",
        crate::data::OsdKind::Game => "Game Mode",
        crate::data::OsdKind::Mic => "Microphone",
        _ => "Sound",
    };
    ui.text(Rect::new(r.x + 14.0, r.y + 6.0, r.w - 28.0, 18.0), title, &Text::new(13.0, mm.label).bold());
    let sr = Rect::new(r.x + 14.0, r.y + 26.0, r.w - 28.0, 26.0);
    if kind == crate::data::OsdKind::Game {
        ui.icon(Rect::new(sr.x, sr.y, 22.0, sr.h), "input-gaming-symbolic", 16.0, Some(mm.label));
        let label = if value > 0.5 { "On - effects paused" } else { "Off" };
        ui.text(Rect::new(sr.x + 30.0, sr.y, sr.w - 30.0, sr.h), label, &Text::new(13.0, mm.label));
        return;
    }
    let v = ui.spring(ui::id("mac.osd"), value.clamp(0.0, 1.0), value.clamp(0.0, 1.0));
    match kind {
        crate::data::OsdKind::Brightness => big_slider(ui, &mm, sr, v, &|cr, c, col| glyph_sun(cr, c, 14.0, col)),
        crate::data::OsdKind::Mic => big_slider(ui, &mm, sr, v, &|cr, c, col| glyph_mic(cr, c, 14.0, col, value <= 0.0)),
        _ => big_slider(ui, &mm, sr, v, &|cr, c, col| glyph_speaker(cr, c, 13.0, col, 1.0, false)),
    }
}

// ───────────────────────────── app menus ─────────────────────────────

pub const MENU_TITLES: &[&str] = &["File", "Edit", "View", "Window", "Help"];

fn shortcut(mods: &str, key: &str) -> Action {
    Action::Hypr(format!("hl.dsp.send_shortcut({{ mods = \"{mods}\", key = \"{key}\" }})"))
}

/// (label, accelerator text, action). `None` action = separator; `Some(Action::None)` = disabled.
fn items(title: &str, app: &str) -> Vec<(String, &'static str, Option<Action>)> {
    let s = |l: &str, acc: &'static str, a: Action| (l.to_string(), acc, Some(a));
    let sep = || (String::new(), "", None);
    match title {
        "app" => vec![
            s(&format!("About {app}"), "", Action::None),
            sep(),
            s("Settings…", "⌃,", shortcut("CTRL", "comma")),
            sep(),
            s(&format!("Hide {app}"), "⌘H", Action::Hypr("hl.dsp.window.move({ workspace = \"special:hidden\" })".into())),
            s(&format!("Quit {app}"), "⌘Q", Action::Hypr("hl.dsp.window.close()".into())),
        ],
        "File" => vec![
            s("New Window", "⌃N", shortcut("CTRL", "N")),
            s("New Tab", "⌃T", shortcut("CTRL", "T")),
            s("Open…", "⌃O", shortcut("CTRL", "O")),
            sep(),
            s("Save", "⌃S", shortcut("CTRL", "S")),
            s("Save As…", "⇧⌃S", shortcut("CTRL SHIFT", "S")),
            sep(),
            s("Print…", "⌃P", shortcut("CTRL", "P")),
            s("Close Window", "⌘W", Action::Hypr("hl.dsp.window.close()".into())),
        ],
        "Edit" => vec![
            s("Undo", "⌃Z", shortcut("CTRL", "Z")),
            s("Redo", "⇧⌃Z", shortcut("CTRL SHIFT", "Z")),
            sep(),
            s("Cut", "⌃X", shortcut("CTRL", "X")),
            s("Copy", "⌃C", shortcut("CTRL", "C")),
            s("Paste", "⌃V", shortcut("CTRL", "V")),
            s("Select All", "⌃A", shortcut("CTRL", "A")),
            sep(),
            s("Find…", "⌃F", shortcut("CTRL", "F")),
            s("Clipboard History", "⌘V", Action::Toggle(Kind::Clipboard)),
            s("Emoji & Symbols", "⌘.", Action::Toggle(Kind::Emoji)),
        ],
        "View" => vec![
            s("Enter Full Screen", "⌃⌘F", Action::Hypr("hl.dsp.window.fullscreen({ mode = \"fullscreen\" })".into())),
            sep(),
            s("Zoom In", "⌃+", shortcut("CTRL", "plus")),
            s("Zoom Out", "⌃−", shortcut("CTRL", "minus")),
            s("Actual Size", "⌃0", shortcut("CTRL", "0")),
            sep(),
            s("Reload", "⌃R", shortcut("CTRL", "R")),
        ],
        "Window" => vec![
            s("Minimise", "⌘M", Action::Hypr("hl.dsp.window.move({ workspace = \"special:minimized\" })".into())),
            s("Zoom", "", Action::Hypr("hl.dsp.window.fullscreen({ mode = \"maximized\", action = \"toggle\" })".into())),
            s("Move & Resize…", "⌘Z", Action::Open(Kind::Snap)),
            sep(),
            s("Centre Window", "", Action::Hypr("hl.dsp.window.center()".into())),
            s("Float on Top", "", Action::Hypr("hl.dsp.window.pin()".into())),
            sep(),
            s("Mission Control", "⌃↑", Action::Toggle(Kind::Overview)),
        ],
        _ => vec![
            s("Creed Help", "", Action::Exec("xdg-open https://github.com/UmxRK/creedfiles#readme".into())),
            s("Keyboard Shortcuts", "", Action::Exec("creed-settings --page desktop".into())),
        ],
    }
}

pub fn menu_entries(title: &str, app: &str) -> Vec<crate::data::MenuEntry> {
    items(title, app)
        .into_iter()
        .enumerate()
        .map(|(i, (label, accel, a))| crate::data::MenuEntry {
            id: i as i32,
            accel: accel.to_string(),
            label,
            enabled: matches!(&a, Some(x) if *x != Action::None),
            separator: a.is_none(),
            toggle: None,
        })
        .collect()
}

pub fn menu_action(title: &str, id: i32) -> Option<Action> {
    items(title, "").into_iter().nth(id.max(0) as usize).and_then(|(_, _, a)| a).filter(|a| *a != Action::None)
}

/// All dispatcher strings the menus can send (validated against Hyprland in tests).
pub fn menu_dispatches() -> Vec<String> {
    let mut v = vec![];
    for t in ["app", "File", "Edit", "View", "Window", "Help"] {
        for (_, _, a) in items(t, "X") {
            if let Some(Action::Hypr(s)) = a {
                v.push(s);
            }
        }
    }
    v
}

/// The open app menu, styled like an NSMenu: 6 px radius, 22 px rows, accelerators right.
pub fn draw_app_menu(ui: &mut Ui, t_open: f64, data: &Data, x: f64) {
    let mm = m(ui);
    let bar = if ui.th.cfg.bar.enabled { (ui.th.cfg.bar.size + ui.th.cfg.bar.margin) as f64 } else { 0.0 };
    let row = 22.0;
    let entries = &data.menu;
    let w = entries
        .iter()
        .map(|e| ui.measure(&e.label, &Text::new(13.0, mm.label)).0 + ui.measure(&e.accel, &Text::new(13.0, mm.label)).0 + 64.0)
        .fold(220.0, f64::max);
    let h = entries.iter().map(|e| if e.separator { 11.0 } else { row }).sum::<f64>() + 10.0;
    let (sw, _) = ui.size;
    let r = Rect::new(x.min(sw - w - 4.0).max(4.0).round(), bar + 2.0, w.round(), h);
    super::popup_transform(ui, r, t_open, true);
    panel(ui, &mm, r, if mm.tahoe { 12.0 } else { 6.0 }, "menu");
    ui.record("menu.panel", r, None);
    ui.hit(ui::id("menu.panel"), r, Action::None, Role::Group, "");
    let mut y = r.y + 5.0;
    for e in entries {
        if e.separator {
            gfx::fill_rounded(ui.cr, Rect::new(r.x + 10.0, y + 5.0, r.w - 20.0, 1.0), 0.0, mm.stroke);
            y += 11.0;
            continue;
        }
        let rr = Rect::new(r.x + 5.0, y, r.w - 10.0, row);
        let k = ui::idn("mac.menu", e.id);
        let hv = if e.enabled { ui.hover_amt(k) } else { 0.0 };
        if hv > 0.5 {
            gfx::fill_rounded(ui.cr, rr, 4.0, mm.accent);
        }
        let fg = if !e.enabled { mm.label3 } else if hv > 0.5 { Color::WHITE } else { mm.label };
        ui.text(Rect::new(rr.x + 14.0, rr.y, rr.w - 80.0, rr.h), &e.label, &Text::new(13.0, fg));
        if !e.accel.is_empty() {
            ui.text(Rect::new(rr.right() - 70.0, rr.y, 60.0, rr.h), &e.accel, &Text::new(13.0, if hv > 0.5 { Color::WHITE } else { mm.label2 }).end());
        }
        if e.enabled {
            ui.hit(k, rr, Action::MenuItem(e.id), Role::Item, e.label.clone());
        }
        y += row;
    }
}

// ───────────────────────────── Aqua (10.4 Tiger) ─────────────────────────────
// Before Control Center, Notification Center and the centred Spotlight: drop-down menus
// from the menu bar, Tiger's Spotlight panel under the blue magnifier, Dashboard over a
// dimmed screen, and Growl-style "smoke" bubbles.

/// White Aqua drop-down: square top under the menu bar, rounded bottom corners, soft shadow.
fn aqua_menu_bg(ui: &mut Ui, r: Rect) {
    gfx::shadow(ui.cr, r, 5.0, 14.0, 6.0, 0.35);
    let cr = ui.cr;
    cr.new_path();
    let rad = 5.0;
    cr.move_to(r.x, r.y);
    cr.line_to(r.right(), r.y);
    cr.line_to(r.right(), r.bottom() - rad);
    cr.arc(r.right() - rad, r.bottom() - rad, rad, 0.0, PI / 2.0);
    cr.line_to(r.x + rad, r.bottom());
    cr.arc(r.x + rad, r.bottom() - rad, rad, PI / 2.0, PI);
    cr.close_path();
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.96);
    let _ = cr.fill_preserve();
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.18);
    cr.set_line_width(1.0);
    let _ = cr.stroke();
}

/// Aqua's glossy blue selection.
fn aqua_sel(ui: &mut Ui, r: Rect) {
    let g = cairo::LinearGradient::new(0.0, r.y, 0.0, r.bottom());
    g.add_color_stop_rgb(0.0, 0.42, 0.65, 0.97);
    g.add_color_stop_rgb(0.5, 0.24, 0.48, 0.91);
    g.add_color_stop_rgb(0.51, 0.16, 0.40, 0.87);
    g.add_color_stop_rgb(1.0, 0.22, 0.50, 0.94);
    ui.cr.rectangle(r.x, r.y, r.w, r.h);
    let _ = ui.cr.set_source(&g);
    let _ = ui.cr.fill();
}

/// Aqua slider: inset track, blue fill, glossy round knob. Registers a slider hit.
fn aqua_slider(ui: &mut Ui, key: ui::Id, r: Rect, v: f64, which: Slider, label: &str) {
    let v = v.clamp(0.0, 1.0);
    let track = Rect::new(r.x + r.h / 2.0, r.cy() - 3.0, r.w - r.h, 6.0);
    gfx::fill_rounded(ui.cr, track, 3.0, Color::rgba(0.0, 0.0, 0.0, 0.18));
    gfx::fill_rounded(ui.cr, Rect::new(track.x, track.y, track.w * v, 6.0), 3.0, Color::rgba(0.22, 0.48, 0.92, 1.0));
    let kx = track.x + track.w * v;
    let k = Rect::centered(kx, r.cy(), 15.0, 15.0);
    gfx::shadow(ui.cr, k, 7.5, 3.0, 1.0, 0.4);
    let g = cairo::RadialGradient::new(kx - 2.0, r.cy() - 3.0, 1.0, kx, r.cy(), 8.0);
    g.add_color_stop_rgb(0.0, 1.0, 1.0, 1.0);
    g.add_color_stop_rgb(0.6, 0.55, 0.75, 0.98);
    g.add_color_stop_rgb(1.0, 0.18, 0.42, 0.85);
    gfx::rounded(ui.cr, k, 7.5);
    let _ = ui.cr.set_source(&g);
    let _ = ui.cr.fill();
    if ui.focus == Some(key) {
        ui.th.focus_ring(ui.cr, r, 4.0);
    }
    let hit = ui.hit(key, r, Action::Slider(which), Role::Slider, label);
    hit.value = Some(v);
}

/// One menu row; returns its rect. `None` action = disabled (grey) row.
fn aqua_row(ui: &mut Ui, key: ui::Id, r: Rect, label: &str, check: bool, action: Option<Action>) {
    let enabled = action.is_some();
    let hv = if enabled { ui.hover_amt(key) } else { 0.0 };
    let sel = hv > 0.5 || (enabled && ui.focus == Some(key));
    if sel {
        aqua_sel(ui, r);
    }
    let fg = if !enabled { Color::rgba(0.0, 0.0, 0.0, 0.35) } else if sel { Color::WHITE } else { Color::BLACK };
    if check {
        ui.text(Rect::new(r.x + 6.0, r.y, 14.0, r.h), "✓", &Text::new(13.0, fg));
    }
    ui.text(Rect::new(r.x + 20.0, r.y, r.w - 30.0, r.h), label, &Text::new(13.0, fg));
    if let Some(a) = action {
        let hit = ui.hit(key, r, a, if check { Role::Toggle } else { Role::Item }, label);
        if check {
            hit.value = Some(1.0);
        }
    }
}

fn aqua_sep(ui: &mut Ui, x: f64, y: f64, w: f64) {
    gfx::fill_rounded(ui.cr, Rect::new(x + 1.0, y + 5.0, w - 2.0, 1.0), 0.0, Color::rgba(0.0, 0.0, 0.0, 0.12));
}

fn aqua_right_drop(ui: &Ui, w: f64) -> Rect {
    let (sw, _) = ui.size;
    let bar = ui.th.cfg.bar.size as f64;
    Rect::new((sw - w - 6.0).round(), bar, w, 0.0)
}

/// Tiger's Spotlight: a panel dropping from the blue magnifier at the right of the menu bar.
pub fn aqua_spotlight(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let results = super::launcher::search(&data.apps, &views.query);
    let special = super::launcher::special_label(&views.query);
    let n = (results.len() + special.is_some() as usize).min(12);
    let mut r = aqua_right_drop(ui, 340.0);
    let rows_h = if views.query.is_empty() { 0.0 } else { (n.max(1) as f64) * 20.0 + 8.0 };
    r.h = 38.0 + rows_h;
    super::popup_transform(ui, r, t_open, true);
    aqua_menu_bg(ui, r);
    ui.record("launcher.panel", r, None);
    ui.hit(ui::id("launcher.panel"), r, Action::None, Role::Group, "");
    ui.text(Rect::new(r.x + 12.0, r.y + 8.0, 80.0, 22.0), "Spotlight", &Text::new(13.0, Color::BLACK).bold());
    let field = Rect::new(r.x + 96.0, r.y + 8.0, r.w - 108.0, 22.0);
    gfx::fill_rounded(ui.cr, field, 11.0, Color::WHITE);
    gfx::stroke_rounded(ui.cr, field, 11.0, 1.0, Color::rgba(0.0, 0.0, 0.0, 0.45));
    let ig = cairo::LinearGradient::new(0.0, field.y, 0.0, field.y + 5.0);
    ig.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 0.18);
    ig.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
    gfx::rounded(ui.cr, field.inset(1.0), 10.0);
    let _ = ui.cr.set_source(&ig);
    let _ = ui.cr.fill();
    glyph_search(ui.cr, (field.x + 12.0, field.cy()), 11.0, Color::rgba(0.0, 0.0, 0.0, 0.55));
    let q = format!("{}{}", views.query, views.preedit);
    let tr = Rect::new(field.x + 22.0, field.y, field.w - 30.0, field.h);
    let b = ui.text(tr, &q, &Text::new(12.0, Color::BLACK));
    gfx::fill_rounded(ui.cr, Rect::new(if q.is_empty() { tr.x } else { b.right() + 1.0 }, field.y + 4.0, 1.2, field.h - 8.0), 0.0, Color::BLACK);
    ui.hit(ui::id("launcher.input"), field, Action::None, Role::Input, "Spotlight");
    if views.query.is_empty() {
        return;
    }
    let mut y = r.y + 40.0;
    if n == 0 {
        ui.text(Rect::new(r.x, y, r.w, 20.0), "No Results", &Text::new(12.0, Color::rgba(0.0, 0.0, 0.0, 0.5)).center());
        return;
    }
    for i in 0..n {
        let row = Rect::new(r.x + 1.0, y, r.w - 2.0, 20.0);
        let sel = views.sel == i;
        let k = ui::idn("aqua.sp", i);
        let hv = ui.hover_amt(k);
        if sel || hv > 0.5 {
            aqua_sel(ui, row);
        }
        let fg = if sel || hv > 0.5 { Color::WHITE } else { Color::BLACK };
        // Category column on the left, like Tiger's results menu.
        // Calculator result first (if any), then the top hit, then the other applications.
        let top = special.is_some() as usize;
        let cat = if special.is_some() && i == 0 { "Calculator" } else if i == top { "Top Hit" } else if i == top + 1 { "Applications" } else { "" };
        ui.text(Rect::new(row.x, row.y, 86.0, row.h), cat, &Text::new(11.0, if sel || hv > 0.5 { Color::WHITE } else { Color::rgba(0.0, 0.0, 0.0, 0.5) }).end());
        let (icon, title) = match (i, special.as_ref()) {
            (0, Some((t, _, ic))) => (ic.to_string(), t.clone()),
            _ => {
                let a = &data.apps[results[i - special.is_some() as usize]];
                (a.icon.clone(), a.name.clone())
            }
        };
        ui.icon(Rect::new(row.x + 96.0, row.y, 16.0, row.h), &icon, 16.0, if icon.ends_with("-symbolic") { Some(fg) } else { None });
        ui.text(Rect::new(row.x + 118.0, row.y, row.w - 126.0, row.h), &title, &Text::new(12.0, fg));
        ui.hit(k, row, Action::LauncherPick(i), Role::Item, title);
        y += 20.0;
    }
}

/// Tiger menu extras: AirPort, Bluetooth, volume and brightness in one drop-down.
pub fn aqua_menu_extra(ui: &mut Ui, t_open: f64, data: &Data, _views: &mut Views) {
    let mut r = aqua_right_drop(ui, 250.0);
    let wifi = data.net.wifi_enabled;
    let bt = data.bt.as_ref().map(|b| b.powered);
    let mut h = 6.0 + 20.0 * 2.0 + 11.0;
    if bt.is_some() {
        h += 40.0 + 11.0;
    }
    if data.audio.is_some() {
        h += 20.0 + 26.0;
    }
    if data.brightness.is_some() {
        h += 20.0 + 26.0;
    }
    h += 11.0 + 20.0 + 11.0 + 20.0 + 6.0;
    r.h = h;
    super::popup_transform(ui, r, t_open, true);
    aqua_menu_bg(ui, r);
    ui.record("control.panel", r, None);
    ui.hit(ui::id("control.panel"), r, Action::None, Role::Group, "");
    let mut y = r.y + 6.0;
    let row = |y: f64| Rect::new(r.x + 1.0, y, r.w - 2.0, 20.0);
    aqua_row(ui, ui::id("aq.ap.state"), row(y), if wifi { "AirPort: On" } else { "AirPort: Off" }, false, None);
    y += 20.0;
    aqua_row(ui, ui::id("aq.ap"), row(y), if wifi { "Turn AirPort Off" } else { "Turn AirPort On" }, false, Some(Action::ToggleWifi));
    y += 20.0;
    aqua_sep(ui, r.x, y, r.w);
    y += 11.0;
    if let Some(on) = bt {
        aqua_row(ui, ui::id("aq.bt.state"), row(y), if on { "Bluetooth: On" } else { "Bluetooth: Off" }, false, None);
        y += 20.0;
        aqua_row(ui, ui::id("aq.bt"), row(y), if on { "Turn Bluetooth Off" } else { "Turn Bluetooth On" }, false, Some(Action::ToggleBluetooth));
        y += 20.0;
        aqua_sep(ui, r.x, y, r.w);
        y += 11.0;
    }
    if let Some(a) = &data.audio {
        aqua_row(ui, ui::id("aq.vol.l"), row(y), "Volume", false, None);
        y += 20.0;
        aqua_slider(ui, ui::id("aq.vol"), Rect::new(r.x + 20.0, y, r.w - 40.0, 22.0), if a.muted { 0.0 } else { a.volume }, Slider::Volume, &format!("Volume {:.0}%", a.volume * 100.0));
        y += 26.0;
    }
    if let Some(b) = data.brightness {
        aqua_row(ui, ui::id("aq.bri.l"), row(y), "Brightness", false, None);
        y += 20.0;
        aqua_slider(ui, ui::id("aq.bri"), Rect::new(r.x + 20.0, y, r.w - 40.0, 22.0), b, Slider::Brightness, &format!("Brightness {:.0}%", b * 100.0));
        y += 26.0;
    }
    aqua_sep(ui, r.x, y, r.w);
    y += 11.0;
    aqua_row(ui, ui::id("aq.game"), row(y), "Game Mode", data.game, Some(Action::ToggleGame));
    y += 20.0;
    aqua_sep(ui, r.x, y, r.w);
    y += 11.0;
    aqua_row(ui, ui::id("aq.prefs"), row(y), "Open System Preferences…", false, Some(Action::Exec("creed-settings".into())));
}

/// Tiger's clock menu: the date, then the recent notifications and a way to clear them.
pub fn aqua_clock_menu(ui: &mut Ui, t_open: f64, data: &Data, _views: &mut Views) {
    let mut r = aqua_right_drop(ui, 300.0);
    let items: Vec<&crate::data::Notif> = data.history.iter().rev().take(8).collect();
    r.h = 6.0 + 20.0 + 11.0 + if items.is_empty() { 20.0 } else { items.len() as f64 * 20.0 } + 11.0 + 40.0 + 6.0;
    super::popup_transform(ui, r, t_open, true);
    aqua_menu_bg(ui, r);
    ui.record("notif-center.panel", r, None);
    ui.hit(ui::id("nc.panel"), r, Action::None, Role::Group, "");
    let mut y = r.y + 6.0;
    let row = |y: f64| Rect::new(r.x + 1.0, y, r.w - 2.0, 20.0);
    aqua_row(ui, ui::id("aq.date"), row(y), &time::format(time::now_secs(), "%A, %-d %B %Y"), false, None);
    y += 20.0;
    aqua_sep(ui, r.x, y, r.w);
    y += 11.0;
    if items.is_empty() {
        aqua_row(ui, ui::id("aq.none"), row(y), "No Recent Notifications", false, None);
        y += 20.0;
    }
    for n in items {
        let label = format!("{} - {}", n.app, super::notif::strip_markup(&n.summary));
        aqua_row(ui, ui::idn("aq.n", n.id), row(y), &label, false, Some(Action::NotifDismiss(n.id)));
        y += 20.0;
    }
    aqua_sep(ui, r.x, y, r.w);
    y += 11.0;
    aqua_row(ui, ui::id("aq.clear"), row(y), "Clear History", false, Some(Action::NotifClear));
    y += 20.0;
    aqua_row(ui, ui::id("aq.dt"), row(y), "Open Date & Time…", false, Some(Action::Exec("creed-settings --page desktop".into())));
}

/// Growl "Smoke": a dark translucent rounded bubble with white text, top-right.
pub fn growl_height(n: &crate::data::Notif) -> f64 {
    let lines = if n.body.is_empty() { 0.0 } else { ((n.body.chars().count() as f64) / 40.0).ceil().clamp(1.0, 3.0) };
    (12.0 + 17.0 + lines * 15.0 + 12.0).max(56.0).round()
}

pub fn growl(ui: &mut Ui, data: &Data) {
    let (sw, _) = ui.size;
    let mut y = 8.0;
    for n in &data.notifs {
        let h = growl_height(n);
        let r = Rect::new(sw - 300.0 - 12.0, y, 300.0, h);
        let k = ui::idn("growl", n.id);
        let t = ui.spring(ui::idn("growl.in", n.id), 1.0, 0.0);
        ui.cr.save().ok();
        ui.cr.push_group();
        gfx::fill_rounded(ui.cr, r, 10.0, Color::rgba(0.0, 0.0, 0.0, 0.72));
        let icon = if n.icon.is_empty() { "dialog-information-symbolic".to_string() } else { n.icon.clone() };
        ui.icon(Rect::new(r.x + 10.0, r.y + 10.0, 32.0, 32.0), &icon, 32.0, if icon.ends_with("-symbolic") { Some(Color::WHITE) } else { None });
        ui.text(Rect::new(r.x + 52.0, r.y + 10.0, r.w - 62.0, 17.0), &super::notif::strip_markup(&n.summary), &Text::new(13.0, Color::WHITE).bold());
        if !n.body.is_empty() {
            ui.text(Rect::new(r.x + 52.0, r.y + 28.0, r.w - 62.0, h - 36.0), &super::notif::strip_markup(&n.body), &Text::new(12.0, Color::rgba(1.0, 1.0, 1.0, 0.9)).lines(3));
        }
        let _ = ui.cr.pop_group_to_source();
        let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
        ui.cr.restore().ok();
        ui.record(&format!("notif.{}", n.id), r, None);
        ui.hit(k, r, Action::NotifDismiss(n.id), Role::Item, format!("{}: {}", n.app, n.summary));
        y += h + 8.0;
    }
}

/// Tiger's Dashboard: the screen dims and the widgets float over it.
pub fn aqua_dashboard(ui: &mut Ui, t_open: f64, data: &Data, views: &mut Views) {
    let (sw, sh) = ui.size;
    ui.cr.rectangle(0.0, 0.0, sw, sh);
    ui.cr.set_source_rgba(0.0, 0.0, 0.0, 0.45 * t_open.clamp(0.0, 1.0));
    let _ = ui.cr.fill();
    let scale = 0.92 + 0.08 * t_open.clamp(0.0, 1.0);
    ui.cr.save().ok();
    ui.cr.translate(sw / 2.0, sh / 2.0);
    ui.cr.scale(scale, scale);
    ui.cr.translate(-sw / 2.0, -sh / 2.0);
    let cx = sw / 2.0;
    let cy = sh * 0.42;
    // Clock: a black glossy face.
    let cc = (cx - 260.0, cy);
    let face = Rect::centered(cc.0, cc.1, 150.0, 150.0);
    gfx::shadow(ui.cr, face, 75.0, 12.0, 6.0, 0.5);
    gfx::fill_rounded(ui.cr, face, 75.0, Color::rgba(0.08, 0.08, 0.1, 1.0));
    gfx::fill_rounded(ui.cr, face.inset(8.0), 67.0, Color::WHITE);
    let now = time::local(time::now_secs());
    for i in 0..12 {
        let a = i as f64 / 12.0 * 2.0 * PI;
        let (s, c) = a.sin_cos();
        ui.cr.move_to(cc.0 + s * 54.0, cc.1 - c * 54.0);
        ui.cr.line_to(cc.0 + s * 62.0, cc.1 - c * 62.0);
    }
    ui.cr.set_source_rgba(0.0, 0.0, 0.0, 0.8);
    ui.cr.set_line_width(2.0);
    let _ = ui.cr.stroke();
    let hand = |ui: &mut Ui, frac: f64, len: f64, wdt: f64, col: Color| {
        let a = frac * 2.0 * PI;
        ui.cr.move_to(cc.0, cc.1);
        ui.cr.line_to(cc.0 + a.sin() * len, cc.1 - a.cos() * len);
        ui.cr.set_line_width(wdt);
        ui.cr.set_line_cap(cairo::LineCap::Round);
        gfx::set(ui.cr, col);
        let _ = ui.cr.stroke();
    };
    hand(ui, (now.hour % 12) as f64 / 12.0 + now.min as f64 / 720.0, 34.0, 4.0, Color::BLACK);
    hand(ui, now.min as f64 / 60.0, 50.0, 3.0, Color::BLACK);
    let place = data.weather.as_ref().map(|w| w.place.clone()).unwrap_or_default();
    ui.text(Rect::new(face.x, face.bottom() + 6.0, face.w, 18.0), if place.is_empty() { "Local Time" } else { &place }, &Text::new(12.0, Color::WHITE).bold().center());
    // Calendar: a page with the month across a red band.
    let cal = Rect::centered(cx, cy, 150.0, 150.0);
    gfx::shadow(ui.cr, cal, 10.0, 12.0, 6.0, 0.5);
    gfx::fill_rounded(ui.cr, cal, 10.0, Color::WHITE);
    ui.cr.save().ok();
    gfx::rounded(ui.cr, cal, 10.0);
    ui.cr.clip();
    let band = cairo::LinearGradient::new(0.0, cal.y, 0.0, cal.y + 34.0);
    band.add_color_stop_rgb(0.0, 0.9, 0.25, 0.22);
    band.add_color_stop_rgb(1.0, 0.68, 0.1, 0.1);
    ui.cr.rectangle(cal.x, cal.y, cal.w, 34.0);
    let _ = ui.cr.set_source(&band);
    let _ = ui.cr.fill();
    ui.cr.restore().ok();
    ui.text(Rect::new(cal.x, cal.y, cal.w, 34.0), &time::format(time::now_secs(), "%B"), &Text::new(15.0, Color::WHITE).bold().center());
    ui.text(Rect::new(cal.x, cal.y + 38.0, cal.w, 76.0), &now.day.to_string(), &Text::new(64.0, Color::BLACK).bold().center());
    ui.text(Rect::new(cal.x, cal.bottom() - 30.0, cal.w, 20.0), &time::format(time::now_secs(), "%A"), &Text::new(13.0, Color::rgba(0.0, 0.0, 0.0, 0.6)).center());
    ui.hit(ui::id("aq.dash.cal"), cal, Action::Toggle(Kind::Calendar), Role::Button, format!("Calendar, {}", time::format(time::now_secs(), "%A %-d %B")));
    // Weather: the blue glossy box.
    let wr = Rect::new(cx + 110.0, cy - 70.0, 300.0, 140.0);
    gfx::shadow(ui.cr, wr, 14.0, 12.0, 6.0, 0.5);
    let wg = cairo::LinearGradient::new(0.0, wr.y, 0.0, wr.bottom());
    wg.add_color_stop_rgb(0.0, 0.33, 0.6, 0.95);
    wg.add_color_stop_rgb(1.0, 0.1, 0.32, 0.72);
    gfx::rounded(ui.cr, wr, 14.0);
    let _ = ui.cr.set_source(&wg);
    let _ = ui.cr.fill();
    let gl = cairo::LinearGradient::new(0.0, wr.y, 0.0, wr.cy());
    gl.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.35);
    gl.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
    gfx::rounded(ui.cr, Rect::new(wr.x + 2.0, wr.y + 2.0, wr.w - 4.0, wr.h / 2.0), 12.0);
    let _ = ui.cr.set_source(&gl);
    let _ = ui.cr.fill();
    match &data.weather {
        Some(w) => {
            ui.icon(Rect::new(wr.x + 16.0, wr.y + 20.0, 64.0, 64.0), &w.icon, 56.0, Some(Color::WHITE));
            ui.text(Rect::new(wr.x + 96.0, wr.y + 16.0, wr.w - 110.0, 60.0), &w.temp, &Text::new(48.0, Color::WHITE).bold().end());
            ui.text(Rect::new(wr.x + 16.0, wr.bottom() - 46.0, wr.w - 32.0, 18.0), &w.place, &Text::new(13.0, Color::WHITE).bold());
            ui.text(Rect::new(wr.x + 16.0, wr.bottom() - 26.0, wr.w - 32.0, 18.0), &format!("{} · {}", w.desc, w.hi_lo), &Text::new(12.0, Color::rgba(1.0, 1.0, 1.0, 0.9)));
        }
        None => {
            ui.text(wr, "Weather unavailable", &Text::new(14.0, Color::WHITE).bold().center());
        }
    }
    ui.hit(ui::id("aq.dash.w"), wr, Action::None, Role::Label, "Weather");
    ui.cr.restore().ok();
    // The round "+" button bottom-left opens the widget settings.
    let plus = Rect::new(18.0, sh - 54.0, 36.0, 36.0);
    gfx::fill_rounded(ui.cr, plus, 18.0, Color::rgba(1.0, 1.0, 1.0, 0.25));
    gfx::stroke_rounded(ui.cr, plus, 18.0, 1.5, Color::rgba(1.0, 1.0, 1.0, 0.8));
    ui.text(plus, "+", &Text::new(24.0, Color::WHITE).bold().center());
    ui.hit(ui::id("aq.dash.plus"), plus, Action::Exec("creed-settings --page desktop".into()), Role::Button, "Widget settings");
    let _ = views;
}
