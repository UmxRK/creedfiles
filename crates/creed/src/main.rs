//! `creed` - command line for the Creed desktop.

mod apply;
mod setup;
mod tweaks;

use creed_core::config::{self, Config};
use creed_core::hardware::Hardware;
use creed_core::{mode, Paths};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::process::ExitCode;

const HELP: &str = "creed - control the Creed desktop

USAGE
  creed <command> [arguments]

APPEARANCE
  mode list                     List installed modes
  mode current                  Show the active mode
  mode set <mode>               Switch the whole desktop to a mode
  theme apply [--no-reload]     Re-generate Hyprland + app themes from the config
  wallpaper <file|creed://name> Set the wallpaper (colours follow it)

SHELL
  toggle|open|close <surface>   launcher, control, dashboard, notif-center, overview,
                                launchpad, clipboard, emoji, power, snap, calendar
  reload                        Reload the shell's configuration
  osd volume|brightness         Show the on-screen display
  game on|off|toggle|status     Game Mode
  snap left|right               Snap the active window to a half of the screen
  screenshot region|screen      Screenshot to clipboard + ~/Pictures/Screenshots

CONFIG
  config path                   Show config file locations
  config validate               Check every config layer (exit 1 on error)
  config get <key>              Read an effective value, e.g. bar.size
  config set <key> <value> [--mode]   Write to overrides.toml (--mode: only this mode)
  config unset <key> [--mode]   Remove an override

TWEAKS (everything is explained before it runs)
  tweak list                    All tweaks with status and recommendation
  tweak explain <id>            What a tweak changes, in plain words
  tweak apply <id>... [--yes] [--confirm]   Apply (asks first unless --yes;
                                --confirm is required for data-deleting cleanups)
  tweak revert <id>...          Restore the original files
  tweak status                  What is applied right now
  tweak recommended             Tweaks recommended for this hardware

SYSTEM
  hardware [--json]             Detected GPU / CPU / laptop / RAM
  doctor                        Check the installation
  ram                           Memory used by Creed right now
  setup [--yes]                 Per-user setup (backup + install configs)
  uninstall [--yes]             Restore your previous configs and revert tweaks
  update                        Update Creed (git install) or explain how (package)
  version
";

pub fn paths() -> Paths {
    Paths::from_env()
}

/// Sends one command to the running shell; returns its reply.
pub fn shell_ipc(cmd: &str) -> Result<String, String> {
    let p = paths().shell_socket();
    let mut s = UnixStream::connect(&p).map_err(|_| "creed-shell is not running".to_string())?;
    let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    s.write_all(format!("{cmd}\n").as_bytes()).map_err(|e| e.to_string())?;
    let _ = s.shutdown(std::net::Shutdown::Write);
    let mut out = String::new();
    s.read_to_string(&mut out).map_err(|e| e.to_string())?;
    let out = out.trim().to_string();
    if let Some(e) = out.strip_prefix("error: ") {
        return Err(e.to_string());
    }
    Ok(out)
}

pub fn ask(question: &str) -> bool {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return false;
    }
    print!("{question} [y/N] ");
    let _ = std::io::stdout().flush();
    let mut s = String::new();
    let _ = std::io::stdin().read_line(&mut s);
    matches!(s.trim().to_lowercase().as_str(), "y" | "yes")
}

fn load(mode: Option<&str>) -> Result<Config, String> {
    Config::load(&paths(), mode).map(|l| l.config).map_err(|e| e.to_string())
}

fn edit_overrides(f: impl FnOnce(&str) -> Result<String, String>) -> Result<(), String> {
    let p = paths();
    std::fs::create_dir_all(&p.config).map_err(|e| e.to_string())?;
    let file = p.overrides();
    let old = std::fs::read_to_string(&file).unwrap_or_default();
    let new = f(&old)?;
    // Validate the result before writing so a bad value never lands on disk.
    config::parse_layer(&file, &new).map_err(|e| e.to_string())?;
    let tmp = file.with_extension("toml.tmp");
    std::fs::write(&tmp, &new).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &file).map_err(|e| e.to_string())?;
    // The merged config must still be valid (range checks across layers).
    if let Err(e) = Config::load(&p, None) {
        std::fs::write(&file, old).map_err(|e| e.to_string())?;
        return Err(format!("rejected: {e}"));
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    let a = |i: usize| args.get(i).map(String::as_str).unwrap_or("");
    let flag = |f: &str| args.iter().any(|x| x == f);
    match a(0) {
        "" | "help" | "--help" | "-h" => {
            print!("{HELP}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("creed {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "mode" => match a(1) {
            "list" | "" => {
                let cur = load(None).map(|c| c.general.mode).unwrap_or_default();
                for m in mode::list(&paths()) {
                    let mark = if m.id == cur { "●" } else { " " };
                    println!("{mark} {:<12} {:<16} {}", m.id, m.meta.name, m.meta.description);
                }
                Ok(())
            }
            "current" => {
                println!("{}", load(None)?.general.mode);
                Ok(())
            }
            "set" => {
                let m = a(2);
                if !mode::valid_name(m) || !paths().mode_dir(m).join("mode.toml").is_file() {
                    return Err(format!("unknown mode '{m}' (see `creed mode list`)"));
                }
                edit_overrides(|t| config::set_key(t, "general.mode", toml::Value::String(m.to_string())))?;
                apply::apply(None, !flag("--no-reload"))?;
                println!("Mode set to {m}.");
                Ok(())
            }
            other => Err(format!("unknown mode command '{other}'")),
        },
        "theme" => match a(1) {
            "apply" => apply::apply(None, !flag("--no-reload")),
            other => Err(format!("unknown theme command '{other}'")),
        },
        "wallpaper" => {
            let w = a(1);
            if w.is_empty() {
                return Err("usage: creed wallpaper <file|creed://name>".into());
            }
            let value = if w.starts_with("creed://") {
                w.to_string()
            } else {
                let p = std::fs::canonicalize(w).map_err(|e| format!("{w}: {e}"))?;
                p.to_string_lossy().to_string()
            };
            edit_overrides(|t| config::set_key(t, "appearance.wallpaper", toml::Value::String(value.clone())))?;
            apply::apply(None, true)
        }
        "toggle" | "open" | "close" => {
            let what = a(1);
            let cmd = match what {
                "launchpad" => format!("{} overview apps", a(0)),
                "" => return Err(format!("usage: creed {} <surface>", a(0))),
                w => format!("{} {w}", a(0)),
            };
            shell_ipc(&cmd).map(|_| ())
        }
        "reload" => shell_ipc("reload").map(|_| ()),
        "osd" => shell_ipc(&format!("osd {}", a(1))).map(|_| ()),
        "game" => {
            let r = shell_ipc(&format!("game {}", if a(1).is_empty() { "status" } else { a(1) }))?;
            println!("Game Mode: {r}");
            Ok(())
        }
        "snap" => apply::snap(a(1)),
        "screenshot" => apply::screenshot(a(1)),
        "session-start" => apply::session_start(),
        "config" => match a(1) {
            "path" => {
                let p = paths();
                println!("user config : {}\noverrides   : {}\ndata        : {}\ngenerated   : {}", p.user_config().display(), p.overrides().display(), p.data.display(), p.generated().display());
                Ok(())
            }
            "validate" => match Config::load(&paths(), None) {
                Ok(l) => {
                    println!("OK - layers: {}", l.layers.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join(", "));
                    Ok(())
                }
                Err(e) => Err(e.to_string()),
            },
            "get" => {
                let v = config::get_key(&load(None)?, a(2)).ok_or_else(|| format!("no such key '{}'", a(2)))?;
                match v {
                    toml::Value::String(s) => println!("{s}"),
                    other => println!("{other}"),
                }
                Ok(())
            }
            "set" => {
                if a(2).is_empty() || args.len() < 4 {
                    return Err("usage: creed config set <key> <value> [--mode]".into());
                }
                let key = if flag("--mode") { format!("modes.{}.{}", load(None)?.general.mode, a(2)) } else { a(2).to_string() };
                let value = config::parse_value(a(3));
                edit_overrides(|t| config::set_key(t, &key, value))?;
                if flag("--apply") {
                    apply::apply(None, true)?;
                }
                Ok(())
            }
            "unset" => {
                let key = if flag("--mode") { format!("modes.{}.{}", load(None)?.general.mode, a(2)) } else { a(2).to_string() };
                edit_overrides(|t| config::unset_key(t, &key))?;
                if flag("--apply") {
                    apply::apply(None, true)?;
                }
                Ok(())
            }
            other => Err(format!("unknown config command '{other}'")),
        },
        "tweak" => tweaks::run(&args[1..]),
        "tweak-root" => tweaks::run_root(&args[1..]),
        "hardware" => {
            let hw = Hardware::detect();
            if flag("--json") {
                println!("{}", serde_json::to_string_pretty(&hw).unwrap());
            } else {
                for g in &hw.gpus {
                    println!("GPU      {:?} ({}) {}{}", g.vendor, g.driver, g.card, if g.boot_vga { " [primary]" } else { "" });
                }
                println!("Hybrid   {}", hw.is_hybrid());
                println!("CPU      {} ({} threads)", if hw.cpu_model.is_empty() { &hw.cpu_vendor } else { &hw.cpu_model }, hw.cores);
                println!("RAM      {} MB", hw.ram_mb);
                println!("Laptop   {}{}", hw.is_laptop, if hw.has_battery { " (battery)" } else { "" });
                println!("Wi-Fi    {}", if hw.wifi.is_empty() { "none".into() } else { hw.wifi.join(", ") });
                println!("Storage  ssd={} hdd={} btrfs-root={} snapper={}", hw.has_ssd, hw.has_hdd, hw.root_btrfs, hw.has_snapper);
                println!("Kernel   {}", hw.kernel);
            }
            Ok(())
        }
        "doctor" => setup::doctor(),
        "ram" => {
            let (total, rows) = setup::ram();
            for (name, pss) in rows {
                println!("{:>8.1} MB  {name}", pss as f64 / 1024.0);
            }
            println!("{:>8.1} MB  total (PSS)", total as f64 / 1024.0);
            Ok(())
        }
        "setup" => setup::setup(flag("--yes")),
        "uninstall" => setup::uninstall(flag("--yes")),
        "update" => setup::update(),
        other => Err(format!("unknown command '{other}' - try `creed help`")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("creed: {e}");
            ExitCode::FAILURE
        }
    }
}
