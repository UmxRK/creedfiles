//! The sidebar To Do list: a small JSON file in Creed's state directory.

use creed_core::Paths;

fn path() -> std::path::PathBuf {
    Paths::from_env().state.join("todo.json")
}

/// (text, done) in display order.
pub fn load() -> Vec<(String, bool)> {
    std::fs::read_to_string(path()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(items: &[(String, bool)]) {
    let p = path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let tmp = p.with_extension("json.tmp");
    if std::fs::write(&tmp, serde_json::to_string(items).unwrap_or_else(|_| "[]".into())).is_ok() {
        let _ = std::fs::rename(&tmp, &p);
    }
}

pub fn add(text: &str) {
    let t = text.trim();
    if t.is_empty() {
        return;
    }
    let mut v = load();
    v.push((t.chars().take(200).collect(), false));
    save(&v);
}

pub fn toggle(i: usize) {
    let mut v = load();
    if let Some(x) = v.get_mut(i) {
        x.1 = !x.1;
        save(&v);
    }
}

pub fn delete(i: usize) {
    let mut v = load();
    if i < v.len() {
        v.remove(i);
        save(&v);
    }
}
