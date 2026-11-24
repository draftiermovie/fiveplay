//! One small Tokio runtime for the client's I/O, so the game's frame never
//! waits on the network or the disk.
//!
//! It handles the server connection (address lookup, the UDP socket; see
//! `net`) and save-file writes (see `save`). It is kept light on memory:
//! - one single-threaded runtime on one thread with a small stack, started
//!   only the first time it is needed;
//! - at most one helper thread for blocking work (address lookups and file
//!   writes), which exits after a couple of idle seconds;
//! - only the Tokio features those jobs need (`rt`, `net`, `sync`, `time`);
//! - bounded channels between it and the game (see `net`).
use std::{sync::OnceLock, time::Duration};
use tokio::runtime::{Builder, Handle};

/// The runtime thread only polls a few sockets and timers.
const STACK_BYTES: usize = 256 * 1024;
/// Address lookups and file writes are small, short jobs.
const BLOCKING_STACK_BYTES: usize = 512 * 1024;
const BLOCKING_IDLE: Duration = Duration::from_secs(2);

static HANDLE: OnceLock<Option<Handle>> = OnceLock::new();

/// The runtime, started on first use. `None` only if the OS refused to
/// create a thread; callers then fall back to doing the work in place.
pub fn handle() -> Option<&'static Handle> {
    HANDLE.get_or_init(start).as_ref()
}

fn start() -> Option<Handle> {
    let runtime = Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .max_blocking_threads(1)
        .thread_keep_alive(BLOCKING_IDLE)
        .thread_stack_size(BLOCKING_STACK_BYTES)
        .thread_name("fiveplay-io-blocking")
        .build()
        .map_err(|err| bevy::log::warn!("Background I/O unavailable: {err}"))
        .ok()?;
    let handle = runtime.handle().clone();
    // A current-thread runtime runs its tasks while a thread is inside
    // `block_on`; this one stays there for the life of the game.
    std::thread::Builder::new()
        .name("fiveplay-io".into())
        .stack_size(STACK_BYTES)
        .spawn(move || runtime.block_on(std::future::pending::<()>()))
        .map_err(|err| bevy::log::warn!("Background I/O unavailable: {err}"))
        .ok()?;
    Some(handle)
}

/// Runs `job` off the game thread (or right here if the runtime could not
/// start). Jobs run one at a time, in the order they were queued.
pub fn blocking(job: impl FnOnce() + Send + 'static) {
    match handle() {
        Some(h) => {
            h.spawn_blocking(job);
        }
        None => job(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jobs_run_in_order_off_the_calling_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let caller = std::thread::current().id();
        for i in 0..5 {
            let tx = tx.clone();
            blocking(move || {
                let _ = tx.send((i, std::thread::current().id() != caller));
            });
        }
        let got: Vec<_> = (0..5)
            .map(|_| rx.recv_timeout(Duration::from_secs(5)).unwrap())
            .collect();
        assert_eq!(got.iter().map(|g| g.0).collect::<Vec<_>>(), [0, 1, 2, 3, 4]);
        assert!(got.iter().all(|g| g.1));
        // The runtime also drives async tasks.
        let (tx, rx) = std::sync::mpsc::channel();
        handle().unwrap().spawn(async move {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let _ = tx.send(42);
        });
        assert_eq!(rx.recv_timeout(Duration::from_secs(5)), Ok(42));
    }
}
