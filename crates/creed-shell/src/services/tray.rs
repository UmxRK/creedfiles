//! StatusNotifierItem tray: acts as the watcher (or as a host of an existing watcher),
//! reads item properties and renders DBusMenu context menus.

use crate::app::Msg;
use crate::data::{MenuEntry, TrayItem};
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use zbus::blocking::{Connection, MessageIterator};
use zbus::proxy::CacheProperties;
use zbus::zvariant::{OwnedValue, Value};

type Items = Arc<Mutex<Vec<String>>>;

struct Watcher {
    items: Items,
    dirty: Arc<Mutex<Option<Instant>>>,
}

fn item_key(service: &str, sender: &str) -> String {
    if service.starts_with('/') {
        format!("{sender}{service}")
    } else if service.contains('/') {
        service.to_string()
    } else {
        format!("{service}/StatusNotifierItem")
    }
}

fn split_key(key: &str) -> (&str, &str) {
    match key.find('/') {
        Some(i) => (&key[..i], &key[i..]),
        None => (key, "/StatusNotifierItem"),
    }
}

#[zbus::interface(name = "org.kde.StatusNotifierWatcher")]
impl Watcher {
    fn register_status_notifier_item(&self, service: &str, #[zbus(header)] hdr: zbus::message::Header<'_>) {
        let sender = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let key = item_key(service, &sender);
        let mut items = self.items.lock().unwrap();
        if !items.contains(&key) {
            items.push(key);
        }
        *self.dirty.lock().unwrap() = Some(Instant::now());
    }

    fn register_status_notifier_host(&self, _service: &str) {}

    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> Vec<String> {
        self.items.lock().unwrap().clone()
    }

    #[zbus(property)]
    fn is_status_notifier_host_registered(&self) -> bool {
        true
    }

    #[zbus(property)]
    fn protocol_version(&self) -> i32 {
        0
    }
}

fn item_proxy<'a>(conn: &'a Connection, key: &'a str, iface: &'a str) -> Option<zbus::blocking::Proxy<'a>> {
    let (bus, path) = split_key(key);
    zbus::blocking::proxy::Builder::new(conn)
        .destination(bus)
        .ok()?
        .path(path)
        .ok()?
        .interface(iface)
        .ok()?
        .cache_properties(CacheProperties::No)
        .build()
        .ok()
}

fn read_item(conn: &Connection, key: &str) -> Option<TrayItem> {
    let p = item_proxy(conn, key, "org.kde.StatusNotifierItem")?;
    let status: String = p.get_property("Status").unwrap_or_else(|_| "Active".into());
    if status == "Passive" {
        return None;
    }
    let id: String = p.get_property("Id").unwrap_or_default();
    let title: String = p.get_property("Title").unwrap_or_default();
    let icon: String = p.get_property("IconName").unwrap_or_default();
    let pixmap = if icon.is_empty() {
        p.get_property::<Vec<(i32, i32, Vec<u8>)>>("IconPixmap")
            .ok()
            .and_then(|v| v.into_iter().min_by_key(|(w, _, _)| (w - 24).abs()))
    } else {
        None
    };
    let menu: Option<String> = p.get_property::<zbus::zvariant::OwnedObjectPath>("Menu").ok().map(|o| o.to_string()).filter(|m| m != "/");
    let item_is_menu: bool = p.get_property("ItemIsMenu").unwrap_or(false);
    Some(TrayItem { key: key.to_string(), id, title, icon, pixmap, menu, item_is_menu })
}

fn publish(conn: &Connection, items: &Items, tx: &Sender<Msg>) -> bool {
    let keys = items.lock().unwrap().clone();
    let list: Vec<TrayItem> = keys.iter().filter_map(|k| read_item(conn, k)).collect();
    tx.send(Msg::Tray(list)).is_ok()
}

pub fn spawn(tx: Sender<Msg>) {
    std::thread::Builder::new()
        .name("tray".into())
        .stack_size(256 * 1024)
        .spawn(move || {
            let items: Items = Arc::new(Mutex::new(vec![]));
            let dirty = Arc::new(Mutex::new(None));
            let watcher = Watcher { items: items.clone(), dirty: dirty.clone() };
            let own = zbus::blocking::connection::Builder::session()
                .and_then(|b| b.name("org.kde.StatusNotifierWatcher"))
                .and_then(|b| b.serve_at("/StatusNotifierWatcher", watcher))
                .and_then(|b| b.build());
            let conn = match own {
                Ok(c) => c,
                Err(_) => {
                    // Someone else is the watcher: be a host.
                    let Ok(c) = Connection::session() else { return };
                    let host = format!("org.kde.StatusNotifierHost-{}", std::process::id());
                    let _ = c.request_name(host.as_str());
                    let _ = c.call_method(Some("org.kde.StatusNotifierWatcher"), "/StatusNotifierWatcher", Some("org.kde.StatusNotifierWatcher"), "RegisterStatusNotifierHost", &(host.as_str(),));
                    if let Ok(p) = zbus::blocking::Proxy::new(&c, "org.kde.StatusNotifierWatcher", "/StatusNotifierWatcher", "org.kde.StatusNotifierWatcher") {
                        if let Ok(v) = p.get_property::<Vec<String>>("RegisteredStatusNotifierItems") {
                            *items.lock().unwrap() = v.iter().map(|s| item_key(s, "")).collect();
                        }
                    }
                    c
                }
            };
            // Debounced publisher.
            {
                let conn = conn.clone();
                let items = items.clone();
                let dirty = dirty.clone();
                let tx = tx.clone();
                std::thread::Builder::new().name("tray-pub".into()).stack_size(256 * 1024).spawn(move || loop {
                    std::thread::sleep(Duration::from_millis(150));
                    let due = { dirty.lock().unwrap().map(|t| t.elapsed() > Duration::from_millis(120)).unwrap_or(false) };
                    if due {
                        *dirty.lock().unwrap() = None;
                        if !publish(&conn, &items, &tx) {
                            return;
                        }
                    }
                }).ok();
            }
            *dirty.lock().unwrap() = Some(Instant::now());
            // Listen for item signals and bus owner changes.
            let rule = zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).build();
            let Ok(it) = MessageIterator::for_match_rule(rule, &conn, Some(64)) else { return };
            for msg in it.flatten() {
                let h = msg.header();
                let iface = h.interface().map(|i| i.to_string()).unwrap_or_default();
                let member = h.member().map(|m| m.to_string()).unwrap_or_default();
                if iface == "org.kde.StatusNotifierItem" {
                    *dirty.lock().unwrap() = Some(Instant::now());
                } else if iface == "org.freedesktop.DBus" && member == "NameOwnerChanged" {
                    if let Ok((name, _old, new)) = msg.body().deserialize::<(String, String, String)>() {
                        if new.is_empty() {
                            let mut v = items.lock().unwrap();
                            let before = v.len();
                            v.retain(|k| split_key(k).0 != name);
                            if v.len() != before {
                                *dirty.lock().unwrap() = Some(Instant::now());
                            }
                        }
                    }
                } else if iface == "org.kde.StatusNotifierWatcher" && member == "StatusNotifierItemRegistered" {
                    if let Ok(s) = msg.body().deserialize::<String>() {
                        let key = item_key(&s, "");
                        let mut v = items.lock().unwrap();
                        if !v.contains(&key) {
                            v.push(key);
                        }
                        *dirty.lock().unwrap() = Some(Instant::now());
                    }
                } else if iface == "org.kde.StatusNotifierWatcher" && member == "StatusNotifierItemUnregistered" {
                    if let Ok(s) = msg.body().deserialize::<String>() {
                        let key = item_key(&s, "");
                        items.lock().unwrap().retain(|k| *k != key);
                        *dirty.lock().unwrap() = Some(Instant::now());
                    }
                }
            }
        })
        .ok();
}

pub fn activate(key: &str, secondary: bool) {
    let key = key.to_string();
    std::thread::spawn(move || {
        let Some(conn) = super::session_bus() else { return };
        let (bus, path) = split_key(&key);
        let method = if secondary { "SecondaryActivate" } else { "Activate" };
        if conn.call_method(Some(bus), path, Some("org.kde.StatusNotifierItem"), method, &(0i32, 0i32)).is_err() && !secondary {
            let _ = conn.call_method(Some(bus), path, Some("org.kde.StatusNotifierItem"), "ContextMenu", &(0i32, 0i32));
        }
    });
}

type Layout = (i32, HashMap<String, OwnedValue>, Vec<OwnedValue>);

fn entries_from(children: &[OwnedValue], depth: usize, out: &mut Vec<MenuEntry>) {
    for c in children {
        let Ok(v) = c.try_clone() else { continue };
        let node: Layout = match Value::from(v) {
            Value::Value(inner) => match <Layout>::try_from(*inner) {
                Ok(l) => l,
                Err(_) => continue,
            },
            other => match <Layout>::try_from(other) {
                Ok(l) => l,
                Err(_) => continue,
            },
        };
        let (id, props, kids) = node;
        let s = |k: &str| props.get(k).and_then(|v| String::try_from(v.try_clone().ok()?).ok());
        let b = |k: &str| props.get(k).and_then(|v| bool::try_from(v).ok());
        if b("visible") == Some(false) {
            continue;
        }
        let separator = s("type").as_deref() == Some("separator");
        let toggle = if s("toggle-type").map(|t| !t.is_empty()).unwrap_or(false) {
            props.get("toggle-state").and_then(|v| i32::try_from(v).ok()).map(|v| v == 1)
        } else {
            None
        };
        let label = s("label").unwrap_or_default();
        let has_kids = !kids.is_empty() && s("children-display").as_deref() == Some("submenu");
        out.push(MenuEntry {
            id,
            label: format!("{}{}", "   ".repeat(depth), label),
            enabled: b("enabled").unwrap_or(true) && !has_kids,
            separator,
            toggle,
        });
        if has_kids && depth < 2 {
            entries_from(&kids, depth + 1, out);
        }
    }
}

pub fn fetch_menu(item: &TrayItem, tx: &Sender<Msg>) {
    let Some(menu) = item.menu.clone() else { return };
    let (bus, _) = split_key(&item.key);
    let bus = bus.to_string();
    let owner = item.key.clone();
    let tx = tx.clone();
    std::thread::spawn(move || {
        let Some(conn) = super::session_bus() else { return };
        let _ = conn.call_method(Some(bus.as_str()), menu.as_str(), Some("com.canonical.dbusmenu"), "AboutToShow", &(0i32,));
        let reply = conn.call_method(Some(bus.as_str()), menu.as_str(), Some("com.canonical.dbusmenu"), "GetLayout", &(0i32, -1i32, Vec::<String>::new()));
        let mut entries = vec![];
        if let Ok(r) = reply {
            if let Ok((_rev, (_id, _props, children))) = r.body().deserialize::<(u32, Layout)>() {
                entries_from(&children, 0, &mut entries);
            }
        }
        // Trim leading/trailing/double separators.
        let mut clean: Vec<MenuEntry> = vec![];
        for e in entries {
            if e.separator && clean.last().map(|l: &MenuEntry| l.separator).unwrap_or(true) {
                continue;
            }
            clean.push(e);
        }
        while clean.last().map(|l| l.separator).unwrap_or(false) {
            clean.pop();
        }
        let _ = tx.send(Msg::Menu(owner, clean));
    });
}

pub fn menu_event(item: &TrayItem, id: i32) {
    let Some(menu) = item.menu.clone() else { return };
    let bus = split_key(&item.key).0.to_string();
    std::thread::spawn(move || {
        if let Some(conn) = super::session_bus() {
            let ts = crate::time::now_secs() as u32;
            let _ = conn.call_method(Some(bus.as_str()), menu.as_str(), Some("com.canonical.dbusmenu"), "Event", &(id, "clicked", Value::from(0i32), ts));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn keys() {
        assert_eq!(item_key(":1.42", ""), ":1.42/StatusNotifierItem");
        assert_eq!(item_key("/org/ayatana/NotificationItem/x", ":1.9"), ":1.9/org/ayatana/NotificationItem/x");
        assert_eq!(split_key(":1.9/org/ayatana/x"), (":1.9", "/org/ayatana/x"));
        assert_eq!(split_key("org.kde.StatusNotifierItem-5-1/StatusNotifierItem"), ("org.kde.StatusNotifierItem-5-1", "/StatusNotifierItem"));
    }
}
