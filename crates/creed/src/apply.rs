//! `creed theme apply` and other session helpers.

use creed_core::config::Config;
use creed_core::hardware::Hardware;
use creed_core::hypr::{lua_str, Hypr};
use creed_core::{theme, Paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn have(cmd: &str) -> bool {
    std::env::var_os("PATH").map(|p| std::env::split_paths(&p).any(|d| d.join(cmd).is_file())).unwrap_or(false)
}

fn icon_theme_exists(home: &Path, name: &str) -> bool {
    [home.join(".local/share/icons"), home.join(".icons"), PathBuf::from("/usr/share/icons")].iter().any(|b| b.join(name).join("index.theme").is_file())
}

/// Writes `content` to `target` unless the file exists and was not written by Creed.
pub fn write_managed(target: &Path, content: &str) -> Result<bool, String> {
    if let Ok(old) = fs::read_to_string(target) {
        if !old.chars().take(400).collect::<String>().contains("Creed") {
            return Ok(false);
        }
        if old == content {
            return Ok(true);
        }
    } else if target.exists() {
        return Ok(false);
    }
    if let Some(p) = target.parent() {
        fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let tmp = target.with_extension("creed-tmp");
    fs::write(&tmp, content).map_err(|e| format!("{}: {e}", tmp.display()))?;
    fs::rename(&tmp, target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(true)
}

fn firefox_profiles(home: &Path) -> Vec<PathBuf> {
    let base = home.join(".mozilla/firefox");
    let Ok(ini) = fs::read_to_string(base.join("profiles.ini")) else { return vec![] };
    ini.lines()
        .filter_map(|l| l.strip_prefix("Path="))
        .map(|p| if p.starts_with('/') { PathBuf::from(p) } else { base.join(p) })
        .filter(|p| p.is_dir())
        .collect()
}

pub fn apply(mode: Option<&str>, reload: bool) -> Result<(), String> {
    let paths = Paths::from_env();
    let mut cfg = Config::load(&paths, mode).map_err(|e| e.to_string())?.config;
    let hw = Hardware::detect();
    // Fall back gracefully when a mode's icon theme has not been downloaded.
    if !icon_theme_exists(&paths.home, &cfg.style.icon_theme) {
        let dark = cfg.scheme() == creed_core::config::Scheme::Dark;
        let fallback = if dark { "Papirus-Dark" } else { "Papirus" };
        eprintln!("creed: icon theme '{}' is not installed; using {fallback} (run `creed setup` to fetch mode assets)", cfg.style.icon_theme);
        cfg.style.icon_theme = fallback.into();
    }
    if !icon_theme_exists(&paths.home, &cfg.style.cursor_theme) {
        cfg.style.cursor_theme = "Adwaita".into();
    }
    let pal = theme::palette_for(&paths, &cfg);
    let files = theme::render_all(&paths, &cfg, &pal, &hw, &have)?;
    let gen = paths.generated();
    fs::create_dir_all(&gen).map_err(|e| format!("{}: {e}", gen.display()))?;
    for (name, content) in &files {
        let target = gen.join(name);
        let tmp = target.with_extension("tmp");
        fs::write(&tmp, content).map_err(|e| format!("{}: {e}", tmp.display()))?;
        fs::rename(&tmp, &target).map_err(|e| e.to_string())?;
    }
    let get = |n: &str| files.iter().find(|(k, _)| k == n).map(|(_, v)| v.as_str()).unwrap_or("");
    let cfgdir = paths.home.join(".config");
    let mut skipped = vec![];
    let mut managed = |rel: &str, name: &str| {
        let t = cfgdir.join(rel);
        match write_managed(&t, get(name)) {
            Ok(false) => skipped.push(t.display().to_string()),
            Err(e) => eprintln!("creed: {e}"),
            _ => {}
        }
    };
    managed("gtk-3.0/gtk.css", "gtk.css");
    managed("gtk-4.0/gtk.css", "gtk.css");
    managed("qt5ct/qt5ct.conf", "qtct.conf");
    managed("qt6ct/qt6ct.conf", "qtct.conf");
    managed("MangoHud/MangoHud.conf", "mangohud.conf");
    if cfg.general.theme_browser {
        for prof in firefox_profiles(&paths.home) {
            let _ = write_managed(&prof.join("chrome/userChrome.css"), get("userChrome.css"));
            let user_js = prof.join("user.js");
            let line = "user_pref(\"toolkit.legacyUserProfileCustomizations.stylesheets\", true); // Creed";
            let cur = fs::read_to_string(&user_js).unwrap_or_default();
            if !cur.contains("legacyUserProfileCustomizations") {
                let _ = fs::write(&user_js, format!("{cur}{}{line}\n", if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" }));
            }
        }
    }
    for s in &skipped {
        eprintln!("creed: left {s} alone (not managed by Creed - `creed setup` can adopt it)");
    }
    // GTK settings through gsettings (GTK4 / portals) when available.
    if have("gsettings") {
        let dark = cfg.scheme() == creed_core::config::Scheme::Dark;
        let font = format!("{} {}", cfg.font(), cfg.appearance.font_size.round());
        let sets: [(&str, String); 6] = [
            ("gtk-theme", cfg.style.gtk_theme.clone()),
            ("icon-theme", cfg.style.icon_theme.clone()),
            ("cursor-theme", cfg.style.cursor_theme.clone()),
            ("cursor-size", cfg.style.cursor_size.to_string()),
            ("font-name", font),
            ("color-scheme", if dark { "prefer-dark".into() } else { "prefer-light".into() }),
        ];
        for (k, v) in sets {
            let _ = Command::new("gsettings").args(["set", "org.gnome.desktop.interface", k, &v]).stderr(Stdio::null()).status();
        }
    }
    if reload {
        if let Some(h) = Hypr::from_env() {
            let _ = h.request("reload");
            let _ = h.request(&format!("setcursor {} {}", cfg.style.cursor_theme, cfg.style.cursor_size));
        }
        let _ = crate::shell_ipc("reload");
        export_lock_wallpaper(&paths);
        sddm_follow(&cfg.general.mode);
    }
    Ok(())
}

/// Procedural wallpapers exist only inside the shell; hyprlock needs a file.
fn export_lock_wallpaper(paths: &Paths) {
    let target = paths.cache.join("lock-wallpaper.png");
    let _ = fs::create_dir_all(&paths.cache);
    let _ = crate::shell_ipc(&format!("export-wallpaper {}", target.display()));
}

fn sddm_follow(mode: &str) {
    if Path::new("/etc/creed/sddm-follow-mode").exists() && Path::new("/usr/lib/creed/creed-helper").exists() {
        let _ = Command::new("pkexec").args(["/usr/lib/creed/creed-helper", "sddm", mode]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

pub fn session_start() -> Result<(), String> {
    let paths = Paths::from_env();
    if !paths.generated().join("hyprland.lua").is_file() {
        apply(None, false)?;
    }
    // First login after install: open the welcome screen once.
    let pending = paths.config.join(".welcome-pending");
    if pending.exists() && have("creed-settings") {
        let _ = fs::remove_file(&pending);
        let _ = Command::new("creed-settings").arg("--welcome").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn();
    }
    // Wait for the shell, then export the lock-screen wallpaper.
    for _ in 0..50 {
        if crate::shell_ipc("ping").is_ok() {
            export_lock_wallpaper(&paths);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
    Ok(())
}

pub fn snap(dir: &str) -> Result<(), String> {
    let h = Hypr::from_env().ok_or("Hyprland is not running")?;
    let c = h.active_window().ok_or("no active window")?;
    let mons = h.monitors();
    let m = mons.iter().find(|m| m.focused).or(mons.first()).ok_or("no monitor")?;
    let cfg = Config::load(&Paths::from_env(), None).map(|l| l.config).unwrap_or_default();
    let gap = cfg.style.window_gaps_out as f64;
    let bar = if cfg.bar.enabled { (cfg.bar.size + cfg.bar.margin) as f64 } else { 0.0 };
    let s = m.scale.max(0.1);
    let (mw, mh) = (m.width as f64 / s, m.height as f64 / s);
    let (mut ax, mut ay, mut aw, mut ah) = (m.x as f64 + gap, m.y as f64 + gap, mw - 2.0 * gap, mh - 2.0 * gap);
    match cfg.bar.position {
        creed_core::config::Edge::Top => {
            ay += bar;
            ah -= bar;
        }
        creed_core::config::Edge::Bottom => ah -= bar,
        creed_core::config::Edge::Left => {
            ax += bar;
            aw -= bar;
        }
        creed_core::config::Edge::Right => aw -= bar,
    }
    let half = ((aw - cfg.style.window_gaps_in as f64) / 2.0).round();
    let x = match dir {
        "left" => ax,
        "right" => ax + aw - half,
        _ => return Err("usage: creed snap left|right".into()),
    };
    let win = lua_str(&format!("address:{}", c.address));
    for l in [
        format!("hl.dsp.focus({{ window = {win} }})"),
        "hl.dsp.window.float({ action = \"set\" })".to_string(),
        format!("hl.dsp.window.resize({{ x = {}, y = {} }})", half, ah.round()),
        format!("hl.dsp.window.move({{ x = {}, y = {} }})", x.round(), ay.round()),
    ] {
        h.dispatch(&l).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn screenshot(kind: &str) -> Result<(), String> {
    for c in ["grim", "wl-copy"] {
        if !have(c) {
            return Err(format!("{c} is not installed"));
        }
    }
    let home = std::env::var("HOME").unwrap_or_default();
    let dir = std::env::var("XDG_PICTURES_DIR").unwrap_or_else(|_| format!("{home}/Pictures"));
    let dir = format!("{dir}/Screenshots");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = Command::new("date").arg("+%Y-%m-%d_%H-%M-%S").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let file = format!("{dir}/Screenshot_{stamp}.png");
    let grim = match kind {
        "region" => {
            if !have("slurp") {
                return Err("slurp is not installed".into());
            }
            let g = Command::new("slurp").output().map_err(|e| e.to_string())?;
            if !g.status.success() {
                return Ok(()); // cancelled
            }
            let geom = String::from_utf8_lossy(&g.stdout).trim().to_string();
            Command::new("grim").args(["-g", &geom, &file]).status()
        }
        "screen" | "" => Command::new("grim").arg(&file).status(),
        other => return Err(format!("unknown screenshot kind '{other}' (region|screen)")),
    };
    if !grim.map(|s| s.success()).unwrap_or(false) {
        return Err("grim failed".into());
    }
    if let Ok(f) = fs::File::open(&file) {
        let _ = Command::new("wl-copy").args(["--type", "image/png"]).stdin(f).status();
    }
    let _ = Command::new("notify-send").args(["-a", "Screenshot", "-i", &file, "Screenshot saved", &format!("Copied to clipboard · {file}")]).status();
    Ok(())
}
