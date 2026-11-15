//! Mode discovery. A mode is a directory under `<data>/modes/<name>/` containing at least
//! `mode.toml`; optional `hypr.lua`, `keybinds.lua` and `templates/` override shared files.

use crate::config::{Config, Meta};
use crate::paths::Paths;
use std::fs;
use std::path::PathBuf;

pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 40
        && name.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !name.starts_with('-')
}

#[derive(Clone, Debug)]
pub struct ModeInfo {
    pub id: String,
    pub meta: Meta,
    pub dir: PathBuf,
}

/// Lists installed modes in a stable order (Creed first, then by family and name).
pub fn list(paths: &Paths) -> Vec<ModeInfo> {
    let mut out = vec![];
    let Ok(rd) = fs::read_dir(paths.modes_dir()) else { return out };
    for e in rd.flatten() {
        let id = e.file_name().to_string_lossy().to_string();
        if !valid_name(&id) {
            continue;
        }
        let f = e.path().join("mode.toml");
        let Ok(text) = fs::read_to_string(&f) else { continue };
        let meta = toml::from_str::<Config>(&text).ok().and_then(|c| c.meta).unwrap_or_default();
        out.push(ModeInfo { id, meta, dir: e.path() });
    }
    let order = |m: &ModeInfo| match m.meta.family.as_str() {
        "creed" => 0,
        "windows" => 1,
        "macos" => 2,
        _ => 3,
    };
    out.sort_by(|a, b| order(a).cmp(&order(b)).then(a.id.cmp(&b.id)));
    out
}

/// Finds a template, preferring the mode's own copy.
pub fn template(paths: &Paths, mode: &str, name: &str) -> Option<PathBuf> {
    let own = paths.mode_dir(mode).join("templates").join(name);
    if own.is_file() {
        return Some(own);
    }
    let shared = paths.templates_dir().join(name);
    shared.is_file().then_some(shared)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert!(valid_name("mac-tahoe"));
        assert!(valid_name("win11"));
        assert!(!valid_name("../x"));
        assert!(!valid_name("Mac"));
        assert!(!valid_name(""));
        assert!(!valid_name("-x"));
    }
}
