//! Storybook-style component stories for the dev style tool preview pane.
//!
//! Each [`ComponentStory`] renders one shared component in a representative
//! state so style-knob and theme changes can be eyeballed without driving the
//! main window.
//!
//! IMPORTANT: every story render fn MUST resolve theme/tokens INSIDE the fn at
//! call time (e.g. `crate::theme::get_cached_theme()`,
//! `ListItemColors::from_theme(&theme)`, `crate::designs::current_main_menu_theme()`,
//! `runtime_overrides::effective_confirm_modal_style()`). Never cache resolved
//! colors or style defs in statics/consts — the preview pane re-renders on
//! every knob/theme change and stale captures would freeze the preview.

use gpui::{div, prelude::*, px, rgb, rgba, AnyElement, App, Window};

use crate::components::footer_chrome::{
    render_footer_hint_action_button_frame, FooterHintActionButtonFrameSpec,
    FooterHintButtonLayoutOverrides, FooterHintContentJustify,
};
use crate::dev_style_tool::runtime_overrides;
use crate::list_item::{ListItem, ListItemColors};

/// Which dev style tool tab a story belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoryGroup {
    MainWindow,
    TextCopy,
    ActionsPopup,
    AgentChat,
    ConfirmModal,
    Common,
}

/// A single inline component preview.
pub struct ComponentStory {
    pub id: &'static str,
    pub title: &'static str,
    pub group: StoryGroup,
    pub render: fn(&mut Window, &mut App) -> AnyElement,
}

pub const COMPONENT_STORIES: &[ComponentStory] = &[
    // ── MainWindow ──────────────────────────────────────────────────────
    ComponentStory {
        id: "story:dev-style-tool:list-item-default",
        title: "ListItem — default",
        group: StoryGroup::MainWindow,
        render: render_list_item_default,
    },
    ComponentStory {
        id: "story:dev-style-tool:list-item-selected",
        title: "ListItem — selected",
        group: StoryGroup::MainWindow,
        render: render_list_item_selected,
    },
    ComponentStory {
        id: "story:dev-style-tool:list-item-long-text",
        title: "ListItem — long text truncation",
        group: StoryGroup::MainWindow,
        render: render_list_item_long_text,
    },
    ComponentStory {
        id: "story:dev-style-tool:section-header",
        title: "Section header",
        group: StoryGroup::MainWindow,
        render: render_section_header_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:empty-state",
        title: "EmptyState",
        group: StoryGroup::MainWindow,
        render: render_empty_state_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:prompt-footer",
        title: "PromptFooter",
        group: StoryGroup::MainWindow,
        render: render_prompt_footer_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:hint-strip",
        title: "Hint strip (universal prompt hints)",
        group: StoryGroup::MainWindow,
        render: render_hint_strip_story,
    },
    // ── TextCopy ────────────────────────────────────────────────────────
    ComponentStory {
        id: "story:dev-style-tool:main-input-placeholder",
        title: "Main input placeholder (effective copy)",
        group: StoryGroup::TextCopy,
        render: render_main_input_placeholder_story,
    },
    // ── ConfirmModal ────────────────────────────────────────────────────
    ComponentStory {
        id: "story:dev-style-tool:confirm-modal",
        title: "Confirm modal shell",
        group: StoryGroup::ConfirmModal,
        render: render_confirm_modal_story,
    },
    // ── Common ──────────────────────────────────────────────────────────
    ComponentStory {
        id: "story:dev-style-tool:button-variants",
        title: "Button — Primary / Ghost / Icon",
        group: StoryGroup::Common,
        render: render_button_variants_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:toast-variants",
        title: "Toast — Info / Success / Warning / Error",
        group: StoryGroup::Common,
        render: render_toast_variants_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:unified-list-item",
        title: "UnifiedListItem + SectionHeader",
        group: StoryGroup::Common,
        render: render_unified_list_item_story,
    },
    ComponentStory {
        id: "story:dev-style-tool:section-divider",
        title: "SectionDivider",
        group: StoryGroup::Common,
        render: render_section_divider_story,
    },
    // ActionsPopup and AgentChat intentionally have no portable stories yet:
    // their renderers require live main-app entities (actions dialog state,
    // ACP threads). The preview pane shows a kitchen-sink hint card instead.
];

/// All stories belonging to `group`, in declaration order.
pub fn stories_for_group(group: StoryGroup) -> impl Iterator<Item = &'static ComponentStory> {
    COMPONENT_STORIES
        .iter()
        .filter(move |story| story.group == group)
}

// ─────────────────────────────────────────────────────────────────────────
// MainWindow stories
// ─────────────────────────────────────────────────────────────────────────

fn render_list_item_default(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = ListItemColors::from_theme(&theme);
    ListItem::new("Open Project", colors)
        .index(0)
        .main_menu_theme(crate::designs::current_main_menu_theme())
        .description("Open a recent project in the editor")
        .icon_kind(crate::list_item::IconKind::Svg("folder".to_string()))
        .shortcut("cmd+o")
        .into_any_element()
}

fn render_list_item_selected(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = ListItemColors::from_theme(&theme);
    ListItem::new("Run Script", colors)
        .index(0)
        .selected(true)
        .main_menu_theme(crate::designs::current_main_menu_theme())
        .description("Execute the highlighted script")
        .icon_kind(crate::list_item::IconKind::Svg("terminal".to_string()))
        .shortcut("enter")
        .into_any_element()
}

fn render_list_item_long_text(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = ListItemColors::from_theme(&theme);
    ListItem::new(
        "An Exceptionally Long Script Name That Should Truncate Instead Of Wrapping Or Overflowing",
        colors,
    )
    .index(0)
    .main_menu_theme(crate::designs::current_main_menu_theme())
    .description(
        "A very long description demonstrating how secondary text truncates when the row \
         runs out of horizontal space in a narrow preview pane",
    )
    .icon_kind(crate::list_item::IconKind::Svg("file-text".to_string()))
    .into_any_element()
}

fn render_section_header_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = ListItemColors::from_theme(&theme);
    crate::list_item::render_section_header("Suggested · 5", Some("star"), colors, true)
        .into_any_element()
}

fn render_empty_state_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    // EmptyState fills its parent height, so give the preview a bounded canvas.
    div()
        .w_full()
        .h(px(120.0))
        .child(
            crate::list_item::EmptyState::new(
                "No results",
                theme.colors.text.primary,
                crate::list_item::FONT_SYSTEM_UI,
            )
            .icon(crate::designs::icon_variations::IconName::MagnifyingGlass)
            .hint("Try a different search term")
            .render(),
        )
        .into_any_element()
}

fn render_prompt_footer_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    // Mirrors the real construction used by the footer gallery built-in.
    let config = crate::components::prompt_footer::PromptFooterConfig::new()
        .primary_label("Run Script")
        .primary_shortcut("↵")
        .secondary_label("Actions")
        .secondary_shortcut("⌘K")
        .show_logo(true)
        .show_primary(true)
        .show_secondary(true)
        .show_info_label(true)
        .info_label("Preview");
    let colors = crate::components::prompt_footer::PromptFooterColors::from_theme(&theme);
    crate::components::prompt_footer::PromptFooter::new(config, colors).into_any_element()
}

fn render_hint_strip_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    crate::components::render_simple_hint_strip(crate::components::universal_prompt_hints(), None)
}

// ─────────────────────────────────────────────────────────────────────────
// TextCopy stories
// ─────────────────────────────────────────────────────────────────────────

fn render_main_input_placeholder_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
    let placeholder = runtime_overrides::effective_copy_value(
        crate::dev_style_tool::copy_catalog::MAIN_INPUT_PLACEHOLDER_COPY_ID,
    );
    div()
        .w_full()
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(crate::ui::chrome::LIQUID_GLASS_COMPACT_RADIUS_PX))
        .border(px(1.0))
        .border_color(rgba(chrome.border_rgba))
        .bg(rgba(chrome.input_surface_rgba))
        .text_lg()
        .text_color(rgb(chrome.text_dimmed_hex))
        .child(placeholder)
        .into_any_element()
}

// ─────────────────────────────────────────────────────────────────────────
// ConfirmModal stories
// ─────────────────────────────────────────────────────────────────────────

fn render_confirm_modal_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
    // Resolved at call time so confirm-modal knob edits re-style this preview.
    let style = runtime_overrides::effective_confirm_modal_style();

    let header = crate::components::confirm_modal_shell::confirm_modal_header(
        "Delete script?",
        gpui::rgb(chrome.accent_hex),
        gpui::rgb(chrome.text_primary_hex),
    );

    let body = div()
        .w_full()
        .min_h(px(0.0))
        .overflow_hidden()
        .text_xs()
        .line_height(px(style.anatomy.body_line_height))
        .text_color(rgb(chrome.text_secondary_hex))
        .child("This permanently removes the script from your kit. This action cannot be undone.");

    let action_layout = FooterHintButtonLayoutOverrides {
        button_padding_x_px: Some(style.actions.padding_x),
        button_padding_y_px: Some(style.actions.padding_y),
        content_gap_px: Some(style.actions.content_gap),
        button_radius_px: Some(style.actions.button_radius),
        edge_padding_x_px: Some(style.actions.edge_padding_x),
        shrink_frame_to_content_px: false,
        hug_frame_to_content: false,
    };
    let action_row = div()
        .w_full()
        .flex()
        .flex_row()
        .justify_end()
        .gap(px(style.actions.gap))
        .child(render_footer_hint_action_button_frame(
            FooterHintActionButtonFrameSpec {
                id: "story-confirm-ok-button",
                label: "Delete".into(),
                key: "↵".into(),
                slot_width_px: style.actions.confirm_slot_width,
                height_px: style.actions.button_height,
                selected: true,
                key_first: false,
                justify: FooterHintContentJustify::Center,
                layout: action_layout,
            },
            &theme,
        ))
        .child(render_footer_hint_action_button_frame(
            FooterHintActionButtonFrameSpec {
                id: "story-confirm-cancel-button",
                label: "Cancel".into(),
                key: "Esc".into(),
                slot_width_px: style.actions.cancel_slot_width,
                height_px: style.actions.button_height,
                selected: false,
                key_first: false,
                justify: FooterHintContentJustify::Center,
                layout: action_layout,
            },
            &theme,
        ));

    let stack = div()
        .w_full()
        .min_h_0()
        .flex()
        .flex_col()
        .child(header)
        .child(div().h(px(style.anatomy.header_body_gap)))
        .child(body)
        .child(div().h(px(style.anatomy.body_actions_gap)))
        .child(action_row);

    crate::components::confirm_modal_shell::confirm_modal_shell(
        crate::components::confirm_modal_shell::ConfirmModalShellConfig {
            content_id: "story-confirm-modal-content",
            width: None,
            padding_x: style.shell.padding_x,
            padding_y: style.shell.padding_y,
            gap: style.shell.gap,
            background: Some(gpui::rgba(chrome.popup_surface_rgba)),
            border: gpui::rgba(chrome.border_rgba),
            radius: style.shell.radius,
            offset_y: 0.0,
            opacity: 1.0,
        },
        vec![stack.into_any_element()],
    )
    .into_any_element()
}

// ─────────────────────────────────────────────────────────────────────────
// Common stories
// ─────────────────────────────────────────────────────────────────────────

fn render_button_variants_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = crate::components::ButtonColors::from_theme(&theme);
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(8.0))
        .child(
            crate::components::Button::new("Primary", colors)
                .variant(crate::components::ButtonVariant::Primary),
        )
        .child(
            crate::components::Button::new("Ghost", colors)
                .variant(crate::components::ButtonVariant::Ghost),
        )
        .child(
            crate::components::Button::new("✦", colors)
                .variant(crate::components::ButtonVariant::Icon),
        )
        .into_any_element()
}

fn render_toast_variants_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let mut column = div().flex().flex_col().w_full().gap(px(6.0));
    for (variant, message) in [
        (crate::components::ToastVariant::Info, "Synced 12 scripts"),
        (crate::components::ToastVariant::Success, "Script saved"),
        (
            crate::components::ToastVariant::Warning,
            "Missing API key — using fallback",
        ),
        (crate::components::ToastVariant::Error, "Script crashed"),
    ] {
        let colors = crate::components::ToastColors::from_theme(&theme, variant);
        column = column.child(
            crate::components::Toast::new(message, colors)
                .variant(variant)
                .persistent(),
        );
    }
    column.into_any_element()
}

fn render_unified_list_item_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let colors = crate::components::UnifiedListItemColors::from_theme(&theme);
    div()
        .flex()
        .flex_col()
        .w_full()
        .child(
            crate::components::SectionHeader::new("Results")
                .count(2)
                .colors(colors),
        )
        .child(
            crate::components::UnifiedListItem::new(
                gpui::ElementId::Name("story:unified-list-item:selected".into()),
                crate::components::TextContent::plain("Selected row"),
            )
            .subtitle(crate::components::TextContent::plain(
                "With subtitle and shortcut",
            ))
            .leading(crate::components::LeadingContent::Emoji("📋".into()))
            .trailing(crate::components::TrailingContent::Shortcut("⌘O".into()))
            .state(crate::components::ItemState {
                is_selected: true,
                is_hovered: false,
                is_disabled: false,
            })
            .density(crate::components::Density::Comfortable)
            .colors(colors),
        )
        .child(
            crate::components::UnifiedListItem::new(
                gpui::ElementId::Name("story:unified-list-item:unselected".into()),
                crate::components::TextContent::plain("Unselected row"),
            )
            .leading(crate::components::LeadingContent::Emoji("📦".into()))
            .trailing(crate::components::TrailingContent::Chevron)
            .state(crate::components::ItemState::default())
            .density(crate::components::Density::Comfortable)
            .colors(colors),
        )
        .into_any_element()
}

fn render_section_divider_story(_window: &mut Window, _cx: &mut App) -> AnyElement {
    div()
        .w_full()
        .py(px(6.0))
        .child(crate::components::SectionDivider::new().id("story:section-divider"))
        .into_any_element()
}
