const LAYOUT_SOURCE: &str = include_str!("../src/app_layout/build_layout_info.rs");
const ELEMENTS_SOURCE: &str = include_str!("../src/app_layout/collect_elements.rs");
const TRIGGER_REGISTRY_SOURCE: &str = include_str!("../src/builtins/trigger_registry.rs");
const ROUTES_SOURCE: &str = include_str!("../src/app_impl/routes.rs");
const TRIGGER_DISPATCH_SOURCE: &str = include_str!("../src/app_impl/trigger_builtin_dispatch.rs");

fn generic_filterable_node_source(name: &str) -> &'static str {
    let start = LAYOUT_SOURCE
        .find(&format!("LayoutComponentInfo::new(\"{name}\""))
        .unwrap_or_else(|| panic!("{name} layout node should exist"));
    let node_source = &LAYOUT_SOURCE[start..];
    let end = node_source
        .find(".with_visual_token")
        .unwrap_or_else(|| panic!("{name} should declare visual metadata"));
    &node_source[..end]
}

#[test]
fn generic_filterable_list_uses_liquid_glass_panel_radius() {
    let source = generic_filterable_node_source("GenericFilterableList");
    assert!(
        source.contains("LayoutComponentType::List"),
        "GenericFilterableList must remain a List node"
    );
    assert!(
        source.contains("Some(chrome_tokens::LIQUID_GLASS_PANEL_RADIUS_PX)"),
        "GenericFilterableList must use the shared Liquid Glass panel radius token"
    );
    assert!(
        !source.contains("Some(0.0)") && !source.contains("None"),
        "GenericFilterableList must not satisfy guideline proof with a zero or missing radius"
    );
}

#[test]
fn generic_filterable_layout_uses_dedicated_full_width_nodes() {
    for needle in [
        "GenericFilterableSurface",
        "GenericFilterableHeader",
        "GenericFilterableSearch",
        "GenericFilterableCount",
        "GenericFilterableDivider",
        "GenericFilterableList",
        "GenericFilterableRow",
        "GenericFilterableEmptyState",
        "GenericFilterableFooter",
        "full-width list surface",
        "no preview panel",
    ] {
        assert!(
            LAYOUT_SOURCE.contains(needle),
            "GenericFilterable layout receipt is missing `{needle}`"
        );
    }
}

#[test]
fn generic_filterable_elements_do_not_fall_back_to_current_view_panel() {
    for needle in [
        "AppView::SearchAiPresetsView",
        "AppView::FavoritesBrowseView",
        "collect_generic_filterable_rows",
        "\"ai-presets-filter\"",
        "\"favorites-filter\"",
        "\"ai-presets\"",
        "\"favorites\"",
        "\"ai-presets-empty\"",
        "\"favorites-empty\"",
    ] {
        assert!(
            ELEMENTS_SOURCE.contains(needle),
            "GenericFilterable element collection is missing `{needle}`"
        );
    }
}

#[test]
fn generic_filterable_variants_have_deterministic_trigger_builtin_routes() {
    for needle in [
        "TriggerBuiltin::Favorites",
        "TriggerBuiltin::SearchAiPresets",
        "\"builtin/favorites\"",
        "\"builtin/search-ai-presets\"",
    ] {
        assert!(
            TRIGGER_REGISTRY_SOURCE.contains(needle),
            "triggerBuiltin registry is missing `{needle}`"
        );
    }

    for needle in [
        "FilterableView::Favorites",
        "FilterableView::SearchAiPresets",
    ] {
        assert!(
            ROUTES_SOURCE.contains(needle),
            "triggerBuiltin route planner is missing `{needle}`"
        );
    }

    for needle in [
        "AppView::FavoritesBrowseView",
        "AppView::SearchAiPresetsView",
    ] {
        assert!(
            TRIGGER_DISPATCH_SOURCE.contains(needle),
            "triggerBuiltin dispatcher is missing `{needle}`"
        );
    }
}
