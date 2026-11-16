//! Application state, Wayland plumbing and the action dispatcher.

use crate::data::*;
use crate::gfx::{Rect, Theme};
use crate::icons::Icons;
use crate::ui::{self, Action, Anims, Hit, Id, Kind, LayoutBox, Role, Slider, Ui};
use crate::views::{self, Spec, Views};
use crate::{a11y, services};
use creed_core::config::{Config, ColorSource};
use creed_core::hypr::{Hypr, HyprEvent};
use creed_core::palette::{self, Palette};
use creed_core::Paths;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState, Region},
    output::{OutputHandler, OutputState},
    reexports::calloop::{channel::Sender, LoopHandle},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{KeyboardInteractivity, LayerShell, LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
        WaylandSurface,
    },
    shm::{
        slot::{Buffer, SlotPool},
        Shm, ShmHandler,
    },
};
use std::collections::BTreeSet;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::time::Instant;
use wayland_client::{
    globals::GlobalList,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, Dispatch, Proxy, QueueHandle,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{Shape, WpCursorShapeDeviceV1};
use wayland_protocols::wp::fractional_scale::v1::client::{
    wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1,
    wp_fractional_scale_v1::{self, WpFractionalScaleV1},
};
use wayland_protocols::wp::text_input::zv3::client::{
    zwp_text_input_manager_v3::ZwpTextInputManagerV3,
    zwp_text_input_v3::{self, ZwpTextInputV3},
};
use wayland_protocols::wp::viewporter::client::{wp_viewport::WpViewport, wp_viewporter::WpViewporter};

pub enum Msg {
    Hypr(HyprEvent),
    Audio(Option<Audio>),
    Mic(Option<Audio>),
    Brightness(Option<f64>),
    Battery(Option<Battery>),
    PowerProfile(Option<String>),
    Net(Net),
    Aps(Vec<AccessPoint>),
    Bt(Option<Bluetooth>),
    Media(Option<Media>),
    Notify(Notif),
    NotifClose(u32),
    Tray(Vec<TrayItem>),
    Menu(String, Vec<MenuEntry>),
    Apps(Vec<AppEntry>),
    Recent(Vec<(String, String, String)>),
    Clips(Vec<(String, String)>),
    Sys(SysStat),
    Weather(Option<WeatherNow>),
    Ipc(String, UnixStream),
    A11y(u64, Id, accesskit::Action),
    ConfigChanged,
    Tick,
}

pub struct Surf {
    pub uid: u64,
    pub kind: Kind,
    pub output: Option<wl_output::WlOutput>,
    pub layer: LayerSurface,
    pub pool: Option<SlotPool>,
    pub buffer: Option<Buffer>,
    pub w: u32,
    pub h: u32,
    pub int_scale: i32,
    pub frac_scale: Option<f64>,
    pub fractional: Option<WpFractionalScaleV1>,
    pub viewport: Option<WpViewport>,
    pub configured: bool,
    pub dirty: bool,
    pub frame_pending: bool,
    pub hits: Vec<Hit>,
    pub boxes: Vec<LayoutBox>,
    pub hover: Option<Id>,
    pub pressed: Option<Id>,
    pub focus: Option<Id>,
    pub pointer: Option<(f64, f64)>,
    pub anims: Anims,
    pub last_frame: Option<Instant>,
    pub closing: bool,
    pub spec: Spec,
    pub a11y: Option<a11y::SurfaceA11y>,
    pub dragging: Option<Id>,
}

impl Surf {
    pub fn scale(&self) -> f64 {
        self.frac_scale.unwrap_or(self.int_scale as f64)
    }
    fn wl(&self) -> &wl_surface::WlSurface {
        self.layer.wl_surface()
    }
}

pub struct App {
    pub conn: Connection,
    pub qh: QueueHandle<App>,
    pub loop_handle: LoopHandle<'static, App>,
    pub tx: Sender<Msg>,
    pub registry: RegistryState,
    pub seat_state: SeatState,
    pub output_state: OutputState,
    pub compositor: CompositorState,
    pub layer_shell: LayerShell,
    pub shm: Shm,
    pub cursor_shape: Option<CursorShapeManager>,
    pub fractional_mgr: Option<WpFractionalScaleManagerV1>,
    pub viewporter: Option<WpViewporter>,
    pub text_input_mgr: Option<ZwpTextInputManagerV3>,
    pub toplevel_export: Option<crate::thumbs::protocol::hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1>,
    pub captures: Vec<crate::thumbs::Pending>,
    pub text_input: Option<ZwpTextInputV3>,
    pub ime_pending: (Option<String>, Option<String>),
    pub keyboard: Option<wl_keyboard::WlKeyboard>,
    pub pointer: Option<wl_pointer::WlPointer>,
    pub cursor_device: Option<WpCursorShapeDeviceV1>,
    pub kb_surface: Option<u64>,
    pub ptr_surface: Option<u64>,
    pub modifiers: Modifiers,

    pub paths: Paths,
    pub theme: Theme,
    pub config_error: Option<String>,
    pub icons: Icons,
    pub data: Data,
    pub views: Views,
    pub surfs: Vec<Surf>,
    pub next_uid: u64,
    pub hypr: Option<Hypr>,
    pub notif_bus: Option<services::notify::Handle>,
    pub game_hidden: bool,
    pub started: Instant,
    pub exit: bool,
    pub minute_marker: i64,
}

pub fn load_theme(paths: &Paths) -> (Theme, Option<String>) {
    let (mut cfg, err) = match Config::load(paths, None) {
        Ok(l) => (l.config, None),
        Err(e) => {
            // Keep running with the last good config (or defaults) and tell the user.
            let fallback = Config::load(paths, Some("creed")).map(|l| l.config).unwrap_or_default();
            (fallback, Some(e.to_string()))
        }
    };
    cfg.resolve_themes(&paths.home);
    let pal = make_palette(paths, &cfg);
    (Theme::new(cfg, pal), err)
}

pub fn make_palette(paths: &Paths, cfg: &Config) -> Palette {
    if let Some(name) = &cfg.style.palette {
        if let Some(p) = palette::system(name, cfg.scheme()) {
            return creed_core::theme::with_accent(p, cfg.appearance.accent);
        }
    }
    let preset_name = cfg.style.palette.clone().unwrap_or_else(|| cfg.appearance.preset.clone());
    let seed = if cfg.style.palette.is_some() || cfg.appearance.color_source == ColorSource::Preset {
        palette::preset_seed(&preset_name)
    } else {
        let wp = &cfg.appearance.wallpaper;
        if let Some(name) = wp.strip_prefix("creed://") {
            views::wallpaper::procedural_seed(name)
        } else if wp.is_empty() {
            None
        } else {
            // Cached seed so startup does not decode the wallpaper twice.
            let path = paths.expand(wp);
            let cache = paths.cache.join("seed.json");
            let key = format!(
                "{}:{}",
                path.display(),
                std::fs::metadata(&path).and_then(|m| m.modified()).map(|t| format!("{t:?}")).unwrap_or_default()
            );
            let cached: Option<(String, f64, f64, f64, f64)> =
                std::fs::read_to_string(&cache).ok().and_then(|t| serde_json::from_str(&t).ok());
            match cached {
                Some((k, h, a, t, c)) if k == key => {
                    Some(palette::Seed { hue: h, alt_hue: a, tint: t, accent_chroma: c, fixed_accent: None })
                }
                _ => palette::seed_from_image(&path).ok().inspect(|s| {
                    let _ = std::fs::create_dir_all(&paths.cache);
                    let _ = std::fs::write(&cache, serde_json::to_string(&(key, s.hue, s.alt_hue, s.tint, s.accent_chroma)).unwrap());
                }),
            }
        }
    };
    let seed = seed.or_else(|| palette::preset_seed(&preset_name)).unwrap_or_else(|| palette::preset_seed("ember").unwrap());
    Palette::generate(seed, cfg.scheme(), cfg.appearance.high_contrast, cfg.appearance.accent)
}

impl App {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        conn: Connection,
        globals: &GlobalList,
        qh: QueueHandle<App>,
        loop_handle: LoopHandle<'static, App>,
        tx: Sender<Msg>,
        paths: Paths,
    ) -> App {
        let compositor = CompositorState::bind(globals, &qh).expect("wl_compositor missing");
        let layer_shell = LayerShell::bind(globals, &qh).expect("this compositor does not support wlr-layer-shell");
        let shm = Shm::bind(globals, &qh).expect("wl_shm missing");
        let cursor_shape = CursorShapeManager::bind(globals, &qh).ok();
        let fractional_mgr = globals.bind::<WpFractionalScaleManagerV1, _, _>(&qh, 1..=1, ()).ok();
        let viewporter = globals.bind::<WpViewporter, _, _>(&qh, 1..=1, ()).ok();
        let text_input_mgr = globals.bind::<ZwpTextInputManagerV3, _, _>(&qh, 1..=1, ()).ok();
        let toplevel_export = globals.bind::<crate::thumbs::protocol::hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1, _, _>(&qh, 1..=1, ()).ok();
        let (theme, config_error) = load_theme(&paths);
        let mut icons = Icons::new(&theme.cfg.style.icon_theme);
        icons.set_shell(theme.cfg.style.shell_icons.as_deref());
        let mut data = Data::default();
        data.user = std::env::var("USER").unwrap_or_default();
        data.host = hostname();
        App {
            conn,
            registry: RegistryState::new(globals),
            seat_state: SeatState::new(globals, &qh),
            output_state: OutputState::new(globals, &qh),
            qh,
            loop_handle,
            tx,
            compositor,
            layer_shell,
            shm,
            cursor_shape,
            fractional_mgr,
            viewporter,
            text_input_mgr,
            toplevel_export,
            captures: vec![],
            text_input: None,
            ime_pending: (None, None),
            keyboard: None,
            pointer: None,
            cursor_device: None,
            kb_surface: None,
            ptr_surface: None,
            modifiers: Modifiers::default(),
            paths,
            theme,
            config_error,
            icons,
            data,
            views: Views::default(),
            surfs: vec![],
            next_uid: 1,
            hypr: Hypr::from_env(),
            notif_bus: None,
            game_hidden: false,
            started: Instant::now(),
            exit: false,
            minute_marker: 0,
        }
    }

    // ───────────────────────────── surfaces ─────────────────────────────

    pub fn output_size(&self, out: Option<&wl_output::WlOutput>) -> (u32, u32) {
        out.and_then(|o| self.output_state.info(o))
            .and_then(|i| i.logical_size.or_else(|| i.modes.iter().find(|m| m.current).map(|m| m.dimensions)))
            .map(|(w, h)| (w.max(1) as u32, h.max(1) as u32))
            .unwrap_or((1920, 1080))
    }

    fn output_name(&self, out: &wl_output::WlOutput) -> String {
        self.output_state.info(out).and_then(|i| i.name).unwrap_or_default()
    }

    /// Output where popups should appear: Hyprland's focused monitor, else the first one.
    pub fn focused_output(&self) -> Option<wl_output::WlOutput> {
        let outs: Vec<_> = self.output_state.outputs().collect();
        outs.iter()
            .find(|o| !self.data.focused_monitor.is_empty() && self.output_name(o) == self.data.focused_monitor)
            .cloned()
            .or_else(|| outs.first().cloned())
    }

    pub fn is_open(&self, kind: Kind) -> bool {
        self.surfs.iter().any(|s| s.kind == kind && !s.closing)
    }

    pub fn create(&mut self, kind: Kind, output: Option<wl_output::WlOutput>) {
        let (ow, oh) = self.output_size(output.as_ref());
        let spec = views::spec(kind, &self.theme, &self.data, &self.views, ow, oh);
        let surface = self.compositor.create_surface(&self.qh);
        let layer = self.layer_shell.create_layer_surface(&self.qh, surface, spec.layer, Some(spec.namespace), output.as_ref());
        apply_spec(&layer, &spec);
        apply_input(&self.compositor, &layer, &spec);
        let fractional =
            self.fractional_mgr.as_ref().map(|m| m.get_fractional_scale(layer.wl_surface(), &self.qh, layer.wl_surface().id().protocol_id()));
        let viewport = if fractional.is_some() { self.viewporter.as_ref().map(|v| v.get_viewport(layer.wl_surface(), &self.qh, ())) } else { None };
        layer.commit();
        let uid = self.next_uid;
        self.next_uid += 1;
        let a11y = if self.theme.cfg.a11y.screen_reader {
            Some(a11y::SurfaceA11y::new(uid, kind, self.tx.clone()))
        } else {
            None
        };
        self.surfs.push(Surf {
            uid,
            kind,
            output,
            layer,
            pool: None,
            buffer: None,
            w: spec.size.0.max(1),
            h: spec.size.1.max(1),
            int_scale: 1,
            frac_scale: None,
            fractional,
            viewport,
            configured: false,
            dirty: true,
            frame_pending: false,
            hits: vec![],
            boxes: vec![],
            hover: None,
            pressed: None,
            focus: None,
            pointer: None,
            anims: Anims::default(),
            last_frame: None,
            closing: false,
            spec,
            a11y,
            dragging: None,
        });
    }

    pub fn destroy(&mut self, uid: u64) {
        if let Some(i) = self.surfs.iter().position(|s| s.uid == uid) {
            let s = self.surfs.remove(i);
            if let Some(f) = &s.fractional {
                f.destroy();
            }
            if let Some(v) = &s.viewport {
                v.destroy();
            }
            if self.kb_surface == Some(uid) {
                self.kb_surface = None;
                if let Some(ti) = &self.text_input {
                    ti.disable();
                    ti.commit();
                }
            }
            if self.ptr_surface == Some(uid) {
                self.ptr_surface = None;
            }
            drop(s);
            let _ = self.conn.flush();
        }
        if !self.surfs.iter().any(|s| s.kind == Kind::Overview) {
            self.data.thumbs.clear();
            for p in self.captures.drain(..) {
                p.frame.destroy();
            }
        }
        if !self.surfs.iter().any(|s| s.kind.is_popup()) {
            // Popups are gone: give memory back.
            self.icons.clear();
            self.views.on_all_closed();
            crate::trim_memory();
        }
    }

    /// (Re)creates the always-on surfaces for every output according to config.
    pub fn sync_persistent(&mut self) {
        let outputs: Vec<_> = self.output_state.outputs().collect();
        let cfg = &self.theme.cfg;
        let mut want: Vec<(Kind, wl_output::WlOutput)> = vec![];
        for o in &outputs {
            want.push((Kind::Wallpaper, o.clone()));
            if !self.game_hidden {
                if cfg.bar.enabled {
                    want.push((Kind::Bar, o.clone()));
                }
                if cfg.dock.enabled && cfg.style.dock != "none" {
                    want.push((Kind::Dock, o.clone()));
                }
            }
        }
        // Remove surfaces that should not exist anymore.
        let stale: Vec<u64> = self
            .surfs
            .iter()
            .filter(|s| matches!(s.kind, Kind::Wallpaper | Kind::Bar | Kind::Dock))
            .filter(|s| !want.iter().any(|(k, o)| *k == s.kind && Some(o) == s.output.as_ref()))
            .map(|s| s.uid)
            .collect();
        for uid in stale {
            self.destroy(uid);
        }
        for (k, o) in want {
            if !self.surfs.iter().any(|s| s.kind == k && s.output.as_ref() == Some(&o)) {
                self.create(k, Some(o));
            }
        }
    }

    /// Re-evaluates layer specs (size, anchors, margins) and marks everything dirty.
    pub fn relayout(&mut self) {
        for i in 0..self.surfs.len() {
            let (ow, oh) = self.output_size(self.surfs[i].output.as_ref());
            let s = &mut self.surfs[i];
            let spec = views::spec(s.kind, &self.theme, &self.data, &self.views, ow, oh);
            if spec != s.spec {
                apply_spec(&s.layer, &spec);
                if spec.input != s.spec.input || spec.click_through != s.spec.click_through {
                    apply_input(&self.compositor, &s.layer, &spec);
                }
                if spec.size != s.spec.size {
                    s.layer.commit();
                    s.configured = false;
                }
                s.spec = spec;
                s.dirty = true;
            } else if s.kind != Kind::Wallpaper {
                s.dirty = true;
            }
        }
    }

    pub fn mark(&mut self, kind: Kind) {
        for s in self.surfs.iter_mut().filter(|s| s.kind == kind) {
            s.dirty = true;
        }
    }

    /// Marks every surface except the wallpaper (redrawing it means decoding an image).
    pub fn mark_all(&mut self) {
        for s in self.surfs.iter_mut().filter(|s| s.kind != Kind::Wallpaper) {
            s.dirty = true;
        }
    }

    /// Kinds whose size depends on data must be re-specced when data changes.
    pub fn respec(&mut self, kind: Kind) {
        for i in 0..self.surfs.len() {
            if self.surfs[i].kind != kind {
                continue;
            }
            let (ow, oh) = self.output_size(self.surfs[i].output.as_ref());
            let s = &mut self.surfs[i];
            let spec = views::spec(s.kind, &self.theme, &self.data, &self.views, ow, oh);
            if spec != s.spec {
                apply_spec(&s.layer, &spec);
                s.layer.commit();
                if spec.size != s.spec.size {
                    s.configured = false;
                }
                s.spec = spec;
            }
            s.dirty = true;
        }
    }

    pub fn draw_dirty(&mut self) {
        for i in 0..self.surfs.len() {
            if i >= self.surfs.len() {
                break;
            }
            let s = &self.surfs[i];
            if s.configured && s.dirty && !s.frame_pending {
                self.draw(i);
            }
        }
        // Remove surfaces that finished their closing animation.
        let done: Vec<u64> = self.surfs.iter().filter(|s| s.closing && s.anims.springs.get(&ui::id("__open")).map(|sp| sp.value <= 0.02).unwrap_or(true)).map(|s| s.uid).collect();
        for uid in done {
            self.destroy(uid);
        }
        let _ = self.conn.flush();
    }

    fn draw(&mut self, i: usize) {
        let qh = self.qh.clone();
        let scale = self.surfs[i].scale();
        let (w, h) = (self.surfs[i].w, self.surfs[i].h);
        let bw = ((w as f64) * scale).round().max(1.0) as i32;
        let bh = ((h as f64) * scale).round().max(1.0) as i32;
        let stride = bw * 4;
        let s = &mut self.surfs[i];
        if s.kind == Kind::Wallpaper {
            // One buffer, exactly sized: never let the pool grow for the largest surface.
            s.buffer = None;
            s.pool = None;
        }
        if s.pool.is_none() {
            match SlotPool::new((stride * bh) as usize, &self.shm) {
                Ok(p) => s.pool = Some(p),
                Err(e) => {
                    eprintln!("creed-shell: shm pool: {e}");
                    return;
                }
            }
        }
        let pool = s.pool.as_mut().unwrap();
        let reuse = match s.buffer.take() {
            Some(b) if b.height() == bh && b.stride() == stride && b.canvas(pool).is_some() => Some(b),
            _ => None,
        };
        let buffer = match reuse {
            Some(b) => b,
            None => match pool.create_buffer(bw, bh, stride, wl_shm::Format::Argb8888) {
                Ok((b, _)) => b,
                Err(e) => {
                    eprintln!("creed-shell: buffer: {e}");
                    return;
                }
            },
        };
        let canvas = buffer.canvas(pool).expect("fresh buffer");
        canvas.fill(0);
        let now = Instant::now();
        let dt = s.last_frame.map(|t| now.duration_since(t).as_secs_f64()).unwrap_or(0.0);
        s.last_frame = Some(now);
        let (hits, boxes, animating);
        {
            let img = unsafe {
                cairo::ImageSurface::create_for_data_unsafe(canvas.as_mut_ptr(), cairo::Format::ARgb32, bw, bh, stride)
            };
            let Ok(img) = img else { return };
            let cr = cairo::Context::new(&img).expect("cairo context");
            cr.scale(scale, scale);
            self.icons.scale = scale;
            let mut ui = Ui {
                cr: &cr,
                th: &self.theme,
                icons: &mut self.icons,
                anims: &mut s.anims,
                hits: Vec::with_capacity(s.hits.len()),
                boxes: vec![],
                hover: s.hover,
                pressed: s.pressed,
                focus: s.focus,
                pointer: s.pointer,
                dt,
                animating: false,
                size: (w as f64, h as f64),
                now: self.started.elapsed().as_secs_f64(),
            };
            let closing = s.closing;
            views::draw(&mut ui, s.kind, closing, &self.data, &mut self.views, &self.paths, self.config_error.as_deref());
            ui.anims.gc();
            hits = std::mem::take(&mut ui.hits);
            boxes = std::mem::take(&mut ui.boxes);
            animating = ui.animating;
            drop(ui);
            drop(cr);
            img.flush();
        }
        s.hits = hits;
        s.boxes = boxes;
        // Re-resolve hover against the new hit list (content may have moved).
        if let Some((px, py)) = s.pointer {
            s.hover = hit_at(&s.hits, px, py).map(|h| h.id);
        }
        let wl = s.wl().clone();
        wl.damage_buffer(0, 0, bw, bh);
        if animating || s.closing {
            wl.frame(&qh, smithay_client_toolkit::compositor::FrameCallbackData(wl.clone()));
            s.frame_pending = true;
        }
        if let Some(vp) = &s.viewport {
            if s.frac_scale.is_some() {
                vp.set_destination(w as i32, h as i32);
                wl.set_buffer_scale(1);
            } else {
                vp.set_destination(-1, -1);
                wl.set_buffer_scale(s.int_scale);
            }
        } else {
            wl.set_buffer_scale(s.int_scale);
        }
        let _ = buffer.attach_to(&wl);
        wl.commit();
        s.buffer = Some(buffer);
        s.dirty = false;
        if let Some(a) = s.a11y.as_mut() {
            a.update(&s.hits, s.focus, (w as f64, h as f64));
        }
    }

    // ───────────────────────────── input ─────────────────────────────

    fn surf_idx(&self, wl: &wl_surface::WlSurface) -> Option<usize> {
        self.surfs.iter().position(|s| s.wl() == wl)
    }

    fn uid_idx(&self, uid: u64) -> Option<usize> {
        self.surfs.iter().position(|s| s.uid == uid)
    }

    fn slider_value(&self, which: &Slider, frac: f64) {
        let v = frac.clamp(0.0, 1.0);
        match which {
            Slider::Volume => services::audio::set_volume(v, false),
            Slider::Mic => services::audio::set_volume(v, true),
            Slider::Brightness => services::power::set_brightness(v),
            Slider::Glass(_) => {}
        }
    }

    fn apply_slider(&mut self, i: usize, id: Id, x: f64) {
        let Some(h) = self.surfs[i].hits.iter().find(|h| h.id == id).cloned() else { return };
        if let Action::Slider(which) = &h.action {
            let frac = ((x - h.rect.x - h.rect.h / 2.0) / (h.rect.w - h.rect.h).max(1.0)).clamp(0.0, 1.0);
            self.slider_value(which, frac);
            match which {
                Slider::Volume => {
                    if let Some(a) = self.data.audio.as_mut() {
                        a.volume = frac;
                    }
                }
                Slider::Mic => {
                    if let Some(a) = self.data.mic.as_mut() {
                        a.volume = frac;
                    }
                }
                Slider::Brightness => self.data.brightness = Some(frac),
                Slider::Glass(_) => {}
            }
            self.mark_all();
        }
    }

    pub fn on_key(&mut self, ev: KeyEvent) {
        let Some(uid) = self.kb_surface else { return };
        let Some(i) = self.uid_idx(uid) else { return };
        let kind = self.surfs[i].kind;
        // Generic focus navigation.
        match ev.keysym {
            Keysym::Escape => {
                if !self.views.escape(kind) {
                    self.close(kind);
                }
                return;
            }
            Keysym::Tab | Keysym::ISO_Left_Tab => {
                let back = ev.keysym == Keysym::ISO_Left_Tab || self.modifiers.shift;
                let s = &mut self.surfs[i];
                let focusables: Vec<Id> = s.hits.iter().filter(|h| h.focusable).map(|h| h.id).collect();
                if !focusables.is_empty() {
                    let pos = s.focus.and_then(|f| focusables.iter().position(|&x| x == f));
                    let next = match (pos, back) {
                        (None, false) => 0,
                        (None, true) => focusables.len() - 1,
                        (Some(p), false) => (p + 1) % focusables.len(),
                        (Some(p), true) => (p + focusables.len() - 1) % focusables.len(),
                    };
                    s.focus = Some(focusables[next]);
                    s.dirty = true;
                }
                return;
            }
            _ => {}
        }
        let s = &self.surfs[i];
        let focused_hit = s.focus.and_then(|f| s.hits.iter().find(|h| h.id == f)).cloned();
        if let Some(action) = self.views.key(kind, &self.data, ev.keysym, ev.utf8.as_deref(), &self.modifiers, focused_hit.as_ref()) {
            self.perform(action);
        } else if matches!(ev.keysym, Keysym::Return | Keysym::KP_Enter | Keysym::space) {
            if let Some(h) = focused_hit {
                if h.role != Role::Input {
                    self.perform(h.action);
                }
            }
        } else if matches!(ev.keysym, Keysym::Left | Keysym::Right) {
            if let Some(h) = focused_hit {
                if let (Action::Slider(w), Some(v)) = (&h.action, h.value) {
                    let nv = (v + if ev.keysym == Keysym::Right { 0.05 } else { -0.05 }).clamp(0.0, 1.0);
                    self.slider_value(w, nv);
                }
            }
        }
        self.mark(kind);
        self.respec(kind);
    }

    // ───────────────────────────── actions ─────────────────────────────

    pub fn open(&mut self, kind: Kind) {
        if self.is_open(kind) {
            return;
        }
        // Only one popup at a time (menus may stack on top of the bar).
        let popups: Vec<Kind> = self.surfs.iter().filter(|s| s.kind.is_popup() && !s.closing).map(|s| s.kind).collect();
        for k in popups {
            self.close(k);
        }
        if kind == Kind::Snap {
            // Snap / window menu always acts on the window that is focused right now.
            self.views.snap_target = self.data.active_addr.clone();
        }
        self.views.on_open(kind, &mut self.data);
        if kind == Kind::Overview && !self.views.overview_launchpad {
            self.capture_windows();
        }
        if matches!(kind, Kind::Dashboard) {
            self.data.uptime = uptime();
        }
        if matches!(kind, Kind::Control | Kind::NotifCenter | Kind::Dashboard) {
            // Popups would overlap the panel; they stay in history.
            self.data.notifs.clear();
            let uids: Vec<u64> = self.surfs.iter().filter(|s| s.kind == Kind::Notifications).map(|s| s.uid).collect();
            for u in uids {
                self.destroy(u);
            }
        }
        services::on_open(kind, &self.tx, &self.theme.cfg);
        let out = self.focused_output();
        self.create(kind, out);
        self.mark(Kind::Bar);
        self.mark(Kind::Dock);
    }

    pub fn close(&mut self, kind: Kind) {
        let motion_off = self.theme.motion() == creed_core::config::Motion::Off;
        let uids: Vec<u64> = self.surfs.iter().filter(|s| s.kind == kind).map(|s| s.uid).collect();
        for uid in uids {
            if motion_off {
                self.destroy(uid);
            } else if let Some(i) = self.uid_idx(uid) {
                let s = &mut self.surfs[i];
                s.closing = true;
                s.dirty = true;
                // Release keyboard immediately.
                s.layer.set_keyboard_interactivity(KeyboardInteractivity::None);
                s.layer.commit();
            }
        }
        self.mark(Kind::Bar);
        self.mark(Kind::Dock);
    }

    /// Requests a fresh preview of every window (Hyprland only).
    fn capture_windows(&mut self) {
        let Some(mgr) = self.toplevel_export.clone() else { return };
        let addrs: Vec<String> = self.data.clients.iter().filter(|c| c.mapped && c.workspace.id > 0).take(24).map(|c| c.address.clone()).collect();
        for addr in addrs {
            if self.captures.iter().any(|p| p.addr == addr) {
                continue;
            }
            let Some(handle) = crate::thumbs::handle_of(&addr) else { continue };
            let frame = mgr.capture_toplevel(0, handle, &self.qh, ());
            self.captures.push(crate::thumbs::Pending { addr, frame, format: None, flags: 0, pool: None, buffer: None });
        }
    }

    pub fn toggle(&mut self, kind: Kind) {
        if self.is_open(kind) {
            self.close(kind);
        } else {
            self.open(kind);
        }
    }

    pub fn close_popups(&mut self) {
        let popups: Vec<Kind> = self.surfs.iter().filter(|s| s.kind.is_popup()).map(|s| s.kind).collect();
        for k in popups {
            self.close(k);
        }
    }

    fn hypr_dispatch(&self, lua: &str) {
        if let Some(h) = &self.hypr {
            if let Err(e) = h.dispatch(lua) {
                eprintln!("creed-shell: hyprland dispatch failed: {e}");
            }
        }
    }

    pub fn perform(&mut self, action: Action) {
        use creed_core::hypr::lua_str;
        match action {
            Action::None => {}
            Action::Toggle(k) => {
                if k == Kind::Overview {
                    self.views.overview_launchpad = false;
                }
                self.toggle(k)
            }
            Action::Open(k) => {
                if k == Kind::Overview {
                    self.views.overview_launchpad = false;
                }
                self.open(k)
            }
            Action::Close(k) => self.close(k),
            Action::CloseAll => self.close_popups(),
            Action::Exec(cmd) => {
                services::spawn_shell(&cmd);
                self.close_popups();
            }
            Action::Launch(id) => {
                if let Some(app) = self.data.apps.iter().find(|a| a.id == id).cloned() {
                    services::apps::launch(&app, &self.theme.cfg.general.terminal);
                } else {
                    services::apps::launch_id(&id);
                }
                self.close_popups();
            }
            Action::Hypr(lua) => self.hypr_dispatch(&lua),
            Action::FocusWindow(addr) => {
                let win = lua_str(&format!("address:{addr}"));
                if self.data.is_minimized(&addr) {
                    // Restore a minimised window onto the current workspace first.
                    let ws = if self.data.active_ws > 0 { self.data.active_ws } else { 1 };
                    self.hypr_dispatch(&format!("hl.dsp.window.move({{ workspace = {ws}, window = {win} }})"));
                }
                self.hypr_dispatch(&format!("hl.dsp.focus({{ window = {win} }})"));
                self.close_popups();
            }
            Action::CloseWindow(addr) => {
                self.hypr_dispatch(&format!("hl.dsp.window.close({{ window = {} }})", lua_str(&format!("address:{addr}"))));
            }
            Action::Workspace(n) => self.hypr_dispatch(&format!("hl.dsp.focus({{ workspace = {n} }})")),
            Action::WorkspaceScroll(d) => {
                self.hypr_dispatch(&format!("hl.dsp.focus({{ workspace = {} }})", lua_str(if d > 0 { "e-1" } else { "e+1" })))
            }
            Action::Slider(_) => {}
            Action::ToggleMute => services::audio::toggle_mute(false),
            Action::ToggleMicMute => services::audio::toggle_mute(true),
            Action::VolumeStep(d) => services::audio::step(d),
            Action::BrightnessStep(d) => services::power::step_brightness(d),
            Action::ToggleWifi => services::network::toggle_wifi(!self.data.net.wifi_enabled, &self.tx),
            Action::ToggleBluetooth => {
                let on = self.data.bt.as_ref().map(|b| !b.powered).unwrap_or(true);
                services::bluetooth::set_power(on, &self.tx);
            }
            Action::ToggleDnd => {
                self.data.dnd = !self.data.dnd;
                if self.data.dnd {
                    self.data.notifs.clear();
                    self.respec(Kind::Notifications);
                }
                self.mark_all();
            }
            Action::ToggleGame => {
                let on = !self.data.game;
                self.set_game(on, None);
            }
            Action::ToggleNightLight => {
                self.data.night_light = !self.data.night_light;
                services::spawn_shell(if self.data.night_light {
                    "pkill -x hyprsunset; hyprsunset -t 4500 >/dev/null 2>&1 &"
                } else {
                    "pkill -x hyprsunset"
                });
                self.mark_all();
            }
            Action::PowerProfile(p) => services::power::set_profile(&p, &self.tx),
            Action::MediaPlayPause => services::mpris::call(&self.data.media, "PlayPause"),
            Action::MediaNext => services::mpris::call(&self.data.media, "Next"),
            Action::MediaPrev => services::mpris::call(&self.data.media, "Previous"),
            Action::NotifDismiss(id) => {
                self.data.notifs.retain(|n| n.id != id);
                self.data.history.retain(|n| n.id != id);
                if let Some(b) = &self.notif_bus {
                    b.closed(id, 2);
                }
                self.respec(Kind::Notifications);
                self.mark_all();
            }
            Action::NotifAction(id, key) => {
                if let Some(b) = &self.notif_bus {
                    b.action(id, &key);
                    b.closed(id, 2);
                }
                self.data.notifs.retain(|n| n.id != id);
                self.respec(Kind::Notifications);
                self.mark_all();
            }
            Action::NotifClear => {
                self.data.history.clear();
                self.data.notifs.clear();
                self.respec(Kind::Notifications);
                self.mark_all();
            }
            Action::TrayActivate(key) => services::tray::activate(&key, false),
            Action::AppMenu(title, x) => {
                let same = self.is_open(Kind::Menu) && self.views.app_menu.as_ref().map(|(t, _)| t == &title).unwrap_or(false);
                if same {
                    self.close(Kind::Menu);
                } else {
                    let app = self.data.active_client().map(|c| Data::pretty_class(&c.class)).unwrap_or_else(|| "Finder".into());
                    self.data.menu = views::mac::menu_entries(&title, &app);
                    self.data.menu_owner = String::new();
                    self.close(Kind::Menu);
                    let uids: Vec<u64> = self.surfs.iter().filter(|s| s.kind == Kind::Menu).map(|s| s.uid).collect();
                    for u in uids {
                        self.destroy(u);
                    }
                    self.open(Kind::Menu);
                    self.views.app_menu = Some((title, x as f64));
                }
            }
            Action::TrayMenu(key) => {
                self.views.app_menu = None;
                if let Some(item) = self.data.tray.iter().find(|t| t.key == key).cloned() {
                    if item.menu.is_some() {
                        self.data.menu_owner = key.clone();
                        self.data.menu.clear();
                        services::tray::fetch_menu(&item, &self.tx);
                        self.open(Kind::Menu);
                    } else {
                        services::tray::activate(&key, true);
                    }
                }
            }
            Action::MenuItem(id) => {
                if let Some((title, _)) = self.views.app_menu.clone() {
                    self.close(Kind::Menu);
                    if let Some(a) = views::mac::menu_action(&title, id) {
                        self.perform(a);
                    }
                    return;
                }
                if let Some(item) = self.data.tray.iter().find(|t| t.key == self.data.menu_owner).cloned() {
                    services::tray::menu_event(&item, id);
                }
                self.close(Kind::Menu);
            }
            Action::ClipboardPick(id) => {
                services::clipboard::pick(&id);
                self.close_popups();
            }
            Action::EmojiPick(e) => {
                services::clipboard::copy_text(&e);
                self.views.emoji_recent(&e);
                self.close_popups();
            }
            Action::LauncherPick(i) => {
                if let Some(a) = self.views.launcher_result(&self.data, i) {
                    self.perform(a);
                }
            }
            Action::SideTab(i) => {
                self.views.side_tab = i.min(1);
                self.views.query.clear();
                self.views.scroll = 0.0;
                self.mark(Kind::Control);
            }
            Action::TodoAdd => {
                views::todo::add(&self.views.query);
                self.views.query.clear();
                self.mark(Kind::Control);
            }
            Action::TodoToggle(i) => {
                views::todo::toggle(i);
                self.mark(Kind::Control);
            }
            Action::TodoDelete(i) => {
                views::todo::delete(i);
                self.mark(Kind::Control);
            }
            Action::Page(p) => {
                self.views.set_page(p);
                self.mark_all();
                for k in [Kind::Control, Kind::Dashboard, Kind::Launcher, Kind::Emoji, Kind::Overview] {
                    self.respec(k);
                }
            }
            Action::WifiConnect(ssid) => {
                let secure = self.data.aps.iter().find(|a| a.ssid == ssid).map(|a| a.secure).unwrap_or(false);
                if secure && self.views.wifi_password_for.as_deref() != Some(&ssid) && !services::network::known(&ssid) {
                    self.views.wifi_password_for = Some(ssid);
                    self.views.password.clear();
                    self.mark_all();
                } else {
                    let pw = if self.views.wifi_password_for.as_deref() == Some(&ssid) { Some(self.views.password.clone()) } else { None };
                    services::network::connect(&ssid, pw.as_deref(), &self.tx);
                    self.views.wifi_password_for = None;
                    self.views.password.clear();
                    self.mark_all();
                }
            }
            Action::Power(what) => {
                self.close_popups();
                services::power::session(what);
            }
            Action::Snap(layout, zone) => {
                let addr = self.views.snap_target.clone();
                if let Some(lua) = views::snap::dispatch_for(layout, zone, &addr, &self.data, &self.theme) {
                    for l in lua {
                        self.hypr_dispatch(&l);
                    }
                }
                self.close_popups();
            }
            Action::Calendar(d) => {
                self.views.calendar_offset = if d == 0 { 0 } else { self.views.calendar_offset + d };
                self.mark_all();
            }
            Action::Mode(m) => {
                services::spawn_shell(&format!("creed mode set {}", shell_quote(&m)));
                self.close_popups();
            }
        }
    }

    pub fn set_game(&mut self, on: bool, pid: Option<i64>) {
        if self.data.game == on {
            return;
        }
        self.data.game = on;
        services::game::apply(on, pid, &self.theme.cfg, self.hypr.as_ref());
        let unload = self.theme.cfg.game.unload_shell;
        if unload {
            self.game_hidden = on;
            if on {
                self.close_popups();
            }
            self.sync_persistent();
            if on {
                // Wallpaper is invisible behind a fullscreen game: free its buffer.
                for s in self.surfs.iter_mut().filter(|s| s.kind == Kind::Wallpaper) {
                    s.buffer = None;
                    s.pool = None;
                }
                crate::trim_memory();
            } else {
                self.mark(Kind::Wallpaper);
            }
        }
        self.data.osd = Some((OsdKind::Game, if on { 1.0 } else { 0.0 }, Instant::now()));
        self.show_osd();
        self.mark_all();
    }

    /// Once the compositor has released the wallpaper buffer it holds its own copy (GPU
    /// compositors such as Hyprland upload shm buffers to a texture), so ours can go.
    fn release_static_buffers(&mut self) {
        let mut freed = false;
        for s in self.surfs.iter_mut().filter(|s| s.kind == Kind::Wallpaper && !s.dirty) {
            if let (Some(b), Some(pool)) = (s.buffer.as_ref(), s.pool.as_mut()) {
                if b.canvas(pool).is_some() {
                    s.buffer = None;
                    s.pool = None;
                    freed = true;
                }
            }
        }
        if freed {
            crate::trim_memory();
        }
    }

    pub fn show_osd(&mut self) {
        // Reuse an OSD that is open *or still fading out* - never stack a second one on top.
        if self.surfs.iter().any(|s| s.kind == Kind::Osd) {
            for s in self.surfs.iter_mut().filter(|s| s.kind == Kind::Osd) {
                s.closing = false;
                s.dirty = true;
            }
        } else {
            let out = self.focused_output();
            self.create(Kind::Osd, out);
        }
    }

    // ───────────────────────────── messages ─────────────────────────────

    pub fn on_msg(&mut self, m: Msg) {
        match m {
            Msg::Hypr(ev) => self.on_hypr(ev),
            Msg::Audio(a) => {
                let changed = self.data.audio.is_some() && a.is_some() && self.data.audio != a;
                self.data.audio = a.clone();
                if changed && self.started.elapsed().as_secs() > 2 {
                    if let Some(a) = a {
                        self.data.osd = Some((OsdKind::Volume, if a.muted { 0.0 } else { a.volume }, Instant::now()));
                        self.show_osd();
                    }
                }
                self.mark(Kind::Bar);
                self.mark(Kind::Control);
                self.mark(Kind::Osd);
            }
            Msg::Mic(a) => {
                // Mic-mute key feedback: show the OSD when the mute state flips (not on level drift).
                let flipped = matches!((&self.data.mic, &a), (Some(o), Some(n)) if o.muted != n.muted);
                if flipped && self.started.elapsed().as_secs() > 2 {
                    if let Some(n) = &a {
                        self.data.osd = Some((OsdKind::Mic, if n.muted { 0.0 } else { n.volume }, Instant::now()));
                        self.show_osd();
                    }
                }
                self.data.mic = a;
                self.mark(Kind::Control);
            }
            Msg::Brightness(b) => {
                let changed = self.data.brightness.is_some() && b.is_some() && self.data.brightness != b;
                self.data.brightness = b;
                if changed && self.started.elapsed().as_secs() > 2 {
                    self.data.osd = Some((OsdKind::Brightness, b.unwrap_or(0.0), Instant::now()));
                    self.show_osd();
                }
                self.mark(Kind::Control);
                self.mark(Kind::Osd);
            }
            Msg::Battery(b) => {
                self.data.battery = b;
                self.mark(Kind::Bar);
                self.mark(Kind::Control);
            }
            Msg::PowerProfile(p) => {
                self.data.power_profile = p;
                self.mark(Kind::Control);
            }
            Msg::Net(n) => {
                self.data.net = n;
                self.mark(Kind::Bar);
                self.mark(Kind::Control);
            }
            Msg::Aps(a) => {
                self.data.aps = a;
                self.respec(Kind::Control);
            }
            Msg::Bt(b) => {
                self.data.bt = b;
                self.mark(Kind::Bar);
                self.mark(Kind::Control);
            }
            Msg::Media(m) => {
                self.data.media = m;
                self.respec(Kind::Bar);
                self.mark(Kind::Dashboard);
                self.mark(Kind::Control);
                self.mark(Kind::NotifCenter);
            }
            Msg::Notify(n) => {
                let id = n.id;
                self.data.history.retain(|h| h.id != id);
                self.data.history.insert(0, n.clone());
                self.data.history.truncate(self.theme.cfg.notifications.history as usize);
                if !self.data.dnd || n.urgency >= 2 {
                    self.data.notifs.retain(|h| h.id != id);
                    self.data.notifs.insert(0, n.clone());
                    let max = self.theme.cfg.notifications.max_visible as usize;
                    self.data.notifs.truncate(max);
                    let timeout = if n.timeout_ms > 0 {
                        n.timeout_ms as u64
                    } else if n.timeout_ms == 0 || n.urgency >= 2 {
                        0
                    } else {
                        self.theme.cfg.notifications.timeout_ms as u64
                    };
                    if timeout > 0 {
                        let tx = self.tx.clone();
                        std::thread::spawn(move || {
                            std::thread::sleep(std::time::Duration::from_millis(timeout));
                            let _ = tx.send(Msg::NotifClose(id));
                        });
                    }
                    if !self.game_hidden || n.urgency >= 2 {
                        if !self.surfs.iter().any(|s| s.kind == Kind::Notifications) {
                            let out = self.focused_output();
                            self.create(Kind::Notifications, out);
                        }
                        self.respec(Kind::Notifications);
                    }
                }
                self.mark(Kind::Bar);
                self.mark(Kind::NotifCenter);
            }
            Msg::NotifClose(id) => {
                self.data.notifs.retain(|n| n.id != id);
                if self.data.notifs.is_empty() {
                    let uids: Vec<u64> = self.surfs.iter().filter(|s| s.kind == Kind::Notifications).map(|s| s.uid).collect();
                    for u in uids {
                        self.destroy(u);
                    }
                } else {
                    self.respec(Kind::Notifications);
                }
            }
            Msg::Tray(t) => {
                self.data.tray = t;
                self.respec(Kind::Bar);
            }
            Msg::Menu(owner, entries) => {
                if owner == self.data.menu_owner {
                    self.data.menu = entries;
                    self.respec(Kind::Menu);
                }
            }
            Msg::Apps(a) => {
                self.data.apps = a;
                self.respec(Kind::Launcher);
                self.respec(Kind::Dock);
                self.respec(Kind::Overview);
            }
            Msg::Recent(r) => {
                self.data.recent = r;
                self.mark(Kind::Launcher);
            }
            Msg::Clips(c) => {
                self.data.clips = c;
                self.mark(Kind::Clipboard);
            }
            Msg::Sys(s) => {
                self.data.sys = s;
                self.mark(Kind::Dashboard);
                self.mark(Kind::Bar);
            }
            Msg::Weather(w) => {
                self.data.weather = w;
                self.mark(Kind::Dashboard);
                self.mark(Kind::Bar);
            }
            Msg::Ipc(cmd, stream) => self.on_ipc(&cmd, stream),
            Msg::A11y(uid, node, action) => {
                if let Some(i) = self.uid_idx(uid) {
                    let hit = self.surfs[i].hits.iter().find(|h| h.id == node).cloned();
                    if let Some(h) = hit {
                        match action {
                            accesskit::Action::Focus => {
                                self.surfs[i].focus = Some(node);
                                self.surfs[i].dirty = true;
                            }
                            accesskit::Action::Increment | accesskit::Action::Decrement => {
                                if let (Action::Slider(w), Some(v)) = (&h.action, h.value) {
                                    let d = if action == accesskit::Action::Increment { 0.05 } else { -0.05 };
                                    self.slider_value(w, v + d);
                                }
                            }
                            _ => self.perform(h.action),
                        }
                    }
                }
            }
            Msg::ConfigChanged => self.reload(),
            Msg::Tick => {
                self.release_static_buffers();
                let t = crate::time::now_secs();
                let minute = t / 60;
                if minute != self.minute_marker {
                    self.minute_marker = minute;
                    self.mark(Kind::Bar);
                    self.mark(Kind::Dashboard);
                    self.mark(Kind::Calendar);
                    self.mark(Kind::NotifCenter);
                }
                if self.theme.cfg.bar.clock_format.contains("%S") || self.theme.cfg.bar.clock_format.contains("%T") {
                    self.mark(Kind::Bar);
                }
                // OSD auto-hide
                if let Some((_, _, at)) = self.data.osd {
                    if at.elapsed().as_millis() > 1600 && self.is_open(Kind::Osd) {
                        self.close(Kind::Osd);
                    }
                }
            }
        }
    }

    pub fn refresh_hypr(&mut self) {
        let Some(h) = self.hypr.clone() else { return };
        self.data.workspaces = h.workspaces();
        self.data.workspaces.sort_by_key(|w| w.id);
        self.data.clients = h.clients();
        self.data.monitors = h.monitors();
        if let Some(m) = self.data.monitors.iter().find(|m| m.focused) {
            self.data.focused_monitor = m.name.clone();
            self.data.active_ws = m.active_workspace.id;
        }
        self.data.active_addr = h.active_window().map(|c| c.address).unwrap_or_default();
        self.data.hypr_ok = true;
    }

    fn on_hypr(&mut self, ev: HyprEvent) {
        match &ev {
            HyprEvent::Workspace { id, .. } => self.data.active_ws = *id,
            HyprEvent::FocusedMonitor { monitor, workspace } => {
                self.data.focused_monitor = monitor.clone();
                self.data.active_ws = *workspace;
            }
            HyprEvent::ActiveWindow { address } => self.data.active_addr = address.clone(),
            HyprEvent::MonitorAdded(_) | HyprEvent::MonitorRemoved(_) => {}
            HyprEvent::Other { name, data } if name == "activelayout" => {
                self.data.kb_layout = data.rsplit(',').next().unwrap_or_default().to_string();
            }
            _ => {}
        }
        if matches!(
            ev,
            HyprEvent::OpenWindow { .. }
                | HyprEvent::CloseWindow { .. }
                | HyprEvent::MoveWindow { .. }
                | HyprEvent::WindowTitle { .. }
                | HyprEvent::WorkspaceCreated { .. }
                | HyprEvent::WorkspaceDestroyed { .. }
                | HyprEvent::Workspace { .. }
                | HyprEvent::ActiveWindow { .. }
                | HyprEvent::Fullscreen(_)
                | HyprEvent::ConfigReloaded
        ) {
            self.refresh_hypr();
        }
        // Game detection.
        if self.theme.cfg.game.auto_detect {
            let game = services::game::detect(&self.data.clients, &self.theme.cfg.game.extra_classes);
            match (game, self.data.game) {
                (Some(pid), false) => self.set_game(true, Some(pid)),
                (None, true) if services::game::auto_started() => self.set_game(false, None),
                _ => {}
            }
        }
        self.respec(Kind::Bar);
        self.respec(Kind::Dock);
        self.mark(Kind::Overview);
    }

    pub fn reload(&mut self) {
        let (theme, err) = load_theme(&self.paths);
        let icon_theme = theme.cfg.style.icon_theme.clone();
        let a11y_was = self.theme.cfg.a11y.screen_reader;
        self.theme = theme;
        self.config_error = err.clone();
        self.icons.set_theme(&icon_theme);
        let shell_icons = self.theme.cfg.style.shell_icons.clone();
        self.icons.set_shell(shell_icons.as_deref());
        if let Some(e) = err {
            eprintln!("creed-shell: config error: {e}");
            self.on_msg(Msg::Notify(Notif {
                id: 900_000 + (crate::time::now_secs() % 1000) as u32,
                app: "Creed".into(),
                icon: "dialog-warning-symbolic".into(),
                summary: "Config has an error - keeping the last good settings".into(),
                body: e,
                actions: vec![],
                urgency: 1,
                timeout_ms: 10000,
                arrived: crate::time::now_secs(),
            }));
        }
        // Recreate everything so mode changes (variants, layers, sizes) apply cleanly.
        self.close_popups();
        let uids: Vec<u64> = self.surfs.iter().filter(|s| !s.kind.is_popup() && s.kind != Kind::Notifications).map(|s| s.uid).collect();
        for u in uids {
            self.destroy(u);
        }
        if a11y_was != self.theme.cfg.a11y.screen_reader {
            for s in &mut self.surfs {
                s.a11y = None;
            }
        }
        self.sync_persistent();
        crate::trim_memory();
    }

    fn on_ipc(&mut self, cmd: &str, mut stream: UnixStream) {
        let mut parts = cmd.split_whitespace();
        let verb = parts.next().unwrap_or("");
        let arg = parts.next().unwrap_or("");
        let reply: String = match verb {
            "ping" => "pong".into(),
            "toggle" | "open" | "close" => match Kind::parse(arg) {
                Some(k) => {
                    if k == Kind::Overview {
                        self.views.overview_launchpad = parts.next() == Some("apps") || arg == "launchpad";
                    }
                    match verb {
                        "toggle" => self.toggle(k),
                        "open" => self.open(k),
                        _ => self.close(k),
                    }
                    "ok".into()
                }
                None => format!("error: unknown surface '{arg}'"),
            },
            "close-all" => {
                self.close_popups();
                "ok".into()
            }
            "reload" => {
                self.reload();
                match &self.config_error {
                    Some(e) => format!("error: {e}"),
                    None => "ok".into(),
                }
            }
            "game" => {
                match arg {
                    "on" => self.set_game(true, None),
                    "off" => self.set_game(false, None),
                    "toggle" => {
                        let on = !self.data.game;
                        self.set_game(on, None)
                    }
                    _ => {}
                }
                if self.data.game { "on".into() } else { "off".into() }
            }
            "osd" => {
                // `osd <kind>` re-reads that state; `osd <kind> <value>` shows that OSD directly
                // (used by tests and scripts).
                let value = cmd.split_whitespace().nth(2).and_then(|v| v.parse::<f64>().ok());
                let kind = match arg {
                    "volume" => Some(OsdKind::Volume),
                    "mic" => Some(OsdKind::Mic),
                    "brightness" => Some(OsdKind::Brightness),
                    "game" => Some(OsdKind::Game),
                    _ => None,
                };
                match (kind, value) {
                    (None, _) => "error: unknown osd kind".into(),
                    (Some(k), Some(v)) => {
                        self.data.osd = Some((k, v.clamp(0.0, 1.0), Instant::now()));
                        self.show_osd();
                        "ok".into()
                    }
                    (Some(OsdKind::Brightness), None) => {
                        services::power::refresh_brightness(&self.tx);
                        "ok".into()
                    }
                    (Some(OsdKind::Game), None) => "error: game osd needs a value".into(),
                    (Some(_), None) => {
                        services::audio::refresh(&self.tx);
                        "ok".into()
                    }
                }
            }
            "dump-layout" => {
                let mut out = vec![];
                for s in &self.surfs {
                    out.push(serde_json::json!({
                        "kind": s.kind.name(),
                        "size": [s.w, s.h],
                        "scale": s.scale(),
                        "configured": s.configured,
                        "boxes": s.boxes,
                        "hits": s.hits.iter().map(|h| serde_json::json!({
                            "label": h.label, "role": h.role, "rect": [h.rect.x, h.rect.y, h.rect.w, h.rect.h]
                        })).collect::<Vec<_>>(),
                    }));
                }
                serde_json::to_string(&out).unwrap_or_default()
            }
            "click" => {
                // Test hook: click the hit whose label matches (in the given surface).
                let label: String = parts.collect::<Vec<_>>().join(" ");
                let found = self
                    .surfs
                    .iter()
                    .filter(|s| s.kind.name() == arg)
                    .find_map(|s| s.hits.iter().find(|h| h.label == label).cloned());
                match found {
                    Some(h) => {
                        self.perform(h.action);
                        "ok".into()
                    }
                    None => format!("error: no '{label}' in {arg}"),
                }
            }
            "type" => {
                let text: String = parts.collect::<Vec<_>>().join(" ");
                let text = if text.is_empty() { arg.to_string() } else { format!("{arg} {text}") };
                if let Some(k) = self.surfs.iter().find(|s| s.kind.is_popup() && !s.closing).map(|s| s.kind) {
                    self.views.type_text(k, &text);
                    self.mark(k);
                    self.respec(k);
                    "ok".into()
                } else {
                    "error: no popup open".into()
                }
            }
            "notify-test" => {
                let n = Notif {
                    id: 800_000 + self.data.history.len() as u32,
                    app: "Creed".into(),
                    icon: "preferences-desktop".into(),
                    summary: "Welcome to Creed".into(),
                    body: "Notifications land here. Click to dismiss, or open the notification center.".into(),
                    actions: vec![("default".into(), "Open".into())],
                    urgency: 1,
                    timeout_ms: -1,
                    arrived: crate::time::now_secs(),
                };
                self.on_msg(Msg::Notify(n));
                "ok".into()
            }
            "state" => serde_json::json!({
                "mode": self.theme.cfg.general.mode,
                "game": self.data.game,
                "surfaces": self.surfs.iter().map(|s| s.kind.name()).collect::<BTreeSet<_>>(),
                "config_error": self.config_error,
            })
            .to_string(),
            "export-wallpaper" => {
                let out = std::path::PathBuf::from(arg);
                let (w, h) = self.output_size(self.focused_output().as_ref());
                let scale = self.output_state.outputs().next().and_then(|o| self.output_state.info(&o)).map(|i| i.scale_factor.max(1)).unwrap_or(1) as u32;
                if arg.is_empty() || !out.is_absolute() {
                    "error: export-wallpaper needs an absolute path".into()
                } else {
                    match views::wallpaper::export_png(&self.theme, &self.paths, w * scale, h * scale, &out) {
                        Ok(()) => "ok".into(),
                        Err(e) => format!("error: {e}"),
                    }
                }
            }
            "quit" => {
                self.exit = true;
                "bye".into()
            }
            _ => format!("error: unknown command '{verb}'"),
        };
        let _ = stream.write_all(reply.as_bytes());
        let _ = stream.write_all(b"\n");
    }
}

fn hostname() -> String {
    let mut buf = [0u8; 256];
    let ok = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) } == 0;
    if !ok {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).to_string()
}

pub fn uptime() -> String {
    let secs = std::fs::read_to_string("/proc/uptime").ok().and_then(|s| s.split_whitespace().next()?.parse::<f64>().ok()).unwrap_or(0.0) as u64;
    let (d, h, m) = (secs / 86400, (secs % 86400) / 3600, (secs % 3600) / 60);
    match (d, h) {
        (0, 0) => format!("{m} min"),
        (0, _) => format!("{h} h {m} min"),
        _ => format!("{d} d {h} h"),
    }
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

pub fn apply_spec(layer: &LayerSurface, spec: &Spec) {
    layer.set_anchor(spec.anchor);
    layer.set_size(spec.size.0, spec.size.1);
    layer.set_exclusive_zone(spec.exclusive);
    layer.set_margin(spec.margin.0, spec.margin.1, spec.margin.2, spec.margin.3);
    layer.set_keyboard_interactivity(spec.keyboard);
    layer.set_layer(spec.layer);
}

/// Input region: none for click-through surfaces, a rectangle when the spec asks for one.
fn apply_input(comp: &CompositorState, layer: &LayerSurface, spec: &Spec) {
    if spec.click_through {
        if let Ok(r) = Region::new(comp) {
            layer.wl_surface().set_input_region(Some(r.wl_region()));
        }
    } else if let Some((x, y, w, h)) = spec.input {
        if let Ok(r) = Region::new(comp) {
            r.add(x, y, w, h);
            layer.wl_surface().set_input_region(Some(r.wl_region()));
        }
    } else {
        layer.wl_surface().set_input_region(None);
    }
}

pub fn hit_at(hits: &[Hit], x: f64, y: f64) -> Option<&Hit> {
    hits.iter().rev().find(|h| h.rect.contains(x, y))
}

// ───────────────────────────── sctk handlers ─────────────────────────────

impl CompositorHandler for App {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, f: i32) {
        if let Some(i) = self.surf_idx(surface) {
            let s = &mut self.surfs[i];
            if s.int_scale != f {
                s.int_scale = f.max(1);
                s.dirty = true;
            }
        }
    }
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, surface: &wl_surface::WlSurface, _: u32) {
        if let Some(i) = self.surf_idx(surface) {
            let s = &mut self.surfs[i];
            s.frame_pending = false;
            if s.configured {
                self.draw(i);
            }
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.sync_persistent();
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.relayout();
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, output: wl_output::WlOutput) {
        let uids: Vec<u64> = self.surfs.iter().filter(|s| s.output.as_ref() == Some(&output)).map(|s| s.uid).collect();
        for u in uids {
            self.destroy(u);
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(i) = self.surf_idx(layer.wl_surface()) {
            let uid = self.surfs[i].uid;
            let kind = self.surfs[i].kind;
            self.destroy(uid);
            if matches!(kind, Kind::Bar | Kind::Dock | Kind::Wallpaper) {
                self.sync_persistent();
            }
        }
    }
    fn configure(&mut self, _: &Connection, _: &QueueHandle<Self>, layer: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        let Some(i) = self.surf_idx(layer.wl_surface()) else { return };
        let s = &mut self.surfs[i];
        let (w, h) = c.new_size;
        let (nw, nh) = (if w == 0 { s.spec.size.0 } else { w }, if h == 0 { s.spec.size.1 } else { h });
        if (nw, nh) != (s.w, s.h) {
            s.w = nw.max(1);
            s.h = nh.max(1);
            if s.kind == Kind::Wallpaper {
                s.buffer = None;
                s.pool = None;
            }
        }
        s.configured = true;
        s.dirty = true;
        if !s.frame_pending {
            self.draw(i);
        }
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat) {
        if self.text_input.is_none() {
            if let Some(m) = &self.text_input_mgr {
                self.text_input = Some(m.get_text_input(&seat, qh, ()));
            }
        }
    }
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            let kb = self.seat_state.get_keyboard_with_repeat(
                qh,
                &seat,
                None,
                self.loop_handle.clone(),
                Box::new(|app: &mut App, _, ev| {
                    app.on_key(ev);
                    app.draw_dirty();
                }),
            );
            self.keyboard = kb.ok();
        }
        if cap == Capability::Pointer && self.pointer.is_none() {
            if let Ok(p) = self.seat_state.get_pointer(qh, &seat) {
                self.cursor_device = self.cursor_shape.as_ref().map(|m| m.get_shape_device(&p, qh));
                self.pointer = Some(p);
            }
        }
        if self.text_input.is_none() {
            if let Some(m) = &self.text_input_mgr {
                self.text_input = Some(m.get_text_input(&seat, qh, ()));
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard {
            if let Some(k) = self.keyboard.take() {
                k.release();
            }
        }
        if cap == Capability::Pointer {
            if let Some(d) = self.cursor_device.take() {
                d.destroy();
            }
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {
        if let Some(i) = self.surf_idx(surface) {
            self.kb_surface = Some(self.surfs[i].uid);
            if let Some(a) = self.surfs[i].a11y.as_mut() {
                a.focus(true);
            }
            if views::has_text_input(self.surfs[i].kind) {
                if let Some(ti) = &self.text_input {
                    ti.enable();
                    ti.set_content_type(zwp_text_input_v3::ContentHint::None, zwp_text_input_v3::ContentPurpose::Normal);
                    ti.commit();
                }
            }
        }
    }
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, surface: &wl_surface::WlSurface, _: u32) {
        if let Some(i) = self.surf_idx(surface) {
            if let Some(a) = self.surfs[i].a11y.as_mut() {
                a.focus(false);
            }
            if self.kb_surface == Some(self.surfs[i].uid) {
                self.kb_surface = None;
            }
        }
        if let Some(ti) = &self.text_input {
            ti.disable();
            ti.commit();
        }
    }
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, ev: KeyEvent) {
        self.on_key(ev);
    }
    fn repeat_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_keyboard::WlKeyboard, _: u32, m: Modifiers, _: RawModifiers, _: u32) {
        self.modifiers = m;
    }
}

impl PointerHandler for App {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_pointer::WlPointer, events: &[PointerEvent]) {
        for ev in events {
            let Some(i) = self.surf_idx(&ev.surface) else { continue };
            let (x, y) = ev.position;
            match ev.kind {
                PointerEventKind::Enter { serial } => {
                    if let Some(d) = &self.cursor_device {
                        d.set_shape(serial, Shape::Default);
                    }
                    self.ptr_surface = Some(self.surfs[i].uid);
                    let s = &mut self.surfs[i];
                    s.pointer = Some((x, y));
                    s.hover = hit_at(&s.hits, x, y).map(|h| h.id);
                    s.dirty = true;
                    if s.kind == Kind::Dock {
                        self.views.dock_pointer = Some((x, y));
                        self.respec(Kind::Dock);
                    }
                }
                PointerEventKind::Leave { .. } => {
                    let s = &mut self.surfs[i];
                    s.pointer = None;
                    s.hover = None;
                    s.pressed = None;
                    s.dragging = None;
                    s.dirty = true;
                    if s.kind == Kind::Dock {
                        self.views.dock_pointer = None;
                        self.respec(Kind::Dock);
                    }
                }
                PointerEventKind::Motion { .. } => {
                    let s = &mut self.surfs[i];
                    s.pointer = Some((x, y));
                    let nh = hit_at(&s.hits, x, y).map(|h| h.id);
                    let glass_follow = self.theme.cfg.glass.specular_follow_cursor && self.theme.cfg.glass.specular > 0.0;
                    if nh != s.hover || glass_follow {
                        s.hover = nh;
                        s.dirty = true;
                    }
                    if let Some(d) = s.dragging {
                        self.apply_slider(i, d, x);
                    }
                    if self.surfs[i].kind == Kind::Dock {
                        self.views.dock_pointer = Some((x, y));
                        self.mark(Kind::Dock);
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let s = &mut self.surfs[i];
                    let hit = hit_at(&s.hits, x, y).cloned();
                    s.pressed = hit.as_ref().map(|h| h.id);
                    s.dirty = true;
                    if let Some(h) = hit {
                        if button == 0x110 {
                            if matches!(h.action, Action::Slider(_)) {
                                s.dragging = Some(h.id);
                                self.apply_slider(i, h.id, x);
                            }
                        } else if button == 0x111 {
                            if let Some(sec) = h.secondary.clone() {
                                self.perform(sec);
                            }
                        } else if button == 0x112 {
                            // Middle click on a notification / tray: dismiss / secondary.
                            if let Action::NotifAction(id, _) | Action::NotifDismiss(id) = h.action {
                                self.perform(Action::NotifDismiss(id));
                            }
                        }
                    }
                }
                PointerEventKind::Release { button, .. } => {
                    let Some(s) = self.surfs.get_mut(i) else { continue };
                    let pressed = s.pressed.take();
                    let was_drag = s.dragging.take().is_some();
                    s.dirty = true;
                    if button != 0x110 || was_drag {
                        continue;
                    }
                    let hit = hit_at(&s.hits, x, y).cloned();
                    if let (Some(h), Some(p)) = (hit, pressed) {
                        if h.id == p {
                            s.focus = None;
                            self.perform(h.action);
                        }
                    }
                }
                PointerEventKind::Axis { vertical, horizontal, .. } => {
                    let d = if vertical.discrete != 0 {
                        -vertical.discrete as f64
                    } else if vertical.absolute != 0.0 {
                        -vertical.absolute / 10.0
                    } else {
                        -horizontal.absolute / 10.0
                    };
                    if d == 0.0 {
                        continue;
                    }
                    let s = &self.surfs[i];
                    let kind = s.kind;
                    if let Some(h) = hit_at(&s.hits, x, y) {
                        if let Some(f) = h.scroll {
                            let a = f(d);
                            self.perform(a);
                            continue;
                        }
                    }
                    if self.views.scroll(kind, d) {
                        self.mark(kind);
                    }
                }
            }
        }
        self.draw_dirty();
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

smithay_client_toolkit::delegate_registry!(App);
smithay_client_toolkit::delegate_dispatch2!(App);

wayland_client::delegate_noop!(App: ignore WpFractionalScaleManagerV1);
wayland_client::delegate_noop!(App: ignore WpViewporter);
wayland_client::delegate_noop!(App: ignore WpViewport);
wayland_client::delegate_noop!(App: ignore ZwpTextInputManagerV3);

impl Dispatch<WpFractionalScaleV1, u32> for App {
    fn event(app: &mut App, _: &WpFractionalScaleV1, ev: wp_fractional_scale_v1::Event, surface_id: &u32, _: &Connection, _: &QueueHandle<App>) {
        if let wp_fractional_scale_v1::Event::PreferredScale { scale } = ev {
            if let Some(s) = app.surfs.iter_mut().find(|s| s.wl().id().protocol_id() == *surface_id) {
                let f = scale as f64 / 120.0;
                if s.frac_scale != Some(f) {
                    s.frac_scale = Some(f);
                    s.dirty = true;
                }
            }
        }
    }
}

impl Dispatch<ZwpTextInputV3, ()> for App {
    fn event(app: &mut App, ti: &ZwpTextInputV3, ev: zwp_text_input_v3::Event, _: &(), _: &Connection, _: &QueueHandle<App>) {
        match ev {
            zwp_text_input_v3::Event::PreeditString { text, .. } => app.ime_pending.0 = Some(text.unwrap_or_default()),
            zwp_text_input_v3::Event::CommitString { text } => app.ime_pending.1 = Some(text.unwrap_or_default()),
            zwp_text_input_v3::Event::Done { .. } => {
                let Some(uid) = app.kb_surface else { return };
                let Some(i) = app.uid_idx(uid) else { return };
                let kind = app.surfs[i].kind;
                let (pre, commit) = std::mem::take(&mut app.ime_pending);
                if let Some(c) = commit {
                    app.views.type_text(kind, &c);
                }
                app.views.preedit = pre.unwrap_or_default();
                // Tell the IME roughly where the caret is.
                if let Some(h) = app.surfs[i].hits.iter().find(|h| h.role == Role::Input) {
                    let r: Rect = h.rect;
                    ti.set_cursor_rectangle(r.x as i32, r.y as i32, r.w as i32, r.h as i32);
                    ti.commit();
                }
                app.mark(kind);
                app.respec(kind);
                app.draw_dirty();
            }
            _ => {}
        }
    }
}
