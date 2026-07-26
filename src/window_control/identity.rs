//! Process-local window identity allocation.
//!
//! A window's identity is a `WindowHandle`: PID + optional native window id +
//! registry generation + a nonzero, monotonically allocated nonce. The nonce
//! is reused across refreshes only when the same window is PROVEN identical
//! (exact PID plus unique native window number, or `CFEqual` against the
//! previous AX element) — never inferred from title/bounds similarity.

/// Allocate the next process-local window nonce. Never returns 0.
pub(crate) fn next_window_nonce() -> u64 {
    static WINDOW_NONCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    loop {
        let value = WINDOW_NONCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if value != 0 {
            return value;
        }
    }
}

/// Advance a registry/topology generation counter, skipping 0 on wrap.
pub(crate) fn next_generation(current: u64) -> u64 {
    let next = current.wrapping_add(1);
    if next == 0 {
        1
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonces_are_nonzero_and_unique() {
        let first = next_window_nonce();
        let second = next_window_nonce();
        assert_ne!(first, 0);
        assert_ne!(second, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn generation_advances_and_skips_zero_on_wrap() {
        assert_eq!(next_generation(1), 2);
        assert_eq!(next_generation(u64::MAX), 1);
    }
}
