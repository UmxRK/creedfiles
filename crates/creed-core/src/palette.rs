//! Palette generation: from a wallpaper (dominant hue in OKLCH) or a named preset.
//! All tones are derived in OKLCH so lightness steps look even across hues.

use crate::color::Color;
use crate::config::Scheme;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub scheme: Scheme,
    pub bg: Color,
    pub surface: Color,
    pub surface_alt: Color,
    pub overlay: Color,
    pub border: Color,
    pub text: Color,
    pub subtext: Color,
    pub muted: Color,
    pub accent: Color,
    pub accent_alt: Color,
    pub on_accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub shadow: Color,
    /// ANSI 0–15.
    pub term: [Color; 16],
}

/// A seed fully determines a palette.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seed {
    pub hue: f64,
    pub alt_hue: f64,
    /// 0 = neutral greys (Windows/macOS look), 1 = fully tinted (Creed look).
    pub tint: f64,
    pub accent_chroma: f64,
    /// Fixed accents (e.g. Windows blue) per scheme: (dark, light).
    pub fixed_accent: Option<(Color, Color)>,
}

pub const PRESETS: &[&str] = &["ember", "blossom", "dusk", "frost", "moss", "sand", "mono", "windows11", "windows10", "macos", "aqua"];

pub fn preset_seed(name: &str) -> Option<Seed> {
    let tinted = |hue: f64, alt: f64| Seed { hue, alt_hue: alt, tint: 1.0, accent_chroma: 0.13, fixed_accent: None };
    let fixed = |d: &str, l: &str| Seed {
        hue: 250.0,
        alt_hue: 250.0,
        tint: 0.0,
        accent_chroma: 0.13,
        fixed_accent: Some((Color::parse(d).unwrap(), Color::parse(l).unwrap())),
    };
    Some(match name {
        "ember" => tinted(45.0, 15.0),
        "blossom" => tinted(355.0, 320.0),
        "dusk" => tinted(295.0, 330.0),
        "frost" => tinted(235.0, 200.0),
        "moss" => tinted(140.0, 100.0),
        "sand" => tinted(75.0, 45.0),
        "mono" => Seed { hue: 60.0, alt_hue: 60.0, tint: 0.15, accent_chroma: 0.0, fixed_accent: None },
        "windows11" => fixed("#4cc2ff", "#005fb8"),
        "windows10" => fixed("#3a96dd", "#0078d7"),
        "macos" => fixed("#0a84ff", "#007aff"),
        "aqua" => fixed("#5aa2ec", "#2f7fd8"),
        _ => return None,
    })
}

/// Extracts a seed from an image: the chroma-weighted dominant hue and a secondary hue.
pub fn seed_from_image(path: &Path) -> Result<Seed, String> {
    let img = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let small = img.thumbnail(96, 96).to_rgb8();
    Ok(seed_from_pixels(small.pixels().map(|p| (p[0], p[1], p[2]))))
}

pub fn seed_from_pixels(pixels: impl Iterator<Item = (u8, u8, u8)>) -> Seed {
    const BINS: usize = 36;
    let mut hist = [0.0f64; BINS];
    let mut total = 0.0;
    let mut chroma_sum = 0.0;
    let mut n = 0.0;
    for (r, g, b) in pixels {
        let c = Color::rgba(r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0, 1.0);
        let (l, ch, h) = c.to_oklch();
        n += 1.0;
        chroma_sum += ch;
        if !(0.15..=0.95).contains(&l) || ch < 0.03 {
            continue;
        }
        let w = ch * ch;
        hist[((h / 10.0) as usize).min(BINS - 1)] += w;
        total += w;
    }
    let avg_chroma = if n > 0.0 { chroma_sum / n } else { 0.0 };
    if total <= 0.0 || avg_chroma < 0.015 {
        // Greyscale wallpaper: warm neutral, lightly tinted.
        return Seed { hue: 60.0, alt_hue: 30.0, tint: 0.25, accent_chroma: 0.08, fixed_accent: None };
    }
    // Smooth over neighbours so near-identical hues reinforce each other.
    let smooth: Vec<f64> =
        (0..BINS).map(|i| hist[(i + BINS - 1) % BINS] * 0.5 + hist[i] + hist[(i + 1) % BINS] * 0.5).collect();
    let best = (0..BINS).max_by(|&a, &b| smooth[a].total_cmp(&smooth[b])).unwrap();
    let hue = best as f64 * 10.0 + 5.0;
    let alt = (0..BINS)
        .filter(|&i| {
            let d = (i as f64 * 10.0 + 5.0 - hue).abs();
            d.min(360.0 - d) >= 40.0
        })
        .max_by(|&a, &b| smooth[a].total_cmp(&smooth[b]))
        .filter(|&i| smooth[i] > smooth[best] * 0.15)
        .map(|i| i as f64 * 10.0 + 5.0)
        .unwrap_or((hue + 35.0) % 360.0);
    Seed { hue, alt_hue: alt, tint: 1.0, accent_chroma: (avg_chroma * 1.6).clamp(0.08, 0.15), fixed_accent: None }
}

fn c(h: &str) -> Color {
    Color::parse(h).unwrap_or(Color::BLACK)
}

fn term(list: [&str; 16]) -> [Color; 16] {
    let mut t = [Color::BLACK; 16];
    for (i, h) in list.iter().enumerate() {
        t[i] = c(h);
    }
    t
}

/// Windows Terminal "Campbell" - the Windows default console scheme.
const CAMPBELL: [&str; 16] = ["#0c0c0c", "#c50f1f", "#13a10e", "#c19c00", "#0037da", "#881798", "#3a96dd", "#cccccc", "#767676", "#e74856", "#16c60c", "#f9f1a5", "#3b78ff", "#b4009e", "#61d6d6", "#f2f2f2"];
/// macOS Terminal "Basic" profile ANSI colours.
const MAC_TERMINAL: [&str; 16] = ["#000000", "#990000", "#00a600", "#999900", "#0000b2", "#b200b2", "#00a6b2", "#bfbfbf", "#666666", "#e50000", "#00d900", "#e5e500", "#0000ff", "#e500e5", "#00e5e5", "#e5e5e5"];

/// Exact system palettes for the OS look-alike modes (no generated tones):
/// Windows values come from Microsoft's WinUI theme resources (Common_themeresources_any.xaml),
/// macOS values from Apple's documented NSColor system/label colours.
pub fn system(name: &str, scheme: Scheme) -> Option<Palette> {
    let dark = scheme == Scheme::Dark;
    let p = match (name, dark) {
        ("windows11", true) => Palette {
            scheme, bg: c("#202020"), surface: c("#2c2c2c"), surface_alt: c("#383838"), overlay: c("#ffffff15"),
            border: c("#ffffff12"), text: c("#ffffff"), subtext: c("#ffffffc5"), muted: c("#ffffff87"),
            accent: c("#4cc2ff"), accent_alt: c("#99ebff"), on_accent: c("#000000"),
            success: c("#6ccb5f"), warning: c("#fce100"), error: c("#ff99a4"), shadow: c("#00000066"), term: term(CAMPBELL),
        },
        ("windows11", false) => Palette {
            scheme, bg: c("#f3f3f3"), surface: c("#f9f9f9"), surface_alt: c("#ffffff"), overlay: c("#00000009"),
            border: c("#0000000f"), text: c("#000000e4"), subtext: c("#0000009e"), muted: c("#00000072"),
            accent: c("#005fb8"), accent_alt: c("#003e92"), on_accent: c("#ffffff"),
            success: c("#0f7b0f"), warning: c("#9d5d00"), error: c("#c42b1c"), shadow: c("#00000024"), term: term(CAMPBELL),
        },
        ("windows10", true) => Palette {
            scheme, bg: c("#101010"), surface: c("#1f1f1f"), surface_alt: c("#2b2b2b"), overlay: c("#ffffff1a"),
            border: c("#ffffff1a"), text: c("#ffffff"), subtext: c("#ffffffb3"), muted: c("#ffffff80"),
            accent: c("#0078d7"), accent_alt: c("#429ce3"), on_accent: c("#ffffff"),
            success: c("#16c60c"), warning: c("#ffb900"), error: c("#e81123"), shadow: c("#00000080"), term: term(CAMPBELL),
        },
        ("windows10", false) => Palette {
            scheme, bg: c("#f0f0f0"), surface: c("#e4e4e4"), surface_alt: c("#ffffff"), overlay: c("#0000001a"),
            border: c("#0000001a"), text: c("#000000"), subtext: c("#000000b3"), muted: c("#00000080"),
            accent: c("#0078d7"), accent_alt: c("#005a9e"), on_accent: c("#ffffff"),
            success: c("#107c10"), warning: c("#9d5d00"), error: c("#e81123"), shadow: c("#00000040"), term: term(CAMPBELL),
        },
        ("macos", true) => Palette {
            scheme, bg: c("#1e1e1e"), surface: c("#282828"), surface_alt: c("#323232"), overlay: c("#ffffff1a"),
            border: c("#ffffff1a"), text: c("#ffffffd9"), subtext: c("#ffffff8c"), muted: c("#ffffff40"),
            accent: c("#0a84ff"), accent_alt: c("#409cff"), on_accent: c("#ffffff"),
            success: c("#32d74b"), warning: c("#ffd60a"), error: c("#ff453a"), shadow: c("#00000059"), term: term(MAC_TERMINAL),
        },
        ("macos", false) => Palette {
            scheme, bg: c("#ececec"), surface: c("#f6f6f6"), surface_alt: c("#ffffff"), overlay: c("#0000000d"),
            border: c("#0000001a"), text: c("#000000d9"), subtext: c("#00000080"), muted: c("#00000040"),
            accent: c("#007aff"), accent_alt: c("#0064d1"), on_accent: c("#ffffff"),
            success: c("#34c759"), warning: c("#ffcc00"), error: c("#ff3b30"), shadow: c("#00000033"), term: term(MAC_TERMINAL),
        },
        ("aqua", _) => Palette {
            scheme, bg: c("#e8e8e8"), surface: c("#ededed"), surface_alt: c("#ffffff"), overlay: c("#0000000f"),
            border: c("#00000033"), text: c("#000000"), subtext: c("#000000a6"), muted: c("#00000066"),
            accent: c("#3875d7"), accent_alt: c("#6fa0e8"), on_accent: c("#ffffff"),
            success: c("#28a745"), warning: c("#f5a623"), error: c("#e0443e"), shadow: c("#00000059"), term: term(MAC_TERMINAL),
        },
        _ => return None,
    };
    Some(p)
}

impl Palette {
    pub fn generate(seed: Seed, scheme: Scheme, high_contrast: bool, accent_override: Option<Color>) -> Palette {
        let dark = scheme == Scheme::Dark;
        let h = seed.hue;
        let t = seed.tint.clamp(0.0, 1.0);
        let tone = |l: f64, c: f64| Color::from_oklch(l, c * t, h);
        let (bg, surface, surface_alt, overlay, border, text, subtext, muted) = if dark {
            (
                tone(0.175, 0.018),
                tone(0.215, 0.022),
                tone(0.262, 0.026),
                tone(0.33, 0.03),
                tone(0.36, 0.028),
                tone(if high_contrast { 0.99 } else { 0.945 }, 0.012),
                tone(if high_contrast { 0.9 } else { 0.79 }, 0.018),
                tone(0.62, 0.02),
            )
        } else {
            (
                tone(0.972, 0.008),
                tone(0.948, 0.012),
                tone(0.912, 0.016),
                tone(0.86, 0.02),
                tone(0.84, 0.018),
                tone(if high_contrast { 0.12 } else { 0.24 }, 0.02),
                tone(if high_contrast { 0.28 } else { 0.42 }, 0.022),
                tone(0.58, 0.02),
            )
        };
        let accent = accent_override.unwrap_or_else(|| match seed.fixed_accent {
            Some((d, l)) => {
                if dark {
                    d
                } else {
                    l
                }
            }
            None => Color::from_oklch(if dark { 0.80 } else { 0.56 }, seed.accent_chroma, h),
        });
        let (al, ac, ah) = accent.to_oklch();
        let accent_alt = match seed.fixed_accent {
            Some(_) => Color::from_oklch((al + if dark { -0.08 } else { 0.08 }).clamp(0.0, 1.0), ac, ah),
            None => Color::from_oklch(if dark { 0.76 } else { 0.6 }, seed.accent_chroma * 0.9, seed.alt_hue),
        };
        let on_accent = if accent.contrast(&Color::WHITE) >= accent.contrast(&Color::BLACK) {
            Color::from_oklch(0.985, 0.005, ah)
        } else {
            Color::from_oklch(0.17, 0.02, ah)
        };
        let sem = |hue: f64| Color::from_oklch(if dark { 0.78 } else { 0.55 }, 0.13, hue);
        let term_base = |light: f64, hue: f64| Color::from_oklch(light, 0.12, hue);
        let (n, b) = if dark { (0.72, 0.82) } else { (0.5, 0.58) };
        let hues = [25.0, 145.0, 85.0, 255.0, 330.0, 200.0];
        let mut term = [Color::BLACK; 16];
        term[0] = if dark { surface_alt } else { tone(0.3, 0.02) };
        term[8] = if dark { overlay.mix(muted, 0.5) } else { tone(0.45, 0.02) };
        term[7] = if dark { subtext } else { tone(0.88, 0.01) };
        term[15] = if dark { text } else { tone(0.97, 0.008) };
        for (i, hue) in hues.iter().enumerate() {
            term[1 + i] = term_base(n, *hue);
            term[9 + i] = term_base(b, *hue);
        }
        Palette {
            scheme,
            bg,
            surface,
            surface_alt,
            overlay,
            border,
            text,
            subtext,
            muted,
            accent,
            accent_alt,
            on_accent,
            success: sem(150.0),
            warning: sem(80.0),
            error: sem(25.0),
            shadow: Color::rgba(0.0, 0.0, 0.0, if dark { 0.45 } else { 0.18 }),
            term,
        }
    }

    pub fn named(&self) -> Vec<(&'static str, Color)> {
        vec![
            ("bg", self.bg),
            ("surface", self.surface),
            ("surface_alt", self.surface_alt),
            ("overlay", self.overlay),
            ("border", self.border),
            ("text", self.text),
            ("subtext", self.subtext),
            ("muted", self.muted),
            ("accent", self.accent),
            ("accent_alt", self.accent_alt),
            ("on_accent", self.on_accent),
            ("success", self.success),
            ("warning", self.warning),
            ("error", self.error),
            ("shadow", self.shadow),
        ]
    }

    /// Template variables: `color.<name>` (#hex), `color.<name>.bare`, `.hypr`, `.css`, `.rgb`; `term.<i>`.
    pub fn vars(&self) -> BTreeMap<String, String> {
        let mut m = BTreeMap::new();
        let mut put = |k: String, c: Color| {
            m.insert(format!("{k}"), c.to_hex());
            m.insert(format!("{k}.bare"), c.to_hex_bare());
            m.insert(format!("{k}.hypr"), c.to_hypr());
            m.insert(format!("{k}.css"), c.to_css());
            m.insert(
                format!("{k}.rgb"),
                format!("{},{},{}", (c.r * 255.0).round(), (c.g * 255.0).round(), (c.b * 255.0).round()),
            );
        };
        for (k, c) in self.named() {
            put(format!("color.{k}"), c);
        }
        for (i, c) in self.term.iter().enumerate() {
            put(format!("term.{i}"), *c);
        }
        m.insert("scheme".into(), if self.scheme == Scheme::Dark { "dark".into() } else { "light".into() });
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hue_dist(a: f64, b: f64) -> f64 {
        let d = (a - b).abs() % 360.0;
        d.min(360.0 - d)
    }

    #[test]
    fn dominant_hue_from_pixels() {
        let orange = Color::parse("#e07a2f").unwrap();
        let (_, _, oh) = orange.to_oklch();
        let px = std::iter::repeat((0xe0, 0x7a, 0x2f)).take(900).chain(std::iter::repeat((0x20, 0x40, 0xa0)).take(100));
        let s = seed_from_pixels(px);
        assert!(hue_dist(s.hue, oh) <= 20.0, "hue {} vs {}", s.hue, oh);
        let p = Palette::generate(s, Scheme::Dark, false, None);
        let (_, _, ah) = p.accent.to_oklch();
        assert!(hue_dist(ah, oh) <= 20.0);
    }

    #[test]
    fn greyscale_is_neutralish() {
        let s = seed_from_pixels(std::iter::repeat((128, 128, 128)).take(100));
        assert!(s.tint < 0.5);
    }

    #[test]
    fn system_palettes_exact() {
        let w = system("windows11", Scheme::Dark).unwrap();
        assert_eq!(w.bg.to_hex(), "#202020");
        assert_eq!(w.accent.to_hex(), "#4cc2ff");
        assert_eq!(system("windows11", Scheme::Light).unwrap().accent.to_hex(), "#005fb8");
        assert_eq!(system("macos", Scheme::Dark).unwrap().accent.to_hex(), "#0a84ff");
        assert_eq!(system("windows10", Scheme::Dark).unwrap().term[1].to_hex(), "#c50f1f");
        assert!(system("nope", Scheme::Dark).is_none());
    }

    #[test]
    fn contrast_all_presets_and_schemes() {
        for name in PRESETS {
            for scheme in [Scheme::Dark, Scheme::Light] {
                for hc in [false, true] {
                    let p = Palette::generate(preset_seed(name).unwrap(), scheme, hc, None);
                    assert!(p.text.contrast(&p.bg) >= 7.0, "{name} {scheme:?} text/bg {}", p.text.contrast(&p.bg));
                    assert!(p.text.contrast(&p.surface_alt) >= 4.5, "{name} {scheme:?}");
                    assert!(p.subtext.contrast(&p.surface) >= 4.5, "{name} {scheme:?} subtext {}", p.subtext.contrast(&p.surface));
                    assert!(p.on_accent.contrast(&p.accent) >= 3.0, "{name} {scheme:?} on_accent {}", p.on_accent.contrast(&p.accent));
                }
            }
        }
    }

    #[test]
    fn vars_shape() {
        let p = Palette::generate(preset_seed("ember").unwrap(), Scheme::Dark, false, None);
        let v = p.vars();
        assert!(v["color.accent"].starts_with('#'));
        assert_eq!(v["color.accent.bare"].len(), 6);
        assert!(v["term.15.hypr"].starts_with("rgba("));
    }
}
