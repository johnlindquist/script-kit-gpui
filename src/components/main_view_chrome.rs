use gpui::{
    div, px, rgb, rgba, AnyElement, ClickEvent, InteractiveElement, IntoElement, ParentElement,
    SharedString, StatefulInteractiveElement, Styled,
};
use gpui_component::{input::Input, Sizable as _, Size};

use crate::designs::{MainMenuSearchTokens, MainMenuThemeDef};

pub(crate) const MAIN_VIEW_SHELL_ID: &str = "main-view-shell";
pub(crate) const MAIN_VIEW_INPUT_SHELL_ID: &str = "main-view-input-shell";
pub(crate) const MAIN_VIEW_INPUT_BODY_ID: &str = "main-view-input-body";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_CONTEXT_ZONE_ID: &str = "main-view-context-zone";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_CONTEXT_LOGO_ID: &str = "main-view-context-logo";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_CONTEXT_CWD_BUTTON_ID: &str = "main-view-context-cwd-button";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID: &str = "main-view-context-model-button";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID: &str = "main-view-context-selection-button";
pub(crate) const MAIN_VIEW_HEADER_ID: &str = "main-view-header";
pub(crate) const MAIN_VIEW_CWD_UNAVAILABLE_LABEL: &str = "No cwd";
pub(crate) const MAIN_VIEW_AGENT_MODEL_UNAVAILABLE_LABEL: &str = "Agent model unavailable";
const DEFAULT_CONTEXT_EDGE_OUTSET_X: f32 = 8.0;
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_HEADER_DIVIDER_ID: &str = "main-view-header-divider";
#[allow(dead_code)]
pub(crate) const MAIN_VIEW_MAIN_ID: &str = "main-view-main";
pub(crate) const MAIN_VIEW_OVERLAY_CLIP_ID: &str = "main-view-overlay-clip";
#[allow(dead_code)] // Used by the binary target through include!-merged built-in render code.
pub(crate) const MAIN_VIEW_SCROLL_FLOW_ID: &str = "main-view-scroll-flow";

/// What pressing Tab actually does on the surface rendering the context row.
/// The header Tab chip must always advertise the real action, so the owning
/// surface computes this from the same state the Tab interceptor branches on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MainViewTabChipAction {
    /// Tab opens the cwd picker — chip shows the cwd label with a ⇥ keycap.
    ChangeCwd,
    /// Tab sends the typed query to the zero-context Quick AI — chip swaps to
    /// a "Quick AI" label with the ⇥ keycap.
    #[allow(dead_code)] // Constructed only in the binary target (ui_window.rs).
    QuickAi,
    /// Tab does something else (or nothing) here — keep the cwd label for
    /// orientation but hide the ⇥ keycap so the chip never lies.
    Inactive,
}

pub(crate) const MAIN_VIEW_QUICK_AI_CHIP_LABEL: &str = "Quick AI";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MainViewContextLabels {
    pub(crate) cwd_label: String,
    pub(crate) agent_model_label: String,
    pub(crate) tab_action: MainViewTabChipAction,
    /// Whether Shift+Tab actually opens the agent/model (profile) picker on
    /// this surface. When false the agent-model chip drops its ⇧⇥ keycap.
    pub(crate) shift_tab_key_active: bool,
}

impl MainViewContextLabels {
    pub(crate) fn new(cwd_label: impl Into<String>, agent_model_label: impl Into<String>) -> Self {
        let cwd_label = non_empty_label(cwd_label.into(), MAIN_VIEW_CWD_UNAVAILABLE_LABEL);
        let agent_model_label = non_empty_label(
            agent_model_label.into(),
            MAIN_VIEW_AGENT_MODEL_UNAVAILABLE_LABEL,
        );

        Self {
            cwd_label,
            agent_model_label,
            tab_action: MainViewTabChipAction::ChangeCwd,
            shift_tab_key_active: true,
        }
    }

    pub(crate) fn with_tab_action(mut self, tab_action: MainViewTabChipAction) -> Self {
        self.tab_action = tab_action;
        self
    }

    #[allow(dead_code)] // WIP builder for the Quick AI Tab chip; caller lands with the mode.
    pub(crate) fn with_shift_tab_key_active(mut self, active: bool) -> Self {
        self.shift_tab_key_active = active;
        self
    }
}

/// Conditional "I see you have text selected" hint chip for the context zone.
/// Present only when the show-time passive AX sniff found a selection in the
/// app the user came from; clicking it routes into the `.style` rewrite flow.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MainViewSelectionHintChip {
    pub(crate) label: String,
}

/// Single-line preview of captured selected text for hint labels: whitespace
/// runs collapse to one space, and text longer than `max_chars` is cut at a
/// char boundary with a trailing ellipsis.
#[allow(dead_code)] // Used by the binary target through include!-merged render code.
pub(crate) fn selection_hint_snippet(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let truncated: String = collapsed.chars().take(max_chars).collect();
    format!("{}\u{2026}", truncated.trim_end())
}

pub(crate) struct MainViewInputChrome {
    pub(crate) body: AnyElement,
    pub(crate) trailing: Vec<AnyElement>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PromptSearchInputKind {
    EntityBacked,
    ControllerOwned,
    PathPrefix,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PromptSearchInputGeometry {
    height: f32,
    font_size: f32,
    radius: f32,
    text_inset_x: f32,
}

impl PromptSearchInputGeometry {
    fn from_main_menu(search: MainMenuSearchTokens) -> Self {
        Self {
            height: search.height,
            font_size: search.font_size,
            radius: search.radius,
            text_inset_x: search.text_inset_x,
        }
    }
}

enum PromptSearchInputContent {
    #[allow(dead_code)] // Constructed by binary-only include!-merged Arg/Mini renderers.
    EntityBacked { input: Input },
    ControllerOwned {
        semantic_id: &'static str,
        text: SharedString,
        placeholder: SharedString,
    },
    PathPrefix {
        semantic_id: &'static str,
        prefix: SharedString,
        text: SharedString,
        placeholder: SharedString,
    },
}

impl PromptSearchInputContent {
    fn kind(&self) -> PromptSearchInputKind {
        match self {
            Self::EntityBacked { .. } => PromptSearchInputKind::EntityBacked,
            Self::ControllerOwned { .. } => PromptSearchInputKind::ControllerOwned,
            Self::PathPrefix { .. } => PromptSearchInputKind::PathPrefix,
        }
    }
}

pub(crate) struct PromptSearchInputChrome {
    content: PromptSearchInputContent,
    trailing: Vec<AnyElement>,
}

impl PromptSearchInputChrome {
    #[allow(dead_code)] // Called by binary-only include!-merged Arg/Mini renderers.
    pub(crate) fn entity_backed(input: Input) -> Self {
        Self {
            content: PromptSearchInputContent::EntityBacked { input },
            trailing: Vec::new(),
        }
    }

    pub(crate) fn controller_owned(
        semantic_id: &'static str,
        text: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        Self {
            content: PromptSearchInputContent::ControllerOwned {
                semantic_id,
                text: text.into(),
                placeholder: placeholder.into(),
            },
            trailing: Vec::new(),
        }
    }

    pub(crate) fn path_prefix(
        semantic_id: &'static str,
        prefix: impl Into<SharedString>,
        text: impl Into<SharedString>,
        placeholder: impl Into<SharedString>,
    ) -> Self {
        Self {
            content: PromptSearchInputContent::PathPrefix {
                semantic_id,
                prefix: prefix.into(),
                text: text.into(),
                placeholder: placeholder.into(),
            },
            trailing: Vec::new(),
        }
    }

    pub(crate) fn with_trailing(mut self, trailing: AnyElement) -> Self {
        self.trailing.push(trailing);
        self
    }
}

fn prompt_search_input_geometry(
    search: MainMenuSearchTokens,
    _kind: PromptSearchInputKind,
) -> PromptSearchInputGeometry {
    PromptSearchInputGeometry::from_main_menu(search)
}

/// Adapt prompt-owned input state or text into the main menu's canonical
/// search shell without transferring editing behavior to the launcher.
pub(crate) fn render_prompt_search_input(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: PromptSearchInputChrome,
) -> AnyElement {
    let search = def.search;
    let geometry = prompt_search_input_geometry(search, chrome.content.kind());
    let colors = crate::theme::AppChromeColors::from_theme(theme);

    let body = match chrome.content {
        PromptSearchInputContent::EntityBacked { input } => input
            .w_full()
            .h(px(geometry.height))
            .line_height(px(geometry.height))
            .font_weight(search.font_weight)
            .px(px(0.0))
            .py(px(0.0))
            .with_size(Size::Size(px(geometry.font_size)))
            .appearance(false)
            .bordered(false)
            .focus_bordered(false)
            .into_any_element(),
        PromptSearchInputContent::ControllerOwned {
            semantic_id,
            text,
            placeholder,
        } => {
            let is_empty = text.is_empty();
            div()
                .id(semantic_id)
                .w_full()
                .h(px(geometry.height))
                .min_w(px(0.0))
                .flex()
                .items_center()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .text_size(px(geometry.font_size))
                .font_weight(search.font_weight)
                .line_height(px(geometry.height))
                .text_color(if is_empty {
                    rgba(colors.placeholder_text_rgba)
                } else {
                    rgb(colors.text_primary_hex)
                })
                .child(if is_empty { placeholder } else { text })
                .into_any_element()
        }
        PromptSearchInputContent::PathPrefix {
            semantic_id,
            prefix,
            text,
            placeholder,
        } => {
            let is_empty = text.is_empty();
            div()
                .id(semantic_id)
                .w_full()
                .h(px(geometry.height))
                .min_w(px(0.0))
                .flex()
                .flex_row()
                .items_center()
                .overflow_hidden()
                .text_size(px(geometry.font_size))
                .font_weight(search.font_weight)
                .line_height(px(geometry.height))
                .child(
                    div()
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(rgba(colors.text_hint_rgba))
                        .child(prefix),
                )
                .child(
                    div()
                        .min_w(px(0.0))
                        .flex_1()
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(if is_empty {
                            rgba(colors.placeholder_text_rgba)
                        } else {
                            rgb(colors.text_primary_hex)
                        })
                        .child(if is_empty { placeholder } else { text }),
                )
                .into_any_element()
        }
    };

    render_main_view_input_shell(
        theme,
        def,
        MainViewInputChrome {
            body,
            trailing: chrome.trailing,
        },
    )
}

pub(crate) struct MainViewHeaderChrome {
    context: Option<AnyElement>,
    input: Option<AnyElement>,
    padding_x: f32,
    padding_y: f32,
    gap: f32,
}

impl MainViewHeaderChrome {
    /// Canonical main-window context + input anatomy. Geometry always comes
    /// from the active main-menu theme; surfaces only provide the two bodies.
    pub(crate) fn canonical(def: MainMenuThemeDef, context: AnyElement, input: AnyElement) -> Self {
        let metrics = main_view_header_metrics(def, Some(def.search.height));
        Self {
            context: Some(context),
            input: Some(input),
            padding_x: metrics.padding_x,
            padding_y: metrics.padding_y,
            gap: metrics.gap,
        }
    }

    /// Intentional context-only anatomy for surfaces whose editable control
    /// lives in the main body (for example Day Page and root-owned prompts).
    pub(crate) fn context_only(def: MainMenuThemeDef, context: AnyElement) -> Self {
        let metrics = main_view_header_metrics(def, None);
        Self {
            context: Some(context),
            input: None,
            padding_x: metrics.padding_x,
            padding_y: metrics.padding_y,
            gap: metrics.gap,
        }
    }

    /// Placeholder accepted only by `render_main_view_chrome_without_header`;
    /// the header is not mounted, so it reserves no geometry.
    #[allow(dead_code)] // Binary-only Permissions renderer consumes this constructor.
    pub(crate) fn hidden() -> Self {
        Self {
            context: None,
            input: None,
            padding_x: 0.0,
            padding_y: 0.0,
            gap: 0.0,
        }
    }
}

/// Theme-derived geometry shared by real rendering and DevTools layout
/// receipts. `input_height = None` is the explicit context-only contract.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainViewHeaderMetrics {
    pub(crate) header_height: f32,
    pub(crate) padding_x: f32,
    pub(crate) padding_y: f32,
    pub(crate) gap: f32,
    pub(crate) context_x: f32,
    pub(crate) context_y: f32,
    pub(crate) context_height: f32,
    pub(crate) input_x: f32,
    pub(crate) input_y: f32,
    pub(crate) input_height: Option<f32>,
}

pub(crate) fn main_view_header_metrics(
    def: MainMenuThemeDef,
    input_height: Option<f32>,
) -> MainViewHeaderMetrics {
    let shell = def.shell;
    let context = def.header_info_bar;
    let input_height = input_height.map(|height| height.max(def.search.height));
    let input_y = shell.header_padding_y + context.height_px + shell.header_gap;
    let header_height = shell.header_padding_y * 2.0
        + context.height_px
        + input_height
            .map(|height| shell.header_gap + height)
            .unwrap_or(0.0);

    MainViewHeaderMetrics {
        header_height,
        padding_x: shell.header_padding_x,
        padding_y: shell.header_padding_y,
        gap: shell.header_gap,
        context_x: shell.header_padding_x - context.context_edge_outset_x,
        context_y: shell.header_padding_y,
        context_height: context.height_px,
        input_x: shell.header_padding_x,
        input_y,
        input_height,
    }
}

pub(crate) struct MainViewDividerChrome {
    pub(crate) margin_x: f32,
    pub(crate) height: f32,
    pub(crate) visible: bool,
}

pub(crate) struct MainViewChrome {
    pub(crate) header: MainViewHeaderChrome,
    pub(crate) divider: MainViewDividerChrome,
    pub(crate) main: AnyElement,
    pub(crate) footer: Option<AnyElement>,
    pub(crate) overlays: Vec<AnyElement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // Used by the binary target through include!-merged built-in render code.
pub(crate) struct MainViewFlowSpacing {
    pub(crate) inset_x: f32,
    pub(crate) inset_y: f32,
    pub(crate) section_gap: f32,
}

/// One horizontal owner for full-width main-window flows. `container_edge_x`
/// positions panels/lists; `text_plane_x` positions headings and prose so
/// they align with the shared list-row text column.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)] // Binary-only Permissions renderer consumes this shared frame.
pub(crate) struct MainViewContentFrame {
    pub(crate) container_edge_x: f32,
    pub(crate) text_plane_x: f32,
    pub(crate) inset_y: f32,
    pub(crate) section_gap: f32,
}

#[allow(dead_code)] // Binary-only Permissions renderer consumes this shared frame.
impl MainViewContentFrame {
    pub(crate) fn text_inset_x(self) -> f32 {
        (self.text_plane_x - self.container_edge_x).max(0.0)
    }
}

#[allow(dead_code)] // Used by the binary target through include!-merged built-in render code.
pub(crate) fn main_view_flow_spacing(
    def: MainMenuThemeDef,
    spacing: crate::designs::DesignSpacing,
) -> MainViewFlowSpacing {
    MainViewFlowSpacing {
        inset_x: def.shell.content_inset_x,
        inset_y: spacing.padding_sm,
        section_gap: spacing.gap_lg,
    }
}

#[allow(dead_code)] // Binary-only Permissions renderer consumes this shared frame.
pub(crate) fn main_view_content_frame(
    def: MainMenuThemeDef,
    spacing: crate::designs::DesignSpacing,
) -> MainViewContentFrame {
    let container_edge_x = def.shell.content_inset_x;
    MainViewContentFrame {
        container_edge_x,
        text_plane_x: container_edge_x + main_view_text_column_x(def),
        inset_y: spacing.padding_sm,
        section_gap: spacing.gap_lg,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MainViewColumnMetrics {
    pub(crate) shell_left_x: f32,
    pub(crate) row_leading_x: f32,
    pub(crate) text_column_x: f32,
    pub(crate) input_text_inset_left: f32,
    pub(crate) content_right_inset_x: f32,
    pub(crate) top_inset_y: f32,
}

pub(crate) fn render_main_view_shell() -> gpui::Stateful<gpui::Div> {
    div()
        .id(MAIN_VIEW_SHELL_ID)
        .debug_selector(|| MAIN_VIEW_SHELL_ID.to_string())
        .w_full()
        .h_full()
        .relative()
        .flex()
        .flex_col()
}

pub(crate) fn render_main_view_chrome(
    root: gpui::Stateful<gpui::Div>,
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewChrome,
) -> AnyElement {
    render_main_view_chrome_with_options(
        root,
        theme,
        def,
        chrome,
        MainViewHeaderPlacement::Flow,
        true,
    )
}

/// List surfaces that already reserve the footer through list padding, an
/// in-flow GPUI footer, or a native-footer spacer must not receive a second
/// bottom inset from the main slot.
#[allow(dead_code)] // Used by binary-target list renderers.
pub(crate) fn render_main_view_chrome_footer_flush(
    root: gpui::Stateful<gpui::Div>,
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewChrome,
) -> AnyElement {
    render_main_view_chrome_with_options(
        root,
        theme,
        def,
        chrome,
        MainViewHeaderPlacement::Flow,
        false,
    )
}

#[allow(dead_code)]
pub(crate) fn render_main_view_chrome_header_overlay_footer_flush(
    root: gpui::Stateful<gpui::Div>,
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewChrome,
) -> AnyElement {
    render_main_view_chrome_with_options(
        root,
        theme,
        def,
        chrome,
        MainViewHeaderPlacement::Overlay,
        false,
    )
}

/// Full-width utility surfaces can put their title inside the same scrollable
/// content frame as their body instead of reserving a separate header plane.
#[allow(dead_code)] // Binary-only full-width built-ins opt into body-owned titles.
pub(crate) fn render_main_view_chrome_without_header(
    root: gpui::Stateful<gpui::Div>,
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewChrome,
) -> AnyElement {
    render_main_view_chrome_with_options(
        root,
        theme,
        def,
        chrome,
        MainViewHeaderPlacement::Hidden,
        true,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MainViewHeaderPlacement {
    Flow,
    Overlay,
    Hidden,
}

fn render_main_view_chrome_with_options(
    mut root: gpui::Stateful<gpui::Div>,
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewChrome,
    header_placement: MainViewHeaderPlacement,
    include_main_bottom_inset: bool,
) -> AnyElement {
    let MainViewChrome {
        header,
        divider,
        main,
        footer,
        overlays,
    } = chrome;
    match header_placement {
        MainViewHeaderPlacement::Flow => {
            root = root.child(render_main_view_header_with_context_outset(
                header,
                def.header_info_bar.context_edge_outset_x,
            ));
            if divider.visible {
                root = root.child(render_main_view_header_divider(
                    theme,
                    def,
                    divider.margin_x,
                    divider.height,
                ));
            }
            root = root.child(render_main_view_main_slot_with_bottom_inset(
                def,
                main,
                include_main_bottom_inset,
            ));
        }
        MainViewHeaderPlacement::Overlay => {
            let header_height =
                main_view_header_metrics(def, Some(def.search.height)).header_height;
            root = root
                .child(render_main_view_overlay_main_slot(def, main, header_height))
                .child(div().absolute().top_0().left_0().right_0().child(
                    render_main_view_header_with_context_outset(
                        header,
                        def.header_info_bar.context_edge_outset_x,
                    ),
                ));
            if divider.visible {
                root = root.child(
                    div()
                        .absolute()
                        .top(px(header_height))
                        .left_0()
                        .right_0()
                        .child(render_main_view_header_divider(
                            theme,
                            def,
                            divider.margin_x,
                            divider.height,
                        )),
                );
            }
        }
        MainViewHeaderPlacement::Hidden => {
            root = root.child(render_main_view_main_slot_with_bottom_inset(
                def,
                main,
                include_main_bottom_inset,
            ));
        }
    }

    if let Some(footer) = footer {
        root = root.child(footer);
    }

    for element in overlays {
        root = root.child(element);
    }

    root.into_any_element()
}

pub(crate) fn render_main_view_header(chrome: MainViewHeaderChrome) -> AnyElement {
    render_main_view_header_with_context_outset(chrome, DEFAULT_CONTEXT_EDGE_OUTSET_X)
}

pub(crate) fn render_main_view_header_with_context_outset(
    chrome: MainViewHeaderChrome,
    context_edge_outset_x: f32,
) -> AnyElement {
    let mut header = div()
        .id(MAIN_VIEW_HEADER_ID)
        .debug_selector(|| MAIN_VIEW_HEADER_ID.to_string())
        .w_full()
        .px(px(chrome.padding_x))
        .py(px(chrome.padding_y))
        .min_h(px(crate::panel::HEADER_BUTTON_HEIGHT))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(chrome.gap));

    if let Some(context) = chrome.context {
        header = header.child(div().w_full().mx(px(-context_edge_outset_x)).child(context));
    }

    if let Some(input) = chrome.input {
        header = header.child(input);
    }

    header.into_any_element()
}

#[allow(dead_code)] // Used by the binary target through include!-merged render code.
pub(crate) fn render_main_view_context_header(
    def: MainMenuThemeDef,
    context: AnyElement,
) -> AnyElement {
    render_main_view_header_with_context_outset(
        MainViewHeaderChrome::context_only(def, context),
        def.header_info_bar.context_edge_outset_x,
    )
}

fn non_empty_label(label: String, fallback: &'static str) -> String {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        fallback.to_string()
    } else {
        label
    }
}

#[allow(dead_code)]
pub(crate) fn render_main_view_context_zone(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    cwd_label: Option<String>,
    agent_model_label: Option<String>,
    on_cwd_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_agent_model_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let labels = MainViewContextLabels::new(
        cwd_label.unwrap_or_else(|| MAIN_VIEW_CWD_UNAVAILABLE_LABEL.to_string()),
        agent_model_label.unwrap_or_else(|| MAIN_VIEW_AGENT_MODEL_UNAVAILABLE_LABEL.to_string()),
    );

    render_main_view_context_zone_required(
        theme,
        def,
        labels,
        None,
        on_cwd_click,
        on_agent_model_click,
        |_event, _window, _cx| {},
    )
}

/// Context-zone keycap sizing, shared with the design-contract exporter so
/// HTML mockups reproduce the exact derived values.
pub(crate) fn context_zone_keycap_font_size(info: &crate::designs::HeaderInfoBarTokens) -> f32 {
    (info.font_size * 0.88).max(8.0)
}

pub(crate) fn context_zone_keycap_height(info: &crate::designs::HeaderInfoBarTokens) -> f32 {
    (info.font_size + 7.0).max(16.0)
}

pub(crate) fn render_main_view_context_zone_required(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    labels: MainViewContextLabels,
    selection_hint: Option<MainViewSelectionHintChip>,
    on_cwd_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_agent_model_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_selection_click: impl Fn(&ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> AnyElement {
    let info = def.header_info_bar;
    let text_alpha = (info.opacity.clamp(0.0, 1.0) * 255.0).round() as u32;
    let border = rgba((theme.colors.ui.border << 8) | info.pill_border_alpha);
    let rest_bg = rgba((theme.colors.background.search_box << 8) | info.pill_bg_alpha);
    let hover_bg = rgba((theme.colors.text.primary << 8) | info.pill_hover_bg_alpha);
    let text_color = rgba((theme.colors.text.primary << 8) | text_alpha);
    let hover_text_color = rgba((theme.colors.text.primary << 8) | info.pill_hover_text_alpha);
    let show_pills = info.pill_padding_x > 0.0 || info.pill_border_alpha > 0;
    let header_keycap_font_size = context_zone_keycap_font_size(&info);
    let header_keycap_height = context_zone_keycap_height(&info);

    let agent_model_label = labels.agent_model_label;

    // The Tab chip always advertises the actual Tab action: the cwd label
    // when Tab opens the cwd picker, "Quick AI" when Tab submits the typed
    // query, and a keycap-less cwd label when Tab does neither here.
    let cwd_label = match labels.tab_action {
        MainViewTabChipAction::QuickAi => MAIN_VIEW_QUICK_AI_CHIP_LABEL.to_string(),
        MainViewTabChipAction::ChangeCwd | MainViewTabChipAction::Inactive => labels.cwd_label,
    };
    let tab_key_active = !matches!(labels.tab_action, MainViewTabChipAction::Inactive);

    // Inactive keeps rendering through the same hint-button component with an
    // empty key (which renders zero keycaps) instead of a bare text div: the
    // component's leading edge padding is what keeps the label's x-position
    // stable, so swapping components would make the chip jump horizontally
    // when Tab activates/deactivates (e.g. entering file navigation).
    let cwd_key = if info.show_keys {
        div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .opacity(info.key_opacity.clamp(0.0, 1.0))
            .child(
                crate::components::footer_chrome::render_footer_hint_button_like_shrinkable(
                    crate::components::footer_chrome::FooterHintButtonSpec {
                        label: cwd_label.clone().into(),
                        key: if tab_key_active { "⇥" } else { "" }.into(),
                        slot_width_px: None,
                        key_first: false,
                        justify: crate::components::footer_chrome::FooterHintContentJustify::Start,
                        label_font_size_px: Some(info.font_size),
                        keycap_font_size_px: Some(header_keycap_font_size),
                        keycap_height_px: Some(header_keycap_height),
                        hover_text_alpha: Some(info.pill_hover_text_alpha),
                        hover_glyph_alpha: Some(info.pill_hover_key_alpha),
                        hover_keycap_border_alpha: Some(info.pill_hover_border_alpha),
                    },
                    theme,
                ),
            )
            .into_any_element()
    } else {
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .child(cwd_label.clone())
            .into_any_element()
    };

    let model_key = if info.show_keys {
        div()
            .flex_1()
            .min_w(px(0.0))
            .overflow_hidden()
            .opacity(info.key_opacity.clamp(0.0, 1.0))
            .child(
                crate::components::footer_chrome::render_footer_hint_button_like_shrinkable(
                    crate::components::footer_chrome::FooterHintButtonSpec {
                        label: agent_model_label.clone().into(),
                        key: if labels.shift_tab_key_active {
                            "⇧⇥"
                        } else {
                            ""
                        }
                        .into(),
                        slot_width_px: None,
                        key_first: false,
                        justify: crate::components::footer_chrome::FooterHintContentJustify::Start,
                        label_font_size_px: Some(info.font_size),
                        keycap_font_size_px: Some(header_keycap_font_size),
                        keycap_height_px: Some(header_keycap_height),
                        hover_text_alpha: Some(info.pill_hover_text_alpha),
                        hover_glyph_alpha: Some(info.pill_hover_key_alpha),
                        hover_keycap_border_alpha: Some(info.pill_hover_border_alpha),
                    },
                    theme,
                ),
            )
            .into_any_element()
    } else {
        div()
            .min_w(px(0.0))
            .overflow_hidden()
            .text_ellipsis()
            .child(agent_model_label.clone())
            .into_any_element()
    };

    let mut cwd_chip = div()
        .id(MAIN_VIEW_CONTEXT_CWD_BUTTON_ID)
        .debug_selector(|| MAIN_VIEW_CONTEXT_CWD_BUTTON_ID.to_string())
        .min_w(px(0.0))
        .flex_shrink()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(info.gap_px))
        .px(px(info.pill_padding_x))
        .py(px(info.pill_padding_y))
        .rounded(px(info.pill_radius))
        .font_family(info.font_family)
        .text_size(px(info.font_size))
        .text_color(text_color)
        .cursor_pointer()
        .on_click(on_cwd_click)
        .child(cwd_key);
    if show_pills {
        cwd_chip = cwd_chip.border_1().border_color(border).bg(rest_bg);
    }

    let mut model_chip = div()
        .id(MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID)
        .debug_selector(|| MAIN_VIEW_CONTEXT_MODEL_BUTTON_ID.to_string())
        .min_w(px(0.0))
        .flex_shrink()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(info.gap_px))
        .px(px(info.pill_padding_x))
        .py(px(info.pill_padding_y))
        .rounded(px(info.pill_radius))
        .font_family(info.font_family)
        .text_size(px(info.font_size))
        .text_color(text_color)
        .cursor_pointer()
        .on_click(on_agent_model_click)
        .child(model_key);
    if show_pills {
        model_chip = model_chip.border_1().border_color(border).bg(rest_bg);
    }

    let selection_chip = selection_hint.map(|hint| {
        let key_slot = if info.show_keys {
            div()
                .flex_1()
                .min_w(px(0.0))
                .overflow_hidden()
                .opacity(info.key_opacity.clamp(0.0, 1.0))
                .child(
                    crate::components::footer_chrome::render_footer_hint_button_like_shrinkable(
                        crate::components::footer_chrome::FooterHintButtonSpec {
                            label: hint.label.clone().into(),
                            key: ".".into(),
                            slot_width_px: None,
                            key_first: false,
                            justify:
                                crate::components::footer_chrome::FooterHintContentJustify::Start,
                            label_font_size_px: Some(info.font_size),
                            keycap_font_size_px: Some(header_keycap_font_size),
                            keycap_height_px: Some(header_keycap_height),
                            hover_text_alpha: Some(info.pill_hover_text_alpha),
                            hover_glyph_alpha: Some(info.pill_hover_key_alpha),
                            hover_keycap_border_alpha: Some(info.pill_hover_border_alpha),
                        },
                        theme,
                    ),
                )
                .into_any_element()
        } else {
            div()
                .min_w(px(0.0))
                .overflow_hidden()
                .text_ellipsis()
                .child(hint.label.clone())
                .into_any_element()
        };

        let mut chip = div()
            .id(MAIN_VIEW_CONTEXT_SELECTION_BUTTON_ID)
            .min_w(px(0.0))
            .flex_shrink()
            .overflow_hidden()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(info.gap_px))
            .px(px(info.pill_padding_x))
            .py(px(info.pill_padding_y))
            .rounded(px(info.pill_radius))
            .font_family(info.font_family)
            .text_size(px(info.font_size))
            .text_color(text_color)
            .cursor_pointer()
            .on_click(on_selection_click)
            .child(key_slot);
        if show_pills {
            chip = chip.border_1().border_color(border).bg(rest_bg);
        }
        chip
    });

    // Header chips deliberately stay OFF the glass-capsule system (user
    // call 2026-07-21: header capsules read as distracting); capsules are a
    // footer/action affordance only.
    let mut left_lane = div()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .justify_start()
        .gap(px(info.gap_px));
    if info.show_cwd {
        left_lane = left_lane.child(cwd_chip);
    }
    if let Some(chip) = selection_chip {
        left_lane = left_lane.child(chip);
    }
    if info.show_cwd
        && info.show_agent_model
        && !matches!(info.layout, crate::designs::HeaderInfoBarLayout::Split)
    {
        left_lane = left_lane.child(
            div()
                .font_family(info.font_family)
                .text_size(px(info.font_size))
                .text_color(text_color)
                .child(info.separator),
        );
    }

    let mut right_lane = div()
        .flex_1()
        .min_w(px(0.0))
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .justify_end()
        .gap(px(info.gap_px));
    if info.show_agent_model {
        right_lane = right_lane.child(model_chip);
    }

    div()
        .id(MAIN_VIEW_CONTEXT_ZONE_ID)
        .debug_selector(|| MAIN_VIEW_CONTEXT_ZONE_ID.to_string())
        .w_full()
        .h(px(info.height_px))
        .flex()
        .flex_row()
        .items_center()
        .gap(px(info.gap_px))
        .child(left_lane)
        .child(right_lane)
        .into_any_element()
}

#[allow(dead_code)]
pub(crate) fn render_main_view_context_zone_inert(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    cwd_label: Option<String>,
    agent_model_label: Option<String>,
) -> AnyElement {
    render_main_view_context_zone(
        theme,
        def,
        cwd_label,
        agent_model_label,
        |_event, _window, _cx| {},
        |_event, _window, _cx| {},
    )
}

#[allow(dead_code)]
pub(crate) fn render_main_view_header_divider(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    margin_x: f32,
    height: f32,
) -> AnyElement {
    div()
        .id(MAIN_VIEW_HEADER_DIVIDER_ID)
        .mx(px(margin_x))
        .h(px(height))
        .bg(rgba(
            (theme.colors.text.primary << 8) | def.shell.divider_alpha,
        ))
        .into_any_element()
}

#[allow(dead_code)]
pub(crate) fn render_main_view_main_slot(def: MainMenuThemeDef, main: AnyElement) -> AnyElement {
    render_main_view_main_slot_with_bottom_inset(def, main, true)
}

fn render_main_view_main_slot_with_bottom_inset(
    def: MainMenuThemeDef,
    main: AnyElement,
    include_bottom_inset: bool,
) -> AnyElement {
    div()
        .id(MAIN_VIEW_MAIN_ID)
        .debug_selector(|| MAIN_VIEW_MAIN_ID.to_string())
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .pb(px(resolved_main_view_main_bottom_inset(
            def,
            include_bottom_inset,
        )))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(main)
        .into_any_element()
}

fn render_main_view_overlay_main_slot(
    def: MainMenuThemeDef,
    main: AnyElement,
    header_height: f32,
) -> AnyElement {
    div()
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .relative()
        .overflow_hidden()
        .child(
            div()
                .id(MAIN_VIEW_OVERLAY_CLIP_ID)
                .debug_selector(|| MAIN_VIEW_OVERLAY_CLIP_ID.to_string())
                .absolute()
                .top(px(header_height))
                .left_0()
                .right_0()
                .bottom_0()
                .overflow_hidden()
                .child(
                    div()
                        .absolute()
                        .top(px(-header_height))
                        .bottom_0()
                        .left_0()
                        .right_0()
                        .flex()
                        .flex_col()
                        .child(render_main_view_main_slot_with_bottom_inset(
                            def, main, false,
                        )),
                ),
        )
        .into_any_element()
}

fn resolved_main_view_main_bottom_inset(def: MainMenuThemeDef, include_bottom_inset: bool) -> f32 {
    if include_bottom_inset {
        def.shell.content_inset_bottom
    } else {
        0.0
    }
}

/// Scrollable, token-spaced content for main-view flows that stack distinct
/// blocks instead of rendering one edge-to-edge list.
#[allow(dead_code)] // Used by the binary target through include!-merged built-in render code.
pub(crate) fn render_main_view_scroll_flow(
    spacing: MainViewFlowSpacing,
    sections: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    div()
        .id(MAIN_VIEW_SCROLL_FLOW_ID)
        .flex_1()
        .min_h(px(0.))
        .w_full()
        .overflow_y_scroll()
        .px(px(spacing.inset_x))
        .py(px(spacing.inset_y))
        .flex()
        .flex_col()
        .gap(px(spacing.section_gap))
        .children(sections)
        .into_any_element()
}

/// Scrollable flow whose panel/list edge and text plane are both declared by
/// one frame contract instead of being re-inset by each child.
#[allow(dead_code)] // Binary-only Permissions renderer consumes this shared frame.
pub(crate) fn render_main_view_content_frame(
    frame: MainViewContentFrame,
    sections: impl IntoIterator<Item = AnyElement>,
) -> AnyElement {
    div()
        .id(MAIN_VIEW_SCROLL_FLOW_ID)
        .flex_1()
        .min_h(px(0.0))
        .w_full()
        .overflow_y_scroll()
        .px(px(frame.container_edge_x))
        .py(px(frame.inset_y))
        .flex()
        .flex_col()
        .gap(px(frame.section_gap))
        .children(sections)
        .into_any_element()
}

#[allow(dead_code)] // Binary-only Permissions renderer consumes this shared frame.
pub(crate) fn render_main_view_text_plane(
    frame: MainViewContentFrame,
    content: AnyElement,
) -> AnyElement {
    div()
        .w_full()
        .pl(px(frame.text_inset_x()))
        .child(content)
        .into_any_element()
}

pub(crate) fn main_view_input_text_inset_left(def: MainMenuThemeDef) -> f32 {
    def.search.text_inset_x
}

pub(crate) fn main_view_row_leading_x(def: MainMenuThemeDef) -> f32 {
    def.row.outer_padding_x + def.row.inner_padding_x
}

pub(crate) fn main_view_text_column_x(def: MainMenuThemeDef) -> f32 {
    main_view_row_leading_x(def) + def.icon.container_size + def.row.icon_text_gap
}

pub(crate) fn main_view_content_columns(def: MainMenuThemeDef) -> MainViewColumnMetrics {
    let text_column_x = main_view_text_column_x(def);
    MainViewColumnMetrics {
        shell_left_x: def.shell.header_padding_x,
        row_leading_x: main_view_row_leading_x(def),
        text_column_x,
        input_text_inset_left: (text_column_x - def.shell.header_padding_x)
            .max(def.search.text_inset_x),
        content_right_inset_x: def.shell.header_padding_x,
        top_inset_y: def.list.first_section_header_height,
    }
}

#[allow(dead_code)] // Binary-only built-in renderers consume the default-height wrapper.
pub(crate) fn render_main_view_input_shell(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewInputChrome,
) -> AnyElement {
    render_main_view_input_shell_with_height(theme, def, chrome, None)
}

fn resolved_main_view_input_height(default_height: f32, requested_height: Option<f32>) -> f32 {
    requested_height
        .unwrap_or(default_height)
        .max(default_height)
}

/// Preserve the main menu's exact one-line input geometry while allowing a
/// multiline surface to grow by one text line at a time.
pub(crate) fn main_view_multiline_input_height(
    default_height: f32,
    line_height: f32,
    visible_lines: usize,
) -> f32 {
    default_height + line_height * visible_lines.saturating_sub(1) as f32
}

/// Render the shared main-view input chrome with an optional surface-owned
/// height. Most search inputs use the theme height; multi-line composers can
/// request a taller shell without rebuilding the shared border and insets.
pub(crate) fn render_main_view_input_shell_with_height(
    theme: &crate::theme::Theme,
    def: MainMenuThemeDef,
    chrome: MainViewInputChrome,
    height: Option<f32>,
) -> AnyElement {
    let search = def.search;
    let text_inset_left = main_view_input_text_inset_left(def);
    let height = resolved_main_view_input_height(search.height, height);

    let mut input = div()
        .id(MAIN_VIEW_INPUT_SHELL_ID)
        .debug_selector(|| MAIN_VIEW_INPUT_SHELL_ID.to_string())
        .w_full()
        .flex_1()
        .h(px(height))
        .rounded(px(search.radius))
        .border_1()
        .border_color(rgba((theme.colors.ui.border << 8) | search.border_alpha))
        .bg(rgba(
            (theme.colors.background.search_box << 8) | search.surface_alpha,
        ))
        .relative()
        .flex()
        .items_center();

    input = input.child(
        div()
            .debug_selector(|| MAIN_VIEW_INPUT_BODY_ID.to_string())
            .flex_1()
            .pl(px(text_inset_left))
            .pr(px(search.text_inset_x * 0.5))
            .flex()
            .flex_row()
            .items_center()
            .child(chrome.body),
    );

    for trailing in chrome.trailing {
        input = input.child(trailing);
    }

    input.into_any_element()
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext as _, InteractiveElement as _, IntoElement as _, Styled as _};

    use super::{
        main_view_content_frame, main_view_flow_spacing, main_view_header_metrics,
        main_view_multiline_input_height, resolved_main_view_input_height,
        resolved_main_view_main_bottom_inset, selection_hint_snippet,
    };

    #[test]
    fn prompt_search_modes_resolve_main_menu_search_geometry() {
        let search = crate::designs::MainMenuThemeVariant::default().def().search;
        let expected = super::PromptSearchInputGeometry::from_main_menu(search);
        let cases = [
            ("ArgPrompt", super::PromptSearchInputKind::EntityBacked),
            ("MiniPrompt", super::PromptSearchInputKind::EntityBacked),
            (
                "SelectPrompt",
                super::PromptSearchInputKind::ControllerOwned,
            ),
            ("PathPrompt", super::PromptSearchInputKind::PathPrefix),
        ];

        for (surface, kind) in cases {
            assert_eq!(
                super::prompt_search_input_geometry(search, kind),
                expected,
                "{surface} must resolve height/font/radius/text inset from MainMenuSearchTokens"
            );
        }
    }

    #[test]
    fn canonical_and_context_only_headers_share_one_theme_derived_geometry_model() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let canonical = main_view_header_metrics(def, Some(def.search.height));
        let context_only = main_view_header_metrics(def, None);

        assert_eq!(canonical.header_height, 58.0);
        assert_eq!(canonical.context_x, -6.0);
        assert_eq!(canonical.context_y, 4.0);
        assert_eq!(canonical.context_height, 22.0);
        assert_eq!(canonical.input_x, 2.0);
        assert_eq!(canonical.input_y, 28.0);
        assert_eq!(canonical.input_height, Some(26.0));
        assert_eq!(context_only.header_height, 30.0);
        assert_eq!(context_only.input_height, None);
    }

    #[test]
    fn footer_flush_list_chrome_has_exactly_one_footer_reservation() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        assert!(def.shell.content_inset_bottom > 0.0);
        assert_eq!(resolved_main_view_main_bottom_inset(def, false), 0.0);
        assert_eq!(
            resolved_main_view_main_bottom_inset(def, true),
            def.shell.content_inset_bottom
        );
    }

    #[test]
    fn multiline_input_keeps_the_main_menu_height_until_a_second_line_is_visible() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let line_height = def.search.height;

        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 0),
            def.search.height
        );
        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 1),
            def.search.height
        );
        assert_eq!(
            main_view_multiline_input_height(def.search.height, line_height, 3),
            def.search.height + line_height * 2.0
        );
    }

    #[test]
    fn input_shell_accepts_taller_surface_height_without_shrinking_theme_default() {
        assert_eq!(resolved_main_view_input_height(26.0, None), 26.0);
        assert_eq!(resolved_main_view_input_height(26.0, Some(152.0)), 152.0);
        assert_eq!(resolved_main_view_input_height(26.0, Some(18.0)), 26.0);
    }

    #[test]
    fn content_frame_declares_one_container_edge_and_row_aligned_text_plane() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let spacing = crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
        let frame = main_view_content_frame(def, spacing);
        assert_eq!(frame.container_edge_x, def.shell.content_inset_x);
        assert_eq!(
            frame.text_plane_x,
            frame.container_edge_x + super::main_view_text_column_x(def)
        );
        assert_eq!(frame.text_inset_x(), super::main_view_text_column_x(def));
    }

    #[test]
    fn snippet_collapses_whitespace_and_truncates_at_char_boundary() {
        assert_eq!(
            selection_hint_snippet("hello   world\n\tnext", 24),
            "hello world next"
        );
        assert_eq!(
            selection_hint_snippet("the quick brown fox jumps over the lazy dog", 15),
            "the quick brown\u{2026}"
        );
        // Multi-byte chars must not split; count is in chars, not bytes.
        assert_eq!(
            selection_hint_snippet("héllö wörld ünïcödé", 7),
            "héllö w\u{2026}"
        );
    }

    #[test]
    fn snippet_short_text_passes_through_unchanged() {
        assert_eq!(selection_hint_snippet("short", 24), "short");
        assert_eq!(selection_hint_snippet("  padded  ", 24), "padded");
    }

    #[test]
    fn flow_spacing_uses_balanced_shell_inset_and_safe_vertical_tokens() {
        let def = crate::designs::MainMenuThemeVariant::default().def();
        let design_spacing = crate::designs::DesignSpacing::default();
        let flow = main_view_flow_spacing(def, design_spacing);

        assert_eq!(flow.inset_x, def.shell.content_inset_x);
        assert_eq!(flow.inset_y, design_spacing.padding_sm);
        assert_eq!(flow.section_gap, design_spacing.gap_lg);
        assert!(flow.inset_x > 0.0);
        assert!(flow.inset_y > 0.0);
        assert!(flow.section_gap > flow.inset_y);
    }

    struct TestMainViewOverlay;

    impl gpui::Render for TestMainViewOverlay {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let def = crate::designs::MainMenuThemeVariant::default().def();
            let theme = crate::theme::Theme::default();

            let chrome = super::MainViewChrome {
                header: super::MainViewHeaderChrome::canonical(
                    def,
                    gpui::div().into_any_element(),
                    gpui::div().into_any_element(),
                ),
                divider: super::MainViewDividerChrome {
                    margin_x: 0.0,
                    height: 1.0,
                    visible: true,
                },
                main: gpui::div()
                    .id("test-main-content")
                    .debug_selector(|| "test-main-content".to_string())
                    .size_full()
                    .into_any_element(),
                footer: None,
                overlays: vec![],
            };

            let root = super::render_main_view_shell();
            super::render_main_view_chrome_header_overlay_footer_flush(root, &theme, def, chrome)
        }
    }

    #[gpui::test]
    fn test_main_view_overlay_main_slot_clip_bounds(cx: &mut gpui::TestAppContext) {
        use gpui::px;

        let def = crate::designs::MainMenuThemeVariant::default().def();
        let header_height = main_view_header_metrics(def, Some(def.search.height)).header_height;

        // Test at window height 600
        let window_600 = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(800.0), px(600.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestMainViewOverlay))
                .unwrap()
        });

        cx.run_until_parked();

        window_600
            .update(cx, |_, window, _| {
                let bounds_entries = window.debug_bounds_entries();

                let clip_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_OVERLAY_CLIP_ID)
                    .expect("clip div should be rendered");

                let main_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_MAIN_ID)
                    .expect("main view main slot should be rendered");

                assert_eq!(main_entry.bounds.size.height, px(600.0));
                assert_eq!(clip_entry.bounds.origin.y, px(header_height));
                assert_eq!(clip_entry.bounds.size.height, px(600.0 - header_height));
                assert_eq!(main_entry.clip_bounds.origin.y, px(header_height));
                assert_eq!(
                    main_entry.clip_bounds.size.height,
                    px(600.0 - header_height)
                );
            })
            .unwrap();

        // Test at window height 800 (validates responsive layout after resize/size change)
        let window_800 = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(800.0), px(800.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestMainViewOverlay))
                .unwrap()
        });

        cx.run_until_parked();

        window_800
            .update(cx, |_, window, _| {
                let bounds_entries = window.debug_bounds_entries();

                let clip_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_OVERLAY_CLIP_ID)
                    .expect("clip div should be rendered");

                let main_entry = bounds_entries
                    .iter()
                    .find(|entry| entry.selector == super::MAIN_VIEW_MAIN_ID)
                    .expect("main view main slot should be rendered");

                assert_eq!(main_entry.bounds.size.height, px(800.0));
                assert_eq!(clip_entry.bounds.origin.y, px(header_height));
                assert_eq!(clip_entry.bounds.size.height, px(800.0 - header_height));
                assert_eq!(main_entry.clip_bounds.origin.y, px(header_height));
                assert_eq!(
                    main_entry.clip_bounds.size.height,
                    px(800.0 - header_height)
                );
            })
            .unwrap();
    }
}
