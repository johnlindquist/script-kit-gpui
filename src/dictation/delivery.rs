use crate::dictation::DictationTarget;

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
    UiFallback,
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
    pub fallback_target: Option<DictationTarget>,
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
    ui_fallback_target: DictationTarget,
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
                fallback_target: Some(active_session_target.unwrap_or(ui_fallback_target)),
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

    DictationDeliveryTargetResolution::Deliver {
        target: ui_fallback_target,
        source: DictationDeliveryTargetSource::UiFallback,
    }
}
