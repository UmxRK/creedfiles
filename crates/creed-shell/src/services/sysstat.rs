//! CPU / memory / disk / temperature, sampled only while something displays them.

use crate::app::Msg;
use crate::data::SysStat;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use std::time::Duration;

static ACTIVE: AtomicU32 = AtomicU32::new(0);
static LAST: Mutex<Option<(u64, u64)>> = Mutex::new(None);

pub fn set_active(who: &str, on: bool) {
    let bit = match who {
        "bar" => 1,
        _ => 2,
    };
    if on {
        ACTIVE.fetch_or(bit, Ordering::SeqCst);
    } else {
        ACTIVE.fetch_and(!bit, Ordering::SeqCst);
    }
}

fn cpu_times() -> Option<(u64, u64)> {
    let s = std::fs::read_to_string("/proc/stat").ok()?;
    let l = s.lines().next()?;
    let v: Vec<u64> = l.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if v.len() < 4 {
        return None;
    }
    let idle = v[3] + v.get(4).copied().unwrap_or(0);
    Some((v.iter().sum(), idle))
}

pub fn sample() -> SysStat {
    let mut s = SysStat::default();
    if let Some((total, idle)) = cpu_times() {
        let mut last = LAST.lock().unwrap();
        if let Some((pt, pi)) = *last {
            let dt = total.saturating_sub(pt) as f64;
            let di = idle.saturating_sub(pi) as f64;
            if dt > 0.0 {
                s.cpu = ((1.0 - di / dt) * 100.0).clamp(0.0, 100.0);
            }
        }
        *last = Some((total, idle));
    }
    if let Ok(m) = std::fs::read_to_string("/proc/meminfo") {
        let get = |k: &str| m.lines().find(|l| l.starts_with(k)).and_then(|l| l.split_whitespace().nth(1)).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
        let total = get("MemTotal:");
        let avail = get("MemAvailable:");
        s.mem_total_mb = total / 1024;
        s.mem_used_mb = total.saturating_sub(avail) / 1024;
    }
    unsafe {
        let mut st: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c"/".as_ptr(), &mut st) == 0 {
            let bs = st.f_frsize as f64;
            s.disk_total_gb = st.f_blocks as f64 * bs / 1e9;
            s.disk_used_gb = (st.f_blocks - st.f_bfree) as f64 * bs / 1e9;
        }
    }
    // Hottest of the CPU-ish thermal zones / hwmon sensors.
    let mut best: Option<f64> = None;
    if let Ok(rd) = std::fs::read_dir("/sys/class/hwmon") {
        for e in rd.flatten() {
            let name = std::fs::read_to_string(e.path().join("name")).unwrap_or_default();
            if !["coretemp", "k10temp", "zenpower", "cpu_thermal", "acpitz"].contains(&name.trim()) {
                continue;
            }
            if let Ok(t) = std::fs::read_to_string(e.path().join("temp1_input")) {
                if let Ok(v) = t.trim().parse::<f64>() {
                    best = Some(best.unwrap_or(0.0).max(v / 1000.0));
                }
            }
        }
    }
    s.temp_c = best;
    s
}

pub fn sample_now(tx: Sender<Msg>) {
    std::thread::spawn(move || {
        let _ = sample();
        std::thread::sleep(Duration::from_millis(250));
        let _ = tx.send(Msg::Sys(sample()));
    });
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("sysstat".into())
        .stack_size(64 * 1024)
        .spawn(move || loop {
            std::thread::sleep(Duration::from_secs(2));
            if ACTIVE.load(Ordering::SeqCst) != 0 && tx.send(Msg::Sys(sample())).is_err() {
                return;
            }
        })
        .ok();
}
