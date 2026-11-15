//! Local time via libc (honours TZ and /etc/localtime without pulling in a date crate).

use std::ffi::CString;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tm {
    pub year: i32,
    pub month: u32, // 1-12
    pub day: u32,
    pub hour: u32,
    pub min: u32,
    pub sec: u32,
    pub wday: u32, // 0 = Sunday
}

pub fn now_secs() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

extern "C" {
    fn tzset();
}

fn local_tm(t: i64) -> libc::tm {
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    let tt = t as libc::time_t;
    unsafe {
        tzset();
        libc::localtime_r(&tt, &mut tm);
    }
    tm
}

pub fn local(t: i64) -> Tm {
    let tm = local_tm(t);
    Tm {
        year: tm.tm_year + 1900,
        month: tm.tm_mon as u32 + 1,
        day: tm.tm_mday as u32,
        hour: tm.tm_hour as u32,
        min: tm.tm_min as u32,
        sec: tm.tm_sec as u32,
        wday: tm.tm_wday as u32,
    }
}

/// strftime with the user's locale.
pub fn format(t: i64, fmt: &str) -> String {
    let tm = local_tm(t);
    let Ok(f) = CString::new(fmt) else { return String::new() };
    let mut buf = vec![0u8; 256];
    let n = unsafe { libc::strftime(buf.as_mut_ptr() as *mut libc::c_char, buf.len(), f.as_ptr(), &tm) };
    buf.truncate(n);
    String::from_utf8_lossy(&buf).to_string()
}

pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 => 29,
        _ => 28,
    }
}

/// Day of week (0 = Sunday) via Sakamoto's algorithm.
pub fn weekday(year: i32, month: u32, day: u32) -> u32 {
    const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
    let y = if month < 3 { year - 1 } else { year };
    ((y + y / 4 - y / 100 + y / 400 + T[(month - 1) as usize] + day as i32) % 7) as u32
}

/// Adds `delta` months to (year, month).
pub fn add_months(year: i32, month: u32, delta: i32) -> (i32, u32) {
    let m0 = year * 12 + month as i32 - 1 + delta;
    (m0.div_euclid(12), (m0.rem_euclid(12) + 1) as u32)
}

pub const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_math() {
        assert_eq!(weekday(2026, 11, 15), 0); // Sunday
        assert_eq!(weekday(2000, 1, 1), 6);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
        assert_eq!(add_months(2026, 1, -1), (2025, 12));
        assert_eq!(add_months(2026, 12, 1), (2027, 1));
        assert_eq!(add_months(2026, 5, -17), (2024, 12));
    }
    #[test]
    fn strftime_works() {
        std::env::set_var("TZ", "UTC");
        assert_eq!(format(0, "%Y-%m-%d %H:%M"), "1970-01-01 00:00");
        assert_eq!(local(86400 * 365).year, 1971);
    }
}
