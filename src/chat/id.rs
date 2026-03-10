use std::sync::atomic::{AtomicU16, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU16 = AtomicU16::new(0);

/// Generate a timestamp-based monotonic ID: (unix_millis << 16) | counter.
/// Naturally time-ordered, supports ~65k IDs per millisecond.
pub fn next_id() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64;
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed) as i64;
    (millis << 16) | (seq & 0xFFFF)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_monotonically_increasing() {
        let a = next_id();
        let b = next_id();
        let c = next_id();
        assert!(b > a, "b={b} should be > a={a}");
        assert!(c > b, "c={c} should be > b={b}");
    }

    #[test]
    fn ids_are_unique() {
        let ids: Vec<i64> = (0..1000).map(|_| next_id()).collect();
        let mut deduped = ids.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(ids.len(), deduped.len(), "all IDs should be unique");
    }
}
