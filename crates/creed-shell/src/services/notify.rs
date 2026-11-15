//! org.freedesktop.Notifications server.

use crate::app::Msg;
use crate::data::Notif;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use zbus::zvariant::OwnedValue;

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

struct Server {
    tx: Sender<Msg>,
}

#[zbus::interface(name = "org.freedesktop.Notifications")]
impl Server {
    #[allow(clippy::too_many_arguments)]
    fn notify(
        &self,
        app_name: String,
        replaces_id: u32,
        app_icon: String,
        summary: String,
        body: String,
        actions: Vec<String>,
        hints: HashMap<String, OwnedValue>,
        expire_timeout: i32,
    ) -> u32 {
        let id = if replaces_id > 0 { replaces_id } else { NEXT_ID.fetch_add(1, Ordering::SeqCst) };
        let urgency = hints.get("urgency").and_then(|v| u8::try_from(v).ok()).unwrap_or(1);
        let image = hints
            .get("image-path")
            .or_else(|| hints.get("image_path"))
            .and_then(|v| String::try_from(v.try_clone().ok()?).ok())
            .map(|s| s.strip_prefix("file://").unwrap_or(&s).to_string());
        let desktop = hints.get("desktop-entry").and_then(|v| String::try_from(v.try_clone().ok()?).ok());
        let icon = if !app_icon.is_empty() {
            app_icon.strip_prefix("file://").unwrap_or(&app_icon).to_string()
        } else {
            image.or(desktop).unwrap_or_default()
        };
        let pairs: Vec<(String, String)> = actions.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect();
        let _ = self.tx.send(Msg::Notify(Notif {
            id,
            app: app_name,
            icon,
            summary,
            body,
            actions: pairs,
            urgency,
            timeout_ms: expire_timeout,
            arrived: crate::time::now_secs(),
        }));
        id
    }

    fn close_notification(&self, id: u32) {
        let _ = self.tx.send(Msg::NotifClose(id));
    }

    fn get_capabilities(&self) -> Vec<String> {
        ["actions", "body", "body-markup", "icon-static", "persistence"].iter().map(|s| s.to_string()).collect()
    }

    fn get_server_information(&self) -> (String, String, String, String) {
        ("Creed".into(), "Creed".into(), env!("CARGO_PKG_VERSION").into(), "1.2".into())
    }
}

/// Handle used by the main thread to emit signals.
pub struct Handle {
    conn: zbus::blocking::Connection,
}

impl Handle {
    pub fn closed(&self, id: u32, reason: u32) {
        let _ = self.conn.emit_signal(None::<()>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "NotificationClosed", &(id, reason));
    }
    pub fn action(&self, id: u32, key: &str) {
        let _ = self.conn.emit_signal(None::<()>, "/org/freedesktop/Notifications", "org.freedesktop.Notifications", "ActionInvoked", &(id, key));
    }
}

pub fn start(tx: Sender<Msg>) -> Option<Handle> {
    let conn = zbus::blocking::connection::Builder::session()
        .ok()?
        .name("org.freedesktop.Notifications")
        .ok()?
        .serve_at("/org/freedesktop/Notifications", Server { tx })
        .ok()?
        .build();
    match conn {
        Ok(c) => Some(Handle { conn: c }),
        Err(e) => {
            eprintln!("creed-shell: notification server not started ({e}); another daemon may own the name");
            None
        }
    }
}
