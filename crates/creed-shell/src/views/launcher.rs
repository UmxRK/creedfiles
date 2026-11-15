//! App launcher. Looks: `creed` (centered spotlight-like card), `start11` (Windows 11 Start),
//! `start10` (Windows 10 Start with tiles), `spotlight` (macOS), `aqua`.
//! Extras: inline calculator (`2+2*3`, `=sqrt` free), `>` runs a shell command.

use super::{Views, open_amount, popup_anchor, popup_transform};
use crate::data::{AppEntry, Data};
use crate::gfx::{self, Align, Rect, Text};
use crate::ui::{self, Action, Kind, Role, Ui};
use creed_core::Color;

/// Ranks apps against a query. Empty query → all apps alphabetically.
pub fn search(apps: &[AppEntry], q: &str) -> Vec<usize> {
    let q = q.trim().to_lowercase();
    if q.starts_with('>') || calc(&q).is_some() && !q.chars().any(|c| c.is_alphabetic()) {
        return vec![];
    }
    let mut scored: Vec<(i32, usize)> = vec![];
    for (i, a) in apps.iter().enumerate() {
        if q.is_empty() {
            scored.push((0, i));
            continue;
        }
        let name = a.name.to_lowercase();
        let score = if name == q {
            1000
        } else if name.starts_with(&q) {
            800
        } else if name.split(|c: char| !c.is_alphanumeric()).any(|w| w.starts_with(&q)) {
            600
        } else if name.contains(&q) {
            400
        } else if a.generic.to_lowercase().contains(&q) || a.keywords.to_lowercase().contains(&q) {
            250
        } else if a.id.to_lowercase().contains(&q) || a.exec.to_lowercase().split_whitespace().next().unwrap_or("").contains(&q) {
            150
        } else if subsequence(&name, &q) {
            80
        } else {
            continue;
        };
        scored.push((score, i));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| apps[a.1].name.to_lowercase().cmp(&apps[b.1].name.to_lowercase())));
    scored.into_iter().map(|(_, i)| i).collect()
}

fn subsequence(hay: &str, needle: &str) -> bool {
    let mut it = hay.chars();
    needle.chars().all(|c| it.any(|h| h == c))
}

/// Calculator / command row for the current query.
pub fn special(q: &str) -> Option<Action> {
    let t = q.trim();
    if let Some(cmd) = t.strip_prefix('>') {
        let cmd = cmd.trim();
        return (!cmd.is_empty()).then(|| Action::Exec(cmd.to_string()));
    }
    calc(t).map(|v| Action::Exec(format!("printf %s {} | wl-copy", fmt_num(v))))
}

pub fn special_label(q: &str) -> Option<(String, String, &'static str)> {
    let t = q.trim();
    if let Some(cmd) = t.strip_prefix('>') {
        let cmd = cmd.trim();
        return (!cmd.is_empty()).then(|| (format!("Run “{cmd}”"), "Runs in the background".to_string(), "utilities-terminal-symbolic"));
    }
    calc(t).map(|v| (format!("= {}", fmt_num(v)), "Press Enter to copy the result".to_string(), "accessories-calculator-symbolic"))
}

pub fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        let s = format!("{v:.10}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// Tiny recursive-descent calculator: + - * / % ^, parentheses, unary minus.
/// Requires at least one operator so plain numbers don't trigger it.
pub fn calc(s: &str) -> Option<f64> {
    let s: String = s.trim_start_matches('=').chars().filter(|c| !c.is_whitespace()).collect();
    if s.is_empty() || !s.chars().any(|c| "+-*/%^(".contains(c)) || !s.chars().all(|c| c.is_ascii_digit() || "+-*/%^().,".contains(c)) {
        return None;
    }
    let s = s.replace(',', ".");
    let b = s.as_bytes();
    let mut pos = 0;
    fn expr(b: &[u8], p: &mut usize) -> Option<f64> {
        let mut v = term(b, p)?;
        while *p < b.len() && (b[*p] == b'+' || b[*p] == b'-') {
            let op = b[*p];
            *p += 1;
            let r = term(b, p)?;
            v = if op == b'+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(b: &[u8], p: &mut usize) -> Option<f64> {
        let mut v = power(b, p)?;
        while *p < b.len() && (b[*p] == b'*' || b[*p] == b'/' || b[*p] == b'%') {
            let op = b[*p];
            *p += 1;
            let r = power(b, p)?;
            v = match op {
                b'*' => v * r,
                b'/' => {
                    if r == 0.0 {
                        return None;
                    }
                    v / r
                }
                _ => {
                    if r == 0.0 {
                        return None;
                    }
                    v % r
                }
            };
        }
        Some(v)
    }
    fn power(b: &[u8], p: &mut usize) -> Option<f64> {
        let base = unary(b, p)?;
        if *p < b.len() && b[*p] == b'^' {
            *p += 1;
            let e = power(b, p)?;
            return Some(base.powf(e));
        }
        Some(base)
    }
    fn unary(b: &[u8], p: &mut usize) -> Option<f64> {
        if *p < b.len() && b[*p] == b'-' {
            *p += 1;
            return Some(-unary(b, p)?);
        }
        atom(b, p)
    }
    fn atom(b: &[u8], p: &mut usize) -> Option<f64> {
        if *p < b.len() && b[*p] == b'(' {
            *p += 1;
            let v = expr(b, p)?;
            if *p < b.len() && b[*p] == b')' {
                *p += 1;
                return Some(v);
            }
            return None;
        }
        let start = *p;
        while *p < b.len() && (b[*p].is_ascii_digit() || b[*p] == b'.') {
            *p += 1;
        }
        std::str::from_utf8(&b[start..*p]).ok()?.parse().ok()
    }
    let v = expr(b, &mut pos)?;
    (pos == b.len() && v.is_finite()).then_some(v)
}

pub fn draw(ui: &mut Ui, closing: bool, data: &Data, views: &mut Views) {
    let t = open_amount(ui, closing);
    ui.backdrop(Action::Close(Kind::Launcher));
    let variant = ui.th.cfg.style.launcher.clone();
    ui.cr.save().ok();
    ui.cr.push_group();
    match variant.as_str() {
        "start11" => draw_start11(ui, t, data, views),
        "start10" => draw_start10(ui, t, data, views),
        _ => draw_card(ui, t, data, views, &variant),
    }
    let _ = ui.cr.pop_group_to_source();
    let _ = ui.cr.paint_with_alpha(t.clamp(0.0, 1.0));
    ui.cr.restore().ok();
}

/// Spotlight-style centred card used by creed / spotlight / aqua.
fn draw_card(ui: &mut Ui, t: f64, data: &Data, views: &mut Views, variant: &str) {
    let th = ui.th;
    let (sw, sh) = ui.size;
    let w = (sw * 0.42).clamp(460.0, 680.0).round();
    let input_h = if variant == "spotlight" || variant == "aqua" { 56.0 } else { 50.0 };
    let row_h = 50.0;
    let results = search(&data.apps, &views.query);
    let special = special_label(&views.query);
    let n = results.len() + special.is_some() as usize;
    let show_list = !(variant == "spotlight" || variant == "aqua") || !views.query.is_empty();
    let max_rows = 8usize;
    let list_h = if show_list { (n.min(max_rows) as f64 * (row_h + 2.0)).max(if n == 0 { 120.0 } else { 0.0 }) + 12.0 } else { 0.0 };
    let pad = 12.0;
    let h = (input_h + pad * 2.0 + if show_list { list_h + 4.0 } else { 0.0 }).round();
    let panel = if variant == "creed" {
        let y = (sh * 0.16).round();
        Rect::new(((sw - w) / 2.0).round(), y, w, h)
    } else {
        Rect::new(((sw - w) / 2.0).round(), (sh * 0.22).round(), w, h)
    };
    popup_transform(ui, panel, t, true);
    let radius = if variant == "aqua" { 12.0 } else { th.radius() + 4.0 };
    th.panel(ui.cr, panel, radius, "launcher", ui.pointer);
    ui.record("launcher.panel", panel, None);
    ui.hit(ui::id("launcher.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset(pad);
    let (input_r, list_r) = inner.take_top(input_h, 4.0);
    let ph = match variant {
        "spotlight" | "aqua" => "Spotlight Search",
        _ => "Search apps, calculate, or > run a command",
    };
    let pre = views.preedit.clone();
    ui.input(ui::id("launcher.input"), input_r, &views.query, ph, true, &pre);
    ui.record("launcher.input", input_r, Some("launcher.panel"));
    if !show_list {
        return;
    }
    let list_r = Rect::new(list_r.x, list_r.y + 6.0, list_r.w, list_r.h - 6.0);
    if n == 0 {
        super::empty(ui, list_r, "edit-find-symbolic", if data.apps.is_empty() { "Loading applications…" } else { "No matches" });
        return;
    }
    let from = ui.hits.len();
    let rows = super::list_layout(views, list_r, n, row_h, 2.0);
    let sel = views.sel;
    super::with_clip(ui, list_r, |ui| {
        for (i, r) in rows {
            let k = ui::idn("launcher.row", i);
            if let (0, Some((title, sub, icon))) = (i, special.as_ref()) {
                ui.row(k, r, Some(icon), title, sub, sel == 0, Action::LauncherPick(0));
                continue;
            }
            let ai = results[i - special.is_some() as usize];
            let a = &data.apps[ai];
            let sub = if a.generic.is_empty() { String::new() } else { a.generic.clone() };
            ui.row(k, r, Some(&a.icon.clone()), &a.name.clone(), &sub, sel == i, Action::LauncherPick(i));
        }
    });
    super::clip_hits(ui, from, list_r);
}

fn app_tile(ui: &mut Ui, r: Rect, a: &AppEntry, selected: bool, key: ui::Id, icon_size: f64) {
    let th = ui.th;
    let hv = ui.hover_amt(key);
    if selected {
        gfx::fill_rounded(ui.cr, r, th.radius_small(), th.pal.accent.with_alpha(0.18));
    }
    th.hover_bg(ui.cr, r, th.radius_small(), hv);
    let (ic, rest) = r.inset_xy(4.0, 8.0).take_top(icon_size, 6.0);
    ui.icon(ic, &a.icon, icon_size, None);
    ui.text(rest, &a.name, &Text::new(th.fs * 0.88, th.pal.text).center());
    if ui.focus == Some(key) {
        th.focus_ring(ui.cr, r, th.radius_small());
    }
    ui.hit(key, r, Action::Launch(a.id.clone()), Role::Button, a.name.clone());
}

fn pinned_apps<'a>(ui: &Ui, data: &'a Data) -> Vec<&'a AppEntry> {
    let mut out: Vec<&AppEntry> = ui.th.cfg.dock.pinned.iter().filter_map(|p| data.apps.iter().find(|a| &a.id == p)).collect();
    // Fill up with other apps for a full grid.
    for a in &data.apps {
        if out.len() >= 18 {
            break;
        }
        if !out.iter().any(|x| x.id == a.id) {
            out.push(a);
        }
    }
    out
}

fn draw_start11(ui: &mut Ui, t: f64, data: &Data, views: &mut Views) {
    let th = ui.th;
    let (w, h) = (640.0, 700.0f64.min(ui.size.1 - 120.0));
    let panel = popup_anchor(ui, w, h, Align::Center);
    popup_transform(ui, panel, t, false);
    th.panel(ui.cr, panel, th.radius(), "launcher", ui.pointer);
    ui.record("launcher.panel", panel, None);
    ui.hit(ui::id("launcher.panel"), panel, Action::None, Role::Group, "");
    let inner = panel.inset_xy(32.0, 28.0);
    let (input_r, rest) = inner.take_top(36.0, 24.0);
    let pre = views.preedit.clone();
    ui.input(ui::id("launcher.input"), input_r, &views.query, "Search for apps, settings, and documents", true, &pre);
    let (body, footer) = rest.take_bottom(56.0, 12.0);
    if views.query.is_empty() {
        let (hd, grid) = body.take_top(28.0, 8.0);
        super::heading(ui, hd, "Pinned");
        let apps = pinned_apps(ui, data);
        let cols = 6;
        let cell_w = grid.w / cols as f64;
        let cell_h = 92.0;
        for (i, a) in apps.iter().enumerate().take(18) {
            let r = Rect::new(grid.x + (i % cols) as f64 * cell_w, grid.y + (i / cols) as f64 * cell_h, cell_w, cell_h - 4.0);
            app_tile(ui, r.inset(2.0), a, views.sel == i && false, ui::idn("start.pin", &a.id), 34.0);
        }
    } else {
        draw_results_list(ui, body, data, views);
    }
    // Footer: user + power.
    let line = if th.dark { Color::rgba(1.0, 1.0, 1.0, 0.06) } else { Color::rgba(0.0, 0.0, 0.0, 0.06) };
    gfx::fill_rounded(ui.cr, Rect::new(panel.x, footer.y - 6.0, panel.w, 1.0), 0.0, line);
    let (user_r, power_r) = footer.take_right(40.0, 8.0);
    let (av, name_r) = power_r.take_left(32.0, 10.0);
    let av = Rect::centered(av.cx(), user_r.cy(), 32.0, 32.0);
    gfx::fill_rounded(ui.cr, av, 16.0, th.pal.accent_alt);
    let initial: String = data.user.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_else(|| "U".into());
    ui.text(av, &initial, &Text::new(th.fs, th.pal.on_accent).bold().center());
    ui.text(name_r, &data.user, &Text::new(th.fs, th.pal.text));
    ui.icon_button(ui::id("start.power"), Rect::centered(user_r.cx(), user_r.cy(), 36.0, 36.0), "system-shutdown-symbolic", "Power", Action::Open(Kind::Power), false);
}

fn draw_start10(ui: &mut Ui, t: f64, data: &Data, views: &mut Views) {
    let th = ui.th;
    let (w, h) = (720.0, 640.0f64.min(ui.size.1 - 80.0));
    let panel = popup_anchor(ui, w, h, Align::Start);
    let panel = Rect::new(0.0, panel.y + 8.0, w, h);
    popup_transform(ui, panel, t, false);
    th.panel(ui.cr, panel, 0.0, "launcher", ui.pointer);
    ui.record("launcher.panel", panel, None);
    ui.hit(ui::id("launcher.panel"), panel, Action::None, Role::Group, "");
    // Left rail.
    let (rail, rest) = panel.take_left(48.0, 0.0);
    let rail_items: [(&str, &str, Action); 4] = [
        ("avatar-default-symbolic", "Account", Action::None),
        ("folder-documents-symbolic", "Documents", Action::Exec(format!("{} ~/Documents", th.cfg.general.file_manager))),
        ("emblem-system-symbolic", "Settings", Action::Exec("creed-settings".into())),
        ("system-shutdown-symbolic", "Power", Action::Open(Kind::Power)),
    ];
    for (i, (ic, label, act)) in rail_items.into_iter().enumerate() {
        let y = if i == 0 { rail.y + 8.0 } else { rail.bottom() - 48.0 * (4 - i) as f64 - 8.0 };
        ui.icon_button(ui::idn("start10.rail", i), Rect::new(rail.x, y, 48.0, 48.0), ic, label, act, false);
    }
    let (list_col, tiles) = rest.take_left(260.0, 8.0);
    let (input_r, list_r) = list_col.inset_xy(8.0, 10.0).take_top(34.0, 10.0);
    let pre = views.preedit.clone();
    ui.input(ui::id("launcher.input"), input_r, &views.query, "Type here to search", true, &pre);
    draw_results_list(ui, list_r, data, views);
    // Tiles (pinned) with accent backgrounds.
    let tiles = tiles.inset(12.0);
    let (hd, grid) = tiles.take_top(24.0, 8.0);
    super::heading(ui, hd, "Life at a glance");
    let apps = pinned_apps(ui, data);
    let cols = 3;
    let cell = ((grid.w - 8.0 * (cols as f64 - 1.0)) / cols as f64).floor();
    for (i, a) in apps.iter().enumerate().take(12) {
        let r = Rect::new(grid.x + (i % cols) as f64 * (cell + 8.0), grid.y + (i / cols) as f64 * (cell * 0.9 + 8.0), cell, cell * 0.9);
        if r.bottom() > grid.bottom() {
            break;
        }
        let k = ui::idn("start10.tile", &a.id);
        let hv = ui.hover_amt(k);
        gfx::fill_rounded(ui.cr, r, 0.0, th.pal.accent.with_alpha(0.75 + 0.15 * hv));
        if hv > 0.0 {
            gfx::stroke_rounded(ui.cr, r, 0.0, 2.0, Color::rgba(1.0, 1.0, 1.0, 0.4 * hv));
        }
        ui.icon(Rect::new(r.x, r.y, r.w, r.h - 22.0), &a.icon, (r.h * 0.38).round(), None);
        ui.text(Rect::new(r.x + 8.0, r.bottom() - 24.0, r.w - 16.0, 20.0), &a.name, &Text::new(th.fs * 0.85, th.pal.on_accent));
        ui.hit(k, r, Action::Launch(a.id.clone()), Role::Button, a.name.clone());
    }
}

fn draw_results_list(ui: &mut Ui, area: Rect, data: &Data, views: &mut Views) {
    let results = search(&data.apps, &views.query);
    let special = special_label(&views.query);
    let n = results.len() + special.is_some() as usize;
    if n == 0 {
        super::empty(ui, area, "edit-find-symbolic", "No matches");
        return;
    }
    let from = ui.hits.len();
    let rows = super::list_layout(views, area, n, 44.0, 2.0);
    let sel = views.sel;
    super::with_clip(ui, area, |ui| {
        for (i, r) in rows {
            let k = ui::idn("launcher.row", i);
            if let (0, Some((title, sub, icon))) = (i, special.as_ref()) {
                ui.row(k, r, Some(icon), title, sub, sel == 0, Action::LauncherPick(0));
                continue;
            }
            let a = &data.apps[results[i - special.is_some() as usize]];
            ui.row(k, r, Some(&a.icon.clone()), &a.name.clone(), "", sel == i, Action::LauncherPick(i));
        }
    });
    super::clip_hits(ui, from, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, name: &str, generic: &str) -> AppEntry {
        AppEntry { id: id.into(), name: name.into(), generic: generic.into(), icon: String::new(), exec: id.into(), keywords: String::new(), terminal: false }
    }

    #[test]
    fn ranking() {
        let apps = vec![app("firefox", "Firefox", "Web Browser"), app("foot", "Foot", "Terminal"), app("files", "Files", "File Manager"), app("gimp", "GNU Image Manipulation Program", "")];
        let r = search(&apps, "f");
        assert_eq!(apps[r[0]].name, "Files");
        assert_eq!(search(&apps, "fire")[0], 0);
        assert_eq!(search(&apps, "browser")[0], 0);
        assert_eq!(search(&apps, "image")[0], 3);
        assert_eq!(search(&apps, "gmp")[0], 3); // subsequence
        assert!(search(&apps, "zzz").is_empty());
        assert_eq!(search(&apps, "").len(), 4);
    }

    #[test]
    fn calculator() {
        assert_eq!(calc("2+2*3"), Some(8.0));
        assert_eq!(calc("(1+2)^2"), Some(9.0));
        assert_eq!(calc("-3+5"), Some(2.0));
        assert_eq!(calc("10/4"), Some(2.5));
        assert_eq!(calc("7%4"), Some(3.0));
        assert_eq!(calc("1/0"), None);
        assert_eq!(calc("42"), None);
        assert_eq!(calc("firefox"), None);
        assert_eq!(calc("2+"), None);
        assert_eq!(fmt_num(2.5), "2.5");
        assert_eq!(fmt_num(8.0), "8");
        assert!(matches!(special("> echo hi"), Some(Action::Exec(_))));
    }
}
