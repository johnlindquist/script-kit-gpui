//! Dynamic Window Resizing Module
//!
//! Handles window height for different view types in Script Kit GPUI.
//!
//! **Key Rules:**
//! - MainWindow: FIXED at 480px, single consolidated window mode
//! - ArgPrompt with choices: Dynamic height based on choice count (capped at 500px)
//! - ArgPrompt without choices (input only): Compact input-only height
//! - Editor/Div/Term: Full height 700px

// --- merged from part_000.rs ---
use crate::config::{self, LayoutConfig};
use crate::list_item::LIST_ITEM_HEIGHT;
use crate::logging;
use crate::window_manager;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
#[cfg(target_os = "macos")]
use cocoa::foundation::{NSPoint, NSRect, NSSize};
use gpui::{px, Pixels};
#[cfg(target_os = "macos")]
use objc::{class, msg_send, sel, sel_impl};
use std::sync::OnceLock;
use tracing::{debug, info, warn};
const RESIZE_MIN_DELTA_PX: f64 = 1.0;
const WINDOW_RESIZE_ANIMATE: bool = false;
/// Tahoe's detached footer is hosted inside the physical main NSWindow.
const MAIN_WINDOW_SINGLE_HOST_FOOTER_ACTIVE: bool = true;
const MAIN_WINDOW_MIN_HEIGHT: f32 = 480.0;
const MAIN_WINDOW_MAX_HEIGHT: f32 = 480.0;
const MAIN_WINDOW_HEADER_HEIGHT: f32 =
    crate::panel::HEADER_TOTAL_HEIGHT - crate::panel::HEADER_DIVIDER_HEIGHT;
// Main-window footer (hint strip) height. Raised 30 -> 36 to give the footer
// action buttons more vertical breathing room (button height = footer - 2*inset,
// so 26 -> 32; the 22pt keycaps now sit with ~5pt above/below instead of ~2pt).
// The window stays fixed at 480pt: this height is SUBTRACTED from the list budget
// (see `main_window_list_budget_height`), so it costs list space, not window growth.
// Row math stays at the 9-row cap: budget 480-44-1-36=399 -> floor(399/40)=9.
const MAIN_WINDOW_HINT_STRIP_HEIGHT: f32 = 36.0;
const MAIN_WINDOW_DIVIDER_HEIGHT: f32 = crate::panel::HEADER_DIVIDER_HEIGHT;
const MAIN_WINDOW_SECTION_HEADER_HEIGHT: f32 = 32.0;
pub(crate) const MAIN_WINDOW_MAX_VISIBLE_ROWS: usize = 9;

/// The fixed full-view main-window height. Exposed for the design-contract
/// exporter (`design/mockups`); the private min/max constants stay the
/// resize-logic authority.
pub(crate) fn main_window_full_height() -> f32 {
    MAIN_WINDOW_MAX_HEIGHT
}

/// Available pixel budget for list content in the main window.
///
/// Subtracts the fixed chrome (header + divider + hint strip) from `MAX_HEIGHT`.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/ui_window.rs
pub(crate) fn main_window_list_budget_height() -> f32 {
    MAIN_WINDOW_MAX_HEIGHT
        - MAIN_WINDOW_HEADER_HEIGHT
        - MAIN_WINDOW_DIVIDER_HEIGHT
        - MAIN_WINDOW_HINT_STRIP_HEIGHT
}

/// Content height for Quick Terminal inside the main window shell.
///
/// This is the *terminal-grid-only* height (panel minus footer hint strip).
/// Use it for the initial PTY resize hint via `TermPrompt::with_height`.
pub fn quick_terminal_content_height() -> Pixels {
    px(MAIN_WINDOW_MAX_HEIGHT - layout::FOOTER_HEIGHT)
}

/// Full panel height for Quick Terminal inside the main window shell.
///
/// This matches the NSPanel size and must be used as `.h()` on the render
/// wrapper that holds *both* the terminal entity AND the footer hint strip.
/// Using the smaller content height here leaves a visible gap of
/// `FOOTER_HEIGHT` pixels below the footer.
pub fn quick_terminal_panel_height() -> Pixels {
    px(MAIN_WINDOW_MAX_HEIGHT)
}

/// Maximum number of selectable rows that can fit without clipping, given
/// `visible_section_headers` section headers that each consume
/// `MAIN_WINDOW_SECTION_HEADER_HEIGHT` pixels of the list budget.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/ui_window.rs
pub(crate) fn capped_main_window_selectable_rows(visible_section_headers: usize) -> usize {
    let remaining_list_height = main_window_list_budget_height()
        - (visible_section_headers as f32 * MAIN_WINDOW_SECTION_HEADER_HEIGHT);

    if remaining_list_height <= 0.0 {
        0
    } else {
        ((remaining_list_height / LIST_ITEM_HEIGHT).floor() as usize)
            .min(MAIN_WINDOW_MAX_VISIBLE_ROWS)
    }
}

/// Shared layout constants for the main window render branch.
/// Both resize logic and render code consume these so the geometry contract stays in sync.
/// Constants are consumed from the binary target via `include!()` render code.
#[allow(dead_code)]
pub(crate) mod main_layout {
    /// Horizontal padding for the header area.
    pub const HEADER_PADDING_X: f32 = 16.0;
    /// Vertical padding for the header area.
    pub const HEADER_PADDING_Y: f32 = 8.0;
    /// Horizontal padding for the hint strip footer.
    pub const HINT_STRIP_PADDING_X: f32 = 14.0;
    /// Vertical padding for the hint strip footer.
    pub const HINT_STRIP_PADDING_Y: f32 = 8.0;
    /// Height of the hint strip area (matches resize contract).
    pub const HINT_STRIP_HEIGHT: f32 = super::MAIN_WINDOW_HINT_STRIP_HEIGHT;
    /// Height reserved for the native AppKit main-window footer host.
    pub const NATIVE_MAIN_WINDOW_FOOTER_HEIGHT: f32 = HINT_STRIP_HEIGHT;
    /// Height of the divider between header and list content.
    pub const DIVIDER_HEIGHT: f32 = super::MAIN_WINDOW_DIVIDER_HEIGHT;
    /// Opacity for hint strip shortcut text (uses OPACITY_TEXT_MUTED from theme/opacity).
    pub const HINT_TEXT_OPACITY: f32 = crate::theme::opacity::OPACITY_TEXT_MUTED;
}
/// Build a content-aware `MainWindowSizing` from grouped items.
///
/// Walks the grouped items list, counting selectable items and section headers
/// visible in the first page.  Uses the header-aware row cap so that section
/// headers explicitly reduce the available selectable-row capacity instead of
/// silently pushing the window height into the max-clamp.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/ui_window.rs
pub(crate) fn main_window_sizing_from_grouped_items(
    grouped_items: &[crate::list_item::GroupedListItem],
) -> MainWindowSizing {
    use crate::list_item::GroupedListItem;

    let mut selectable_items = 0usize;
    let mut visible_section_headers = 0usize;

    for item in grouped_items {
        let selectable_cap = capped_main_window_selectable_rows(visible_section_headers);

        if selectable_items >= selectable_cap {
            break;
        }

        match item {
            GroupedListItem::SectionHeader(..) => {
                visible_section_headers += 1;
            }
            GroupedListItem::Status(..) => {
                visible_section_headers += 1;
            }
            GroupedListItem::Item(_) => {
                selectable_items += 1;
            }
        }
    }

    MainWindowSizing {
        selectable_items,
        visible_section_headers,
        is_empty: grouped_items.is_empty(),
    }
}

/// Content-aware sizing input for the main window.
///
/// Instead of passing a flat `item_count` (which conflates section headers with
/// selectable items), callers build this struct so the height formula can account
/// for the different row heights of headers vs items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct MainWindowSizing {
    /// Number of selectable items visible in the first page (capped at MAX_VISIBLE_ROWS).
    pub selectable_items: usize,
    /// Number of section headers visible in the first page.
    pub visible_section_headers: usize,
    /// True when the grouped items list is completely empty.
    pub is_empty: bool,
}

/// Reason for a main window resize — used for structured telemetry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Variants used from include!()-ed code in app_impl/ui_window.rs
pub(crate) enum ResizeReason {
    /// Filter text changed → grouped results changed → resize needed.
    FilterChanged,
    /// Grouped results changed (data refresh, view transition, etc.).
    GroupedResultsChanged,
    /// Entering main window mode.
    ViewModeEntered,
    /// Flat item-count fallback was used instead of content-aware sizing.
    FlatFallback,
}

/// Emit a structured sizing receipt for every main window resize.
///
/// All mini resize paths must call this so that every height decision is
/// machine-parseable under the `MINI_WINDOW` tracing target.
pub(crate) fn log_main_window_sizing(
    reason: ResizeReason,
    sizing: MainWindowSizing,
    target_height_px: f32,
) {
    if !crate::logging::filter_perf_trace_enabled() {
        return;
    }

    info!(
        target: "MAIN_WINDOW",
        ?reason,
        selectable_items = sizing.selectable_items,
        visible_section_headers = sizing.visible_section_headers,
        is_empty = sizing.is_empty,
        target_height_px,
        min_height_px = MAIN_WINDOW_MIN_HEIGHT,
        max_height_px = MAIN_WINDOW_MAX_HEIGHT,
        header_height_px = MAIN_WINDOW_HEADER_HEIGHT,
        divider_height_px = MAIN_WINDOW_DIVIDER_HEIGHT,
        hint_strip_height_px = MAIN_WINDOW_HINT_STRIP_HEIGHT,
        section_header_height_px = MAIN_WINDOW_SECTION_HEADER_HEIGHT,
        "main window sizing receipt"
    );
}

/// Calculate the target height for the main window.
///
/// Returns a fixed height to eliminate resize jank. The main window always uses
/// the full max height so transitions between views (e.g. main menu → file search)
/// and filter changes never cause visible resizing.
pub(crate) fn height_for_main_window(_sizing: MainWindowSizing) -> Pixels {
    px(MAIN_WINDOW_MAX_HEIGHT)
}

/// Canonical size target for the main menu.
///
/// Main menu sizing is intentionally separate from prompt sizing so a prompt
/// cannot leak its shorter height into the next main window show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Used by binary-only main-window show/reset paths.
pub(crate) struct MainMenuSizingTarget(pub MainWindowSizing);

impl MainMenuSizingTarget {
    #[allow(dead_code)] // Used by binary-only main-window show/reset paths.
    pub(crate) fn width(self) -> f32 {
        MAIN_WINDOW_WIDTH
    }

    #[allow(dead_code)] // Used by binary-only main-window show/reset paths.
    pub(crate) fn height(self) -> Pixels {
        height_for_main_window(self.0)
    }
}

/// Defer a main window resize to the end of the current effect cycle.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/ui_window.rs
pub(crate) fn defer_resize_to_main_window(
    sizing: MainWindowSizing,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let target_height = height_for_main_window(sizing);
    crate::window_ops::queue_resize_with_width(
        f32::from(target_height),
        Some(MAIN_WINDOW_WIDTH),
        window,
        cx,
    );
}

/// Resize the main window synchronously.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/ui_window.rs
pub(crate) fn resize_to_main_window_sync(sizing: MainWindowSizing) {
    let target_height = height_for_main_window(sizing);
    resize_first_window_to_size(target_height, Some(MAIN_WINDOW_WIDTH));
}

/// Resize the main window synchronously to its default ScriptList mini bounds.
#[allow(dead_code)] // Called from hide paths that need hidden-phase automation bounds reset.
pub(crate) fn resize_to_mini_main_window_sync() {
    resize_to_main_window_sync(Default::default());
}

/// File search mode has no section headers, so sizing is a straight row cap.
#[allow(dead_code)] // Called from include!()-ed code in app_impl/filter_input_*.rs
pub(crate) fn file_search_sizing(result_count: usize) -> MainWindowSizing {
    MainWindowSizing {
        selectable_items: result_count.min(MAIN_WINDOW_MAX_VISIBLE_ROWS),
        visible_section_headers: 0,
        is_empty: result_count == 0,
    }
}

#[allow(dead_code)] // Called from include!()-ed code in app_impl/filter_input_*.rs
pub(crate) fn resize_to_file_search_window_sync(result_count: usize) {
    resize_to_main_window_sync(file_search_sizing(result_count));
}

/// Width for the main window (standard launcher)
pub(crate) const MAIN_WINDOW_WIDTH: f32 = 750.0;
const FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT: f32 = crate::panel::PROMPT_INPUT_FIELD_HEIGHT;
const FOCUSED_TEXT_MINI_RESULT_HEIGHT: f32 = 150.0;
/// Window height when the mini shows the three stacked variation cards.
/// Mirrors `focused_text_variation_area_height(3)` in the Agent Chat view:
/// 3 × card-min(96) + 2 × gap(8) + 2 × padding(8) = 320, plus the input row.
const FOCUSED_TEXT_MINI_VARIATIONS_HEIGHT: f32 = FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT + 320.0;
const FOCUSED_TEXT_MINI_RESIZE_ANIMATE: bool = true;

pub(crate) fn focused_text_mini_input_height() -> f32 {
    FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
}

pub(crate) fn focused_text_mini_result_height() -> f32 {
    FOCUSED_TEXT_MINI_RESULT_HEIGHT
}

pub(crate) fn focused_text_mini_preview_height() -> f32 {
    (FOCUSED_TEXT_MINI_RESULT_HEIGHT - FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT).max(0.0)
}

pub(crate) fn focused_text_mini_inner_height(window_height: f32) -> f32 {
    (window_height - layout::WINDOW_BORDER_Y).max(0.0)
}
#[derive(Debug, Clone, Copy, PartialEq)]
struct FrameGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
impl FrameGeometry {
    const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}
#[cfg(target_os = "macos")]
impl FrameGeometry {
    fn from_ns_rect(rect: NSRect) -> Self {
        Self::new(
            rect.origin.x,
            rect.origin.y,
            rect.size.width,
            rect.size.height,
        )
    }

    fn to_ns_rect(self) -> NSRect {
        NSRect::new(
            NSPoint::new(self.x, self.y),
            NSSize::new(self.width, self.height),
        )
    }
}
#[cfg(target_os = "macos")]
#[derive(Debug, Clone, Copy)]
struct ScreenGeometry {
    visible_bounds: FrameGeometry,
    backing_scale: f64,
}
fn sanitize_backing_scale(backing_scale: Option<f64>) -> Option<f64> {
    backing_scale.filter(|scale| scale.is_finite() && *scale > 0.0)
}
fn round_to_backing_scale(value: f64, backing_scale: f64) -> f64 {
    (value * backing_scale).round() / backing_scale
}
fn normalize_frame_to_backing_scale(frame: FrameGeometry, backing_scale: f64) -> FrameGeometry {
    FrameGeometry::new(
        round_to_backing_scale(frame.x, backing_scale),
        round_to_backing_scale(frame.y, backing_scale),
        round_to_backing_scale(frame.width, backing_scale),
        round_to_backing_scale(frame.height, backing_scale),
    )
}
fn clamp_dimension_to_visible_bounds(size: f64, visible_size: f64, edge_margin: f64) -> f64 {
    let max_size = (visible_size - (edge_margin * 2.0)).max(1.0);
    size.min(max_size)
}
fn clamp_axis_to_visible_bounds(
    origin: f64,
    size: f64,
    visible_origin: f64,
    visible_size: f64,
    edge_margin: f64,
) -> f64 {
    let min_origin = visible_origin + edge_margin;
    let max_origin = visible_origin + visible_size - size - edge_margin;

    if min_origin <= max_origin {
        origin.clamp(min_origin, max_origin)
    } else {
        min_origin
    }
}
fn clamp_frame_to_visible_bounds(
    frame: FrameGeometry,
    visible_bounds: FrameGeometry,
    edge_margin: f64,
) -> FrameGeometry {
    let edge_margin = edge_margin.max(0.0);
    let width = clamp_dimension_to_visible_bounds(frame.width, visible_bounds.width, edge_margin);
    let height =
        clamp_dimension_to_visible_bounds(frame.height, visible_bounds.height, edge_margin);
    let x = clamp_axis_to_visible_bounds(
        frame.x,
        width,
        visible_bounds.x,
        visible_bounds.width,
        edge_margin,
    );
    let y = clamp_axis_to_visible_bounds(
        frame.y,
        height,
        visible_bounds.y,
        visible_bounds.height,
        edge_margin,
    );
    FrameGeometry::new(x, y, width, height)
}
fn calculate_resized_frame(
    current_frame: FrameGeometry,
    target_height: f64,
    visible_bounds: Option<FrameGeometry>,
    backing_scale: Option<f64>,
) -> FrameGeometry {
    calculate_resized_frame_with_width(
        current_frame,
        target_height,
        None,
        visible_bounds,
        backing_scale,
    )
}
fn calculate_resized_frame_with_width(
    current_frame: FrameGeometry,
    target_height: f64,
    target_width: Option<f64>,
    visible_bounds: Option<FrameGeometry>,
    backing_scale: Option<f64>,
) -> FrameGeometry {
    let height_delta = target_height - current_frame.height;
    let new_origin_y = current_frame.y - height_delta;
    let new_width = target_width.unwrap_or(current_frame.width);
    // Preserve visual center when width changes, then clamp to visible bounds below.
    let new_origin_x = current_frame.x - ((new_width - current_frame.width) / 2.0);
    let mut resized = FrameGeometry::new(new_origin_x, new_origin_y, new_width, target_height);

    if let Some(backing_scale) = sanitize_backing_scale(backing_scale) {
        resized = normalize_frame_to_backing_scale(resized, backing_scale);
    }

    if let Some(visible_bounds) = visible_bounds {
        resized = clamp_frame_to_visible_bounds(
            resized,
            visible_bounds,
            crate::panel::WINDOW_VISIBLE_EDGE_MARGIN,
        );
    }

    if let Some(backing_scale) = sanitize_backing_scale(backing_scale) {
        resized = normalize_frame_to_backing_scale(resized, backing_scale);
    }

    resized
}
fn should_apply_resize(current_height: f64, target_height: f64) -> bool {
    (current_height - target_height).abs() >= RESIZE_MIN_DELTA_PX
}
#[cfg(target_os = "macos")]
fn ns_rect_contains_point(rect: NSRect, x: f64, y: f64) -> bool {
    x >= rect.origin.x
        && x < rect.origin.x + rect.size.width
        && y >= rect.origin.y
        && y < rect.origin.y + rect.size.height
}
#[cfg(target_os = "macos")]
unsafe fn screen_geometry_from_screen(screen: id) -> Option<ScreenGeometry> {
    if screen == nil {
        return None;
    }

    let visible_frame: NSRect = msg_send![screen, visibleFrame];
    let backing_scale: f64 = msg_send![screen, backingScaleFactor];
    Some(ScreenGeometry {
        visible_bounds: FrameGeometry::from_ns_rect(visible_frame),
        backing_scale,
    })
}
#[cfg(target_os = "macos")]
unsafe fn screen_geometry_for_window_frame(
    window: id,
    frame: FrameGeometry,
) -> Option<ScreenGeometry> {
    let center_x = frame.x + (frame.width / 2.0);
    let center_y = frame.y + (frame.height / 2.0);

    let screens: id = msg_send![class!(NSScreen), screens];
    if screens != nil {
        let count: usize = msg_send![screens, count];
        for index in 0..count {
            let screen: id = msg_send![screens, objectAtIndex:index];
            if screen == nil {
                continue;
            }

            let screen_frame: NSRect = msg_send![screen, frame];
            if ns_rect_contains_point(screen_frame, center_x, center_y) {
                if let Some(geometry) = screen_geometry_from_screen(screen) {
                    return Some(geometry);
                }
            }
        }
    }

    let window_screen: id = msg_send![window, screen];
    if let Some(geometry) = screen_geometry_from_screen(window_screen) {
        return Some(geometry);
    }

    let main_screen: id = msg_send![class!(NSScreen), mainScreen];
    screen_geometry_from_screen(main_screen)
}
/// Layout constants for height calculations
pub mod layout {
    use crate::panel::{CURSOR_HEIGHT_LG, CURSOR_MARGIN_Y};
    use gpui::{px, Pixels};

    /// List container vertical padding (top + bottom, matches default padding_xs)
    pub const ARG_LIST_PADDING_Y: f32 = 8.0;
    /// Divider thickness (matches default design border_thin)
    pub const ARG_DIVIDER_HEIGHT: f32 = 1.0;
    /// Main-window context chrome measured by the shared layout contract.
    pub const SELECT_CONTEXT_HEADER_HEIGHT: f32 = 30.0;
    /// Painted shell/footer boundary consumed outside the logical row budget.
    pub const SELECT_SHELL_FOOTER_BOUNDARY: f32 = 1.0;
    /// Input row text height (cursor height + margins)
    pub const ARG_INPUT_LINE_HEIGHT: f32 = CURSOR_HEIGHT_LG + (CURSOR_MARGIN_Y * 2.0);
    /// Footer height (matches PromptFooter)
    pub const FOOTER_HEIGHT: f32 = 30.0;
    /// Window border overhead (1px top + 1px bottom from render_impl.rs .border_1())
    pub const WINDOW_BORDER_Y: f32 = 2.0;
    /// Total input-only height (header only, no list, but with footer)
    /// Uses HEADER_PADDING_Y from ui::chrome (same as render scaffold) for accurate height
    pub const ARG_HEADER_HEIGHT: f32 =
        (super::main_layout::HEADER_PADDING_Y * 2.0) + ARG_INPUT_LINE_HEIGHT + FOOTER_HEIGHT;

    /// Minimum window height (input only) - for input-only prompts
    pub const MIN_HEIGHT: Pixels = px(ARG_HEADER_HEIGHT);

    /// Standard height for views with preview panel (script list, arg with choices)
    /// This is FIXED - these views do NOT resize dynamically
    pub const STANDARD_HEIGHT: Pixels = px(crate::config::defaults::DEFAULT_LAYOUT_STANDARD_HEIGHT);

    /// Maximum window height for full-content views (editor, div, term)
    pub const MAX_HEIGHT: Pixels = px(crate::config::defaults::DEFAULT_LAYOUT_MAX_HEIGHT);
}
fn sanitize_dimension(value: f32, fallback: f32) -> f32 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        fallback
    }
}
fn sanitize_layout_config(layout: LayoutConfig) -> LayoutConfig {
    let defaults = LayoutConfig::default();
    let min_height = f32::from(layout::MIN_HEIGHT);

    let standard_height =
        sanitize_dimension(layout.standard_height, defaults.standard_height).max(min_height);
    let max_height =
        sanitize_dimension(layout.max_height, defaults.max_height).max(standard_height);

    LayoutConfig {
        standard_height,
        max_height,
    }
}
fn runtime_layout_config() -> LayoutConfig {
    static LAYOUT_CONFIG_CACHE: OnceLock<LayoutConfig> = OnceLock::new();

    LAYOUT_CONFIG_CACHE
        .get_or_init(|| sanitize_layout_config(config::load_user_preferences().layout))
        .clone()
}
fn height_for_view_with_layout(
    view_type: ViewType,
    item_count: usize,
    layout_config: &LayoutConfig,
) -> Pixels {
    use layout::*;

    let standard_height = px(layout_config.standard_height);
    let max_height = px(layout_config.max_height);

    let clamp_height = |height: Pixels| -> Pixels {
        let height_f = f32::from(height);
        let min_f = f32::from(MIN_HEIGHT);
        let max_f = f32::from(standard_height);
        px(height_f.clamp(min_f, max_f))
    };

    match view_type {
        // Views with preview panel - FIXED height, no dynamic resizing
        // DivPrompt also uses standard height to match main window
        ViewType::ScriptList | ViewType::DivPrompt => standard_height,
        ViewType::MainWindow | ViewType::MiniAiChat => {
            // Flat item_count fallback: assumes all items are selectable (no section headers).
            // Prefer height_for_main_window(MainWindowSizing) for content-aware sizing.
            let visible_items = item_count.min(MAIN_WINDOW_MAX_VISIBLE_ROWS);
            let sizing = MainWindowSizing {
                selectable_items: visible_items,
                visible_section_headers: 0,
                is_empty: item_count == 0,
            };
            let height = height_for_main_window(sizing);
            // Flat calls are behavior-identical to content-aware sizing while
            // `height_for_main_window` returns a fixed height, and builtin
            // filterable lists route here once per keystroke — a WARN would
            // fire constantly and train readers to ignore real violations
            // (chaos battery 11).  Structured telemetry below still records
            // every flat-path resize under ResizeReason::FlatFallback.
            debug!(
                target: "MAIN_WINDOW",
                item_count,
                "flat sizing fallback used; content-aware sizing should be preferred"
            );
            log_main_window_sizing(ResizeReason::FlatFallback, sizing, f32::from(height));
            height
        }
        ViewType::FocusedTextMini => match item_count {
            0 => px(FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT + layout::WINDOW_BORDER_Y),
            // `focused_text_mini_sizing_count` adds one when the optional
            // scope row is visible. The extra 44px must be represented in the
            // native window height instead of being clipped inside the root.
            1 => px(FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT * 2.0 + layout::WINDOW_BORDER_Y),
            2 => px(FOCUSED_TEXT_MINI_RESULT_HEIGHT + layout::WINDOW_BORDER_Y),
            3..=4 => px(FOCUSED_TEXT_MINI_RESULT_HEIGHT
                + FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
                + layout::WINDOW_BORDER_Y),
            5 => px(FOCUSED_TEXT_MINI_VARIATIONS_HEIGHT + layout::WINDOW_BORDER_Y),
            _ => px(FOCUSED_TEXT_MINI_VARIATIONS_HEIGHT
                + FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
                + layout::WINDOW_BORDER_Y),
        },
        ViewType::ArgPromptWithChoices => {
            let visible_items = item_count.max(1) as f32;
            let list_height =
                (visible_items * LIST_ITEM_HEIGHT) + ARG_LIST_PADDING_Y + ARG_DIVIDER_HEIGHT;
            let total_height = ARG_HEADER_HEIGHT + list_height + WINDOW_BORDER_Y;
            clamp_height(px(total_height))
        }
        ViewType::SelectPrompt => {
            // Select uses the shared prompt header/footer budget plus the
            // main-window context header. Routing it through Arg sizing
            // omitted that 30px context row and clipped the first 40px
            // Comfortable choice row at small choice counts (OF-13).
            let visible_items = item_count.max(1) as f32;
            let list_height =
                (visible_items * LIST_ITEM_HEIGHT) + ARG_LIST_PADDING_Y + ARG_DIVIDER_HEIGHT;
            let total_height = ARG_HEADER_HEIGHT
                + SELECT_CONTEXT_HEADER_HEIGHT
                + list_height
                + SELECT_SHELL_FOOTER_BOUNDARY
                + WINDOW_BORDER_Y;
            clamp_height(px(total_height))
        }
        ViewType::MiniPrompt => {
            let visible_items = item_count.clamp(1, 5) as f32;
            let list_height =
                (visible_items * LIST_ITEM_HEIGHT) + ARG_LIST_PADDING_Y + ARG_DIVIDER_HEIGHT;
            let total_height = ARG_HEADER_HEIGHT + list_height + WINDOW_BORDER_Y;
            clamp_height(px(total_height))
        }
        // Input-only prompt - compact
        ViewType::ArgPromptNoChoices => MIN_HEIGHT,
        // Full content views (editor, terminal) - max height
        ViewType::EditorPrompt | ViewType::TermPrompt => max_height,
    }
}
fn initial_window_height_with_layout(layout_config: &LayoutConfig) -> Pixels {
    px(layout_config.standard_height)
}
/// View types for height calculation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewType {
    /// Script list view (main launcher) - has preview panel, FIXED height
    ScriptList,
    /// Main window — consolidated single window mode.
    MainWindow,
    /// Prompt rendered in the main window width.
    MiniPrompt,
    /// Inline AI chat rendered in the main window width.
    MiniAiChat,
    /// Focused-text Mini Agent rendered as a compact input/output overlay.
    FocusedTextMini,
    /// Arg prompt with choices - dynamic height based on item count
    ArgPromptWithChoices,
    /// Select prompt with a main-window context header plus an internal search header.
    SelectPrompt,
    /// Arg prompt without choices (input only) - compact height
    ArgPromptNoChoices,
    /// Div prompt (HTML display) - full height
    DivPrompt,
    /// Editor prompt (code editor) - full height
    EditorPrompt,
    /// Terminal prompt - full height
    TermPrompt,
}

/// Get the target height for a specific view type
///
/// # Arguments
/// * `view_type` - The type of view being displayed
/// * `item_count` - Number of items in the current view (used for dynamic sizing)
///
/// # Returns
/// The window height for this view type
pub fn height_for_view(view_type: ViewType, item_count: usize) -> Pixels {
    let layout_config = runtime_layout_config();
    height_for_view_with_layout(view_type, item_count, &layout_config)
}
/// Get the target width for a specific view type, if it differs from current.
///
/// Returns `Some(width)` when the view needs a specific width (e.g. main window mode),
/// or `None` when the current width should be preserved.
pub fn width_for_view(view_type: ViewType) -> Option<f32> {
    match view_type {
        ViewType::MainWindow
        | ViewType::MiniPrompt
        | ViewType::MiniAiChat
        | ViewType::FocusedTextMini => Some(MAIN_WINDOW_WIDTH),
        ViewType::ScriptList => Some(MAIN_WINDOW_WIDTH),
        _ => None,
    }
}
/// Calculate the initial window height for app startup
pub fn initial_window_height() -> Pixels {
    let layout_config = runtime_layout_config();
    initial_window_height_with_layout(&layout_config)
}
/// Initial main-window size for app startup: the shared main-window width
/// plus the layout-config-driven initial height. Single source of truth so
/// entry points never hardcode the 750px width.
///
/// This is the PHYSICAL frame size (used for window creation and bounds
/// math): in floating-footer glass mode the footer strip lives outside the
/// frame, so the height is shorter than the logical
/// [`initial_window_height`] by the strip.
pub fn initial_window_size() -> gpui::Size<Pixels> {
    let logical_height = f32::from(initial_window_height()) as f64;
    gpui::size(
        gpui::px(MAIN_WINDOW_WIDTH),
        px(physical_main_window_height_for_footer_mode(
            logical_height,
            MAIN_WINDOW_SINGLE_HOST_FOOTER_ACTIVE,
            f64::from(crate::footer_popup::main_window_float_footer_strip_height()),
        ) as f32),
    )
}
/// Defer a window resize to the end of the current effect cycle.
///
/// This version uses `Window::defer()` for coalesced, deferred execution.
/// Use when you have direct Window access (e.g., in window update closures, hotkey handlers).
///
/// # Arguments
/// * `view_type` - The type of view to resize for
/// * `item_count` - Item count (used for some view types)
/// * `window` - The GPUI Window reference
/// * `cx` - The GPUI App context
///
pub fn defer_resize_to_view(
    view_type: ViewType,
    item_count: usize,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) {
    let target_height = height_for_view(view_type, item_count);
    let target_width = width_for_view(view_type);
    crate::window_ops::queue_resize_with_width(f32::from(target_height), target_width, window, cx);
}
/// Resize window synchronously based on view type.
///
/// Use this version when you only have ViewContext access (e.g., in prompt message handlers
/// running from async tasks via `cx.spawn`). These handlers run outside the render cycle,
/// so direct resize is safe and won't cause RefCell borrow conflicts.
///
/// # Arguments
/// * `view_type` - The type of view to resize for
/// * `item_count` - Item count (used for some view types)
///
/// # Example
/// ```rust,ignore
/// // In handle_prompt_message or similar ViewContext methods:
/// resize_to_view_sync(ViewType::ArgPromptWithChoices, choices.len());
/// ```
pub fn resize_to_view_sync(view_type: ViewType, item_count: usize) {
    let target_height = height_for_view(view_type, item_count);
    let target_width = width_for_view(view_type);
    if matches!(
        view_type,
        ViewType::MainWindow
            | ViewType::MiniPrompt
            | ViewType::MiniAiChat
            | ViewType::FocusedTextMini
    ) {
        let visible_rows = item_count.clamp(4, MAIN_WINDOW_MAX_VISIBLE_ROWS);
        debug!(
            view_type = ?view_type,
            width_px = target_width.unwrap_or(0.0),
            height_px = f32::from(target_height),
            item_count = item_count,
            visible_row_count = visible_rows,
            "main_window sizing selected"
        );
    }
    if target_width.is_some() {
        if matches!(view_type, ViewType::FocusedTextMini) {
            resize_first_window_to_size_with_animation(
                target_height,
                target_width,
                FOCUSED_TEXT_MINI_RESIZE_ANIMATE,
            );
        } else {
            resize_first_window_to_size(target_height, target_width);
        }
    } else {
        resize_first_window_to_height(target_height);
    }
}
/// Force reset the debounce timer (kept for API compatibility)
pub fn reset_resize_debounce() {
    // No-op - we removed debouncing since resizes are now rare
}
/// Resize the main window to a new height, keeping the top edge fixed.
///
/// # Arguments
/// * `target_height` - The desired window height in pixels
///
/// # Platform
/// This function only works on macOS. On other platforms, it's a no-op.
#[cfg(target_os = "macos")]
/// Physical NSWindow height for a logical main-window height.
///
/// In floating-footer glass mode the footer strip lives OUTSIDE the window
/// (in the footer child window below the frame), so the NSWindow frame is
/// shorter than the logical height by the strip. All height math elsewhere
/// stays logical; this boundary (and its inverse in
/// [`get_first_window_height`]) is the only conversion point.
#[cfg(target_os = "macos")]
fn physical_main_window_height(logical: f64) -> f64 {
    physical_main_window_height_for_footer_mode(
        logical,
        MAIN_WINDOW_SINGLE_HOST_FOOTER_ACTIVE,
        f64::from(crate::footer_popup::main_window_float_footer_strip_height()),
    )
}

fn physical_main_window_height_for_footer_mode(
    logical: f64,
    single_host_active: bool,
    strip_height: f64,
) -> f64 {
    if single_host_active {
        logical.max(120.0)
    } else {
        (logical - strip_height.max(0.0)).max(120.0)
    }
}

fn logical_main_window_height_for_footer_mode(
    physical: f64,
    single_host_active: bool,
    strip_height: f64,
) -> f64 {
    if single_host_active {
        physical
    } else {
        physical + strip_height.max(0.0)
    }
}

pub fn resize_first_window_to_height(target_height: Pixels) {
    let height_f64: f64 = physical_main_window_height(f32::from(target_height) as f64);

    // Get the main window from WindowManager
    let window = match window_manager::get_main_window() {
        Some(w) => w,
        None => {
            warn!("Main window not registered in WindowManager, cannot resize");
            logging::log(
                "RESIZE",
                "WARNING: Main window not registered in WindowManager.",
            );
            return;
        }
    };

    unsafe {
        // Get current window frame
        let current_frame: NSRect = msg_send![window, frame];

        // Skip if height is already correct (within 1px tolerance)
        let current_height = current_frame.size.height;
        if !should_apply_resize(current_height, height_f64) {
            return;
        }

        let correlation_id = format!("resize:{}", uuid::Uuid::new_v4());

        // Log actual resizes at debug level (these are rare events, not hot-path).
        debug!(
            from_height = current_height,
            to_height = height_f64,
            correlation_id = %correlation_id,
            "Resizing window instantly"
        );
        logging::log(
            "RESIZE",
            &format!(
                "[RESIZE_START] correlation_id={} from={:.0} to={:.0} animate={}",
                correlation_id, current_height, height_f64, WINDOW_RESIZE_ANIMATE
            ),
        );

        let current_geometry = FrameGeometry::from_ns_rect(current_frame);
        let screen_geometry = screen_geometry_for_window_frame(window, current_geometry);
        let new_frame = calculate_resized_frame(
            current_geometry,
            height_f64,
            screen_geometry.map(|geometry| geometry.visible_bounds),
            screen_geometry.map(|geometry| geometry.backing_scale),
        )
        .to_ns_rect();

        // Apply the new frame instantly to avoid any native resize animation.
        let _: () = msg_send![
            window,
            setFrame:new_frame
            display:true
            animate:WINDOW_RESIZE_ANIMATE
        ];

        logging::log(
            "RESIZE",
            &format!(
                "[RESIZE_END] correlation_id={} applied_height={:.0}",
                correlation_id, height_f64
            ),
        );
    }
}
/// Get the current height of the main window
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn get_first_window_height() -> Option<Pixels> {
    let window = window_manager::get_main_window()?;

    unsafe {
        let frame: NSRect = msg_send![window, frame];
        // Inverse of `physical_main_window_height`: report the LOGICAL height
        // so callers can compare against `height_for_view` outputs.
        Some(px(logical_main_window_height_for_footer_mode(
            frame.size.height,
            MAIN_WINDOW_SINGLE_HOST_FOOTER_ACTIVE,
            f64::from(crate::footer_popup::main_window_float_footer_strip_height()),
        ) as f32))
    }
}
/// Non-macOS stub for resize function
#[cfg(not(target_os = "macos"))]
pub fn resize_first_window_to_height(_target_height: Pixels) {
    logging::log("RESIZE", "Window resize is only supported on macOS");
}
/// Non-macOS stub for get_first_window_height
#[allow(dead_code)]
#[cfg(not(target_os = "macos"))]
pub fn get_first_window_height() -> Option<Pixels> {
    None
}
/// Resize the main window to a new height and optionally a new width, keeping the top edge fixed.
/// When width changes, the window preserves its original x position and expands/shrinks to the right.
#[cfg(target_os = "macos")]
pub fn resize_first_window_to_size(target_height: Pixels, target_width: Option<f32>) {
    resize_first_window_to_size_with_animation(target_height, target_width, WINDOW_RESIZE_ANIMATE);
}

/// Resize the main window with an explicit native animation policy.
#[cfg(target_os = "macos")]
pub fn resize_first_window_to_size_with_animation(
    target_height: Pixels,
    target_width: Option<f32>,
    animate: bool,
) {
    let height_f64: f64 = physical_main_window_height(f32::from(target_height) as f64);
    let width_f64: Option<f64> = target_width.map(|w| w as f64);

    let window = match window_manager::get_main_window() {
        Some(w) => w,
        None => {
            warn!("Main window not registered in WindowManager, cannot resize");
            return;
        }
    };

    // SAFETY: NSWindow frame/setFrame are standard AppKit APIs called on the main thread.
    // The window pointer is validated as non-nil by get_main_window().
    unsafe {
        let current_frame: NSRect = msg_send![window, frame];
        let current_height = current_frame.size.height;
        let current_width = current_frame.size.width;

        let height_changed = should_apply_resize(current_height, height_f64);
        let width_changed = width_f64
            .map(|w| (current_width - w).abs() >= RESIZE_MIN_DELTA_PX)
            .unwrap_or(false);

        if !height_changed && !width_changed {
            return;
        }

        let correlation_id = format!("resize:{}", uuid::Uuid::new_v4());

        debug!(
            from_height = current_height,
            to_height = height_f64,
            from_width = current_width,
            to_width = ?width_f64,
            animate,
            correlation_id = %correlation_id,
            "Resizing window (height+width)"
        );

        let current_geometry = FrameGeometry::from_ns_rect(current_frame);
        let screen_geometry = screen_geometry_for_window_frame(window, current_geometry);
        let new_frame = calculate_resized_frame_with_width(
            current_geometry,
            height_f64,
            width_f64,
            screen_geometry.map(|g| g.visible_bounds),
            screen_geometry.map(|g| g.backing_scale),
        )
        .to_ns_rect();

        // SAFETY: setFrame:display:animate: is a standard NSWindow method.
        let _: () = msg_send![
            window,
            setFrame:new_frame
            display:true
            animate:animate
        ];

        logging::log(
            "RESIZE",
            &format!(
                "[RESIZE_SIZE_END] correlation_id={} height={:.0} width={:.0}",
                correlation_id,
                height_f64,
                width_f64.unwrap_or(current_width)
            ),
        );
    }
}
/// Non-macOS stub for resize_first_window_to_size
#[cfg(not(target_os = "macos"))]
pub fn resize_first_window_to_size(_target_height: Pixels, _target_width: Option<f32>) {
    logging::log("RESIZE", "Window resize is only supported on macOS");
}

#[cfg(not(target_os = "macos"))]
pub fn resize_first_window_to_size_with_animation(
    _target_height: Pixels,
    _target_width: Option<f32>,
    _animate: bool,
) {
    logging::log("RESIZE", "Window resize is only supported on macOS");
}

// --- merged from part_001.rs ---
#[cfg(test)]
mod tests;

#[cfg(test)]
mod resize_tests {
    use super::*;
    use gpui::px;

    fn default_layout() -> LayoutConfig {
        LayoutConfig::default()
    }

    #[test]
    fn test_script_list_fixed_height() {
        let layout = default_layout();

        // Script list should always be STANDARD_HEIGHT regardless of item count
        assert_eq!(
            height_for_view_with_layout(ViewType::ScriptList, 0, &layout),
            layout::STANDARD_HEIGHT
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::ScriptList, 5, &layout),
            layout::STANDARD_HEIGHT
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::ScriptList, 100, &layout),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_main_window_fixed_height() {
        let layout = default_layout();
        // Main window always uses the fixed max height to prevent resize jank
        assert_eq!(
            height_for_view_with_layout(ViewType::MainWindow, 0, &layout),
            px(MAIN_WINDOW_MAX_HEIGHT)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::MainWindow, 4, &layout),
            px(MAIN_WINDOW_MAX_HEIGHT)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::MainWindow, 8, &layout),
            px(MAIN_WINDOW_MAX_HEIGHT)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::MainWindow, 100, &layout),
            px(MAIN_WINDOW_MAX_HEIGHT)
        );
    }

    #[test]
    fn test_main_window_width() {
        assert_eq!(
            width_for_view(ViewType::MainWindow),
            Some(MAIN_WINDOW_WIDTH)
        );
    }

    #[test]
    fn test_main_window_height_fixed_regardless_of_content() {
        // All sizing configurations return the fixed max height
        assert_eq!(
            f32::from(height_for_main_window(MainWindowSizing {
                selectable_items: 3,
                visible_section_headers: 0,
                is_empty: false,
            })),
            MAIN_WINDOW_MAX_HEIGHT
        );

        assert_eq!(
            f32::from(height_for_main_window(MainWindowSizing {
                selectable_items: 6,
                visible_section_headers: 1,
                is_empty: false,
            })),
            MAIN_WINDOW_MAX_HEIGHT
        );

        assert_eq!(
            f32::from(height_for_main_window(MainWindowSizing {
                selectable_items: 8,
                visible_section_headers: 2,
                is_empty: false,
            })),
            MAIN_WINDOW_MAX_HEIGHT
        );
    }

    #[test]
    fn single_host_physical_height_equals_logical_group_height() {
        let strip_height = 40.0;
        assert_eq!(
            physical_main_window_height_for_footer_mode(480.0, true, strip_height),
            480.0
        );
        assert_eq!(
            logical_main_window_height_for_footer_mode(480.0, true, strip_height),
            480.0
        );

        assert_eq!(
            physical_main_window_height_for_footer_mode(480.0, false, strip_height),
            440.0
        );
        assert_eq!(
            logical_main_window_height_for_footer_mode(440.0, false, strip_height),
            480.0
        );
    }

    #[test]
    fn test_arg_with_choices_dynamic_height() {
        let layout = default_layout();

        // Arg with choices should size to items, clamped to STANDARD_HEIGHT
        let base_height = layout::ARG_HEADER_HEIGHT
            + layout::ARG_DIVIDER_HEIGHT
            + layout::ARG_LIST_PADDING_Y
            + layout::WINDOW_BORDER_Y;
        assert_eq!(
            height_for_view_with_layout(ViewType::ArgPromptWithChoices, 1, &layout),
            px(base_height + LIST_ITEM_HEIGHT)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::ArgPromptWithChoices, 2, &layout),
            px(base_height + (2.0 * LIST_ITEM_HEIGHT))
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::ArgPromptWithChoices, 100, &layout),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_select_one_choice_reserves_context_header_and_one_full_row() {
        let layout = default_layout();
        let height = f32::from(height_for_view_with_layout(
            ViewType::SelectPrompt,
            1,
            &layout,
        ));
        let required = layout::ARG_HEADER_HEIGHT
            + layout::SELECT_CONTEXT_HEADER_HEIGHT
            + LIST_ITEM_HEIGHT
            + layout::ARG_LIST_PADDING_Y
            + layout::ARG_DIVIDER_HEIGHT
            + layout::SELECT_SHELL_FOOTER_BOUNDARY
            + layout::WINDOW_BORDER_Y;

        assert_eq!(height, required);
        assert!(height >= 149.0, "Select must retain one complete 40px row");
    }

    #[test]
    fn test_arg_no_choices_compact() {
        let layout = default_layout();

        // Arg without choices should be MIN_HEIGHT
        assert_eq!(
            height_for_view_with_layout(ViewType::ArgPromptNoChoices, 0, &layout),
            layout::MIN_HEIGHT
        );
    }

    #[test]
    fn test_full_height_views() {
        let layout = default_layout();

        // Editor and Terminal use MAX_HEIGHT (700px)
        assert_eq!(
            height_for_view_with_layout(ViewType::EditorPrompt, 0, &layout),
            layout::MAX_HEIGHT
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::TermPrompt, 0, &layout),
            layout::MAX_HEIGHT
        );
    }

    #[test]
    fn test_div_prompt_standard_height() {
        let layout = default_layout();

        // DivPrompt uses STANDARD_HEIGHT (500px) to match main window
        assert_eq!(
            height_for_view_with_layout(ViewType::DivPrompt, 0, &layout),
            layout::STANDARD_HEIGHT
        );
    }

    #[test]
    fn test_initial_window_height() {
        let layout = default_layout();
        assert_eq!(
            initial_window_height_with_layout(&layout),
            layout::STANDARD_HEIGHT
        );
        assert_eq!(
            initial_window_height(),
            height_for_view(ViewType::ScriptList, 0)
        );
    }

    #[test]
    fn test_height_constants() {
        assert_eq!(layout::MIN_HEIGHT, px(layout::ARG_HEADER_HEIGHT));
        assert_eq!(layout::STANDARD_HEIGHT, px(500.0));
        assert_eq!(layout::MAX_HEIGHT, px(700.0));
    }

    #[test]
    fn test_layout_uses_configured_standard_and_max_height() {
        let custom_layout = LayoutConfig {
            standard_height: 540.0,
            max_height: 860.0,
        };

        assert_eq!(
            height_for_view_with_layout(ViewType::ScriptList, 0, &custom_layout),
            px(540.0)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::MainWindow, 0, &custom_layout),
            px(MAIN_WINDOW_MAX_HEIGHT)
        );
        assert_eq!(
            height_for_view_with_layout(ViewType::EditorPrompt, 0, &custom_layout),
            px(860.0)
        );
        assert_eq!(initial_window_height_with_layout(&custom_layout), px(540.0));
    }

    #[test]
    fn test_sanitize_layout_config_enforces_bounds() {
        let sanitized = sanitize_layout_config(LayoutConfig {
            standard_height: 10.0,
            max_height: 5.0,
        });

        assert_eq!(sanitized.standard_height, f32::from(layout::MIN_HEIGHT));
        assert_eq!(sanitized.max_height, f32::from(layout::MIN_HEIGHT));
    }

    #[test]
    fn test_calculate_resized_frame_keeps_top_edge_fixed() {
        let current = FrameGeometry::new(100.0, 200.0, 750.0, 500.0);
        let resized = calculate_resized_frame(current, 700.0, None, None);

        assert!((resized.y - 0.0).abs() < 0.001);
        assert!((resized.height - 700.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_resized_frame_clamps_bottom_to_visible_bounds() {
        let current = FrameGeometry::new(100.0, 200.0, 750.0, 500.0);
        let visible = FrameGeometry::new(0.0, 50.0, 1920.0, 800.0);
        let resized = calculate_resized_frame(current, 700.0, Some(visible), None);

        assert!((resized.y - 54.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_resized_frame_caps_height_to_visible_bounds() {
        let current = FrameGeometry::new(100.0, 300.0, 750.0, 400.0);
        let visible = FrameGeometry::new(0.0, 0.0, 1920.0, 700.0);
        let resized = calculate_resized_frame(current, 900.0, Some(visible), None);

        assert!((resized.height - 692.0).abs() < 0.001);
        assert!((resized.y - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_resized_frame_rounds_to_backing_scale() {
        let current = FrameGeometry::new(10.1, 20.2, 749.7, 500.3);
        let resized = calculate_resized_frame(current, 700.7, None, Some(2.0));

        assert!((resized.x - 10.0).abs() < 0.001);
        assert!((resized.y - -180.0).abs() < 0.001);
        assert!((resized.width - 749.5).abs() < 0.001);
        assert!((resized.height - 700.5).abs() < 0.001);
    }

    #[test]
    fn test_should_apply_resize_true_when_height_changes() {
        assert!(should_apply_resize(500.0, 700.0));
    }

    #[test]
    fn test_should_apply_resize_false_when_height_is_effectively_unchanged() {
        assert!(!should_apply_resize(500.0, 500.4));
    }

    #[test]
    fn test_window_resize_animation_flag_is_disabled() {
        let flag = WINDOW_RESIZE_ANIMATE;
        assert!(
            !flag,
            "Window resize must stay instant with animation disabled"
        );
    }

    #[test]
    fn test_width_for_view_main_window() {
        assert_eq!(
            width_for_view(ViewType::MainWindow),
            Some(MAIN_WINDOW_WIDTH)
        );
        assert_eq!(
            width_for_view(ViewType::MiniPrompt),
            Some(MAIN_WINDOW_WIDTH)
        );
        assert_eq!(
            width_for_view(ViewType::MiniAiChat),
            Some(MAIN_WINDOW_WIDTH)
        );
    }

    #[test]
    fn test_width_for_view_script_list() {
        assert_eq!(
            width_for_view(ViewType::ScriptList),
            Some(MAIN_WINDOW_WIDTH)
        );
    }

    #[test]
    fn test_width_for_view_other_types_none() {
        assert_eq!(width_for_view(ViewType::ArgPromptWithChoices), None);
        assert_eq!(width_for_view(ViewType::ArgPromptNoChoices), None);
        assert_eq!(width_for_view(ViewType::DivPrompt), None);
        assert_eq!(width_for_view(ViewType::EditorPrompt), None);
        assert_eq!(width_for_view(ViewType::TermPrompt), None);
    }

    #[test]
    fn test_main_window_width_constant() {
        assert_eq!(MAIN_WINDOW_WIDTH, 750.0);
    }

    #[test]
    fn focused_text_mini_scope_row_is_included_in_every_phase_height() {
        let height = |item_count| f32::from(height_for_view(ViewType::FocusedTextMini, item_count));

        assert_eq!(
            height(0),
            FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            height(1),
            FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT * 2.0 + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            height(2),
            FOCUSED_TEXT_MINI_RESULT_HEIGHT + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            height(3),
            FOCUSED_TEXT_MINI_RESULT_HEIGHT
                + FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
                + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            height(5),
            FOCUSED_TEXT_MINI_VARIATIONS_HEIGHT + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            height(6),
            FOCUSED_TEXT_MINI_VARIATIONS_HEIGHT
                + FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
                + layout::WINDOW_BORDER_Y
        );
        assert_eq!(
            focused_text_mini_inner_height(height(3)),
            FOCUSED_TEXT_MINI_RESULT_HEIGHT + FOCUSED_TEXT_MINI_INPUT_ONLY_HEIGHT
        );
    }

    #[test]
    fn test_calculate_resized_frame_with_width_preserves_center_x() {
        let current = FrameGeometry::new(100.0, 200.0, 750.0, 500.0);
        let resized = calculate_resized_frame_with_width(current, 400.0, Some(480.0), None, None);

        // Width should be 480
        assert!((resized.width - 480.0).abs() < 0.001);
        // Height should be 400
        assert!((resized.height - 400.0).abs() < 0.001);
        // Center X should be preserved: 100 + 750/2 == 235 + 480/2.
        assert!((resized.x - 235.0).abs() < 0.001);
    }

    #[test]
    fn test_calculate_resized_frame_with_width_center_clamps_to_visible_bounds() {
        let current = FrameGeometry::new(0.0, 200.0, 480.0, 500.0);
        let visible = FrameGeometry::new(0.0, 0.0, 800.0, 700.0);
        let resized =
            calculate_resized_frame_with_width(current, 400.0, Some(750.0), Some(visible), None);

        assert!((resized.width - 750.0).abs() < 0.001);
        assert!(
            resized.x >= crate::panel::WINDOW_VISIBLE_EDGE_MARGIN - 0.001,
            "visible-bounds clamp should keep centered expansion on screen"
        );
    }

    #[test]
    fn test_calculate_resized_frame_with_width_none_preserves_width() {
        let current = FrameGeometry::new(100.0, 200.0, 750.0, 500.0);
        let resized = calculate_resized_frame_with_width(current, 400.0, None, None, None);

        // Width should be preserved
        assert!((resized.width - 750.0).abs() < 0.001);
        // X should be preserved
        assert!((resized.x - 100.0).abs() < 0.001);
    }
}

#[cfg(test)]
mod main_window_layout_tests {
    use super::{
        capped_main_window_selectable_rows, height_for_main_window, MainWindowSizing,
        MAIN_WINDOW_MAX_HEIGHT, MAIN_WINDOW_MAX_VISIBLE_ROWS,
    };

    #[test]
    fn capped_rows_account_for_section_headers() {
        assert_eq!(
            capped_main_window_selectable_rows(0),
            MAIN_WINDOW_MAX_VISIBLE_ROWS
        );
        // With 480px max, budget = 480-44-1-30 = 405; 1 header: (405-32)/40 = 9.325 → 9
        assert_eq!(capped_main_window_selectable_rows(1), 9);
        // 2 headers: (405-64)/40 = 8.525 → 8
        assert_eq!(capped_main_window_selectable_rows(2), 8);
    }

    #[test]
    fn two_headers_and_capped_rows_return_fixed_height() {
        let height = height_for_main_window(MainWindowSizing {
            selectable_items: capped_main_window_selectable_rows(2),
            visible_section_headers: 2,
            is_empty: false,
        });

        // Fixed height regardless of content
        assert_eq!(f32::from(height), MAIN_WINDOW_MAX_HEIGHT);
    }

    #[test]
    fn uncapped_two_headers_and_eight_rows_would_hit_max_clamp() {
        let height = height_for_main_window(MainWindowSizing {
            selectable_items: 8,
            visible_section_headers: 2,
            is_empty: false,
        });

        assert_eq!(f32::from(height), MAIN_WINDOW_MAX_HEIGHT);
    }

    // --- Source-audit regression tests ---
    // These verify that structured tracing targets exist so that agentic
    // verification can rely on machine-parseable log lines.

    #[test]
    fn source_audit_resize_receipt_log_exists() {
        let source = std::fs::read_to_string("src/window_resize/mod.rs")
            .expect("should read window_resize/mod.rs");
        assert!(
            source.contains("target: \"MAIN_WINDOW\""),
            "resize flow should emit structured MAIN_WINDOW logs"
        );
    }

    #[test]
    fn source_audit_scroll_reveal_reason_is_logged() {
        let source = std::fs::read_to_string("src/app_navigation/impl_scroll.rs")
            .expect("should read impl_scroll.rs");
        assert!(
            source.contains("reason,"),
            "scroll reveal should log the caller-provided reason"
        );
    }

    #[test]
    fn source_audit_resize_reason_enum_exists() {
        let source = std::fs::read_to_string("src/window_resize/mod.rs")
            .expect("should read window_resize/mod.rs");
        assert!(
            source.contains("ResizeReason"),
            "ResizeReason enum should exist for structured resize receipts"
        );
    }
}
