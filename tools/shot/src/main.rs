//! Minimal wlr-screencopy client: `creed-shot out.png` captures the first output.
use std::os::fd::{AsFd, FromRawFd, OwnedFd};
use wayland_client::protocol::{wl_buffer, wl_output, wl_registry, wl_shm, wl_shm_pool};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};
use wayland_protocols_wlr::screencopy::v1::client::{zwlr_screencopy_frame_v1 as frame, zwlr_screencopy_manager_v1 as mgr};

#[derive(Default)]
struct S {
    shm: Option<wl_shm::WlShm>,
    output: Option<wl_output::WlOutput>,
    mgr: Option<mgr::ZwlrScreencopyManagerV1>,
    fmt: Option<(u32, u32, u32, u32)>,
    done: bool,
    failed: bool,
    flags: u32,
}

impl Dispatch<wl_registry::WlRegistry, ()> for S {
    fn event(s: &mut S, r: &wl_registry::WlRegistry, e: wl_registry::Event, _: &(), _: &Connection, qh: &QueueHandle<S>) {
        if let wl_registry::Event::Global { name, interface, version } = e {
            match interface.as_str() {
                "wl_shm" => s.shm = Some(r.bind(name, 1, qh, ())),
                "wl_output" if s.output.is_none() => s.output = Some(r.bind(name, version.min(4), qh, ())),
                "zwlr_screencopy_manager_v1" => s.mgr = Some(r.bind(name, version.min(3), qh, ())),
                _ => {}
            }
        }
    }
}
impl Dispatch<frame::ZwlrScreencopyFrameV1, ()> for S {
    fn event(s: &mut S, _: &frame::ZwlrScreencopyFrameV1, e: frame::Event, _: &(), _: &Connection, _: &QueueHandle<S>) {
        match e {
            frame::Event::Buffer { format: WEnum::Value(f), width, height, stride } if s.fmt.is_none() => s.fmt = Some((f as u32, width, height, stride)),
            frame::Event::Flags { flags: WEnum::Value(f) } => s.flags = f.bits(),
            frame::Event::Ready { .. } => s.done = true,
            frame::Event::Failed => s.failed = true,
            _ => {}
        }
    }
}
wayland_client::delegate_noop!(S: ignore wl_shm::WlShm);
wayland_client::delegate_noop!(S: ignore wl_output::WlOutput);
wayland_client::delegate_noop!(S: ignore wl_shm_pool::WlShmPool);
wayland_client::delegate_noop!(S: ignore wl_buffer::WlBuffer);
wayland_client::delegate_noop!(S: ignore mgr::ZwlrScreencopyManagerV1);

fn main() {
    let out = std::env::args().nth(1).expect("usage: creed-shot out.png");
    let conn = Connection::connect_to_env().expect("wayland");
    let mut q = conn.new_event_queue();
    let qh = q.handle();
    conn.display().get_registry(&qh, ());
    let mut s = S::default();
    q.roundtrip(&mut s).unwrap();
    let (m, o, shm) = (s.mgr.clone().expect("no screencopy"), s.output.clone().expect("no output"), s.shm.clone().unwrap());
    let f = m.capture_output(0, &o, &qh, ());
    while s.fmt.is_none() && !s.failed {
        q.blocking_dispatch(&mut s).unwrap();
    }
    let (format, w, h, stride) = s.fmt.expect("no format");
    let size = (stride * h) as usize;
    let fd = unsafe {
        let fd = libc::memfd_create(c"shot".as_ptr(), 0);
        libc::ftruncate(fd, size as i64);
        OwnedFd::from_raw_fd(fd)
    };
    let pool = shm.create_pool(fd.as_fd(), size as i32, &qh, ());
    let buf = pool.create_buffer(0, w as i32, h as i32, stride as i32, wl_shm::Format::try_from(format).unwrap(), &qh, ());
    f.copy(&buf);
    while !s.done && !s.failed {
        q.blocking_dispatch(&mut s).unwrap();
    }
    assert!(!s.failed, "capture failed");
    let ptr = unsafe { libc::mmap(std::ptr::null_mut(), size, libc::PROT_READ, libc::MAP_SHARED, std::os::fd::AsRawFd::as_raw_fd(&fd), 0) } as *const u8;
    let data = unsafe { std::slice::from_raw_parts(ptr, size) };
    let mut img = image::RgbImage::new(w, h);
    let flip = s.flags & 1 != 0;
    for y in 0..h {
        let sy = if flip { h - 1 - y } else { y };
        for x in 0..w {
            let i = (sy * stride + x * 4) as usize;
            // xrgb8888 / argb8888 little endian: B G R A; xbgr8888: R G B A
            let (r, g, b) = if format == 0 || format == 1 { (data[i + 2], data[i + 1], data[i]) } else { (data[i], data[i + 1], data[i + 2]) };
            img.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }
    img.save(&out).unwrap();
}
