use crate::ai::context_contract::ContextAttachmentKind;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpinePreviewNeeds {
    pub cheap_context: bool,
    pub browser_url: bool,
    pub selection_text: bool,
}

impl SpinePreviewNeeds {
    pub(crate) const CONTEXT_ROOT: Self = Self {
        cheap_context: true,
        browser_url: true,
        selection_text: true,
    };
    pub(crate) const STYLE: Self = Self {
        cheap_context: false,
        browser_url: false,
        selection_text: true,
    };
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SpineLivePreview {
    pub frontmost_app_name: Option<String>,
    pub active_window_title: Option<String>,
    pub browser_url: Option<String>,
    pub browser_url_is_focused: bool,
    pub browser_url_app_name: Option<String>,
    pub selection_text: Option<String>,
    /// True when `selection_text` is the focused field's whole text (no
    /// selection existed) rather than an explicit selection.
    pub selection_is_draft: bool,
    /// App the selection/draft preview was read from, when known.
    pub selection_source_app: Option<String>,
    pub clipboard_text: Option<String>,
    pub menu_bar_summary: Option<String>,
    pub script_count: Option<usize>,
}

#[derive(Debug)]
struct ExpensiveResult {
    browser_url: Option<String>,
    browser_url_is_focused: bool,
    browser_url_app_name: Option<String>,
    selection_text: Option<String>,
    selection_is_draft: bool,
    selection_source_app: Option<String>,
}

#[derive(Debug, Default)]
struct ExpensiveSlot {
    next_request_id: u64,
    in_flight: Option<(u64, std::time::Instant)>,
    ready: Option<(u64, ExpensiveResult)>,
}

/// How long a pasteboard read stays fresh before `refresh_cheap_fields`
/// touches the system clipboard again. The "cheap" refresh runs per
/// keystroke while the context root is open; without a TTL every keypress
/// performs a synchronous NSPasteboard round trip.
pub(crate) const CLIPBOARD_PREVIEW_TTL: std::time::Duration = std::time::Duration::from_millis(750);

#[derive(Debug)]
pub(crate) struct SpineLivePreviewCache {
    pub current: SpineLivePreview,
    pub generation: u64,
    last_expensive_kick: Option<std::time::Instant>,
    last_clipboard_read: Option<std::time::Instant>,
    pending_expensive: Arc<Mutex<ExpensiveSlot>>,
}

impl Default for SpineLivePreviewCache {
    fn default() -> Self {
        Self {
            current: SpineLivePreview::default(),
            generation: 0,
            last_expensive_kick: None,
            last_clipboard_read: None,
            pending_expensive: Arc::new(Mutex::new(ExpensiveSlot::default())),
        }
    }
}

impl SpineLivePreviewCache {
    pub(crate) fn refresh_cheap_fields(&mut self) {
        let tracked_app = crate::frontmost_app_tracker::get_last_real_app();

        let new_app = tracked_app.as_ref().map(|a| a.name.clone());
        let new_title = tracked_app.as_ref().and_then(|a| a.window_title.clone());

        let menu = crate::frontmost_app_tracker::get_cached_menu_summary();
        let new_menu = match (menu.app.as_ref(), menu.status) {
            (Some(app), crate::frontmost_app_tracker::CachedMenuStatus::Ready) => Some(format!(
                "{} \u{b7} {} menus cached",
                app.name, menu.item_count
            )),
            (Some(app), crate::frontmost_app_tracker::CachedMenuStatus::Fetching) => {
                Some(format!("{} \u{b7} menu loading\u{2026}", app.name))
            }
            (Some(app), crate::frontmost_app_tracker::CachedMenuStatus::NoCache) => {
                Some(format!("{} \u{b7} menu not cached yet", app.name))
            }
            (Some(app), crate::frontmost_app_tracker::CachedMenuStatus::StaleCacheHidden) => {
                Some(format!("{} \u{b7} menu cache stale", app.name))
            }
            _ => Some("No tracked app".to_string()),
        };

        let new_clipboard = self.clipboard_text_with_ttl(|| {
            arboard::Clipboard::new()
                .ok()
                .and_then(|mut cb| cb.get_text().ok())
                .filter(|t| !t.trim().is_empty())
        });

        if new_app != self.current.frontmost_app_name
            || new_title != self.current.active_window_title
            || new_menu != self.current.menu_bar_summary
            || new_clipboard != self.current.clipboard_text
        {
            self.current.frontmost_app_name = new_app;
            self.current.active_window_title = new_title;
            self.current.menu_bar_summary = new_menu;
            self.current.clipboard_text = new_clipboard;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    /// Return the cached clipboard preview while the last read is fresher
    /// than [`CLIPBOARD_PREVIEW_TTL`]; otherwise run `read` and restamp.
    fn clipboard_text_with_ttl(&mut self, read: impl FnOnce() -> Option<String>) -> Option<String> {
        if self
            .last_clipboard_read
            .is_some_and(|at| at.elapsed() < CLIPBOARD_PREVIEW_TTL)
        {
            return self.current.clipboard_text.clone();
        }
        self.last_clipboard_read = Some(std::time::Instant::now());
        read()
    }

    fn collect_pending_expensive(&mut self) {
        let result = self
            .pending_expensive
            .lock()
            .ok()
            .and_then(|mut slot| slot.ready.take().map(|(_, r)| r));
        if let Some(r) = result {
            let changed = r.browser_url != self.current.browser_url
                || r.browser_url_is_focused != self.current.browser_url_is_focused
                || r.browser_url_app_name != self.current.browser_url_app_name
                || r.selection_text != self.current.selection_text
                || r.selection_is_draft != self.current.selection_is_draft
                || r.selection_source_app != self.current.selection_source_app;
            if changed {
                self.current.browser_url = r.browser_url;
                self.current.browser_url_is_focused = r.browser_url_is_focused;
                self.current.browser_url_app_name = r.browser_url_app_name;
                self.current.selection_text = r.selection_text;
                self.current.selection_is_draft = r.selection_is_draft;
                self.current.selection_source_app = r.selection_source_app;
                self.generation = self.generation.wrapping_add(1);
            }
        }
    }

    /// Seed the selection preview from the show-time passive AX sniff so the
    /// header hint chip, style rows, and `@selection` submit-freeze all agree
    /// on the same captured text without re-reading AX.
    pub(crate) fn seed_selection_preview(
        &mut self,
        selection: Option<String>,
        is_draft: bool,
        source_app: Option<String>,
    ) {
        if self.current.selection_text != selection
            || self.current.selection_is_draft != is_draft
            || self.current.selection_source_app != source_app
        {
            self.current.selection_text = selection;
            self.current.selection_is_draft = is_draft;
            self.current.selection_source_app = source_app;
            self.generation = self.generation.wrapping_add(1);
        }
    }

    fn seed_style_preview_fixture(&mut self, needs: SpinePreviewNeeds) -> bool {
        if needs != SpinePreviewNeeds::STYLE
            || std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() != Some("1")
        {
            return false;
        }

        let Ok(selection_text) = std::env::var("SCRIPT_KIT_TEST_SPINE_STYLE_SELECTION_TEXT") else {
            return false;
        };
        let selection = (!selection_text.trim().is_empty()).then_some(selection_text);
        let is_draft = std::env::var("SCRIPT_KIT_TEST_SPINE_STYLE_SELECTION_KIND")
            .ok()
            .as_deref()
            == Some("draft");
        let source_app = std::env::var("SCRIPT_KIT_TEST_SPINE_STYLE_SOURCE_APP")
            .ok()
            .filter(|app| !app.trim().is_empty());
        self.seed_selection_preview(selection, is_draft, source_app);
        true
    }

    pub(crate) fn refresh_preview_nonblocking(&mut self, needs: SpinePreviewNeeds) {
        self.collect_pending_expensive();

        if needs.cheap_context {
            self.refresh_cheap_fields();
        }

        if self.seed_style_preview_fixture(needs) {
            return;
        }

        if !needs.browser_url && !needs.selection_text {
            return;
        }

        const THROTTLE: std::time::Duration = std::time::Duration::from_secs(2);
        const MAX_IN_FLIGHT: std::time::Duration = std::time::Duration::from_secs(8);

        if self
            .last_expensive_kick
            .is_some_and(|last| last.elapsed() < THROTTLE)
        {
            return;
        }

        let request_id = {
            let Ok(mut slot) = self.pending_expensive.lock() else {
                tracing::warn!(target: "script_kit::spine", "preview expensive slot poisoned");
                return;
            };
            if let Some((_, started_at)) = slot.in_flight {
                if started_at.elapsed() < MAX_IN_FLIGHT {
                    return;
                }
            }
            slot.next_request_id = slot.next_request_id.wrapping_add(1).max(1);
            let id = slot.next_request_id;
            slot.in_flight = Some((id, std::time::Instant::now()));
            id
        };

        self.last_expensive_kick = Some(std::time::Instant::now());

        let slot = Arc::clone(&self.pending_expensive);
        std::thread::spawn(move || {
            let (browser_url, browser_url_is_focused, browser_url_app_name) = if needs.browser_url {
                match crate::platform::get_any_browser_tab_url_with_source() {
                    Some(hit) => {
                        let is_focused = matches!(
                            hit.source,
                            crate::platform::BrowserUrlSource::FocusedBrowser
                        );
                        let app_name = match &hit.source {
                            crate::platform::BrowserUrlSource::RunningBrowserFallback {
                                app_name,
                            } => Some(app_name.clone()),
                            _ => None,
                        };
                        (Some(hit.url), is_focused, app_name)
                    }
                    None => (None, false, None),
                }
            } else {
                (None, false, None)
            };
            // Passive per-pid reads only: the system-wide focused element is
            // unreliable once our panel is key, and a passive preview must
            // never post keystrokes or touch the pasteboard. Selection first,
            // then the focused field's whole text ("draft").
            let (selection_text, selection_is_draft, selection_source_app) = if needs.selection_text
            {
                let source = crate::frontmost_app_tracker::get_last_real_app();
                let source_app = source.as_ref().map(|app| app.name.clone());
                let source_pid = source.map(|app| app.pid);
                match crate::platform::accessibility::focused_text::selected_text_for_app_ax_only(
                    source_pid,
                ) {
                    Ok(Some(selection)) => (Some(selection), false, source_app),
                    Ok(None) | Err(_) => {
                        match crate::platform::accessibility::focused_text::focused_text_for_app_ax_only(
                            source_pid,
                        ) {
                            Ok(Some(draft)) => (Some(draft), true, source_app),
                            Ok(None) => {
                                tracing::debug!(
                                    target: "script_kit::spine",
                                    event = "spine_preview_selection_ax_only_empty"
                                );
                                (None, false, source_app)
                            }
                            Err(error) => {
                                tracing::debug!(
                                    target: "script_kit::spine",
                                    event = "spine_preview_selection_ax_only_failed",
                                    error = %error
                                );
                                (None, false, source_app)
                            }
                        }
                    }
                }
            } else {
                (None, false, None)
            };
            let result = ExpensiveResult {
                browser_url,
                browser_url_is_focused,
                browser_url_app_name,
                selection_text,
                selection_is_draft,
                selection_source_app,
            };
            if let Ok(mut slot) = slot.lock() {
                if slot.in_flight.map(|(id, _)| id) == Some(request_id) {
                    slot.ready = Some((request_id, result));
                    slot.in_flight = None;
                }
            }
        });
    }

    pub(crate) fn set_script_count(&mut self, count: usize) {
        if self.current.script_count != Some(count) {
            self.current.script_count = Some(count);
            self.generation = self.generation.wrapping_add(1);
        }
    }
}

impl SpineLivePreview {
    /// Live data promoted to the row title. Returns `None` when the static
    /// label from `ContextAttachmentSpec.label` should be used as-is.
    pub(crate) fn title_for_context_kind(&self, kind: ContextAttachmentKind) -> Option<String> {
        match kind {
            ContextAttachmentKind::Current | ContextAttachmentKind::Full => None,
            ContextAttachmentKind::Selection => {
                if let Some(t) = &self.selection_text {
                    let preview = truncate_preview(t, 50);
                    if !preview.is_empty() {
                        return Some(format!("\u{201c}{preview}\u{201d}"));
                    }
                }
                None
            }
            ContextAttachmentKind::Browser => self
                .browser_url
                .as_ref()
                .map(|url| truncate_preview(url, 55)),
            ContextAttachmentKind::Window => {
                match (&self.active_window_title, &self.frontmost_app_name) {
                    (Some(title), Some(app)) if !title.trim().is_empty() => {
                        Some(format!("{} \u{b7} {}", truncate_preview(title, 40), app))
                    }
                    (Some(title), _) if !title.trim().is_empty() => {
                        Some(truncate_preview(title, 55))
                    }
                    (_, Some(app)) if !app.trim().is_empty() => Some(app.clone()),
                    _ => None,
                }
            }
            ContextAttachmentKind::Screenshot => self
                .frontmost_app_name
                .as_ref()
                .map(|app| format!("Screenshot \u{b7} {app}")),
            ContextAttachmentKind::Clipboard => {
                if let Some(t) = &self.clipboard_text {
                    let preview = truncate_preview(t, 50);
                    if !preview.is_empty() {
                        return Some(format!("\u{201c}{preview}\u{201d}"));
                    }
                }
                None
            }
            ContextAttachmentKind::FrontmostApp => self.frontmost_app_name.clone(),
            ContextAttachmentKind::MenuBar => self.menu_bar_summary.clone(),
            ContextAttachmentKind::RecentScripts => {
                self.script_count.map(|c| format!("{c} scripts indexed"))
            }
            _ => None,
        }
    }

    pub(crate) fn subtitle_for_context_kind(&self, kind: ContextAttachmentKind) -> String {
        match kind {
            ContextAttachmentKind::Current => {
                let mut parts = Vec::new();
                parts.push("screenshot");
                if let Some(app) = &self.frontmost_app_name {
                    parts.push(app.as_str());
                }
                if self.active_window_title.is_some() {
                    parts.push("focused window");
                }
                if self.browser_url.is_some() {
                    parts.push("browser URL");
                }
                format!("Includes {}", parts.join(" \u{b7} "))
            }
            ContextAttachmentKind::Full => {
                let mut parts = Vec::new();
                parts.push("screenshot");
                if self.clipboard_text.is_some() {
                    parts.push("clipboard");
                }
                if let Some(app) = &self.frontmost_app_name {
                    parts.push(app.as_str());
                }
                if self.browser_url.is_some() {
                    parts.push("browser");
                }
                if self.active_window_title.is_some() {
                    parts.push("window");
                }
                parts.push("menu bar");
                format!("Includes {}", parts.join(" \u{b7} "))
            }
            ContextAttachmentKind::Selection => {
                if self.selection_text.is_some() {
                    "Attach the selected text to your command".into()
                } else {
                    "No text selected \u{2014} the focused text will be captured when you submit"
                        .into()
                }
            }
            ContextAttachmentKind::Browser => {
                if self.browser_url.is_some() {
                    if self.browser_url_is_focused {
                        "Attach this page\u{2019}s URL to your command".into()
                    } else if let Some(app) = &self.browser_url_app_name {
                        format!("Attach URL from running {app}")
                    } else {
                        "Attach browser URL".into()
                    }
                } else {
                    "No supported browser focused".into()
                }
            }
            ContextAttachmentKind::Window => {
                "Attach the focused window info to your command".into()
            }
            ContextAttachmentKind::Diagnostics => {
                "Attach capture source health and permissions".into()
            }
            ContextAttachmentKind::Screenshot => {
                "Attach a screenshot of the screen behind Script Kit".into()
            }
            ContextAttachmentKind::Clipboard => {
                if self.clipboard_text.is_some() {
                    "Attach clipboard contents to your command".into()
                } else {
                    "Clipboard is empty".into()
                }
            }
            ContextAttachmentKind::FrontmostApp => {
                "Attach frontmost app info to your command".into()
            }
            ContextAttachmentKind::MenuBar => "Attach menu bar items to your command".into(),
            ContextAttachmentKind::RecentScripts => "Attach recent script invocations".into(),
            ContextAttachmentKind::GitStatus => "Attach current repository status".into(),
            ContextAttachmentKind::GitDiff => "Attach current repository diff".into(),
            ContextAttachmentKind::Processes => "Attach running processes snapshot".into(),
            ContextAttachmentKind::System => "Attach system, hardware, and battery info".into(),
            ContextAttachmentKind::Dictation => "Attach dictation transcript".into(),
            ContextAttachmentKind::Calendar => "Attach calendar events".into(),
            ContextAttachmentKind::Notifications => "Attach recent notifications".into(),
        }
    }

    pub(crate) fn style_target_subtitle(&self) -> String {
        let selection_app = self
            .selection_source_app
            .as_deref()
            .map(str::trim)
            .filter(|app| !app.is_empty());
        let frontmost_app = self
            .frontmost_app_name
            .as_deref()
            .map(str::trim)
            .filter(|app| !app.is_empty());
        let app = selection_app.or(frontmost_app);
        let has_readable_text = self
            .selection_text
            .as_deref()
            .is_some_and(|text| !text.trim().is_empty());

        match (has_readable_text, self.selection_is_draft, app) {
            (true, false, Some(app)) => format!("Rewrites your selection in {app}"),
            (true, true, Some(app)) => format!("Rewrites your draft in {app}"),
            (_, _, Some(app)) => format!("Rewrites the selected or focused text in {app}"),
            _ => "Rewrites the selected or focused text".to_string(),
        }
    }
}

fn truncate_preview(input: &str, max_chars: usize) -> String {
    super::text_preview::single_line_truncate(input, max_chars)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clipboard_read_is_skipped_within_ttl() {
        let mut cache = SpineLivePreviewCache::default();
        cache.current.clipboard_text = Some("cached".to_string());
        cache.last_clipboard_read = Some(std::time::Instant::now());

        let value = cache.clipboard_text_with_ttl(|| panic!("must not touch the pasteboard"));
        assert_eq!(value.as_deref(), Some("cached"));
    }

    #[test]
    fn clipboard_read_runs_after_ttl_expires() {
        let mut cache = SpineLivePreviewCache::default();
        cache.current.clipboard_text = Some("stale".to_string());
        cache.last_clipboard_read = Some(
            std::time::Instant::now() - CLIPBOARD_PREVIEW_TTL - std::time::Duration::from_millis(1),
        );

        let value = cache.clipboard_text_with_ttl(|| Some("fresh".to_string()));
        assert_eq!(value.as_deref(), Some("fresh"));
        assert!(cache
            .last_clipboard_read
            .is_some_and(|at| at.elapsed() < CLIPBOARD_PREVIEW_TTL));
    }

    #[test]
    fn style_target_subtitle_uses_closed_non_leaking_templates() {
        const SELECTION_SENTINEL: &str = "P01_SELECTION_BYTES_MUST_NOT_LEAK";
        const DRAFT_SENTINEL: &str = "P01_DRAFT_BYTES_MUST_NOT_LEAK";
        let cases = [
            (
                "selection",
                SpineLivePreview {
                    selection_text: Some(SELECTION_SENTINEL.to_string()),
                    selection_source_app: Some("Mail".to_string()),
                    ..Default::default()
                },
                "Rewrites your selection in Mail",
            ),
            (
                "draft",
                SpineLivePreview {
                    selection_text: Some(DRAFT_SENTINEL.to_string()),
                    selection_is_draft: true,
                    selection_source_app: Some("Notes".to_string()),
                    ..Default::default()
                },
                "Rewrites your draft in Notes",
            ),
            (
                "no-readable-text",
                SpineLivePreview {
                    frontmost_app_name: Some("Safari".to_string()),
                    ..Default::default()
                },
                "Rewrites the selected or focused text in Safari",
            ),
            (
                "unknown-app",
                SpineLivePreview {
                    selection_text: Some(SELECTION_SENTINEL.to_string()),
                    ..Default::default()
                },
                "Rewrites the selected or focused text",
            ),
        ];

        for (label, preview, expected) in cases {
            let subtitle = preview.style_target_subtitle();
            assert_eq!(subtitle, expected, "{label}");
            assert!(
                !subtitle.contains(SELECTION_SENTINEL),
                "{label} leaked selection bytes"
            );
            assert!(
                !subtitle.contains(DRAFT_SENTINEL),
                "{label} leaked draft bytes"
            );
        }
    }

    #[test]
    fn clipboard_read_runs_on_first_refresh() {
        let mut cache = SpineLivePreviewCache::default();
        let value = cache.clipboard_text_with_ttl(|| Some("first".to_string()));
        assert_eq!(value.as_deref(), Some("first"));
        assert!(cache.last_clipboard_read.is_some());
    }
}
