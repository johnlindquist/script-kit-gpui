fn append_settings_and_day_design_tokens(
    b: &mut BundleBuilder,
    theme: &Theme,
    def: MainMenuThemeDef,
    chrome: &AppChromeColors,
    _metrics: &ListItemMetricsOverride,
    fm: crate::designs::FooterMetricsTokens,
    default_spacing: crate::designs::DesignSpacing,
) {
    // ── Settings hub (built-in list surface) ────────────────────────────
    // Settings shares themed main-view sections and the common count-label
    // owner. Its rows are structurally iconless; no parser fallback is involved.
    let settings_layout =
        crate::settings_hub_contract::resolved_settings_hub_layout_for(default_spacing, def);
    let count_label_style =
        crate::builtin_main_input_contract::resolved_builtin_main_input_count_label_style(
            def, chrome,
        );
    let settings_facts_fresh = crate::settings_hub_contract::settings_hub_contract_facts(false);
    let settings_facts_custom = crate::settings_hub_contract::settings_hub_contract_facts(true);
    debug_assert_eq!(settings_layout.list_padding_y, default_spacing.padding_xs);
    debug_assert_eq!(count_label_style.inset_right, def.search.text_inset_x);
    debug_assert_eq!(count_label_style.text_rgba, chrome.text_hint_rgba);

    b.source_len(
        "design.spacing.paddingXs",
        "--sk-spacing-padding-xs",
        settings_layout.list_padding_y,
        "DesignSpacing.padding_xs (Default variant; render_settings maps its content padding-block here via resolved_settings_hub_layout)",
    );
    b.add(
        "resolved.builtinMainInput.countLabel.fontSize",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-font-size"),
        TokenValue::Length {
            value: count_label_style.font_size_px as f64,
        },
        None,
        false,
        &["gpui Styled::text_sm() rems(0.875) × 16px rem (render_builtin_main_input_count_label)"],
    );
    b.add(
        "resolved.builtinMainInput.countLabel.lineHeight",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-line-height"),
        TokenValue::Length {
            value: count_label_style.line_height_px as f64,
        },
        None,
        false,
        &["gpui TextStyle default phi() line height, rounded (14 → 23)"],
    );
    b.add(
        "resolved.builtinMainInput.countLabel.fontWeight",
        TokenStage::Resolved,
        Some("--sk-builtin-main-input-count-font-weight"),
        TokenValue::FontWeight {
            value: count_label_style.font_weight.0 as f64,
        },
        None,
        false,
        &["gpui::FontWeight::NORMAL — the count helper sets no weight; it must not inherit the search body's 430"],
    );

    // JSON-only settings facts (text/number records; no CSS role, never
    // writable through the design-token reverse path).
    for (id, value, path) in [
        (
            "settingsHub.section.emptyFilterLabel",
            settings_facts_fresh.empty_filter_section_label.to_string(),
            "settings_hub_contract::SETTINGS_HUB_EMPTY_FILTER_SECTION_LABEL (persistent leading separator, empty filter)",
        ),
        (
            "settingsHub.section.filteredLabel",
            settings_facts_fresh.filtered_section_label.to_string(),
            "settings_hub_contract::SETTINGS_HUB_FILTERED_SECTION_LABEL (persistent leading separator, active filter)",
        ),
        (
            "settingsHub.countLabel.counts",
            "visibleFilteredRows".to_string(),
            "render_settings item_count = filtered_settings_items(items, filter).len()",
        ),
        (
            "settingsHub.countLabel.pluralization",
            format!(
                "{} / {}",
                crate::settings_hub_contract::format_settings_count_label(1),
                crate::settings_hub_contract::format_settings_count_label(2),
            ),
            "settings_hub_contract::format_settings_count_label",
        ),
        (
            "settingsHub.census.optionalRow",
            crate::settings_hub_contract::SETTINGS_HUB_OPTIONAL_ROW_NAME.to_string(),
            "get_settings_items_for(has_custom_positions) conditional push",
        ),
        (
            "settingsHub.census.optionalPredicate",
            "windowState.hasCustomPositions".to_string(),
            "crate::window_state::has_custom_positions via the bin-side get_settings_items wrapper",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text { value },
            Some(path),
            false,
            &[],
        );
    }
    for (id, value, path) in [
        (
            "settingsHub.census.baseCount",
            settings_facts_fresh.row_count as f64,
            "settings_hub_contract_facts(false).row_count",
        ),
        (
            "settingsHub.census.customPositionsCount",
            settings_facts_custom.row_count as f64,
            "settings_hub_contract_facts(true).row_count",
        ),
        (
            "settingsHub.icons.resolvedRowIconCount",
            settings_facts_custom.resolved_icon_rows as f64,
            "SettingsRowIconPolicy::Iconless (SettingsItem has no icon field)",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Number { value },
            Some(path),
            false,
            &[],
        );
    }

    let icon_policy = match settings_facts_fresh.icon_policy {
        crate::settings_hub_contract::SettingsRowIconPolicy::Iconless => "iconless",
    };
    b.add("settingsHub.icons.policy", TokenStage::Resolved, None,
        TokenValue::Text { value: icon_policy.into() },
        Some("settings_hub_contract::SETTINGS_ROW_ICON_POLICY"), false, &[]);
    let items = crate::settings_hub_contract::get_settings_items_for(false);
    if let Some(action) = crate::settings_hub_contract::selected_settings_action_descriptor(
        &items, "", 0, crate::settings_hub_contract::SettingsActionAvailability::all_available(),
    ) {
        for (id, value) in [
            ("settingsHub.footer.primaryActionId", action.action_id.as_str()),
            ("settingsHub.footer.primaryLabel", action.primary_verb),
        ] {
            b.add(id, TokenStage::Resolved, None, TokenValue::Text { value: value.into() },
                Some("settings_hub_contract::selected_settings_action_descriptor(items, empty_filter, 0, all_available)"),
                false, &[]);
        }
        b.add("settingsHub.footer.primaryEnabled", TokenStage::Resolved, None,
            TokenValue::Text { value: action.enabled.to_string() },
            Some("SettingsActionDescriptor.enabled"), false, &[]);
    }

    // ── Shared main-view / component-theme owners (Day Page slice) ──────
    // Per the 2026-07-11 Oracle review: the Day Page mints NO editor, link,
    // caret, color, or footer tokens — it consumes shared owners. These
    // records are the shared side; the Day-owned geometry follows below.
    let columns = crate::components::main_view_chrome::main_view_content_columns(def);
    b.add(
        "resolved.mainView.contentRightInsetX",
        TokenStage::Resolved,
        Some("--sk-main-view-content-right-inset-x"),
        TokenValue::Length {
            value: columns.content_right_inset_x as f64,
        },
        None,
        false,
        &["main_view_content_columns(def).content_right_inset_x = shell.header_padding_x"],
    );
    // gpui-component theme colors every cx.theme() consumer paints with
    // (Day shelf toggle rest/hover, compact resource row rest/hover, …).
    let bridge_theme =
        crate::theme::gpui_integration::map_scriptkit_to_gpui_theme(theme, theme.is_dark_mode());
    b.add(
        "resolved.componentTheme.mutedForeground",
        TokenStage::Resolved,
        Some("--sk-component-theme-muted-foreground"),
        hsla_color_value(bridge_theme.muted_foreground),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "theme.opacity.textPlaceholder",
            "map_scriptkit_to_gpui_theme → theme_color.muted_foreground",
        ],
    );
    b.add(
        "resolved.componentTheme.foreground",
        TokenStage::Resolved,
        Some("--sk-component-theme-foreground"),
        hsla_color_value(bridge_theme.foreground),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "map_scriptkit_to_gpui_theme → theme_color.foreground",
        ],
    );
    // Shared compact resource row (render_compact_resource_row) — the Day
    // shelf's expanded rows and any future kit:// resource lists share it.
    let compact_row =
        crate::components::resource_preview::resolved_compact_resource_row_style(theme);
    debug_assert_eq!(compact_row.rest_color, bridge_theme.muted_foreground);
    debug_assert_eq!(compact_row.hover_color, bridge_theme.foreground);
    b.source_len(
        "resourcePreview.compactRow.paddingX",
        "--sk-compact-resource-row-padding-x",
        compact_row.padding_x,
        "components::INFO_SPACING.xs",
    );
    b.source_len(
        "resourcePreview.compactRow.paddingY",
        "--sk-compact-resource-row-padding-y",
        compact_row.padding_y,
        "components::INFO_SPACING.xxs",
    );
    b.add(
        "resolved.resourcePreview.compactRow.gap",
        TokenStage::Resolved,
        Some("--sk-compact-resource-row-gap"),
        TokenValue::Length {
            value: compact_row.gap as f64,
        },
        None,
        false,
        &["gpui Styled::gap_2 (0.5rem × 16px rem) — resource_preview mirror tripwire"],
    );
    // Framework text helpers (gpui `Styled`, rem-relative, no accessor):
    // one shared resolved token each, consumed by the shelf toggle AND the
    // compact row instead of per-surface copies.
    b.add(
        "resolved.framework.textXsFontSize",
        TokenStage::Resolved,
        Some("--sk-framework-text-xs-font-size"),
        TokenValue::Length {
            value: compact_row.font_size as f64,
        },
        None,
        false,
        &["gpui Styled::text_xs (0.75rem × 16px rem) — resource_preview mirror tripwire"],
    );
    b.add(
        "resolved.framework.gap1",
        TokenStage::Resolved,
        Some("--sk-framework-gap-1"),
        TokenValue::Length { value: 4.0 },
        None,
        false,
        &["gpui Styled::gap_1 (0.25rem × 16px rem) — Day shelf toggle glyph/label gap"],
    );

    // ── Day Page (Today view, main window) ──────────────────────────────
    // Anatomy: shared main-view chrome; context-only header = inert context
    // row (30 total, with no phantom input/gap); shared NotesEditor markdown input (adopted
    // 16/12 wrapper + Size::Medium 12/8 → text origin x=30, first text top
    // y=50; mono 16 in the 20px line box); clipboard shelf accessory
    // (6 + 20 [+ 4 + 24·n expanded] + 12); GPUI footer band = empty 32pt
    // rail spacer (the native overlay owns the buttons). Day Page owns ONLY
    // the shelf/accessory geometry below — everything else resolves through
    // the shared owners above.
    b.source_len(
        "dayPage.editor.minHeight",
        "--sk-day-page-editor-min-height",
        crate::day_page::layout::DAY_PAGE_MIN_EDITOR_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_MIN_EDITOR_HEIGHT_PX",
    );
    b.source_len(
        "dayPage.shelf.topPadding",
        "--sk-day-page-shelf-top-padding",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX",
    );
    b.source_len(
        "dayPage.shelf.toggleHeight",
        "--sk-day-page-shelf-toggle-height",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX",
    );
    // Toggle ↔ expanded-list gap. NOT the toggle's inline glyph/label gap
    // (that is the framework .gap_1 — a different authority, also 4 today).
    b.source_len(
        "dayPage.shelf.expandedListGap",
        "--sk-day-page-shelf-expanded-list-gap",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_GAP_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_GAP_PX",
    );
    // The Day renderer's fixed 24px row wrapper (the compact resource row
    // renders inside this slot).
    b.source_len(
        "dayPage.shelf.rowSlotHeight",
        "--sk-day-page-shelf-row-slot-height",
        crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX,
        "day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX",
    );
    // Authored responsive cap — layout source, deliberately NO CSS variable.
    b.add(
        "dayPage.shelf.maxBodyFraction",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: crate::day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION as f64,
        },
        Some("day_page::layout::DAY_PAGE_CLIPBOARD_SHELF_MAX_BODY_FRACTION"),
        true,
        &[],
    );

    // JSON-only Day Page contract facts (markup/behavior, not tokens).
    let day_spine =
        crate::components::notes_editor::spine::NotesEditorHostSpineContract::day_page();
    let day_spine_overlay = match day_spine.local_overlay {
        crate::components::notes_editor::spine::NotesEditorLocalSpineOverlay::Disabled => {
            "disabled"
        }
        crate::components::notes_editor::spine::NotesEditorLocalSpineOverlay::Overlay {
            ..
        } => "overlay",
    };
    let day_spine_mentions = match day_spine.context_mentions {
        crate::components::notes_editor::spine::NotesEditorContextMentionBehavior::MainMenuRoundTrip => {
            "mainMenuRoundTrip"
        }
        crate::components::notes_editor::spine::NotesEditorContextMentionBehavior::LocalPicker => {
            "localPicker"
        }
    };
    for (id, value, path) in [
        (
            "dayPage.header.contextInteraction",
            "inert",
            "render_inert_main_view_context_zone (src/app_impl/ui_window.rs) — same chips, no-op handlers, NO keycaps",
        ),
        (
            "dayPage.header.inputSlot",
            "none",
            "DayPageView::render — MainViewHeaderChrome::context_only",
        ),
        (
            "dayPage.header.dividerVisible",
            "false",
            "DayPageView::render — MainViewDividerChrome { visible: false }",
        ),
        (
            "dayPage.editor.spine.localOverlay",
            day_spine_overlay,
            "NotesEditorHostSpineContract::day_page().local_overlay",
        ),
        (
            "dayPage.editor.spine.contextMentions",
            day_spine_mentions,
            "NotesEditorHostSpineContract::day_page().context_mentions",
        ),
        (
            "dayPage.shelf.defaultExpanded",
            "false",
            "DayPageView::new — clipboard_shelf_expanded: false (collapsed is the shipped rest state)",
        ),
        (
            "dayPage.shelf.hiddenWhenEmpty",
            "true",
            "DayPageView::render_clipboard_shelf — returns None when clipboard_shelf is empty",
        ),
        (
            "dayPage.shelf.hiddenDuringKitPreview",
            "true",
            "DayPageView::render_clipboard_shelf — returns None while kit_resource_preview is open",
        ),
        (
            "dayPage.shelf.sourceLines",
            "liftedFromEditor",
            "adopt_clipboard_shelf_from / day_page::split_day_page_clipboard_shelf (rejoined on save)",
        ),
        (
            "dayPage.footer.presentation",
            "gpuiSpacerPlusNativeOverlay",
            "render_native_main_window_footer_spacer + native AppKit overlay (day_page_footer_buttons)",
        ),
        (
            "dayPage.footer.defaultAction",
            "actions",
            "day_page_footer_buttons — plain Day Page paints a single Actions ⌘K native button",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Text {
                value: value.to_string(),
            },
            Some(path),
            false,
            &[],
        );
    }

    // ── Day Page conflicts (recorded, not collapsed) ─────────────────────
    b.conflict(
        "dayPageFooter.spacerVsNativeHostBand",
        &[
            (
                "GPUI Day Page footer spacer (footer.railHeight)",
                format!("{}", fm.height_px),
            ),
            (
                "native footer HOST band (window.nativeFooterHostHeight)",
                format!(
                    "{}",
                    crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                ),
            ),
        ],
        "warning",
        "The Day Page GPUI layer reserves the 32px footer rail \
         (render_native_main_window_footer_spacer = current_main_menu_footer_height) \
         while the native AppKit footer HOST band is modeled at 36px. The footer \
         height ladder continues — do NOT 'fix' either value in the exporter; \
         painted truth for the bottom band needs an activeFooter probe + pixel check.",
    );
}
