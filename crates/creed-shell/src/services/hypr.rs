//! Hyprland event socket → messages. Reconnects if Hyprland restarts.

use crate::app::Msg;
use creed_core::hypr::{Hypr, HyprEvent};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::{BufRead, BufReader};
use std::time::Duration;

pub fn spawn(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("hypr-events".into())
        .stack_size(64 * 1024)
        .spawn(move || loop {
            if let Some(h) = Hypr::from_env() {
                if let Ok(sock) = h.event_socket() {
                    // Initial state.
                    if tx.send(Msg::Hypr(HyprEvent::ConfigReloaded)).is_err() {
                        return;
                    }
                    let reader = BufReader::new(sock);
                    for line in reader.lines() {
                        let Ok(line) = line else { break };
                        if let Some(ev) = HyprEvent::parse(&line) {
                            if tx.send(Msg::Hypr(ev)).is_err() {
                                return;
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        })
        .ok();
}
