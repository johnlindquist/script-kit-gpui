//! Shared formatting helpers for human-readable file sizes, relative times, and durations.
//!
//! Consolidates duplicate implementations from across the codebase into a single source of truth.

use chrono::{DateTime, Datelike, Local, TimeZone, Utc};
use chrono_humanize::{Accuracy, HumanTime, Tense};

/// Format a byte count as a human-readable file size (e.g. "1.5 MB", "456 KB", "12 B").
///
/// Uses binary (1024-based) thresholds with conventional suffixes (KB, MB, GB).
/// Delegates to `humansize` with the BINARY preset and normalises "KiB"→"KB" etc.
pub fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        // Small files: no decimal places, matches existing "X B" output.
        format!("{bytes} B")
    } else {
        // BINARY: powers of 1024 with "KiB", "MiB", "GiB" suffixes.
        // decimal_zeroes(1) keeps trailing ".0" to match legacy "{:.1}" formatting.
        let opts = humansize::FormatSizeOptions::from(humansize::BINARY)
            .decimal_places(1)
            .decimal_zeroes(1);
        // Normalise IEC suffixes to conventional ones: "KiB"→"KB", "MiB"→"MB", etc.
        humansize::format_size(bytes, opts).replace("iB", "B")
    }
}

/// Format a Unix timestamp (seconds since epoch) as a human-friendly relative time string.
///
/// Output examples: "just now", "3 minutes ago", "1 hour ago", "2 weeks ago".
/// Used in file search results and environment variable prompts.
pub fn format_relative_time_long(unix_timestamp: u64) -> String {
    if unix_timestamp == 0 {
        return "Unknown".to_string();
    }

    let now = Utc::now().timestamp();
    format_relative_seconds(now.saturating_sub(unix_timestamp as i64).max(0))
}

/// Format a `chrono::DateTime<Utc>` as a human-friendly relative time string.
///
/// Same output style as [`format_relative_time_long`] but accepts a chrono DateTime.
pub fn format_relative_time_long_dt(dt: DateTime<Utc>) -> String {
    format_relative_datetime(dt)
}

/// Format a `chrono::DateTime<Utc>` as a human-friendly relative time string.
///
/// Output examples: "just now", "5 minutes ago", "2 hours ago", "1 day ago".
/// Used in notes, clipboard history, and AI chat timestamps.
pub fn format_relative_time_short_dt(dt: DateTime<Utc>) -> String {
    format_relative_datetime(dt)
}

/// Format a millisecond timestamp (e.g. clipboard entry timestamps) as a short relative time.
///
/// Output examples: "just now", "3 minutes ago", "2 hours ago", "5 days ago".
#[allow(dead_code)]
pub fn format_relative_time_short_millis(timestamp_ms: i64) -> String {
    format_relative_time_short_millis_at(timestamp_ms, Utc::now().timestamp_millis())
}

/// Same production formatting with an explicit wall-clock reference.
pub fn format_relative_time_short_millis_at(timestamp_ms: i64, now_ms: i64) -> String {
    let age_secs = (now_ms - timestamp_ms) / 1000;

    format_relative_seconds(age_secs.max(0))
}

/// Format a `chrono::DateTime<Utc>` as a local, readable absolute timestamp.
pub fn format_absolute_datetime(dt: DateTime<Utc>) -> String {
    dt.with_timezone(&Local)
        .format("%b %-d, %Y at %-I:%M %p")
        .to_string()
}

/// Format a Unix timestamp as a local, readable absolute timestamp.
pub fn format_absolute_unix_seconds(unix_timestamp: i64) -> String {
    Utc.timestamp_opt(unix_timestamp, 0)
        .single()
        .map(format_absolute_datetime)
        .unwrap_or_else(|| "unknown time".to_string())
}

/// Format a Unix millisecond timestamp as a local, readable absolute timestamp.
pub fn format_absolute_unix_millis(unix_timestamp_ms: i64) -> String {
    DateTime::<Utc>::from_timestamp_millis(unix_timestamp_ms)
        .map(format_absolute_datetime)
        .unwrap_or_else(|| "unknown time".to_string())
}

/// Format a file modification timestamp like Finder's Date Modified column.
///
/// Uses compact contextual labels instead of verbose strings such as
/// "Today at 7:08 AM" or "May 19, 2026 at 11:16 PM".
pub fn format_finder_modified_timestamp(unix_timestamp: u64) -> String {
    if unix_timestamp == 0 {
        return "—".to_string();
    }

    let Some(dt) = Utc.timestamp_opt(unix_timestamp as i64, 0).single() else {
        return "—".to_string();
    };

    let dt = dt.with_timezone(&Local);
    let now = Local::now();
    let today = now.date_naive();
    let date = dt.date_naive();
    let days_ago = today.signed_duration_since(date).num_days();

    match days_ago {
        // Today and yesterday: time only ("7:08 AM").
        0 | 1 => dt.format("%-I:%M %p").to_string(),
        // Earlier this week: weekday ("Mon").
        2..=6 => dt.format("%a").to_string(),
        // Same calendar year: month/day ("May 19").
        _ if dt.year() == now.year() => dt.format("%b %-e").to_string(),
        // Prior years: numeric date ("5/19/23").
        _ => dt.format("%-m/%-e/%y").to_string(),
    }
}

/// Format an RFC3339 timestamp as a local display date.
pub fn format_rfc3339_date_for_display(timestamp: &str) -> String {
    DateTime::parse_from_rfc3339(timestamp)
        .map(|dt| dt.with_timezone(&Local).format("%b %-d, %Y").to_string())
        .unwrap_or_else(|_| timestamp.to_string())
}

/// Format how long a process has been running using a caller-supplied clock.
pub fn format_running_duration(now: DateTime<Utc>, started_at: DateTime<Utc>) -> String {
    let elapsed = now.signed_duration_since(started_at);
    if elapsed.num_hours() > 0 {
        format!("{}h {}m", elapsed.num_hours(), elapsed.num_minutes() % 60)
    } else if elapsed.num_minutes() > 0 {
        format!("{}m {}s", elapsed.num_minutes(), elapsed.num_seconds() % 60)
    } else {
        format!("{}s", elapsed.num_seconds().max(0))
    }
}

/// Format a `std::time::Duration` as a compact human-readable label.
///
/// Output: "345ms" for sub-second, "5s" for whole seconds.
/// Uses `humantime::format_duration` for consistent output.
pub fn format_duration_compact(dur: std::time::Duration) -> String {
    if dur.as_secs() < 1 {
        humantime::format_duration(std::time::Duration::from_millis(dur.as_millis() as u64))
            .to_string()
    } else {
        humantime::format_duration(std::time::Duration::from_secs(dur.as_secs())).to_string()
    }
}

// ── Internal ──────────────────────────────────────────────────────────────────

pub(crate) fn format_relative_seconds(seconds: i64) -> String {
    if seconds < 60 {
        return "just now".to_string();
    }

    let minute_floor = seconds - (seconds % 60);
    HumanTime::from(chrono::Duration::seconds(-minute_floor))
        .to_text_en(Accuracy::Precise, Tense::Past)
}

fn format_relative_datetime(dt: DateTime<Utc>) -> String {
    let now = Utc::now();
    if dt > now {
        return "just now".to_string();
    }

    format_relative_seconds(now.signed_duration_since(dt).num_seconds())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use std::time::Duration;

    // ── format_file_size ──────────────────────────────────────────────────

    #[test]
    fn test_format_file_size_zero() {
        assert_eq!(format_file_size(0), "0 B");
    }

    #[test]
    fn test_format_file_size_bytes() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1023), "1023 B");
    }

    #[test]
    fn test_format_file_size_kilobytes() {
        let result = format_file_size(1024);
        assert!(result.contains("KB"), "expected KB, got: {result}");
    }

    #[test]
    fn test_format_file_size_megabytes() {
        let result = format_file_size(1024 * 1024 * 3);
        assert!(result.contains("MB"), "expected MB, got: {result}");
    }

    #[test]
    fn test_format_file_size_gigabytes() {
        let result = format_file_size(1024 * 1024 * 1024 * 2);
        assert!(result.contains("GB"), "expected GB, got: {result}");
    }

    // ── format_relative_time_long ─────────────────────────────────────────

    #[test]
    fn test_relative_time_long_zero_timestamp() {
        assert_eq!(format_relative_time_long(0), "Unknown");
    }

    #[test]
    fn test_relative_time_long_just_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time_long(now), "just now");
    }

    #[test]
    fn test_relative_time_long_minutes() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time_long(now - 120), "2 minutes ago");
    }

    #[test]
    fn test_relative_time_long_hours() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time_long(now - 7200), "2 hours ago");
    }

    #[test]
    fn test_relative_time_long_singular() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        assert_eq!(format_relative_time_long(now - 60), "1 minute ago");
        assert_eq!(format_relative_time_long(now - 3600), "1 hour ago");
    }

    // ── format_relative_time_long_dt ──────────────────────────────────────

    #[test]
    fn test_relative_time_long_dt_future() {
        let future = Utc::now() + TimeDelta::hours(1);
        assert_eq!(format_relative_time_long_dt(future), "just now");
    }

    #[test]
    fn test_relative_time_long_dt_days() {
        let past = Utc::now() - TimeDelta::days(3);
        assert_eq!(format_relative_time_long_dt(past), "3 days ago");
    }

    // ── format_relative_time_short_dt ─────────────────────────────────────

    #[test]
    fn test_relative_time_short_just_now() {
        let now = Utc::now();
        assert_eq!(format_relative_time_short_dt(now), "just now");
    }

    #[test]
    fn test_relative_time_short_seconds() {
        let past = Utc::now() - TimeDelta::seconds(30);
        assert_eq!(format_relative_time_short_dt(past), "just now");
    }

    #[test]
    fn test_relative_time_short_floors_to_whole_minutes() {
        let past = Utc::now() - TimeDelta::seconds(95);
        assert_eq!(format_relative_time_short_dt(past), "1 minute ago");
    }

    #[test]
    fn test_relative_time_short_minutes() {
        let past = Utc::now() - TimeDelta::minutes(5);
        assert_eq!(format_relative_time_short_dt(past), "5 minutes ago");
    }

    #[test]
    fn test_relative_time_short_hours() {
        let past = Utc::now() - TimeDelta::hours(3);
        assert_eq!(format_relative_time_short_dt(past), "3 hours ago");
    }

    #[test]
    fn test_relative_time_short_days() {
        let past = Utc::now() - TimeDelta::days(2);
        assert_eq!(format_relative_time_short_dt(past), "2 days ago");
    }

    #[test]
    fn test_relative_time_short_old_shows_date() {
        let past = Utc::now() - TimeDelta::days(14);
        let result = format_relative_time_short_dt(past);
        assert_eq!(result, "2 weeks ago");
    }

    #[test]
    fn test_rfc3339_date_display_formats_local_date() {
        assert_ne!(
            format_rfc3339_date_for_display("2026-05-14T12:34:56Z"),
            "2026-05-14T12:34:56Z"
        );
    }

    #[test]
    fn test_running_duration_formats_compactly() {
        let started = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
        let now = started + TimeDelta::minutes(62) + TimeDelta::seconds(4);
        assert_eq!(format_running_duration(now, started), "1h 2m");
    }

    #[test]
    fn test_relative_time_short_omits_seconds_for_old_values() {
        let past = Utc::now()
            - TimeDelta::weeks(7)
            - TimeDelta::days(3)
            - TimeDelta::hours(4)
            - TimeDelta::minutes(28)
            - TimeDelta::seconds(17);
        let result = format_relative_time_short_dt(past);
        assert!(
            !result.contains("second"),
            "relative timestamp should not repaint at second precision: {result}"
        );
    }

    // ── format_relative_time_short_millis ─────────────────────────────────

    #[test]
    fn test_relative_time_short_millis_just_now() {
        let now_ms = Utc::now().timestamp_millis();
        assert_eq!(format_relative_time_short_millis(now_ms), "just now");
    }

    #[test]
    fn test_relative_time_short_millis_minutes() {
        let now_ms = Utc::now().timestamp_millis();
        let five_min_ago = now_ms - 5 * 60 * 1000;
        assert_eq!(
            format_relative_time_short_millis(five_min_ago),
            "5 minutes ago"
        );
    }

    #[test]
    fn test_relative_time_short_millis_floors_to_whole_minutes() {
        let now_ms = Utc::now().timestamp_millis();
        let ninety_five_seconds_ago = now_ms - 95 * 1000;
        assert_eq!(
            format_relative_time_short_millis(ninety_five_seconds_ago),
            "1 minute ago"
        );
    }

    #[test]
    fn relative_time_millis_uses_the_explicit_reference_clock() {
        let timestamp = 1_777_593_600_000;
        assert_eq!(
            format_relative_time_short_millis_at(timestamp, timestamp + 60_000),
            "1 minute ago"
        );
        assert_eq!(
            format_relative_time_short_millis_at(timestamp, timestamp + 119_999),
            "1 minute ago"
        );
        assert_eq!(
            format_relative_time_short_millis_at(timestamp, timestamp + 120_000),
            "2 minutes ago"
        );
        assert_eq!(
            format_relative_time_short_millis_at(timestamp, timestamp - 1),
            "just now"
        );
    }

    #[test]
    fn test_absolute_unix_seconds_uses_readable_local_format() {
        // Rendering is in LOCAL time, so epoch 0 lands in 1969 west of UTC.
        // Use a mid-1970 timestamp (200 days, in seconds) so "1970" holds in
        // every timezone.
        let formatted = format_absolute_unix_seconds(200 * 86_400);

        assert_ne!(formatted, "unknown time");
        assert!(formatted.contains("1970"));
        assert!(formatted.contains(" at "));
    }

    #[test]
    fn test_finder_modified_timestamp_zero_is_dash() {
        assert_eq!(format_finder_modified_timestamp(0), "—");
    }

    #[test]
    fn test_finder_modified_timestamp_today_is_time_only() {
        let now = Local::now();
        let formatted = format_finder_modified_timestamp(now.timestamp() as u64);

        assert!(!formatted.contains("Today"));
        assert!(!formatted.contains("Yesterday"));
        assert!(!formatted.contains(" at "));
    }

    #[test]
    fn test_finder_modified_timestamp_same_year_uses_month_day() {
        let now = Local::now();
        let dt = now - TimeDelta::days(30);
        if dt.year() == now.year() {
            let formatted = format_finder_modified_timestamp(dt.timestamp() as u64);
            assert!(formatted.contains(' '));
            assert!(!formatted.contains(" at "));
            assert!(!formatted.contains(':'));
        }
    }

    #[test]
    fn test_finder_modified_timestamp_prior_year_uses_numeric_date() {
        let dt = Local.with_ymd_and_hms(2020, 5, 19, 23, 16, 0).unwrap();
        let formatted = format_finder_modified_timestamp(dt.timestamp() as u64);
        assert_eq!(formatted, "5/19/20");
    }

    // ── format_duration_compact ───────────────────────────────────────────

    #[test]
    fn test_duration_compact_millis() {
        let result = format_duration_compact(Duration::from_millis(345));
        assert_eq!(result, "345ms");
    }

    #[test]
    fn test_duration_compact_seconds() {
        let result = format_duration_compact(Duration::from_secs(5));
        assert_eq!(result, "5s");
    }

    #[test]
    fn test_duration_compact_zero() {
        let result = format_duration_compact(Duration::ZERO);
        assert_eq!(result, "0s");
    }
}
