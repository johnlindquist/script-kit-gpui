use chrono::{DateTime, NaiveDateTime, TimeZone};
use chrono_english::{parse_date_string, Dialect};
use chrono_tz::Tz;
use serde::{Deserialize, Serialize};

use super::payload::{CaptureInvocation, DateRole};

#[derive(Debug, Clone)]
pub struct MenuSyntaxClock {
    pub now: DateTime<Tz>,
    pub timezone: Tz,
    pub timezone_label: String,
    pub dialect: Dialect,
}

impl MenuSyntaxClock {
    pub fn fixed(now_iso_local: &str, tz: Tz) -> Option<Self> {
        let naive = NaiveDateTime::parse_from_str(now_iso_local, "%Y-%m-%dT%H:%M:%S").ok()?;
        let now = match tz.from_local_datetime(&naive) {
            chrono::LocalResult::Single(dt) => dt,
            chrono::LocalResult::Ambiguous(dt, _) => dt,
            chrono::LocalResult::None => return None,
        };
        Some(Self {
            now,
            timezone: tz,
            timezone_label: tz.name().to_string(),
            dialect: Dialect::Us,
        })
    }

    /// Build a clock from the current wall clock, detecting the local IANA
    /// timezone before falling back to UTC. Prefer [`fixed`] in tests so DST
    /// pinning stays deterministic.
    pub fn local_now() -> Self {
        static LOCAL_TZ: std::sync::OnceLock<Tz> = std::sync::OnceLock::new();
        let tz = *LOCAL_TZ.get_or_init(|| detect_local_timezone().unwrap_or(chrono_tz::UTC));
        let utc_now = chrono::Utc::now();
        let now = utc_now.with_timezone(&tz);
        Self {
            now,
            timezone: tz,
            timezone_label: tz.name().to_string(),
            dialect: Dialect::Us,
        }
    }
}

fn detect_local_timezone() -> Option<Tz> {
    parse_tz_env()
        .or_else(detect_local_timezone_from_etc_localtime)
        .or_else(detect_local_timezone_from_systemsetup)
}

fn parse_tz_env() -> Option<Tz> {
    let name = std::env::var("TZ").ok()?;
    parse_tz_name(&name)
}

fn parse_tz_name(name: &str) -> Option<Tz> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return None;
    }
    // POSIX allows TZ values like ":America/Denver".
    let trimmed = trimmed.strip_prefix(':').unwrap_or(trimmed);
    trimmed.parse::<Tz>().ok()
}

#[cfg(unix)]
fn detect_local_timezone_from_etc_localtime() -> Option<Tz> {
    let link = std::fs::read_link("/etc/localtime").ok()?;
    let link = link.to_string_lossy();
    for marker in ["zoneinfo/", "zoneinfo.default/"] {
        if let Some((_, tail)) = link.rsplit_once(marker) {
            let tail = tail
                .strip_prefix("posix/")
                .or_else(|| tail.strip_prefix("right/"))
                .unwrap_or(tail);
            if let Some(tz) = parse_tz_name(tail) {
                return Some(tz);
            }
        }
    }
    None
}

#[cfg(not(unix))]
fn detect_local_timezone_from_etc_localtime() -> Option<Tz> {
    None
}

#[cfg(target_os = "macos")]
fn detect_local_timezone_from_systemsetup() -> Option<Tz> {
    let output = std::process::Command::new("/usr/sbin/systemsetup")
        .arg("-gettimezone")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let name = stdout
        .lines()
        .find_map(|line| line.split_once(':').map(|(_, value)| value.trim()))?;
    parse_tz_name(name)
}

#[cfg(not(target_os = "macos"))]
fn detect_local_timezone_from_systemsetup() -> Option<Tz> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DateGranularity {
    Date,
    Minute,
    Second,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDate {
    pub role: DateRole,
    pub source: String,
    pub source_span: (usize, usize),
    pub iso: String,
    /// Optional end timestamp for range expressions like `9-10am`. Skipped from
    /// JSON serialization when None so existing payload files stay byte-identical
    /// — readers without this field see the same shape as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end_iso: Option<String>,
    pub relative: String,
    pub timezone: String,
    pub all_day: bool,
    pub granularity: DateGranularity,
    pub confidence: f32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDuration {
    pub source: String,
    pub source_span: (usize, usize),
    pub seconds: u32,
    pub minutes: u32,
    pub iso8601: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurrenceFrequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecurrenceWeekday {
    Mon,
    Tue,
    Wed,
    Thu,
    Fri,
    Sat,
    Sun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRecurrence {
    pub source: String,
    pub source_span: (usize, usize),
    pub frequency: RecurrenceFrequency,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub weekdays: Vec<RecurrenceWeekday>,
    pub rrule: String,
    pub label: String,
}

/// A date phrase the parser recognized as belonging to a date slot but could
/// not interpret. Used by the snapshot/UI surface to tell the user which key
/// failed (e.g. `due:asdf` → `UnresolvedDate { role: Due, source: "asdf", .. }`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnresolvedDate {
    pub role: DateRole,
    pub source: String,
    pub source_span: (usize, usize),
}

/// The full result of attempting to parse one date phrase. `Empty` means the
/// caller passed whitespace/nothing; `Resolved` carries the parsed `ResolvedDate`;
/// `Unresolved` reports a known date slot that did not parse so the UI can show
/// "due: asdf — not a date" instead of silently swallowing the input.
#[derive(Debug, Clone, PartialEq)]
pub enum DateParseResult {
    Empty,
    Resolved(ResolvedDate),
    Unresolved(UnresolvedDate),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCaptureInvocation {
    pub target: String,
    pub body: String,
    pub tags: Vec<String>,
    pub priority: Option<u8>,
    pub url: Option<String>,
    pub duration: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_resolved: Option<ResolvedDuration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recurrence: Option<ResolvedRecurrence>,
    pub kv: Vec<(String, String)>,
    pub dates: Vec<ResolvedDate>,
    #[serde(default)]
    pub unresolved_dates: Vec<UnresolvedDate>,
    pub raw: String,
}

/// Normalize Unicode smart quotes (curly `“ ” ‘ ’ « »`) to their ASCII
/// equivalents so a phrase the user pasted from a doc with autocorrect-on
/// (e.g. macOS Mail) parses the same as one they typed in a code editor.
/// Only quote characters are touched — every other char passes through
/// byte-identical. Returns `Cow::Borrowed` when the input has no smart
/// quotes so the common case allocates nothing.
pub fn normalize_smart_quotes(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.chars().any(|c| {
        matches!(
            c,
            '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}' | '\u{00AB}' | '\u{00BB}'
        )
    }) {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\u{201C}' | '\u{201D}' | '\u{00AB}' | '\u{00BB}' => out.push('"'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            other => out.push(other),
        }
    }
    std::borrow::Cow::Owned(out)
}

pub fn resolve_date_phrase(raw: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let normalized = normalize_smart_quotes(raw);
    let source = normalized.trim();
    if source.is_empty() {
        return None;
    }
    if let Some(shorthand) = resolve_shorthand_phrase(source, clock) {
        return Some(shorthand);
    }
    if let Some(at_prefixed) = resolve_at_prefix(source, clock) {
        return Some(at_prefixed);
    }
    if let Some(daypart) = resolve_time_of_day_shorthand(source, clock) {
        return Some(daypart);
    }
    if let Some(numeric_time) = resolve_numeric_time_phrase(source, clock) {
        return Some(numeric_time);
    }
    if let Some(absolute) = resolve_absolute_date(source, clock) {
        return Some(absolute);
    }
    if let Some(relative) = resolve_relative_offset(source, clock) {
        return Some(relative);
    }
    if let Some(zoned) = resolve_timezone_suffix(source, clock) {
        return Some(zoned);
    }
    if let Some(ranged) = resolve_time_range(source, clock) {
        return Some(ranged);
    }
    // Guard against an upstream panic in chrono-english 0.1.8 on inputs with
    // non-ASCII multi-byte characters at certain byte boundaries (e.g. `p3é`
    // panics with `byte index 3 is not a char boundary` in
    // `chrono-english/src/types.rs:392`). We don't try to interpret non-ASCII
    // date phrases anyway — Power Syntax dates are ASCII-only — so reject
    // here and let the caller see Unresolved.
    if !source.is_ascii() {
        return None;
    }
    let parsed = parse_date_string(source, clock.now, clock.dialect).ok()?;
    let granularity = infer_granularity(source);
    Some(ResolvedDate {
        role: DateRole::Inferred,
        source: source.to_string(),
        source_span: (0, raw.len()),
        iso: parsed.to_rfc3339(),
        end_iso: None,
        relative: source.to_string(),
        timezone: clock.timezone_label.clone(),
        all_day: matches!(granularity, DateGranularity::Date),
        granularity,
        confidence: 0.9,
    })
}

/// Resolve Todoist-style shorthand tokens (`noon`, `midnight`, `eod`, `eom`)
/// that `chrono_english` doesn't recognize natively. Returns `Some` only when
/// the input matches one of these tokens exactly (case-insensitive,
/// whitespace-trimmed) — leaves anything else for the chrono_english fallback.
///
/// Mappings:
/// - `noon` → today at 12:00:00 (Minute granularity)
/// - `midnight` → today at 00:00:00 (Minute granularity)
/// - `eod` → today at 23:59:00 (Minute granularity)
/// - `eom` → last day of current month at 23:59:00 (Minute granularity)
fn resolve_shorthand_phrase(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    use chrono::{Datelike, NaiveDate};
    let lower = source.to_ascii_lowercase();
    let now = clock.now;
    match lower.as_str() {
        "noon" => build_local_time_resolved(now.date_naive(), 12, 0, source, clock, 0.95),
        "midnight" => build_local_time_resolved(now.date_naive(), 0, 0, source, clock, 0.95),
        "eod" => build_local_time_resolved(now.date_naive(), 23, 59, source, clock, 0.95),
        "eom" => {
            let year = now.year();
            let month = now.month();
            let next_month_first = if month == 12 {
                NaiveDate::from_ymd_opt(year + 1, 1, 1)?
            } else {
                NaiveDate::from_ymd_opt(year, month + 1, 1)?
            };
            let last_day = next_month_first.pred_opt()?;
            build_local_time_resolved(last_day, 23, 59, source, clock, 0.95)
        }
        _ => None,
    }
}

fn resolve_at_prefix(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let rest = strip_ascii_at_prefix(source)?.trim();
    if rest.is_empty() {
        return None;
    }
    let mut resolved = resolve_shorthand_phrase(rest, clock)
        .or_else(|| resolve_numeric_time_phrase(rest, clock))?;
    resolved.source = source.to_string();
    resolved.relative = source.to_string();
    resolved.source_span = (0, source.len());
    Some(resolved)
}

fn strip_ascii_at_prefix(source: &str) -> Option<&str> {
    let bytes = source.as_bytes();
    if bytes.len() < 3 {
        return None;
    }
    if bytes[0].eq_ignore_ascii_case(&b'a')
        && bytes[1].eq_ignore_ascii_case(&b't')
        && bytes[2] == b' '
    {
        Some(&source[3..])
    } else {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TimeMeridiem {
    Am,
    Pm,
}

fn resolve_numeric_time_phrase(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let (hour, minute) = parse_numeric_time_parts(source)?;
    build_local_time_resolved(clock.now.date_naive(), hour, minute, source, clock, 0.95)
}

fn parse_numeric_time_parts(source: &str) -> Option<(u32, u32)> {
    let lower = source.trim().to_ascii_lowercase();
    if lower.is_empty() || lower.chars().any(char::is_whitespace) {
        return None;
    }
    let (body, meridiem) = if let Some(head) = lower.strip_suffix("am") {
        (head, Some(TimeMeridiem::Am))
    } else if let Some(head) = lower.strip_suffix("pm") {
        (head, Some(TimeMeridiem::Pm))
    } else {
        (lower.as_str(), None)
    };
    if body.is_empty() {
        return None;
    }
    let (hour, minute, has_colon) = if let Some((hour, minute)) = body.split_once(':') {
        (hour.parse::<u32>().ok()?, minute.parse::<u32>().ok()?, true)
    } else {
        (body.parse::<u32>().ok()?, 0, false)
    };
    if hour > 23 || minute > 59 {
        return None;
    }
    if meridiem.is_some() && !(1..=12).contains(&hour) {
        return None;
    }
    let hour = materialize_numeric_hour(hour, meridiem, has_colon)?;
    Some((hour, minute))
}

fn materialize_numeric_hour(
    hour: u32,
    meridiem: Option<TimeMeridiem>,
    has_colon: bool,
) -> Option<u32> {
    match meridiem {
        Some(TimeMeridiem::Am) => {
            if hour == 12 {
                Some(0)
            } else {
                Some(hour)
            }
        }
        Some(TimeMeridiem::Pm) => {
            if hour == 12 {
                Some(12)
            } else {
                Some(hour + 12)
            }
        }
        None if has_colon => Some(hour),
        None => match hour {
            0 => Some(0),
            1..=7 => Some(hour + 12),
            8..=23 => Some(hour),
            _ => None,
        },
    }
}

fn resolve_time_of_day_shorthand(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let lower = source.to_ascii_lowercase();
    let (hour, minute) = match lower.as_str() {
        "morning" => (9, 0),
        "afternoon" => (14, 0),
        "evening" => (19, 0),
        "tonight" => (20, 0),
        _ => return None,
    };
    build_local_time_resolved(clock.now.date_naive(), hour, minute, source, clock, 0.95)
}

fn build_local_time_resolved(
    date: chrono::NaiveDate,
    hour: u32,
    minute: u32,
    source: &str,
    clock: &MenuSyntaxClock,
    confidence: f32,
) -> Option<ResolvedDate> {
    let naive = date.and_hms_opt(hour, minute, 0)?;
    let local = match clock.timezone.from_local_datetime(&naive) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return None,
    };
    Some(ResolvedDate {
        role: DateRole::Inferred,
        source: source.to_string(),
        source_span: (0, source.len()),
        iso: local.to_rfc3339(),
        end_iso: None,
        relative: source.to_string(),
        timezone: clock.timezone_label.clone(),
        all_day: false,
        granularity: DateGranularity::Minute,
        confidence,
    })
}

/// Parse absolute calendar dates that `chrono_english` doesn't handle on its
/// own: ISO `YYYY-MM-DD` and US slash `M/D` (year inferred — current year if
/// the resulting date is today-or-later, else next year — matching the
/// disambiguation rule from the date-weekdays-and-absolute story). Returns
/// `Some` only when the input is a recognized absolute-date shape; leaves
/// everything else for the chrono_english fallback so weekday phrases like
/// "next friday" route through their existing path.
fn resolve_absolute_date(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    use chrono::{Datelike, NaiveDate};

    let trimmed = source.trim();

    // ISO date: 2026-04-30.
    if let Ok(naive) = NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        return build_all_day_resolved(naive, source, clock, 0.95);
    }

    // US slash dates: 5/4 → May 4 (year inferred), 5/4/2026 → May 4 2026.
    let parts: Vec<&str> = trimmed.split('/').collect();
    let now_date = clock.now.date_naive();
    let candidate = match parts.as_slice() {
        [m, d] => parse_us_md(m, d, now_date.year()),
        [m, d, y] => parse_us_mdy(m, d, y),
        _ => None,
    };
    let naive = candidate?;

    // Year-rollover for bare M/D: if the date already passed this year, roll
    // to next year so `5/4` in late 2026 lands on 2027-05-04.
    let final_naive = if parts.len() == 2 && naive < now_date {
        NaiveDate::from_ymd_opt(naive.year() + 1, naive.month(), naive.day())?
    } else {
        naive
    };
    build_all_day_resolved(final_naive, source, clock, 0.70)
}

fn parse_us_md(m: &str, d: &str, year: i32) -> Option<chrono::NaiveDate> {
    let month: u32 = m.trim().parse().ok()?;
    let day: u32 = d.trim().parse().ok()?;
    chrono::NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_us_mdy(m: &str, d: &str, y: &str) -> Option<chrono::NaiveDate> {
    let month: u32 = m.trim().parse().ok()?;
    let day: u32 = d.trim().parse().ok()?;
    let year_raw: i32 = y.trim().parse().ok()?;
    let year = if year_raw < 100 {
        2000 + year_raw
    } else {
        year_raw
    };
    chrono::NaiveDate::from_ymd_opt(year, month, day)
}

fn build_all_day_resolved(
    date: chrono::NaiveDate,
    source: &str,
    clock: &MenuSyntaxClock,
    confidence: f32,
) -> Option<ResolvedDate> {
    let naive_dt = date.and_hms_opt(0, 0, 0)?;
    let local = match clock.timezone.from_local_datetime(&naive_dt) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(dt, _) => dt,
        chrono::LocalResult::None => return None,
    };
    Some(ResolvedDate {
        role: DateRole::Inferred,
        source: source.to_string(),
        source_span: (0, source.len()),
        iso: local.to_rfc3339(),
        end_iso: None,
        relative: source.to_string(),
        timezone: clock.timezone_label.clone(),
        all_day: true,
        granularity: DateGranularity::Date,
        confidence,
    })
}

/// Parse `in N <unit>` and compound `in N1 <unit1> N2 <unit2>` phrases
/// that `chrono_english` does not handle natively. Recognized units
/// (case-insensitive): `s`/`sec`/`secs`/`second`/`seconds`,
/// `m`/`min`/`mins`/`minute`/`minutes`,
/// `h`/`hr`/`hrs`/`hour`/`hours`, `d`/`day`/`days`,
/// `w`/`wk`/`wks`/`week`/`weeks`. Returns `None` when the phrase doesn't
/// start with `in ` or any token doesn't parse — the chrono_english
/// fallback then takes over. Confidence: 0.9. Granularity: Minute when
/// any sub-day unit is present, else Date (with `all_day: false` because
/// the offset preserves the current time-of-day even for day/week
/// units — `in 3 days` from 12:00 → 12:00 three days later).
fn resolve_relative_offset(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let lower = source.to_ascii_lowercase();
    let rest = lower.strip_prefix("in ")?.trim();
    if rest.is_empty() {
        return None;
    }
    let mut tokens: std::str::SplitWhitespace<'_> = rest.split_whitespace();
    let mut total_secs: i64 = 0;
    let mut has_subday = false;
    let mut consumed_pairs = 0usize;
    while let Some(num_tok) = tokens.next() {
        // Allow compact form `30m` / `2h` where number+unit are glued.
        let (n, unit_str): (i64, String) = match split_compact(num_tok) {
            Some((n, u)) => (n, u.to_string()),
            None => {
                let n: i64 = num_tok.parse().ok()?;
                let u = tokens.next()?.to_string();
                (n, u)
            }
        };
        let secs_per = match unit_str.as_str() {
            "s" | "sec" | "secs" | "second" | "seconds" => {
                has_subday = true;
                1
            }
            "m" | "min" | "mins" | "minute" | "minutes" => {
                has_subday = true;
                60
            }
            "h" | "hr" | "hrs" | "hour" | "hours" => {
                has_subday = true;
                3600
            }
            "d" | "day" | "days" => 86_400,
            "w" | "wk" | "wks" | "week" | "weeks" => 7 * 86_400,
            _ => return None,
        };
        total_secs = total_secs.checked_add(n.checked_mul(secs_per)?)?;
        consumed_pairs += 1;
        if consumed_pairs > 4 {
            // Defensive cap — typing more than 4 unit pairs is almost
            // certainly not what the user meant.
            return None;
        }
    }
    if consumed_pairs == 0 {
        return None;
    }
    let target = clock
        .now
        .checked_add_signed(chrono::Duration::seconds(total_secs))?;
    let granularity = if has_subday {
        DateGranularity::Minute
    } else {
        DateGranularity::Date
    };
    Some(ResolvedDate {
        role: DateRole::Inferred,
        source: source.to_string(),
        source_span: (0, source.len()),
        iso: target.to_rfc3339(),
        end_iso: None,
        relative: source.to_string(),
        timezone: clock.timezone_label.clone(),
        all_day: false,
        granularity,
        confidence: 0.9,
    })
}

/// Split a compact token like `30m` / `2h` into `(30, "m")` / `(2, "h")`.
/// Returns None when the token isn't number-then-letters.
fn split_compact(tok: &str) -> Option<(i64, &str)> {
    let split_at = tok.find(|c: char| !c.is_ascii_digit())?;
    if split_at == 0 {
        return None;
    }
    let (num, unit) = tok.split_at(split_at);
    let n: i64 = num.parse().ok()?;
    if unit.is_empty() {
        return None;
    }
    Some((n, unit))
}

/// Parse a time-range phrase like `9-10am`, `2-3pm`, `23:00-01:00` (cross-midnight),
/// or a date-prefixed range like `next mon 9-10am`. Output is a `ResolvedDate`
/// where `iso` is the start instant and `end_iso` carries the end instant. The
/// END half MUST carry a meridiem suffix (`am`/`pm`) OR a colon (24h notation);
/// otherwise the phrase is rejected so a bare `9-10` falls through and ends up
/// `Unresolved`. When start and end resolve to the same day and end < start, the
/// range is treated as cross-midnight and end is bumped one day forward.
fn resolve_time_range(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let dash_idx = source.rfind('-')?;
    let (left, right_with_dash) = source.split_at(dash_idx);
    let right = right_with_dash.strip_prefix('-')?.trim();
    if right.is_empty() {
        return None;
    }
    // Find the trailing time token in `left` — everything after the last
    // whitespace. Anything before is treated as a date prefix.
    let left = left.trim_end();
    // `rsplit_once` is char-boundary-safe: it splits on the whole matched
    // whitespace char, so a multibyte whitespace (e.g. NBSP `\u{00A0}`) cannot
    // land a byte offset mid-char the way `rfind(..)` + `left[i + 1..]` did.
    let (date_prefix, start_tok) = match left.rsplit_once(char::is_whitespace) {
        Some((prefix, tok)) => (prefix.trim(), tok.trim()),
        None => ("", left.trim()),
    };
    if start_tok.is_empty() {
        return None;
    }
    if !looks_like_time(start_tok) {
        return None;
    }
    if !looks_like_time(right) {
        return None;
    }
    // The END half must carry an explicit meridiem OR colon to anchor the
    // whole range — `9-10` (no anchor) is rejected.
    let end_has_meridiem = right.ends_with("am") || right.ends_with("pm");
    let end_has_colon = right.contains(':');
    if !end_has_meridiem && !end_has_colon {
        return None;
    }
    // If start lacks meridiem but end has one, inherit it.
    let start_full = if !start_tok.ends_with("am") && !start_tok.ends_with("pm") && end_has_meridiem
    {
        let suffix = if right.ends_with("am") { "am" } else { "pm" };
        format!("{start_tok}{suffix}")
    } else {
        start_tok.to_string()
    };
    let start_full_with_prefix = if date_prefix.is_empty() {
        start_full
    } else {
        format!("{date_prefix} {start_full}")
    };
    let end_full_with_prefix = if date_prefix.is_empty() {
        right.to_string()
    } else {
        format!("{date_prefix} {right}")
    };
    let mut start_resolved = resolve_date_phrase(&start_full_with_prefix, clock)?;
    let end_resolved = resolve_date_phrase(&end_full_with_prefix, clock)?;
    let start_dt = chrono::DateTime::parse_from_rfc3339(&start_resolved.iso).ok()?;
    let mut end_dt = chrono::DateTime::parse_from_rfc3339(&end_resolved.iso).ok()?;
    // Cross-midnight: end < start → end is the next day.
    if end_dt < start_dt {
        end_dt = end_dt.checked_add_signed(chrono::Duration::days(1))?;
    }
    start_resolved.source = source.to_string();
    start_resolved.relative = source.to_string();
    start_resolved.source_span = (0, source.len());
    start_resolved.end_iso = Some(end_dt.to_rfc3339());
    Some(start_resolved)
}

/// True for tokens that look like a time-of-day half of a range:
/// digits-only (`9`, `23`), digits + meridiem (`10am`, `2pm`), or colon
/// notation (`23:00`, `01:00`, `9:30am`). Rejects anything with extra
/// punctuation or alpha besides am/pm.
fn looks_like_time(tok: &str) -> bool {
    if tok.is_empty() {
        return false;
    }
    let mut chars = tok.chars().peekable();
    let mut saw_digit = false;
    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }
    if !saw_digit {
        return false;
    }
    let rest: String = chars.collect();
    if rest.is_empty() {
        return true;
    }
    if rest == "am" || rest == "pm" {
        return true;
    }
    // colon-separated: must be `:DD` optionally followed by am/pm
    if let Some(after_colon) = rest.strip_prefix(':') {
        let mut digits = 0;
        let bytes = after_colon.as_bytes();
        while digits < bytes.len() && bytes[digits].is_ascii_digit() {
            digits += 1;
        }
        if digits == 0 {
            return false;
        }
        let tail = &after_colon[digits..];
        return tail.is_empty() || tail == "am" || tail == "pm";
    }
    false
}

/// Parse an explicit timezone suffix on a date phrase. Recognized tail tokens
/// (case-insensitive for abbreviations and `Z`/`UTC`):
/// - `Z`, `UTC` → `chrono_tz::UTC`
/// - US abbrevs `PST`/`PDT`, `MST`/`MDT`, `CST`/`CDT`, `EST`/`EDT` → IANA zone
/// - `GMT`/`BST` → `Europe/London`
/// - Any IANA name parseable by `chrono_tz::Tz::from_str` (e.g.
///   `America/Los_Angeles`, `Europe/Berlin`)
/// - Numeric offsets `±HH:MM`, `±HHMM`, `±HH` mapped to `Etc/GMT±N` (the IANA
///   sign convention is inverted, so `-08:00` → `Etc/GMT+8`). Sub-hour offsets
///   are rejected because `Etc/GMT*` zones only carry hour granularity.
///
/// The leading head must itself resolve via [`resolve_date_phrase`] in the
/// override timezone; empty heads or heads that fail to resolve cause this
/// helper to return None so the chrono_english fallback still gets a shot.
fn resolve_timezone_suffix(source: &str, clock: &MenuSyntaxClock) -> Option<ResolvedDate> {
    let trimmed = source.trim();
    let split_idx = trimmed.rfind(char::is_whitespace)?;
    let (head_raw, tail_raw) = trimmed.split_at(split_idx);
    let tail = tail_raw.trim();
    let head = head_raw.trim();
    if head.is_empty() || tail.is_empty() {
        return None;
    }
    let tz = parse_timezone_token(tail)?;
    let new_clock = MenuSyntaxClock {
        now: clock.now.with_timezone(&tz),
        timezone: tz,
        timezone_label: tz.name().to_string(),
        dialect: clock.dialect,
    };
    let mut resolved = resolve_date_phrase(head, &new_clock)?;
    resolved.source = source.to_string();
    resolved.relative = source.to_string();
    resolved.source_span = (0, source.len());
    Some(resolved)
}

fn parse_timezone_token(tok: &str) -> Option<Tz> {
    if tok.eq_ignore_ascii_case("z") || tok.eq_ignore_ascii_case("utc") {
        return Some(chrono_tz::UTC);
    }
    let upper = tok.to_ascii_uppercase();
    let abbrev: Option<Tz> = match upper.as_str() {
        "PST" | "PDT" => "America/Los_Angeles".parse().ok(),
        "MST" | "MDT" => "America/Denver".parse().ok(),
        "CST" | "CDT" => "America/Chicago".parse().ok(),
        "EST" | "EDT" => "America/New_York".parse().ok(),
        "GMT" | "BST" => "Europe/London".parse().ok(),
        _ => None,
    };
    if abbrev.is_some() {
        return abbrev;
    }
    if let Ok(tz) = tok.parse::<Tz>() {
        return Some(tz);
    }
    parse_numeric_offset_to_etc(tok)
}

fn parse_numeric_offset_to_etc(tok: &str) -> Option<Tz> {
    let bytes = tok.as_bytes();
    if bytes.len() < 2 {
        return None;
    }
    let (sign, rest): (i32, &str) = match bytes[0] {
        b'+' => (1, &tok[1..]),
        b'-' => (-1, &tok[1..]),
        _ => return None,
    };
    // A numeric timezone offset is ASCII digits (optionally with `:`). Rejecting
    // non-ASCII here prevents the `rest.len() == 4` byte-length branch below from
    // slicing `&rest[..2]` inside a multibyte char (e.g. `aéb`) and panicking.
    if !rest.is_ascii() {
        return None;
    }
    let (h_str, m_str) = if let Some(idx) = rest.find(':') {
        (&rest[..idx], &rest[idx + 1..])
    } else if rest.len() == 4 {
        (&rest[..2], &rest[2..])
    } else if rest.len() == 2 || rest.len() == 1 {
        (rest, "00")
    } else {
        return None;
    };
    let h: i32 = h_str.parse().ok()?;
    let m: u32 = m_str.parse().ok()?;
    if h > 14 || m >= 60 {
        return None;
    }
    if m != 0 {
        return None;
    }
    let inverted = -sign * h;
    let name = if inverted == 0 {
        "Etc/GMT".to_string()
    } else if inverted > 0 {
        format!("Etc/GMT+{}", inverted)
    } else {
        format!("Etc/GMT-{}", inverted.abs())
    };
    name.parse().ok()
}

/// Variant of [`resolve_date_phrase`] that returns a [`DateParseResult`] so
/// callers can distinguish empty input, successful resolution, and a recognized
/// but unparsable date slot. Used by Story W2 follow-ups; today the body simply
/// delegates to `resolve_date_phrase` and tags the result with the supplied
/// role and span. A future story replaces the delegated body with the new
/// natural-language parser.
pub fn parse_date_phrase_result(
    raw: &str,
    source_span: (usize, usize),
    role: DateRole,
    clock: &MenuSyntaxClock,
) -> DateParseResult {
    let normalized = normalize_smart_quotes(raw);
    let source = normalized.trim();
    if source.is_empty() {
        return DateParseResult::Empty;
    }
    match resolve_date_phrase(raw, clock) {
        Some(mut resolved) => {
            resolved.role = role;
            resolved.source_span = source_span;
            DateParseResult::Resolved(resolved)
        }
        None => DateParseResult::Unresolved(UnresolvedDate {
            role,
            source: source.to_string(),
            source_span,
        }),
    }
}

pub fn resolve_capture_dates(
    invocation: &CaptureInvocation,
    clock: &MenuSyntaxClock,
) -> ResolvedCaptureInvocation {
    resolve_capture_dates_with_accepts(invocation, clock, &[])
}

pub(crate) fn builtin_capture_accepts_for_target(target: &str) -> Vec<String> {
    let tokens: &[&str] =
        if target.eq_ignore_ascii_case("cal") || target.eq_ignore_ascii_case("mcal") {
            &[
                "tags",
                "date",
                "dateRange",
                "duration",
                "recurrence",
                "daily",
                "multiWeekday",
                "monthly",
                "yearly",
                "kv",
            ]
        } else if target.eq_ignore_ascii_case("todo")
            || target.eq_ignore_ascii_case("reminder")
            || target.eq_ignore_ascii_case("snooze")
            || target.eq_ignore_ascii_case("defer")
        {
            &[
                "tags",
                "date",
                "relativeDate",
                "recurrence",
                "daily",
                "multiWeekday",
                "priority",
                "url",
            ]
        } else {
            &[]
        };
    tokens.iter().map(|token| token.to_string()).collect()
}

pub fn resolve_capture_dates_with_accepts(
    invocation: &CaptureInvocation,
    clock: &MenuSyntaxClock,
    accepts: &[String],
) -> ResolvedCaptureInvocation {
    let mut dates: Vec<ResolvedDate> = Vec::new();
    let mut unresolved_dates: Vec<UnresolvedDate> = Vec::new();
    let mut resolved_explicit = false;

    for phrase in &invocation.date_phrases {
        if let Some(mut resolved) = resolve_date_phrase(&phrase.source, clock) {
            resolved.role = phrase.role.clone();
            resolved.source_span = phrase.source_span;
            resolved.end_iso = None;
            dates.push(resolved);
            resolved_explicit = true;
        } else {
            let trimmed = phrase.source.trim();
            if !trimmed.is_empty() {
                unresolved_dates.push(UnresolvedDate {
                    role: phrase.role.clone(),
                    source: trimmed.to_string(),
                    source_span: phrase.source_span,
                });
            }
        }
    }

    let mut body = invocation.body.clone();
    let mut duration = invocation.duration.clone();
    let mut duration_resolved = None;
    let mut recurrence = None;
    if !resolved_explicit {
        let accepts_nl = accepts.iter().any(|a| {
            matches!(
                a.as_str(),
                "date"
                    | "dateRange"
                    | "duration"
                    | "recurrence"
                    | "relativeDate"
                    | "daily"
                    | "multiWeekday"
                    | "monthly"
                    | "yearly"
            )
        }) || invocation.target.eq_ignore_ascii_case("cal")
            || invocation.target.eq_ignore_ascii_case("mcal");
        if accepts_nl {
            let nl = crate::menu_syntax::nl_phrase::resolve_capture_nl_phrase_with_accepts(
                invocation, clock, accepts,
            );
            if !nl.fragments.is_empty() {
                if let Some(mut nl_date) = nl.date {
                    if nl_date.role == DateRole::Inferred {
                        nl_date.role = DateRole::Start;
                    }
                    dates.push(nl_date);
                }
                if let Some(nl_duration) = nl.duration {
                    duration = Some(nl_duration.source.clone());
                    duration_resolved = Some(nl_duration);
                }
                recurrence = nl.recurrence;
                body = nl.subject;
            } else if let Some((suffix, trimmed)) = infer_body_suffix_date(&invocation.body, clock)
            {
                dates.push(suffix);
                body = trimmed;
            }
        } else if let Some((suffix, trimmed)) = infer_body_suffix_date(&invocation.body, clock) {
            dates.push(suffix);
            body = trimmed;
        }
    }

    ResolvedCaptureInvocation {
        target: invocation.target.clone(),
        body,
        tags: invocation.tags.clone(),
        priority: invocation.priority,
        url: invocation.url.clone(),
        duration,
        duration_resolved,
        recurrence,
        kv: invocation.kv.clone(),
        dates,
        unresolved_dates,
        raw: invocation.raw.clone(),
    }
}

fn infer_body_suffix_date(body: &str, clock: &MenuSyntaxClock) -> Option<(ResolvedDate, String)> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let max_window = tokens.len().min(6);
    for window in (1..=max_window).rev() {
        let start_idx = tokens.len() - window;
        let candidate = tokens[start_idx..].join(" ");
        if candidate.len() > 64 {
            continue;
        }
        if let Some(mut resolved) = resolve_date_phrase(&candidate, clock) {
            resolved.role = DateRole::Inferred;
            let prefix: String = tokens[..start_idx].join(" ");
            return Some((resolved, prefix));
        }
    }
    None
}

fn infer_granularity(raw: &str) -> DateGranularity {
    let lower = raw.to_ascii_lowercase();
    if lower.contains("am")
        || lower.contains("pm")
        || lower.contains("noon")
        || lower.contains("midnight")
        || lower.chars().any(|c| c == ':')
    {
        DateGranularity::Minute
    } else {
        DateGranularity::Date
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu_syntax::capture::{parse_capture, CaptureParse};
    use chrono_tz::America::Denver;

    fn denver(now: &str) -> MenuSyntaxClock {
        MenuSyntaxClock::fixed(now, Denver).expect("fixed clock")
    }

    fn parse_ok(input: &str) -> CaptureInvocation {
        match parse_capture(input) {
            CaptureParse::Ok(inv) => inv,
            CaptureParse::Incomplete(s) => panic!("expected ok, got {s:?}"),
        }
    }

    // -------- Smart-quote normalization (Run 11 Pass 43) --------

    #[test]
    fn normalize_smart_quotes_passthrough_when_no_curly() {
        let input = "today 5pm";
        let out = normalize_smart_quotes(input);
        assert!(matches!(out, std::borrow::Cow::Borrowed(_)));
        assert_eq!(&*out, input);
    }

    #[test]
    fn normalize_smart_quotes_curly_double_to_straight() {
        let out = normalize_smart_quotes("\u{201C}tomorrow\u{201D}");
        assert_eq!(&*out, "\"tomorrow\"");
    }

    #[test]
    fn normalize_smart_quotes_curly_single_to_apostrophe() {
        let out = normalize_smart_quotes("o\u{2019}clock");
        assert_eq!(&*out, "o'clock");
    }

    #[test]
    fn normalize_smart_quotes_guillemets_to_double() {
        let out = normalize_smart_quotes("\u{00AB}noon\u{00BB}");
        assert_eq!(&*out, "\"noon\"");
    }

    #[test]
    fn normalize_smart_quotes_only_quote_chars_change() {
        // Falsifier: only quote chars are touched — every other byte passes
        // through unchanged. If a future edit accidentally rewrites another
        // char (e.g. a dash), this test catches it.
        let input = "café — \u{201C}noon\u{201D} 5/4 +café";
        let expected = "café — \"noon\" 5/4 +café";
        assert_eq!(&*normalize_smart_quotes(input), expected);
    }

    #[test]
    fn resolve_date_phrase_normalizes_smart_quotes_in_input() {
        // Falsifier: the source field of an Unresolved must show the
        // NORMALIZED form (`"today"`), proving normalization ran. If the
        // smart-quote pre-pass were skipped, source would be the raw curly
        // version `\u{201C}today\u{201D}`.
        let clock = denver("2026-04-23T12:00:00");
        let r =
            parse_date_phrase_result("\u{201C}today\u{201D}", (0, 0), DateRole::Inferred, &clock);
        match r {
            DateParseResult::Unresolved(u) => {
                assert_eq!(u.source, "\"today\"", "expected normalized source");
            }
            DateParseResult::Resolved(r) => {
                // chrono_english may resolve `"today"` — accept that, but
                // verify the source still shows the normalized form.
                assert_eq!(r.source, "\"today\"", "expected normalized source");
            }
            DateParseResult::Empty => panic!("non-empty input should not be Empty"),
        }
    }

    #[test]
    fn resolves_tomorrow_3pm_with_fixed_clock() {
        let clock = denver("2026-04-23T12:00:00");
        let resolved = resolve_date_phrase("tomorrow 3pm", &clock).expect("resolved");
        assert!(resolved.iso.starts_with("2026-04-24T15:00:00"));
        assert_eq!(resolved.granularity, DateGranularity::Minute);
        assert!(!resolved.all_day);
    }

    #[test]
    fn infers_suffix_date_and_trims_body() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Renew passport tomorrow 3pm #errands p1");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert_eq!(resolved.body, "Renew passport");
        assert_eq!(resolved.dates.len(), 1);
        assert!(resolved.dates[0].iso.starts_with("2026-04-24T15:00:00"));
        assert_eq!(resolved.tags, vec!["errands".to_string()]);
        assert_eq!(resolved.priority, Some(1));
    }

    #[test]
    fn explicit_due_wins_over_suffix_inference() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Renew passport tomorrow 3pm due:friday #errands");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert_eq!(resolved.dates.len(), 1, "got {:?}", resolved.dates);
        assert_eq!(resolved.dates[0].role, DateRole::Due);
        assert_eq!(resolved.dates[0].source, "friday");
        assert!(resolved.body.contains("tomorrow 3pm"));
    }

    #[test]
    fn spring_forward_tomorrow_3pm_uses_post_dst_offset() {
        let clock = denver("2026-03-07T12:00:00");
        let resolved = resolve_date_phrase("tomorrow 3pm", &clock).expect("resolved");
        assert!(resolved.iso.starts_with("2026-03-08T15:00:00-06:00"));
    }

    #[test]
    fn fall_back_tomorrow_3pm_uses_post_dst_offset() {
        let clock = denver("2026-10-31T12:00:00");
        let resolved = resolve_date_phrase("tomorrow 3pm", &clock).expect("resolved");
        assert!(resolved.iso.starts_with("2026-11-01T15:00:00-07:00"));
    }

    #[test]
    fn empty_phrase_yields_none() {
        let clock = denver("2026-04-23T12:00:00");
        assert!(resolve_date_phrase("", &clock).is_none());
        assert!(resolve_date_phrase("   ", &clock).is_none());
    }

    #[test]
    fn non_date_suffix_leaves_body_intact() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";note some random idea about parsers #thoughts");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert_eq!(resolved.body, "some random idea about parsers");
        assert!(resolved.dates.is_empty());
    }

    #[test]
    fn parse_date_phrase_result_empty_returns_empty_variant() {
        let clock = denver("2026-04-23T12:00:00");
        let result = parse_date_phrase_result("", (0, 0), DateRole::Start, &clock);
        assert_eq!(result, DateParseResult::Empty);
        let result = parse_date_phrase_result("   ", (0, 3), DateRole::Due, &clock);
        assert_eq!(result, DateParseResult::Empty);
    }

    #[test]
    fn parse_date_phrase_result_resolved_carries_role_and_span() {
        let clock = denver("2026-04-23T12:00:00");
        let result = parse_date_phrase_result("tomorrow 3pm", (5, 17), DateRole::Start, &clock);
        match result {
            DateParseResult::Resolved(r) => {
                assert_eq!(r.role, DateRole::Start);
                assert_eq!(r.source_span, (5, 17));
                assert!(r.iso.starts_with("2026-04-24T15:00:00"));
                assert_eq!(r.end_iso, None);
            }
            other => panic!("expected Resolved, got {other:?}"),
        }
    }

    #[test]
    fn parse_date_phrase_result_unresolved_returns_unresolved_variant() {
        let clock = denver("2026-04-23T12:00:00");
        let result = parse_date_phrase_result("asdfasdf", (4, 12), DateRole::Due, &clock);
        match result {
            DateParseResult::Unresolved(u) => {
                assert_eq!(u.role, DateRole::Due);
                assert_eq!(u.source, "asdfasdf");
                assert_eq!(u.source_span, (4, 12));
            }
            other => panic!("expected Unresolved, got {other:?}"),
        }
    }

    #[test]
    fn resolved_date_serializes_without_end_iso_when_none() {
        let clock = denver("2026-04-23T12:00:00");
        let resolved = resolve_date_phrase("tomorrow 3pm", &clock).expect("resolved");
        let json = serde_json::to_string(&resolved).expect("serde");
        assert!(
            !json.contains("endIso"),
            "endIso should be omitted when None for backward-compat: {json}"
        );
        assert!(json.contains("\"iso\":"));
    }

    #[test]
    fn resolved_date_serializes_with_end_iso_when_some() {
        let clock = denver("2026-04-23T12:00:00");
        let mut resolved = resolve_date_phrase("tomorrow 3pm", &clock).expect("resolved");
        resolved.end_iso = Some("2026-04-24T16:00:00-06:00".to_string());
        let json = serde_json::to_string(&resolved).expect("serde");
        assert!(json.contains("\"endIso\":\"2026-04-24T16:00:00-06:00\""));
    }

    #[test]
    fn resolved_date_round_trips_with_end_iso() {
        let resolved = ResolvedDate {
            role: DateRole::Start,
            source: "next mon 9-10am".to_string(),
            source_span: (0, 14),
            iso: "2026-04-27T09:00:00-06:00".to_string(),
            end_iso: Some("2026-04-27T10:00:00-06:00".to_string()),
            relative: "next mon 9-10am".to_string(),
            timezone: "America/Denver".to_string(),
            all_day: false,
            granularity: DateGranularity::Minute,
            confidence: 0.85,
        };
        let json = serde_json::to_string(&resolved).expect("serde");
        let restored: ResolvedDate = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored, resolved);
    }

    #[test]
    fn resolved_date_deserializes_without_end_iso_field() {
        // Old-format payloads (pre-W2) have no endIso field; default must be None.
        let legacy = r#"{
            "role":"due",
            "source":"friday",
            "sourceSpan":[0,6],
            "iso":"2026-05-01T00:00:00-06:00",
            "relative":"friday",
            "timezone":"America/Denver",
            "allDay":true,
            "granularity":"date",
            "confidence":0.9
        }"#;
        let parsed: ResolvedDate = serde_json::from_str(legacy).expect("legacy parses");
        assert_eq!(parsed.end_iso, None);
        assert_eq!(parsed.role, DateRole::Due);
    }

    #[test]
    fn unresolved_date_serializes_camel_case() {
        let u = UnresolvedDate {
            role: DateRole::At,
            source: "asdf".to_string(),
            source_span: (3, 7),
        };
        let json = serde_json::to_string(&u).expect("serde");
        assert!(json.contains("\"role\":\"at\""));
        assert!(json.contains("\"source\":\"asdf\""));
        assert!(json.contains("\"sourceSpan\":[3,7]"));
    }

    // ── resolve_capture_dates: unresolved-key reporting ─────────────────────

    #[test]
    fn capture_due_with_garbage_phrase_reports_unresolved_due_key() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Thing due:asdf");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert!(resolved.dates.is_empty(), "no resolved dates expected");
        assert_eq!(resolved.unresolved_dates.len(), 1);
        assert_eq!(resolved.unresolved_dates[0].role, DateRole::Due);
        assert_eq!(resolved.unresolved_dates[0].source, "asdf");
        // Body MUST be preserved — the bad date doesn't poison body extraction.
        assert_eq!(resolved.body, "Thing");
    }

    #[test]
    fn capture_mixed_resolved_and_unresolved_dates_each_reported_once() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Plan due:tomorrow start:zzzz end:later");
        let resolved = resolve_capture_dates(&invocation, &clock);
        // `tomorrow` resolves; `zzzz` and `later` do not.
        assert_eq!(resolved.dates.len(), 1, "got dates {:?}", resolved.dates);
        assert_eq!(resolved.dates[0].role, DateRole::Due);
        let unresolved_roles: Vec<DateRole> = resolved
            .unresolved_dates
            .iter()
            .map(|u| u.role.clone())
            .collect();
        assert!(unresolved_roles.contains(&DateRole::Start));
        assert!(unresolved_roles.contains(&DateRole::End));
        assert_eq!(resolved.unresolved_dates.len(), 2);
    }

    #[test]
    fn capture_all_resolved_leaves_unresolved_empty() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Renew passport tomorrow 3pm");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert_eq!(resolved.dates.len(), 1);
        assert!(
            resolved.unresolved_dates.is_empty(),
            "no unresolved expected, got {:?}",
            resolved.unresolved_dates
        );
    }

    #[test]
    fn capture_no_dates_at_all_leaves_both_lists_empty() {
        // Falsifier for over-collection: a capture with no date keys and no
        // suffix-inferred date must produce both `dates` and `unresolved_dates` empty.
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";note random idea about parsers");
        let resolved = resolve_capture_dates(&invocation, &clock);
        assert!(resolved.dates.is_empty());
        assert!(resolved.unresolved_dates.is_empty());
    }

    #[test]
    fn resolved_capture_invocation_serializes_unresolved_dates_camel_case() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Thing due:asdf");
        let resolved = resolve_capture_dates(&invocation, &clock);
        let json = serde_json::to_string(&resolved).unwrap();
        // Field name must be camelCase for the snapshot/UI consumer.
        assert!(
            json.contains("\"unresolvedDates\":["),
            "missing camelCase key in {json}"
        );
        assert!(json.contains("\"role\":\"due\""));
        assert!(json.contains("\"source\":\"asdf\""));
    }

    #[test]
    fn resolved_capture_invocation_omits_new_fields_when_none() {
        let clock = denver("2026-04-23T12:00:00");
        let invocation = parse_ok(";todo Thing");
        let resolved = resolve_capture_dates(&invocation, &clock);
        let json = serde_json::to_string(&resolved).unwrap();
        assert!(!json.contains("durationResolved"), "{json}");
        assert!(!json.contains("recurrence"), "{json}");
    }

    #[test]
    fn resolved_capture_invocation_serializes_duration_resolved_camel_case() {
        let duration = ResolvedDuration {
            source: "30mins".to_string(),
            source_span: (10, 16),
            seconds: 1800,
            minutes: 30,
            iso8601: "PT30M".to_string(),
        };
        let json = serde_json::to_string(&duration).unwrap();
        assert!(json.contains("\"sourceSpan\":[10,16]"));
        assert!(json.contains("\"iso8601\":\"PT30M\""));
    }

    #[test]
    fn resolved_capture_invocation_serializes_recurrence_camel_case() {
        let recurrence = ResolvedRecurrence {
            source: "every mon".to_string(),
            source_span: (5, 14),
            frequency: RecurrenceFrequency::Weekly,
            weekdays: vec![RecurrenceWeekday::Mon],
            rrule: "FREQ=WEEKLY;BYDAY=MO".to_string(),
            label: "every Monday".to_string(),
        };
        let json = serde_json::to_string(&recurrence).unwrap();
        assert!(json.contains("\"sourceSpan\":[5,14]"));
        assert!(json.contains("\"frequency\":\"weekly\""));
        assert!(json.contains("\"weekdays\":[\"mon\"]"));
    }

    #[test]
    fn recurrence_frequency_serializes_daily_monthly_yearly_lowercase() {
        assert_eq!(
            serde_json::to_string(&RecurrenceFrequency::Daily).unwrap(),
            "\"daily\""
        );
        assert_eq!(
            serde_json::to_string(&RecurrenceFrequency::Monthly).unwrap(),
            "\"monthly\""
        );
        assert_eq!(
            serde_json::to_string(&RecurrenceFrequency::Yearly).unwrap(),
            "\"yearly\""
        );
    }

    #[test]
    fn resolved_recurrence_omits_empty_weekdays_for_daily() {
        let recurrence = ResolvedRecurrence {
            source: "every day".to_string(),
            source_span: (0, 9),
            frequency: RecurrenceFrequency::Daily,
            weekdays: vec![],
            rrule: "FREQ=DAILY".to_string(),
            label: "every day".to_string(),
        };
        let json = serde_json::to_string(&recurrence).unwrap();
        assert!(!json.contains("weekdays"), "{json}");
        assert!(json.contains("\"frequency\":\"daily\""), "{json}");
    }

    #[test]
    fn resolve_capture_dates_with_accepts_mcal_til_sets_end_iso() {
        let clock = denver("2026-04-26T09:00:00");
        let invocation = parse_ok(";mcal Lunch with Ryan tomorrow at 12pm til 1pm");
        let resolved = resolve_capture_dates_with_accepts(&invocation, &clock, &[]);
        assert_eq!(resolved.body, "Lunch with Ryan");
        assert_eq!(resolved.dates[0].iso, "2026-04-27T12:00:00-06:00");
        assert_eq!(
            resolved.dates[0].end_iso.as_deref(),
            Some("2026-04-27T13:00:00-06:00")
        );
    }

    #[test]
    fn resolve_capture_dates_with_accepts_mcal_for_duration_sets_duration_resolved() {
        let clock = denver("2026-04-26T09:00:00");
        let invocation = parse_ok(";mcal Lunch with Ryan tom 12pm for 30mins");
        let resolved = resolve_capture_dates_with_accepts(&invocation, &clock, &[]);
        assert_eq!(resolved.duration.as_deref(), Some("30mins"));
        assert_eq!(resolved.duration_resolved.unwrap().minutes, 30);
        assert_eq!(
            resolved.dates[0].end_iso.as_deref(),
            Some("2026-04-27T12:30:00-06:00")
        );
    }

    #[test]
    fn resolve_capture_dates_with_accepts_mcal_every_mon_sets_recurrence() {
        let clock = denver("2026-04-26T09:00:00");
        let invocation = parse_ok(";mcal Lunch w/ Ryan every mon from 1 til 2");
        let resolved = resolve_capture_dates_with_accepts(&invocation, &clock, &[]);
        assert_eq!(resolved.body, "Lunch w/ Ryan");
        assert_eq!(
            resolved.recurrence.as_ref().map(|r| r.rrule.as_str()),
            Some("FREQ=WEEKLY;BYDAY=MO")
        );
    }

    #[test]
    fn resolve_capture_dates_without_accepts_preserves_legacy_suffix_behavior() {
        let clock = denver("2026-04-26T09:00:00");
        let invocation = parse_ok(";todo Lunch with Ryan tomorrow at 12pm til 1pm");
        let resolved = resolve_capture_dates_with_accepts(&invocation, &clock, &[]);
        assert_ne!(resolved.body, "Lunch with Ryan");
        assert!(resolved.duration_resolved.is_none());
        assert!(resolved.recurrence.is_none());
    }

    #[test]
    fn granularity_inference_picks_minute_for_times() {
        assert_eq!(infer_granularity("tomorrow 3pm"), DateGranularity::Minute);
        assert_eq!(infer_granularity("next Friday"), DateGranularity::Date);
        assert_eq!(infer_granularity("noon"), DateGranularity::Minute);
        assert_eq!(infer_granularity("15:30 tomorrow"), DateGranularity::Minute);
    }

    // -------- Multibyte-input panic regressions (chaos-monkey) --------

    #[test]
    fn resolve_time_range_survives_multibyte_whitespace_before_range() {
        // Regression: `left.rfind(char::is_whitespace)` returns the byte index of
        // a multibyte whitespace char (e.g. NBSP U+00A0 = bytes C2 A0); slicing
        // `left[i + 1..]` then landed inside the continuation byte and panicked
        // with "byte index is not a char boundary". Must not panic.
        let clock = denver("2026-07-17T12:00:00");
        // NBSP between `x` and the `9-10am` range.
        let _ = resolve_time_range("x\u{00A0}9-10am", &clock);
        // The public dispatch reaches this resolver before the ASCII guard, so a
        // user typing a date phrase with a pasted NBSP must not crash the app.
        let _ = resolve_date_phrase("x\u{00A0}9-10am", &clock);
    }

    #[test]
    fn parse_numeric_offset_rejects_non_ascii_four_byte_remainder() {
        // Regression: `rest.len() == 4` is a *byte* check; `&rest[..2]` / `&rest[2..]`
        // sliced mid-char for a 4-byte non-ASCII remainder like `aéb`
        // (61 C3 A9 62), where byte 2 is a UTF-8 continuation byte. Must return
        // None, not panic.
        assert_eq!(parse_numeric_offset_to_etc("+aéb"), None);
        assert_eq!(parse_numeric_offset_to_etc("-aéb"), None);
        // Reached through the public dispatch as a `<phrase> <tz>` suffix.
        let clock = denver("2026-07-17T12:00:00");
        let _ = resolve_date_phrase("tomorrow +aéb", &clock);
    }
}
