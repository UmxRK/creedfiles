//! Volume via PipeWire's wpctl; change notifications via `pactl subscribe`.

use crate::app::Msg;
use crate::data::Audio;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

fn read(target: &str) -> Option<Audio> {
    let out = super::output("wpctl", &["get-volume", target])?;
    parse(&out)
}

/// Parses `Volume: 0.45 [MUTED]`.
pub fn parse(s: &str) -> Option<Audio> {
    let rest = s.strip_prefix("Volume:")?.trim();
    let v: f64 = rest.split_whitespace().next()?.parse().ok()?;
    Some(Audio { volume: v.clamp(0.0, 1.5), muted: rest.contains("[MUTED]") })
}

pub fn refresh(tx: &Sender<Msg>) {
    let _ = tx.send(Msg::Audio(read("@DEFAULT_AUDIO_SINK@")));
    let _ = tx.send(Msg::Mic(read("@DEFAULT_AUDIO_SOURCE@")));
}

pub fn spawn(tx: Sender<Msg>) {
    if !super::have("wpctl") {
        return;
    }
    refresh(&tx);
    if !super::have("pactl") {
        return;
    }
    std::thread::Builder::new()
        .name("audio".into())
        .stack_size(64 * 1024)
        .spawn(move || loop {
            let child = Command::new("pactl").arg("subscribe").stdout(Stdio::piped()).stderr(Stdio::null()).spawn();
            let Ok(mut child) = child else { return };
            let Some(out) = child.stdout.take() else { return };
            for line in BufReader::new(out).lines().map_while(Result::ok) {
                if line.contains("on sink") || line.contains("on source") || line.contains("on server") {
                    refresh(&tx);
                }
            }
            let _ = child.wait();
            std::thread::sleep(std::time::Duration::from_secs(3));
        })
        .ok();
}

pub fn set_volume(v: f64, mic: bool) {
    let target = if mic { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" };
    let v = format!("{:.2}", v.clamp(0.0, 1.0));
    let _ = Command::new("wpctl").args(["set-volume", target, &v]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
    let _ = Command::new("wpctl").args(["set-mute", target, "0"]).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|mut c| std::thread::spawn(move || c.wait()));
}

pub fn toggle_mute(mic: bool) {
    let target = if mic { "@DEFAULT_AUDIO_SOURCE@" } else { "@DEFAULT_AUDIO_SINK@" };
    super::spawn_shell(&format!("wpctl set-mute {target} toggle"));
}

pub fn step(d: f64) {
    let pct = (d.abs() * 100.0).round() as i32;
    let sign = if d > 0.0 { "+" } else { "-" };
    super::spawn_shell(&format!("wpctl set-volume -l 1 @DEFAULT_AUDIO_SINK@ {pct}%{sign}"));
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_wpctl() {
        let a = super::parse("Volume: 0.45").unwrap();
        assert!((a.volume - 0.45).abs() < 1e-9 && !a.muted);
        assert!(super::parse("Volume: 1.00 [MUTED]").unwrap().muted);
        assert!(super::parse("garbage").is_none());
    }
}
