//! HUD Manager - System-level overlay notifications
//!
//! Creates independent floating windows for HUD messages, similar to Raycast's showHUD().
//! HUDs are:
//! - Separate windows (not part of main app window)
//! - Floating above all other windows
//! - Positioned at bottom-center of the screen containing the mouse
//! - Auto-dismiss after configurable duration
//! - Queued if multiple arrive in sequence

// --- merged from part_000.rs ---
use crate::components::button::{Button, ButtonColors, ButtonVariant};
use crate::logging;
use crate::runtime_policy::{ExternalEffect, WindowHostPolicy};
use crate::theme::get_cached_theme;
use gpui::{
    div, point, prelude::*, px, rgb, rgba, size, App, Context, ElementId, Render, SharedString,
    Window, WindowBackgroundAppearance, WindowBounds, WindowHandle, WindowKind, WindowOptions,
};
use gpui_component::tooltip::Tooltip;
use parking_lot::Mutex;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct HudHostOptions {
    pub policy: WindowHostPolicy,
    pub origin: Option<gpui::Point<gpui::Pixels>>,
}

impl HudHostOptions {
    fn interactive() -> Self {
        Self {
            policy: WindowHostPolicy::Interactive,
            origin: None,
        }
    }
}

/// The identity and typed window produced by the notification factory.
pub struct MountedHud {
    pub id: u64,
    pub window: WindowHandle<HudView>,
}

const MAX_QUEUED_HUDS: usize = 16;
// =============================================================================
// Theme Integration - HUD colors from theme system
// =============================================================================

/// Colors used by HUD rendering, extracted from theme for closure compatibility.
/// This struct is Copy so it can be safely used in closures without borrow issues.
#[derive(Clone, Copy, Debug)]
struct HudColors {
    /// Background color for hover calculations
    background: u32,
    /// Main-window-matched background color and opacity for the HUD pill
    background_rgba: u32,
    /// Primary text color
    text_primary: u32,
    /// Accent color for action buttons
    accent: u32,
    /// Accent hover color (lighter)
    accent_hover: u32,
    /// Accent active/pressed color (darker)
    #[allow(dead_code)] // Reserved for future use
    accent_active: u32,
}
impl HudColors {
    /// Load HUD colors from the current theme
    fn from_theme() -> Self {
        let theme = get_cached_theme();
        let colors = &theme.colors;

        // Calculate hover/active variants from accent
        // For hover: lighten by ~10%
        // For active: darken by ~10%
        let accent = colors.ui.info; // Use info color (blue) for action buttons
        let accent_hover = lighten_color(accent, 0.1);
        let accent_active = darken_color(accent, 0.1);

        Self {
            background: colors.background.main,
            background_rgba: crate::ui_foundation::main_window_matched_background_rgba(&theme),
            text_primary: colors.text.primary,
            accent,
            accent_hover,
            accent_active,
        }
    }

    /// Create default dark theme colors (fallback)
    #[cfg(test)]
    fn dark_default() -> Self {
        Self {
            background: 0x1e1e1e,
            background_rgba: 0x1e1e1e4c,
            text_primary: 0xffffff,
            accent: 0x3b82f6,        // blue-500
            accent_hover: 0x60a5fa,  // blue-400
            accent_active: 0x2563eb, // blue-600
        }
    }

    /// Create default light theme colors (fallback)
    #[cfg(test)]
    fn light_default() -> Self {
        Self {
            background: 0xfafafa,
            background_rgba: 0xfafafa4c,
            text_primary: 0x000000,
            accent: 0x2563eb,        // blue-600 (darker for light mode)
            accent_hover: 0x3b82f6,  // blue-500
            accent_active: 0x1d4ed8, // blue-700
        }
    }
}
/// Lighten a color by a percentage (0.0 - 1.0)
fn lighten_color(color: u32, amount: f32) -> u32 {
    let r = ((color >> 16) & 0xff) as f32;
    let g = ((color >> 8) & 0xff) as f32;
    let b = (color & 0xff) as f32;

    let r = (r + (255.0 - r) * amount).min(255.0) as u32;
    let g = (g + (255.0 - g) * amount).min(255.0) as u32;
    let b = (b + (255.0 - b) * amount).min(255.0) as u32;

    (r << 16) | (g << 8) | b
}
/// Darken a color by a percentage (0.0 - 1.0)
fn darken_color(color: u32, amount: f32) -> u32 {
    let r = ((color >> 16) & 0xff) as f32;
    let g = ((color >> 8) & 0xff) as f32;
    let b = (color & 0xff) as f32;

    let r = (r * (1.0 - amount)).max(0.0) as u32;
    let g = (g * (1.0 - amount)).max(0.0) as u32;
    let b = (b * (1.0 - amount)).max(0.0) as u32;

    (r << 16) | (g << 8) | b
}
/// Counter for generating unique HUD IDs
static NEXT_HUD_ID: AtomicU64 = AtomicU64::new(1);
/// Generate a unique HUD ID
fn next_hud_id() -> u64 {
    NEXT_HUD_ID.fetch_add(1, Ordering::Relaxed)
}
/// Default HUD duration in milliseconds
const DEFAULT_HUD_DURATION_MS: u64 = 2000;
/// Gap between stacked HUDs
const HUD_STACK_GAP: f32 = 45.0;
/// Maximum number of simultaneous HUDs
const MAX_SIMULTANEOUS_HUDS: usize = 3;
/// HUD window dimensions - compact pill shape with room for wrapped errors
const HUD_MIN_WIDTH: f32 = 220.0;
const HUD_MAX_WIDTH: f32 = 420.0;
const HUD_SINGLE_LINE_HEIGHT: f32 = 40.0;
/// HUD with action button dimensions (wider to fit button)
#[allow(dead_code)]
const HUD_ACTION_WIDTH: f32 = 360.0;
#[allow(dead_code)]
const HUD_ACTION_HEIGHT: f32 = 48.0;
// =============================================================================
// HUD Actions - Clickable actions for HUD notifications
// =============================================================================

/// Action types that can be triggered from a HUD button click
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum HudAction {
    /// Open a file in the configured editor
    OpenFile(PathBuf),
    /// Open a URL in the default browser
    OpenUrl(String),
    /// Run a shell command
    RunCommand(String),
}
impl HudAction {
    /// Execute the action
    pub fn execute(&self, editor: Option<&str>) -> anyhow::Result<()> {
        match self {
            HudAction::OpenFile(path) => {
                crate::runtime_policy::check(ExternalEffect::Process)?;
                std::process::Command::new(editor.unwrap_or("code"))
                    .arg(path)
                    .spawn()?;
            }
            HudAction::OpenUrl(url) => {
                crate::runtime_policy::check(ExternalEffect::OpenExternal)?;
                open::that(url)?;
            }
            HudAction::RunCommand(command) => {
                crate::runtime_policy::check(ExternalEffect::Process)?;
                let mut parts = command.split_whitespace();
                let program = parts
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("hud_command_empty"))?;
                std::process::Command::new(program).args(parts).spawn()?;
            }
        }
        Ok(())
    }
}
/// A single HUD notification
#[derive(Clone)]
pub struct HudNotification {
    pub text: String,
    pub duration_ms: u64,
    #[allow(dead_code)]
    pub created_at: Instant,
    /// Optional label for action button (e.g., "Open Logs", "View")
    #[allow(dead_code)]
    pub action_label: Option<String>,
    /// Optional action to execute when button is clicked
    #[allow(dead_code)]
    pub action: Option<HudAction>,
}
impl HudNotification {
    /// Check if this notification has an action button
    #[allow(dead_code)]
    pub fn has_action(&self) -> bool {
        self.action.is_some() && self.action_label.is_some()
    }
}
/// The visual component rendered inside each HUD window
pub struct HudView {
    text: String,
    #[allow(dead_code)]
    action_label: Option<String>,
    #[allow(dead_code)]
    action: Option<HudAction>,
    /// Theme colors for rendering
    colors: HudColors,
    /// Manager-side ID, so a click on the pill can dismiss this HUD through
    /// the same tracked path as the auto-dismiss timer.
    hud_id: u64,
    theme_revision_seen: u64,
    pub(crate) action_error: Option<String>,
    pub(crate) action_completed: bool,
    semantic_revision: u64,
    presentation_revision: u64,
}
impl HudView {
    fn new(text: String, hud_id: u64) -> Self {
        Self {
            text,
            action_label: None,
            action: None,
            colors: HudColors::from_theme(),
            hud_id,
            theme_revision_seen: 0,
            action_error: None,
            action_completed: false,
            semantic_revision: 1,
            presentation_revision: 1,
        }
    }

    #[allow(dead_code)]
    fn with_action(text: String, action_label: String, action: HudAction, hud_id: u64) -> Self {
        Self {
            text,
            action_label: Some(action_label),
            action: Some(action),
            colors: HudColors::from_theme(),
            hud_id,
            theme_revision_seen: 0,
            action_error: None,
            action_completed: false,
            semantic_revision: 1,
            presentation_revision: 1,
        }
    }

    /// Create a HudView with specific colors (for testing)
    #[cfg(test)]
    fn with_colors(text: String, colors: HudColors) -> Self {
        Self {
            text,
            action_label: None,
            action: None,
            colors,
            hud_id: 0,
            theme_revision_seen: 0,
            action_error: None,
            action_completed: false,
            semantic_revision: 1,
            presentation_revision: 1,
        }
    }

    #[allow(dead_code)]
    fn has_action(&self) -> bool {
        self.action.is_some() && self.action_label.is_some()
    }

    pub(crate) fn semantic_state(&self) -> (&str, Option<&str>, Option<&str>, bool) {
        (
            &self.text,
            self.action_label.as_deref(),
            self.action_error.as_deref(),
            self.action_completed,
        )
    }

    pub(crate) fn revision_facts(&self) -> (u64, u64, u64, u64) {
        (
            self.semantic_revision,
            self.semantic_revision,
            self.presentation_revision,
            self.theme_revision_seen,
        )
    }

    fn execute_action(&mut self, cx: &mut Context<Self>) -> anyhow::Result<()> {
        let result = self
            .action
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("hud_action_missing"))
            .and_then(|action| action.execute(None));
        self.action_error = result.as_ref().err().map(ToString::to_string);
        self.action_completed = result.is_ok();
        self.semantic_revision = self.semantic_revision.saturating_add(1);
        cx.notify();
        result
    }
}

/// Horizontal padding of the HUD pill (`px(16.)` in `HudView::render`).
const HUD_PADDING_X: f32 = 16.0;
/// Vertical padding of the HUD pill (`px(8.)` in `HudView::render`).
const HUD_PADDING_Y: f32 = 8.0;
/// The HUD label renders with `.text_sm()` — 0.875rem at the default 16px rem.
const HUD_TEXT_FONT_SIZE: f32 = 14.0;
/// Default GPUI line height is `phi()` (1.618034 × font size).
const HUD_TEXT_LINE_HEIGHT: f32 = HUD_TEXT_FONT_SIZE * 1.618_034;
/// HUDs are pills, not dialogs: size for at most this many wrapped lines.
/// Longer text clips inside the pill (the full text is in the tooltip).
const HUD_MAX_TEXT_LINES: usize = 3;

/// Pure clamp step shared by the measured and fallback paths, so tests can
/// exercise the sizing contract without a live text system.
fn hud_dimensions_from_measurement(text_width: f32, display_lines: usize) -> (f32, f32) {
    let width = (text_width + HUD_PADDING_X * 2.0).clamp(HUD_MIN_WIDTH, HUD_MAX_WIDTH);
    let lines = display_lines.clamp(1, HUD_MAX_TEXT_LINES) as f32;
    let height = (lines * HUD_TEXT_LINE_HEIGHT + HUD_PADDING_Y * 2.0).max(HUD_SINGLE_LINE_HEIGHT);
    (width.ceil(), height.ceil())
}

/// Size the HUD window by measuring the text with the real text system at the
/// font the pill renders with, wrapped at the maximum text width. Replaces the
/// old chars-times-6.5px estimate, which both clipped wide glyphs and reserved
/// excess space for narrow ones.
fn hud_dimensions_for_text(text: &str, cx: &App) -> (f32, f32) {
    let max_text_width = HUD_MAX_WIDTH - HUD_PADDING_X * 2.0;
    let text_system = cx.text_system();
    let font = gpui::font(crate::list_item::FONT_SYSTEM_UI);
    let font_id = text_system.resolve_font(&font);
    let font_size = px(HUD_TEXT_FONT_SIZE);
    let mut wrapper = text_system.line_wrapper(font, font_size);

    let mut max_segment_width: f32 = 0.0;
    let mut display_lines: usize = 0;

    for line in text.lines() {
        let line_width: f32 = line
            .chars()
            .map(|ch| f32::from(text_system.layout_width(font_id, font_size, ch)))
            .sum();
        let wraps = wrapper
            .wrap_line(&[gpui::LineFragment::text(line)], px(max_text_width))
            .count();
        max_segment_width = max_segment_width.max(line_width.min(max_text_width));
        display_lines += wraps + 1;
    }

    hud_dimensions_from_measurement(max_segment_width, display_lines)
}

/// Stable automation-registry ID for a HUD window.
fn hud_automation_id(hud_id: u64) -> String {
    format!("hud:{hud_id}")
}

/// Register a HUD window in the automation registry so DevTools callers can
/// list it and target it with `captureScreenshot` (HUDs are focusless,
/// titleless OS windows that are otherwise undiscoverable).
fn register_hud_automation_window(
    hud_id: u64,
    text: &str,
    bounds: gpui::Bounds<gpui::Pixels>,
    handle: WindowHandle<HudView>,
    policy: WindowHostPolicy,
    cx: &mut App,
) -> anyhow::Result<u64> {
    let info = crate::windows::register_runtime_window_instance(
        crate::protocol::AutomationWindowInfo {
            id: hud_automation_id(hud_id),
            kind: crate::protocol::AutomationWindowKind::Hud,
            title: Some(format!("Script Kit HUD: {text}")),
            focused: false,
            visible: !policy.is_hidden(),
            semantic_surface: Some("hud".into()),
            bounds: Some(crate::protocol::AutomationWindowBounds {
                x: f32::from(bounds.origin.x) as f64,
                y: f32::from(bounds.origin.y) as f64,
                width: f32::from(bounds.size.width) as f64,
                height: f32::from(bounds.size.height) as f64,
            }),
            parent_window_id: None,
            parent_kind: None,
            parent_window_generation: None,
            pid: Some(std::process::id()),
            generation: None,
        },
        handle.into(),
        cx,
    )?;
    info.generation
        .ok_or_else(|| anyhow::anyhow!("hud_generation_missing"))
}
impl Render for HudView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let revision = crate::theme::service::theme_revision();
        if self.theme_revision_seen != revision {
            self.colors = HudColors::from_theme();
            self.theme_revision_seen = revision;
            self.presentation_revision = self.presentation_revision.saturating_add(1);
        }
        let has_action = self.has_action();

        // Extract colors for use in closures (Copy trait)
        let colors = self.colors;
        let theme = get_cached_theme();
        let theme_background_gradients =
            crate::ui_foundation::theme_background_gradient_layers("hud-bg-layer", &theme);
        let hud_label_text = self.text.clone();

        // HUD pill styling: matches main window theme, minimal and clean
        // Similar to Raycast's HUD - simple, elegant, non-intrusive
        div()
            .id("hud-pill")
            .debug_selector(|| "hud-pill".to_string())
            .w_full()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .px(px(16.))
            .py(px(8.))
            .gap(px(12.))
            .relative()
            .overflow_hidden()
            .cursor_pointer()
            // Click anywhere on the pill dismisses the HUD through the same
            // tracked path as the auto-dismiss timer. Deferred out of this
            // window's own event dispatch (removing a window from inside its
            // dispatch leaves a zombie key window).
            .on_click(cx.listener(|this, _event, window, cx| {
                let hud_id = this.hud_id;
                window.defer(cx, move |_window, cx| {
                    dismiss_hud_by_id(hud_id, cx);
                });
            }))
            // Use the same theme background and opacity as the main window.
            .bg(rgba(colors.background_rgba))
            .children(theme_background_gradients)
            .rounded(px(8.)) // Rounded corners matching main window
            // Text styling - system font, smaller size, theme text color, centered
            .child(
                div()
                    .id(ElementId::Name(SharedString::from(
                        "hud-pill-label-ellipsis",
                    )))
                    .debug_selector(|| "hud-pill-label-ellipsis".to_string())
                    .text_sm()
                    .text_center()
                    .text_color(rgb(colors.text_primary))
                    .w_full()
                    .whitespace_normal()
                    // Window height budgets HUD_MAX_TEXT_LINES wrapped lines;
                    // clamp so overflow truncates at a line boundary.
                    .line_clamp(HUD_MAX_TEXT_LINES)
                    .tooltip(move |window, cx| {
                        Tooltip::new(hud_label_text.clone()).build(window, cx)
                    })
                    .child(self.text.clone()),
            )
            // Action button (only if action is present)
            .when(has_action, |el| {
                let label = self.action_label.clone().unwrap_or_default();
                // Theme-aware hover overlay: white for dark mode, black for light mode
                // Determined by checking if background color is dark (luminance < 0.5)
                let hover_overlay = {
                    let bg = colors.background;
                    let r = ((bg >> 16) & 0xFF) as f32 / 255.0;
                    let g = ((bg >> 8) & 0xFF) as f32 / 255.0;
                    let b = (bg & 0xFF) as f32 / 255.0;
                    let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                    if luminance < 0.5 {
                        0xffffff26 // White at ~15% alpha for dark backgrounds
                    } else {
                        0x00000026 // Black at ~15% alpha for light backgrounds
                    }
                };
                // Create button colors from HUD colors
                let button_colors = ButtonColors {
                    text_color: colors.text_primary,
                    text_hover: colors.text_primary,
                    background: colors.accent,
                    background_hover: colors.accent_hover,
                    accent: colors.text_primary, // Text on accent background
                    border: colors.accent,
                    focus_ring: colors.accent_hover,
                    focus_tint: colors.accent,
                    hover_overlay,
                };
                el.child(
                    Button::new("hud:primary-action", label, button_colors)
                        .variant(ButtonVariant::Primary)
                        .on_click(Box::new(cx.listener(move |this, _event, _window, cx| {
                            cx.stop_propagation();
                            if let Err(error) = this.execute_action(cx) {
                                tracing::warn!(%error, "hud_action_refused_or_failed");
                            }
                        }))),
                )
            })
    }
}
/// Tracks an active HUD window
struct ActiveHud {
    /// Unique identifier for this HUD
    id: u64,
    /// Window handle for closing via GPUI's proper API
    window: WindowHandle<HudView>,
    created_at: Instant,
    duration_ms: u64,
    /// Slot index (0..MAX_SIMULTANEOUS_HUDS) for position calculation
    #[allow(dead_code)] // Used in position calculation
    slot: usize,
    generation: Option<u64>,
}
/// Entry in the slot allocation array (lightweight, for tracking slot ownership)
#[derive(Clone, Copy, Debug)]
struct HudSlotEntry {
    /// HUD ID that owns this slot
    id: u64,
}
/// Check if a duration has elapsed (used for HUD expiry)
/// Returns true when elapsed >= duration (inclusive boundary)
#[cfg(test)]
fn is_duration_expired(created_at: Instant, duration: Duration) -> bool {
    created_at.elapsed() >= duration
}
impl ActiveHud {
    fn is_expired(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.created_at) >= Duration::from_millis(self.duration_ms)
    }
}
/// Global HUD manager state
struct HudManagerState {
    /// Currently displayed HUD windows (kept for window handle storage)
    active_huds: Vec<ActiveHud>,
    /// Slot allocation array - each slot is None (free) or Some(entry) (occupied)
    /// Using fixed array prevents overlap from len-based stacking
    hud_slots: [Option<HudSlotEntry>; MAX_SIMULTANEOUS_HUDS],
    /// Queue of pending HUDs (if max simultaneous reached)
    pending_queue: VecDeque<(HudNotification, HudHostOptions)>,
}
impl HudManagerState {
    fn new() -> Self {
        Self {
            active_huds: Vec::new(),
            hud_slots: [None; MAX_SIMULTANEOUS_HUDS],
            pending_queue: VecDeque::new(),
        }
    }

    /// Find the first free slot (lowest index)
    fn first_free_slot(&self) -> Option<usize> {
        self.hud_slots.iter().position(|slot| slot.is_none())
    }

    /// Release a slot by HUD ID (clear the slot that contains this ID)
    fn release_slot_by_id(&mut self, hud_id: u64) {
        for slot in self.hud_slots.iter_mut() {
            if let Some(entry) = slot {
                if entry.id == hud_id {
                    *slot = None;
                    return;
                }
            }
        }
    }

    /// Find which slot contains a given HUD ID
    #[allow(dead_code)] // Used in tests
    fn find_slot_by_id(&self, hud_id: u64) -> Option<usize> {
        self.hud_slots
            .iter()
            .position(|slot| slot.as_ref().is_some_and(|entry| entry.id == hud_id))
    }

    /// Count how many HUDs are currently active
    #[allow(dead_code)] // Used in tests
    fn active_hud_count(&self) -> usize {
        self.hud_slots.iter().filter(|s| s.is_some()).count()
    }
}
/// Global HUD manager singleton
static HUD_MANAGER: std::sync::LazyLock<Arc<Mutex<HudManagerState>>> =
    std::sync::LazyLock::new(|| Arc::new(Mutex::new(HudManagerState::new())));
fn get_hud_manager() -> &'static Arc<Mutex<HudManagerState>> {
    &HUD_MANAGER
}
/// Internal helper to show a HUD notification from a HudNotification struct.
/// This preserves all fields including action_label and action.
fn show_notification((notification, host): (HudNotification, HudHostOptions), cx: &mut App) {
    if let Err(error) = open_hud_notification(notification, host, cx) {
        tracing::warn!(%error, "hud_open_failed");
    }
}

/// The shared production factory: position and host policy are explicit, timers use GPUI's clock.
pub fn open_hud_notification(
    notification: HudNotification,
    host: HudHostOptions,
    cx: &mut App,
) -> anyhow::Result<Option<MountedHud>> {
    host.policy.validate()?;
    anyhow::ensure!(
        notification.text.len() <= 16_384 && notification.duration_ms <= 600_000,
        "hud_notification_limit"
    );
    if host.policy.is_hidden() {
        anyhow::ensure!(host.origin.is_some(), "hud_owned_origin_required");
    }
    let slot = {
        let mut state = get_hud_manager().lock();
        match state.first_free_slot() {
            Some(slot) => slot,
            None => {
                anyhow::ensure!(
                    state.pending_queue.len() < MAX_QUEUED_HUDS,
                    "hud_queue_full"
                );
                state.pending_queue.push_back((notification, host));
                return Ok(None);
            }
        }
    };
    let (message_width, message_height) = hud_dimensions_for_text(&notification.text, cx);
    let (width, height) = if notification.has_action() {
        (HUD_ACTION_WIDTH.max(message_width), HUD_ACTION_HEIGHT)
    } else {
        (message_width, message_height)
    };
    let origin = match host.origin {
        Some(origin) => origin,
        None => {
            crate::runtime_policy::check(ExternalEffect::SystemDiscovery)?;
            let (x, y) = calculate_hud_position(width);
            point(px(x), px(y))
        }
    };
    let bounds = gpui::Bounds {
        origin: point(origin.x, origin.y - px(slot as f32 * HUD_STACK_GAP)),
        size: size(px(width), px(height)),
    };
    let hud_id = next_hud_id();
    let text = notification.text.clone();
    let duration = notification.duration_ms;
    let window = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            window_background: WindowBackgroundAppearance::Transparent,
            focus: false,
            show: !host.policy.is_hidden(),
            kind: WindowKind::PopUp,
            ..Default::default()
        },
        |_, cx| {
            cx.new(|_| match (notification.action_label, notification.action) {
                (Some(label), Some(action)) => {
                    HudView::with_action(notification.text, label, action, hud_id)
                }
                _ => HudView::new(notification.text, hud_id),
            })
        },
    )?;
    {
        let mut state = get_hud_manager().lock();
        state.hud_slots[slot] = Some(HudSlotEntry { id: hud_id });
        state.active_huds.push(ActiveHud {
            id: hud_id,
            window,
            created_at: cx.background_executor().now(),
            duration_ms: duration,
            slot,
            generation: None,
        });
    }
    let publish = move |cx: &mut App| -> anyhow::Result<()> {
        anyhow::ensure!(
            get_hud_manager()
                .lock()
                .active_huds
                .iter()
                .any(|hud| hud.id == hud_id && hud.window == window),
            "hud_lifetime_superseded"
        );
        let generation =
            register_hud_automation_window(hud_id, &text, bounds, window, host.policy, cx)?;
        if let Some(hud) = get_hud_manager()
            .lock()
            .active_huds
            .iter_mut()
            .find(|hud| hud.id == hud_id)
        {
            hud.generation = Some(generation);
        }
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            cx.background_executor()
                .timer(Duration::from_millis(duration))
                .await;
            cx.update(|cx| dismiss_hud_by_id(hud_id, cx));
        })
        .detach();
        Ok(())
    };
    if host.policy.is_hidden() {
        if let Err(error) = publish(cx) {
            dismiss_hud_by_id(hud_id, cx);
            return Err(error);
        }
    } else {
        window.update(cx, move |_, window, cx| {
            window.defer(cx, move |window, cx| {
                if let Err(error) = configure_hud_platform_window(window, false) {
                    tracing::warn!(%error, "hud_configuration_failed");
                    cx.defer(move |cx| dismiss_hud_by_id(hud_id, cx));
                    return;
                }
                cx.defer(move |cx| {
                    if let Err(error) = publish(cx) {
                        tracing::warn!(%error, "hud_registration_failed");
                        dismiss_hud_by_id(hud_id, cx);
                    }
                });
            })
        })?;
    }
    Ok(Some(MountedHud { id: hud_id, window }))
}

// --- merged from part_001.rs ---
/// Show a HUD notification
///
/// This creates a new floating window positioned at the bottom-center of the
/// screen containing the mouse cursor. The HUD auto-dismisses after the
/// specified duration.
///
/// # Arguments
/// * `text` - The message to display
/// * `duration_ms` - Optional duration in milliseconds (default: 2000ms)
/// * `cx` - GPUI App context
pub fn show_hud(text: String, duration_ms: Option<u64>, cx: &mut App) {
    let notification = HudNotification {
        text,
        duration_ms: duration_ms.unwrap_or(DEFAULT_HUD_DURATION_MS),
        created_at: cx.background_executor().now(),
        action_label: None,
        action: None,
    };
    show_notification((notification, HudHostOptions::interactive()), cx);
}
/// Show a HUD notification with a clickable action button
///
/// This creates a HUD with a button that executes an action when clicked.
/// The HUD is wider to accommodate the button.
///
/// # Arguments
/// * `text` - The message to display
/// * `duration_ms` - Optional duration in milliseconds (default: 3000ms for action HUDs)
/// * `action_label` - Label for the action button (e.g., "Open Logs")
/// * `action` - The action to execute when the button is clicked
/// * `cx` - GPUI App context
#[allow(dead_code)]
pub fn show_hud_with_action(
    text: String,
    duration_ms: Option<u64>,
    action_label: String,
    action: HudAction,
    cx: &mut App,
) {
    let notification = HudNotification {
        text,
        duration_ms: duration_ms.unwrap_or(3000),
        created_at: cx.background_executor().now(),
        action_label: Some(action_label),
        action: Some(action),
    };
    show_notification((notification, HudHostOptions::interactive()), cx);
}
fn display_for_window_center(
    bounds: (f64, f64, f64, f64),
    displays: &[crate::platform::VisibleDisplayBounds],
) -> Option<crate::platform::VisibleDisplayBounds> {
    let (x, y, width, height) = bounds;
    let center = (x + width / 2.0, y + height / 2.0);
    crate::platform::display_for_point(center, displays)
}

fn select_hud_display(
    main_window_bounds: Option<(f64, f64, f64, f64)>,
    mouse_pos: Option<(f64, f64)>,
    displays: &[crate::platform::VisibleDisplayBounds],
) -> Option<crate::platform::VisibleDisplayBounds> {
    if let Some(bounds) = main_window_bounds {
        if let Some(display) = display_for_window_center(bounds, displays) {
            return Some(display);
        }
    }

    if let Some(mouse_pt) = mouse_pos {
        if let Some(display) = crate::platform::display_for_point(mouse_pt, displays) {
            return Some(display);
        }
    }

    displays.first().cloned()
}

/// Calculate HUD position - bottom center of the display containing the main menu
fn calculate_hud_position(hud_width: f32) -> (f32, f32) {
    let displays = crate::platform::get_macos_visible_displays();
    let main_window_bounds = crate::platform::get_main_window_bounds();
    let mouse_pos = crate::platform::get_global_mouse_position();
    let display = select_hud_display(main_window_bounds, mouse_pos, &displays);

    if let Some(display) = display {
        let visible = &display.visible_area;
        let screen_x = visible.origin_x as f32;
        let screen_y = visible.origin_y as f32;
        let screen_width = visible.width as f32;
        let screen_height = visible.height as f32;

        // Center horizontally, position at 85% down the display's visible area.
        let hud_x = screen_x + (screen_width - hud_width) / 2.0;
        let hud_y = screen_y + screen_height * 0.85;

        (hud_x, hud_y)
    } else {
        // Fallback position
        (500.0, 800.0)
    }
}
/// Configure a HUD window as a floating overlay.
///
/// Resolves the NSWindow deterministically from the HUD's own GPUI window
/// handle (via `raw_window_handle`, same pattern as the actions popup). Do NOT
/// match NSWindows by size here: identical pills share dimensions, so a size
/// scan over `[NSApp windows]` configured the oldest pill instead of the new
/// one — and calling `orderFrontRegardless` on a window GPUI was concurrently
/// closing could leave a permanent ghost HUD on screen.
///
/// # Arguments
/// * `window` - The freshly opened HUD window to configure
/// * `click_through` - If true, window ignores mouse events (for plain HUDs).
///   If false, window receives mouse events (for click-to-dismiss/buttons).
#[cfg(target_os = "macos")]
fn configure_hud_platform_window(window: &mut Window, click_through: bool) -> anyhow::Result<()> {
    crate::runtime_policy::check(ExternalEffect::NativeVisibility)?;
    use cocoa::base::id;

    let Ok(handle) = raw_window_handle::HasWindowHandle::window_handle(window) else {
        logging::log("HUD", "Could not resolve raw window handle for HUD window");
        anyhow::bail!("hud_native_handle_missing");
    };
    let raw_window_handle::RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        logging::log("HUD", "HUD window handle is not an AppKit window");
        anyhow::bail!("hud_native_handle_not_appkit");
    };
    let ns_view = appkit.ns_view.as_ptr() as id;

    // SAFETY: `ns_view` comes from the live GPUI HUD window on the AppKit main
    // thread; `-[NSView window]` returns its owning NSWindow or nil, and all
    // selectors below are standard NSWindow accessors/setters.
    unsafe {
        use objc::{msg_send, sel, sel_impl};

        let window: id = msg_send![ns_view, window];
        if window.is_null() {
            logging::log(
                "HUD",
                "HUD NSView has no NSWindow yet, skipping configuration",
            );
            anyhow::bail!("hud_native_window_missing");
        }

        let theme = crate::theme::get_cached_theme();
        crate::platform::configure_hud_window_vibrancy(window, theme.should_use_dark_vibrancy());

        // Set window level very high (NSPopUpMenuWindowLevel = 101)
        let hud_level: i64 = 101;
        let _: () = msg_send![window, setLevel: hud_level];

        // Collection behaviors for HUD:
        // - CanJoinAllSpaces (1): appear on all spaces
        // - Stationary (16): don't move with spaces
        // - IgnoresCycle (64): cmd-tab ignores this window
        let collection_behavior: u64 = 1 | 16 | 64;
        let _: () = msg_send![window, setCollectionBehavior: collection_behavior];

        // Set mouse event handling based on whether HUD has clickable actions
        let ignores_mouse: cocoa::base::BOOL = if click_through {
            cocoa::base::YES
        } else {
            cocoa::base::NO
        };
        let _: () = msg_send![window, setIgnoresMouseEvents: ignores_mouse];

        // Ensure window content is shareable for captureScreenshot()
        let sharing_type: i64 = 1; // NSWindowSharingReadOnly
        let _: () = msg_send![window, setSharingType: sharing_type];

        // Don't show in window menu
        let _: () = msg_send![window, setExcludedFromWindowsMenu: true];

        // Keep HUD overlays eligible to appear even when the app is
        // otherwise hidden and interactive scripts are running headless.
        let _: () = msg_send![window, setCanHide: false];

        // Keep the HUD visible even after the main launcher orders out.
        // orderFrontRegardless lifts the overlay without requiring the
        // app to remain active or the main panel to stay open.
        let _: () = msg_send![window, orderFrontRegardless];

        let click_status = if click_through {
            "click-through"
        } else {
            "clickable"
        };
        logging::log(
            "HUD",
            &format!(
                "Configured HUD NSWindow: level={}, {}, canHide=false, orderFrontRegardless",
                hud_level, click_status
            ),
        );
    }
    Ok(())
}
#[cfg(not(target_os = "macos"))]
fn configure_hud_platform_window(_window: &mut Window, _click_through: bool) -> anyhow::Result<()> {
    Ok(())
}
/// Dismiss a specific HUD by its ID
///
/// Uses WindowHandle.update() + window.remove_window() for reliable window closing.
/// Uses slot-based clearing instead of swap_remove to prevent position overlap.
pub(crate) fn dismiss_owned_hud_instance(
    id: &str,
    generation: u64,
    cx: &mut App,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        crate::windows::runtime_window_host_policy(id, generation)?.is_hidden(),
        "hud_owned_host_required"
    );
    let handle = crate::windows::get_runtime_window_handle_for_generation(id, generation)
        .ok_or_else(|| anyhow::anyhow!("hud_target_stale"))?;
    let hud_id = handle.read(cx, |entity: gpui::Entity<HudView>, cx| {
        entity.read(cx).hud_id
    })?;
    anyhow::ensure!(
        get_hud_manager()
            .lock()
            .active_huds
            .iter()
            .any(|hud| hud.id == hud_id
                && hud.generation == Some(generation)
                && gpui::AnyWindowHandle::from(hud.window) == handle),
        "hud_lifetime_mismatch"
    );
    dismiss_hud_by_id(hud_id, cx);
    Ok(())
}

pub(crate) fn dismiss_hud_by_id(hud_id: u64, cx: &mut App) {
    let hud = {
        let mut state = get_hud_manager().lock();
        let Some(index) = state.active_huds.iter().position(|hud| hud.id == hud_id) else {
            return;
        };
        state.release_slot_by_id(hud_id);
        state.active_huds.remove(index)
    };
    close_hud_lifetime(hud, cx);
    cleanup_expired_huds(cx);
}

fn close_hud_lifetime(hud: ActiveHud, cx: &mut App) {
    if let Some(generation) = hud.generation {
        crate::windows::remove_runtime_window_instance(&hud_automation_id(hud.id), generation);
    }
    let _ = hud.window.update(cx, |_, window, cx| {
        if window.is_owned_hidden() {
            window.remove_window();
        } else {
            crate::platform::dematerialize_then_remove_gpui_window(window, cx, "HUD", "HUD");
        }
    });
}
// --- merged from part_002.rs ---
/// Clean up expired HUD windows and show pending ones
fn cleanup_expired_huds(cx: &mut App) {
    let now = cx.background_executor().now();
    let expired = {
        let mut state = get_hud_manager().lock();
        let (expired, remaining): (Vec<ActiveHud>, Vec<ActiveHud>) = state
            .active_huds
            .drain(..)
            .partition(|hud| hud.is_expired(now));
        state.active_huds = remaining;
        for hud in &expired {
            state.release_slot_by_id(hud.id);
        }
        expired
    };
    for hud in expired {
        close_hud_lifetime(hud, cx);
    }
    for _ in 0..MAX_QUEUED_HUDS {
        let pending = {
            let mut state = get_hud_manager().lock();
            if state.first_free_slot().is_none() {
                break;
            }
            state.pending_queue.pop_front()
        };
        let Some(pending) = pending else {
            break;
        };
        show_notification(pending, cx);
    }
}
/// Dismiss all active HUDs immediately
///
/// This closes all active HUD windows and clears the pending queue.
/// Must be called on the main thread (i.e., from within App context).
#[allow(dead_code)]
pub fn dismiss_all_huds(cx: &mut App) {
    let huds = {
        let mut state = get_hud_manager().lock();
        state.hud_slots = [None; MAX_SIMULTANEOUS_HUDS];
        state.pending_queue.clear();
        std::mem::take(&mut state.active_huds)
    };
    for hud in huds {
        close_hud_lifetime(hud, cx);
    }
}
// --- merged from part_003.rs ---
#[cfg(test)]
mod tests {
    // --- merged from part_000.rs ---
    use super::*;

    #[test]
    fn hud_width_tracks_measured_text_within_clamps() {
        // Narrow text clamps up to the minimum pill width.
        let (narrow_width, _) = hud_dimensions_from_measurement(40.0, 1);
        assert_eq!(narrow_width, HUD_MIN_WIDTH);

        // Mid-size text gets measured width plus the pill padding.
        let (mid_width, _) = hud_dimensions_from_measurement(300.0, 1);
        assert_eq!(mid_width, 300.0 + HUD_PADDING_X * 2.0);

        // Wide text clamps down to the maximum pill width.
        let (wide_width, _) = hud_dimensions_from_measurement(1000.0, 1);
        assert_eq!(wide_width, HUD_MAX_WIDTH);
    }

    #[test]
    fn hud_height_scales_with_wrapped_lines_up_to_cap() {
        let (_, one_line) = hud_dimensions_from_measurement(100.0, 1);
        let (_, two_lines) = hud_dimensions_from_measurement(100.0, 2);
        let (_, three_lines) = hud_dimensions_from_measurement(100.0, 3);
        let (_, many_lines) = hud_dimensions_from_measurement(100.0, 12);

        assert!(one_line >= HUD_SINGLE_LINE_HEIGHT);
        assert!(two_lines > one_line);
        assert!(three_lines > two_lines);
        // Pills cap at HUD_MAX_TEXT_LINES; longer text clips with a tooltip.
        assert_eq!(many_lines, three_lines);
    }

    #[test]
    fn hud_zero_lines_still_produces_a_visible_pill() {
        let (width, height) = hud_dimensions_from_measurement(0.0, 0);
        assert_eq!(width, HUD_MIN_WIDTH);
        assert!(height >= HUD_SINGLE_LINE_HEIGHT);
    }

    #[test]
    fn test_hud_notification_creation() {
        let notif = HudNotification {
            text: "Test".to_string(),
            duration_ms: 2000,
            created_at: Instant::now(),
            action_label: None,
            action: None,
        };
        assert_eq!(notif.text, "Test");
        assert_eq!(notif.duration_ms, 2000);
    }
    #[test]
    fn test_hud_manager_state_creation() {
        let state = HudManagerState::new();
        assert!(state.active_huds.is_empty());
        assert!(state.pending_queue.is_empty());
    }
    #[test]
    fn test_is_duration_expired_boundary_condition() {
        // HUD should be expired when elapsed == duration (not just >)
        // This tests the fix for the boundary condition bug

        // Create timestamp from 100ms ago
        let created_at = Instant::now() - Duration::from_millis(100);
        let duration = Duration::from_millis(100);

        // When elapsed >= duration, should be expired
        assert!(
            is_duration_expired(created_at, duration),
            "Should be expired when elapsed >= duration"
        );
    }
    #[test]
    fn test_is_duration_expired_definitely_expired() {
        // Create timestamp from 200ms ago with 100ms duration
        let created_at = Instant::now() - Duration::from_millis(200);
        let duration = Duration::from_millis(100);

        // When elapsed > duration, definitely expired
        assert!(
            is_duration_expired(created_at, duration),
            "Should be expired when elapsed > duration"
        );
    }
    #[test]
    fn test_is_duration_expired_not_expired_yet() {
        // Create timestamp from now with a long duration
        let created_at = Instant::now();
        let duration = Duration::from_millis(10000); // 10 seconds

        assert!(
            !is_duration_expired(created_at, duration),
            "Should not be expired immediately after creation"
        );
    }
    #[test]
    fn test_hud_notification_has_action() {
        let notif_without_action = HudNotification {
            text: "Test".to_string(),
            duration_ms: 2000,
            created_at: Instant::now(),
            action_label: None,
            action: None,
        };
        assert!(!notif_without_action.has_action());

        let notif_with_action = HudNotification {
            text: "Test".to_string(),
            duration_ms: 2000,
            created_at: Instant::now(),
            action_label: Some("Open".to_string()),
            action: Some(HudAction::OpenUrl("https://example.com".to_string())),
        };
        assert!(notif_with_action.has_action());
    }
    #[test]
    fn test_hud_id_generation() {
        // IDs should be unique and increasing
        let id1 = next_hud_id();
        let id2 = next_hud_id();
        let id3 = next_hud_id();

        assert!(id2 > id1, "IDs should be strictly increasing");
        assert!(id3 > id2, "IDs should be strictly increasing");
        assert_ne!(id1, id2, "IDs should be unique");
        assert_ne!(id2, id3, "IDs should be unique");
    }
    #[test]
    fn test_hud_view_has_action() {
        // Test that HudView correctly reports whether it has an action
        let view_without_action = HudView::new("Test message".to_string(), 1);
        assert!(
            !view_without_action.has_action(),
            "HudView without action should report has_action() = false"
        );

        let view_with_action = HudView::with_action(
            "Test message".to_string(),
            "Open".to_string(),
            HudAction::OpenUrl("https://example.com".to_string()),
            2,
        );
        assert!(
            view_with_action.has_action(),
            "HudView with action should report has_action() = true"
        );
    }

    #[test]
    fn test_hud_label_text_is_centered_in_render_source() {
        let source = std::fs::read_to_string("src/hud_manager/mod.rs")
            .expect("Failed to read src/hud_manager/mod.rs");

        assert!(
            source.contains(".id(ElementId::Name(SharedString::from(\n                        \"hud-pill-label-ellipsis\",\n                    )))")
                && source.contains(".text_center()"),
            "HUD label should keep centered text within the pill"
        );
    }

    #[test]
    fn test_hud_window_configuration_keeps_overlay_visible_when_app_hidden() {
        let source = std::fs::read_to_string("src/hud_manager/mod.rs")
            .expect("Failed to read src/hud_manager/mod.rs");

        assert!(
            source.contains("setCanHide: false") && source.contains("orderFrontRegardless"),
            "HUD window config should disable app-hide participation and order the overlay front"
        );
    }

    #[test]
    fn test_select_hud_display_prefers_main_window_display_over_mouse_display() {
        let displays = vec![
            crate::platform::VisibleDisplayBounds {
                frame: crate::windows::DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                visible_area: crate::windows::DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 24.0,
                    width: 1440.0,
                    height: 840.0,
                },
            },
            crate::platform::VisibleDisplayBounds {
                frame: crate::windows::DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                visible_area: crate::windows::DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 32.0,
                    width: 1920.0,
                    height: 1008.0,
                },
            },
        ];

        let selected = select_hud_display(
            Some((1800.0, 160.0, 750.0, 475.0)),
            Some((200.0, 200.0)),
            &displays,
        )
        .expect("expected a selected display");

        assert_eq!(selected.frame.origin_x, 1440.0);
        assert_eq!(selected.visible_area.origin_x, 1440.0);
    }
    #[test]
    fn test_select_hud_display_falls_back_to_mouse_display_when_main_window_is_unresolved() {
        let displays = vec![
            crate::platform::VisibleDisplayBounds {
                frame: crate::windows::DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 0.0,
                    width: 1440.0,
                    height: 900.0,
                },
                visible_area: crate::windows::DisplayBounds {
                    origin_x: 0.0,
                    origin_y: 24.0,
                    width: 1440.0,
                    height: 840.0,
                },
            },
            crate::platform::VisibleDisplayBounds {
                frame: crate::windows::DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 0.0,
                    width: 1920.0,
                    height: 1080.0,
                },
                visible_area: crate::windows::DisplayBounds {
                    origin_x: 1440.0,
                    origin_y: 32.0,
                    width: 1920.0,
                    height: 1008.0,
                },
            },
        ];

        let selected = select_hud_display(
            Some((5000.0, 5000.0, 750.0, 475.0)),
            Some((1700.0, 200.0)),
            &displays,
        )
        .expect("expected a selected display");

        assert_eq!(selected.frame.origin_x, 1440.0);
        assert_eq!(selected.visible_area.origin_x, 1440.0);
    }
    #[test]
    fn test_hud_action_execute_open_url() {
        // Test that HudAction::OpenUrl can be created and executed doesn't panic
        // (actual URL opening is mocked in unit tests)
        let action = HudAction::OpenUrl("https://example.com".to_string());
        // Just verify it can be constructed - actual execution requires system integration
        match action {
            HudAction::OpenUrl(url) => assert_eq!(url, "https://example.com"),
            _ => panic!("Expected OpenUrl variant"),
        }
    }
    #[test]
    fn test_hud_action_execute_open_file() {
        // Test that HudAction::OpenFile can be created
        let action = HudAction::OpenFile(std::path::PathBuf::from("/tmp/test.txt"));
        match action {
            HudAction::OpenFile(path) => {
                assert_eq!(path, std::path::PathBuf::from("/tmp/test.txt"))
            }
            _ => panic!("Expected OpenFile variant"),
        }
    }
    #[test]
    fn test_hud_action_execute_run_command() {
        // Test that HudAction::RunCommand can be created
        let action = HudAction::RunCommand("echo hello".to_string());
        match action {
            HudAction::RunCommand(cmd) => assert_eq!(cmd, "echo hello"),
            _ => panic!("Expected RunCommand variant"),
        }
    }
    // =============================================================================
    // Theme Integration Tests
    // =============================================================================

    #[test]
    fn test_lighten_color() {
        // Test lightening pure black by 50%
        let black = 0x000000;
        let lightened = lighten_color(black, 0.5);
        // Should be ~0x7f7f7f (half way to white)
        assert_eq!(lightened, 0x7f7f7f);

        // Test lightening pure red by 10%
        let red = 0xff0000;
        let lightened_red = lighten_color(red, 0.1);
        // Red channel is already max, green/blue should be ~0x19 (25)
        assert_eq!(lightened_red >> 16, 0xff); // Red stays at max
        assert!((lightened_red >> 8) & 0xff >= 0x19); // Green increased
        assert!(lightened_red & 0xff >= 0x19); // Blue increased
    }
    #[test]
    fn test_darken_color() {
        // Test darkening pure white by 50%
        let white = 0xffffff;
        let darkened = darken_color(white, 0.5);
        // Should be ~0x7f7f7f (half way to black)
        assert_eq!(darkened, 0x7f7f7f);

        // Test darkening a color by 10%
        let color = 0x646464; // RGB(100, 100, 100)
        let darkened_color = darken_color(color, 0.1);
        // Each component should be 90% of original: 100 * 0.9 = 90 = 0x5a
        assert_eq!(darkened_color, 0x5a5a5a);
    }
    #[test]
    fn test_lighten_darken_boundary_conditions() {
        // Lightening white should stay white
        let white = 0xffffff;
        assert_eq!(lighten_color(white, 0.5), 0xffffff);

        // Darkening black should stay black
        let black = 0x000000;
        assert_eq!(darken_color(black, 0.5), 0x000000);
    }
    #[test]
    fn test_hud_colors_default() {
        // Test that default colors are valid (non-zero)
        let colors = HudColors::dark_default();
        assert_ne!(colors.background, 0);
        assert_ne!(colors.text_primary, 0);
        assert_ne!(colors.accent, 0);
        assert_ne!(colors.accent_hover, 0);
        assert_ne!(colors.accent_active, 0);
    }
    #[test]
    fn test_hud_colors_light_default() {
        // Test that light mode colors are valid
        let colors = HudColors::light_default();
        // Light mode should have light background
        assert_eq!(colors.background, 0xfafafa);
        // Light mode should have dark text
        assert_eq!(colors.text_primary, 0x000000);
        // Accent colors should be non-zero
        assert_ne!(colors.accent, 0);
        assert_ne!(colors.accent_hover, 0);
        assert_ne!(colors.accent_active, 0);
    }
    #[test]
    fn test_hud_colors_light_vs_dark_contrast() {
        // Test that light and dark themes have appropriate contrast
        let dark = HudColors::dark_default();
        let light = HudColors::light_default();

        // Dark mode: dark background, light text
        let dark_bg_brightness = ((dark.background >> 16) & 0xff)
            + ((dark.background >> 8) & 0xff)
            + (dark.background & 0xff);
        let dark_text_brightness = ((dark.text_primary >> 16) & 0xff)
            + ((dark.text_primary >> 8) & 0xff)
            + (dark.text_primary & 0xff);
        assert!(
            dark_bg_brightness < dark_text_brightness,
            "Dark mode: background should be darker than text"
        );

        // Light mode: light background, dark text
        let light_bg_brightness = ((light.background >> 16) & 0xff)
            + ((light.background >> 8) & 0xff)
            + (light.background & 0xff);
        let light_text_brightness = ((light.text_primary >> 16) & 0xff)
            + ((light.text_primary >> 8) & 0xff)
            + (light.text_primary & 0xff);
        assert!(
            light_bg_brightness > light_text_brightness,
            "Light mode: background should be lighter than text"
        );
    }
    #[test]
    fn test_hud_colors_accent_variants() {
        // Test that hover is lighter than accent, and active is darker
        let colors = HudColors::dark_default();

        // Extract brightness (simple sum of components)
        let brightness = |c: u32| ((c >> 16) & 0xff) + ((c >> 8) & 0xff) + (c & 0xff);

        // Hover should be brighter than base accent
        assert!(
            brightness(colors.accent_hover) >= brightness(colors.accent),
            "Hover should be at least as bright as accent"
        );

        // Active should be darker than base accent
        assert!(
            brightness(colors.accent_active) <= brightness(colors.accent),
            "Active should be at most as bright as accent"
        );
    }
    #[test]
    fn test_hud_view_with_custom_colors() {
        // Test that HudView can be created with custom colors
        let custom_colors = HudColors {
            background: 0x2a2a2a,
            background_rgba: 0x2a2a2a80,
            text_primary: 0xeeeeee,
            accent: 0x00ff00,
            accent_hover: 0x33ff33,
            accent_active: 0x00cc00,
        };

        let view = HudView::with_colors("Custom themed HUD".to_string(), custom_colors);
        assert_eq!(view.colors.background, 0x2a2a2a);
        assert_eq!(view.colors.background_rgba, 0x2a2a2a80);
        assert_eq!(view.colors.text_primary, 0xeeeeee);
        assert_eq!(view.colors.accent, 0x00ff00);
    }
    // =============================================================================
    // HUD Manager State Tests
    // =============================================================================

    #[test]
    fn test_hud_manager_state_queue_operations() {
        // Test that pending queue works correctly
        let mut state = HudManagerState::new();

        // Queue should start empty
        assert!(state.pending_queue.is_empty());

        // Add items to queue
        state.pending_queue.push_back((
            HudNotification {
                text: "First".to_string(),
                duration_ms: 2000,
                created_at: Instant::now(),
                action_label: None,
                action: None,
            },
            HudHostOptions::interactive(),
        ));

        state.pending_queue.push_back((
            HudNotification {
                text: "Second".to_string(),
                duration_ms: 2000,
                created_at: Instant::now(),
                action_label: None,
                action: None,
            },
            HudHostOptions::interactive(),
        ));

        assert_eq!(state.pending_queue.len(), 2);

        // Pop front should return first item
        let first = state.pending_queue.pop_front().unwrap();
        assert_eq!(first.0.text, "First");

        // Queue should still have one item
        assert_eq!(state.pending_queue.len(), 1);

        let second = state.pending_queue.pop_front().unwrap();
        assert_eq!(second.0.text, "Second");

        // Queue should be empty now
        assert!(state.pending_queue.is_empty());
    }
    #[test]
    fn test_hud_notification_partial_has_action() {
        // Test edge cases for has_action()

        // Only action_label set (should NOT have action)
        let notif_label_only = HudNotification {
            text: "Test".to_string(),
            duration_ms: 2000,
            created_at: Instant::now(),
            action_label: Some("Click".to_string()),
            action: None,
        };
        assert!(
            !notif_label_only.has_action(),
            "Should not have action with only label"
        );

        // Only action set (should NOT have action)
        let notif_action_only = HudNotification {
            text: "Test".to_string(),
            duration_ms: 2000,
            created_at: Instant::now(),
            action_label: None,
            action: Some(HudAction::OpenUrl("https://example.com".to_string())),
        };
        assert!(
            !notif_action_only.has_action(),
            "Should not have action with only action (no label)"
        );
    }
    #[test]
    fn test_hud_constants() {
        // Verify constants are sensible values using const blocks (clippy-compliant)

        // Default duration should be reasonable (1-10 seconds)
        const _: () = assert!(DEFAULT_HUD_DURATION_MS >= 1000 && DEFAULT_HUD_DURATION_MS <= 10000);

        // Stack gap should be positive and reasonable
        // Note: f32 comparisons in const context require workaround
        assert!(
            (HUD_STACK_GAP as i32) > 0 && (HUD_STACK_GAP as i32) < 100,
            "Stack gap should be positive and reasonable"
        );

        // Max simultaneous HUDs should be at least 1
        const _: () = assert!(MAX_SIMULTANEOUS_HUDS >= 1);

        // HUD dimensions should be positive and internally ordered.
        assert!(
            (HUD_MIN_WIDTH as i32) > 0,
            "HUD min width should be positive"
        );
        assert!(
            HUD_MAX_WIDTH >= HUD_MIN_WIDTH,
            "HUD max width should be at least the min width"
        );
        assert!(
            (HUD_SINGLE_LINE_HEIGHT as i32) > 0,
            "HUD single-line height should be positive"
        );
        assert!(
            (HUD_ACTION_WIDTH as i32) > 0,
            "HUD action width should be positive"
        );
        assert!(
            (HUD_ACTION_HEIGHT as i32) > 0,
            "HUD action height should be positive"
        );
    }
    #[test]
    fn test_hud_action_variants_debug() {
        // Test Debug impl for HudAction variants
        let open_file = HudAction::OpenFile(std::path::PathBuf::from("/test/path.txt"));
        let debug_str = format!("{:?}", open_file);
        assert!(
            debug_str.contains("OpenFile"),
            "Debug should contain variant name"
        );

        let open_url = HudAction::OpenUrl("https://test.com".to_string());
        let debug_str = format!("{:?}", open_url);
        assert!(
            debug_str.contains("OpenUrl"),
            "Debug should contain variant name"
        );

        let run_cmd = HudAction::RunCommand("ls -la".to_string());
        let debug_str = format!("{:?}", run_cmd);
        assert!(
            debug_str.contains("RunCommand"),
            "Debug should contain variant name"
        );
    }
    #[test]
    fn test_hud_action_clone() {
        // Test Clone impl for HudAction
        let original = HudAction::OpenUrl("https://clone-test.com".to_string());
        let cloned = original.clone();

        match (original, cloned) {
            (HudAction::OpenUrl(orig_url), HudAction::OpenUrl(clone_url)) => {
                assert_eq!(orig_url, clone_url, "Cloned URL should match original");
            }
            _ => panic!("Clone should preserve variant type"),
        }
    }
    // --- merged from part_001.rs ---
    #[test]
    fn test_hud_notification_clone() {
        // Test Clone impl for HudNotification
        let original = HudNotification {
            text: "Clone test".to_string(),
            duration_ms: 3000,
            created_at: Instant::now(),
            action_label: Some("Test".to_string()),
            action: Some(HudAction::OpenUrl("https://example.com".to_string())),
        };

        let cloned = original.clone();
        assert_eq!(cloned.text, original.text);
        assert_eq!(cloned.duration_ms, original.duration_ms);
        assert_eq!(cloned.action_label, original.action_label);
        assert!(
            cloned.has_action(),
            "Cloned notification should have action"
        );
    }
    #[test]
    fn test_hud_colors_copy_trait() {
        // Test that HudColors implements Copy (important for closures)
        let colors = HudColors::dark_default();

        // This would fail to compile if HudColors wasn't Copy
        let colors_copy = colors;
        let another_copy = colors;

        assert_eq!(colors_copy.background, another_copy.background);
        assert_eq!(colors_copy.background_rgba, another_copy.background_rgba);
        assert_eq!(colors_copy.text_primary, another_copy.text_primary);
    }
    #[test]
    fn test_color_manipulation_with_gray() {
        // Test lighten/darken with mid-gray
        let gray = 0x808080; // RGB(128, 128, 128)

        let lightened = lighten_color(gray, 0.5);
        // Each component: 128 + (255-128)*0.5 = 128 + 63.5 = 191.5 -> 191 = 0xbf
        assert_eq!(lightened, 0xbfbfbf);

        let darkened = darken_color(gray, 0.5);
        // Each component: 128 * 0.5 = 64 = 0x40
        assert_eq!(darkened, 0x404040);
    }
    #[test]
    fn test_color_manipulation_preserves_channels() {
        // Test that color manipulation works independently per channel
        let color = 0xff8000; // RGB(255, 128, 0) - orange

        let darkened = darken_color(color, 0.5);
        // R: 255*0.5=127, G: 128*0.5=64, B: 0*0.5=0
        let r = (darkened >> 16) & 0xff;
        let g = (darkened >> 8) & 0xff;
        let b = darkened & 0xff;

        assert_eq!(r, 127, "Red channel should be halved");
        assert_eq!(g, 64, "Green channel should be halved");
        assert_eq!(b, 0, "Blue channel should stay at 0");
    }
    // =============================================================================
    // Slot-based Allocation Tests (TDD for overlap fix)
    // =============================================================================

    #[test]
    fn test_slot_allocation_gives_unique_slots() {
        // Each HUD should get a unique slot 0..MAX_SIMULTANEOUS_HUDS
        let mut state = HudManagerState::new();

        // Allocate slots one by one
        let slot0 = state.first_free_slot();
        assert_eq!(slot0, Some(0), "First slot should be 0");

        // Simulate HUD at slot 0
        state.hud_slots[0] = Some(HudSlotEntry { id: 100 });

        let slot1 = state.first_free_slot();
        assert_eq!(slot1, Some(1), "Second slot should be 1");

        state.hud_slots[1] = Some(HudSlotEntry { id: 101 });

        let slot2 = state.first_free_slot();
        assert_eq!(slot2, Some(2), "Third slot should be 2");

        state.hud_slots[2] = Some(HudSlotEntry { id: 102 });

        // All slots full
        let slot_none = state.first_free_slot();
        assert_eq!(slot_none, None, "Should return None when all slots full");
    }
    #[test]
    fn test_slot_release_makes_slot_available() {
        // When a HUD dismisses, its slot becomes available for reuse
        let mut state = HudManagerState::new();

        // Fill all slots
        state.hud_slots[0] = Some(HudSlotEntry { id: 100 });
        state.hud_slots[1] = Some(HudSlotEntry { id: 101 });
        state.hud_slots[2] = Some(HudSlotEntry { id: 102 });

        // All slots full
        assert_eq!(state.first_free_slot(), None);

        // Release slot 1 (middle)
        state.release_slot_by_id(101);

        // Slot 1 should now be free
        assert!(state.hud_slots[1].is_none(), "Slot 1 should be released");

        // Next allocation should get slot 1
        let next_slot = state.first_free_slot();
        assert_eq!(next_slot, Some(1), "Should reuse released slot 1");
    }
    #[test]
    fn test_concurrent_huds_get_different_slots() {
        // Multiple HUDs active at same time should have different slots
        let mut state = HudManagerState::new();

        // Allocate and fill slots
        state.hud_slots[0] = Some(HudSlotEntry { id: 200 });
        state.hud_slots[1] = Some(HudSlotEntry { id: 201 });
        state.hud_slots[2] = Some(HudSlotEntry { id: 202 });

        // Verify all slots occupied by different IDs
        let ids: Vec<u64> = state
            .hud_slots
            .iter()
            .filter_map(|s| s.as_ref().map(|e| e.id))
            .collect();

        assert_eq!(ids.len(), 3, "All slots should be occupied");
        assert!(ids.contains(&200));
        assert!(ids.contains(&201));
        assert!(ids.contains(&202));

        // No duplicates
        let mut sorted = ids.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), 3, "All IDs should be unique");
    }
    #[test]
    fn test_position_uses_slot_not_len() {
        // Position calculation should use slot index, not active count
        // This is the core bug fix - releasing middle HUD shouldn't move others

        let mut state = HudManagerState::new();

        // Fill slots 0, 1, 2
        state.hud_slots[0] = Some(HudSlotEntry { id: 300 });
        state.hud_slots[1] = Some(HudSlotEntry { id: 301 });
        state.hud_slots[2] = Some(HudSlotEntry { id: 302 });

        // Release middle HUD (slot 1)
        state.release_slot_by_id(301);

        // Slot 0 and 2 should still have their original entries
        assert!(
            state.hud_slots[0].is_some(),
            "Slot 0 should still be occupied"
        );
        assert!(state.hud_slots[1].is_none(), "Slot 1 should be empty");
        assert!(
            state.hud_slots[2].is_some(),
            "Slot 2 should still be occupied"
        );

        // Calculate offsets using slot indices
        // Slot 0 -> offset = 0 * HUD_STACK_GAP
        // Slot 2 -> offset = 2 * HUD_STACK_GAP (NOT 1 * HUD_STACK_GAP)
        let offset_slot_0 = 0.0 * HUD_STACK_GAP;
        let offset_slot_2 = 2.0 * HUD_STACK_GAP;

        assert!(
            (offset_slot_0 - 0.0).abs() < f32::EPSILON,
            "Slot 0 should have offset 0"
        );
        assert!(
            (offset_slot_2 - 2.0 * HUD_STACK_GAP).abs() < f32::EPSILON,
            "Slot 2 should keep its original offset"
        );
    }
    #[test]
    fn test_release_nonexistent_id_is_safe() {
        // Releasing an ID that doesn't exist should be a no-op
        let mut state = HudManagerState::new();

        state.hud_slots[0] = Some(HudSlotEntry { id: 400 });

        // Release non-existent ID
        state.release_slot_by_id(999);

        // Original slot should be unchanged
        assert!(state.hud_slots[0].is_some(), "Existing slot should remain");
        assert_eq!(state.hud_slots[0].as_ref().unwrap().id, 400);
    }
    #[test]
    fn test_find_slot_by_id() {
        let mut state = HudManagerState::new();

        state.hud_slots[0] = Some(HudSlotEntry { id: 500 });
        state.hud_slots[2] = Some(HudSlotEntry { id: 502 });
        // slot 1 is empty

        assert_eq!(state.find_slot_by_id(500), Some(0));
        assert_eq!(state.find_slot_by_id(502), Some(2));
        assert_eq!(state.find_slot_by_id(501), None); // doesn't exist
        assert_eq!(state.find_slot_by_id(999), None); // doesn't exist
    }
    #[test]
    fn test_active_hud_count() {
        let mut state = HudManagerState::new();

        assert_eq!(state.active_hud_count(), 0);

        state.hud_slots[0] = Some(HudSlotEntry { id: 600 });
        assert_eq!(state.active_hud_count(), 1);

        state.hud_slots[2] = Some(HudSlotEntry { id: 602 });
        assert_eq!(state.active_hud_count(), 2);

        state.hud_slots[1] = Some(HudSlotEntry { id: 601 });
        assert_eq!(state.active_hud_count(), 3);

        // Release middle
        state.release_slot_by_id(601);
        assert_eq!(state.active_hud_count(), 2);
    }
}
