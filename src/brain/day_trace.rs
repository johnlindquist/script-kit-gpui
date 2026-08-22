//! Agent Chat traces on the day page — one line per thread per calendar day.
//!
//! Appended at the post-turn ingestion point in the Agent Chat pipeline.
//! Best-effort only: failures are logged and never propagated to the chat turn.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

use chrono::{DateTime, NaiveDate, Utc};

use crate::brain::substrate::{BrainSubstrate, DayEntry};

/// In-process dedup key: thread id + local calendar day (substrate timezone).
type TraceKey = (String, NaiveDate);

/// Tracks Agent Chat thread appearances on day pages.
#[derive(Debug)]
pub struct AgentChatDayTrace {
    substrate: BrainSubstrate,
    traced: Mutex<HashSet<TraceKey>>,
}

impl AgentChatDayTrace {
    pub fn new(substrate: BrainSubstrate) -> Self {
        Self {
            substrate,
            traced: Mutex::new(HashSet::new()),
        }
    }

    /// Best-effort trace append for the first turn of a thread on a given day.
    ///
    /// `thread_label` is the thread title or first-message snippet (see
    /// [`format_agent_chat_trace_summary`]).
    pub fn maybe_append(&self, thread_id: &str, thread_label: &str, now: DateTime<Utc>) {
        let thread_id = thread_id.trim();
        if thread_id.is_empty() {
            return;
        }

        let date = now.with_timezone(&self.substrate.timezone()).date_naive();
        let key = (thread_id.to_string(), date);
        let provenance = provenance_link(thread_id);

        if self.already_traced(&key, date, &provenance) {
            return;
        }

        let entry = DayEntry::Trace {
            summary: format_agent_chat_trace_summary(thread_label),
            provenance_link: provenance,
        };

        match self.substrate.append_to_day(now, entry) {
            Ok(()) => {
                if let Ok(mut traced) = self.traced.lock() {
                    traced.insert(key);
                }
            }
            Err(error) => {
                tracing::debug!(
                    target: "script_kit::brain",
                    error = %error,
                    thread_id = %thread_id,
                    "agent chat day trace append failed"
                );
            }
        }
    }

    fn already_traced(&self, key: &TraceKey, date: NaiveDate, provenance: &str) -> bool {
        if let Ok(traced) = self.traced.lock() {
            if traced.contains(key) {
                return true;
            }
        }

        // Restart dedup: if today's day page already contains this thread's
        // provenance link, treat it as traced for the rest of the session.
        // We re-read the file rather than persisting dedup state so a prior
        // run's trace line prevents duplicates after relaunch without extra
        // bookkeeping.
        if day_page_contains_provenance(&self.substrate, date, provenance) {
            if let Ok(mut traced) = self.traced.lock() {
                traced.insert(key.clone());
            }
            return true;
        }

        false
    }
}

fn global_trace() -> &'static AgentChatDayTrace {
    static TRACE: OnceLock<AgentChatDayTrace> = OnceLock::new();
    TRACE.get_or_init(|| AgentChatDayTrace::new(BrainSubstrate::default_kit()))
}

/// Append a day-page trace for this Agent Chat thread when it is the first turn
/// of the day. Never blocks or fails the caller.
pub fn maybe_append_agent_chat_trace(thread_id: &str, thread_label: &str) {
    global_trace().maybe_append(thread_id, thread_label, Utc::now());
}

pub fn provenance_link(thread_id: &str) -> String {
    format!("scriptkit://agent-chat/{thread_id}")
}

/// Format the trace summary line body: `Agent Chat: <label>`.
pub fn format_agent_chat_trace_summary(thread_label: &str) -> String {
    let label = normalize_trace_label(thread_label);
    if label.is_empty() {
        "Agent Chat".to_string()
    } else {
        format!("Agent Chat: {label}")
    }
}

fn normalize_trace_label(raw: &str) -> String {
    let collapsed: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = collapsed.chars().take(100).collect();
    if collapsed.chars().count() > 100 {
        out.push('\u{2026}');
    }
    out
}

fn day_page_contains_provenance(
    substrate: &BrainSubstrate,
    date: NaiveDate,
    provenance: &str,
) -> bool {
    let path = substrate.paths().day_page(date);
    if !path.exists() {
        return false;
    }
    crate::brain::substrate::io::read_private_document(&path)
        .map(|contents| contents.contains(provenance))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone as _;
    use std::fs;
    use std::path::Path;

    fn test_substrate(base: &Path) -> BrainSubstrate {
        BrainSubstrate::with_timezone(base, chrono_tz::UTC)
    }

    fn test_substrate_with_timezone(base: &Path, tz: chrono_tz::Tz) -> BrainSubstrate {
        BrainSubstrate::with_timezone(base, tz)
    }

    fn day_contents(substrate: &BrainSubstrate, date: NaiveDate) -> String {
        fs::read_to_string(substrate.paths().day_page(date)).unwrap_or_default()
    }

    fn count_trace_lines(contents: &str, thread_id: &str) -> usize {
        let link = provenance_link(thread_id);
        contents.lines().filter(|line| line.contains(&link)).count()
    }

    #[test]
    fn two_turns_same_thread_same_day_append_one_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = test_substrate(&dir.path().join("brain"));
        let trace = AgentChatDayTrace::new(substrate);
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 9, 40, 0).unwrap();

        trace.maybe_append("thread-a", "flaky clock-mock test", now);
        trace.maybe_append(
            "thread-a",
            "follow-up question",
            now + chrono::Duration::hours(1),
        );

        let contents = day_contents(&trace.substrate, now.date_naive());
        assert_eq!(count_trace_lines(&contents, "thread-a"), 1);
        assert!(contents.contains(
            "09:40 [Agent Chat: flaky clock-mock test](scriptkit://agent-chat/thread-a)"
        ));
        assert!(contents.contains(&provenance_link("thread-a")));
    }

    #[test]
    fn same_thread_next_day_appends_second_line() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = test_substrate(&dir.path().join("brain"));
        let trace = AgentChatDayTrace::new(substrate);
        let day_one = Utc.with_ymd_and_hms(2026, 6, 11, 10, 0, 0).unwrap();
        let day_two = Utc.with_ymd_and_hms(2026, 6, 12, 8, 15, 0).unwrap();

        trace.maybe_append("thread-b", "first day chat", day_one);
        trace.maybe_append("thread-b", "second day chat", day_two);

        let first_day = day_contents(&trace.substrate, day_one.date_naive());
        let second_day = day_contents(&trace.substrate, day_two.date_naive());
        assert_eq!(count_trace_lines(&first_day, "thread-b"), 1);
        assert_eq!(count_trace_lines(&second_day, "thread-b"), 1);
        assert!(first_day
            .contains("10:00 [Agent Chat: first day chat](scriptkit://agent-chat/thread-b)"));
        assert!(second_day
            .contains("08:15 [Agent Chat: second day chat](scriptkit://agent-chat/thread-b)"));
    }

    #[test]
    fn agent_chat_trace_dedupes_by_configured_local_day() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate =
            test_substrate_with_timezone(&dir.path().join("brain"), chrono_tz::America::Denver);
        let trace = AgentChatDayTrace::new(substrate);
        let late_local_day = Utc.with_ymd_and_hms(2026, 6, 14, 5, 30, 0).unwrap();
        let same_local_day = late_local_day + chrono::Duration::minutes(20);

        trace.maybe_append("thread-local", "first local day trace", late_local_day);
        trace.maybe_append("thread-local", "same local day follow-up", same_local_day);

        let local_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let utc_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 14).unwrap();
        let local_contents = day_contents(&trace.substrate, local_date);
        let utc_contents = day_contents(&trace.substrate, utc_date);
        assert_eq!(count_trace_lines(&local_contents, "thread-local"), 1);
        assert!(local_contents.contains(
            "23:30 [Agent Chat: first local day trace](scriptkit://agent-chat/thread-local)"
        ));
        assert!(utc_contents.is_empty(), "UTC day must not receive trace");
    }

    #[test]
    fn agent_chat_trace_writes_local_timestamp() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate =
            test_substrate_with_timezone(&dir.path().join("brain"), chrono_tz::America::Denver);
        let trace = AgentChatDayTrace::new(substrate);
        let utc_boundary = Utc.with_ymd_and_hms(2026, 6, 14, 5, 30, 0).unwrap();

        trace.maybe_append("thread-local-time", "local timestamp", utc_boundary);

        let local_date = chrono::NaiveDate::from_ymd_opt(2026, 6, 13).unwrap();
        let contents = day_contents(&trace.substrate, local_date);
        assert!(contents.contains(
            "23:30 [Agent Chat: local timestamp](scriptkit://agent-chat/thread-local-time)"
        ));
        assert!(contents.contains(&provenance_link("thread-local-time")));
    }

    #[test]
    fn restart_dedup_reads_existing_day_page_link() {
        let dir = tempfile::tempdir().expect("tempdir");
        let substrate = test_substrate(&dir.path().join("brain"));
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 12, 0, 0).unwrap();
        substrate
            .append_to_day(
                now,
                DayEntry::Trace {
                    summary: "Agent Chat: prior session".to_string(),
                    provenance_link: provenance_link("thread-restart"),
                },
            )
            .expect("seed trace");

        let fresh_trace = AgentChatDayTrace::new(substrate);
        fresh_trace.maybe_append("thread-restart", "new label", now);

        let contents = day_contents(&fresh_trace.substrate, now.date_naive());
        assert_eq!(count_trace_lines(&contents, "thread-restart"), 1);
        assert!(!contents.contains("new label"));
    }

    #[test]
    fn substrate_write_error_does_not_panic() {
        let dir = tempfile::tempdir().expect("tempdir");
        let brain_base = dir.path().join("brain");
        fs::create_dir_all(&brain_base).expect("brain dir");
        // `days` as a file blocks day-page creation.
        fs::write(brain_base.join("days"), "blocked").expect("block days");
        let trace = AgentChatDayTrace::new(test_substrate(&brain_base));
        let now = Utc.with_ymd_and_hms(2026, 6, 11, 9, 0, 0).unwrap();

        trace.maybe_append("thread-err", "should not panic", now);
    }

    #[test]
    fn format_summary_handles_empty_label() {
        assert_eq!(format_agent_chat_trace_summary("  "), "Agent Chat");
        assert_eq!(
            format_agent_chat_trace_summary("deploy pipeline"),
            "Agent Chat: deploy pipeline"
        );
    }
}
