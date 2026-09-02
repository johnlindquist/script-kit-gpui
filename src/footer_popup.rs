use gpui::{
    div, prelude::FluentBuilder, px, rgba, svg, AnyElement, AnyWindowHandle, App, AppContext,
    Bounds, Context, DisplayId, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, NO, YES};
#[cfg(target_os = "macos")]
#[allow(unused_imports)]
use objc::{msg_send, sel, sel_impl};

#[cfg(target_os = "macos")]
const FOOTER_EFFECT_ID: &str = "script-kit-footer-effect";
#[cfg(target_os = "macos")]
const FOOTER_GLASS_CONTAINER_ID: &str = "script-kit-main-footer-glass-container";
#[cfg(target_os = "macos")]
const FOOTER_DIVIDER_ID: &str = "script-kit-footer-divider";
#[cfg(target_os = "macos")]
const FOOTER_HINTS_ID: &str = "script-kit-footer-hints";
#[cfg(target_os = "macos")]
const FOOTER_HINT_ITEM_GAP: f64 =
    crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_KEY_LABEL_GAP: f64 =
    crate::components::footer_chrome::FOOTER_ACTION_CONTENT_GAP_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_SIDE_INSET: f64 = crate::window_resize::main_layout::HINT_STRIP_PADDING_X as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_PADDING_X: f64 =
    crate::components::footer_chrome::FOOTER_ACTION_CONTENT_PADDING_X_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_RUN_HINT_PADDING_X: f64 =
    crate::components::footer_chrome::FOOTER_KEY_ANCHORED_CONTENT_PADDING_X_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_RADIUS: f64 =
    crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_TEXT_ALIGN_LEFT: usize = 0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_TEXT_ALIGN_RIGHT: usize = 2;
#[cfg(target_os = "macos")]
const FOOTER_HINT_BUTTON_ID_PREFIX: &str = "script-kit-footer-button-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_ITEM_ID_PREFIX: &str = "script-kit-footer-item-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_CAPSULE_ID_PREFIX: &str = "script-kit-footer-capsule-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_CAPSULE_CONTENT_ID_PREFIX: &str = "script-kit-footer-capsule-content-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_STATE_LAYER_ID_PREFIX: &str = "script-kit-footer-state-layer-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_LABEL_CHIP_ID_PREFIX: &str = "script-kit-footer-label-chip-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_LABEL_ID_PREFIX: &str = "script-kit-footer-label-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_KEYS_ID_PREFIX: &str = "script-kit-footer-keys-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_KEYCAP_ID_PREFIX: &str = "script-kit-footer-keycap-";
#[cfg(target_os = "macos")]
const FOOTER_HINT_KEYCAP_GLYPH_ID_PREFIX: &str = "script-kit-footer-keycap-glyph-";
/// Identifier prefix for the per-button leading status dot view, so DevTools /
/// layout proofs can find e.g. `script-kit-footer-leading-dot-agentModel`.
#[cfg(target_os = "macos")]
const FOOTER_HINT_LEADING_DOT_ID_PREFIX: &str = "script-kit-footer-leading-dot-";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_ID: &str = "script-kit-footer-left-info";
#[cfg(target_os = "macos")]
const FOOTER_STATUS_DOT_ID: &str = "script-kit-footer-status-dot";
const FOOTER_CWD_CHIP_ICON_ID: &str = "script-kit-footer-cwd-chip-icon";
const FOOTER_CWD_CHIP_LABEL_ID: &str = "script-kit-footer-cwd-chip-label";
const FOOTER_CWD_CHIP_KEYCAP_ID: &str = "script-kit-footer-cwd-chip-keycap";
const FOOTER_CWD_CHIP_KEYCAP_GLYPH_ID: &str = "script-kit-footer-cwd-chip-keycap-glyph";
const FOOTER_CWD_CHIP_HIT_TARGET_ID: &str = "script-kit-footer-cwd-chip-hit";
const FOOTER_CWD_CHIP_TRAILING_GAP_PX: f64 = 12.0;
#[cfg(target_os = "macos")]
const FOOTER_MODEL_LABEL_ID: &str = "script-kit-footer-model-label";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_PROFILE_ICON_ID: &str = "script-kit-footer-left-profile-icon";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_KEYCAP_ID: &str = "script-kit-footer-left-info-keycap";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_KEYCAP_GLYPH_ID: &str = "script-kit-footer-left-info-keycap-glyph";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_HIT_TARGET_ID: &str = "script-kit-footer-left-info-hit-target";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_CAPSULE_ID: &str = "script-kit-footer-left-info-capsule";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID: &str = "script-kit-footer-left-info-capsule-content";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_STATE_LAYER_ID: &str = "script-kit-footer-left-info-state-layer";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_PROFILE_ICON_SIZE: f64 = 13.0;
#[cfg(target_os = "macos")]
const FOOTER_STREAMING_DOT_SIZE: f64 =
    crate::components::footer_chrome::FOOTER_STATUS_DOT_SIZE_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_LEFT_DOT_LABEL_GAP: f64 =
    crate::components::footer_chrome::FOOTER_LEFT_INFO_GAP_PX as f64;
/// Braille loading spinner in the footer left info: glyph size and the
/// fixed lane width that keeps the label from shifting as frames cycle.
const FOOTER_BRAILLE_SPINNER_FONT_PX: f32 =
    crate::components::footer_chrome::FOOTER_BRAILLE_SPINNER_FONT_PX;
const FOOTER_BRAILLE_SPINNER_LANE_PX: f32 =
    crate::components::footer_chrome::FOOTER_BRAILLE_SPINNER_LANE_PX;
#[cfg(target_os = "macos")]
const FOOTER_ACTIVE_DOT_MIN_OPACITY: f32 = 0.6;
#[cfg(target_os = "macos")]
const FOOTER_ACTIVE_DOT_HALF_CYCLE_SECONDS: f64 = 1.0;
#[cfg(target_os = "macos")]
const FOOTER_RUN_SLOT_MIN_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_RUN_SLOT_MIN_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_RUN_SLOT_MAX_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_RUN_SLOT_MAX_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_ACTIONS_SLOT_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_ACTIONS_SLOT_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_AI_SLOT_WIDTH: f64 = crate::components::footer_chrome::FOOTER_AI_SLOT_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_APPLY_SLOT_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_APPLY_SLOT_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_CLOSE_SLOT_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_CLOSE_SLOT_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_STOP_SLOT_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_STOP_SLOT_WIDTH_PX as f64;
#[cfg(target_os = "macos")]
const FOOTER_PASTE_RESPONSE_SLOT_WIDTH: f64 =
    crate::components::footer_chrome::FOOTER_PASTE_RESPONSE_SLOT_WIDTH_PX as f64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterAction {
    Run,
    Actions,
    Ai,
    Apply,
    Replace,
    Append,
    Copy,
    Expand,
    Retry,
    Close,
    Stop,
    PasteResponse,
    /// Click the CWD footer chip — opens the directory picker so the user
    /// can change their current working directory.
    Cwd,
    /// Click the Agent · Model footer chip — opens the Shift+Tab Agent & Model
    /// picker so the user can change the agent (Pi provider) and model used by
    /// the next launch.
    AgentModel,
    Tips,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterPlacement {
    Leading,
    Trailing,
}

/// Canonical behavior descriptor consumed by both the GPUI and AppKit footer
/// renderers. `key` is retained as the raw display spelling; routing and audit
/// use the cached canonical shortcut and token stream instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterButtonConfig {
    /// Stable semantic control identity. Never derived from vector position.
    pub id: SharedString,
    /// Executable route shared by click and keyboard dispatch.
    pub action: FooterAction,
    /// Raw shortcut/icon spelling supplied by the owning surface.
    pub key: SharedString,
    /// Canonical shortcut tokens from the shared UX-002 parser.
    pub shortcut_tokens: Vec<String>,
    /// Canonical key route, or `None` for empty/icon-only controls.
    pub canonical_shortcut: Option<String>,
    /// False for disabled controls and canonical shortcut collisions. The raw
    /// value remains available for diagnostics but is not rendered as a keycap.
    pub shortcut_routable: bool,
    /// User-facing action verb.
    pub label: SharedString,
    pub selected: bool,
    pub enabled: bool,
    pub disabled_reason: Option<SharedString>,
    pub placement: FooterPlacement,
    /// Optional status dot rendered at the leading edge of the button, INSIDE
    /// the chip (e.g. the Agent Chat streaming/idle dot on the Agent·Model chip). When
    /// `Some(_)` a fixed-width dot lane is reserved so the chip's width stays
    /// stable as the status changes; `Some(Hidden)` reserves the lane but draws
    /// nothing. `None` reserves no lane (the common case — keeps ScriptList and
    /// every other button dot-free).
    pub leading_dot: Option<FooterDotStatus>,
}

pub(crate) const MAIN_WINDOW_FOOTER_MAX_ACTION_SLOTS: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterSlotRole {
    ActionSlot,
    ContextChip,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MainWindowFooterSlotModel {
    pub surface: &'static str,
    pub button_count: usize,
    pub action_slot_count: usize,
    pub context_chip_count: usize,
    pub duplicate_action_ids: Vec<String>,
    pub duplicate_shortcut_keys: Vec<String>,
    pub violation: Option<&'static str>,
}

pub(crate) fn footer_button_slot_role(button: &FooterButtonConfig) -> FooterSlotRole {
    if matches!(button.action, FooterAction::Cwd | FooterAction::AgentModel) {
        return FooterSlotRole::ContextChip;
    }

    if matches!(button.action, FooterAction::Ai)
        && button.key.as_ref() == crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN
    {
        return FooterSlotRole::ContextChip;
    }

    FooterSlotRole::ActionSlot
}

impl FooterButtonConfig {
    pub(crate) fn new(
        action: FooterAction,
        key: impl Into<SharedString>,
        label: impl Into<SharedString>,
    ) -> Self {
        let key = key.into();
        let icon_only = crate::components::footer_chrome::is_footer_icon_token(key.as_ref());
        let shortcut_tokens = if key.trim().is_empty() || icon_only {
            Vec::new()
        } else {
            crate::components::hint_strip::shortcut_tokens_from_hint(key.as_ref())
        };
        let canonical_shortcut = if shortcut_tokens.is_empty() {
            None
        } else {
            let canonical = crate::components::hint_strip::canonical_shortcut_hint(key.as_ref());
            (!canonical.is_empty()).then_some(canonical)
        };
        let placement = if matches!(action, FooterAction::Cwd | FooterAction::AgentModel)
            || (matches!(action, FooterAction::Ai) && icon_only)
        {
            FooterPlacement::Leading
        } else {
            FooterPlacement::Trailing
        };
        Self {
            id: SharedString::from(format!("footer-action:{}", action.semantic_key())),
            action,
            key,
            shortcut_tokens,
            shortcut_routable: canonical_shortcut.is_some(),
            canonical_shortcut,
            label: label.into(),
            selected: false,
            enabled: true,
            disabled_reason: None,
            placement,
            leading_dot: None,
        }
    }

    pub(crate) fn id(mut self, id: impl Into<SharedString>) -> Self {
        let id = id.into();
        assert!(!id.trim().is_empty(), "footer action IDs must not be blank");
        self.id = id;
        self
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn left_pinned(mut self) -> Self {
        self.placement = FooterPlacement::Leading;
        self
    }

    /// Reserve a fixed-width leading dot lane inside the chip and render the dot
    /// for `status` (`Hidden` keeps the lane but draws nothing).
    pub(crate) fn leading_dot(mut self, status: FooterDotStatus) -> Self {
        self.leading_dot = Some(status);
        self
    }

    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.shortcut_routable = enabled && self.canonical_shortcut.is_some();
        if enabled {
            self.disabled_reason = None;
        }
        self
    }

    pub(crate) fn disabled_reason(mut self, reason: impl Into<SharedString>) -> Self {
        let reason = reason.into();
        assert!(
            !reason.trim().is_empty(),
            "disabled footer actions require a non-empty reason"
        );
        self.disabled_reason = Some(reason);
        self.enabled = false;
        self.shortcut_routable = false;
        self
    }
}

/// Apply deterministic descriptor variants for real runtime verification.
///
/// The fixture is double-gated so normal launches can never inherit test-only
/// labels, disabled actions, or shortcut collisions.
pub(crate) fn apply_footer_descriptor_test_fixture(buttons: &mut [FooterButtonConfig]) {
    let test_status = std::env::var("SCRIPT_KIT_TEST_STATUS").ok().as_deref() == Some("1");
    let Ok(mode) = std::env::var("SCRIPT_KIT_TEST_FOOTER_DESCRIPTOR_FIXTURE") else {
        return;
    };
    if !test_status {
        return;
    }

    apply_footer_descriptor_test_fixture_mode(buttons, &mode);
}

fn apply_footer_descriptor_test_fixture_mode(buttons: &mut [FooterButtonConfig], mode: &str) {
    match mode {
        "disabled" => {
            if let Some(button) = buttons
                .iter_mut()
                .find(|button| button.action == FooterAction::Actions)
            {
                *button = button
                    .clone()
                    .disabled_reason("Unavailable in the footer descriptor test fixture");
            }
        }
        "collision" => {
            if let Some(button) = buttons
                .iter_mut()
                .find(|button| button.action == FooterAction::Ai)
            {
                button.key = SharedString::from("⌘K");
                button.shortcut_tokens =
                    crate::components::hint_strip::shortcut_tokens_from_hint("⌘K");
                button.canonical_shortcut = Some("cmd+k".to_string());
                button.shortcut_routable = true;
            }
        }
        "renamed" => {
            if let Some(button) = buttons
                .iter_mut()
                .find(|button| button.action == FooterAction::Actions)
            {
                button.label = SharedString::from("More Actions");
            }
        }
        _ => {}
    }
}

impl FooterAction {
    pub(crate) const fn semantic_key(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Actions => "actions",
            Self::Ai => "ai",
            Self::Apply => "apply",
            Self::Replace => "replace",
            Self::Append => "append",
            Self::Copy => "copy",
            Self::Expand => "expand",
            Self::Retry => "retry",
            Self::Close => "close",
            Self::Stop => "stop",
            Self::PasteResponse => "pasteResponse",
            Self::Cwd => "cwd",
            Self::AgentModel => "agentModel",
            Self::Tips => "tips",
        }
    }

    pub(crate) fn is_actions(self) -> bool {
        matches!(self, Self::Actions)
    }
}

/// Status of the Agent Chat thread, used to pick dot color and animation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum FooterDotStatus {
    /// No dot shown.
    #[default]
    Hidden,
    /// Streaming — pulsing, high-contrast theme-aligned dot.
    Streaming,
    /// Waiting for user permission — same pulsing active dot treatment.
    WaitingForPermission,
    /// Idle / done — subtle theme-matched dot.
    Idle,
    /// Error — solid theme error dot.
    Error,
}

/// Optional left-side info for the native footer (status dot + model name).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct FooterLeftInfo {
    /// Controls dot color and animation.
    pub dot_status: FooterDotStatus,
    /// Model display name (e.g. "Claude Sonnet 4"). Empty = hide label.
    pub model_name: String,
    /// When true, active Agent Chat states should use the accent token instead of the
    /// generic high-contrast fallback so the footer clearly reads as AI-active.
    pub prefer_accent_for_active_states: bool,
    /// Human-readable profile name for automation and accessibility snapshots.
    pub profile_name: Option<String>,
    /// Optional compact icon token rendered inside the merged left marker.
    pub icon_token: Option<String>,
    /// Optional key/sigil rendered as a footer-style keycap chip between the
    /// icon and the label (e.g. the ";" in the footer tip).
    pub keycap: Option<String>,
    /// Render the label at semibold weight (footer tips).
    pub bold_label: bool,
    /// Optional accent-colored braille spinner glyph rendered before the
    /// label (loading states, e.g. "Fetching tabs"). The caller re-syncs the
    /// footer config with the current frame each render tick; only the GPUI
    /// overlay renders it — main-window surfaces always own their glyphs
    /// there, and no detached-footer surface sets it.
    pub spinner_glyph: Option<String>,
    /// Optional action dispatched when the merged left marker is clicked.
    pub action: Option<FooterAction>,
    /// Whether the merged left marker should render as selected/open.
    pub selected: bool,
    /// Separate CWD chip rendered at the far left, independent of the model
    /// marker. When set, the model marker is centered between this chip and
    /// the trailing buttons.
    pub cwd_chip: Option<FooterCwdChip>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterCwdChip {
    pub label: String,
    pub icon_token: String,
    /// Optional keycap glyph shown after the label (e.g. "⇥") so the chip
    /// communicates the keyboard shortcut that opens the cwd picker. Renders
    /// with the same bordered chrome as the trailing footer buttons.
    pub key: Option<String>,
    pub tooltip: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MainWindowFooterConfig {
    pub surface: &'static str,
    pub buttons: Vec<FooterButtonConfig>,
    pub left_info: Option<FooterLeftInfo>,
}

/// Fail-closed authorization for an action on the *currently presented*
/// footer. Native callbacks can outlive a surface transition, so the mere
/// existence of a global action variant never grants dispatch authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterActionDispatchAuthorization<'a> {
    PresentedButton,
    PresentedLeftAffordance,
    PresentedHeaderAffordance,
    Disabled { reason: Option<&'a str> },
    NotPresented,
}

impl MainWindowFooterConfig {
    pub(crate) fn new(surface: &'static str, mut buttons: Vec<FooterButtonConfig>) -> Self {
        let mut shortcut_counts = BTreeMap::<String, usize>::new();
        for button in &buttons {
            assert!(
                button.disabled_reason.is_none() || !button.enabled,
                "footer actions with disabled reasons must be disabled"
            );
            if button.enabled {
                if let Some(canonical) = button.canonical_shortcut.as_ref() {
                    *shortcut_counts.entry(canonical.clone()).or_insert(0) += 1;
                }
            }
        }
        for button in &mut buttons {
            button.shortcut_routable = button.enabled
                && button.canonical_shortcut.as_ref().is_some_and(|canonical| {
                    shortcut_counts.get(canonical).copied().unwrap_or_default() == 1
                });
        }

        let config = Self {
            surface,
            buttons,
            left_info: None,
        };
        let model = config.slot_model();
        assert!(
            model.duplicate_action_ids.is_empty(),
            "main window footer action IDs must be unique on {surface}: {:?}",
            model.duplicate_action_ids
        );
        if let Some(violation) = model.violation {
            debug_assert!(
                false,
                "main window footer slot contract violation on {surface}: {violation}"
            );
            tracing::warn!(
                surface,
                action_slot_count = model.action_slot_count,
                context_chip_count = model.context_chip_count,
                button_count = model.button_count,
                duplicate_action_ids = ?model.duplicate_action_ids,
                duplicate_shortcut_keys = ?model.duplicate_shortcut_keys,
                violation,
                "Main window footer slot contract violation"
            );
        } else if !model.duplicate_shortcut_keys.is_empty() {
            tracing::warn!(
                surface,
                duplicate_shortcut_keys = ?model.duplicate_shortcut_keys,
                "Footer shortcut collision hidden until the producer resolves it"
            );
        }
        config
    }

    pub(crate) fn slot_model(&self) -> MainWindowFooterSlotModel {
        let mut action_slot_count = 0usize;
        let mut context_chip_count = 0usize;
        let mut id_counts = BTreeMap::<String, usize>::new();
        let mut shortcut_counts = BTreeMap::<String, usize>::new();

        for button in &self.buttons {
            *id_counts.entry(button.id.to_string()).or_insert(0) += 1;
            if button.enabled {
                if let Some(canonical) = button.canonical_shortcut.as_ref() {
                    *shortcut_counts.entry(canonical.clone()).or_insert(0) += 1;
                }
            }
            match footer_button_slot_role(button) {
                FooterSlotRole::ActionSlot => action_slot_count += 1,
                FooterSlotRole::ContextChip => context_chip_count += 1,
            }
        }

        let duplicate_action_ids = id_counts
            .into_iter()
            .filter_map(|(id, count)| (count > 1).then_some(id))
            .collect::<Vec<_>>();
        let duplicate_shortcut_keys = shortcut_counts
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect::<Vec<_>>();
        let violation = (action_slot_count > MAIN_WINDOW_FOOTER_MAX_ACTION_SLOTS)
            .then_some("too_many_action_slots");

        MainWindowFooterSlotModel {
            surface: self.surface,
            button_count: self.buttons.len(),
            action_slot_count,
            context_chip_count,
            duplicate_action_ids,
            duplicate_shortcut_keys,
            violation,
        }
    }

    pub(crate) fn descriptor_for_action(
        &self,
        action: FooterAction,
    ) -> Option<&FooterButtonConfig> {
        self.buttons
            .iter()
            .find(|descriptor| descriptor.action == action)
    }

    /// Authorize only a live enabled button, an exact rendered left marker, or
    /// a separately validated header cwd/model control. A disabled button
    /// always wins over another possible affordance for the same action.
    pub(crate) fn action_dispatch_authorization(
        &self,
        action: FooterAction,
        live_header_affordance: bool,
    ) -> FooterActionDispatchAuthorization<'_> {
        if let Some(descriptor) = self.descriptor_for_action(action) {
            return if descriptor.enabled && descriptor.disabled_reason.is_none() {
                FooterActionDispatchAuthorization::PresentedButton
            } else {
                FooterActionDispatchAuthorization::Disabled {
                    reason: descriptor
                        .disabled_reason
                        .as_deref()
                        .map(|reason| &**reason),
                }
            };
        }

        if let Some(info) = self.left_info.as_ref() {
            let exact_marker = info.action == Some(action);
            let cwd_chip = action == FooterAction::Cwd && info.cwd_chip.is_some();
            let model_chip = action == FooterAction::AgentModel
                && info.profile_name.is_some()
                && !info.model_name.trim().is_empty();
            if exact_marker || cwd_chip || model_chip {
                return FooterActionDispatchAuthorization::PresentedLeftAffordance;
            }
        }

        if live_header_affordance && matches!(action, FooterAction::Cwd | FooterAction::AgentModel)
        {
            return FooterActionDispatchAuthorization::PresentedHeaderAffordance;
        }

        FooterActionDispatchAuthorization::NotPresented
    }

    pub(crate) fn has_canonical_shortcut_candidate(&self, canonical: &str) -> bool {
        self.buttons
            .iter()
            .any(|descriptor| descriptor.canonical_shortcut.as_deref() == Some(canonical))
    }

    pub(crate) fn action_for_canonical_shortcut(&self, canonical: &str) -> Option<FooterAction> {
        self.buttons
            .iter()
            .find(|descriptor| {
                descriptor.shortcut_routable
                    && descriptor.canonical_shortcut.as_deref() == Some(canonical)
            })
            .map(|descriptor| descriptor.action)
    }

    pub(crate) fn slot_contract_violation(&self) -> Option<&'static str> {
        self.slot_model().violation
    }
}

fn footer_active_dot_hex(theme: &crate::theme::Theme, prefer_accent: bool) -> u32 {
    let colors = &theme.colors;
    let accent = colors.accent.selected;

    if prefer_accent {
        return accent;
    }

    let background = colors.background.main;
    let primary_text = colors.text.primary;

    if crate::theme::contrast_ratio(accent, background)
        >= crate::theme::contrast_ratio(primary_text, background)
    {
        accent
    } else {
        primary_text
    }
}

fn footer_dot_hex(
    status: FooterDotStatus,
    theme: &crate::theme::Theme,
    prefer_accent_for_active_states: bool,
) -> u32 {
    let colors = &theme.colors;
    match status {
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission => {
            footer_active_dot_hex(theme, prefer_accent_for_active_states)
        }
        FooterDotStatus::Idle => colors.text.secondary,
        FooterDotStatus::Error => colors.ui.error,
        FooterDotStatus::Hidden => unreachable!(),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MainWindowFooterHostSnapshot {
    pub requested_surface: Option<&'static str>,
    pub installed_surface: Option<&'static str>,
    pub native_host_installed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MainWindowFooterRefreshSignature {
    config: MainWindowFooterConfig,
    theme_revision: u64,
    content_width_bits: u64,
    dark: bool,
    material: crate::theme::VibrancyMaterial,
    divider_rgba: u32,
    text_primary_hex: u32,
    background_hex: u32,
    glass_tint_opacity_bits: u32,
    accent_hex: u32,
    selection_rgba: u32,
    hover_rgba: u32,
    left_dot_hex: Option<u32>,
    #[cfg(target_os = "macos")]
    native_glass_signature: crate::platform::NativeGlassStyleSignature,
    #[cfg(target_os = "macos")]
    native_visual_theme: NativeFooterVisualTheme,
    /// Active main-menu theme discriminant. The native footer reads the *global*
    /// current theme (not threaded through `config`), so the discriminant is
    /// folded into the signature to force a rebuild on cycle.
    main_menu_theme: u8,
    /// Whether the GPUI overlay owns the glyphs for this refresh (main window)
    /// or AppKit renders them natively (detached Agent Chat / dictation).
    /// Folded in so alternating refreshes across hosts with otherwise
    /// identical configs still rebuild the hint subviews.
    gpui_overlay_owns_glyphs: bool,
    /// Per-button leading-dot colors (parallel to `config.buttons`). A theme
    /// change can recolor a button's status dot without changing the config, and
    /// the AppKit dot layer is created inside the content rebuild, so this is
    /// folded into `footer_content_changed`.
    button_leading_dot_hexes: Vec<Option<u32>>,
}

#[cfg(target_os = "macos")]
fn native_footer_refresh_signature(
    config: &MainWindowFooterConfig,
    snapshot: &crate::theme::live_edit::PublishedTheme,
    content_width: f64,
    gpui_overlay_owns_glyphs: bool,
) -> MainWindowFooterRefreshSignature {
    let theme = snapshot.theme.as_ref();
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    MainWindowFooterRefreshSignature {
        config: config.clone(),
        theme_revision: snapshot.revision,
        content_width_bits: content_width.to_bits(),
        dark: theme.should_use_dark_vibrancy(),
        material: theme.get_vibrancy().material,
        divider_rgba: chrome.divider_rgba,
        text_primary_hex: theme.colors.text.primary,
        background_hex: theme.colors.background.main,
        glass_tint_opacity_bits: theme
            .get_opacity()
            .glass_tint_opacity
            .unwrap_or(0.0)
            .to_bits(),
        accent_hex: chrome.accent_hex,
        selection_rgba: chrome.selection_rgba,
        hover_rgba: chrome.hover_rgba,
        left_dot_hex: config.left_info.as_ref().and_then(|info| {
            (!matches!(info.dot_status, FooterDotStatus::Hidden)).then(|| {
                footer_dot_hex(info.dot_status, theme, info.prefer_accent_for_active_states)
            })
        }),
        native_glass_signature: crate::platform::resolve_native_glass_style(
            theme,
            crate::platform::NativeGlassSurfaceRole::FloatingCapsule,
        )
        .signature,
        native_visual_theme: resolve_native_footer_visual_theme(theme),
        main_menu_theme: crate::designs::current_main_menu_theme() as u8,
        gpui_overlay_owns_glyphs,
        button_leading_dot_hexes: config
            .buttons
            .iter()
            .map(|button| {
                button.leading_dot.and_then(|status| {
                    (!matches!(status, FooterDotStatus::Hidden))
                        .then(|| footer_dot_hex(status, theme, true))
                })
            })
            .collect(),
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeFooterVisualTheme {
    row_palette: crate::theme::MainMenuRowStatePalette,
    keycap_hex: u32,
    rest_border_alpha_bits: u32,
    hover_border_alpha_bits: u32,
    active_border_alpha_bits: u32,
}

#[cfg(target_os = "macos")]
impl NativeFooterVisualTheme {
    fn border_alpha(self, state: crate::theme::MainMenuRowState) -> f32 {
        f32::from_bits(match state {
            crate::theme::MainMenuRowState::Rest => self.rest_border_alpha_bits,
            crate::theme::MainMenuRowState::Hover => self.hover_border_alpha_bits,
            crate::theme::MainMenuRowState::Active => self.active_border_alpha_bits,
        })
    }
}

#[cfg(target_os = "macos")]
fn resolve_native_footer_visual_theme(theme: &crate::theme::Theme) -> NativeFooterVisualTheme {
    let border_alpha = |state| {
        crate::components::footer_chrome::footer_keycap_border_alpha_for_state(theme, state)
    };
    native_footer_visual_theme_from_parts(
        crate::components::footer_chrome::resolved_footer_button_visual_colors(theme).row_states,
        footer_keycap_hex(theme),
        border_alpha(crate::theme::MainMenuRowState::Rest),
        border_alpha(crate::theme::MainMenuRowState::Hover),
        border_alpha(crate::theme::MainMenuRowState::Active),
    )
}

#[cfg(target_os = "macos")]
fn native_footer_visual_theme_from_parts(
    row_palette: crate::theme::MainMenuRowStatePalette,
    keycap_hex: u32,
    rest_border_alpha: f32,
    hover_border_alpha: f32,
    active_border_alpha: f32,
) -> NativeFooterVisualTheme {
    NativeFooterVisualTheme {
        row_palette,
        keycap_hex,
        rest_border_alpha_bits: rest_border_alpha.to_bits(),
        hover_border_alpha_bits: hover_border_alpha.to_bits(),
        active_border_alpha_bits: active_border_alpha.to_bits(),
    }
}

include!("footer_popup_ownership.rs");

include!("footer_popup_fidelity.rs");
include!("footer_popup_overlay.rs");
include!("footer_popup_adapters.rs");

struct GpuiFooterOverlay {
    config: MainWindowFooterConfig,
    binding: FooterBinding,
    overlay_width_px: f32,
    last_reported_row_palette: Option<crate::theme::MainMenuRowStatePalette>,
    close_subscription: Option<gpui::Subscription>,
    painted_binding: Option<FooterBinding>,
    painted_frame_generation: u64,
}

impl GpuiFooterOverlay {
    fn new(config: MainWindowFooterConfig, binding: FooterBinding, overlay_width_px: f32) -> Self {
        Self {
            config,
            binding,
            overlay_width_px,
            last_reported_row_palette: None,
            close_subscription: None,
            painted_binding: None,
            painted_frame_generation: 0,
        }
    }

    fn set_config(
        &mut self,
        config: MainWindowFooterConfig,
        binding: FooterBinding,
        overlay_width_px: f32,
    ) {
        self.config = config;
        self.binding = binding;
        self.overlay_width_px = overlay_width_px;
    }

    fn render_left_info(
        &self,
        left_info: Option<&FooterLeftInfo>,
        theme: &crate::theme::Theme,
    ) -> AnyElement {
        let Some(info) = left_info else {
            return div().into_any_element();
        };

        let row_id = if info.action.is_some() {
            "agent-chat-profile-display"
        } else {
            "footer-left-info"
        };
        let interactive = info.action.is_some();
        let row_states =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(theme)
                .row_states;
        let base_state = if interactive && info.selected {
            row_states.active
        } else {
            row_states.rest
        };
        let hover_state = if interactive && info.selected {
            row_states.active
        } else {
            row_states.hover
        };
        let base_foreground = rgba(base_state.primary_foreground_rgba);
        let hover_foreground: gpui::Hsla = rgba(hover_state.primary_foreground_rgba).into();
        let mut row = div()
            .id(row_id)
            .debug_selector(|| "agent-chat.footer-overlay.profile".to_string())
            .flex()
            .items_center()
            .gap(px(FOOTER_LEFT_DOT_LABEL_GAP as f32))
            .min_w(px(0.0))
            .overflow_hidden();

        if let Some(action) = info.action {
            let binding = self.binding.clone();
            // Clickable left-info markers (footer tip, Agent Chat profile)
            // are real footer buttons: same hover pill, radius, and pressed
            // fill as the trailing action buttons, with label/keycap/glyph
            // brightening through the shared footer-action-button group.
            let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
            let hover_bg = rgba(row_states.hover.background_rgba.unwrap_or_default());
            let active_bg = rgba(row_states.active.background_rgba.unwrap_or_default());
            row = row
                .h(px(crate::components::footer_chrome::footer_button_height(
                    crate::components::footer_chrome::current_main_menu_footer_height(),
                )))
                .px(px(
                    crate::components::footer_chrome::footer_centered_action_edge_padding_x(),
                ))
                .rounded(px(metrics.button_radius))
                .group("footer-action-button")
                .cursor_pointer()
                .when(!info.selected, |style| {
                    style.hover(move |style| style.bg(hover_bg))
                })
                .active(move |style| style.bg(active_bg))
                .on_mouse_down(
                    MouseButton::Left,
                    move |_event: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                        if record_footer_held_action(&binding, Some(action)) {
                            _window.refresh();
                        }
                        dispatch_bound_footer_action(&binding, action);
                    },
                );
        } else {
            row = row.flex_1();
        }

        if info.selected && interactive {
            row = row.bg(rgba(row_states.active.background_rgba.unwrap_or_default()));
        } else if info.selected {
            let accent = theme.colors.accent.selected;
            row = row.bg(rgba((accent << 8) | 0x18));
            row = row.rounded(px(4.0)).px(px(4.0)).py(px(1.0));
        }

        if info.icon_token.is_none() && !matches!(info.dot_status, FooterDotStatus::Hidden) {
            row = row.child(
                div()
                    .size(px(FOOTER_STREAMING_DOT_SIZE as f32))
                    .rounded(px((FOOTER_STREAMING_DOT_SIZE / 2.0) as f32))
                    .bg(rgba(
                        (footer_dot_hex(
                            info.dot_status,
                            theme,
                            info.prefer_accent_for_active_states,
                        ) << 8)
                            | 0xff,
                    )),
            );
        }

        if let Some(glyph) = info
            .spinner_glyph
            .as_ref()
            .filter(|glyph| !glyph.trim().is_empty())
        {
            let accent = theme.colors.accent.selected;
            row = row.child(
                div()
                    .id("footer-braille-spinner")
                    .debug_selector(|| "agent-chat.footer-overlay.spinner".to_string())
                    .w(px(FOOTER_BRAILLE_SPINNER_LANE_PX))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .font_family(crate::list_item::FONT_MONO)
                    .text_size(px(FOOTER_BRAILLE_SPINNER_FONT_PX))
                    .text_color(rgba((accent << 8) | 0xff))
                    .child(glyph.clone()),
            );
        }

        if let Some(path) = info
            .icon_token
            .as_deref()
            .and_then(crate::components::footer_chrome::footer_icon_path)
        {
            row = row.child(
                svg()
                    .path(path)
                    .size(px(13.0))
                    .flex_shrink_0()
                    .text_color(base_foreground)
                    .group_hover("footer-action-button", move |style| {
                        style.text_color(hover_foreground)
                    }),
            );
        }

        if let Some(keycap) = info.keycap.as_ref().filter(|key| !key.trim().is_empty()) {
            row = row.child(
                crate::components::footer_chrome::render_footer_shortcut_keycaps_for_state(
                    keycap.clone(),
                    theme,
                    interactive && info.selected,
                ),
            );
        }

        if !info.model_name.trim().is_empty() {
            let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
            let label_weight = if info.bold_label {
                gpui::FontWeight::SEMIBOLD
            } else {
                metrics.font_weight
            };
            row = row.child(
                div()
                    .id("agent_chat-model-display")
                    .debug_selector(|| "agent-chat.footer-overlay.model".to_string())
                    .min_w(px(0.0))
                    .font_family(crate::list_item::FONT_SYSTEM_UI)
                    .font_weight(label_weight)
                    .text_size(px(metrics.label_font_size))
                    .text_color(base_foreground)
                    .group_hover("footer-action-button", move |style| {
                        style.text_color(hover_foreground)
                    })
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .child(info.model_name.clone()),
            );
        }

        if interactive {
            // The button pill hugs its content inside the left lane instead
            // of stretching a hover highlight across the empty footer.
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .flex()
                .items_center()
                .justify_start()
                .child(row)
                .into_any_element()
        } else {
            row.into_any_element()
        }
    }

    fn render_button(
        &self,
        button: FooterButtonConfig,
        theme: &crate::theme::Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let row_states =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(theme)
                .row_states;
        let action = button.action;
        let descriptor_id = button.id.clone();
        let key_is_icon =
            crate::components::footer_chrome::is_footer_icon_token(button.key.as_ref());
        let displayed_key = if key_is_icon || button.shortcut_routable {
            button.key.clone()
        } else {
            SharedString::from("")
        };
        let selected_bg = rgba(row_states.active.background_rgba.unwrap_or_default());
        let hover_bg = rgba(row_states.hover.background_rgba.unwrap_or_default());
        let active_bg = selected_bg;
        let item_height = crate::components::footer_chrome::footer_button_height(
            crate::components::footer_chrome::current_main_menu_footer_height(),
        );
        let key_first = is_footer_left_pinned_button(&button)
            && !matches!(action, FooterAction::Cwd | FooterAction::AgentModel);
        let justify = if matches!(action, FooterAction::Cwd | FooterAction::AgentModel) || key_first
        {
            crate::components::footer_chrome::FooterHintContentJustify::Start
        } else if matches!(action, FooterAction::Run) {
            crate::components::footer_chrome::FooterHintContentJustify::KeyAnchored
        } else {
            crate::components::footer_chrome::FooterHintContentJustify::Center
        };

        // Flexbox-native sizing: each button takes its intrinsic content
        // width (GPUI measures the real text), floored at the action's slot
        // minimum and capped for the Run slot so long script names shrink and
        // ellipsize under real layout pressure instead of against estimated
        // character widths.
        let min_width = footer_hint_slot_width(action) as f32;
        let fidelity_id = format!("agent-chat.footer-overlay.{}", descriptor_id);
        let mut item = div()
            .id(descriptor_id)
            .debug_selector(move || fidelity_id.clone())
            .min_w(px(min_width))
            .when(matches!(action, FooterAction::Run), |style| {
                style.max_w(px(
                    crate::components::footer_chrome::FOOTER_RUN_SLOT_MAX_WIDTH_PX,
                ))
            })
            .h(px(item_height))
            .rounded(px(
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX,
            ))
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .group("footer-action-button")
            .when(button.selected, |style| style.bg(selected_bg))
            .child(if button.selected {
                crate::components::footer_chrome::render_footer_hint_content_flex_for_state(
                    button.label.clone(),
                    displayed_key.clone(),
                    crate::components::footer_chrome::FooterHintKeyMode::Shortcut,
                    theme,
                    key_first,
                    justify,
                    button.selected,
                )
            } else {
                crate::components::footer_chrome::render_footer_hint_content_flex(
                    button.label.clone(),
                    displayed_key.clone(),
                    crate::components::footer_chrome::FooterHintKeyMode::Shortcut,
                    theme,
                    key_first,
                    justify,
                )
            });

        if button.enabled {
            item = item
                .cursor_pointer()
                .when(!button.selected, |style| {
                    style.hover(move |style| style.bg(hover_bg))
                })
                .active(move |style| style.bg(active_bg))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |_this, _event: &MouseDownEvent, _window, cx| {
                        cx.stop_propagation();
                        if record_footer_held_action(&_this.binding, Some(action)) {
                            cx.notify();
                        }
                        dispatch_bound_footer_action(&_this.binding, action);
                    }),
                );
        } else {
            item = item.opacity(0.45).cursor_default();
        }

        item.into_any_element()
    }
}

impl Render for GpuiFooterOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(parent) = prepare_footer_overlay_render(self, window) else {
            window.defer(cx, |window, _| window.remove_window());
            return div().into_any_element();
        };
        let parent_id = self.binding.window_id.clone();
        let painted_binding = self.binding.clone();
        let overlay = cx.entity().downgrade();
        window.defer(cx, move |window, cx| {
            let _ = overlay.update(cx, |overlay, _| {
                overlay.painted_binding = Some(painted_binding);
                overlay.painted_frame_generation = window.rendered_frame_generation();
            });
        });
        if window.fidelity_capture_active() {
            // App effects flush after this draw completes, so the deferred
            // callback observes the completed frame rendered below. This is
            // also deterministic on GPUI's test platform, whose platform
            // frame callback is intentionally inert.
            window.defer(cx, move |window, _cx| {
                if !window.fidelity_capture_active() {
                    return;
                }
                let snapshot = crate::fidelity_capture::paint_target_snapshot(
                    window,
                    GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID,
                    "footerOverlay",
                    Some(parent_id),
                );
                store_footer_overlay_fidelity_snapshot(parent, snapshot);
            });
        } else {
            clear_footer_overlay_fidelity_snapshot(parent);
        }

        let theme = crate::theme::get_cached_theme();
        let row_palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        if self.last_reported_row_palette != Some(row_palette) {
            tracing::info!(
                target: "script_kit::footer_popup",
                event = "gpui_footer_row_palette_resolved",
                rest_background_rgba = row_palette.rest.background_rgba.unwrap_or_default(),
                rest_has_background = row_palette.rest.background_rgba.is_some(),
                rest_foreground_rgba = row_palette.rest.primary_foreground_rgba,
                hover_background_rgba = row_palette.hover.background_rgba.unwrap_or_default(),
                hover_foreground_rgba = row_palette.hover.primary_foreground_rgba,
                active_background_rgba = row_palette.active.background_rgba.unwrap_or_default(),
                active_foreground_rgba = row_palette.active.primary_foreground_rgba,
                "Resolved GPUI footer controls from the canonical main-menu row palette"
            );
            self.last_reported_row_palette = Some(row_palette);
        }
        let left_pinned_buttons: Vec<_> = self
            .config
            .buttons
            .iter()
            .filter(|button| is_footer_left_pinned_button(button))
            .cloned()
            .collect();
        let trailing_buttons: Vec<_> = self
            .config
            .buttons
            .iter()
            .filter(|button| !is_footer_left_pinned_button(button))
            .cloned()
            .collect();

        // Pure flexbox layout: the left group absorbs spare space and shrinks
        // first (flex_1 + min_w 0); the trailing group keeps intrinsic width,
        // with each button able to shrink to its slot minimum, so the two
        // groups can never overlap regardless of window width.
        div()
            .id(GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID)
            .debug_selector(|| GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID.to_string())
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|overlay, _, _, cx| {
                    if record_footer_held_action(&overlay.binding, None) {
                        cx.notify();
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|overlay, _, _, cx| {
                    if record_footer_held_action(&overlay.binding, None) {
                        cx.notify();
                    }
                }),
            )
            .w_full()
            .h_full()
            .px(px(crate::window_resize::main_layout::HINT_STRIP_PADDING_X))
            .py(px(
                crate::components::footer_chrome::FOOTER_BUTTON_VERTICAL_INSET_PX,
            ))
            .flex()
            .items_center()
            .gap(px(
                crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
            ))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .gap(px(
                        crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
                    ))
                    .min_w(px(0.0))
                    .overflow_hidden()
                    .children(
                        left_pinned_buttons
                            .into_iter()
                            .map(|button| self.render_button(button, &theme, cx)),
                    )
                    .child(self.render_left_info(self.config.left_info.as_ref(), &theme)),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(
                        crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
                    ))
                    .children(
                        trailing_buttons
                            .into_iter()
                            .map(|button| self.render_button(button, &theme, cx)),
                    ),
            )
            .into_any_element()
    }
}

/// The GPUI flexbox footer overlay is the default main-window footer
/// renderer: AppKit keeps only the vibrancy material + divider sandwich
/// underneath while GPUI owns the glyphs in a child overlay window.
/// Set `SCRIPT_KIT_GPUI_FOOTER_OVERLAY=0` to fall back to native AppKit
/// glyph rendering for the main window.
fn gpui_footer_overlay_enabled() -> bool {
    std::env::var("SCRIPT_KIT_GPUI_FOOTER_OVERLAY")
        .map(|value| !matches!(value.as_str(), "0" | "false" | "FALSE" | "no" | "NO"))
        .unwrap_or(true)
}

fn should_use_gpui_footer_overlay(glass_mode: bool, overlay_enabled: bool) -> bool {
    !glass_mode && overlay_enabled
}

fn main_footer_gpui_overlay_active() -> bool {
    should_use_gpui_footer_overlay(glass_scroll_bands_active(), gpui_footer_overlay_enabled())
}

pub(crate) fn main_footer_gpui_overlay_visible() -> bool {
    main_footer_handle().is_some_and(|handle| {
        FOOTER_HOSTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&handle.window_id())
            .is_some_and(|host| host.overlay.is_some())
    })
}

fn gpui_footer_overlay_bounds(parent_bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    let footer_height = crate::components::footer_chrome::current_main_menu_footer_height();
    Bounds {
        origin: gpui::point(
            parent_bounds.origin.x,
            parent_bounds.origin.y + parent_bounds.size.height - px(footer_height),
        ),
        size: gpui::size(parent_bounds.size.width, px(footer_height)),
    }
}

fn gpui_footer_overlay_window_options(
    bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        window_background: WindowBackgroundAppearance::Transparent,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        is_resizable: false,
        is_minimizable: false,
        display_id,
        ..Default::default()
    }
}

fn clear_main_window_footer_refresh_signature() {
    if let Some(handle) = main_footer_handle() {
        clear_footer_refresh_signature(handle);
    }
}

/// Re-apply the last resolved footer config after native geometry, backing
/// scale, appearance, or visibility changed outside a GPUI render pass.
/// This never creates a second window: it only reconciles the footer already
/// owned by the main NSWindow and removes it when fallback mode is active.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn refresh_main_window_footer_from_last_config(ns_window: id) {
    let config = native_footer_binding(ns_window)
        .and_then(|(handle, _, _)| footer_config_for_window(handle));
    sync_main_window_glass_scroll_bands(ns_window);
    if let Some(config) = config.as_ref() {
        let _ = refresh_main_footer_host(ns_window, config);
    } else {
        remove_main_window_footer_host(ns_window);
    }
}

fn close_gpui_footer_overlay(cx: &mut App) {
    if let Some(handle) = main_footer_handle() {
        close_footer_overlay_for_parent(handle, cx);
    }
}

/// While a main-window glass morph is in flight, park the overlay at alpha 0
/// and fade it back in once the morph settles. The overlay is a separate
/// NSWindow tracking the main window's bounds — without this it appears
/// instantly at full alpha and visibly chases the animating frame.
fn park_overlay_during_glass_morph(handle: gpui::WindowHandle<GpuiFooterOverlay>, cx: &mut App) {
    let parked = handle
        .update(cx, |_overlay, window, _cx| {
            crate::platform::park_gpui_window_alpha_if_morphing(window)
        })
        .ok()
        .flatten();
    let Some(delay) = parked else {
        return;
    };
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor().timer(delay).await;
        let _ = handle.update(cx, |_overlay, window, _cx| {
            crate::platform::restore_gpui_window_alpha_animated(window);
        });
    })
    .detach();
}

fn sync_gpui_footer_overlay(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    parent_bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    _config: MainWindowFooterConfig,
) {
    if !main_footer_gpui_overlay_active() {
        close_footer_overlay_for_parent(parent_window_handle, cx);
        return;
    }
    if let Err(error) = open_or_sync_footer_overlay(
        parent_window_handle,
        parent_bounds,
        display_id,
        crate::runtime_policy::WindowHostPolicy::Interactive,
        cx,
    ) {
        tracing::warn!(%error, "Failed to synchronize footer overlay");
    }
}

fn update_main_window_footer_host_state(
    requested_surface: Option<&'static str>,
    installed_surface: Option<&'static str>,
    native_host_installed: bool,
) {
    if let Some(handle) = main_footer_handle() {
        if let Some(host) = FOOTER_HOSTS
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get_mut(&handle.window_id())
        {
            host.snapshot = MainWindowFooterHostSnapshot {
                requested_surface,
                installed_surface,
                native_host_installed,
            };
        }
    }
}

pub(crate) fn main_window_footer_host_snapshot() -> MainWindowFooterHostSnapshot {
    main_footer_handle()
        .map(footer_host_snapshot)
        .unwrap_or_default()
}

pub(crate) fn active_main_window_footer_surface() -> Option<&'static str> {
    main_window_footer_host_snapshot().installed_surface
}

#[cfg(target_os = "macos")]
pub(crate) unsafe fn native_footer_host_attached(ns_window: cocoa::base::id) -> bool {
    if ns_window == cocoa::base::nil {
        return false;
    }
    find_subview_by_identifier(
        reusable_window_footer_search_root(ns_window),
        FOOTER_EFFECT_ID,
    ) != cocoa::base::nil
}

pub(crate) fn sync_main_footer_popup(
    window: &mut Window,
    config: Option<&MainWindowFooterConfig>,
    cx: &mut App,
) {
    sync_footer_owner(window, config);
    notify_changed_footer_overlay(window.window_handle(), cx);
    if window.is_owned_hidden() {
        // The evaluator mounts the same overlay factory explicitly; no native material claims.
        return;
    }
    // Logical visibility flips before the native exit fade begins so rapid
    // hotkeys can supersede it. A render in that interval resolves `None`;
    // treating that as ordinary footer removal makes GPUI draw its fallback
    // footer inside the stage for one frame. The native-hidden completion
    // path owns teardown for this case.
    if config.is_none() && !crate::is_main_window_visible() {
        close_gpui_footer_overlay(cx);
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_footer_preserved_for_native_exit",
            "Preserved detached footer host until the native exit surface retires"
        );
        return;
    }

    // The in-window AppKit host rides the main NSWindow's frame morph. The
    // separate GPUI overlay NSWindow does not, so glass mode keeps only the
    // native host active and always closes the overlay.
    let requested_surface = config.map(|cfg| cfg.surface);
    update_main_window_footer_host_state(requested_surface, None, false);
    let parent_window_handle = window.window_handle();
    let parent_bounds = window.bounds();
    let display_id = window.display(cx).as_ref().map(|display| display.id());

    #[cfg(target_os = "macos")]
    {
        let Some((gpui_view, ns_window)) = window_gpui_view_and_ns_window(window) else {
            tracing::warn!(
                target: "script_kit::footer_popup",
                event = "native_footer_missing_ns_window",
                "Unable to resolve NSWindow for native footer host"
            );
            return;
        };

        // SAFETY: `ns_window` comes from the live GPUI main window currently
        // being rendered/observed on the AppKit thread.
        unsafe {
            if let Some(config) = config {
                let existed = find_subview_by_identifier(
                    main_window_footer_search_root(ns_window),
                    FOOTER_EFFECT_ID,
                ) != nil;
                let installed_host = ensure_main_window_footer_host(gpui_view, ns_window);
                if installed_host && !existed {
                    clear_main_window_footer_refresh_signature();
                }
                let installed = installed_host && refresh_main_footer_host(ns_window, config);
                update_main_window_footer_host_state(
                    requested_surface,
                    installed.then_some(config.surface),
                    installed,
                );
            } else {
                clear_main_window_footer_refresh_signature();
                remove_main_window_footer_host(ns_window);
                update_main_window_footer_host_state(None, None, false);
            }
        }
    }

    defer_gpui_footer_overlay_sync(cx, parent_window_handle, parent_bounds, display_id, config);

    #[cfg(not(target_os = "macos"))]
    let _ = (window, config);
}

/// Sync the GPUI footer overlay child window OUTSIDE the caller's draw.
///
/// `sync_main_footer_popup` is called from the main
/// window's `render()`. Opening or drawing another window mid-draw allocates
/// into — and `open_window` then clears — the shared per-App element arena,
/// dangling every element of the in-progress draw (real SIGSEGV: dangling
/// `Rc<InspectorElementPath>` drop in `Drawable::request_layout` on the first
/// hotkey show). Deferring runs the overlay sync after the current update
/// cycle, when no draw is in progress.
fn defer_gpui_footer_overlay_sync(
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
    parent_bounds: Bounds<Pixels>,
    display_id: Option<DisplayId>,
    config: Option<&MainWindowFooterConfig>,
) {
    let config = config.cloned();
    cx.defer(move |cx| {
        if !main_footer_gpui_overlay_active() {
            close_gpui_footer_overlay(cx);
        } else if let Some(config) = config {
            sync_gpui_footer_overlay(cx, parent_window_handle, parent_bounds, display_id, config);
        } else {
            close_gpui_footer_overlay(cx);
        }
    });
}

pub(crate) fn sync_window_footer_popup(
    window: &mut Window,
    config: &MainWindowFooterConfig,
    _cx: &mut App,
) {
    sync_footer_owner(window, Some(config));
    notify_changed_footer_overlay(window.window_handle(), _cx);
    if window.is_owned_hidden() {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) {
        // SAFETY: this is the current live GPUI window on the AppKit thread.
        unsafe {
            if ensure_reusable_window_footer_host(ns_window)
                && refresh_window_footer_host(ns_window, config)
            {
                mark_footer_installed(window.window_handle(), true);
            }
        }
    }
}

/// Remove the reusable native footer host associated with THIS window (not the
/// main-window global). This is the symmetric teardown to
/// [`sync_window_footer_popup`]: it tears down only the footer-effect subview
/// on this window's own NSWindow via `removeFromSuperview`, leaving the native
/// NSVisualEffectView blur trio on every OTHER window untouched. Agent Chat
/// calls this when its resolved footer owner transitions away from the native
/// spacer (to an inline rail or an external host) so a detached window never
/// leaves an orphan native footer host behind.
///
/// It is a no-op when no host is installed on this window (`remove_reusable_window_footer_host`
/// finds no matching subview and returns), so callers may invoke it defensively
/// on any non-native owner.
pub(crate) fn clear_window_footer_popup(window: &mut Window, cx: &mut App) {
    close_footer_overlay_for_parent(window.window_handle(), cx);
    sync_footer_owner(window, None);
    if window.is_owned_hidden() {
        return;
    }
    #[cfg(target_os = "macos")]
    if let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) {
        // SAFETY: this is the current live GPUI window on the AppKit thread.
        unsafe {
            remove_reusable_window_footer_host(ns_window);
        }
    }
}

pub(crate) fn close_main_footer_popup(cx: &mut App) {
    clear_main_window_footer_refresh_signature();
    update_main_window_footer_host_state(None, None, false);
    close_gpui_footer_overlay(cx);

    let Some(window_handle) = crate::get_main_window_handle() else {
        return;
    };

    let _ = window_handle.update(cx, move |_, window, _cx| {
        sync_footer_owner(window, None);
        if window.is_owned_hidden() {
            return;
        }
        #[cfg(target_os = "macos")]
        {
            let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
                return;
            };

            // SAFETY: `ns_window` comes from the live GPUI main window on the
            // AppKit main thread while `update_window` is executing.
            unsafe {
                remove_main_window_footer_host(ns_window);
            }
        }

        #[cfg(not(target_os = "macos"))]
        let _ = window;
    });
}

/// Remove the main footer host only after WindowServer has retired the final
/// ordered-out surface. AppKit reports `isVisible == false` before the last
/// composited frame is necessarily gone; mutating the hidden host immediately
/// can therefore leak one fallback-footer frame into an exit filmstrip.
///
/// A rapid re-show advances the visibility generation and leaves the reusable
/// host intact, so hammering the launcher hotkey cannot race a stale cleanup.
pub(crate) fn close_main_footer_popup_after_hidden_settle(
    cx: &mut gpui::AsyncApp,
    expected_visibility_generation: u64,
) {
    // Glass mode: the footer is an in-window NSGlassEffectContainerView that
    // hides WITH the window. Tearing it down after every ordinary hide forced
    // the next show to recreate its NSGlassEffectView capsules, whose private
    // material re-materializes through the following entry — measured as
    // capsule-vs-main relation drift during entry plus post-settle color
    // movement once every hide route played the calibrated exit (2026-08-13
    // exit-fade restoration receipts). Keep the container installed across
    // hides, exactly like the main glass backdrop; content refresh on the
    // next show already restyles it on theme/content change.
    if glass_scroll_bands_active() {
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_footer_hidden_cleanup_skipped_glass_mode",
            expected_visibility_generation,
            "Kept the in-window glass footer host installed across the hide"
        );
        return;
    }
    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        cx.background_executor()
            .timer(std::time::Duration::from_millis(80))
            .await;
        cx.update(|cx| {
            if crate::is_main_window_visible()
                || crate::main_window_visibility_generation() != expected_visibility_generation
            {
                tracing::info!(
                    target: "script_kit::footer_popup",
                    event = "main_footer_hidden_cleanup_superseded",
                    expected_visibility_generation,
                    current_visibility_generation = crate::main_window_visibility_generation(),
                    "Skipped stale footer cleanup after a rapid main-window re-show"
                );
                return;
            }
            close_main_footer_popup(cx);
        });
    })
    .detach();
}

fn set_gpui_footer_overlay_window_bounds(
    window: &mut Window,
    bounds: Bounds<Pixels>,
    cx: &mut App,
) {
    crate::components::inline_popup_window::set_inline_popup_window_bounds(window, bounds, cx);
}

fn configure_gpui_footer_overlay_window<T: 'static>(
    handle: &WindowHandle<T>,
    cx: &mut App,
    parent_window_handle: AnyWindowHandle,
) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    {
        handle
            .update(cx, move |_overlay, window, cx| {
                window.defer(cx, move |window, cx| {
                    if let Some(ns_window) =
                        crate::components::inline_popup_window::inline_popup_ns_window(window)
                    {
                        // SAFETY: `ns_window` is the live GPUI overlay NSWindow.
                        // The overlay is visual-only; mouse and key focus
                        // must continue to belong to the main launcher window.
                        unsafe {
                            configure_gpui_footer_overlay_ns_window(ns_window);
                        }
                        crate::components::inline_popup_window::attach_inline_popup_to_parent_window(
                            cx,
                            parent_window_handle,
                            ns_window,
                        );
                    }
                });
            })
            .map_err(|_| anyhow::anyhow!("failed to configure GPUI footer overlay window"))?;
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (handle, cx, parent_window_handle);

    Ok(())
}

#[cfg(target_os = "macos")]
unsafe fn configure_gpui_footer_overlay_ns_window(ns_window: id) {
    use cocoa::base::NO;
    use objc::{class, msg_send, sel, sel_impl};

    if ns_window == nil {
        return;
    }

    let clear_color: id = msg_send![class!(NSColor), clearColor];
    if clear_color != nil {
        let _: () = msg_send![ns_window, setBackgroundColor: clear_color];
    }
    let _: () = msg_send![ns_window, setOpaque: NO];
    let _: () = msg_send![ns_window, setHasShadow: NO];
    let _: () = msg_send![ns_window, setIgnoresMouseEvents: NO];
    let _: () = msg_send![ns_window, setBecomesKeyOnlyIfNeeded: YES];
    let _: () = msg_send![ns_window, setMovable: NO];
    let _: () = msg_send![ns_window, setMovableByWindowBackground: NO];
    let _: () = msg_send![ns_window, setAnimationBehavior: 2isize];
    let _: () = msg_send![ns_window, setRestorable: NO];

    // SAFETY: `ns_window` is a live NSWindow owned by the GPUI footer overlay.
    // collectionBehavior / setCollectionBehavior are standard NSWindow accessors.
    let current_collection_behavior: u64 = msg_send![ns_window, collectionBehavior];
    let desired_collection_behavior =
        crate::platform::main_panel_collection_behavior(current_collection_behavior);
    let _: () = msg_send![ns_window, setCollectionBehavior: desired_collection_behavior];

    let title = ns_string(GPUI_FOOTER_OVERLAY_WINDOW_TITLE);
    if title != nil {
        let _: () = msg_send![ns_window, setTitle: title];
    }
}

/// True when the main window should use the Tahoe above-Metal scroll bands.
/// Keeping this policy in the native-band owner prevents AppKit installation,
/// GPUI insets, and scroll receipts from drifting onto different gates.
pub(crate) fn glass_scroll_bands_active() -> bool {
    crate::platform::tahoe_native_glass_composition_available()
        && crate::theme::get_cached_theme().is_vibrancy_enabled()
}

/// Desktop gap between the main container's bottom edge and the floating
/// footer capsules.
pub(crate) const FLOAT_FOOTER_CONTAINER_GAP_PX: f32 = 8.0;

include!("footer_popup_glass_geometry.rs");

#[cfg(target_os = "macos")]
unsafe fn install_footer_host_view(root: id, width: f64, glass_mode: bool) -> bool {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if root == nil {
        return false;
    }
    let footer_frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, footer_height()));
    let footer_cls = if glass_mode {
        if let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") {
            log_glass_effect_view_properties_once(glass_class);
        }
        float_footer_host_view_class().unwrap_or_else(footer_effect_view_class)
    } else {
        footer_effect_view_class()
    };
    let footer_view: id = msg_send![footer_cls, alloc];
    let footer_view: id = msg_send![footer_view, initWithFrame: footer_frame];
    if footer_view == nil {
        return false;
    }

    let effect_identifier = ns_string(FOOTER_EFFECT_ID);
    if effect_identifier != nil {
        let _: () = msg_send![footer_view, setIdentifier: effect_identifier];
    }
    let _: () = msg_send![footer_view, setAutoresizingMask: NSViewWidthSizable];
    let _: () = msg_send![footer_view, setWantsLayer: YES];

    let divider_view: id = msg_send![class!(NSView), alloc];
    let divider_view: id = msg_send![
        divider_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, footer_height() - 1.0),
            NSSize::new(width, 1.0)
        )
    ];
    if divider_view != nil {
        let divider_identifier = ns_string(FOOTER_DIVIDER_ID);
        if divider_identifier != nil {
            let _: () = msg_send![divider_view, setIdentifier: divider_identifier];
        }
        let _: () = msg_send![divider_view, setAutoresizingMask: NSViewWidthSizable];
        let _: () = msg_send![divider_view, setWantsLayer: YES];
        let _: () = msg_send![footer_view, addSubview: divider_view];
    }

    let hints_view: id = msg_send![class!(NSView), alloc];
    let hints_view: id = msg_send![hints_view, initWithFrame: footer_hints_frame(width)];
    if hints_view != nil {
        let hints_identifier = ns_string(FOOTER_HINTS_ID);
        if hints_identifier != nil {
            let _: () = msg_send![hints_view, setIdentifier: hints_identifier];
        }
        let _: () = msg_send![hints_view, setAutoresizingMask: NSViewWidthSizable];
        let _: () = msg_send![footer_view, addSubview: hints_view];
    }

    let initial_hints = footer_hints_frame(width);
    let initial_lanes =
        resolve_native_footer_lanes(initial_hints.size.width, 0.0, initial_hints.size.width);
    let left_info_view: id = msg_send![footer_passthrough_view_class(), alloc];
    let left_info_view: id =
        msg_send![left_info_view, initWithFrame: footer_left_info_frame(initial_lanes)];
    if left_info_view != nil {
        let left_info_id = ns_string(FOOTER_LEFT_INFO_ID);
        if left_info_id != nil {
            let _: () = msg_send![left_info_view, setIdentifier: left_info_id];
        }
        let _: () = msg_send![left_info_view, setAutoresizingMask: NSViewWidthSizable];
        let _: () = msg_send![footer_view, addSubview: left_info_view];
    }

    let _: () = msg_send![root, addSubview: footer_view positioned: 1isize relativeTo: nil];
    find_subview_by_identifier(root, FOOTER_EFFECT_ID) != nil
}

#[cfg(target_os = "macos")]
unsafe fn ensure_main_window_footer_host(gpui_view: id, ns_window: id) -> bool {
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("ensure_main_window_footer_host") {
        return false;
    }
    sync_main_window_glass_scroll_bands(ns_window);
    let glass_mode = glass_scroll_bands_active();
    let root = if glass_mode {
        ensure_main_window_footer_glass_container(gpui_view, ns_window)
    } else {
        msg_send![ns_window, contentView]
    };
    if root == nil {
        return false;
    }
    if find_subview_by_identifier(root, FOOTER_EFFECT_ID) != nil {
        return true;
    }
    let bounds: NSRect = msg_send![root, bounds];
    let installed = install_footer_host_view(root, bounds.size.width, glass_mode);
    if installed {
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "main_window_same_host_footer_installed",
            height = footer_height(),
            "Installed detached footer container inside the main NSWindow"
        );
    }
    installed
}

#[cfg(target_os = "macos")]
unsafe fn ensure_reusable_window_footer_host(ns_window: id) -> bool {
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("ensure_reusable_window_footer_host") {
        return false;
    }

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return false;
    }

    let existing = find_subview_by_identifier(
        reusable_window_footer_search_root(ns_window),
        FOOTER_EFFECT_ID,
    );
    if existing != nil {
        return true;
    }

    let glass_mode = glass_scroll_bands_active();
    let root = if glass_mode {
        // Preserve the detached-window contract for reusable secondary
        // surfaces. Only the production main window migrates in MWND-03.
        let child = ensure_float_footer_child_window(ns_window);
        if child == nil {
            nil
        } else {
            msg_send![child, contentView]
        }
    } else {
        content_view
    };
    if root == nil {
        return false;
    }
    let root_bounds: NSRect = msg_send![root, bounds];
    let installed = install_footer_host_view(root, root_bounds.size.width, glass_mode);
    if !installed {
        return false;
    }

    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_host_installed",
        "Installed native footer host"
    );
    if glass_mode {
        sync_float_footer_child_frame(ns_window);
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "glass_footer_float_host_installed",
            height = footer_height(),
            "Installed floating footer host in the non-activating child window below the container"
        );
    }

    find_subview_by_identifier(
        reusable_window_footer_search_root(ns_window),
        FOOTER_EFFECT_ID,
    ) != nil
}

/// Hex (0xRRGGBB) for the native footer's keycap borders.
#[cfg(target_os = "macos")]
fn footer_keycap_hex(theme: &crate::theme::Theme) -> u32 {
    theme.colors.text.primary
}

/// Border alpha for the native footer's keycaps.
#[cfg(target_os = "macos")]
fn footer_keycap_border_alpha(theme: &crate::theme::Theme, selected: bool) -> f64 {
    crate::components::footer_chrome::themed_footer_button_border_alpha(theme, selected) as f64
}

/// Resting button-background rgba for the current main-menu theme.
#[cfg(target_os = "macos")]
fn footer_button_rest_fill_rgba(theme: &crate::theme::Theme) -> Option<u32> {
    crate::components::footer_chrome::themed_footer_button_rest_rgba(theme)
}

/// Active/selected background rgba for a footer button.
#[cfg(target_os = "macos")]
fn footer_button_active_fill_rgba(_action: FooterAction, theme: &crate::theme::Theme) -> u32 {
    crate::components::footer_chrome::themed_footer_button_active_rgba(theme)
}

fn resolved_native_footer_button_state(
    selected: bool,
    hovered: bool,
    actions_window_open: bool,
    is_actions: bool,
) -> crate::theme::MainMenuRowState {
    crate::theme::main_menu_row_state_from_flags(
        selected || (is_actions && actions_window_open),
        hovered,
    )
}

/// Packed RGBA for the native footer's top divider line. Replaces the default
/// divider color with a strong accent line when the footer-divider axis is on.
#[cfg(target_os = "macos")]
fn footer_divider_rgba(theme: &crate::theme::Theme, default_rgba: u32) -> u32 {
    let footer = crate::designs::current_main_menu_theme().def().footer;
    if footer.divider_alpha == 0 {
        0
    } else if footer.divider_accent {
        (crate::theme::AppChromeColors::from_theme(theme).accent_hex << 8) | footer.divider_alpha
    } else if footer.divider_alpha < 0xFF {
        (theme.colors.text.primary << 8) | footer.divider_alpha
    } else {
        default_rgba
    }
}

/// Refresh the main-window footer host. When the GPUI footer overlay is
/// enabled (the default), AppKit keeps only the material/divider and the
/// overlay child window owns the glyphs.
#[cfg(target_os = "macos")]
unsafe fn refresh_main_footer_host(ns_window: id, config: &MainWindowFooterConfig) -> bool {
    sync_main_window_glass_scroll_bands(ns_window);
    refresh_footer_host_impl(
        ns_window,
        main_window_footer_search_root(ns_window),
        config,
        gpui_footer_overlay_enabled() && !glass_scroll_bands_active(),
    )
}

/// Refresh a reusable (non-main) window footer host — detached Agent Chat and
/// the dictation overlay. These windows have no GPUI overlay child, so AppKit
/// always renders the glyphs natively.
#[cfg(target_os = "macos")]
unsafe fn refresh_window_footer_host(ns_window: id, config: &MainWindowFooterConfig) -> bool {
    sync_float_footer_child_frame(ns_window);
    refresh_footer_host_impl(
        ns_window,
        reusable_window_footer_search_root(ns_window),
        config,
        false,
    )
}

#[cfg(target_os = "macos")]
unsafe fn sync_native_view_tree_contents_scale(view: id, contents_scale: f64) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil || !contents_scale.is_finite() || contents_scale <= 0.0 {
        return;
    }
    let layer: id = msg_send![view, layer];
    if layer != nil {
        let _: () = msg_send![layer, setContentsScale: contents_scale];
    }
    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        sync_native_view_tree_contents_scale(child, contents_scale);
    }
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_host_impl(
    ns_window: id,
    search_root: id,
    config: &MainWindowFooterConfig,
    gpui_overlay_owns_glyphs: bool,
) -> bool {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("refresh_main_footer_host") {
        return false;
    }

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return false;
    }

    let footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
    if footer_view == nil {
        return false;
    }
    let Some((owner, _, _)) = native_footer_binding(ns_window) else {
        return false;
    };
    // Resolve required peers before changing ownership or publishing a cache hit.
    // A partially removed host must be retried, never remembered as refreshed.
    let divider_view = find_subview_by_identifier(footer_view, FOOTER_DIVIDER_ID);
    let hints_view = find_subview_by_identifier(footer_view, FOOTER_HINTS_ID);
    let left_info_view = find_subview_by_identifier(footer_view, FOOTER_LEFT_INFO_ID);
    if divider_view == nil || hints_view == nil || left_info_view == nil {
        clear_footer_refresh_signature(owner);
        return false;
    }
    let theme_snapshot = crate::theme::get_theme_snapshot();
    {
        let mut hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        let Some(host) = hosts.get_mut(&owner.window_id()) else {
            return false;
        };
        let theme_revision = theme_snapshot.revision;
        let replaced = host.native_view != footer_view as usize;
        let theme_changed = host
            .binding
            .as_ref()
            .is_some_and(|binding| binding.theme_revision != theme_revision);
        if replaced || theme_changed {
            host.presentation_revision += 1;
            host.native_token = next_footer_lifetime();
            if replaced {
                let previous_generation = host.host_generation;
                host.host_generation = next_footer_lifetime();
                host.refresh_signature = None;
                for (parent, generation, _) in FLOAT_FOOTER_WINDOWS
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .iter_mut()
                {
                    if *parent == ns_window as usize && *generation == previous_generation {
                        *generation = host.host_generation;
                    }
                }
            }
            host.native_view = footer_view as usize;
            if let Some(binding) = host.binding.as_mut() {
                binding.host_generation = host.host_generation;
                binding.theme_revision = theme_revision;
                binding.presentation_revision = host.presentation_revision;
            }
        }
    }
    let Some((refresh_owner, refresh_binding, refresh_token)) = native_footer_binding(ns_window)
    else {
        return false;
    };
    let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
    sync_native_view_tree_contents_scale(footer_view, backing_scale);
    let footer_is_glass = objc::runtime::Class::get("NSGlassEffectView")
        .map(|glass_class| {
            let is_glass: cocoa::base::BOOL = msg_send![footer_view, isKindOfClass: glass_class];
            is_glass == YES
        })
        .unwrap_or(false);

    let theme = theme_snapshot.theme.as_ref();
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    let is_dark = theme.should_use_dark_vibrancy();
    let material = match theme.get_vibrancy().material {
        crate::theme::VibrancyMaterial::Hud => {
            crate::platform::ns_visual_effect_material::HUD_WINDOW
        }
        crate::theme::VibrancyMaterial::Popover => {
            crate::platform::ns_visual_effect_material::POPOVER
        }
        crate::theme::VibrancyMaterial::Menu => crate::platform::ns_visual_effect_material::MENU,
        crate::theme::VibrancyMaterial::Sidebar => {
            crate::platform::ns_visual_effect_material::SIDEBAR
        }
        crate::theme::VibrancyMaterial::Content => {
            crate::platform::ns_visual_effect_material::CONTENT_BACKGROUND
        }
    };
    let content_bounds: NSRect = msg_send![content_view, bounds];
    let signature = native_footer_refresh_signature(
        config,
        &theme_snapshot,
        content_bounds.size.width,
        gpui_overlay_owns_glyphs,
    );
    let native_visual_theme = signature.native_visual_theme;
    let (
        footer_geometry_changed,
        footer_content_changed,
        footer_visuals_changed,
        effect_theme_changed,
    ) = {
        if !footer_binding_is_live(&refresh_binding, refresh_owner)
            || refresh_binding.theme_revision != signature.theme_revision
            || signature.theme_revision != crate::theme::get_theme_snapshot().revision
        {
            return false;
        }
        let hosts = FOOTER_HOSTS.lock().unwrap_or_else(|p| p.into_inner());
        let Some(host) = hosts.get(&refresh_owner.window_id()) else {
            return false;
        };
        if host.binding.as_ref() != Some(&refresh_binding)
            || host.config.as_ref() != Some(&signature.config)
        {
            return false;
        }
        let guard = host.refresh_signature.as_ref();
        if guard == Some(&signature) {
            return true;
        }
        let footer_geometry_changed = guard
            .as_ref()
            .map(|previous| previous.content_width_bits != signature.content_width_bits)
            .unwrap_or(true);
        let footer_content_changed = guard
            .as_ref()
            .map(|previous| {
                previous.config != signature.config
                    || previous.content_width_bits != signature.content_width_bits
                    || previous.gpui_overlay_owns_glyphs != signature.gpui_overlay_owns_glyphs
                    || previous.button_leading_dot_hexes != signature.button_leading_dot_hexes
                    // Theme cycling must fully rebuild the hint buttons so the
                    // keycap borders and label text pick up the new tokens (the
                    // lighter visuals-only recolor path doesn't reach every
                    // AppKit subview reliably).
                    || previous.main_menu_theme != signature.main_menu_theme
                    || previous.theme_revision != signature.theme_revision
            })
            .unwrap_or(true);
        let footer_visuals_changed = guard
            .as_ref()
            .map(|previous| {
                previous.divider_rgba != signature.divider_rgba
                    || previous.text_primary_hex != signature.text_primary_hex
                    || previous.background_hex != signature.background_hex
                    || previous.glass_tint_opacity_bits != signature.glass_tint_opacity_bits
                    || previous.accent_hex != signature.accent_hex
                    || previous.selection_rgba != signature.selection_rgba
                    || previous.hover_rgba != signature.hover_rgba
                    || previous.left_dot_hex != signature.left_dot_hex
                    || previous.native_glass_signature != signature.native_glass_signature
                    || previous.native_visual_theme != signature.native_visual_theme
                    || previous.main_menu_theme != signature.main_menu_theme
            })
            .unwrap_or(true);
        let effect_theme_changed = guard
            .as_ref()
            .map(|previous| {
                previous.dark != signature.dark
                    || previous.material != signature.material
                    || previous.native_glass_signature != signature.native_glass_signature
            })
            .unwrap_or(true);
        (
            footer_geometry_changed,
            footer_content_changed,
            footer_visuals_changed,
            effect_theme_changed,
        )
    };

    if effect_theme_changed {
        let appearance_name = if is_dark {
            ns_string("NSAppearanceNameVibrantDark")
        } else {
            ns_string("NSAppearanceNameVibrantLight")
        };
        if appearance_name != nil {
            let appearance: id = msg_send![class!(NSAppearance), appearanceNamed: appearance_name];
            if appearance != nil {
                let _: () = msg_send![footer_view, setAppearance: appearance];
            }
        }

        let footer_is_vev = objc::runtime::Class::get("NSVisualEffectView")
            .map(|vev_class| {
                let is_vev: cocoa::base::BOOL = msg_send![footer_view, isKindOfClass: vev_class];
                is_vev == YES
            })
            .unwrap_or(false);
        if footer_is_vev {
            let _: () = msg_send![footer_view, setMaterial: material];
            let _: () = msg_send![footer_view, setState: 1isize];
            let _: () = msg_send![footer_view, setBlendingMode: 1isize];
            let _: () = msg_send![footer_view, setEmphasized: is_dark];
        }
    }

    if footer_geometry_changed {
        let footer_frame = NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(content_bounds.size.width, footer_height()),
        );
        let _: () = msg_send![footer_view, setFrame: footer_frame];

        if footer_is_glass {
            // Square band: rounding the glass view rounds its TOP corners
            // too (user-rejected); the window's own corner mask supplies the
            // bottom rounding.
            let _: () = msg_send![footer_view, setCornerRadius: 0.0_f64];
        } else {
            let footer_layer: id = msg_send![footer_view, layer];
            if footer_layer != nil {
                let _: () = msg_send![footer_layer, setCornerRadius: 0.0_f64];
                let _: () = msg_send![footer_layer, setMasksToBounds: YES];
            }
        }
    }

    if divider_view != nil {
        // The GPUI overlay supplies the visible footer controls above this
        // native material host. Hide only the hard divider in that mode so
        // the controls read as floating, while keeping the identified view
        // installed for fidelity inspection and the native-only fallback.
        let divider_hidden =
            if gpui_overlay_owns_glyphs || footer_is_glass || glass_scroll_bands_active() {
                YES
            } else {
                NO
            };
        let _: () = msg_send![divider_view, setHidden: divider_hidden];

        if footer_geometry_changed {
            let divider_frame = NSRect::new(
                NSPoint::new(0.0, footer_height() - 1.0),
                NSSize::new(content_bounds.size.width, 1.0),
            );
            let _: () = msg_send![divider_view, setFrame: divider_frame];
        }
        let divider_layer: id = msg_send![divider_view, layer];
        if divider_layer != nil {
            let divider_color = ns_color_from_rgba(footer_divider_rgba(theme, chrome.divider_rgba));
            if divider_color != nil {
                let cg_color: id = msg_send![divider_color, CGColor];
                if cg_color != nil {
                    let _: () = msg_send![divider_layer, setBackgroundColor: cg_color];
                }
            }
        }
    }

    let text_color =
        ns_color_from_rgba(native_visual_theme.row_palette.rest.primary_foreground_rgba);

    let default_hints_frame = footer_hints_frame(content_bounds.size.width);
    let mut native_footer_lanes = resolve_native_footer_lanes(
        default_hints_frame.size.width,
        0.0,
        default_hints_frame.size.width,
    );
    if hints_view != nil {
        if footer_content_changed {
            let _: () = msg_send![hints_view, setFrame: default_hints_frame];
            if gpui_overlay_owns_glyphs {
                // Sandwich layering: AppKit keeps only the material/divider
                // while GPUI owns the footer glyphs in a child overlay window
                // above this footer host.
                native_footer_lanes = layout_footer_hints(hints_view, text_color, &[], theme);
            } else {
                native_footer_lanes =
                    layout_footer_hints(hints_view, text_color, &config.buttons, theme);
            }
        } else if footer_visuals_changed {
            recolor_footer_hint_subviews_with_visual_theme(hints_view, theme, native_visual_theme);
            native_footer_lanes = measure_native_footer_lanes(hints_view, &config.buttons);
        } else {
            native_footer_lanes = measure_native_footer_lanes(hints_view, &config.buttons);
        }
        if footer_content_changed || footer_visuals_changed || effect_theme_changed {
            restyle_footer_glass_capsules(hints_view, theme);
            refresh_footer_button_visual_states_with_theme(hints_view, native_visual_theme);
        }
    }

    // Left info (streaming dot + model name)
    if left_info_view != nil {
        if footer_content_changed {
            let _: () = msg_send![
                left_info_view,
                setFrame: footer_left_info_frame(native_footer_lanes)
            ];
        }
        if footer_content_changed || (footer_visuals_changed && config.left_info.is_some()) {
            if gpui_overlay_owns_glyphs {
                layout_footer_left_info(left_info_view, None, text_color);
            } else {
                layout_footer_left_info(left_info_view, config.left_info.as_ref(), text_color);
            }
            refresh_footer_button_visual_states_with_theme(left_info_view, native_visual_theme);
        }
        if footer_content_changed || footer_visuals_changed || effect_theme_changed {
            restyle_footer_glass_capsules(left_info_view, theme);
        }
    }
    // Content nodes can be created by the layout paths above. Apply the
    // current Retina scale after construction as well as before refresh so
    // newly allocated keycap, border, icon, and status-dot layers are sharp on
    // their first frame.
    sync_native_view_tree_contents_scale(footer_view, backing_scale);
    invalidate_footer_effect_view_theme(
        footer_view,
        effect_theme_changed
            || footer_geometry_changed
            || footer_content_changed
            || footer_visuals_changed,
    );

    tracing::debug!(
        target: "script_kit::footer_popup",
        event = "native_footer_host_refreshed",
        surface = config.surface,
        button_count = config.buttons.len(),
        width = content_bounds.size.width,
        height = footer_height(),
        dark = is_dark,
        footer_geometry_changed,
        footer_content_changed,
        footer_visuals_changed,
        effect_theme_changed,
        "Refreshed native footer host"
    );
    bind_native_footer_buttons(footer_view, refresh_token);
    commit_footer_refresh(refresh_owner, &refresh_binding, signature)
}

#[cfg(target_os = "macos")]
unsafe fn invalidate_footer_effect_view_theme(footer_view: id, effect_theme_changed: bool) {
    use objc::{msg_send, sel, sel_impl};

    if footer_view != nil && effect_theme_changed {
        let _: () = msg_send![footer_view, setNeedsLayout: YES];
        let _: () = msg_send![footer_view, setNeedsDisplay: YES];

        let footer_layer: id = msg_send![footer_view, layer];
        if footer_layer != nil {
            let _: () = msg_send![footer_layer, setNeedsDisplay];
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn remove_main_window_footer_host(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("remove_main_window_footer_host") {
        return;
    }

    clear_main_window_footer_refresh_signature();

    let footer_view =
        find_subview_by_identifier(main_window_footer_search_root(ns_window), FOOTER_EFFECT_ID);
    if footer_view != nil {
        let _: () = msg_send![footer_view, removeFromSuperview];
    }
    remove_main_window_footer_glass_container(ns_window);
}

#[cfg(target_os = "macos")]
unsafe fn remove_reusable_window_footer_host(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("remove_reusable_window_footer_host") {
        return;
    }

    let footer_view = find_subview_by_identifier(
        reusable_window_footer_search_root(ns_window),
        FOOTER_EFFECT_ID,
    );
    if footer_view != nil {
        let _: () = msg_send![footer_view, removeFromSuperview];
    }
    remove_float_footer_child_window(ns_window);
}

#[cfg(target_os = "macos")]
fn footer_identifier_is_entry_capsule(identifier: &str) -> bool {
    identifier == FOOTER_LEFT_INFO_CAPSULE_ID
        || (identifier.starts_with(FOOTER_HINT_CAPSULE_ID_PREFIX)
            && !identifier.starts_with(FOOTER_HINT_CAPSULE_CONTENT_ID_PREFIX))
}

#[cfg(target_os = "macos")]
unsafe fn collect_main_window_footer_entry_capsules(view: id, capsules: &mut Vec<id>) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let is_glass_capsule = appkit_view_identifier(view)
        .as_deref()
        .is_some_and(footer_identifier_is_entry_capsule)
        && objc::runtime::Class::get("NSGlassEffectView")
            .map(|glass_class| {
                let is_glass: cocoa::base::BOOL = msg_send![view, isKindOfClass: glass_class];
                is_glass == YES
            })
            .unwrap_or(false);
    if is_glass_capsule {
        capsules.push(view);
        return;
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        collect_main_window_footer_entry_capsules(child, capsules);
    }
}

/// The alpha-only native footer target for the main-window entry, or `nil`
/// when no same-host footer is installed.
///
/// The outer container may inherit the owning NSWindow's alpha and geometry,
/// but it must NEVER receive an entry layer filter: it spans the transparent
/// inter-capsule gaps. Defocus targets are returned separately by
/// [`main_window_footer_entry_capsules`].
#[cfg(target_os = "macos")]
pub(crate) unsafe fn main_window_footer_entry_alpha_target(ns_window: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    if ns_window == nil {
        return nil;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return nil;
    }
    let container = find_subview_by_identifier(content_view, FOOTER_GLASS_CONTAINER_ID);
    if container != nil {
        return container;
    }
    find_subview_by_identifier(content_view, FOOTER_EFFECT_ID)
}

/// Return only the clipped NSGlassEffectView capsules that may receive the
/// main window's onset defocus. The footer container, hints host, capsule
/// content views, state layers, and transparent gaps are excluded by a
/// closed-world identifier and class check.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn main_window_footer_entry_capsules(ns_window: id) -> Vec<id> {
    let root = main_window_footer_entry_alpha_target(ns_window);
    if root == nil {
        return Vec::new();
    }
    let mut capsules = Vec::new();
    collect_main_window_footer_entry_capsules(root, &mut capsules);
    capsules
}

/// Cancel and clear an in-flight per-capsule entry defocus. This is called by
/// the same exit-supersession path that clears the main backdrop filter, so a
/// rapid re-show cannot leave stale capsule filters or ramps behind.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn clear_main_window_footer_entry_capsule_effects(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    let nil_filters: id = nil;
    let animation_key = ns_string("entryBlurRamp");
    for capsule in main_window_footer_entry_capsules(ns_window) {
        let layer: id = msg_send![capsule, layer];
        if layer == nil {
            continue;
        }
        let _: () = msg_send![layer, setFilters: nil_filters];
        if animation_key != nil {
            let _: () = msg_send![layer, removeAnimationForKey: animation_key];
        }
    }
}

include!("footer_popup_native_layout.rs");
#[cfg(target_os = "macos")]
fn footer_height() -> f64 {
    crate::components::footer_chrome::current_main_menu_footer_height() as f64
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_left_info(
    left_info_view: id,
    left_info: Option<&FooterLeftInfo>,
    text_color: id,
) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    let bounds: NSRect = msg_send![left_info_view, bounds];
    let Some(info) = left_info else {
        remove_identified_subview(left_info_view, FOOTER_STATUS_DOT_ID);
        remove_identified_subview(left_info_view, FOOTER_MODEL_LABEL_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_PROFILE_ICON_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_ICON_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_KEYCAP_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_KEYCAP_ID);
        ensure_footer_left_info_capsule(left_info_view, 0.0, bounds.size.height);
        return;
    };

    let (visual_parent, visual_offset_x, visual_offset_y) =
        ensure_footer_left_info_visual_parent(left_info_view, bounds.size.height);
    // Measure the actual AppKit content before choosing a degradation mode.
    // Fixed icon/keycap widths are reserved first; labels only receive the
    // remaining explicit budget, so no label can push a keycap or hit target
    // into the trailing action lane.
    let cwd_label_width = info
        .cwd_chip
        .as_ref()
        .map(|cwd| {
            let label =
                ensure_footer_cwd_chip_label(left_info_view, visual_parent, &cwd.label, text_color);
            if label == nil {
                0.0
            } else {
                let size: NSSize = msg_send![label, fittingSize];
                size.width
            }
        })
        .unwrap_or(0.0);
    let primary_label_width = if info.model_name.is_empty() {
        0.0
    } else {
        let label =
            ensure_footer_model_label(left_info_view, visual_parent, &info.model_name, text_color);
        if label == nil {
            0.0
        } else {
            let size: NSSize = msg_send![label, fittingSize];
            size.width
        }
    };
    let cwd_keycap_width = info
        .cwd_chip
        .as_ref()
        .and_then(|cwd| cwd.key.as_deref())
        .filter(|key| !key.trim().is_empty())
        .map(|key| {
            layout_footer_left_keycap(
                left_info_view,
                visual_parent,
                FOOTER_CWD_CHIP_KEYCAP_ID,
                FOOTER_CWD_CHIP_KEYCAP_GLYPH_ID,
                key,
                0.0,
                bounds.size.height,
                visual_offset_x,
                visual_offset_y,
                text_color,
            )
        })
        .unwrap_or(0.0);
    let primary_keycap_width = info
        .keycap
        .as_deref()
        .filter(|key| !key.trim().is_empty())
        .map(|key| {
            layout_footer_left_keycap(
                left_info_view,
                visual_parent,
                FOOTER_LEFT_INFO_KEYCAP_ID,
                FOOTER_LEFT_INFO_KEYCAP_GLYPH_ID,
                key,
                0.0,
                bounds.size.height,
                visual_offset_x,
                visual_offset_y,
                text_color,
            )
        })
        .unwrap_or(0.0);
    let primary_marker_width = if info.icon_token.is_some() {
        FOOTER_LEFT_PROFILE_ICON_SIZE + FOOTER_LEFT_DOT_LABEL_GAP
    } else if !matches!(info.dot_status, FooterDotStatus::Hidden) {
        FOOTER_STREAMING_DOT_SIZE + FOOTER_LEFT_DOT_LABEL_GAP
    } else {
        0.0
    };
    let measured = FooterLeftInfoMeasurements {
        cwd_fixed_width: if info.cwd_chip.is_some() {
            FOOTER_LEFT_PROFILE_ICON_SIZE
                + FOOTER_LEFT_DOT_LABEL_GAP
                + if cwd_keycap_width > 0.0 {
                    FOOTER_HINT_KEY_LABEL_GAP + cwd_keycap_width
                } else {
                    0.0
                }
                + FOOTER_CWD_CHIP_TRAILING_GAP_PX
        } else {
            0.0
        },
        cwd_label_width,
        primary_fixed_width: primary_marker_width
            + primary_keycap_width
            + if primary_keycap_width > 0.0 && primary_label_width > 0.0 {
                FOOTER_HINT_KEY_LABEL_GAP
            } else {
                0.0
            },
        primary_label_width,
        has_cwd: info.cwd_chip.is_some(),
        primary_visible_without_label: primary_marker_width + primary_keycap_width > 0.0,
    };
    let allocation = resolve_footer_left_info_allocation(bounds.size.width, measured);
    record_footer_left_allocation(allocation);
    if matches!(allocation.degradation, FooterLeftInfoDegradation::Hidden) {
        remove_identified_subview(left_info_view, FOOTER_STATUS_DOT_ID);
        remove_identified_subview(left_info_view, FOOTER_MODEL_LABEL_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_PROFILE_ICON_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_ICON_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_KEYCAP_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_KEYCAP_ID);
        ensure_footer_left_info_capsule(left_info_view, 0.0, bounds.size.height);
        return;
    }
    let mut x = 0.0_f64;

    // ── CWD chip (always on the far left, independent of model marker) ──
    let show_cwd = matches!(
        allocation.degradation,
        FooterLeftInfoDegradation::Full
            | FooterLeftInfoDegradation::TruncatedLabels
            | FooterLeftInfoDegradation::CwdAffordanceOnly
    );
    let show_primary = !matches!(allocation.degradation, FooterLeftInfoDegradation::Hidden);
    let show_cwd_label = allocation.cwd_label_width > 0.0;
    let show_primary_label = allocation.primary_label_width > 0.0;
    let max_content_x = allocation.available_width;

    if let Some(cwd_chip) = info.cwd_chip.as_ref().filter(|_| show_cwd) {
        let chip_start_x = x;

        // Folder icon.
        let icon_view = ensure_footer_cwd_chip_icon_view(left_info_view, visual_parent);
        if icon_view != nil {
            let image = footer_icon_image(&cwd_chip.icon_token);
            if image != nil {
                let _: () = msg_send![icon_view, setImage: image];
            }
            let icon_y = ((bounds.size.height - FOOTER_LEFT_PROFILE_ICON_SIZE) / 2.0).round();
            let _: () = msg_send![
                icon_view,
                setFrame: NSRect::new(
                    NSPoint::new(x + visual_offset_x, icon_y + visual_offset_y),
                    NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
                )
            ];
            let _: () = msg_send![icon_view, setHidden: NO];
            let icon_layer: id = msg_send![icon_view, layer];
            if icon_layer != nil {
                update_footer_icon_layer(icon_layer, info);
            }
            x += FOOTER_LEFT_PROFILE_ICON_SIZE + FOOTER_LEFT_DOT_LABEL_GAP;
        }

        let label = if show_cwd_label {
            ensure_footer_cwd_chip_label(left_info_view, visual_parent, &cwd_chip.label, text_color)
        } else {
            remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
            nil
        };
        if label != nil && x < max_content_x {
            let label_size: NSSize = msg_send![label, fittingSize];
            let label_width = label_size.width.min(allocation.cwd_label_width);
            let label_y = ((bounds.size.height - label_size.height) / 2.0).round();
            let _: () = msg_send![label, setLineBreakMode: 4usize];
            let _: () = msg_send![
                label,
                setFrame: NSRect::new(
                    NSPoint::new(x + visual_offset_x, label_y + visual_offset_y),
                    NSSize::new(label_width, label_size.height),
                )
            ];
            x += label_width;
        }

        if let Some(key_glyph) = cwd_chip.key.as_deref().filter(|key| !key.trim().is_empty()) {
            x += FOOTER_HINT_KEY_LABEL_GAP;
            x += layout_footer_left_keycap(
                left_info_view,
                visual_parent,
                FOOTER_CWD_CHIP_KEYCAP_ID,
                FOOTER_CWD_CHIP_KEYCAP_GLYPH_ID,
                key_glyph,
                x,
                bounds.size.height,
                visual_offset_x,
                visual_offset_y,
                text_color,
            );
        } else {
            remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_KEYCAP_ID);
        }

        // Hit target so clicks dispatch FooterAction::Cwd.
        layout_footer_cwd_chip_hit_target(
            left_info_view,
            NSRect::new(
                NSPoint::new(chip_start_x, 0.0),
                NSSize::new((x - chip_start_x).max(0.0), bounds.size.height),
            ),
            cwd_chip.tooltip.as_deref(),
            info.selected,
            true,
        );

        x += FOOTER_CWD_CHIP_TRAILING_GAP_PX;
    } else {
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_ICON_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_KEYCAP_ID);
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
    }

    let hit_start_x = x;

    // ── Status dot (legacy left-info path only; Agent Chat profile markers pulse the icon) ──
    let show_dot = show_primary
        && info.icon_token.is_none()
        && !matches!(info.dot_status, FooterDotStatus::Hidden);
    if show_dot {
        let dot_y = ((bounds.size.height - FOOTER_STREAMING_DOT_SIZE) / 2.0).round();
        let dot_view = ensure_footer_status_dot_view(left_info_view, visual_parent);
        if dot_view != nil {
            let _: () = msg_send![
                dot_view,
                setFrame: NSRect::new(
                    NSPoint::new(x + visual_offset_x, dot_y + visual_offset_y),
                    NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
                )
            ];
            let dot_layer: id = msg_send![dot_view, layer];
            if dot_layer != nil {
                update_footer_dot_layer(dot_layer, info);
            }
            x += FOOTER_STREAMING_DOT_SIZE + FOOTER_LEFT_DOT_LABEL_GAP;
        }
    } else {
        remove_identified_subview(left_info_view, FOOTER_STATUS_DOT_ID);
    }

    // ── Optional merged profile icon ──
    if let Some(token) = info.icon_token.as_deref().filter(|_| show_primary) {
        let icon_view = ensure_footer_left_profile_icon_view(left_info_view, visual_parent);
        if icon_view != nil {
            let image = footer_icon_image(token);
            if image != nil {
                let _: () = msg_send![icon_view, setImage: image];
            }
            let icon_y = ((bounds.size.height - FOOTER_LEFT_PROFILE_ICON_SIZE) / 2.0).round();
            let _: () = msg_send![
                icon_view,
                setFrame: NSRect::new(
                    NSPoint::new(x + visual_offset_x, icon_y + visual_offset_y),
                    NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
                )
            ];
            let _: () = msg_send![icon_view, setHidden: NO];
            let icon_layer: id = msg_send![icon_view, layer];
            if icon_layer != nil {
                update_footer_icon_layer(icon_layer, info);
            }
            x += FOOTER_LEFT_PROFILE_ICON_SIZE + FOOTER_LEFT_DOT_LABEL_GAP;
        }
    } else {
        remove_identified_subview(left_info_view, FOOTER_LEFT_PROFILE_ICON_ID);
    }

    // Left-aligned markers use the same bordered keycap chrome as trailing
    // footer actions. This was previously rendered only by the GPUI fallback,
    // leaving the native glass owner with a missing shortcut glyph.
    if let Some(keycap) = info
        .keycap
        .as_deref()
        .filter(|key| show_primary && !key.trim().is_empty())
    {
        x += layout_footer_left_keycap(
            left_info_view,
            visual_parent,
            FOOTER_LEFT_INFO_KEYCAP_ID,
            FOOTER_LEFT_INFO_KEYCAP_GLYPH_ID,
            keycap,
            x,
            bounds.size.height,
            visual_offset_x,
            visual_offset_y,
            text_color,
        );
        x += FOOTER_HINT_KEY_LABEL_GAP;
    } else {
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_KEYCAP_ID);
    }

    // ── Model name label ──
    if info.model_name.is_empty() || !show_primary || !show_primary_label {
        remove_identified_subview(left_info_view, FOOTER_MODEL_LABEL_ID);
    } else {
        let label =
            ensure_footer_model_label(left_info_view, visual_parent, &info.model_name, text_color);
        if label != nil {
            let label_size: NSSize = msg_send![label, fittingSize];
            let label_width = label_size.width.min(allocation.primary_label_width);
            let label_y = ((bounds.size.height - label_size.height) / 2.0).round();
            let _: () = msg_send![label, setLineBreakMode: 4usize];
            let _: () = msg_send![
                label,
                setFrame: NSRect::new(
                    NSPoint::new(x + visual_offset_x, label_y + visual_offset_y),
                    NSSize::new(label_width, label_size.height),
                )
            ];
            x += label_width;
        }
    }

    layout_footer_left_info_hit_target(
        left_info_view,
        info.action,
        NSRect::new(
            NSPoint::new(hit_start_x, 0.0),
            NSSize::new((x - hit_start_x).max(0.0), bounds.size.height),
        ),
        info.selected,
        info.action.is_some(),
    );

    ensure_footer_left_info_capsule(left_info_view, x.min(max_content_x), bounds.size.height);
}
/// Attach a repeating CoreAnimation color/opacity pulse for active work.
#[cfg(target_os = "macos")]
unsafe fn add_active_dot_pulse_animation(layer: id) {
    use objc::{class, msg_send, sel, sel_impl};

    // Use ease-in-ease-out for a smooth sine-like curve.
    let timing_name = ns_string("easeInEaseOut");
    let timing: id = if timing_name != nil {
        msg_send![
            class!(CAMediaTimingFunction),
            functionWithName: timing_name
        ]
    } else {
        nil
    };

    let duration: f64 = FOOTER_ACTIVE_DOT_HALF_CYCLE_SECONDS;

    // SAFETY: `layer` is a live CALayer. Keep the pulse visual-only; do not
    // scale the dot because size motion is distracting in the compact footer.
    let opacity_key_path = ns_string("opacity");
    if opacity_key_path != nil {
        let opacity_anim: id =
            msg_send![class!(CABasicAnimation), animationWithKeyPath: opacity_key_path];
        if opacity_anim != nil {
            let from_value: id =
                msg_send![class!(NSNumber), numberWithFloat: FOOTER_ACTIVE_DOT_MIN_OPACITY];
            let to_value: id = msg_send![class!(NSNumber), numberWithFloat: 1.0_f32];

            let _: () = msg_send![opacity_anim, setFromValue: from_value];
            let _: () = msg_send![opacity_anim, setToValue: to_value];
            let _: () = msg_send![opacity_anim, setDuration: duration];
            let _: () = msg_send![opacity_anim, setAutoreverses: YES];
            let _: () = msg_send![opacity_anim, setRepeatCount: f32::INFINITY];
            let _: () = msg_send![opacity_anim, setRemovedOnCompletion: NO];
            if timing != nil {
                let _: () = msg_send![opacity_anim, setTimingFunction: timing];
            }

            let anim_key = ns_string("pulseOpacity");
            if anim_key != nil {
                let _: () = msg_send![layer, addAnimation: opacity_anim forKey: anim_key];
            }
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_hints(
    hints_view: id,
    text_color: id,
    buttons: &[FooterButtonConfig],
    theme: &crate::theme::Theme,
) -> NativeFooterLaneLayout {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    // Remove tracking areas from all buttons BEFORE removing them from the
    // view hierarchy. This prevents use-after-free crashes when AppKit tries
    // to deliver mouseEntered/mouseExited to a deallocated button owner.
    let subviews: id = msg_send![hints_view, subviews];
    if subviews != nil {
        let count: usize = msg_send![subviews, count];
        for index in (0..count).rev() {
            let container: id = msg_send![subviews, objectAtIndex: index];
            if container != nil {
                // Find and clean up tracking areas on any NSButton inside this container.
                let container_subs: id = msg_send![container, subviews];
                if container_subs != nil {
                    let sub_count: usize = msg_send![container_subs, count];
                    for si in 0..sub_count {
                        let child: id = msg_send![container_subs, objectAtIndex: si];
                        if child != nil {
                            let is_button: cocoa::base::BOOL =
                                msg_send![child, isKindOfClass: objc::class!(NSButton)];
                            if is_button == YES {
                                let areas: id = msg_send![child, trackingAreas];
                                if areas != nil {
                                    let ac: usize = msg_send![areas, count];
                                    for ai in (0..ac).rev() {
                                        let area: id = msg_send![areas, objectAtIndex: ai];
                                        let _: () = msg_send![child, removeTrackingArea: area];
                                    }
                                }
                            }
                        }
                    }
                }
                let _: () = msg_send![container, removeFromSuperview];
            }
        }
    }

    let hints_bounds: NSRect = msg_send![hints_view, bounds];
    let font: id = msg_send![
        objc::class!(NSFont),
        systemFontOfSize: crate::components::footer_chrome::current_main_menu_footer_metrics().label_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];

    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    let item_gap = footer_hint_item_gap(glass_scroll_bands_active(), metrics.item_gap_px as f64);
    let mut items = Vec::new();
    let mut trailing_item_width = 0.0_f64;
    for button_cfg in buttons {
        let max_item_width =
            footer_hint_max_item_width(button_cfg.action, hints_bounds.size.width, buttons);
        let item = make_footer_hint_item(button_cfg, font, text_color, max_item_width, theme);
        if item == nil {
            continue;
        }
        let item_frame: NSRect = msg_send![item, frame];
        let target_width = footer_hint_slot_width(button_cfg.action).max(item_frame.size.width);
        let left_pinned = is_footer_left_pinned_button(button_cfg);
        if !left_pinned {
            if trailing_item_width > 0.0 {
                trailing_item_width += item_gap;
            }
            trailing_item_width += target_width;
        }
        items.push((
            item,
            target_width,
            button_cfg.action,
            button_cfg.enabled,
            left_pinned,
        ));
    }

    // Trailing actions own the right edge. Never shift them right to rescue a
    // left lane; if the two clusters exhaust the strip, the left allocation
    // degrades or disappears instead.
    let mut trailing_x = (hints_bounds.size.width - trailing_item_width).max(0.0);
    let trailing_start_x = if trailing_item_width > 0.0 {
        trailing_x
    } else {
        hints_bounds.size.width
    };
    // Left-pinned buttons (e.g. Cwd, then Agent·Model) lay out left-to-right
    // from x=0 so multiple left chips sit side by side instead of overlapping.
    let mut left_x = 0.0;
    for (item, target_width, action, enabled, left_pinned) in items {
        let x = if left_pinned { left_x } else { trailing_x };
        let item_y = metrics.button_padding_y as f64;
        let item_height =
            crate::components::footer_chrome::footer_button_height(hints_bounds.size.height as f32)
                as f64;
        let frame = NSRect::new(
            NSPoint::new(x, item_y),
            NSSize::new(target_width, item_height),
        );
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_item_layout",
            action = footer_action_key(action),
            x,
            y = item_y,
            width = target_width,
            height = item_height,
            enabled,
            "Laid out native footer item slot"
        );
        let _: () = msg_send![item, setFrame: frame];
        let _: () = msg_send![hints_view, addSubview: item];
        if left_pinned {
            left_x += target_width + item_gap;
        } else {
            trailing_x += target_width + item_gap;
        }
    }
    let left_pinned_end_x = if left_x > 0.0 {
        (left_x - item_gap).max(0.0)
    } else {
        0.0
    };
    resolve_native_footer_lanes(hints_bounds.size.width, left_pinned_end_x, trailing_start_x)
}

#[cfg(target_os = "macos")]
unsafe fn measure_native_footer_lanes(
    hints_view: id,
    buttons: &[FooterButtonConfig],
) -> NativeFooterLaneLayout {
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};

    let bounds: NSRect = msg_send![hints_view, bounds];
    let subviews: id = msg_send![hints_view, subviews];
    if subviews == nil {
        return resolve_native_footer_lanes(bounds.size.width, 0.0, bounds.size.width);
    }
    let count: usize = msg_send![subviews, count];
    let mut left_end = 0.0_f64;
    let mut trailing_start = bounds.size.width;
    for (index, button) in buttons.iter().enumerate().take(count) {
        let item: id = msg_send![subviews, objectAtIndex: index];
        let frame: NSRect = msg_send![item, frame];
        if is_footer_left_pinned_button(button) {
            left_end = left_end.max(frame.origin.x + frame.size.width);
        } else {
            trailing_start = trailing_start.min(frame.origin.x);
        }
    }
    resolve_native_footer_lanes(bounds.size.width, left_end, trailing_start)
}

#[cfg(target_os = "macos")]
fn is_footer_left_pinned_button(button_cfg: &FooterButtonConfig) -> bool {
    matches!(button_cfg.placement, FooterPlacement::Leading)
}

/// Extra trailing slack added to a hint item's minimum width. The trailing
/// action buttons reserve a comfortable
/// `FOOTER_TRAILING_ACTION_EXTRA_PADDING_X_PX` so their bordered chrome
/// doesn't crowd the rail edge. The Run button and the left-pinned chips (Cwd /
/// Agent·Model) are start-anchored, so that slack would land as dead space
/// *after* their keycaps — which is exactly what made the left group's gaps look
/// uneven versus the right group. They get no extra padding so every group
/// advances on the same `width + FOOTER_HINT_ITEM_GAP` rule.
fn footer_hint_legacy_extra_padding(button_cfg: &FooterButtonConfig) -> f64 {
    if matches!(button_cfg.action, FooterAction::Run) || is_footer_left_pinned_button(button_cfg) {
        0.0
    } else {
        crate::components::footer_chrome::FOOTER_TRAILING_ACTION_EXTRA_PADDING_X_PX as f64
    }
}

#[cfg(target_os = "macos")]
fn footer_hint_max_item_width(
    action: FooterAction,
    hints_width: f64,
    buttons: &[FooterButtonConfig],
) -> Option<f64> {
    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    let mic_button = buttons
        .iter()
        .find(|button| is_footer_left_pinned_button(button));
    if let Some(mic_button) = mic_button {
        if matches!(action, FooterAction::Ai) && is_footer_left_pinned_button(mic_button) {
            let trailing_reserved_width = buttons
                .iter()
                .filter(|button| !is_footer_left_pinned_button(button))
                .map(|button| footer_hint_slot_width(button.action))
                .sum::<f64>()
                + buttons.len().saturating_sub(1) as f64 * metrics.item_gap_px as f64;
            return Some(
                (hints_width - trailing_reserved_width)
                    .clamp(metrics.ai_slot_width as f64, 220.0)
                    .round(),
            );
        }
    }

    if !matches!(action, FooterAction::Run) {
        return None;
    }

    let gap_width = buttons.len().saturating_sub(1) as f64 * metrics.item_gap_px as f64;
    let reserved_width = buttons
        .iter()
        .filter(|button| !matches!(button.action, FooterAction::Run))
        .map(|button| footer_hint_slot_width(button.action))
        .sum::<f64>()
        + gap_width;

    Some(
        (hints_width - reserved_width)
            .clamp(
                metrics.run_slot_min_width as f64,
                metrics.run_slot_max_width as f64,
            )
            .round(),
    )
}

#[cfg(target_os = "macos")]
fn footer_hint_slot_width(action: FooterAction) -> f64 {
    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    match action {
        FooterAction::Run => metrics.run_slot_min_width,
        FooterAction::Actions => metrics.actions_slot_width,
        FooterAction::Ai => metrics.ai_slot_width,
        FooterAction::Apply => metrics.apply_slot_width,
        FooterAction::Replace => metrics.apply_slot_width,
        FooterAction::Append => metrics.apply_slot_width,
        FooterAction::Copy => metrics.apply_slot_width,
        FooterAction::Expand => metrics.apply_slot_width,
        FooterAction::Retry => metrics.stop_slot_width,
        FooterAction::Close => metrics.close_slot_width,
        FooterAction::Stop => metrics.stop_slot_width,
        FooterAction::PasteResponse => metrics.paste_response_slot_width,
        FooterAction::Cwd => metrics.ai_slot_width,
        FooterAction::AgentModel => metrics.ai_slot_width,
        FooterAction::Tips => metrics.ai_slot_width,
    }
    .into()
}

fn footer_hint_content_layout(
    action: FooterAction,
    item_width: f64,
    label_width: f64,
    key_width: f64,
    content_gap: f64,
    run_padding_x: f64,
) -> (f64, f64, f64) {
    let has_label = label_width > 0.0;
    let has_key = key_width > 0.0;
    let gap_width = if has_label && has_key {
        content_gap
    } else {
        0.0
    };
    let content_width = label_width + gap_width + key_width;

    if matches!(action, FooterAction::Run) {
        let key_x = (item_width - run_padding_x - key_width).round();
        let label_x = (key_x - gap_width - label_width).max(0.0).round();
        return (label_x, key_x, content_width);
    }

    let label_x = ((item_width - content_width) / 2.0).max(0.0).round();
    let key_x = (label_x + label_width + gap_width).round();
    (label_x, key_x, content_width)
}

fn footer_hint_content_layout_for_button(
    button_cfg: &FooterButtonConfig,
    item_width: f64,
    label_width: f64,
    key_width: f64,
    content_gap: f64,
    button_padding_x: f64,
    run_padding_x: f64,
) -> (f64, f64, f64) {
    if matches!(
        button_cfg.action,
        FooterAction::Cwd | FooterAction::AgentModel
    ) {
        // Left-pinned, but label appears LEFT of the keycap to mirror the
        // trailing buttons' "label then key" reading order.
        let gap_width = if label_width > 0.0 && key_width > 0.0 {
            content_gap
        } else {
            0.0
        };
        let label_x = run_padding_x.round();
        let key_x = (label_x + label_width + gap_width).round();
        return (label_x, key_x, label_width + gap_width + key_width);
    }
    if is_footer_left_pinned_button(button_cfg) {
        let gap_width = if label_width > 0.0 && key_width > 0.0 {
            content_gap
        } else {
            0.0
        };
        let key_x = button_padding_x.round();
        let label_x = (key_x + key_width + gap_width).round();
        return (label_x, key_x, label_width + gap_width + key_width);
    }

    footer_hint_content_layout(
        button_cfg.action,
        item_width,
        label_width,
        key_width,
        content_gap,
        run_padding_x,
    )
}

#[cfg(target_os = "macos")]
fn footer_hint_label_widths(
    natural_label_width: f64,
    label_padding_x: f64,
    label_chip_height: f64,
    max_item_width: Option<f64>,
    keys_view_width: f64,
    edge_padding_x: f64,
) -> (f64, f64) {
    let max_label_chip_width = max_item_width.map(|max_width| {
        (max_width - (edge_padding_x * 2.0) - FOOTER_HINT_KEY_LABEL_GAP - keys_view_width)
            .max(label_chip_height)
    });
    let label_chip_width = (natural_label_width + label_padding_x * 2.0)
        .max(label_chip_height)
        .min(max_label_chip_width.unwrap_or(f64::MAX));
    let label_text_width = (label_chip_width - label_padding_x * 2.0).max(0.0);
    (label_chip_width, label_text_width)
}

#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_item(
    button_cfg: &FooterButtonConfig,
    font: id,
    _text_color: id,
    max_item_width: Option<f64>,
    theme: &crate::theme::Theme,
) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    let row_states =
        crate::components::footer_chrome::resolved_footer_button_visual_colors(theme).row_states;
    let initial_state = if button_cfg.selected {
        row_states.active
    } else {
        row_states.rest
    };
    let text_color = ns_color_from_rgba(initial_state.primary_foreground_rgba);
    let item_height =
        crate::components::footer_chrome::footer_button_height(footer_height() as f32) as f64;

    let container: id = msg_send![class!(NSView), alloc];
    let container: id = msg_send![
        container,
        initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, item_height))
    ];
    if container == nil {
        return nil;
    }
    let action_key = button_cfg.id.as_ref();
    let item_identifier = ns_string(&format!("{FOOTER_HINT_ITEM_ID_PREFIX}{action_key}"));
    if item_identifier != nil {
        let _: () = msg_send![container, setIdentifier: item_identifier];
    }

    // AppKit only guarantees correct glass compositing for content mounted
    // through NSGlassEffectView.contentView. Keep the transparent NSButton in
    // the outer wrapper for hit testing, but put every visual child inside the
    // capsule's foreground content hierarchy.
    let mut visual_content_parent = container;
    let mut state_background_view = container;

    // Floating-chrome mode: each button rides its own glass capsule. The
    // bounded footer glass container is isolated from the bounded main
    // backdrop by the transparent 8pt gutter, so neighbouring capsules remain
    // independent instead of becoming a full-width meniscus shelf.
    if glass_scroll_bands_active() {
        if let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") {
            let capsule: id = msg_send![glass_class, alloc];
            let capsule: id = msg_send![
                capsule,
                initWithFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(0.0, item_height)
                )
            ];
            if capsule != nil {
                let capsule_identifier =
                    ns_string(&format!("{FOOTER_HINT_CAPSULE_ID_PREFIX}{action_key}"));
                if capsule_identifier != nil {
                    let _: () = msg_send![capsule, setIdentifier: capsule_identifier];
                }
                let _: () = msg_send![capsule, setAutoresizingMask: 18u64];

                let capsule_content: id = msg_send![class!(NSView), alloc];
                let capsule_content: id = msg_send![
                    capsule_content,
                    initWithFrame: NSRect::new(
                        NSPoint::new(0.0, 0.0),
                        NSSize::new(0.0, item_height)
                    )
                ];
                if capsule_content != nil {
                    let content_identifier = ns_string(&format!(
                        "{FOOTER_HINT_CAPSULE_CONTENT_ID_PREFIX}{action_key}"
                    ));
                    if content_identifier != nil {
                        let _: () = msg_send![capsule_content, setIdentifier: content_identifier];
                    }
                    let _: () = msg_send![capsule_content, setAutoresizingMask: 18u64];
                    let _: () = msg_send![capsule, setContentView: capsule_content];
                    visual_content_parent = capsule_content;

                    let state_view: id = msg_send![class!(NSView), alloc];
                    let state_view: id = msg_send![
                        state_view,
                        initWithFrame: NSRect::new(
                            NSPoint::new(0.0, 0.0),
                            NSSize::new(0.0, item_height)
                        )
                    ];
                    if state_view != nil {
                        let state_identifier =
                            ns_string(&format!("{FOOTER_HINT_STATE_LAYER_ID_PREFIX}{action_key}"));
                        if state_identifier != nil {
                            let _: () = msg_send![state_view, setIdentifier: state_identifier];
                        }
                        let _: () = msg_send![state_view, setAutoresizingMask: 18u64];
                        let _: () = msg_send![state_view, setWantsLayer: YES];
                        let state_layer: id = msg_send![state_view, layer];
                        if state_layer != nil {
                            let _: () = msg_send![
                                state_layer,
                                setCornerRadius:
                                    crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
                            ];
                        }
                        let _: () = msg_send![capsule_content, addSubview: state_view];
                        state_background_view = state_view;
                    }
                }

                let _: () = msg_send![
                    capsule,
                    setCornerRadius:
                        crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
                ];
                style_float_footer_capsule(capsule, theme);
                let _: () = msg_send![
                    container,
                    addSubview: capsule
                    positioned: -1isize
                    relativeTo: cocoa::base::nil
                ];
            }
        }
    }

    let has_label = !button_cfg.label.as_ref().is_empty();
    let label_field = if has_label {
        make_footer_hint_text_field(
            button_cfg.label.as_ref(),
            font,
            text_color,
            FOOTER_HINT_TEXT_ALIGN_RIGHT,
        )
    } else {
        nil
    };
    if has_label && label_field == nil {
        return nil;
    }
    if label_field != nil {
        let label_identifier = ns_string(&format!("{FOOTER_HINT_LABEL_ID_PREFIX}{action_key}"));
        if label_identifier != nil {
            let _: () = msg_send![label_field, setIdentifier: label_identifier];
        }
    }

    let edge_padding_x = if matches!(
        button_cfg.action,
        FooterAction::Run | FooterAction::Cwd | FooterAction::AgentModel
    ) {
        metrics.run_button_padding_x as f64
    } else {
        metrics.button_padding_x as f64
    };
    let keycap_border_color = ns_color_from_hex_with_alpha(
        footer_keycap_hex(theme),
        footer_keycap_border_alpha(theme, button_cfg.selected),
    );
    let key_font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: metrics.keycap_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];

    let shortcut_keys =
        if crate::components::footer_chrome::is_footer_icon_token(button_cfg.key.as_ref()) {
            vec![button_cfg.key.to_string()]
        } else if button_cfg.shortcut_routable {
            button_cfg.shortcut_tokens.clone()
        } else {
            Vec::new()
        };

    let keys_view: id = msg_send![class!(NSView), alloc];
    let keys_view: id = msg_send![
        keys_view,
        initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, item_height))
    ];
    if keys_view == nil {
        return nil;
    }
    let keys_identifier = ns_string(&format!("{FOOTER_HINT_KEYS_ID_PREFIX}{action_key}"));
    if keys_identifier != nil {
        let _: () = msg_send![keys_view, setIdentifier: keys_identifier];
    }
    let _: () = msg_send![keys_view, setWantsLayer: YES];

    let mut keys_view_width = 0.0_f64;
    // Keycap-to-keycap spacing must match the width the estimator reserves
    // (FOOTER_ACTION_CONTENT_GAP_PX), so multi-key groups like ⇧⇥ / ⌘K are laid
    // out exactly as sized — no AppKit-only magic number.
    let key_gap = metrics.content_gap as f64;

    for (i, key_str) in shortcut_keys.iter().enumerate() {
        let chip_view: id = msg_send![class!(NSView), alloc];
        let chip_view: id = msg_send![
            chip_view,
            initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0))
        ];
        if chip_view == nil {
            continue;
        }
        let keycap_identifier =
            ns_string(&format!("{FOOTER_HINT_KEYCAP_ID_PREFIX}{action_key}-{i}"));
        if keycap_identifier != nil {
            let _: () = msg_send![chip_view, setIdentifier: keycap_identifier];
        }

        let _: () = msg_send![chip_view, setWantsLayer: YES];
        let chip_layer: id = msg_send![chip_view, layer];
        if chip_layer != nil {
            let _: () = msg_send![
                chip_layer,
                setCornerRadius: metrics.keycap_radius as f64
            ];
            let _: () = msg_send![chip_layer, setBorderWidth: 1.0_f64];
            if keycap_border_color != nil {
                let cg_border: id = msg_send![keycap_border_color, CGColor];
                if cg_border != nil {
                    let _: () = msg_send![chip_layer, setBorderColor: cg_border];
                }
            }
        }

        let is_icon = crate::components::footer_chrome::is_footer_icon_token(key_str);
        let chip_padding_x =
            crate::components::footer_chrome::footer_keycap_padding_x_for_token(key_str, &metrics)
                as f64;
        let chip_padding_y = metrics.keycap_padding_y as f64;
        let chip_height = metrics.keycap_height as f64;
        let (glyph_view, glyph_size) = if is_icon {
            let image = footer_icon_image(key_str);
            if image == nil {
                continue;
            }
            let image_view: id = msg_send![class!(NSImageView), alloc];
            let image_view: id = msg_send![
                image_view,
                initWithFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(metrics.keycap_font_size as f64, metrics.keycap_font_size as f64)
                )
            ];
            if image_view == nil {
                continue;
            }
            let _: () = msg_send![image_view, setImage: image];
            let _: () = msg_send![image_view, setContentTintColor: text_color];
            let _: () = msg_send![image_view, setAlphaValue: 1.0_f64];
            let _: () = msg_send![image_view, setImageScaling: 0usize];
            (
                image_view,
                NSSize::new(
                    metrics.keycap_font_size as f64,
                    metrics.keycap_font_size as f64,
                ),
            )
        } else {
            let glyph_field = make_footer_hint_text_field(
                key_str,
                key_font,
                text_color,
                FOOTER_HINT_TEXT_ALIGN_LEFT,
            );
            if glyph_field == nil {
                continue;
            }
            let glyph_size: NSSize = msg_send![glyph_field, fittingSize];
            (glyph_field, glyph_size)
        };
        let glyph_identifier = ns_string(&format!(
            "{FOOTER_HINT_KEYCAP_GLYPH_ID_PREFIX}{action_key}-{i}"
        ));
        if glyph_identifier != nil {
            let _: () = msg_send![glyph_view, setIdentifier: glyph_identifier];
        }
        let chip_width = (glyph_size.width + chip_padding_x * 2.0).max(chip_height);

        let glyph_x = crate::components::footer_chrome::footer_appkit_glyph_x(
            key_str,
            chip_width,
            glyph_size.width,
        );
        let glyph_y = chip_padding_y
            + crate::components::footer_chrome::footer_appkit_glyph_y(
                key_str,
                (chip_height - chip_padding_y * 2.0).max(0.0),
                glyph_size.height,
            );

        let _: () = msg_send![
            glyph_view,
            setFrame: NSRect::new(
                NSPoint::new(glyph_x, glyph_y),
                NSSize::new(glyph_size.width, glyph_size.height)
            )
        ];
        let _: () = msg_send![chip_view, addSubview: glyph_view];

        let chip_y = ((item_height - chip_height) / 2.0).round();
        let chip_x = keys_view_width;

        let _: () = msg_send![
            chip_view,
            setFrame: NSRect::new(
                NSPoint::new(chip_x, chip_y),
                NSSize::new(chip_width, chip_height)
            )
        ];

        let _: () = msg_send![keys_view, addSubview: chip_view];

        keys_view_width += chip_width;
        if i < shortcut_keys.len() - 1 {
            keys_view_width += key_gap;
        }
    }

    let _: () = msg_send![
        keys_view,
        setFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(keys_view_width, item_height)
        )
    ];

    let label_padding_x = metrics.keycap_padding_x as f64;
    let label_chip_height = metrics.keycap_height as f64;

    // Optional leading status dot (e.g. Agent Chat streaming dot on the Agent·Model
    // chip), rendered inside the chip ahead of the label. The lane is a FIXED
    // width whenever `leading_dot` is `Some(_)` — including `Some(Hidden)` — so
    // the chip's x/width never jump as the status changes during streaming.
    let leading_dot_status = button_cfg.leading_dot;
    let leading_dot_width = if leading_dot_status.is_some() {
        FOOTER_STREAMING_DOT_SIZE + FOOTER_LEFT_DOT_LABEL_GAP
    } else {
        0.0
    };
    // Reserve the dot lane out of the label's width budget so a capped label
    // truncates inside the chip instead of pushing into sibling buttons.
    let label_max_item_width = max_item_width.map(|width| (width - leading_dot_width).max(0.0));

    let (label_view, label_chip_width, _label_text_width) = if has_label {
        let label_size: NSSize = msg_send![label_field, fittingSize];
        let (label_chip_width, label_text_width) = footer_hint_label_widths(
            label_size.width,
            label_padding_x,
            label_chip_height,
            label_max_item_width,
            keys_view_width,
            edge_padding_x,
        );
        let label_view: id = msg_send![class!(NSView), alloc];
        let label_view: id = msg_send![
            label_view,
            initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(label_chip_width, label_chip_height))
        ];
        if label_view == nil {
            return nil;
        }
        let label_chip_identifier =
            ns_string(&format!("{FOOTER_HINT_LABEL_CHIP_ID_PREFIX}{action_key}"));
        if label_chip_identifier != nil {
            let _: () = msg_send![label_view, setIdentifier: label_chip_identifier];
        }
        let _: () = msg_send![label_view, setWantsLayer: YES];
        let label_layer: id = msg_send![label_view, layer];
        if label_layer != nil {
            let _: () = msg_send![
                label_layer,
                setCornerRadius: metrics.keycap_radius as f64
            ];
            let _: () = msg_send![label_layer, setBorderWidth: 0.0_f64];
        }

        let label_field_x = ((label_chip_width - label_text_width) / 2.0).round();
        let label_field_y = ((label_chip_height - label_size.height) / 2.0).round();
        let _: () = msg_send![
            label_field,
            setFrame: NSRect::new(
                NSPoint::new(label_field_x, label_field_y),
                NSSize::new(label_text_width, label_size.height)
            )
        ];
        let _: () = msg_send![label_view, addSubview: label_field];
        (label_view, label_chip_width, label_text_width)
    } else {
        (nil, 0.0_f64, 0.0_f64)
    };

    // The dot + label form a single leading "label group"; the keycap lays out
    // after it. Using the group width everywhere keeps the dot non-overlapping.
    let label_group_width = label_chip_width + leading_dot_width;
    let gap_width = if label_group_width > 0.0 && keys_view_width > 0.0 {
        metrics.content_gap as f64
    } else {
        0.0
    };
    let legacy_extra_padding = footer_hint_legacy_extra_padding(button_cfg);
    let min_content_width = keys_view_width
        + label_group_width
        + gap_width
        + (edge_padding_x * 2.0)
        + legacy_extra_padding;
    let content_width = label_group_width + gap_width + keys_view_width;
    let intrinsic_width = content_width + (edge_padding_x * 2.0);
    let mut item_width = footer_hint_slot_width(button_cfg.action)
        .max(min_content_width)
        .max(intrinsic_width);
    if let Some(max_item_width) = max_item_width {
        item_width = item_width.min(max_item_width.max(min_content_width));
    }
    let label_y = ((item_height - label_chip_height) / 2.0).round();
    let (label_group_x, key_x, _) = footer_hint_content_layout_for_button(
        button_cfg,
        item_width,
        label_group_width,
        keys_view_width,
        metrics.content_gap as f64,
        metrics.button_padding_x as f64,
        metrics.run_button_padding_x as f64,
    );
    let dot_x = label_group_x;
    let label_x = label_group_x + leading_dot_width;

    if let Some(dot_status) = leading_dot_status {
        let dot_view = make_footer_hint_leading_dot_view(button_cfg.action, dot_status);
        if dot_view != nil {
            let dot_y = ((item_height - FOOTER_STREAMING_DOT_SIZE) / 2.0).round();
            let _: () = msg_send![
                dot_view,
                setFrame: NSRect::new(
                    NSPoint::new(dot_x, dot_y),
                    NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE)
                )
            ];
            let _: () = msg_send![visual_content_parent, addSubview: dot_view];
        }
    }

    if has_label && label_view != nil {
        let _: () = msg_send![
            label_view,
            setFrame: NSRect::new(
                NSPoint::new(label_x, label_y),
                NSSize::new(label_chip_width, label_chip_height)
            )
        ];
    }
    let _: () = msg_send![
        keys_view,
        setFrame: NSRect::new(
            NSPoint::new(key_x, 0.0),
            NSSize::new(keys_view_width, item_height)
        )
    ];
    let _: () = msg_send![container, setWantsLayer: YES];
    let background_layer: id = msg_send![state_background_view, layer];
    if background_layer != nil {
        let _: () = msg_send![background_layer, setCornerRadius: FOOTER_HINT_RADIUS];
        // Resolve the resting background: selected buttons use the active fill,
        // otherwise accent-fill variations paint a subtle resting tint and all
        // other variations stay transparent. Every action routes through the
        // same helpers so the buttons stay perfectly in sync.
        let rest_rgba = if button_cfg.selected {
            Some(footer_button_active_fill_rgba(button_cfg.action, theme))
        } else {
            footer_button_rest_fill_rgba(theme)
        };
        if let Some(rest_rgba) = rest_rgba {
            let rest_ns: id = ns_color_from_rgba(rest_rgba);
            if rest_ns != nil {
                let cg: id = msg_send![rest_ns, CGColor];
                if cg != nil {
                    let _: () = msg_send![background_layer, setBackgroundColor: cg];
                }
            }
        }
    }

    let button: id = msg_send![footer_button_class(), alloc];
    let button: id = msg_send![
        button,
        initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(item_width, item_height))
    ];
    if button != nil {
        let empty_title = ns_string("");
        if empty_title != nil {
            let _: () = msg_send![button, setTitle: empty_title];
        }
        let button_id = format!(
            "{}{}",
            FOOTER_HINT_BUTTON_ID_PREFIX,
            footer_action_key(button_cfg.action)
        );
        set_footer_view_identifier(button, &button_id);
        set_footer_button_accessibility(button, button_cfg);
        let _: () = msg_send![button, setBordered: NO];
        let _: () = msg_send![button, setBezelStyle: 0usize];
        let _: () = msg_send![button, setButtonType: 0usize];
        let _: () = msg_send![button, setTransparent: YES];
        let _: () = msg_send![button, setEnabled: if button_cfg.enabled { YES } else { NO }];
        let _: () = msg_send![button, setTarget: footer_action_target()];
        let _: () = msg_send![button, setAction: footer_action_selector(button_cfg.action)];

        // Store button state for hover/cursor behavior and selected restoration.
        let is_actions = matches!(button_cfg.action, FooterAction::Actions);
        if let Some(obj) = button.as_mut() {
            obj.set_ivar::<cocoa::base::BOOL>(
                "_isActionsButton",
                if is_actions { YES } else { NO },
            );
            obj.set_ivar::<cocoa::base::BOOL>(
                "_selected",
                if button_cfg.selected { YES } else { NO },
            );
            obj.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
            obj.set_ivar::<cocoa::base::BOOL>(
                "_enabled",
                if button_cfg.enabled { YES } else { NO },
            );
            obj.set_ivar::<usize>("_stateView", state_background_view as usize);
            obj.set_ivar::<usize>("_visualRoot", visual_content_parent as usize);
        }
    }

    if has_label && label_view != nil {
        let _: () = msg_send![visual_content_parent, addSubview: label_view];
    }
    let _: () = msg_send![visual_content_parent, addSubview: keys_view];
    if button != nil {
        let _: () = msg_send![container, addSubview: button];
        apply_footer_button_visual_state(
            button,
            crate::theme::main_menu_row_state_from_flags(button_cfg.selected, false),
        );
    }
    let _: () = msg_send![
        container,
        setFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(item_width, item_height))
    ];

    container
}

#[cfg(test)]
include!("footer_popup_tests.rs");

#[cfg(target_os = "macos")]
fn send_footer_action_from_sender(sender: id, action: FooterAction) {
    // SAFETY: AppKit supplies a live instance of our button subclass.
    let token = unsafe {
        let Some(button) = sender.as_ref() else {
            return;
        };
        if *button.get_ivar::<cocoa::base::BOOL>("_enabled") != YES {
            return;
        }
        *button.get_ivar::<u64>("_footerBindingToken")
    };
    let binding = FOOTER_HOSTS
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .values()
        .find(|host| token != 0 && host.native_token == token)
        .and_then(|host| host.binding.clone());
    if let Some(binding) = binding {
        dispatch_bound_footer_action(&binding, action);
    }
}

include!("footer_popup_native_dispatch.rs");
#[cfg(target_os = "macos")]
fn footer_passthrough_view_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

    // SAFETY: ObjC class registration is serialized by `OnceLock`. Superclass
    // is `NSView`; installed methods match the expected ObjC ABI signatures.
    *CLASS.get_or_init(|| unsafe {
        let superclass = class!(NSView);
        let Some(mut decl) = ClassDecl::new("ScriptKitFooterPassthroughView", superclass) else {
            return class!(NSView) as *const _ as usize;
        };
        decl.add_method(
            sel!(hitTest:),
            footer_passthrough_hit_test
                as extern "C" fn(&Object, Sel, cocoa::foundation::NSPoint) -> id,
        );
        decl.register() as *const _ as usize
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
extern "C" fn footer_passthrough_hit_test(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    point: cocoa::foundation::NSPoint,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    // Preserve passthrough for empty background while allowing the transparent
    // CWD/context NSButtons mounted in this view to participate in AppKit's
    // normal child hit-test traversal.
    unsafe {
        let this_id = this as *const _ as id;
        let hit: id = msg_send![super(this_id, class!(NSView)), hitTest: point];
        if hit == nil || hit == this_id {
            return nil;
        }
        let is_button: cocoa::base::BOOL = msg_send![hit, isKindOfClass: class!(NSButton)];
        if is_button == YES {
            hit
        } else {
            nil
        }
    }
}

#[cfg(target_os = "macos")]
fn footer_button_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

    *CLASS.get_or_init(|| {
        // SAFETY: Registering an ObjC class from NSButton. ClassDecl::new returns
        // None only if the class name is already registered, in which case we
        // fall back to the plain NSButton class.
        unsafe {
            let superclass = class!(NSButton);
            let Some(mut decl) = ClassDecl::new("ScriptKitFooterButton", superclass) else {
                return class!(NSButton) as *const _ as usize;
            };
            decl.add_ivar::<usize>("_hoverCGColor");
            decl.add_ivar::<usize>("_selectedCGColor");
            decl.add_ivar::<cocoa::base::BOOL>("_isActionsButton");
            decl.add_ivar::<cocoa::base::BOOL>("_selected");
            decl.add_ivar::<cocoa::base::BOOL>("_hovered");
            decl.add_ivar::<cocoa::base::BOOL>("_enabled");
            decl.add_ivar::<usize>("_stateView");
            decl.add_ivar::<usize>("_visualRoot");
            decl.add_ivar::<u64>("_footerBindingToken");
            decl.add_method(
                sel!(acceptsFirstMouse:),
                footer_button_accepts_first_mouse
                    as extern "C" fn(&Object, Sel, id) -> cocoa::base::BOOL,
            );
            decl.add_method(
                sel!(mouseDownCanMoveWindow),
                footer_button_mouse_down_can_move_window
                    as extern "C" fn(&Object, Sel) -> cocoa::base::BOOL,
            );
            decl.add_method(
                sel!(mouseDown:),
                footer_button_mouse_down as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(resetCursorRects),
                footer_button_reset_cursor_rects as extern "C" fn(&Object, Sel),
            );
            decl.add_method(
                sel!(updateTrackingAreas),
                footer_button_update_tracking_areas as extern "C" fn(&Object, Sel),
            );
            decl.add_method(
                sel!(mouseEntered:),
                footer_button_mouse_entered as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseExited:),
                footer_button_mouse_exited as extern "C" fn(&Object, Sel, id),
            );
            decl.register() as *const _ as usize
        }
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_accepts_first_mouse(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _: id,
) -> cocoa::base::BOOL {
    // SAFETY: `this` is a live instance of our registered NSButton subclass,
    // so reading the `_enabled` ivar is valid for the duration of this call.
    let enabled: cocoa::base::BOOL = unsafe { *this.get_ivar::<cocoa::base::BOOL>("_enabled") };
    if enabled != YES {
        return NO;
    }
    tracing::debug!(
        target: "script_kit::footer_popup",
        event = "native_footer_button_accepts_first_mouse",
        "Native footer button accepted first mouse"
    );
    YES
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_down_can_move_window(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) -> cocoa::base::BOOL {
    NO
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_reset_cursor_rects(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `this` is a live NSButton subclass. We add a cursor rect covering
    // the full button bounds so the footer keeps the default arrow cursor.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            return;
        }
        let bounds: cocoa::foundation::NSRect = msg_send![this, bounds];
        let cursor: id = msg_send![class!(NSCursor), arrowCursor];
        let _: () = msg_send![this, addCursorRect:bounds cursor:cursor];
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_update_tracking_areas(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
) {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: Replace old tracking areas with a fresh one matching the button
    // bounds. This is the standard AppKit pattern for views that change size.
    unsafe {
        // Call super first.
        let this_id = this as *const _ as id;
        let _: () = msg_send![super(this_id, class!(NSButton)), updateTrackingAreas];

        // Remove existing tracking areas.
        let existing: id = msg_send![this, trackingAreas];
        if existing != nil {
            let count: usize = msg_send![existing, count];
            for i in (0..count).rev() {
                let area: id = msg_send![existing, objectAtIndex: i];
                let _: () = msg_send![this, removeTrackingArea: area];
            }
        }

        // Add a new tracking area for mouseEntered/mouseExited.
        let opts: usize = 0x01 /* MouseEnteredAndExited */ | 0x80 /* ActiveAlways */ | 0x20 /* InVisibleRect */;
        let bounds: cocoa::foundation::NSRect = msg_send![this, bounds];
        let area: id = msg_send![class!(NSTrackingArea), alloc];
        let area: id = msg_send![
            area,
            initWithRect: bounds
            options: opts
            owner: this_id
            userInfo: nil
        ];
        if area != nil {
            let _: () = msg_send![this, addTrackingArea: area];
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_footer_button_foreground_rgba(view: id, foreground_rgba: u32) {
    use objc::{class, msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let color = ns_color_from_rgba(foreground_rgba);
    if color == nil {
        return;
    }

    let is_text_field: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSTextField)];
    if is_text_field == YES {
        let _: () = msg_send![view, setTextColor: color];
    }
    let is_image_view: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSImageView)];
    if is_image_view == YES {
        let _: () = msg_send![view, setContentTintColor: color];
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: i];
        set_footer_button_foreground_rgba(child, foreground_rgba);
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_footer_button_border(view: id, keycap_hex: u32, alpha: f64) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let color = ns_color_from_hex_with_alpha(keycap_hex, alpha);
    if color == nil {
        return;
    }

    if appkit_view_identifier(view)
        .as_deref()
        .is_some_and(footer_identifier_uses_keycap_border)
    {
        let layer: id = msg_send![view, layer];
        if layer != nil {
            let border_width: f64 = msg_send![layer, borderWidth];
            if border_width > 0.0 {
                let cg_border: id = msg_send![color, CGColor];
                if cg_border != nil {
                    let _: () = msg_send![layer, setBorderColor: cg_border];
                }
            }
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for i in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: i];
        set_footer_button_border(child, keycap_hex, alpha);
    }
}

#[cfg(target_os = "macos")]
unsafe fn apply_footer_button_background(button: id, rgba_value: Option<u32>) {
    use objc::{msg_send, sel, sel_impl};

    if button == nil {
        return;
    }

    let state_view = button
        .as_ref()
        .map(|object| *object.get_ivar::<usize>("_stateView") as id)
        .unwrap_or(nil);
    let target_view: id = if state_view != nil {
        state_view
    } else {
        msg_send![button, superview]
    };
    if target_view == nil {
        return;
    }

    let layer: id = msg_send![target_view, layer];
    if layer == nil {
        return;
    }

    if let Some(rgba_value) = rgba_value {
        let ns_color: id = ns_color_from_rgba(rgba_value);
        if ns_color != nil {
            let cg: id = msg_send![ns_color, CGColor];
            if cg != nil {
                let _: () = msg_send![layer, setBackgroundColor: cg];
            }
        }
    } else {
        let null_color: id = std::ptr::null_mut();
        let _: () = msg_send![layer, setBackgroundColor: null_color];
    }
}

#[cfg(target_os = "macos")]
unsafe fn footer_button_visual_root(button: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    if button == nil {
        return nil;
    }
    let visual_root = button
        .as_ref()
        .map(|object| *object.get_ivar::<usize>("_visualRoot") as id)
        .unwrap_or(nil);
    if visual_root != nil {
        visual_root
    } else {
        msg_send![button, superview]
    }
}

#[cfg(target_os = "macos")]
unsafe fn apply_footer_button_visual_state(button: id, state: crate::theme::MainMenuRowState) {
    let theme = crate::theme::get_cached_theme();
    apply_footer_button_visual_state_with_theme(
        button,
        state,
        resolve_native_footer_visual_theme(&theme),
    );
}

#[cfg(target_os = "macos")]
unsafe fn apply_footer_button_visual_state_with_theme(
    button: id,
    state: crate::theme::MainMenuRowState,
    visual_theme: NativeFooterVisualTheme,
) {
    if button == nil {
        return;
    }

    let colors = visual_theme.row_palette.for_state(state);
    let border_alpha = visual_theme.border_alpha(state);
    let state_code = match state {
        crate::theme::MainMenuRowState::Rest => 0usize,
        crate::theme::MainMenuRowState::Hover => 1usize,
        crate::theme::MainMenuRowState::Active => 2usize,
    };
    let state_signature = state_code
        | ((colors.background_rgba.is_some() as usize) << 8)
        | ((colors.background_rgba.unwrap_or_default() as usize) << 16);
    let color_signature =
        colors.primary_foreground_rgba as usize | ((border_alpha.to_bits() as usize) << 32);

    apply_footer_button_background(button, colors.background_rgba);
    let visual_root = footer_button_visual_root(button);
    set_footer_button_foreground_rgba(visual_root, colors.primary_foreground_rgba);
    set_footer_button_border(visual_root, visual_theme.keycap_hex, border_alpha as f64);

    let button_id =
        appkit_view_identifier(button).unwrap_or_else(|| "unidentified-footer-button".to_string());
    if !native_footer_visual_event_changed(
        &button_id,
        state_signature,
        color_signature,
        visual_theme.keycap_hex,
    ) {
        return;
    }
    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_button_visual_state_applied",
        button_id = %button_id,
        state = ?state,
        has_background = colors.background_rgba.is_some(),
        background_rgba = colors.background_rgba.unwrap_or_default(),
        foreground_rgba = colors.primary_foreground_rgba,
        keycap_border_hex = visual_theme.keycap_hex,
        keycap_border_alpha_bits = border_alpha.to_bits(),
        "Applied canonical main-menu row colors to native footer button"
    );
}

/// Invisible float-mode footer host: plain NSView container with the same
/// button-scoped hit-testing as the old glass band; draws nothing itself.
#[cfg(target_os = "macos")]
fn float_footer_host_view_class() -> Option<*const objc::runtime::Class> {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Class, Object, Sel};
    use objc::{sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();
    let ptr = *CLASS.get_or_init(|| unsafe {
        if let Some(existing) = Class::get("ScriptKitFooterFloatHostView") {
            return existing as *const Class as usize;
        }
        let Some(superclass) = Class::get("NSView") else {
            return 0;
        };
        let Some(mut decl) = ClassDecl::new("ScriptKitFooterFloatHostView", superclass) else {
            return 0;
        };
        decl.add_method(
            sel!(hitTest:),
            glass_footer_hit_test as extern "C" fn(&Object, Sel, cocoa::foundation::NSPoint) -> id,
        );
        decl.register() as *const Class as usize
    });
    (ptr != 0).then_some(ptr as *const objc::runtime::Class)
}

#[cfg(target_os = "macos")]
fn footer_effect_view_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

    // SAFETY: ObjC class registration is serialized by `OnceLock`. Superclass
    // is `NSVisualEffectView`; installed methods match expected ObjC ABI signatures.
    *CLASS.get_or_init(|| unsafe {
        let superclass = class!(NSVisualEffectView);
        let Some(mut decl) = ClassDecl::new("ScriptKitFooterEffectView", superclass) else {
            return class!(NSVisualEffectView) as *const _ as usize;
        };
        decl.add_method(
            sel!(hitTest:),
            footer_hit_test as extern "C" fn(&Object, Sel, cocoa::foundation::NSPoint) -> id,
        );
        decl.add_method(
            sel!(mouseDown:),
            footer_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseUp:),
            footer_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(mouseDragged:),
            footer_mouse_dragged as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(rightMouseDown:),
            footer_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(rightMouseUp:),
            footer_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(otherMouseDown:),
            footer_mouse_down as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(otherMouseUp:),
            footer_mouse_up as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(scrollWheel:),
            footer_scroll_wheel as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(acceptsFirstMouse:),
            footer_accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> cocoa::base::BOOL,
        );
        decl.register() as *const _ as usize
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
/// Walk up the view hierarchy from `view` looking for the nearest NSButton.
/// Returns the button if found, nil otherwise.
///
/// SAFETY: Caller must ensure `view` is a valid, live AppKit view pointer on
/// the main thread.
unsafe fn nearest_footer_button(mut view: id) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    while view != nil {
        let is_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSButton)];
        if is_button == YES {
            return view;
        }

        let superview: id = msg_send![view, superview];
        if superview == nil || superview == view {
            break;
        }
        view = superview;
    }

    nil
}

#[cfg(target_os = "macos")]
/// Return a footer button contained by `view`, if `view` is one of the native
/// footer item wrappers.
///
/// SAFETY: Caller must ensure `view` is a valid, live AppKit view pointer on
/// the main thread.
unsafe fn footer_button_in_subviews(view: id) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    if view == nil {
        return nil;
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return nil;
    }

    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        if child == nil {
            continue;
        }

        let is_button: cocoa::base::BOOL = msg_send![child, isKindOfClass: class!(NSButton)];
        if is_button == YES {
            return child;
        }
    }

    nil
}

#[cfg(target_os = "macos")]
/// Resolve text-field or empty-area hits inside a footer item wrapper to the
/// sibling button that owns that whole visual slot.
///
/// SAFETY: Caller must ensure `view` is a valid, live AppKit view pointer on
/// the main thread.
unsafe fn nearest_footer_item_button(mut view: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    while view != nil {
        let button = footer_button_in_subviews(view);
        if button != nil {
            return button;
        }

        let superview: id = msg_send![view, superview];
        if superview == nil || superview == view {
            break;
        }
        view = superview;
    }

    nil
}

#[cfg(target_os = "macos")]
fn ns_point_in_rect(point: cocoa::foundation::NSPoint, rect: cocoa::foundation::NSRect) -> bool {
    point.x >= rect.origin.x
        && point.y >= rect.origin.y
        && point.x < rect.origin.x + rect.size.width
        && point.y < rect.origin.y + rect.size.height
}

#[cfg(target_os = "macos")]
/// Resolve a footer point to the button inside the visible hint item frame,
/// before AppKit's normal hit test can return an unrelated overlay sibling.
///
/// SAFETY: Caller must ensure `footer_view` is a valid footer AppKit view
/// pointer on the main thread.
unsafe fn footer_item_button_at_point(
    footer_view: id,
    point_in_footer: cocoa::foundation::NSPoint,
) -> id {
    use objc::{msg_send, sel, sel_impl};

    let hints_view = find_subview_by_identifier(footer_view, FOOTER_HINTS_ID);
    if hints_view == nil {
        return nil;
    }

    let point_in_hints: cocoa::foundation::NSPoint =
        msg_send![hints_view, convertPoint: point_in_footer fromView: footer_view];
    let hints_bounds: cocoa::foundation::NSRect = msg_send![hints_view, bounds];
    if !ns_point_in_rect(point_in_hints, hints_bounds) {
        return nil;
    }

    let items: id = msg_send![hints_view, subviews];
    if items == nil {
        return nil;
    }

    let count: usize = msg_send![items, count];
    for index in (0..count).rev() {
        let item: id = msg_send![items, objectAtIndex: index];
        if item == nil {
            continue;
        }

        let point_in_item: cocoa::foundation::NSPoint =
            msg_send![item, convertPoint: point_in_hints fromView: hints_view];
        let item_bounds: cocoa::foundation::NSRect = msg_send![item, bounds];
        if !ns_point_in_rect(point_in_item, item_bounds) {
            continue;
        }

        let button = footer_button_in_subviews(item);
        if button != nil {
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "native_footer_hit_test_item_geometry",
                x = point_in_footer.x,
                y = point_in_footer.y,
                "Resolved native footer hit by item geometry"
            );
            return button;
        }
    }

    nil
}

#[cfg(target_os = "macos")]
/// Resolve the two left-pinned transparent buttons by geometry. Their visual
/// glass subtree is decorative and may win AppKit's ordinary descendant hit
/// test even though the owning control is a sibling.
unsafe fn footer_left_button_at_point(
    footer_view: id,
    point_in_footer: cocoa::foundation::NSPoint,
) -> id {
    use objc::{msg_send, sel, sel_impl};

    let left_info = find_subview_by_identifier(footer_view, FOOTER_LEFT_INFO_ID);
    if left_info == nil {
        return nil;
    }
    for identifier in [
        FOOTER_CWD_CHIP_HIT_TARGET_ID,
        FOOTER_LEFT_INFO_HIT_TARGET_ID,
    ] {
        let button = find_subview_by_identifier(left_info, identifier);
        if button == nil {
            continue;
        }
        let hidden: cocoa::base::BOOL = msg_send![button, isHidden];
        let enabled: cocoa::base::BOOL = msg_send![button, isEnabled];
        if hidden == YES || enabled != YES {
            continue;
        }
        let point_in_button: cocoa::foundation::NSPoint =
            msg_send![button, convertPoint: point_in_footer fromView: footer_view];
        let bounds: cocoa::foundation::NSRect = msg_send![button, bounds];
        if ns_point_in_rect(point_in_button, bounds) {
            tracing::debug!(
                target: "script_kit::footer_popup",
                event = "native_footer_hit_test_left_geometry",
                identifier,
                x = point_in_footer.x,
                y = point_in_footer.y,
                "Resolved native left footer hit by control geometry"
            );
            return button;
        }
    }
    nil
}

#[cfg(target_os = "macos")]
extern "C" fn glass_footer_hit_test(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    point: cocoa::foundation::NSPoint,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `this` is a live NSGlassEffectView subclass. Visible AppKit
    // controls remain interactive; every background hit returns nil so wheel,
    // hover, and row clicks continue to the GPUI Metal sibling underneath.
    unsafe {
        let this_id = this as *const _ as id;
        let left_button = footer_left_button_at_point(this_id, point);
        if left_button != nil {
            return left_button;
        }
        let item_button = footer_item_button_at_point(this_id, point);
        if item_button != nil {
            return item_button;
        }

        // ScriptKitFooterFloatHostView is registered directly under NSView;
        // super dispatch must name that actual registered superclass.
        let hit: id = msg_send![super(this_id, class!(NSView)), hitTest: point];
        let button = nearest_footer_button(hit);
        if button != nil {
            return button;
        }
        let item_button = nearest_footer_item_button(hit);
        if item_button != nil {
            return item_button;
        }

        nil
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_hit_test(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    point: cocoa::foundation::NSPoint,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `this` is a live NSVisualEffectView subclass instance. We delegate
    // Route clicks to buttons, let everything else (scroll, hover) fall
    // through to the GPUI Metal view behind us. Returning nil for non-button
    // areas is critical — returning self would intercept scroll events and
    // break list scrolling.
    unsafe {
        let this_id = this as *const _ as id;
        let left_button = footer_left_button_at_point(this_id, point);
        if left_button != nil {
            return left_button;
        }
        let item_button = footer_item_button_at_point(this_id, point);
        if item_button != nil {
            return item_button;
        }

        let hit: id = msg_send![super(this_id, class!(NSVisualEffectView)), hitTest: point];
        let button = nearest_footer_button(hit);
        if button != nil {
            return button;
        }
        let item_button = nearest_footer_item_button(hit);
        if item_button != nil {
            return item_button;
        }
        nil
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_mouse_down(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    tracing::debug!(
        target: "script_kit::footer_popup",
        event = "native_footer_background_mouse_swallowed",
        "Swallowed background mouseDown in native footer"
    );
}

#[cfg(target_os = "macos")]
extern "C" fn footer_mouse_up(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    tracing::debug!(
        target: "script_kit::footer_popup",
        event = "native_footer_background_mouse_up_swallowed",
        "Swallowed background mouseUp in native footer"
    );
}

#[cfg(target_os = "macos")]
extern "C" fn footer_mouse_dragged(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    tracing::debug!(
        target: "script_kit::footer_popup",
        event = "native_footer_background_mouse_dragged_swallowed",
        "Swallowed background mouseDragged in native footer"
    );
}

#[cfg(target_os = "macos")]
extern "C" fn footer_scroll_wheel(this: &objc::runtime::Object, _: objc::runtime::Sel, event: id) {
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: Forward scroll events to the next responder so the GPUI list
    // behind the footer can scroll.
    unsafe {
        let next: id = msg_send![this, nextResponder];
        if next != nil {
            let _: () = msg_send![next, scrollWheel: event];
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_accepts_first_mouse(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _: id,
) -> cocoa::base::BOOL {
    YES
}
