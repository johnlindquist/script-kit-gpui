use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::clipboard::write_plain_text_to_pasteboard;
use super::focused_text::{FocusedTextError, FocusedTextSessionId};
use super::metrics::TextMetrics;

/// In-memory focused-text targets used by fixture/test sessions that have no
/// real registered AX element (e.g. the `openFocusedTextAgentChatWith*Data`
/// devtools fixtures and `focused_text_snapshot_for_tests`). A fixture session
/// registers its captured text here; Replace/Append then mutate this buffer and
/// report a truthful `changed_text: true`, so the full capture → rewrite →
/// paste-back round-trip is exercisable end-to-end without a foreign app.
///
/// This is a genuine (in-memory) mutation target, not a faked receipt: the
/// buffer actually changes and can be read back via
/// [`in_memory_focused_text`]. Real captures register a real AX element and
/// never touch this map.
fn in_memory_focused_text_targets() -> &'static Mutex<HashMap<String, String>> {
    static TARGETS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    TARGETS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Register (or reset) an in-memory focused-text target for a fixture session.
pub fn register_in_memory_focused_text_target(session_id: &FocusedTextSessionId, initial: &str) {
    if let Ok(mut targets) = in_memory_focused_text_targets().lock() {
        targets.insert(session_id.to_string(), initial.to_string());
    }
}

/// Sessions whose capture came from the selection/Cmd+C ladder rather than a
/// registered AX element (AX-opaque apps like Chrome/Electron). Replace for
/// these sessions applies by pasting over the still-active selection in the
/// source app (clipboard-preserving Cmd+V) instead of an AX field write.
fn selection_paste_targets() -> &'static Mutex<std::collections::HashSet<String>> {
    static TARGETS: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    TARGETS.get_or_init(|| Mutex::new(std::collections::HashSet::new()))
}

/// Mark a session as a selection-paste target (see [`selection_paste_targets`]).
pub fn register_selection_paste_target(session_id: &FocusedTextSessionId) {
    if let Ok(mut targets) = selection_paste_targets().lock() {
        targets.insert(session_id.to_string());
    }
}

/// Whether Replace for this session applies via paste-over-selection.
pub fn is_selection_paste_target(session_id: &FocusedTextSessionId) -> bool {
    selection_paste_targets()
        .lock()
        .map(|targets| targets.contains(&session_id.to_string()))
        .unwrap_or(false)
}

/// Read the current contents of an in-memory fixture target, if any.
pub fn in_memory_focused_text(session_id: &FocusedTextSessionId) -> Option<String> {
    in_memory_focused_text_targets()
        .lock()
        .ok()
        .and_then(|targets| targets.get(&session_id.to_string()).cloned())
}

/// Apply a mutation to an in-memory fixture target. Returns `None` when the
/// session is not an in-memory fixture (callers then fall through to the real
/// AX mutation path).
fn mutate_in_memory_target(
    session_id: &FocusedTextSessionId,
    action: TextMutationAction,
    text: &str,
) -> Option<TextMutationResult> {
    let mut targets = in_memory_focused_text_targets().lock().ok()?;
    let current = targets.get(&session_id.to_string())?;
    let next = match action {
        TextMutationAction::Append => format!("{current}{text}"),
        // Replace (and Copy, which never reaches here) overwrite the buffer.
        _ => text.to_string(),
    };
    let changed = next != *current;
    targets.insert(session_id.to_string(), next);
    if changed {
        crate::runtime_policy::record_completed_fixture_effect();
    }
    Some(TextMutationResult {
        action,
        changed_text: changed,
        copied_to_clipboard: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TextMutationOptions {
    pub allow_stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMutationResult {
    pub action: TextMutationAction,
    pub changed_text: bool,
    pub copied_to_clipboard: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMutationAction {
    Replace,
    Append,
    Copy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FocusedTextMutationSession {
    pub session_id: FocusedTextSessionId,
    pub captured_at_ms: u128,
    pub current_text: Option<String>,
    pub ttl_ms: u128,
}

impl FocusedTextMutationSession {
    pub fn is_stale_at(&self, now_ms: u128) -> bool {
        now_ms.saturating_sub(self.captured_at_ms) > self.ttl_ms
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppendMutationPlan {
    DirectSet {
        text: String,
        metrics: TextMetrics,
    },
    PasteOutputAtEnd {
        output: String,
        output_metrics: TextMetrics,
    },
    SelectAllAndPaste {
        text: String,
        metrics: TextMetrics,
    },
}

pub fn validate_mutation_session(
    session: &FocusedTextMutationSession,
    options: TextMutationOptions,
    now_ms: u128,
) -> Result<(), FocusedTextError> {
    if session.is_stale_at(now_ms) && !options.allow_stale {
        return Err(FocusedTextError::StaleSession);
    }
    Ok(())
}

pub fn plan_append_mutation(
    current_text: Option<&str>,
    output: &str,
    can_set_value_directly: bool,
    can_set_caret_to_end: bool,
) -> AppendMutationPlan {
    match (current_text, can_set_value_directly, can_set_caret_to_end) {
        (Some(current), true, _) => {
            let text = format!("{current}{output}");
            AppendMutationPlan::DirectSet {
                metrics: TextMetrics::from_text(&text),
                text,
            }
        }
        (None, true, true) => AppendMutationPlan::PasteOutputAtEnd {
            output: output.to_string(),
            output_metrics: TextMetrics::from_text(output),
        },
        (None, true, false) => AppendMutationPlan::SelectAllAndPaste {
            text: output.to_string(),
            metrics: TextMetrics::from_text(output),
        },
        (_, false, true) => AppendMutationPlan::PasteOutputAtEnd {
            output: output.to_string(),
            output_metrics: TextMetrics::from_text(output),
        },
        (Some(current), false, false) => {
            let text = format!("{current}{output}");
            AppendMutationPlan::SelectAllAndPaste {
                metrics: TextMetrics::from_text(&text),
                text,
            }
        }
        (None, false, false) => AppendMutationPlan::SelectAllAndPaste {
            text: output.to_string(),
            metrics: TextMetrics::from_text(output),
        },
    }
}

pub fn replace_focused_text(
    session_id: FocusedTextSessionId,
    text: &str,
    options: TextMutationOptions,
) -> Result<TextMutationResult, FocusedTextError> {
    if let Some(result) = mutate_in_memory_target(&session_id, TextMutationAction::Replace, text) {
        return Ok(result);
    }
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| FocusedTextError::Platform(error.to_string()))?;

    if is_selection_paste_target(&session_id) {
        #[cfg(target_os = "macos")]
        {
            let _ = options;
            return crate::selected_text::set_selected_text(text)
                .map(|_| TextMutationResult {
                    action: TextMutationAction::Replace,
                    changed_text: true,
                    copied_to_clipboard: false,
                })
                .map_err(|err| FocusedTextError::Platform(err.to_string()));
        }
        #[cfg(not(target_os = "macos"))]
        {
            return Err(FocusedTextError::UnsupportedTarget);
        }
    }

    #[cfg(target_os = "macos")]
    {
        super::ax::replace_registered_focused_text(&session_id, text, options, current_time_ms())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(FocusedTextError::UnsupportedTarget)
    }
}

pub fn append_focused_text(
    session_id: FocusedTextSessionId,
    text: &str,
    options: TextMutationOptions,
) -> Result<TextMutationResult, FocusedTextError> {
    if let Some(result) = mutate_in_memory_target(&session_id, TextMutationAction::Append, text) {
        return Ok(result);
    }
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| FocusedTextError::Platform(error.to_string()))?;

    #[cfg(target_os = "macos")]
    {
        super::ax::append_registered_focused_text(&session_id, text, options, current_time_ms())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = options;
        Err(FocusedTextError::UnsupportedTarget)
    }
}

pub fn copy_text_output(text: &str) -> Result<TextMutationResult, FocusedTextError> {
    write_plain_text_to_pasteboard(text)
        .map_err(|err| FocusedTextError::Platform(err.to_string()))?;
    Ok(TextMutationResult {
        action: TextMutationAction::Copy,
        changed_text: false,
        copied_to_clipboard: true,
    })
}

fn current_time_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

#[cfg(test)]
mod in_memory_target_tests {
    use super::*;

    // QA stories #3 / #20: fixture focused-text sessions register an in-memory
    // mutable target so Replace/Append produce a truthful `changed_text: true`
    // receipt — the paste-back round-trip is exercisable without a foreign app.
    #[test]
    fn replace_mutates_registered_in_memory_target_and_reports_changed() {
        let id = FocusedTextSessionId::new_for_tests("ft-in-memory-replace");
        register_in_memory_focused_text_target(&id, "original draft");
        let result = replace_focused_text(
            id.clone(),
            "Friendlier rewrite",
            TextMutationOptions::default(),
        )
        .expect("replace against in-memory fixture target must succeed");
        assert_eq!(result.action, TextMutationAction::Replace);
        assert!(result.changed_text, "replace must report changed_text");
        assert!(!result.copied_to_clipboard);
        assert_eq!(
            in_memory_focused_text(&id).as_deref(),
            Some("Friendlier rewrite"),
            "the in-memory buffer must actually hold the new text"
        );
    }

    #[test]
    fn selection_paste_registry_marks_sessions() {
        let id = FocusedTextSessionId::new_for_tests("ft-selection-paste");
        assert!(!is_selection_paste_target(&id));
        register_selection_paste_target(&id);
        assert!(is_selection_paste_target(&id));
        let other = FocusedTextSessionId::new_for_tests("ft-selection-paste-other");
        assert!(!is_selection_paste_target(&other));
    }

    #[test]
    fn selection_paste_snapshot_replaces_but_never_appends() {
        let snapshot = super::super::focused_text::focused_text_snapshot_for_selection_paste(
            "picked in Chrome".to_string(),
            Some("Google Chrome".to_string()),
        );
        assert!(snapshot.capabilities.can_replace);
        assert!(
            !snapshot.capabilities.can_append,
            "paste-over-selection cannot append"
        );
        assert!(snapshot.capabilities.can_copy);
        assert_eq!(snapshot.app.name, "Google Chrome");
        assert!(
            is_selection_paste_target(&snapshot.session_id),
            "constructor must register the paste-apply session"
        );
    }

    #[test]
    fn append_concatenates_in_memory_target() {
        let id = FocusedTextSessionId::new_for_tests("ft-in-memory-append");
        register_in_memory_focused_text_target(&id, "abc");
        let result = append_focused_text(id.clone(), "def", TextMutationOptions::default())
            .expect("append against in-memory fixture target must succeed");
        assert_eq!(result.action, TextMutationAction::Append);
        assert!(result.changed_text);
        assert_eq!(in_memory_focused_text(&id).as_deref(), Some("abcdef"));
    }

    #[test]
    fn replace_with_identical_text_reports_unchanged() {
        let id = FocusedTextSessionId::new_for_tests("ft-in-memory-noop");
        register_in_memory_focused_text_target(&id, "same");
        let result = replace_focused_text(id, "same", TextMutationOptions::default())
            .expect("replace must succeed even when text is unchanged");
        assert!(
            !result.changed_text,
            "identical replacement must report changed_text=false"
        );
    }

    #[test]
    fn unregistered_session_is_not_treated_as_in_memory() {
        // A session never registered as in-memory must fall through to the real
        // (AX) path rather than silently succeeding.
        let id = FocusedTextSessionId::new_for_tests("ft-never-registered");
        assert!(in_memory_focused_text(&id).is_none());
    }
}
