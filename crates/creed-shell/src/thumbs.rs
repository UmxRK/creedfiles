//! Live window previews for the overview through Hyprland's toplevel-export protocol.
//! Only used while the overview is open; every capture is downscaled to a small cairo
//! surface and the shared-memory buffer is released immediately. On compositors without
//! the protocol the overview simply keeps showing app icons.

pub mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables, non_upper_case_globals, non_snake_case, unused_imports, missing_docs, clippy::all)]
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/hyprland-toplevel-export-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/hyprland-toplevel-export-v1.xml");
}

use protocol::hyprland_toplevel_export_frame_v1::{self as frame, HyprlandToplevelExportFrameV1};
use protocol::hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1;
use smithay_client_toolkit::shm::slot::{Buffer, SlotPool};
use wayland_client::protocol::wl_shm;
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

/// Longest edge of a stored preview, in pixels.
pub const MAX_EDGE: f64 = 480.0;

pub struct Pending {
    pub addr: String,
    pub frame: HyprlandToplevelExportFrameV1,
    pub format: Option<(wl_shm::Format, u32, u32, u32)>,
    pub flags: u32,
    pub pool: Option<SlotPool>,
    pub buffer: Option<Buffer>,
}

/// Window address (`0x55d1a2b0…`) → protocol handle (its lower 32 bits).
pub fn handle_of(addr: &str) -> Option<u32> {
    u64::from_str_radix(addr.trim_start_matches("0x"), 16).ok().map(|v| (v & 0xffff_ffff) as u32)
}

/// Converts a captured shm frame into a downscaled cairo surface.
pub fn to_surface(data: &[u8], format: wl_shm::Format, w: u32, h: u32, stride: u32, y_invert: bool) -> Option<cairo::ImageSurface> {
    if w == 0 || h == 0 || data.len() < (stride * h) as usize {
        return None;
    }
    let swap = matches!(format, wl_shm::Format::Abgr8888 | wl_shm::Format::Xbgr8888);
    let opaque = matches!(format, wl_shm::Format::Xrgb8888 | wl_shm::Format::Xbgr8888);
    if !matches!(format, wl_shm::Format::Argb8888 | wl_shm::Format::Xrgb8888 | wl_shm::Format::Abgr8888 | wl_shm::Format::Xbgr8888) {
        return None;
    }
    let mut full = cairo::ImageSurface::create(cairo::Format::ARgb32, w as i32, h as i32).ok()?;
    let fs = full.stride() as usize;
    {
        let mut d = full.data().ok()?;
        for y in 0..h as usize {
            let sy = if y_invert { h as usize - 1 - y } else { y };
            let src = &data[sy * stride as usize..sy * stride as usize + w as usize * 4];
            let dst = &mut d[y * fs..y * fs + w as usize * 4];
            for x in 0..w as usize {
                let i = x * 4;
                let (b, g, r) = if swap { (src[i + 2], src[i + 1], src[i]) } else { (src[i], src[i + 1], src[i + 2]) };
                dst[i] = b;
                dst[i + 1] = g;
                dst[i + 2] = r;
                dst[i + 3] = if opaque { 255 } else { src[i + 3] };
            }
        }
    }
    let k = (MAX_EDGE / w.max(h) as f64).min(1.0);
    let (tw, th) = (((w as f64) * k).round().max(1.0) as i32, ((h as f64) * k).round().max(1.0) as i32);
    let small = cairo::ImageSurface::create(cairo::Format::ARgb32, tw, th).ok()?;
    {
        let cr = cairo::Context::new(&small).ok()?;
        cr.scale(k, k);
        cr.set_source_surface(&full, 0.0, 0.0).ok()?;
        cr.source().set_filter(cairo::Filter::Good);
        cr.paint().ok()?;
    }
    small.flush();
    Some(small)
}

wayland_client::delegate_noop!(crate::app::App: ignore HyprlandToplevelExportManagerV1);

impl Dispatch<HyprlandToplevelExportFrameV1, ()> for crate::app::App {
    fn event(app: &mut crate::app::App, f: &HyprlandToplevelExportFrameV1, ev: frame::Event, _: &(), _: &Connection, _: &QueueHandle<crate::app::App>) {
        let Some(i) = app.captures.iter().position(|p| &p.frame == f) else { return };
        match ev {
            frame::Event::Buffer { format: WEnum::Value(fmt), width, height, stride } => {
                if app.captures[i].format.is_none() {
                    app.captures[i].format = Some((fmt, width, height, stride));
                }
            }
            frame::Event::Flags { flags: WEnum::Value(fl) } => app.captures[i].flags = fl.bits(),
            frame::Event::BufferDone => {
                let Some((fmt, w, h, stride)) = app.captures[i].format else {
                    let p = app.captures.remove(i);
                    p.frame.destroy();
                    return;
                };
                let size = (stride * h) as usize;
                let made = SlotPool::new(size.max(4096), &app.shm).ok().and_then(|mut pool| {
                    let buffer = pool.create_buffer(w as i32, h as i32, stride as i32, fmt).ok().map(|(b, _)| b)?;
                    Some((pool, buffer))
                });
                match made {
                    Some((pool, buffer)) => {
                        f.copy(buffer.wl_buffer(), 1);
                        app.captures[i].pool = Some(pool);
                        app.captures[i].buffer = Some(buffer);
                    }
                    None => {
                        let p = app.captures.remove(i);
                        p.frame.destroy();
                    }
                }
            }
            frame::Event::Ready { .. } => {
                let mut p = app.captures.remove(i);
                if let (Some((fmt, w, h, stride)), Some(buffer), Some(pool)) = (p.format, p.buffer.as_ref(), p.pool.as_mut()) {
                    if let Some(canvas) = buffer.canvas(pool) {
                        if let Some(s) = to_surface(canvas, fmt, w, h, stride, p.flags & 1 != 0) {
                            app.data.thumbs.insert(p.addr.clone(), s);
                        }
                    }
                }
                p.frame.destroy();
                drop(p);
                app.mark(crate::ui::Kind::Overview);
            }
            frame::Event::Failed => {
                let p = app.captures.remove(i);
                p.frame.destroy();
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles() {
        assert_eq!(handle_of("0x55d1a2b0"), Some(0x55d1a2b0));
        assert_eq!(handle_of("0x5612345678ab"), Some(0x345678ab));
        assert_eq!(handle_of("nope"), None);
    }

    #[test]
    fn converts_and_downscales() {
        // 960×2 xrgb red, y-inverted → 480×1 opaque red.
        let (w, h) = (960u32, 2u32);
        let mut d = vec![0u8; (w * h * 4) as usize];
        for px in d.chunks_mut(4) {
            px.copy_from_slice(&[0, 0, 255, 0]);
        }
        let mut s = to_surface(&d, wl_shm::Format::Xrgb8888, w, h, w * 4, true).unwrap();
        assert_eq!((s.width(), s.height()), (480, 1));
        let data = s.data().unwrap();
        assert_eq!(&data[0..4], &[0, 0, 255, 255]);
        assert!(to_surface(&d, wl_shm::Format::Rgb565, w, h, w * 4, false).is_none());
        assert!(to_surface(&d[..10], wl_shm::Format::Xrgb8888, w, h, w * 4, false).is_none());
    }
}
