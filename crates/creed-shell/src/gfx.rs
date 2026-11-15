//! Drawing primitives on top of cairo + pango, and the material painter (glass, frost,
//! mica, acrylic, aqua, solid). Everything is in logical pixels; the surface applies scale.

use cairo::{Context, LinearGradient, RadialGradient};
use creed_core::config::{Config, Material, Motion, Scheme};
use creed_core::palette::Palette;
use creed_core::Color;
use std::f64::consts::PI;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn contains(&self, px: f64, py: f64) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn inset(&self, d: f64) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2.0 * d).max(0.0), (self.h - 2.0 * d).max(0.0))
    }
    pub fn inset_xy(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, (self.w - 2.0 * dx).max(0.0), (self.h - 2.0 * dy).max(0.0))
    }
    pub fn cx(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn right(&self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }
    pub fn centered(cx: f64, cy: f64, w: f64, h: f64) -> Rect {
        Rect::new(cx - w / 2.0, cy - h / 2.0, w, h)
    }
    /// Takes `h` from the top and returns (taken, rest), with `gap` between them.
    pub fn take_top(&self, h: f64, gap: f64) -> (Rect, Rect) {
        let h = h.min(self.h);
        (Rect::new(self.x, self.y, self.w, h), Rect::new(self.x, self.y + h + gap, self.w, (self.h - h - gap).max(0.0)))
    }
    pub fn take_left(&self, w: f64, gap: f64) -> (Rect, Rect) {
        let w = w.min(self.w);
        (Rect::new(self.x, self.y, w, self.h), Rect::new(self.x + w + gap, self.y, (self.w - w - gap).max(0.0), self.h))
    }
    pub fn take_right(&self, w: f64, gap: f64) -> (Rect, Rect) {
        let w = w.min(self.w);
        (Rect::new(self.right() - w, self.y, w, self.h), Rect::new(self.x, self.y, (self.w - w - gap).max(0.0), self.h))
    }
    pub fn take_bottom(&self, h: f64, gap: f64) -> (Rect, Rect) {
        let h = h.min(self.h);
        (Rect::new(self.x, self.bottom() - h, self.w, h), Rect::new(self.x, self.y, self.w, (self.h - h - gap).max(0.0)))
    }
    /// Splits horizontally into `n` equal columns separated by `gap`.
    pub fn columns(&self, n: usize, gap: f64) -> Vec<Rect> {
        let n = n.max(1);
        let w = (self.w - gap * (n as f64 - 1.0)) / n as f64;
        (0..n).map(|i| Rect::new(self.x + i as f64 * (w + gap), self.y, w, self.h)).collect()
    }
    pub fn rows(&self, n: usize, gap: f64) -> Vec<Rect> {
        let n = n.max(1);
        let h = (self.h - gap * (n as f64 - 1.0)) / n as f64;
        (0..n).map(|i| Rect::new(self.x, self.y + i as f64 * (h + gap), self.w, h)).collect()
    }
    pub fn round(&self) -> Rect {
        Rect::new(self.x.round(), self.y.round(), self.w.round(), self.h.round())
    }
}

pub fn set(cr: &Context, c: Color) {
    cr.set_source_rgba(c.r, c.g, c.b, c.a);
}

/// Rounded rectangle path with a "squircle-ish" smoothing: radius is clamped to half the
/// smaller side so pills stay perfect pills.
pub fn rounded(cr: &Context, r: Rect, radius: f64) {
    let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
    cr.new_sub_path();
    if rad <= 0.01 {
        cr.rectangle(r.x, r.y, r.w, r.h);
        return;
    }
    cr.arc(r.x + r.w - rad, r.y + rad, rad, -PI / 2.0, 0.0);
    cr.arc(r.x + r.w - rad, r.y + r.h - rad, rad, 0.0, PI / 2.0);
    cr.arc(r.x + rad, r.y + r.h - rad, rad, PI / 2.0, PI);
    cr.arc(r.x + rad, r.y + rad, rad, PI, 1.5 * PI);
    cr.close_path();
}

pub fn fill_rounded(cr: &Context, r: Rect, radius: f64, c: Color) {
    rounded(cr, r, radius);
    set(cr, c);
    let _ = cr.fill();
}

pub fn stroke_rounded(cr: &Context, r: Rect, radius: f64, width: f64, c: Color) {
    let h = width / 2.0;
    rounded(cr, Rect::new(r.x + h, r.y + h, r.w - width, r.h - width), (radius - h).max(0.0));
    set(cr, c);
    cr.set_line_width(width);
    let _ = cr.stroke();
}

/// Soft drop shadow: stacked rounded rects with quadratic alpha falloff. Cheap and smooth
/// enough for UI radii up to ~24 px.
pub fn shadow(cr: &Context, r: Rect, radius: f64, spread: f64, dy: f64, strength: f64) {
    if strength <= 0.0 || spread <= 0.0 {
        return;
    }
    let steps = (spread / 1.5).ceil().clamp(4.0, 16.0) as i32;
    let total = strength * 0.5;
    for i in 0..steps {
        let t = i as f64 / steps as f64; // 0 outermost
        let grow = spread * (1.0 - t);
        let a = total * (t * t) / steps as f64 * 3.0;
        rounded(cr, Rect::new(r.x - grow, r.y - grow + dy, r.w + grow * 2.0, r.h + grow * 2.0), radius + grow);
        cr.set_source_rgba(0.0, 0.0, 0.0, a.min(1.0));
        let _ = cr.fill();
    }
}

/// Everything the views need to look right.
pub struct Theme {
    pub pal: Palette,
    pub cfg: Config,
    pub font: String,
    pub mono: String,
    /// Base font size in logical pixels.
    pub fs: f64,
    pub dark: bool,
}

impl Theme {
    pub fn new(cfg: Config, pal: Palette) -> Theme {
        let font = cfg.font().to_string();
        let mono = cfg.appearance.mono_font.clone();
        // font_size is in points; 1pt = 4/3 px at 96 dpi.
        let fs = cfg.appearance.font_size * 4.0 / 3.0 * cfg.appearance.text_scale;
        let dark = cfg.scheme() == Scheme::Dark;
        Theme { pal, cfg, font, mono, fs, dark }
    }

    pub fn radius(&self) -> f64 {
        self.cfg.style.radius
    }
    pub fn radius_small(&self) -> f64 {
        self.cfg.style.radius_small
    }
    pub fn pad(&self) -> f64 {
        self.cfg.style.padding
    }
    pub fn gap(&self) -> f64 {
        self.cfg.style.spacing
    }
    pub fn motion(&self) -> Motion {
        self.cfg.effective_motion()
    }
    pub fn variant(&self, which: &str) -> &str {
        let s = &self.cfg.style;
        match which {
            "bar" => &s.bar,
            "dock" => &s.dock,
            "launcher" => &s.launcher,
            "control" => &s.control,
            "overview" => &s.overview,
            "dashboard" => &s.dashboard,
            "notifications" => &s.notifications,
            "osd" => &s.osd,
            _ => "creed",
        }
    }

    /// Paints a panel using the mode's material. `element` selects per-element glass overrides.
    /// `pointer` (relative to the surface) drives the cursor-reactive specular highlight.
    pub fn panel(&self, cr: &Context, r: Rect, radius: f64, element: &str, pointer: Option<(f64, f64)>) {
        let p = &self.pal;
        let st = &self.cfg.style;
        if st.shadow > 0.0 {
            shadow(cr, r, radius, 10.0, 3.0, st.shadow);
        }
        let glass_on = self.cfg.glass.enabled;
        let (tint, frost, specular) = self.cfg.glass_for(element);
        let material = if glass_on { st.material } else { Material::Solid };
        let base = if self.dark { p.surface } else { p.surface };
        match material {
            Material::Solid => fill_rounded(cr, r, radius, base),
            Material::Frost | Material::Mica | Material::Acrylic => {
                // Compositor blurs what's behind; we lay a translucent tone on top.
                let alpha = match material {
                    Material::Mica => frost.max(0.8),
                    Material::Acrylic => frost.min(0.75),
                    _ => frost,
                };
                fill_rounded(cr, r, radius, base.with_alpha(alpha));
                fill_rounded(cr, r, radius, tint);
                if material == Material::Acrylic {
                    noise(cr, r, radius, if self.dark { 0.035 } else { 0.025 });
                }
            }
            Material::Glass => {
                // Liquid glass: very light fill, bright rim, specular sheen.
                fill_rounded(cr, r, radius, base.with_alpha(frost * 0.55));
                fill_rounded(cr, r, radius, tint);
                // inner glow at edges (lens thickness)
                rounded(cr, r, radius);
                let edge = LinearGradient::new(r.x, r.y, r.x, r.bottom());
                let hi = if self.dark { 0.10 } else { 0.22 };
                edge.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, hi * specular * 2.0);
                edge.add_color_stop_rgba(0.35, 1.0, 1.0, 1.0, 0.0);
                edge.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, hi * specular);
                let _ = cr.set_source(&edge);
                let _ = cr.fill();
                if specular > 0.0 {
                    let (sx, sy) = match (pointer, self.cfg.glass.specular_follow_cursor) {
                        (Some((x, y)), true) if r.contains(x, y) => (x, y),
                        _ => (r.x + r.w * 0.25, r.y),
                    };
                    let rad = (r.w.max(r.h) * 0.6).max(40.0);
                    let g = RadialGradient::new(sx, sy, 0.0, sx, sy, rad);
                    g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.16 * specular);
                    g.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
                    rounded(cr, r, radius);
                    let _ = cr.set_source(&g);
                    let _ = cr.fill();
                }
                // Rim: lighter on top, faint below - like light caught on a lens edge.
                let rim = LinearGradient::new(r.x, r.y, r.x, r.bottom());
                rim.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, 0.55 * specular.max(0.2));
                rim.add_color_stop_rgba(0.5, 1.0, 1.0, 1.0, 0.08);
                rim.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.25 * specular.max(0.2));
                rounded(cr, r.inset(0.5), (radius - 0.5).max(0.0));
                let _ = cr.set_source(&rim);
                cr.set_line_width(1.0);
                let _ = cr.stroke();
                return;
            }
            Material::Aqua => {
                // Brushed pinstripe base with glossy top - the early-2000s look.
                fill_rounded(cr, r, radius, if self.dark { p.surface } else { Color::rgba(0.93, 0.93, 0.93, 0.97) });
                cr.save().ok();
                rounded(cr, r, radius);
                cr.clip();
                let stripe = if self.dark { Color::rgba(1.0, 1.0, 1.0, 0.025) } else { Color::rgba(1.0, 1.0, 1.0, 0.55) };
                set(cr, stripe);
                let mut y = r.y;
                while y < r.bottom() {
                    cr.rectangle(r.x, y, r.w, 1.0);
                    y += 4.0;
                }
                let _ = cr.fill();
                cr.restore().ok();
            }
        }
        if st.border_width > 0.0 {
            let border = if self.dark { Color::rgba(1.0, 1.0, 1.0, 0.08) } else { Color::rgba(0.0, 0.0, 0.0, 0.10) };
            stroke_rounded(cr, r, radius, st.border_width, border);
        }
        if specular > 0.0 && matches!(material, Material::Frost | Material::Acrylic) {
            // A single hairline highlight on the top edge - restrained, no glow.
            cr.save().ok();
            rounded(cr, r, radius);
            cr.clip();
            let g = LinearGradient::new(r.x, r.y, r.x, r.y + radius.max(6.0));
            g.add_color_stop_rgba(0.0, 1.0, 1.0, 1.0, (if self.dark { 0.09 } else { 0.35 }) * specular);
            g.add_color_stop_rgba(1.0, 1.0, 1.0, 1.0, 0.0);
            cr.rectangle(r.x, r.y, r.w, radius.max(6.0));
            let _ = cr.set_source(&g);
            let _ = cr.fill();
            cr.restore().ok();
        }
    }

    /// Small control surface (buttons, list rows) on top of a panel.
    pub fn chip(&self, cr: &Context, r: Rect, radius: f64, hover: f64, active: bool) {
        let p = &self.pal;
        let c = if active {
            p.accent
        } else {
            let base = if self.dark { Color::rgba(1.0, 1.0, 1.0, 0.06) } else { Color::rgba(0.0, 0.0, 0.0, 0.05) };
            let hov = if self.dark { Color::rgba(1.0, 1.0, 1.0, 0.13) } else { Color::rgba(0.0, 0.0, 0.0, 0.10) };
            base.mix(hov, hover)
        };
        fill_rounded(cr, r, radius, c);
        if active && hover > 0.0 {
            fill_rounded(cr, r, radius, Color::rgba(1.0, 1.0, 1.0, 0.08 * hover));
        }
    }

    pub fn hover_bg(&self, cr: &Context, r: Rect, radius: f64, hover: f64) {
        if hover <= 0.001 {
            return;
        }
        let c = if self.dark { Color::rgba(1.0, 1.0, 1.0, 0.10 * hover) } else { Color::rgba(0.0, 0.0, 0.0, 0.07 * hover) };
        fill_rounded(cr, r, radius, c);
    }

    pub fn focus_ring(&self, cr: &Context, r: Rect, radius: f64) {
        if !self.cfg.a11y.focus_ring {
            return;
        }
        stroke_rounded(cr, Rect::new(r.x - 2.0, r.y - 2.0, r.w + 4.0, r.h + 4.0), radius + 2.0, 2.0, self.pal.accent);
    }
}

/// Deterministic fine grain for acrylic (no RNG, so screenshots are stable).
fn noise(cr: &Context, r: Rect, radius: f64, strength: f64) {
    cr.save().ok();
    rounded(cr, r, radius);
    cr.clip();
    let mut seed: u32 = 0x9e3779b9;
    let step = 3.0;
    let mut y = r.y;
    while y < r.bottom() {
        let mut x = r.x;
        while x < r.right() {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let v = (seed & 0xff) as f64 / 255.0;
            if v > 0.5 {
                cr.set_source_rgba(1.0, 1.0, 1.0, strength * (v - 0.5) * 2.0);
            } else {
                cr.set_source_rgba(0.0, 0.0, 0.0, strength * (0.5 - v) * 2.0);
            }
            cr.rectangle(x, y, 1.0, 1.0);
            let _ = cr.fill();
            x += step;
        }
        y += step;
    }
    cr.restore().ok();
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Align {
    Start,
    Center,
    End,
}

#[derive(Clone, Copy, Debug)]
pub struct Text {
    pub size: f64,
    pub weight: pango::Weight,
    pub color: Color,
    pub align: Align,
    pub mono: bool,
    pub wrap: bool,
    pub max_lines: i32,
}

impl Text {
    pub fn new(size: f64, color: Color) -> Text {
        Text { size, weight: pango::Weight::Normal, color, align: Align::Start, mono: false, wrap: false, max_lines: 1 }
    }
    pub fn bold(mut self) -> Text {
        self.weight = pango::Weight::Semibold;
        self
    }
    pub fn heavy(mut self) -> Text {
        self.weight = pango::Weight::Bold;
        self
    }
    pub fn medium(mut self) -> Text {
        self.weight = pango::Weight::Medium;
        self
    }
    pub fn center(mut self) -> Text {
        self.align = Align::Center;
        self
    }
    pub fn end(mut self) -> Text {
        self.align = Align::End;
        self
    }
    pub fn mono(mut self) -> Text {
        self.mono = true;
        self
    }
    pub fn lines(mut self, n: i32) -> Text {
        self.wrap = n != 1;
        self.max_lines = n;
        self
    }
}

pub fn layout(cr: &Context, th: &Theme, s: &str, t: &Text, max_w: Option<f64>) -> pango::Layout {
    let l = pangocairo::functions::create_layout(cr);
    let mut fd = pango::FontDescription::new();
    fd.set_family(if t.mono { &th.mono } else { &th.font });
    fd.set_weight(t.weight);
    fd.set_absolute_size(t.size * pango::SCALE as f64);
    l.set_font_description(Some(&fd));
    l.set_text(s);
    if let Some(w) = max_w {
        l.set_width((w.max(1.0) * pango::SCALE as f64) as i32);
        l.set_ellipsize(pango::EllipsizeMode::End);
        if t.wrap {
            l.set_wrap(pango::WrapMode::WordChar);
            l.set_height(-t.max_lines.max(1));
        }
    }
    l.set_alignment(match t.align {
        Align::Start => pango::Alignment::Left,
        Align::Center => pango::Alignment::Center,
        Align::End => pango::Alignment::Right,
    });
    l
}

/// Measures text in logical pixels (ink-independent logical extents).
pub fn measure(cr: &Context, th: &Theme, s: &str, t: &Text) -> (f64, f64) {
    let l = layout(cr, th, s, t, None);
    let (w, h) = l.pixel_size();
    (w as f64, h as f64)
}

/// Draws text inside `r`, vertically centred, clipped/ellipsized to the rect.
/// Returns the drawn text's bounds.
pub fn text(cr: &Context, th: &Theme, r: Rect, s: &str, t: &Text) -> Rect {
    if r.w <= 0.0 || r.h <= 0.0 || s.is_empty() {
        return Rect::new(r.x, r.y, 0.0, 0.0);
    }
    let l = layout(cr, th, s, t, Some(r.w));
    let (_ink, logical) = l.pixel_extents();
    let h = logical.height() as f64;
    let w = (logical.width() as f64).min(r.w);
    let y = if t.wrap { r.y } else { r.y + ((r.h - h) / 2.0).round() };
    let x = match t.align {
        Align::Start => r.x,
        Align::Center => r.x + (r.w - w) / 2.0,
        Align::End => r.right() - w,
    };
    cr.move_to(r.x, y);
    set(cr, t.color);
    pangocairo::functions::show_layout(cr, &l);
    Rect::new(x, y, w, h)
}

/// Spring/tween parameters per motion preset: (stiffness, damping).
pub fn spring_params(m: Motion) -> Option<(f64, f64)> {
    match m {
        Motion::Expressive => Some((340.0, 22.0)),
        Motion::Smooth => Some((520.0, 46.0)),
        Motion::Minimal => Some((900.0, 60.0)),
        Motion::Off => None,
    }
}
