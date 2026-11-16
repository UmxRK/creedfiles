//! `creed tweak …` (user side) and `creed tweak-root …` (run as root by creed-helper).

use creed_core::hardware::Hardware;
use creed_core::tweak::{Engine, Status, CATEGORIES};
use std::path::PathBuf;
use std::process::Command;

const HELPER: &str = "/usr/lib/creed/creed-helper";

fn status_word(s: Status) -> &'static str {
    match s {
        Status::Applied => "applied",
        Status::NotApplied => "off",
        Status::Unavailable => "n/a",
    }
}

fn privileged(action: &str, id: &str, confirm: bool) -> Result<(), String> {
    if !std::path::Path::new(HELPER).exists() {
        return Err(format!("{HELPER} is missing - run the installer to set up system tweaks"));
    }
    // The installer already holds sudo and sets CREED_PRIV=sudo; everywhere else polkit is used.
    let mut c = if std::env::var("CREED_PRIV").as_deref() == Ok("sudo") {
        let mut c = Command::new("sudo");
        c.args(["-n", HELPER]);
        c
    } else {
        let mut c = Command::new("pkexec");
        c.arg(HELPER);
        c
    };
    c.args(["tweak", action, id]);
    if confirm {
        c.arg("--confirm");
    }
    let st = c.status().map_err(|e| format!("pkexec: {e}"))?;
    if st.success() {
        Ok(())
    } else {
        Err(format!("{action} {id} failed (exit {})", st.code().unwrap_or(-1)))
    }
}

pub fn run(args: &[String]) -> Result<(), String> {
    let paths = crate::paths();
    let eng = Engine::user(&paths);
    let hw = Hardware::detect();
    let flag = |f: &str| args.iter().any(|x| x == f);
    let ids: Vec<&String> = args.iter().skip(1).filter(|a| !a.starts_with("--")).collect();
    match args.first().map(String::as_str).unwrap_or("list") {
        "list" | "status" | "recommended" => {
            let only_applied = args.first().map(|s| s == "status").unwrap_or(false);
            let only_rec = args.first().map(|s| s == "recommended").unwrap_or(false);
            let all = eng.load_all();
            if flag("--json") {
                let v: Vec<_> = all
                    .iter()
                    .map(|t| {
                        serde_json::json!({
                            "id": t.id, "category": t.category, "title": t.def.title, "explain": t.def.explain,
                            "status": status_word(eng.status(t, &hw)), "recommended": hw.matches(&t.def.recommend),
                            "risk": t.def.risk, "privileged": t.def.privileged, "destructive": t.def.destructive,
                            "touches": t.def.touches, "needs_reboot": t.def.needs_reboot,
                        })
                    })
                    .collect();
                println!("{}", serde_json::to_string_pretty(&v).unwrap());
                return Ok(());
            }
            for (cat, title) in CATEGORIES {
                let items: Vec<_> = all.iter().filter(|t| t.category == *cat).collect();
                let items: Vec<_> = items
                    .into_iter()
                    .filter(|t| !only_applied || eng.status(t, &hw) == Status::Applied)
                    .filter(|t| !only_rec || (hw.matches(&t.def.recommend) && eng.status(t, &hw) == Status::NotApplied))
                    .collect();
                if items.is_empty() {
                    continue;
                }
                println!("\n{title}");
                for t in items {
                    let rec = if hw.matches(&t.def.recommend) { "★" } else { " " };
                    println!("  {rec} {:<26} {:<8} {}", t.id, status_word(eng.status(t, &hw)), t.def.title);
                }
            }
            println!("\n★ = recommended for this machine.  `creed tweak explain <id>` shows exactly what a tweak does.");
            Ok(())
        }
        "explain" => {
            for id in ids {
                let t = eng.get(id)?;
                println!("{}", eng.explain(&t, &paths.home));
            }
            Ok(())
        }
        "apply" => {
            if ids.is_empty() {
                return Err("usage: creed tweak apply <id>... [--yes] [--confirm]".into());
            }
            for id in ids {
                let t = eng.get(id)?;
                println!("{}", eng.explain(&t, &paths.home));
                if eng.status(&t, &hw) == Status::Unavailable {
                    return Err(format!("{id} is not available on this system"));
                }
                if t.def.destructive && !flag("--confirm") {
                    return Err(format!("{id} deletes data. Read the summary above, then run again with --confirm."));
                }
                if !flag("--yes") && !crate::ask(&format!("Apply “{}”?", t.def.title)) {
                    println!("Skipped {id}.");
                    continue;
                }
                if t.def.privileged {
                    privileged("apply", id, t.def.destructive)?;
                } else {
                    let out = eng.apply(&t, &hw, &paths.home, true)?;
                    print!("{out}");
                }
                println!("✓ {id} applied{}", if t.def.needs_reboot { " (takes effect after a reboot)" } else { "" });
            }
            Ok(())
        }
        "revert" => {
            if ids.is_empty() {
                return Err("usage: creed tweak revert <id>...".into());
            }
            for id in ids {
                let t = eng.get(id)?;
                if t.def.privileged {
                    privileged("revert", id, false)?;
                } else {
                    print!("{}", eng.revert(&t, &paths.home)?);
                }
                println!("✓ {id} reverted");
            }
            Ok(())
        }
        other => Err(format!("unknown tweak command '{other}'")),
    }
}

/// Root side. Only ever runs tweaks shipped in a root-owned data directory.
pub fn run_root(args: &[String]) -> Result<(), String> {
    if unsafe { libc::geteuid() } != 0 && std::env::var_os("CREED_ROOT").is_none() {
        return Err("tweak-root must run as root (through creed-helper)".into());
    }
    let data = std::env::var_os("CREED_DATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/usr/share/creed"));
    let root = std::env::var_os("CREED_ROOT").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/"));
    let eng = Engine::root(data.join("tweaks"), root.clone());
    let hw = Hardware::detect_at(&root);
    let action = args.first().map(String::as_str).unwrap_or("");
    let id = args.get(1).map(String::as_str).unwrap_or("");
    let confirm = args.iter().any(|a| a == "--confirm");
    let t = eng.get(id)?;
    if !t.def.privileged {
        return Err(format!("{id} is not a system tweak"));
    }
    let home = std::env::var_os("CREED_USER_HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("/root"));
    match action {
        "apply" => {
            if hw.has_snapper && !t.def.destructive && root == *"/" {
                let _ = Command::new("snapper").args(["-c", "root", "create", "-c", "number", "-d", &format!("creed: before {id}")]).status();
            }
            print!("{}", eng.apply(&t, &hw, &home, confirm)?);
            Ok(())
        }
        "revert" => {
            print!("{}", eng.revert(&t, &home)?);
            Ok(())
        }
        "status" => {
            println!("{}", status_word(eng.status(&t, &hw)));
            Ok(())
        }
        other => Err(format!("unknown action '{other}'")),
    }
}
