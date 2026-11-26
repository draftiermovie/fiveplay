//! FivePlay dedicated server.
//!
//! Configuration (environment, all optional):
//! - `FIVEPLAY_PORT`            UDP game port (default 7777)
//! - `FIVEPLAY_STATUS_PORT`     TCP port for the HTTP status page / health
//!   check (default 8080, `0` disables it)
//! - `FIVEPLAY_ROTATION`        maps and modes played in turn, e.g.
//!   `sector05:ffa,skyreach:tdm,foundry:gun`; maps `sector05`, `skyreach`,
//!   `foundry`, modes `ffa`, `tdm`, `gun` (default `sector05:ffa,skyreach:ffa`);
//!   with voting on, only the first match comes from the rotation
//! - `FIVEPLAY_MATCH_SECONDS`   match length (default 300)
//! - `FIVEPLAY_VOTE`            `1` (default): players vote on the next map
//!   and mode after each match; `0`: play the rotation in order
//! - `FIVEPLAY_MAX_PLAYERS`     1-6 (default 6)
//! - `FIVEPLAY_THREADS`         worker threads for parallel work (default 2,
//!   capped at the number of CPU cores)
//!
//! `fiveplay-server --health` probes the status port and exits 0 when the
//! server answers (used by the container health check).
mod game;
mod workers;

use fiveplay_net::*;
use game::{Config, Game, parse_rotation};
use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream, UdpSocket},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

fn env<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

fn health(port: u16) -> std::process::ExitCode {
    let ok = TcpStream::connect_timeout(
        &SocketAddr::from(([127, 0, 0, 1], port)),
        Duration::from_secs(2),
    )
    .and_then(|mut s| {
        s.set_read_timeout(Some(Duration::from_secs(2)))?;
        s.write_all(b"GET /health HTTP/1.0\r\n\r\n")?;
        let mut reply = String::new();
        let _ = s.read_to_string(&mut reply);
        Ok(reply.starts_with("HTTP/1.1 200"))
    })
    .unwrap_or(false);
    if ok {
        std::process::ExitCode::SUCCESS
    } else {
        std::process::ExitCode::FAILURE
    }
}

/// Answers any HTTP request on the status port with the latest state the
/// game loop published. Runs on its own thread so a slow or silent client
/// can never stall the match.
fn serve_status(listener: TcpListener, status: Arc<Mutex<String>>) {
    for stream in listener.incoming() {
        let Ok(mut stream) = stream else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_millis(250)));
        let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
        let mut buf = [0u8; 1024];
        let _ = stream.read(&mut buf);
        let body = status.lock().map(|s| s.clone()).unwrap_or_default();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
    }
}

fn main() -> std::process::ExitCode {
    let status_port: u16 = env("FIVEPLAY_STATUS_PORT", 8080);
    if std::env::args().any(|a| a == "--health") {
        return health(status_port);
    }
    let port: u16 = env("FIVEPLAY_PORT", DEFAULT_PORT);
    let mut rotation = parse_rotation(&env(
        "FIVEPLAY_ROTATION",
        "sector05:ffa,skyreach:ffa".to_string(),
    ));
    if rotation.is_empty() {
        eprintln!("FIVEPLAY_ROTATION has no valid entries; using sector05:ffa");
        rotation = parse_rotation("sector05:ffa");
    }
    let cfg = Config {
        rotation,
        match_seconds: env("FIVEPLAY_MATCH_SECONDS", 300f32).clamp(30., 3600.),
        max_players: env("FIVEPLAY_MAX_PLAYERS", MAX_PLAYERS).clamp(1, MAX_PLAYERS),
        vote: env("FIVEPLAY_VOTE", 1u8) != 0,
        ..Default::default()
    };
    let socket = match UdpSocket::bind(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("cannot bind UDP port {port}: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };
    socket
        .set_read_timeout(Some(Duration::from_millis(4)))
        .expect("socket timeout");
    let status = (status_port != 0)
        .then(|| TcpListener::bind(("0.0.0.0", status_port)))
        .transpose()
        .unwrap_or_else(|err| {
            eprintln!("status page disabled, cannot bind TCP {status_port}: {err}");
            None
        });
    println!(
        "FivePlay server (protocol {VERSION}) on UDP {port}, status on TCP {status_port}, {} players max, map vote {}, rotation: {}",
        cfg.max_players,
        if cfg.vote { "on" } else { "off" },
        cfg.rotation
            .iter()
            .map(|(map, mode)| format!("{}:{}", game::map_name(*map), game::mode_name(*mode)))
            .collect::<Vec<_>>()
            .join(",")
    );

    let threads = workers::init(env("FIVEPLAY_THREADS", workers::DEFAULT_THREADS));
    println!("Parallel work on {threads} worker thread(s)");

    let mut game = Game::new(cfg);
    let status_json = Arc::new(Mutex::new(game.status_json()));
    if let Some(listener) = status {
        let shared = status_json.clone();
        std::thread::Builder::new()
            .name("fiveplay-status".into())
            .stack_size(64 * 1024)
            .spawn(move || serve_status(listener, shared))
            .expect("status thread");
    }
    let start = Instant::now();
    let tick = 1. / TICK_RATE;
    let snap_every = 1. / SNAPSHOT_RATE;
    let (mut next_tick, mut next_snap, mut next_status) = (0f32, 0f32, 0f32);
    let mut buf = [0u8; 2048];
    loop {
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, addr)) => {
                    let bytes = &buf[..n];
                    if let Some(msg) = ClientMsg::decode(bytes) {
                        game.handle(addr, msg);
                    } else if version_mismatch(bytes) {
                        let reject = ServerMsg::Reject {
                            reason: "VERSION MISMATCH / UPDATE FIVEPLAY".into(),
                        };
                        let _ = socket.send_to(&reject.encode(), addr);
                    }
                }
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => break,
                // Windows reports ICMP "port unreachable" from an earlier send
                // as a receive error; it says nothing about this socket.
                Err(e) if e.kind() == ErrorKind::ConnectionReset => continue,
                Err(e) => {
                    eprintln!("receive error: {e}");
                    break;
                }
            }
        }
        let now = start.elapsed().as_secs_f32();
        while next_tick <= now {
            game.step(tick);
            next_tick += tick;
        }
        if next_snap <= now {
            game.send_snapshots();
            next_snap = now + snap_every;
        }
        workers::send_all(&socket, &mut game.outbox);
        for line in game.log.drain(..) {
            println!("{line}");
        }
        if next_status <= now {
            if let Ok(mut s) = status_json.lock() {
                *s = game.status_json();
            }
            next_status = now + 0.5;
        }
    }
}
