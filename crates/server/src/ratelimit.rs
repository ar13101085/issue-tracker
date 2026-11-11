//! Slows down password guessing: a small in-memory counter of failed sign-ins.
//! It is per process and forgets on restart, which is enough to make guessing
//! impractical without needing a database round trip on every attempt.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(10 * 60);
const MAX_FAILURES: u32 = 10;

#[derive(Default)]
pub struct Limiter {
    failures: Mutex<HashMap<String, (u32, Instant)>>,
}

impl Limiter {
    /// Seconds to wait when `key` has failed too often, `None` when it may try.
    pub fn blocked(&self, key: &str) -> Option<u64> {
        let map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        match map.get(key) {
            Some((count, since)) if *count >= MAX_FAILURES && since.elapsed() < WINDOW => Some((WINDOW - since.elapsed()).as_secs().max(1)),
            _ => None,
        }
    }

    pub fn failed(&self, key: &str) {
        let mut map = self.failures.lock().unwrap_or_else(|e| e.into_inner());
        // Old entries are dropped here so the map cannot grow without bound.
        if map.len() > 10_000 {
            map.retain(|_, (_, since)| since.elapsed() < WINDOW);
        }
        let entry = map.entry(key.to_string()).or_insert((0, Instant::now()));
        if entry.1.elapsed() >= WINDOW {
            *entry = (0, Instant::now());
        }
        entry.0 += 1;
    }

    pub fn succeeded(&self, key: &str) {
        self.failures.lock().unwrap_or_else(|e| e.into_inner()).remove(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_after_repeated_failures_and_clears_on_success() {
        let l = Limiter::default();
        for _ in 0..MAX_FAILURES - 1 {
            l.failed("a");
        }
        assert!(l.blocked("a").is_none());
        l.failed("a");
        assert!(l.blocked("a").is_some());
        assert!(l.blocked("b").is_none(), "other keys are unaffected");
        l.succeeded("a");
        assert!(l.blocked("a").is_none());
    }
}
