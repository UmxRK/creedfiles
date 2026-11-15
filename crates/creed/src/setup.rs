//! Per-user setup, uninstall, update, doctor and RAM report.

use creed_core::config::Config;
use creed_core::tweak::{Engine, Status};
use creed_core::{hardware::Hardware, Paths};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Files Creed takes over. Originals are backed up once, before the first setup.
pub const MANAGED: &[&str] = &[
    ".config/hypr/hyprland.lua",
    ".config/hypr/hyprland.conf",
    ".config/hypr/hyprlock.conf",
    ".config/hypr/hypridle.conf",
    ".config/foot/foot.ini",
    ".config/kitty/kitty.conf",
    ".config/gtk-3.0/gtk.css",
    ".config/gtk-4.0/gtk.css",
    ".config/qt5ct/qt5ct.conf",
    ".config/qt6ct/qt6ct.conf",
    ".config/MangoHud/MangoHud.conf",
];

const RC_MARK: &str = "# >>> creed starship >>>";
const RC_END: &str = "# <<< creed starship <<<";

fn stamp() -> String {
    Command::new("date").arg("+%Y%m%d-%H%M%S").output().map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_else(|_| "backup".into())
}

fn backup_root(p: &Paths) -> PathBuf {
    p.home.join(".config/creed-backup")
}

/// Copies originals into ~/.config/creed-backup/<stamp>/ with a manifest of what existed.
fn backup(p: &Paths) -> Result<PathBuf, String> {
    let dir = backup_root(p).join(stamp());
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut manifest = String::from("# Creed backup manifest: <existed 0/1> <path relative to $HOME>\n");
    for rel in MANAGED {
        let src = p.home.join(rel);
        let existed = src.is_file();
        if existed {
            let dst = dir.join(rel);
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(&src, &dst).map_err(|e| format!("backup {}: {e}", src.display()))?;
        }
        manifest.push_str(&format!("{} {rel}\n", existed as u8));
    }
    for rc in [".bashrc", ".zshrc", ".config/fish/config.fish"] {
        let src = p.home.join(rc);
        if src.is_file() {
            let dst = dir.join(rc);
            fs::create_dir_all(dst.parent().unwrap()).map_err(|e| e.to_string())?;
            fs::copy(&src, &dst).map_err(|e| e.to_string())?;
        }
    }
    fs::write(dir.join("MANIFEST"), manifest).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn write(p: &Path, s: &str) -> Result<(), String> {
    if let Some(d) = p.parent() {
        fs::create_dir_all(d).map_err(|e| format!("{}: {e}", d.display()))?;
    }
    fs::write(p, s).map_err(|e| format!("{}: {e}", p.display()))
}

fn add_rc(path: &Path, body: &str) -> Result<(), String> {
    let cur = fs::read_to_string(path).unwrap_or_default();
    if cur.contains(RC_MARK) {
        return Ok(());
    }
    let sep = if cur.is_empty() || cur.ends_with('\n') { "" } else { "\n" };
    write(path, &format!("{cur}{sep}{RC_MARK}\n{body}\n{RC_END}\n"))
}

fn strip_rc(path: &Path) {
    let Ok(cur) = fs::read_to_string(path) else { return };
    if let (Some(a), Some(b)) = (cur.find(RC_MARK), cur.find(RC_END)) {
        let mut out = cur[..a].to_string();
        out.push_str(cur[b + RC_END.len()..].trim_start_matches('\n'));
        let _ = fs::write(path, out);
    }
}

pub fn setup(yes: bool) -> Result<(), String> {
    let p = Paths::from_env();
    let marker = p.config.join(".setup-done");
    println!("Creed setup for {}", p.home.display());
    println!("This will back up and replace: {}", MANAGED.join(", "));
    println!("Your originals go to {}/<date>/ and `creed uninstall` puts them back.", backup_root(&p).display());
    if !yes && !crate::ask("Continue?") {
        return Err("cancelled".into());
    }
    if !marker.exists() {
        let b = backup(&p)?;
        println!("✓ Backed up existing configs to {}", b.display());
    }
    let data = &p.data;
    let entry = fs::read_to_string(data.join("templates/hypr/entry.lua")).map_err(|e| format!("templates missing in {}: {e}", data.display()))?;
    write(&p.home.join(".config/hypr/hyprland.lua"), &entry)?;
    // An old hyprland.conf would shadow nothing in Lua mode, but keep the tree tidy.
    let old_conf = p.home.join(".config/hypr/hyprland.conf");
    if old_conf.is_file() {
        let _ = fs::remove_file(&old_conf);
    }
    let user = p.home.join(".config/hypr/user.lua");
    if !user.exists() {
        write(&user, &fs::read_to_string(data.join("templates/hypr/user.lua")).unwrap_or_default())?;
    }
    write(&p.home.join(".config/hypr/hyprlock.conf"), "# Managed by Creed - the look follows your mode.\nsource = ~/.config/creed/generated/hyprlock.conf\n")?;
    write(
        &p.home.join(".config/hypr/hypridle.conf"),
        "# Managed by Creed\ngeneral {\n    lock_cmd = pidof hyprlock || hyprlock\n    before_sleep_cmd = loginctl lock-session\n    after_sleep_cmd = hyprctl dispatch 'hl.dsp.dpms({ action = \"on\" })'\n}\n\nlistener {\n    timeout = 300\n    on-timeout = loginctl lock-session\n}\n\nlistener {\n    timeout = 360\n    on-timeout = hyprctl dispatch 'hl.dsp.dpms({ action = \"off\" })'\n    on-resume = hyprctl dispatch 'hl.dsp.dpms({ action = \"on\" })'\n}\n",
    )?;
    write(&p.home.join(".config/foot/foot.ini"), "# Managed by Creed - colours and font follow your mode.\ninclude=~/.config/creed/generated/foot-colors.ini\n")?;
    write(&p.home.join(".config/kitty/kitty.conf"), "# Managed by Creed - colours and font follow your mode.\ninclude ~/.config/creed/generated/kitty-colors.conf\nconfirm_os_window_close 0\n")?;
    for rel in [".config/gtk-3.0/gtk.css", ".config/gtk-4.0/gtk.css", ".config/qt5ct/qt5ct.conf", ".config/qt6ct/qt6ct.conf", ".config/MangoHud/MangoHud.conf"] {
        // Adopt: replace with a Creed-marked placeholder so `theme apply` may manage it.
        write(&p.home.join(rel), "/* Managed by Creed */\n")?;
    }
    let starship = "export STARSHIP_CONFIG=\"$HOME/.config/creed/generated/starship.toml\"\ncommand -v starship >/dev/null && eval \"$(starship init SHELL)\"";
    add_rc(&p.home.join(".bashrc"), &starship.replace("SHELL", "bash"))?;
    if p.home.join(".zshrc").exists() || which("zsh") {
        add_rc(&p.home.join(".zshrc"), &starship.replace("SHELL", "zsh"))?;
    }
    if which("fish") {
        add_rc(
            &p.home.join(".config/fish/config.fish"),
            "set -gx STARSHIP_CONFIG \"$HOME/.config/creed/generated/starship.toml\"\ncommand -q starship; and starship init fish | source",
        )?;
    }
    let user_cfg = p.user_config();
    if !user_cfg.exists() {
        write(
            &user_cfg,
            "# Creed configuration.\n#\n# Most people never touch this file: use the Creed Settings app (or `creed config set`),\n# which writes to overrides.toml next to it. Anything you put here is kept across updates.\n#\n# Example:\n# [bar]\n# position = \"bottom\"\n",
        )?;
    }
    write(&marker, "")?;
    crate::apply::apply(None, true)?;
    println!("✓ Creed is set up. Log in to Hyprland (or run `hyprctl reload`) to see it.");
    Ok(())
}

fn which(cmd: &str) -> bool {
    crate::apply::have(cmd)
}

pub fn uninstall(yes: bool) -> Result<(), String> {
    let p = Paths::from_env();
    let root = backup_root(&p);
    let mut backups: Vec<PathBuf> = fs::read_dir(&root).map(|r| r.flatten().map(|e| e.path()).filter(|p| p.join("MANIFEST").is_file()).collect()).unwrap_or_default();
    backups.sort();
    let first = backups.first().cloned();
    println!("This will:");
    println!("  • revert every applied tweak (system tweaks ask for your password once through polkit)");
    match &first {
        Some(b) => println!("  • restore your original configs from {}", b.display()),
        None => println!("  • remove Creed's managed config files (no backup was found)"),
    }
    println!("  • remove Creed's starship lines from your shell rc files");
    println!("  • keep ~/.config/creed (your settings) so you can come back later");
    if !yes && !crate::ask("Uninstall Creed for this user?") {
        return Err("cancelled".into());
    }
    // Tweaks first, while the helper still exists.
    let eng = Engine::user(&p);
    let hw = Hardware::detect();
    for t in eng.load_all() {
        if t.def.destructive || eng.status(&t, &hw) != Status::Applied {
            continue;
        }
        let r = if t.def.privileged {
            Command::new("pkexec").args(["/usr/lib/creed/creed-helper", "tweak", "revert", &t.id]).status().map(|s| s.success()).unwrap_or(false)
        } else {
            eng.revert(&t, &p.home).is_ok()
        };
        println!("{} revert {}", if r { "✓" } else { "✗" }, t.id);
    }
    match first {
        Some(b) => {
            let manifest = fs::read_to_string(b.join("MANIFEST")).map_err(|e| e.to_string())?;
            for line in manifest.lines().filter(|l| !l.starts_with('#')) {
                let Some((existed, rel)) = line.split_once(' ') else { continue };
                let target = p.home.join(rel);
                if existed == "1" {
                    if let Some(d) = target.parent() {
                        let _ = fs::create_dir_all(d);
                    }
                    fs::copy(b.join(rel), &target).map_err(|e| format!("restore {}: {e}", target.display()))?;
                } else if target.exists() {
                    let _ = fs::remove_file(&target);
                }
            }
        }
        None => {
            for rel in MANAGED {
                let t = p.home.join(rel);
                if fs::read_to_string(&t).map(|s| s.contains("Creed")).unwrap_or(false) {
                    let _ = fs::remove_file(&t);
                }
            }
        }
    }
    for rc in [".bashrc", ".zshrc", ".config/fish/config.fish"] {
        strip_rc(&p.home.join(rc));
    }
    let _ = fs::remove_dir_all(p.generated());
    let _ = fs::remove_file(p.config.join(".setup-done"));
    println!("✓ Creed removed for this user. Packages stay installed; remove them with pacman if you like.");
    Ok(())
}

pub fn update() -> Result<(), String> {
    let p = Paths::from_env();
    let info = fs::read_to_string(p.config.join(".install")).unwrap_or_default();
    let get = |k: &str| info.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).map(|v| v.trim_matches('"').to_string());
    match get("method").as_deref() {
        Some("git") => {
            let src = get("source").ok_or("install record has no source path")?;
            println!("Updating Creed from {src} …");
            let ok = Command::new("git").args(["-C", &src, "pull", "--ff-only"]).status().map(|s| s.success()).unwrap_or(false);
            if !ok {
                return Err("git pull failed (local changes in the checkout?)".into());
            }
            let ok = Command::new(format!("{src}/install.sh")).args(["--update", "--yes"]).status().map(|s| s.success()).unwrap_or(false);
            if ok {
                Ok(())
            } else {
                Err("install.sh --update failed".into())
            }
        }
        _ => {
            println!("Creed was installed as a package. Update it with your system:\n  paru -Syu    (or yay -Syu)");
            Ok(())
        }
    }
}

/// (total PSS kB, [(name, pss kB)]) for this user's Creed processes.
pub fn ram() -> (u64, Vec<(String, u64)>) {
    let uid = unsafe { libc::getuid() };
    let mut rows = vec![];
    let mut total = 0;
    if let Ok(rd) = fs::read_dir("/proc") {
        for e in rd.flatten() {
            let pid = e.file_name().to_string_lossy().to_string();
            if !pid.chars().all(|c| c.is_ascii_digit()) {
                continue;
            }
            let Ok(meta) = fs::metadata(e.path()) else { continue };
            if std::os::unix::fs::MetadataExt::uid(&meta) != uid {
                continue;
            }
            let comm = fs::read_to_string(e.path().join("comm")).unwrap_or_default().trim().to_string();
            if !comm.starts_with("creed-shell") {
                continue;
            }
            let pss = fs::read_to_string(e.path().join("smaps_rollup"))
                .ok()
                .and_then(|s| s.lines().find(|l| l.starts_with("Pss:")).and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok()))
                .unwrap_or(0);
            total += pss;
            rows.push((format!("{comm} (pid {pid})"), pss));
        }
    }
    (total, rows)
}

pub fn doctor() -> Result<(), String> {
    let p = Paths::from_env();
    let mut bad = 0;
    let mut check = |ok: bool, what: &str, hint: &str| {
        if ok {
            println!("  ✓ {what}");
        } else {
            bad += 1;
            println!("  ✗ {what}\n      → {hint}");
        }
    };
    println!("Creed doctor\n");
    let hv = Command::new("Hyprland").arg("--version").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    let ver = hv.split_whitespace().nth(1).unwrap_or("").to_string();
    let parts: Vec<u32> = ver.split('.').filter_map(|x| x.parse().ok()).collect();
    check(parts.len() >= 2 && (parts[0] > 0 || parts[1] >= 57), &format!("Hyprland ≥ 0.57 (found {})", if ver.is_empty() { "none" } else { &ver }), "install/update Hyprland: sudo pacman -Syu hyprland");
    check(which("creed-shell"), "creed-shell installed", "re-run ./install.sh");
    match Config::load(&p, None) {
        Ok(_) => check(true, "configuration is valid", ""),
        Err(e) => check(false, "configuration is valid", &e.to_string()),
    }
    check(p.generated().join("hyprland.lua").is_file(), "generated Hyprland config exists", "creed theme apply");
    check(fs::read_to_string(p.home.join(".config/hypr/hyprland.lua")).map(|s| s.contains("Creed")).unwrap_or(false), "~/.config/hypr/hyprland.lua is Creed's entry file", "creed setup");
    check(crate::shell_ipc("ping").is_ok(), "creed-shell is running", "start a Hyprland session, or run creed-shell");
    for (cmd, pkg) in [("wpctl", "wireplumber"), ("nmcli", "networkmanager"), ("grim", "grim"), ("slurp", "slurp"), ("wl-copy", "wl-clipboard"), ("cliphist", "cliphist"), ("hyprlock", "hyprlock"), ("hypridle", "hypridle"), ("brightnessctl", "brightnessctl"), ("playerctl", "playerctl"), ("notify-send", "libnotify")] {
        check(which(cmd), &format!("{cmd} available"), &format!("sudo pacman -S {pkg}"));
    }
    let fonts = Command::new("fc-list").arg(":family").output().map(|o| String::from_utf8_lossy(&o.stdout).to_string()).unwrap_or_default();
    check(fonts.contains("Inter"), "Inter font", "sudo pacman -S inter-font");
    check(Path::new("/usr/lib/creed/creed-helper").exists(), "system tweak helper installed", "re-run ./install.sh (needs sudo once)");
    check(Path::new("/etc/polkit-1/rules.d/50-creed.rules").exists() || Path::new("/usr/share/polkit-1/rules.d/50-creed.rules").exists(), "polkit rule for password-free tweaks", "re-run ./install.sh");
    println!();
    if bad == 0 {
        println!("Everything looks good.");
        Ok(())
    } else {
        Err(format!("{bad} problem(s) found"))
    }
}
