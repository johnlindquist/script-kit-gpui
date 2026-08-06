use crate::dictation::{DictationTarget, FrozenDictationDestination};

fn fingerprint(value: &str) -> String {
    crate::dictation::redacted_transcript_fingerprint(value)
}

#[cfg(target_os = "macos")]
fn matching_external_windows(pid: i32) -> Result<Vec<(u32, String, bool)>, String> {
    use xcap::Window;

    let mut matches = Vec::new();
    for window in Window::all().map_err(|error| format!("cannot enumerate windows: {error}"))? {
        if window.pid().ok() != u32::try_from(pid).ok() || window.is_minimized().unwrap_or(true) {
            continue;
        }
        let width = window.width().unwrap_or(0);
        let height = window.height().unwrap_or(0);
        if width < 100 || height < 100 {
            continue;
        }
        let id = window
            .id()
            .map_err(|error| format!("cannot identify target window: {error}"))?;
        matches.push((
            id,
            window.title().unwrap_or_default(),
            window.is_focused().unwrap_or(false),
        ));
    }
    Ok(matches)
}

fn choose_external_window<'a>(
    windows: &'a [(u32, String, bool)],
    tracked_title: Option<&str>,
) -> Result<&'a (u32, String, bool), String> {
    if let Some(title) = tracked_title {
        let matching = windows
            .iter()
            .filter(|(_, candidate, _)| candidate == title)
            .collect::<Vec<_>>();
        if matching.len() == 1 {
            return Ok(matching[0]);
        }
    } else {
        let focused = windows
            .iter()
            .filter(|(_, _, focused)| *focused)
            .collect::<Vec<_>>();
        if focused.len() == 1 {
            return Ok(focused[0]);
        }
        if windows.len() == 1 {
            return Ok(&windows[0]);
        }
    }
    Err(
        "The previous app's exact window cannot be identified; choose another destination"
            .to_string(),
    )
}

#[cfg(target_os = "macos")]
pub fn capture_frozen_external_destination() -> Result<FrozenDictationDestination, String> {
    let app = crate::frontmost_app_tracker::get_last_real_app()
        .ok_or_else(|| "No previously focused app is available".to_string())?;
    let windows = matching_external_windows(app.pid)?;
    let selected = choose_external_window(&windows, app.window_title.as_deref())?;
    let bundle_fingerprint = fingerprint(&app.bundle_id);
    let window_identity_fingerprint = format!(
        "cgwindow:{}:{}",
        selected.0,
        fingerprint(&format!("{}:{}:{}", app.pid, app.bundle_id, selected.0))
    );
    Ok(FrozenDictationDestination::ExternalApp {
        pid: app.pid,
        bundle_fingerprint,
        window_identity_fingerprint,
        display_label: app.name,
        icon_identity: Some(app.bundle_id),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn capture_frozen_external_destination() -> Result<FrozenDictationDestination, String> {
    Err("Exact external-window identity is only available on macOS".to_string())
}

pub fn validate_frozen_external_destination(
    destination: &FrozenDictationDestination,
) -> Result<(), String> {
    let FrozenDictationDestination::ExternalApp {
        pid,
        bundle_fingerprint,
        window_identity_fingerprint,
        ..
    } = destination
    else {
        return Err("Destination is not an external app".to_string());
    };
    let app = crate::frontmost_app_tracker::get_last_real_app()
        .filter(|app| app.pid == *pid && fingerprint(&app.bundle_id) == *bundle_fingerprint)
        .ok_or_else(|| "The selected app is no longer available".to_string())?;
    #[cfg(target_os = "macos")]
    {
        let expected_id = window_identity_fingerprint
            .strip_prefix("cgwindow:")
            .and_then(|value| value.split(':').next())
            .and_then(|value| value.parse::<u32>().ok())
            .ok_or_else(|| "The selected window identity is malformed".to_string())?;
        let still_present = matching_external_windows(*pid)?
            .into_iter()
            .any(|(id, _, _)| id == expected_id);
        if !still_present {
            return Err("The selected app window is no longer available".to_string());
        }
        let expected = format!(
            "cgwindow:{}:{}",
            expected_id,
            fingerprint(&format!("{}:{}:{}", pid, app.bundle_id, expected_id))
        );
        if expected != *window_identity_fingerprint {
            return Err("The selected app window identity changed".to_string());
        }
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationDeliveryTargetResolution {
    Deliver {
        target: DictationTarget,
        source: DictationDeliveryTargetSource,
    },
    Refuse(DictationWrongTargetRefusalDraft),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationDeliveryTargetSource {
    ExplicitLabel,
    ActiveSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationWrongTargetReason {
    UnknownTargetLabel,
    TargetUnavailable,
    TargetStale,
}

impl DictationWrongTargetReason {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::UnknownTargetLabel => "unknownTargetLabel",
            Self::TargetUnavailable => "targetUnavailable",
            Self::TargetStale => "targetStale",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationWrongTargetRefusalDraft {
    pub reason: DictationWrongTargetReason,
    pub requested_target_label: Option<String>,
    pub requested_target: Option<DictationTarget>,
    pub delivery_generation_before: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictationTargetLabelResolution {
    pub target: DictationTarget,
    pub migrated_legacy_ai_chat: bool,
}

pub fn resolve_dictation_target_label(label: &str) -> Option<DictationTargetLabelResolution> {
    let normalized = label
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect::<String>();

    let target = match normalized.as_str() {
        "mainwindowfilter" | "scriptkit" | "launcher" | "filter" => {
            DictationTarget::MainWindowFilter
        }
        "mainwindowprompt" | "prompt" => DictationTarget::MainWindowPrompt,
        "noteseditor" | "notes" => DictationTarget::NotesEditor,
        "aichatcomposer" | "aichat" | "legacyai" => {
            return Some(DictationTargetLabelResolution {
                target: DictationTarget::TabAiHarness,
                migrated_legacy_ai_chat: true,
            });
        }
        "tabaiharness" | "agentchat" | "agentchatchat" | "ai" => DictationTarget::TabAiHarness,
        "externalapp" | "frontmostapp" | "frontmost" | "app" => DictationTarget::ExternalApp,
        "daypagetoday" | "daypage" | "today" | "todaynote" => DictationTarget::DayPageToday,
        "quickaiquestion" | "quickai" | "ask" | "askai" => DictationTarget::QuickAiQuestion,
        _ => return None,
    };

    Some(DictationTargetLabelResolution {
        target,
        migrated_legacy_ai_chat: false,
    })
}

pub fn parse_dictation_target_label(label: &str) -> Option<DictationTarget> {
    resolve_dictation_target_label(label).map(|resolution| resolution.target)
}

pub fn resolve_delivery_target_request(
    explicit_label: Option<&str>,
    active_session_target: Option<DictationTarget>,
    delivery_generation_before: u64,
) -> DictationDeliveryTargetResolution {
    if let Some(label) = explicit_label {
        return match parse_dictation_target_label(label) {
            Some(target) => DictationDeliveryTargetResolution::Deliver {
                target,
                source: DictationDeliveryTargetSource::ExplicitLabel,
            },
            None => DictationDeliveryTargetResolution::Refuse(DictationWrongTargetRefusalDraft {
                reason: DictationWrongTargetReason::UnknownTargetLabel,
                requested_target_label: Some(label.to_string()),
                requested_target: None,
                delivery_generation_before,
            }),
        };
    }

    if let Some(target) = active_session_target {
        return DictationDeliveryTargetResolution::Deliver {
            target,
            source: DictationDeliveryTargetSource::ActiveSession,
        };
    }

    DictationDeliveryTargetResolution::Refuse(DictationWrongTargetRefusalDraft {
        reason: DictationWrongTargetReason::TargetUnavailable,
        requested_target_label: None,
        requested_target: None,
        delivery_generation_before,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_external_window_selection_refuses_ambiguous_titles() {
        let windows = vec![
            (11, "Document".to_string(), false),
            (12, "Document".to_string(), false),
        ];
        assert!(choose_external_window(&windows, Some("Document")).is_err());
    }

    #[test]
    fn exact_external_window_selection_uses_unique_title_or_focus() {
        let windows = vec![
            (11, "First".to_string(), false),
            (12, "Second".to_string(), true),
        ];
        assert_eq!(
            choose_external_window(&windows, Some("First")).unwrap().0,
            11
        );
        assert_eq!(choose_external_window(&windows, None).unwrap().0, 12);
    }

    #[test]
    fn missing_explicit_and_active_target_refuses_without_ui_fallback() {
        assert!(matches!(
            resolve_delivery_target_request(None, None, 4),
            DictationDeliveryTargetResolution::Refuse(DictationWrongTargetRefusalDraft {
                reason: DictationWrongTargetReason::TargetUnavailable,
                ..
            })
        ));
    }
}
