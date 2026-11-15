//! Desktop entries (freedesktop) for the launcher, dock and taskbar.

use crate::app::Msg;
use crate::data::AppEntry;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn data_dirs() -> Vec<PathBuf> {
    let home = std::env::var("HOME").unwrap_or_default();
    let mut v = vec![std::env::var("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from(&home).join(".local/share"))];
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(dirs.split(':').filter(|d| !d.is_empty()).map(PathBuf::from));
    v.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    v.push(PathBuf::from(&home).join(".local/share/flatpak/exports/share"));
    v
}

fn walk(dir: &Path, prefix: &str, out: &mut Vec<(String, PathBuf)>) {
    let Ok(rd) = fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_string();
        if p.is_dir() {
            walk(&p, &format!("{prefix}{name}-"), out);
        } else if let Some(stem) = name.strip_suffix(".desktop") {
            out.push((format!("{prefix}{stem}"), p));
        }
    }
}

/// Parses the `[Desktop Entry]` group. Returns None for hidden / non-application entries.
pub fn parse(id: &str, text: &str, lang: &str) -> Option<(AppEntry, String)> {
    let mut in_entry = false;
    let mut kv: HashMap<String, String> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if !in_entry || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            kv.entry(k.trim().to_string()).or_insert_with(|| v.trim().to_string());
        }
    }
    let get = |k: &str| -> String {
        let lang_short = lang.split(['_', '.', '@']).next().unwrap_or("");
        kv.get(&format!("{k}[{lang}]")).or_else(|| kv.get(&format!("{k}[{lang_short}]"))).or_else(|| kv.get(k)).cloned().unwrap_or_default()
    };
    if kv.get("Type").map(|t| t != "Application").unwrap_or(true) {
        return None;
    }
    if kv.get("NoDisplay").map(|v| v == "true").unwrap_or(false) || kv.get("Hidden").map(|v| v == "true").unwrap_or(false) {
        return None;
    }
    let desktop = "Hyprland";
    if let Some(only) = kv.get("OnlyShowIn") {
        if !only.split(';').any(|d| d == desktop) {
            return None;
        }
    }
    if let Some(not) = kv.get("NotShowIn") {
        if not.split(';').any(|d| d == desktop) {
            return None;
        }
    }
    if let Some(try_exec) = kv.get("TryExec") {
        if !try_exec.is_empty() && !(Path::new(try_exec).is_file() || super::have(try_exec)) {
            return None;
        }
    }
    let exec = kv.get("Exec").cloned().unwrap_or_default();
    if exec.is_empty() {
        return None;
    }
    let name = get("Name");
    if name.is_empty() {
        return None;
    }
    Some((
        AppEntry {
            id: id.to_string(),
            name,
            generic: get("GenericName"),
            icon: kv.get("Icon").cloned().unwrap_or_default(),
            exec,
            keywords: get("Keywords").replace(';', " "),
            terminal: kv.get("Terminal").map(|v| v == "true").unwrap_or(false),
        },
        kv.get("StartupWMClass").cloned().unwrap_or_default(),
    ))
}

static WM_CLASSES: std::sync::Mutex<Vec<(String, String)>> = std::sync::Mutex::new(Vec::new());

pub fn scan() -> Vec<AppEntry> {
    let lang = std::env::var("LC_MESSAGES").or_else(|_| std::env::var("LANG")).unwrap_or_default();
    let lang = lang.split('.').next().unwrap_or("").to_string();
    let mut seen: HashMap<String, ()> = HashMap::new();
    let mut apps = vec![];
    let mut classes = vec![];
    for d in data_dirs() {
        let mut files = vec![];
        walk(&d.join("applications"), "", &mut files);
        files.sort();
        for (id, path) in files {
            if seen.contains_key(&id) {
                continue;
            }
            seen.insert(id.clone(), ());
            let Ok(text) = fs::read_to_string(&path) else { continue };
            if let Some((a, wm)) = parse(&id, &text, &lang) {
                if !wm.is_empty() {
                    classes.push((wm.to_lowercase(), a.icon.clone()));
                }
                apps.push(a);
            }
        }
    }
    *WM_CLASSES.lock().unwrap() = classes;
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

pub fn refresh(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let _ = tx.send(Msg::Apps(scan()));
    });
}

/// Best icon for a window class.
pub fn icon_for_class(apps: &[AppEntry], class: &str) -> String {
    let c = class.to_lowercase();
    if let Some((_, icon)) = WM_CLASSES.lock().unwrap().iter().find(|(w, _)| *w == c) {
        return icon.clone();
    }
    apps.iter()
        .find(|a| a.id.to_lowercase() == c || a.id.to_lowercase().ends_with(&format!(".{c}")) || a.name.to_lowercase() == c)
        .map(|a| a.icon.clone())
        .unwrap_or(c)
}

/// Removes desktop-entry field codes (%f %F %u %U %i %c %k …) and handles `%%`.
pub fn clean_exec(exec: &str) -> String {
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            match chars.next() {
                Some('%') => out.push('%'),
                _ => {}
            }
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn launch(app: &AppEntry, terminal: &str) {
    let cmd = clean_exec(&app.exec);
    let cmd = if app.terminal { format!("{terminal} -e {cmd}") } else { cmd };
    super::spawn_shell(&cmd);
}

pub fn launch_id(id: &str) {
    if let Some(app) = scan().into_iter().find(|a| a.id == id) {
        let term = creed_core::Config::load(&creed_core::Paths::from_env(), None).map(|l| l.config.general.terminal).unwrap_or_else(|_| "foot".into());
        launch(&app, &term);
    } else if id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c)) {
        super::spawn_shell(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn desktop_parsing() {
        let t = "[Desktop Entry]\nType=Application\nName=Files\nName[de]=Dateien\nGenericName=File Manager\nExec=thunar %F\nIcon=org.xfce.thunar\nKeywords=folder;explorer;\nStartupWMClass=Thunar\n[Desktop Action new]\nName=New Window\nExec=thunar --new\n";
        let (a, wm) = parse("thunar", t, "de_DE").unwrap();
        assert_eq!(a.name, "Dateien");
        assert_eq!(a.exec, "thunar %F");
        assert_eq!(a.keywords, "folder explorer ");
        assert_eq!(wm, "Thunar");
        assert!(parse("x", "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n", "").is_none());
        assert!(parse("x", "[Desktop Entry]\nType=Link\nName=X\nExec=x\n", "").is_none());
        assert!(parse("x", "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=KDE;\n", "").is_none());
        assert!(parse("x", "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/definitely/missing\n", "").is_none());
        assert_eq!(clean_exec("firefox %u --x 100%% %F"), "firefox --x 100%");
    }
}
