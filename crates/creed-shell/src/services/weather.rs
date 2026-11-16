//! Weather from wttr.in (no API key). Fetched at start and every 30 minutes.

use crate::app::Msg;
use crate::data::WeatherNow;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::time::Duration;

pub fn icon_for(code: u32, night: bool) -> &'static str {
    match code {
        113 => if night { "weather-clear-night-symbolic" } else { "weather-clear-symbolic" },
        116 => if night { "weather-few-clouds-night-symbolic" } else { "weather-few-clouds-symbolic" },
        119 | 122 => "weather-overcast-symbolic",
        143 | 248 | 260 => "weather-fog-symbolic",
        176 | 263 | 266 | 293 | 296 | 299 | 302 | 305 | 308 | 311 | 314 | 353 | 356 | 359 => "weather-showers-symbolic",
        179 | 182 | 185 | 227 | 230 | 281 | 284 | 317 | 320 | 323 | 326 | 329 | 332 | 335 | 338 | 350 | 362 | 365 | 368 | 371 | 374 | 377 => "weather-snow-symbolic",
        200 | 386 | 389 | 392 | 395 => "weather-storm-symbolic",
        _ => "weather-overcast-symbolic",
    }
}

pub fn parse(json: &str, metric: bool) -> Option<WeatherNow> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let cur = v.get("current_condition")?.get(0)?;
    let temp = cur.get(if metric { "temp_C" } else { "temp_F" })?.as_str()?;
    let desc = cur.get("weatherDesc")?.get(0)?.get("value")?.as_str()?.to_string();
    let code: u32 = cur.get("weatherCode")?.as_str()?.parse().ok()?;
    let place = v.get("nearest_area").and_then(|a| a.get(0)).and_then(|a| a.get("areaName")).and_then(|a| a.get(0)).and_then(|a| a.get("value")).and_then(|a| a.as_str()).unwrap_or("").to_string();
    let today = v.get("weather").and_then(|w| w.get(0));
    let hi_lo = today
        .and_then(|d| {
            let (hk, lk) = if metric { ("maxtempC", "mintempC") } else { ("maxtempF", "mintempF") };
            Some(format!("H {}° L {}°", d.get(hk)?.as_str()?, d.get(lk)?.as_str()?))
        })
        .unwrap_or_default();
    let hour = crate::time::local(crate::time::now_secs()).hour;
    // wttr.in gives 3-hourly slots per day ("0", "300", … "2100"); take the next six from now.
    let mut hourly = vec![];
    if let Some(days) = v.get("weather").and_then(|w| w.as_array()) {
        for (di, d) in days.iter().enumerate().take(2) {
            for h in d.get("hourly").and_then(|h| h.as_array()).into_iter().flatten() {
                let t: u32 = h.get("time").and_then(|t| t.as_str()).and_then(|t| t.parse().ok()).unwrap_or(0) / 100;
                if di == 0 && t <= hour {
                    continue;
                }
                let tk = if metric { "tempC" } else { "tempF" };
                let (Some(tv), Some(c)) = (h.get(tk).and_then(|x| x.as_str()), h.get("weatherCode").and_then(|x| x.as_str()).and_then(|x| x.parse::<u32>().ok())) else { continue };
                if hourly.len() < 6 {
                    hourly.push((t, format!("{tv}°"), icon_for(c, !(6..19).contains(&t)).to_string()));
                }
            }
        }
    }
    Some(WeatherNow { temp: format!("{temp}°"), desc, icon: icon_for(code, !(6..19).contains(&hour)).into(), place, hi_lo, hourly })
}

pub fn spawn(tx: Sender<Msg>, location: String, metric: bool) {
    if !super::have("curl") {
        return;
    }
    let loc: String = location.chars().filter(|c| c.is_alphanumeric() || " ,-+.".contains(*c)).collect::<String>().replace(' ', "+");
    std::thread::Builder::new()
        .name("weather".into())
        .stack_size(128 * 1024)
        .spawn(move || loop {
            let url = format!("https://wttr.in/{loc}?format=j1");
            let w = super::output("curl", &["-fsS", "--max-time", "10", "-A", "creed-shell", &url]).and_then(|j| parse(&j, metric));
            if tx.send(Msg::Weather(w)).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_secs(1800));
        })
        .ok();
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_wttr() {
        let j = r#"{"current_condition":[{"temp_C":"12","temp_F":"54","weatherCode":"116","weatherDesc":[{"value":"Partly cloudy"}]}],
            "nearest_area":[{"areaName":[{"value":"Leeds"}]}],"weather":[{"maxtempC":"14","mintempC":"6",
            "hourly":[{"time":"2100","tempC":"9","tempF":"48","weatherCode":"113"}]},
            {"maxtempC":"13","mintempC":"5","hourly":[{"time":"0","tempC":"8","tempF":"46","weatherCode":"116"},{"time":"300","tempC":"7","tempF":"45","weatherCode":"296"}]}]}"#;
        let w = super::parse(j, true).unwrap();
        assert_eq!(w.temp, "12°");
        assert_eq!(w.place, "Leeds");
        assert_eq!(w.hi_lo, "H 14° L 6°");
        assert!(w.icon.starts_with("weather-few-clouds"));
        // Tomorrow's slots always follow; today's 21:00 only when it is still ahead.
        assert!(w.hourly.iter().any(|(h, t, i)| *h == 3 && t == "7°" && i == "weather-showers-symbolic"));
        assert!(w.hourly.len() <= 6);
        assert!(super::parse("{}", true).is_none());
    }
}
