//! One place every core event goes through, so the native window and any
//! browser tab see the same stream.

use crate::model::CoreEvent;
use crate::sessions::EventSink;
use std::collections::VecDeque;
use std::sync::{Condvar, Mutex};
use std::time::{Duration, Instant};

const KEEP: usize = 512;

#[derive(Default)]
struct State {
    seq: u64,
    buf: VecDeque<(u64, CoreEvent)>,
}

#[derive(Default)]
pub struct EventHub {
    state: Mutex<State>,
    cv: Condvar,
}

impl EventHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sequence number of the newest event (0 before any).
    pub fn seq(&self) -> u64 {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).seq
    }

    /// Events newer than `after`, waiting up to `timeout` for one. Returns the new
    /// sequence number to pass next time. If `after` is ahead of the hub (the app
    /// restarted), returns at once so the caller can resync.
    pub fn wait_after(&self, after: u64, timeout: Duration) -> (u64, Vec<CoreEvent>) {
        let deadline = Instant::now() + timeout;
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if after > guard.seq {
                return (guard.seq, Vec::new());
            }
            if guard.seq > after {
                let events = guard
                    .buf
                    .iter()
                    .filter(|(n, _)| *n > after)
                    .map(|(_, e)| e.clone())
                    .collect();
                return (guard.seq, events);
            }
            let now = Instant::now();
            if now >= deadline {
                return (guard.seq, Vec::new());
            }
            guard = self
                .cv
                .wait_timeout(guard, deadline - now)
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
    }
}

impl EventSink for EventHub {
    fn emit(&self, event: CoreEvent) {
        let mut guard = self.state.lock().unwrap_or_else(|e| e.into_inner());
        guard.seq += 1;
        let n = guard.seq;
        guard.buf.push_back((n, event));
        while guard.buf.len() > KEEP {
            guard.buf.pop_front();
        }
        drop(guard);
        self.cv.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn delivers_events_in_order_and_resyncs_after_restart() {
        let hub = Arc::new(EventHub::new());
        hub.emit(CoreEvent::LibraryChanged);
        hub.emit(CoreEvent::LibraryChanged);
        let (seq, events) = hub.wait_after(0, Duration::from_millis(10));
        assert_eq!((seq, events.len()), (2, 2));
        let (seq, events) = hub.wait_after(2, Duration::from_millis(10));
        assert_eq!((seq, events.len()), (2, 0));
        let (seq, events) = hub.wait_after(99, Duration::from_secs(5));
        assert_eq!((seq, events.len()), (2, 0));
    }

    #[test]
    fn a_waiter_wakes_when_an_event_arrives() {
        let hub = Arc::new(EventHub::new());
        let waiter = {
            let hub = Arc::clone(&hub);
            std::thread::spawn(move || hub.wait_after(0, Duration::from_secs(5)))
        };
        std::thread::sleep(Duration::from_millis(50));
        hub.emit(CoreEvent::LibraryChanged);
        let (seq, events) = waiter.join().unwrap();
        assert_eq!((seq, events.len()), (1, 1));
    }
}
