// Pure settings-hub contract: item census, filter predicate, section
// labels, count-label copy, and the layout resolver `render_settings`
// consumes.
//
// The ONLY config input is the explicit `has_custom_positions` condition —
// the live `crate::window_state::has_custom_positions()` read stays in the
// binary-side `get_settings_items()` wrapper (`settings.rs`) so the
// design-token exporter can census both states without reading the
// developer's HOME (2026-07-11 Oracle review, settings-hub slice).
//
// Physically lives under `src/render_builtins/` (pulled into the binary via
// the `render_builtins/mod.rs` include chain); the lib re-exports the same
// file (`#[path]` module in `src/lib.rs`, the `path_action` pattern) so the
// exporter and `cargo test --lib` reach it without linking the binary.

/// Persistent leading separator label with an empty filter (POLISH.md §2 —
/// the row never appears/disappears; only the label swaps).
pub const SETTINGS_HUB_EMPTY_FILTER_SECTION_LABEL: &str = "Settings";
/// Persistent leading separator label while a filter is active.
pub const SETTINGS_HUB_FILTERED_SECTION_LABEL: &str = "Results";
/// The one config-dependent row: appended only when the window-state store
/// holds custom positions (`windowState.hasCustomPositions`).
pub const SETTINGS_HUB_OPTIONAL_ROW_NAME: &str = "Reset Window Positions";

/// Settings item definition for the hub view.
pub struct SettingsItem {
    pub name: &'static str,
    pub description: &'static str,
    pub icon: &'static str,
    #[allow(dead_code)] // read by the binary's execute_settings_action only
    pub action: SettingsAction,
}

/// Action to execute when a settings item is selected.
#[derive(Clone)]
pub enum SettingsAction {
    ChooseTheme,
    DictationSetup,
    SelectMicrophone,
    ClearSuggested,
    CheckPermissions,
    SetupPermissions,
    AllowAccessibility,
    AllowScreenRecording,
    RequestAccessibilityPermission,
    OpenAccessibilitySettings,
    ConfigureSnapMode,
    ResetWindowPositions,
}

/// Deterministic item construction. 11 unconditional rows; the optional
/// `Reset Window Positions` row appends only under the explicit condition.
pub fn get_settings_items_for(has_custom_positions: bool) -> Vec<SettingsItem> {
    let mut items = vec![
        SettingsItem {
            name: "Theme Designer",
            description: "Design your color theme with live preview",
            icon: "palette",
            action: SettingsAction::ChooseTheme,
        },
        SettingsItem {
            name: "Dictation Setup",
            description: "Check model, microphone, and hotkey readiness",
            icon: "sliders-horizontal",
            action: SettingsAction::DictationSetup,
        },
        SettingsItem {
            name: "Select Microphone",
            description: "Choose which microphone to use for dictation",
            icon: "mic",
            action: SettingsAction::SelectMicrophone,
        },
        SettingsItem {
            name: "Clear Suggested Items",
            description: "Reset Suggested and Recently Used launcher history",
            icon: "eraser",
            action: SettingsAction::ClearSuggested,
        },
        SettingsItem {
            name: "Check Permissions",
            description: "Run a check for the macOS permissions Script Kit needs",
            icon: "circle-check",
            action: SettingsAction::CheckPermissions,
        },
        SettingsItem {
            name: "Set Up Permissions",
            description: "Open the guided wizard for granting macOS permissions",
            icon: "shield-check",
            action: SettingsAction::SetupPermissions,
        },
        SettingsItem {
            name: "Open Accessibility Assistant",
            description: "Open the Permission Assistant for Accessibility",
            icon: "shield-check",
            action: SettingsAction::AllowAccessibility,
        },
        SettingsItem {
            name: "Open Screen Recording Assistant",
            description: "Open the Permission Assistant for Screen Recording",
            icon: "shield-check",
            action: SettingsAction::AllowScreenRecording,
        },
        SettingsItem {
            name: "Request Accessibility Access",
            description: "Prompt macOS to grant Script Kit accessibility access",
            icon: "key-round",
            action: SettingsAction::RequestAccessibilityPermission,
        },
        SettingsItem {
            name: "Open Accessibility Settings",
            description: "Open the Accessibility pane in macOS System Settings",
            icon: "accessibility",
            action: SettingsAction::OpenAccessibilitySettings,
        },
    ];

    items.push(SettingsItem {
        name: "Choose Window Snap Mode",
        description: "Choose a snapping grid density or disable drag snapping",
        icon: "square-split-horizontal",
        action: SettingsAction::ConfigureSnapMode,
    });

    if has_custom_positions {
        items.push(SettingsItem {
            name: SETTINGS_HUB_OPTIONAL_ROW_NAME,
            description: "Restore all windows to default positions",
            icon: "rotate-ccw",
            action: SettingsAction::ResetWindowPositions,
        });
    }

    items
}

pub fn settings_item_matches_filter(item: &SettingsItem, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }

    let filter_lower = filter.to_lowercase();
    item.name.to_lowercase().contains(&filter_lower)
        || item.description.to_lowercase().contains(&filter_lower)
}

pub fn filtered_settings_items<'a>(
    items: &'a [SettingsItem],
    filter: &str,
) -> Vec<&'a SettingsItem> {
    items
        .iter()
        .filter(|item| settings_item_matches_filter(item, filter))
        .collect()
}

/// Count-label copy: "1 setting" / "N settings" over the VISIBLE (filtered)
/// row count — pluralization is behavior-tested here, never reconstructed in
/// the exporter.
pub fn format_settings_count_label(count: usize) -> String {
    format!("{} setting{}", count, if count == 1 { "" } else { "s" })
}

/// Layout values `render_settings` consumes (and the token exporter mirrors).
pub struct SettingsHubLayout {
    /// Content column `py` — `DesignSpacing.padding_xs` (the canonical
    /// `design.spacing.paddingXs` source token, NOT a settings alias).
    pub list_padding_y: f32,
}

pub fn resolved_settings_hub_layout(spacing: crate::designs::DesignSpacing) -> SettingsHubLayout {
    SettingsHubLayout {
        list_padding_y: spacing.padding_xs,
    }
}

/// Narrow contract summary for the design-token exporter — row census,
/// section labels, and the authored-vs-resolved icon truth — derived from
/// the REAL item definitions plus `IconKind::from_icon_hint`, never a
/// hardcoded list.
#[allow(dead_code)] // consumed by the lib-side design-contract exporter; the binary compiles this file too
pub struct SettingsHubContractFacts {
    pub row_count: usize,
    pub authored_icon_hint_rows: usize,
    pub distinct_authored_icon_hints: usize,
    pub resolved_icon_rows: usize,
    pub empty_filter_section_label: &'static str,
    pub filtered_section_label: &'static str,
}

#[allow(dead_code)] // consumed by the lib-side design-contract exporter; the binary compiles this file too
pub fn settings_hub_contract_facts(has_custom_positions: bool) -> SettingsHubContractFacts {
    let items = get_settings_items_for(has_custom_positions);
    let authored: Vec<&'static str> = items
        .iter()
        .map(|item| item.icon)
        .filter(|icon| !icon.trim().is_empty())
        .collect();
    let mut distinct = authored.clone();
    distinct.sort_unstable();
    distinct.dedup();
    let resolved_icon_rows = items
        .iter()
        .filter(|item| crate::list_item::IconKind::from_icon_hint(item.icon).is_some())
        .count();
    SettingsHubContractFacts {
        row_count: items.len(),
        authored_icon_hint_rows: authored.len(),
        distinct_authored_icon_hints: distinct.len(),
        resolved_icon_rows,
        empty_filter_section_label: SETTINGS_HUB_EMPTY_FILTER_SECTION_LABEL,
        filtered_section_label: SETTINGS_HUB_FILTERED_SECTION_LABEL,
    }
}

#[cfg(test)]
mod settings_hub_contract_behavior {
    use super::*;

    #[test]
    fn census_is_11_without_and_12_with_custom_positions() {
        assert_eq!(get_settings_items_for(false).len(), 11);
        assert_eq!(get_settings_items_for(true).len(), 12);
        let with_names: Vec<_> = get_settings_items_for(true)
            .iter()
            .map(|item| item.name)
            .collect();
        assert_eq!(
            with_names.last().copied(),
            Some(SETTINGS_HUB_OPTIONAL_ROW_NAME)
        );
        assert!(!get_settings_items_for(false)
            .iter()
            .any(|item| item.name == SETTINGS_HUB_OPTIONAL_ROW_NAME));
    }

    #[test]
    fn count_label_pluralizes() {
        assert_eq!(format_settings_count_label(1), "1 setting");
        assert_eq!(format_settings_count_label(11), "11 settings");
        assert_eq!(format_settings_count_label(12), "12 settings");
    }

    #[test]
    fn empty_filter_shows_all_rows_and_filter_narrows_visible_rows() {
        let items = get_settings_items_for(true);
        // Empty filter: the count label counts ALL rows.
        assert_eq!(filtered_settings_items(&items, "").len(), items.len());
        // Active filter: the count label counts VISIBLE rows only.
        let filtered = filtered_settings_items(&items, "permission");
        assert!(!filtered.is_empty());
        assert!(filtered.len() < items.len());
        // Matching is case-insensitive over name OR description.
        assert!(filtered_settings_items(&items, "THEME")
            .iter()
            .any(|item| item.name == "Theme Designer"));
    }

    /// Painted truth: every authored icon hint fails
    /// `IconKind::from_icon_hint` (lucide-style names unknown to
    /// `icon_name_from_str`; ASCII content rejects the emoji fallback), so
    /// rows paint ICONLESS — text origin at outer 4 + inner 14 = 18px.
    ///
    /// If the parser ever learns these names this test fails ON PURPOSE:
    /// remove/update the `settingsRows.authoredIconHintsVsResolvedNone`
    /// conflict, add icons to the settings mockup, shift the row-name
    /// origins, and regenerate the reference receipt in the SAME change.
    #[test]
    fn authored_icon_hints_resolve_to_zero_row_icons() {
        let with = settings_hub_contract_facts(true);
        assert_eq!(with.row_count, 12);
        assert_eq!(with.authored_icon_hint_rows, 12);
        assert_eq!(with.distinct_authored_icon_hints, 10);
        assert_eq!(with.resolved_icon_rows, 0);

        let without = settings_hub_contract_facts(false);
        assert_eq!(without.row_count, 11);
        assert_eq!(without.authored_icon_hint_rows, 11);
        assert_eq!(without.distinct_authored_icon_hints, 9);
        assert_eq!(without.resolved_icon_rows, 0);
    }

    #[test]
    fn layout_padding_is_design_spacing_padding_xs() {
        let spacing = crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
        let layout = resolved_settings_hub_layout(spacing);
        assert_eq!(layout.list_padding_y, spacing.padding_xs);
        assert_eq!(layout.list_padding_y, 4.0);
    }

    #[test]
    fn section_labels_are_the_contract_constants() {
        let facts = settings_hub_contract_facts(false);
        assert_eq!(facts.empty_filter_section_label, "Settings");
        assert_eq!(facts.filtered_section_label, "Results");
    }
}
