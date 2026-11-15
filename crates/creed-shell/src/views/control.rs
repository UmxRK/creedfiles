//! Control center / Quick Settings / Action Center.
//! Pages: 0 main, 1 Wi-Fi list, 2 Bluetooth, 3 power profiles.

use super::{open_amount, popup_anchor, popup_transform, Views};
use crate::data::{Data, NetKind};
use crate::gfx::{self, Align, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Slider, Ui};

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Control));
    let th = ui.th;
    let variant = th.cfg.style.control.clone();
    let w = match variant.as_str() {
        "win10" => 380.0,
        "mac" | "aqua" => 340.0,
        _ => 380.0,
    };
    let h = match views.page {
        0 => main_height(ui, data, &variant),
        _ => 460.0,
    };
    let align = match variant.as_str() {
        "creed" | "mac" | "aqua" | "win11" | "win10" => Align::End,
        _ => Align::End,
    };
    let mut panel = popup_anchor(ui, w, h, align);
    if variant == "win10" {
        // Action Center is a full-height side sheet.
        let (sw, sh) = ui.size;
        let bar = (th.cfg.bar.size + th.cfg.bar.margin) as f64;
        panel = Rect::new(sw - w, 0.0, w, sh - bar);
    }
    ui.cr.save().ok();
    ui.cr.push_group();
    popup_transform(ui, panel, t, th.cfg.bar.position == creed_core::config::Edge::Top);
    let radius = if variant == "win10" { 0.0 } else { th.radius() + 2.0 };
    th.panel(ui.cr, panel, radius, "control", ui.pointer);
    ui.record("control.panel", panel, None);
    ui.hit(ui::id("control.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(16.0);
    match views.page {
        1 => wifi_page(ui, inner, data, views),
        2 => bt_page(ui, inner, data),
        3 => power_page(ui, inner, data),
        _ => main_page(ui, inner, data, &variant),
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

fn main_height(ui: &Ui, data: &Data, _variant: &str) -> f64 {
    let tile = 58.0;
    let mut h = 16.0 * 2.0;
    h += tile * 3.0 + 10.0 * 2.0; // 3 rows of toggles
    h += if data.audio.is_some() || data.mic.is_some() || data.brightness.is_some() { 18.0 } else { 8.0 };
    let sliders = data.audio.is_some() as usize + data.mic.is_some() as usize + data.brightness.is_some() as usize;
    h += sliders as f64 * (36.0 + 12.0);
    if data.media.is_some() {
        h += 84.0;
    }
    h += 48.0; // footer
    let _ = ui;
    h.round()
}

fn main_page(ui: &mut Ui, inner: Rect, data: &Data, variant: &str) {
    let th = ui.th;
    let tile = 58.0;
    let gap = 10.0;
    let mut y = inner.y;
    let row = |y: f64| Rect::new(inner.x, y, inner.w, tile).columns(2, gap);

    // Row 1: Wi-Fi, Bluetooth
    let r1 = row(y);
    let (wifi_sub, wifi_on) = match data.net.kind {
        NetKind::Wifi => (data.net.name.clone(), true),
        NetKind::Ethernet => ("Wired".to_string(), true),
        NetKind::None if !data.net.available => ("Unavailable".to_string(), false),
        NetKind::None => (if data.net.wifi_enabled { "Not connected".into() } else { "Off".into() }, data.net.wifi_enabled),
    };
    ui.toggle_tile(ui::id("cc.wifi"), r1[0], "network-wireless-symbolic", "Wi-Fi", &wifi_sub, wifi_on, Action::ToggleWifi);
    if data.net.available {
        chevron(ui, r1[0], ui::id("cc.wifi.more"), "Wi-Fi networks", Action::Page(1), wifi_on);
    }
    let (bt_sub, bt_on) = match &data.bt {
        None => ("Unavailable".to_string(), false),
        Some(b) if !b.powered => ("Off".to_string(), false),
        Some(b) if b.connected.is_empty() => ("On".to_string(), true),
        Some(b) => (b.connected.join(", "), true),
    };
    ui.toggle_tile(ui::id("cc.bt"), r1[1], "bluetooth-active-symbolic", "Bluetooth", &bt_sub, bt_on, Action::ToggleBluetooth);
    if data.bt.is_some() {
        chevron(ui, r1[1], ui::id("cc.bt.more"), "Bluetooth devices", Action::Page(2), bt_on);
    }
    y += tile + gap;

    // Row 2: Do not disturb, Game Mode
    let r2 = row(y);
    ui.toggle_tile(ui::id("cc.dnd"), r2[0], "notifications-disabled-symbolic", if variant == "mac" || variant == "aqua" { "Focus" } else { "Do Not Disturb" }, "", data.dnd, Action::ToggleDnd);
    ui.toggle_tile(ui::id("cc.game"), r2[1], "input-gaming-symbolic", "Game Mode", if data.game { "On" } else { "" }, data.game, Action::ToggleGame);
    y += tile + gap;

    // Row 3: Night light, Power profile
    let r3 = row(y);
    ui.toggle_tile(ui::id("cc.night"), r3[0], "night-light-symbolic", "Night Light", "", data.night_light, Action::ToggleNightLight);
    let prof = data.power_profile.clone().unwrap_or_else(|| "Unavailable".into());
    let pretty = match prof.as_str() {
        "power-saver" => "Power saver",
        "balanced" => "Balanced",
        "performance" => "Performance",
        o => o,
    }
    .to_string();
    ui.toggle_tile(ui::id("cc.power"), r3[1], "power-profile-balanced-symbolic", "Power mode", &pretty, prof == "performance", Action::Page(3));
    y += tile + if data.audio.is_some() || data.mic.is_some() || data.brightness.is_some() { 18.0 } else { 8.0 };

    // Sliders
    let sl = |y: f64| Rect::new(inner.x, y, inner.w, 36.0);
    if let Some(a) = &data.audio {
        let vol = if a.muted { 0.0 } else { a.volume.min(1.0) };
        let vol_icon = if vol <= 0.0 { "audio-volume-muted-symbolic" } else { "audio-volume-high-symbolic" };
        ui.slider(ui::id("cc.vol"), sl(y), vol_icon, &format!("Volume {:.0}%", vol * 100.0), vol, Slider::Volume, Action::ToggleMute);
        y += 48.0;
    }
    if let Some(m) = &data.mic {
        let v = if m.muted { 0.0 } else { m.volume };
        ui.slider(ui::id("cc.mic"), sl(y), if m.muted { "microphone-disabled-symbolic" } else { "audio-input-microphone-symbolic" }, &format!("Microphone {:.0}%", v * 100.0), v, Slider::Mic, Action::ToggleMicMute);
        y += 48.0;
    }
    if let Some(b) = data.brightness {
        ui.slider(ui::id("cc.bright"), sl(y), "display-brightness-symbolic", &format!("Brightness {:.0}%", b * 100.0), b, Slider::Brightness, Action::None);
        y += 48.0;
    }

    if let Some(m) = &data.media {
        let card = Rect::new(inner.x, y, inner.w, 72.0);
        media_card(ui, card, m);
        y += 84.0;
    }

    // Footer: battery + settings + power
    let foot = Rect::new(inner.x, y, inner.w, 40.0);
    let (left, right) = foot.take_left(inner.w - 92.0, 4.0);
    if let Some(b) = &data.battery {
        let (ic, rest) = left.take_left(28.0, 6.0);
        ui.icon(ic, &super::bar::battery_icon(b.percent, b.charging), 18.0, Some(th.pal.text));
        ui.text(rest, &format!("{:.0}%{}", b.percent, if b.charging { " · charging" } else { "" }), &Text::new(th.fs * 0.95, th.pal.subtext));
    } else {
        let up = match (data.user.is_empty(), data.host.is_empty()) {
            (true, _) => String::new(),
            (false, true) => data.user.clone(),
            (false, false) => format!("{}@{}", data.user, data.host),
        };
        ui.text(left, &up, &Text::new(th.fs * 0.95, th.pal.subtext));
    }
    let btns = right.columns(2, 4.0);
    ui.icon_button(ui::id("cc.settings"), Rect::centered(btns[0].cx(), btns[0].cy(), 40.0, 40.0), "emblem-system-symbolic", "Creed settings", Action::Exec("creed-settings".into()), false);
    ui.icon_button(ui::id("cc.powerbtn"), Rect::centered(btns[1].cx(), btns[1].cy(), 40.0, 40.0), "system-shutdown-symbolic", "Power menu", Action::Open(Kind::Power), false);
}

fn chevron(ui: &mut Ui, tile: Rect, key: ui::Id, label: &str, action: Action, on: bool) {
    let th = ui.th;
    let r = Rect::new(tile.right() - 34.0, tile.y + 8.0, 28.0, tile.h - 16.0);
    let hv = ui.hover_amt(key);
    let col = if on { th.pal.on_accent } else { th.pal.text };
    gfx::fill_rounded(ui.cr, r, 8.0, col.with_alpha(0.08 + 0.1 * hv));
    ui.icon(r, "go-next-symbolic", 14.0, Some(col));
    ui.hit(key, r, action, Role::Button, label);
}

pub fn media_card(ui: &mut Ui, card: Rect, m: &crate::data::Media) {
    let th = ui.th;
    let p = &th.pal;
    gfx::fill_rounded(ui.cr, card, th.radius_small() + 4.0, if th.dark { creed_core::Color::rgba(1.0, 1.0, 1.0, 0.05) } else { creed_core::Color::rgba(0.0, 0.0, 0.0, 0.04) });
    let inner = card.inset(12.0);
    let (art, rest) = inner.take_left(inner.h, 12.0);
    gfx::fill_rounded(ui.cr, art, 10.0, p.accent.with_alpha(0.22));
    ui.icon(art, "audio-x-generic-symbolic", (art.h * 0.5).round(), Some(p.accent));
    let (text_r, btns) = rest.take_right(110.0, 8.0);
    let (a, b) = text_r.take_top(text_r.h / 2.0, 0.0);
    ui.text(a, &m.title, &Text::new(th.fs, p.text).medium());
    let sub = if m.artist.is_empty() { m.player.clone() } else { m.artist.clone() };
    ui.text(b, &sub, &Text::new(th.fs * 0.9, p.subtext));
    let cols = btns.columns(3, 2.0);
    ui.icon_button(ui::id("media.prev"), Rect::centered(cols[0].cx(), cols[0].cy(), 34.0, 34.0), "media-skip-backward-symbolic", "Previous", Action::MediaPrev, false);
    ui.icon_button(ui::id("media.play"), Rect::centered(cols[1].cx(), cols[1].cy(), 38.0, 38.0), if m.playing { "media-playback-pause-symbolic" } else { "media-playback-start-symbolic" }, if m.playing { "Pause" } else { "Play" }, Action::MediaPlayPause, true);
    ui.icon_button(ui::id("media.next"), Rect::centered(cols[2].cx(), cols[2].cy(), 34.0, 34.0), "media-skip-forward-symbolic", "Next", Action::MediaNext, false);
}

fn page_header(ui: &mut Ui, r: Rect, title: &str) -> Rect {
    let (hdr, rest) = r.take_top(36.0, 10.0);
    let (back, title_r) = hdr.take_left(36.0, 8.0);
    ui.icon_button(ui::id("cc.back"), back, "go-previous-symbolic", "Back", Action::Page(0), false);
    ui.text(title_r, title, &Text::new(ui.th.fs * 1.15, ui.th.pal.text).bold());
    rest
}

fn wifi_page(ui: &mut Ui, r: Rect, data: &Data, views: &mut Views) {
    let rest = page_header(ui, r, "Wi-Fi");
    if let Some(ssid) = views.wifi_password_for.clone() {
        let (l, rest2) = rest.take_top(28.0, 8.0);
        ui.text(l, &format!("Password for “{ssid}”"), &Text::new(ui.th.fs, ui.th.pal.subtext));
        let (inp, rest3) = rest2.take_top(40.0, 12.0);
        let masked: String = "•".repeat(views.password.chars().count());
        ui.input(ui::id("cc.pw"), inp, &masked, "Password", true, "");
        let (btn, _) = rest3.take_top(38.0, 0.0);
        let b = Rect::new(btn.right() - 120.0, btn.y, 120.0, 38.0);
        let k = ui::id("cc.pw.connect");
        let hv = ui.hover_amt(k);
        ui.th.chip(ui.cr, b, 19.0, hv, true);
        ui.text(b, "Connect", &Text::new(ui.th.fs, ui.th.pal.on_accent).medium().center());
        ui.hit(k, b, Action::WifiConnect(ssid), Role::Button, "Connect");
        return;
    }
    if !data.net.available {
        super::empty(ui, rest, "network-wireless-disabled-symbolic", "No Wi-Fi adapter (NetworkManager not running?)");
        return;
    }
    if data.aps.is_empty() {
        super::empty(ui, rest, "network-wireless-acquiring-symbolic", if data.net.wifi_enabled { "Scanning…" } else { "Wi-Fi is off" });
        return;
    }
    let from = ui.hits.len();
    let rows = super::list_layout(views, rest, data.aps.len(), 46.0, 2.0);
    super::with_clip(ui, rest, |ui| {
        for (i, rr) in rows {
            let ap = &data.aps[i];
            let lvl = if ap.signal > 75 { "excellent" } else if ap.signal > 50 { "good" } else if ap.signal > 25 { "ok" } else { "weak" };
            let icon = format!("network-wireless-signal-{lvl}-symbolic");
            let sub = format!("{}{}", if ap.active { "Connected · " } else { "" }, if ap.secure { "Secured" } else { "Open" });
            let k = ui::idn("cc.ap", &ap.ssid);
            ui.row(k, rr, None, &ap.ssid, &sub, ap.active, Action::WifiConnect(ap.ssid.clone()));
            ui.icon(Rect::new(rr.right() - 36.0, rr.y, 28.0, rr.h), &icon, 16.0, Some(ui.th.pal.text));
        }
    });
    super::clip_hits(ui, from, rest);
}

fn bt_page(ui: &mut Ui, r: Rect, data: &Data) {
    let rest = page_header(ui, r, "Bluetooth");
    match &data.bt {
        None => super::empty(ui, rest, "bluetooth-disabled-symbolic", "No Bluetooth adapter"),
        Some(b) if !b.powered => super::empty(ui, rest, "bluetooth-disabled-symbolic", "Bluetooth is off"),
        Some(b) if b.connected.is_empty() => {
            let (msg, btn) = rest.take_top(rest.h - 50.0, 8.0);
            super::empty(ui, msg, "bluetooth-active-symbolic", "No devices connected");
            let b = Rect::new(btn.x, btn.y, btn.w, 40.0);
            let k = ui::id("cc.bt.manager");
            let hv = ui.hover_amt(k);
            ui.th.chip(ui.cr, b, 20.0, hv, false);
            ui.text(b, "Open Bluetooth manager", &Text::new(ui.th.fs, ui.th.pal.text).center());
            ui.hit(k, b, Action::Exec("blueman-manager || overskride || foot -e bluetoothctl".into()), Role::Button, "Open Bluetooth manager");
        }
        Some(b) => {
            for (i, name) in b.connected.iter().enumerate() {
                let rr = Rect::new(rest.x, rest.y + i as f64 * 48.0, rest.w, 46.0);
                ui.row(ui::idn("cc.btdev", i), rr, Some("bluetooth-active-symbolic"), name, "Connected", false, Action::None);
            }
        }
    }
}

fn power_page(ui: &mut Ui, r: Rect, data: &Data) {
    let rest = page_header(ui, r, "Power mode");
    if data.power_profile.is_none() {
        super::empty(ui, rest, "battery-symbolic", "power-profiles-daemon is not running");
        return;
    }
    let cur = data.power_profile.clone().unwrap_or_default();
    let items = [
        ("power-saver", "Power saver", "Quieter, cooler, longer battery"),
        ("balanced", "Balanced", "Recommended for most work"),
        ("performance", "Performance", "Maximum speed, more heat"),
    ];
    for (i, (id, title, sub)) in items.iter().enumerate() {
        let rr = Rect::new(rest.x, rest.y + i as f64 * 56.0, rest.w, 52.0);
        ui.row(ui::idn("cc.prof", i), rr, Some(&format!("power-profile-{id}-symbolic")), title, sub, cur == *id, Action::PowerProfile(id.to_string()));
    }
}
