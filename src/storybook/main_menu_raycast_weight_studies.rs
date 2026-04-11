use gpui::prelude::FluentBuilder;
use gpui::*;

use crate::list_item::FONT_MONO;
use crate::storybook::{story_container, story_section, StoryVariant};
use crate::theme::get_cached_theme;
use crate::ui_foundation::HexColorExt;

#[derive(Clone, Copy, Debug, PartialEq)]
struct MainMenuWeightSpec {
    id: &'static str,
    name: &'static str,
    family: &'static str,
    description: &'static str,
    search_weight: FontWeight,
    section_weight: FontWeight,
    title_weight: FontWeight,
    selected_title_weight: FontWeight,
    source_weight: FontWeight,
    chip_weight: FontWeight,
    kind_weight: FontWeight,
    search_size: f32,
    ask_size: f32,
    section_size: f32,
    row_height: f32,
    icon_size: f32,
    title_size: f32,
    meta_size: f32,
    kind_size: f32,
    chip_size: f32,
    footer_size: f32,
    title_line_height: f32,
    metadata_line_height: f32,
    header_height: f32,
    footer_height: f32,
    row_gap: f32,
    content_gap: f32,
    row_radius: f32,
    search_opacity: f32,
    metadata_opacity: f32,
    kind_opacity: f32,
    selected_fill_opacity: f32,
}

#[derive(Clone, Copy)]
struct MenuPreviewRow {
    title: &'static str,
    source: &'static str,
    chip: Option<&'static str>,
    kind: &'static str,
    icon_label: &'static str,
    icon_bg: u32,
    selected: bool,
}

const ROWS: &[MenuPreviewRow] = &[
    MenuPreviewRow {
        title: "Clipboard History",
        source: "Raycast",
        chip: Some("ch"),
        kind: "Command",
        icon_label: "▣",
        icon_bg: 0xE15C4D,
        selected: true,
    },
    MenuPreviewRow {
        title: "Activity Monitor",
        source: "",
        chip: None,
        kind: "Application",
        icon_label: "⌗",
        icon_bg: 0x22342E,
        selected: false,
    },
    MenuPreviewRow {
        title: "Empty Trash",
        source: "System",
        chip: None,
        kind: "Command",
        icon_label: "⌫",
        icon_bg: 0x52565E,
        selected: false,
    },
    MenuPreviewRow {
        title: "AI Chat",
        source: "Raycast AI",
        chip: None,
        kind: "Command",
        icon_label: "✦",
        icon_bg: 0x2E1D32,
        selected: false,
    },
    MenuPreviewRow {
        title: "Raycast Notes",
        source: "Raycast Notes",
        chip: None,
        kind: "Command",
        icon_label: "T",
        icon_bg: 0xE26461,
        selected: false,
    },
    MenuPreviewRow {
        title: "My Passwords",
        source: "1Password",
        chip: None,
        kind: "Command",
        icon_label: "◔",
        icon_bg: 0xF2F2F2,
        selected: false,
    },
    MenuPreviewRow {
        title: "Generate Password",
        source: "1Password",
        chip: None,
        kind: "Command",
        icon_label: "◔",
        icon_bg: 0xF2F2F2,
        selected: false,
    },
    MenuPreviewRow {
        title: "My Vaults",
        source: "1Password",
        chip: None,
        kind: "Command",
        icon_label: "◔",
        icon_bg: 0xF2F2F2,
        selected: false,
    },
];

const SPECS: [MainMenuWeightSpec; 15] = [
    MainMenuWeightSpec {
        id: "raycast-baseline",
        name: "Raycast Baseline",
        family: "Balanced",
        description: "Close to the reference: medium search, regular titles, medium accessories, and a calm selected fill.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::MEDIUM,
        search_size: 23.0,
        ask_size: 16.0,
        section_size: 14.0,
        row_height: 62.0,
        icon_size: 28.0,
        title_size: 21.0,
        meta_size: 18.0,
        kind_size: 18.0,
        chip_size: 14.0,
        footer_size: 16.0,
        title_line_height: 24.0,
        metadata_line_height: 20.0,
        header_height: 84.0,
        footer_height: 62.0,
        row_gap: 6.0,
        content_gap: 14.0,
        row_radius: 14.0,
        search_opacity: 0.56,
        metadata_opacity: 0.62,
        kind_opacity: 0.70,
        selected_fill_opacity: 0.12,
    },
    MainMenuWeightSpec {
        id: "lighter-search",
        name: "Lighter Search",
        family: "Balanced",
        description: "Lighter search prompt, slightly smaller metadata, and a tighter row cadence.",
        search_weight: FontWeight::NORMAL,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::MEDIUM,
        search_size: 21.0,
        ask_size: 15.0,
        section_size: 13.0,
        row_height: 58.0,
        icon_size: 26.0,
        title_size: 20.0,
        meta_size: 16.5,
        kind_size: 16.5,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 23.0,
        metadata_line_height: 19.0,
        header_height: 78.0,
        footer_height: 58.0,
        row_gap: 5.0,
        content_gap: 12.0,
        row_radius: 13.0,
        search_opacity: 0.46,
        metadata_opacity: 0.58,
        kind_opacity: 0.64,
        selected_fill_opacity: 0.11,
    },
    MainMenuWeightSpec {
        id: "softer-sections",
        name: "Softer Sections",
        family: "Balanced",
        description: "Quieter section headers, slightly airier rows, and more distance between the title and accessories.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::NORMAL,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::NORMAL,
        kind_weight: FontWeight::MEDIUM,
        search_size: 22.0,
        ask_size: 15.0,
        section_size: 12.5,
        row_height: 64.0,
        icon_size: 28.0,
        title_size: 21.0,
        meta_size: 17.0,
        kind_size: 17.0,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 24.0,
        metadata_line_height: 20.0,
        header_height: 82.0,
        footer_height: 60.0,
        row_gap: 7.0,
        content_gap: 16.0,
        row_radius: 14.0,
        search_opacity: 0.54,
        metadata_opacity: 0.60,
        kind_opacity: 0.64,
        selected_fill_opacity: 0.11,
    },
    MainMenuWeightSpec {
        id: "selected-contrast",
        name: "Selected Contrast",
        family: "Balanced",
        description: "Larger selected title, slightly taller rows, and a clearer delta between focused and unfocused items.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::SEMIBOLD,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::MEDIUM,
        search_size: 23.0,
        ask_size: 16.0,
        section_size: 14.0,
        row_height: 66.0,
        icon_size: 29.0,
        title_size: 22.0,
        meta_size: 17.5,
        kind_size: 17.5,
        chip_size: 14.0,
        footer_size: 16.0,
        title_line_height: 25.0,
        metadata_line_height: 20.0,
        header_height: 84.0,
        footer_height: 62.0,
        row_gap: 6.0,
        content_gap: 14.0,
        row_radius: 15.0,
        search_opacity: 0.56,
        metadata_opacity: 0.62,
        kind_opacity: 0.68,
        selected_fill_opacity: 0.10,
    },
    MainMenuWeightSpec {
        id: "all-normal",
        name: "All Normal",
        family: "Balanced",
        description: "Regular weight almost everywhere, smaller accessory sizes, and denser rows for a flatter look.",
        search_weight: FontWeight::NORMAL,
        section_weight: FontWeight::NORMAL,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::NORMAL,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::NORMAL,
        kind_weight: FontWeight::NORMAL,
        search_size: 21.0,
        ask_size: 14.0,
        section_size: 12.5,
        row_height: 56.0,
        icon_size: 25.0,
        title_size: 19.5,
        meta_size: 15.0,
        kind_size: 15.0,
        chip_size: 12.0,
        footer_size: 14.0,
        title_line_height: 22.0,
        metadata_line_height: 18.0,
        header_height: 76.0,
        footer_height: 54.0,
        row_gap: 4.0,
        content_gap: 11.0,
        row_radius: 12.0,
        search_opacity: 0.48,
        metadata_opacity: 0.54,
        kind_opacity: 0.56,
        selected_fill_opacity: 0.10,
    },
    MainMenuWeightSpec {
        id: "vendor-forward",
        name: "Vendor Forward",
        family: "Metadata",
        description: "Larger vendor labels and roomier metadata spacing so sources become part of the scan path.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::NORMAL,
        search_size: 22.5,
        ask_size: 15.0,
        section_size: 13.5,
        row_height: 64.0,
        icon_size: 28.0,
        title_size: 20.5,
        meta_size: 19.0,
        kind_size: 16.0,
        chip_size: 14.0,
        footer_size: 15.0,
        title_line_height: 23.0,
        metadata_line_height: 20.0,
        header_height: 82.0,
        footer_height: 58.0,
        row_gap: 6.0,
        content_gap: 16.0,
        row_radius: 14.0,
        search_opacity: 0.56,
        metadata_opacity: 0.72,
        kind_opacity: 0.56,
        selected_fill_opacity: 0.12,
    },
    MainMenuWeightSpec {
        id: "kind-quiet",
        name: "Kind Quiet",
        family: "Metadata",
        description: "Smaller right-edge labels, tighter kind column, and more emphasis on the left-aligned title block.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::NORMAL,
        search_size: 22.0,
        ask_size: 15.0,
        section_size: 13.5,
        row_height: 60.0,
        icon_size: 27.0,
        title_size: 21.0,
        meta_size: 17.0,
        kind_size: 14.5,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 24.0,
        metadata_line_height: 19.0,
        header_height: 80.0,
        footer_height: 58.0,
        row_gap: 5.0,
        content_gap: 15.0,
        row_radius: 13.0,
        search_opacity: 0.56,
        metadata_opacity: 0.60,
        kind_opacity: 0.42,
        selected_fill_opacity: 0.12,
    },
    MainMenuWeightSpec {
        id: "chip-forward",
        name: "Chip Forward",
        family: "Metadata",
        description: "Larger chip size and stronger chip weight so the alias becomes a deliberate secondary signal.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::SEMIBOLD,
        kind_weight: FontWeight::MEDIUM,
        search_size: 22.0,
        ask_size: 15.0,
        section_size: 13.5,
        row_height: 61.0,
        icon_size: 27.0,
        title_size: 20.5,
        meta_size: 16.0,
        kind_size: 16.5,
        chip_size: 15.0,
        footer_size: 15.0,
        title_line_height: 23.0,
        metadata_line_height: 18.5,
        header_height: 80.0,
        footer_height: 58.0,
        row_gap: 5.0,
        content_gap: 13.0,
        row_radius: 13.0,
        search_opacity: 0.56,
        metadata_opacity: 0.56,
        kind_opacity: 0.66,
        selected_fill_opacity: 0.12,
    },
    MainMenuWeightSpec {
        id: "whisper-metadata",
        name: "Whisper Metadata",
        family: "Metadata",
        description: "Smaller metadata, slightly bigger titles, and extra title line-height to isolate the primary label.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::NORMAL,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::NORMAL,
        kind_weight: FontWeight::NORMAL,
        search_size: 22.0,
        ask_size: 15.0,
        section_size: 12.0,
        row_height: 60.0,
        icon_size: 26.0,
        title_size: 21.5,
        meta_size: 14.0,
        kind_size: 14.0,
        chip_size: 12.5,
        footer_size: 14.0,
        title_line_height: 25.0,
        metadata_line_height: 17.5,
        header_height: 80.0,
        footer_height: 56.0,
        row_gap: 5.0,
        content_gap: 15.0,
        row_radius: 13.0,
        search_opacity: 0.54,
        metadata_opacity: 0.46,
        kind_opacity: 0.46,
        selected_fill_opacity: 0.10,
    },
    MainMenuWeightSpec {
        id: "directory-kind",
        name: "Directory Kind",
        family: "Metadata",
        description: "Bigger kind labels and taller rows for a more app-directory feeling scan pattern.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::NORMAL,
        kind_weight: FontWeight::SEMIBOLD,
        search_size: 22.5,
        ask_size: 15.5,
        section_size: 13.5,
        row_height: 66.0,
        icon_size: 29.0,
        title_size: 21.0,
        meta_size: 16.0,
        kind_size: 19.0,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 24.0,
        metadata_line_height: 18.0,
        header_height: 82.0,
        footer_height: 60.0,
        row_gap: 6.5,
        content_gap: 14.0,
        row_radius: 14.0,
        search_opacity: 0.56,
        metadata_opacity: 0.56,
        kind_opacity: 0.76,
        selected_fill_opacity: 0.12,
    },
    MainMenuWeightSpec {
        id: "title-forward",
        name: "Title Forward",
        family: "Primary",
        description: "Bigger row titles, stronger selected label, and slightly larger search prompt for a bolder first read.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::MEDIUM,
        selected_title_weight: FontWeight::SEMIBOLD,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::NORMAL,
        search_size: 24.0,
        ask_size: 16.0,
        section_size: 14.0,
        row_height: 65.0,
        icon_size: 28.5,
        title_size: 22.5,
        meta_size: 16.0,
        kind_size: 16.0,
        chip_size: 13.5,
        footer_size: 16.0,
        title_line_height: 26.0,
        metadata_line_height: 18.5,
        header_height: 86.0,
        footer_height: 62.0,
        row_gap: 6.0,
        content_gap: 13.0,
        row_radius: 14.0,
        search_opacity: 0.58,
        metadata_opacity: 0.56,
        kind_opacity: 0.56,
        selected_fill_opacity: 0.11,
    },
    MainMenuWeightSpec {
        id: "selected-medium",
        name: "Selected Medium",
        family: "Primary",
        description: "Heavier fill, taller rows, and slightly smaller accessories so the active row dominates the surface.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::NORMAL,
        search_size: 23.0,
        ask_size: 15.5,
        section_size: 13.5,
        row_height: 68.0,
        icon_size: 29.0,
        title_size: 22.0,
        meta_size: 15.0,
        kind_size: 15.0,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 25.0,
        metadata_line_height: 17.5,
        header_height: 84.0,
        footer_height: 60.0,
        row_gap: 6.0,
        content_gap: 12.0,
        row_radius: 15.0,
        search_opacity: 0.56,
        metadata_opacity: 0.54,
        kind_opacity: 0.54,
        selected_fill_opacity: 0.18,
    },
    MainMenuWeightSpec {
        id: "crisp-raycast",
        name: "Crisp Raycast",
        family: "Primary",
        description: "Sharper size contrast between titles and metadata, with a slightly taller search line and a disciplined selected row.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::MEDIUM,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::MEDIUM,
        search_size: 24.0,
        ask_size: 16.0,
        section_size: 13.5,
        row_height: 62.0,
        icon_size: 28.0,
        title_size: 21.5,
        meta_size: 16.0,
        kind_size: 16.5,
        chip_size: 13.5,
        footer_size: 15.5,
        title_line_height: 24.5,
        metadata_line_height: 18.0,
        header_height: 86.0,
        footer_height: 60.0,
        row_gap: 5.5,
        content_gap: 14.0,
        row_radius: 14.0,
        search_opacity: 0.58,
        metadata_opacity: 0.58,
        kind_opacity: 0.66,
        selected_fill_opacity: 0.14,
    },
    MainMenuWeightSpec {
        id: "semibold-hero",
        name: "Semibold Hero",
        family: "Primary",
        description: "Biggest selected title, slightly larger search text, and compressed metadata to make the first row pop hard.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::SEMIBOLD,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::NORMAL,
        search_size: 24.0,
        ask_size: 16.0,
        section_size: 13.5,
        row_height: 64.0,
        icon_size: 28.0,
        title_size: 23.0,
        meta_size: 15.0,
        kind_size: 15.0,
        chip_size: 13.0,
        footer_size: 15.0,
        title_line_height: 26.0,
        metadata_line_height: 17.0,
        header_height: 86.0,
        footer_height: 60.0,
        row_gap: 5.5,
        content_gap: 12.0,
        row_radius: 14.0,
        search_opacity: 0.56,
        metadata_opacity: 0.52,
        kind_opacity: 0.50,
        selected_fill_opacity: 0.15,
    },
    MainMenuWeightSpec {
        id: "metadata-split",
        name: "Metadata Split",
        family: "Primary",
        description: "Larger chip and kind sizes, smaller vendor text, and slightly denser rows so the accessory hierarchy feels deliberately split.",
        search_weight: FontWeight::MEDIUM,
        section_weight: FontWeight::MEDIUM,
        title_weight: FontWeight::NORMAL,
        selected_title_weight: FontWeight::MEDIUM,
        source_weight: FontWeight::NORMAL,
        chip_weight: FontWeight::MEDIUM,
        kind_weight: FontWeight::MEDIUM,
        search_size: 22.5,
        ask_size: 15.0,
        section_size: 13.0,
        row_height: 59.0,
        icon_size: 27.0,
        title_size: 20.5,
        meta_size: 15.0,
        kind_size: 17.5,
        chip_size: 15.0,
        footer_size: 15.0,
        title_line_height: 23.5,
        metadata_line_height: 18.0,
        header_height: 80.0,
        footer_height: 58.0,
        row_gap: 4.5,
        content_gap: 12.0,
        row_radius: 13.0,
        search_opacity: 0.56,
        metadata_opacity: 0.56,
        kind_opacity: 0.68,
        selected_fill_opacity: 0.13,
    },
];

pub fn main_menu_raycast_weight_story_variants() -> Vec<StoryVariant> {
    SPECS
        .iter()
        .map(|spec| {
            StoryVariant::default_named(spec.id, spec.name)
                .description(spec.description)
                .with_prop("surface", "mainMenu")
                .with_prop("family", spec.family)
                .with_prop("variantId", spec.id)
        })
        .collect()
}

pub fn render_main_menu_raycast_weight_story_preview(stable_id: &str) -> AnyElement {
    render_spec_stage(resolve_spec(stable_id).unwrap_or(SPECS[0]), false)
}

pub fn render_main_menu_raycast_weight_compare_thumbnail(stable_id: &str) -> AnyElement {
    render_spec_stage(resolve_spec(stable_id).unwrap_or(SPECS[0]), true)
}

pub fn render_main_menu_raycast_weight_gallery() -> AnyElement {
    let theme = get_cached_theme();
    let mut root = story_container().gap_6().child(
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.colors.text.tertiary.to_rgb())
                    .child("Main Menu"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.colors.text.muted.to_rgb())
                    .child(
                        "Fifteen Raycast-inspired hierarchy studies that now vary size, line rhythm, density, and accessory emphasis as well as weight.",
                    ),
            ),
    );

    for family in ["Balanced", "Metadata", "Primary"] {
        let mut section = story_section(family).gap(px(12.0));
        for spec in SPECS.iter().copied().filter(|spec| spec.family == family) {
            section = section.child(render_gallery_item(spec));
        }
        root = root.child(section);
    }

    root.into_any_element()
}

fn resolve_spec(stable_id: &str) -> Option<MainMenuWeightSpec> {
    SPECS.iter().copied().find(|spec| spec.id == stable_id)
}

fn scale(value: f32, compact: bool) -> f32 {
    if compact {
        value * 0.60
    } else {
        value
    }
}

fn type_scale(value: f32, compact: bool) -> f32 {
    if compact {
        value * 0.58
    } else {
        value
    }
}

fn render_gallery_item(spec: MainMenuWeightSpec) -> AnyElement {
    let theme = get_cached_theme();
    div()
        .w(px(1220.0))
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(12.0))
        .bg(theme.colors.background.title_bar.with_opacity(0.22))
        .border_1()
        .border_color(theme.colors.ui.border.with_opacity(0.18))
        .rounded(px(12.0))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.colors.text.primary.to_rgb())
                        .child(spec.name),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.colors.text.muted.to_rgb())
                        .child(spec.description),
                ),
        )
        .child(render_spec_stage(spec, false))
        .into_any_element()
}

fn render_spec_stage(spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let theme = get_cached_theme();
    let width = if compact { 320.0 } else { 1216.0 };
    let height = if compact { 240.0 } else { 760.0 };

    div()
        .w(px(width))
        .h(px(height))
        .bg(theme.colors.background.main.with_opacity(0.28))
        .border_1()
        .border_color(theme.colors.ui.border.with_opacity(0.14))
        .rounded(px(16.0))
        .overflow_hidden()
        .child(render_main_menu_shell(spec, compact))
        .into_any_element()
}

fn render_main_menu_shell(spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let theme = get_cached_theme();

    div()
        .size_full()
        .flex()
        .flex_col()
        .bg(theme.colors.background.main.to_rgb())
        .rounded(px(18.0))
        .border_1()
        .border_color(theme.colors.ui.border.with_opacity(0.28))
        .child(render_header(spec, compact))
        .child(
            div().mx(px(scale(22.0, compact))).h(px(1.0)).bg(theme
                .colors
                .ui
                .border
                .with_opacity(0.24)),
        )
        .child(render_rows(spec, compact))
        .child(render_footer(spec, compact))
        .into_any_element()
}

fn render_header(spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let theme = get_cached_theme();
    let search_size = type_scale(spec.search_size, compact);
    let ask_size = type_scale(spec.ask_size, compact);
    let header = div()
        .w_full()
        .h(px(scale(spec.header_height, compact)))
        .px(px(scale(22.0, compact)))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(scale(10.0, compact)))
                .child(
                    div().w(px(2.0)).h(px(scale(34.0, compact))).bg(theme
                        .colors
                        .text
                        .primary
                        .with_opacity(0.70)),
                )
                .child(
                    div()
                        .text_size(px(search_size))
                        .line_height(px(search_size + type_scale(2.0, compact)))
                        .font_weight(spec.search_weight)
                        .text_color(theme.colors.text.primary.with_opacity(spec.search_opacity))
                        .child("Search for apps and commands..."),
                ),
        );

    if compact {
        header.into_any_element()
    } else {
        header
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(scale(12.0, compact)))
                    .child(
                        div()
                            .text_size(px(ask_size))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.colors.text.primary.with_opacity(0.64))
                            .child("Ask AI"),
                    )
                    .child(
                        div()
                            .px(px(scale(12.0, compact)))
                            .py(px(scale(6.0, compact)))
                            .rounded(px(scale(9.0, compact)))
                            .border_1()
                            .border_color(theme.colors.ui.border.with_opacity(0.32))
                            .text_size(px(type_scale(14.0, compact)))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(theme.colors.text.primary.with_opacity(0.60))
                            .child("Tab"),
                    ),
            )
            .into_any_element()
    }
}

fn render_rows(spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let mut column = div()
        .w_full()
        .flex_1()
        .min_h(px(0.0))
        .flex()
        .flex_col()
        .overflow_hidden()
        .px(px(scale(22.0, compact)))
        .pt(px(scale(18.0, compact)))
        .gap(px(scale(spec.row_gap, compact)));

    column = column.child(render_section_header("Suggestions", spec, compact));
    for row in ROWS.iter().take(if compact { 4 } else { 5 }).copied() {
        column = column.child(render_row(row, spec, compact));
    }

    if !compact {
        column = column.child(render_section_header("Commands", spec, compact));
        for row in ROWS.iter().skip(5).take(3).copied() {
            column = column.child(render_row(row, spec, compact));
        }
    }

    column.into_any_element()
}

fn render_section_header(
    label: &'static str,
    spec: MainMenuWeightSpec,
    compact: bool,
) -> AnyElement {
    let theme = get_cached_theme();
    let section_size = type_scale(spec.section_size, compact);

    div()
        .w_full()
        .pt(px(scale(6.0, compact)))
        .pb(px(scale(3.0, compact)))
        .text_size(px(section_size))
        .line_height(px(section_size + type_scale(2.0, compact)))
        .font_weight(spec.section_weight)
        .text_color(theme.colors.text.primary.with_opacity(0.62))
        .child(label)
        .into_any_element()
}

fn render_row(row: MenuPreviewRow, spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let theme = get_cached_theme();
    let row_height = scale(spec.row_height + 8.0, compact);
    let icon_size = scale(spec.icon_size, compact);
    let title_size = type_scale(spec.title_size, compact);
    let meta_size = type_scale(spec.meta_size, compact);
    let kind_size = type_scale(spec.kind_size, compact);
    let chip_size = type_scale(spec.chip_size, compact);
    let title_line_height = type_scale(spec.title_line_height, compact);
    let metadata_line_height = type_scale(spec.metadata_line_height, compact);

    div()
        .w_full()
        .h(px(row_height))
        .px(px(scale(14.0, compact)))
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .rounded(px(scale(spec.row_radius, compact)))
        .when(row.selected, |d| {
            d.bg(theme
                .colors
                .text
                .primary
                .with_opacity(spec.selected_fill_opacity))
        })
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(scale(spec.content_gap, compact)))
                .child(
                    div()
                        .size(px(icon_size))
                        .rounded(px(scale(8.0, compact)))
                        .bg(rgb(row.icon_bg))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(type_scale(14.0, compact)))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if row.icon_bg == 0xF2F2F2 {
                            rgb(0x3478F6)
                        } else {
                            rgb(0xFFFFFF)
                        })
                        .child(row.icon_label),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(scale(spec.content_gap - 2.0, compact)))
                        .child(
                            div()
                                .text_size(px(title_size))
                                .line_height(px(title_line_height))
                                .font_weight(if row.selected {
                                    spec.selected_title_weight
                                } else {
                                    spec.title_weight
                                })
                                .text_color(
                                    theme.colors.text.primary.with_opacity(if row.selected {
                                        0.96
                                    } else {
                                        0.90
                                    }),
                                )
                                .child(row.title),
                        )
                        .when(!row.source.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_size(px(meta_size))
                                    .line_height(px(metadata_line_height))
                                    .font_weight(spec.source_weight)
                                    .text_color(
                                        theme
                                            .colors
                                            .text
                                            .primary
                                            .with_opacity(spec.metadata_opacity),
                                    )
                                    .child(row.source),
                            )
                        })
                        .when_some(row.chip, |d, chip| {
                            d.child(
                                div()
                                    .px(px(scale(10.0, compact)))
                                    .py(px(scale(5.0, compact)))
                                    .rounded(px(scale(8.0, compact)))
                                    .border_1()
                                    .border_color(theme.colors.ui.border.with_opacity(0.34))
                                    .font_family(FONT_MONO)
                                    .text_size(px(chip_size))
                                    .line_height(px(chip_size + type_scale(2.0, compact)))
                                    .font_weight(spec.chip_weight)
                                    .text_color(theme.colors.text.primary.with_opacity(0.58))
                                    .child(chip),
                            )
                        }),
                ),
        )
        .child(
            div()
                .text_size(px(kind_size))
                .line_height(px(kind_size + type_scale(2.0, compact)))
                .font_weight(spec.kind_weight)
                .text_color(theme.colors.text.primary.with_opacity(spec.kind_opacity))
                .child(row.kind),
        )
        .into_any_element()
}

fn render_footer(spec: MainMenuWeightSpec, compact: bool) -> AnyElement {
    let theme = get_cached_theme();
    let text_size = type_scale(spec.footer_size, compact);

    div()
        .w_full()
        .h(px(scale(spec.footer_height, compact)))
        .px(px(scale(22.0, compact)))
        .border_t_1()
        .border_color(theme.colors.ui.border.with_opacity(0.24))
        .bg(theme.colors.background.title_bar.with_opacity(0.28))
        .flex()
        .items_center()
        .justify_end()
        .gap(px(scale(24.0, compact)))
        .child(render_footer_item("Open Command", "↩", text_size))
        .child(render_footer_item("Actions", "⌘K", text_size))
        .into_any_element()
}

fn render_footer_item(label: &'static str, shortcut: &'static str, text_size: f32) -> AnyElement {
    let theme = get_cached_theme();

    div()
        .flex()
        .items_center()
        .gap(px(scale_local(8.0, text_size)))
        .child(
            div()
                .text_size(px(text_size))
                .line_height(px(text_size + 2.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.colors.text.primary.with_opacity(0.88))
                .child(label),
        )
        .child(
            div()
                .px(px(scale_local(8.0, text_size)))
                .py(px(scale_local(4.0, text_size)))
                .rounded(px(scale_local(8.0, text_size)))
                .bg(theme.colors.text.primary.with_opacity(0.08))
                .font_family(FONT_MONO)
                .text_size(px(text_size - 1.0))
                .line_height(px(text_size + 1.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.colors.text.primary.with_opacity(0.58))
                .child(shortcut),
        )
        .into_any_element()
}

fn scale_local(base: f32, text_size: f32) -> f32 {
    if text_size < 13.0 {
        base * 0.72
    } else if text_size > 15.5 {
        base * 1.04
    } else {
        base
    }
}

#[cfg(test)]
mod tests {
    use super::{main_menu_raycast_weight_story_variants, SPECS};

    #[test]
    fn main_menu_raycast_story_exposes_fifteen_variants() {
        assert_eq!(main_menu_raycast_weight_story_variants().len(), 15);
        assert_eq!(SPECS.len(), 15);
    }
}
