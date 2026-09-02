use crate::dictation::{DictationTarget, FrozenDictationDestination};

pub fn dictation_mutation_error_outcome(
    detail: &str,
) -> crate::dictation::DictationDeliveryOutcome {
    if detail.starts_with("mutation_failed:") {
        crate::dictation::DictationDeliveryOutcome::Failed {
            failure: crate::ai::reliability::destination_failure(false, detail),
            reason: crate::dictation::DictationDeliveryFailureReason::MutationOutcomeUnknown,
            retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
        }
    } else {
        crate::dictation::DictationDeliveryOutcome::Refused {
            failure: crate::ai::reliability::destination_failure(true, detail),
            reason: crate::dictation::DictationDeliveryFailureReason::DestinationStale,
        }
    }
}

pub fn with_frozen_dictation_destination<T>(
    expected: &crate::dictation::FrozenDictationDestination,
    current: &crate::dictation::FrozenDictationDestination,
    mutate: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    if expected != current {
        return Err("The frozen Dictation destination changed".into());
    }
    mutate()
}

pub fn dictation_outcome_from_insertion_range(
    request: &crate::dictation::DictationDeliveryRequest,
    range: &serde_json::Value,
) -> Result<crate::dictation::DictationDeliveryOutcome, String> {
    let observed = |key: &str| {
        range
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("Missing observed Dictation mutation {key}"))
    };
    let start = observed("start")?;
    let end = observed("end")?;
    let inserted_length = observed("insertedLength")?;
    if end.checked_sub(start) != Some(inserted_length) {
        return Err("Inconsistent observed Dictation mutation range".into());
    }
    Ok(crate::dictation::DictationDeliveryOutcome::Delivered {
        destination: request.selection.destination.clone(),
        mutation_receipt: crate::dictation::DictationMutationReceipt {
            delivery_id: request.delivery_id,
            destination_kind: request.selection.destination.kind(),
            identity_fingerprint: request.selection.destination.identity_fingerprint(),
            insertion_start: Some(start),
            insertion_end: Some(end),
            inserted_length,
            duplicate: false,
        },
    })
}
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
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::SystemDiscovery)
        .map_err(|error| error.to_string())?;
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
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::NativeInput)
        .map_err(|error| error.to_string())?;
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
#[cfg(test)]
mod dictation_delivery_coordinator_tests {
    use super::{
        dictation_mutation_error_outcome, dictation_outcome_from_insertion_range,
        with_frozen_dictation_destination,
    };
    use crate::dictation::{
        DictationDeliveryOutcome, DictationDeliveryRequest, DictationTarget,
        DictationTargetSelection, FrozenDictationDestination, ImmutableDictationTranscript,
    };

    fn owner() -> crate::dictation::FrozenMainDictationOwner {
        crate::dictation::FrozenMainDictationOwner {
            root_entity_id: 1,
            window_generation: Some(2),
            surface_generation: 3,
            visibility_generation: 4,
        }
    }

    #[test]
    fn failed_persistence_after_edit_is_not_a_safe_pre_mutation_refusal() {
        assert!(matches!(
            dictation_mutation_error_outcome("stale_destination: replaced host"),
            DictationDeliveryOutcome::Refused {
                reason: crate::dictation::DictationDeliveryFailureReason::DestinationStale,
                ..
            }
        ));
        assert!(matches!(
            dictation_mutation_error_outcome("mutation_failed: canonical save failed"),
            DictationDeliveryOutcome::Failed {
                reason: crate::dictation::DictationDeliveryFailureReason::MutationOutcomeUnknown,
                retry_safety: sk_protocol::ai_reliability::RetrySafety::Never,
                ..
            }
        ));
    }

    #[test]
    fn stale_reopened_and_retargeted_destinations_never_enter_mutation() {
        let frozen = FrozenDictationDestination::MainWindowPrompt {
            prompt_id: "same-id".into(),
            prompt_generation: Some(7),
            input_generation: 11,
            prompt_entity_id: Some(5),
            owner: owner(),
        };
        let changed = [
            FrozenDictationDestination::MainWindowPrompt {
                prompt_id: "same-id".into(),
                prompt_generation: Some(7),
                input_generation: 12,
                prompt_entity_id: Some(5),
                owner: owner(),
            },
            FrozenDictationDestination::MainWindowPrompt {
                prompt_id: "same-id".into(),
                prompt_generation: Some(8),
                input_generation: 11,
                prompt_entity_id: Some(5),
                owner: owner(),
            },
            FrozenDictationDestination::MainWindowPrompt {
                prompt_id: "other-id".into(),
                prompt_generation: Some(7),
                input_generation: 11,
                prompt_entity_id: Some(5),
                owner: owner(),
            },
            FrozenDictationDestination::MainWindowFilter {
                owner: owner(),
                input_generation: 11,
            },
        ];
        let mut text = String::from("untouched");
        for current in changed {
            assert!(with_frozen_dictation_destination(&frozen, &current, || {
                text.push_str(" dictated");
                Ok(())
            })
            .is_err());
            assert_eq!(text, "untouched");
        }
        for changed_owner in [
            crate::dictation::FrozenMainDictationOwner {
                root_entity_id: 2,
                ..owner()
            },
            crate::dictation::FrozenMainDictationOwner {
                window_generation: Some(3),
                ..owner()
            },
            crate::dictation::FrozenMainDictationOwner {
                window_generation: None,
                ..owner()
            },
            crate::dictation::FrozenMainDictationOwner {
                surface_generation: 4,
                ..owner()
            },
            crate::dictation::FrozenMainDictationOwner {
                visibility_generation: 5,
                ..owner()
            },
        ] {
            let current = FrozenDictationDestination::MainWindowPrompt {
                prompt_id: "same-id".into(),
                prompt_generation: Some(7),
                input_generation: 11,
                prompt_entity_id: Some(5),
                owner: changed_owner,
            };
            assert!(with_frozen_dictation_destination(&frozen, &current, || {
                text.push_str(" wrong lifetime");
                Ok(())
            })
            .is_err());
            assert_eq!(text, "untouched");
        }
        with_frozen_dictation_destination(&frozen, &frozen, || {
            text.push_str(" dictated");
            Ok(())
        })
        .unwrap();
        assert_eq!(text, "untouched dictated");
    }

    fn request() -> DictationDeliveryRequest {
        DictationDeliveryRequest {
            delivery_id: 8,
            session_generation: 4,
            attempt: 1,
            history_entry_id: String::new(),
            selection: DictationTargetSelection {
                target: DictationTarget::MainWindowFilter,
                destination: FrozenDictationDestination::MainWindowFilter {
                    owner: owner(),
                    input_generation: 9,
                },
                display_label: "Filter".into(),
                icon_identity: None,
                selection_generation: 6,
            },
            transcript: ImmutableDictationTranscript::new("transcript", "one\r\ntwo".into()),
        }
    }

    #[test]
    fn mutation_receipt_reports_observed_sink_range_not_transcript_length() {
        let request = request();
        let outcome = dictation_outcome_from_insertion_range(
            &request,
            &serde_json::json!({
                "start": 0, "end": 7, "insertedLength": 7,
            }),
        )
        .unwrap();
        let DictationDeliveryOutcome::Delivered {
            destination,
            mutation_receipt,
        } = outcome
        else {
            panic!("expected delivered");
        };
        assert_eq!(destination, request.selection.destination);
        assert_eq!(mutation_receipt.delivery_id, request.delivery_id);
        assert_eq!(mutation_receipt.inserted_length, 7);
        assert_ne!(mutation_receipt.inserted_length, request.transcript.len());
        assert!(!mutation_receipt.duplicate);
    }

    #[test]
    fn missing_or_inconsistent_mutation_evidence_cannot_report_delivery() {
        for range in [
            serde_json::json!({}),
            serde_json::json!({"start": 4, "end": 3, "insertedLength": 1}),
            serde_json::json!({"start": 0, "end": 3, "insertedLength": 8}),
        ] {
            assert!(dictation_outcome_from_insertion_range(&request(), &range).is_err());
        }
    }
}
