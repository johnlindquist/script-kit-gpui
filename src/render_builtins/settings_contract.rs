// Pure settings-hub contract: item census, filter predicate, section
// labels, count-label copy, the action-descriptor contract (GEO-006), the
// structural iconless policy (GEO-007), and the layout resolver
// `render_settings` consumes.
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

// ── GEO-007: structural iconless policy ──────────────────────────────────

/// Settings rows are ICONLESS by policy, not by parser failure. The
/// [`SettingsItem`] struct has NO icon field, so a future icon-parser
/// improvement cannot silently make Settings rows display icons — adding
/// icons back is a deliberate API change to this contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsRowIconPolicy {
    Iconless,
}

pub const SETTINGS_ROW_ICON_POLICY: SettingsRowIconPolicy = SettingsRowIconPolicy::Iconless;

/// Settings item definition for the hub view. Structurally iconless — see
/// [`SettingsRowIconPolicy`].
pub struct SettingsItem {
    pub name: &'static str,
    pub description: &'static str,
    #[allow(dead_code)] // read by the binary's execute_settings_action only
    pub action: SettingsAction,
}

// ── GEO-006: canonical Settings action contract ──────────────────────────
//
// No shared cross-app action descriptor exists in the current tree (the
// candidates — `NotesActionDescriptor`, footer `FooterActionButton`,
// `actions::Action` — each lack primary verb and/or destination semantics),
// so this is the plan's Branch B: a NARROW Settings-only adapter. It is not
// a generic app action system and must not become a cross-surface registry;
// when a canonical shared descriptor lands, convert into it and delete these
// local types.

/// Stable, label-independent Settings action ID — the ONLY execution
/// currency between selection surfaces (Enter, click, native footer, GPUI
/// fallback) and the Settings execution owner.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SettingsActionId(&'static str);

impl SettingsActionId {
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

/// The one shortcut a Settings row exposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsShortcut {
    Enter,
}

/// Where an executed Settings action lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsDestination {
    pub surface: &'static str,
    pub operation: &'static str,
}

/// The full descriptor every projection (footer hint, native footer,
/// semantic projection, AX projection) and both execution routes consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsActionDescriptor {
    pub action_id: SettingsActionId,
    pub label: &'static str,
    pub primary_verb: &'static str,
    pub shortcut: SettingsShortcut,
    pub enabled: bool,
    pub disabled_reason: Option<&'static str>,
    pub destination: SettingsDestination,
}

/// Real runtime prerequisites that can disable a Settings action. Do NOT
/// manufacture disabled states: today only the configure-snap-mode builtin
/// can genuinely be missing from the builtin registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsActionAvailability {
    pub configure_snap_mode: bool,
}

impl SettingsActionAvailability {
    /// Everything available — the pure-contract default (exporter, tests).
    pub const fn all_available() -> Self {
        Self {
            configure_snap_mode: true,
        }
    }
}

pub const SETTINGS_CONFIGURE_SNAP_MODE_DISABLED_REASON: &str = "Configure Snap Mode is unavailable";

/// Action to execute when a settings item is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

/// Every Settings action, for exhaustive iteration (ID round-trips, tests).
pub const ALL_SETTINGS_ACTIONS: [SettingsAction; 12] = [
    SettingsAction::ChooseTheme,
    SettingsAction::DictationSetup,
    SettingsAction::SelectMicrophone,
    SettingsAction::ClearSuggested,
    SettingsAction::CheckPermissions,
    SettingsAction::SetupPermissions,
    SettingsAction::AllowAccessibility,
    SettingsAction::AllowScreenRecording,
    SettingsAction::RequestAccessibilityPermission,
    SettingsAction::OpenAccessibilitySettings,
    SettingsAction::ConfigureSnapMode,
    SettingsAction::ResetWindowPositions,
];

impl SettingsAction {
    /// Stable action ID (never derived from the user-visible label).
    pub const fn action_id(self) -> SettingsActionId {
        SettingsActionId::new(match self {
            Self::ChooseTheme => "settings.choose-theme",
            Self::DictationSetup => "settings.dictation-setup",
            Self::SelectMicrophone => "settings.select-microphone",
            Self::ClearSuggested => "settings.clear-suggested",
            Self::CheckPermissions => "settings.check-permissions",
            Self::SetupPermissions => "settings.setup-permissions",
            Self::AllowAccessibility => "settings.accessibility-assistant",
            Self::AllowScreenRecording => "settings.screen-recording-assistant",
            Self::RequestAccessibilityPermission => "settings.request-accessibility",
            Self::OpenAccessibilitySettings => "settings.open-accessibility-settings",
            Self::ConfigureSnapMode => "settings.configure-snap-mode",
            Self::ResetWindowPositions => "settings.reset-window-positions",
        })
    }

    /// Reverse of [`Self::action_id`]; `None` for unknown IDs.
    pub fn from_id(id: SettingsActionId) -> Option<Self> {
        ALL_SETTINGS_ACTIONS
            .into_iter()
            .find(|action| action.action_id() == id)
    }

    /// The full projection descriptor. Every action's primary verb is the
    /// canonical `Open` with the Enter shortcut; the destination operation
    /// stays honest (`clear`, `request`, `reset`, `run-check`).
    pub fn descriptor(self, availability: SettingsActionAvailability) -> SettingsActionDescriptor {
        let (label, destination) = match self {
            Self::ChooseTheme => (
                "Theme Designer",
                SettingsDestination {
                    surface: "settings/theme-designer",
                    operation: "open",
                },
            ),
            Self::DictationSetup => (
                "Dictation Setup",
                SettingsDestination {
                    surface: "builtin/dictation-setup",
                    operation: "open",
                },
            ),
            Self::SelectMicrophone => (
                "Select Microphone",
                SettingsDestination {
                    surface: "builtin/select-microphone",
                    operation: "open",
                },
            ),
            Self::ClearSuggested => (
                "Clear Suggested Items",
                SettingsDestination {
                    surface: "builtin/clear-suggested",
                    operation: "clear",
                },
            ),
            Self::CheckPermissions => (
                "Check Permissions",
                SettingsDestination {
                    surface: "builtin/check-permissions",
                    operation: "run-check",
                },
            ),
            Self::SetupPermissions => (
                "Set Up Permissions",
                SettingsDestination {
                    surface: "settings/permissions-wizard",
                    operation: "open",
                },
            ),
            Self::AllowAccessibility => (
                "Open Accessibility Assistant",
                SettingsDestination {
                    surface: "builtin/allow-accessibility",
                    operation: "open",
                },
            ),
            Self::AllowScreenRecording => (
                "Open Screen Recording Assistant",
                SettingsDestination {
                    surface: "builtin/allow-screen-recording",
                    operation: "open",
                },
            ),
            Self::RequestAccessibilityPermission => (
                "Request Accessibility Access",
                SettingsDestination {
                    surface: "builtin/request-accessibility",
                    operation: "request",
                },
            ),
            Self::OpenAccessibilitySettings => (
                "Open Accessibility Settings",
                SettingsDestination {
                    surface: "builtin/accessibility-settings",
                    operation: "open",
                },
            ),
            Self::ConfigureSnapMode => (
                "Choose Window Snap Mode",
                SettingsDestination {
                    surface: "builtin/configure-snap-mode",
                    operation: "open",
                },
            ),
            Self::ResetWindowPositions => (
                SETTINGS_HUB_OPTIONAL_ROW_NAME,
                SettingsDestination {
                    surface: "settings/window-positions",
                    operation: "reset",
                },
            ),
        };
        let (enabled, disabled_reason) = match self {
            Self::ConfigureSnapMode if !availability.configure_snap_mode => {
                (false, Some(SETTINGS_CONFIGURE_SNAP_MODE_DISABLED_REASON))
            }
            _ => (true, None),
        };
        SettingsActionDescriptor {
            action_id: self.action_id(),
            label,
            primary_verb: "Open",
            shortcut: SettingsShortcut::Enter,
            enabled,
            disabled_reason,
            destination,
        }
    }
}

/// The selected visible row's descriptor — the REQUIRED single source for
/// the GPUI footer hint, native footer, semantic/AX projections, Enter, and
/// the executing click. `None` when the filter leaves no visible rows.
pub fn selected_settings_action_descriptor(
    items: &[SettingsItem],
    filter: &str,
    selected_index: usize,
    availability: SettingsActionAvailability,
) -> Option<SettingsActionDescriptor> {
    filtered_settings_items(items, filter)
        .get(selected_index)
        .map(|item| item.action.descriptor(availability))
}

/// Deterministic item construction. 11 unconditional rows; the optional
/// `Reset Window Positions` row appends only under the explicit condition.
pub fn get_settings_items_for(has_custom_positions: bool) -> Vec<SettingsItem> {
    let mut items = vec![
        SettingsItem {
            name: "Theme Designer",
            description: "Design your color theme with live preview",
            action: SettingsAction::ChooseTheme,
        },
        SettingsItem {
            name: "Dictation Setup",
            description: "Check model, microphone, and hotkey readiness",
            action: SettingsAction::DictationSetup,
        },
        SettingsItem {
            name: "Select Microphone",
            description: "Choose which microphone to use for dictation",
            action: SettingsAction::SelectMicrophone,
        },
        SettingsItem {
            name: "Clear Suggested Items",
            description: "Reset Suggested and Recently Used launcher history",
            action: SettingsAction::ClearSuggested,
        },
        SettingsItem {
            name: "Check Permissions",
            description: "Run a check for the macOS permissions Script Kit needs",
            action: SettingsAction::CheckPermissions,
        },
        SettingsItem {
            name: "Set Up Permissions",
            description: "Open the guided wizard for granting macOS permissions",
            action: SettingsAction::SetupPermissions,
        },
        SettingsItem {
            name: "Open Accessibility Assistant",
            description: "Open the Permission Assistant for Accessibility",
            action: SettingsAction::AllowAccessibility,
        },
        SettingsItem {
            name: "Open Screen Recording Assistant",
            description: "Open the Permission Assistant for Screen Recording",
            action: SettingsAction::AllowScreenRecording,
        },
        SettingsItem {
            name: "Request Accessibility Access",
            description: "Prompt macOS to grant Script Kit accessibility access",
            action: SettingsAction::RequestAccessibilityPermission,
        },
        SettingsItem {
            name: "Open Accessibility Settings",
            description: "Open the Accessibility pane in macOS System Settings",
            action: SettingsAction::OpenAccessibilitySettings,
        },
    ];

    items.push(SettingsItem {
        name: "Choose Window Snap Mode",
        description: "Choose a snapping grid density or disable drag snapping",
        action: SettingsAction::ConfigureSnapMode,
    });

    if has_custom_positions {
        items.push(SettingsItem {
            name: SETTINGS_HUB_OPTIONAL_ROW_NAME,
            description: "Restore all windows to default positions",
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
    /// Themed section geometry/typography from the active main-menu theme
    /// (`MainMenuThemeDef::section_metrics`) — the shared section owner.
    #[allow(dead_code)] // consumed by render_settings + the design-contract exporter
    pub section: crate::designs::MainMenuSectionMetrics,
}

/// Explicit-theme layout resolver: `render_settings` resolves `menu_def`
/// FIRST and passes it here, so Settings section geometry provably comes
/// from the active main-menu theme.
pub fn resolved_settings_hub_layout_for(
    spacing: crate::designs::DesignSpacing,
    menu_theme: crate::designs::MainMenuThemeDef,
) -> SettingsHubLayout {
    SettingsHubLayout {
        list_padding_y: spacing.padding_xs,
        section: menu_theme.section_metrics(),
    }
}

/// Narrow contract summary for the design-token exporter — row census,
/// section labels, and the STRUCTURAL iconless truth (GEO-007). The icon
/// counts are structural zeros derived from the icon-field-free
/// [`SettingsItem`]; no icon parser is (or may be) invoked here.
#[allow(dead_code)] // consumed by the lib-side design-contract exporter; the binary compiles this file too
pub struct SettingsHubContractFacts {
    pub row_count: usize,
    pub icon_policy: SettingsRowIconPolicy,
    pub authored_icon_hint_rows: usize,
    pub distinct_authored_icon_hints: usize,
    pub resolved_icon_rows: usize,
    pub expected_ax_image_roles: usize,
    pub empty_filter_section_label: &'static str,
    pub filtered_section_label: &'static str,
}

#[allow(dead_code)] // consumed by the lib-side design-contract exporter; the binary compiles this file too
pub fn settings_hub_contract_facts(has_custom_positions: bool) -> SettingsHubContractFacts {
    let items = get_settings_items_for(has_custom_positions);
    SettingsHubContractFacts {
        row_count: items.len(),
        icon_policy: SETTINGS_ROW_ICON_POLICY,
        authored_icon_hint_rows: 0,
        distinct_authored_icon_hints: 0,
        resolved_icon_rows: 0,
        expected_ax_image_roles: 0,
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

    // ── GEO-006: action-descriptor contract ─────────────────────────────

    #[test]
    fn every_settings_action_has_unique_stable_id() {
        let mut ids: Vec<&'static str> = ALL_SETTINGS_ACTIONS
            .into_iter()
            .map(|action| action.action_id().as_str())
            .collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before, "duplicate settings action IDs");
        for id in ids {
            assert!(id.starts_with("settings."), "non-canonical id: {id}");
        }
    }

    #[test]
    fn every_settings_action_round_trips_through_id() {
        for action in ALL_SETTINGS_ACTIONS {
            assert_eq!(SettingsAction::from_id(action.action_id()), Some(action));
        }
        assert_eq!(
            SettingsAction::from_id(SettingsActionId::new("settings.not-a-real-action")),
            None
        );
    }

    #[test]
    fn every_settings_action_primary_verb_is_open() {
        for action in ALL_SETTINGS_ACTIONS {
            let descriptor = action.descriptor(SettingsActionAvailability::all_available());
            assert_eq!(descriptor.primary_verb, "Open", "{:?}", action);
        }
    }

    #[test]
    fn every_settings_action_has_enter_shortcut() {
        for action in ALL_SETTINGS_ACTIONS {
            let descriptor = action.descriptor(SettingsActionAvailability::all_available());
            assert_eq!(descriptor.shortcut, SettingsShortcut::Enter, "{:?}", action);
        }
    }

    #[test]
    fn every_settings_action_has_nonempty_destination() {
        for action in ALL_SETTINGS_ACTIONS {
            let descriptor = action.descriptor(SettingsActionAvailability::all_available());
            assert!(!descriptor.destination.surface.is_empty(), "{:?}", action);
            assert!(!descriptor.destination.operation.is_empty(), "{:?}", action);
        }
    }

    #[test]
    fn descriptor_label_matches_settings_row_label() {
        let availability = SettingsActionAvailability::all_available();
        for item in get_settings_items_for(true) {
            let descriptor = item.action.descriptor(availability);
            assert_eq!(descriptor.label, item.name);
        }
    }

    #[test]
    fn configure_snap_mode_descriptor_is_disabled_when_builtin_is_missing() {
        let missing = SettingsAction::ConfigureSnapMode.descriptor(SettingsActionAvailability {
            configure_snap_mode: false,
        });
        assert!(!missing.enabled);
        assert_eq!(
            missing.disabled_reason,
            Some(SETTINGS_CONFIGURE_SNAP_MODE_DISABLED_REASON)
        );
        // Availability disables ONLY its own action.
        for action in ALL_SETTINGS_ACTIONS {
            if action == SettingsAction::ConfigureSnapMode {
                continue;
            }
            let descriptor = action.descriptor(SettingsActionAvailability {
                configure_snap_mode: false,
            });
            assert!(descriptor.enabled, "{:?}", action);
            assert_eq!(descriptor.disabled_reason, None, "{:?}", action);
        }
        let present = SettingsAction::ConfigureSnapMode
            .descriptor(SettingsActionAvailability::all_available());
        assert!(present.enabled);
        assert_eq!(present.disabled_reason, None);
    }

    #[test]
    fn selected_descriptor_tracks_the_filtered_selection() {
        let items = get_settings_items_for(true);
        let availability = SettingsActionAvailability::all_available();
        // Empty filter, first row.
        let first = selected_settings_action_descriptor(&items, "", 0, availability)
            .expect("first row descriptor");
        assert_eq!(first.action_id.as_str(), "settings.choose-theme");
        assert_eq!(first.label, "Theme Designer");
        // Filtered: "theme" keeps Theme Designer first.
        let filtered = selected_settings_action_descriptor(&items, "theme", 0, availability)
            .expect("filtered descriptor");
        assert_eq!(filtered.action_id, first.action_id);
        // Out-of-range selection resolves to no descriptor (Back-only
        // footer), never a made-up action.
        assert_eq!(
            selected_settings_action_descriptor(&items, "no-such-row", 0, availability),
            None
        );
    }

    // ── GEO-007: structural iconless policy ─────────────────────────────

    #[test]
    fn icon_policy_is_structurally_iconless() {
        assert_eq!(SETTINGS_ROW_ICON_POLICY, SettingsRowIconPolicy::Iconless);
        assert_eq!(
            settings_hub_contract_facts(false).icon_policy,
            SettingsRowIconPolicy::Iconless
        );
    }

    #[test]
    fn icon_contract_counts_are_zero_with_and_without_optional_row() {
        for has_custom_positions in [false, true] {
            let facts = settings_hub_contract_facts(has_custom_positions);
            assert_eq!(facts.authored_icon_hint_rows, 0);
            assert_eq!(facts.distinct_authored_icon_hints, 0);
            assert_eq!(facts.resolved_icon_rows, 0);
            assert_eq!(facts.expected_ax_image_roles, 0);
        }
        assert_eq!(settings_hub_contract_facts(false).row_count, 11);
        assert_eq!(settings_hub_contract_facts(true).row_count, 12);
    }

    // ── GEO-007: themed section geometry ────────────────────────────────

    #[test]
    fn layout_padding_is_design_spacing_padding_xs() {
        let spacing = crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
        let menu_def = crate::designs::current_main_menu_theme().def();
        let layout = resolved_settings_hub_layout_for(spacing, menu_def);
        assert_eq!(layout.list_padding_y, spacing.padding_xs);
        assert_eq!(layout.list_padding_y, 4.0);
    }

    #[test]
    fn settings_section_metrics_come_from_active_main_menu_theme() {
        let spacing = crate::designs::get_tokens(crate::designs::DesignVariant::Default).spacing();
        let menu_def = crate::designs::current_main_menu_theme().def();
        let layout = resolved_settings_hub_layout_for(spacing, menu_def);
        assert_eq!(layout.section, menu_def.section_metrics());
    }

    #[test]
    fn section_labels_are_the_contract_constants() {
        let facts = settings_hub_contract_facts(false);
        assert_eq!(facts.empty_filter_section_label, "Settings");
        assert_eq!(facts.filtered_section_label, "Results");
    }
}
