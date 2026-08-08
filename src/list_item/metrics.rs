//! GEO-009 — Explicit list presentation modes for every predictive list
//! calculation.
//!
//! A predictive caller (anything that computes list/window geometry before or
//! outside paint) must first choose a [`ListPresentationMode`] and resolve a
//! [`ResolvedListPresentationMetrics`] for it. There is no mode-less
//! predictive API: the helpers in this module require the resolved object, so
//! the compiler rejects callers that never chose a mode.
//!
//! Mode rules (program plan Step 11):
//! - `MainMenuThemed` uses the current themed `ListItemMetricsOverride`
//!   resolvers (the exact metrics the launcher renderer paints).
//! - `ActionsConstrained` preserves the Actions popup's constrained density
//!   from `crate::designs::current_actions_popup_theme()` — read-only; no
//!   general-row expansion, no new Actions style.
//! - `SelectPromptUnified` uses the exact `UnifiedListItem` Comfortable layout
//!   the SelectPrompt renderer paints.
//! - `LegacyCompatibility` is the historical constant model
//!   (`LIST_ITEM_HEIGHT`/`SECTION_HEADER_HEIGHT`), allowed only for callers
//!   with a [`LegacyListCallerRecord`] carrying an explicit deletion trigger.
//!
//! Selected paint stays component-resolved: predictions export only the
//! [`SelectedPaintDerivation`] metadata, never copied color bytes.
//!
//! Specialized densities (browse cards, micro rows) do not enter the general
//! resolver; they use [`SpecializedListPresentationMetrics`] with a typed
//! owner and rationale.

use super::geometry_roles::GeometryRole;

/// The four standard predictive list presentations. No default; every caller
/// names its mode.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ListPresentationMode {
    MainMenuThemed,
    ActionsConstrained,
    LegacyCompatibility,
    SelectPromptUnified,
}

impl ListPresentationMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MainMenuThemed => "MainMenuThemed",
            Self::ActionsConstrained => "ActionsConstrained",
            Self::LegacyCompatibility => "LegacyCompatibility",
            Self::SelectPromptUnified => "SelectPromptUnified",
        }
    }
}

/// All modes, for exhaustive enumeration in exports and tests.
pub const ALL_LIST_PRESENTATION_MODES: &[ListPresentationMode] = &[
    ListPresentationMode::MainMenuThemed,
    ListPresentationMode::ActionsConstrained,
    ListPresentationMode::LegacyCompatibility,
    ListPresentationMode::SelectPromptUnified,
];

/// Which section slot a predictive row occupies. First and ordinary sections
/// share role `SectionSlot` but are distinct metrics, never aliases of each
/// other or of `RowSlot`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SectionSlotKind {
    First,
    Ordinary,
    /// A reserved first-section slot the renderer actually reserves
    /// (`GroupedListItem::ReservedSectionSlot`); uses the first-section metric.
    ReservedFirst,
}

/// How the selected-row paint is derived. Prediction exports only this
/// metadata; the paint truth stays with the rendering component.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SelectedPaintDerivation {
    ComponentResolved {
        component: &'static str,
        resolver: &'static str,
        token_family: &'static str,
    },
}

/// Resolved predictive metrics for one presentation mode.
///
/// Contains no color bytes: selected paint is derivation metadata only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedListPresentationMetrics {
    pub mode: ListPresentationMode,
    pub row_slot_height: f32,
    pub first_section_slot_height: f32,
    pub section_slot_height: f32,
    pub source_status_slot_height: f32,
    pub row_role: GeometryRole,
    pub section_role: GeometryRole,
    pub row_metric_source: &'static str,
    pub first_section_metric_source: &'static str,
    pub section_metric_source: &'static str,
    pub status_metric_source: &'static str,
    pub selected_paint_derivation: SelectedPaintDerivation,
}

/// Resolve the predictive metrics for an explicitly chosen mode.
///
/// This is the ONLY entry into standard predictive list metrics; there is no
/// row-count-only or `ViewType`-only overload.
pub fn resolved_list_presentation_metrics(
    mode: ListPresentationMode,
    design: crate::designs::DesignVariant,
    menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    match mode {
        ListPresentationMode::MainMenuThemed => {
            resolve_main_menu_themed_metrics(design, menu_theme)
        }
        ListPresentationMode::ActionsConstrained => {
            resolve_actions_constrained_metrics(design, menu_theme)
        }
        ListPresentationMode::LegacyCompatibility => {
            resolve_legacy_compatibility_metrics(design, menu_theme)
        }
        ListPresentationMode::SelectPromptUnified => {
            resolve_select_prompt_unified_metrics(design, menu_theme)
        }
    }
}

fn resolve_main_menu_themed_metrics(
    _design: crate::designs::DesignVariant,
    menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    let themed = super::ListItemMetricsOverride::from_main_menu_theme(menu_theme);
    ResolvedListPresentationMetrics {
        mode: ListPresentationMode::MainMenuThemed,
        row_slot_height: themed.item_height,
        first_section_slot_height: themed.first_section_header_height,
        section_slot_height: themed.section_header_height,
        source_status_slot_height: themed.source_status_row_height,
        row_role: GeometryRole::RowSlot,
        section_role: GeometryRole::SectionSlot,
        row_metric_source: "crate::list_item::ListItemMetricsOverride::from_main_menu_theme::item_height",
        first_section_metric_source:
            "crate::list_item::ListItemMetricsOverride::from_main_menu_theme::first_section_header_height",
        section_metric_source:
            "crate::list_item::ListItemMetricsOverride::from_main_menu_theme::section_header_height",
        status_metric_source:
            "crate::list_item::ListItemMetricsOverride::from_main_menu_theme::source_status_row_height",
        selected_paint_derivation: SelectedPaintDerivation::ComponentResolved {
            component: "crate::list_item::ListItem",
            resolver: "crate::list_item::resolved_main_menu_row_fill",
            token_family: "MainMenuThemeDef::row",
        },
    }
}

fn resolve_actions_constrained_metrics(
    _design: crate::designs::DesignVariant,
    _menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    // Read-only consumption of the Actions popup's own constrained tokens.
    // Do NOT copy these values anywhere or widen them to the general row.
    let tokens = crate::designs::current_actions_popup_theme();
    ResolvedListPresentationMetrics {
        mode: ListPresentationMode::ActionsConstrained,
        row_slot_height: tokens.list.row_height,
        // The Actions popup renders one section-header metric; the first/
        // ordinary fields stay distinct metric IDs even when values agree.
        first_section_slot_height: tokens.list.section_header_height,
        section_slot_height: tokens.list.section_header_height,
        source_status_slot_height: 0.0,
        row_role: GeometryRole::RowSlot,
        section_role: GeometryRole::SectionSlot,
        row_metric_source: "crate::designs::current_actions_popup_theme::list.row_height",
        first_section_metric_source:
            "crate::designs::current_actions_popup_theme::list.section_header_height(first)",
        section_metric_source:
            "crate::designs::current_actions_popup_theme::list.section_header_height(ordinary)",
        status_metric_source: "actions-popup:none",
        selected_paint_derivation: SelectedPaintDerivation::ComponentResolved {
            component: "crate::actions::dialog",
            resolver: "actions_dialog_default_style",
            token_family: "ActionsPopupThemeDef::row",
        },
    }
}

fn resolve_legacy_compatibility_metrics(
    _design: crate::designs::DesignVariant,
    _menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    ResolvedListPresentationMetrics {
        mode: ListPresentationMode::LegacyCompatibility,
        row_slot_height: super::LIST_ITEM_HEIGHT,
        // The legacy constant model never had a distinct first-section value;
        // the metric IDs remain distinct so the fields can diverge when a
        // ledgered caller is migrated.
        first_section_slot_height: super::SECTION_HEADER_HEIGHT,
        section_slot_height: super::SECTION_HEADER_HEIGHT,
        source_status_slot_height: super::SOURCE_STATUS_ROW_HEIGHT,
        row_role: GeometryRole::RowSlot,
        section_role: GeometryRole::SectionSlot,
        row_metric_source: "crate::list_item::LIST_ITEM_HEIGHT",
        first_section_metric_source: "crate::list_item::SECTION_HEADER_HEIGHT(first)",
        section_metric_source: "crate::list_item::SECTION_HEADER_HEIGHT(ordinary)",
        status_metric_source: "crate::list_item::SOURCE_STATUS_ROW_HEIGHT",
        selected_paint_derivation: SelectedPaintDerivation::ComponentResolved {
            component: "crate::list_item::ListItem",
            resolver: "crate::list_item::ListItemColors::from_theme",
            token_family: "theme.colors.accent",
        },
    }
}

fn resolve_select_prompt_unified_metrics(
    _design: crate::designs::DesignVariant,
    _menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    let layout = crate::components::unified_list_item::ListItemLayout::from_density(
        crate::components::unified_list_item::Density::Comfortable,
    );
    ResolvedListPresentationMetrics {
        mode: ListPresentationMode::SelectPromptUnified,
        row_slot_height: layout.height,
        // SelectPrompt renders no section headers today; the slots stay
        // explicit (and zero) instead of silently borrowing another mode's.
        first_section_slot_height: 0.0,
        section_slot_height: 0.0,
        source_status_slot_height: 0.0,
        row_role: GeometryRole::RowSlot,
        section_role: GeometryRole::SectionSlot,
        row_metric_source:
            "crate::components::unified_list_item::ListItemLayout::from_density(Comfortable).height",
        first_section_metric_source: "unified-list-item:none(first)",
        section_metric_source: "unified-list-item:none(ordinary)",
        status_metric_source: "unified-list-item:none",
        selected_paint_derivation: SelectedPaintDerivation::ComponentResolved {
            component: "crate::components::unified_list_item::UnifiedListItem",
            resolver: "UnifiedListItemColors::from_theme",
            token_family: "theme.colors",
        },
    }
}

/// One predictive slot in a content-height calculation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PredictiveListSlot {
    Row,
    FirstSection,
    Section,
    ReservedFirstSection,
    SourceStatus,
    /// A section header the renderer suppresses (e.g.
    /// `hide_initial_section_header`); contributes zero height explicitly.
    HiddenSection,
}

/// Sum the content height for an ordered slot sequence under one resolved
/// mode. This replaces ad-hoc `count * LIST_ITEM_HEIGHT` arithmetic; there is
/// no overload that accepts only a row count or a `ViewType`.
pub fn resolved_list_content_height(
    metrics: &ResolvedListPresentationMetrics,
    slots: impl IntoIterator<Item = PredictiveListSlot>,
) -> f32 {
    slots
        .into_iter()
        .map(|slot| match slot {
            PredictiveListSlot::Row => metrics.row_slot_height,
            PredictiveListSlot::FirstSection => metrics.first_section_slot_height,
            PredictiveListSlot::Section => metrics.section_slot_height,
            PredictiveListSlot::ReservedFirstSection => metrics.first_section_slot_height,
            PredictiveListSlot::SourceStatus => metrics.source_status_slot_height,
            PredictiveListSlot::HiddenSection => 0.0,
        })
        .sum()
}

/// Typed metrics for rows that intentionally do NOT participate in the
/// standard row system (browse cards, micro rows, …). Specialized densities
/// never masquerade as a standard mode.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpecializedListPresentationMetrics {
    pub owner: &'static str,
    pub row_height: f32,
    pub rationale: &'static str,
}

/// Every production caller still allowed to predict with the legacy constant
/// model. Additions require a ledger entry with a deletion trigger; a caller
/// that is not in this ledger must not resolve `LegacyCompatibility`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LegacyListCallerId {
    /// Arg prompt choice sizing (`src/window_resize/arg_layout.rs` adapter).
    /// The renderer is the legacy `ListItem` component (themed metrics at
    /// paint); prediction resolves the same themed override.
    ArgPromptChoices,
    /// Mini prompt choice sizing; same renderer family as Arg.
    MiniPromptChoices,
    /// Main-window selectable-row budget (`capped_main_window_selectable_rows`
    /// and `height_for_view` flat fallback) still on the 40px constant model.
    MainWindowRowBudget,
    /// Installed-kit store rows modeled in `build_layout_info.rs`.
    InstalledKits,
    /// Generic filterable builtin rows modeled in `build_layout_info.rs`.
    GenericFilterable,
    /// Attachment portal rows modeled in `build_layout_info.rs`.
    AttachmentPortal,
}

/// Ledger record binding a legacy caller to its source and deletion trigger.
pub struct LegacyListCallerRecord {
    pub id: LegacyListCallerId,
    pub source_path: &'static str,
    pub source_symbol: &'static str,
    pub renderer_owner: &'static str,
    pub deletion_trigger: &'static str,
}

/// The authoritative legacy-caller ledger. A source inventory must match this
/// list in both directions (no unledgered caller, no stale entry).
pub const LEGACY_LIST_CALLERS: &[LegacyListCallerRecord] = &[
    LegacyListCallerRecord {
        id: LegacyListCallerId::ArgPromptChoices,
        source_path: "src/window_resize/mod.rs",
        source_symbol: "height_for_view_with_layout::ArgPromptWithChoices",
        renderer_owner: "crate::list_item::ListItem (themed metrics at paint)",
        deletion_trigger: "Arg choice list migrates to UnifiedListItem (SelectPromptUnified)",
    },
    LegacyListCallerRecord {
        id: LegacyListCallerId::MiniPromptChoices,
        source_path: "src/window_resize/mod.rs",
        source_symbol: "height_for_view_with_layout::MiniPrompt",
        renderer_owner: "crate::list_item::ListItem (themed metrics at paint)",
        deletion_trigger: "Mini choice list migrates to UnifiedListItem (SelectPromptUnified)",
    },
    LegacyListCallerRecord {
        id: LegacyListCallerId::MainWindowRowBudget,
        source_path: "src/window_resize/mod.rs",
        source_symbol: "capped_main_window_selectable_rows",
        renderer_owner: "crate::render_script_list (themed rows at paint)",
        deletion_trigger: "main-window row budget reconciled with themed row metrics \
                           (IR-05: app_impl/ui_window.rs callers pass MainMenuThemed metrics)",
    },
    LegacyListCallerRecord {
        id: LegacyListCallerId::InstalledKits,
        source_path: "src/app_layout/build_layout_info.rs",
        source_symbol: "KIT_STORE_ROW_HEIGHT",
        renderer_owner: "render_builtins kit-store rows",
        deletion_trigger: "kit-store layout model re-derived from its active row renderer",
    },
    LegacyListCallerRecord {
        id: LegacyListCallerId::GenericFilterable,
        source_path: "src/app_layout/build_layout_info.rs",
        source_symbol: "GENERIC_ROW_HEIGHT",
        renderer_owner: "render_builtins generic filterable rows",
        deletion_trigger: "generic-filterable layout model re-derived from its active row renderer",
    },
    LegacyListCallerRecord {
        id: LegacyListCallerId::AttachmentPortal,
        source_path: "src/app_layout/build_layout_info.rs",
        source_symbol: "PORTAL_ROW_HEIGHT",
        renderer_owner: "attachment portal rows",
        deletion_trigger: "attachment-portal layout model re-derived from its active row renderer",
    },
];

/// Resolve legacy metrics for a ledgered caller. Panics (debug contract) when
/// the caller is not in the ledger — stale callers fail tests instead of
/// silently predicting with the legacy constant.
pub fn resolved_legacy_metrics_for_caller(
    caller: LegacyListCallerId,
    design: crate::designs::DesignVariant,
    menu_theme: crate::designs::MainMenuThemeVariant,
) -> ResolvedListPresentationMetrics {
    debug_assert!(
        LEGACY_LIST_CALLERS.iter().any(|record| record.id == caller),
        "legacy list caller {caller:?} is not in LEGACY_LIST_CALLERS"
    );
    resolved_list_presentation_metrics(
        ListPresentationMode::LegacyCompatibility,
        design,
        menu_theme,
    )
}

#[cfg(test)]
mod list_presentation_mode_contract_tests {
    use super::*;

    fn defaults() -> (
        crate::designs::DesignVariant,
        crate::designs::MainMenuThemeVariant,
    ) {
        (
            crate::designs::DesignVariant::default(),
            crate::designs::MainMenuThemeVariant::default(),
        )
    }

    #[test]
    fn all_four_modes_resolve() {
        let (design, menu_theme) = defaults();
        for mode in ALL_LIST_PRESENTATION_MODES {
            let metrics = resolved_list_presentation_metrics(*mode, design, menu_theme);
            assert_eq!(metrics.mode, *mode);
            assert!(metrics.row_slot_height >= 0.0);
        }
        assert_eq!(ALL_LIST_PRESENTATION_MODES.len(), 4);
    }

    #[test]
    fn main_menu_uses_themed_metrics() {
        let (design, menu_theme) = defaults();
        let metrics = resolved_list_presentation_metrics(
            ListPresentationMode::MainMenuThemed,
            design,
            menu_theme,
        );
        let themed = crate::list_item::ListItemMetricsOverride::from_main_menu_theme(menu_theme);
        assert_eq!(metrics.row_slot_height, themed.item_height);
        assert_eq!(
            metrics.first_section_slot_height,
            themed.first_section_header_height
        );
        assert_eq!(metrics.section_slot_height, themed.section_header_height);
        assert_eq!(
            metrics.source_status_slot_height,
            themed.source_status_row_height
        );
        assert!(metrics.row_metric_source.contains("from_main_menu_theme"));
    }

    #[test]
    fn actions_preserves_constrained_density() {
        let (design, menu_theme) = defaults();
        let metrics = resolved_list_presentation_metrics(
            ListPresentationMode::ActionsConstrained,
            design,
            menu_theme,
        );
        let tokens = crate::designs::current_actions_popup_theme();
        assert_eq!(metrics.row_slot_height, tokens.list.row_height);
        assert_eq!(
            metrics.section_slot_height,
            tokens.list.section_header_height
        );
        // Constrained density must remain distinct from the themed general row.
        let themed = resolved_list_presentation_metrics(
            ListPresentationMode::MainMenuThemed,
            design,
            menu_theme,
        );
        assert_ne!(metrics.row_slot_height, themed.row_slot_height);
        assert!(metrics
            .row_metric_source
            .contains("current_actions_popup_theme"));
    }

    #[test]
    fn select_uses_unified_renderer_metrics() {
        let (design, menu_theme) = defaults();
        let metrics = resolved_list_presentation_metrics(
            ListPresentationMode::SelectPromptUnified,
            design,
            menu_theme,
        );
        let layout = crate::components::unified_list_item::ListItemLayout::from_density(
            crate::components::unified_list_item::Density::Comfortable,
        );
        assert_eq!(metrics.row_slot_height, layout.height);
        assert!(metrics.row_metric_source.contains("unified_list_item"));
    }

    #[test]
    fn legacy_callers_match_deletion_ledger() {
        use LegacyListCallerId as Id;
        let expected = [
            Id::ArgPromptChoices,
            Id::MiniPromptChoices,
            Id::MainWindowRowBudget,
            Id::InstalledKits,
            Id::GenericFilterable,
            Id::AttachmentPortal,
        ];
        // Both directions: every expected caller is ledgered, no extras.
        assert_eq!(LEGACY_LIST_CALLERS.len(), expected.len());
        for id in expected {
            let record = LEGACY_LIST_CALLERS
                .iter()
                .find(|record| record.id == id)
                .unwrap_or_else(|| panic!("legacy caller {id:?} missing from ledger"));
            assert!(
                !record.deletion_trigger.is_empty(),
                "{id:?} needs a deletion trigger"
            );
            assert!(!record.source_path.is_empty());
            assert!(!record.source_symbol.is_empty());
            assert!(!record.renderer_owner.is_empty());
        }
    }

    #[test]
    fn first_and_ordinary_section_slots_are_distinct() {
        let (design, menu_theme) = defaults();
        for mode in ALL_LIST_PRESENTATION_MODES {
            let metrics = resolved_list_presentation_metrics(*mode, design, menu_theme);
            // Distinct metric identities in every mode…
            assert_ne!(
                metrics.first_section_metric_source, metrics.section_metric_source,
                "{mode:?} first/ordinary section metric IDs must differ"
            );
        }
        // Metric identity stays distinct even when the active theme currently
        // authors equal numeric heights. Equality of values must not collapse
        // the first/ordinary section ownership distinction.
        let themed = resolved_list_presentation_metrics(
            ListPresentationMode::MainMenuThemed,
            design,
            menu_theme,
        );
        assert_eq!(
            themed.first_section_slot_height, themed.section_slot_height,
            "the current theme intentionally authors equal section heights"
        );
        assert_ne!(
            themed.first_section_metric_source, themed.section_metric_source,
            "equal values must retain distinct metric identities"
        );
    }

    #[test]
    fn row_and_section_roles_are_not_aliases() {
        let (design, menu_theme) = defaults();
        for mode in ALL_LIST_PRESENTATION_MODES {
            let metrics = resolved_list_presentation_metrics(*mode, design, menu_theme);
            assert_eq!(metrics.row_role, GeometryRole::RowSlot);
            assert_eq!(metrics.section_role, GeometryRole::SectionSlot);
            assert!(!metrics.row_role.comparable_to(metrics.section_role));
        }
    }

    #[test]
    fn selected_paint_is_component_derived() {
        let (design, menu_theme) = defaults();
        for mode in ALL_LIST_PRESENTATION_MODES {
            let metrics = resolved_list_presentation_metrics(*mode, design, menu_theme);
            let SelectedPaintDerivation::ComponentResolved {
                component,
                resolver,
                token_family,
            } = metrics.selected_paint_derivation;
            assert!(!component.is_empty());
            assert!(!resolver.is_empty());
            assert!(!token_family.is_empty());
        }
    }

    #[test]
    fn specialized_rows_do_not_enter_general_resolver() {
        // A specialized density is a typed record with owner + rationale —
        // it is not one of the four standard modes and cannot be produced by
        // the general resolver.
        let specialized = SpecializedListPresentationMetrics {
            owner: "notes-browse card rows",
            row_height: 72.0,
            rationale: "two-line preview card; not a general list row",
        };
        let (design, menu_theme) = defaults();
        for mode in ALL_LIST_PRESENTATION_MODES {
            let metrics = resolved_list_presentation_metrics(*mode, design, menu_theme);
            assert_ne!(
                metrics.row_slot_height, specialized.row_height,
                "{mode:?} must not absorb the specialized 72px card"
            );
        }
        assert!(!specialized.rationale.is_empty());
    }

    /// The predictive helper's signature IS the lock: it requires a resolved
    /// metrics object. This doc-test proves the compiler rejects a mode-less
    /// call shape (no overload accepts a bare row count).
    ///
    /// ```compile_fail
    /// // A predictive caller cannot compute content height from a count
    /// // alone — there is no mode-less overload.
    /// let _ = script_kit_gpui::list_item::metrics::resolved_list_content_height(
    ///     6,
    ///     std::iter::empty(),
    /// );
    /// ```
    #[test]
    fn modeless_predictive_api_is_absent() {
        let (design, menu_theme) = defaults();
        let metrics = resolved_list_presentation_metrics(
            ListPresentationMode::MainMenuThemed,
            design,
            menu_theme,
        );
        let height = resolved_list_content_height(
            &metrics,
            [
                PredictiveListSlot::FirstSection,
                PredictiveListSlot::Row,
                PredictiveListSlot::Row,
            ],
        );
        assert_eq!(
            height,
            metrics.first_section_slot_height + 2.0 * metrics.row_slot_height
        );
    }

    #[test]
    fn themed_prediction_cannot_use_legacy_constant() {
        let (design, menu_theme) = defaults();
        let themed = resolved_list_presentation_metrics(
            ListPresentationMode::MainMenuThemed,
            design,
            menu_theme,
        );
        let legacy = resolved_list_presentation_metrics(
            ListPresentationMode::LegacyCompatibility,
            design,
            menu_theme,
        );
        // The themed row comes from the theme def, the legacy row from the
        // constant; with current defs these are distinct values and distinct
        // sources. A themed prediction can therefore never silently be the
        // legacy constant.
        assert_eq!(legacy.row_slot_height, crate::list_item::LIST_ITEM_HEIGHT);
        assert_ne!(themed.row_slot_height, legacy.row_slot_height);
        assert_ne!(themed.row_metric_source, legacy.row_metric_source);
        assert!(!themed.row_metric_source.contains("LIST_ITEM_HEIGHT"));
    }
}
