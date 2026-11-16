//! Turns config + palette + hardware into template variables and renders every generated
//! file. Pure (no side effects) so it can be tested; the CLI writes and applies the output.

use crate::config::{Config, Material, Motion, Quality, Scheme, Titlebar, WindowMode};
use crate::hardware::{GpuVendor, Hardware};
use crate::palette::{self, Palette};
use crate::paths::Paths;
use crate::template::{self, Vars};
use crate::Color;
use std::fs;

/// Files rendered into `~/.config/creed/generated/`.
pub const OUTPUTS: &[&str] = &[
    "hyprland.lua",
    "gtk.css",
    "qt-colors.conf",
    "qtct.conf",
    "foot-colors.ini",
    "kitty-colors.conf",
    "starship.toml",
    "hyprlock.conf",
    "mangohud.conf",
    "userChrome.css",
];

pub fn family(cfg: &Config) -> String {
    cfg.meta.as_ref().map(|m| m.family.clone()).filter(|f| !f.is_empty()).unwrap_or_else(|| "creed".into())
}

/// Effective glass quality after `auto` resolution.
pub fn quality(cfg: &Config, hw: &Hardware) -> Quality {
    if !cfg.glass.enabled {
        return Quality::Off;
    }
    match cfg.glass.quality {
        Quality::Auto => {
            if hw.weak_gpu() {
                Quality::Low
            } else {
                Quality::High
            }
        }
        q => q,
    }
}

fn f(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".into()
    } else {
        s.to_string()
    }
}

/// The `decoration.blur` table body (Lua) for the configured material and quality.
pub fn blur_block(cfg: &Config, hw: &Hardware, pal: &Palette) -> String {
    let q = quality(cfg, hw);
    let g = &cfg.glass;
    if q == Quality::Off {
        return "            enabled = false,".into();
    }
    let passes = match q {
        Quality::Low => 2,
        Quality::Medium => 3,
        Quality::Ultra => (g.blur_passes + 1).min(6),
        _ => g.blur_passes,
    };
    let size = match q {
        Quality::Low => g.blur_size.min(6),
        _ => g.blur_size,
    };
    let dark = pal.scheme == Scheme::Dark;
    let material = cfg.style.material;
    let mut lines = vec![
        "enabled = true".to_string(),
        format!("size = {size}"),
        format!("passes = {passes}"),
        "new_optimizations = true".into(),
        "xray = false".into(),
        "popups = true".into(),
        "special = false".into(),
        format!("noise = {}", if material == Material::Acrylic { "0.025" } else { "0.01" }),
        format!("contrast = {}", if dark { "0.92" } else { "1.0" }),
        format!("brightness = {}", if dark { "0.82" } else { "1.0" }),
        format!("vibrancy = {}", if material == Material::Mica { "0.35" } else { "0.17" }),
        "vibrancy_darkness = 0.1".into(),
    ];
    let rich = matches!(q, Quality::High | Quality::Ultra);
    match material {
        Material::Glass if rich => {
            // Hyprland's refraction glass: refraction 0–20, lens size 4–512, roughness 0–1.
            lines.push("variant = 1".into());
            lines.push(format!(
                "glass = {{ refraction = {}, size = {}, roughness = {} }}",
                f((g.refraction * 20.0).clamp(0.0, 20.0)),
                f(g.thickness.clamp(4.0, 512.0)),
                f(g.roughness.clamp(0.0, 1.0))
            ));
        }
        Material::Acrylic if q != Quality::Low => {
            lines.push("variant = 8".into());
            lines.push(format!(
                "acrylic = {{ refraction = {}, bulb = {}, clarity = {}, aberration = {}, tint = \"{}\" }}",
                f((g.refraction * 12.0).clamp(0.0, 48.0)),
                f((g.thickness * 2.0).clamp(4.0, 256.0)),
                f((1.0 - g.roughness).clamp(0.0, 1.0)),
                f(if rich { 0.015 } else { 0.0 }),
                g.tint.to_hypr()
            ));
        }
        _ => lines.push("variant = 0".into()),
    }
    lines.iter().map(|l| format!("            {l},")).collect::<Vec<_>>().join("\n")
}

/// Animation curves and leaves (Lua) for the motion preset and OS family.
pub fn animations_block(cfg: &Config, family: &str) -> String {
    let motion = cfg.effective_motion();
    if motion == Motion::Off {
        return "-- Motion is off.".into();
    }
    let mut v = vec![
        "hl.curve(\"creedOut\", { type = \"bezier\", points = { {0.16, 1}, {0.3, 1} } })".to_string(),
        "hl.curve(\"creedInOut\", { type = \"bezier\", points = { {0.65, 0}, {0.35, 1} } })".into(),
        "hl.curve(\"creedLinear\", { type = \"bezier\", points = { {0, 0}, {1, 1} } })".into(),
        "hl.curve(\"creedSpring\", { type = \"spring\", mass = 1, stiffness = 210, damping = 19 })".into(),
        "hl.curve(\"creedSoft\", { type = \"spring\", mass = 1, stiffness = 260, damping = 30 })".into(),
    ];
    // (speed multiplier: lower = slower in Hyprland's units)
    let (speed, win_style, ws_style) = match (motion, family) {
        (Motion::Expressive, "windows") => (1.0, "popin 90%", "slide"),
        (Motion::Expressive, "macos") => (1.0, "popin 85%", "slide"),
        (Motion::Expressive, _) => (1.0, "popin 80%", "slidefade 18%"),
        (Motion::Smooth, _) => (1.4, "popin 92%", "slide"),
        _ => (2.2, "popin 96%", "fade"),
    };
    let s = |base: f64| f((base / speed).max(0.5));
    let spring = motion == Motion::Expressive;
    let win_in = if spring { "spring = \"creedSpring\"" } else { "bezier = \"creedOut\"" };
    let ws = if spring { "spring = \"creedSoft\"" } else { "bezier = \"creedOut\"" };
    v.push(format!("hl.animation({{ leaf = \"global\", enabled = true, speed = {}, bezier = \"creedOut\" }})", s(8.0)));
    v.push(format!("hl.animation({{ leaf = \"windows\", enabled = true, speed = {}, {win_in} }})", s(4.6)));
    v.push(format!("hl.animation({{ leaf = \"windowsIn\", enabled = true, speed = {}, {win_in}, style = \"{win_style}\" }})", s(4.2)));
    v.push(format!("hl.animation({{ leaf = \"windowsOut\", enabled = true, speed = {}, bezier = \"creedLinear\", style = \"{win_style}\" }})", s(1.6)));
    v.push(format!("hl.animation({{ leaf = \"fade\", enabled = true, speed = {}, bezier = \"creedOut\" }})", s(3.0)));
    v.push(format!("hl.animation({{ leaf = \"border\", enabled = true, speed = {}, bezier = \"creedOut\" }})", s(5.0)));
    v.push(format!("hl.animation({{ leaf = \"layers\", enabled = true, speed = {}, bezier = \"creedOut\", style = \"fade\" }})", s(4.0)));
    v.push(format!("hl.animation({{ leaf = \"workspaces\", enabled = true, speed = {}, {ws}, style = \"{ws_style}\" }})", s(3.2)));
    v.push(format!("hl.animation({{ leaf = \"specialWorkspace\", enabled = true, speed = {}, bezier = \"creedOut\", style = \"slidevert\" }})", s(3.4)));
    v.join("\n")
}

/// Detects the console/X keyboard layout so Hyprland starts with the user's layout.
pub fn system_kb_layout() -> String {
    if let Ok(t) = fs::read_to_string("/etc/X11/xorg.conf.d/00-keyboard.conf") {
        for l in t.lines() {
            let l = l.trim();
            if l.starts_with("Option") && l.contains("\"XkbLayout\"") {
                if let Some(v) = l.split('"').nth(3) {
                    return v.to_string();
                }
            }
        }
    }
    if let Ok(t) = fs::read_to_string("/etc/vconsole.conf") {
        for l in t.lines() {
            if let Some(v) = l.strip_prefix("KEYMAP=") {
                let v = v.trim_matches('"');
                // Console keymaps like "de-latin1" / "uk" → xkb layout.
                let base = v.split('-').next().unwrap_or("us");
                return match base {
                    "uk" => "gb".into(),
                    "" => "us".into(),
                    b => b.to_string(),
                };
            }
        }
    }
    "us".into()
}

pub fn env_block(cfg: &Config, hw: &Hardware) -> String {
    let mut e: Vec<(String, String)> = vec![
        ("XCURSOR_THEME".into(), cfg.style.cursor_theme.clone()),
        ("XCURSOR_SIZE".into(), cfg.style.cursor_size.to_string()),
        ("HYPRCURSOR_SIZE".into(), cfg.style.cursor_size.to_string()),
        ("QT_QPA_PLATFORM".into(), "wayland;xcb".into()),
        ("QT_QPA_PLATFORMTHEME".into(), "qt6ct".into()),
        ("QT_WAYLAND_DISABLE_WINDOWDECORATION".into(), "1".into()),
        ("GDK_BACKEND".into(), "wayland,x11,*".into()),
        ("MOZ_ENABLE_WAYLAND".into(), "1".into()),
        ("ELECTRON_OZONE_PLATFORM_HINT".into(), "auto".into()),
        ("XDG_CURRENT_DESKTOP".into(), "Hyprland".into()),
        ("XDG_SESSION_TYPE".into(), "wayland".into()),
        ("XDG_SESSION_DESKTOP".into(), "Hyprland".into()),
        ("STARSHIP_CONFIG".into(), "$HOME/.config/creed/generated/starship.toml".into()),
    ];
    if hw.has_vendor(GpuVendor::Nvidia) {
        e.push(("LIBVA_DRIVER_NAME".into(), "nvidia".into()));
        e.push(("NVD_BACKEND".into(), "direct".into()));
        if !hw.is_hybrid() {
            // On hybrid laptops the iGPU drives the desktop; only force NVIDIA GLX on desktops.
            e.push(("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into()));
            e.push(("GBM_BACKEND".into(), "nvidia-drm".into()));
        }
    }
    if hw.is_hybrid() {
        // Desktop on the integrated GPU (battery), discrete GPU available for offload.
        let mut cards: Vec<&crate::hardware::Gpu> = hw.gpus.iter().collect();
        cards.sort_by_key(|g| (!g.boot_vga, g.vendor == GpuVendor::Nvidia));
        let list: Vec<String> = cards.iter().map(|g| format!("/dev/dri/{}", g.card)).collect();
        e.push(("AQ_DRM_DEVICES".into(), list.join(":")));
    }
    e.iter()
        .map(|(k, v)| {
            let val = if v.contains("$HOME") { format!("os.getenv(\"HOME\") .. \"{}\"", v.trim_start_matches("$HOME")) } else { format!("\"{v}\"") };
            format!("hl.env(\"{k}\", {val})")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn autostart_block(have: &dyn Fn(&str) -> bool) -> String {
    let mut v = vec![];
    if have("hypridle") {
        v.push("    hl.exec_cmd(\"hypridle\")".to_string());
    }
    if have("wl-paste") && have("cliphist") {
        v.push("    hl.exec_cmd(\"wl-paste --type text --watch cliphist store\")".into());
        v.push("    hl.exec_cmd(\"wl-paste --type image --watch cliphist store\")".into());
    }
    if std::path::Path::new("/usr/lib/hyprpolkitagent/hyprpolkitagent").exists() {
        v.push("    hl.exec_cmd(\"/usr/lib/hyprpolkitagent/hyprpolkitagent\")".into());
    } else if std::path::Path::new("/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1").exists() {
        v.push("    hl.exec_cmd(\"/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1\")".into());
    }
    v.push("    hl.exec_cmd(\"creed session-start\")".into());
    v.join("\n")
}

/// hyprbars (titlebar plugin) configuration for the mode. Everything is guarded by
/// `if hl.plugin.hyprbars` so the config stays valid when the plugin is not loaded.
pub fn titlebar_block(cfg: &Config, pal: &Palette) -> String {
    let tb = cfg.style.titlebar;
    if tb == Titlebar::None {
        return "if hl.plugin.hyprbars then\n    hl.config({ plugin = { hyprbars = { enabled = false } } })\nend".into();
    }
    let dark = pal.scheme == Scheme::Dark;
    let font = cfg.font().replace('"', "");
    let close = "hyprctl dispatch 'hl.dsp.window.close()'";
    let maximize = "hyprctl dispatch 'hl.dsp.window.fullscreen({ mode = \"maximized\", action = \"toggle\" })'";
    let minimize = "hyprctl dispatch 'hl.dsp.window.move({ workspace = \"special:minimized\" })'";
    let (opts, buttons): (Vec<String>, Vec<(String, String, u32, &str, &str)>) = match tb {
        Titlebar::Windows => {
            // Windows 11 caption bar: 32 px, title left in 12 px Segoe/Selawik, glyph buttons right.
            let fg = pal.text.to_hypr();
            (
                vec![
                    "bar_height = 32".into(),
                    format!("bar_color = \"{}\"", pal.bg.to_hypr()),
                    format!("[\"col.text\"] = \"{}\"", pal.text.to_hypr()),
                    format!("bar_text_font = \"{font}\""),
                    "bar_text_size = 9".into(),
                    "bar_text_align = \"left\"".into(),
                    "bar_buttons_alignment = \"right\"".into(),
                    "bar_padding = 16".into(),
                    "bar_button_padding = 30".into(),
                    "bar_blur = true".into(),
                    "bar_part_of_window = true".into(),
                    "bar_precedence_over_border = true".into(),
                    "icon_on_hover = false".into(),
                    format!("inactive_button_color = \"{}\"", Color::rgba(0.0, 0.0, 0.0, 0.0).to_hypr()),
                ],
                // Order: first button sits at the right edge.
                vec![
                    ("rgba(00000000)".into(), fg.clone(), 16, "✕", close),
                    ("rgba(00000000)".into(), fg.clone(), 16, "▢", maximize),
                    ("rgba(00000000)".into(), fg, 16, "—", minimize),
                ],
            )
        }
        Titlebar::Mac | Titlebar::None => {
            // macOS: 28 px title bar, centred title, traffic lights on the left (12 px, 8 px apart).
            let glyph = "rgba(0000008c)".to_string();
            (
                vec![
                    "bar_height = 28".into(),
                    format!("bar_color = \"{}\"", if dark { pal.surface.to_hypr() } else { pal.surface_alt.to_hypr() }),
                    format!("[\"col.text\"] = \"{}\"", pal.subtext.to_hypr()),
                    format!("bar_text_font = \"{font}\""),
                    "bar_text_size = 10".into(),
                    "bar_text_weight = \"semibold\"".into(),
                    "bar_text_align = \"center\"".into(),
                    "bar_buttons_alignment = \"left\"".into(),
                    "bar_padding = 9".into(),
                    "bar_button_padding = 8".into(),
                    "bar_blur = true".into(),
                    "bar_part_of_window = true".into(),
                    "bar_precedence_over_border = true".into(),
                    "icon_on_hover = true".into(),
                    format!("inactive_button_color = \"{}\"", if dark { "rgba(5a5a5aff)" } else { "rgba(d1d1d1ff)" }),
                ],
                vec![
                    ("rgba(ff5f57ff)".into(), glyph.clone(), 12, "×", close),
                    ("rgba(febc2eff)".into(), glyph.clone(), 12, "−", minimize),
                    ("rgba(28c840ff)".into(), glyph, 12, "+", maximize),
                ],
            )
        }
    };
    let mut s = String::from("if hl.plugin.hyprbars then\n    hl.config({ plugin = { hyprbars = {\n        enabled = true,\n");
    for o in opts {
        s.push_str(&format!("        {o},\n"));
    }
    s.push_str(&format!("        on_double_click = [[{maximize}]],\n    }} }} }})\n"));
    for (bg, fg, size, icon, action) in buttons {
        s.push_str(&format!("    hl.plugin.hyprbars.add_button({{ bg_color = \"{bg}\", fg_color = \"{fg}\", size = {size}, icon = \"{icon}\", action = [[{action}]] }})\n"));
    }
    s.push_str("end");
    s
}

/// All template variables.
pub fn vars(paths: &Paths, cfg: &Config, pal: &Palette, hw: &Hardware, have: &dyn Fn(&str) -> bool) -> Vars {
    let mut v = pal.vars();
    let mode = cfg.general.mode.clone();
    let fam = family(cfg);
    let st = &cfg.style;
    let mut put = |k: &str, val: String| {
        v.insert(k.to_string(), val);
    };
    put("mode", mode.clone());
    put("mode.name", cfg.meta.as_ref().map(|m| m.name.clone()).unwrap_or_else(|| mode.clone()));
    put("family", fam.clone());
    put("family.creed", (fam == "creed").to_string());
    put("family.windows", (fam == "windows").to_string());
    put("family.macos", (fam == "macos").to_string());
    put("font", cfg.font().to_string());
    put("font.mono", cfg.appearance.mono_font.clone());
    put("font.size", f(cfg.appearance.font_size * cfg.appearance.text_scale));
    put("font.size.int", ((cfg.appearance.font_size * cfg.appearance.text_scale).round() as i64).to_string());
    put("style.radius", f(st.radius));
    put("style.radius_small", f(st.radius_small));
    put("style.radius_small.int", (st.radius_small.round() as i64).to_string());
    put("style.window_rounding", st.window_rounding.to_string());
    put("style.window_gaps_in", st.window_gaps_in.to_string());
    put("style.window_gaps_out", st.window_gaps_out.to_string());
    put("style.window_border", st.window_border.to_string());
    put("style.icon_theme", st.icon_theme.clone());
    put("style.cursor_theme", st.cursor_theme.clone());
    put("style.cursor_size", st.cursor_size.to_string());
    put("style.gtk_theme", st.gtk_theme.clone());
    put(
        "style.rounding_power",
        match fam.as_str() {
            "macos" => "4.0".into(),
            "windows" => "2.0".into(),
            _ => "2.5".into(),
        },
    );
    put("window.float", (st.window_mode == WindowMode::Float).to_string());
    put("titlebar", st.titlebar.as_str().to_string());
    put("titlebar.on", (st.titlebar != Titlebar::None).to_string());
    let shadow_on = st.shadow > 0.0;
    put("shadow.enabled", shadow_on.to_string());
    put("shadow.range", ((14.0 + 22.0 * st.shadow).round() as i64).to_string());
    put("shadow.color", Color::rgba(0.0, 0.0, 0.0, (st.shadow * 0.9).clamp(0.0, 1.0)).to_hypr());
    put("shadow.color_inactive", Color::rgba(0.0, 0.0, 0.0, (st.shadow * 0.55).clamp(0.0, 1.0)).to_hypr());
    let (active, inactive) = match fam.as_str() {
        "macos" => (pal.border.with_alpha(0.5), pal.border.with_alpha(0.3)),
        "windows" => (pal.accent, pal.border.with_alpha(0.6)),
        _ => (pal.accent.with_alpha(0.93), pal.border.with_alpha(0.55)),
    };
    put("border.active", active.to_hypr());
    put("border.inactive", inactive.to_hypr());
    put("anim.enabled", (cfg.effective_motion() != Motion::Off).to_string());
    put("hypr.blur", blur_block(cfg, hw, pal));
    put("hypr.animations", animations_block(cfg, &fam));
    put("hypr.env", env_block(cfg, hw));
    put("hypr.autostart", autostart_block(have));
    put("hypr.titlebars", titlebar_block(cfg, pal));
    put("glass.layers", (quality(cfg, hw) != Quality::Off).to_string());
    put("hw.tearing", "false".into());
    put("hw.vrr", "2".into());
    put("hw.no_hw_cursors", hw.has_vendor(GpuVendor::Nvidia).to_string());
    put("input.kb_layout", if cfg.general.kb_layout.is_empty() { system_kb_layout() } else { cfg.general.kb_layout.clone() });
    put("input.natural_scroll", (fam == "macos").to_string());
    put("app.terminal", cfg.general.terminal.clone());
    put("app.file_manager", cfg.general.file_manager.clone());
    put("app.browser", cfg.general.browser.clone());
    put("path.generated", paths.generated().to_string_lossy().to_string());
    put("term.alpha", if fam == "macos" { "0.92".into() } else { "0.96".into() });
    put(
        "prompt.symbol",
        match fam.as_str() {
            "windows" => ">".into(),
            "macos" => "%".into(),
            _ => "❯".into(),
        },
    );
    put("lock.rounding", if mode == "win10" { "0".into() } else { "6".into() });
    // Windows 10 sets the lock clock bottom-left; Windows 11 centres it near the top.
    let win10 = mode == "win10";
    put("lock.halign", if win10 { "left" } else { "center" }.into());
    put("lock.valign", if win10 { "bottom" } else { "top" }.into());
    put("lock.clock_pos", if win10 { "64, 170" } else { "0, -150" }.into());
    put("lock.date_pos", if win10 { "70, 110" } else { "0, -300" }.into());
    put("lock.clock_size", if win10 { "110" } else { "120" }.into());
    put("lock.clock_weight", if win10 { "Light" } else { "Semibold" }.into());
    let wp = &cfg.appearance.wallpaper;
    let lock_wp = if wp.is_empty() || wp.starts_with("creed://") {
        paths.cache.join("lock-wallpaper.png").to_string_lossy().to_string()
    } else {
        paths.expand(wp).to_string_lossy().to_string()
    };
    put("lock.wallpaper", lock_wp);
    // User picture for the macOS-style lock screen: ~/.face, else AccountsService's copy.
    let user = std::env::var("USER").unwrap_or_default();
    let avatar = [paths.home.join(".face"), std::path::PathBuf::from(format!("/var/lib/AccountsService/icons/{user}"))]
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    put("lock.avatar", avatar);
    v
}

/// Renders one template by name (mode-specific copy wins).
pub fn render_one(paths: &Paths, mode: &str, name: &str, vars: &Vars) -> Result<String, String> {
    let path = crate::mode::template(paths, mode, name).ok_or_else(|| format!("template '{name}' not found"))?;
    let src = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    template::render(&src, vars).map_err(|e| format!("{}: {e}", path.display()))
}

/// Renders everything. Returns (file name, contents).
pub fn render_all(paths: &Paths, cfg: &Config, pal: &Palette, hw: &Hardware, have: &dyn Fn(&str) -> bool) -> Result<Vec<(String, String)>, String> {
    let vars = vars(paths, cfg, pal, hw, have);
    let mode = cfg.general.mode.as_str();
    let mut out = vec![];
    // Hyprland: base + mode extras + shared keys + profile keys.
    let mut hypr = render_one(paths, mode, "hypr/base.lua", &vars)?;
    let mode_extra = paths.mode_dir(mode).join("hypr.lua");
    if mode_extra.is_file() {
        let src = fs::read_to_string(&mode_extra).map_err(|e| e.to_string())?;
        hypr.push_str("\n------------------------------ mode extras -----------------------------\n");
        hypr.push_str(&template::render(&src, &vars).map_err(|e| format!("{}: {e}", mode_extra.display()))?);
    }
    hypr.push('\n');
    hypr.push_str(&render_one(paths, mode, "hypr/keybinds-common.lua", &vars)?);
    let profile = if cfg.general.keybind_profile == "creed" { "creed".to_string() } else { family(cfg) };
    let profile = match profile.as_str() {
        "windows" => "windows",
        "macos" => "mac",
        _ => "creed",
    };
    hypr.push('\n');
    hypr.push_str(&render_one(paths, mode, &format!("hypr/keybinds-{profile}.lua"), &vars)?);
    out.push(("hyprland.lua".to_string(), hypr));
    for name in OUTPUTS.iter().filter(|n| **n != "hyprland.lua") {
        out.push((name.to_string(), render_one(paths, mode, name, &vars)?));
    }
    Ok(out)
}

/// Palette for a config (same rules the shell uses).
pub fn palette_for(paths: &Paths, cfg: &Config) -> Palette {
    if let Some(name) = &cfg.style.palette {
        if let Some(p) = palette::system(name, cfg.scheme()) {
            return with_accent(p, cfg.appearance.accent);
        }
    }
    let preset = cfg.style.palette.clone().unwrap_or_else(|| cfg.appearance.preset.clone());
    let wp = &cfg.appearance.wallpaper;
    let seed = if cfg.style.palette.is_some() || cfg.appearance.color_source == crate::config::ColorSource::Preset {
        palette::preset_seed(&preset)
    } else if let Some(name) = wp.strip_prefix("creed://") {
        procedural_seed(name)
    } else if !wp.is_empty() {
        palette::seed_from_image(&paths.expand(wp)).ok()
    } else {
        None
    };
    let seed = seed.or_else(|| palette::preset_seed(&preset)).unwrap_or_else(|| palette::preset_seed("ember").unwrap());
    Palette::generate(seed, cfg.scheme(), cfg.appearance.high_contrast, cfg.appearance.accent)
}

/// Seeds for the shell's procedural wallpapers (kept in sync with creed-shell).
pub fn procedural_seed(name: &str) -> Option<palette::Seed> {
    let s = |hue: f64, alt: f64| palette::Seed { hue, alt_hue: alt, tint: 1.0, accent_chroma: 0.12, fixed_accent: None };
    Some(match name {
        "dawn" => s(45.0, 15.0),
        "dusk" => s(300.0, 330.0),
        "bloom" => s(250.0, 280.0),
        "ribbon" => s(240.0, 220.0),
        "dunes" => s(55.0, 30.0),
        "horizon" => s(260.0, 320.0),
        "tahoe" => s(255.0, 70.0),
        "sequoia" => s(285.0, 340.0),
        "aurora" => s(290.0, 200.0),
        "aqua" => s(245.0, 220.0),
        _ => return None,
    })
}

/// Applies a user accent override to a fixed system palette.
pub fn with_accent(mut p: Palette, accent: Option<Color>) -> Palette {
    if let Some(a) = accent {
        p.accent = a;
        p.on_accent = if a.contrast(&Color::WHITE) >= a.contrast(&Color::BLACK) { Color::WHITE } else { Color::BLACK };
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn blur_respects_quality_and_ranges() {
        let hw_strong = Hardware { gpus: vec![crate::hardware::Gpu { card: "card0".into(), vendor: GpuVendor::Amd, driver: "amdgpu".into(), pci: "x".into(), boot_vga: true }], ..Hardware::detect_at(std::path::Path::new("/nonexistent")) };
        let hw_weak = Hardware::detect_at(std::path::Path::new("/nonexistent"));
        let pal = Palette::generate(palette::preset_seed("ember").unwrap(), Scheme::Dark, false, None);
        let mut cfg = Config::default();
        cfg.style.material = Material::Glass;
        cfg.glass.refraction = 1.0;
        let b = blur_block(&cfg, &hw_strong, &pal);
        assert!(b.contains("variant = 1") && b.contains("refraction = 20"), "{b}");
        let b = blur_block(&cfg, &hw_weak, &pal);
        assert!(b.contains("variant = 0") && b.contains("passes = 2"), "{b}");
        cfg.style.material = Material::Acrylic;
        let b = blur_block(&cfg, &hw_strong, &pal);
        assert!(b.contains("variant = 8") && b.contains("bulb = 24"), "{b}");
        cfg.glass.enabled = false;
        assert!(blur_block(&cfg, &hw_strong, &pal).contains("enabled = false"));
    }

    #[test]
    fn titlebars() {
        let pal = Palette::generate(palette::preset_seed("macos").unwrap(), Scheme::Dark, false, None);
        let mut cfg = Config::default();
        assert!(titlebar_block(&cfg, &pal).contains("enabled = false"));
        cfg.style.titlebar = Titlebar::Mac;
        let b = titlebar_block(&cfg, &pal);
        assert!(b.starts_with("if hl.plugin.hyprbars then") && b.ends_with("end"));
        assert!(b.contains("bar_buttons_alignment = \"left\"") && b.contains("ff5f57"));
        assert_eq!(b.matches("add_button").count(), 3);
        cfg.style.titlebar = Titlebar::Windows;
        assert!(titlebar_block(&cfg, &pal).contains("bar_buttons_alignment = \"right\""));
    }

    #[test]
    fn number_format() {
        assert_eq!(f(2.0), "2");
        assert_eq!(f(0.125), "0.125");
        assert_eq!(f(0.0), "0");
        assert_eq!(f(1.2500001), "1.25");
    }

    #[test]
    fn motion_off_has_no_animations() {
        let mut cfg = Config::default();
        cfg.appearance.motion = Motion::Off;
        assert!(!animations_block(&cfg, "creed").contains("hl.animation"));
        cfg.appearance.motion = Motion::Expressive;
        assert!(animations_block(&cfg, "macos").contains("popin 85%"));
    }
}
