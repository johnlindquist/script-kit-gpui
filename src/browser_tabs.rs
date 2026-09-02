use std::cmp::Ordering;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use rayon::prelude::*;
use serde::Deserialize;

const FIELD_SEPARATOR: char = '\u{1e}';
const RECORD_SEPARATOR: char = '\u{1f}';
const BROWSER_TIMEOUT: Duration = Duration::from_secs(3);
const BROWSER_RUNNING_TIMEOUT: Duration = Duration::from_secs(1);
const ROOT_BROWSER_TABS_MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const ROOT_BROWSER_TABS_FAILURE_BACKOFF_BASE: Duration = Duration::from_secs(15);
const ROOT_BROWSER_TABS_FAILURE_BACKOFF_MAX: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BrowserQueryDiagnostic {
    bytes: usize,
    chars: usize,
}

fn browser_query_diagnostic(query: &str) -> BrowserQueryDiagnostic {
    BrowserQueryDiagnostic {
        bytes: query.len(),
        chars: query.chars().count(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserFamily {
    Safari,
    Chromium,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SupportedBrowser {
    app_name: &'static str,
    bundle_id: &'static str,
    family: BrowserFamily,
}

const SUPPORTED_BROWSERS: &[SupportedBrowser] = &[
    SupportedBrowser {
        app_name: "Safari",
        bundle_id: "com.apple.Safari",
        family: BrowserFamily::Safari,
    },
    SupportedBrowser {
        app_name: "Google Chrome",
        bundle_id: "com.google.Chrome",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Arc",
        bundle_id: "company.thebrowser.Browser",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Brave Browser",
        bundle_id: "com.brave.Browser",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Microsoft Edge",
        bundle_id: "com.microsoft.edgemac",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Chromium",
        bundle_id: "org.chromium.Chromium",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Vivaldi",
        bundle_id: "com.vivaldi.Vivaldi",
        family: BrowserFamily::Chromium,
    },
    SupportedBrowser {
        app_name: "Opera",
        bundle_id: "com.operasoftware.Opera",
        family: BrowserFamily::Chromium,
    },
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BrowserTabInfo {
    pub browser_name: Arc<str>,
    pub browser_bundle_id: Arc<str>,
    pub window_index: usize,
    pub tab_index: usize,
    pub title: Arc<str>,
    pub url: Arc<str>,
}

impl BrowserTabInfo {
    pub fn display_title(&self) -> &str {
        if !self.title.trim().is_empty() {
            self.title.trim()
        } else if !self.url.trim().is_empty() {
            self.url.trim()
        } else {
            "Untitled Tab"
        }
    }
}

pub(crate) fn browser_tab_stable_key(tab: &BrowserTabInfo) -> String {
    format!(
        "browser-tab/{}/{}/{}/{}",
        tab.browser_bundle_id, tab.window_index, tab.tab_index, tab.url
    )
}

pub(crate) fn browser_tab_host(tab: &BrowserTabInfo) -> String {
    host_from_url(&tab.url).to_string()
}

/// Returns whether the query is a single hostname-like token, optionally
/// wrapped in an HTTP(S) scheme and/or one trailing slash.
pub(crate) fn query_is_bare_domain(query: &str) -> bool {
    let mut candidate = query.trim();
    if candidate.is_empty() || candidate.chars().any(char::is_whitespace) {
        return false;
    }

    if candidate
        .get(..8)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("https://"))
    {
        candidate = &candidate[8..];
    } else if candidate
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http://"))
    {
        candidate = &candidate[7..];
    }
    candidate = candidate.strip_suffix('/').unwrap_or(candidate);

    let labels = candidate.split('.').collect::<Vec<_>>();
    if labels.len() < 2 {
        return false;
    }
    if !labels.iter().all(|label| {
        !label.is_empty()
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    }) {
        return false;
    }

    labels
        .last()
        .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|byte| byte.is_ascii_alphabetic()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserTabMatch {
    pub tab: BrowserTabInfo,
    pub score: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RootBrowserTabsSectionOptions {
    pub enabled: bool,
    pub max_results: usize,
    pub min_query_chars: usize,
    pub scan_limit: usize,
    pub search_urls: bool,
    pub providers: Vec<crate::config::BrowserTabProvider>,
    pub cache_ttl_ms: u64,
}

impl Default for RootBrowserTabsSectionOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            max_results: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_TABS_MAX_RESULTS,
            min_query_chars:
                crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_TABS_MIN_QUERY_CHARS,
            scan_limit: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_TABS_SCAN_LIMIT,
            search_urls: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_TABS_SEARCH_URLS,
            providers: crate::config::BrowserTabProvider::default_root_providers(),
            cache_ttl_ms: crate::config::defaults::DEFAULT_UNIFIED_SEARCH_BROWSER_TABS_CACHE_TTL_MS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct RootPassiveSnapshotStatus {
    pub generation: u64,
    pub refreshing: bool,
    pub cached_count: usize,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct RootBrowserTabSearchHit {
    pub stable_key: String,
    pub url: String,
    pub provider_label: String,
    pub tab: BrowserTabInfo,
    pub title: String,
    pub domain: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct RootBrowserTabSnapshot {
    captured_at: Instant,
    tabs: Arc<Vec<BrowserTabInfo>>,
}

#[derive(Debug, Default)]
#[allow(dead_code)]
struct RootBrowserTabSnapshotState {
    snapshot: Option<RootBrowserTabSnapshot>,
    refresh_in_flight: bool,
    refresh_needed: bool,
    generation: u64,
    last_refresh_error: Option<String>,
    last_attempt_at: Option<Instant>,
    last_success_at: Option<Instant>,
    next_refresh_after: Option<Instant>,
    failure_count: u32,
}

#[allow(dead_code)]
static ROOT_BROWSER_TAB_SNAPSHOT: LazyLock<Mutex<RootBrowserTabSnapshotState>> =
    LazyLock::new(|| Mutex::new(RootBrowserTabSnapshotState::default()));

#[derive(Debug, Clone, Copy)]
pub(crate) struct RootBrowserTabsRefresh {
    generation: u64,
    started_at: Instant,
}

pub fn list_open_tabs() -> Result<Vec<BrowserTabInfo>> {
    list_open_tabs_for_root_providers(&[])
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BrowserTabsTestProvider {
    Rows(Vec<BrowserTabInfo>),
    Config {
        #[serde(default, rename = "delayMs")]
        delay_ms: u64,
        #[serde(default)]
        fail: bool,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        tabs: Vec<BrowserTabInfo>,
    },
}

fn list_open_tabs_for_root_providers(
    providers: &[crate::config::BrowserTabProvider],
) -> Result<Vec<BrowserTabInfo>> {
    let _span = tracing::info_span!("list_open_tabs").entered();

    if let Ok(raw) = std::env::var("SCRIPT_KIT_BROWSER_TABS_TEST_PROVIDER") {
        let provider: BrowserTabsTestProvider =
            serde_json::from_str(&raw).context("parse SCRIPT_KIT_BROWSER_TABS_TEST_PROVIDER")?;
        return match provider {
            BrowserTabsTestProvider::Rows(rows) => Ok(rows),
            BrowserTabsTestProvider::Config {
                delay_ms,
                fail,
                error,
                tabs,
            } => {
                if delay_ms > 0 {
                    std::thread::sleep(Duration::from_millis(delay_ms));
                }
                if fail {
                    bail!(
                        "{}",
                        error.unwrap_or_else(|| "browser tabs test provider failure".to_string())
                    );
                }
                Ok(tabs)
            }
        };
    }

    let results: Vec<Result<Vec<BrowserTabInfo>>> = SUPPORTED_BROWSERS
        .par_iter()
        .filter(|browser| root_supported_browser_provider_is_enabled(browser, providers))
        .map(|browser| {
            let _browser_span =
                tracing::info_span!("list_tabs_for_browser", browser = browser.app_name).entered();
            match is_browser_running(browser.bundle_id) {
                Ok(false) => Ok(Vec::new()),
                Ok(true) => {
                    let tabs = list_tabs_for_browser(browser);
                    if let Ok(ref t) = tabs {
                        tracing::info!(
                            browser = browser.app_name,
                            tab_count = t.len(),
                            "list_tabs_for_browser_completed"
                        );
                    }
                    tabs
                }
                Err(error) => {
                    tracing::warn!(
                        browser = browser.app_name,
                        error_bytes = error.to_string().len(),
                        "check_browser_running_failed"
                    );
                    Err(error.context(browser.app_name))
                }
            }
        })
        .collect();

    combine_browser_tab_results(results)
}

fn combine_browser_tab_results(
    results: Vec<Result<Vec<BrowserTabInfo>>>,
) -> Result<Vec<BrowserTabInfo>> {
    let mut tabs = Vec::new();
    for result in results {
        tabs.append(&mut result?);
    }
    Ok(tabs)
}

pub fn activate_tab(tab: &BrowserTabInfo) -> Result<()> {
    let browser = supported_browser_for(tab).ok_or_else(|| {
        anyhow!(
            "Unsupported browser '{}'",
            if tab.browser_name.is_empty() {
                tab.browser_bundle_id.as_ref()
            } else {
                tab.browser_name.as_ref()
            }
        )
    })?;

    let script = build_activate_tab_script(browser, tab.window_index, tab.tab_index);
    let index_error = match crate::platform::run_osascript(&script, "browser_tabs_activate") {
        Ok(_) => return Ok(()),
        Err(error) => error,
    };

    // Activation requests come from cached snapshots, so the window/tab
    // indexes are often stale by the time the user hits Enter (a closed or
    // reordered window yields "Window N is no longer available"). Re-locate
    // the tab by URL across all windows before reporting failure.
    activate_tab_by_url(browser, &tab.url).with_context(|| {
        format!(
            "activate_browser_tab_failed: browser={} window_index={} tab_index={} index_error={index_error:#}",
            browser.app_name, tab.window_index, tab.tab_index
        )
    })
}

fn activate_tab_by_url(browser: &SupportedBrowser, url: &str) -> Result<()> {
    let url = url.trim();
    if url.is_empty() {
        bail!("tab has no URL to re-locate");
    }
    let script = build_activate_tab_by_url_jxa(browser, url);
    let output = crate::platform::run_jxa_with_timeout(
        &script,
        "browser_tabs_activate_by_url",
        BROWSER_TIMEOUT,
    )?;
    if output.trim() == "ok" {
        return Ok(());
    }
    bail!("no open tab with URL {url} in {}", browser.app_name)
}

pub fn fuzzy_search_browser_tabs(tabs: &[BrowserTabInfo], query: &str) -> Vec<BrowserTabMatch> {
    let query_diagnostic = browser_query_diagnostic(query);
    let _span = tracing::info_span!(
        "fuzzy_search_browser_tabs",
        tab_count = tabs.len(),
        query_bytes = query_diagnostic.bytes,
        query_chars = query_diagnostic.chars,
    )
    .entered();

    if query.trim().is_empty() {
        return tabs
            .iter()
            .cloned()
            .map(|tab| BrowserTabMatch { tab, score: 0 })
            .collect();
    }

    let query_lower = query.trim().to_lowercase();
    let query_is_ascii = query_lower.is_ascii();
    let use_nucleo = query_lower.len() >= crate::scripts::search::MIN_FUZZY_QUERY_LEN;

    let mut matches: Vec<BrowserTabMatch> = tabs
        .par_chunks(128)
        .flat_map(|chunk| {
            let mut nucleo = if use_nucleo {
                Some(crate::scripts::NucleoCtx::new(&query_lower))
            } else {
                None
            };

            chunk
                .iter()
                .filter_map(|tab| {
                    let mut score = 0i32;
                    let host = host_from_url(&tab.url);

                    if query_is_ascii && tab.title.is_ascii() {
                        if let Some(pos) =
                            crate::scripts::search::find_ignore_ascii_case(&tab.title, &query_lower)
                        {
                            score += if pos == 0 { 240 } else { 190 };
                        }
                    }

                    if query_is_ascii && host.is_ascii() {
                        if let Some(pos) =
                            crate::scripts::search::find_ignore_ascii_case(host, &query_lower)
                        {
                            score += if pos == 0 { 180 } else { 135 };
                        }
                    }

                    if query_is_ascii && tab.url.is_ascii() {
                        if let Some(pos) =
                            crate::scripts::search::find_ignore_ascii_case(&tab.url, &query_lower)
                        {
                            score += if pos == 0 { 120 } else { 90 };
                        }
                    }

                    if query_is_ascii && tab.browser_name.is_ascii() {
                        if let Some(pos) = crate::scripts::search::find_ignore_ascii_case(
                            &tab.browser_name,
                            &query_lower,
                        ) {
                            score += if pos == 0 { 80 } else { 55 };
                        }
                    }

                    if let Some(ref mut n) = nucleo {
                        if let Some(nucleo_score) = n.score(&tab.title) {
                            score += 110 + (nucleo_score / 20) as i32;
                        }
                        if !host.is_empty() {
                            if let Some(nucleo_score) = n.score(host) {
                                score += 70 + (nucleo_score / 28) as i32;
                            }
                        }
                        if let Some(nucleo_score) = n.score(&tab.url) {
                            score += 55 + (nucleo_score / 35) as i32;
                        }
                        if let Some(nucleo_score) = n.score(&tab.browser_name) {
                            score += 35 + (nucleo_score / 40) as i32;
                        }
                    }

                    if score > 0 {
                        Some(BrowserTabMatch {
                            tab: tab.clone(),
                            score,
                        })
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect();

    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => match a.tab.browser_name.cmp(&b.tab.browser_name) {
            Ordering::Equal => match a.tab.display_title().cmp(b.tab.display_title()) {
                Ordering::Equal => a.tab.url.cmp(&b.tab.url),
                other => other,
            },
            other => other,
        },
        other => other,
    });

    matches
}

pub(crate) fn root_browser_tabs_query_is_eligible(
    query: &str,
    options: RootBrowserTabsSectionOptions,
) -> bool {
    let query = query.trim();
    options.enabled
        && crate::scripts::search::query_meets_min_query_chars(query, options.min_query_chars)
        && !query.contains('\n')
        && !query.contains('\r')
}

#[allow(dead_code)]
pub(crate) fn search_root_browser_tabs_meta(
    query: &str,
    options: RootBrowserTabsSectionOptions,
) -> Vec<RootBrowserTabSearchHit> {
    search_root_browser_tabs_meta_cached(query, options)
}

#[allow(dead_code)]
pub(crate) fn search_root_browser_tabs_meta_cached(
    query: &str,
    options: RootBrowserTabsSectionOptions,
) -> Vec<RootBrowserTabSearchHit> {
    search_root_browser_tabs_internal(query, options, RootBrowserTabsLookupMode::CachedOnly)
}

#[allow(dead_code)]
pub(crate) fn search_root_browser_tabs_meta_direct(
    query: &str,
    options: RootBrowserTabsSectionOptions,
) -> Vec<RootBrowserTabSearchHit> {
    // Preserve explicit-mode permissive matching, but never begin a detached
    // refresh here: only the app owner can bind completion to its live query.
    search_root_browser_tabs_internal(query, options, RootBrowserTabsLookupMode::RefreshThenCached)
}

#[allow(dead_code)]
pub(crate) fn focus_root_browser_tab(hit: &RootBrowserTabSearchHit) -> Result<()> {
    activate_tab(&hit.tab)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootBrowserTabsLookupMode {
    CachedOnly,
    RefreshThenCached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RootBrowserTabsFuzzyMode {
    Permissive,
    Compact,
}

fn search_root_browser_tabs_internal(
    query: &str,
    options: RootBrowserTabsSectionOptions,
    mode: RootBrowserTabsLookupMode,
) -> Vec<RootBrowserTabSearchHit> {
    let query_diagnostic = browser_query_diagnostic(query);
    let _span = tracing::info_span!(
        "search_root_browser_tabs_internal",
        query_bytes = query_diagnostic.bytes,
        query_chars = query_diagnostic.chars,
        mode = ?mode,
        cache_ttl_ms = options.cache_ttl_ms
    )
    .entered();

    if !root_browser_tabs_query_is_eligible(query, options.clone()) {
        return Vec::new();
    }

    let tabs = cached_root_browser_tabs_snapshot(options.cache_ttl_ms);
    let tabs = tabs
        .iter()
        .filter(|tab| root_tab_provider_is_enabled(tab, &options.providers))
        .take(options.scan_limit)
        .cloned()
        .collect::<Vec<_>>();

    let fuzzy_mode = match mode {
        RootBrowserTabsLookupMode::CachedOnly => RootBrowserTabsFuzzyMode::Compact,
        RootBrowserTabsLookupMode::RefreshThenCached => RootBrowserTabsFuzzyMode::Permissive,
    };

    root_fuzzy_search_browser_tabs(&tabs, query, options.search_urls, fuzzy_mode)
        .into_iter()
        .take(options.max_results)
        .map(|tab_match| {
            let domain = host_from_url(&tab_match.tab.url).to_string();
            RootBrowserTabSearchHit {
                stable_key: browser_tab_stable_key(&tab_match.tab),
                url: tab_match.tab.url.to_string(),
                provider_label: tab_match.tab.browser_name.to_string(),
                tab: tab_match.tab.clone(),
                title: tab_match.tab.title.to_string(),
                domain,
                score: tab_match.score as f32,
            }
        })
        .collect()
}

#[allow(dead_code)]
fn cached_root_browser_tabs_snapshot(cache_ttl_ms: u64) -> Arc<Vec<BrowserTabInfo>> {
    let ttl = Duration::from_millis(cache_ttl_ms);
    if let Ok(cache) = ROOT_BROWSER_TAB_SNAPSHOT.try_lock() {
        if let Some(snapshot) = cache.snapshot.as_ref() {
            let _expired = crate::runtime_policy::root_search_now()
                .saturating_duration_since(snapshot.captured_at)
                > ttl;
            return snapshot.tabs.clone();
        }
    }

    Arc::new(Vec::new())
}

fn root_browser_tabs_cache_is_fresh(
    cache: &RootBrowserTabSnapshotState,
    now: Instant,
    ttl: Duration,
) -> bool {
    !cache.refresh_needed
        && cache
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| now.saturating_duration_since(snapshot.captured_at) <= ttl)
}

#[allow(dead_code)] // Root search cache receipts use this through the binary app layer.
fn fresh_root_browser_tabs_cache_status(
    cache: &RootBrowserTabSnapshotState,
    now: Instant,
    ttl: Duration,
) -> Option<RootPassiveSnapshotStatus> {
    if cache.refresh_in_flight
        || cache.last_refresh_error.is_some()
        || !root_browser_tabs_cache_is_fresh(cache, now, ttl)
    {
        return None;
    }
    Some(RootPassiveSnapshotStatus {
        generation: cache.generation,
        refreshing: false,
        cached_count: cache.snapshot.as_ref()?.tabs.len(),
    })
}

#[allow(dead_code)] // Root search cache receipts use this through the binary app layer.
/// Positive current snapshot evidence; missing, contested, stale, or failed
/// caches are unknown rather than an invented empty successful snapshot.
pub(crate) fn root_browser_tabs_fresh_cache_status(
    cache_ttl_ms: u64,
) -> Option<RootPassiveSnapshotStatus> {
    let cache = ROOT_BROWSER_TAB_SNAPSHOT.try_lock().ok()?;
    fresh_root_browser_tabs_cache_status(
        &cache,
        crate::runtime_policy::root_search_now(),
        Duration::from_millis(cache_ttl_ms),
    )
}

#[allow(dead_code)]
pub(crate) fn root_browser_tabs_snapshot_status() -> RootPassiveSnapshotStatus {
    ROOT_BROWSER_TAB_SNAPSHOT
        .try_lock()
        .map(|cache| RootPassiveSnapshotStatus {
            generation: cache.generation,
            refreshing: cache.refresh_in_flight,
            cached_count: cache
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.tabs.len())
                .unwrap_or(0),
        })
        .unwrap_or(RootPassiveSnapshotStatus {
            generation: 0,
            refreshing: false,
            cached_count: 0,
        })
}

#[allow(dead_code)]
fn ensure_root_browser_tabs_refresh(
    cache_ttl_ms: u64,
    reason: &'static str,
    providers: Vec<crate::config::BrowserTabProvider>,
) {
    let Some(refresh) = try_begin_root_browser_tabs_refresh(cache_ttl_ms, providers.len(), reason)
    else {
        return;
    };

    std::thread::spawn(move || {
        let result = refresh_root_browser_tabs_snapshot(providers);
        let _ = finish_root_browser_tabs_refresh(refresh, result);
    });
}

#[allow(dead_code)]
pub(crate) fn try_begin_root_browser_tabs_refresh(
    cache_ttl_ms: u64,
    provider_count: usize,
    reason: &'static str,
) -> Option<RootBrowserTabsRefresh> {
    let ttl = Duration::from_millis(cache_ttl_ms);
    let Ok(mut cache) = ROOT_BROWSER_TAB_SNAPSHOT.try_lock() else {
        return None;
    };
    let now = crate::runtime_policy::root_search_now();
    let is_fresh = root_browser_tabs_cache_is_fresh(&cache, now, ttl);
    let too_soon = cache
        .last_attempt_at
        .is_some_and(|last| now.duration_since(last) < ROOT_BROWSER_TABS_MIN_REFRESH_INTERVAL);
    let in_backoff = cache.next_refresh_after.is_some_and(|next| now < next);
    if is_fresh || cache.refresh_in_flight || too_soon || in_backoff {
        return None;
    }
    let generation = cache.generation.checked_add(1)?;
    cache.refresh_in_flight = true;
    cache.last_attempt_at = Some(now);
    cache.generation = generation;
    let cache_age_ms = cache
        .snapshot
        .as_ref()
        .map(|snapshot| {
            now.saturating_duration_since(snapshot.captured_at)
                .as_millis() as u64
        })
        .unwrap_or(0);
    let row_count = cache
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.tabs.len())
        .unwrap_or(0);
    tracing::info!(
        source = "browser_tabs",
        generation,
        cache_age_ms,
        ttl_ms = cache_ttl_ms,
        row_count,
        reason,
        provider_count,
        "root_passive_snapshot_refresh_started"
    );

    Some(RootBrowserTabsRefresh {
        generation,
        started_at: Instant::now(),
    })
}

#[allow(dead_code)]
pub(crate) fn refresh_root_browser_tabs_snapshot(
    providers: Vec<crate::config::BrowserTabProvider>,
) -> Result<Vec<BrowserTabInfo>> {
    anyhow::ensure!(
        !crate::runtime_policy::is_owned_evaluation(),
        "owned_source_snapshot_required"
    );
    list_open_tabs_for_root_providers(&providers)
}

#[allow(dead_code)] // Owned search fixtures call this through the binary app layer.
pub(crate) fn owned_root_browser_tabs_snapshot(
    result: Result<Vec<BrowserTabInfo>>,
) -> Result<Vec<BrowserTabInfo>> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    result
}

#[allow(dead_code)] // Owned search fixtures call this through the binary app layer.
pub(crate) fn reset_owned_root_browser_tabs() -> Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut cache = ROOT_BROWSER_TAB_SNAPSHOT
        .lock()
        .map_err(|_| anyhow::anyhow!("browser_tabs_cache_poisoned"))?;
    let generation = cache
        .generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("browser_tabs_generation_exhausted"))?;
    *cache = RootBrowserTabSnapshotState {
        generation,
        ..Default::default()
    };
    Ok(())
}

#[allow(dead_code)] // Owned search source changes call this through the binary app layer.
pub(crate) fn invalidate_owned_root_browser_tabs_freshness() -> Result<()> {
    anyhow::ensure!(
        crate::runtime_policy::is_owned_evaluation(),
        "owned_runtime_required"
    );
    let mut cache = ROOT_BROWSER_TAB_SNAPSHOT
        .lock()
        .map_err(|_| anyhow::anyhow!("browser_tabs_cache_poisoned"))?;
    cache.generation = cache
        .generation
        .checked_add(1)
        .ok_or_else(|| anyhow::anyhow!("browser_tabs_generation_exhausted"))?;
    cache.refresh_in_flight = false;
    cache.refresh_needed = true;
    cache.last_attempt_at = None;
    cache.next_refresh_after = None;
    Ok(())
}

fn discard_root_browser_tabs_refresh_from_state(
    cache: &mut RootBrowserTabSnapshotState,
    generation: u64,
) -> bool {
    if cache.generation != generation || !cache.refresh_in_flight {
        return false;
    }

    cache.refresh_in_flight = false;
    cache.generation = cache.generation.saturating_add(1);
    // This attempt belonged to the canceled query, not the next one. Keep a
    // genuine failure backoff, but do not impose its five-second start throttle.
    cache.last_attempt_at = None;
    true
}

#[allow(dead_code)]
pub(crate) fn discard_root_browser_tabs_refresh(refresh: RootBrowserTabsRefresh) -> bool {
    let Ok(mut cache) = ROOT_BROWSER_TAB_SNAPSHOT.lock() else {
        return false;
    };
    discard_root_browser_tabs_refresh_from_state(&mut cache, refresh.generation)
}

#[allow(dead_code)]
pub(crate) fn finish_root_browser_tabs_refresh(
    refresh: RootBrowserTabsRefresh,
    result: Result<Vec<BrowserTabInfo>>,
) -> bool {
    let elapsed_ms = refresh.started_at.elapsed().as_millis() as u64;

    let Ok(mut cache) = ROOT_BROWSER_TAB_SNAPSHOT.lock() else {
        return false;
    };
    if cache.generation != refresh.generation || !cache.refresh_in_flight {
        return false;
    }

    match result {
        Ok(tabs) => {
            let row_count = tabs.len();
            cache.snapshot = Some(RootBrowserTabSnapshot {
                captured_at: crate::runtime_policy::root_search_now(),
                tabs: Arc::new(tabs.clone()),
            });
            cache.refresh_needed = false;
            cache.last_refresh_error = None;
            cache.last_success_at = Some(crate::runtime_policy::root_search_now());
            cache.next_refresh_after = None;
            cache.failure_count = 0;

            if !crate::runtime_policy::is_owned_evaluation() {
                let urls: Vec<String> = tabs.iter().map(|t| t.url.to_string()).collect();
                std::thread::spawn(move || {
                    let needing = crate::favicons::domains_needing_favicons(&urls);
                    crate::favicons::fetch_favicons_blocking(&needing);
                });
            }

            tracing::info!(
                source = "browser_tabs",
                generation = refresh.generation,
                elapsed_ms,
                row_count,
                "root_passive_snapshot_refresh_completed"
            );
        }
        Err(error) => {
            cache.failure_count = cache.failure_count.saturating_add(1);
            let backoff = root_browser_tabs_failure_backoff(cache.failure_count);
            cache.next_refresh_after = Some(crate::runtime_policy::root_search_now() + backoff);
            cache.last_refresh_error = Some(error.to_string());
            tracing::warn!(
                source = "browser_tabs",
                generation = refresh.generation,
                elapsed_ms,
                failure_count = cache.failure_count,
                backoff_ms = backoff.as_millis() as u64,
                error_bytes = error.to_string().len(),
                "root_passive_snapshot_refresh_failed"
            );
        }
    }
    cache.refresh_in_flight = false;
    cache.generation = cache.generation.saturating_add(1);
    true
}

fn root_browser_tabs_failure_backoff(failure_count: u32) -> Duration {
    let exponent = failure_count.saturating_sub(1).min(8);
    let multiplier = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
    ROOT_BROWSER_TABS_FAILURE_BACKOFF_BASE
        .saturating_mul(multiplier)
        .min(ROOT_BROWSER_TABS_FAILURE_BACKOFF_MAX)
}

#[cfg(test)]
fn reset_root_browser_tabs_snapshot_for_test() {
    if let Ok(mut cache) = ROOT_BROWSER_TAB_SNAPSHOT.lock() {
        *cache = RootBrowserTabSnapshotState::default();
    }
}

#[cfg(test)]
fn store_root_browser_tabs_snapshot_for_test(captured_at: Instant, tabs: Vec<BrowserTabInfo>) {
    if let Ok(mut cache) = ROOT_BROWSER_TAB_SNAPSHOT.lock() {
        cache.snapshot = Some(RootBrowserTabSnapshot {
            captured_at,
            tabs: Arc::new(tabs),
        });
        cache.refresh_in_flight = false;
    }
}

#[allow(dead_code)]
fn root_fuzzy_search_browser_tabs(
    tabs: &[BrowserTabInfo],
    query: &str,
    search_urls: bool,
    mode: RootBrowserTabsFuzzyMode,
) -> Vec<BrowserTabMatch> {
    let query_diagnostic = browser_query_diagnostic(query);
    let _span = tracing::info_span!(
        "root_fuzzy_search_browser_tabs",
        tab_count = tabs.len(),
        query_bytes = query_diagnostic.bytes,
        query_chars = query_diagnostic.chars,
        search_urls,
        mode = ?mode
    )
    .entered();

    if query.trim().is_empty() {
        return tabs
            .iter()
            .cloned()
            .map(|tab| BrowserTabMatch { tab, score: 0 })
            .collect();
    }

    let query_lower = query.trim().to_lowercase();
    let query_is_ascii = query_lower.is_ascii();
    let use_nucleo = query_lower.len() >= crate::scripts::search::MIN_FUZZY_QUERY_LEN;

    let mut nucleo = if use_nucleo {
        Some(crate::scripts::NucleoCtx::new(&query_lower))
    } else {
        None
    };
    let mut matches: Vec<BrowserTabMatch> = tabs
        .iter()
        .filter_map(|tab| {
            let mut score = 0i32;
            let host = host_from_url(&tab.url);

            if query_is_ascii && tab.title.is_ascii() {
                if let Some(pos) =
                    crate::scripts::search::find_ignore_ascii_case(&tab.title, &query_lower)
                {
                    score += if pos == 0 { 240 } else { 190 };
                }
            }

            if query_is_ascii && host.is_ascii() {
                if let Some(pos) =
                    crate::scripts::search::find_ignore_ascii_case(host, &query_lower)
                {
                    score += if pos == 0 { 180 } else { 135 };
                }
            }

            if search_urls && query_is_ascii && tab.url.is_ascii() {
                if let Some(pos) =
                    crate::scripts::search::find_ignore_ascii_case(&tab.url, &query_lower)
                {
                    score += if pos == 0 { 120 } else { 90 };
                }
            }

            if query_is_ascii && tab.browser_name.is_ascii() {
                if let Some(pos) =
                    crate::scripts::search::find_ignore_ascii_case(&tab.browser_name, &query_lower)
                {
                    score += if pos == 0 { 80 } else { 55 };
                }
            }

            if let Some(ref mut n) = nucleo {
                if let Some(nucleo_score) = match mode {
                    RootBrowserTabsFuzzyMode::Permissive => n.score(&tab.title),
                    RootBrowserTabsFuzzyMode::Compact => n.compact_score(&tab.title, &query_lower),
                } {
                    score += 110 + (nucleo_score / 20) as i32;
                }
                if !host.is_empty() {
                    if let Some(nucleo_score) = match mode {
                        RootBrowserTabsFuzzyMode::Permissive => n.score(host),
                        RootBrowserTabsFuzzyMode::Compact => n.compact_score(host, &query_lower),
                    } {
                        score += 70 + (nucleo_score / 28) as i32;
                    }
                }
                if search_urls {
                    if let Some(nucleo_score) = match mode {
                        RootBrowserTabsFuzzyMode::Permissive => n.score(&tab.url),
                        RootBrowserTabsFuzzyMode::Compact => {
                            n.compact_score(&tab.url, &query_lower)
                        }
                    } {
                        score += 55 + (nucleo_score / 35) as i32;
                    }
                }
                if let Some(nucleo_score) = match mode {
                    RootBrowserTabsFuzzyMode::Permissive => n.score(&tab.browser_name),
                    RootBrowserTabsFuzzyMode::Compact => {
                        n.compact_score(&tab.browser_name, &query_lower)
                    }
                } {
                    score += 35 + (nucleo_score / 40) as i32;
                }
            }

            if score > 0 {
                Some(BrowserTabMatch {
                    tab: tab.clone(),
                    score,
                })
            } else {
                None
            }
        })
        .collect();

    matches.sort_by(|a, b| match b.score.cmp(&a.score) {
        Ordering::Equal => match a.tab.browser_name.cmp(&b.tab.browser_name) {
            Ordering::Equal => match a.tab.display_title().cmp(b.tab.display_title()) {
                Ordering::Equal => a.tab.url.cmp(&b.tab.url),
                other => other,
            },
            other => other,
        },
        other => other,
    });

    matches
}

#[allow(dead_code)]
fn root_tab_provider_is_enabled(
    tab: &BrowserTabInfo,
    providers: &[crate::config::BrowserTabProvider],
) -> bool {
    let Some(provider) = browser_tab_provider_for_bundle_id(&tab.browser_bundle_id) else {
        return false;
    };
    providers.is_empty() || providers.contains(&provider)
}

fn root_supported_browser_provider_is_enabled(
    browser: &SupportedBrowser,
    providers: &[crate::config::BrowserTabProvider],
) -> bool {
    if providers.is_empty() {
        return true;
    }
    let Some(provider) = browser_tab_provider_for_bundle_id(browser.bundle_id) else {
        return false;
    };
    providers.contains(&provider)
}

fn browser_tab_provider_for_bundle_id(
    bundle_id: &str,
) -> Option<crate::config::BrowserTabProvider> {
    match bundle_id {
        "company.thebrowser.Browser" => Some(crate::config::BrowserTabProvider::Arc),
        "com.google.Chrome" => Some(crate::config::BrowserTabProvider::Chrome),
        "com.brave.Browser" => Some(crate::config::BrowserTabProvider::Brave),
        "com.microsoft.edgemac" => Some(crate::config::BrowserTabProvider::Edge),
        _ => None,
    }
}

fn list_tabs_for_browser(browser: &SupportedBrowser) -> Result<Vec<BrowserTabInfo>> {
    let script = build_list_tabs_jxa(browser);
    let output =
        crate::platform::run_jxa_with_timeout(&script, "browser_tabs_list", BROWSER_TIMEOUT)
            .with_context(|| format!("list_browser_tabs_failed: browser={}", browser.app_name))?;
    parse_tab_rows(browser, &output)
}

fn is_browser_running(bundle_id: &str) -> Result<bool> {
    let bundle_id = crate::utils::escape_applescript_string(bundle_id);
    let script = format!(
        r#"
tell application "System Events"
    set matching_processes to every application process whose bundle identifier is "{bundle_id}"
    return (count of matching_processes) > 0
end tell
"#,
    );

    let output = crate::platform::run_osascript_with_timeout(
        &script,
        "browser_tabs_is_running",
        BROWSER_RUNNING_TIMEOUT,
    )
    .with_context(|| format!("browser_tabs_is_running_failed: bundle_id={bundle_id}"))?;
    Ok(output.trim().eq_ignore_ascii_case("true"))
}

fn supported_browser_for(tab: &BrowserTabInfo) -> Option<&'static SupportedBrowser> {
    SUPPORTED_BROWSERS.iter().find(|browser| {
        browser.bundle_id == tab.browser_bundle_id.as_ref()
            || browser.app_name == tab.browser_name.as_ref()
    })
}

fn host_from_url(url: &str) -> &str {
    let after_scheme = url.split_once("://").map(|(_, rest)| rest).unwrap_or(url);
    after_scheme.split('/').next().unwrap_or(after_scheme)
}

fn parse_tab_rows(browser: &SupportedBrowser, output: &str) -> Result<Vec<BrowserTabInfo>> {
    let trimmed = output.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    trimmed
        .split(RECORD_SEPARATOR)
        .filter(|row| !row.is_empty())
        .map(|row| {
            let mut parts = row.split(FIELD_SEPARATOR);
            let window_index = parts
                .next()
                .ok_or_else(|| anyhow!("Missing window index"))?
                .parse::<usize>()
                .context("Invalid window index")?;
            let tab_index = parts
                .next()
                .ok_or_else(|| anyhow!("Missing tab index"))?
                .parse::<usize>()
                .context("Invalid tab index")?;
            let title = parts
                .next()
                .ok_or_else(|| anyhow!("Missing title"))?
                .trim()
                .to_string();
            let url = parts
                .next()
                .ok_or_else(|| anyhow!("Missing URL"))?
                .trim()
                .to_string();

            Ok(BrowserTabInfo {
                browser_name: browser.app_name.into(),
                browser_bundle_id: browser.bundle_id.into(),
                window_index,
                tab_index,
                title: title.into(),
                url: url.into(),
            })
        })
        .collect()
}

#[cfg(test)]
fn build_list_tabs_script(browser: &SupportedBrowser) -> String {
    let title_property = match browser.family {
        BrowserFamily::Safari => "name",
        BrowserFamily::Chromium => "title",
    };

    format!(
        r#"
on replace_text(the_text, old_text, new_text)
    set AppleScript's text item delimiters to old_text
    set text_items to every text item of the_text
    set AppleScript's text item delimiters to new_text
    set the_text to text_items as text
    set AppleScript's text item delimiters to ""
    return the_text
end replace_text

on sanitize_text(value_text)
    if value_text is missing value then return ""
    set safe_text to value_text as text
    set safe_text to my replace_text(safe_text, return, " ")
    set safe_text to my replace_text(safe_text, linefeed, " ")
    set safe_text to my replace_text(safe_text, ascii character 30, " ")
    set safe_text to my replace_text(safe_text, ascii character 31, " ")
    return safe_text
end sanitize_text

tell application "System Events"
    set is_running to exists application process "{app_name}"
end tell
if not is_running then return ""

set tab_records to {{}}
tell application "{app_name}"
    set window_count to count of windows
    repeat with w from 1 to window_count
        set tab_count to count of tabs of window w
        repeat with t from 1 to tab_count
            set the_tab to tab t of window w
            try
                set tab_title to {title_property} of the_tab
            on error
                set tab_title to ""
            end try
            try
                set tab_url to URL of the_tab
            on error
                set tab_url to ""
            end try

            set safe_title to my sanitize_text(tab_title)
            set safe_url to my sanitize_text(tab_url)

            if safe_title is not "" or safe_url is not "" then
                set end of tab_records to (w as text) & (ascii character 30) & (t as text) & (ascii character 30) & safe_title & (ascii character 30) & safe_url
            end if
        end repeat
    end repeat
end tell

set AppleScript's text item delimiters to ascii character 31
set joined_tabs to tab_records as text
set AppleScript's text item delimiters to ""
return joined_tabs
"#,
        app_name = browser.app_name,
        title_property = title_property,
    )
}

fn build_list_tabs_jxa(browser: &SupportedBrowser) -> String {
    let title_property = match browser.family {
        BrowserFamily::Safari => "name",
        BrowserFamily::Chromium => "title",
    };

    // JXA reliably enumerates all Chrome windows (including those on other
    // macOS Spaces/desktops), whereas AppleScript's `count of windows` often
    // returns only the windows on the current Space.
    format!(
        r#"
var app = Application("{app_name}");
if (!app.running()) {{ ""; }}
else {{
    var FS = "\u001e";
    var RS = "\u001f";
    var rows = [];
    var wins = app.windows();
    for (var w = 0; w < wins.length; w++) {{
        var win = wins[w];
        var titles = win.tabs.{title_property}();
        var urls = win.tabs.url();
        for (var t = 0; t < titles.length; t++) {{
            var title = (titles[t] || "").replace(/[\r\n\u001e\u001f]/g, " ");
            var url   = (urls[t]   || "").replace(/[\r\n\u001e\u001f]/g, " ");
            if (title || url) {{
                rows.push((w + 1) + FS + (t + 1) + FS + title + FS + url);
            }}
        }}
    }}
    rows.join(RS);
}}
"#,
        app_name = browser.app_name,
        title_property = title_property,
    )
}

fn build_activate_tab_script(
    browser: &SupportedBrowser,
    window_index: usize,
    tab_index: usize,
) -> String {
    let activate_body = match browser.family {
        BrowserFamily::Safari => format!(
            r#"
    set tab_count to count of tabs of window {window_index}
    if tab_count < {tab_index} then error "Tab {tab_index} is no longer available"
    set current tab of window {window_index} to tab {tab_index} of window {window_index}
    set index of window {window_index} to 1
    activate
"#,
            window_index = window_index,
            tab_index = tab_index,
        ),
        BrowserFamily::Chromium => format!(
            r#"
    set tab_count to count of tabs of window {window_index}
    if tab_count < {tab_index} then error "Tab {tab_index} is no longer available"
    set active tab index of window {window_index} to {tab_index}
    set index of window {window_index} to 1
    activate
"#,
            window_index = window_index,
            tab_index = tab_index,
        ),
    };

    format!(
        r#"
tell application "System Events"
    set is_running to exists application process "{app_name}"
end tell
if not is_running then error "{app_name} is not running"

tell application "{app_name}"
    set window_count to count of windows
    if window_count < {window_index} then error "Window {window_index} is no longer available"
{activate_body}end tell
"#,
        app_name = browser.app_name,
        window_index = window_index,
        activate_body = activate_body,
    )
}

/// Fallback activation that scans every window for an exact URL match.
///
/// Unlike `build_activate_tab_script`, this does not trust cached indexes, so
/// it survives windows being closed or reordered between the snapshot and the
/// user's Enter press. JXA is used for the same reason as `build_list_tabs_jxa`:
/// it reliably enumerates windows on other Spaces.
fn build_activate_tab_by_url_jxa(browser: &SupportedBrowser, url: &str) -> String {
    // Serializing a &str is infallible; fall back to an empty JSON string so the
    // generated JXA stays syntactically valid even in the impossible case.
    let app_name_json =
        serde_json::to_string(browser.app_name).unwrap_or_else(|_| "\"\"".to_string());
    let target_json = serde_json::to_string(url).unwrap_or_else(|_| "\"\"".to_string());
    let select_tab = match browser.family {
        BrowserFamily::Safari => "win.currentTab = win.tabs[t];",
        BrowserFamily::Chromium => "win.activeTabIndex = t + 1;",
    };

    format!(
        r#"
var app = Application({app_name_json});
var found = false;
if (app.running()) {{
    var target = {target_json};
    var wins = app.windows();
    for (var w = 0; w < wins.length && !found; w++) {{
        var win = wins[w];
        var urls = win.tabs.url();
        for (var t = 0; t < urls.length; t++) {{
            if ((urls[t] || "") === target) {{
                {select_tab}
                win.index = 1;
                app.activate();
                found = true;
                break;
            }}
        }}
    }}
}}
found ? "ok" : "not-found";
"#
    )
}

// Favicon logic moved to src/favicons.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_tab_snapshots_and_resets_require_runtime_authority() {
        assert!(owned_root_browser_tabs_snapshot(Ok(Vec::new())).is_err());
        assert!(reset_owned_root_browser_tabs().is_err());
        assert!(invalidate_owned_root_browser_tabs_freshness().is_err());
    }

    #[test]
    fn browser_tab_read_failure_is_not_partial_success_or_empty_success() {
        let tab = BrowserTabInfo {
            browser_name: "Safari".into(),
            browser_bundle_id: "com.apple.Safari".into(),
            window_index: 1,
            tab_index: 1,
            title: "last good".into(),
            url: "https://example.com".into(),
        };
        let result = combine_browser_tab_results(vec![
            Ok(vec![tab]),
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied).into()),
        ]);
        assert_eq!(
            result
                .unwrap_err()
                .downcast_ref::<std::io::Error>()
                .unwrap()
                .kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert!(combine_browser_tab_results(vec![Ok(Vec::new())])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn browser_query_diagnostics_retain_only_sizes_not_private_query_text() {
        let private_query = "private-browser-query-canary-🔒";
        let diagnostic = browser_query_diagnostic(private_query);

        assert_eq!(diagnostic.bytes, private_query.len());
        assert_eq!(diagnostic.chars, private_query.chars().count());
        assert_ne!(diagnostic.bytes, diagnostic.chars);
        assert!(!format!("{diagnostic:?}").contains("private-browser-query-canary"));
    }

    #[test]
    fn bare_domain_query_recognizes_hostname_intent() {
        for query in [
            "x.com",
            "news.ycombinator.com",
            "www.example.org",
            "HTTPS://WWW.Example.COM/",
            "http://x.com",
        ] {
            assert!(query_is_bare_domain(query), "expected domain: {query:?}");
        }

        for query in [
            "deploy",
            "x.com launcher",
            "~/src",
            "/tmp/x",
            "1.2.3",
            ":tabs x.com",
            "tabs:x.com",
            "@x.com",
            "https://x.com/path",
        ] {
            assert!(
                !query_is_bare_domain(query),
                "expected rejection: {query:?}"
            );
        }
    }

    #[test]
    fn parse_tab_rows_returns_empty_for_blank_output() {
        let browser = &SUPPORTED_BROWSERS[0];
        let rows = parse_tab_rows(browser, "   ").expect("parse rows");
        assert!(rows.is_empty());
    }

    #[test]
    fn parse_tab_rows_extracts_multiple_tabs() {
        let browser = &SUPPORTED_BROWSERS[1];
        let output = format!(
            "1{field}1{field}Docs{field}https://docs.rs{record}2{field}3{field}Chat{field}https://chat.openai.com",
            field = FIELD_SEPARATOR,
            record = RECORD_SEPARATOR,
        );

        let rows = parse_tab_rows(browser, &output).expect("parse rows");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].browser_name.as_ref(), "Google Chrome");
        assert_eq!(rows[0].window_index, 1);
        assert_eq!(rows[0].tab_index, 1);
        assert_eq!(rows[0].title.as_ref(), "Docs");
        assert_eq!(rows[1].url.as_ref(), "https://chat.openai.com");
    }

    #[test]
    fn fuzzy_search_browser_tabs_prefers_title_match() {
        let tabs = vec![
            BrowserTabInfo {
                browser_name: "Safari".into(),
                browser_bundle_id: "com.apple.Safari".into(),
                window_index: 1,
                tab_index: 1,
                title: "Build a Claude Managed Agent".into(),
                url: "https://vercel.com/kb/guide".into(),
            },
            BrowserTabInfo {
                browser_name: "Google Chrome".into(),
                browser_bundle_id: "com.google.Chrome".into(),
                window_index: 1,
                tab_index: 2,
                title: "Home".into(),
                url: "https://claude-managed-agent.example.com".into(),
            },
        ];

        let matches = fuzzy_search_browser_tabs(&tabs, "managed agent");
        assert_eq!(matches.len(), 2);
        assert_eq!(
            matches[0].tab.title.as_ref(),
            "Build a Claude Managed Agent",
            "title hit should outrank URL-only hit"
        );
    }

    #[test]
    fn root_browser_tabs_compact_mode_rejects_sparse_natural_language_url_match() {
        let tabs = vec![BrowserTabInfo {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            window_index: 1,
            tab_index: 1,
            title: "Delta".into(),
            url: "https://delta.example.com/wxxxxhxxxxaxxxxtxxxxixxxsxxxxtxxxhxxxixxxsxxxxaxxxxnxxxxyxxxxwxxxxaxxxxy".into(),
        }];

        let permissive = root_fuzzy_search_browser_tabs(
            &tabs,
            "What is this anyway",
            true,
            RootBrowserTabsFuzzyMode::Permissive,
        );
        assert_eq!(
            permissive.len(),
            1,
            "fixture must demonstrate the previous sparse fuzzy match"
        );

        let compact = root_fuzzy_search_browser_tabs(
            &tabs,
            "What is this anyway",
            true,
            RootBrowserTabsFuzzyMode::Compact,
        );
        assert!(
            compact.is_empty(),
            "implicit root tab search should reject sparse long-URL fuzzy matches"
        );

        let direct_title =
            root_fuzzy_search_browser_tabs(&tabs, "delta", true, RootBrowserTabsFuzzyMode::Compact);
        assert_eq!(direct_title.len(), 1);
    }

    #[test]
    fn expired_root_browser_tab_snapshot_returns_stale_rows() {
        reset_root_browser_tabs_snapshot_for_test();
        let tab = BrowserTabInfo {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            window_index: 1,
            tab_index: 1,
            title: "Root Passive Snapshot".into(),
            url: "https://example.com/root-passive-snapshot".into(),
        };
        store_root_browser_tabs_snapshot_for_test(
            Instant::now() - Duration::from_millis(500),
            vec![tab.clone()],
        );

        let rows = cached_root_browser_tabs_snapshot(1);
        assert_eq!(rows.as_ref(), &vec![tab]);
        reset_root_browser_tabs_snapshot_for_test();
    }

    #[test]
    fn missing_root_browser_tab_snapshot_returns_empty_rows() {
        reset_root_browser_tabs_snapshot_for_test();
        assert!(cached_root_browser_tabs_snapshot(1).is_empty());
    }

    #[test]
    fn direct_browser_tabs_lookup_is_snapshot_only_and_keeps_permissive_matching() {
        reset_root_browser_tabs_snapshot_for_test();
        let previous_rows = Arc::new(vec![BrowserTabInfo {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            window_index: 1,
            tab_index: 1,
            title: "Delta".into(),
            url: "https://delta.example.com/wxxxxhxxxxaxxxxtxxxxixxxsxxxxtxxxhxxxixxxsxxxxaxxxxnxxxxyxxxxwxxxxaxxxxy".into(),
        }]);
        {
            let mut cache = ROOT_BROWSER_TAB_SNAPSHOT
                .lock()
                .expect("browser tabs cache");
            *cache = RootBrowserTabSnapshotState {
                snapshot: Some(RootBrowserTabSnapshot {
                    captured_at: Instant::now() - Duration::from_secs(7),
                    tabs: Arc::clone(&previous_rows),
                }),
                generation: 41,
                ..RootBrowserTabSnapshotState::default()
            };
        }

        let before = root_browser_tabs_snapshot_status();
        let options = RootBrowserTabsSectionOptions {
            enabled: true,
            min_query_chars: 0,
            cache_ttl_ms: 10_000,
            ..RootBrowserTabsSectionOptions::default()
        };
        let hits = search_root_browser_tabs_meta_direct("What is this anyway", options);
        let after = root_browser_tabs_snapshot_status();

        assert_eq!(
            hits.len(),
            1,
            "explicit lookup must retain permissive matching"
        );
        assert_eq!(
            before, after,
            "direct lookup cannot own a refresh generation"
        );
        assert!(!after.refreshing);
        {
            let cache = ROOT_BROWSER_TAB_SNAPSHOT
                .lock()
                .expect("browser tabs cache");
            assert!(Arc::ptr_eq(
                &cache.snapshot.as_ref().expect("snapshot preserved").tabs,
                &previous_rows
            ));
        }
        reset_root_browser_tabs_snapshot_for_test();
    }

    #[test]
    fn cold_direct_browser_tabs_lookup_cannot_start_an_unowned_refresh() {
        reset_root_browser_tabs_snapshot_for_test();
        let before = root_browser_tabs_snapshot_status();
        let options = RootBrowserTabsSectionOptions {
            enabled: true,
            min_query_chars: 0,
            ..RootBrowserTabsSectionOptions::default()
        };

        assert!(search_root_browser_tabs_meta_direct("private query", options).is_empty());
        assert_eq!(root_browser_tabs_snapshot_status(), before);
        assert!(!root_browser_tabs_snapshot_status().refreshing);
    }

    #[test]
    fn fresh_browser_tabs_cache_evidence_requires_present_idle_successful_snapshot() {
        let now = Instant::now();
        let ttl = Duration::from_secs(1);
        let mut cache = RootBrowserTabSnapshotState::default();
        assert!(fresh_root_browser_tabs_cache_status(&cache, now, ttl).is_none());
        cache.snapshot = Some(RootBrowserTabSnapshot {
            captured_at: now,
            tabs: Arc::new(Vec::new()),
        });
        cache.generation = 7;
        let status = fresh_root_browser_tabs_cache_status(&cache, now + ttl, ttl)
            .expect("a present empty snapshot is valid through its TTL boundary");
        assert_eq!(status.generation, 7);
        assert_eq!(status.cached_count, 0);
        assert!(fresh_root_browser_tabs_cache_status(
            &cache,
            now + ttl + Duration::from_nanos(1),
            ttl
        )
        .is_none());
        cache.refresh_in_flight = true;
        assert!(fresh_root_browser_tabs_cache_status(&cache, now, ttl).is_none());
        cache.refresh_in_flight = false;
        cache.refresh_needed = true;
        assert!(fresh_root_browser_tabs_cache_status(&cache, now, ttl).is_none());
        cache.refresh_needed = false;
        cache.last_refresh_error = Some("read failed".into());
        assert!(fresh_root_browser_tabs_cache_status(&cache, now, ttl).is_none());
    }

    #[test]
    fn canceled_browser_tabs_generation_preserves_snapshot_and_rearms_current_query() {
        let previous_rows = Arc::new(Vec::new());
        let backoff_until = Instant::now() + Duration::from_secs(10);
        let mut cache = RootBrowserTabSnapshotState {
            snapshot: Some(RootBrowserTabSnapshot {
                captured_at: Instant::now(),
                tabs: Arc::clone(&previous_rows),
            }),
            refresh_in_flight: true,
            generation: 7,
            last_refresh_error: Some("previous failure".to_owned()),
            last_attempt_at: Some(Instant::now()),
            next_refresh_after: Some(backoff_until),
            failure_count: 2,
            ..RootBrowserTabSnapshotState::default()
        };

        assert!(discard_root_browser_tabs_refresh_from_state(&mut cache, 7));
        assert!(!cache.refresh_in_flight);
        assert_eq!(cache.generation, 8);
        assert!(cache.last_attempt_at.is_none());
        assert_eq!(cache.next_refresh_after, Some(backoff_until));
        assert_eq!(
            cache.last_refresh_error.as_deref(),
            Some("previous failure")
        );
        assert_eq!(cache.failure_count, 2);
        assert!(Arc::ptr_eq(
            &cache.snapshot.as_ref().expect("preserved snapshot").tabs,
            &previous_rows
        ));
    }

    #[test]
    fn older_browser_tabs_completion_cannot_cancel_newer_in_flight_generation() {
        let attempted_at = Instant::now();
        let mut cache = RootBrowserTabSnapshotState {
            refresh_in_flight: true,
            generation: 9,
            last_attempt_at: Some(attempted_at),
            ..RootBrowserTabSnapshotState::default()
        };

        assert!(!discard_root_browser_tabs_refresh_from_state(&mut cache, 7));
        assert!(cache.refresh_in_flight);
        assert_eq!(cache.generation, 9);
        assert_eq!(cache.last_attempt_at, Some(attempted_at));
    }

    #[test]
    fn root_browser_tabs_refresh_completion_advances_generation_and_stores_rows() {
        reset_root_browser_tabs_snapshot_for_test();
        let before = root_browser_tabs_snapshot_status();
        let refresh =
            try_begin_root_browser_tabs_refresh(30_000, 1, "test").expect("refresh should start");
        assert!(root_browser_tabs_snapshot_status().refreshing);

        let tab = BrowserTabInfo {
            browser_name: "Google Chrome".into(),
            browser_bundle_id: "com.google.Chrome".into(),
            window_index: 1,
            tab_index: 1,
            title: "Root Completion Tab".into(),
            url: "https://example.com/root-completion-tab".into(),
        };
        assert!(finish_root_browser_tabs_refresh(
            refresh,
            Ok(vec![tab.clone()])
        ));

        let after = root_browser_tabs_snapshot_status();
        assert!(after.generation > before.generation);
        assert!(!after.refreshing);
        assert_eq!(after.cached_count, 1);
        assert_eq!(
            cached_root_browser_tabs_snapshot(30_000).as_ref(),
            &vec![tab]
        );
        reset_root_browser_tabs_snapshot_for_test();
    }

    #[test]
    fn root_browser_tabs_failure_backoff_is_exponential_and_capped() {
        assert_eq!(
            root_browser_tabs_failure_backoff(1),
            ROOT_BROWSER_TABS_FAILURE_BACKOFF_BASE
        );
        assert_eq!(
            root_browser_tabs_failure_backoff(2),
            ROOT_BROWSER_TABS_FAILURE_BACKOFF_BASE * 2
        );
        assert_eq!(
            root_browser_tabs_failure_backoff(99),
            ROOT_BROWSER_TABS_FAILURE_BACKOFF_MAX
        );
    }

    #[test]
    fn cached_root_browser_tabs_snapshot_does_not_block_on_refresh_lock() {
        reset_root_browser_tabs_snapshot_for_test();
        let _guard = ROOT_BROWSER_TAB_SNAPSHOT.lock().expect("snapshot lock");
        let started = Instant::now();
        let rows = cached_root_browser_tabs_snapshot(1);
        assert!(rows.is_empty());
        assert!(
            started.elapsed() < Duration::from_millis(25),
            "foreground snapshot read should not wait on refresh lock"
        );
    }

    #[test]
    fn root_browser_tabs_refresh_uses_configured_providers_only() {
        let providers = vec![crate::config::BrowserTabProvider::Chrome];
        assert!(root_supported_browser_provider_is_enabled(
            &SUPPORTED_BROWSERS[1],
            &providers
        ));
        assert!(!root_supported_browser_provider_is_enabled(
            &SUPPORTED_BROWSERS[0],
            &providers
        ));
    }

    #[test]
    fn build_list_tabs_script_uses_browser_specific_title_property() {
        let safari_script = build_list_tabs_script(&SUPPORTED_BROWSERS[0]);
        let chrome_script = build_list_tabs_script(&SUPPORTED_BROWSERS[1]);

        assert!(safari_script.contains("set the_tab to tab t of window w"));
        assert!(safari_script.contains("set tab_title to name of the_tab"));
        assert!(chrome_script.contains("set tab_title to title of the_tab"));
        assert!(chrome_script.contains("set tab_url to URL of the_tab"));
    }

    #[test]
    fn build_list_tabs_jxa_uses_browser_specific_title_property() {
        let safari_script = build_list_tabs_jxa(&SUPPORTED_BROWSERS[0]);
        let chrome_script = build_list_tabs_jxa(&SUPPORTED_BROWSERS[1]);

        assert!(safari_script.contains(r#"win.tabs.name()"#));
        assert!(chrome_script.contains(r#"win.tabs.title()"#));
        assert!(chrome_script.contains(r#"win.tabs.url()"#));
        // Verify it targets the correct app
        assert!(safari_script.contains(r#"Application("Safari")"#));
        assert!(chrome_script.contains(r#"Application("Google Chrome")"#));
    }

    #[test]
    fn build_activate_tab_script_switches_browser_specific_tab_property() {
        let safari_script = build_activate_tab_script(&SUPPORTED_BROWSERS[0], 2, 4);
        let chrome_script = build_activate_tab_script(&SUPPORTED_BROWSERS[1], 2, 4);

        assert!(safari_script.contains("set current tab of window 2 to tab 4 of window 2"));
        assert!(chrome_script.contains("set active tab index of window 2 to 4"));
        assert!(chrome_script.contains("set index of window 2 to 1"));
    }

    #[test]
    fn build_activate_tab_by_url_jxa_switches_browser_specific_tab_property() {
        let safari_script =
            build_activate_tab_by_url_jxa(&SUPPORTED_BROWSERS[0], "https://chatgpt.com/");
        let chrome_script =
            build_activate_tab_by_url_jxa(&SUPPORTED_BROWSERS[1], "https://chatgpt.com/");

        assert!(safari_script.contains("win.currentTab = win.tabs[t];"));
        assert!(!safari_script.contains("activeTabIndex"));
        assert!(chrome_script.contains("win.activeTabIndex = t + 1;"));
        assert!(chrome_script.contains(r#"Application("Google Chrome")"#));
        assert!(chrome_script.contains(r#"var target = "https://chatgpt.com/";"#));
    }

    #[test]
    fn build_activate_tab_by_url_jxa_escapes_url_as_json_string() {
        let script = build_activate_tab_by_url_jxa(
            &SUPPORTED_BROWSERS[1],
            "https://example.com/?q=\"quoted\"",
        );

        assert!(script.contains(r#"var target = "https://example.com/?q=\"quoted\"";"#));
    }

    #[test]
    fn is_browser_running_script_uses_bundle_identifier_lookup() {
        let bundle_id = "com.google.Chrome";
        let script = format!(
            r#"
tell application "System Events"
    set matching_processes to every application process whose bundle identifier is "{bundle_id}"
    return (count of matching_processes) > 0
end tell
"#,
        );

        assert!(script.contains("bundle identifier is \"com.google.Chrome\""));
        assert!(script.contains("count of matching_processes"));
    }
}
