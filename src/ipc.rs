//! Local socket between the screensaver instances (one per monitor) and the `abyss` CLI.
//! The instance that holds the `lock` file (the primary) serves; the others subscribe.
//! Messages are one JSON object per line:
//!   server -> clients: {"t":"state","save":{..}} {"t":"event","kind":..,"value":..} {"t":"wake"} {"t":"stop"}
//!   client -> server : {"t":"wake"} (from `abyss wake`) {"t":"stop"} (a key was pressed on a secondary)

use serde_json::{Value, json};
use std::{
    io::{self, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::state::{self, GameEvent};

pub fn sock_path() -> PathBuf {
    // A test or custom state folder keeps its own socket; otherwise the runtime dir (tmpfs, per user).
    if std::env::var_os("ABYSS_STATE_DIR").is_none() {
        if let Some(rt) = std::env::var_os("XDG_RUNTIME_DIR") {
            return PathBuf::from(rt).join("abyss.sock");
        }
    }
    state::state_dir().join("abyss.sock")
}

pub fn encode(ev: &GameEvent) -> Value {
    let (kind, value) = match ev {
        GameEvent::Trophy(id) => ("trophy", id.to_string()),
        GameEvent::LevelUp(l) => ("level", l.to_string()),
        GameEvent::Discovery(id) => ("discovery", id.to_string()),
        GameEvent::Quest(id) => ("quest", id.to_string()),
    };
    json!({ "t": "event", "kind": kind, "value": value })
}

pub fn decode(m: &Value) -> Option<GameEvent> {
    let value = m["value"].as_str()?;
    match m["kind"].as_str()? {
        "trophy" => state::trophy(value).map(|t| GameEvent::Trophy(t.id)),
        "level" => value.parse().ok().map(GameEvent::LevelUp),
        "discovery" => state::creature(value).map(|c| GameEvent::Discovery(c.id)),
        "quest" => state::quest(value).map(|q| GameEvent::Quest(q.id)),
        _ => None,
    }
}

/// Reads whatever is available without blocking; returns the complete lines parsed as JSON and
/// whether the peer closed the connection.
fn read_lines(stream: &mut UnixStream, buf: &mut Vec<u8>) -> (Vec<Value>, bool) {
    let mut closed = false;
    let mut chunk = [0u8; 4096];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => {
                closed = true;
                break;
            }
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(_) => {
                closed = true;
                break;
            }
        }
    }
    let mut out = Vec::new();
    while let Some(i) = buf.iter().position(|&b| b == b'\n') {
        let line: Vec<u8> = buf.drain(..=i).collect();
        if let Ok(v) = serde_json::from_slice::<Value>(&line) {
            out.push(v);
        }
    }
    (out, closed)
}

fn write_line(stream: &mut UnixStream, v: &Value) -> io::Result<()> {
    let mut line = v.to_string();
    line.push('\n');
    stream.write_all(line.as_bytes())
}

#[derive(Debug, PartialEq)]
pub enum Request {
    /// A new subscriber connected: send it the current state.
    Joined,
    Wake,
    Stop,
}

struct Peer {
    stream: UnixStream,
    buf: Vec<u8>,
}

pub struct Server {
    listener: UnixListener,
    peers: Vec<Peer>,
    path: PathBuf,
}

impl Server {
    /// Binds the socket. The caller holds the primary lock, so any socket file left there is stale.
    pub fn bind(path: &Path) -> io::Result<Self> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        listener.set_nonblocking(true)?;
        Ok(Self { listener, peers: Vec::new(), path: path.to_path_buf() })
    }

    pub fn poll(&mut self) -> Vec<Request> {
        let mut out = Vec::new();
        while let Ok((stream, _)) = self.listener.accept() {
            // Writes to a stuck peer must not freeze the frame loop.
            if stream.set_nonblocking(true).is_ok() {
                self.peers.push(Peer { stream, buf: Vec::new() });
                out.push(Request::Joined);
            }
        }
        self.peers.retain_mut(|p| {
            let (msgs, closed) = read_lines(&mut p.stream, &mut p.buf);
            for m in msgs {
                match m["t"].as_str() {
                    Some("wake") => out.push(Request::Wake),
                    Some("stop") => out.push(Request::Stop),
                    _ => {}
                }
            }
            !closed
        });
        out
    }

    /// Sends to every subscriber; one that cannot take the message is dropped.
    pub fn broadcast(&mut self, v: &Value) {
        self.peers.retain_mut(|p| write_line(&mut p.stream, v).is_ok());
    }

    pub fn peers(&self) -> usize {
        self.peers.len()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct Client {
    stream: UnixStream,
    buf: Vec<u8>,
}

impl Client {
    /// Connects, retrying while the primary is still starting up.
    pub fn connect(path: &Path, wait: Duration) -> Option<Self> {
        let deadline = Instant::now() + wait;
        loop {
            if let Ok(stream) = UnixStream::connect(path) {
                stream.set_nonblocking(true).ok()?;
                return Some(Self { stream, buf: Vec::new() });
            }
            if Instant::now() >= deadline {
                return None;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// New messages, and `false` as the second item once the primary has gone away.
    pub fn poll(&mut self) -> (Vec<Value>, bool) {
        let (msgs, closed) = read_lines(&mut self.stream, &mut self.buf);
        (msgs, !closed)
    }

    pub fn send(&mut self, v: &Value) {
        let _ = write_line(&mut self.stream, v);
    }
}

/// One-shot message for the CLI. Returns false when no screensaver is listening.
pub fn send_once(path: &Path, v: &Value) -> bool {
    UnixStream::connect(path).and_then(|mut s| write_line(&mut s, v)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sock(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("abyss-ipc-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d.join("abyss.sock")
    }

    fn wait_for<T>(mut f: impl FnMut() -> Option<T>) -> T {
        let end = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(v) = f() {
                return v;
            }
            assert!(Instant::now() < end, "timed out");
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn broadcast_reaches_subscribers_and_requests_reach_server() {
        let path = sock("roundtrip");
        let mut server = Server::bind(&path).unwrap();
        let mut a = Client::connect(&path, Duration::from_secs(1)).unwrap();
        let mut b = Client::connect(&path, Duration::from_secs(1)).unwrap();
        let mut joined = 0;
        wait_for(|| {
            joined += server.poll().iter().filter(|r| **r == Request::Joined).count();
            (joined == 2).then_some(())
        });
        server.broadcast(&json!({ "t": "wake" }));
        for c in [&mut a, &mut b] {
            let msgs = wait_for(|| Some(c.poll().0).filter(|m| !m.is_empty()));
            assert_eq!(msgs[0]["t"], "wake");
        }
        a.send(&json!({ "t": "stop" }));
        let reqs = wait_for(|| Some(server.poll()).filter(|r| !r.is_empty()));
        assert_eq!(reqs, vec![Request::Stop]);
    }

    #[test]
    fn one_shot_wake_and_missing_server() {
        let path = sock("oneshot");
        assert!(!send_once(&path, &json!({ "t": "wake" })), "nobody listening");
        let mut server = Server::bind(&path).unwrap();
        assert!(send_once(&path, &json!({ "t": "wake" })));
        let reqs = wait_for(|| Some(server.poll()).filter(|r| r.contains(&Request::Wake)));
        assert!(reqs.contains(&Request::Wake));
    }

    #[test]
    fn stale_socket_is_replaced_and_closed_clients_are_noticed() {
        let path = sock("stale");
        std::fs::write(&path, b"leftover").unwrap();
        let mut server = Server::bind(&path).unwrap();
        let mut c = Client::connect(&path, Duration::from_secs(1)).unwrap();
        wait_for(|| server.poll().contains(&Request::Joined).then_some(()));
        drop(server);
        let alive = wait_for(|| Some(c.poll().1).filter(|alive| !alive));
        assert!(!alive, "client sees the primary go away");
        assert!(!path.exists(), "socket file removed on drop");
    }

    #[test]
    fn events_round_trip() {
        for ev in [GameEvent::LevelUp(4), GameEvent::Trophy("first_dive"), GameEvent::Discovery("angler"), GameEvent::Quest("whale")] {
            assert_eq!(decode(&encode(&ev)), Some(ev));
        }
        assert_eq!(decode(&json!({ "kind": "trophy", "value": "nope" })), None);
    }
}
