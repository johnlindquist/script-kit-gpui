use gpui::{div, prelude::*, px, rems, rgb, rgba, AnyElement, Div, FontWeight, Rgba, SharedString};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PromptFrameConfig {
    pub relative: bool,
    pub rounded_corners: Option<f32>,
    pub min_height_px: f32,
    pub clip_overflow: bool,
}

impl Default for PromptFrameConfig {
    fn default() -> Self {
        Self {
            relative: false,
            rounded_corners: None,
            min_height_px: 0.0,
            clip_overflow: true,
        }
    }
}

impl PromptFrameConfig {
    pub fn with_relative(mut self, relative: bool) -> Self {
        self.relative = relative;
        self
    }

    pub fn with_rounded_corners(mut self, radius: f32) -> Self {
        self.rounded_corners = Some(radius);
        self
    }
}

pub(crate) fn prompt_shell_frame_config(radius: f32) -> PromptFrameConfig {
    PromptFrameConfig::default()
        .with_relative(true)
        .with_rounded_corners(radius)
}

pub(crate) fn prompt_frame_root(config: PromptFrameConfig) -> Div {
    let mut frame = div()
        .flex()
        .flex_col()
        .w_full()
        .h_full()
        .min_h(px(config.min_height_px));

    if config.clip_overflow {
        frame = frame.overflow_hidden();
    }

    if config.relative {
        frame = frame.relative();
    }

    if let Some(radius) = config.rounded_corners {
        frame = frame.rounded(px(radius));
    }

    frame
}

pub(crate) fn prompt_frame_fill_content(content: impl IntoElement) -> Div {
    div()
        .flex_1()
        .w_full()
        .min_h(px(0.))
        .overflow_hidden()
        .child(content)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum PromptBodyInsets {
    #[allow(dead_code)] // Explicit opt-out for specialized/full-bleed body policies.
    None,
    MainMenu(crate::designs::DesignVariant),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedPromptBodyInsets {
    pub x_px: f32,
    pub y_px: f32,
}

impl PromptBodyInsets {
    pub(crate) fn resolve(self) -> ResolvedPromptBodyInsets {
        match self {
            Self::None => ResolvedPromptBodyInsets {
                x_px: 0.0,
                y_px: 0.0,
            },
            Self::MainMenu(design_variant) => {
                let def = crate::designs::current_main_menu_theme().def();
                let spacing = crate::designs::get_tokens(design_variant).spacing();
                ResolvedPromptBodyInsets {
                    x_px: def.shell.content_inset_x,
                    y_px: spacing.padding_sm,
                }
            }
        }
    }
}

pub(crate) fn render_inset_prompt_body(
    id: impl Into<gpui::ElementId>,
    body: impl IntoElement,
    insets: PromptBodyInsets,
) -> gpui::Stateful<Div> {
    let insets = insets.resolve();
    div()
        .id(id.into())
        .w_full()
        .h_full()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .px(px(insets.x_px))
        .py(px(insets.y_px))
        .child(body)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct PromptSingleLineControlMetrics {
    pub total_height_px: f32,
    pub radius_px: f32,
    pub text_inset_x_px: f32,
}

pub(crate) fn prompt_single_line_control_metrics() -> PromptSingleLineControlMetrics {
    let search = crate::designs::current_main_menu_theme().def().search;
    PromptSingleLineControlMetrics {
        total_height_px: search.height,
        radius_px: search.radius,
        text_inset_x_px: search.text_inset_x,
    }
}

/// Shared single-line prompt control surface.
///
/// The main-menu search height is the total border box: vertical padding is
/// intentionally absent here so callers cannot compose extra height onto it.
pub(crate) fn prompt_single_line_control_surface(background: Rgba, border: Rgba) -> Div {
    let metrics = prompt_single_line_control_metrics();
    div()
        .w_full()
        .h(px(metrics.total_height_px))
        .min_h(px(metrics.total_height_px))
        .max_h(px(metrics.total_height_px))
        .px(px(metrics.text_inset_x_px))
        .bg(background)
        .border_1()
        .border_color(border)
        .rounded(px(metrics.radius_px))
}

/// Shared inner card surface for form fields and content cards.
///
/// Returns a full-width rounded div with consistent padding, border, and
/// background — use this for text inputs, preview cards, and any other
/// "card-on-prompt" surface so every step of a multi-step flow shares the
/// same visual language.
pub(crate) fn prompt_surface(background: Rgba, border: Rgba) -> Div {
    div()
        .w_full()
        .px(rems(0.875))
        .py(rems(0.625))
        .bg(background)
        .border_1()
        .border_color(border)
        .rounded(px(8.0))
}

/// Shared intro block for create-flow screens (title + description).
pub(crate) fn prompt_form_intro(
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    title_color: Rgba,
    description_color: Rgba,
    gap_px: f32,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(gap_px))
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(title_color)
                .child(title.into()),
        )
        .child(
            div()
                .text_sm()
                .text_color(description_color)
                .child(description.into()),
        )
}

/// Shared labeled section for create-flow screens (label above content).
pub(crate) fn prompt_form_section(
    label: impl Into<SharedString>,
    label_color: Rgba,
    gap_px: f32,
    content: impl IntoElement,
) -> Div {
    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(gap_px))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(label_color)
                .child(label.into()),
        )
        .child(content)
}

/// Shared helper text for create-flow screens.
pub(crate) fn prompt_form_help(text: impl Into<SharedString>, color: Rgba) -> Div {
    div().text_xs().text_color(color).child(text.into())
}

/// Shared text color ladder for prompt chrome.
#[derive(Clone, Copy)]
pub(crate) struct PromptTextPalette {
    pub primary: Rgba,
    #[allow(dead_code)]
    pub label: Rgba,
    pub help: Rgba,
    pub placeholder: Rgba,
}

pub(crate) fn prompt_text_palette(theme: &crate::theme::Theme) -> PromptTextPalette {
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    PromptTextPalette {
        primary: rgb(chrome.text_primary_hex),
        label: rgba(chrome.text_muted_rgba),
        help: rgba(chrome.text_hint_rgba),
        placeholder: rgba(chrome.placeholder_text_rgba),
    }
}

/// State of a form field within a create-flow prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PromptFieldState {
    Default,
    Active,
    Error,
    ReadOnly,
}

/// Pre-computed colors for a form field based on its state.
#[derive(Clone, Copy)]
pub(crate) struct PromptFieldStyle {
    pub background: Rgba,
    pub border: Rgba,
    pub value: Rgba,
}

/// Compute field colors from the theme, field state, and whether the value is empty.
///
/// All color/opacity decisions route through [`AppChromeColors`] so prompt
/// fields stay consistent with the rest of the app chrome.
pub(crate) fn prompt_field_style(
    theme: &crate::theme::Theme,
    state: PromptFieldState,
    empty: bool,
) -> PromptFieldStyle {
    let chrome = crate::theme::AppChromeColors::from_theme(theme);
    let value = if empty {
        rgba(chrome.placeholder_text_rgba)
    } else {
        rgb(chrome.text_primary_hex)
    };

    match state {
        PromptFieldState::Default => PromptFieldStyle {
            background: rgba(chrome.input_surface_rgba),
            border: rgba(chrome.badge_border_rgba),
            value,
        },
        PromptFieldState::Active => PromptFieldStyle {
            background: rgba(chrome.selection_rgba),
            border: rgb(chrome.accent_hex),
            value,
        },
        PromptFieldState::Error => PromptFieldStyle {
            background: rgba(chrome.input_surface_rgba),
            border: rgb(theme.colors.ui.error),
            value,
        },
        PromptFieldState::ReadOnly => PromptFieldStyle {
            background: rgba(chrome.selection_rgba),
            border: rgba(chrome.badge_border_rgba),
            value: rgb(chrome.text_primary_hex),
        },
    }
}

/// Single-line text field card using the shared prompt surface.
pub(crate) fn prompt_text_field(value: impl Into<SharedString>, style: PromptFieldStyle) -> Div {
    prompt_single_line_control_surface(style.background, style.border)
        .flex()
        .items_center()
        .child(
            div()
                .w_full()
                .text_sm()
                .text_color(style.value)
                .child(value.into()),
        )
}

/// Multi-line detail card with headline, supporting text, and detail text rows.
#[allow(dead_code, clippy::too_many_arguments)]
pub(crate) fn prompt_detail_card(
    headline: impl Into<SharedString>,
    supporting_text: impl Into<SharedString>,
    detail_text: impl Into<SharedString>,
    headline_color: Rgba,
    supporting_color: Rgba,
    detail_color: Rgba,
    style: PromptFieldStyle,
    gap_px: f32,
) -> Div {
    prompt_surface(style.background, style.border).child(
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(gap_px))
            .child(
                div()
                    .text_sm()
                    .text_color(headline_color)
                    .child(headline.into()),
            )
            .child(prompt_form_help(supporting_text, supporting_color))
            .child(prompt_form_help(detail_text, detail_color)),
    )
}

/// Horizontally scrollable single-line value for long paths or strings.
#[allow(dead_code)]
pub(crate) fn prompt_scroll_value(
    value: impl Into<SharedString>,
    color: Rgba,
) -> gpui::Stateful<Div> {
    prompt_scroll_value_with_id("prompt-scroll-value", value, color)
}

/// Horizontally scrollable single-line value with a custom element ID.
///
/// Use this when multiple scroll values appear in the same view to avoid
/// duplicate element IDs.
pub(crate) fn prompt_scroll_value_with_id(
    id: impl Into<gpui::ElementId>,
    value: impl Into<SharedString>,
    color: Rgba,
) -> gpui::Stateful<Div> {
    div()
        .id(id.into())
        .w_full()
        .overflow_x_scroll()
        .overflow_y_hidden()
        .child(
            div()
                .text_xs()
                .text_color(color)
                .whitespace_nowrap()
                .child(value.into()),
        )
}

/// Shared outer shell used by prompt wrappers in `render_prompts/*`.
///
/// This normalizes the frame layout for prompt views:
/// - relative root for overlays
/// - column flex flow
/// - full-width/full-height frame
/// - clipped content with rounded corners
pub fn prompt_shell_container(radius: f32, vibrancy_bg: Option<Rgba>) -> Div {
    prompt_frame_root(prompt_shell_frame_config(radius)).when_some(vibrancy_bg, |d, bg| d.bg(bg))
}

/// Shared content slot used by prompt wrappers.
///
/// This guarantees consistent flex/overflow behavior for the inner prompt entity.
pub fn prompt_shell_content(content: impl IntoElement) -> Div {
    prompt_frame_fill_content(content)
}

/// Shared outer shell for minimal-chrome prompt surfaces.
///
/// Combines `prompt_shell_container` + `prompt_shell_content` with an optional
/// footer element (typically a `HintStrip`). Callers pass body content and an
/// optional `AnyElement` footer — the shell handles the column layout, vibrancy
/// background, and rounded corners.
#[allow(dead_code)]
pub(crate) fn render_simple_prompt_shell(
    radius: f32,
    vibrancy_bg: Option<Rgba>,
    body: impl IntoElement,
    footer: Option<AnyElement>,
) -> Div {
    let shell = prompt_shell_container(radius, vibrancy_bg).child(prompt_shell_content(body));

    if let Some(footer) = footer {
        shell.child(footer)
    } else {
        shell
    }
}

#[allow(dead_code)]
pub(crate) fn render_minimal_list_prompt_scaffold(
    header: impl IntoElement,
    content: impl IntoElement,
    hints: impl crate::components::hint_strip::IntoHints,
    leading: Option<AnyElement>,
) -> Div {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .px(px(crate::ui::chrome::HEADER_PADDING_X))
                .py(px(crate::ui::chrome::HEADER_PADDING_Y))
                .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(header),
        )
        // Divider matching main menu
        .child(render_header_divider())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                .child(content),
        )
        .child(render_simple_hint_strip(hints, leading))
}

#[allow(dead_code)]
pub(crate) fn render_minimal_list_prompt_shell(
    radius: f32,
    vibrancy_bg: Option<Rgba>,
    header: impl IntoElement,
    content: impl IntoElement,
    hints: impl crate::components::hint_strip::IntoHints,
    leading: Option<AnyElement>,
) -> Div {
    render_simple_prompt_shell(
        radius,
        vibrancy_bg,
        render_minimal_list_prompt_scaffold(header, content, hints, leading),
        None,
    )
}

/// Footer-aware variant of [`render_minimal_list_prompt_shell`].
///
/// Accepts a pre-built footer element (typically from `main_window_footer_slot`)
/// instead of raw hints, so callers can swap between the native AppKit footer
/// spacer and the GPUI hint strip without duplicating scaffold logic.
#[allow(dead_code)]
pub(crate) fn render_minimal_list_prompt_shell_with_footer(
    radius: f32,
    vibrancy_bg: Option<Rgba>,
    header: impl IntoElement,
    content: impl IntoElement,
    footer: Option<AnyElement>,
) -> Div {
    let scaffold = div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .child(
            div()
                .w_full()
                .px(px(crate::ui::chrome::HEADER_PADDING_X))
                .py(px(crate::ui::chrome::HEADER_PADDING_Y))
                .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(header),
        )
        // Divider matching main menu
        .child(render_header_divider())
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                .child(content),
        );

    let scaffold = if let Some(footer) = footer {
        scaffold.child(footer)
    } else {
        scaffold
    };

    render_simple_prompt_shell(radius, vibrancy_bg, scaffold, None)
}

/// Shared scaffold for expanded-view surfaces (list + preview split).
///
/// Composes a header row, a chromeless 50/50 split content area (list left,
/// preview right), and the canonical three-key hint strip footer via
/// [`universal_prompt_hints`]. No `SectionDivider`, no rounded preview wrapper,
/// no hardcoded opacity literals — all chrome defers to the caller's content.
///
/// `header` is the full-width header element (typically an input row).
/// `list_pane` is the left half (mini-style list).
/// `preview_pane` is the right half (chromeless preview slot).
#[allow(dead_code)]
pub(crate) fn render_expanded_view_scaffold(
    header: impl IntoElement,
    list_pane: impl IntoElement,
    preview_pane: impl IntoElement,
) -> Div {
    let hints = universal_prompt_hints();

    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        // Header row with shared padding
        .child(
            div()
                .w_full()
                .px(px(crate::ui::chrome::HEADER_PADDING_X))
                .py(px(crate::ui::chrome::HEADER_PADDING_Y))
                .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(header),
        )
        // Divider matching main menu
        .child(render_header_divider())
        // 50/50 split content area — no wrapper chrome
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                // Left: mini-style list pane
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(list_pane),
                )
                // Right: chromeless preview slot
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(preview_pane),
                ),
        )
        // Footer — canonical three-key hint strip
        .child(render_simple_hint_strip(hints, None))
}

/// Expanded-view scaffold with caller-supplied hints and optional leading element.
///
/// Same layout as [`render_expanded_view_scaffold`] but lets the caller specify
/// custom footer hints instead of the generic [`universal_prompt_hints`].
#[allow(dead_code)]
pub(crate) fn render_expanded_view_scaffold_with_hints(
    header: impl IntoElement,
    list_pane: impl IntoElement,
    preview_pane: impl IntoElement,
    hints: impl crate::components::hint_strip::IntoHints,
    leading: Option<AnyElement>,
) -> Div {
    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        // Header row with shared padding
        .child(
            div()
                .w_full()
                .px(px(crate::ui::chrome::HEADER_PADDING_X))
                .py(px(crate::ui::chrome::HEADER_PADDING_Y))
                .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(header),
        )
        // Divider matching main menu
        .child(render_header_divider())
        // 50/50 split content area — no wrapper chrome
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                // Left: mini-style list pane
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(list_pane),
                )
                // Right: chromeless preview slot
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(preview_pane),
                ),
        )
        // Footer — caller-supplied hints
        .child(render_simple_hint_strip(hints, leading))
}

/// Footer-aware variant of [`render_expanded_view_scaffold_with_hints`].
///
/// Accepts a pre-built footer element (typically from `main_window_footer_slot`)
/// instead of raw hints, so callers can swap between the native AppKit footer
/// spacer and the GPUI hint strip without duplicating scaffold logic.
#[allow(dead_code)]
pub(crate) fn render_expanded_view_scaffold_with_footer(
    header: impl IntoElement,
    list_pane: impl IntoElement,
    preview_pane: impl IntoElement,
    footer: Option<AnyElement>,
) -> Div {
    let scaffold = div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        // Header row with shared padding
        .child(
            div()
                .w_full()
                .px(px(crate::ui::chrome::HEADER_PADDING_X))
                .py(px(crate::ui::chrome::HEADER_PADDING_Y))
                .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
                .flex()
                .flex_row()
                .items_center()
                .child(header),
        )
        // Divider matching main menu
        .child(render_header_divider())
        // 50/50 split content area — no wrapper chrome
        .child(
            div()
                .flex()
                .flex_row()
                .flex_1()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                // Left: mini-style list pane
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(list_pane),
                )
                // Right: chromeless preview slot
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(preview_pane),
                ),
        );

    if let Some(footer) = footer {
        scaffold.child(footer)
    } else {
        scaffold
    }
}

/// Expanded-view scaffold wrapped in the shared prompt shell container.
///
/// Same as [`render_expanded_view_scaffold`] but wrapped in
/// `prompt_shell_container` for surfaces that need rounded corners and
/// vibrancy background.
#[allow(dead_code)]
pub(crate) fn render_expanded_view_prompt_shell(
    radius: f32,
    vibrancy_bg: Option<Rgba>,
    header: impl IntoElement,
    list_pane: impl IntoElement,
    preview_pane: impl IntoElement,
) -> Div {
    render_simple_prompt_shell(
        radius,
        vibrancy_bg,
        render_expanded_view_scaffold(header, list_pane, preview_pane),
        None,
    )
}

/// Build a hint-strip footer with optional leading status text.
///
/// Renders footer hints as the shared footer-chrome button rail, optionally
/// with a leading element (e.g., contextual status text) in the native
/// footer's left-info slot. Every surface that feeds this into a footer slot
/// gets the universal footer language — rail geometry, button frames, and
/// per-token keycap nudges — so builtin footers cannot drift from the main
/// window. (Formerly wrapped the text `HintStrip`, which is now reserved for
/// non-footer hint rows.)
#[allow(dead_code)]
pub(crate) fn render_simple_hint_strip(
    hints: impl crate::components::hint_strip::IntoHints,
    leading: Option<AnyElement>,
) -> AnyElement {
    crate::components::footer_chrome::render_static_footer_hint_action_rail_with_leading(
        "simple-hint-strip-footer-rail",
        crate::components::hint_strip::IntoHints::into_hints(hints),
        leading,
    )
}

/// Render muted leading text for a minimal hint strip footer.
///
/// Computes the text color from a theme text color (`0xAARRGGBB`) combined with
/// [`HINT_TEXT_OPACITY`] so callers avoid duplicating the opacity math.
#[allow(dead_code)]
pub(crate) fn render_hint_strip_leading_text(
    text: impl Into<SharedString>,
    text_primary: u32,
) -> AnyElement {
    let metrics = crate::components::footer_chrome::current_main_menu_footer_metrics();
    div()
        .font_family(crate::list_item::FONT_SYSTEM_UI)
        .text_size(px(metrics.label_font_size))
        .whitespace_nowrap()
        .text_color(rgba(
            ((text_primary & 0x00FF_FFFF) << 8)
                | crate::ui::chrome::alpha_from_opacity(crate::ui::chrome::HINT_TEXT_OPACITY),
        ))
        .child(text.into())
        .into_any_element()
}

/// Number of footer hints the design spec mandates: `↵ Run`, `⌘K Actions`, Agent.
pub(crate) const UNIVERSAL_PROMPT_HINT_COUNT: usize = 3;

/// The canonical three-key footer hints from `.impeccable.md`.
#[allow(dead_code)]
#[inline]
pub(crate) fn universal_prompt_hints() -> Vec<SharedString> {
    universal_prompt_hints_with_primary_label("Run")
}

#[allow(dead_code)]
#[inline]
pub(crate) fn universal_prompt_hints_with_primary_label(
    primary_label: impl AsRef<str>,
) -> Vec<SharedString> {
    universal_prompt_hints_with_primary_key_label("↵", primary_label)
}

#[allow(dead_code)]
#[inline]
pub(crate) fn universal_prompt_hints_with_primary_key_label(
    primary_key: impl AsRef<str>,
    primary_label: impl AsRef<str>,
) -> Vec<SharedString> {
    vec![
        format!("{} {}", primary_key.as_ref(), primary_label.as_ref()).into(),
        "⌘K Actions".into(),
        crate::ai::agent_chat::ui::labels::AGENT_CHAT_CMD_ENTER_HINT.into(),
    ]
}

/// Surface-specific footer hints for the tab-through template prompt.
#[allow(dead_code)]
#[inline]
pub(crate) fn template_prompt_hints() -> Vec<SharedString> {
    vec![
        "↵ Submit".into(),
        "⇥ Next Field".into(),
        "⌘K Actions".into(),
    ]
}

/// Surface-specific footer hints for the SDK editor prompt.
///
/// The editor must not use the universal set: plain Enter inserts a newline
/// (submit is ⌘↵/⌘S) and ⌘↵ is reserved by the editor for submit, so both
/// "↵ Run" and "⌘↵ Agent" would lie on this surface.
#[allow(dead_code)]
#[inline]
pub(crate) fn editor_prompt_hints() -> Vec<SharedString> {
    vec!["⌘↵ Submit".into(), "⌘K Actions".into(), "Esc Cancel".into()]
}

/// Zero-argument renderer for the canonical three-key footer.
#[allow(dead_code)]
#[inline]
pub(crate) fn render_universal_prompt_hint_strip() -> AnyElement {
    render_simple_hint_strip(universal_prompt_hints(), None)
}

/// Transparent spacer div matching the native footer height.
///
/// Used in place of the GPUI hint strip when the native NSVisualEffectView
/// footer is active, so content doesn't get hidden behind the AppKit footer.
#[allow(dead_code)]
pub(crate) fn render_native_main_window_footer_spacer() -> AnyElement {
    div()
        .id("native-main-window-footer-spacer")
        .debug_selector(|| "native-main-window-footer-spacer".to_string())
        .w_full()
        .h(px(
            crate::components::footer_chrome::current_main_menu_footer_height(),
        ))
        .into_any_element()
}

/// Transparent absolute hit-test layer for surfaces whose content should keep
/// flowing behind the native main-window footer material.
#[allow(dead_code)]
pub(crate) fn render_native_main_window_footer_hover_blocker() -> AnyElement {
    // Floating footer chrome: the capsules hang below the container in a
    // transparent window strip, so nothing overlaps in-container content and
    // a blocker would swallow hovers/clicks on the last visible rows.
    if crate::footer_popup::glass_scroll_bands_active() {
        return div().into_any_element();
    }
    gpui::deferred(
        div()
            .id("native-main-window-footer-hover-blocker")
            .absolute()
            .bottom_0()
            .left_0()
            .w_full()
            .h(px(
                crate::components::footer_chrome::current_main_menu_footer_height(),
            ))
            .block_mouse_except_scroll(),
    )
    .into_any_element()
}

/// Return a GPUI fallback footer or the native-footer spacer for prompt entities.
///
/// Prompt entities cannot call `ScriptListApp::main_window_footer_slot`, but
/// they still need one shared policy for native footer fallback ownership.
#[allow(dead_code)]
pub(crate) fn render_main_window_footer_slot_for_prompt_surface(
    expected_surface: &'static str,
    render_gpui_footer: impl FnOnce() -> AnyElement,
) -> AnyElement {
    if !crate::is_main_window_visible() {
        return render_gpui_footer();
    }

    match crate::footer_popup::active_main_window_footer_surface() {
        Some(active_surface) if active_surface == expected_surface => {
            render_native_main_window_footer_spacer()
        }
        Some(active_surface) => {
            tracing::warn!(
                target: "script_kit::prompt_chrome",
                event = "native_footer_surface_mismatch",
                expected_surface,
                active_surface,
                "Prompt renderer saw a different installed native footer surface; rendering GPUI fallback so stale native state cannot suppress prompt chrome"
            );
            render_gpui_footer()
        }
        None => render_gpui_footer(),
    }
}

#[allow(dead_code)]
pub(crate) fn main_window_footer_slot_for_prompt_surface(
    expected_surface: &'static str,
    render_gpui_footer: impl FnOnce() -> AnyElement,
) -> Option<AnyElement> {
    Some(render_main_window_footer_slot_for_prompt_surface(
        expected_surface,
        render_gpui_footer,
    ))
}

/// Renderer for the canonical three-key footer with click handlers.
///
/// `on_run` fires for "↵ Run", `on_actions` for "⌘K Actions", `on_ai` for "⌘↵ Agent".
#[allow(dead_code)]
pub(crate) fn render_universal_prompt_hint_strip_clickable(
    on_run: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    render_universal_prompt_hint_strip_clickable_with_primary_label(
        "Run", on_run, on_actions, on_ai,
    )
}

#[allow(dead_code)]
pub(crate) fn render_universal_prompt_hint_strip_clickable_with_primary_label(
    primary_label: impl AsRef<str>,
    on_run: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    render_universal_prompt_hint_strip_clickable_with_primary_key_label(
        "↵",
        primary_label,
        on_run,
        on_actions,
        on_ai,
    )
}

#[allow(dead_code)]
pub(crate) fn render_universal_prompt_hint_strip_clickable_with_primary_key_label(
    primary_key: impl AsRef<str>,
    primary_label: impl AsRef<str>,
    on_run: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    crate::components::HintStrip::new(universal_prompt_hints_with_primary_key_label(
        primary_key,
        primary_label,
    ))
    .on_hint_click(0, "prompt-footer-run", on_run)
    .on_hint_click(1, "prompt-footer-actions", on_actions)
    .on_hint_click(2, "prompt-footer-agent", on_ai)
    .into_any_element()
}

/// Canonical three-slot footer BUTTON row — the same `footer_chrome` button
/// frame (label + one keycap per key) the main window footer renders. Window
/// footers must use this instead of a text `HintStrip` so every surface's
/// footer buttons share one component and one keycap language.
#[allow(dead_code)]
pub(crate) fn render_universal_footer_action_buttons(
    id_prefix: &'static str,
    primary_key: &'static str,
    primary_label: impl Into<SharedString>,
    on_primary: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let frames = render_universal_footer_action_button_frames(
        id_prefix,
        primary_key,
        primary_label,
        on_primary,
        on_actions,
        on_ai,
    );
    let theme = crate::theme::get_cached_theme();
    let rail = crate::components::footer_chrome::footer_rail_chrome(&theme);

    div()
        .flex()
        .flex_none()
        .flex_row()
        .items_center()
        .gap(px(rail.item_gap_px))
        .children(frames)
        .into_any_element()
}

/// One shared frame builder so every universal footer button — with or
/// without a surface primary — keeps the same pill/keycap language.
fn universal_footer_button_frame(
    theme: &crate::theme::Theme,
    id: &'static str,
    label: SharedString,
    key: &'static str,
    slot_width_px: f32,
    height_px: f32,
) -> gpui::Stateful<gpui::Div> {
    use crate::components::footer_chrome::{
        render_footer_hint_action_button_frame, FooterHintActionButtonFrameSpec,
        FooterHintButtonLayoutOverrides, FooterHintContentJustify,
    };
    render_footer_hint_action_button_frame(
        FooterHintActionButtonFrameSpec {
            id: id.into(),
            label,
            key: SharedString::from(key),
            slot_width_px,
            height_px,
            selected: false,
            key_first: false,
            justify: FooterHintContentJustify::Center,
            layout: FooterHintButtonLayoutOverrides {
                // Flexbox-native: pill AND slot hug label + keycaps (slot
                // width is only a max bound), so the row stays whole in
                // narrow windows instead of truncating labels.
                shrink_frame_to_content_px: true,
                hug_frame_to_content: true,
                ..FooterHintButtonLayoutOverrides::default()
            },
        },
        theme,
    )
}

/// Actions + Agent footer frames WITHOUT a surface-primary button — for
/// surfaces whose primary affordance is keyboard-only. Notes uses this: the
/// ⌘P note-switcher keybinding stays, but its third capsule crowded the
/// footer and clipped the status/mention text (user report 2026-07-26).
pub(crate) fn render_footer_actions_agent_button_frames(
    id_prefix: &'static str,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Vec<AnyElement> {
    use crate::components::footer_chrome::{
        footer_action_slot_width, footer_button_height, footer_rail_chrome, FooterActionSlot,
    };

    let theme = crate::theme::get_cached_theme();
    let rail = footer_rail_chrome(&theme);
    let height = footer_button_height(rail.height_px);
    let (actions_id, ai_id) = match id_prefix {
        "notes" => ("notes-footer-actions", "notes-footer-ai"),
        _ => ("universal-footer-actions", "universal-footer-ai"),
    };

    vec![
        (universal_footer_button_frame(
            &theme,
            actions_id,
            SharedString::from("Actions"),
            "⌘K",
            footer_action_slot_width(FooterActionSlot::Actions),
            height,
        )
        .on_click(move |event, window, cx| on_actions(event, window, cx))
        .into_any_element()),
        (universal_footer_button_frame(
            &theme,
            ai_id,
            SharedString::from("Agent"),
            "⌘↵",
            footer_action_slot_width(FooterActionSlot::Ai),
            height,
        )
        .on_click(move |event, window, cx| on_ai(event, window, cx))
        .into_any_element()),
    ]
}

pub(crate) fn render_universal_footer_action_button_frames(
    id_prefix: &'static str,
    primary_key: &'static str,
    primary_label: impl Into<SharedString>,
    on_primary: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Vec<AnyElement> {
    use crate::components::footer_chrome::{
        footer_action_slot_width, footer_button_height, footer_rail_chrome, FooterActionSlot,
    };

    let theme = crate::theme::get_cached_theme();
    let primary_label = primary_label.into();
    let rail = footer_rail_chrome(&theme);
    let height = footer_button_height(rail.height_px);
    // Ids are formatted per-surface via the prefix so multiple windows can host
    // the row without colliding element ids.
    let primary_id = match id_prefix {
        "notes" => "notes-footer-primary",
        _ => "universal-footer-primary",
    };

    let mut frames = vec![
        (universal_footer_button_frame(
            &theme,
            primary_id,
            primary_label,
            primary_key,
            footer_action_slot_width(FooterActionSlot::Run),
            height,
        )
        .on_click(move |event, window, cx| on_primary(event, window, cx))
        .into_any_element()),
    ];
    frames.extend(render_footer_actions_agent_button_frames(
        id_prefix, on_actions, on_ai,
    ));
    frames
}

/// Canonical in-window universal footer: native rail geometry containing the
/// shared footer action-button frames and keycaps.
#[allow(dead_code)] // callers live in binary-only render modules
pub(crate) fn render_universal_footer_action_rail(
    id_prefix: &'static str,
    primary_key: &'static str,
    primary_label: SharedString,
    on_primary: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_actions: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_ai: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let buttons = render_universal_footer_action_button_frames(
        id_prefix,
        primary_key,
        primary_label,
        on_primary,
        on_actions,
        on_ai,
    );
    crate::components::footer_chrome::render_footer_action_rail(
        "universal-footer-action-rail",
        buttons,
    )
}

/// Returns `true` when `hints` matches the canonical three-key ANATOMY in
/// exact order: an `↵`-primary (any truthful verb — `Run`, `Paste`,
/// `Open App`… — the blessed `universal_prompt_hints_with_primary_label`
/// variants), then `⌘K Actions`, then the Agent hint. A different primary
/// KEY (`⌘↵ Submit`, `⇥ Next Field`) or any other slot change is not
/// universal and belongs in `emit_surface_prompt_hint_audit`. Chaos battery
/// 06: the exact-literal check false-flagged every blessed relabel
/// (clipboard history's `↵ Paste`) as a contract violation on activation.
#[allow(dead_code)]
#[inline]
pub(crate) fn is_universal_prompt_hints(hints: &[SharedString]) -> bool {
    if hints.len() != UNIVERSAL_PROMPT_HINT_COUNT {
        return false;
    }
    let primary = hints[0].as_ref();
    primary
        .strip_prefix("↵ ")
        .is_some_and(|label| !label.trim().is_empty())
        && hints[1].as_ref() == "⌘K Actions"
        && hints[2].as_ref() == crate::ai::agent_chat::ui::labels::AGENT_CHAT_CMD_ENTER_HINT
}

/// Structured audit record for a prompt surface's footer hints.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub(crate) struct PromptHintAudit {
    pub surface: &'static str,
    pub hint_count: usize,
    pub hints_joined: String,
    pub is_universal: bool,
}

fn seen_prompt_hint_audits() -> &'static Mutex<HashSet<PromptHintAudit>> {
    static SEEN: OnceLock<Mutex<HashSet<PromptHintAudit>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

fn mark_prompt_hint_audit_seen(audit: &PromptHintAudit) -> bool {
    let mut seen = seen_prompt_hint_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    seen.insert(audit.clone())
}

/// Emit a structured log line describing the footer hints for a prompt surface.
///
/// Emits a warning when the footer diverges from the canonical three-key contract.
/// Identical audits are emitted at most once per process.
#[allow(dead_code)]
pub(crate) fn emit_prompt_hint_audit(surface: &'static str, hints: &[SharedString]) {
    let actual: Vec<String> = hints.iter().map(|h| h.to_string()).collect();
    let audit = PromptHintAudit {
        surface,
        hint_count: actual.len(),
        hints_joined: actual.join(" | "),
        is_universal: is_universal_prompt_hints(hints),
    };

    if !mark_prompt_hint_audit_seen(&audit) {
        return;
    }

    tracing::info!(
        target: "script_kit::prompt_chrome",
        event = "prompt_hint_audit",
        surface = audit.surface,
        hint_count = audit.hint_count,
        hints = %audit.hints_joined,
        is_universal = audit.is_universal,
        "prompt hint audit"
    );

    if !audit.is_universal {
        tracing::warn!(
            target: "script_kit::prompt_chrome",
            event = "prompt_hint_contract_violation",
            surface = audit.surface,
            expected = "↵ Run | ⌘K Actions | ⌘↵ Agent",
            actual = %audit.hints_joined,
            "prompt footer diverged from universal three-key contract"
        );
    }
}

/// Emit a structured log line for an intentional surface-specific prompt footer.
///
/// These footers stay capped at the same three-affordance budget as the
/// universal prompt hints, but their labels reflect surface-owned actions.
#[allow(dead_code)]
pub(crate) fn emit_surface_prompt_hint_audit(
    surface: &'static str,
    hints: &[SharedString],
    reason: &'static str,
) {
    let actual: Vec<String> = hints.iter().map(|h| h.to_string()).collect();
    let audit = PromptHintAudit {
        surface,
        hint_count: actual.len(),
        hints_joined: actual.join(" | "),
        is_universal: is_universal_prompt_hints(hints),
    };

    if !mark_prompt_hint_audit_seen(&audit) {
        return;
    }

    tracing::info!(
        target: "script_kit::prompt_chrome",
        event = "surface_prompt_hint_audit",
        surface = audit.surface,
        hint_count = audit.hint_count,
        hints = %audit.hints_joined,
        is_universal = audit.is_universal,
        reason,
        "surface-specific prompt hint audit"
    );

    if audit.hint_count == 0 || audit.hint_count > UNIVERSAL_PROMPT_HINT_COUNT {
        tracing::warn!(
            target: "script_kit::prompt_chrome",
            event = "prompt_hint_contract_violation",
            surface = audit.surface,
            max_hint_count = UNIVERSAL_PROMPT_HINT_COUNT,
            actual = %audit.hints_joined,
            reason,
            "surface-specific prompt footer violated the three-affordance budget"
        );
    }
}

/// Machine-readable contract describing how a prompt surface resolves its chrome.
///
/// Emitted via [`emit_prompt_chrome_audit`] at surface-activation time (not per-frame)
/// so that agents and structured-log consumers can verify which surfaces are minimal,
/// which are intentional exceptions, and which have silently drifted.
///
/// The `layout_mode` field encodes the surface layout decision from `.impeccable.md`:
/// `"mini"`, `"editor"`, `"expanded"`, `"grid"`, or `"custom"` (for exceptions).
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize)]
pub(crate) struct PromptChromeAudit {
    pub surface: &'static str,
    pub layout_mode: &'static str,
    pub input_mode: &'static str,
    pub divider_mode: &'static str,
    pub footer_mode: &'static str,
    pub header_padding_x: u16,
    pub header_padding_y: u16,
    pub hint_count: usize,
    pub has_leading_status: bool,
    pub has_actions: bool,
    pub exception_reason: Option<&'static str>,
}

#[allow(dead_code)]
impl PromptChromeAudit {
    /// Contract for a mini list surface (name IS the content — script, app, process).
    pub(crate) fn minimal_list(surface: &'static str, has_actions: bool) -> Self {
        Self {
            surface,
            layout_mode: "mini",
            input_mode: "bare",
            divider_mode: "none",
            footer_mode: "hint_strip",
            header_padding_x: crate::ui::chrome::HEADER_PADDING_X as u16,
            header_padding_y: crate::ui::chrome::HEADER_PADDING_Y as u16,
            hint_count: UNIVERSAL_PROMPT_HINT_COUNT,
            has_leading_status: false,
            has_actions,
            exception_reason: None,
        }
    }

    /// Contract for an editor surface (justified exception — full editor area).
    pub(crate) fn editor(surface: &'static str, has_actions: bool) -> Self {
        Self {
            surface,
            layout_mode: "editor",
            input_mode: "bare",
            divider_mode: "none",
            footer_mode: "hint_strip",
            header_padding_x: crate::ui::chrome::HEADER_PADDING_X as u16,
            header_padding_y: crate::ui::chrome::HEADER_PADDING_Y as u16,
            hint_count: UNIVERSAL_PROMPT_HINT_COUNT,
            has_leading_status: false,
            has_actions,
            exception_reason: None,
        }
    }

    /// Contract for an expanded view surface (preview IS the decision — clipboard, files, themes).
    pub(crate) fn expanded(surface: &'static str, has_actions: bool) -> Self {
        Self {
            surface,
            layout_mode: "expanded",
            input_mode: "bare",
            divider_mode: "none",
            footer_mode: "hint_strip",
            header_padding_x: crate::ui::chrome::HEADER_PADDING_X as u16,
            header_padding_y: crate::ui::chrome::HEADER_PADDING_Y as u16,
            hint_count: UNIVERSAL_PROMPT_HINT_COUNT,
            has_leading_status: false,
            has_actions,
            exception_reason: None,
        }
    }

    /// Contract for a grid surface (visual scan content — emoji, icons).
    pub(crate) fn grid(surface: &'static str, has_actions: bool) -> Self {
        Self {
            surface,
            layout_mode: "grid",
            input_mode: "bare",
            divider_mode: "none",
            footer_mode: "hint_strip",
            header_padding_x: crate::ui::chrome::HEADER_PADDING_X as u16,
            header_padding_y: crate::ui::chrome::HEADER_PADDING_Y as u16,
            hint_count: UNIVERSAL_PROMPT_HINT_COUNT,
            has_leading_status: false,
            has_actions,
            exception_reason: None,
        }
    }

    /// Backward-compatible adapter for existing minimal callers.
    ///
    /// Accepts the legacy `hint_count` and `has_leading_status` parameters for
    /// source compatibility. New call sites should prefer [`Self::minimal_list`].
    pub(crate) fn minimal(
        surface: &'static str,
        hint_count: usize,
        has_leading_status: bool,
        has_actions: bool,
    ) -> Self {
        Self {
            surface,
            layout_mode: "mini",
            input_mode: "bare",
            divider_mode: "none",
            footer_mode: "hint_strip",
            header_padding_x: crate::ui::chrome::HEADER_PADDING_X as u16,
            header_padding_y: crate::ui::chrome::HEADER_PADDING_Y as u16,
            hint_count,
            has_leading_status,
            has_actions,
            exception_reason: None,
        }
    }

    /// Contract for a surface that intentionally keeps rich chrome (PromptFooter).
    pub(crate) fn exception(surface: &'static str, reason: &'static str) -> Self {
        Self {
            surface,
            layout_mode: "custom",
            input_mode: "custom",
            divider_mode: "custom",
            footer_mode: "prompt_footer",
            header_padding_x: 0,
            header_padding_y: 0,
            hint_count: 0,
            has_leading_status: false,
            has_actions: false,
            exception_reason: Some(reason),
        }
    }
}

fn seen_prompt_chrome_audits() -> &'static Mutex<HashSet<PromptChromeAudit>> {
    static SEEN: OnceLock<Mutex<HashSet<PromptChromeAudit>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

/// Record an audit contract and return `true` if it was first-seen, `false` if duplicate.
///
/// Uses `Hash + Eq` on the full struct so any field change is treated as a new contract.
pub(crate) fn mark_prompt_chrome_audit_seen(audit: &PromptChromeAudit) -> bool {
    let mut seen = seen_prompt_chrome_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    seen.insert(audit.clone())
}

/// Emit a structured log line describing the chrome contract for a prompt surface.
///
/// Call this from surface-activation or configuration paths, **not** from `render()`.
/// Identical contracts are emitted at most once per process.
/// Non-exception surfaces that still resolve to `prompt_footer` emit a warning.
#[allow(dead_code)]
pub(crate) fn emit_prompt_chrome_audit(audit: &PromptChromeAudit) {
    if !mark_prompt_chrome_audit_seen(audit) {
        return;
    }

    tracing::info!(
        target: "script_kit::prompt_chrome",
        event = "prompt_chrome_audit",
        surface = audit.surface,
        layout_mode = audit.layout_mode,
        input_mode = audit.input_mode,
        divider_mode = audit.divider_mode,
        footer_mode = audit.footer_mode,
        header_padding_x = audit.header_padding_x,
        header_padding_y = audit.header_padding_y,
        hint_count = audit.hint_count,
        has_leading_status = audit.has_leading_status,
        has_actions = audit.has_actions,
        exception_reason = audit.exception_reason.unwrap_or(""),
        "prompt chrome audit"
    );

    if audit.exception_reason.is_none() && audit.footer_mode == "prompt_footer" {
        tracing::warn!(
            target: "script_kit::prompt_chrome",
            event = "prompt_chrome_contract_violation",
            surface = audit.surface,
            footer_mode = audit.footer_mode,
            "non-exception surface resolved to prompt_footer"
        );
    }
}

/// Renders a horizontal divider matching the main menu's header divider.
fn render_header_divider() -> Div {
    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
    div()
        .mx(px(crate::panel::HEADER_DIVIDER_MARGIN))
        .h(px(crate::panel::HEADER_DIVIDER_HEIGHT))
        .bg(rgba(chrome.divider_rgba))
}

#[cfg(test)]
include!("prompt_layout_shell_tests.rs");
