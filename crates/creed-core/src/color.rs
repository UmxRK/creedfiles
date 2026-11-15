//! sRGB colours with OKLab/OKLCH helpers used by the palette generator.

use serde::{de, Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f64,
    pub g: f64,
    pub b: f64,
    pub a: f64,
}

impl Color {
    pub const fn rgba(r: f64, g: f64, b: f64, a: f64) -> Self {
        Self { r, g, b, a }
    }

    pub const TRANSPARENT: Color = Color::rgba(0.0, 0.0, 0.0, 0.0);
    pub const WHITE: Color = Color::rgba(1.0, 1.0, 1.0, 1.0);
    pub const BLACK: Color = Color::rgba(0.0, 0.0, 0.0, 1.0);

    /// Parses `#rgb`, `#rrggbb`, `#rrggbbaa` (leading `#` optional) and `rgba(r,g,b,a)`.
    pub fn parse(s: &str) -> Option<Color> {
        let s = s.trim();
        if let Some(inner) = s.strip_prefix("rgba(").and_then(|x| x.strip_suffix(')')) {
            let p: Vec<f64> = inner.split(',').filter_map(|v| v.trim().parse().ok()).collect();
            if p.len() != 4 {
                return None;
            }
            return Some(Color::rgba(p[0] / 255.0, p[1] / 255.0, p[2] / 255.0, p[3].clamp(0.0, 1.0)));
        }
        let h = s.strip_prefix('#').unwrap_or(s);
        if !h.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|v| v as f64 / 255.0);
        match h.len() {
            3 => {
                let d = |i: usize| u8::from_str_radix(&h[i..i + 1], 16).ok().map(|v| (v * 17) as f64 / 255.0);
                Some(Color::rgba(d(0)?, d(1)?, d(2)?, 1.0))
            }
            6 => Some(Color::rgba(byte(0)?, byte(2)?, byte(4)?, 1.0)),
            8 => Some(Color::rgba(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
            _ => None,
        }
    }

    fn to_u8(v: f64) -> u8 {
        (v.clamp(0.0, 1.0) * 255.0).round() as u8
    }

    /// `#rrggbb` when opaque, `#rrggbbaa` otherwise.
    pub fn to_hex(&self) -> String {
        if self.a >= 0.999 {
            format!("#{:02x}{:02x}{:02x}", Self::to_u8(self.r), Self::to_u8(self.g), Self::to_u8(self.b))
        } else {
            format!(
                "#{:02x}{:02x}{:02x}{:02x}",
                Self::to_u8(self.r),
                Self::to_u8(self.g),
                Self::to_u8(self.b),
                Self::to_u8(self.a)
            )
        }
    }

    /// `rrggbb` without `#` (for foot/kitty style configs).
    pub fn to_hex_bare(&self) -> String {
        format!("{:02x}{:02x}{:02x}", Self::to_u8(self.r), Self::to_u8(self.g), Self::to_u8(self.b))
    }

    /// `rgba(rrggbbaa)` as Hyprland expects.
    pub fn to_hypr(&self) -> String {
        format!(
            "rgba({:02x}{:02x}{:02x}{:02x})",
            Self::to_u8(self.r),
            Self::to_u8(self.g),
            Self::to_u8(self.b),
            Self::to_u8(self.a)
        )
    }

    /// `rgba(r, g, b, a)` CSS form.
    pub fn to_css(&self) -> String {
        format!(
            "rgba({}, {}, {}, {:.3})",
            Self::to_u8(self.r),
            Self::to_u8(self.g),
            Self::to_u8(self.b),
            self.a.clamp(0.0, 1.0)
        )
    }

    pub fn with_alpha(self, a: f64) -> Color {
        Color { a, ..self }
    }

    pub fn mix(self, other: Color, t: f64) -> Color {
        let t = t.clamp(0.0, 1.0);
        Color::rgba(
            self.r + (other.r - self.r) * t,
            self.g + (other.g - self.g) * t,
            self.b + (other.b - self.b) * t,
            self.a + (other.a - self.a) * t,
        )
    }

    /// WCAG relative luminance.
    pub fn luminance(&self) -> f64 {
        let lin = |c: f64| if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) };
        0.2126 * lin(self.r) + 0.7152 * lin(self.g) + 0.0722 * lin(self.b)
    }

    pub fn contrast(&self, other: &Color) -> f64 {
        let (a, b) = (self.luminance(), other.luminance());
        let (hi, lo) = if a > b { (a, b) } else { (b, a) };
        (hi + 0.05) / (lo + 0.05)
    }

    pub fn to_oklch(&self) -> (f64, f64, f64) {
        let (l, a, b) = srgb_to_oklab(self.r, self.g, self.b);
        let c = (a * a + b * b).sqrt();
        let mut h = b.atan2(a).to_degrees();
        if h < 0.0 {
            h += 360.0;
        }
        (l, c, h)
    }

    /// Builds a colour from OKLCH, reducing chroma until it fits in sRGB.
    pub fn from_oklch(l: f64, c: f64, h: f64) -> Color {
        let mut c = c.max(0.0);
        for _ in 0..40 {
            let hr = h.to_radians();
            let (r, g, b) = oklab_to_srgb(l, c * hr.cos(), c * hr.sin());
            let eps = 1e-4;
            if (-eps..=1.0 + eps).contains(&r) && (-eps..=1.0 + eps).contains(&g) && (-eps..=1.0 + eps).contains(&b) {
                return Color::rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0);
            }
            c *= 0.9;
        }
        let (r, g, b) = oklab_to_srgb(l, 0.0, 0.0);
        Color::rgba(r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0), 1.0)
    }
}

fn to_linear(c: f64) -> f64 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

pub fn srgb_to_oklab(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let (r, g, b) = (to_linear(r), to_linear(g), to_linear(b));
    let l = (0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b).cbrt();
    let m = (0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b).cbrt();
    let s = (0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b).cbrt();
    (
        0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s,
        1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s,
        0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s,
    )
}

pub fn oklab_to_srgb(l: f64, a: f64, b: f64) -> (f64, f64, f64) {
    let l_ = l + 0.3963377774 * a + 0.2158037573 * b;
    let m_ = l - 0.1055613458 * a - 0.0638541728 * b;
    let s_ = l - 0.0894841775 * a - 1.2914855480 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    (
        to_srgb(4.0767416621 * l3 - 3.3077115913 * m3 + 0.2309699292 * s3),
        to_srgb(-1.2684380046 * l3 + 2.6097574011 * m3 - 0.3413193965 * s3),
        to_srgb(-0.0041960863 * l3 - 0.7034186147 * m3 + 1.7076147010 * s3),
    )
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Serialize for Color {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Color {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        Color::parse(&s).ok_or_else(|| de::Error::custom(format!("invalid colour '{s}' (use #rrggbb or #rrggbbaa)")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_format_roundtrip() {
        assert_eq!(Color::parse("#ff8800").unwrap().to_hex(), "#ff8800");
        assert_eq!(Color::parse("ff880080").unwrap().to_hex(), "#ff880080");
        assert_eq!(Color::parse("#f80").unwrap().to_hex(), "#ff8800");
        assert_eq!(Color::parse("rgba(255,136,0,0.5)").unwrap().to_hex(), "#ff880080");
        assert!(Color::parse("#zzzzzz").is_none());
        assert!(Color::parse("#12345").is_none());
        assert_eq!(Color::parse("#11223344").unwrap().to_hypr(), "rgba(11223344)");
    }

    #[test]
    fn oklch_roundtrip() {
        for hex in ["#ff8800", "#3366cc", "#7a7a7a", "#e8d5c4"] {
            let c = Color::parse(hex).unwrap();
            let (l, ch, h) = c.to_oklch();
            assert_eq!(Color::from_oklch(l, ch, h).to_hex(), hex);
        }
    }

    #[test]
    fn contrast_black_white() {
        assert!((Color::BLACK.contrast(&Color::WHITE) - 21.0).abs() < 0.01);
    }
}
