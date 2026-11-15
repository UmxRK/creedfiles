//! Clipboard history through cliphist + wl-clipboard.

use crate::app::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::Write;
use std::process::{Command, Stdio};

pub fn refresh(tx: Sender<Msg>) {
    if !super::have("cliphist") {
        let _ = tx.send(Msg::Clips(vec![]));
        return;
    }
    std::thread::spawn(move || {
        let list = super::output("cliphist", &["list"]).unwrap_or_default();
        let items: Vec<(String, String)> = list
            .lines()
            .take(200)
            .filter_map(|l| l.split_once('\t').map(|(id, t)| (id.to_string(), t.to_string())))
            .collect();
        let _ = tx.send(Msg::Clips(items));
    });
}

pub fn pick(id: &str) {
    if !id.chars().all(|c| c.is_ascii_digit()) {
        return;
    }
    super::spawn_shell(&format!("cliphist list | grep -m1 '^{id}\t' | cliphist decode | wl-copy"));
}

pub fn copy_text(t: &str) {
    if let Ok(mut c) = Command::new("wl-copy").stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
        if let Some(mut si) = c.stdin.take() {
            let _ = si.write_all(t.as_bytes());
        }
        std::thread::spawn(move || c.wait());
    }
}
