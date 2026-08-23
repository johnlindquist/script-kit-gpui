use std::cmp::Ordering;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};

const CHROMIUM_EPOCH_OFFSET_SECS: i64 = 11_644_473_600;
const SAFARI_EPOCH_OFFSET_SECS: i64 = 978_307_200;
const PER_BROWSER_LIMIT: usize = 250;
const HISTORY_CACHE_TTL: Duration = Duration::from_secs(30);

static HISTORY_LOAD_CACHE: LazyLock<Mutex<Option<BrowserHistoryLoadCache>>> =
    LazyLock::new(|| Mutex::new(None));
static HISTORY_SEARCH_CACHE: LazyLock<Mutex<Option<BrowserHistorySearchCache>>> =
    LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Clone)]
struct BrowserHistoryLoadCache {
    loaded_at: Instant,
    requested_limit: usize,
    entries: Vec<BrowserHistoryEntry>,
}

#[derive(Debug, Clone)]
struct BrowserHistorySearchCache {
    entries_ptr: usize,
    entries_len: usize,
    query: String,
    matches: Vec<BrowserHistoryMatch>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserHistoryFamily {
    Safari,
    Chromium,
    Firefox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedBrowserHistory {
    app_name: &'static str,
    bundle_id: &'static str,
    family: BrowserHistoryFamily,
    profile_root: &'static str,
}

const SUPPORTED_BROWSERS: &[SupportedBrowserHistory] = &[
    SupportedBrowserHistory {
        app_name: "Safari",
        bundle_id: "com.apple.Safari",
        family: BrowserHistoryFamily::Safari,
        profile_root: "Library/Safari",
    },
    SupportedBrowserHistory {
        app_name: "Google Chrome",
        bundle_id: "com.google.Chrome",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/Google/Chrome",
    },
    SupportedBrowserHistory {
        app_name: "Arc",
        bundle_id: "company.thebrowser.Browser",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/Arc/User Data",
    },
    SupportedBrowserHistory {
        app_name: "Brave Browser",
        bundle_id: "com.brave.Browser",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/BraveSoftware/Brave-Browser",
    },
    SupportedBrowserHistory {
        app_name: "Microsoft Edge",
        bundle_id: "com.microsoft.edgemac",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/Microsoft Edge",
    },
    SupportedBrowserHistory {
        app_name: "Chromium",
        bundle_id: "org.chromium.Chromium",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/Chromium",
    },
    SupportedBrowserHistory {
        app_name: "Vivaldi",
        bundle_id: "com.vivaldi.Vivaldi",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/Vivaldi",
    },
    SupportedBrowserHistory {
        app_name: "Opera",
        bundle_id: "com.operasoftware.Opera",
        family: BrowserHistoryFamily::Chromium,
        profile_root: "Library/Application Support/com.operasoftware.Opera",
    },
    SupportedBrowserHistory {
        app_name: "Firefox",
        bundle_id: "org.mozilla.firefox",
        family: BrowserHistoryFamily::Firefox,
        profile_root: "Library/Application Support/Firefox/Profiles",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserHistoryEntry {
    pub browser_name: Arc<str>,
    pub browser_bundle_id: Arc<str>,
    pub title: Arc<str>,
    pub url: Arc<str>,
    pub host: Arc<str>,
    pub last_visited_at_ms: i64,
    pub visit_count: i64,
    pub profile: Arc<str>,
}

impl BrowserHistoryEntry {
    pub fn display_title(&self) -> &str {
        let trimmed = self.title.trim();
        if !trimmed.is_empty() {
            trimmed
        } else if !self.host.trim().is_empty() {
            self.host.trim()
        } else if !self.url.trim().is_empty() {
            self.url.trim()
        } else {
            "Untitled History Entry"
        }
    }

    pub fn history_key(&self) -> String {
        format!("{}:{}:{}", self.browser_bundle_id, self.profile, self.url)
    }

    #[allow(dead_code)]
    pub(crate) fn convert_to_root_hit(&self) -> RootBrowserHistorySearchHit {
        RootBrowserHistorySearchHit {
            stable_key: self.history_key(),
            provider_label: self.browser_name.to_string(),
            profile_label: self.profile.to_string(),
            title: self.title.to_string(),
            url: self.url.to_string(),
            domain: self.host.to_string(),
            last_visit_unix_ms: self.last_visited_at_ms,
            visit_count: self.visit_count,
        }
    }
}

pub fn format_browser_history_meta(entry: &BrowserHistoryEntry) -> String {
    format!(
        "{} · {} visits · {}",
        entry.browser_name,
        entry.visit_count,
        format_history_timestamp(entry.last_visited_at_ms)
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserHistoryMatch {
    pub entry: BrowserHistoryEntry,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootBrowserHistorySectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
    pub max_age_days: u32,
    pub providers: Vec<crate::config::BrowserHistoryProvider>,
    pub search_urls: bool,
    pub scan_limit: usize,
    pub cache_ttl_ms: u64,
}

impl Default for RootBrowserHistorySectionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_results: 3,
            min_query_chars: 4,
            max_age_days: 90,
            providers: crate::config::BrowserHistoryProvider::default_root_providers(),
            search_urls: true,
            scan_limit: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_HISTORY_SCAN_LIMIT,
            cache_ttl_ms:
                crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_HISTORY_CACHE_TTL_MS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RootPassiveSnapshotStatus {
    pub generation: u64,
    pub refreshing: bool,
    pub cached_count: usize,
    pub last_refreshed_at: Option<Instant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RootBrowserHistorySearchHit {
    pub stable_key: String,
    pub provider_label: String,
    pub profile_label: String,
    pub title: String,
    pub url: String,
    pub domain: String,
    pub last_visit_unix_ms: i64,
    pub visit_count: i64,
}

#[derive(Debug, Clone)]
#[allow(dead_code)] // Root unified search calls this through the binary app layer.
struct RootBrowserHistorySnapshot {
    captured_at: Instant,
    hits: Arc<Vec<RootBrowserHistorySearchHit>>,
}

#[derive(Debug, Default)]
#[allow(dead_code)] // Root unified search calls this through the binary app layer.
struct RootBrowserHistorySnapshotState {
    snapshot: Option<RootBrowserHistorySnapshot>,
    refresh_in_flight: bool,
    generation: u64,
    last_refresh_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootBrowserHistoryRefresh {
    generation: u64,
    started_at: Instant,
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
static ROOT_BROWSER_HISTORY_SNAPSHOT: LazyLock<Mutex<RootBrowserHistorySnapshotState>> =
    LazyLock::new(|| Mutex::new(RootBrowserHistorySnapshotState::default()));

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // Root unified search calls this through the binary app layer.
struct RootBrowserHistoryProviderSpec {
    provider: crate::config::BrowserHistoryProvider,
    provider_label: &'static str,
    profile_root: &'static str,
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
const ROOT_BROWSER_HISTORY_PROVIDERS: &[RootBrowserHistoryProviderSpec] = &[
    RootBrowserHistoryProviderSpec {
        provider: crate::config::BrowserHistoryProvider::Arc,
        provider_label: "Arc",
        profile_root: "Library/Application Support/Arc/User Data",
    },
    RootBrowserHistoryProviderSpec {
        provider: crate::config::BrowserHistoryProvider::Chrome,
        provider_label: "Chrome",
        profile_root: "Library/Application Support/Google/Chrome",
    },
    RootBrowserHistoryProviderSpec {
        provider: crate::config::BrowserHistoryProvider::Brave,
        provider_label: "Brave",
        profile_root: "Library/Application Support/BraveSoftware/Brave-Browser",
    },
    RootBrowserHistoryProviderSpec {
        provider: crate::config::BrowserHistoryProvider::Edge,
        provider_label: "Edge",
        profile_root: "Library/Application Support/Microsoft Edge",
    },
];

pub(crate) fn root_browser_history_query_is_eligible(
    query: &str,
    options: RootBrowserHistorySectionOptions,
) -> bool {
    let trimmed = query.trim();
    options.enabled
        && crate::scripts::search::query_meets_min_query_chars(trimmed, options.min_query_chars)
        && !trimmed.contains('\n')
        && !trimmed.contains('\r')
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn search_root_browser_history_meta(
    query: &str,
    options: RootBrowserHistorySectionOptions,
) -> Vec<RootBrowserHistorySearchHit> {
    if !root_browser_history_query_is_eligible(query, options.clone()) {
        return Vec::new();
    }

    let home = match std::env::var_os("HOME").map(PathBuf::from) {
        Some(home) => home,
        None => return Vec::new(),
    };

    search_root_browser_history_meta_from_home(&home, query, options)
}

#[allow(dead_code)]
/// Nonblocking snapshot-only lookup for the implicit (passive) typing path.
/// Mirrors `search_root_browser_tabs_meta_cached`: no refresh spawn, no env
/// access — one mutex lock plus a bounded fuzzy scan over the cached
/// snapshot (stale snapshots are still served). Freshness is the app
/// layer's job (`maybe_start_root_browser_history_refresh_for_query`).
pub(crate) fn search_root_browser_history_meta_cached(
    query: &str,
    options: RootBrowserHistorySectionOptions,
) -> Vec<RootBrowserHistorySearchHit> {
    if !root_browser_history_query_is_eligible(query, options.clone()) {
        return Vec::new();
    }

    let candidates = cached_root_browser_history_snapshot(options.cache_ttl_ms);
    root_fuzzy_search_browser_history_hits(&candidates, query, options.search_urls)
        .into_iter()
        .take(options.max_results)
        .collect()
}

pub(crate) fn search_root_browser_history_meta_direct(
    query: &str,
    options: RootBrowserHistorySectionOptions,
) -> Vec<RootBrowserHistorySearchHit> {
    // Direct means explicit matching, not ownership of a background provider.
    // A cold composer honestly returns no rows until an app-owned, query-fenced
    // refresh publishes a snapshot; detached refreshes cannot prove live intent.
    search_root_browser_history_meta_cached(query, options)
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn search_root_browser_history_meta_from_home(
    home: &Path,
    query: &str,
    options: RootBrowserHistorySectionOptions,
) -> Vec<RootBrowserHistorySearchHit> {
    ensure_root_browser_history_refresh(home.to_path_buf(), options.clone(), "root_search_query");

    let selected_provider_labels: HashSet<String> = options
        .providers
        .iter()
        .filter_map(|provider| root_browser_history_provider_label(*provider))
        .map(|s| s.to_string())
        .collect();
    let cutoff_unix_ms = Utc::now()
        .timestamp_millis()
        .saturating_sub(i64::from(options.max_age_days).saturating_mul(24 * 60 * 60 * 1000));
    let candidates = cached_root_browser_history_snapshot(options.cache_ttl_ms);
    let candidates = candidates
        .iter()
        .filter(|hit| selected_provider_labels.contains(hit.provider_label.as_str()))
        .filter(|hit| hit.last_visit_unix_ms >= cutoff_unix_ms)
        .take(options.scan_limit)
        .cloned()
        .collect::<Vec<_>>();

    root_fuzzy_search_browser_history_hits(&candidates, query, options.search_urls)
        .into_iter()
        .take(options.max_results)
        .collect()
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn cached_root_browser_history_snapshot(
    cache_ttl_ms: u64,
) -> Arc<Vec<RootBrowserHistorySearchHit>> {
    let ttl = Duration::from_millis(cache_ttl_ms);
    if let Ok(cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
        if let Some(snapshot) = cache.snapshot.as_ref() {
            let _expired = snapshot.captured_at.elapsed() > ttl;
            return snapshot.hits.clone();
        }
    }

    Arc::new(Vec::new())
}

#[allow(dead_code)] // Runtime state receipts use this through the binary app layer.
pub(crate) fn root_browser_history_snapshot_status() -> RootPassiveSnapshotStatus {
    ROOT_BROWSER_HISTORY_SNAPSHOT
        .lock()
        .map(|cache| RootPassiveSnapshotStatus {
            generation: cache.generation,
            refreshing: cache.refresh_in_flight,
            cached_count: cache
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.hits.len())
                .unwrap_or(0),
            last_refreshed_at: cache.snapshot.as_ref().map(|s| s.captured_at),
        })
        .unwrap_or(RootPassiveSnapshotStatus {
            generation: 0,
            refreshing: false,
            cached_count: 0,
            last_refreshed_at: None,
        })
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn ensure_root_browser_history_refresh(
    home: PathBuf,
    options: RootBrowserHistorySectionOptions,
    reason: &'static str,
) {
    let Some(refresh) = try_begin_root_browser_history_refresh(&options, reason) else {
        return;
    };

    std::thread::spawn(move || {
        let result = refresh_root_browser_history_snapshot_from_home(&home, &options);
        let _ = finish_root_browser_history_refresh(refresh, result);
    });
}

#[allow(dead_code)]
pub(crate) fn try_begin_root_browser_history_refresh(
    options: &RootBrowserHistorySectionOptions,
    reason: &'static str,
) -> Option<RootBrowserHistoryRefresh> {
    let ttl = Duration::from_millis(options.cache_ttl_ms);
    let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() else {
        return None;
    };
    let now = Instant::now();
    let is_fresh = cache
        .snapshot
        .as_ref()
        .is_some_and(|snapshot| snapshot.captured_at.elapsed() <= ttl);
    if is_fresh || cache.refresh_in_flight {
        return None;
    }
    cache.refresh_in_flight = true;
    cache.generation = cache.generation.wrapping_add(1);
    let generation = cache.generation;
    let cache_age_ms = cache
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.captured_at.elapsed().as_millis() as u64)
        .unwrap_or(0);
    let row_count = cache
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.hits.len())
        .unwrap_or(0);
    tracing::info!(
        source = "browser_history",
        generation,
        cache_age_ms,
        ttl_ms = options.cache_ttl_ms,
        row_count,
        reason,
        "root_passive_snapshot_refresh_started"
    );

    Some(RootBrowserHistoryRefresh {
        generation,
        started_at: now,
    })
}

fn discard_root_browser_history_refresh_from_state(
    cache: &mut RootBrowserHistorySnapshotState,
    generation: u64,
) -> bool {
    if cache.generation != generation || !cache.refresh_in_flight {
        return false;
    }

    cache.refresh_in_flight = false;
    cache.generation = cache.generation.wrapping_add(1);
    true
}

#[allow(dead_code)]
pub(crate) fn discard_root_browser_history_refresh(refresh: RootBrowserHistoryRefresh) -> bool {
    let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() else {
        return false;
    };
    discard_root_browser_history_refresh_from_state(&mut cache, refresh.generation)
}

#[allow(dead_code)]
pub(crate) fn finish_root_browser_history_refresh(
    refresh: RootBrowserHistoryRefresh,
    result: Result<Vec<RootBrowserHistorySearchHit>>,
) -> bool {
    let elapsed_ms = refresh.started_at.elapsed().as_millis() as u64;

    let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() else {
        return false;
    };
    if cache.generation != refresh.generation {
        return false;
    }

    match result {
        Ok(hits) => {
            let row_count = hits.len();
            cache.snapshot = Some(RootBrowserHistorySnapshot {
                captured_at: Instant::now(),
                hits: Arc::new(hits.clone()),
            });
            cache.last_refresh_error = None;

            let urls: Vec<String> = hits.iter().take(100).map(|h| h.url.to_string()).collect();
            std::thread::spawn(move || {
                let needing = crate::favicons::domains_needing_favicons(&urls);
                crate::favicons::fetch_favicons_blocking(&needing);
            });

            tracing::info!(
                source = "browser_history",
                generation = refresh.generation,
                elapsed_ms,
                row_count,
                "root_passive_snapshot_refresh_completed"
            );
        }
        Err(error) => {
            cache.last_refresh_error = Some(error.to_string());
            tracing::warn!(
                source = "browser_history",
                generation = refresh.generation,
                elapsed_ms,
                error_bytes = error.to_string().len(),
                "root_passive_snapshot_refresh_failed"
            );
        }
    }
    cache.refresh_in_flight = false;
    cache.generation = cache.generation.wrapping_add(1);
    true
}

#[allow(dead_code)]
pub(crate) fn mark_root_browser_history_refresh_in_flight() -> bool {
    if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
        if cache.refresh_in_flight {
            return false;
        }
        cache.refresh_in_flight = true;
        cache.generation = cache.generation.wrapping_add(1);
        return true;
    }
    false
}

#[allow(dead_code)]
pub(crate) fn update_root_browser_history_snapshot(hits: Vec<RootBrowserHistorySearchHit>) {
    if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
        cache.snapshot = Some(RootBrowserHistorySnapshot {
            captured_at: Instant::now(),
            hits: Arc::new(hits),
        });
        cache.generation = cache.generation.wrapping_add(1);
        cache.refresh_in_flight = false;
        cache.last_refresh_error = None;
    }
}

#[allow(dead_code)]
pub(crate) fn clear_root_browser_history_refresh_in_flight(error: Option<String>) {
    if let Ok(mut cache) = ROOT_BROWSER_HISTORY_SNAPSHOT.lock() {
        cache.refresh_in_flight = false;
        cache.generation = cache.generation.wrapping_add(1);
        if let Some(err) = error {
            cache.last_refresh_error = Some(err);
        }
    }
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn refresh_root_browser_history_snapshot_from_home(
    home: &Path,
    options: &RootBrowserHistorySectionOptions,
) -> Result<Vec<RootBrowserHistorySearchHit>> {
    let mut hits = Vec::new();
    let selected_providers: HashSet<_> = options.providers.iter().copied().collect();
    let cutoff = chromium_cutoff_time_for_max_age_days(options.max_age_days);
    let per_db_limit = options.scan_limit.max(options.max_results).max(1);

    for spec in ROOT_BROWSER_HISTORY_PROVIDERS {
        if !selected_providers.contains(&spec.provider) {
            continue;
        }

        for db_path in root_chromium_history_db_paths(spec, home) {
            let profile_label = db_path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())
                .unwrap_or("Default")
                .to_string();

            match query_root_chromium_history_db(
                spec,
                &profile_label,
                &db_path,
                "",
                cutoff,
                per_db_limit,
                true,
            ) {
                Ok(mut db_hits) => hits.append(&mut db_hits),
                Err(error) => {
                    tracing::debug!(
                        provider = spec.provider_label,
                        profile_bytes = profile_label.len(),
                        path_bytes = db_path.as_os_str().to_string_lossy().len(),
                        error_bytes = error.to_string().len(),
                        "root browser history source skipped"
                    );
                }
            }
        }
    }

    Ok(dedupe_root_browser_history_hits(hits, options.scan_limit))
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn root_fuzzy_search_browser_history_hits(
    hits: &[RootBrowserHistorySearchHit],
    query: &str,
    search_urls: bool,
) -> Vec<RootBrowserHistorySearchHit> {
    let query = query.trim();
    if query.is_empty() {
        return hits.to_vec();
    }

    let query_lower = query.to_lowercase();
    let query_terms: Vec<&str> = query_lower.split_whitespace().collect();
    let query_is_ascii = query_lower.is_ascii();
    let use_nucleo = query_lower.len() >= crate::scripts::search::MIN_FUZZY_QUERY_LEN;
    let mut nucleo = crate::scripts::NucleoCtx::new(&query_lower);
    let mut scored = Vec::with_capacity(hits.len());

    for hit in hits {
        let title_lower = hit.title.to_lowercase();
        let domain_lower = hit.domain.to_lowercase();
        let url_lower = search_urls.then(|| hit.url.to_lowercase());
        let qualifies = query_terms.iter().all(|term| {
            title_lower.contains(term)
                || domain_lower.contains(term)
                || url_lower.as_ref().is_some_and(|url| url.contains(term))
        });
        if !qualifies {
            continue;
        }

        let mut score = 0i32;
        if query_is_ascii && hit.title.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&hit.title, &query_lower)
            {
                score += if pos == 0 { 220 } else { 175 };
            }
        }
        if query_is_ascii && hit.domain.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&hit.domain, &query_lower)
            {
                score += if pos == 0 { 160 } else { 120 };
            }
        }
        if search_urls && query_is_ascii && hit.url.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&hit.url, &query_lower)
            {
                score += if pos == 0 { 90 } else { 65 };
            }
        }
        if use_nucleo {
            if let Some(nucleo_score) = nucleo.score(&hit.title) {
                score += 100 + (nucleo_score / 24) as i32;
            }
            if let Some(nucleo_score) = nucleo.score(&hit.domain) {
                score += 65 + (nucleo_score / 32) as i32;
            }
            if search_urls {
                if let Some(nucleo_score) = nucleo.score(&hit.url) {
                    score += 45 + (nucleo_score / 40) as i32;
                }
            }
        }
        scored.push((score, hit.clone()));
    }

    scored.sort_by(|(score_a, hit_a), (score_b, hit_b)| {
        score_b
            .cmp(score_a)
            .then_with(|| hit_b.last_visit_unix_ms.cmp(&hit_a.last_visit_unix_ms))
            .then_with(|| hit_b.visit_count.cmp(&hit_a.visit_count))
            .then_with(|| hit_a.title.cmp(&hit_b.title))
    });
    scored.into_iter().map(|(_, hit)| hit).collect()
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn root_browser_history_provider_label(
    provider: crate::config::BrowserHistoryProvider,
) -> Option<&'static str> {
    ROOT_BROWSER_HISTORY_PROVIDERS
        .iter()
        .find(|spec| spec.provider == provider)
        .map(|spec| spec.provider_label)
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn open_browser_history_url(url: &str) -> Result<()> {
    ensure_browser_history_url_is_http_or_https(url)?;
    open::that(url).context("open browser history URL with default handler")
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
pub(crate) fn ensure_browser_history_url_is_http_or_https(url: &str) -> Result<()> {
    if root_browser_history_url_is_http_or_https(url) {
        Ok(())
    } else {
        bail!("unsupported browser history URL scheme")
    }
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn root_browser_history_url_is_http_or_https(url: &str) -> bool {
    let trimmed = url.trim();
    if trimmed.is_empty() || trimmed.chars().any(char::is_control) {
        return false;
    }

    let Some((scheme, _)) = trimmed.split_once(':') else {
        return false;
    };
    if scheme.is_empty() {
        return false;
    }

    let mut chars = scheme.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.')) {
        return false;
    }

    scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")
}

pub fn list_recent_history(limit: usize) -> Result<Vec<BrowserHistoryEntry>> {
    let final_limit = limit.max(1);

    if let Some(cached) = load_history_from_cache(final_limit) {
        return Ok(cached);
    }

    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("HOME is not set"))?;
    let entries = list_recent_history_from_home(&home, final_limit)?;
    store_history_load_cache(final_limit, &entries);
    Ok(entries)
}

fn list_recent_history_from_home(home: &Path, limit: usize) -> Result<Vec<BrowserHistoryEntry>> {
    let final_limit = limit.max(1);
    let mut entries = Vec::new();
    let mut errors = Vec::new();

    for browser in SUPPORTED_BROWSERS {
        let db_paths = history_db_paths_for_browser(browser, home);
        if db_paths.is_empty() {
            continue;
        }

        match load_history_for_browser(browser, &db_paths, PER_BROWSER_LIMIT) {
            Ok(mut browser_entries) => entries.append(&mut browser_entries),
            Err(error) => errors.push(format!("{}: {error}", browser.app_name)),
        }
    }

    if entries.is_empty() && !errors.is_empty() {
        bail!("Failed to read browser history: {}", errors.join(" | "));
    }

    entries.sort_by(|a, b| {
        b.last_visited_at_ms
            .cmp(&a.last_visited_at_ms)
            .then_with(|| a.browser_name.cmp(&b.browser_name))
            .then_with(|| a.url.cmp(&b.url))
    });

    Ok(dedupe_history_entries(entries, final_limit))
}

pub fn fuzzy_search_browser_history(
    entries: &[BrowserHistoryEntry],
    query: &str,
) -> Vec<BrowserHistoryMatch> {
    let query_trimmed = query.trim();
    if let Some(cached) = load_history_search_cache(entries, query_trimmed) {
        return cached;
    }

    let matches = fuzzy_search_browser_history_uncached(entries, query_trimmed);
    store_history_search_cache(entries, query_trimmed, &matches);
    matches
}

fn fuzzy_search_browser_history_uncached(
    entries: &[BrowserHistoryEntry],
    query: &str,
) -> Vec<BrowserHistoryMatch> {
    if query.trim().is_empty() {
        return entries
            .iter()
            .cloned()
            .map(|entry| BrowserHistoryMatch { entry, score: 0 })
            .collect();
    }

    let query_lower = query.trim().to_lowercase();
    let query_is_ascii = query_lower.is_ascii();
    let use_nucleo = query_lower.len() >= crate::scripts::search::MIN_FUZZY_QUERY_LEN;
    let mut nucleo = crate::scripts::NucleoCtx::new(&query_lower);
    let mut matches = Vec::with_capacity(entries.len());

    for entry in entries {
        let mut score = 0i32;

        if query_is_ascii && entry.title.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&entry.title, &query_lower)
            {
                score += if pos == 0 { 260 } else { 210 };
            }
        }

        if query_is_ascii && entry.host.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&entry.host, &query_lower)
            {
                score += if pos == 0 { 220 } else { 170 };
            }
        }

        if query_is_ascii && entry.url.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&entry.url, &query_lower)
            {
                score += if pos == 0 { 140 } else { 110 };
            }
        }

        if query_is_ascii && entry.browser_name.is_ascii() {
            if let Some(pos) =
                crate::scripts::search::find_ignore_ascii_case(&entry.browser_name, &query_lower)
            {
                score += if pos == 0 { 90 } else { 60 };
            }
        }

        if use_nucleo {
            if let Some(nucleo_score) = nucleo.score(&entry.title) {
                score += 120 + (nucleo_score / 18) as i32;
            }
            if !entry.host.is_empty() {
                if let Some(nucleo_score) = nucleo.score(&entry.host) {
                    score += 85 + (nucleo_score / 24) as i32;
                }
            }
            if let Some(nucleo_score) = nucleo.score(&entry.url) {
                score += 60 + (nucleo_score / 32) as i32;
            }
        }

        if score > 0 {
            score += recency_bonus(entry.last_visited_at_ms);
            matches.push(BrowserHistoryMatch {
                entry: entry.clone(),
                score,
            });
        }
    }

    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => match b.entry.last_visited_at_ms.cmp(&a.entry.last_visited_at_ms) {
            Ordering::Equal => match a.entry.browser_name.cmp(&b.entry.browser_name) {
                Ordering::Equal => a.entry.url.cmp(&b.entry.url),
                other => other,
            },
            other => other,
        },
        other => other,
    });

    matches
}

fn load_history_from_cache(limit: usize) -> Option<Vec<BrowserHistoryEntry>> {
    let cache = HISTORY_LOAD_CACHE.lock().ok()?;
    let cached = cache.as_ref()?;
    if cached.loaded_at.elapsed() > HISTORY_CACHE_TTL {
        return None;
    }

    let cache_is_complete = cached.entries.len() < cached.requested_limit;
    if cached.requested_limit >= limit || cache_is_complete {
        return Some(cached.entries.iter().take(limit).cloned().collect());
    }

    None
}

fn store_history_load_cache(limit: usize, entries: &[BrowserHistoryEntry]) {
    if let Ok(mut cache) = HISTORY_LOAD_CACHE.lock() {
        *cache = Some(BrowserHistoryLoadCache {
            loaded_at: Instant::now(),
            requested_limit: limit,
            entries: entries.to_vec(),
        });
    }
}

fn load_history_search_cache(
    entries: &[BrowserHistoryEntry],
    query: &str,
) -> Option<Vec<BrowserHistoryMatch>> {
    let cache = HISTORY_SEARCH_CACHE.lock().ok()?;
    let cached = cache.as_ref()?;
    let entries_ptr = entries.as_ptr() as usize;
    if cached.entries_ptr == entries_ptr
        && cached.entries_len == entries.len()
        && cached.query == query
    {
        return Some(cached.matches.clone());
    }

    None
}

fn store_history_search_cache(
    entries: &[BrowserHistoryEntry],
    query: &str,
    matches: &[BrowserHistoryMatch],
) {
    if let Ok(mut cache) = HISTORY_SEARCH_CACHE.lock() {
        *cache = Some(BrowserHistorySearchCache {
            entries_ptr: entries.as_ptr() as usize,
            entries_len: entries.len(),
            query: query.to_string(),
            matches: matches.to_vec(),
        });
    }
}

fn dedupe_history_entries(
    entries: Vec<BrowserHistoryEntry>,
    limit: usize,
) -> Vec<BrowserHistoryEntry> {
    let mut deduped = Vec::with_capacity(entries.len());
    let mut seen_exact = HashSet::new();
    let mut seen_urls = HashSet::new();
    let mut seen_title_host = HashSet::new();

    for entry in entries {
        let exact_key = entry.history_key();
        if !seen_exact.insert(exact_key) {
            continue;
        }

        let normalized_url = normalized_url_key(&entry.url);
        if normalized_url
            .as_ref()
            .is_some_and(|key| seen_urls.contains(key))
        {
            continue;
        }

        let title_host = normalized_title_host_key(&entry);
        if title_host
            .as_ref()
            .is_some_and(|key| seen_title_host.contains(key))
        {
            continue;
        }

        if let Some(key) = normalized_url {
            seen_urls.insert(key);
        }
        if let Some(key) = title_host {
            seen_title_host.insert(key);
        }

        deduped.push(entry);
        if deduped.len() >= limit {
            break;
        }
    }

    deduped
}

fn normalized_url_key(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let normalized = without_fragment.trim_end_matches('/').to_lowercase();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

fn normalized_title_host_key(entry: &BrowserHistoryEntry) -> Option<String> {
    let title = entry.display_title().trim().to_lowercase();
    let host = entry.host.trim().to_lowercase();
    if title.is_empty() || host.is_empty() {
        return None;
    }

    Some(format!("{host}::{title}"))
}

pub fn format_history_timestamp(last_visited_at_ms: i64) -> String {
    if last_visited_at_ms <= 0 {
        return "Unknown time".to_string();
    }

    let formatted = crate::formatting::format_absolute_unix_millis(last_visited_at_ms);
    if formatted == "unknown time" {
        "Unknown time".to_string()
    } else {
        formatted
    }
}

fn recency_bonus(last_visited_at_ms: i64) -> i32 {
    let Some(utc) = DateTime::<Utc>::from_timestamp_millis(last_visited_at_ms) else {
        return 0;
    };
    let age_hours = (Utc::now() - utc).num_hours();
    if age_hours <= 24 {
        40
    } else if age_hours <= 24 * 7 {
        25
    } else if age_hours <= 24 * 30 {
        10
    } else {
        0
    }
}

fn history_db_paths_for_browser(browser: &SupportedBrowserHistory, home: &Path) -> Vec<PathBuf> {
    let root = home.join(browser.profile_root);
    match browser.family {
        BrowserHistoryFamily::Safari => {
            let path = root.join("History.db");
            if path.exists() {
                vec![path]
            } else {
                Vec::new()
            }
        }
        BrowserHistoryFamily::Firefox => collect_profile_db_paths(&root, "places.sqlite"),
        BrowserHistoryFamily::Chromium => {
            let mut paths = collect_profile_db_paths(&root, "History");
            let root_history = root.join("History");
            if root_history.exists() && !paths.iter().any(|path| path == &root_history) {
                paths.push(root_history);
            }
            paths
        }
    }
}

fn collect_profile_db_paths(root: &Path, db_name: &str) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if !root.exists() {
        return out;
    }

    let direct = root.join(db_name);
    if direct.exists() {
        out.push(direct);
    }

    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let db_path = path.join(db_name);
        if db_path.exists() {
            out.push(db_path);
        }
    }

    out.sort();
    out
}

fn load_history_for_browser(
    browser: &SupportedBrowserHistory,
    db_paths: &[PathBuf],
    per_browser_limit: usize,
) -> Result<Vec<BrowserHistoryEntry>> {
    let mut entries = Vec::new();
    for db_path in db_paths {
        let profile = db_path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .unwrap_or("Default")
            .to_string();

        let mut profile_entries =
            load_history_from_db(browser, db_path, &profile, per_browser_limit).with_context(
                || format!("load_browser_history_failed: path={}", db_path.display()),
            )?;
        entries.append(&mut profile_entries);
    }
    Ok(entries)
}

fn load_history_from_db(
    browser: &SupportedBrowserHistory,
    db_path: &Path,
    profile: &str,
    limit: usize,
) -> Result<Vec<BrowserHistoryEntry>> {
    let _db_name = db_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("History");
    let copied_db = copy_sqlite_db_snapshot(db_path, browser.app_name, profile)?;
    let conn = Connection::open_with_flags(
        &copied_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| {
        format!(
            "open_browser_history_db_failed: path={}",
            copied_db.display()
        )
    })?;

    match browser.family {
        BrowserHistoryFamily::Chromium => query_chromium_history(&conn, browser, profile, limit),
        BrowserHistoryFamily::Safari => query_safari_history(&conn, browser, profile, limit),
        BrowserHistoryFamily::Firefox => query_firefox_history(&conn, browser, profile, limit),
    }
}

fn copy_sqlite_db_snapshot(db_path: &Path, provider: &str, profile: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("missing HOME env var"))?;

    // Stable cache path: ~/.scriptkit/cache/browser-history/{provider}/{profile}
    let cache_root = home
        .join(".scriptkit/cache/browser-history")
        .join(provider.to_lowercase().replace(' ', "-"))
        .join(short_sha256_hex(profile));

    fs::create_dir_all(&cache_root)
        .with_context(|| format!("create_cache_dir_failed: {}", cache_root.display()))?;

    let db_name = db_path
        .file_name()
        .ok_or_else(|| anyhow!("missing database filename"))?;
    let dest_db = cache_root.join(db_name);

    let src_metadata = fs::metadata(db_path)
        .with_context(|| format!("stat_source_db_failed: {}", db_path.display()))?;
    let src_mtime = src_metadata.modified()?;

    if dest_db.exists() {
        let dest_metadata = fs::metadata(&dest_db)?;
        if let Ok(dest_mtime) = dest_metadata.modified() {
            if src_mtime <= dest_mtime {
                // Snapshot is already up to date based on mtime.
                return Ok(dest_db);
            }
        }
    }

    fs::copy(db_path, &dest_db).with_context(|| {
        format!(
            "copy_browser_history_db_failed: {} -> {}",
            db_path.display(),
            dest_db.display()
        )
    })?;

    // Also copy sidecar files if they exist (WAL mode)
    for suffix in ["-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{}{}", db_path.display(), suffix));
        if candidate.exists() {
            let candidate_name = candidate
                .file_name()
                .ok_or_else(|| anyhow!("missing sqlite sidecar filename"))?;
            let dest = cache_root.join(candidate_name);
            let _ = fs::copy(&candidate, dest);
        }
    }

    Ok(dest_db)
}

fn query_chromium_history(
    conn: &Connection,
    browser: &SupportedBrowserHistory,
    profile: &str,
    limit: usize,
) -> Result<Vec<BrowserHistoryEntry>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            urls.url,
            COALESCE(urls.title, ''),
            COALESCE(urls.visit_count, 0),
            MAX(visits.visit_time)
        FROM urls
        JOIN visits ON visits.url = urls.id
        GROUP BY urls.id
        ORDER BY MAX(visits.visit_time) DESC
        LIMIT ?1
        "#,
    )?;

    let rows = stmt.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
        let url: String = row.get(0)?;
        let title: String = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let visit_time: i64 = row.get(3)?;
        Ok(browser_history_entry(
            browser,
            profile,
            title,
            url,
            visit_count,
            chromium_visit_time_to_unix_ms(visit_time),
        ))
    })?;

    collect_rows(rows)
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn root_chromium_history_db_paths(
    spec: &RootBrowserHistoryProviderSpec,
    home: &Path,
) -> Vec<PathBuf> {
    collect_profile_db_paths(&home.join(spec.profile_root), "History")
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn query_root_chromium_history_db(
    spec: &RootBrowserHistoryProviderSpec,
    profile_label: &str,
    db_path: &Path,
    query: &str,
    cutoff_chromium_time: i64,
    limit: usize,
    search_urls: bool,
) -> Result<Vec<RootBrowserHistorySearchHit>> {
    let copied_db = copy_sqlite_db_snapshot(db_path, spec.provider_label, profile_label)?;
    let conn = Connection::open_with_flags(
        &copied_db,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_URI,
    )
    .with_context(|| {
        format!(
            "open_root_browser_history_db_failed: path={}",
            copied_db.display()
        )
    })?;

    query_root_chromium_history_conn(
        &conn,
        spec,
        profile_label,
        query,
        cutoff_chromium_time,
        limit,
        search_urls,
    )
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn query_root_chromium_history_conn(
    conn: &Connection,
    spec: &RootBrowserHistoryProviderSpec,
    profile_label: &str,
    query: &str,
    cutoff_chromium_time: i64,
    limit: usize,
    search_urls: bool,
) -> Result<Vec<RootBrowserHistorySearchHit>> {
    let like_pattern = format!("%{}%", escape_sql_like(query.trim()));
    let mut stmt = conn.prepare(
        r#"
        SELECT id, url, COALESCE(title, ''), COALESCE(visit_count, 0), last_visit_time
        FROM urls
        WHERE last_visit_time >= ?1
          AND (title LIKE ?2 ESCAPE '\'
               OR (?3 != 0 AND url LIKE ?2 ESCAPE '\'))
        ORDER BY last_visit_time DESC, typed_count DESC, visit_count DESC
        LIMIT ?4
        "#,
    )?;

    let rows = stmt.query_map(
        params![
            cutoff_chromium_time,
            like_pattern,
            if search_urls { 1_i64 } else { 0_i64 },
            i64::try_from(limit).unwrap_or(i64::MAX),
        ],
        |row| {
            let id: i64 = row.get(0)?;
            let url: String = row.get(1)?;
            let title: String = row.get(2)?;
            let visit_count: i64 = row.get(3)?;
            let visit_time: i64 = row.get(4)?;
            let domain = host_from_url(&url).to_string();
            Ok(RootBrowserHistorySearchHit {
                stable_key: root_browser_history_stable_key(
                    spec.provider_label,
                    profile_label,
                    id,
                    &url,
                ),
                provider_label: spec.provider_label.to_string(),
                profile_label: profile_label.to_string(),
                title: root_browser_history_display_title(&title, &domain, &url),
                url,
                domain,
                last_visit_unix_ms: chromium_visit_time_to_unix_ms(visit_time),
                visit_count,
            })
        },
    )?;

    let mut hits = Vec::new();
    for row in rows {
        let hit = row?;
        if root_browser_history_url_is_http_or_https(&hit.url) {
            hits.push(hit);
        }
    }
    Ok(hits)
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn dedupe_root_browser_history_hits(
    mut hits: Vec<RootBrowserHistorySearchHit>,
    limit: usize,
) -> Vec<RootBrowserHistorySearchHit> {
    hits.sort_by(|a, b| {
        b.last_visit_unix_ms
            .cmp(&a.last_visit_unix_ms)
            .then_with(|| b.visit_count.cmp(&a.visit_count))
            .then_with(|| a.provider_label.cmp(&b.provider_label))
            .then_with(|| a.url.cmp(&b.url))
    });

    let mut out = Vec::with_capacity(limit.min(hits.len()));
    let mut seen_urls = HashSet::new();
    for hit in hits {
        let normalized = normalized_url_key(&hit.url).unwrap_or_else(|| hit.url.to_lowercase());
        if !seen_urls.insert(normalized) {
            continue;
        }
        out.push(hit);
        if out.len() >= limit {
            break;
        }
    }
    out
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn root_browser_history_display_title(title: &str, domain: &str, url: &str) -> String {
    let title = title.trim();
    if !title.is_empty() {
        return title.to_string();
    }
    let domain = domain.trim();
    if !domain.is_empty() {
        return domain.to_string();
    }
    url.trim().to_string()
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn root_browser_history_stable_key(
    provider_label: &str,
    profile_label: &str,
    row_id: i64,
    url: &str,
) -> String {
    format!(
        "browser-history/{}/{}/{}",
        provider_label.to_ascii_lowercase().replace(' ', "-"),
        short_sha256_hex(profile_label),
        short_sha256_hex(&format!("{row_id}:{url}"))
    )
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn short_sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(&digest[..6])
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn escape_sql_like(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '%' | '_' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[allow(dead_code)] // Root unified search calls this through the binary app layer.
fn chromium_cutoff_time_for_max_age_days(max_age_days: u32) -> i64 {
    let cutoff_unix_secs =
        Utc::now().timestamp() - i64::from(max_age_days).saturating_mul(24 * 60 * 60);
    (cutoff_unix_secs + CHROMIUM_EPOCH_OFFSET_SECS) * 1_000_000
}

fn query_safari_history(
    conn: &Connection,
    browser: &SupportedBrowserHistory,
    profile: &str,
    limit: usize,
) -> Result<Vec<BrowserHistoryEntry>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            history_items.url,
            COALESCE(MAX(NULLIF(history_visits.title, '')), ''),
            COUNT(history_visits.id),
            MAX(history_visits.visit_time)
        FROM history_items
        JOIN history_visits ON history_visits.history_item = history_items.id
        GROUP BY history_items.id
        ORDER BY MAX(history_visits.visit_time) DESC
        LIMIT ?1
        "#,
    )?;

    let rows = stmt.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
        let url: String = row.get(0)?;
        let title: String = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let visit_time: f64 = row.get(3)?;
        Ok(browser_history_entry(
            browser,
            profile,
            title,
            url,
            visit_count,
            safari_visit_time_to_unix_ms(visit_time),
        ))
    })?;

    collect_rows(rows)
}

fn query_firefox_history(
    conn: &Connection,
    browser: &SupportedBrowserHistory,
    profile: &str,
    limit: usize,
) -> Result<Vec<BrowserHistoryEntry>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT
            moz_places.url,
            COALESCE(moz_places.title, ''),
            COALESCE(moz_places.visit_count, 0),
            MAX(moz_historyvisits.visit_date)
        FROM moz_places
        JOIN moz_historyvisits ON moz_historyvisits.place_id = moz_places.id
        WHERE moz_places.hidden = 0
        GROUP BY moz_places.id
        ORDER BY MAX(moz_historyvisits.visit_date) DESC
        LIMIT ?1
        "#,
    )?;

    let rows = stmt.query_map([i64::try_from(limit).unwrap_or(i64::MAX)], |row| {
        let url: String = row.get(0)?;
        let title: String = row.get(1)?;
        let visit_count: i64 = row.get(2)?;
        let visit_time: i64 = row.get(3)?;
        Ok(browser_history_entry(
            browser,
            profile,
            title,
            url,
            visit_count,
            firefox_visit_time_to_unix_ms(visit_time),
        ))
    })?;

    collect_rows(rows)
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
) -> Result<Vec<T>> {
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn browser_history_entry(
    browser: &SupportedBrowserHistory,
    profile: &str,
    title: String,
    url: String,
    visit_count: i64,
    last_visited_at_ms: i64,
) -> BrowserHistoryEntry {
    BrowserHistoryEntry {
        browser_name: browser.app_name.into(),
        browser_bundle_id: browser.bundle_id.into(),
        host: host_from_url(&url).into(),
        title: title.into(),
        url: url.into(),
        last_visited_at_ms,
        visit_count,
        profile: profile.into(),
    }
}

fn host_from_url(url: &str) -> &str {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    after_scheme.split('/').next().unwrap_or(after_scheme)
}

fn chromium_visit_time_to_unix_ms(visit_time: i64) -> i64 {
    ((visit_time / 1_000_000) - CHROMIUM_EPOCH_OFFSET_SECS) * 1000
}

fn safari_visit_time_to_unix_ms(visit_time: f64) -> i64 {
    ((visit_time as i64) + SAFARI_EPOCH_OFFSET_SECS) * 1000
}

fn firefox_visit_time_to_unix_ms(visit_time: i64) -> i64 {
    visit_time / 1000
}

#[cfg(test)]
include!("browser_history_tests.rs");
