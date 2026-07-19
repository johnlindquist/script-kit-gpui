//! Emoji usage frecency — load/save per-emoji usage state to
//! ~/.kenv/emoji-usage.json and score it with an exponential half-life decay.
//!
//! Contract (Oracle-Session `emoji-picker-frecency-recency`):
//! - `record_use` adds +1 to the decayed score, updates `last_used_at_ms`.
//! - `decayed_score` returns `score * 0.5^(age_secs / half_life_secs)`.
//! - `frequent_emojis` returns the top-N emoji strings sorted by
//!   (decayed score desc, last_used_at_ms desc, dataset order asc).
//! - Writes use an atomic temp-file rename so a mid-write crash cannot leave
//!   a truncated JSON file.

use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub const EMOJI_USAGE_SCHEMA_VERSION: u32 = 1;

/// Half-life after which a score halves. 14 days balances bursty experiments
/// fading fast vs. habitual emojis staying put.
pub const EMOJI_USAGE_HALF_LIFE_SECS: f64 = 14.0 * 24.0 * 60.0 * 60.0;

/// How many emoji slots to surface in the picker's "Frequently Used" row.
/// Two rows at GRID_COLS = 8.
pub const EMOJI_FREQUENT_LIMIT: usize = crate::emoji::GRID_COLS * 2;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EmojiUsageStore {
    pub schema_version: u32,
    pub half_life_secs: f64,
    pub entries: BTreeMap<String, EmojiUsageEntry>,
}

impl Default for EmojiUsageStore {
    fn default() -> Self {
        Self {
            schema_version: EMOJI_USAGE_SCHEMA_VERSION,
            half_life_secs: EMOJI_USAGE_HALF_LIFE_SECS,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct EmojiUsageEntry {
    pub total_uses: u64,
    pub score: f64,
    pub score_updated_at_ms: i64,
    pub last_used_at_ms: i64,
    pub first_used_at_ms: Option<i64>,
}

impl Default for EmojiUsageEntry {
    fn default() -> Self {
        Self {
            total_uses: 0,
            score: 0.0,
            score_updated_at_ms: 0,
            last_used_at_ms: 0,
            first_used_at_ms: None,
        }
    }
}

fn emoji_usage_path() -> PathBuf {
    dirs::home_dir()
        .map(|home| home.join(".kenv").join("emoji-usage.json"))
        .unwrap_or_else(|| PathBuf::from("emoji-usage.json"))
}

/// Load the usage store from disk. Missing file returns an empty default store
/// (not an error). Corrupt JSON surfaces as an error to the caller.
pub fn load_emoji_usage() -> anyhow::Result<EmojiUsageStore> {
    load_emoji_usage_from_path(&emoji_usage_path())
}

pub fn load_emoji_usage_from_path(path: &std::path::Path) -> anyhow::Result<EmojiUsageStore> {
    if !path.exists() {
        return Ok(EmojiUsageStore::default());
    }

    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let store: EmojiUsageStore = serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.display()))?;

    Ok(store)
}

/// Save the store with an atomic temp-file rename so a mid-write crash cannot
/// leave a partially-written file.
pub fn save_emoji_usage(store: &EmojiUsageStore) -> anyhow::Result<()> {
    save_emoji_usage_to_path(store, &emoji_usage_path())
}

pub fn save_emoji_usage_to_path(
    store: &EmojiUsageStore,
    path: &std::path::Path,
) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(store).context("Failed to serialize emoji usage")?;
    // Unique-temp atomic write (see src/atomic_file.rs): a fixed temp path let
    // concurrent savers corrupt the file.
    crate::atomic_file::write_atomic(path, json.as_bytes())
        .with_context(|| format!("Failed to atomically write {}", path.display()))?;
    Ok(())
}

/// Score with exponential half-life decay, normalized to `now_ms`. Returns the
/// stored score unchanged if the clock is not ahead of the last update.
pub fn decayed_score(entry: &EmojiUsageEntry, now_ms: i64, half_life_secs: f64) -> f64 {
    if entry.score <= 0.0 || entry.score_updated_at_ms <= 0 || now_ms <= entry.score_updated_at_ms {
        return entry.score.max(0.0);
    }
    let age_secs = (now_ms - entry.score_updated_at_ms) as f64 / 1000.0;
    entry.score * 0.5_f64.powf(age_secs / half_life_secs)
}

/// Record a single selection. Decays the existing score to `now_ms` then adds 1.
pub fn record_use(store: &mut EmojiUsageStore, emoji: &str, now_ms: i64) {
    let half_life_secs = store.half_life_secs;
    let entry = store.entries.entry(emoji.to_string()).or_default();
    let current = decayed_score(entry, now_ms, half_life_secs);
    entry.score = current + 1.0;
    entry.score_updated_at_ms = now_ms;
    entry.last_used_at_ms = now_ms;
    entry.total_uses = entry.total_uses.saturating_add(1);
    entry.first_used_at_ms.get_or_insert(now_ms);
}

/// Return up to `limit` emoji strings sorted by (decayed score desc,
/// last_used desc, dataset order asc). `dataset_order` is a map from the emoji
/// string to its index in the canonical EMOJIS slice; missing entries sort last
/// on that key.
pub fn ranked_frequent(
    store: &EmojiUsageStore,
    now_ms: i64,
    limit: usize,
    dataset_order: impl Fn(&str) -> Option<usize>,
) -> Vec<String> {
    let half_life_secs = store.half_life_secs;
    let mut scored: Vec<(f64, i64, usize, &String)> = store
        .entries
        .iter()
        .filter_map(|(emoji, entry)| {
            let score = decayed_score(entry, now_ms, half_life_secs);
            if score <= 0.0 {
                return None;
            }
            let order = dataset_order(emoji).unwrap_or(usize::MAX);
            Some((score, entry.last_used_at_ms, order, emoji))
        })
        .collect();
    scored.sort_by(|a, b| {
        b.0.partial_cmp(&a.0)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.2.cmp(&b.2))
    });
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, _, emoji)| emoji.clone())
        .collect()
}

/// Convenience: record a use at the current wall-clock time, persisting through
/// the atomic-rename save. Errors are logged by the caller.
pub fn record_emoji_use(emoji: &str) -> anyhow::Result<()> {
    let mut store = load_emoji_usage().unwrap_or_default();
    let now_ms = chrono::Utc::now().timestamp_millis();
    record_use(&mut store, emoji, now_ms);
    save_emoji_usage(&store)
}

/// Build the frequent-emoji snapshot from disk. On missing or corrupt files we
/// fall back to an empty snapshot so the picker never blocks on usage I/O.
pub fn load_frequent_snapshot(limit: usize) -> Vec<String> {
    let store = match load_emoji_usage() {
        Ok(s) => s,
        Err(error) => {
            tracing::warn!(
                error = %error,
                "failed to load emoji usage; treating as empty",
            );
            return Vec::new();
        }
    };
    let now_ms = chrono::Utc::now().timestamp_millis();
    ranked_frequent(&store, now_ms, limit, crate::emoji::dataset_order_of)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    const T0: i64 = 1_700_000_000_000; // arbitrary fixed epoch for decay math

    fn half_life_ms() -> i64 {
        (EMOJI_USAGE_HALF_LIFE_SECS * 1000.0) as i64
    }

    #[test]
    fn round_trip_serialization() {
        let mut store = EmojiUsageStore::default();
        record_use(&mut store, "😀", T0);
        record_use(&mut store, "❤️", T0 + 500);

        let json = serde_json::to_string_pretty(&store).expect("serialize");
        let loaded: EmojiUsageStore = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(loaded, store);
    }

    #[test]
    fn missing_file_returns_default() {
        let path =
            std::env::temp_dir().join(format!("emoji-usage-missing-{}.json", uuid::Uuid::new_v4()));
        assert!(!path.exists());
        let store = load_emoji_usage_from_path(&path).expect("load");
        assert_eq!(store, EmojiUsageStore::default());
    }

    #[test]
    fn corrupt_file_returns_error() {
        let temp = NamedTempFile::new().expect("temp");
        std::fs::write(temp.path(), b"not json").expect("write");
        let result = load_emoji_usage_from_path(temp.path());
        assert!(result.is_err());
    }

    #[test]
    fn decay_math_halves_over_half_life() {
        let mut entry = EmojiUsageEntry::default();
        entry.score = 4.0;
        entry.score_updated_at_ms = T0;

        let now = decayed_score(&entry, T0, EMOJI_USAGE_HALF_LIFE_SECS);
        assert!((now - 4.0).abs() < 1e-9);

        let one_hl = decayed_score(&entry, T0 + half_life_ms(), EMOJI_USAGE_HALF_LIFE_SECS);
        assert!(
            (one_hl - 2.0).abs() < 1e-3,
            "one half-life => ~2.0, got {}",
            one_hl
        );

        let two_hl = decayed_score(&entry, T0 + 2 * half_life_ms(), EMOJI_USAGE_HALF_LIFE_SECS);
        assert!(
            (two_hl - 1.0).abs() < 1e-3,
            "two half-lives => ~1.0, got {}",
            two_hl
        );
    }

    #[test]
    fn record_use_accumulates_decayed_score() {
        let mut store = EmojiUsageStore::default();
        record_use(&mut store, "😀", T0);
        {
            let entry = store.entries.get("😀").unwrap();
            assert_eq!(entry.total_uses, 1);
            assert!((entry.score - 1.0).abs() < 1e-9);
            assert_eq!(entry.last_used_at_ms, T0);
            assert_eq!(entry.first_used_at_ms, Some(T0));
        }

        for _ in 0..3 {
            record_use(&mut store, "😀", T0);
        }
        let entry = store.entries.get("😀").unwrap();
        assert_eq!(entry.total_uses, 4);
        assert!((entry.score - 4.0).abs() < 1e-9);

        record_use(&mut store, "😀", T0 + half_life_ms());
        let entry = store.entries.get("😀").unwrap();
        assert_eq!(entry.total_uses, 5);
        assert!(
            (entry.score - 3.0).abs() < 1e-3,
            "2.0 decayed + 1.0 recorded = ~3.0, got {}",
            entry.score
        );
    }

    #[test]
    fn ranked_frequent_orders_by_score_then_last_used_then_dataset() {
        let mut store = EmojiUsageStore::default();
        // Equal score by recording once each; break ties by last_used and dataset order.
        record_use(&mut store, "🥉", T0);
        record_use(&mut store, "🥈", T0 + 10);
        record_use(&mut store, "🥇", T0 + 20);

        let dataset = |emoji: &str| match emoji {
            "🥇" => Some(0),
            "🥈" => Some(1),
            "🥉" => Some(2),
            _ => None,
        };

        let ranked = ranked_frequent(&store, T0 + 100, 3, dataset);
        assert_eq!(
            ranked,
            vec!["🥇".to_string(), "🥈".to_string(), "🥉".to_string()]
        );

        // Boost 🥉 so it leads purely by score.
        record_use(&mut store, "🥉", T0 + 30);
        record_use(&mut store, "🥉", T0 + 40);
        let ranked = ranked_frequent(&store, T0 + 100, 3, dataset);
        assert_eq!(ranked[0], "🥉".to_string());
    }

    #[test]
    fn ranked_frequent_respects_limit() {
        let mut store = EmojiUsageStore::default();
        for &e in &["🥇", "🥈", "🥉", "😀"] {
            record_use(&mut store, e, T0);
        }
        let dataset = |_: &str| Some(0usize);
        let ranked = ranked_frequent(&store, T0 + 1, 2, dataset);
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn ranked_frequent_skips_decayed_to_zero() {
        let mut store = EmojiUsageStore::default();
        record_use(&mut store, "😀", T0);
        // Fully decay the score so it falls below the inclusion threshold.
        store.entries.get_mut("😀").unwrap().score = 0.0;
        let ranked = ranked_frequent(&store, T0, 5, |_| Some(0usize));
        assert!(ranked.is_empty());
    }

    #[test]
    fn save_and_load_round_trip_atomic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("emoji-usage.json");
        let mut store = EmojiUsageStore::default();
        record_use(&mut store, "😀", T0);
        save_emoji_usage_to_path(&store, &path).expect("save");
        let loaded = load_emoji_usage_from_path(&path).expect("load");
        assert_eq!(loaded, store);
    }
}
