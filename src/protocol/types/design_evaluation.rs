//! Data-only messages for the owned production evaluator.

use serde::{Deserialize, Serialize};

pub const MAX_LIVE_THEME_EDITS: usize = 16;

/// Transport-only opt-in. The decoded response keeps its original protocol identity.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum OwnedResponseEncoding {
    #[serde(rename = "zlib-json-base64-v1")]
    ZlibJsonBase64V1,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnedResponseCodec {
    pub version: u8,
    pub encoding: OwnedResponseEncoding,
    pub request_field: &'static str,
    pub response_type: &'static str,
    pub delivery: &'static str,
    pub max_decoded_bytes: usize,
    pub max_compressed_bytes: usize,
}

/// The compressed bound leaves base64 and header room inside the existing 6 MiB line limit.
/// Each opted request gets one independently compressed complete response, including refusals.
/// Unrequested lifecycle observations and requests without the field remain unencoded.
pub const OWNED_RESPONSE_CODEC: OwnedResponseCodec = OwnedResponseCodec {
    version: 1,
    encoding: OwnedResponseEncoding::ZlibJsonBase64V1,
    request_field: "responseEncoding",
    response_type: "encodedResponse",
    delivery: "always",
    max_decoded_bytes: 6 * 1024 * 1024,
    max_compressed_bytes: 4 * 1024 * 1024,
};

/// Only runtime-editable values belong here. Locked native motion/material and
/// glyph calibration values are intentionally not deserializable operations.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tokenId", content = "value", deny_unknown_fields)]
pub enum LiveThemeEdit {
    #[serde(rename = "theme.colors.accent.selected")]
    Accent(u32),
    #[serde(rename = "theme.colors.background.main")]
    MainBackground(u32),
    #[serde(rename = "theme.colors.background.searchBox")]
    SearchBackground(u32),
    #[serde(rename = "theme.colors.ui.error")]
    ErrorColor(u32),
    #[serde(rename = "theme.opacity.hover")]
    Hover(f32),
    #[serde(rename = "theme.opacity.selected")]
    Selected(f32),
    #[serde(rename = "theme.opacity.textStrong")]
    TextStrong(f32),
    #[serde(rename = "theme.opacity.textMutedAlpha")]
    TextMuted(f32),
    #[serde(rename = "theme.opacity.textHint")]
    TextHint(f32),
    #[serde(rename = "theme.opacity.textPlaceholder")]
    TextPlaceholder(f32),
    #[serde(rename = "theme.opacity.textIcon")]
    TextIcon(f32),
}

impl LiveThemeEdit {
    pub const fn id(&self) -> &'static str {
        match self {
            Self::Accent(_) => "theme.colors.accent.selected",
            Self::MainBackground(_) => "theme.colors.background.main",
            Self::SearchBackground(_) => "theme.colors.background.searchBox",
            Self::ErrorColor(_) => "theme.colors.ui.error",
            Self::Hover(_) => "theme.opacity.hover",
            Self::Selected(_) => "theme.opacity.selected",
            Self::TextStrong(_) => "theme.opacity.textStrong",
            Self::TextMuted(_) => "theme.opacity.textMutedAlpha",
            Self::TextHint(_) => "theme.opacity.textHint",
            Self::TextPlaceholder(_) => "theme.opacity.textPlaceholder",
            Self::TextIcon(_) => "theme.opacity.textIcon",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedRuntimeIdentity {
    pub pid: u32,
    pub process_start_time: String,
    pub process_instance_id: String,
    pub session_generation: String,
    pub binary_sha256: String,
    pub manifest_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CompletedFrameIdentity {
    #[serde(flatten)]
    pub runtime: OwnedRuntimeIdentity,
    pub requested_target: super::AutomationWindowTarget,
    pub target: super::AutomationTargetIdentitySnapshot,
    pub native_window_id: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ObservedEffect {
    StateChanged {
        owner: String,
        revision: u64,
    },
    SubmissionDelivered {
        owner: String,
        receipt_id: String,
        prompt_instance_id: String,
        delivery_count: u64,
    },
    PopupOpened {
        target: super::AutomationWindowTarget,
    },
    PopupClosed {
        target: super::AutomationWindowTarget,
    },
    RootClosed {
        target: super::AutomationWindowTarget,
    },
    NoOp {
        reason: String,
    },
    Refused {
        code: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopedActionReceipt {
    pub request_id: String,
    pub operation_id: String,
    pub before: super::AutomationTargetIdentitySnapshot,
    pub after: Option<super::AutomationTargetIdentitySnapshot>,
    pub dispatch_completed: bool,
    pub was_deferred: bool,
    pub effect: ObservedEffect,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvaluationLimits {
    pub max_windows: u32,
    pub max_requests: u32,
    pub max_frames: u32,
    pub max_lifetime_ms: u64,
    pub max_image_pixels: u32,
    pub max_png_bytes: u32,
    pub max_retained_images: u32,
    pub max_log_bytes: u32,
}

pub const OWNED_EVALUATION_LIMITS: EvaluationLimits = EvaluationLimits {
    max_windows: 8,
    max_requests: 4096,
    max_frames: 2048,
    max_lifetime_ms: 600_000,
    max_image_pixels: 4_194_304,
    max_png_bytes: 4_194_304,
    max_retained_images: 8,
    max_log_bytes: 8_388_608,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FixtureDescriptor {
    pub id: String,
    pub family: String,
    pub root: String,
    pub owner: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub factory_owners: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_fixture_id: Option<String>,
    pub proof_boundary: String,
    pub native_exclusions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_view_variant: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation_owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub surface_variant: Option<String>,
    pub expected_semantic_surface: String,
    pub required_semantic_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum AgentChatFixtureCommand {
    Submit { text: String },
    MutateInputBeforePaint { text: String },
    Retry {},
    Stop {},
    HoldDrain {},
    RetainDrain {},
    ReleaseDrain { turn_generation: u64 },
    EmitText { turn_generation: u64, text: String },
    Complete { turn_generation: u64 },
    Fail { turn_generation: u64 },
    OpenHistory {},
    OpenSlashPicker {},
    OpenProfilePicker {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FlowFixtureCommand {
    Submit {
        session_id: u64,
        text: String,
    },
    Retry {
        session_id: u64,
    },
    Stop {
        session_id: u64,
    },
    Background {
        session_id: u64,
    },
    Resume {
        session_id: u64,
    },
    EmitText {
        session_id: u64,
        message_id: String,
        text: String,
    },
    Complete {
        session_id: u64,
        message_id: String,
    },
    Fail {
        session_id: u64,
        message_id: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SdkChatFixtureCommand {
    Submit { text: String },
    Retry {},
    Stop {},
    EmitText { message_id: String, text: String },
    Complete { message_id: String },
    Fail { message_id: String },
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DictationFixtureDestination {
    MainFilter,
    MainPrompt,
    Notes,
    AgentChat,
    DayPage,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DictationFixtureCommand {
    Begin {
        destination: DictationFixtureDestination,
    },
    Recording {
        text: String,
        bars: [f32; 9],
    },
    Confirm {},
    Resume {},
    Transcribe {},
    Deliver {},
    Retarget {
        destination: DictationFixtureDestination,
    },
    OpenMicrophonePicker {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum NotesFixtureCommand {
    ToggleTask {
        marker_start: usize,
        marker_end: usize,
        checked: bool,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SearchFixtureCommand {
    Prepare {
        scenario: String,
    },
    Release {
        #[serde(deserialize_with = "deserialize_search_run_ids")]
        run_ids: Vec<u64>,
    },
    Advance {
        milliseconds: u32,
    },
}

pub const MAX_SEARCH_RELEASE_RUNS: usize = 128;

pub(crate) fn validate_search_run_ids(run_ids: &[u64]) -> Result<(), &'static str> {
    if run_ids.is_empty() || run_ids.len() > MAX_SEARCH_RELEASE_RUNS {
        return Err("search_release_count_out_of_bounds");
    }
    let mut seen = std::collections::HashSet::with_capacity(run_ids.len());
    if run_ids.iter().any(|id| !seen.insert(*id)) {
        return Err("search_release_duplicate_run");
    }
    Ok(())
}

fn deserialize_search_run_ids<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<u64>, D::Error> {
    let run_ids = Vec::<u64>::deserialize(deserializer)?;
    validate_search_run_ids(&run_ids).map_err(serde::de::Error::custom)?;
    Ok(run_ids)
}

/// Read-only continuation within one retained owned-frame trace lifetime.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedFrameCursor {
    pub trace_generation: u64,
    pub after_frame_generation: u64,
}

impl OwnedFrameCursor {
    pub(crate) fn validate(
        self,
        trace_generation: u64,
        retired_before_frame_generation: u64,
        latest_frame_generation: u64,
    ) -> Result<(), &'static str> {
        if self.trace_generation != trace_generation {
            return Err("frame_cursor_stale");
        }
        if self.after_frame_generation < retired_before_frame_generation {
            return Err("frame_cursor_retired");
        }
        if self.after_frame_generation > latest_frame_generation {
            return Err("frame_cursor_future");
        }
        Ok(())
    }
}

/// This extension belongs to owned getState, not the ordinary protocol message.
/// Presence is deliberate: explicit null must not become an omitted cursor.
pub(crate) fn parse_owned_frame_cursor(
    request: &serde_json::Value,
) -> Result<Option<OwnedFrameCursor>, &'static str> {
    let Some(cursor) = request.get("frameCursor") else {
        return Ok(None);
    };
    if request["type"].as_str() != Some("getState") {
        return Err("frame_cursor_invalid");
    }
    serde_json::from_value(cursor.clone())
        .map(Some)
        .map_err(|_| "frame_cursor_invalid")
}

fn deserialize_owned_frame_cursor<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<OwnedFrameCursor>, D::Error> {
    OwnedFrameCursor::deserialize(deserializer)
        .map(Some)
        .map_err(|_| serde::de::Error::custom("frame_cursor_invalid"))
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum OwnedSearchProviderSource {
    Files,
    Directory,
    BrainLexical,
    BrainSemantic,
    Tabs,
    History,
    Windows,
    Icons,
    Notes,
    Todos,
    Clipboard,
    Dictation,
    Conversations,
    Spine,
    BrainInbox,
    Scripts,
    Apps,
    Skills,
    Validation,
    FlowRoster,
}

impl OwnedSearchProviderSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Files => "files",
            Self::Directory => "directory",
            Self::BrainLexical => "brain-lexical",
            Self::BrainSemantic => "brain-semantic",
            Self::Tabs => "tabs",
            Self::History => "history",
            Self::Windows => "windows",
            Self::Icons => "icons",
            Self::Notes => "notes",
            Self::Todos => "todos",
            Self::Clipboard => "clipboard",
            Self::Dictation => "dictation",
            Self::Conversations => "conversations",
            Self::Spine => "spine",
            Self::BrainInbox => "brain-inbox",
            Self::Scripts => "scripts",
            Self::Apps => "apps",
            Self::Skills => "skills",
            Self::Validation => "validation",
            Self::FlowRoster => "flow-roster",
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OwnedSearchQueryStamp {
    pub lifetime: u64,
    pub revision: u64,
    pub scope_revision: u64,
}

/// Owned-only waitFor extension; ordinary application wait semantics are unchanged.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OwnedSearchProviderCondition {
    SearchProvider {
        source: OwnedSearchProviderSource,
        query: OwnedSearchQueryStamp,
        after_run_id: u64,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        accept_cached: bool,
    },
}

/// Waits on the real FileSearch worker generation, independently of root search providers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OwnedFileSearchStreamCondition {
    FileSearchStream { generation: u64, query: String },
}

/// Waits for the exact FileSearch decoder result to be held before delivery.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum OwnedFileSearchPreviewCondition {
    FileSearchPreview {
        generation: std::num::NonZeroU64,
        query: String,
        work_sequence: std::num::NonZeroU64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduledFrameRequirement {
    pub expected: super::AutomationTargetIdentitySnapshot,
    pub after_frame_generation: u64,
    pub after_notification_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "family",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FixtureControl {
    AgentChat(AgentChatFixtureCommand),
    Flow(FlowFixtureCommand),
    SdkChat(SdkChatFixtureCommand),
    Dictation(DictationFixtureCommand),
    Notes(NotesFixtureCommand),
    Theme(ThemeFixtureCommand),
    Search(SearchFixtureCommand),
    Fault {
        operation: ThemeFaultOperation,
        target: super::AutomationWindowTarget,
    },
}

/// Filesystem faults are confined to the evaluator's synthetic theme path.
/// Application, saving, and reload still use the production theme owners.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "camelCase", deny_unknown_fields)]
pub enum ThemeFixtureCommand {
    ArmSaveFailure {},
    ClearSaveFailure {},
    MalformedReload {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeFaultOperation {
    SuppressThemeNotification,
}

/// Bounded negative fixtures only; execution is not evidence of production adoption.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NativeSafetyProbe {
    InvalidShow,
    InvalidFocus,
    InvalidDialog,
    InvalidTabbing,
    InvalidOversize,
    NativeActivation,
    NativeIme,
    GlobalPointer,
    ClipboardRead,
    ClipboardWrite,
    DirectAppActivation,
    Process,
    Provider,
    Credentials,
    Device,
    OpenExternal,
    Notification,
    BlankReadback,
    FailedReadback,
    MissingRequiredImage,
    MissingRequiredSvg,
    OversizedImage,
    DuplicateSemanticIdentity,
    DuplicateMeasurementIdentity,
    DeferredDispatch,
}

impl NativeSafetyProbe {
    pub const ALL: &'static [Self] = &[
        Self::InvalidShow,
        Self::InvalidFocus,
        Self::InvalidDialog,
        Self::InvalidTabbing,
        Self::InvalidOversize,
        Self::NativeActivation,
        Self::NativeIme,
        Self::GlobalPointer,
        Self::ClipboardRead,
        Self::ClipboardWrite,
        Self::DirectAppActivation,
        Self::Process,
        Self::Provider,
        Self::Credentials,
        Self::Device,
        Self::OpenExternal,
        Self::Notification,
        Self::BlankReadback,
        Self::FailedReadback,
        Self::MissingRequiredImage,
        Self::MissingRequiredSvg,
        Self::OversizedImage,
        Self::DuplicateSemanticIdentity,
        Self::DuplicateMeasurementIdentity,
        Self::DeferredDispatch,
    ];
}

/// The only coordinator-owned SDK child accepted by the native evaluator.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum SdkPromptFixtureId {
    #[serde(rename = "sdk.arg-roundtrip.v1")]
    ArgRoundtrip,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SdkCompletionChannel {
    Connected,
    Full,
    Disconnected,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SdkPromptCommand {
    Begin {
        fixture_id: SdkPromptFixtureId,
        message: serde_json::Value,
        channel: SdkCompletionChannel,
    },
    Drain {},
    ReleaseCapacity {},
    Close {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    tag = "operation",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum DesignCommand {
    Bootstrap {
        launch_nonce: String,
        policy_sha256: String,
    },
    Catalog {},
    Mount {
        fixture_id: String,
        parent: Option<super::AutomationWindowTarget>,
    },
    CaptureFrame {
        target: super::AutomationWindowTarget,
        include_image: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scheduled: Option<ScheduledFrameRequirement>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "deserialize_owned_frame_cursor"
        )]
        frame_cursor: Option<OwnedFrameCursor>,
    },
    /// Release acknowledged owned trace history, retaining the cursor frame as
    /// the next capture baseline. This never draws or advances a read cursor.
    AcknowledgeFrames {
        target: super::AutomationWindowTarget,
        expected: super::AutomationTargetIdentitySnapshot,
        cursor: OwnedFrameCursor,
    },
    ApplyTheme {
        expected_revision: u64,
        edits: Vec<LiveThemeEdit>,
    },
    RevertTheme {
        expected_revision: u64,
    },
    Unmount {
        target: super::AutomationWindowTarget,
        expected: super::AutomationTargetIdentitySnapshot,
    },
    FixtureControl {
        target: super::AutomationWindowTarget,
        expected: super::AutomationTargetIdentitySnapshot,
        control: FixtureControl,
    },
    SdkPrompt {
        target: super::AutomationWindowTarget,
        expected: super::AutomationTargetIdentitySnapshot,
        command: SdkPromptCommand,
    },
    ProbeSafety {
        target: super::AutomationWindowTarget,
        expected: super::AutomationTargetIdentitySnapshot,
        probe: NativeSafetyProbe,
    },
    Diagnose {},
    End {},
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInvalidation {
    pub target: super::AutomationWindowTarget,
    pub revision: u64,
    pub cause: ThemeInvalidationCause,
    pub invalidation_epoch: u64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeInvalidationCause {
    ThemePublication,
}
