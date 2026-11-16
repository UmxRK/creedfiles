//! Resolves the icon / cursor / GTK theme a mode asks for to one that is installed.
//!
//! A mode lists candidates in preference order, e.g.
//! `icon_theme = "MacTahoe-{scheme}, WhiteSur-{scheme}, Papirus-Dark"`.
//! Tokens pick the variant for the active colour scheme:
//! `{scheme}` → `dark`/`light`, `{Scheme}` → `Dark`/`Light`, `{-dark}` → `-dark`/`` (nothing).
//! The first installed candidate wins; when none is installed a safe default is used.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Icon,
    Cursor,
    Gtk,
}

/// Expands the scheme tokens of one candidate.
pub fn expand(candidate: &str, dark: bool) -> String {
    candidate
        .trim()
        .replace("{scheme}", if dark { "dark" } else { "light" })
        .replace("{Scheme}", if dark { "Dark" } else { "Light" })
        .replace("{-dark}", if dark { "-dark" } else { "" })
}

/// Every candidate of a spec, expanded, in order.
pub fn candidates(spec: &str, dark: bool) -> Vec<String> {
    spec.split(',').map(|c| expand(c, dark)).filter(|c| !c.is_empty()).collect()
}

fn icon_bases(home: &Path) -> Vec<PathBuf> {
    let mut v = vec![home.join(".local/share/icons"), home.join(".icons")];
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(dirs.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("icons")));
    v.push(PathBuf::from("/usr/share/icons"));
    v
}

fn theme_bases(home: &Path) -> Vec<PathBuf> {
    let mut v = vec![home.join(".local/share/themes"), home.join(".themes")];
    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    v.extend(dirs.split(':').filter(|d| !d.is_empty()).map(|d| PathBuf::from(d).join("themes")));
    v.push(PathBuf::from("/usr/share/themes"));
    v
}

/// Directory of an installed theme, if any.
pub fn find(kind: Kind, name: &str, home: &Path) -> Option<PathBuf> {
    if name.is_empty() || name.contains('/') || name.contains("..") {
        return None;
    }
    let ok = |d: &Path| match kind {
        Kind::Icon => d.join("index.theme").is_file(),
        Kind::Cursor => d.join("cursors").is_dir(),
        Kind::Gtk => d.join("gtk-3.0").is_dir() || d.join("gtk-4.0").is_dir(),
    };
    let bases = if kind == Kind::Gtk { theme_bases(home) } else { icon_bases(home) };
    bases.into_iter().map(|b| b.join(name)).find(|d| ok(d))
}

fn default_for(kind: Kind, dark: bool) -> &'static str {
    match (kind, dark) {
        (Kind::Icon, true) => "Papirus-Dark",
        (Kind::Icon, false) => "Papirus",
        (Kind::Cursor, _) => "Adwaita",
        (Kind::Gtk, true) => "adw-gtk3-dark",
        (Kind::Gtk, false) => "adw-gtk3",
    }
}

/// The installed theme to use for `spec` (see module docs).
pub fn resolve(kind: Kind, spec: &str, dark: bool, home: &Path) -> String {
    let cands = candidates(spec, dark);
    if let Some(c) = cands.iter().find(|c| find(kind, c, home).is_some()) {
        return c.clone();
    }
    // Nothing installed: the generic default if present, otherwise the first candidate as-is.
    let d = default_for(kind, dark);
    if find(kind, d, home).is_some() || cands.is_empty() {
        d.to_string()
    } else {
        cands[0].clone()
    }
}

/// True for full GTK themes that style widgets themselves (MacTahoe, Fluent, …), as opposed to
/// adw-gtk3/Adwaita which Creed recolours through named colours.
pub fn is_full_gtk_theme(name: &str) -> bool {
    let n = name.to_lowercase();
    !(n.is_empty() || n.starts_with("adw") || n == "default")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn tokens_and_order() {
        assert_eq!(expand("MacTahoe-{scheme}", true), "MacTahoe-dark");
        assert_eq!(expand("Fluent-round-{Scheme}", false), "Fluent-round-Light");
        assert_eq!(expand("Win11{-dark}", true), "Win11-dark");
        assert_eq!(expand("Win11{-dark}", false), "Win11");
        let tmp = std::env::temp_dir().join(format!("creed-themes-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(tmp.join(".local/share/icons/WhiteSur-dark")).unwrap();
        fs::write(tmp.join(".local/share/icons/WhiteSur-dark/index.theme"), "[Icon Theme]\n").unwrap();
        fs::create_dir_all(tmp.join(".themes/Fluent-round-Dark/gtk-3.0")).unwrap();
        fs::create_dir_all(tmp.join(".icons/Fluent-dark-cursors/cursors")).unwrap();
        assert_eq!(resolve(Kind::Icon, "MacTahoe-{scheme}, WhiteSur-{scheme}, Papirus-Dark", true, &tmp), "WhiteSur-dark");
        assert_eq!(resolve(Kind::Gtk, "Fluent-round-{Scheme}, adw-gtk3-dark", true, &tmp), "Fluent-round-Dark");
        assert_eq!(resolve(Kind::Cursor, "Fluent{-dark}-cursors", true, &tmp), "Fluent-dark-cursors");
        assert!(find(Kind::Icon, "../etc", &tmp).is_none());
        assert!(is_full_gtk_theme("MacTahoe-Dark") && !is_full_gtk_theme("adw-gtk3-dark"));
        let _ = fs::remove_dir_all(&tmp);
    }
}
