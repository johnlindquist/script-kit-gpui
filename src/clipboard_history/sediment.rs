//! Clipboard sediment: URL auto-keep and re-paste promotion (T10).
//!
//! ## Re-paste promotion (v1 proxy)
//! The clipboard monitor cannot observe paste events. Promotion therefore fires
//! when the same text is *copied* again: each deduped re-copy bumps
//! [`copy_count`] at a new timestamp (see `add_entry`). When `copy_count ≥ 2`
//! for a non-URL entry that is not yet brain-kept, a clipboard-history deeplink
//! is promoted to the day page.
//!
//! ## URL dedup
//! The same URL copied repeatedly within one calendar day keeps a single
//! kept-URL line on that day's page; later copies only bump `copy_count`.

use std::sync::{OnceLock, RwLock};

use chrono::{DateTime, Utc};
use tracing::{debug, warn};

use crate::brain::store::{self, ClipboardSedimentTier};
use crate::brain::substrate::{BrainSubstrate, DayEntry};

#[cfg(test)]
use super::database::{get_entry_content, remove_entry};
use super::database::{get_entry_sediment_state, mark_brain_kept, SedimentState};

static SUBSTRATE: OnceLock<RwLock<Option<BrainSubstrate>>> = OnceLock::new();

fn substrate_lock() -> &'static RwLock<Option<BrainSubstrate>> {
    SUBSTRATE.get_or_init(|| RwLock::new(None))
}

/// Resolve the brain substrate for sediment writes (production: `default_kit`).
pub fn sediment_substrate() -> BrainSubstrate {
    if let Ok(guard) = substrate_lock().read() {
        if let Some(substrate) = guard.as_ref() {
            return substrate.clone();
        }
    }
    BrainSubstrate::default_kit()
}

/// Override the substrate base path (tests only — never `~/.scriptkit`).
#[cfg(test)]
pub fn set_test_sediment_substrate(substrate: BrainSubstrate) {
    if let Ok(mut guard) = substrate_lock().write() {
        *guard = Some(substrate);
    }
}

/// Clear a test substrate override.
#[cfg(test)]
pub fn clear_test_sediment_substrate() {
    if let Ok(mut guard) = substrate_lock().write() {
        *guard = None;
    }
}

/// True when `text` is a single http/https token (no surrounding prose).
pub fn is_single_token_http_url(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.split_whitespace().count() != 1 {
        return false;
    }
    trimmed.starts_with("http://") || trimmed.starts_with("https://")
}

/// Whether a kept-URL day-page line should be skipped for today's date.
pub fn should_skip_url_day_line(kept_url_day: Option<&str>, today: &str) -> bool {
    kept_url_day.is_some_and(|day| day == today)
}

/// Core decision for sediment actions (unit-tested without sqlite/substrate).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SedimentDecision {
    /// First copy of non-URL text — history only, not brain-kept.
    NoOp,
    /// Auto-keep a URL (optionally skip duplicate day-page line same day).
    KeepUrl { skip_day_line: bool },
    /// Re-copy promotion for non-URL text (fragment vs inline chosen at apply).
    PromoteReCopy,
}

pub fn decide_sediment(text: &str, state: &SedimentState, today: &str) -> SedimentDecision {
    if is_single_token_http_url(text) {
        return SedimentDecision::KeepUrl {
            skip_day_line: should_skip_url_day_line(state.kept_url_day.as_deref(), today),
        };
    }

    if state.brain_kept {
        return SedimentDecision::NoOp;
    }

    if state.copy_count >= 2 {
        SedimentDecision::PromoteReCopy
    } else {
        SedimentDecision::NoOp
    }
}

/// Apply sediment rules after a text entry is stored (post-rejection gate).
pub fn process_text_sediment(entry_id: &str, text: &str, now: DateTime<Utc>) {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)
    {
        tracing::warn!(%error, "Clipboard sediment access refused");
        return;
    }
    let Some(state) = get_entry_sediment_state(entry_id) else {
        warn!(entry_id = %entry_id, "sediment skipped: entry not found");
        return;
    };

    let today = now.format("%Y-%m-%d").to_string();
    let decision = decide_sediment(text, &state, &today);

    match decision {
        SedimentDecision::NoOp => {
            debug!(entry_id = %entry_id, copy_count = state.copy_count, "sediment no-op");
        }
        SedimentDecision::KeepUrl { skip_day_line } => {
            if let Err(error) = keep_url(entry_id, text, now, skip_day_line, &today) {
                warn!(entry_id = %entry_id, error = %error, "URL sediment keep failed");
            }
        }
        SedimentDecision::PromoteReCopy => {
            if let Err(error) = promote_recopy(entry_id, text, now) {
                warn!(entry_id = %entry_id, error = %error, "re-copy promotion failed");
            }
        }
    }
}

fn keep_url(
    entry_id: &str,
    url: &str,
    now: DateTime<Utc>,
    skip_day_line: bool,
    today: &str,
) -> anyhow::Result<()> {
    let substrate = sediment_substrate();
    if !skip_day_line {
        substrate.append_to_day(
            now,
            DayEntry::ClipboardRef {
                entry_id: entry_id.to_string(),
            },
        )?;
    }

    mark_brain_kept(
        entry_id,
        ClipboardSedimentTier::Sediment.as_i64(),
        Some(today),
    )?;
    store::record_sediment_signals(url);
    crate::nux::sediment_disclosure::record_activity();
    debug!(
        entry_id = %entry_id,
        skip_day_line,
        "clipboard URL auto-kept"
    );
    Ok(())
}

/// Manually keep a clipboard entry on today's day page — the visible
/// counterpart to the silent auto-keep rules, surfaced as a Cmd+K action in
/// Clipboard History. Returns `false` when the entry was already brain-kept
/// (no duplicate day-page line is written).
pub fn keep_entry_in_today(entry_id: &str, now: DateTime<Utc>) -> anyhow::Result<bool> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemClipboard)?;
    let state = get_entry_sediment_state(entry_id)
        .ok_or_else(|| anyhow::anyhow!("clipboard entry not found: {entry_id}"))?;
    if state.brain_kept {
        return Ok(false);
    }

    let substrate = sediment_substrate();
    substrate.append_to_day(
        now,
        DayEntry::ClipboardRef {
            entry_id: entry_id.to_string(),
        },
    )?;
    mark_brain_kept(entry_id, ClipboardSedimentTier::Sediment.as_i64(), None)?;
    store::record_sediment_signals(&crate::clipboard_history::entry_resource_uri(entry_id));
    debug!(entry_id = %entry_id, "clipboard entry manually kept to Today");
    Ok(true)
}

/// Promote a clipboard entry to brain with an optional post-copy why (T12).
#[cfg(test)]
pub fn annotate_clipboard_entry(
    entry_id: &str,
    why: &str,
    now: DateTime<Utc>,
) -> anyhow::Result<()> {
    let _text = get_entry_content(entry_id)
        .filter(|content| !content.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("clipboard entry not found: {entry_id}"))?;

    let substrate = sediment_substrate();
    let trimmed_why = why.trim();
    substrate.append_to_day(
        now,
        DayEntry::ClipboardRef {
            entry_id: entry_id.to_string(),
        },
    )?;
    if !trimmed_why.is_empty() {
        tracing::debug!(entry_id = %entry_id, "clipboard annotation why is not persisted with raw-free refs");
    }

    mark_brain_kept(entry_id, ClipboardSedimentTier::Sediment.as_i64(), None)?;
    store::record_sediment_signals(&crate::clipboard_history::entry_resource_uri(entry_id));
    debug!(entry_id = %entry_id, "clipboard entry annotated to brain");
    Ok(())
}

#[cfg(test)]
fn reject_clipboard_entry_at(entry_id: &str, now: DateTime<Utc>) -> anyhow::Result<()> {
    let text = get_entry_content(entry_id).unwrap_or_default();
    let state = get_entry_sediment_state(entry_id);
    let substrate = sediment_substrate();

    if let Some(state) = state.as_ref() {
        substrate.undo_clipboard_sediment_lines(
            now,
            entry_id,
            &text,
            state.kept_url_day.as_deref(),
            state.brain_kept,
        )?;
    }

    remove_entry(entry_id)?;
    debug!(entry_id = %entry_id, "clipboard entry rejected");
    Ok(())
}

fn promote_recopy(entry_id: &str, text: &str, now: DateTime<Utc>) -> anyhow::Result<()> {
    let substrate = sediment_substrate();
    substrate.append_to_day(
        now,
        DayEntry::ClipboardRef {
            entry_id: entry_id.to_string(),
        },
    )?;

    mark_brain_kept(entry_id, ClipboardSedimentTier::Sediment.as_i64(), None)?;
    store::record_sediment_signals(text);
    crate::nux::sediment_disclosure::record_activity();
    debug!(entry_id = %entry_id, "clipboard re-copy promoted to brain");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard_history::database::{
        add_entry, get_entry_sediment_state, init_test_clipboard_db, reset_test_clipboard_db,
        test_db_lock,
    };
    use crate::clipboard_history::types::ContentType;
    use chrono::TimeZone as _;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_paths(test_name: &str) -> (tempfile::TempDir, PathBuf, BrainSubstrate) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join(format!("{test_name}-{nanos}.sqlite"));
        let brain_base = dir.path().join("brain");
        let substrate = BrainSubstrate::with_timezone(&brain_base, chrono_tz::UTC);
        (dir, db_path, substrate)
    }

    fn fixed_now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 6, 11, 14, 30, 0).unwrap()
    }

    fn read_today_day_page(substrate: &BrainSubstrate, now: DateTime<Utc>) -> String {
        let path = substrate.paths().day_page(now.date_naive());
        fs::read_to_string(path).unwrap_or_default()
    }

    fn count_substr(haystack: &str, needle: &str) -> usize {
        haystack.match_indices(needle).count()
    }

    fn clipboard_ref(entry_id: &str) -> String {
        format!(
            "[Clipboard entry]({})",
            crate::clipboard_history::entry_resource_uri(entry_id)
        )
    }

    #[test]
    fn is_single_token_http_url_accepts_bare_urls_rejects_prose() {
        assert!(is_single_token_http_url("https://example.com"));
        assert!(is_single_token_http_url("http://a.co/path"));
        assert!(!is_single_token_http_url("see https://example.com"));
        assert!(!is_single_token_http_url("https://a.com https://b.com"));
        assert!(!is_single_token_http_url("not a url"));
    }

    #[test]
    fn decide_sediment_url_first_copy_keeps() {
        let state = SedimentState {
            brain_kept: false,
            brain_tier: 0,
            copy_count: 1,
            kept_url_day: None,
        };
        assert_eq!(
            decide_sediment("https://example.com", &state, "2026-06-11"),
            SedimentDecision::KeepUrl {
                skip_day_line: false
            }
        );
    }

    #[test]
    fn decide_sediment_url_same_day_skips_duplicate_line() {
        let state = SedimentState {
            brain_kept: true,
            brain_tier: 1,
            copy_count: 3,
            kept_url_day: Some("2026-06-11".to_string()),
        };
        assert_eq!(
            decide_sediment("https://example.com", &state, "2026-06-11"),
            SedimentDecision::KeepUrl {
                skip_day_line: true
            }
        );
    }

    #[test]
    fn decide_sediment_non_url_single_copy_is_no_op() {
        let state = SedimentState {
            brain_kept: false,
            brain_tier: 0,
            copy_count: 1,
            kept_url_day: None,
        };
        assert_eq!(
            decide_sediment("hello world", &state, "2026-06-11"),
            SedimentDecision::NoOp
        );
    }

    #[test]
    fn decide_sediment_non_url_recopy_promotes() {
        let state = SedimentState {
            brain_kept: false,
            brain_tier: 0,
            copy_count: 2,
            kept_url_day: None,
        };
        assert_eq!(
            decide_sediment("hello world", &state, "2026-06-11"),
            SedimentDecision::PromoteReCopy
        );
    }

    #[test]
    fn annotate_and_reject_clipboard_entry_round_trip() {
        let _guard = test_db_lock();
        let (_dir, db_path, substrate) = unique_temp_paths("annotate-reject");
        init_test_clipboard_db(&db_path).expect("test db");
        set_test_sediment_substrate(substrate.clone());
        let now = fixed_now();

        let text = "research link for the new API";
        let entry_id = add_entry(text, ContentType::Text).expect("add");
        annotate_clipboard_entry(&entry_id, "needed for the auth doc", now).expect("annotate");

        let state = get_entry_sediment_state(&entry_id).expect("state");
        assert!(state.brain_kept);
        let day_page = read_today_day_page(&substrate, now);
        assert!(day_page.contains(&clipboard_ref(&entry_id)));
        assert!(!day_page.contains(text));
        assert!(!day_page.contains("Why: needed for the auth doc"));

        reject_clipboard_entry_at(&entry_id, now).expect("reject");
        assert!(get_entry_sediment_state(&entry_id).is_none());
        let day_after_reject = read_today_day_page(&substrate, now);
        assert!(!day_after_reject.contains(text));

        clear_test_sediment_substrate();
        reset_test_clipboard_db();
    }

    #[test]
    fn sediment_behavior_contract() {
        let _guard = test_db_lock();
        let (_dir, db_path, substrate) = unique_temp_paths("sediment-contract");
        init_test_clipboard_db(&db_path).expect("test db");
        set_test_sediment_substrate(substrate.clone());
        let now = fixed_now();
        let today = "2026-06-11";

        // URL copy → day-page line + brain-kept entry
        let url = "https://example.com/doc";
        let url_id = add_entry(url, ContentType::Text).expect("url add");
        process_text_sediment(&url_id, url, now);
        let url_state = get_entry_sediment_state(&url_id).expect("url state");
        assert!(url_state.brain_kept);
        assert_eq!(
            url_state.brain_tier,
            ClipboardSedimentTier::Sediment.as_i64()
        );
        assert_eq!(url_state.kept_url_day.as_deref(), Some(today));
        let day_after_url = read_today_day_page(&substrate, now);
        assert!(day_after_url.contains(&clipboard_ref(&url_id)));
        assert!(!day_after_url.contains(url));
        assert_eq!(count_substr(&day_after_url, &clipboard_ref(&url_id)), 1);

        // Second copy same URL same day → no duplicate day line
        let url_id_again = add_entry(url, ContentType::Text).expect("url re-add");
        assert_eq!(url_id_again, url_id);
        process_text_sediment(&url_id_again, url, now + chrono::Duration::minutes(2));
        let day_after_repeat_url = read_today_day_page(&substrate, now);
        assert_eq!(
            count_substr(&day_after_repeat_url, &clipboard_ref(&url_id)),
            1
        );
        let repeat_state = get_entry_sediment_state(&url_id).expect("repeat state");
        assert!(repeat_state.copy_count >= 2);

        // Non-URL single copy → NOT kept
        let short = "meeting notes for Thursday";
        let short_id = add_entry(short, ContentType::Text).expect("short add");
        process_text_sediment(&short_id, short, now);
        let short_state = get_entry_sediment_state(&short_id).expect("short state");
        assert!(!short_state.brain_kept);
        let day_after_short = read_today_day_page(&substrate, now);
        assert!(!day_after_short.contains(short));

        // Re-copy short text → promoted as inline day-page line
        let short_id_2 = add_entry(short, ContentType::Text).expect("short re-add");
        assert_eq!(short_id_2, short_id);
        process_text_sediment(&short_id_2, short, now + chrono::Duration::minutes(5));
        let short_promoted = get_entry_sediment_state(&short_id).expect("promoted state");
        assert!(short_promoted.brain_kept);
        let day_after_promote = read_today_day_page(&substrate, now);
        assert!(day_after_promote.contains(&clipboard_ref(&short_id)));
        assert!(!day_after_promote.contains(short));

        // Re-copy long text → same raw-free clipboard ref, no fragment body.
        let long_body: String = (0..250)
            .map(|i| format!("word{i}"))
            .collect::<Vec<_>>()
            .join(" ");
        let long_id = add_entry(&long_body, ContentType::Text).expect("long add");
        process_text_sediment(&long_id, &long_body, now);
        assert!(!get_entry_sediment_state(&long_id).unwrap().brain_kept);
        let long_id_2 = add_entry(&long_body, ContentType::Text).expect("long re-add");
        process_text_sediment(&long_id_2, &long_body, now + chrono::Duration::minutes(7));
        assert!(get_entry_sediment_state(&long_id).unwrap().brain_kept);
        let day_after_long = read_today_day_page(&substrate, now);
        assert!(day_after_long.contains(&clipboard_ref(&long_id)));
        assert!(!day_after_long.contains("word249"));
        assert!(!day_after_long.contains("../fragments/"));
        assert!(
            !substrate.paths().fragments_dir().exists(),
            "clipboard sediment must not create raw clipboard fragments"
        );

        clear_test_sediment_substrate();
        reset_test_clipboard_db();
    }
}
