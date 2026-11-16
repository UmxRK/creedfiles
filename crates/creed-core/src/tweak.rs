//! Tweak engine. Each tweak lives in `<data>/tweaks/<category>/<id>/` with `tweak.toml` and
//! bash scripts `apply`, optional `revert`, `status`, `plan`.
//!
//! Every file listed in `touches` is backed up before `apply` and restored byte-for-byte on
//! revert (files that did not exist are removed again). Scripts receive `CREED_ROOT`, a path
//! prefix that is empty on real systems and a temp dir in tests.
//!
//! Privileged tweaks are executed by root through `/usr/lib/creed/creed-helper`; the engine
//! itself runs in "root mode" there, with state in `<root>/var/lib/creed`.

use crate::hardware::Hardware;
use crate::paths::Paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TweakDef {
    pub title: String,
    pub explain: String,
    #[serde(default)]
    pub touches: Vec<String>,
    /// Hardware predicate (see `Hardware::matches`) under which the tweak is recommended.
    #[serde(default = "never")]
    pub recommend: String,
    /// Predicate under which the tweak is offered at all.
    #[serde(default = "always")]
    pub available: String,
    #[serde(default = "low")]
    pub risk: String,
    #[serde(default)]
    pub privileged: bool,
    /// Deletes data (caches, packages). Never automatic, always asks, cannot be reverted.
    #[serde(default)]
    pub destructive: bool,
    /// Commands that must exist for the tweak to be offered.
    #[serde(default)]
    pub requires: Vec<String>,
    /// Takes effect only after a reboot / relogin.
    #[serde(default)]
    pub needs_reboot: bool,
}
fn never() -> String {
    "!always".into()
}
fn always() -> String {
    "always".into()
}
fn low() -> String {
    "low".into()
}

#[derive(Clone, Debug)]
pub struct Tweak {
    pub id: String,
    pub category: String,
    pub dir: PathBuf,
    pub def: TweakDef,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct FileRecord {
    pub path: String,
    pub existed: bool,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Applied {
    pub files: Vec<FileRecord>,
    pub applied_at: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub applied: BTreeMap<String, Applied>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Applied,
    NotApplied,
    Unavailable,
}

pub const CATEGORIES: &[(&str, &str)] = &[
    ("gaming", "Gaming"),
    ("network", "Network & Wi-Fi"),
    ("system", "System & memory"),
    ("battery", "Battery & laptop"),
    ("cleanup", "Cleanup & free space"),
];

pub struct Engine {
    pub tweaks_dir: PathBuf,
    /// Filesystem prefix ("" on real systems).
    pub root: PathBuf,
    /// Where state + backups live.
    pub state_dir: PathBuf,
    pub root_mode: bool,
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

pub fn command_exists(cmd: &str, root: &Path) -> bool {
    if cmd.contains('/') {
        return root.join(cmd.trim_start_matches('/')).exists();
    }
    ["usr/bin", "usr/local/bin", "bin", "usr/sbin", "sbin"].iter().any(|d| root.join(d).join(cmd).exists())
}

impl Engine {
    /// Engine for the current user (unprivileged tweaks + reading root state).
    pub fn user(paths: &Paths) -> Engine {
        let root = std::env::var_os("CREED_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
        Engine { tweaks_dir: paths.tweaks_dir(), state_dir: paths.state.clone(), root, root_mode: false }
    }

    /// Engine running as root inside the helper.
    pub fn root(tweaks_dir: PathBuf, root: PathBuf) -> Engine {
        let state_dir = root.join("var/lib/creed");
        Engine { tweaks_dir, root, state_dir, root_mode: true }
    }

    pub fn load_all(&self) -> Vec<Tweak> {
        let mut out = vec![];
        for (cat, _) in CATEGORIES {
            let Ok(rd) = fs::read_dir(self.tweaks_dir.join(cat)) else { continue };
            let mut entries: Vec<_> = rd.flatten().collect();
            entries.sort_by_key(|e| e.file_name());
            for e in entries {
                let id = e.file_name().to_string_lossy().to_string();
                if !valid_id(&id) {
                    continue;
                }
                if let Ok(t) = self.load_dir(cat, &id, &e.path()) {
                    out.push(t);
                }
            }
        }
        out
    }

    fn load_dir(&self, cat: &str, id: &str, dir: &Path) -> Result<Tweak, String> {
        let text = fs::read_to_string(dir.join("tweak.toml")).map_err(|e| format!("{id}: {e}"))?;
        let def: TweakDef = toml::from_str(&text).map_err(|e| format!("{id}/tweak.toml: {e}"))?;
        Ok(Tweak { id: id.to_string(), category: cat.to_string(), dir: dir.to_path_buf(), def })
    }

    pub fn get(&self, id: &str) -> Result<Tweak, String> {
        if !valid_id(id) {
            return Err(format!("invalid tweak id '{id}'"));
        }
        self.load_all().into_iter().find(|t| t.id == id).ok_or_else(|| format!("no tweak named '{id}'"))
    }

    fn state_file(&self, privileged: bool) -> PathBuf {
        if privileged && !self.root_mode {
            self.root.join("var/lib/creed/tweaks.json")
        } else {
            self.state_dir.join("tweaks.json")
        }
    }

    pub fn state(&self, privileged: bool) -> State {
        fs::read_to_string(self.state_file(privileged)).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    fn save_state(&self, privileged: bool, s: &State) -> Result<(), String> {
        let f = self.state_file(privileged);
        fs::create_dir_all(f.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = f.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_string_pretty(s).unwrap()).map_err(|e| e.to_string())?;
        fs::rename(&tmp, &f).map_err(|e| e.to_string())
    }

    /// Resolves a `touches` entry (`/etc/...` or `~/...`) to a real path under the root prefix.
    fn resolve(&self, p: &str, home: &Path) -> PathBuf {
        if let Some(rest) = p.strip_prefix("~/") {
            home.join(rest)
        } else {
            self.root.join(p.trim_start_matches('/'))
        }
    }

    pub fn available(&self, t: &Tweak, hw: &Hardware) -> bool {
        hw.matches(&t.def.available) && t.def.requires.iter().all(|c| command_exists(c, &self.root))
    }

    fn run(&self, t: &Tweak, script: &str, extra: &[(&str, &str)]) -> Result<String, String> {
        let path = t.dir.join(script);
        if !path.exists() {
            return Ok(String::new());
        }
        let root = self.root.to_string_lossy().trim_end_matches('/').to_string();
        let mut cmd = Command::new("bash");
        cmd.arg(&path).env("CREED_ROOT", &root).env("CREED_TWEAK_DIR", &t.dir).env("CREED_TWEAK_ID", &t.id);
        for (k, v) in extra {
            cmd.env(k, v);
        }
        let out = cmd.output().map_err(|e| format!("{}: {e}", path.display()))?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(format!("{} {script} failed: {}", t.id, err.trim()));
        }
        Ok(stdout)
    }

    pub fn status(&self, t: &Tweak, hw: &Hardware) -> Status {
        if !self.available(t, hw) {
            return Status::Unavailable;
        }
        if t.dir.join("status").exists() {
            return match self.run(t, "status", &[]) {
                Ok(s) if s.trim() == "applied" => Status::Applied,
                _ => Status::NotApplied,
            };
        }
        if self.state(t.def.privileged).applied.contains_key(&t.id) {
            Status::Applied
        } else {
            Status::NotApplied
        }
    }

    /// Plain-language description shown before anything happens.
    pub fn explain(&self, t: &Tweak, home: &Path) -> String {
        let mut s = format!("{}\n\n{}\n", t.def.title, t.def.explain.trim());
        if t.dir.join("plan").exists() {
            match self.run(t, "plan", &[("HOME", &home.to_string_lossy())]) {
                Ok(p) if !p.trim().is_empty() => s.push_str(&format!("\nWhat will happen right now:\n{}\n", p.trim_end())),
                Ok(_) => s.push_str("\nWhat will happen right now:\n  Nothing to do - already clean.\n"),
                Err(e) => s.push_str(&format!("\nCould not compute the plan: {e}\n")),
            }
        }
        if !t.def.touches.is_empty() {
            s.push_str("\nFiles it changes:\n");
            for f in &t.def.touches {
                s.push_str(&format!("  • {f}\n"));
            }
        }
        s.push_str(&format!(
            "\nNeeds admin rights: {}\nReversible: {}\nRisk: {}{}\n",
            if t.def.privileged { "yes (granted once at install)" } else { "no" },
            if t.def.destructive { "no - deleted data cannot be restored" } else { "yes - `creed tweak revert` restores the original files" },
            t.def.risk,
            if t.def.needs_reboot { "\nTakes effect after: a reboot" } else { "" }
        ));
        s
    }

    pub fn apply(&self, t: &Tweak, hw: &Hardware, home: &Path, confirmed: bool) -> Result<String, String> {
        if t.def.privileged && !self.root_mode {
            return Err("privileged tweak must be applied through creed-helper".into());
        }
        if !self.available(t, hw) {
            return Err(format!("{} is not available on this system", t.id));
        }
        if t.def.destructive && !confirmed {
            return Err(format!("{} deletes data; confirm explicitly (--confirm) after reading `creed tweak explain {}`", t.id, t.id));
        }
        let home_s = home.to_string_lossy().to_string();
        if t.def.destructive {
            return self.run(t, "apply", &[("HOME", &home_s)]);
        }
        let mut state = self.state(t.def.privileged);
        let backup = self.state_dir.join("backups").join(&t.id);
        if !state.applied.contains_key(&t.id) {
            // First application: snapshot originals.
            let _ = fs::remove_dir_all(&backup);
            fs::create_dir_all(&backup).map_err(|e| e.to_string())?;
            let mut files = vec![];
            for (i, p) in t.def.touches.iter().enumerate() {
                let real = self.resolve(p, home);
                let existed = real.exists();
                if existed {
                    fs::copy(&real, backup.join(i.to_string())).map_err(|e| format!("backup {}: {e}", real.display()))?;
                }
                files.push(FileRecord { path: p.clone(), existed });
            }
            let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            state.applied.insert(t.id.clone(), Applied { files, applied_at: now });
            self.save_state(t.def.privileged, &state)?;
        }
        match self.run(t, "apply", &[("HOME", &home_s)]) {
            Ok(out) => Ok(out),
            Err(e) => {
                // Roll back partial changes.
                let _ = self.revert(t, home);
                Err(e)
            }
        }
    }

    pub fn revert(&self, t: &Tweak, home: &Path) -> Result<String, String> {
        if t.def.privileged && !self.root_mode {
            return Err("privileged tweak must be reverted through creed-helper".into());
        }
        if t.def.destructive {
            return Err(format!("{} deleted data and cannot be reverted", t.id));
        }
        let mut state = self.state(t.def.privileged);
        let Some(rec) = state.applied.get(&t.id).cloned() else {
            return Ok(format!("{} is not applied", t.id));
        };
        let backup = self.state_dir.join("backups").join(&t.id);
        for (i, f) in rec.files.iter().enumerate() {
            let real = self.resolve(&f.path, home);
            if f.existed {
                if let Some(parent) = real.parent() {
                    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fs::copy(backup.join(i.to_string()), &real).map_err(|e| format!("restore {}: {e}", real.display()))?;
            } else if real.exists() {
                fs::remove_file(&real).map_err(|e| format!("remove {}: {e}", real.display()))?;
            }
        }
        let out = self.run(t, "revert", &[("HOME", &home.to_string_lossy())])?;
        state.applied.remove(&t.id);
        self.save_state(t.def.privileged, &state)?;
        let _ = fs::remove_dir_all(&backup);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(root: &Path, cat: &str, id: &str, toml: &str, apply: &str) -> PathBuf {
        let d = root.join("tweaks").join(cat).join(id);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("tweak.toml"), toml).unwrap();
        fs::write(d.join("apply"), apply).unwrap();
        d
    }

    fn hw() -> Hardware {
        Hardware::detect_at(Path::new("/nonexistent"))
    }

    #[test]
    fn roundtrip_restores_bytes() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("root");
        fs::create_dir_all(root.join("etc/sysctl.d")).unwrap();
        fs::write(root.join("etc/sysctl.d/existing.conf"), "original\n").unwrap();
        mk(
            t.path(),
            "system",
            "demo",
            "title = \"Demo\"\nexplain = \"Changes two files.\"\nprivileged = true\ntouches = [\"/etc/sysctl.d/existing.conf\", \"/etc/sysctl.d/new.conf\"]\n",
            "echo changed > \"$CREED_ROOT/etc/sysctl.d/existing.conf\"\necho new > \"$CREED_ROOT/etc/sysctl.d/new.conf\"\n",
        );
        let e = Engine::root(t.path().join("tweaks"), root.clone());
        let tw = e.get("demo").unwrap();
        let ex = e.explain(&tw, t.path());
        assert!(ex.contains("/etc/sysctl.d/new.conf") && ex.contains("Reversible: yes"));
        e.apply(&tw, &hw(), t.path(), false).unwrap();
        e.apply(&tw, &hw(), t.path(), false).unwrap(); // idempotent: backup not overwritten
        assert_eq!(e.status(&tw, &hw()), Status::Applied);
        assert_eq!(fs::read_to_string(root.join("etc/sysctl.d/existing.conf")).unwrap(), "changed\n");
        e.revert(&tw, t.path()).unwrap();
        assert_eq!(fs::read_to_string(root.join("etc/sysctl.d/existing.conf")).unwrap(), "original\n");
        assert!(!root.join("etc/sysctl.d/new.conf").exists());
        assert_eq!(e.status(&tw, &hw()), Status::NotApplied);
        assert!(e.revert(&tw, t.path()).unwrap().contains("not applied"));
    }

    #[test]
    fn failing_apply_rolls_back() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("root");
        fs::create_dir_all(root.join("etc")).unwrap();
        fs::write(root.join("etc/a.conf"), "orig").unwrap();
        mk(t.path(), "system", "bad", "title = \"Bad\"\nexplain = \"x\"\ntouches = [\"/etc/a.conf\"]\n", "echo broken > \"$CREED_ROOT/etc/a.conf\"\nexit 3\n");
        let e = Engine::root(t.path().join("tweaks"), root.clone());
        let tw = e.get("bad").unwrap();
        assert!(e.apply(&tw, &hw(), t.path(), false).is_err());
        assert_eq!(fs::read_to_string(root.join("etc/a.conf")).unwrap(), "orig");
        assert_eq!(e.status(&tw, &hw()), Status::NotApplied);
    }

    #[test]
    fn destructive_needs_confirmation_and_shows_plan() {
        let t = tempfile::tempdir().unwrap();
        let d = mk(t.path(), "cleanup", "wipe", "title = \"Wipe\"\nexplain = \"Deletes cache.\"\ndestructive = true\n", "rm -rf \"$HOME/cache\"\n");
        fs::write(d.join("plan"), "echo \"  Removes 12 MB from ~/cache\"\n").unwrap();
        fs::create_dir_all(t.path().join("cache")).unwrap();
        let e = Engine { tweaks_dir: t.path().join("tweaks"), root: t.path().into(), state_dir: t.path().join("st"), root_mode: false };
        let tw = e.get("wipe").unwrap();
        let ex = e.explain(&tw, t.path());
        assert!(ex.contains("Removes 12 MB") && ex.contains("cannot be restored"), "{ex}");
        assert!(e.apply(&tw, &hw(), t.path(), false).unwrap_err().contains("confirm"));
        e.apply(&tw, &hw(), t.path(), true).unwrap();
        assert!(!t.path().join("cache").exists());
        assert!(e.revert(&tw, t.path()).is_err());
    }

    #[test]
    fn privileged_requires_helper_and_requires_commands() {
        let t = tempfile::tempdir().unwrap();
        mk(t.path(), "gaming", "p", "title = \"P\"\nexplain = \"x\"\nprivileged = true\n", "true\n");
        mk(t.path(), "gaming", "q", "title = \"Q\"\nexplain = \"x\"\nrequires = [\"definitely-not-a-command\"]\n", "true\n");
        let e = Engine { tweaks_dir: t.path().join("tweaks"), root: t.path().into(), state_dir: t.path().join("st"), root_mode: false };
        assert!(e.apply(&e.get("p").unwrap(), &hw(), t.path(), false).unwrap_err().contains("helper"));
        assert_eq!(e.status(&e.get("q").unwrap(), &hw()), Status::Unavailable);
        assert!(e.get("../etc").is_err());
    }
}
