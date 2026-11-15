//! Hardware detection from sysfs/procfs. The root is injectable for tests.

use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Gpu {
    /// DRM card node name, e.g. `card1`.
    pub card: String,
    pub vendor: GpuVendor,
    pub driver: String,
    pub pci: String,
    pub boot_vga: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Hardware {
    pub gpus: Vec<Gpu>,
    pub cpu_vendor: String,
    pub cpu_model: String,
    pub cores: usize,
    pub ram_mb: u64,
    pub is_laptop: bool,
    pub has_battery: bool,
    pub wifi: Vec<String>,
    pub root_btrfs: bool,
    pub has_snapper: bool,
    pub has_ssd: bool,
    pub has_hdd: bool,
    pub kernel: String,
    pub ntsync: bool,
}

fn read(p: impl AsRef<Path>) -> String {
    fs::read_to_string(p).map(|s| s.trim().to_string()).unwrap_or_default()
}

fn dir(p: impl AsRef<Path>) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(p).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
    v.sort();
    v
}

impl Hardware {
    pub fn detect() -> Hardware {
        Self::detect_at(Path::new("/"))
    }

    pub fn detect_at(root: &Path) -> Hardware {
        let r = |s: &str| root.join(s.trim_start_matches('/'));

        let mut gpus = vec![];
        for card in dir(r("/sys/class/drm")) {
            let name = card.file_name().unwrap().to_string_lossy().to_string();
            if !name.starts_with("card") || name.contains('-') {
                continue;
            }
            let dev = card.join("device");
            let vendor = match read(dev.join("vendor")).as_str() {
                "0x10de" => GpuVendor::Nvidia,
                "0x1002" => GpuVendor::Amd,
                "0x8086" => GpuVendor::Intel,
                "" => continue,
                _ => GpuVendor::Other,
            };
            let driver = fs::read_link(dev.join("driver"))
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
                .unwrap_or_default();
            let pci = fs::read_link(&dev)
                .ok()
                .and_then(|p| p.file_name().map(|f| f.to_string_lossy().to_string()))
                .unwrap_or_else(|| name.clone());
            let boot_vga = read(dev.join("boot_vga")) == "1";
            if !gpus.iter().any(|g: &Gpu| g.pci == pci) {
                gpus.push(Gpu { card: name.clone(), vendor, driver, pci, boot_vga });
            }
        }

        let cpuinfo = read(r("/proc/cpuinfo"));
        let field = |k: &str| {
            cpuinfo
                .lines()
                .find(|l| l.starts_with(k))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_string())
                .unwrap_or_default()
        };
        let cores = cpuinfo.lines().filter(|l| l.starts_with("processor")).count().max(1);
        let ram_kb: u64 = read(r("/proc/meminfo"))
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        let has_battery = dir(r("/sys/class/power_supply")).iter().any(|p| read(p.join("type")) == "Battery");
        let chassis: u32 = read(r("/sys/class/dmi/id/chassis_type")).parse().unwrap_or(0);
        let is_laptop = has_battery || matches!(chassis, 8 | 9 | 10 | 14 | 31 | 32);

        let wifi = dir(r("/sys/class/net"))
            .iter()
            .filter(|p| p.join("wireless").exists() || p.join("phy80211").exists())
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();

        let mounts = read(r("/proc/mounts"));
        let root_btrfs = mounts.lines().any(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            f.len() > 2 && f[1] == "/" && f[2] == "btrfs"
        });
        let has_snapper = r("/usr/bin/snapper").exists();

        let mut has_ssd = false;
        let mut has_hdd = false;
        for b in dir(r("/sys/block")) {
            let n = b.file_name().unwrap().to_string_lossy().to_string();
            if n.starts_with("loop") || n.starts_with("zram") || n.starts_with("ram") || n.starts_with("dm-") {
                continue;
            }
            match read(b.join("queue/rotational")).as_str() {
                "0" => has_ssd = true,
                "1" => has_hdd = true,
                _ => {}
            }
        }

        Hardware {
            gpus,
            cpu_vendor: field("vendor_id"),
            cpu_model: field("model name"),
            cores,
            ram_mb: ram_kb / 1024,
            is_laptop,
            has_battery,
            wifi,
            root_btrfs,
            has_snapper,
            has_ssd,
            has_hdd,
            kernel: read(r("/proc/sys/kernel/osrelease")),
            ntsync: r("/dev/ntsync").exists(),
        }
    }

    pub fn has_vendor(&self, v: GpuVendor) -> bool {
        self.gpus.iter().any(|g| g.vendor == v)
    }

    /// True when an NVIDIA GPU sits next to an integrated GPU (PRIME offload setup).
    pub fn is_hybrid(&self) -> bool {
        self.has_vendor(GpuVendor::Nvidia) && (self.has_vendor(GpuVendor::Intel) || self.has_vendor(GpuVendor::Amd))
            || (self.gpus.len() > 1 && self.has_vendor(GpuVendor::Amd) && self.has_vendor(GpuVendor::Intel))
    }

    /// The GPU that drives the desktop (boot VGA, else first).
    pub fn primary_gpu(&self) -> Option<&Gpu> {
        self.gpus.iter().find(|g| g.boot_vga).or(self.gpus.first())
    }

    /// Integrated-only machines get cheaper glass by default.
    pub fn weak_gpu(&self) -> bool {
        self.gpus.is_empty() || self.gpus.iter().all(|g| g.vendor == GpuVendor::Intel)
    }

    /// Predicate language used by tweaks: `laptop`, `desktop`, `nvidia`, `amd`, `intel`, `hybrid`,
    /// `wifi`, `btrfs`, `snapper`, `ssd`, `hdd`, `battery`, `ram<8192`, `ram>=16384`, `cpu:amd`,
    /// `cpu:intel`, `ntsync`, `always`. Prefix `!` negates. Comma-separated terms are ANDed.
    pub fn matches(&self, predicate: &str) -> bool {
        predicate.split(',').map(str::trim).filter(|t| !t.is_empty()).all(|term| {
            let (neg, t) = match term.strip_prefix('!') {
                Some(t) => (true, t),
                None => (false, term),
            };
            let v = if let Some(n) = t.strip_prefix("ram>=") {
                n.parse::<u64>().map(|n| self.ram_mb >= n).unwrap_or(false)
            } else if let Some(n) = t.strip_prefix("ram<") {
                n.parse::<u64>().map(|n| self.ram_mb < n).unwrap_or(false)
            } else {
                match t {
                    "always" => true,
                    "laptop" => self.is_laptop,
                    "desktop" => !self.is_laptop,
                    "battery" => self.has_battery,
                    "nvidia" => self.has_vendor(GpuVendor::Nvidia),
                    "amd" => self.has_vendor(GpuVendor::Amd),
                    "intel" => self.has_vendor(GpuVendor::Intel),
                    "hybrid" => self.is_hybrid(),
                    "wifi" => !self.wifi.is_empty(),
                    "btrfs" => self.root_btrfs,
                    "snapper" => self.has_snapper,
                    "ssd" => self.has_ssd,
                    "hdd" => self.has_hdd,
                    "ntsync" => self.ntsync,
                    "cpu:amd" => self.cpu_vendor == "AuthenticAMD",
                    "cpu:intel" => self.cpu_vendor == "GenuineIntel",
                    _ => false,
                }
            };
            v != neg
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    fn write(root: &Path, p: &str, s: &str) {
        let f = root.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, s).unwrap();
    }

    fn gpu(root: &Path, card: &str, pci: &str, vendor: &str, driver: &str, boot: &str) {
        let dev = root.join(format!("sys/devices/pci0000:00/{pci}"));
        fs::create_dir_all(&dev).unwrap();
        fs::write(dev.join("vendor"), vendor).unwrap();
        fs::write(dev.join("boot_vga"), boot).unwrap();
        let drv = root.join(format!("sys/bus/pci/drivers/{driver}"));
        fs::create_dir_all(&drv).unwrap();
        symlink(&drv, dev.join("driver")).unwrap();
        let c = root.join(format!("sys/class/drm/{card}"));
        fs::create_dir_all(&c).unwrap();
        symlink(&dev, c.join("device")).unwrap();
        // connector entries must be ignored
        fs::create_dir_all(root.join(format!("sys/class/drm/{card}-eDP-1"))).unwrap();
    }

    #[test]
    fn hybrid_laptop() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        gpu(r, "card0", "0000:00:02.0", "0x8086", "i915", "1");
        gpu(r, "card1", "0000:01:00.0", "0x10de", "nvidia", "0");
        write(r, "proc/cpuinfo", "processor\t: 0\nvendor_id\t: GenuineIntel\nmodel name\t: Intel(R) Core(TM) i7\nprocessor\t: 1\n");
        write(r, "proc/meminfo", "MemTotal:       16303104 kB\n");
        write(r, "sys/class/power_supply/BAT0/type", "Battery\n");
        write(r, "sys/class/net/wlan0/wireless/x", "");
        write(r, "proc/mounts", "/dev/nvme0n1p2 / btrfs rw 0 0\n");
        write(r, "sys/block/nvme0n1/queue/rotational", "0\n");
        let h = Hardware::detect_at(r);
        assert_eq!(h.gpus.len(), 2);
        assert!(h.is_hybrid());
        assert_eq!(h.primary_gpu().unwrap().vendor, GpuVendor::Intel);
        assert_eq!(h.gpus[1].driver, "nvidia");
        assert_eq!(h.gpus[1].card, "card1");
        assert!(h.is_laptop && h.has_battery && h.root_btrfs && h.has_ssd && !h.has_hdd);
        assert_eq!(h.cores, 2);
        assert_eq!(h.ram_mb, 15921);
        assert_eq!(h.wifi, vec!["wlan0".to_string()]);
        assert!(h.matches("laptop,nvidia,hybrid,wifi,ram>=8192,cpu:intel"));
        assert!(!h.matches("desktop"));
        assert!(h.matches("!desktop,!amd"));
        assert!(!h.matches("ram<8192"));
    }

    #[test]
    fn amd_desktop() {
        let t = tempfile::tempdir().unwrap();
        let r = t.path();
        gpu(r, "card1", "0000:03:00.0", "0x1002", "amdgpu", "1");
        write(r, "proc/cpuinfo", "processor\t: 0\nvendor_id\t: AuthenticAMD\n");
        write(r, "proc/meminfo", "MemTotal:       4000000 kB\n");
        write(r, "sys/class/dmi/id/chassis_type", "3\n");
        write(r, "proc/mounts", "/dev/sda2 / ext4 rw 0 0\n");
        write(r, "sys/block/sda/queue/rotational", "1\n");
        let h = Hardware::detect_at(r);
        assert!(!h.is_laptop && !h.is_hybrid() && !h.weak_gpu());
        assert!(h.matches("desktop,amd,cpu:amd,ram<8192,hdd,!btrfs,!wifi"));
    }

    #[test]
    fn empty_root_is_safe() {
        let t = tempfile::tempdir().unwrap();
        let h = Hardware::detect_at(t.path());
        assert!(h.gpus.is_empty() && h.weak_gpu() && h.cores == 1);
    }
}
