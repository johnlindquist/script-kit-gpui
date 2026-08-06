use std::time::Duration;

use sk_protocol::ai_reliability::RetrySafety;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DictationDeviceId(pub String);

/// Transport type for an audio input device.
///
/// Used to rank devices when the user has no explicit preference: built-in
/// microphones are preferred over USB/Bluetooth/virtual devices as a safe
/// first-launch default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DictationDeviceTransport {
    BuiltIn,
    Usb,
    Bluetooth,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationDeviceInfo {
    pub id: DictationDeviceId,
    pub name: String,
    pub is_default: bool,
    pub transport: DictationDeviceTransport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationCaptureConfig {
    pub sample_rate_hz: u32,
    pub chunk_duration: Duration,
    pub level_window: Duration,
}

impl Default for DictationCaptureConfig {
    fn default() -> Self {
        Self {
            sample_rate_hz: 16_000,
            chunk_duration: Duration::from_millis(40),
            level_window: Duration::from_millis(60),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawAudioChunk {
    pub sample_rate_hz: u32,
    pub channels: u16,
    pub samples: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapturedAudioChunk {
    pub sample_rate_hz: u32,
    pub samples: Vec<f32>,
    pub duration: Duration,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DictationCaptureEvent {
    Chunk(CapturedAudioChunk),
    /// FFT-derived frequency-domain bar levels (0.0–1.0 each, 9 bars).
    Bars([f32; 9]),
    EndOfStream,
}

// --- Capture completion types ---

/// Audio data returned when dictation recording is stopped.
///
/// Contains the collected audio chunks and their total duration.  The caller
/// is responsible for transcription and delivery — the runtime only captures.
#[derive(Debug, Clone, PartialEq)]
pub struct CompletedDictationCapture {
    pub chunks: Vec<CapturedAudioChunk>,
    pub audio_duration: Duration,
    /// `true` when collection hit its deadline before `EndOfStream`, meaning
    /// the tail of the recording may be missing from `chunks`.
    pub truncated: bool,
}

/// Outcome of a `toggle_dictation()` call.
#[derive(Debug, Clone, PartialEq)]
pub enum DictationToggleOutcome {
    /// A new recording session was started.
    Started,
    /// An active recording was stopped.  `Some(capture)` when audio was
    /// collected, `None` for an empty recording.
    Stopped(Option<CompletedDictationCapture>),
}

// --- Session / transcription types ---

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationDestination {
    MainWindowFilter,
    ActivePrompt,
    FrontmostApp,
    NotesEditor,
    AiChatComposer,
    TabAiHarness,
    /// Appended to today's Day Page as a timestamped capture line.
    DayPageToday,
    /// Submitted to the AI window as a question (answer mode) or staged in
    /// its composer (composer mode), per `dictation.quickAi` config.
    QuickAiQuestion,
}

/// The Script Kit surface that was active when dictation was invoked.
///
/// Determined at dictation start time and stored in the session so the
/// transcript delivery path knows where to route without re-inspecting
/// the UI (which may have changed while the user was speaking).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationTarget {
    /// The shared launcher/main-menu filter in the primary window (`AppView::ScriptList`).
    MainWindowFilter,
    /// A prompt in the main window that accepts text input (arg, path,
    /// select, env, template, form, file search, mini, micro).
    MainWindowPrompt,
    /// The notes window editor.
    NotesEditor,
    /// Legacy AI composer target retained only for persisted-history/config migration.
    AiChatComposer,
    /// The embedded Agent Chat composer.
    TabAiHarness,
    /// No internal Script Kit surface was active — deliver to the
    /// frontmost external app via simulated paste.
    ExternalApp,
    /// Append the transcript to today's Day Page as a timestamped capture.
    /// Never resolved implicitly — chosen via overlay chip, config, or an
    /// explicit delivery label.
    DayPageToday,
    /// Treat the transcript as a question for the AI window: fire-and-show
    /// the answer, or stage it in the composer, per `dictation.quickAi`.
    /// Never resolved implicitly — chosen via overlay chip, config, or an
    /// explicit delivery label.
    QuickAiQuestion,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrozenAgentChatPolicy {
    ExistingThread { thread_id: String, generation: u64 },
    FreshStandard { host_generation: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrozenDictationDestination {
    MainWindowFilter {
        window_generation: u64,
        input_generation: u64,
    },
    MainWindowPrompt {
        prompt_id: String,
        prompt_generation: u64,
        input_generation: u64,
    },
    NotesEditor {
        notes_instance_id: u64,
        document_id: String,
        editor_generation: String,
        insertion_anchor: std::ops::Range<usize>,
    },
    AgentChat {
        policy: FrozenAgentChatPolicy,
    },
    ExternalApp {
        pid: i32,
        bundle_fingerprint: String,
        window_identity_fingerprint: String,
        display_label: String,
        icon_identity: Option<String>,
    },
    DayPage {
        date: chrono::NaiveDate,
        substrate_fingerprint: String,
        entity_generation: u64,
    },
    QuickAi {
        request_generation: u64,
    },
}

impl FrozenDictationDestination {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::MainWindowFilter { .. } => "mainWindowFilter",
            Self::MainWindowPrompt { .. } => "mainWindowPrompt",
            Self::NotesEditor { .. } => "notesEditor",
            Self::AgentChat { .. } => "agentChat",
            Self::ExternalApp { .. } => "externalApp",
            Self::DayPage { .. } => "dayPage",
            Self::QuickAi { .. } => "quickAi",
        }
    }

    pub fn external_window_id(&self) -> Option<u32> {
        let Self::ExternalApp {
            window_identity_fingerprint,
            ..
        } = self
        else {
            return None;
        };
        window_identity_fingerprint
            .strip_prefix("cgwindow:")?
            .split(':')
            .next()?
            .parse()
            .ok()
    }

    pub fn identity_fingerprint(&self) -> String {
        let identity = match self {
            Self::MainWindowFilter {
                window_generation,
                input_generation,
            } => format!("filter:{window_generation}:{input_generation}"),
            Self::MainWindowPrompt {
                prompt_id,
                prompt_generation,
                input_generation,
            } => format!("prompt:{prompt_id}:{prompt_generation}:{input_generation}"),
            Self::NotesEditor {
                notes_instance_id,
                document_id,
                editor_generation,
                insertion_anchor,
            } => format!(
                "notes:{notes_instance_id}:{document_id}:{editor_generation}:{}:{}",
                insertion_anchor.start, insertion_anchor.end
            ),
            Self::AgentChat { policy } => match policy {
                FrozenAgentChatPolicy::ExistingThread {
                    thread_id,
                    generation,
                } => format!("agent-chat:existing:{thread_id}:{generation}"),
                FrozenAgentChatPolicy::FreshStandard { host_generation } => {
                    format!("agent-chat:fresh:{host_generation}")
                }
            },
            Self::ExternalApp {
                pid,
                bundle_fingerprint,
                window_identity_fingerprint,
                ..
            } => format!("external:{pid}:{bundle_fingerprint}:{window_identity_fingerprint}"),
            Self::DayPage {
                date,
                substrate_fingerprint,
                entity_generation,
            } => format!("day:{date}:{substrate_fingerprint}:{entity_generation}"),
            Self::QuickAi { request_generation } => format!("quick-ai:{request_generation}"),
        };
        crate::dictation::redacted_transcript_fingerprint(&identity)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationTargetSelection {
    pub target: DictationTarget,
    pub destination: FrozenDictationDestination,
    pub display_label: String,
    pub icon_identity: Option<String>,
    pub selection_generation: u64,
}

impl DictationTargetSelection {
    pub fn is_compatible_with(&self, target: DictationTarget) -> bool {
        let canonical = match target {
            DictationTarget::AiChatComposer => DictationTarget::TabAiHarness,
            target => target,
        };
        let selected = match self.target {
            DictationTarget::AiChatComposer => DictationTarget::TabAiHarness,
            target => target,
        };
        canonical == selected
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ImmutableDictationTranscript {
    id: String,
    text: String,
    fingerprint: String,
}

impl std::fmt::Debug for ImmutableDictationTranscript {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImmutableDictationTranscript")
            .field("id", &self.id)
            .field("text_len", &self.text.len())
            .field("fingerprint", &self.fingerprint)
            .finish()
    }
}

impl ImmutableDictationTranscript {
    pub fn new(id: impl Into<String>, text: String) -> Self {
        let fingerprint = crate::dictation::redacted_transcript_fingerprint(&text);
        Self {
            id: id.into(),
            text,
            fingerprint,
        }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationDeliveryRequest {
    pub delivery_id: u64,
    pub session_generation: u64,
    pub selection: DictationTargetSelection,
    pub transcript: ImmutableDictationTranscript,
    pub history_entry_id: String,
    pub attempt: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationDeliveryFailureReason {
    DestinationUnavailable,
    DestinationStale,
    MutationFailed,
    MutationOutcomeUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationMutationReceipt {
    pub delivery_id: u64,
    pub destination_kind: &'static str,
    pub identity_fingerprint: String,
    pub insertion_start: Option<usize>,
    pub insertion_end: Option<usize>,
    pub inserted_length: usize,
    pub duplicate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationDeliveryOutcome {
    Delivered {
        destination: FrozenDictationDestination,
        mutation_receipt: DictationMutationReceipt,
    },
    Refused {
        failure: crate::ai::reliability::AppFailureRecord,
        reason: DictationDeliveryFailureReason,
    },
    Failed {
        failure: crate::ai::reliability::AppFailureRecord,
        reason: DictationDeliveryFailureReason,
        retry_safety: RetrySafety,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationTargetPersistenceClass {
    Contextual,
    Sticky,
    LegacyReadOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DictationAutoSubmitPermission {
    Never,
    ExplicitSend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictationRecoveryCapabilities {
    pub retry: bool,
    pub copy_transcript: bool,
    pub retarget: bool,
}

impl DictationRecoveryCapabilities {
    const STANDARD: Self = Self {
        retry: true,
        copy_transcript: true,
        retarget: true,
    };

    const LEGACY: Self = Self {
        retry: false,
        copy_transcript: true,
        retarget: false,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DictationTargetDescriptor {
    pub target: DictationTarget,
    pub stable_id: &'static str,
    pub selector_label: &'static str,
    pub badge_label: &'static str,
    pub icon: &'static str,
    pub delivery_verb: &'static str,
    pub description: &'static str,
    pub persistence_class: DictationTargetPersistenceClass,
    pub requires_frozen_identity: bool,
    pub auto_submit_permission: DictationAutoSubmitPermission,
    pub recovery_capabilities: DictationRecoveryCapabilities,
    pub quick_chip: bool,
    pub quick_chip_label: Option<&'static str>,
    pub quick_chip_order: Option<u8>,
    pub selectable: bool,
}

pub const ALL_DICTATION_TARGETS: [DictationTargetDescriptor; 8] = [
    DictationTargetDescriptor {
        target: DictationTarget::MainWindowFilter,
        stable_id: "filter",
        selector_label: "Script Kit",
        badge_label: "Script Kit",
        icon: "search",
        delivery_verb: "Insert",
        description: "Insert the transcript into the Script Kit filter",
        persistence_class: DictationTargetPersistenceClass::Contextual,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: false,
        quick_chip_label: None,
        quick_chip_order: None,
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::MainWindowPrompt,
        stable_id: "prompt",
        selector_label: "Prompt",
        badge_label: "Prompt",
        icon: "text-cursor-input",
        delivery_verb: "Insert",
        description: "Insert the transcript into the active prompt",
        persistence_class: DictationTargetPersistenceClass::Contextual,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: false,
        quick_chip_label: None,
        quick_chip_order: None,
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::NotesEditor,
        stable_id: "notes",
        selector_label: "Notes",
        badge_label: "Notes",
        icon: "notebook-tabs",
        delivery_verb: "Append",
        description: "Append the transcript to the active note",
        persistence_class: DictationTargetPersistenceClass::Sticky,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: false,
        quick_chip_label: None,
        quick_chip_order: None,
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::AiChatComposer,
        stable_id: "aichat",
        selector_label: "AI Chat (legacy)",
        badge_label: "AI",
        icon: "bot",
        delivery_verb: "Stage",
        description: "Legacy AI composer destination; migrated to Agent Chat",
        persistence_class: DictationTargetPersistenceClass::LegacyReadOnly,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::LEGACY,
        quick_chip: false,
        quick_chip_label: None,
        quick_chip_order: None,
        selectable: false,
    },
    DictationTargetDescriptor {
        target: DictationTarget::TabAiHarness,
        stable_id: "agentchat",
        selector_label: "Agent Chat",
        badge_label: "Agent",
        icon: "bot",
        delivery_verb: "Send",
        description: "Send the transcript to Agent Chat",
        persistence_class: DictationTargetPersistenceClass::Sticky,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::ExplicitSend,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: true,
        quick_chip_label: Some("Send"),
        quick_chip_order: Some(3),
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::ExternalApp,
        stable_id: "frontmost",
        selector_label: "Frontmost App",
        badge_label: "App",
        icon: "clipboard-paste",
        delivery_verb: "Paste",
        description: "Paste the transcript into the frontmost app",
        persistence_class: DictationTargetPersistenceClass::Sticky,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: true,
        quick_chip_label: Some("Paste"),
        quick_chip_order: Some(0),
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::DayPageToday,
        stable_id: "today",
        selector_label: "Today",
        badge_label: "Today",
        icon: "calendar-days",
        delivery_verb: "Append",
        description: "Append the transcript to today's note",
        persistence_class: DictationTargetPersistenceClass::Sticky,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::Never,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: true,
        quick_chip_label: Some("Today"),
        quick_chip_order: Some(1),
        selectable: true,
    },
    DictationTargetDescriptor {
        target: DictationTarget::QuickAiQuestion,
        stable_id: "ask",
        selector_label: "Ask AI",
        badge_label: "Ask AI",
        icon: "sparkles",
        delivery_verb: "Ask",
        description: "Ask Quick AI with the transcript",
        persistence_class: DictationTargetPersistenceClass::Sticky,
        requires_frozen_identity: true,
        auto_submit_permission: DictationAutoSubmitPermission::ExplicitSend,
        recovery_capabilities: DictationRecoveryCapabilities::STANDARD,
        quick_chip: true,
        quick_chip_label: Some("Ask"),
        quick_chip_order: Some(2),
        selectable: true,
    },
];

impl DictationTarget {
    pub fn descriptor(self) -> &'static DictationTargetDescriptor {
        match self {
            Self::MainWindowFilter => &ALL_DICTATION_TARGETS[0],
            Self::MainWindowPrompt => &ALL_DICTATION_TARGETS[1],
            Self::NotesEditor => &ALL_DICTATION_TARGETS[2],
            Self::AiChatComposer => &ALL_DICTATION_TARGETS[3],
            Self::TabAiHarness => &ALL_DICTATION_TARGETS[4],
            Self::ExternalApp => &ALL_DICTATION_TARGETS[5],
            Self::DayPageToday => &ALL_DICTATION_TARGETS[6],
            Self::QuickAiQuestion => &ALL_DICTATION_TARGETS[7],
        }
    }

    pub fn action_descriptors() -> impl Iterator<Item = &'static DictationTargetDescriptor> {
        ALL_DICTATION_TARGETS
            .iter()
            .filter(|descriptor| descriptor.selectable)
    }

    pub fn quick_chip_descriptors() -> impl Iterator<Item = &'static DictationTargetDescriptor> {
        (0_u8..4).filter_map(|order| {
            ALL_DICTATION_TARGETS.iter().find(|descriptor| {
                descriptor.quick_chip
                    && descriptor.selectable
                    && descriptor.quick_chip_order == Some(order)
            })
        })
    }

    /// The delivery destination recorded in receipts for this target.
    ///
    /// Single source of truth for the `DictationTarget` →
    /// `DictationDestination` mapping so the delivery pipeline and receipts
    /// cannot drift.
    pub fn destination(self) -> DictationDestination {
        match self {
            Self::MainWindowFilter => DictationDestination::MainWindowFilter,
            Self::MainWindowPrompt => DictationDestination::ActivePrompt,
            Self::NotesEditor => DictationDestination::NotesEditor,
            Self::AiChatComposer => DictationDestination::AiChatComposer,
            Self::TabAiHarness => DictationDestination::TabAiHarness,
            Self::ExternalApp => DictationDestination::FrontmostApp,
            Self::DayPageToday => DictationDestination::DayPageToday,
            Self::QuickAiQuestion => DictationDestination::QuickAiQuestion,
        }
    }

    /// Canonical lowercase token persisted as `dictation.lastTarget` and
    /// accepted back by `parse_dictation_target_label` — the two must stay
    /// round-trippable.
    pub fn sticky_label(self) -> &'static str {
        self.descriptor().stable_id
    }

    /// Short, stable label for the overlay destination badge.
    pub fn overlay_label(self) -> &'static str {
        self.descriptor().badge_label
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationSessionPhase {
    Idle,
    Recording,
    /// Escape pressed during a long recording (>= 5s) — overlay shows
    /// Stop/Discard/Continue affordances.  Enter or clicking Stop finishes the
    /// recording and transcribes it; Backspace or clicking Discard cancels the
    /// session; Escape or clicking Continue dismisses confirmation and
    /// recording keeps running.
    Confirming,
    Transcribing,
    Delivering,
    Finished,
    Failed(String),
}

impl DictationSessionPhase {
    /// Stable camelCase identifier for automation receipts.
    ///
    /// The `Failed(_)` variant collapses to the single token `"failed"` — the
    /// inner message is not part of the automation contract.
    pub fn as_automation_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Recording => "recording",
            Self::Confirming => "confirming",
            Self::Transcribing => "transcribing",
            Self::Delivering => "delivering",
            Self::Finished => "finished",
            Self::Failed(_) => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictationSessionResult {
    pub transcript: String,
    pub destination: DictationDestination,
    pub audio_duration: Duration,
}

// --- Model availability ---

/// Whether the dictation engine's model is ready to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictationModelStatus {
    /// Model files are present and ready.
    Available,
    /// Model is not downloaded yet.
    NotDownloaded,
    /// Model is currently being downloaded.
    Downloading {
        percentage: u8,
        /// Bytes downloaded so far (0 when unknown).
        downloaded_bytes: u64,
        /// Total expected bytes (0 when unknown).
        total_bytes: u64,
        /// Transfer speed in bytes/sec (0 when not yet measured).
        speed_bytes_per_sec: u64,
        /// Estimated seconds remaining, or `None` when not enough data exists yet.
        eta_seconds: Option<u64>,
    },
    /// Model is being extracted from the archive.
    Extracting,
    /// Download or extraction failed.
    DownloadFailed(String),
}
