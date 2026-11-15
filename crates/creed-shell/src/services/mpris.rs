//! MPRIS media players on the session bus.

use crate::app::Msg;
use crate::data::Media;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use zbus::blocking::{Connection, MessageIterator, Proxy};
use zbus::proxy::CacheProperties;
use zbus::zvariant::OwnedValue;

fn proxy<'a>(conn: &'a Connection, bus: &'a str, iface: &'a str) -> Option<Proxy<'a>> {
    zbus::blocking::proxy::Builder::new(conn)
        .destination(bus)
        .ok()?
        .path("/org/mpris/MediaPlayer2")
        .ok()?
        .interface(iface)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .ok()
}

fn read(conn: &Connection, bus: &str) -> Option<Media> {
    let p = proxy(conn, bus, "org.mpris.MediaPlayer2.Player")?;
    let status: String = p.get_property("PlaybackStatus").ok()?;
    let md: HashMap<String, OwnedValue> = p.get_property("Metadata").unwrap_or_default();
    let title = md.get("xesam:title").and_then(|v| String::try_from(v.try_clone().ok()?).ok()).unwrap_or_default();
    let artist = md
        .get("xesam:artist")
        .and_then(|v| <Vec<String>>::try_from(v.try_clone().ok()?).ok())
        .map(|a| a.join(", "))
        .unwrap_or_default();
    let identity = proxy(conn, bus, "org.mpris.MediaPlayer2").and_then(|r| r.get_property::<String>("Identity").ok()).unwrap_or_else(|| bus.trim_start_matches("org.mpris.MediaPlayer2.").to_string());
    Some(Media {
        bus: bus.to_string(),
        player: identity,
        title,
        artist,
        playing: status == "Playing",
        can_next: p.get_property("CanGoNext").unwrap_or(true),
        can_prev: p.get_property("CanGoPrevious").unwrap_or(true),
    })
}

fn current(conn: &Connection) -> Option<Media> {
    let dbus = zbus::blocking::fdo::DBusProxy::new(conn).ok()?;
    let names = dbus.list_names().ok()?;
    let players: Vec<Media> = names.iter().map(|n| n.as_str()).filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).filter_map(|n| read(conn, n)).filter(|m| !m.title.is_empty()).collect();
    players.iter().find(|m| m.playing).cloned().or_else(|| players.into_iter().next())
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("mpris".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let Ok(conn) = Connection::session() else { return };
            if tx.send(Msg::Media(current(&conn))).is_err() {
                return;
            }
            let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).path("/org/mpris/MediaPlayer2").map(|b| b.build());
            let owner_rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .interface("org.freedesktop.DBus")
                .and_then(|b| b.member("NameOwnerChanged"))
                .and_then(|b| b.arg0ns("org.mpris.MediaPlayer2"))
                .map(|b| b.build());
            // Second listener for players appearing/disappearing.
            if let Ok(rule2) = owner_rule {
                let tx2 = tx.clone();
                let conn2 = conn.clone();
                std::thread::Builder::new().name("mpris-owner".into()).stack_size(128 * 1024).spawn(move || {
                    if let Ok(it) = MessageIterator::for_match_rule(rule2, &conn2, Some(8)) {
                        for _ in it {
                            if tx2.send(Msg::Media(current(&conn2))).is_err() {
                                return;
                            }
                        }
                    }
                }).ok();
            }
            let Ok(rule) = rule else { return };
            let Ok(it) = MessageIterator::for_match_rule(rule, &conn, Some(32)) else { return };
            for _ in it {
                if tx.send(Msg::Media(current(&conn))).is_err() {
                    return;
                }
            }
        })
        .ok();
}

pub fn call(media: &Option<Media>, method: &'static str) {
    let Some(m) = media.clone() else { return };
    std::thread::spawn(move || {
        if let Some(conn) = super::session_bus() {
            let _ = conn.call_method(Some(m.bus.as_str()), "/org/mpris/MediaPlayer2", Some("org.mpris.MediaPlayer2.Player"), method, &());
        }
    });
}
