use gpui::{App, SharedString, Window};

#[cfg(target_os = "macos")]
use cocoa::base::{id, nil, NO, YES};

#[cfg(target_os = "macos")]
const FOOTER_EFFECT_ID: &str = "script-kit-footer-effect";
#[cfg(target_os = "macos")]
const FOOTER_DIVIDER_ID: &str = "script-kit-footer-divider";
#[cfg(target_os = "macos")]
const FOOTER_HINTS_ID: &str = "script-kit-footer-hints";
#[cfg(target_os = "macos")]
const FOOTER_HINT_ITEM_GAP: f64 = 4.0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_KEY_LABEL_GAP: f64 = 3.0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_SIDE_INSET: f64 = crate::window_resize::mini_layout::HINT_STRIP_PADDING_X as f64;
#[cfg(target_os = "macos")]
const FOOTER_HINT_PADDING_X: f64 = 4.0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_PADDING_Y: f64 = 2.0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_RADIUS: f64 = 4.0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_FONT_SIZE: f64 = 12.5;
#[cfg(target_os = "macos")]
const FOOTER_HINT_FONT_WEIGHT_LIGHT: f64 = 0.18;
#[cfg(target_os = "macos")]
const FOOTER_HINT_TEXT_ALIGN_LEFT: usize = 0;
#[cfg(target_os = "macos")]
const FOOTER_HINT_TEXT_ALIGN_RIGHT: usize = 2;
#[cfg(target_os = "macos")]
const FOOTER_HINT_BUTTON_ID_PREFIX: &str = "script-kit-footer-button-";
#[cfg(target_os = "macos")]
const FOOTER_LEFT_INFO_ID: &str = "script-kit-footer-left-info";
#[cfg(target_os = "macos")]
const FOOTER_STREAMING_DOT_SIZE: f64 = 6.0;
#[cfg(target_os = "macos")]
const FOOTER_LEFT_DOT_LABEL_GAP: f64 = 6.0;
#[cfg(target_os = "macos")]
const FOOTER_RUN_SLOT_MIN_WIDTH: f64 = 96.0;
#[cfg(target_os = "macos")]
const FOOTER_ACTIONS_SLOT_WIDTH: f64 = 96.0;
#[cfg(target_os = "macos")]
const FOOTER_AI_SLOT_WIDTH: f64 = 56.0;
#[cfg(target_os = "macos")]
const FOOTER_APPLY_SLOT_WIDTH: f64 = 88.0;
#[cfg(target_os = "macos")]
const FOOTER_CLOSE_SLOT_WIDTH: f64 = 88.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FooterAction {
    Run,
    Actions,
    Ai,
    Apply,
    Close,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FooterButtonConfig {
    pub action: FooterAction,
    pub key: &'static str,
    pub label: SharedString,
    pub selected: bool,
    pub enabled: bool,
}

impl FooterButtonConfig {
    pub(crate) fn new(
        action: FooterAction,
        key: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            action,
            key,
            label: label.into(),
            selected: false,
            enabled: true,
        }
    }

    pub(crate) fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

impl FooterAction {
    pub(crate) fn is_actions(self) -> bool {
        matches!(self, Self::Actions)
    }
}

/// Status of the ACP thread, used to pick dot color and animation.
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
#[derive(Clone, Debug, Default)]
pub(crate) struct FooterLeftInfo {
    /// Controls dot color and animation.
    pub dot_status: FooterDotStatus,
    /// Model display name (e.g. "Claude Sonnet 4"). Empty = hide label.
    pub model_name: String,
    /// When true, active ACP states should use the accent token instead of the
    /// generic high-contrast fallback so the footer clearly reads as AI-active.
    pub prefer_accent_for_active_states: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct MainWindowFooterConfig {
    pub surface: &'static str,
    pub buttons: Vec<FooterButtonConfig>,
    pub left_info: Option<FooterLeftInfo>,
}

impl MainWindowFooterConfig {
    pub(crate) fn new(surface: &'static str, buttons: Vec<FooterButtonConfig>) -> Self {
        Self {
            surface,
            buttons,
            left_info: None,
        }
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

static ACTIVE_MAIN_WINDOW_FOOTER_SURFACE: std::sync::Mutex<Option<&'static str>> =
    std::sync::Mutex::new(None);

fn set_active_main_window_footer_surface(surface: Option<&'static str>) {
    *ACTIVE_MAIN_WINDOW_FOOTER_SURFACE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner()) = surface;
}

pub(crate) fn active_main_window_footer_surface() -> Option<&'static str> {
    *ACTIVE_MAIN_WINDOW_FOOTER_SURFACE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

pub(crate) fn footer_action_channel() -> &'static (
    async_channel::Sender<FooterAction>,
    async_channel::Receiver<FooterAction>,
) {
    &FOOTER_ACTION_CHANNEL
}

pub(crate) fn sync_main_footer_popup(
    window: &mut Window,
    config: Option<&MainWindowFooterConfig>,
    _cx: &mut App,
) {
    set_active_main_window_footer_surface(config.map(|cfg| cfg.surface));

    #[cfg(target_os = "macos")]
    {
        let Some(ns_window) = main_window_ns_window(window) else {
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
                ensure_main_footer_host(ns_window);
                refresh_main_footer_host(ns_window, config);
            } else {
                remove_main_footer_host(ns_window);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, config);
}

pub(crate) fn notify_main_footer_popup(
    window: &mut Window,
    config: Option<&MainWindowFooterConfig>,
    _cx: &mut App,
) {
    set_active_main_window_footer_surface(config.map(|cfg| cfg.surface));

    #[cfg(target_os = "macos")]
    {
        let Some(ns_window) = main_window_ns_window(window) else {
            return;
        };

        // SAFETY: `ns_window` comes from the live GPUI main window currently
        // being rendered/observed on the AppKit thread.
        unsafe {
            if let Some(config) = config {
                refresh_main_footer_host(ns_window, config);
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    let _ = (window, config);
}

pub(crate) fn close_main_footer_popup(cx: &mut App) {
    set_active_main_window_footer_surface(None);

    let Some(window_handle) = crate::get_main_window_handle() else {
        return;
    };

    let _ = window_handle.update(cx, move |_, window, _cx| {
        #[cfg(target_os = "macos")]
        {
            let Some(ns_window) = main_window_ns_window(window) else {
                return;
            };

            // SAFETY: `ns_window` comes from the live GPUI main window on the
            // AppKit main thread while `update_window` is executing.
            unsafe {
                remove_main_footer_host(ns_window);
            }
        }

        #[cfg(not(target_os = "macos"))]
        let _ = window;
    });
}

#[cfg(target_os = "macos")]
fn main_window_ns_window(window: &mut Window) -> Option<id> {
    if let Ok(window_handle) = raw_window_handle::HasWindowHandle::window_handle(window) {
        if let raw_window_handle::RawWindowHandle::AppKit(appkit) = window_handle.as_raw() {
            use objc::{msg_send, sel, sel_impl};

            let ns_view = appkit.ns_view.as_ptr() as id;
            // SAFETY: `ns_view` comes from a live GPUI window on the AppKit
            // main thread. `-[NSView window]` returns the owning NSWindow or nil.
            unsafe {
                let ns_window: id = msg_send![ns_view, window];
                if ns_window != nil {
                    return Some(ns_window);
                }
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
unsafe fn ensure_main_footer_host(ns_window: id) {
    use cocoa::appkit::NSViewWidthSizable;
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("ensure_main_footer_host") {
        return;
    }

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }

    let existing = find_subview_by_identifier(content_view, FOOTER_EFFECT_ID);
    if existing != nil {
        return;
    }

    let content_bounds: NSRect = msg_send![content_view, bounds];
    let footer_frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(content_bounds.size.width, footer_height()),
    );

    let footer_cls = footer_effect_view_class();
    let footer_view: id = msg_send![footer_cls, alloc];
    let footer_view: id = msg_send![footer_view, initWithFrame: footer_frame];
    if footer_view == nil {
        return;
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
            NSSize::new(content_bounds.size.width, 1.0)
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
    let hints_view: id =
        msg_send![hints_view, initWithFrame: footer_hints_frame(content_bounds.size.width)];
    if hints_view != nil {
        let hints_identifier = ns_string(FOOTER_HINTS_ID);
        if hints_identifier != nil {
            let _: () = msg_send![hints_view, setIdentifier: hints_identifier];
        }
        let _: () = msg_send![hints_view, setAutoresizingMask: NSViewWidthSizable];
        let _: () = msg_send![footer_view, addSubview: hints_view];
    }

    // Left-info container (streaming dot + model label)
    let left_info_view: id = msg_send![class!(NSView), alloc];
    let left_info_view: id = msg_send![
        left_info_view,
        initWithFrame: footer_left_info_frame(content_bounds.size.width)
    ];
    if left_info_view != nil {
        let left_info_id = ns_string(FOOTER_LEFT_INFO_ID);
        if left_info_id != nil {
            let _: () = msg_send![left_info_view, setIdentifier: left_info_id];
        }
        let _: () = msg_send![left_info_view, setAutoresizingMask: NSViewWidthSizable];
        let _: () = msg_send![footer_view, addSubview: left_info_view];
    }

    let _: () = msg_send![
        content_view,
        addSubview: footer_view
        positioned: 1isize
        relativeTo: nil
    ];

    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_host_installed",
        "Installed native footer host inside the main window contentView"
    );
}

#[cfg(target_os = "macos")]
unsafe fn refresh_main_footer_host(ns_window: id, config: &MainWindowFooterConfig) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("refresh_main_footer_host") {
        return;
    }

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }

    let footer_view = find_subview_by_identifier(content_view, FOOTER_EFFECT_ID);
    if footer_view == nil {
        return;
    }

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

    let _: () = msg_send![footer_view, setMaterial: material];
    let _: () = msg_send![footer_view, setState: 1isize];
    let _: () = msg_send![footer_view, setBlendingMode: 1isize];
    let _: () = msg_send![footer_view, setEmphasized: is_dark];
    let _: () = msg_send![footer_view, setNeedsDisplay: YES];

    let content_bounds: NSRect = msg_send![content_view, bounds];
    let footer_frame = NSRect::new(
        NSPoint::new(0.0, 0.0),
        NSSize::new(content_bounds.size.width, footer_height()),
    );
    let _: () = msg_send![footer_view, setFrame: footer_frame];

    let footer_layer: id = msg_send![footer_view, layer];
    if footer_layer != nil {
        let _: () = msg_send![footer_layer, setCornerRadius: 0.0_f64];
        let _: () = msg_send![footer_layer, setMasksToBounds: YES];
    }

    let divider_view = find_subview_by_identifier(footer_view, FOOTER_DIVIDER_ID);
    if divider_view != nil {
        let divider_frame = NSRect::new(
            NSPoint::new(0.0, footer_height() - 1.0),
            NSSize::new(content_bounds.size.width, 1.0),
        );
        let _: () = msg_send![divider_view, setFrame: divider_frame];
        let divider_layer: id = msg_send![divider_view, layer];
        if divider_layer != nil {
            let divider_color = ns_color_from_rgba(chrome.divider_rgba);
            if divider_color != nil {
                let cg_color: id = msg_send![divider_color, CGColor];
                if cg_color != nil {
                    let _: () = msg_send![divider_layer, setBackgroundColor: cg_color];
                }
            }
        }
    }

    let alpha = crate::window_resize::mini_layout::HINT_TEXT_OPACITY as f64;
    let text_color = ns_color_from_hex_with_alpha(theme.colors.text.primary, alpha);

    let hints_view = find_subview_by_identifier(footer_view, FOOTER_HINTS_ID);
    if hints_view != nil {
        let _: () = msg_send![hints_view, setFrame: footer_hints_frame(content_bounds.size.width)];
        layout_footer_hints(hints_view, text_color, &config.buttons);
    }

    // Left info (streaming dot + model name)
    let left_info_view = find_subview_by_identifier(footer_view, FOOTER_LEFT_INFO_ID);
    if left_info_view != nil {
        let _: () = msg_send![
            left_info_view,
            setFrame: footer_left_info_frame(content_bounds.size.width)
        ];
        layout_footer_left_info(left_info_view, config.left_info.as_ref(), text_color);
    }

    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_host_refreshed",
        surface = config.surface,
        button_count = config.buttons.len(),
        width = content_bounds.size.width,
        height = footer_height(),
        dark = is_dark,
        "Refreshed native footer host"
    );
}

#[cfg(target_os = "macos")]
unsafe fn remove_main_footer_host(ns_window: id) {
    use objc::{msg_send, sel, sel_impl};

    if crate::platform::require_main_thread("remove_main_footer_host") {
        return;
    }

    let content_view: id = msg_send![ns_window, contentView];
    if content_view == nil {
        return;
    }

    let footer_view = find_subview_by_identifier(content_view, FOOTER_EFFECT_ID);
    if footer_view == nil {
        return;
    }

    let _: () = msg_send![footer_view, removeFromSuperview];
}

#[cfg(target_os = "macos")]
unsafe fn find_subview_by_identifier(parent: id, identifier: &str) -> id {
    use objc::{msg_send, sel, sel_impl};

    let identifier = ns_string(identifier);
    if parent == nil || identifier == nil {
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
                msg_send![view_identifier, isEqualToString: identifier];
            if matches == YES {
                return view;
            }
        }
    }

    nil
}

#[cfg(target_os = "macos")]
fn footer_height() -> f64 {
    crate::window_resize::mini_layout::HINT_STRIP_HEIGHT as f64
}

#[cfg(target_os = "macos")]
fn footer_hints_frame(width: f64) -> cocoa::foundation::NSRect {
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(FOOTER_HINT_SIDE_INSET, 0.0),
        cocoa::foundation::NSSize::new(width - (FOOTER_HINT_SIDE_INSET * 2.0), footer_height()),
    )
}

#[cfg(target_os = "macos")]
fn footer_left_info_frame(width: f64) -> cocoa::foundation::NSRect {
    cocoa::foundation::NSRect::new(
        cocoa::foundation::NSPoint::new(FOOTER_HINT_SIDE_INSET, 0.0),
        cocoa::foundation::NSSize::new(width / 2.0, footer_height()),
    )
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_left_info(
    left_info_view: id,
    left_info: Option<&FooterLeftInfo>,
    text_color: id,
) {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `left_info_view` is a live NSView managed by the footer host on
    // the AppKit main thread. We remove and recreate subviews to match state.

    // Remove all existing subviews (rebuild each refresh, like hints do).
    let subviews: id = msg_send![left_info_view, subviews];
    if subviews != nil {
        let count: usize = msg_send![subviews, count];
        for i in (0..count).rev() {
            let child: id = msg_send![subviews, objectAtIndex: i];
            if child != nil {
                let _: () = msg_send![child, removeFromSuperview];
            }
        }
    }

    let Some(info) = left_info else { return };
    if info.model_name.is_empty() {
        return;
    }

    let bounds: NSRect = msg_send![left_info_view, bounds];
    let mut x = 0.0_f64;

    // ── Status dot (color + animation depends on thread status) ──
    let show_dot = !matches!(info.dot_status, FooterDotStatus::Hidden);
    if show_dot {
        let theme = crate::theme::get_cached_theme();
        let dot_hex = footer_dot_hex(
            info.dot_status,
            &theme,
            info.prefer_accent_for_active_states,
        );
        let should_pulse = matches!(
            info.dot_status,
            FooterDotStatus::Streaming | FooterDotStatus::WaitingForPermission
        );

        let dot_y = ((bounds.size.height - FOOTER_STREAMING_DOT_SIZE) / 2.0).round();
        let dot_view: id = msg_send![class!(NSView), alloc];
        let dot_view: id = msg_send![
            dot_view,
            initWithFrame: NSRect::new(
                NSPoint::new(x, dot_y),
                NSSize::new(FOOTER_STREAMING_DOT_SIZE, FOOTER_STREAMING_DOT_SIZE),
            )
        ];
        if dot_view != nil {
            let _: () = msg_send![dot_view, setWantsLayer: YES];
            let dot_layer: id = msg_send![dot_view, layer];
            if dot_layer != nil {
                let _: () = msg_send![dot_layer, setCornerRadius: FOOTER_STREAMING_DOT_SIZE / 2.0];

                let dot_ns = ns_color_from_hex_with_alpha(dot_hex, 1.0);
                if dot_ns != nil {
                    // SAFETY: `dot_ns` is a valid NSColor.
                    let cg: id = msg_send![dot_ns, CGColor];
                    if cg != nil {
                        let _: () = msg_send![dot_layer, setBackgroundColor: cg];
                    }
                }

                if should_pulse {
                    add_pulsing_opacity_animation(dot_layer);
                }
            }
            let _: () = msg_send![left_info_view, addSubview: dot_view];
            x += FOOTER_STREAMING_DOT_SIZE + FOOTER_LEFT_DOT_LABEL_GAP;
        }
    }

    // ── Model name label ──
    let font: id = msg_send![
        class!(NSFont),
        systemFontOfSize: FOOTER_HINT_FONT_SIZE
        weight: FOOTER_HINT_FONT_WEIGHT_LIGHT
    ];
    let label = make_footer_hint_text_field(
        &info.model_name,
        font,
        text_color,
        FOOTER_HINT_TEXT_ALIGN_LEFT,
    );
    if label != nil {
        let label_size: NSSize = msg_send![label, fittingSize];
        let label_y = ((bounds.size.height - label_size.height) / 2.0).round();
        let _: () = msg_send![
            label,
            setFrame: NSRect::new(
                NSPoint::new(x, label_y),
                NSSize::new(label_size.width, label_size.height),
            )
        ];
        let _: () = msg_send![left_info_view, addSubview: label];
    }
}

/// Attach a repeating CABasicAnimation that pulses a layer's opacity between
/// 0.5 and 1.0 over a 1.2 s cycle (0.6 s each way, autoreverses).
#[cfg(target_os = "macos")]
unsafe fn add_pulsing_opacity_animation(layer: id) {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `layer` is a live CALayer. We create a CABasicAnimation on
    // the "opacity" keypath with autoreversal for a sine-like pulse.
    let key_path = ns_string("opacity");
    if key_path == nil {
        return;
    }

    let anim: id = msg_send![class!(CABasicAnimation), animationWithKeyPath: key_path];
    if anim == nil {
        return;
    }

    let from_value: id = msg_send![class!(NSNumber), numberWithFloat: 0.5_f32];
    let to_value: id = msg_send![class!(NSNumber), numberWithFloat: 1.0_f32];

    let _: () = msg_send![anim, setFromValue: from_value];
    let _: () = msg_send![anim, setToValue: to_value];
    let _: () = msg_send![anim, setDuration: 0.6_f64]; // half-cycle; autoreverses
    let _: () = msg_send![anim, setAutoreverses: YES];
    let _: () = msg_send![anim, setRepeatCount: f32::INFINITY];

    // Use ease-in-ease-out for a smooth sine-like curve.
    let timing_name = ns_string("easeInEaseOut");
    if timing_name != nil {
        // SAFETY: Standard CoreAnimation timing function lookup.
        let timing: id = msg_send![
            class!(CAMediaTimingFunction),
            functionWithName: timing_name
        ];
        if timing != nil {
            let _: () = msg_send![anim, setTimingFunction: timing];
        }
    }

    let anim_key = ns_string("pulseOpacity");
    if anim_key != nil {
        let _: () = msg_send![layer, addAnimation: anim forKey: anim_key];
    }
}

#[cfg(target_os = "macos")]
unsafe fn layout_footer_hints(hints_view: id, text_color: id, buttons: &[FooterButtonConfig]) {
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
        systemFontOfSize: FOOTER_HINT_FONT_SIZE
        weight: FOOTER_HINT_FONT_WEIGHT_LIGHT
    ];

    let mut items = Vec::new();
    let mut total_item_width = 0.0_f64;
    for (index, button_cfg) in buttons.iter().enumerate() {
        let item = make_footer_hint_item(button_cfg, font, text_color);
        if item == nil {
            continue;
        }
        let item_frame: NSRect = msg_send![item, frame];
        let target_width = footer_hint_slot_width(button_cfg.action).max(item_frame.size.width);
        total_item_width += target_width;
        if index > 0 {
            total_item_width += FOOTER_HINT_ITEM_GAP;
        }
        items.push((item, target_width));
    }

    let mut x = (hints_bounds.size.width - total_item_width).max(0.0);
    for (item, target_width) in items {
        let frame = NSRect::new(
            NSPoint::new(x, 0.0),
            NSSize::new(target_width, hints_bounds.size.height),
        );
        let _: () = msg_send![item, setFrame: frame];
        let _: () = msg_send![hints_view, addSubview: item];
        x += target_width + FOOTER_HINT_ITEM_GAP;
    }
}

#[cfg(target_os = "macos")]
fn footer_hint_slot_width(action: FooterAction) -> f64 {
    match action {
        FooterAction::Run => FOOTER_RUN_SLOT_MIN_WIDTH,
        FooterAction::Actions => FOOTER_ACTIONS_SLOT_WIDTH,
        FooterAction::Ai => FOOTER_AI_SLOT_WIDTH,
        FooterAction::Apply => FOOTER_APPLY_SLOT_WIDTH,
        FooterAction::Close => FOOTER_CLOSE_SLOT_WIDTH,
    }
}

fn footer_hint_content_layout(
    action: FooterAction,
    item_width: f64,
    label_width: f64,
    key_width: f64,
) -> (f64, f64, f64) {
    let content_width = label_width + FOOTER_HINT_KEY_LABEL_GAP + key_width;

    if matches!(action, FooterAction::Run) {
        let key_x = (item_width - FOOTER_HINT_PADDING_X - key_width).round();
        let label_x = (key_x - FOOTER_HINT_KEY_LABEL_GAP - label_width)
            .max(0.0)
            .round();
        return (label_x, key_x, content_width);
    }

    let label_x = ((item_width - content_width) / 2.0).max(0.0).round();
    let key_x = (label_x + label_width + FOOTER_HINT_KEY_LABEL_GAP).round();
    (label_x, key_x, content_width)
}

#[cfg(target_os = "macos")]
unsafe fn make_footer_hint_item(button_cfg: &FooterButtonConfig, font: id, text_color: id) -> id {
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use objc::{class, msg_send, sel, sel_impl};

    let container: id = msg_send![class!(NSView), alloc];
    let container: id = msg_send![
        container,
        initWithFrame: NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, footer_height()))
    ];
    if container == nil {
        return nil;
    }

    let key_field = make_footer_hint_text_field(
        button_cfg.key,
        font,
        text_color,
        FOOTER_HINT_TEXT_ALIGN_LEFT,
    );
    let label_field = make_footer_hint_text_field(
        button_cfg.label.as_ref(),
        font,
        text_color,
        FOOTER_HINT_TEXT_ALIGN_RIGHT,
    );
    if key_field == nil || label_field == nil {
        return nil;
    }

    let key_size: NSSize = msg_send![key_field, fittingSize];
    let label_size: NSSize = msg_send![label_field, fittingSize];
    let min_content_width = key_size.width + (FOOTER_HINT_PADDING_X * 2.0) + 12.0;
    let content_width = label_size.width + FOOTER_HINT_KEY_LABEL_GAP + key_size.width;
    let intrinsic_width = content_width + (FOOTER_HINT_PADDING_X * 2.0);
    let item_width = footer_hint_slot_width(button_cfg.action)
        .max(min_content_width)
        .max(intrinsic_width);
    let item_height = footer_height();
    let content_height = key_size.height.max(label_size.height) + (FOOTER_HINT_PADDING_Y * 2.0);
    let content_y = ((item_height - content_height) / 2.0).round();
    let key_y = (content_y + FOOTER_HINT_PADDING_Y).round();
    let label_y = (content_y + FOOTER_HINT_PADDING_Y).round();
    let (label_x, key_x, _) = footer_hint_content_layout(
        button_cfg.action,
        item_width,
        label_size.width,
        key_size.width,
    );

    let _: () = msg_send![
        label_field,
        setFrame: NSRect::new(
            NSPoint::new(label_x, label_y),
            NSSize::new(label_size.width, label_size.height)
        )
    ];
    let _: () = msg_send![
        key_field,
        setFrame: NSRect::new(
            NSPoint::new(key_x, key_y),
            NSSize::new(key_size.width, key_size.height)
        )
    ];
    let _: () = msg_send![container, setWantsLayer: YES];
    let container_layer: id = msg_send![container, layer];
    if container_layer != nil {
        let _: () = msg_send![container_layer, setCornerRadius: FOOTER_HINT_RADIUS];
        if button_cfg.selected {
            let theme = crate::theme::get_cached_theme();
            let chrome = crate::theme::AppChromeColors::from_theme(&theme);
            let selected_ns: id = ns_color_from_rgba(chrome.selection_rgba);
            if selected_ns != nil {
                let cg: id = msg_send![selected_ns, CGColor];
                if cg != nil {
                    let _: () = msg_send![container_layer, setBackgroundColor: cg];
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
        let button_id = ns_string(&format!(
            "{}{}",
            FOOTER_HINT_BUTTON_ID_PREFIX,
            footer_action_key(button_cfg.action)
        ));
        if button_id != nil {
            let _: () = msg_send![button, setIdentifier: button_id];
        }
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
            obj.set_ivar::<cocoa::base::BOOL>(
                "_enabled",
                if button_cfg.enabled { YES } else { NO },
            );
        }
    }

    let _: () = msg_send![container, addSubview: label_field];
    let _: () = msg_send![container, addSubview: key_field];
    if button != nil {
        let _: () = msg_send![container, addSubview: button];
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
        footer_active_dot_hex, footer_dot_hex, footer_hint_content_layout, footer_hint_slot_width,
        FooterAction, FooterDotStatus,
    };

    #[test]
    fn footer_hint_slot_widths_are_stable_per_action() {
        assert_eq!(footer_hint_slot_width(FooterAction::Run), 160.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Actions), 104.0);
        assert_eq!(footer_hint_slot_width(FooterAction::Ai), 64.0);
    }

    #[test]
    fn run_slot_is_wider_than_trailing_slots() {
        assert!(
            footer_hint_slot_width(FooterAction::Run)
                > footer_hint_slot_width(FooterAction::Actions)
        );
        assert!(
            footer_hint_slot_width(FooterAction::Run) > footer_hint_slot_width(FooterAction::Ai)
        );
    }

    #[test]
    fn footer_hint_content_group_is_centered_within_slot() {
        let item_width = 96.0;
        let label_width = 34.0;
        let key_width = 18.0;

        let (label_x, key_x, content_width) =
            footer_hint_content_layout(FooterAction::Actions, item_width, label_width, key_width);
        let left_padding = label_x;
        let right_padding = item_width - (key_x + key_width);

        assert_eq!(content_width, label_width + 3.0 + key_width);
        assert!((left_padding - right_padding).abs() <= 1.0);
    }

    #[test]
    fn run_hint_keeps_key_glyph_anchored_to_trailing_padding() {
        let short = footer_hint_content_layout(FooterAction::Run, 96.0, 20.0, 18.0);
        let long = footer_hint_content_layout(FooterAction::Run, 140.0, 64.0, 18.0);

        assert_eq!(short.1, 74.0);
        assert_eq!(long.1, 118.0);
        assert_eq!(96.0 - (short.1 + 18.0), 4.0);
        assert_eq!(140.0 - (long.1 + 18.0), 4.0);
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
        assert_eq!(
            footer_active_dot_hex(&theme, false),
            theme.colors.accent.selected
        );
    }

    #[test]
    fn active_dot_can_force_accent_for_acp_states() {
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
fn send_footer_action(action: FooterAction) {
    let action_name = footer_action_key(action);
    tracing::info!(
        target: "script_kit::footer_popup",
        event = "native_footer_action_enqueued",
        action = action_name,
        "Enqueued native footer action"
    );
    let (tx, _) = footer_action_channel();
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

fn footer_action_key(action: FooterAction) -> &'static str {
    match action {
        FooterAction::Run => "run",
        FooterAction::Actions => "actions",
        FooterAction::Ai => "ai",
        FooterAction::Apply => "apply",
        FooterAction::Close => "close",
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
            decl.add_ivar::<cocoa::base::BOOL>("_enabled");
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
extern "C" fn footer_button_mouse_entered(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: Set hover background on the parent container's layer.
    // Recompute color from theme each time to avoid dangling CGColor pointers.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            return;
        }
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_entered",
            is_actions_button = is_actions == YES,
            "Native footer button hover entered"
        );

        let superview: id = msg_send![this, superview];
        if superview == nil {
            return;
        }
        let layer: id = msg_send![superview, layer];
        if layer == nil {
            return;
        }
        let theme = crate::theme::get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let hover_ns: id = ns_color_from_rgba(chrome.hover_rgba);
        if hover_ns != nil {
            let cg: id = msg_send![hover_ns, CGColor];
            if cg != nil {
                let _: () = msg_send![layer, setBackgroundColor: cg];
            }
        }
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_exited(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    use objc::{msg_send, sel, sel_impl};

    // SAFETY: Clear hover background on the parent container's layer.
    // If this button has _selected set, restore the selected color instead
    // of clearing.
    unsafe {
        let selected: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_selected");
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        let actions_window_open = crate::actions::is_actions_window_open();
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_exited",
            is_actions_button = is_actions == YES,
            selected = selected == YES,
            actions_window_open,
            "Native footer button hover exited"
        );

        let superview: id = msg_send![this, superview];
        if superview == nil {
            return;
        }
        let layer: id = msg_send![superview, layer];
        if layer == nil {
            return;
        }

        if selected == YES || (is_actions == YES && actions_window_open) {
            let theme = crate::theme::get_cached_theme();
            let chrome = crate::theme::AppChromeColors::from_theme(&theme);
            let selected_ns: id = ns_color_from_rgba(chrome.selection_rgba);
            if selected_ns != nil {
                let cg: id = msg_send![selected_ns, CGColor];
                if cg != nil {
                    let _: () = msg_send![layer, setBackgroundColor: cg];
                }
            }
        } else {
            let null_color: id = std::ptr::null_mut();
            let _: () = msg_send![layer, setBackgroundColor: null_color];
        }
    }
}

#[cfg(target_os = "macos")]
fn footer_effect_view_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

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
        let hit: id = msg_send![super(this_id, class!(NSVisualEffectView)), hitTest: point];
        let button = nearest_footer_button(hit);
        if button != nil {
            return button;
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
        FooterAction::Close => sel!(closeFooterAction:),
    }
}

#[cfg(target_os = "macos")]
fn footer_action_target_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

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
            sel!(closeFooterAction:),
            footer_close_action as extern "C" fn(&Object, Sel, id),
        );
        decl.register() as *const _ as usize
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
extern "C" fn footer_run_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    send_footer_action(FooterAction::Run);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_actions_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    send_footer_action(FooterAction::Actions);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_ai_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    send_footer_action(FooterAction::Ai);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_apply_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    send_footer_action(FooterAction::Apply);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_close_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, _: id) {
    send_footer_action(FooterAction::Close);
}
