//! Wallpaper: an image file (cover-scaled) or one of Creed's procedural wallpapers
//! (`creed://<name>`), which are drawn at native resolution and cost no disk space.

use crate::gfx::{self, Rect};
use crate::ui::Ui;
use cairo::{Context, LinearGradient, RadialGradient};
use creed_core::palette::Seed;
use creed_core::{Color, Paths};

pub const PROCEDURAL: &[(&str, &str)] = &[
    ("dawn", "Creed Dawn - warm drifting light"),
    ("dusk", "Creed Dusk - violet evening haze"),
    ("bloom", "Bloom - soft folded petals"),
    ("ribbon", "Ribbon - beams of light in blue"),
    ("dunes", "Dunes - layered desert hills"),
    ("horizon", "Horizon - liquid colour waves"),
    ("tahoe", "Tahoe - blue wave under a warm sky"),
    ("aurora", "Aurora - colourful curves"),
    ("aqua", "Aqua - glossy blue swirl"),
];

pub fn procedural_seed(name: &str) -> Option<Seed> {
    let s = |hue: f64, alt: f64| Seed { hue, alt_hue: alt, tint: 1.0, accent_chroma: 0.12, fixed_accent: None };
    Some(match name {
        "dawn" => s(45.0, 15.0),
        "dusk" => s(300.0, 330.0),
        "bloom" => s(250.0, 280.0),
        "ribbon" => s(240.0, 220.0),
        "dunes" => s(55.0, 30.0),
        "horizon" => s(260.0, 320.0),
        "tahoe" => s(255.0, 70.0),
        "aurora" => s(290.0, 200.0),
        "aqua" => s(245.0, 220.0),
        _ => return None,
    })
}

fn c(hex: &str) -> Color {
    Color::parse(hex).unwrap_or(Color::BLACK)
}

fn blob(cr: &Context, x: f64, y: f64, r: f64, col: Color) {
    let g = RadialGradient::new(x, y, 0.0, x, y, r);
    g.add_color_stop_rgba(0.0, col.r, col.g, col.b, col.a);
    g.add_color_stop_rgba(0.55, col.r, col.g, col.b, col.a * 0.45);
    g.add_color_stop_rgba(1.0, col.r, col.g, col.b, 0.0);
    let _ = cr.set_source(&g);
    let _ = cr.paint();
}

fn vgrad(cr: &Context, w: f64, h: f64, stops: &[(f64, Color)]) {
    let g = LinearGradient::new(0.0, 0.0, w * 0.15, h);
    for (o, col) in stops {
        g.add_color_stop_rgba(*o, col.r, col.g, col.b, col.a);
    }
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.set_source(&g);
    let _ = cr.fill();
}

/// Smooth wave band between two sine curves.
fn wave(cr: &Context, w: f64, h: f64, y0: f64, amp: f64, freq: f64, phase: f64, col_a: Color, col_b: Color) {
    cr.new_path();
    let steps = 96;
    cr.move_to(0.0, h);
    for i in 0..=steps {
        let x = w * i as f64 / steps as f64;
        let y = y0 + amp * (x / w * std::f64::consts::TAU * freq + phase).sin() + amp * 0.35 * (x / w * std::f64::consts::TAU * freq * 2.3 + phase * 1.7).sin();
        cr.line_to(x, y);
    }
    cr.line_to(w, h);
    cr.close_path();
    let g = LinearGradient::new(0.0, y0 - amp, 0.0, h);
    g.add_color_stop_rgba(0.0, col_a.r, col_a.g, col_a.b, col_a.a);
    g.add_color_stop_rgba(1.0, col_b.r, col_b.g, col_b.b, col_b.a);
    let _ = cr.set_source(&g);
    let _ = cr.fill();
}

/// Fine film grain so smooth gradients don't band on 8-bit panels.
fn grain(cr: &Context, strength: f64) {
    let Ok(mut tile) = cairo::ImageSurface::create(cairo::Format::ARgb32, 96, 96) else { return };
    let stride = tile.stride() as usize;
    if let Ok(mut d) = tile.data() {
        let mut seed: u32 = 0x2545f491;
        for y in 0..96 {
            for x in 0..96 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let v = (seed >> 24) as u8;
                let o = y * stride + x * 4;
                // premultiplied grey with alpha = v
                let g = ((v as u32 * v as u32) / 255) as u8;
                d[o] = g;
                d[o + 1] = g;
                d[o + 2] = g;
                d[o + 3] = v;
            }
        }
    }
    let pat = cairo::SurfacePattern::create(&tile);
    pat.set_extend(cairo::Extend::Repeat);
    let _ = cr.set_source(&pat);
    let _ = cr.paint_with_alpha(strength);
}

/// Folded-fabric flower in the spirit of Windows 11's Bloom: two rings of curved petals, each
/// shaded from a deep base to a bright rim, layered back to front on a navy (or pale) ground.
fn draw_bloom(cr: &Context, w: f64, h: f64, dark: bool) {
    let m = w.min(h * 1.6);
    if dark {
        vgrad(cr, w, h, &[(0.0, c("#060d1f")), (0.55, c("#0a1630")), (1.0, c("#040915"))]);
        blob(cr, w * 0.5, h * 0.55, m * 0.55, c("#0d2a6a88"));
    } else {
        vgrad(cr, w, h, &[(0.0, c("#c9dcf2")), (1.0, c("#9fbedf"))]);
        blob(cr, w * 0.5, h * 0.5, m * 0.6, c("#e8f1fbaa"));
    }
    let (cx, cy) = (w * 0.5, h * 0.6);
    // (count, length, width, rotation offset, base colour, rim colour)
    let rings: [(usize, f64, f64, f64, &str, &str); 3] = [
        (9, 0.44, 0.20, 0.10, "#0a1f8c", "#2f6bff"),
        (8, 0.36, 0.19, 0.42, "#0b2fb8", "#4d8dff"),
        (7, 0.26, 0.17, 0.05, "#1442d6", "#79b4ff"),
    ];
    for (n, len, wid, off, base, rim) in rings {
        for k in 0..n {
            let a = off + k as f64 / n as f64 * std::f64::consts::TAU;
            let (l, wd) = (m * len, m * wid);
            cr.save().ok();
            cr.translate(cx, cy);
            cr.scale(1.0, 0.78);
            cr.rotate(a);
            // Petal: a cupped lobe whose outer edge curls (two offset curves).
            let petal = |cr: &Context, grow: f64| {
                cr.new_path();
                cr.move_to(0.0, 0.0);
                cr.curve_to(l * 0.25, -wd * 0.9 * grow, l * 0.85, -wd * 0.75 * grow, l * grow, -wd * 0.05);
                cr.curve_to(l * 0.9, wd * 0.55 * grow, l * 0.35, wd * 0.45 * grow, 0.0, 0.0);
                cr.close_path();
            };
            // Soft shadow beneath for depth.
            petal(cr, 1.04);
            cr.set_source_rgba(0.0, 0.02, 0.1, if dark { 0.45 } else { 0.10 });
            let _ = cr.fill();
            petal(cr, 1.0);
            let g = LinearGradient::new(0.0, 0.0, l, -wd * 0.3);
            let (b, r) = (c(base), c(rim));
            g.add_color_stop_rgba(0.0, b.r * 0.6, b.g * 0.6, b.b * 0.8, 1.0);
            g.add_color_stop_rgba(0.55, b.r, b.g, b.b, 1.0);
            g.add_color_stop_rgba(1.0, r.r, r.g, r.b, 1.0);
            let _ = cr.set_source(&g);
            let _ = cr.fill_preserve();
            // Folded highlight along the upper edge.
            cr.new_path();
            cr.move_to(l * 0.2, -wd * 0.55);
            cr.curve_to(l * 0.55, -wd * 0.78, l * 0.85, -wd * 0.62, l * 0.98, -wd * 0.08);
            cr.set_line_width(m * 0.006);
            cr.set_source_rgba(0.75, 0.88, 1.0, 0.55);
            let _ = cr.stroke();
            cr.restore().ok();
        }
    }
    blob(cr, cx, cy - m * 0.02, m * 0.09, c("#9cc8ff66"));
}

/// macOS 26-style wave: a pale sky (blue fading to warm cream) cut by a sweeping S-curve of deep
/// blue glass with a bright, glossy edge line; a cool night variant for dark mode.
fn draw_tahoe(cr: &Context, w: f64, h: f64, dark: bool) {
    let sky: [(f64, &str); 3] = if dark { [(0.0, "#0b1a3a"), (0.5, "#1b2a52"), (1.0, "#2a3c66")] } else { [(0.0, "#7fa9dd"), (0.42, "#c9cbd3"), (0.72, "#e7dcae")] };
    let g = LinearGradient::new(0.0, 0.0, w * 0.1, h * 0.8);
    for (o, col) in sky {
        let k = c(col);
        g.add_color_stop_rgba(o, k.r, k.g, k.b, 1.0);
    }
    cr.rectangle(0.0, 0.0, w, h);
    let _ = cr.set_source(&g);
    let _ = cr.fill();
    // Distant soft hills on the left.
    blob(cr, w * 0.08, h * 0.5, w * 0.22, if dark { c("#1e6fb5aa") } else { c("#22a6d9cc") });
    blob(cr, w * 0.22, h * 0.62, w * 0.25, if dark { c("#1c4fa0aa") } else { c("#1f6fe0bb") });
    // The wave: everything right of / below the S-curve is deep blue.
    let curve = |cr: &Context| {
        cr.move_to(-w * 0.02, h * 0.92);
        cr.curve_to(w * 0.28, h * 0.78, w * 0.42, h * 0.38, w * 0.62, h * 0.18);
        cr.curve_to(w * 0.7, h * 0.1, w * 0.78, h * 0.02, w * 0.84, -h * 0.02);
    };
    cr.new_path();
    curve(cr);
    cr.line_to(w * 1.02, -h * 0.02);
    cr.line_to(w * 1.02, h * 1.02);
    cr.line_to(-w * 0.02, h * 1.02);
    cr.close_path();
    let body = LinearGradient::new(w * 0.45, h * 0.2, w * 0.8, h);
    let stops: [(f64, &str); 4] = if dark {
        [(0.0, "#2a63c9"), (0.3, "#173f9e"), (0.7, "#0b2470"), (1.0, "#06143f")]
    } else {
        [(0.0, "#62b7ef"), (0.25, "#2a7ae8"), (0.65, "#0b4bd0"), (1.0, "#0835a8")]
    };
    for (o, col) in stops {
        let k = c(col);
        body.add_color_stop_rgba(o, k.r, k.g, k.b, 1.0);
    }
    let _ = cr.set_source(&body);
    let _ = cr.fill();
    // Inner swell and a pale mass at the bottom right, like light inside thick glass.
    blob(cr, w * 0.62, h * 0.48, w * 0.28, if dark { c("#2f6fd055") } else { c("#6cb6ff66") });
    blob(cr, w * 0.95, h * 0.95, w * 0.32, if dark { c("#3a4f8066") } else { c("#cfe0f5aa") });
    // Glossy edge: a wide soft glow plus a crisp white line along the curve.
    for (lw, a) in [(w * 0.018, 0.18), (w * 0.006, 0.45), (w * 0.0016, 0.95)] {
        cr.new_path();
        curve(cr);
        cr.set_line_width(lw);
        cr.set_line_cap(cairo::LineCap::Round);
        cr.set_source_rgba(1.0, 1.0, 1.0, if dark { a * 0.6 } else { a });
        let _ = cr.stroke();
    }
}

pub fn draw_procedural(cr: &Context, name: &str, w: f64, h: f64, dark: bool) {
    draw_procedural_inner(cr, name, w, h, dark);
    grain(cr, if dark { 0.035 } else { 0.03 });
}

fn draw_procedural_inner(cr: &Context, name: &str, w: f64, h: f64, dark: bool) {
    let m = w.max(h);
    // Photo-backed names fall back to their closest drawn wallpaper when no photo is downloaded.
    let name = match name {
        "bigsur" => "aurora",
        "sonoma" | "ventura" | "monterey" => "dunes",
        n => n,
    };
    match name {
        "dusk" => {
            vgrad(cr, w, h, &[(0.0, c("#1b1530")), (0.6, c("#3a2350")), (1.0, c("#5b2d4a"))]);
            blob(cr, w * 0.75, h * 0.3, m * 0.5, c("#b0609acc"));
            blob(cr, w * 0.2, h * 0.85, m * 0.45, c("#6a4bc0aa"));
            blob(cr, w * 0.5, h * 1.05, m * 0.4, c("#f08a6a88"));
        }
        "bloom" => draw_bloom(cr, w, h, dark),
        "tahoe" => draw_tahoe(cr, w, h, dark),
        "ribbon" => {
            vgrad(cr, w, h, &[(0.0, c("#03122b")), (1.0, c("#06244f"))]);
            for (i, alpha) in [(0, 0.55), (1, 0.4), (2, 0.28)] {
                cr.save().ok();
                cr.translate(w * (0.18 + 0.12 * i as f64), h * 0.5);
                cr.rotate(-0.55);
                let g = LinearGradient::new(0.0, -m * 0.06, 0.0, m * 0.06);
                g.add_color_stop_rgba(0.0, 0.2, 0.55, 1.0, 0.0);
                g.add_color_stop_rgba(0.5, 0.45, 0.75, 1.0, alpha);
                g.add_color_stop_rgba(1.0, 0.2, 0.55, 1.0, 0.0);
                cr.rectangle(-m, -m * 0.06, m * 3.0, m * 0.12);
                let _ = cr.set_source(&g);
                let _ = cr.fill();
                cr.restore().ok();
            }
            blob(cr, w * 0.3, h * 0.45, m * 0.25, c("#7fb6ff55"));
        }
        "dunes" => {
            let stops: Vec<(f64, Color)> = if dark { [(0.0, c("#1d2033")), (1.0, c("#3a2f3f"))].to_vec() } else { [(0.0, c("#f6d9b8")), (1.0, c("#e9b48a"))].to_vec() };
            vgrad(cr, w, h, &stops);
            let layers: [(f64, &str, &str); 4] = if dark {
                [(0.52, "#4a3a52", "#2d2538"), (0.62, "#6b4a52", "#3b2c38"), (0.72, "#8a5a4e", "#4a3236"), (0.84, "#a8694c", "#55372f")]
            } else {
                [(0.5, "#f0c49a", "#e7a983"), (0.6, "#e7a678", "#d98d63"), (0.71, "#d88a5d", "#c4704b"), (0.83, "#c46d45", "#a85436")]
            };
            for (i, (y, a, b)) in layers.iter().enumerate() {
                wave(cr, w, h, h * y, h * 0.035, 0.8 + i as f64 * 0.3, i as f64 * 1.3, c(a), c(b));
            }
        }
        "horizon" => {
            let stops: Vec<(f64, Color)> = if dark { [(0.0, c("#0d0f2b")), (1.0, c("#1d1446"))].to_vec() } else { [(0.0, c("#e9ecff")), (1.0, c("#d6d9ff"))].to_vec() };
            vgrad(cr, w, h, &stops);
            wave(cr, w, h, h * 0.48, h * 0.06, 0.7, 0.3, c("#5b6cff99"), c("#3a2c9e00"));
            wave(cr, w, h, h * 0.58, h * 0.05, 0.9, 2.1, c("#c06cff88"), c("#6a2ca000"));
            wave(cr, w, h, h * 0.68, h * 0.045, 1.1, 4.0, c("#ff8ab488"), c("#a02c6a00"));
            blob(cr, w * 0.7, h * 0.2, m * 0.35, c("#8fa2ff44"));
        }
        "aurora" => {
            vgrad(cr, w, h, &[(0.0, c("#0f1e3f")), (1.0, c("#2a1a4a"))]);
            wave(cr, w, h, h * 0.42, h * 0.08, 0.6, 0.0, c("#ff6a8acc"), c("#ff6a8a00"));
            wave(cr, w, h, h * 0.52, h * 0.07, 0.7, 1.4, c("#ffb35ccc"), c("#ffb35c00"));
            wave(cr, w, h, h * 0.62, h * 0.06, 0.8, 2.8, c("#4fc3ffcc"), c("#4fc3ff00"));
            wave(cr, w, h, h * 0.74, h * 0.05, 0.9, 4.2, c("#6d5cffcc"), c("#2a1a4a"));
        }
        "aqua" => {
            vgrad(cr, w, h, &[(0.0, c("#1e5fb8")), (1.0, c("#0b2c66"))]);
            for (i, (x, y, r, col)) in [(0.3, 0.4, 0.5, "#5aa9ffaa"), (0.7, 0.6, 0.45, "#7fd0ffaa"), (0.5, 0.9, 0.5, "#2c7fe6aa")].iter().enumerate() {
                cr.save().ok();
                cr.translate(w * x, h * y);
                cr.rotate(0.6 * i as f64);
                cr.scale(1.0, 0.35);
                blob(cr, 0.0, 0.0, m * r, c(col));
                cr.restore().ok();
            }
        }
        _ => {
            // dawn (default)
            let stops: Vec<(f64, Color)> = if dark { [(0.0, c("#2a1d1a")), (1.0, c("#3a2620"))].to_vec() } else { [(0.0, c("#fbefe3")), (1.0, c("#f4dcc6"))].to_vec() };
            vgrad(cr, w, h, &stops);
            blob(cr, w * 0.80, h * 0.24, m * 0.42, if dark { c("#e8905acc") } else { c("#f2a46ecc") });
            blob(cr, w * 0.18, h * 0.78, m * 0.40, if dark { c("#c25a6cb0") } else { c("#ee8f8fb0") });
            blob(cr, w * 0.58, h * 1.02, m * 0.32, if dark { c("#f0b860a0") } else { c("#f6cf7aaa") });
            blob(cr, w * 0.08, h * 0.06, m * 0.24, if dark { c("#7a5aa080") } else { c("#d9a3c088") });
            blob(cr, w * 0.45, h * 0.45, m * 0.18, if dark { c("#f3c9a033") } else { c("#fff3e066") });
        }
    }
}

/// Renders the current wallpaper to a PNG (used for the lock screen).
pub fn export_png(th: &crate::gfx::Theme, paths: &Paths, w: u32, h: u32, out: &std::path::Path) -> Result<(), String> {
    let surf = cairo::ImageSurface::create(cairo::Format::Rgb24, w as i32, h as i32).map_err(|e| e.to_string())?;
    {
        let cr = Context::new(&surf).map_err(|e| e.to_string())?;
        let wp = th.cfg.appearance.wallpaper.clone();
        let name = if wp.is_empty() { Some(default_for(th).to_string()) } else { wp.strip_prefix("creed://").map(str::to_string) };
        let photo = name.as_deref().and_then(|n| paths.wallpaper_photo(n, th.dark)).and_then(|p| load_cover(&p, w, h));
        match (name, photo) {
            (_, Some(img)) => {
                let _ = cr.set_source_surface(&img, 0.0, 0.0);
                let _ = cr.paint();
            }
            (Some(n), None) => draw_procedural(&cr, &n, w as f64, h as f64, th.dark),
            (None, None) => match load_cover(&paths.expand(&wp), w, h) {
                Some(img) => {
                    let _ = cr.set_source_surface(&img, 0.0, 0.0);
                    let _ = cr.paint();
                }
                None => draw_procedural(&cr, default_for(th), w as f64, h as f64, th.dark),
            },
        }
    }
    let tmp = out.with_extension("tmp.png");
    let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    surf.write_to_png(&mut f).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, out).map_err(|e| e.to_string())?;
    drop(surf);
    crate::trim_memory();
    Ok(())
}

fn default_for(th: &crate::gfx::Theme) -> &'static str {
    match th.cfg.style.bar.as_str() {
        "win11" => "bloom",
        "win10" => "ribbon",
        "aqua" => "aqua",
        "mac" => match th.cfg.style.material {
            creed_core::config::Material::Glass => "tahoe",
            _ => "dunes",
        },
        _ => "dawn",
    }
}

pub fn draw(ui: &mut Ui, paths: &Paths) {
    let (w, h) = ui.size;
    let cr = ui.cr;
    let wp = ui.th.cfg.appearance.wallpaper.clone();
    let dark = ui.th.dark;
    let default_name = match ui.th.cfg.style.bar.as_str() {
        "win11" => "bloom",
        "win10" => "ribbon",
        "aqua" => "aqua",
        "mac" => match ui.th.cfg.style.material {
            creed_core::config::Material::Glass => "tahoe",
            _ => "dunes",
        },
        _ => "dawn",
    };
    let name = if wp.is_empty() { Some(default_name.to_string()) } else { wp.strip_prefix("creed://").map(str::to_string) };
    let photo = name.as_deref().and_then(|n| paths.wallpaper_photo(n, dark));
    if let (Some(n), None) = (&name, &photo) {
        draw_procedural(cr, n, w, h, dark);
        return;
    }
    let path = photo.unwrap_or_else(|| paths.expand(&wp));
    let scale = ui.icons.scale;
    let (pw, ph) = ((w * scale).round() as u32, (h * scale).round() as u32);
    match load_cover(&path, pw, ph) {
        Some(surf) => {
            cr.save().ok();
            cr.scale(1.0 / scale, 1.0 / scale);
            let _ = cr.set_source_surface(&surf, 0.0, 0.0);
            let _ = cr.paint();
            cr.restore().ok();
        }
        None => {
            draw_procedural(cr, default_name, w, h, dark);
            gfx::text(
                cr,
                ui.th,
                Rect::new(16.0, h - 40.0, w - 32.0, 24.0),
                &format!("Wallpaper not found: {}", path.display()),
                &gfx::Text::new(ui.th.fs * 0.9, Color::rgba(1.0, 1.0, 1.0, 0.7)),
            );
        }
    }
    crate::trim_memory();
}

/// Decodes and cover-scales an image to exactly (w, h) pixels.
fn load_cover(path: &std::path::Path, w: u32, h: u32) -> Option<cairo::ImageSurface> {
    let img = image::open(path).ok()?;
    let (iw, ih) = (img.width() as f64, img.height() as f64);
    let s = (w as f64 / iw).max(h as f64 / ih);
    let (sw, sh) = ((iw * s).ceil() as u32, (ih * s).ceil() as u32);
    let scaled = img.resize_exact(sw.max(1), sh.max(1), image::imageops::FilterType::Triangle).to_rgb8();
    drop(img);
    let (ox, oy) = ((sw.saturating_sub(w)) / 2, (sh.saturating_sub(h)) / 2);
    let mut surf = cairo::ImageSurface::create(cairo::Format::Rgb24, w as i32, h as i32).ok()?;
    let stride = surf.stride() as usize;
    {
        let mut d = surf.data().ok()?;
        for y in 0..h.min(sh) {
            for x in 0..w.min(sw) {
                let p = scaled.get_pixel(x + ox, y + oy);
                let o = y as usize * stride + x as usize * 4;
                d[o] = p[2];
                d[o + 1] = p[1];
                d[o + 2] = p[0];
                d[o + 3] = 255;
            }
        }
    }
    Some(surf)
}
