//! Layered configuration.
//!
//! Precedence (lowest → highest):
//! `defaults.toml` → `modes/<mode>/mode.toml` → `~/.config/creed/creed.toml`
//! → `overrides.toml` (global) → `overrides.toml [modes.<mode>]`.
//!
//! Every layer is validated on its own (so errors carry file and line) and then deep-merged.

use crate::color::Color;
use crate::paths::Paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ConfigError {
    pub file: PathBuf,
    pub line: Option<usize>,
    pub msg: String,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(l) => write!(f, "{}:{}: {}", self.file.display(), l, self.msg),
            None => write!(f, "{}: {}", self.file.display(), self.msg),
        }
    }
}
impl std::error::Error for ConfigError {}

macro_rules! enum_str {
    ($name:ident { $($var:ident = $s:literal),+ $(,)? } default $def:ident) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $s)] $var,)+ }
        impl Default for $name { fn default() -> Self { $name::$def } }
        impl $name {
            pub fn resolve(self) -> $name { self }
            pub fn as_str(self) -> &'static str { match self { $($name::$var => $s,)+ } }
            pub fn all() -> &'static [&'static str] { &[$($s),+] }
            pub fn from_str(s: &str) -> Option<$name> { match s { $($s => Some($name::$var),)+ _ => None } }
        }
    };
}

enum_str!(Edge { Top = "top", Bottom = "bottom", Left = "left", Right = "right" } default Top);
enum_str!(BarStyle { Floating = "floating", Edge = "edge", Islands = "islands" } default Floating);
enum_str!(Motion { Expressive = "expressive", Smooth = "smooth", Minimal = "minimal", Off = "off" } default Expressive);
enum_str!(ColorSource { Wallpaper = "wallpaper", Preset = "preset" } default Wallpaper);
enum_str!(Scheme { Dark = "dark", Light = "light" } default Dark);
enum_str!(Quality { Auto = "auto", Off = "off", Low = "low", Medium = "medium", High = "high", Ultra = "ultra" } default Auto);
enum_str!(Corner { TopRight = "top-right", TopLeft = "top-left", BottomRight = "bottom-right", BottomLeft = "bottom-left", TopCenter = "top-center" } default TopRight);
enum_str!(Material { Glass = "glass", Frost = "frost", Mica = "mica", Acrylic = "acrylic", Aqua = "aqua", Solid = "solid" } default Frost);
enum_str!(WindowMode { Tile = "tile", Float = "float" } default Tile);
enum_str!(Titlebar { None = "none", Windows = "windows", Mac = "mac" } default None);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Meta {
    pub name: String,
    pub family: String,
    pub description: String,
    pub preview: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct General {
    pub mode: String,
    /// `mode` = use the active mode's keybinds, `creed` = always Creed keybinds.
    pub keybind_profile: String,
    pub terminal: String,
    pub file_manager: String,
    pub browser: String,
    pub locale_24h: bool,
    /// Theme Firefox (userChrome.css) along with the mode.
    pub theme_browser: bool,
    /// Keyboard layout passed to Hyprland (empty = detect from the system).
    pub kb_layout: String,
}
impl Default for General {
    fn default() -> Self {
        Self {
            mode: "creed".into(),
            keybind_profile: "mode".into(),
            terminal: "foot".into(),
            file_manager: "thunar".into(),
            browser: "firefox".into(),
            locale_24h: true,
            theme_browser: true,
            kb_layout: String::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Appearance {
    pub color_source: ColorSource,
    pub preset: String,
    pub scheme: Scheme,
    pub wallpaper: String,
    pub accent: Option<Color>,
    pub motion: Motion,
    pub font: String,
    pub mono_font: String,
    pub font_size: f64,
    pub text_scale: f64,
    pub high_contrast: bool,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            color_source: ColorSource::Wallpaper,
            preset: "ember".into(),
            scheme: Scheme::Dark,
            wallpaper: String::new(),
            accent: None,
            motion: Motion::Expressive,
            font: "Inter".into(),
            mono_font: "JetBrains Mono".into(),
            font_size: 10.5,
            text_scale: 1.0,
            high_contrast: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Bar {
    pub enabled: bool,
    pub position: Edge,
    pub style: BarStyle,
    /// Thickness in logical pixels.
    pub size: u32,
    /// Gap between bar and screen edge (floating / islands).
    pub margin: u32,
    pub radius: f64,
    pub left: Vec<String>,
    pub center: Vec<String>,
    pub right: Vec<String>,
    pub clock_format: String,
    pub date_format: String,
    pub workspaces: u32,
}
impl Default for Bar {
    fn default() -> Self {
        Self {
            enabled: true,
            position: Edge::Top,
            style: BarStyle::Floating,
            size: 38,
            margin: 8,
            radius: 19.0,
            left: vec!["launcher".into(), "workspaces".into(), "window".into()],
            center: vec!["clock".into()],
            right: vec![
                "tray".into(),
                "media".into(),
                "network".into(),
                "bluetooth".into(),
                "volume".into(),
                "battery".into(),
                "control".into(),
            ],
            clock_format: "%H:%M".into(),
            date_format: "%a %d %b".into(),
            workspaces: 5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Dock {
    pub enabled: bool,
    pub position: Edge,
    pub autohide: bool,
    pub icon_size: u32,
    /// 1.0 disables magnification; 1.6 grows hovered icons by 60 %.
    pub magnify: f64,
    pub pinned: Vec<String>,
    pub show_running: bool,
    pub margin: u32,
}
impl Default for Dock {
    fn default() -> Self {
        Self {
            enabled: true,
            position: Edge::Bottom,
            autohide: false,
            icon_size: 44,
            magnify: 1.0,
            pinned: vec!["foot".into(), "thunar".into(), "firefox".into(), "creed-settings".into()],
            show_running: true,
            margin: 8,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GlassOverride {
    pub tint: Option<Color>,
    pub frost: Option<f64>,
    pub specular: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Glass {
    pub enabled: bool,
    pub quality: Quality,
    /// Compositor refraction strength (0–1).
    pub refraction: f64,
    /// Edge lens thickness (maps to Hyprland glass size, 4–40).
    pub thickness: f64,
    /// Surface roughness (0 = clear, 1 = rough).
    pub roughness: f64,
    /// Blur radius used for frost / mica / acrylic.
    pub blur_size: u32,
    pub blur_passes: u32,
    pub tint: Color,
    /// Opacity of the drawn surface fill on top of the compositor blur (0 clear – 1 opaque).
    pub frost: f64,
    pub specular: f64,
    pub specular_follow_cursor: bool,
    pub elements: BTreeMap<String, GlassOverride>,
}
impl Default for Glass {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: Quality::Auto,
            refraction: 0.35,
            thickness: 12.0,
            roughness: 0.25,
            blur_size: 8,
            blur_passes: 3,
            tint: Color::rgba(1.0, 1.0, 1.0, 0.06),
            frost: 0.72,
            specular: 0.25,
            specular_follow_cursor: true,
            elements: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub position: Corner,
    pub timeout_ms: u32,
    pub max_visible: u32,
    pub do_not_disturb: bool,
    pub history: u32,
}
impl Default for Notifications {
    fn default() -> Self {
        Self { position: Corner::TopRight, timeout_ms: 6000, max_visible: 4, do_not_disturb: false, history: 50 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Game {
    pub auto_detect: bool,
    pub unload_shell: bool,
    pub disable_effects: bool,
    pub performance_governor: bool,
    pub extra_classes: Vec<String>,
}
impl Default for Game {
    fn default() -> Self {
        Self { auto_detect: true, unload_shell: true, disable_effects: true, performance_governor: true, extra_classes: vec![] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct A11y {
    pub screen_reader: bool,
    pub reduce_motion: bool,
    pub focus_ring: bool,
}
impl Default for A11y {
    fn default() -> Self {
        Self { screen_reader: true, reduce_motion: false, focus_ring: true }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Weather {
    pub enabled: bool,
    pub location: String,
    pub metric: bool,
}
impl Default for Weather {
    fn default() -> Self {
        Self { enabled: true, location: String::new(), metric: true }
    }
}

/// Visual tokens. Modes set these; users may override any of them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Style {
    pub radius: f64,
    pub radius_small: f64,
    pub spacing: f64,
    pub padding: f64,
    pub border_width: f64,
    pub shadow: f64,
    pub material: Material,
    pub window_mode: WindowMode,
    pub titlebar: Titlebar,
    pub icon_theme: String,
    pub cursor_theme: String,
    pub cursor_size: u32,
    pub gtk_theme: String,
    /// Icon set the shell uses for its own `-symbolic` glyphs before the app icon theme
    /// (Fluent System Icons for Windows modes, Material Symbols for Creed).
    pub shell_icons: Option<String>,
    pub font: Option<String>,
    pub scheme: Option<Scheme>,
    pub palette: Option<String>,
    /// Drawing variant per surface family, e.g. bar = "win11".
    pub bar: String,
    pub dock: String,
    pub launcher: String,
    pub control: String,
    pub overview: String,
    pub dashboard: String,
    pub notifications: String,
    pub osd: String,
    pub window_gaps_in: u32,
    pub window_gaps_out: u32,
    pub window_rounding: u32,
    pub window_border: u32,
}
impl Default for Style {
    fn default() -> Self {
        Self {
            radius: 18.0,
            radius_small: 10.0,
            spacing: 8.0,
            padding: 12.0,
            border_width: 1.0,
            shadow: 0.22,
            material: Material::Frost,
            window_mode: WindowMode::Tile,
            titlebar: Titlebar::None,
            icon_theme: "Papirus-Dark".into(),
            cursor_theme: "Adwaita".into(),
            cursor_size: 24,
            gtk_theme: "adw-gtk3-dark".into(),
            shell_icons: None,
            font: None,
            scheme: None,
            palette: None,
            bar: "creed".into(),
            dock: "creed".into(),
            launcher: "creed".into(),
            control: "creed".into(),
            overview: "creed".into(),
            dashboard: "creed".into(),
            notifications: "creed".into(),
            osd: "creed".into(),
            window_gaps_in: 5,
            window_gaps_out: 12,
            window_rounding: 14,
            window_border: 2,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    pub meta: Option<Meta>,
    pub general: General,
    pub appearance: Appearance,
    pub bar: Bar,
    pub dock: Dock,
    pub glass: Glass,
    pub notifications: Notifications,
    pub game: Game,
    pub a11y: A11y,
    pub weather: Weather,
    pub style: Style,
    /// Per-mode overrides (only meaningful inside overrides.toml / creed.toml).
    pub modes: BTreeMap<String, toml::Table>,
}

fn line_of(text: &str, offset: usize) -> usize {
    text[..offset.min(text.len())].bytes().filter(|&b| b == b'\n').count() + 1
}

/// Parses and validates one layer, returning its raw table.
pub fn parse_layer(path: &Path, text: &str) -> Result<toml::Table, ConfigError> {
    let err = |e: toml::de::Error| ConfigError {
        file: path.to_path_buf(),
        line: e.span().map(|s| line_of(text, s.start)),
        msg: e.message().to_string(),
    };
    let table: toml::Table = toml::from_str(text).map_err(err)?;
    // Validate the layer against the schema on its own so errors keep their location.
    toml::from_str::<Config>(text).map_err(err)?;
    Ok(table)
}

fn read_layer(path: &Path) -> Result<Option<toml::Table>, ConfigError> {
    match fs::read_to_string(path) {
        Ok(t) => parse_layer(path, &t).map(Some),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(ConfigError { file: path.to_path_buf(), line: None, msg: e.to_string() }),
    }
}

/// Deep-merges `over` into `base`. Tables merge recursively; everything else is replaced.
pub fn merge(base: &mut toml::Table, over: &toml::Table) {
    for (k, v) in over {
        match (base.get_mut(k), v) {
            (Some(toml::Value::Table(b)), toml::Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(k.clone(), v.clone());
            }
        }
    }
}

/// Result of loading: the merged config plus where each layer came from.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub config: Config,
    pub layers: Vec<PathBuf>,
}

impl Config {
    /// Loads all layers. `mode_override` forces a mode (used by `creed mode set` previews).
    pub fn load(paths: &Paths, mode_override: Option<&str>) -> Result<Loaded, ConfigError> {
        let mut merged = toml::Table::new();
        let mut layers = vec![];
        let defaults = paths.defaults();
        if let Some(t) = read_layer(&defaults)? {
            merge(&mut merged, &t);
            layers.push(defaults);
        }
        let user = read_layer(&paths.user_config())?;
        let overrides = read_layer(&paths.overrides())?;

        // Determine mode: override arg > overrides > user > defaults.
        let mode_of = |t: &Option<toml::Table>| {
            t.as_ref()
                .and_then(|t| t.get("general"))
                .and_then(|g| g.get("mode"))
                .and_then(|m| m.as_str())
                .map(str::to_string)
        };
        let mode = mode_override
            .map(str::to_string)
            .or_else(|| mode_of(&overrides))
            .or_else(|| mode_of(&user))
            .or_else(|| mode_of(&Some(merged.clone())))
            .unwrap_or_else(|| "creed".into());
        if !crate::mode::valid_name(&mode) {
            return Err(ConfigError { file: paths.user_config(), line: None, msg: format!("invalid mode name '{mode}'") });
        }

        let mode_file = paths.mode_dir(&mode).join("mode.toml");
        match read_layer(&mode_file)? {
            Some(t) => {
                merge(&mut merged, &t);
                layers.push(mode_file);
            }
            None => {
                return Err(ConfigError { file: mode_file, line: None, msg: format!("mode '{mode}' is not installed") })
            }
        }
        for (layer, path) in [(user, paths.user_config()), (overrides, paths.overrides())] {
            if let Some(mut t) = layer {
                let scoped = t.remove("modes");
                merge(&mut merged, &t);
                layers.push(path.clone());
                if let Some(toml::Value::Table(modes)) = scoped {
                    if let Some(toml::Value::Table(m)) = modes.get(&mode) {
                        // Validate the scoped table as its own layer.
                        let text = toml::to_string(m).unwrap_or_default();
                        parse_layer(&path, &text).map_err(|mut e| {
                            e.msg = format!("[modes.{mode}] {}", e.msg);
                            e.line = None;
                            e
                        })?;
                        merge(&mut merged, m);
                    }
                }
            }
        }
        merged.insert("general".into(), {
            let mut g = merged.get("general").and_then(|g| g.as_table()).cloned().unwrap_or_default();
            g.insert("mode".into(), toml::Value::String(mode.clone()));
            toml::Value::Table(g)
        });
        merged.remove("modes");
        let config: Config = toml::Value::Table(merged).try_into().map_err(|e: toml::de::Error| ConfigError {
            file: paths.overrides(),
            line: None,
            msg: e.message().to_string(),
        })?;
        config.check().map_err(|msg| ConfigError { file: paths.overrides(), line: None, msg })?;
        Ok(Loaded { config, layers })
    }

    /// Range checks that serde cannot express.
    pub fn check(&self) -> Result<(), String> {
        let r = |name: &str, v: f64, lo: f64, hi: f64| {
            if v.is_finite() && v >= lo && v <= hi {
                Ok(())
            } else {
                Err(format!("{name} = {v} is out of range ({lo}–{hi})"))
            }
        };
        r("bar.size", self.bar.size as f64, 20.0, 96.0)?;
        r("bar.margin", self.bar.margin as f64, 0.0, 64.0)?;
        r("bar.radius", self.bar.radius, 0.0, 48.0)?;
        r("bar.workspaces", self.bar.workspaces as f64, 1.0, 20.0)?;
        r("dock.icon_size", self.dock.icon_size as f64, 16.0, 128.0)?;
        r("dock.magnify", self.dock.magnify, 1.0, 2.5)?;
        r("glass.refraction", self.glass.refraction, 0.0, 1.0)?;
        r("glass.thickness", self.glass.thickness, 4.0, 40.0)?;
        r("glass.roughness", self.glass.roughness, 0.0, 1.0)?;
        r("glass.blur_size", self.glass.blur_size as f64, 1.0, 30.0)?;
        r("glass.blur_passes", self.glass.blur_passes as f64, 1.0, 6.0)?;
        r("glass.frost", self.glass.frost, 0.0, 1.0)?;
        r("glass.specular", self.glass.specular, 0.0, 1.0)?;
        for (k, o) in &self.glass.elements {
            if let Some(f) = o.frost {
                r(&format!("glass.elements.{k}.frost"), f, 0.0, 1.0)?;
            }
            if let Some(s) = o.specular {
                r(&format!("glass.elements.{k}.specular"), s, 0.0, 1.0)?;
            }
        }
        r("appearance.font_size", self.appearance.font_size, 6.0, 32.0)?;
        r("appearance.text_scale", self.appearance.text_scale, 0.5, 3.0)?;
        r("notifications.timeout_ms", self.notifications.timeout_ms as f64, 0.0, 120000.0)?;
        r("notifications.max_visible", self.notifications.max_visible as f64, 1.0, 10.0)?;
        r("style.radius", self.style.radius, 0.0, 48.0)?;
        r("style.radius_small", self.style.radius_small, 0.0, 32.0)?;
        r("style.spacing", self.style.spacing, 0.0, 40.0)?;
        r("style.padding", self.style.padding, 0.0, 40.0)?;
        r("style.shadow", self.style.shadow, 0.0, 1.0)?;
        r("style.cursor_size", self.style.cursor_size as f64, 12.0, 96.0)?;
        let known = crate::modules::BAR_MODULES;
        for m in self.bar.left.iter().chain(&self.bar.center).chain(&self.bar.right) {
            if !known.contains(&m.as_str()) {
                return Err(format!("unknown bar module '{m}' (known: {})", known.join(", ")));
            }
        }
        if !["mode", "creed"].contains(&self.general.keybind_profile.as_str()) {
            return Err("general.keybind_profile must be \"mode\" or \"creed\"".into());
        }
        Ok(())
    }

    pub fn glass_for(&self, element: &str) -> (Color, f64, f64) {
        let o = self.glass.elements.get(element).cloned().unwrap_or_default();
        (
            o.tint.unwrap_or(self.glass.tint),
            o.frost.unwrap_or(self.glass.frost),
            o.specular.unwrap_or(self.glass.specular),
        )
    }

    pub fn effective_motion(&self) -> Motion {
        if self.a11y.reduce_motion {
            Motion::Minimal
        } else {
            self.appearance.motion.resolve()
        }
    }

    pub fn font(&self) -> &str {
        self.style.font.as_deref().unwrap_or(&self.appearance.font)
    }

    pub fn scheme(&self) -> Scheme {
        self.style.scheme.unwrap_or(self.appearance.scheme).resolve()
    }

    /// Replaces the icon / cursor / GTK theme candidate lists with the installed theme each one
    /// resolves to (see `themes`). Call once after loading, before using those fields.
    pub fn resolve_themes(&mut self, home: &std::path::Path) {
        use crate::themes::{resolve, Kind};
        let dark = self.scheme() == Scheme::Dark;
        self.style.icon_theme = resolve(Kind::Icon, &self.style.icon_theme, dark, home);
        self.style.cursor_theme = resolve(Kind::Cursor, &self.style.cursor_theme, dark, home);
        self.style.gtk_theme = resolve(Kind::Gtk, &self.style.gtk_theme, dark, home);
    }
}

/// Edits a single dotted key in a TOML document, preserving the rest of the file.
/// Used by `creed config set` and the settings app.
pub fn set_key(doc_text: &str, key: &str, value: toml::Value) -> Result<String, String> {
    let mut table: toml::Table = if doc_text.trim().is_empty() {
        toml::Table::new()
    } else {
        toml::from_str(doc_text).map_err(|e| e.to_string())?
    };
    let parts: Vec<&str> = key.split('.').collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(format!("invalid key '{key}'"));
    }
    let mut cur = &mut table;
    for p in &parts[..parts.len() - 1] {
        let entry = cur.entry(p.to_string()).or_insert_with(|| toml::Value::Table(toml::Table::new()));
        cur = match entry {
            toml::Value::Table(t) => t,
            _ => return Err(format!("'{p}' is not a table")),
        };
    }
    cur.insert(parts[parts.len() - 1].to_string(), value);
    toml::to_string_pretty(&table).map_err(|e| e.to_string())
}

pub fn unset_key(doc_text: &str, key: &str) -> Result<String, String> {
    let mut table: toml::Table = toml::from_str(doc_text).map_err(|e| e.to_string())?;
    let parts: Vec<&str> = key.split('.').collect();
    let mut cur = &mut table;
    for p in &parts[..parts.len() - 1] {
        match cur.get_mut(*p) {
            Some(toml::Value::Table(t)) => cur = t,
            _ => return Ok(toml::to_string_pretty(&table).map_err(|e| e.to_string())?),
        }
    }
    cur.remove(parts[parts.len() - 1]);
    toml::to_string_pretty(&table).map_err(|e| e.to_string())
}

/// Parses a CLI value: bool, integer, float, array (`[..]`), or falls back to string.
pub fn parse_value(s: &str) -> toml::Value {
    let wrapped = format!("v = {s}");
    if let Ok(t) = toml::from_str::<toml::Table>(&wrapped) {
        if let Some(v) = t.get("v") {
            return v.clone();
        }
    }
    toml::Value::String(s.to_string())
}

/// Reads a dotted key from a config serialized to TOML.
pub fn get_key(config: &Config, key: &str) -> Option<toml::Value> {
    let v = toml::Value::try_from(config).ok()?;
    let mut cur = &v;
    for p in key.split('.') {
        cur = cur.get(p)?;
    }
    Some(cur.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup() -> (tempfile::TempDir, Paths) {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let cfg = dir.path().join("cfg");
        fs::create_dir_all(data.join("modes/creed")).unwrap();
        fs::create_dir_all(data.join("modes/win11")).unwrap();
        fs::create_dir_all(&cfg).unwrap();
        fs::write(data.join("defaults.toml"), "[bar]\nsize = 30\nmargin = 4\n").unwrap();
        fs::write(data.join("modes/creed/mode.toml"), "[meta]\nname = \"Creed\"\n[bar]\nsize = 40\n").unwrap();
        fs::write(data.join("modes/win11/mode.toml"), "[meta]\nname = \"Windows 11\"\n[bar]\nposition = \"bottom\"\n[style]\nbar = \"win11\"\n").unwrap();
        let paths = Paths {
            config: cfg,
            data,
            state: dir.path().join("state"),
            cache: dir.path().join("cache"),
            runtime: dir.path().join("run"),
            home: dir.path().to_path_buf(),
        };
        (dir, paths)
    }

    #[test]
    fn precedence() {
        let (_d, p) = setup();
        let c = Config::load(&p, None).unwrap().config;
        assert_eq!(c.bar.size, 40); // mode beats defaults
        assert_eq!(c.bar.margin, 4); // defaults kept
        fs::write(p.user_config(), "[bar]\nsize = 50\n").unwrap();
        assert_eq!(Config::load(&p, None).unwrap().config.bar.size, 50);
        fs::write(p.overrides(), "[bar]\nsize = 60\n[modes.creed.bar]\nsize = 70\n").unwrap();
        assert_eq!(Config::load(&p, None).unwrap().config.bar.size, 70);
        // Scoped override does not leak into other modes.
        let w = Config::load(&p, Some("win11")).unwrap().config;
        assert_eq!(w.bar.size, 60);
        assert_eq!(w.bar.position.resolve(), Edge::Bottom);
        assert_eq!(w.style.bar, "win11");
        assert_eq!(w.general.mode, "win11");
    }

    #[test]
    fn mode_from_overrides() {
        let (_d, p) = setup();
        fs::write(p.overrides(), "[general]\nmode = \"win11\"\n").unwrap();
        assert_eq!(Config::load(&p, None).unwrap().config.style.bar, "win11");
    }

    #[test]
    fn error_has_line() {
        let (_d, p) = setup();
        fs::write(p.overrides(), "[bar]\nsize = 40\nsizee = 3\n").unwrap();
        let e = Config::load(&p, None).unwrap_err();
        assert_eq!(e.line, Some(3), "{e}");
        assert!(e.msg.contains("sizee"), "{e}");
        fs::write(p.overrides(), "[bar]\nposition = \"diagonal\"\n").unwrap();
        let e = Config::load(&p, None).unwrap_err();
        assert_eq!(e.line, Some(2), "{e}");
    }

    #[test]
    fn range_and_module_checks() {
        let (_d, p) = setup();
        fs::write(p.overrides(), "[glass]\nfrost = 2.0\n").unwrap();
        assert!(Config::load(&p, None).unwrap_err().msg.contains("glass.frost"));
        fs::write(p.overrides(), "[bar]\nleft = [\"nope\"]\n").unwrap();
        assert!(Config::load(&p, None).unwrap_err().msg.contains("nope"));
    }

    #[test]
    fn missing_mode() {
        let (_d, p) = setup();
        fs::write(p.overrides(), "[general]\nmode = \"bogus\"\n").unwrap();
        assert!(Config::load(&p, None).unwrap_err().msg.contains("not installed"));
        fs::write(p.overrides(), "[general]\nmode = \"../etc\"\n").unwrap();
        assert!(Config::load(&p, None).unwrap_err().msg.contains("invalid mode"));
    }

    #[test]
    fn set_and_get_keys() {
        let t = set_key("[bar]\nsize = 3\n", "glass.elements.bar.frost", parse_value("0.5")).unwrap();
        let t = set_key(&t, "bar.left", parse_value("[\"clock\"]")).unwrap();
        let tab: toml::Table = toml::from_str(&t).unwrap();
        assert_eq!(tab["glass"]["elements"]["bar"]["frost"].as_float(), Some(0.5));
        assert_eq!(tab["bar"]["size"].as_integer(), Some(3));
        assert_eq!(tab["bar"]["left"].as_array().unwrap().len(), 1);
        let t = unset_key(&t, "bar.size").unwrap();
        assert!(!t.contains("size"));
        assert_eq!(parse_value("hello"), toml::Value::String("hello".into()));
        assert_eq!(parse_value("true"), toml::Value::Boolean(true));
        let c = Config::default();
        assert_eq!(get_key(&c, "bar.size").unwrap().as_integer(), Some(38));
    }
}
