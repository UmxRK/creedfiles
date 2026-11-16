//! Filesystem locations. Everything is overridable through environment variables so that
//! tests and the installer can point Creed at temporary trees.

use std::env;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Paths {
    /// `~/.config/creed`
    pub config: PathBuf,
    /// Shipped data: modes, templates, tweaks, wallpapers.
    pub data: PathBuf,
    /// `~/.local/state/creed`
    pub state: PathBuf,
    /// `~/.cache/creed`
    pub cache: PathBuf,
    /// `$XDG_RUNTIME_DIR/creed`
    pub runtime: PathBuf,
    /// `$HOME`
    pub home: PathBuf,
}

fn xdg(var: &str, home: &Path, fallback: &str) -> PathBuf {
    match env::var_os(var) {
        Some(v) if !v.is_empty() => PathBuf::from(v),
        _ => home.join(fallback),
    }
}

impl Paths {
    pub fn from_env() -> Paths {
        let home = env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/tmp"));
        let config = env::var_os("CREED_CONFIG")
            .map(PathBuf::from)
            .unwrap_or_else(|| xdg("XDG_CONFIG_HOME", &home, ".config").join("creed"));
        let data = env::var_os("CREED_DATA").map(PathBuf::from).unwrap_or_else(|| Self::find_data(&home));
        let state = env::var_os("CREED_STATE")
            .map(PathBuf::from)
            .unwrap_or_else(|| xdg("XDG_STATE_HOME", &home, ".local/state").join("creed"));
        let cache = xdg("XDG_CACHE_HOME", &home, ".cache").join("creed");
        let runtime = env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(env::temp_dir)
            .join("creed");
        Paths { config, data, state, cache, runtime, home }
    }

    fn find_data(home: &Path) -> PathBuf {
        let user = xdg("XDG_DATA_HOME", home, ".local/share").join("creed");
        if user.join("modes").is_dir() {
            return user;
        }
        // Running from a source checkout: the binary lives in <repo>/target/<profile>/.
        if let Ok(exe) = env::current_exe() {
            for anc in exe.ancestors().skip(1).take(4) {
                if anc.join("modes").is_dir() && anc.join("templates").is_dir() {
                    return anc.to_path_buf();
                }
            }
        }
        PathBuf::from("/usr/share/creed")
    }

    /// Downloaded look-alike photo wallpapers (`creed setup` → mode assets).
    pub fn wallpapers_dir(&self) -> PathBuf {
        xdg("XDG_DATA_HOME", &self.home, ".local/share").join("creed/wallpapers")
    }

    /// Photo that stands in for the procedural wallpaper `creed://name` when the mode assets were
    /// downloaded: `<name>-dark.jpg` / `<name>-light.jpg`, else `<name>.jpg`.
    pub fn wallpaper_photo(&self, name: &str, dark: bool) -> Option<PathBuf> {
        if name.is_empty() || name.contains('/') || name.contains("..") {
            return None;
        }
        let d = self.wallpapers_dir();
        let variant = d.join(format!("{name}-{}.jpg", if dark { "dark" } else { "light" }));
        let plain = d.join(format!("{name}.jpg"));
        [variant, plain].into_iter().find(|p| p.is_file())
    }

    pub fn user_config(&self) -> PathBuf {
        self.config.join("creed.toml")
    }
    pub fn overrides(&self) -> PathBuf {
        self.config.join("overrides.toml")
    }
    pub fn defaults(&self) -> PathBuf {
        self.data.join("defaults.toml")
    }
    pub fn modes_dir(&self) -> PathBuf {
        self.data.join("modes")
    }
    pub fn mode_dir(&self, mode: &str) -> PathBuf {
        self.modes_dir().join(mode)
    }
    pub fn templates_dir(&self) -> PathBuf {
        self.data.join("templates")
    }
    pub fn tweaks_dir(&self) -> PathBuf {
        self.data.join("tweaks")
    }
    /// Where generated (rendered) configs go.
    pub fn generated(&self) -> PathBuf {
        self.config.join("generated")
    }
    pub fn shell_socket(&self) -> PathBuf {
        self.runtime.join("shell.sock")
    }
    pub fn expand(&self, p: &str) -> PathBuf {
        if let Some(rest) = p.strip_prefix("~/") {
            self.home.join(rest)
        } else if p == "~" {
            self.home.clone()
        } else {
            PathBuf::from(p)
        }
    }
}
