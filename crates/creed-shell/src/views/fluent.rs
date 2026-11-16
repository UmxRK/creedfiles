//! Fluent design tokens and primitives shared by the Windows 10 / 11 views.
//! Colour values are Microsoft's WinUI theme resources (Common_themeresources_any.xaml,
//! AcrylicBrush_themeresources.xaml); radii: OverlayCornerRadius 8, ControlCornerRadius 4.
//! Type ramp (Segoe UI Variable): Caption 12, Body 14, Subtitle 20, Title 28.

use crate::gfx::{self, Rect, Text, Theme};
use creed_core::Color;

pub const OVERLAY_RADIUS: f64 = 8.0;
pub const CONTROL_RADIUS: f64 = 4.0;
pub const CAPTION: f64 = 12.0;
pub const BODY: f64 = 14.0;
pub const SUBTITLE: f64 = 20.0;

fn c(h: &str) -> Color {
    Color::parse(h).unwrap_or(Color::BLACK)
}

/// Theme-dependent Fluent tokens.
pub struct Tok {
    pub dark: bool,
    pub text1: Color,
    pub text2: Color,
    pub text3: Color,
    pub text_disabled: Color,
    pub ctl_fill: Color,
    pub ctl_fill2: Color,
    pub ctl_fill3: Color,
    pub subtle2: Color,
    pub subtle3: Color,
    pub card: Color,
    pub card2: Color,
    pub ctl_stroke: Color,
    pub card_stroke: Color,
    pub divider: Color,
    pub flyout_stroke: Color,
    pub acrylic: Color,
    pub mica: Color,
    pub solid_base: Color,
    pub solid_secondary: Color,
    pub layer: Color,
    pub accent: Color,
    pub on_accent: Color,
}

pub fn tok(th: &Theme) -> Tok {
    let d = th.dark;
    Tok {
        dark: d,
        text1: if d { c("#ffffff") } else { c("#000000e4") },
        text2: if d { c("#ffffffc5") } else { c("#0000009e") },
        text3: if d { c("#ffffff87") } else { c("#00000072") },
        text_disabled: if d { c("#ffffff5d") } else { c("#0000005c") },
        ctl_fill: if d { c("#ffffff0f") } else { c("#ffffffb3") },
        ctl_fill2: if d { c("#ffffff15") } else { c("#f9f9f980") },
        ctl_fill3: if d { c("#ffffff08") } else { c("#f9f9f94d") },
        subtle2: if d { c("#ffffff0f") } else { c("#00000009") },
        subtle3: if d { c("#ffffff0a") } else { c("#00000006") },
        card: if d { c("#ffffff0d") } else { c("#ffffffb3") },
        card2: if d { c("#ffffff08") } else { c("#f6f6f680") },
        ctl_stroke: if d { c("#ffffff12") } else { c("#0000000f") },
        card_stroke: if d { c("#00000019") } else { c("#0000000f") },
        divider: if d { c("#ffffff15") } else { c("#0000000f") },
        flyout_stroke: if d { c("#00000033") } else { c("#0000000f") },
        // AcrylicBackgroundFillColorDefault: tint #2C2C2C / luminosity 0.96 (dark),
        // #FCFCFC / 0.85 (light). Compositor blur sits underneath.
        acrylic: if d { c("#2c2c2cf5") } else { c("#fcfcfcd9") },
        // Mica: #202020 @ 0.8 (dark), #F3F3F3 @ 0.5 over luminosity (light).
        mica: if d { c("#202020d9") } else { c("#f3f3f3e0") },
        solid_base: if d { c("#202020") } else { c("#f3f3f3") },
        solid_secondary: if d { c("#1c1c1c") } else { c("#eeeeee") },
        layer: if d { c("#3a3a3a4c") } else { c("#ffffff80") },
        accent: th.pal.accent,
        on_accent: th.pal.on_accent,
    }
}

/// Flyout surface: acrylic fill, 1 px stroke, Fluent shadow.
pub fn flyout(cr: &cairo::Context, t: &Tok, r: Rect, radius: f64) {
    gfx::shadow(cr, r, radius, 18.0, 8.0, if t.dark { 0.42 } else { 0.18 });
    gfx::fill_rounded(cr, r, radius, t.acrylic);
    gfx::stroke_rounded(cr, r, radius, 1.0, if t.dark { c("#ffffff14") } else { c("#0000000f") });
}

pub fn text(size: f64, color: Color) -> Text {
    Text::new(size, color)
}

/// Standard Fluent button (ControlFill + ControlStroke), returns nothing; caller registers hit.
pub fn button(cr: &cairo::Context, t: &Tok, r: Rect, hover: f64, pressed: bool, accent: bool) {
    if accent {
        let base = t.accent;
        let col = if pressed { base.with_alpha(0.8) } else { base.mix(if t.dark { Color::BLACK } else { Color::WHITE }, 0.1 * hover) };
        gfx::fill_rounded(cr, r, CONTROL_RADIUS, col);
        return;
    }
    let fill = if pressed { t.ctl_fill3 } else if hover > 0.5 { t.ctl_fill2 } else { t.ctl_fill };
    gfx::fill_rounded(cr, r, CONTROL_RADIUS, fill);
    gfx::stroke_rounded(cr, r, CONTROL_RADIUS, 1.0, t.ctl_stroke);
}

/// Subtle (transparent) button background used on the taskbar and in lists.
pub fn subtle(cr: &cairo::Context, t: &Tok, r: Rect, radius: f64, hover: f64, pressed: bool) {
    if pressed {
        gfx::fill_rounded(cr, r, radius, t.subtle3);
    } else if hover > 0.01 {
        gfx::fill_rounded(cr, r, radius, t.subtle2.with_alpha(t.subtle2.a * hover));
    }
}

/// Fluent slider: 4 px track, accent fill, 20 px thumb with 12 px accent core.
pub fn slider(cr: &cairo::Context, t: &Tok, r: Rect, value: f64, hover: f64) {
    let v = value.clamp(0.0, 1.0);
    let track = Rect::new(r.x + 10.0, r.cy() - 2.0, r.w - 20.0, 4.0);
    gfx::fill_rounded(cr, track, 2.0, if t.dark { c("#ffffff8b") } else { c("#00000072") });
    gfx::fill_rounded(cr, Rect::new(track.x, track.y, track.w * v, 4.0), 2.0, t.accent);
    let tx = track.x + track.w * v;
    let thumb = Rect::centered(tx, r.cy(), 20.0, 20.0);
    gfx::fill_rounded(cr, thumb, 10.0, if t.dark { c("#454545") } else { c("#ffffff") });
    gfx::stroke_rounded(cr, thumb, 10.0, 1.0, t.ctl_stroke);
    let core = 12.0 + 2.0 * hover;
    gfx::fill_rounded(cr, Rect::centered(tx, r.cy(), core, core), core / 2.0, t.accent);
}
