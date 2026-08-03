use gpui::*;
use std::rc::Rc;

use super::{
    types::{
        BUTTON_CONTENT_GAP_PX, BUTTON_ICON_PADDING_X, BUTTON_ICON_PADDING_Y,
        BUTTON_PRIMARY_PADDING_X, BUTTON_PRIMARY_PADDING_Y, BUTTON_RADIUS_PX,
        BUTTON_SHORTCUT_MARGIN_LEFT_PX,
    },
    ButtonColors, ButtonVariant, BUTTON_BORDER_WIDTH_PX, BUTTON_GHOST_HEIGHT,
    BUTTON_GHOST_PADDING_X, BUTTON_GHOST_PADDING_Y,
};

/// Callback type for button click events
pub type OnClickCallback = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// A reusable button component for interactive actions
///
/// Supports:
/// - Label text (required)
/// - Keyboard shortcut display (optional)
/// - Three variants: Primary, Ghost, Icon
/// - Hover states with themed colors
/// - Focus ring styling
/// - Click callback
///
#[derive(IntoElement)]
pub struct Button {
    label: SharedString,
    colors: ButtonColors,
    variant: ButtonVariant,
    shortcut_tokens: Option<Vec<String>>,
    id: SharedString,
    disabled: bool,
    loading: bool,
    loading_label: Option<SharedString>,
    focused: bool,
    on_click: Option<Rc<OnClickCallback>>,
    focus_handle: Option<FocusHandle>,
}

impl Button {
    /// Create a button with an explicit stable control ID, display label, and colors.
    pub fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        colors: ButtonColors,
    ) -> Self {
        let id = id.into();
        assert!(!id.is_empty(), "Button stable ID must not be empty");
        Self {
            label: label.into(),
            colors,
            variant: ButtonVariant::default(),
            shortcut_tokens: None,
            id,
            disabled: false,
            loading: false,
            loading_label: None,
            focused: false,
            on_click: None,
            focus_handle: None,
        }
    }

    /// Set the button variant (Primary, Ghost, Icon)
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub(crate) fn resolve_shortcut_tokens(shortcut: &str) -> Vec<String> {
        crate::components::hint_strip::shortcut_tokens_from_hint(shortcut)
    }

    /// Set the keyboard shortcut display text
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        let shortcut = shortcut.into();
        self.shortcut_tokens = Some(Self::resolve_shortcut_tokens(&shortcut));
        self
    }

    /// Set an optional shortcut (convenience for Option<String>)
    pub fn shortcut_opt(mut self, shortcut: Option<String>) -> Self {
        self.shortcut_tokens = shortcut.map(|shortcut| Self::resolve_shortcut_tokens(&shortcut));
        self
    }

    /// Set whether the button is disabled
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Set whether the button is in loading state
    pub fn loading(mut self, loading: bool) -> Self {
        self.loading = loading;
        self
    }

    /// Set optional loading label text
    pub fn loading_label(mut self, loading_label: impl Into<SharedString>) -> Self {
        self.loading_label = Some(loading_label.into());
        self
    }

    /// Set whether the button is focused (shows focus ring)
    pub fn focused(mut self, focused: bool) -> Self {
        self.focused = focused;
        self
    }

    /// Set the click callback
    pub fn on_click(mut self, callback: OnClickCallback) -> Self {
        self.on_click = Some(Rc::new(callback));
        self
    }

    /// Set the label text
    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the focus handle for keyboard accessibility
    pub fn focus_handle(mut self, handle: FocusHandle) -> Self {
        self.focus_handle = Some(handle);
        self
    }

    pub(crate) fn resolve_element_id(id: &SharedString) -> SharedString {
        id.clone()
    }

    pub(crate) fn stable_id(&self) -> &SharedString {
        &self.id
    }

    pub(crate) fn display_label(&self) -> &SharedString {
        &self.label
    }

    pub(crate) fn should_show_pointer(
        has_click_handler: bool,
        disabled: bool,
        loading: bool,
    ) -> bool {
        has_click_handler && !disabled && !loading
    }

    fn is_activation_key(key: &str) -> bool {
        matches!(
            key,
            "enter" | "return" | "Enter" | "Return" | " " | "space" | "Space"
        )
    }

    pub(crate) fn can_activate_from_key(
        key: &str,
        has_click_handler: bool,
        disabled: bool,
        loading: bool,
    ) -> bool {
        Self::should_show_pointer(has_click_handler, disabled, loading)
            && Self::is_activation_key(key)
    }

    pub(crate) fn resolve_focus_state(explicit_focus: bool, runtime_focus: Option<bool>) -> bool {
        runtime_focus.unwrap_or(explicit_focus)
    }

    pub(crate) fn should_show_focus_indicator(
        focused: bool,
        has_click_handler: bool,
        disabled: bool,
        loading: bool,
    ) -> bool {
        focused && Self::should_show_pointer(has_click_handler, disabled, loading)
    }

    pub(crate) fn hover_background_token(variant: ButtonVariant, colors: ButtonColors) -> u32 {
        match variant {
            ButtonVariant::Primary => (colors.background_hover << 8) | 0xB0,
            ButtonVariant::Ghost | ButtonVariant::Icon => colors.hover_overlay,
        }
    }
}

pub(crate) const TRANSPARENT: u32 = 0x00000000;

impl RenderOnce for Button {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Button {
            label,
            colors,
            variant,
            shortcut_tokens,
            id,
            disabled,
            loading,
            loading_label,
            focused,
            on_click,
            focus_handle,
        } = self;
        let on_click_callback = on_click.clone();
        let on_click_for_key = on_click;
        let has_click_handler = on_click_callback.is_some();
        let show_pointer = Self::should_show_pointer(has_click_handler, disabled, loading);
        let element_id = Self::resolve_element_id(&id);
        let label_for_log = label.clone();
        let focus_handle = if show_pointer {
            focus_handle.or_else(|| {
                Some(
                    window
                        .use_keyed_state(ElementId::Name(element_id.clone()), cx, |_, cx| {
                            cx.focus_handle()
                        })
                        .read(cx)
                        .clone(),
                )
            })
        } else {
            None
        };
        let focused = Self::resolve_focus_state(
            focused,
            focus_handle
                .as_ref()
                .map(|handle| handle.is_focused(window)),
        );
        let label_text = if loading {
            loading_label.unwrap_or_else(|| label.clone())
        } else {
            label.clone()
        };

        // Calculate colors based on variant
        let hover_bg = rgba(Self::hover_background_token(variant, colors));

        // Focus styling colors
        // 0xA0 = 62.5% opacity for visible focus ring
        let focus_ring_color = rgba((colors.focus_ring << 8) | 0xA0);
        // 0x20 = 12.5% opacity for subtle background tint
        let focus_tint = rgba((colors.focus_tint << 8) | 0x20);
        // Border color for unfocused state — subtle but visible
        let unfocused_border = rgba((colors.border << 8) | 0x60);

        let (text_color, bg_color, border_color) = match variant {
            ButtonVariant::Primary => {
                // Primary: accent text on subtle dark bg, accent border for emphasis
                let bg = if focused {
                    rgba((colors.accent << 8) | 0x30)
                } else {
                    rgba((colors.accent << 8) | 0x18)
                };
                let text = rgb(colors.accent);
                let border = if focused {
                    focus_ring_color
                } else {
                    rgba((colors.accent << 8) | 0x60)
                };
                (text, bg, border)
            }
            ButtonVariant::Ghost => {
                // Ghost: primary text, transparent bg, subtle border
                let bg = if focused {
                    focus_tint
                } else {
                    rgba(TRANSPARENT)
                };
                let border = if focused {
                    focus_ring_color
                } else {
                    unfocused_border
                };
                (rgb(colors.text_color), bg, border)
            }
            ButtonVariant::Icon => {
                // Icon: compact, accent color
                let bg = if focused {
                    focus_tint
                } else {
                    rgba(TRANSPARENT)
                };
                let border = if focused {
                    focus_ring_color
                } else {
                    rgba(TRANSPARENT)
                };
                (rgb(colors.accent), bg, border)
            }
        };

        // Wrap label text in a div so we can set cursor_pointer on it.
        // GPUI cursor styles don't inherit to children, so the deepest
        // element under the mouse determines the cursor.
        let mut label_element = div().child(label_text);
        if show_pointer {
            label_element = label_element.cursor_pointer();
        }

        // Render cached canonical tokens through the same compact shortcut owner
        // used by rows, Actions, the recorder, and footer adapters.
        let shortcut_element = if let Some(tokens) = shortcut_tokens {
            let shortcut_color = match variant {
                ButtonVariant::Primary | ButtonVariant::Icon => colors.accent,
                ButtonVariant::Ghost => colors.text_color,
            };
            let mut el = div()
                .flex()
                .items_center()
                .ml(px(BUTTON_SHORTCUT_MARGIN_LEFT_PX))
                .child(crate::components::hint_strip::render_inline_shortcut_keys(
                    tokens.iter().map(String::as_str),
                    crate::components::hint_strip::whisper_inline_shortcut_colors(
                        rgba((shortcut_color << 8) | 0xCC).into(),
                        rgba((shortcut_color << 8) | 0xFF).into(),
                        true,
                    ),
                ));
            if show_pointer {
                el = el.cursor_pointer();
            }
            el
        } else {
            div()
        };

        // Determine padding based on variant using canonical button spacing tokens.
        let (padding_x, padding_y) = match variant {
            ButtonVariant::Primary => (BUTTON_PRIMARY_PADDING_X, BUTTON_PRIMARY_PADDING_Y),
            ButtonVariant::Ghost => (BUTTON_GHOST_PADDING_X, BUTTON_GHOST_PADDING_Y),
            ButtonVariant::Icon => (BUTTON_ICON_PADDING_X, BUTTON_ICON_PADDING_Y),
        };

        // Build the button element
        let debug_element_id = element_id.clone();
        let mut button = div()
            .id(ElementId::Name(element_id))
            .debug_selector(move || debug_element_id.to_string())
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap(px(BUTTON_CONTENT_GAP_PX))
            .px(px(padding_x))
            .py(px(padding_y))
            .min_h(px(BUTTON_GHOST_HEIGHT))
            .rounded(px(BUTTON_RADIUS_PX))
            .bg(bg_color)
            .text_color(text_color)
            .text_sm()
            .font_weight(FontWeight::MEDIUM)
            .font_family(crate::list_item::FONT_SYSTEM_UI)
            .cursor_default()
            .child(label_element)
            .child(shortcut_element);

        if loading {
            button = button.child(div().text_xs().opacity(0.7).child("…"));
        }

        // Apply border at a constant width to prevent layout shift on focus change.
        button = button
            .border(px(BUTTON_BORDER_WIDTH_PX))
            .border_color(border_color);

        // Apply hover styles unless disabled
        // Keep text color the same, just add subtle background lift
        if show_pointer {
            button = button.cursor_pointer().hover(move |s| s.bg(hover_bg));
        } else if disabled {
            button = button.opacity(0.5).cursor_default();
        } else if loading {
            button = button.opacity(0.7).cursor_default();
        } else {
            button = button.cursor_default();
        }

        // Add click handler if provided
        if let Some(callback) = on_click_callback {
            if show_pointer {
                button = button.on_click(move |event, window, cx| {
                    tracing::debug!(button = %label_for_log, "Button clicked");
                    callback(event, window, cx);
                });
            }
        }

        // Add focus tracking and keyboard handler if focus_handle is provided
        if let Some(handle) = focus_handle {
            button = button.track_focus(&handle.tab_stop(show_pointer));

            if show_pointer {
                if let Some(callback) = on_click_for_key {
                    button = button.on_key_down(move |event: &KeyDownEvent, window, cx| {
                        let key = event.keystroke.key.as_str();
                        if Button::can_activate_from_key(key, true, disabled, loading) {
                            tracing::debug!("Button activated via keyboard");
                            // Create a default click event for keyboard activation
                            let click_event = ClickEvent::default();
                            callback(&click_event, window, cx);
                            cx.stop_propagation();
                        } else {
                            cx.propagate();
                        }
                    });
                }
            }
        }

        button
    }
}
