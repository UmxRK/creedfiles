//! creed-shell: the Creed desktop shell (bar, dock, panels, notifications, wallpaper).
// Style-only lints: the immediate-mode UI passes explicit geometry and colours by design.
#![allow(clippy::too_many_arguments, clippy::type_complexity, clippy::needless_range_loop, clippy::field_reassign_with_default)]

mod a11y;
mod app;
mod data;
mod gfx;
mod icons;
mod services;
mod thumbs;
mod time;
mod ui;
mod views;

use app::{App, Msg};
use smithay_client_toolkit::reexports::calloop::{
    channel,
    timer::{TimeoutAction, Timer},
    EventLoop,
};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use std::time::Duration;
use wayland_client::{globals::registry_queue_init, Connection};

/// Returns freed heap pages to the kernel (glibc keeps them otherwise).
pub fn trim_memory() {
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("creed-shell {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if args.iter().any(|a| a == "--print-dispatches") {
        // Test hook: every Hyprland dispatcher the shell can send, one per line.
        let mut all = views::snap::sample_dispatches();
        all.extend(views::mac::menu_dispatches());
        all.push("hl.dsp.focus({ workspace = 3 })".into());
        all.push("hl.dsp.focus({ workspace = \"e-1\" })".into());
        all.push("hl.dsp.focus({ workspace = \"empty\" })".into());
        all.push("hl.dsp.window.move({ workspace = 2, window = \"address:0x1\" })".into());
        all.push("hl.dsp.window.move({ workspace = \"special:minimized\", window = \"address:0x1\" })".into());
        all.push("hl.dsp.exec_cmd(\"hyprctl switchxkblayout all next\")".into());
        for d in all {
            println!("{d}");
        }
        return;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("creed-shell - the Creed desktop shell\n\nUsage: creed-shell [--version]\n\nControl a running shell with `creed toggle <surface>`, `creed reload`, …");
        return;
    }
    // Fewer glibc arenas: each thread arena would otherwise reserve its own heap.
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::mallopt(libc::M_ARENA_MAX, 2);
        // Fixed thresholds: big temporary buffers (cairo groups, decoded images) are mmapped
        // and handed straight back to the kernel instead of growing the heap for good.
        libc::mallopt(libc::M_MMAP_THRESHOLD, 128 * 1024);
        libc::mallopt(libc::M_TRIM_THRESHOLD, 256 * 1024);
    }

    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("creed-shell: cannot connect to a Wayland compositor: {e}");
            std::process::exit(1);
        }
    };
    let (globals, queue) = registry_queue_init::<App>(&conn).expect("wayland registry");
    let qh = queue.handle();
    let mut event_loop: EventLoop<'static, App> = EventLoop::try_new().expect("event loop");
    let handle = event_loop.handle();
    WaylandSource::new(conn.clone(), queue).insert(handle.clone()).expect("wayland source");

    let (tx, rx) = channel::channel::<Msg>();
    handle
        .insert_source(rx, |ev, _, app: &mut App| {
            if let channel::Event::Msg(m) = ev {
                app.on_msg(m);
            }
        })
        .expect("channel");

    let paths = creed_core::Paths::from_env();
    let mut app = App::new(conn, &globals, qh, handle.clone(), tx.clone(), paths);
    services::start_all(&tx, &app.theme.cfg);
    app.notif_bus = services::notify::start(tx.clone());
    app.refresh_hypr();
    if let Some(e) = app.config_error.clone() {
        eprintln!("creed-shell: config error: {e}");
    }

    // One tick per second drives the clock and OSD timeouts.
    let tick_tx = tx.clone();
    handle
        .insert_source(Timer::from_duration(Duration::from_secs(1)), move |_, _, _| {
            let _ = tick_tx.send(Msg::Tick);
            TimeoutAction::ToDuration(Duration::from_secs(1))
        })
        .expect("timer");

    // Watch config files for changes.
    watch_config(tx.clone());

    // Create surfaces once outputs are known.
    let _ = event_loop.dispatch(Some(Duration::from_millis(50)), &mut app);
    app.sync_persistent();
    loop {
        if let Err(e) = event_loop.dispatch(Some(Duration::from_millis(500)), &mut app) {
            eprintln!("creed-shell: event loop error: {e}");
            break;
        }
        app.draw_dirty();
        if app.exit {
            break;
        }
    }
    let _ = std::fs::remove_file(app.paths.shell_socket());
}

/// Polls config file mtimes (cheap; avoids an inotify crate) and asks for a reload on change.
fn watch_config(tx: channel::Sender<Msg>) {
    let paths = creed_core::Paths::from_env();
    std::thread::Builder::new()
        .name("config-watch".into())
        .stack_size(64 * 1024)
        .spawn(move || {
            let files = [paths.user_config(), paths.overrides()];
            let stamp = |f: &std::path::PathBuf| std::fs::metadata(f).and_then(|m| m.modified()).ok();
            let mut last: Vec<_> = files.iter().map(stamp).collect();
            loop {
                std::thread::sleep(Duration::from_millis(1500));
                let now: Vec<_> = files.iter().map(stamp).collect();
                if now != last {
                    last = now;
                    // Let editors finish writing.
                    std::thread::sleep(Duration::from_millis(200));
                    if tx.send(Msg::ConfigChanged).is_err() {
                        return;
                    }
                }
            }
        })
        .ok();
}
