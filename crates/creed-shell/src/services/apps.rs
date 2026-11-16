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
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps
}

fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// Parses GTK's recently-used.xbel: newest first, existing local files only.
pub fn parse_recent(xml: &str, now: i64) -> Vec<(String, String, String)> {
    let mut items: Vec<(String, String, i64)> = vec![];
    for chunk in xml.split("<bookmark ").skip(1) {
        let attr = |name: &str| chunk.split(&format!("{name}=\"")).nth(1).and_then(|r| r.split('"').next()).map(str::to_string);
        let Some(href) = attr("href") else { continue };
        let Some(path) = href.strip_prefix("file://").map(url_decode) else { continue };
        let when = attr("modified").or_else(|| attr("visited")).unwrap_or_default();
        let ts = parse_iso(&when).unwrap_or(0);
        let name = path.rsplit('/').next().unwrap_or(&path).to_string();
        if !std::path::Path::new(&path).is_file() {
            continue;
        }
        items.push((path, name, ts));
    }
    items.sort_by_key(|x| std::cmp::Reverse(x.2));
    items.dedup_by(|a, b| a.0 == b.0);
    items
        .into_iter()
        .take(8)
        .map(|(p, n, ts)| {
            let age = (now - ts).max(0);
            let when = if age < 3600 { format!("{}m ago", (age / 60).max(1)) } else if age < 86400 { format!("{}h ago", age / 3600) } else { crate::time::format(ts, "%d %b") };
            (p, n, when)
        })
        .collect()
}

/// Minimal ISO-8601 (YYYY-MM-DDTHH:MM:SS…Z) → unix seconds, UTC.
fn parse_iso(s: &str) -> Option<i64> {
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let mo: i64 = s.get(5..7)?.parse().ok()?;
    let d: i64 = s.get(8..10)?.parse().ok()?;
    let h: i64 = s.get(11..13)?.parse().ok()?;
    let mi: i64 = s.get(14..16)?.parse().ok()?;
    let se: i64 = s.get(17..19)?.parse().ok()?;
    // days from civil (Howard Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (mo + if mo > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(days * 86400 + h * 3600 + mi * 60 + se)
}

pub fn refresh_recent(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let home = std::env::var("HOME").unwrap_or_default();
        let base = std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{home}/.local/share"));
        let xml = fs::read_to_string(format!("{base}/recently-used.xbel")).unwrap_or_default();
        let _ = tx.send(Msg::Recent(parse_recent(&xml, crate::time::now_secs())));
    });
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
            if let Some('%') = chars.next() { out.push('%') }
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
        assert_eq!(parse_iso("1970-01-02T00:00:10Z"), Some(86410));
        assert_eq!(parse_iso("2026-11-16T12:00:00.123Z"), Some(1794830400));
        // Per-process directory: a fixed /tmp path breaks when another user created it first.
        let dir = std::env::temp_dir().join(format!("creed-recent-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("creed recent test.txt");
        std::fs::write(&f, "x").unwrap();
        let xml = format!("<xbel><bookmark href=\"file://{}\" modified=\"2026-11-16T10:00:00Z\"/><bookmark href=\"file:///nope/missing.txt\" modified=\"2026-11-16T11:00:00Z\"/></xbel>", f.display().to_string().replace(' ', "%20"));
        let r = parse_recent(&xml, 1794830400);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].1, "creed recent test.txt");
        assert_eq!(r[0].2, "2h ago");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
