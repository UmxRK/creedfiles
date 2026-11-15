//! Freedesktop icon lookup + rasterisation with a small bounded cache.

use crate::gfx::{self, Rect, Theme};
use cairo::{Context, Format, ImageSurface};
use creed_core::Color;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct Dir {
    path: String,
    size: u32,
    min: u32,
    max: u32,
    scalable: bool,
}

#[derive(Clone, Debug, Default)]
struct ThemeIndex {
    roots: Vec<PathBuf>,
    dirs: Vec<Dir>,
    inherits: Vec<String>,
}

pub struct Icons {
    theme: String,
    bases: Vec<PathBuf>,
    themes: HashMap<String, Option<ThemeIndex>>,
    paths: HashMap<(String, u32), Option<PathBuf>>,
    raster: HashMap<(String, u32), Option<ImageSurface>>,
    order: VecDeque<(String, u32)>,
    pub scale: f64,
    cap: usize,
}

impl Icons {
    pub fn new(theme: &str) -> Icons {
        let home = std::env::var("HOME").unwrap_or_default();
        let mut bases = vec![PathBuf::from(&home).join(".local/share/icons"), PathBuf::from(&home).join(".icons")];
        let data_dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
        for d in data_dirs.split(':').filter(|d| !d.is_empty()) {
            bases.push(PathBuf::from(d).join("icons"));
        }
        let mut me = Icons {
            theme: String::new(),
            bases,
            themes: HashMap::new(),
            paths: HashMap::new(),
            raster: HashMap::new(),
            order: VecDeque::new(),
            scale: 1.0,
            cap: 160,
        };
        me.theme = me.resolve_theme(theme);
        me
    }

    fn installed(&self, theme: &str) -> bool {
        self.bases.iter().any(|b| b.join(theme).join("index.theme").is_file())
    }

    /// Uses `theme` if installed, otherwise the closest installed fallback.
    pub fn resolve_theme(&self, theme: &str) -> String {
        if self.installed(theme) {
            return theme.to_string();
        }
        let light = !theme.to_lowercase().contains("dark");
        let candidates: &[&str] = if light { &["Papirus", "Papirus-Light", "Adwaita"] } else { &["Papirus-Dark", "Papirus", "Adwaita"] };
        candidates.iter().find(|c| self.installed(c)).map(|c| c.to_string()).unwrap_or_else(|| "hicolor".into())
    }

    pub fn set_theme(&mut self, theme: &str) {
        let theme = self.resolve_theme(theme);
        if theme != self.theme {
            self.theme = theme;
            self.paths.clear();
            self.clear();
        }
    }

    /// Drops rasterised icons (called when popups close to keep idle memory low).
    pub fn clear(&mut self) {
        self.raster.clear();
        self.order.clear();
    }

    fn index(&mut self, name: &str) -> Option<ThemeIndex> {
        if let Some(t) = self.themes.get(name) {
            return t.clone();
        }
        let roots: Vec<PathBuf> = self.bases.iter().map(|b| b.join(name)).filter(|p| p.is_dir()).collect();
        let idx = roots.iter().find_map(|r| fs::read_to_string(r.join("index.theme")).ok()).map(|text| {
            let mut dirs = vec![];
            let mut inherits = vec![];
            let mut section = String::new();
            let mut cur: Option<Dir> = None;
            let mut listed: Vec<String> = vec![];
            for line in text.lines() {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']') {
                    if let Some(d) = cur.take() {
                        dirs.push(d);
                    }
                    section = line[1..line.len() - 1].to_string();
                    if section != "Icon Theme" && listed.contains(&section) {
                        cur = Some(Dir { path: section.clone(), size: 0, min: 0, max: 0, scalable: false });
                    }
                    continue;
                }
                let Some((k, v)) = line.split_once('=') else { continue };
                let (k, v) = (k.trim(), v.trim());
                if section == "Icon Theme" {
                    match k {
                        "Directories" | "ScaledDirectories" => listed.extend(v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty())),
                        "Inherits" => inherits = v.split(',').map(|s| s.trim().to_string()).collect(),
                        _ => {}
                    }
                } else if let Some(d) = cur.as_mut() {
                    match k {
                        "Size" => d.size = v.parse().unwrap_or(0),
                        "MinSize" => d.min = v.parse().unwrap_or(0),
                        "MaxSize" => d.max = v.parse().unwrap_or(0),
                        "Type" => d.scalable = v == "Scalable",
                        _ => {}
                    }
                }
            }
            if let Some(d) = cur.take() {
                dirs.push(d);
            }
            ThemeIndex { roots: roots.clone(), dirs, inherits }
        });
        self.themes.insert(name.to_string(), idx.clone());
        idx
    }

    fn lookup_in(&mut self, theme: &str, name: &str, size: u32, depth: u32, visited: &mut Vec<String>) -> Option<PathBuf> {
        if depth > 6 || visited.iter().any(|v| v == theme) {
            return None;
        }
        visited.push(theme.to_string());
        let idx = self.index(theme)?;
        // Score dirs: exact/containing size first, then scalable, then nearest.
        let mut best: Option<(u32, PathBuf)> = None;
        for d in &idx.dirs {
            let contains = if d.scalable {
                size >= d.min.max(1) && size <= d.max.max(d.size).max(512)
            } else {
                d.size == size
            };
            let dist = if contains { 0 } else { (d.size as i64 - size as i64).unsigned_abs() as u32 + if d.size < size { 1000 } else { 0 } };
            if best.as_ref().map(|(b, _)| dist >= *b).unwrap_or(false) {
                continue;
            }
            for root in &idx.roots {
                for ext in ["svg", "png"] {
                    let p = root.join(&d.path).join(format!("{name}.{ext}"));
                    if p.is_file() {
                        best = Some((dist, p));
                        break;
                    }
                }
                if best.as_ref().map(|(b, _)| *b == dist).unwrap_or(false) {
                    break;
                }
            }
            if matches!(best, Some((0, _))) {
                break;
            }
        }
        if let Some((_, p)) = best {
            return Some(p);
        }
        for parent in idx.inherits.clone() {
            if let Some(p) = self.lookup_in(&parent, name, size, depth + 1, visited) {
                return Some(p);
            }
        }
        None
    }

    pub fn find(&mut self, name: &str, size: u32) -> Option<PathBuf> {
        if name.is_empty() {
            return None;
        }
        let key = (name.to_string(), size);
        if let Some(p) = self.paths.get(&key) {
            return p.clone();
        }
        let found = if name.starts_with('/') {
            Path::new(name).is_file().then(|| PathBuf::from(name))
        } else {
            let mut visited = vec![];
            let theme = self.theme.clone();
            self.lookup_in(&theme, name, size, 0, &mut visited)
                .or_else(|| self.lookup_in("Adwaita", name, size, 0, &mut visited))
                .or_else(|| self.lookup_in("hicolor", name, size, 0, &mut visited))
                .or_else(|| {
                    ["svg", "png"].iter().map(|e| PathBuf::from(format!("/usr/share/pixmaps/{name}.{e}"))).find(|p| p.is_file())
                })
                // Symbolic fallback: "foo-symbolic" → "foo" and vice versa.
                .or_else(|| {
                    if let Some(base) = name.strip_suffix("-symbolic") {
                        let mut v = vec![];
                        self.lookup_in(&theme, base, size, 0, &mut v)
                    } else {
                        None
                    }
                })
        };
        if self.paths.len() > 2000 {
            self.paths.clear();
        }
        self.paths.insert(key, found.clone());
        found
    }

    pub fn exists(&mut self, name: &str) -> bool {
        self.find(name, 32).is_some()
    }

    fn rasterize(path: &Path, px: u32) -> Option<ImageSurface> {
        let px = px.clamp(1, 1024) as i32;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "svg" {
            let data = fs::read(path).ok()?;
            let tree = resvg::usvg::Tree::from_data(&data, &resvg::usvg::Options::default()).ok()?;
            let sz = tree.size();
            let mut pm = resvg::tiny_skia::Pixmap::new(px as u32, px as u32)?;
            let s = (px as f32 / sz.width()).min(px as f32 / sz.height());
            let tx = (px as f32 - sz.width() * s) / 2.0;
            let ty = (px as f32 - sz.height() * s) / 2.0;
            resvg::render(&tree, resvg::tiny_skia::Transform::from_row(s, 0.0, 0.0, s, tx, ty), &mut pm.as_mut());
            let mut surf = ImageSurface::create(Format::ARgb32, px, px).ok()?;
            let stride = surf.stride() as usize;
            {
                let mut d = surf.data().ok()?;
                let src = pm.data();
                for y in 0..px as usize {
                    for x in 0..px as usize {
                        let i = (y * px as usize + x) * 4;
                        let o = y * stride + x * 4;
                        // tiny-skia: premultiplied RGBA; cairo: premultiplied BGRA (little-endian ARGB32)
                        d[o] = src[i + 2];
                        d[o + 1] = src[i + 1];
                        d[o + 2] = src[i];
                        d[o + 3] = src[i + 3];
                    }
                }
            }
            Some(surf)
        } else {
            let img = image::open(path).ok()?.thumbnail(px as u32, px as u32).to_rgba8();
            let (w, h) = img.dimensions();
            let mut surf = ImageSurface::create(Format::ARgb32, w as i32, h as i32).ok()?;
            let stride = surf.stride() as usize;
            {
                let mut d = surf.data().ok()?;
                for (x, y, p) in img.enumerate_pixels() {
                    let a = p[3] as u32;
                    let o = y as usize * stride + x as usize * 4;
                    d[o] = ((p[2] as u32 * a) / 255) as u8;
                    d[o + 1] = ((p[1] as u32 * a) / 255) as u8;
                    d[o + 2] = ((p[0] as u32 * a) / 255) as u8;
                    d[o + 3] = a as u8;
                }
            }
            Some(surf)
        }
    }

    fn surface(&mut self, name: &str, px: u32) -> Option<ImageSurface> {
        let key = (name.to_string(), px);
        if let Some(s) = self.raster.get(&key) {
            return s.clone();
        }
        let s = self.find(name, px).and_then(|p| Self::rasterize(&p, px));
        if self.order.len() >= self.cap {
            if let Some(old) = self.order.pop_front() {
                self.raster.remove(&old);
            }
        }
        self.order.push_back(key.clone());
        self.raster.insert(key, s.clone());
        s
    }

    /// Draws `name` into `r`. Symbolic icons (or any icon when `tint` is set and the name ends in
    /// `-symbolic`) are recoloured. Missing app icons get a lettered tile so nothing is blank.
    pub fn draw(&mut self, cr: &Context, name: &str, r: Rect, tint: Option<Color>, th: &Theme) {
        let px = (r.w.max(r.h) * self.scale).round().max(1.0) as u32;
        let symbolic = name.ends_with("-symbolic");
        match self.surface(name, px) {
            Some(s) => {
                cr.save().ok();
                cr.translate(r.x, r.y);
                let k = r.w / (s.width() as f64);
                cr.scale(k, k);
                match (symbolic, tint) {
                    (true, Some(c)) => {
                        gfx::set(cr, c);
                        let _ = cr.mask_surface(&s, 0.0, 0.0);
                    }
                    _ => {
                        let _ = cr.set_source_surface(&s, 0.0, 0.0);
                        cr.source().set_filter(cairo::Filter::Good);
                        let _ = cr.paint();
                    }
                }
                cr.restore().ok();
            }
            None if !symbolic => {
                // Lettered tile in the accent colour.
                let letter: String = name
                    .rsplit(['.', '/'])
                    .next()
                    .unwrap_or(name)
                    .chars()
                    .find(|c| c.is_alphanumeric())
                    .map(|c| c.to_uppercase().collect())
                    .unwrap_or_else(|| "?".into());
                gfx::fill_rounded(cr, r, r.w * 0.24, th.pal.accent_alt);
                gfx::text(cr, th, r, &letter, &gfx::Text::new(r.h * 0.5, th.pal.on_accent).bold().center());
            }
            None => {
                // Unknown symbolic: small dot so layout stays balanced.
                if let Some(c) = tint {
                    gfx::fill_rounded(cr, Rect::centered(r.cx(), r.cy(), r.w * 0.3, r.w * 0.3), r.w, c.with_alpha(0.6));
                }
            }
        }
    }
}
