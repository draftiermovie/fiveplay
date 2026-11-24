//! The server's Rayon thread pool and the work it runs in parallel.
//!
//! Rayon normally starts one thread per CPU core. A game server usually
//! shares its machine (Dokploy runs other apps next to it), so the pool is
//! capped: `FIVEPLAY_THREADS` (default 2), never more than the cores present.
//!
//! One 6-player match is light work, so the pool is only used when a batch
//! is big enough to pay for handing it to other threads; small batches run
//! on the main thread as before.
use fiveplay_net::ServerMsg;
use rayon::prelude::*;
use std::net::{SocketAddr, UdpSocket};

pub const DEFAULT_THREADS: usize = 2;
/// Outgoing packets below this count are encoded and sent on the main thread.
pub const PARALLEL_MIN: usize = 24;
/// Pool threads only encode packets and call `send_to`; a small stack keeps
/// each thread's memory low.
const STACK_BYTES: usize = 256 * 1024;

/// Threads used for `requested` on a machine with `cores` cores (1..=cores).
pub fn thread_count(requested: usize, cores: usize) -> usize {
    requested.clamp(1, cores.max(1))
}

/// Starts the global pool and returns its size. Safe to call more than once
/// (only the first call sizes the pool).
pub fn init(requested: usize) -> usize {
    let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
    let threads = thread_count(requested, cores);
    let _ = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(STACK_BYTES)
        .thread_name(|i| format!("fiveplay-worker-{i}"))
        .build_global();
    rayon::current_num_threads()
}

/// Encodes and sends every queued message, emptying `outbox`. Large batches
/// (many players shooting at once) are split across the pool.
pub fn send_all(socket: &UdpSocket, outbox: &mut Vec<(SocketAddr, ServerMsg)>) {
    let send = |(addr, msg): &(SocketAddr, ServerMsg)| {
        let _ = socket.send_to(&msg.encode(), addr);
    };
    if outbox.len() >= PARALLEL_MIN && rayon::current_num_threads() > 1 {
        outbox.par_iter().for_each(send);
    } else {
        outbox.iter().for_each(send);
    }
    outbox.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use fiveplay_net::ClientMsg;

    #[test]
    fn thread_count_is_limited_by_the_setting_and_the_cores() {
        assert_eq!(thread_count(2, 16), 2);
        assert_eq!(thread_count(64, 4), 4);
        assert_eq!(thread_count(0, 8), 1);
        assert_eq!(thread_count(3, 0), 1);
    }

    #[test]
    fn small_and_large_batches_are_all_delivered() {
        init(2);
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();
        let to = client.local_addr().unwrap();
        for count in [3, PARALLEL_MIN + 10] {
            let mut outbox: Vec<_> = (0..count)
                .map(|i| {
                    (
                        to,
                        ServerMsg::Kill {
                            killer: i as u8,
                            victim: 0,
                            headshot: false,
                        },
                    )
                })
                .collect();
            send_all(&server, &mut outbox);
            assert!(outbox.is_empty());
            let mut seen = vec![false; count];
            let mut buf = [0u8; 256];
            for _ in 0..count {
                let n = client.recv(&mut buf).unwrap();
                match ServerMsg::decode(&buf[..n]) {
                    Some(ServerMsg::Kill { killer, .. }) => seen[killer as usize] = true,
                    other => panic!("unexpected {other:?}"),
                }
            }
            assert!(seen.iter().all(|s| *s), "{count} messages");
        }
        // Not a server message: the client protocol is separate.
        assert!(ServerMsg::decode(&ClientMsg::Bye.encode()).is_none());
    }
}
