//! Control socket at `$XDG_RUNTIME_DIR/creed/shell.sock`: one text command per connection.

use crate::app::Msg;
use smithay_client_toolkit::reexports::calloop::channel::Sender;
use std::io::Read;
use std::os::unix::net::UnixListener;

pub fn spawn(tx: Sender<Msg>) {
    let paths = creed_core::Paths::from_env();
    let _ = std::fs::create_dir_all(&paths.runtime);
    let sock = paths.shell_socket();
    // Refuse to start twice.
    if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
        eprintln!("creed-shell: another instance is already running ({})", sock.display());
        std::process::exit(1);
    }
    let _ = std::fs::remove_file(&sock);
    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("creed-shell: cannot bind {}: {e}", sock.display());
            return;
        }
    };
    std::thread::Builder::new()
        .name("ipc".into())
        .stack_size(64 * 1024)
        .spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut s = stream;
                let _ = s.set_read_timeout(Some(std::time::Duration::from_secs(2)));
                let mut buf = Vec::with_capacity(256);
                let mut chunk = [0u8; 256];
                // Read until newline or EOF, max 4 KiB.
                loop {
                    match s.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            if buf.contains(&b'\n') || buf.len() > 4096 {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                let cmd = String::from_utf8_lossy(&buf).lines().next().unwrap_or("").trim().to_string();
                if tx.send(Msg::Ipc(cmd, s)).is_err() {
                    return;
                }
            }
        })
        .ok();
}
