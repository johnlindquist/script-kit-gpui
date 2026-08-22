//! Byte-capped, UTF-8-safe previews for untrusted user values in logs.
//!
//! Oracle-Session `logging-observability-next-pass` PR1 (A): every log
//! site that interpolates untrusted input (stdin text, chat titles,
//! dictation queries, triggerBuiltin names, Agent Chat command display strings,
//! …) must route through [`log_user_value`] so the preview can never
//! exceed [`SAFE_USER_VALUE_MAX_BYTES`].
//!
//! The cap is **bytes, not chars**. Log budget is disk + JSONL bytes, and
//! a 120-char value mixing emoji + combining marks can still exceed
//! 480 bytes. Rust `&str` is valid UTF-8, so we cap by byte budget, walk
//! back to the nearest char boundary, trim trailing whitespace, and
//! append an ellipsis inside the budget.
//!
//! Usage:
//!
//! ```ignore
//! let name_safe = logging::log_user_value(name);
//! tracing::warn!(
//!     category = "STDIN",
//!     event_type = "trigger_builtin_unknown",
//!     name_preview = %name_safe,
//!     name_bytes = name_safe.raw_bytes,
//!     name_safe_bytes = name_safe.safe_bytes,
//!     name_truncated = name_safe.truncated,
//!     "triggerBuiltin unknown name — dispatch no-op"
//! );
//! ```

use std::borrow::Cow;
use std::fmt;
use std::sync::OnceLock;

use sha2::{Digest, Sha256};

/// Default byte cap for a single log preview.
pub const SAFE_USER_VALUE_MAX_BYTES: usize = 200;

/// Ellipsis marker appended to truncated previews (3 bytes in UTF-8).
const ELLIPSIS: &str = "…";

/// Private to this process: never persisted, emitted, or accepted from callers.
static PRIVATE_LOG_HMAC_KEY: OnceLock<[u8; 16]> = OnceLock::new();

/// A genuinely private diagnostic identity that never stores user content.
///
/// Unlike [`LogSafe`], which deliberately exposes a bounded preview, this
/// representation is suitable for prompts, filters, URLs, clipboard labels,
/// provider errors, and any other value that must never reach a log sink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrivateLogValue {
    /// Exact byte length of the original UTF-8 value.
    pub raw_bytes: usize,
    /// Process-stable HMAC-SHA-256 fingerprint; the original value is not retained.
    pub sha256: String,
}

impl PrivateLogValue {
    /// Byte length of the digest-only representation emitted by `Display`.
    pub fn safe_bytes(&self) -> usize {
        "sha256:".len() + self.sha256.len()
    }
}

impl fmt::Display for PrivateLogValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "sha256:{}", self.sha256)
    }
}

/// Fingerprint sensitive user content with a process-private HMAC-SHA-256 key.
///
/// A public SHA-256 of progressively typed prefixes is reversible: an observer
/// can test every possible next character against each successive log entry.
/// The ephemeral key preserves correlation within this process without making
/// offline guessing or cross-session tracking possible.
pub fn log_private_user_value(raw: &str) -> PrivateLogValue {
    let key = PRIVATE_LOG_HMAC_KEY.get_or_init(|| *uuid::Uuid::new_v4().as_bytes());
    PrivateLogValue {
        raw_bytes: raw.len(),
        sha256: hmac_sha256(key, raw.as_bytes()),
    }
}

/// RFC 2104 HMAC with SHA-256's 64-byte block and standard long-key normalization.
fn hmac_sha256(key: &[u8], message: &[u8]) -> String {
    const BLOCK_BYTES: usize = 64;

    let mut normalized_key = [0_u8; BLOCK_BYTES];
    if key.len() > BLOCK_BYTES {
        let shortened_key = Sha256::digest(key);
        normalized_key[..shortened_key.len()].copy_from_slice(&shortened_key);
    } else {
        normalized_key[..key.len()].copy_from_slice(key);
    }

    let mut inner_pad = [0x36_u8; BLOCK_BYTES];
    let mut outer_pad = [0x5c_u8; BLOCK_BYTES];
    for ((inner, outer), byte) in inner_pad
        .iter_mut()
        .zip(outer_pad.iter_mut())
        .zip(normalized_key)
    {
        *inner ^= byte;
        *outer ^= byte;
    }

    let mut inner = Sha256::new();
    inner.update(inner_pad);
    inner.update(message);

    let mut outer = Sha256::new();
    outer.update(outer_pad);
    outer.update(inner.finalize());
    format!("{:x}", outer.finalize())
}

/// Byte-capped preview of an untrusted value plus byte-level metadata.
///
/// `Display` emits only the preview. The `raw_bytes`, `safe_bytes`, and
/// `truncated` fields are intended to be logged as separate structured
/// fields alongside the preview so downstream budget accounting can key
/// off them without re-measuring the string.
#[derive(Clone, Debug)]
pub struct LogSafe<'a> {
    value: Cow<'a, str>,
    /// Byte length of the original (untrimmed) input.
    pub raw_bytes: usize,
    /// Byte length of the emitted preview (always ≤ the byte limit).
    pub safe_bytes: usize,
    /// `true` when the original overflowed the byte budget and the
    /// preview has the ellipsis suffix.
    pub truncated: bool,
}

impl<'a> LogSafe<'a> {
    /// Borrow the preview as `&str`.
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for LogSafe<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

/// Preview `raw` with the default byte cap.
pub fn log_user_value(raw: &str) -> LogSafe<'_> {
    log_user_value_with_limit(raw, SAFE_USER_VALUE_MAX_BYTES)
}

/// Preview `raw` with a caller-chosen byte cap. The budget includes the
/// ellipsis suffix, so the emitted preview is always ≤ `max_bytes`.
pub fn log_user_value_with_limit(raw: &str, max_bytes: usize) -> LogSafe<'_> {
    let trimmed = raw.trim();
    let raw_bytes = raw.len();

    if trimmed.len() <= max_bytes {
        let value = if trimmed.len() == raw.len() {
            Cow::Borrowed(raw)
        } else {
            Cow::Owned(trimmed.to_string())
        };
        let safe_bytes = value.len();
        return LogSafe {
            value,
            raw_bytes,
            safe_bytes,
            truncated: false,
        };
    }

    let budget = max_bytes.saturating_sub(ELLIPSIS.len());
    let mut end = budget.min(trimmed.len());
    while end > 0 && !trimmed.is_char_boundary(end) {
        end -= 1;
    }

    let mut out = trimmed[..end].trim_end().to_string();
    if max_bytes >= ELLIPSIS.len() {
        out.push_str(ELLIPSIS);
    }
    let safe_bytes = out.len();
    LogSafe {
        value: Cow::Owned(out),
        raw_bytes,
        safe_bytes,
        truncated: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_log_value_never_contains_user_content() {
        let secret = "sk-live-secret-query🔐 https://private.example/path?token=hunter2";
        let private = log_private_user_value(secret);

        assert_eq!(private.raw_bytes, secret.len());
        assert_eq!(private.sha256.len(), 64);
        assert_eq!(private.safe_bytes(), private.to_string().len());
        assert!(private
            .sha256
            .chars()
            .all(|value| value.is_ascii_hexdigit()));
        assert_eq!(private.to_string(), format!("sha256:{}", private.sha256));
        assert!(!private.to_string().contains("sk-live-secret"));
        assert!(!format!("{private:?}").contains("hunter2"));
        assert!(!format!("{private:?}").contains("private.example"));
    }

    #[test]
    fn private_log_value_is_stable_and_distinguishes_equal_length_secrets() {
        let left = log_private_user_value("secret-a");
        let same = log_private_user_value("secret-a");
        let right = log_private_user_value("secret-b");

        assert_eq!(left, same);
        assert_eq!(left.raw_bytes, right.raw_bytes);
        assert_ne!(left.sha256, right.sha256);
    }

    #[test]
    fn private_log_hmac_matches_the_rfc4231_sha256_vector() {
        let key = [0x0b_u8; 20];
        assert_eq!(
            hmac_sha256(&key, b"Hi There"),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn private_log_hmac_normalizes_keys_longer_than_the_sha256_block() {
        let key = [0xaa_u8; 131];
        assert_eq!(
            hmac_sha256(
                &key,
                b"Test Using Larger Than Block-Size Key - Hash Key First"
            ),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );
    }

    #[test]
    fn progressively_typed_prefixes_never_expose_public_sha256_digests() {
        let mut previous = None;

        for prefix in ["s", "sk", "sk-", "sk-live", "sk-live-private"] {
            let private = log_private_user_value(prefix);
            let public_sha256 = format!("{:x}", Sha256::digest(prefix.as_bytes()));

            assert_eq!(private.raw_bytes, prefix.len());
            assert_ne!(private.sha256, public_sha256);
            assert_eq!(private, log_private_user_value(prefix));
            if let Some(previous_digest) = previous.replace(private.sha256.clone()) {
                assert_ne!(private.sha256, previous_digest);
            }
        }
    }

    #[test]
    fn short_ascii_is_borrowed_unchanged() {
        let safe = log_user_value("hello");
        assert_eq!(safe.as_str(), "hello");
        assert_eq!(safe.raw_bytes, 5);
        assert_eq!(safe.safe_bytes, 5);
        assert!(!safe.truncated);
    }

    #[test]
    fn trims_whitespace_without_marking_truncated() {
        let safe = log_user_value("   hi   ");
        assert_eq!(safe.as_str(), "hi");
        assert_eq!(safe.raw_bytes, 8);
        assert_eq!(safe.safe_bytes, 2);
        assert!(!safe.truncated);
    }

    #[test]
    fn long_ascii_is_byte_capped_with_ellipsis() {
        let long: String = "x".repeat(1024);
        let safe = log_user_value(&long);
        assert!(safe.truncated);
        assert_eq!(safe.raw_bytes, 1024);
        assert!(safe.safe_bytes <= SAFE_USER_VALUE_MAX_BYTES);
        assert!(safe.as_str().ends_with('…'));
    }

    #[test]
    fn emoji_cap_walks_back_to_char_boundary() {
        // 🙂 = 4 bytes. Pack 60 of them (240 bytes) over the default 200-byte cap.
        let emoji = "🙂".repeat(60);
        let safe = log_user_value(&emoji);
        assert!(safe.truncated);
        assert!(safe.safe_bytes <= SAFE_USER_VALUE_MAX_BYTES);
        // The preview must still be valid UTF-8 after truncation — any
        // slice past a mid-char boundary would have panicked by now.
        let preview = safe.as_str().trim_end_matches('…');
        assert!(
            preview.chars().all(|c| c == '🙂'),
            "truncated preview should only contain whole 🙂 chars: {preview:?}"
        );
    }

    #[test]
    fn combining_marks_stay_on_boundary() {
        // "e" + combining acute accent = 3 bytes per visual char.
        let combining = "e\u{0301}".repeat(120);
        let safe = log_user_value(&combining);
        assert!(safe.truncated);
        assert!(safe.safe_bytes <= SAFE_USER_VALUE_MAX_BYTES);
        assert!(safe.as_str().is_char_boundary(safe.as_str().len()));
    }

    #[test]
    fn tiny_budget_drops_ellipsis_when_impossible() {
        let safe = log_user_value_with_limit("long value", 2);
        assert!(safe.truncated);
        assert_eq!(safe.safe_bytes, 0);
    }

    #[test]
    fn exactly_on_budget_not_truncated() {
        let payload = "a".repeat(SAFE_USER_VALUE_MAX_BYTES);
        let safe = log_user_value(&payload);
        assert!(!safe.truncated);
        assert_eq!(safe.safe_bytes, SAFE_USER_VALUE_MAX_BYTES);
        assert_eq!(safe.raw_bytes, SAFE_USER_VALUE_MAX_BYTES);
    }

    #[test]
    fn one_byte_over_budget_is_truncated() {
        let payload = "a".repeat(SAFE_USER_VALUE_MAX_BYTES + 1);
        let safe = log_user_value(&payload);
        assert!(safe.truncated);
        assert!(safe.safe_bytes <= SAFE_USER_VALUE_MAX_BYTES);
        assert!(safe.as_str().ends_with('…'));
    }

    #[test]
    fn display_writes_preview_string_only() {
        let safe = log_user_value("preview");
        assert_eq!(format!("{safe}"), "preview");
    }
}
