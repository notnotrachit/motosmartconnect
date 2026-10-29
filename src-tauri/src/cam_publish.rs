use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Once;
use std::time::Duration;

static STARTED: Once = Once::new();
const SINK: SocketAddr = SocketAddr::new(std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST), 59731);

pub fn ensure() {
    STARTED.call_once(|| {
        let Some(script) = script_path() else {
            return;
        };
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/phone-camera-publish.log")
            .ok();
        let mut command = Command::new("python3");
        command.arg(script).stdin(Stdio::null()).stdout(Stdio::null());
        if let Some(log) = log {
            command.stderr(Stdio::from(log));
        } else {
            command.stderr(Stdio::null());
        }
        let _ = command.spawn();
    });
}

/// Opens OBS only while the phone camera is on, and closes the OBS this app started.
pub fn set_open(on: bool) {
    ensure();
    std::thread::spawn(move || {
        let body = if on { "1" } else { "0" };
        for _ in 0..25 {
            if post_obs(body).is_ok() {
                return;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });
}

fn post_obs(body: &str) -> std::io::Result<()> {
    let mut stream = TcpStream::connect_timeout(&SINK, Duration::from_millis(200))?;
    stream.set_write_timeout(Some(Duration::from_millis(400)))?;
    stream.set_read_timeout(Some(Duration::from_millis(400)))?;
    write!(
        stream,
        "POST /obs HTTP/1.0\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )?;
    let mut buf = [0u8; 32];
    let _ = stream.read(&mut buf);
    Ok(())
}

fn script_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let mut dir = exe.parent()?.to_path_buf();
    for _ in 0..8 {
        let candidate = dir.join("scripts/phone_camera_publish.py");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}
