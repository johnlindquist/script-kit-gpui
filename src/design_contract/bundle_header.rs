fn append_header_design_tokens(
    b: &mut BundleBuilder,
    def: MainMenuThemeDef,
    metrics: &ListItemMetricsOverride,
    fill: crate::list_item::ResolvedMainMenuRowFill,
    colors: &crate::theme::ColorScheme,
) {
    // ── Header context zone (info bar) ──────────────────────────────────
    let info = def.header_info_bar;
    b.source_len(
        "mainMenu.shell.headerPaddingX",
        "--sk-main-menu-header-padding-x",
        def.shell.header_padding_x,
        "MainMenuShellTokens.header_padding_x",
    );
    b.source_len(
        "mainMenu.shell.headerPaddingY",
        "--sk-main-menu-header-padding-y",
        def.shell.header_padding_y,
        "MainMenuShellTokens.header_padding_y",
    );
    b.source_len(
        "mainMenu.shell.headerGap",
        "--sk-main-menu-header-gap",
        def.shell.header_gap,
        "MainMenuShellTokens.header_gap",
    );
    b.source_len(
        "mainMenu.shell.contentInsetX",
        "--sk-main-menu-content-inset-x",
        def.shell.content_inset_x,
        "MainMenuShellTokens.content_inset_x",
    );
    b.add(
        "mainMenu.context.fontFamily",
        TokenStage::Source,
        Some("--sk-main-menu-context-font-family"),
        TokenValue::Text {
            value: info.font_family.to_string(),
        },
        Some("HeaderInfoBarTokens.font_family"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.context.fontSize",
        "--sk-main-menu-context-font-size",
        info.font_size,
        "HeaderInfoBarTokens.font_size",
    );
    b.add(
        "mainMenu.context.opacity",
        TokenStage::Source,
        Some("--sk-main-menu-context-opacity"),
        TokenValue::Number {
            value: info.opacity as f64,
        },
        Some("HeaderInfoBarTokens.opacity"),
        true,
        &[],
    );
    b.add(
        "mainMenu.context.keyOpacity",
        TokenStage::Source,
        Some("--sk-main-menu-context-key-opacity"),
        TokenValue::Number {
            value: info.key_opacity as f64,
        },
        Some("HeaderInfoBarTokens.key_opacity"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.context.height",
        "--sk-main-menu-context-height",
        info.height_px,
        "HeaderInfoBarTokens.height_px",
    );
    b.source_len(
        "mainMenu.context.gap",
        "--sk-main-menu-context-gap",
        info.gap_px,
        "HeaderInfoBarTokens.gap_px",
    );
    b.source_len(
        "mainMenu.context.pillPaddingX",
        "--sk-main-menu-context-pill-padding-x",
        info.pill_padding_x,
        "HeaderInfoBarTokens.pill_padding_x",
    );
    b.source_len(
        "mainMenu.context.pillRadius",
        "--sk-main-menu-context-pill-radius",
        info.pill_radius,
        "HeaderInfoBarTokens.pill_radius",
    );
    b.source_len(
        "mainMenu.context.edgeOutsetX",
        "--sk-main-menu-context-edge-outset-x",
        info.context_edge_outset_x,
        "HeaderInfoBarTokens.context_edge_outset_x",
    );
    b.add(
        "resolved.mainMenu.context.keycapFontSize",
        TokenStage::Resolved,
        Some("--sk-main-menu-context-keycap-font-size"),
        TokenValue::Length {
            value: crate::components::main_view_chrome::context_zone_keycap_font_size(&info) as f64,
        },
        None,
        false,
        &["mainMenu.context.fontSize"],
    );
    b.add(
        "resolved.mainMenu.context.keycapHeight",
        TokenStage::Resolved,
        Some("--sk-main-menu-context-keycap-height"),
        TokenValue::Length {
            value: crate::components::main_view_chrome::context_zone_keycap_height(&info) as f64,
        },
        None,
        false,
        &["mainMenu.context.fontSize"],
    );
    b.add(
        "mainMenu.context.separator",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: info.separator.to_string(),
        },
        Some("HeaderInfoBarTokens.separator"),
        true,
        &[],
    );

    // ── Search input ────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.search.height",
        "--sk-main-menu-search-height",
        def.search.height,
        "MainMenuSearchTokens.height",
    );
    b.source_len(
        "mainMenu.search.textInsetX",
        "--sk-main-menu-search-text-inset-x",
        def.search.text_inset_x,
        "MainMenuSearchTokens.text_inset_x",
    );
    b.source_len(
        "mainMenu.search.fontSize",
        "--sk-main-menu-search-font-size",
        def.search.font_size,
        "MainMenuSearchTokens.font_size",
    );
    b.add(
        "mainMenu.search.fontWeight",
        TokenStage::Source,
        Some("--sk-main-menu-search-font-weight"),
        TokenValue::FontWeight {
            value: def.search.font_weight.0 as f64,
        },
        Some("MainMenuSearchTokens.font_weight"),
        true,
        &[],
    );
    b.add(
        "mainMenu.search.placeholder",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: crate::ROOT_LAUNCHER_PLACEHOLDER.to_string(),
        },
        Some("crate::ROOT_LAUNCHER_PLACEHOLDER"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.caret.width",
        "--sk-caret-width",
        crate::panel::CURSOR_WIDTH,
        "crate::panel::CURSOR_WIDTH",
    );
    b.source_len(
        "mainMenu.caret.height",
        "--sk-caret-height",
        crate::panel::CURSOR_HEIGHT_LG,
        "crate::panel::CURSOR_HEIGHT_LG",
    );

    // ── List / sections ─────────────────────────────────────────────────
    b.source_len(
        "mainMenu.list.rowHeight",
        "--sk-main-menu-row-height",
        metrics.item_height,
        "MainMenuListTokens.item_height",
    );
    b.source_len(
        "mainMenu.list.sectionSlotHeight",
        "--sk-main-menu-section-slot-height",
        metrics.section_header_height,
        "MainMenuListTokens.section_header_height",
    );
    b.source_len(
        "mainMenu.list.firstSectionSlotHeight",
        "--sk-main-menu-first-section-slot-height",
        metrics.first_section_header_height,
        "MainMenuListTokens.first_section_header_height",
    );
    b.source_len(
        "mainMenu.section.paddingX",
        "--sk-main-menu-section-padding-x",
        metrics.section_padding_x,
        "MainMenuListTokens.section_padding_x",
    );
    b.source_len(
        "mainMenu.section.paddingTop",
        "--sk-main-menu-section-padding-top",
        metrics.section_padding_top,
        "MainMenuListTokens.section_padding_top",
    );
    b.source_len(
        "mainMenu.section.firstPaddingTop",
        "--sk-main-menu-first-section-padding-top",
        metrics.first_section_padding_top,
        "ListItemMetricsOverride.first_section_padding_top",
    );
    b.source_len(
        "mainMenu.section.paddingBottom",
        "--sk-main-menu-section-padding-bottom",
        metrics.section_padding_bottom,
        "MainMenuListTokens.section_padding_bottom",
    );
    b.source_len(
        "mainMenu.section.gap",
        "--sk-main-menu-section-gap",
        metrics.section_gap,
        "MainMenuListTokens.section_gap",
    );
    b.source_len(
        "mainMenu.section.iconSize",
        "--sk-main-menu-section-icon-size",
        metrics.section_icon_size,
        "MainMenuListTokens.section_icon_size",
    );
    b.source_len(
        "mainMenu.section.fontSize",
        "--sk-main-menu-section-font-size",
        metrics.section_header_font_size,
        "MainMenuTypographyTokens.section_font_size",
    );
    b.add(
        "mainMenu.section.fontWeight",
        TokenStage::Source,
        Some("--sk-main-menu-section-font-weight"),
        TokenValue::FontWeight {
            value: metrics.section_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.section_weight"),
        true,
        &[],
    );

    // ── Row geometry + resolved fills ───────────────────────────────────
    b.source_len(
        "mainMenu.row.outerPaddingX",
        "--sk-main-menu-row-outer-padding-x",
        metrics.row_outer_padding_x,
        "MainMenuRowTokens.outer_padding_x",
    );
    b.source_len(
        "mainMenu.row.outerPaddingY",
        "--sk-main-menu-row-outer-padding-y",
        metrics.row_outer_padding_y,
        "MainMenuRowTokens.outer_padding_y",
    );
    b.source_len(
        "mainMenu.row.innerPaddingX",
        "--sk-main-menu-row-inner-padding-x",
        metrics.row_inner_padding_x,
        "MainMenuRowTokens.inner_padding_x",
    );
    b.source_len(
        "mainMenu.row.innerPaddingY",
        "--sk-main-menu-row-inner-padding-y",
        metrics.row_inner_padding_y,
        "MainMenuRowTokens.inner_padding_y",
    );
    b.source_len(
        "mainMenu.row.radius",
        "--sk-main-menu-row-radius",
        metrics.row_radius,
        "MainMenuRowTokens.radius",
    );
    b.source_len(
        "mainMenu.row.iconTextGap",
        "--sk-main-menu-row-icon-text-gap",
        metrics.icon_text_gap,
        "MainMenuRowTokens.icon_text_gap",
    );
    b.source_len(
        "mainMenu.row.nameDescGap",
        "--sk-main-menu-row-name-description-gap",
        metrics.name_desc_gap,
        "MainMenuRowTokens.name_desc_gap",
    );
    b.source_len(
        "mainMenu.row.accessoryGap",
        "--sk-main-menu-row-accessory-gap",
        metrics.accessory_gap,
        "MainMenuRowTokens.accessory_gap",
    );
    b.source_len(
        "mainMenu.row.selectedMarkerWidth",
        "--sk-main-menu-row-selected-marker-width",
        metrics.row_selected_marker_width,
        "MainMenuRowTokens.selected_marker_width",
    );
    b.source_len(
        "mainMenu.row.selectedMarkerHeight",
        "--sk-main-menu-row-selected-marker-height",
        metrics.row_selected_marker_height,
        "MainMenuRowTokens.selected_marker_height",
    );
    b.source_len(
        "mainMenu.row.selectedMarkerInsetX",
        "--sk-main-menu-row-selected-marker-inset-x",
        metrics.row_selected_marker_inset_x,
        "MainMenuRowTokens.selected_marker_inset_x",
    );
    b.source_len(
        "mainMenu.row.selectedMarkerRadius",
        "--sk-main-menu-row-selected-marker-radius",
        metrics.row_selected_marker_radius,
        "MainMenuRowTokens.selected_marker_radius",
    );
    b.add(
        "mainMenu.row.selectedMarkerOpacity",
        TokenStage::Source,
        Some("--sk-main-menu-row-selected-marker-opacity"),
        TokenValue::Number {
            value: metrics.row_selected_marker_alpha as f64 / 255.0,
        },
        Some("MainMenuRowTokens.selected_marker_alpha"),
        true,
        &[],
    );
    b.resolved_color(
        "resolved.mainMenu.row.selectedMarker",
        "--sk-main-menu-row-selected-marker",
        (colors.accent.selected << 8) | metrics.row_selected_marker_alpha,
        &[
            "theme.colors.accent.selected",
            "mainMenu.row.selectedMarkerOpacity",
        ],
    );

    let selected_fill = match fill.base {
        MainMenuRowFillBase::TextPrimary => (colors.text.primary << 8) | fill.selected_alpha as u32,
        MainMenuRowFillBase::Accent => (colors.accent.selected << 8) | fill.selected_alpha as u32,
    };
    let hover_fill = match fill.base {
        MainMenuRowFillBase::TextPrimary => (colors.text.primary << 8) | fill.hover_alpha as u32,
        MainMenuRowFillBase::Accent => (colors.accent.selected << 8) | fill.hover_alpha as u32,
    };
    b.resolved_color(
        "resolved.mainMenu.row.selectedBackground",
        "--sk-main-menu-row-selected-background",
        selected_fill,
        &[
            "theme.colors.text.primary",
            "mainMenu.row.selectedFillAlpha",
        ],
    );
    b.resolved_color(
        "resolved.mainMenu.row.hoverBackground",
        "--sk-main-menu-row-hover-background",
        hover_fill,
        &[
            "theme.colors.text.primary",
            "theme.opacity.hover",
            "mainMenu.row.hoverFillAlpha",
        ],
    );

    // ── Icon tile ───────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.icon.containerSize",
        "--sk-main-menu-icon-container-size",
        metrics.icon_container_size,
        "MainMenuIconTokens.container_size",
    );
    b.source_len(
        "mainMenu.icon.svgSize",
        "--sk-main-menu-icon-svg-size",
        metrics.icon_svg_size,
        "MainMenuIconTokens.svg_size",
    );
    b.source_len(
        "mainMenu.icon.tileSize",
        "--sk-main-menu-icon-tile-size",
        metrics.icon_tile_size,
        "MainMenuIconTokens.tile_size",
    );
    b.add(
        "resolved.mainMenu.icon.tileRadius",
        TokenStage::Resolved,
        Some("--sk-main-menu-icon-tile-radius"),
        TokenValue::Length {
            value: fill.icon_tile_radius as f64,
        },
        None,
        false,
        &["MainMenuIconTokens.tile_radius"],
    );
    b.resolved_color(
        "resolved.mainMenu.icon.tileBackground",
        "--sk-main-menu-icon-tile-background",
        (colors.accent.selected << 8) | fill.icon_tile_alpha,
        &[
            "theme.colors.accent.selected",
            "MainMenuIconTokens.tile_fill_alpha",
        ],
    );

    // ── Typography ──────────────────────────────────────────────────────
    b.source_len(
        "mainMenu.type.nameFontSize",
        "--sk-main-menu-name-font-size",
        metrics.name_font_size,
        "MainMenuTypographyTokens.name_font_size",
    );
    b.source_len(
        "mainMenu.type.nameLineHeight",
        "--sk-main-menu-name-line-height",
        metrics.name_line_height,
        "MainMenuTypographyTokens.name_line_height",
    );
    b.add(
        "mainMenu.type.nameWeight",
        TokenStage::Source,
        Some("--sk-main-menu-name-font-weight"),
        TokenValue::FontWeight {
            value: metrics.name_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.name_weight"),
        true,
        &[],
    );
    b.add(
        "mainMenu.type.selectedNameWeight",
        TokenStage::Source,
        Some("--sk-main-menu-selected-name-font-weight"),
        TokenValue::FontWeight {
            value: metrics.selected_name_weight.0 as f64,
        },
        Some("MainMenuTypographyTokens.selected_name_weight"),
        true,
        &[],
    );
    b.source_len(
        "mainMenu.type.descFontSize",
        "--sk-main-menu-description-font-size",
        metrics.desc_font_size,
        "MainMenuTypographyTokens.desc_font_size",
    );
    b.source_len(
        "mainMenu.type.descLineHeight",
        "--sk-main-menu-description-line-height",
        metrics.desc_line_height,
        "MainMenuTypographyTokens.desc_line_height",
    );
    b.add(
        "mainMenu.type.uiFontFamily",
        TokenStage::Source,
        Some("--sk-font-ui"),
        TokenValue::Text {
            value: crate::list_item::FONT_SYSTEM_UI.to_string(),
        },
        Some("crate::list_item::FONT_SYSTEM_UI"),
        true,
        &[],
    );
    b.add(
        "mainMenu.type.monoFontFamily",
        TokenStage::Source,
        Some("--sk-font-mono"),
        TokenValue::Text {
            value: crate::list_item::FONT_MONO.to_string(),
        },
        Some("crate::list_item::FONT_MONO"),
        true,
        &[],
    );
}
