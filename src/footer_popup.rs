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

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterButtonConfig {
    pub action: FooterAction,
    pub key: SharedString,
    pub label: SharedString,
    pub selected: bool,
    pub enabled: bool,
    pub disabled_reason: Option<&'static str>,
    /// Optional status dot rendered at the leading edge of the button, INSIDE
    /// the chip (e.g. the Agent Chat streaming/idle dot on the Agent·Model chip). When
    /// `Some(_)` a fixed-width dot lane is reserved so the chip's width stays
    /// stable as the status changes; `Some(Hidden)` reserves the lane but draws
    /// nothing. `None` reserves no lane (the common case — keeps ScriptList and
    /// every other button dot-free).
    pub leading_dot: Option<FooterDotStatus>,
    /// Place this ordinary shared footer button on the leading rail.
    pub left_pinned: bool,
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
        Self {
            action,
            key: key.into(),
            label: label.into(),
            selected: false,
            enabled: true,
            disabled_reason: None,
            leading_dot: None,
            left_pinned: false,
        }
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn left_pinned(mut self) -> Self {
        self.left_pinned = true;
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
        self
    }

    pub(crate) fn disabled_reason(mut self, reason: &'static str) -> Self {
        self.disabled_reason = Some(reason);
        self.enabled = false;
        self
    }
}

impl FooterAction {
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

impl MainWindowFooterConfig {
    pub(crate) fn new(surface: &'static str, buttons: Vec<FooterButtonConfig>) -> Self {
        let config = Self {
            surface,
            buttons,
            left_info: None,
        };
        let model = config.slot_model();
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
                duplicate_shortcut_keys = ?model.duplicate_shortcut_keys,
                violation,
                "Main window footer slot contract violation"
            );
        }
        config
    }

    pub(crate) fn slot_model(&self) -> MainWindowFooterSlotModel {
        let mut action_slot_count = 0usize;
        let mut context_chip_count = 0usize;
        let mut shortcut_counts = BTreeMap::<String, usize>::new();

        for button in &self.buttons {
            match footer_button_slot_role(button) {
                FooterSlotRole::ActionSlot => {
                    action_slot_count += 1;
                    let key = button.key.trim();
                    if !key.is_empty() {
                        *shortcut_counts.entry(key.to_string()).or_insert(0) += 1;
                    }
                }
                FooterSlotRole::ContextChip => {
                    context_chip_count += 1;
                }
            }
        }

        let duplicate_shortcut_keys = shortcut_counts
            .into_iter()
            .filter_map(|(key, count)| (count > 1).then_some(key))
            .collect::<Vec<_>>();
        let violation = if action_slot_count > MAIN_WINDOW_FOOTER_MAX_ACTION_SLOTS {
            Some("too_many_action_slots")
        } else if !duplicate_shortcut_keys.is_empty() {
            Some("duplicate_shortcut_keys")
        } else {
            None
        };

        MainWindowFooterSlotModel {
            surface: self.surface,
            button_count: self.buttons.len(),
            action_slot_count,
            context_chip_count,
            duplicate_shortcut_keys,
            violation,
        }
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

static FOOTER_ACTION_CHANNEL: std::sync::LazyLock<(
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
)> = std::sync::LazyLock::new(|| async_channel::bounded(32));

static DICTATION_FOOTER_ACTION_CHANNEL: std::sync::LazyLock<(
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
)> = std::sync::LazyLock::new(|| async_channel::bounded(32));

static AGENT_CHAT_FOOTER_ACTION_CHANNEL: std::sync::LazyLock<(
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
)> = std::sync::LazyLock::new(|| async_channel::bounded(32));

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MainWindowFooterHostSnapshot {
    pub requested_surface: Option<&'static str>,
    pub installed_surface: Option<&'static str>,
    pub native_host_installed: bool,
}

static MAIN_WINDOW_FOOTER_HOST_STATE: std::sync::Mutex<MainWindowFooterHostSnapshot> =
    std::sync::Mutex::new(MainWindowFooterHostSnapshot {
        requested_surface: None,
        installed_surface: None,
        native_host_installed: false,
    });

#[derive(Clone, Debug, PartialEq, Eq)]
struct MainWindowFooterRefreshSignature {
    config: MainWindowFooterConfig,
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

static MAIN_WINDOW_FOOTER_REFRESH_SIGNATURE: std::sync::Mutex<
    Option<MainWindowFooterRefreshSignature>,
> = std::sync::Mutex::new(None);

static MAIN_WINDOW_FOOTER_LAST_CONFIG: std::sync::Mutex<Option<MainWindowFooterConfig>> =
    std::sync::Mutex::new(None);

struct GpuiFooterOverlaySlot {
    handle: WindowHandle<GpuiFooterOverlay>,
    parent_window_handle: AnyWindowHandle,
}

/// Stable automation-registry identity for the GPUI footer overlay window so
/// DevTools primitives (captureWindow, inspectAutomationWindow) can target it.
const GPUI_FOOTER_OVERLAY_AUTOMATION_ID: &str = "footer-overlay";
const GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID: &str = "gpui-footer-overlay";
const GPUI_FOOTER_OVERLAY_WINDOW_TITLE: &str = "Script Kit Footer Overlay";

fn automation_bounds_from_gpui(bounds: Bounds<Pixels>) -> crate::protocol::AutomationWindowBounds {
    crate::protocol::AutomationWindowBounds {
        x: f32::from(bounds.origin.x) as f64,
        y: f32::from(bounds.origin.y) as f64,
        width: f32::from(bounds.size.width) as f64,
        height: f32::from(bounds.size.height) as f64,
    }
}

static MAIN_WINDOW_GPUI_FOOTER_OVERLAY: OnceLock<Mutex<Option<GpuiFooterOverlaySlot>>> =
    OnceLock::new();

static MAIN_WINDOW_GPUI_FOOTER_OVERLAY_FIDELITY: OnceLock<
    Mutex<Option<crate::protocol::FidelityPaintTargetSnapshot>>,
> = OnceLock::new();

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AppKitFidelityCaptureOutcome {
    pub status: crate::protocol::FidelityCaptureStatus,
    pub snapshot: Option<crate::protocol::AppKitFidelitySnapshot>,
}

impl AppKitFidelityCaptureOutcome {
    fn blocked(status: crate::protocol::FidelityCaptureStatus) -> Self {
        Self {
            status,
            snapshot: None,
        }
    }

    fn captured(snapshot: crate::protocol::AppKitFidelitySnapshot) -> Self {
        Self {
            status: crate::protocol::FidelityCaptureStatus::Captured,
            snapshot: Some(snapshot),
        }
    }
}

fn appkit_fidelity_inventory_blocker(
    nodes: &[crate::protocol::AppKitFidelityNode],
) -> Option<crate::protocol::FidelityCaptureStatus> {
    if nodes.is_empty() {
        return Some(crate::protocol::FidelityCaptureStatus::EmptyInventory);
    }
    let unique_ids: BTreeSet<_> = nodes.iter().map(|node| node.id.as_str()).collect();
    (unique_ids.len() != nodes.len())
        .then_some(crate::protocol::FidelityCaptureStatus::DuplicateIdentifiers)
}

fn clear_main_footer_overlay_fidelity_snapshot() {
    let storage = MAIN_WINDOW_GPUI_FOOTER_OVERLAY_FIDELITY.get_or_init(|| Mutex::new(None));
    if let Ok(mut snapshot) = storage.lock() {
        *snapshot = None;
    }
}

fn store_main_footer_overlay_fidelity_snapshot(
    snapshot: crate::protocol::FidelityPaintTargetSnapshot,
) {
    let storage = MAIN_WINDOW_GPUI_FOOTER_OVERLAY_FIDELITY.get_or_init(|| Mutex::new(None));
    if let Ok(mut current) = storage.lock() {
        *current = Some(snapshot);
    }
}

pub(crate) fn main_footer_overlay_fidelity_snapshot(
) -> Option<crate::protocol::FidelityPaintTargetSnapshot> {
    MAIN_WINDOW_GPUI_FOOTER_OVERLAY_FIDELITY
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|snapshot| snapshot.clone())
}

struct GpuiFooterOverlay {
    config: MainWindowFooterConfig,
    overlay_width_px: f32,
    last_reported_row_palette: Option<crate::theme::MainMenuRowStatePalette>,
}

impl GpuiFooterOverlay {
    fn new(config: MainWindowFooterConfig, overlay_width_px: f32) -> Self {
        Self {
            config,
            overlay_width_px,
            last_reported_row_palette: None,
        }
    }

    fn set_config(&mut self, config: MainWindowFooterConfig, overlay_width_px: f32) {
        self.config = config;
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
            // Clickable left-info markers (footer tip, Agent Chat profile)
            // are real footer buttons: same hover pill, radius, and pressed
            // fill as the trailing action buttons, with label/keycap/glyph
            // brightening through the shared footer-action-button group.
            let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
            let hover_bg = rgba(
                row_states
                    .hover
                    .background_rgba
                    .expect("main-menu hover row state always provides a background"),
            );
            let active_bg = rgba(
                row_states
                    .active
                    .background_rgba
                    .expect("main-menu active row state always provides a background"),
            );
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
                        if matches!(action, FooterAction::Tips) {
                            send_footer_action_to_channel(action, false);
                        } else {
                            dispatch_agent_chat_footer_action(action);
                        }
                    },
                );
        } else {
            row = row.flex_1();
        }

        if info.selected && interactive {
            row =
                row.bg(rgba(row_states.active.background_rgba.expect(
                    "main-menu active row state always provides a background",
                )));
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
        let selected_bg = rgba(
            row_states
                .active
                .background_rgba
                .expect("main-menu active row state always provides a background"),
        );
        let hover_bg = rgba(
            row_states
                .hover
                .background_rgba
                .expect("main-menu hover row state always provides a background"),
        );
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
        let fidelity_id = format!(
            "agent-chat.footer-overlay.button.{}",
            footer_action_key(action)
        );
        let mut item = div()
            .id(format!(
                "gpui-footer-overlay-button-{}",
                footer_action_key(action)
            ))
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
                    button.key.clone(),
                    crate::components::footer_chrome::FooterHintKeyMode::Shortcut,
                    theme,
                    key_first,
                    justify,
                    button.selected,
                )
            } else {
                crate::components::footer_chrome::render_footer_hint_content_flex(
                    button.label.clone(),
                    button.key.clone(),
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
                        send_footer_action_to_channel(action, false);
                    }),
                );
        } else {
            item = item.opacity(0.45);
        }

        item.into_any_element()
    }
}

impl Render for GpuiFooterOverlay {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if window.fidelity_capture_active() {
            // App effects flush after this draw completes, so the deferred
            // callback observes the completed frame rendered below. This is
            // also deterministic on GPUI's test platform, whose platform
            // frame callback is intentionally inert.
            window.defer(cx, |window, _cx| {
                if !window.fidelity_capture_active() {
                    return;
                }
                let snapshot = crate::fidelity_capture::paint_target_snapshot(
                    window,
                    GPUI_FOOTER_OVERLAY_FIDELITY_TARGET_ID,
                    "footerOverlay",
                    Some("main".to_string()),
                );
                store_main_footer_overlay_fidelity_snapshot(snapshot);
            });
        } else {
            clear_main_footer_overlay_fidelity_snapshot();
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
    *MAIN_WINDOW_FOOTER_REFRESH_SIGNATURE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = None;
}

fn set_main_window_footer_last_config(config: Option<&MainWindowFooterConfig>) {
    *MAIN_WINDOW_FOOTER_LAST_CONFIG
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = config.cloned();
}

/// Re-apply the last resolved footer config after native geometry, backing
/// scale, appearance, or visibility changed outside a GPUI render pass.
/// This never creates a second window: it only reconciles the footer already
/// owned by the main NSWindow and removes it when fallback mode is active.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn refresh_main_window_footer_from_last_config(ns_window: id) {
    let config = MAIN_WINDOW_FOOTER_LAST_CONFIG
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .clone();
    sync_main_window_glass_scroll_bands(ns_window);
    if let Some(config) = config.as_ref() {
        let _ = refresh_main_footer_host(ns_window, config);
    } else {
        remove_main_window_footer_host(ns_window);
    }
}

fn close_gpui_footer_overlay(cx: &mut App) {
    clear_main_footer_overlay_fidelity_snapshot();
    let storage = MAIN_WINDOW_GPUI_FOOTER_OVERLAY.get_or_init(|| Mutex::new(None));
    let slot = storage.lock().ok().and_then(|mut guard| guard.take());
    if let Some(slot) = slot {
        let _ = slot.handle.update(cx, |_overlay, window, _cx| {
            window.remove_window();
        });
        crate::windows::remove_automation_window(GPUI_FOOTER_OVERLAY_AUTOMATION_ID);
        crate::windows::remove_runtime_window_handle(GPUI_FOOTER_OVERLAY_AUTOMATION_ID);
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
    config: MainWindowFooterConfig,
) {
    // Re-check ownership at execution time. Appearance/vibrancy changes can
    // land between the caller scheduling this work and GPUI running it; a
    // stale fallback request must never recreate a second footer window after
    // native one-window glass mode became active.
    if !main_footer_gpui_overlay_active() {
        close_gpui_footer_overlay(cx);
        return;
    }

    let bounds = gpui_footer_overlay_bounds(parent_bounds);
    clear_main_footer_overlay_fidelity_snapshot();
    let overlay_width_px: f32 = bounds.size.width.into();
    let storage = MAIN_WINDOW_GPUI_FOOTER_OVERLAY.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = storage.lock() {
        if let Some(slot) = guard.as_ref() {
            if slot.parent_window_handle == parent_window_handle {
                let update_result = slot.handle.update(cx, |overlay, window, cx| {
                    overlay.set_config(config.clone(), overlay_width_px);
                    set_gpui_footer_overlay_window_bounds(window, bounds, cx);
                    cx.notify();
                });
                if update_result.is_ok() {
                    crate::windows::set_automation_bounds(
                        GPUI_FOOTER_OVERLAY_AUTOMATION_ID,
                        Some(automation_bounds_from_gpui(bounds)),
                    );
                    let overlay_handle = slot.handle;
                    park_overlay_during_glass_morph(overlay_handle, cx);
                    return;
                }
                *guard = None;
            } else {
                let _ = slot.handle.update(cx, |_overlay, window, _cx| {
                    window.remove_window();
                });
                *guard = None;
            }
        }
    }

    let options = gpui_footer_overlay_window_options(bounds, display_id);
    let Ok(handle) = cx.open_window(options, |_window, cx| {
        cx.new(|_| GpuiFooterOverlay::new(config.clone(), overlay_width_px))
    }) else {
        tracing::warn!(
            target: "script_kit::footer_popup",
            event = "gpui_footer_overlay_open_failed",
            "Failed to open GPUI footer overlay"
        );
        return;
    };

    if configure_gpui_footer_overlay_window(&handle, cx, parent_window_handle).is_err() {
        let _ = handle.update(cx, |_overlay, window, _cx| {
            window.remove_window();
        });
        return;
    }

    // Register the overlay's live GPUI handle so simulated pointer events
    // (hover proofs, click probes) dispatch into this window's own scene
    // instead of falling back to parent-translated main-window dispatch,
    // which can never reach elements painted by this renderer.
    crate::windows::upsert_runtime_window_handle(GPUI_FOOTER_OVERLAY_AUTOMATION_ID, handle.into());

    if let Ok(mut guard) = storage.lock() {
        *guard = Some(GpuiFooterOverlaySlot {
            handle,
            parent_window_handle,
        });
    }

    park_overlay_during_glass_morph(handle, cx);

    if let Err(error) = crate::windows::register_attached_popup(
        GPUI_FOOTER_OVERLAY_AUTOMATION_ID.to_string(),
        crate::protocol::AutomationWindowKind::PromptPopup,
        Some(GPUI_FOOTER_OVERLAY_WINDOW_TITLE.to_string()),
        Some("footerOverlay".to_string()),
        Some(automation_bounds_from_gpui(bounds)),
        Some("main"),
    ) {
        tracing::warn!(
            target: "script_kit::footer_popup",
            event = "gpui_footer_overlay_automation_register_failed",
            %error,
            "GPUI footer overlay opened but automation registration failed"
        );
    }
}

fn update_main_window_footer_host_state(
    requested_surface: Option<&'static str>,
    installed_surface: Option<&'static str>,
    native_host_installed: bool,
) {
    *MAIN_WINDOW_FOOTER_HOST_STATE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = MainWindowFooterHostSnapshot {
        requested_surface,
        installed_surface,
        native_host_installed,
    };
}

pub(crate) fn main_window_footer_host_snapshot() -> MainWindowFooterHostSnapshot {
    *MAIN_WINDOW_FOOTER_HOST_STATE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
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

pub(crate) fn footer_action_channel() -> &'static (
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
) {
    &FOOTER_ACTION_CHANNEL
}

pub(crate) fn dictation_footer_action_channel() -> &'static (
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
) {
    &DICTATION_FOOTER_ACTION_CHANNEL
}

pub(crate) fn agent_chat_footer_action_channel() -> &'static (
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
) {
    &AGENT_CHAT_FOOTER_ACTION_CHANNEL
}

pub(crate) fn dispatch_agent_chat_footer_action(action: FooterAction) {
    if let Err(error) = agent_chat_footer_action_channel().0.try_send(action) {
        tracing::warn!(
            target: "script_kit::footer_popup",
            event = "agent_chat_footer_left_info_action_send_failed",
            action = footer_action_key(action),
            %error,
            "Failed to enqueue Agent Chat footer left-info action"
        );
    }
}

pub(crate) fn sync_main_footer_popup(
    window: &mut Window,
    config: Option<&MainWindowFooterConfig>,
    cx: &mut App,
) {
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
    set_main_window_footer_last_config(config);
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

pub(crate) fn sync_window_footer_popup(window: &mut Window, config: &MainWindowFooterConfig) {
    #[cfg(target_os = "macos")]
    {
        let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
            tracing::warn!(
                target: "script_kit::footer_popup",
                event = "native_footer_missing_ns_window",
                surface = config.surface,
                "Unable to resolve NSWindow for reusable native footer host"
            );
            return;
        };

        // SAFETY: `ns_window` comes from the live GPUI window currently being
        // rendered/observed on the AppKit thread.
        unsafe {
            let installed = ensure_reusable_window_footer_host(ns_window);
            if installed {
                let _ = refresh_window_footer_host(ns_window, config);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, config);
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
pub(crate) fn clear_window_footer_popup(window: &mut Window) {
    #[cfg(target_os = "macos")]
    {
        let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
            return;
        };

        // SAFETY: `ns_window` comes from the live GPUI window currently being
        // rendered/observed on the AppKit main thread.
        unsafe {
            remove_reusable_window_footer_host(ns_window);
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub(crate) fn close_main_footer_popup(cx: &mut App) {
    set_main_window_footer_last_config(None);
    clear_main_window_footer_refresh_signature();
    update_main_window_footer_host_state(None, None, false);
    close_gpui_footer_overlay(cx);

    let Some(window_handle) = crate::get_main_window_handle() else {
        return;
    };

    let _ = window_handle.update(cx, move |_, window, _cx| {
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

#[cfg(target_os = "macos")]
fn window_gpui_view_and_ns_window(window: &Window) -> Option<(id, id)> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: `ns_view` comes from a live GPUI window on the AppKit
            // main thread. `-[NSView window]` returns the owning NSWindow or nil.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some((ns_view, ns_window));
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn appkit_layout_bounds(rect: cocoa::foundation::NSRect) -> crate::protocol::LayoutBounds {
    crate::protocol::LayoutBounds {
        x: rect.origin.x as f32,
        y: rect.origin.y as f32,
        width: rect.size.width as f32,
        height: rect.size.height as f32,
    }
}

#[cfg(target_os = "macos")]
fn appkit_screenshot_bounds(
    window_rect: cocoa::foundation::NSRect,
    screenshot_height: f64,
) -> crate::protocol::LayoutBounds {
    crate::protocol::LayoutBounds {
        x: window_rect.origin.x as f32,
        y: (screenshot_height - window_rect.origin.y - window_rect.size.height) as f32,
        width: window_rect.size.width as f32,
        height: window_rect.size.height as f32,
    }
}

#[cfg(target_os = "macos")]
unsafe fn appkit_ns_string(value: id) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;

    if value == nil {
        return None;
    }
    let utf8: *const std::os::raw::c_char = msg_send![value, UTF8String];
    if utf8.is_null() {
        return None;
    }
    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_view_identifier(view: id) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return None;
    }
    let identifier: id = msg_send![view, identifier];
    appkit_ns_string(identifier).filter(|identifier| !identifier.is_empty())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_class_name(view: id) -> String {
    use objc::{msg_send, sel, sel_impl};

    let class: id = msg_send![view, class];
    let class_name: id = if class == nil {
        nil
    } else {
        msg_send![class, className]
    };
    appkit_ns_string(class_name).unwrap_or_else(|| "unknown".to_string())
}

#[cfg(target_os = "macos")]
unsafe fn appkit_color_from_ns_color(color: id) -> Option<crate::protocol::AppKitFidelityColor> {
    use objc::{class, msg_send, sel, sel_impl};

    if color == nil {
        return None;
    }
    let color_space: id = msg_send![class!(NSColorSpace), sRGBColorSpace];
    let color: id = msg_send![color, colorUsingColorSpace: color_space];
    if color == nil {
        return None;
    }
    let mut red = 0.0_f64;
    let mut green = 0.0_f64;
    let mut blue = 0.0_f64;
    let mut alpha = 0.0_f64;
    let _: () = msg_send![
        color,
        getRed: &mut red
        green: &mut green
        blue: &mut blue
        alpha: &mut alpha
    ];
    Some(crate::protocol::AppKitFidelityColor {
        red,
        green,
        blue,
        alpha,
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_color_from_cg_color(color: id) -> Option<crate::protocol::AppKitFidelityColor> {
    use objc::{class, msg_send, sel, sel_impl};

    if color == nil {
        return None;
    }
    let ns_color: id = msg_send![class!(NSColor), colorWithCGColor: color];
    appkit_color_from_ns_color(ns_color)
}

#[cfg(target_os = "macos")]
unsafe fn appkit_layer_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityLayer> {
    use objc::{msg_send, sel, sel_impl};

    let layer: id = msg_send![view, layer];
    if layer == nil {
        return None;
    }
    let contents_scale: f64 = msg_send![layer, contentsScale];
    let masks_to_bounds: cocoa::base::BOOL = msg_send![layer, masksToBounds];
    let border_width: f64 = msg_send![layer, borderWidth];
    let corner_radius: f64 = msg_send![layer, cornerRadius];
    let background_color: id = msg_send![layer, backgroundColor];
    let border_color: id = msg_send![layer, borderColor];
    let shadow_opacity: f32 = msg_send![layer, shadowOpacity];
    let shadow_radius: f64 = msg_send![layer, shadowRadius];
    let shadow_offset: cocoa::foundation::NSSize = msg_send![layer, shadowOffset];
    let shadow_path: id = msg_send![layer, shadowPath];
    Some(crate::protocol::AppKitFidelityLayer {
        contents_scale,
        masks_to_bounds: masks_to_bounds == YES,
        border_width,
        corner_radius,
        background_color: appkit_color_from_cg_color(background_color),
        border_color: appkit_color_from_cg_color(border_color),
        shadow_opacity: f64::from(shadow_opacity),
        shadow_radius,
        shadow_offset_x: shadow_offset.width,
        shadow_offset_y: shadow_offset.height,
        has_shadow_path: shadow_path != nil,
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_text_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityText> {
    use objc::{class, msg_send, sel, sel_impl};
    use sha2::{Digest as _, Sha256};

    let is_text_field: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSTextField)];
    if is_text_field != YES {
        return None;
    }
    let value: id = msg_send![view, stringValue];
    let value = appkit_ns_string(value).unwrap_or_default();
    let font: id = msg_send![view, font];
    let font_name = if font == nil {
        String::new()
    } else {
        let name: id = msg_send![font, fontName];
        appkit_ns_string(name).unwrap_or_default()
    };
    let font_size: f64 = if font == nil {
        0.0
    } else {
        msg_send![font, pointSize]
    };
    let font_weight: isize = if font == nil {
        0
    } else {
        let manager: id = msg_send![class!(NSFontManager), sharedFontManager];
        msg_send![manager, weightOfFont: font]
    };
    let alignment: usize = msg_send![view, alignment];
    let fitting_size: cocoa::foundation::NSSize = msg_send![view, fittingSize];
    let text_color: id = msg_send![view, textColor];
    let mut hasher = Sha256::new();
    hasher.update(value.as_bytes());
    Some(crate::protocol::AppKitFidelityText {
        value,
        value_sha256: format!("{:x}", hasher.finalize()),
        font_name,
        font_size,
        font_weight: font_weight as i64,
        alignment: alignment as i64,
        fitting_size: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width: fitting_size.width as f32,
            height: fitting_size.height as f32,
        },
        color: appkit_color_from_ns_color(text_color),
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_image_fidelity(view: id) -> Option<crate::protocol::AppKitFidelityImage> {
    use objc::{class, msg_send, sel, sel_impl};

    let is_image_view: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSImageView)];
    if is_image_view != YES {
        return None;
    }
    let image: id = msg_send![view, image];
    let size = if image == nil {
        cocoa::foundation::NSSize::new(0.0, 0.0)
    } else {
        msg_send![image, size]
    };
    let supports_tint: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(contentTintColor)];
    let tint: id = if supports_tint == YES {
        msg_send![view, contentTintColor]
    } else {
        nil
    };
    Some(crate::protocol::AppKitFidelityImage {
        width: size.width,
        height: size.height,
        tint: appkit_color_from_ns_color(tint),
    })
}

#[cfg(target_os = "macos")]
unsafe fn appkit_action_selector(view: id) -> Option<String> {
    use objc::{class, msg_send, sel, sel_impl};

    let is_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSButton)];
    if is_button != YES {
        return None;
    }
    let action: objc::runtime::Sel = msg_send![view, action];
    (!action.as_ptr().is_null()).then(|| action.name().to_string())
}

#[cfg(target_os = "macos")]
unsafe fn collect_identified_appkit_views(
    view: id,
    content_view: id,
    parent_id: Option<String>,
    subview_order: usize,
    screenshot_height: f64,
    nodes: &mut Vec<crate::protocol::AppKitFidelityNode>,
) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let identifier = appkit_view_identifier(view);
    let node_parent_id = parent_id.clone();
    let child_parent_id = identifier.clone().or(parent_id);
    if let Some(identifier) = identifier {
        let frame: cocoa::foundation::NSRect = msg_send![view, frame];
        let bounds: cocoa::foundation::NSRect = msg_send![view, bounds];
        let window_frame: cocoa::foundation::NSRect =
            msg_send![view, convertRect: bounds toView: content_view];
        let hidden: cocoa::base::BOOL = msg_send![view, isHidden];
        let alpha: f64 = msg_send![view, alphaValue];
        let layer = appkit_layer_fidelity(view);
        let layer_masks_to_bounds = layer
            .as_ref()
            .map(|layer| layer.masks_to_bounds)
            .unwrap_or(false);
        nodes.push(crate::protocol::AppKitFidelityNode {
            id: identifier,
            parent_id: node_parent_id,
            class_name: appkit_class_name(view),
            subview_order,
            frame: appkit_layout_bounds(frame),
            bounds: appkit_layout_bounds(bounds),
            window_frame: appkit_layout_bounds(window_frame),
            screenshot_frame: appkit_screenshot_bounds(window_frame, screenshot_height),
            hidden: hidden == YES,
            alpha,
            // `-[NSView clipsToBounds]` is not available on every supported
            // macOS SDK/runtime pair. The backing layer is the raster clipping
            // authority here and avoids an unrecognized-selector crash.
            clips_to_bounds: layer_masks_to_bounds,
            layer,
            text: appkit_text_fidelity(view),
            image: appkit_image_fidelity(view),
            action_selector: appkit_action_selector(view),
        });
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        collect_identified_appkit_views(
            child,
            content_view,
            child_parent_id.clone(),
            index,
            screenshot_height,
            nodes,
        );
    }
}

#[cfg(target_os = "macos")]
unsafe fn appkit_subview_order(parent: id, child: id) -> usize {
    use objc::{msg_send, sel, sel_impl};

    let subviews: id = msg_send![parent, subviews];
    if subviews == nil {
        return 0;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let candidate: id = msg_send![subviews, objectAtIndex: index];
        if candidate == child {
            return index;
        }
    }
    0
}

/// Collect capture-only AppKit telemetry for the in-window footer material
/// host. The separate GPUI glyph overlay is intentionally excluded and emitted
/// through `main_footer_overlay_fidelity_snapshot`.
pub(crate) fn collect_main_footer_appkit_fidelity_snapshot(
    window: &Window,
) -> AppKitFidelityCaptureOutcome {
    if !window.fidelity_capture_active() {
        return AppKitFidelityCaptureOutcome::blocked(
            crate::protocol::FidelityCaptureStatus::NotRequested,
        );
    }

    #[cfg(target_os = "macos")]
    {
        let Some((_, ns_window)) = window_gpui_view_and_ns_window(window) else {
            return AppKitFidelityCaptureOutcome::blocked(
                crate::protocol::FidelityCaptureStatus::MissingWindow,
            );
        };
        // SAFETY: `ns_window` and its content tree belong to the live main
        // window. getLayoutInfo invokes this on the AppKit/GPUI main thread.
        unsafe {
            use objc::{msg_send, sel, sel_impl};

            let content_view: id = msg_send![ns_window, contentView];
            if content_view == nil {
                return AppKitFidelityCaptureOutcome::blocked(
                    crate::protocol::FidelityCaptureStatus::MissingContentView,
                );
            }
            let search_root = main_window_footer_search_root(ns_window);
            let footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
            if footer_view == nil {
                return AppKitFidelityCaptureOutcome::blocked(
                    crate::protocol::FidelityCaptureStatus::MissingFooterHost,
                );
            }
            let content_bounds: cocoa::foundation::NSRect = msg_send![search_root, bounds];
            let mut nodes = Vec::new();
            let footer_order = appkit_subview_order(search_root, footer_view);
            collect_identified_appkit_views(
                footer_view,
                search_root,
                None,
                footer_order,
                content_bounds.size.height,
                &mut nodes,
            );
            if let Some(status) = appkit_fidelity_inventory_blocker(&nodes) {
                return AppKitFidelityCaptureOutcome::blocked(status);
            }

            let backdrop = find_subview_by_identifier(
                content_view,
                crate::platform::TAHOE_GLASS_BACKDROP_IDENTIFIER,
            );
            let footer_container =
                find_subview_by_identifier(content_view, FOOTER_GLASS_CONTAINER_ID);
            let main_backdrop_frame = (backdrop != nil).then(|| {
                let frame: cocoa::foundation::NSRect = msg_send![backdrop, frame];
                appkit_layout_bounds(frame)
            });
            let footer_container_frame = (footer_container != nil).then(|| {
                let frame: cocoa::foundation::NSRect = msg_send![footer_container, frame];
                appkit_layout_bounds(frame)
            });
            let (transparent_gap_points, backdrop_footer_intersection_area) =
                match (&main_backdrop_frame, &footer_container_frame) {
                    (Some(backdrop), Some(footer)) => {
                        let gap = backdrop.y - (footer.y + footer.height);
                        let overlap_width = (backdrop.x + backdrop.width)
                            .min(footer.x + footer.width)
                            - backdrop.x.max(footer.x);
                        let overlap_height = (backdrop.y + backdrop.height)
                            .min(footer.y + footer.height)
                            - backdrop.y.max(footer.y);
                        (
                            Some(gap),
                            Some(overlap_width.max(0.0) * overlap_height.max(0.0)),
                        )
                    }
                    _ => (None, None),
                };
            let has_shadow: cocoa::base::BOOL = msg_send![ns_window, hasShadow];
            let main_backdrop_layer = (backdrop != nil)
                .then(|| appkit_layer_fidelity(backdrop))
                .flatten();
            let mut material_bearing_view_ids = nodes
                .iter()
                .filter(|node| node.class_name == "NSGlassEffectView")
                .map(|node| node.id.clone())
                .collect::<Vec<_>>();
            if backdrop != nil {
                material_bearing_view_ids
                    .push(crate::platform::TAHOE_GLASS_BACKDROP_IDENTIFIER.to_string());
            }
            material_bearing_view_ids.sort();
            material_bearing_view_ids.dedup();

            AppKitFidelityCaptureOutcome::captured(crate::protocol::AppKitFidelitySnapshot {
                target_id: "main-footer-host".to_string(),
                target_kind: "appKitFooterHost".to_string(),
                coordinate_space: "appkit-content-bottom-left+screenshot-top-left".to_string(),
                window_bounds: crate::fidelity_capture::layout_bounds(window.bounds()),
                main_backdrop_frame,
                footer_container_frame,
                transparent_gap_points,
                backdrop_footer_intersection_area,
                outer_window_has_shadow: Some(has_shadow == YES),
                main_backdrop_layer,
                footer_left_allocation: footer_left_allocation_snapshot(),
                material_bearing_view_ids,
                nodes,
            })
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        AppKitFidelityCaptureOutcome::blocked(
            crate::protocol::FidelityCaptureStatus::UnsupportedPlatform,
        )
    }
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

/// Canonical partition of the expanded main-window host used by the detached
/// footer composition. The same physical regions are expressed in either
/// GPUI's top-left coordinate space or AppKit's bottom-left coordinate space.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MainWindowDetachedFooterRegions {
    pub host: crate::protocol::LayoutBounds,
    pub main_content: crate::protocol::LayoutBounds,
    pub transparent_gap: crate::protocol::LayoutBounds,
    pub footer: crate::protocol::LayoutBounds,
}

fn round_footer_region_value(value: f32, backing_scale: f32) -> f32 {
    let scale = if backing_scale.is_finite() && backing_scale > 0.0 {
        backing_scale
    } else {
        1.0
    };
    (value * scale).round() / scale
}

fn main_window_detached_footer_region_dimensions(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> (f32, f32, f32, f32) {
    let width = round_footer_region_value(width.max(0.0), backing_scale);
    let host_height = round_footer_region_value(host_height.max(0.0), backing_scale);
    let footer_height =
        round_footer_region_value(footer_height.max(0.0).min(host_height), backing_scale)
            .min(host_height);
    let gap_height = round_footer_region_value(
        gap_height.max(0.0).min(host_height - footer_height),
        backing_scale,
    )
    .min(host_height - footer_height);
    let main_height = host_height - footer_height - gap_height;
    (width, host_height, main_height, gap_height)
}

/// Partition an expanded host in GPUI's top-left, y-down coordinate space.
pub(crate) fn main_window_detached_footer_regions_gpui(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> MainWindowDetachedFooterRegions {
    let (width, host_height, main_height, gap_height) =
        main_window_detached_footer_region_dimensions(
            width,
            host_height,
            footer_height,
            gap_height,
            backing_scale,
        );
    let footer_height = host_height - main_height - gap_height;
    MainWindowDetachedFooterRegions {
        host: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: host_height,
        },
        main_content: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: main_height,
        },
        transparent_gap: crate::protocol::LayoutBounds {
            x: 0.0,
            y: main_height,
            width,
            height: gap_height,
        },
        footer: crate::protocol::LayoutBounds {
            x: 0.0,
            y: main_height + gap_height,
            width,
            height: footer_height,
        },
    }
}

/// Partition an expanded host in AppKit's bottom-left, y-up coordinate space.
pub(crate) fn main_window_detached_footer_regions_appkit(
    width: f32,
    host_height: f32,
    footer_height: f32,
    gap_height: f32,
    backing_scale: f32,
) -> MainWindowDetachedFooterRegions {
    let (width, host_height, main_height, gap_height) =
        main_window_detached_footer_region_dimensions(
            width,
            host_height,
            footer_height,
            gap_height,
            backing_scale,
        );
    let footer_height = host_height - main_height - gap_height;
    MainWindowDetachedFooterRegions {
        host: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: host_height,
        },
        main_content: crate::protocol::LayoutBounds {
            x: 0.0,
            y: footer_height + gap_height,
            width,
            height: main_height,
        },
        transparent_gap: crate::protocol::LayoutBounds {
            x: 0.0,
            y: footer_height,
            width,
            height: gap_height,
        },
        footer: crate::protocol::LayoutBounds {
            x: 0.0,
            y: 0.0,
            width,
            height: footer_height,
        },
    }
}

/// Height of the fully transparent strip the main window reserves below its
/// glass container so the footer capsules float over the bare desktop.
/// Both the GPUI root (bottom padding) and the native NSGlassEffectView
/// backdrop (bottom frame inset) subtract this same value; 0 when float
/// chrome is off.
pub(crate) fn main_window_float_footer_strip_height() -> f32 {
    if glass_scroll_bands_active() {
        crate::components::footer_chrome::current_main_menu_footer_height()
            + FLOAT_FOOTER_CONTAINER_GAP_PX
    } else {
        0.0
    }
}

/// Reconcile the platform-managed footer band and header edge strip with the
/// main window's current glass mode and frame. Called beside Tahoe backdrop
/// recreation and again when the footer host is created/refreshed.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn sync_main_window_glass_scroll_bands(ns_window: id) {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::base::YES;
    use cocoa::foundation::NSRect;
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("sync_main_window_glass_scroll_bands") {
        return;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }

    let active = glass_scroll_bands_active();
    let search_root = main_window_footer_search_root(ns_window);
    let mut footer_view = find_subview_by_identifier(search_root, FOOTER_EFFECT_ID);
    if footer_view != nil {
        // Mode changed (same-window Tahoe container <-> blur-era in-window
        // VEV): recreate the host in the correct native parent.
        let float_ok = float_footer_host_view_class()
            .map(|cls| {
                let is_float: cocoa::base::BOOL = msg_send![footer_view, isKindOfClass: cls];
                is_float == YES
            })
            .unwrap_or(false);
        if float_ok != active {
            let _: () = msg_send![footer_view, removeFromSuperview];
            if !active {
                remove_main_window_footer_glass_container(ns_window);
            }
            clear_main_window_footer_refresh_signature();
            footer_view = nil;
        }
    }

    let content_bounds: NSRect = msg_send![content_view, bounds];
    if footer_view != nil {
        let footer_frame = cocoa::foundation::NSRect::new(
            cocoa::foundation::NSPoint::new(0.0, 0.0),
            cocoa::foundation::NSSize::new(content_bounds.size.width, footer_height()),
        );
        let _: () = msg_send![footer_view, setFrame: footer_frame];
    }
    if !active && main_window_footer_glass_root(ns_window) != nil {
        remove_main_window_footer_glass_container(ns_window);
    }
    log_strip_views_debug(ns_window);
    let _ = NSViewWidthSizable;
}

/// Identifier for the content view of the floating-footer child window.
#[cfg(target_os = "macos")]
const FLOAT_FOOTER_LAYER_ID: &str = "script-kit-float-footer-layer";

/// Shared styling for every floating footer capsule. The window backdrop and
/// every capsule resolve through the same appearance/RGB/effective-tint
/// policy; only the capsule role may add the shared adaptive separation rim.
#[cfg(target_os = "macos")]
unsafe fn style_float_footer_capsule(capsule: id, theme: &crate::theme::Theme) {
    let style = crate::platform::resolve_native_glass_style(
        theme,
        crate::platform::NativeGlassSurfaceRole::FloatingCapsule,
    );
    let _ = crate::platform::apply_native_glass_style(capsule, style);
}

#[cfg(target_os = "macos")]
thread_local! {
    /// Tahoe main-window footer containers. These are native siblings of the
    /// GPUI Metal view inside the same NSWindow, so WindowServer translates
    /// the complete composition atomically during a live drag.
    static MAIN_WINDOW_FOOTER_GLASS_HOSTS: std::cell::RefCell<
        std::collections::HashMap<usize, crate::platform::glass_button_host::NativeGlassContainerHost>
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

#[cfg(target_os = "macos")]
unsafe fn main_window_footer_glass_root(ns_window: id) -> id {
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        let mut hosts = hosts.borrow_mut();
        hosts.retain(|_, host| host.window_is_alive());
        hosts
            .get(&(ns_window as usize))
            .map(|host| host.inner())
            .unwrap_or(nil)
    })
}

#[cfg(target_os = "macos")]
unsafe fn ensure_main_window_footer_glass_container(gpui_view: id, ns_window: id) -> id {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return nil;
    }
    let content_bounds: NSRect = msg_send![content_view, bounds];
    let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
    let regions = main_window_detached_footer_regions_appkit(
        content_bounds.size.width as f32,
        content_bounds.size.height as f32,
        footer_height() as f32,
        FLOAT_FOOTER_CONTAINER_GAP_PX,
        backing_scale as f32,
    );
    let footer_frame = NSRect::new(
        NSPoint::new(regions.footer.x as f64, regions.footer.y as f64),
        NSSize::new(regions.footer.width as f64, regions.footer.height as f64),
    );

    let existing = MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        let hosts = hosts.borrow();
        hosts.get(&(ns_window as usize)).map(|host| {
            let container = host.container();
            let inner = host.inner();
            let _: () = msg_send![container, setFrame: footer_frame];
            let _: () = msg_send![
                inner,
                setFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(footer_frame.size.width, footer_frame.size.height)
                )
            ];
            inner
        })
    });
    if let Some(root) = existing {
        return root;
    }

    let Some(host) = crate::platform::glass_button_host::install_native_glass_container(
        ns_window,
        gpui_view,
        footer_frame,
        crate::platform::glass_button_host::NativeViewOrdering::AboveGpui,
        crate::platform::glass_button_host::shared_glass_spacing(),
        FOOTER_GLASS_CONTAINER_ID,
    ) else {
        return nil;
    };
    let _: () = msg_send![host.container(), setAutoresizingMask: NSViewWidthSizable];
    let root = host.inner();
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        hosts.borrow_mut().insert(ns_window as usize, host);
    });
    root
}

#[cfg(target_os = "macos")]
unsafe fn remove_main_window_footer_glass_container(ns_window: id) {
    MAIN_WINDOW_FOOTER_GLASS_HOSTS.with(|hosts| {
        hosts.borrow_mut().remove(&(ns_window as usize));
    });
}

/// Registry of the floating-footer window per parent (`(parent_ptr, footer_ptr)`).
///
/// The footer window is deliberately NOT attached via `addChildWindow:` —
/// attached children join the parent's window-server SHADOW GROUP, which puts
/// the capsule shapes back into the parent's shadow shape (the hairline
/// bridge between capsules, probe-proven). Ordering and visibility are
/// managed manually instead: frame/order in `sync_float_footer_child_frame`
/// (render-driven) and hide in the platform `orderOut:` choke points.
#[cfg(target_os = "macos")]
static FLOAT_FOOTER_WINDOWS: std::sync::Mutex<Vec<(usize, usize)>> =
    std::sync::Mutex::new(Vec::new());

/// Find the floating-footer window registered for `ns_window`, if any.
#[cfg(target_os = "macos")]
unsafe fn float_footer_child_window(ns_window: id) -> id {
    let guard = FLOAT_FOOTER_WINDOWS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    guard
        .iter()
        .find(|(parent, _)| *parent == ns_window as usize)
        .map(|(_, footer)| *footer as id)
        .unwrap_or(nil)
}

/// Create (or reuse) the borderless, non-activating child panel that carries
/// the ENTIRE floating footer (host view: buttons, keycaps, left-info, and
/// their per-button glass capsules) below the main container.
///
/// Why a separate window:
/// - NSGlassEffectViews in the SAME window as the Tahoe backdrop auto-merge
///   with it across the 8px container gap (a full-width meniscus "shelf"
///   line bridging the capsules — user-reported).
/// - Any pixels left in the main window's strip (button text, keycaps) put
///   those rows back into the window-server shadow shape, which bridges them
///   into a rectangular rim around the strip.
/// Moving the whole footer out empties the strip completely: the main
/// window's shadow hugs the container, and the footer's glass samples the
/// desktop directly. The child has no shadow of its own.
#[cfg(target_os = "macos")]
unsafe fn ensure_float_footer_child_window(ns_window: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = float_footer_child_window(ns_window);
    if existing != nil {
        return existing;
    }

    let frame: NSRect = msg_send![ns_window, frame];
    let child_frame = NSRect::new(
        NSPoint::new(
            frame.origin.x,
            frame.origin.y - f64::from(main_window_float_footer_strip_height()),
        ),
        NSSize::new(frame.size.width, footer_height()),
    );
    let child: id = msg_send![class!(NSPanel), alloc];
    // styleMask 128 = borderless non-activating panel; backing 2 = buffered.
    let child: id = msg_send![
        child,
        initWithContentRect: child_frame
        styleMask: 128u64
        backing: 2u64
        defer: NO
    ];
    if child == nil {
        return nil;
    }
    let clear: id = msg_send![class!(NSColor), clearColor];
    let _: () = msg_send![child, setBackgroundColor: clear];
    let _: () = msg_send![child, setOpaque: NO];
    let _: () = msg_send![child, setHasShadow: NO];
    let _: () = msg_send![child, setReleasedWhenClosed: NO];
    let _: () = msg_send![child, setBecomesKeyOnlyIfNeeded: YES];
    let level: isize = msg_send![ns_window, level];
    let _: () = msg_send![child, setLevel: level];

    let content: id = msg_send![child, contentView];
    if content != nil {
        let identifier = ns_string(FLOAT_FOOTER_LAYER_ID);
        if identifier != nil {
            let _: () = msg_send![content, setIdentifier: identifier];
        }
        let _: () = msg_send![content, setWantsLayer: YES];
    }

    // Match the parent's Spaces/collection behavior so the footer follows the
    // launcher across Spaces and fullscreen setups.
    let collection_behavior: u64 = msg_send![ns_window, collectionBehavior];
    let _: () = msg_send![child, setCollectionBehavior: collection_behavior];

    FLOAT_FOOTER_WINDOWS
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .push((ns_window as usize, child as usize));

    tracing::info!(
        target: "script_kit::footer_popup",
        event = "float_footer_child_window_installed",
        height = footer_height(),
        "Installed floating-footer window (unattached, shadow-group-free) below the main container"
    );
    child
}

/// Hide and unregister the floating-footer window, if present.
#[cfg(target_os = "macos")]
unsafe fn remove_float_footer_child_window(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    let child = float_footer_child_window(ns_window);
    if child != nil {
        let _: () = msg_send![child, orderOut: nil];
        FLOAT_FOOTER_WINDOWS
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .retain(|(parent, _)| *parent != ns_window as usize);
    }
}

/// Hide the floating-footer window alongside its parent (called from the
/// platform `orderOut:` choke points — the footer is unattached, so AppKit
/// will not hide it for us). Keeps the registration for the next show.
#[cfg(target_os = "macos")]
pub(crate) fn hide_float_footer_for_window(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: called on the main thread from the platform hide paths; the
    // registered footer window pointer is retained for the process lifetime.
    unsafe {
        let child = float_footer_child_window(ns_window);
        if child != nil {
            let _: () = msg_send![child, orderOut: nil];
        }
    }
}

/// Keep the floating-footer window glued to the strip BELOW the main
/// window's frame (the frame ends at the container; the strip is outside it —
/// see `window_resize::physical_main_window_height`) and mirror the parent's
/// on-screen state (unattached window: manual ordering) and appearance (the
/// capsule glass must adapt to the same light/dark appearance as the main
/// window's backdrop, not the child's own resolved appearance).
#[cfg(target_os = "macos")]
unsafe fn sync_float_footer_child_frame(ns_window: id) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    let child = float_footer_child_window(ns_window);
    if child == nil {
        return;
    }
    let main_frame: NSRect = msg_send![ns_window, frame];
    let strip = f64::from(main_window_float_footer_strip_height());
    let child_frame = NSRect::new(
        NSPoint::new(main_frame.origin.x, main_frame.origin.y - strip),
        NSSize::new(main_frame.size.width, footer_height()),
    );
    let _: () = msg_send![child, setFrame: child_frame display: YES];

    let parent_appearance: id = msg_send![ns_window, effectiveAppearance];
    if parent_appearance != nil {
        let _: () = msg_send![child, setAppearance: parent_appearance];
    }

    // Re-assert shadowlessness every pass: the capsule shapes otherwise get a
    // window-server shadow whose row spans bridge the gaps between capsules
    // into a hairline shelf (probe-proven).
    let child_has_shadow: cocoa::base::BOOL = msg_send![child, hasShadow];
    if child_has_shadow == YES {
        let _: () = msg_send![child, setHasShadow: NO];
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "float_footer_shadow_reasserted",
            "Floating footer window shadow was re-enabled by AppKit; disabled again"
        );
    }
    let _: () = msg_send![child, invalidateShadow];

    let parent_visible: cocoa::base::BOOL = msg_send![ns_window, isVisible];
    let child_visible: cocoa::base::BOOL = msg_send![child, isVisible];
    if parent_visible == YES {
        let parent_number: isize = msg_send![ns_window, windowNumber];
        let _: () = msg_send![child, orderWindow: 1isize relativeTo: parent_number];
    } else if child_visible == YES {
        let _: () = msg_send![child, orderOut: nil];
    }
}

/// Resolve the view that footer host lookups should search: the floating
/// child window's contentView when the float footer is active, otherwise the
/// main window's contentView (blur-era in-window host).
#[cfg(target_os = "macos")]
unsafe fn reusable_window_footer_search_root(ns_window: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    let child = float_footer_child_window(ns_window);
    if child != nil {
        let content: id = msg_send![child, contentView];
        if content != nil {
            return content;
        }
    }
    msg_send![ns_window, contentView]
}

/// Main-window footer lookup root. Tahoe uses the same-window glass
/// container's inner view; fallback mode keeps the existing in-content host.
#[cfg(target_os = "macos")]
unsafe fn main_window_footer_search_root(ns_window: id) -> id {
    use objc::{msg_send, sel, sel_impl};

    let glass_root = main_window_footer_glass_root(ns_window);
    if glass_root != nil {
        glass_root
    } else {
        msg_send![ns_window, contentView]
    }
}

/// Debug aid (SCRIPT_KIT_GLASS_BAND_DEBUG=1): walk the contentView tree and
/// log every view whose frame intersects the transparent footer strip, with
/// visibility/alpha/layer state — used to find what still contributes pixels
/// (and therefore window-server shape) inside the strip.
#[cfg(target_os = "macos")]
unsafe fn log_strip_views_debug(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    if std::env::var("SCRIPT_KIT_GLASS_BAND_DEBUG").is_err() {
        return;
    }
    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }
    let strip = f64::from(main_window_float_footer_strip_height());
    if strip <= 0.0 {
        return;
    }

    unsafe fn walk(view: id, content_view: id, strip: f64, depth: usize, out: &mut Vec<String>) {
        use objc::{msg_send, sel, sel_impl};
        let subviews: id = msg_send![view, subviews];
        if subviews == nil {
            return;
        }
        let count: usize = msg_send![subviews, count];
        for index in 0..count {
            let child: id = msg_send![subviews, objectAtIndex: index];
            if child == nil {
                continue;
            }
            let frame: cocoa::foundation::NSRect = msg_send![child, frame];
            let superview: id = msg_send![child, superview];
            let origin_in_content: cocoa::foundation::NSPoint = msg_send![
                content_view,
                convertPoint: frame.origin
                fromView: superview
            ];
            if origin_in_content.y < strip + 2.0 {
                let hidden: cocoa::base::BOOL = msg_send![child, isHidden];
                let alpha: f64 = msg_send![child, alphaValue];
                let cls: id = msg_send![child, class];
                let cls_name: id = msg_send![cls, className];
                let utf8: *const std::os::raw::c_char = msg_send![cls_name, UTF8String];
                let cls_name = if utf8.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(utf8)
                        .to_string_lossy()
                        .into_owned()
                };
                let layer: id = msg_send![child, layer];
                let layer_bg_alpha = if layer != nil {
                    let bg: *const std::ffi::c_void = msg_send![layer, backgroundColor];
                    if bg.is_null() {
                        -1.0
                    } else {
                        #[link(name = "CoreGraphics", kind = "framework")]
                        extern "C" {
                            fn CGColorGetAlpha(color: *const std::ffi::c_void) -> f64;
                        }
                        CGColorGetAlpha(bg)
                    }
                } else {
                    -2.0
                };
                out.push(format!(
                    "{}{} y={:.1} frame=({:.1},{:.1},{:.1},{:.1}) hidden={} alpha={:.4} layer_bg_alpha={:.4}",
                    "  ".repeat(depth),
                    cls_name,
                    origin_in_content.y,
                    frame.origin.x,
                    frame.origin.y,
                    frame.size.width,
                    frame.size.height,
                    hidden == YES,
                    alpha,
                    layer_bg_alpha,
                ));
            }
            walk(child, content_view, strip, depth + 1, out);
        }
    }

    let mut out = Vec::new();
    walk(content_view, content_view, strip, 0, &mut out);
    let has_shadow: cocoa::base::BOOL = msg_send![ns_window, hasShadow];
    tracing::info!(
        target: "script_kit::footer_popup",
        event = "glass_strip_view_dump",
        window_has_shadow = has_shadow == YES,
        views = %out.join(" | "),
        "Views intersecting the transparent footer strip"
    );
}

/// One-shot introspection: log NSGlassEffectView's declared properties so we
/// can discover any rim/style knobs Apple exposes (macOS 26 API surface is
/// underdocumented). Debug aid; logs once per process.
#[cfg(target_os = "macos")]
unsafe fn log_glass_effect_view_properties_once(glass_class: &objc::runtime::Class) {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        #[link(name = "objc")]
        extern "C" {
            fn class_copyPropertyList(
                cls: *const objc::runtime::Class,
                out_count: *mut u32,
            ) -> *mut *const std::ffi::c_void;
            fn property_getName(property: *const std::ffi::c_void) -> *const std::os::raw::c_char;
            fn property_getAttributes(
                property: *const std::ffi::c_void,
            ) -> *const std::os::raw::c_char;
            fn free(ptr: *mut std::ffi::c_void);
        }
        let mut count: u32 = 0;
        let list = class_copyPropertyList(glass_class as *const _, &mut count);
        if list.is_null() {
            return;
        }
        let mut names = Vec::new();
        for index in 0..count as usize {
            let property = *list.add(index);
            let name = property_getName(property);
            let attrs = property_getAttributes(property);
            if !name.is_null() {
                let attr_text = if attrs.is_null() {
                    String::new()
                } else {
                    std::ffi::CStr::from_ptr(attrs)
                        .to_string_lossy()
                        .into_owned()
                };
                names.push(format!(
                    "{}[{}]",
                    std::ffi::CStr::from_ptr(name).to_string_lossy(),
                    attr_text,
                ));
            }
        }
        free(list as *mut std::ffi::c_void);
        tracing::info!(
            target: "script_kit::footer_popup",
            event = "glass_effect_view_properties",
            properties = %names.join(","),
            "NSGlassEffectView declared properties"
        );
    });
}

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
    let backing_scale: f64 = msg_send![ns_window, backingScaleFactor];
    sync_native_view_tree_contents_scale(footer_view, backing_scale);
    let footer_is_glass = objc::runtime::Class::get("NSGlassEffectView")
        .map(|glass_class| {
            let is_glass: cocoa::base::BOOL = msg_send![footer_view, isKindOfClass: glass_class];
            is_glass == YES
        })
        .unwrap_or(false);

    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
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
    let left_dot_hex = config.left_info.as_ref().and_then(|info| {
        if matches!(info.dot_status, FooterDotStatus::Hidden) {
            None
        } else {
            Some(footer_dot_hex(
                info.dot_status,
                &theme,
                info.prefer_accent_for_active_states,
            ))
        }
    });
    let button_leading_dot_hexes = config
        .buttons
        .iter()
        .map(|button| {
            button.leading_dot.and_then(|status| {
                if matches!(status, FooterDotStatus::Hidden) {
                    None
                } else {
                    Some(footer_dot_hex(status, &theme, true))
                }
            })
        })
        .collect::<Vec<_>>();
    let native_visual_theme = resolve_native_footer_visual_theme(&theme);
    let signature = MainWindowFooterRefreshSignature {
        config: config.clone(),
        content_width_bits: content_bounds.size.width.to_bits(),
        dark: is_dark,
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
        left_dot_hex,
        #[cfg(target_os = "macos")]
        native_glass_signature: crate::platform::resolve_native_glass_style(
            &theme,
            crate::platform::NativeGlassSurfaceRole::FloatingCapsule,
        )
        .signature,
        native_visual_theme,
        main_menu_theme: crate::designs::current_main_menu_theme() as u8,
        gpui_overlay_owns_glyphs,
        button_leading_dot_hexes,
    };
    let (
        footer_geometry_changed,
        footer_content_changed,
        footer_visuals_changed,
        effect_theme_changed,
    ) = {
        let mut guard = MAIN_WINDOW_FOOTER_REFRESH_SIGNATURE
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if guard.as_ref() == Some(&signature) {
            update_main_window_footer_host_state(Some(config.surface), Some(config.surface), true);
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
        *guard = Some(signature);
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

    let divider_view = find_subview_by_identifier(footer_view, FOOTER_DIVIDER_ID);
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
            let divider_color =
                ns_color_from_rgba(footer_divider_rgba(&theme, chrome.divider_rgba));
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

    let hints_view = find_subview_by_identifier(footer_view, FOOTER_HINTS_ID);
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
                native_footer_lanes = layout_footer_hints(hints_view, text_color, &[], &theme);
            } else {
                native_footer_lanes =
                    layout_footer_hints(hints_view, text_color, &config.buttons, &theme);
            }
        } else if footer_visuals_changed {
            recolor_footer_hint_subviews_with_visual_theme(hints_view, &theme, native_visual_theme);
            native_footer_lanes = measure_native_footer_lanes(hints_view, &config.buttons);
        } else {
            native_footer_lanes = measure_native_footer_lanes(hints_view, &config.buttons);
        }
        if footer_content_changed || footer_visuals_changed || effect_theme_changed {
            restyle_footer_glass_capsules(hints_view, &theme);
            refresh_footer_button_visual_states_with_theme(hints_view, native_visual_theme);
        }
    }

    // Left info (streaming dot + model name)
    let left_info_view = find_subview_by_identifier(footer_view, FOOTER_LEFT_INFO_ID);
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
            restyle_footer_glass_capsules(left_info_view, &theme);
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

    true
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
/// The native footer view that should participate in the main window's entry
/// choreography, or `nil` when no same-host footer is installed.
///
/// The floating footer buttons live in the main NSWindow but OUTSIDE both
/// surfaces the entry animates: the glass backdrop (which is laid out to stop
/// above the 8pt gutter) and the GPUI content roots (whose collection loop
/// deliberately skips `NSGlassEffectContainerView`, which is exactly what the
/// glass-mode footer is hosted in). The result is buttons that sit at full
/// alpha and full sharpness from the first frame while everything above them
/// materializes — user report 2026-07-27: "why aren't the floating buttons
/// fading/blurring too?".
///
/// Prefers the glass container (glass mode) and falls back to the plain footer
/// host, so the caller animates the outermost footer surface in either mode.
#[cfg(target_os = "macos")]
pub(crate) unsafe fn main_window_footer_entry_target(ns_window: id) -> id {
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

#[cfg(target_os = "macos")]
unsafe fn find_subview_by_identifier(parent: id, identifier: &str) -> id {
    use objc::{msg_send, sel, sel_impl};

    let ns_identifier = ns_string(identifier);
    if parent == nil || ns_identifier == nil {
        return nil;
    }

    let subviews: id = msg_send![parent, subviews];
    if subviews == nil {
        return nil;
    }

    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let view: id = msg_send![subviews, objectAtIndex: index];
        if view == nil {
            continue;
        }
        let view_identifier: id = msg_send![view, identifier];
        if view_identifier != nil {
            let matches: cocoa::base::BOOL =
                msg_send![view_identifier, isEqualToString: ns_identifier];
            if matches == YES {
                return view;
            }
        }

        // Glass foregrounds live below NSGlassEffectView.contentView. Search
        // the actual hierarchy instead of assuming every identified node is a
        // direct child of the footer host.
        let nested = find_subview_by_identifier(view, identifier);
        if nested != nil {
            return nested;
        }
    }

    nil
}

#[cfg(target_os = "macos")]
fn footer_height() -> f64 {
    crate::components::footer_chrome::current_main_menu_footer_height() as f64
}

#[cfg(target_os = "macos")]
fn footer_hints_frame(width: f64) -> cocoa::foundation::NSRect {
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(FOOTER_HINT_SIDE_INSET, 0.0),
        cocoa::foundation::NSSize::new(width - (FOOTER_HINT_SIDE_INSET * 2.0), footer_height()),
    )
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct NativeFooterLaneLayout {
    hints_width: f64,
    left_pinned_end_x: f64,
    trailing_start_x: f64,
    left_info_x: f64,
    left_info_width: f64,
    trailing_overflow: bool,
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterLeftInfoDegradation {
    Full,
    TruncatedLabels,
    CwdAffordanceOnly,
    PrimaryOnly,
    PrimaryAffordanceOnly,
    Hidden,
}

#[cfg(target_os = "macos")]
impl FooterLeftInfoDegradation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::TruncatedLabels => "truncatedLabels",
            Self::CwdAffordanceOnly => "cwdAffordanceOnly",
            Self::PrimaryOnly => "primaryOnly",
            Self::PrimaryAffordanceOnly => "primaryAffordanceOnly",
            Self::Hidden => "hidden",
        }
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq)]
struct FooterLeftInfoAllocation {
    degradation: FooterLeftInfoDegradation,
    available_width: f64,
    cwd_label_width: f64,
    primary_label_width: f64,
}

#[cfg(target_os = "macos")]
static LAST_FOOTER_LEFT_ALLOCATION: OnceLock<
    Mutex<Option<crate::protocol::AppKitFooterLeftAllocation>>,
> = OnceLock::new();

#[cfg(target_os = "macos")]
fn record_footer_left_allocation(allocation: FooterLeftInfoAllocation) {
    let snapshot = crate::protocol::AppKitFooterLeftAllocation {
        degradation: allocation.degradation.as_str().to_string(),
        available_width: allocation.available_width,
        cwd_label_width: allocation.cwd_label_width,
        primary_label_width: allocation.primary_label_width,
    };
    if let Ok(mut slot) = LAST_FOOTER_LEFT_ALLOCATION
        .get_or_init(|| Mutex::new(None))
        .lock()
    {
        *slot = Some(snapshot);
    }
}

#[cfg(target_os = "macos")]
fn footer_left_allocation_snapshot() -> Option<crate::protocol::AppKitFooterLeftAllocation> {
    LAST_FOOTER_LEFT_ALLOCATION
        .get_or_init(|| Mutex::new(None))
        .lock()
        .ok()
        .and_then(|slot| slot.clone())
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FooterLeftInfoMeasurements {
    cwd_fixed_width: f64,
    cwd_label_width: f64,
    primary_fixed_width: f64,
    primary_label_width: f64,
    has_cwd: bool,
    primary_visible_without_label: bool,
}

#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_CAPSULE_PAD_X: f64 = 8.0;

#[cfg(target_os = "macos")]
fn resolve_native_footer_lanes(
    hints_width: f64,
    left_pinned_end_x: f64,
    trailing_start_x: f64,
) -> NativeFooterLaneLayout {
    let gap = f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX);
    let left_pinned_end_x = left_pinned_end_x.clamp(0.0, hints_width.max(0.0));
    let trailing_start_x = trailing_start_x.clamp(0.0, hints_width.max(0.0));
    let left_info_x = left_pinned_end_x + gap + FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let left_info_end_x = trailing_start_x - gap - FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let trailing_overflow = trailing_start_x < left_pinned_end_x + gap;
    NativeFooterLaneLayout {
        hints_width,
        left_pinned_end_x,
        trailing_start_x,
        left_info_x,
        left_info_width: if trailing_overflow {
            0.0
        } else {
            (left_info_end_x - left_info_x).max(0.0)
        },
        trailing_overflow,
    }
}

#[cfg(target_os = "macos")]
fn resolve_footer_left_info_allocation(
    available_width: f64,
    measured: FooterLeftInfoMeasurements,
) -> FooterLeftInfoAllocation {
    let available_width = available_width.max(0.0);
    let cwd_min = measured.cwd_label_width.min(f64::from(
        crate::components::footer_chrome::FOOTER_CWD_LABEL_MIN_WIDTH_PX,
    ));
    let primary_min = measured.primary_label_width.min(f64::from(
        crate::components::footer_chrome::FOOTER_PRIMARY_LABEL_MIN_WIDTH_PX,
    ));
    let fixed = measured.cwd_fixed_width + measured.primary_fixed_width;
    let full_required = fixed + measured.cwd_label_width + measured.primary_label_width;
    let truncated_required = fixed + cwd_min + primary_min;
    let cwd_affordance_required = fixed + primary_min;
    let primary_only_required = measured.primary_fixed_width + primary_min;

    let (degradation, cwd_label_width, primary_label_width) = if full_required <= 0.0 {
        (FooterLeftInfoDegradation::Hidden, 0.0, 0.0)
    } else if available_width >= full_required {
        (
            FooterLeftInfoDegradation::Full,
            measured.cwd_label_width,
            measured.primary_label_width,
        )
    } else if measured.has_cwd && available_width >= truncated_required {
        let flexible = (available_width - fixed - cwd_min - primary_min).max(0.0);
        let cwd_extra = (measured.cwd_label_width - cwd_min).max(0.0);
        let primary_extra = (measured.primary_label_width - primary_min).max(0.0);
        let total_extra = cwd_extra + primary_extra;
        let cwd_share = if total_extra > 0.0 {
            flexible * cwd_extra / total_extra
        } else {
            0.0
        };
        let cwd_width = (cwd_min + cwd_share).min(measured.cwd_label_width);
        (
            FooterLeftInfoDegradation::TruncatedLabels,
            cwd_width,
            (available_width - fixed - cwd_width)
                .max(primary_min)
                .min(measured.primary_label_width),
        )
    } else if measured.has_cwd && available_width >= cwd_affordance_required {
        (
            FooterLeftInfoDegradation::CwdAffordanceOnly,
            0.0,
            (available_width - fixed)
                .max(0.0)
                .min(measured.primary_label_width),
        )
    } else if available_width >= primary_only_required {
        (
            FooterLeftInfoDegradation::PrimaryOnly,
            0.0,
            (available_width - measured.primary_fixed_width)
                .max(0.0)
                .min(measured.primary_label_width),
        )
    } else if measured.primary_visible_without_label
        && available_width >= measured.primary_fixed_width
    {
        (FooterLeftInfoDegradation::PrimaryAffordanceOnly, 0.0, 0.0)
    } else {
        (FooterLeftInfoDegradation::Hidden, 0.0, 0.0)
    };
    FooterLeftInfoAllocation {
        degradation,
        available_width,
        cwd_label_width,
        primary_label_width,
    }
}

#[cfg(target_os = "macos")]
fn footer_left_info_frame(layout: NativeFooterLaneLayout) -> cocoa::foundation::NSRect {
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(FOOTER_HINT_SIDE_INSET + layout.left_info_x, 0.0),
        cocoa::foundation::NSSize::new(layout.left_info_width, footer_height()),
    )
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

/// Return the owning foreground view for left-info visuals. Glass foregrounds
/// must be mounted through `NSGlassEffectView.contentView`; siblings are
/// treated as refracted background and become washed out. The returned
/// offsets preserve the left-info coordinate system while the capsule extends
/// beyond it by its shared horizontal padding.
#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_info_visual_parent(left_info_view: id, height: f64) -> (id, f64, f64) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if !glass_scroll_bands_active() {
        return (left_info_view, 0.0, 0.0);
    }
    let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") else {
        return (left_info_view, 0.0, 0.0);
    };

    const PAD_X: f64 = FOOTER_LEFT_INFO_CAPSULE_PAD_X;
    let item_height =
        crate::components::footer_chrome::footer_button_height(footer_height() as f32) as f64;
    let capsule_y = ((height - item_height) / 2.0).round();
    let provisional_frame = NSRect::new(
        NSPoint::new(-PAD_X, capsule_y),
        NSSize::new(PAD_X * 2.0 + 1.0, item_height),
    );
    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_CAPSULE_ID);
    let capsule = if existing != nil {
        existing
    } else {
        let capsule: id = msg_send![glass_class, alloc];
        let capsule: id = msg_send![capsule, initWithFrame: provisional_frame];
        if capsule == nil {
            return (left_info_view, 0.0, 0.0);
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_ID);
        if identifier != nil {
            let _: () = msg_send![capsule, setIdentifier: identifier];
        }
        let _: () = msg_send![
            capsule,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
        let _: () = msg_send![
            left_info_view,
            addSubview: capsule
            positioned: -1isize
            relativeTo: cocoa::base::nil
        ];
        capsule
    };

    let existing_content = find_subview_by_identifier(capsule, FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID);
    let content = if existing_content != nil {
        existing_content
    } else {
        let content: id = msg_send![class!(NSView), alloc];
        let content: id = msg_send![
            content,
            initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PAD_X * 2.0 + 1.0, item_height)
            )
        ];
        if content == nil {
            return (left_info_view, 0.0, 0.0);
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_CONTENT_ID);
        if identifier != nil {
            let _: () = msg_send![content, setIdentifier: identifier];
        }
        let _: () = msg_send![content, setAutoresizingMask: 18u64];
        let _: () = msg_send![capsule, setContentView: content];
        content
    };
    let _: () = msg_send![content, setWantsLayer: YES];
    let content_layer: id = msg_send![content, layer];
    if content_layer != nil {
        let _: () = msg_send![
            content_layer,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        let _: () = msg_send![content_layer, setMasksToBounds: YES];
    }
    let state_view = find_subview_by_identifier(content, FOOTER_LEFT_INFO_STATE_LAYER_ID);
    let state_view = if state_view != nil {
        state_view
    } else {
        let state_view: id = msg_send![class!(NSView), alloc];
        let state_view: id = msg_send![
            state_view,
            initWithFrame: NSRect::new(
                NSPoint::new(0.0, 0.0),
                NSSize::new(PAD_X * 2.0 + 1.0, item_height)
            )
        ];
        if state_view != nil {
            let identifier = ns_string(FOOTER_LEFT_INFO_STATE_LAYER_ID);
            if identifier != nil {
                let _: () = msg_send![state_view, setIdentifier: identifier];
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
                let _: () = msg_send![state_layer, setMasksToBounds: YES];
            }
            let _: () = msg_send![
                content,
                addSubview: state_view
                positioned: -1isize
                relativeTo: cocoa::base::nil
            ];
        }
        state_view
    };
    if state_view != nil {
        let content_bounds: NSRect = msg_send![content, bounds];
        let _: () = msg_send![state_view, setFrame: content_bounds];
    }
    style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
    let _: () = msg_send![capsule, setHidden: NO];
    (content, PAD_X, -capsule_y)
}

/// Floating-chrome mode: size the left-info capsule to its laid-out content.
/// Hidden when there is no content or float mode is off.
#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_info_capsule(left_info_view: id, content_width: f64, height: f64) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{msg_send, sel, sel_impl};

    // Safe in-window: the footer container is bounded to the 32pt footer band
    // and separated from the main backdrop by the transparent gutter.
    const PAD_X: f64 = 8.0;
    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_CAPSULE_ID);
    let active = glass_scroll_bands_active() && content_width > 1.0;
    if !active {
        if existing != nil {
            let _: () = msg_send![existing, setHidden: YES];
        }
        return;
    }
    let item_height =
        crate::components::footer_chrome::footer_button_height(footer_height() as f32) as f64;
    let frame = NSRect::new(
        NSPoint::new(-PAD_X, ((height - item_height) / 2.0).round()),
        NSSize::new(content_width + PAD_X * 2.0, item_height),
    );
    let capsule = if existing != nil {
        existing
    } else {
        let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") else {
            return;
        };
        let capsule: id = msg_send![glass_class, alloc];
        let capsule: id = msg_send![capsule, initWithFrame: frame];
        if capsule == nil {
            return;
        }
        let identifier = ns_string(FOOTER_LEFT_INFO_CAPSULE_ID);
        if identifier != nil {
            let _: () = msg_send![capsule, setIdentifier: identifier];
        }
        let _: () = msg_send![
            capsule,
            setCornerRadius:
                crate::components::footer_chrome::FOOTER_ACTION_BUTTON_RADIUS_PX as f64
        ];
        style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
        let _: () = msg_send![
            left_info_view,
            addSubview: capsule
            positioned: -1isize
            relativeTo: cocoa::base::nil
        ];
        capsule
    };
    let _: () = msg_send![capsule, setHidden: NO];
    let _: () = msg_send![capsule, setFrame: frame];
    // Resizing/attaching an NSGlassEffectView may replace its private
    // foreground backing. Reapply the shared policy after the final frame so
    // the left capsule keeps the same veil and rim as trailing capsules.
    style_float_footer_capsule(capsule, &crate::theme::get_cached_theme());
}

#[cfg(target_os = "macos")]
unsafe fn remove_identified_subview(parent: id, identifier: &str) {
    use objc::{msg_send, sel, sel_impl};

    // Remove every match. Older refreshes could leave duplicate nested nodes;
    // one teardown must restore the closed-world identifier inventory.
    loop {
        let view = find_subview_by_identifier(parent, identifier);
        if view == nil {
            break;
        }
        let layer: id = msg_send![view, layer];
        if layer != nil {
            remove_active_dot_pulse_animation(layer);
        }
        let _: () = msg_send![view, removeFromSuperview];
    }
}

#[cfg(target_os = "macos")]
unsafe fn set_footer_view_identifier(view: id, identifier: &str) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    let identifier = ns_string(identifier);
    if identifier == nil {
        return;
    }
    let _: () = msg_send![view, setIdentifier: identifier];
    let supports_ax_identifier: cocoa::base::BOOL =
        msg_send![view, respondsToSelector: sel!(setAccessibilityIdentifier:)];
    if supports_ax_identifier == YES {
        let _: () = msg_send![view, setAccessibilityIdentifier: identifier];
    }
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_status_dot_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_STATUS_DOT_ID);
    if existing != nil {
        return existing;
    }

    let dot_view: id = msg_send![class!(NSView), alloc];
    let dot_view: id = msg_send![
        dot_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
        )
    ];
    if dot_view == nil {
        return nil;
    }

    let identifier = ns_string(FOOTER_STATUS_DOT_ID);
    if identifier != nil {
        let _: () = msg_send![dot_view, setIdentifier: identifier];
    }

    let layer: id = msg_send![class!(CALayer), layer];
    if layer != nil {
        let _: () = msg_send![layer, setMasksToBounds: NO];
        let _: () = msg_send![layer, setCornerRadius: FOOTER_STREAMING_DOT_SIZE / 2.0_f64];
        let _: () = msg_send![dot_view, setLayer: layer];
    }
    let _: () = msg_send![dot_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: dot_view];
    dot_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_model_label(
    left_info_view: id,
    visual_parent: id,
    text: &str,
    text_color: id,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: crate::components::footer_chrome::current_main_menu_footer_metrics().label_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let label = find_subview_by_identifier(left_info_view, FOOTER_MODEL_LABEL_ID);
    if label != nil {
        let string_value = ns_string(text);
        if string_value != nil {
            let _: () = msg_send![label, setStringValue: string_value];
        }
        if font != nil {
            let _: () = msg_send![label, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![label, setTextColor: text_color];
        }
        let _: () = msg_send![label, setAlignment: FOOTER_HINT_TEXT_ALIGN_LEFT];
        let _: () = msg_send![label, sizeToFit];
        return label;
    }

    let label = make_footer_hint_text_field(text, font, text_color, FOOTER_HINT_TEXT_ALIGN_LEFT);
    if label != nil {
        let identifier = ns_string(FOOTER_MODEL_LABEL_ID);
        if identifier != nil {
            let _: () = msg_send![label, setIdentifier: identifier];
        }
        let _: () = msg_send![visual_parent, addSubview: label];
    }
    label
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_left_profile_icon_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_LEFT_PROFILE_ICON_ID);
    if existing != nil {
        return existing;
    }

    let image_view: id = msg_send![class!(NSImageView), alloc];
    let image_view: id = msg_send![
        image_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
        )
    ];
    if image_view == nil {
        return nil;
    }
    let identifier = ns_string(FOOTER_LEFT_PROFILE_ICON_ID);
    if identifier != nil {
        let _: () = msg_send![image_view, setIdentifier: identifier];
    }
    let _: () = msg_send![image_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: image_view];
    image_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_cwd_chip_icon_view(left_info_view: id, visual_parent: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let existing = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_ICON_ID);
    if existing != nil {
        return existing;
    }

    let image_view: id = msg_send![class!(NSImageView), alloc];
    let image_view: id = msg_send![
        image_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_LEFT_PROFILE_ICON_SIZE, FOOTER_LEFT_PROFILE_ICON_SIZE),
        )
    ];
    if image_view == nil {
        return nil;
    }
    let identifier = ns_string(FOOTER_CWD_CHIP_ICON_ID);
    if identifier != nil {
        let _: () = msg_send![image_view, setIdentifier: identifier];
    }
    let _: () = msg_send![image_view, setWantsLayer: YES];
    let _: () = msg_send![visual_parent, addSubview: image_view];
    image_view
}

#[cfg(target_os = "macos")]
unsafe fn ensure_footer_cwd_chip_label(
    left_info_view: id,
    visual_parent: id,
    text: &str,
    text_color: id,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: crate::components::footer_chrome::current_main_menu_footer_metrics().label_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let label = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_LABEL_ID);
    if label != nil {
        let string_value = ns_string(text);
        if string_value != nil {
            let _: () = msg_send![label, setStringValue: string_value];
        }
        if font != nil {
            let _: () = msg_send![label, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![label, setTextColor: text_color];
        }
        let _: () = msg_send![label, setAlignment: FOOTER_HINT_TEXT_ALIGN_LEFT];
        let _: () = msg_send![label, sizeToFit];
        return label;
    }

    let label = make_footer_hint_text_field(text, font, text_color, FOOTER_HINT_TEXT_ALIGN_LEFT);
    if label != nil {
        let identifier = ns_string(FOOTER_CWD_CHIP_LABEL_ID);
        if identifier != nil {
            let _: () = msg_send![label, setIdentifier: identifier];
        }
        let _: () = msg_send![visual_parent, addSubview: label];
    }
    label
}

#[cfg(target_os = "macos")]
#[allow(clippy::too_many_arguments)]
unsafe fn layout_footer_left_keycap(
    search_root: id,
    visual_parent: id,
    keycap_id: &str,
    glyph_id: &str,
    glyph: &str,
    x: f64,
    host_height: f64,
    visual_offset_x: f64,
    visual_offset_y: f64,
    text_color: id,
) -> f64 {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    let keycap_height = metrics.keycap_height as f64;
    let shortcut_tokens = crate::components::footer_chrome::split_footer_shortcut(glyph);
    if shortcut_tokens.is_empty() {
        remove_identified_subview(search_root, keycap_id);
        return 0.0;
    }
    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: metrics.keycap_font_size as f64
        weight: crate::components::footer_chrome::current_main_menu_footer_appkit_font_weight()
    ];
    let mut keycap = find_subview_by_identifier(search_root, keycap_id);
    if keycap == nil {
        keycap = msg_send![class!(NSView), alloc];
        keycap = msg_send![keycap, initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(keycap_height, keycap_height))];
        if keycap == nil {
            return 0.0;
        }
        set_footer_view_identifier(keycap, keycap_id);
        let _: () = msg_send![keycap, setWantsLayer: YES];
        let _: () = msg_send![visual_parent, addSubview: keycap];
    }

    // The left lane used to treat a shortcut such as ⌘↵ as one text run in
    // one wide chip. Trailing buttons split that shortcut into one keycap per
    // token, so the left return glyph never received the calibrated ↵ offset.
    // Keep the outer view as a transparent run container, then build every
    // token with the same tokenizer, gap, padding, and optical correction
    // helpers as the trailing button path.
    remove_identified_subview(keycap, glyph_id);
    let keycap_layer: id = msg_send![keycap, layer];
    if keycap_layer != nil {
        let _: () = msg_send![keycap_layer, setCornerRadius: 0.0_f64];
        let _: () = msg_send![keycap_layer, setBorderWidth: 0.0_f64];
    }

    let theme = crate::theme::get_cached_theme();
    let border = ns_color_from_hex_with_alpha(
        footer_keycap_hex(&theme),
        footer_keycap_border_alpha(&theme, false),
    );
    let key_gap = metrics.content_gap as f64;
    let mut keycap_run_width = 0.0_f64;

    for (index, token) in shortcut_tokens.iter().enumerate() {
        let token_keycap_id = format!("{keycap_id}-{index}");
        let token_glyph_id = format!("{glyph_id}-{index}");
        let mut token_keycap = find_subview_by_identifier(keycap, &token_keycap_id);
        if token_keycap == nil {
            token_keycap = msg_send![class!(NSView), alloc];
            token_keycap = msg_send![
                token_keycap,
                initWithFrame: NSRect::new(
                    NSPoint::new(0.0, 0.0),
                    NSSize::new(keycap_height, keycap_height)
                )
            ];
            if token_keycap == nil {
                continue;
            }
            set_footer_view_identifier(token_keycap, &token_keycap_id);
            let _: () = msg_send![token_keycap, setWantsLayer: YES];
            let _: () = msg_send![keycap, addSubview: token_keycap];
        }

        let mut glyph_view = find_subview_by_identifier(token_keycap, &token_glyph_id);
        if glyph_view == nil {
            glyph_view = make_footer_hint_text_field(token, font, text_color, 1usize);
            if glyph_view == nil {
                continue;
            }
            set_footer_view_identifier(glyph_view, &token_glyph_id);
            let _: () = msg_send![token_keycap, addSubview: glyph_view];
        }
        let value = ns_string(token);
        if value != nil {
            let _: () = msg_send![glyph_view, setStringValue: value];
        }
        if font != nil {
            let _: () = msg_send![glyph_view, setFont: font];
        }
        if text_color != nil {
            let _: () = msg_send![glyph_view, setTextColor: text_color];
        }
        let _: () = msg_send![glyph_view, sizeToFit];
        let glyph_size: NSSize = msg_send![glyph_view, fittingSize];
        let keycap_padding_x =
            crate::components::footer_chrome::footer_keycap_padding_x_for_token(token, &metrics)
                as f64;
        let token_keycap_width = (glyph_size.width + keycap_padding_x * 2.0).max(keycap_height);
        let glyph_x = crate::components::footer_chrome::footer_appkit_glyph_x(
            token,
            token_keycap_width,
            glyph_size.width,
        );
        let glyph_y = metrics.keycap_padding_y as f64
            + crate::components::footer_chrome::footer_appkit_glyph_y(
                token,
                (keycap_height - metrics.keycap_padding_y as f64 * 2.0).max(0.0),
                glyph_size.height,
            );
        let _: () = msg_send![
            glyph_view,
            setFrame: NSRect::new(NSPoint::new(glyph_x, glyph_y), glyph_size)
        ];

        let token_layer: id = msg_send![token_keycap, layer];
        if token_layer != nil {
            let _: () = msg_send![
                token_layer,
                setCornerRadius: metrics.keycap_radius as f64
            ];
            let _: () = msg_send![token_layer, setBorderWidth: 1.0_f64];
            if border != nil {
                let cg: id = msg_send![border, CGColor];
                if cg != nil {
                    let _: () = msg_send![token_layer, setBorderColor: cg];
                }
            }
        }
        let _: () = msg_send![
            token_keycap,
            setFrame: NSRect::new(
                NSPoint::new(keycap_run_width, 0.0),
                NSSize::new(token_keycap_width, keycap_height)
            )
        ];
        let _: () = msg_send![token_keycap, setHidden: NO];
        keycap_run_width += token_keycap_width;
        if index + 1 < shortcut_tokens.len() {
            keycap_run_width += key_gap;
        }
    }

    // A tip can rotate from a longer shortcut to a shorter one while the
    // native footer host is reused. Remove any no-longer-owned token views.
    for index in shortcut_tokens.len()..16 {
        let stale_id = format!("{keycap_id}-{index}");
        remove_identified_subview(keycap, &stale_id);
    }
    let keycap_y = ((host_height - keycap_height) / 2.0).round();
    let _: () = msg_send![keycap, setFrame: NSRect::new(
        NSPoint::new(x + visual_offset_x, keycap_y + visual_offset_y),
        NSSize::new(keycap_run_width, keycap_height),
    )];
    let _: () = msg_send![keycap, setHidden: NO];
    keycap_run_width
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct NativeFooterLeftHitTargetFlags {
    selected: bool,
    enabled: bool,
}

fn native_footer_left_hit_target_flags(
    selected: bool,
    enabled: bool,
) -> NativeFooterLeftHitTargetFlags {
    NativeFooterLeftHitTargetFlags { selected, enabled }
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_cwd_chip_hit_target(
    left_info_view: id,
    frame: cocoa::foundation::NSRect,
    tooltip: Option<&str>,
    selected: bool,
    enabled: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
        remove_identified_subview(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        return;
    }

    let mut button = find_subview_by_identifier(left_info_view, FOOTER_CWD_CHIP_HIT_TARGET_ID);
    if button == nil {
        button = msg_send![footer_button_class(), alloc];
        button = msg_send![button, initWithFrame: frame];
        if button == nil {
            return;
        }
        set_footer_view_identifier(button, FOOTER_CWD_CHIP_HIT_TARGET_ID);
        let _: () = msg_send![button, setBordered: NO];
        let _: () = msg_send![button, setBezelStyle: 0usize];
        let _: () = msg_send![button, setButtonType: 0usize];
        let _: () = msg_send![button, setTransparent: YES];
        if let Some(object) = button.as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
            object.set_ivar::<usize>("_stateView", 0);
            object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
        }
        let _: () = msg_send![left_info_view, addSubview: button];
    }
    let _: () = msg_send![button, setFrame: frame];
    let _: () = msg_send![button, setEnabled: if enabled { YES } else { NO }];
    let _: () = msg_send![button, setTarget: footer_action_target()];
    let action_selector = footer_action_selector(FooterAction::Cwd);
    let previous_action: objc::runtime::Sel = msg_send![button, action];
    let _: () = msg_send![button, setAction: action_selector];
    let flags = native_footer_left_hit_target_flags(selected, enabled);
    if let Some(object) = button.as_mut() {
        object.set_ivar::<cocoa::base::BOOL>("_isActionsButton", NO);
        object.set_ivar::<cocoa::base::BOOL>("_selected", if flags.selected { YES } else { NO });
        object.set_ivar::<cocoa::base::BOOL>("_enabled", if flags.enabled { YES } else { NO });
        if previous_action != action_selector {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        object.set_ivar::<usize>(
            "_stateView",
            find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_STATE_LAYER_ID) as usize,
        );
        object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
    }
    let tooltip = tooltip.map(ns_string).unwrap_or(nil);
    let _: () = msg_send![button, setToolTip: tooltip];
    refresh_footer_button_visual_states(left_info_view);
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_left_info_hit_target(
    left_info_view: id,
    action: Option<FooterAction>,
    frame: cocoa::foundation::NSRect,
    selected: bool,
    enabled: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    let Some(action) = action else {
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        return;
    };
    if frame.size.width <= 0.0 || frame.size.height <= 0.0 {
        remove_identified_subview(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        return;
    }

    let mut button = find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_HIT_TARGET_ID);
    if button == nil {
        button = msg_send![footer_button_class(), alloc];
        button = msg_send![button, initWithFrame: frame];
        if button == nil {
            return;
        }
        set_footer_view_identifier(button, FOOTER_LEFT_INFO_HIT_TARGET_ID);
        let _: () = msg_send![button, setBordered: NO];
        let _: () = msg_send![button, setBezelStyle: 0usize];
        let _: () = msg_send![button, setButtonType: 0usize];
        let _: () = msg_send![button, setTransparent: YES];
        if let Some(object) = button.as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
            object.set_ivar::<usize>("_stateView", 0);
            object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
        }
        let _: () = msg_send![left_info_view, addSubview: button];
    }
    let _: () = msg_send![button, setFrame: frame];
    let _: () = msg_send![button, setEnabled: if enabled { YES } else { NO }];
    let _: () = msg_send![button, setTarget: footer_action_target()];
    let action_selector = footer_action_selector(action);
    let previous_action: objc::runtime::Sel = msg_send![button, action];
    let _: () = msg_send![button, setAction: action_selector];
    let flags = native_footer_left_hit_target_flags(selected, enabled);
    if let Some(object) = button.as_mut() {
        object.set_ivar::<cocoa::base::BOOL>("_isActionsButton", NO);
        object.set_ivar::<cocoa::base::BOOL>("_selected", if flags.selected { YES } else { NO });
        object.set_ivar::<cocoa::base::BOOL>("_enabled", if flags.enabled { YES } else { NO });
        if previous_action != action_selector {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        object.set_ivar::<usize>(
            "_stateView",
            find_subview_by_identifier(left_info_view, FOOTER_LEFT_INFO_STATE_LAYER_ID) as usize,
        );
        object.set_ivar::<usize>("_visualRoot", left_info_view as usize);
    }
    refresh_footer_button_visual_states(left_info_view);
}

#[cfg(target_os = "macos")]
unsafe fn update_footer_dot_layer(layer: id, info: &FooterLeftInfo) {
    update_footer_dot_layer_for_status(
        layer,
        info.dot_status,
        info.prefer_accent_for_active_states,
    );
}

/// Status-driven dot layer update, shared by the legacy left-info marker and the
/// per-button leading dot (Agent·Model chip). `Hidden` collapses the dot to fully
/// transparent + no pulse so a reserved lane can stay width-stable without
/// showing anything.
#[cfg(target_os = "macos")]
unsafe fn update_footer_dot_layer_for_status(
    layer: id,
    dot_status: FooterDotStatus,
    prefer_accent_for_active_states: bool,
) {
    use objc::{msg_send, sel, sel_impl};

    let _: () = msg_send![layer, setCornerRadius: FOOTER_STREAMING_DOT_SIZE / 2.0_f64];

    if matches!(dot_status, FooterDotStatus::Hidden) {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 0.0_f32];
        return;
    }

    let theme = crate::theme::get_cached_theme();
    let dot_hex = footer_dot_hex(dot_status, &theme, prefer_accent_for_active_states);

    let dot_ns = ns_color_from_hex_with_alpha(dot_hex, 1.0);
    if dot_ns != nil {
        let cg: id = msg_send![dot_ns, CGColor];
        if cg != nil {
            let _: () = msg_send![layer, setBackgroundColor: cg];
        }
    }

    let should_pulse = matches!(
        dot_status,
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
    );
    if should_pulse {
        ensure_active_dot_pulse_animation(layer);
    } else {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 1.0_f32];
    }
}

#[cfg(target_os = "macos")]
unsafe fn update_footer_icon_layer(layer: id, info: &FooterLeftInfo) {
    use objc::{msg_send, sel, sel_impl};

    let should_pulse = matches!(
        info.dot_status,
        FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
    );
    if should_pulse {
        ensure_active_dot_pulse_animation(layer);
    } else {
        remove_active_dot_pulse_animation(layer);
        let _: () = msg_send![layer, setOpacity: 1.0_f32];
    }
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
unsafe fn layer_has_animation(layer: id, key: &str) -> bool {
    use objc::{msg_send, sel, sel_impl};

    let key = ns_string(key);
    if key == nil {
        return false;
    }
    let animation: id = msg_send![layer, animationForKey: key];
    animation != nil
}

#[cfg(target_os = "macos")]
unsafe fn ensure_active_dot_pulse_animation(layer: id) {
    if layer == nil {
        return;
    }
    let has_opacity = layer_has_animation(layer, "pulseOpacity");
    if has_opacity {
        remove_active_dot_scale_animation(layer);
        return;
    }
    remove_active_dot_pulse_animation(layer);
    add_active_dot_pulse_animation(layer);
}

#[cfg(target_os = "macos")]
unsafe fn remove_active_dot_pulse_animation(layer: id) {
    use objc::{msg_send, sel, sel_impl};

    let opacity_key = ns_string("pulseOpacity");
    if opacity_key != nil {
        let _: () = msg_send![layer, removeAnimationForKey: opacity_key];
    }
    remove_active_dot_scale_animation(layer);
}

#[cfg(target_os = "macos")]
unsafe fn remove_active_dot_scale_animation(layer: id) {
    use objc::{msg_send, sel, sel_impl};

    let scale_key = ns_string("pulseScale");
    if scale_key != nil {
        let _: () = msg_send![layer, removeAnimationForKey: scale_key];
    }
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews(view: id, theme: &crate::theme::Theme) {
    recolor_footer_hint_subviews_with_visual_theme(
        view,
        theme,
        resolve_native_footer_visual_theme(theme),
    );
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews_with_visual_theme(
    view: id,
    _theme: &crate::theme::Theme,
    visual_theme: NativeFooterVisualTheme,
) {
    if view == nil {
        return;
    }

    let text_color = ns_color_from_rgba(visual_theme.row_palette.rest.primary_foreground_rgba);
    let border_color = ns_color_from_hex_with_alpha(
        visual_theme.keycap_hex,
        visual_theme.border_alpha(crate::theme::MainMenuRowState::Rest) as f64,
    );

    recolor_footer_hint_subviews_with_colors(view, text_color, border_color);
    refresh_footer_button_visual_states_with_theme(view, visual_theme);
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_states(view: id) {
    let theme = crate::theme::get_cached_theme();
    refresh_footer_button_visual_states_with_theme(
        view,
        resolve_native_footer_visual_theme(&theme),
    );
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_states_with_theme(
    view: id,
    visual_theme: NativeFooterVisualTheme,
) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let mut states_by_visual_root = std::collections::HashMap::new();
    collect_footer_button_visual_states(view, &mut states_by_visual_root);
    for (_visual_root, (button, state)) in states_by_visual_root {
        apply_footer_button_visual_state_with_theme(button, state, visual_theme);
    }
}

#[cfg(target_os = "macos")]
unsafe fn collect_footer_button_visual_states(
    view: id,
    states_by_visual_root: &mut std::collections::HashMap<
        usize,
        (id, crate::theme::MainMenuRowState),
    >,
) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    let is_footer_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: footer_button_class()];
    if is_footer_button == YES {
        if let Some(object) = view.as_ref() {
            let selected = *object.get_ivar::<cocoa::base::BOOL>("_selected") == YES;
            let hovered = *object.get_ivar::<cocoa::base::BOOL>("_hovered") == YES;
            let is_actions = *object.get_ivar::<cocoa::base::BOOL>("_isActionsButton") == YES;
            let state = resolved_native_footer_button_state(
                selected,
                hovered,
                crate::actions::is_actions_window_open(),
                is_actions,
            );
            let visual_root = footer_button_visual_root(view) as usize;
            match states_by_visual_root.get(&visual_root) {
                Some((_, existing_state))
                    if native_footer_visual_root_state(Some(*existing_state), state)
                        == *existing_state => {}
                _ => {
                    states_by_visual_root.insert(visual_root, (view, state));
                }
            }
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        collect_footer_button_visual_states(child, states_by_visual_root);
    }
}

fn native_footer_state_rank(state: crate::theme::MainMenuRowState) -> u8 {
    match state {
        crate::theme::MainMenuRowState::Rest => 0,
        crate::theme::MainMenuRowState::Hover => 1,
        crate::theme::MainMenuRowState::Active => 2,
    }
}

fn native_footer_visual_root_state(
    current: Option<crate::theme::MainMenuRowState>,
    incoming: crate::theme::MainMenuRowState,
) -> crate::theme::MainMenuRowState {
    match current {
        Some(current)
            if native_footer_state_rank(current) >= native_footer_state_rank(incoming) =>
        {
            current
        }
        _ => incoming,
    }
}

#[cfg(target_os = "macos")]
unsafe fn restyle_footer_glass_capsules(view: id, theme: &crate::theme::Theme) {
    use objc::{msg_send, sel, sel_impl};

    if view == nil {
        return;
    }
    if let Some(glass_class) = objc::runtime::Class::get("NSGlassEffectView") {
        let is_glass: cocoa::base::BOOL = msg_send![view, isKindOfClass: glass_class];
        if is_glass == YES {
            style_float_footer_capsule(view, theme);
        }
    }

    let subviews: id = msg_send![view, subviews];
    if subviews == nil {
        return;
    }
    let count: usize = msg_send![subviews, count];
    for index in 0..count {
        let child: id = msg_send![subviews, objectAtIndex: index];
        restyle_footer_glass_capsules(child, theme);
    }
}

#[cfg(target_os = "macos")]
unsafe fn recolor_footer_hint_subviews_with_colors(view: id, text_color: id, border_color: id) {
    use objc::{class, msg_send, sel, sel_impl};

    if view == nil {
        return;
    }

    if text_color != nil {
        let is_text_field: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSTextField)];
        if is_text_field == YES {
            let _: () = msg_send![view, setTextColor: text_color];
        }
        let is_image_view: cocoa::base::BOOL = msg_send![view, isKindOfClass: class!(NSImageView)];
        if is_image_view == YES {
            let _: () = msg_send![view, setContentTintColor: text_color];
        }
    }

    if border_color != nil
        && appkit_view_identifier(view)
            .as_deref()
            .is_some_and(footer_identifier_uses_keycap_border)
    {
        let layer: id = msg_send![view, layer];
        if layer != nil {
            let border_width: f64 = msg_send![layer, borderWidth];
            if border_width > 0.0 {
                let cg_border: id = msg_send![border_color, CGColor];
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
        recolor_footer_hint_subviews_with_colors(child, text_color, border_color);
    }
}

fn footer_identifier_uses_keycap_border(identifier: &str) -> bool {
    identifier.contains("keycap")
}

#[cfg(target_os = "macos")]
fn footer_hint_item_gap(glass_active: bool, ordinary_gap: f64) -> f64 {
    if glass_active {
        crate::components::footer_chrome::FOOTER_GLASS_BUTTON_GAP_PX as f64
    } else {
        ordinary_gap
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
    for index in 0..count.min(buttons.len()) {
        let item: id = msg_send![subviews, objectAtIndex: index];
        let frame: NSRect = msg_send![item, frame];
        if is_footer_left_pinned_button(&buttons[index]) {
            left_end = left_end.max(frame.origin.x + frame.size.width);
        } else {
            trailing_start = trailing_start.min(frame.origin.x);
        }
    }
    resolve_native_footer_lanes(bounds.size.width, left_end, trailing_start)
}

#[cfg(target_os = "macos")]
fn is_footer_left_pinned_button(button_cfg: &FooterButtonConfig) -> bool {
    if button_cfg.left_pinned {
        return true;
    }
    if matches!(
        button_cfg.action,
        FooterAction::Cwd | FooterAction::AgentModel
    ) {
        // Cwd chip is rendered as a regular footer button (bordered label +
        // bordered keycap + hover state, parity with trailing buttons) and
        // pinned to the far left. Shares the left-pinned helper because the
        // layout pass already handles the splitting of left- vs right-side
        // items via this predicate.
        return true;
    }
    matches!(button_cfg.action, FooterAction::Ai)
        && button_cfg.key.as_ref() == crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN
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
const FOOTER_MIC_ICON_SVG: &str =
    include_str!("../vendor/gpui-component/crates/assets/assets/icons/mic.svg");
#[cfg(target_os = "macos")]
const FOOTER_PROFILE_ICON_SVG: &str =
    include_str!("../vendor/gpui-component/crates/assets/assets/icons/bot.svg");

#[cfg(target_os = "macos")]
fn footer_icon_png_from_svg(svg: &str) -> Option<Vec<u8>> {
    let svg = svg.replace("currentColor", "white");
    let opts = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg, &opts).ok()?;
    let size = 32_u32;
    let mut pixmap = tiny_skia::Pixmap::new(size, size)?;
    let svg_size = tree.size();
    let scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap.take();
    if !rgba.chunks_exact(4).any(|pixel| pixel[3] != 0) {
        return None;
    }
    let image = image::RgbaImage::from_raw(size, size, rgba)?;
    let mut cursor = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, image::ImageFormat::Png)
        .ok()?;
    Some(cursor.into_inner())
}

#[cfg(target_os = "macos")]
fn footer_mic_icon_png_data() -> Option<&'static [u8]> {
    static PNG_DATA: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    PNG_DATA
        .get_or_init(|| footer_icon_png_from_svg(FOOTER_MIC_ICON_SVG))
        .as_deref()
}

#[cfg(target_os = "macos")]
fn footer_profile_icon_png_data() -> Option<&'static [u8]> {
    static PNG_DATA: std::sync::OnceLock<Option<Vec<u8>>> = std::sync::OnceLock::new();
    PNG_DATA
        .get_or_init(|| footer_icon_png_from_svg(FOOTER_PROFILE_ICON_SVG))
        .as_deref()
}

#[cfg(target_os = "macos")]
fn footer_icon_png_data(token: &str) -> Option<&'static [u8]> {
    match token {
        crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN => footer_mic_icon_png_data(),
        crate::components::footer_chrome::FOOTER_PROFILE_ICON_TOKEN => {
            footer_profile_icon_png_data()
        }
        _ => None,
    }
}

#[cfg(target_os = "macos")]
fn footer_icon_png_bytes(token: &str) -> Option<Vec<u8>> {
    if let Some(data) = footer_icon_png_data(token) {
        return Some(data.to_vec());
    }
    let path = crate::components::footer_chrome::footer_icon_path(token)
        .unwrap_or_else(|| crate::components::footer_chrome::FOOTER_PROFILE_ICON_PATH.to_string());
    let svg = if std::path::Path::new(&path).is_absolute() {
        std::fs::read_to_string(path).ok()?
    } else {
        String::from_utf8(crate::utils::assets::embedded_asset_bytes(&path)?).ok()?
    };
    footer_icon_png_from_svg(&svg)
}

#[cfg(target_os = "macos")]
unsafe fn footer_icon_image(token: &str) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let Some(png_data) = footer_icon_png_bytes(token) else {
        return nil;
    };
    let data: id = msg_send![
        class!(NSData),
        dataWithBytes: png_data.as_ptr()
        length: png_data.len()
    ];
    if data == nil {
        return nil;
    }
    let image: id = msg_send![class!(NSImage), alloc];
    let image: id = msg_send![image, initWithData: data];
    if image != nil {
        let _: () = msg_send![image, setTemplate: YES];
    }
    image
}

/// Build a small status-dot NSView for the leading edge of a footer button
/// (the Agent Chat streaming/idle dot inside the Agent·Model chip). Uses
/// accent-preferred active states to match the legacy Agent Chat left-info marker.
#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_leading_dot_view(
    action: FooterAction,
    dot_status: FooterDotStatus,
) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let dot_view: id = msg_send![class!(NSView), alloc];
    let dot_view: id = msg_send![
        dot_view,
        initWithFrame: NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
        )
    ];
    if dot_view == nil {
        return nil;
    }

    let identifier = ns_string(&format!(
        "{}{}",
        FOOTER_HINT_LEADING_DOT_ID_PREFIX,
        footer_action_key(action)
    ));
    if identifier != nil {
        let _: () = msg_send![dot_view, setIdentifier: identifier];
    }

    let _: () = msg_send![dot_view, setWantsLayer: YES];
    let layer: id = msg_send![dot_view, layer];
    if layer != nil {
        let _: () = msg_send![layer, setMasksToBounds: NO];
        update_footer_dot_layer_for_status(layer, dot_status, true);
    }
    let _: () = msg_send![
        dot_view,
        setHidden: if matches!(dot_status, FooterDotStatus::Hidden) {
            YES
        } else {
            NO
        }
    ];
    dot_view
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
    let action_key = footer_action_key(button_cfg.action);
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
        crate::components::footer_chrome::split_footer_shortcut(button_cfg.key.as_ref());

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

#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_text_field(
    text: &str,
    font: id,
    text_color: id,
    alignment: usize,
) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let field: id = msg_send![class!(NSTextField), alloc];
    let field: id = msg_send![field, init];
    if field == nil {
        return nil;
    }

    let string_value = ns_string(text);
    if string_value == nil {
        return nil;
    }

    let _: () = msg_send![field, setStringValue: string_value];
    let _: () = msg_send![field, setBezeled: NO];
    let _: () = msg_send![field, setBordered: NO];
    let _: () = msg_send![field, setDrawsBackground: NO];
    let _: () = msg_send![field, setEditable: NO];
    let _: () = msg_send![field, setSelectable: NO];
    if font != nil {
        let _: () = msg_send![field, setFont: font];
    }
    if text_color != nil {
        let _: () = msg_send![field, setTextColor: text_color];
    }
    let _: () = msg_send![field, setAlignment: alignment];
    let _: () = msg_send![field, setLineBreakMode: 4usize];
    let _: () = msg_send![field, setUsesSingleLineMode: YES];
    let _: () = msg_send![field, sizeToFit];
    field
}

#[cfg(test)]
mod footer_layout_tests {
    use super::{
        footer_active_dot_hex, footer_dot_hex, footer_hint_content_layout,
        footer_hint_content_layout_for_button, footer_hint_item_gap, footer_hint_label_widths,
        footer_hint_legacy_extra_padding, footer_hint_max_item_width, footer_hint_slot_width,
        footer_identifier_uses_keycap_border, main_window_detached_footer_regions_appkit,
        main_window_detached_footer_regions_gpui, native_footer_left_hit_target_flags,
        native_footer_visual_event_changed, native_footer_visual_root_state,
        resolved_native_footer_button_state, should_use_gpui_footer_overlay, FooterAction,
        FooterButtonConfig, FooterDotStatus, NativeFooterLeftHitTargetFlags,
        NativeFooterVisualTheme, FOOTER_HINT_KEY_LABEL_GAP, FOOTER_HINT_PADDING_X,
        FOOTER_RUN_HINT_PADDING_X,
    };
    #[cfg(target_os = "macos")]
    use super::{native_footer_visual_theme_from_parts, resolve_native_footer_visual_theme};

    fn assert_partitions_host(regions: &super::MainWindowDetachedFooterRegions) {
        let partition_height =
            regions.main_content.height + regions.transparent_gap.height + regions.footer.height;
        assert!((partition_height - regions.host.height).abs() < f32::EPSILON);
        assert!(regions.main_content.height >= 0.0);
        assert!(regions.transparent_gap.height >= 0.0);
        assert!(regions.footer.height >= 0.0);
    }

    #[test]
    fn native_glass_capsules_use_the_shared_open_gap() {
        assert_eq!(footer_hint_item_gap(true, 2.0), 6.0);
        assert_eq!(footer_hint_item_gap(false, 2.0), 2.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_footer_lane_keeps_left_capsule_clear_of_trailing_actions() {
        let lane = super::resolve_native_footer_lanes(718.0, 120.0, 506.0);
        let capsule_min_x = lane.left_info_x - super::FOOTER_LEFT_INFO_CAPSULE_PAD_X;
        let capsule_max_x =
            lane.left_info_x + lane.left_info_width + super::FOOTER_LEFT_INFO_CAPSULE_PAD_X;
        let gap = f64::from(crate::components::footer_chrome::FOOTER_LEFT_RIGHT_MIN_GAP_PX);

        assert!(capsule_min_x >= lane.left_pinned_end_x + gap);
        assert!(capsule_max_x <= lane.trailing_start_x - gap);
        assert!(!lane.trailing_overflow);
    }

    #[cfg(target_os = "macos")]
    fn representative_left_info_measurements() -> super::FooterLeftInfoMeasurements {
        super::FooterLeftInfoMeasurements {
            cwd_fixed_width: 40.0,
            cwd_label_width: 80.0,
            primary_fixed_width: 30.0,
            primary_label_width: 100.0,
            has_cwd: true,
            primary_visible_without_label: true,
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn native_footer_lane_hides_left_info_when_clusters_exhaust_width() {
        let lane = super::resolve_native_footer_lanes(250.0, 132.0, 136.0);
        let allocation = super::resolve_footer_left_info_allocation(
            lane.left_info_width,
            representative_left_info_measurements(),
        );

        assert!(lane.trailing_overflow);
        assert_eq!(lane.left_info_width, 0.0);
        assert_eq!(
            allocation.degradation,
            super::FooterLeftInfoDegradation::Hidden
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn footer_left_info_allocation_degrades_monotonically() {
        use super::FooterLeftInfoDegradation::*;
        let measured = representative_left_info_measurements();

        assert_eq!(
            super::resolve_footer_left_info_allocation(300.0, measured).degradation,
            Full
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(200.0, measured).degradation,
            TruncatedLabels
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(110.0, measured).degradation,
            CwdAffordanceOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(80.0, measured).degradation,
            PrimaryOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(40.0, measured).degradation,
            PrimaryAffordanceOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(20.0, measured).degradation,
            Hidden
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn footer_left_info_allocator_handles_no_cwd_and_long_labels() {
        use super::FooterLeftInfoDegradation::*;

        let no_cwd = super::FooterLeftInfoMeasurements {
            cwd_fixed_width: 0.0,
            cwd_label_width: 0.0,
            primary_fixed_width: 30.0,
            primary_label_width: 100.0,
            has_cwd: false,
            primary_visible_without_label: true,
        };
        assert_eq!(
            super::resolve_footer_left_info_allocation(90.0, no_cwd).degradation,
            PrimaryOnly
        );
        assert_eq!(
            super::resolve_footer_left_info_allocation(30.0, no_cwd).degradation,
            PrimaryAffordanceOnly
        );

        let long = super::FooterLeftInfoMeasurements {
            cwd_label_width: 480.0,
            primary_label_width: 640.0,
            ..representative_left_info_measurements()
        };
        let allocation = super::resolve_footer_left_info_allocation(180.0, long);
        assert_eq!(allocation.degradation, TruncatedLabels);
        assert!(allocation.cwd_label_width >= 24.0);
        assert!(allocation.primary_label_width >= 32.0);
        assert!(
            long.cwd_fixed_width
                + long.primary_fixed_width
                + allocation.cwd_label_width
                + allocation.primary_label_width
                <= allocation.available_width + f64::EPSILON
        );
    }

    #[test]
    fn native_glass_mode_always_owns_the_main_footer_without_an_overlay() {
        assert!(!should_use_gpui_footer_overlay(true, true));
        assert!(!should_use_gpui_footer_overlay(true, false));
        assert!(should_use_gpui_footer_overlay(false, true));
        assert!(!should_use_gpui_footer_overlay(false, false));
    }

    #[test]
    fn detached_footer_regions_partition_host_without_overlap() {
        let gpui = main_window_detached_footer_regions_gpui(750.0, 480.0, 32.0, 8.0, 2.0);
        let appkit = main_window_detached_footer_regions_appkit(750.0, 480.0, 32.0, 8.0, 2.0);

        assert_partitions_host(&gpui);
        assert_partitions_host(&appkit);
        assert_eq!(
            gpui.main_content.y + gpui.main_content.height,
            gpui.transparent_gap.y
        );
        assert_eq!(
            gpui.transparent_gap.y + gpui.transparent_gap.height,
            gpui.footer.y
        );
        assert_eq!(
            appkit.footer.y + appkit.footer.height,
            appkit.transparent_gap.y
        );
        assert_eq!(
            appkit.transparent_gap.y + appkit.transparent_gap.height,
            appkit.main_content.y
        );
    }

    #[test]
    fn detached_footer_regions_preserve_main_top_edge() {
        let short = main_window_detached_footer_regions_appkit(750.0, 480.0, 32.0, 8.0, 2.0);
        let tall = main_window_detached_footer_regions_appkit(750.0, 620.0, 32.0, 8.0, 2.0);

        assert_eq!(
            short.main_content.y + short.main_content.height,
            short.host.height
        );
        assert_eq!(
            tall.main_content.y + tall.main_content.height,
            tall.host.height
        );
        assert_eq!(short.main_content.y, tall.main_content.y);
    }

    #[test]
    fn detached_regions_round_to_two_x_backing_scale() {
        let regions = main_window_detached_footer_regions_gpui(749.74, 480.24, 31.76, 8.24, 2.0);

        for value in [
            regions.host.width,
            regions.host.height,
            regions.main_content.height,
            regions.transparent_gap.y,
            regions.transparent_gap.height,
            regions.footer.y,
            regions.footer.height,
        ] {
            assert_eq!(value * 2.0, (value * 2.0).round());
        }
        assert_partitions_host(&regions);
    }

    #[test]
    fn legacy_zero_strip_geometry_is_unchanged() {
        let regions = main_window_detached_footer_regions_gpui(750.0, 480.0, 0.0, 0.0, 2.0);

        assert_eq!(regions.main_content, regions.host);
        assert_eq!(regions.transparent_gap.height, 0.0);
        assert_eq!(regions.footer.height, 0.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn appkit_footer_rect_converts_to_top_left_screenshot_coordinates() {
        use cocoa::foundation::{NSPoint, NSRect, NSSize};

        let converted = super::appkit_screenshot_bounds(
            NSRect::new(NSPoint::new(10.0, 4.0), NSSize::new(100.0, 28.0)),
            480.0,
        );

        assert_eq!(converted.x, 10.0);
        assert_eq!(converted.y, 448.0);
        assert_eq!(converted.width, 100.0);
        assert_eq!(converted.height, 28.0);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn tips_keeps_a_distinct_native_action_selector() {
        use objc::{sel, sel_impl};

        assert_eq!(
            super::footer_action_selector(super::FooterAction::Tips),
            sel!(tipsFooterAction:)
        );
        assert_ne!(
            super::footer_action_selector(super::FooterAction::Tips),
            super::footer_action_selector(super::FooterAction::Actions)
        );
    }

    #[test]
    fn appkit_inventory_fails_closed_for_empty_or_duplicate_ids() {
        use crate::protocol::{AppKitFidelityNode, FidelityCaptureStatus};

        assert_eq!(
            super::appkit_fidelity_inventory_blocker(&[]),
            Some(FidelityCaptureStatus::EmptyInventory)
        );

        let duplicate = AppKitFidelityNode {
            id: "script-kit-footer-effect".to_string(),
            ..Default::default()
        };
        assert_eq!(
            super::appkit_fidelity_inventory_blocker(&[duplicate.clone(), duplicate]),
            Some(FidelityCaptureStatus::DuplicateIdentifiers)
        );

        let unique = AppKitFidelityNode {
            id: "script-kit-footer-divider".to_string(),
            ..Default::default()
        };
        assert_eq!(super::appkit_fidelity_inventory_blocker(&[unique]), None);
    }

    #[gpui::test]
    fn footer_overlay_fidelity_is_a_separate_paint_target(cx: &mut gpui::TestAppContext) {
        use gpui::AppContext as _;

        super::clear_main_footer_overlay_fidelity_snapshot();
        let window = cx.update(|cx| {
            gpui_component::init(cx);
            let mut config = super::MainWindowFooterConfig::new(
                "agent_chat",
                vec![super::FooterButtonConfig::new(
                    super::FooterAction::Run,
                    "↵",
                    "Send",
                )],
            );
            config.left_info = Some(super::FooterLeftInfo {
                model_name: "GPT-5.6 SOL".to_string(),
                ..Default::default()
            });
            cx.open_window(Default::default(), |_, cx| {
                cx.new(|_| super::GpuiFooterOverlay::new(config, 320.0))
            })
            .unwrap()
        });

        window
            .update(cx, |_, window, cx| {
                window.set_fidelity_capture_target_for_test(Some("agent-chat"));
                cx.notify();
            })
            .unwrap();
        cx.run_until_parked();

        let snapshot = super::main_footer_overlay_fidelity_snapshot()
            .expect("footer overlay completed-frame fidelity snapshot");
        assert_eq!(snapshot.target_id, "gpui-footer-overlay");
        assert_eq!(snapshot.target_kind, "footerOverlay");
        assert_eq!(snapshot.parent_target_id.as_deref(), Some("main"));
        assert!(snapshot.frame_generation > 0);
        assert!(snapshot
            .nodes
            .iter()
            .any(|node| node.id == "agent-chat.footer-overlay.button.run"
                && node.primitive_count > 0));
        assert!(snapshot
            .nodes
            .iter()
            .any(|node| node.id == "agent-chat.footer-overlay.model" && node.primitive_count > 0));
        assert!(snapshot.nodes.iter().all(|node| {
            node.measurement_frame_generation == snapshot.frame_generation
                && node.measurement_provenance == "paint-time"
        }));

        super::clear_main_footer_overlay_fidelity_snapshot();
    }

    #[test]
    fn left_pinned_buttons_do_not_receive_legacy_extra_padding() {
        // The left chips and the Run button are start-anchored, so trailing
        // padding would show up as a visibly wider gap before the next item.
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Cwd,
                "⇥",
                "~/ai_completion"
            )),
            0.0
        );
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::AgentModel,
                "⇧⇥",
                "Codex · GPT-5.6 SOL"
            )),
            0.0
        );
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Run,
                "↵",
                "Send"
            )),
            0.0
        );
        // Trailing action buttons keep the comfortable 12px reserve.
        assert_eq!(
            footer_hint_legacy_extra_padding(&FooterButtonConfig::new(
                FooterAction::Actions,
                "⌘K",
                "Actions"
            )),
            12.0
        );
    }

    #[test]
    fn left_pinned_cwd_uses_same_label_to_key_gap_as_trailing_buttons() {
        let button = FooterButtonConfig::new(FooterAction::Cwd, "⇥", "~/ai_completion");
        let label_width = 92.0;
        let key_width = 20.0;
        let item_width =
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width + FOOTER_RUN_HINT_PADDING_X * 2.0;
        let (label_x, key_x, _) = footer_hint_content_layout_for_button(
            &button,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_HINT_PADDING_X,
            FOOTER_RUN_HINT_PADDING_X,
        );
        // Label anchored at the leading padding, keycap exactly one content-gap
        // after the label — identical spacing to the right-side buttons.
        assert_eq!(label_x, FOOTER_RUN_HINT_PADDING_X.round());
        assert_eq!(key_x - (label_x + label_width), FOOTER_HINT_KEY_LABEL_GAP);
    }

    #[test]
    fn footer_hint_slot_widths_are_stable_per_action() {
        assert_eq!(footer_hint_slot_width(FooterAction::Run), 92.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Actions), 92.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Ai), 52.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Stop), 76.0);
        assert_eq!(footer_hint_slot_width(FooterAction::PasteResponse), 140.0);
    }

    #[test]
    fn run_slot_remains_at_least_as_wide_as_actions_and_wider_than_ai() {
        assert!(
            footer_hint_slot_width(FooterAction::Run)
                >= footer_hint_slot_width(FooterAction::Actions)
        );
        assert!(
            footer_hint_slot_width(FooterAction::Run) > footer_hint_slot_width(FooterAction::Ai)
        );
    }

    #[test]
    fn footer_hint_content_group_is_centered_within_slot() {
        let item_width = 92.0;
        let label_width = 34.0;
        let key_width = 18.0;

        let (label_x, key_x, content_width) = footer_hint_content_layout(
            FooterAction::Actions,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );
        let left_padding = label_x;
        let right_padding = item_width - (key_x + key_width);

        assert_eq!(
            content_width,
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width
        );
        assert!((left_padding - right_padding).abs() <= 1.0);
    }

    #[test]
    fn run_hint_keeps_key_glyph_anchored_to_trailing_padding() {
        let short = footer_hint_content_layout(
            FooterAction::Run,
            92.0,
            20.0,
            18.0,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );
        let long = footer_hint_content_layout(
            FooterAction::Run,
            140.0,
            64.0,
            18.0,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );

        assert_eq!(short.1, 68.0);
        assert_eq!(long.1, 116.0);
        assert_eq!(92.0 - (short.1 + 18.0), 6.0);
        assert_eq!(140.0 - (long.1 + 18.0), 6.0);
    }

    #[test]
    fn run_hint_native_layout_can_balance_short_label_padding() {
        let label_width = 26.0;
        let key_width = 20.0;
        let item_width =
            label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width + FOOTER_RUN_HINT_PADDING_X * 2.0;
        let (label_x, key_x, _) = footer_hint_content_layout(
            FooterAction::Run,
            item_width,
            label_width,
            key_width,
            FOOTER_HINT_KEY_LABEL_GAP,
            FOOTER_RUN_HINT_PADDING_X,
        );

        assert_eq!(label_x, FOOTER_RUN_HINT_PADDING_X);
        assert_eq!(item_width - (key_x + key_width), FOOTER_RUN_HINT_PADDING_X);
    }

    #[test]
    fn all_selected_footer_actions_use_the_main_menu_active_row_fill() {
        let theme = crate::theme::Theme::dark_default();
        let active = crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
            .row_states
            .active
            .background_rgba
            .expect("active rows have a fill");

        for _action in [FooterAction::Actions, FooterAction::Run, FooterAction::Ai] {
            assert_eq!(
                crate::components::footer_chrome::themed_footer_button_active_rgba(&theme),
                active
            );
        }
    }

    #[test]
    fn native_footer_state_keeps_active_precedence_over_hover() {
        use crate::theme::MainMenuRowState::{Active, Hover, Rest};

        assert_eq!(
            resolved_native_footer_button_state(false, false, false, false),
            Rest
        );
        assert_eq!(
            resolved_native_footer_button_state(false, true, false, false),
            Hover
        );
        assert_eq!(
            resolved_native_footer_button_state(true, true, false, false),
            Active
        );
        assert_eq!(
            resolved_native_footer_button_state(false, true, true, true),
            Active
        );
        assert_eq!(
            resolved_native_footer_button_state(false, false, false, true),
            Rest
        );
    }

    #[test]
    fn left_info_hit_target_carries_selected_state() {
        assert_eq!(
            native_footer_left_hit_target_flags(true, true),
            NativeFooterLeftHitTargetFlags {
                selected: true,
                enabled: true,
            }
        );
    }

    #[test]
    fn cwd_chip_hit_target_carries_selected_state() {
        let flags = native_footer_left_hit_target_flags(true, true);
        assert!(flags.selected);
        assert!(flags.enabled);
    }

    #[test]
    fn left_visual_root_prefers_active_over_sibling_hover() {
        use crate::theme::MainMenuRowState::{Active, Hover, Rest};

        assert_eq!(native_footer_visual_root_state(None, Rest), Rest);
        assert_eq!(native_footer_visual_root_state(Some(Rest), Hover), Hover);
        assert_eq!(native_footer_visual_root_state(Some(Hover), Active), Active);
        assert_eq!(native_footer_visual_root_state(Some(Active), Hover), Active);
    }

    #[test]
    fn reused_left_hit_target_receives_fresh_state() {
        let initial = native_footer_left_hit_target_flags(false, true);
        let reused = native_footer_left_hit_target_flags(true, false);
        assert_ne!(initial, reused);
        assert!(reused.selected);
        assert!(!reused.enabled);
    }

    #[test]
    fn native_footer_visual_event_reports_only_signature_changes() {
        let id = "unit-native-footer-visual-event";
        assert!(native_footer_visual_event_changed(id, 1, 10, 0x112233));
        assert!(!native_footer_visual_event_changed(id, 1, 10, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 10, 0x112233));
        assert!(!native_footer_visual_event_changed(id, 2, 10, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 11, 0x112233));
        assert!(native_footer_visual_event_changed(id, 2, 11, 0x445566));
    }

    #[test]
    fn native_footer_hover_uses_hover_keycap_border_alpha() {
        let theme = crate::theme::Theme::dark_default();
        let visual_theme = resolve_native_footer_visual_theme(&theme);
        let rest = visual_theme.border_alpha(crate::theme::MainMenuRowState::Rest);
        let hover = visual_theme.border_alpha(crate::theme::MainMenuRowState::Hover);
        let active = visual_theme.border_alpha(crate::theme::MainMenuRowState::Active);

        assert_eq!(
            hover,
            crate::components::footer_chrome::footer_keycap_border_alpha_for_state(
                &theme,
                crate::theme::MainMenuRowState::Hover,
            )
        );
        assert!(hover >= rest);
        assert!(active >= rest);
    }

    #[test]
    fn native_footer_refresh_signature_tracks_canonical_palette() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0x112233, 0.1, 0.2, 0.3);

        let mut changed = palette;
        changed.hover.background_rgba = Some(0x44556677);
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(changed, 0x112233, 0.1, 0.2, 0.3)
        );
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(palette, 0x445566, 0.1, 0.2, 0.3)
        );
        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(palette, 0x112233, 0.1, 0.25, 0.3)
        );
    }

    #[test]
    fn native_footer_refresh_signature_changes_with_text_name_alpha() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0xffffff, 0.1, 0.2, 0.3);
        let mut changed = palette;
        changed.rest.primary_foreground_rgba =
            (changed.rest.primary_foreground_rgba & 0xffffff00) | 0x7f;

        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(changed, 0xffffff, 0.1, 0.2, 0.3)
        );
    }

    #[test]
    fn native_footer_refresh_signature_changes_with_row_kind_and_accent() {
        let theme = crate::theme::Theme::dark_default();
        let palette =
            crate::components::footer_chrome::resolved_footer_button_visual_colors(&theme)
                .row_states;
        let baseline = native_footer_visual_theme_from_parts(palette, 0xffffff, 0.1, 0.2, 0.3);
        let mut accent = palette;
        accent.active.background_rgba = Some(0x18a0fbff);
        accent.active.primary_foreground_rgba = 0x001122ff;

        assert_ne!(
            baseline,
            native_footer_visual_theme_from_parts(accent, 0xffffff, 0.1, 0.2, 0.3)
        );
    }

    #[test]
    fn native_footer_theme_refresh_preserves_hover_state() {
        use crate::theme::MainMenuRowState::Hover;

        let theme = crate::theme::Theme::dark_default();
        let old_visual = resolve_native_footer_visual_theme(&theme);
        let mut new_palette = old_visual.row_palette;
        new_palette.hover.background_rgba = Some(0x44556612);
        let new_visual =
            native_footer_visual_theme_from_parts(new_palette, 0xffffff, 0.11, 0.37, 0.73);

        assert_eq!(
            resolved_native_footer_button_state(false, true, false, false),
            Hover
        );
        assert_eq!(
            new_visual.row_palette.for_state(Hover).background_rgba,
            Some(0x44556612)
        );
    }

    #[test]
    fn native_footer_theme_refresh_preserves_active_state() {
        use crate::theme::MainMenuRowState::Active;

        let theme = crate::theme::Theme::dark_default();
        let old_visual = resolve_native_footer_visual_theme(&theme);
        let mut new_palette = old_visual.row_palette;
        new_palette.active.background_rgba = Some(0x77889920);
        let new_visual =
            native_footer_visual_theme_from_parts(new_palette, 0xffffff, 0.11, 0.37, 0.73);

        assert_eq!(
            resolved_native_footer_button_state(true, true, false, false),
            Active
        );
        assert_eq!(
            new_visual.row_palette.for_state(Active).background_rgba,
            Some(0x77889920)
        );
    }

    #[test]
    fn footer_state_recolors_only_keycap_borders_not_glass_capsule_rims() {
        for identifier in [
            "script-kit-footer-keycap-actions-0",
            "script-kit-footer-left-info-keycap-0",
            "script-kit-footer-cwd-chip-keycap-0",
        ] {
            assert!(footer_identifier_uses_keycap_border(identifier));
        }
        for identifier in [
            "script-kit-footer-capsule-content-actions",
            "script-kit-footer-left-info-capsule-content",
            "script-kit-footer-state-layer-actions",
        ] {
            assert!(!footer_identifier_uses_keycap_border(identifier));
        }
    }

    #[test]
    fn run_hint_width_is_capped_to_stable_slot() {
        let buttons = vec![
            FooterButtonConfig::new(
                FooterAction::Run,
                "↵",
                "Open Screen Recording Permission Assistant",
            ),
            FooterButtonConfig::new(FooterAction::Ai, "⌘↵", "Agent"),
            FooterButtonConfig::new(FooterAction::Actions, "⌘K", "Actions"),
        ];

        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 480.0, &buttons),
            Some(242.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 640.0, &buttons),
            Some(242.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Run, 120.0, &buttons),
            Some(92.0)
        );
        assert_eq!(
            footer_hint_max_item_width(FooterAction::Ai, 480.0, &buttons),
            None
        );
    }

    // The GPUI footer overlay no longer estimates label widths in Rust: the
    // Run button takes its intrinsic (text-measured) width via flexbox,
    // floored at the slot minimum and capped at FOOTER_RUN_SLOT_MAX_WIDTH_PX.
    // See tests/main_window_footer_surface_owner_contract.rs for the contract.

    #[test]
    fn run_hint_label_text_width_truncates_inside_remaining_slot() {
        let (chip_width, text_width) =
            footer_hint_label_widths(360.0, 5.0, 18.0, Some(180.0), 20.0, FOOTER_HINT_PADDING_X);

        // Derived from the shared chrome tokens so token tuning does not
        // invalidate the truncation contract being tested here.
        let expected_chip = 180.0 - FOOTER_HINT_PADDING_X * 2.0 - FOOTER_HINT_KEY_LABEL_GAP - 20.0;
        assert_eq!(chip_width, expected_chip);
        assert_eq!(text_width, expected_chip - 10.0);
        assert!(text_width < 360.0);
    }

    #[test]
    fn footer_buttons_keep_two_pixel_vertical_inset() {
        assert_eq!(
            crate::components::footer_chrome::FOOTER_BUTTON_VERTICAL_INSET_PX,
            2.0
        );
        assert_eq!(
            crate::components::footer_chrome::footer_button_height(32.0),
            28.0
        );
    }

    #[test]
    fn active_dot_prefers_the_most_contrasting_theme_color() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.background.main = 0x101114;
        theme.colors.accent.selected = 0x3a4250;
        theme.colors.text.primary = 0xf5f7fa;

        assert_eq!(
            footer_active_dot_hex(&theme, false),
            theme.colors.text.primary
        );

        theme.colors.accent.selected = 0xffc600;
        theme.colors.text.primary = 0x8892a0;
        assert_eq!(
            footer_active_dot_hex(&theme, false),
            theme.colors.accent.selected
        );
    }

    #[test]
    fn active_dot_can_force_accent_for_agent_chat_states() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.background.main = 0x101114;
        theme.colors.accent.selected = 0x3a4250;
        theme.colors.text.primary = 0xf5f7fa;

        assert_eq!(
            footer_active_dot_hex(&theme, true),
            theme.colors.accent.selected
        );
    }

    #[test]
    fn footer_dot_colors_follow_theme_tokens() {
        let mut theme = crate::theme::Theme::dark_default();
        theme.colors.text.secondary = 0x778899;
        theme.colors.ui.error = 0xaa3344;

        assert_eq!(
            footer_dot_hex(FooterDotStatus::Idle, &theme, false),
            theme.colors.text.secondary
        );
        assert_eq!(
            footer_dot_hex(FooterDotStatus::Error, &theme, false),
            theme.colors.ui.error
        );
    }
}

#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FooterWindowKind {
    Main,
    Dictation,
    AgentChat,
}

#[cfg(target_os = "macos")]
fn send_footer_action_from_sender(sender: id, action: FooterAction) {
    // SAFETY: `sender` is a live NSButton passed by AppKit's target/action dispatch.
    let title = unsafe { footer_sender_window_title(sender) };
    let window_kind = if let Some(ref t) = title {
        if t.contains("Script Kit Dictation") {
            FooterWindowKind::Dictation
        } else if t.contains("Script Kit Agent Chat") {
            FooterWindowKind::AgentChat
        } else {
            FooterWindowKind::Main
        }
    } else {
        FooterWindowKind::Main
    };
    send_footer_action_to_channel_v2(action, window_kind);
}

#[cfg(target_os = "macos")]
fn send_footer_action_to_channel_v2(action: FooterAction, window_kind: FooterWindowKind) {
    let action_name = footer_action_key(action);
    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_action_enqueued",
        action = action_name,
        ?window_kind,
        "Enqueued native footer action"
    );
    let (tx, _) = match window_kind {
        FooterWindowKind::Dictation => dictation_footer_action_channel(),
        FooterWindowKind::AgentChat => agent_chat_footer_action_channel(),
        FooterWindowKind::Main => footer_action_channel(),
    };
    if let Err(error) = tx.try_send(action) {
        tracing::warn!(
            target: "script_kit::footer_popup",
            event = "native_footer_action_enqueue_failed",
            action = action_name,
            %error,
            "Failed to enqueue footer action"
        );
    }
}

fn send_footer_action_to_channel(action: FooterAction, dictation_footer: bool) {
    #[cfg(target_os = "macos")]
    {
        let window_kind = if dictation_footer {
            FooterWindowKind::Dictation
        } else {
            FooterWindowKind::Main
        };
        send_footer_action_to_channel_v2(action, window_kind);
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (tx, _) = if dictation_footer {
            dictation_footer_action_channel()
        } else {
            footer_action_channel()
        };
        if let Err(error) = tx.try_send(action) {
            tracing::warn!(
                target: "script_kit::footer_popup",
                event = "native_footer_action_enqueue_failed",
                action = footer_action_key(action),
                %error,
                "Failed to enqueue footer action"
            );
        }
    }
}

#[cfg(target_os = "macos")]
unsafe fn footer_sender_window_title(sender: id) -> Option<String> {
    use objc::{msg_send, sel, sel_impl};
    use std::ffi::CStr;

    if sender == nil {
        return None;
    }

    let ns_window: id = msg_send![sender, window];
    if ns_window == nil {
        return None;
    }

    let title: id = msg_send![ns_window, title];
    if title == nil {
        return None;
    }

    let utf8: *const std::os::raw::c_char = msg_send![title, UTF8String];
    if utf8.is_null() {
        return None;
    }

    Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
}

fn footer_action_key(action: FooterAction) -> &'static str {
    match action {
        FooterAction::Run => "run",
        FooterAction::Actions => "actions",
        FooterAction::Ai => "ai",
        FooterAction::Apply => "apply",
        FooterAction::Replace => "replace",
        FooterAction::Append => "append",
        FooterAction::Copy => "copy",
        FooterAction::Expand => "expand",
        FooterAction::Retry => "retry",
        FooterAction::Close => "close",
        FooterAction::Stop => "stop",
        FooterAction::PasteResponse => "pasteResponse",
        FooterAction::Cwd => "cwd",
        FooterAction::AgentModel => "agentModel",
        FooterAction::Tips => "tips",
    }
}

#[cfg(target_os = "macos")]
fn ns_string(text: &str) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let Ok(c_string) = std::ffi::CString::new(text) else {
        return nil;
    };

    // SAFETY: The CString is NUL-terminated and lives for the duration of the call.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c_string.as_ptr()] }
}

#[cfg(target_os = "macos")]
unsafe fn ns_color_from_rgba(rgba: u32) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let red = ((rgba >> 24) & 0xFF) as f64 / 255.0;
    let green = ((rgba >> 16) & 0xFF) as f64 / 255.0;
    let blue = ((rgba >> 8) & 0xFF) as f64 / 255.0;
    let alpha = (rgba & 0xFF) as f64 / 255.0;

    // SAFETY: Standard AppKit color construction on the main thread.
    msg_send![
        class!(NSColor),
        colorWithSRGBRed: red
        green: green
        blue: blue
        alpha: alpha
    ]
}

#[cfg(target_os = "macos")]
unsafe fn ns_color_from_hex_with_alpha(hex: u32, alpha: f64) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let red = ((hex >> 16) & 0xFF) as f64 / 255.0;
    let green = ((hex >> 8) & 0xFF) as f64 / 255.0;
    let blue = (hex & 0xFF) as f64 / 255.0;

    // SAFETY: Standard AppKit color construction on the main thread.
    msg_send![
        class!(NSColor),
        colorWithSRGBRed: red
        green: green
        blue: blue
        alpha: alpha
    ]
}

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

fn native_footer_visual_event_changed(
    button_id: &str,
    state_signature: usize,
    color_signature: usize,
    keycap_hex: u32,
) -> bool {
    static LAST_REPORTED: std::sync::OnceLock<
        std::sync::Mutex<std::collections::HashMap<String, (usize, usize, u32)>>,
    > = std::sync::OnceLock::new();
    let reported =
        LAST_REPORTED.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let Ok(mut reported) = reported.lock() else {
        // A poisoned diagnostics cache must never suppress real rendering.
        return true;
    };
    let signature = (state_signature, color_signature, keycap_hex);
    if reported.get(button_id).copied() == Some(signature) {
        false
    } else {
        reported.insert(button_id.to_string(), signature);
        true
    }
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_state_group(button: id) {
    use objc::{msg_send, sel, sel_impl};

    if button == nil {
        return;
    }
    let group_root: id = msg_send![button, superview];
    if group_root != nil {
        refresh_footer_button_visual_states(group_root);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_down(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    event: id,
) {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `this` is our NSButton subclass. Actions opens a persistent popup,
    // so it owns selected visuals on mouse down instead of waiting for AppKit's
    // mouse-up action cycle to briefly clear and restore the state.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            let this_id = this as *const _ as id;
            let _: () = msg_send![super(this_id, class!(NSButton)), mouseDown: event];
            return;
        }

        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        if is_actions != YES {
            let this_id = this as *const _ as id;
            let _: () = msg_send![super(this_id, class!(NSButton)), mouseDown: event];
            return;
        }

        let button_id = this as *const _ as id;
        if let Some(obj) = button_id.as_mut() {
            obj.set_ivar::<cocoa::base::BOOL>("_selected", YES);
        }
        refresh_footer_button_visual_state_group(button_id);

        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_actions_mouse_down_selected",
            "Selected native footer Actions on mouse down"
        );
        let this_id = this as *const _ as id;
        send_footer_action_from_sender(this_id, FooterAction::Actions);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_entered(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    // SAFETY: Set hover background on the parent container's layer.
    // Recompute color from theme each time to avoid dangling CGColor pointers.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            return;
        }
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        let selected: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_selected");
        if let Some(object) = (this as *const _ as id).as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", YES);
        }
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_entered",
            is_actions_button = is_actions == YES,
            "Native footer button hover entered"
        );

        refresh_footer_button_visual_state_group(this as *const _ as id);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_exited(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    // SAFETY: Clear hover background on the parent container's layer.
    // If this button has _selected set, restore the selected color instead
    // of clearing.
    unsafe {
        let selected: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_selected");
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        let actions_window_open = crate::actions::is_actions_window_open();
        if let Some(object) = (this as *const _ as id).as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_exited",
            is_actions_button = is_actions == YES,
            selected = selected == YES,
            actions_window_open,
            "Native footer button hover exited"
        );

        refresh_footer_button_visual_state_group(this as *const _ as id);
    }
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

#[cfg(target_os = "macos")]
fn footer_action_target() -> id {
    use std::sync::OnceLock;

    use objc::{msg_send, sel, sel_impl};

    static TARGET: OnceLock<usize> = OnceLock::new();

    // SAFETY: Creates the singleton footer action target via ObjC `new`; stored
    // for process lifetime in `OnceLock`.
    *TARGET.get_or_init(|| unsafe {
        let target: id = msg_send![footer_action_target_class(), new];
        target as usize
    }) as id
}

#[cfg(target_os = "macos")]
fn footer_action_selector(action: FooterAction) -> objc::runtime::Sel {
    use objc::{sel, sel_impl};

    match action {
        FooterAction::Run => sel!(runFooterAction:),
        FooterAction::Actions => sel!(actionsFooterAction:),
        FooterAction::Ai => sel!(aiFooterAction:),
        FooterAction::Apply => sel!(applyFooterAction:),
        FooterAction::Replace => sel!(replaceFooterAction:),
        FooterAction::Append => sel!(appendFooterAction:),
        FooterAction::Copy => sel!(copyFooterAction:),
        FooterAction::Expand => sel!(expandFooterAction:),
        FooterAction::Retry => sel!(retryFooterAction:),
        FooterAction::Close => sel!(closeFooterAction:),
        FooterAction::Stop => sel!(stopFooterAction:),
        FooterAction::PasteResponse => sel!(pasteResponseFooterAction:),
        FooterAction::Cwd => sel!(cwdFooterAction:),
        FooterAction::AgentModel => sel!(agentModelFooterAction:),
        FooterAction::Tips => sel!(tipsFooterAction:),
    }
}

#[cfg(target_os = "macos")]
fn footer_action_target_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

    // SAFETY: ObjC class registration is serialized by `OnceLock`. Superclass
    // is `NSObject`; installed action methods match AppKit target/action ABI.
    *CLASS.get_or_init(|| unsafe {
        let superclass = class!(NSObject);
        let Some(mut decl) = ClassDecl::new("ScriptKitFooterActionTarget", superclass) else {
            return class!(NSObject) as *const _ as usize;
        };
        decl.add_method(
            sel!(runFooterAction:),
            footer_run_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(actionsFooterAction:),
            footer_actions_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(aiFooterAction:),
            footer_ai_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applyFooterAction:),
            footer_apply_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(replaceFooterAction:),
            footer_replace_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(appendFooterAction:),
            footer_append_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(copyFooterAction:),
            footer_copy_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(expandFooterAction:),
            footer_expand_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(retryFooterAction:),
            footer_retry_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(closeFooterAction:),
            footer_close_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(stopFooterAction:),
            footer_stop_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(pasteResponseFooterAction:),
            footer_paste_response_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(cwdFooterAction:),
            footer_cwd_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(agentModelFooterAction:),
            footer_agent_model_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(tipsFooterAction:),
            footer_tips_action as extern "C" fn(&Object, Sel, id),
        );
        decl.register() as *const _ as usize
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
extern "C" fn footer_run_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Run);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_actions_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Actions);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_ai_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Ai);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_apply_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Apply);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_replace_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Replace);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_append_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Append);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_copy_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Copy);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_expand_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Expand);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_retry_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Retry);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_close_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Close);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_stop_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Stop);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_paste_response_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::PasteResponse);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_cwd_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Cwd);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_agent_model_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::AgentModel);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_tips_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Tips);
}
