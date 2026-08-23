fn append_notes_design_tokens(
    b: &mut BundleBuilder,
    theme: &Theme,
    fm: crate::designs::FooterMetricsTokens,
) {
    // ── Notes window (separate NSPanel) ─────────────────────────────────
    // App-authored chrome + the layout model come from the production
    // contract (`notes::window::contract`) — the SAME typed source
    // window_ops, the titlebar renderer, and autosize consume (and
    // explicitly NOT the feature-sensitive `adopted_style()`). Editor
    // typography/caret resolve through the notes-editor contract (the
    // theme → gpui-component bridge, NOT `FontConfig::default()`), the
    // painted footer band through the shared footer_chrome formula owner,
    // and markdown capture styles through the real highlight-theme resolver
    // beside `register_markdown_highlighter`.
    let notes_chrome = crate::notes::window::contract::production_notes_window_contract();
    let notes_layout = crate::notes::window::contract::production_notes_layout_model();
    let notes_editor =
        crate::components::notes_editor::contract::resolved_notes_editor_metrics(&theme);
    let notes_markdown =
        crate::notes::markdown_highlighting::resolved_notes_markdown_styles(&theme, true);
    let notes_markdown_runtime =
        crate::notes::markdown_highlighting::markdown_editor_runtime_info();
    let notes_footer_intrinsic =
        crate::notes::window::contract::resolved_notes_footer_intrinsic_height(fm.button_padding_y);

    // Source: app-authored Notes chrome (writable leaves).
    for (id, var, value, path) in [
        (
            "notes.window.defaultWidth",
            "--sk-notes-window-width",
            notes_chrome.default_width,
            "notes::window::contract::NOTES_DEFAULT_WIDTH",
        ),
        (
            "notes.window.defaultHeight",
            "--sk-notes-window-height",
            notes_chrome.default_height,
            "notes::window::contract::NOTES_DEFAULT_HEIGHT",
        ),
        (
            "notes.titlebar.height",
            "--sk-notes-titlebar-height",
            notes_chrome.titlebar_height,
            "NotesWindowStyle::current().titlebar_height",
        ),
        (
            "notes.titlebar.paddingX",
            "--sk-notes-titlebar-padding-x",
            notes_chrome.titlebar_padding_x,
            "notes::window::contract::NOTES_TITLEBAR_PADDING_X",
        ),
        (
            "notes.titlebar.leadingReserveWidth",
            "--sk-notes-titlebar-traffic-width",
            notes_chrome.titlebar_leading_reserve_width,
            "notes::window::TITLEBAR_TRAFFIC_LIGHT_W",
        ),
        (
            "notes.titlebar.trailingReserveWidth",
            "--sk-notes-titlebar-icons-width",
            notes_chrome.titlebar_trailing_reserve_width,
            "notes::window::TITLEBAR_ICONS_W",
        ),
        (
            "notes.titlebar.trafficLightOriginX",
            "--sk-notes-traffic-x",
            notes_chrome.traffic_light_origin_x,
            "notes::window::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_X",
        ),
        (
            "notes.titlebar.trafficLightOriginY",
            "--sk-notes-traffic-y",
            notes_chrome.traffic_light_origin_y,
            "notes::window::contract::NOTES_TRAFFIC_LIGHT_ORIGIN_Y",
        ),
        (
            "notes.editor.paddingX",
            "--sk-notes-editor-padding-x",
            notes_chrome.editor_padding_x,
            "NotesWindowStyle::current().editor_padding_x",
        ),
        (
            "notes.editor.paddingY",
            "--sk-notes-editor-padding-y",
            notes_chrome.editor_padding_y,
            "NotesWindowStyle::current().editor_padding_y",
        ),
        (
            "notes.footer.statusMinWidth",
            "--sk-notes-footer-status-min-width",
            notes_chrome.footer_status_min_width,
            "notes::window::MIN_TARGET_SIZE",
        ),
        (
            "notes.footer.contentInsetX",
            "--sk-notes-footer-content-inset-x",
            notes_chrome.footer_content_inset_x,
            "crate::window_resize::main_layout::HINT_STRIP_PADDING_X",
        ),
        (
            "notes.footer.actionGap",
            "--sk-notes-footer-action-gap",
            crate::components::footer_chrome::FOOTER_ACTION_ITEM_GAP_PX,
            "footer_chrome::FOOTER_ACTION_ITEM_GAP_PX",
        ),
    ] {
        b.source_len(id, var, value, path);
    }
    b.add(
        "notes.window.defaultEdgePadding",
        TokenStage::Source,
        None,
        TokenValue::Length {
            value: notes_chrome.default_edge_padding as f64,
        },
        Some("notes::window::contract::NOTES_DEFAULT_EDGE_PADDING"),
        true,
        &[],
    );
    b.add(
        "notes.titlebar.titleRestOpacity",
        TokenStage::Source,
        Some("--sk-notes-titlebar-title-rest-opacity"),
        TokenValue::Number {
            value: notes_chrome.title_rest_opacity as f64,
        },
        Some("notes::window::OPACITY_MUTED"),
        true,
        &[],
    );
    b.add(
        "notes.footer.restOpacity",
        TokenStage::Source,
        Some("--sk-notes-footer-rest-opacity"),
        TokenValue::Number {
            value: notes_chrome.footer_rest_opacity as f64,
        },
        Some("notes::window::OPACITY_SUBTLE"),
        true,
        &[],
    );

    // Layout MODEL (autosize + automation_layout_info reservation), under
    // honest model names — NOT painted geometry. The 28px footer
    // reservation deliberately stays 28 (see the conflict below).
    b.add(
        "notes.layout.footerReservationHeight",
        TokenStage::Source,
        None,
        TokenValue::Length {
            value: notes_layout.footer_reservation_height as f64,
        },
        Some("NotesLayoutMetrics::footer_height (autosize + automation_layout_info)"),
        true,
        &[],
    );
    for (id, value, path) in [
        (
            "notes.layout.autoResize.maxHeight",
            notes_layout.auto_resize_max_height as f64,
            "NotesLayoutMetrics::auto_resize_max_height",
        ),
        (
            "notes.layout.autoResize.assumedLineHeight",
            notes_layout.auto_resize_assumed_line_height as f64,
            "NotesLayoutMetrics::auto_resize_line_height — an autosize ASSUMPTION, not the Input's painted line box",
        ),
        (
            "notes.layout.autoResize.applyThreshold",
            notes_layout.auto_resize_threshold as f64,
            "NotesLayoutMetrics::auto_resize_threshold",
        ),
    ] {
        b.add(
            id,
            TokenStage::Source,
            None,
            TokenValue::Length { value },
            Some(path),
            false,
            &[],
        );
    }

    // Contract facts (JSON-only text records; not visual numbers).
    for (id, value, path) in [
        (
            "notes.footer.presentation",
            crate::notes::window::contract::NOTES_FOOTER_PRESENTATION.to_string(),
            "notes::window::contract::NOTES_FOOTER_PRESENTATION",
        ),
        (
            "notes.footer.nativeOverlay",
            crate::notes::window::contract::NOTES_FOOTER_NATIVE_OVERLAY.to_string(),
            "notes::window::contract::NOTES_FOOTER_NATIVE_OVERLAY",
        ),
        (
            "notes.footer.visibility",
            crate::notes::window::contract::NOTES_FOOTER_VISIBILITY.to_string(),
            "notes::window::contract::NOTES_FOOTER_VISIBILITY",
        ),
        (
            "notes.editor.markdown.language",
            notes_markdown_runtime.language.clone(),
            "notes::markdown_highlighting::MARKDOWN_LANGUAGE",
        ),
        (
            "notes.editor.markdown.highlightQueryFingerprint",
            notes_markdown_runtime.highlight_query_fingerprint.clone(),
            "markdown_editor_runtime_info().highlight_query_fingerprint",
        ),
        (
            "notes.editor.markdown.injectionQueryFingerprint",
            notes_markdown_runtime.injection_query_fingerprint.clone(),
            "markdown_editor_runtime_info().injection_query_fingerprint",
        ),
        (
            "notes.editor.markdown.inlineHighlightQueryFingerprint",
            notes_markdown_runtime
                .inline_highlight_query_fingerprint
                .clone(),
            "markdown_editor_runtime_info().inline_highlight_query_fingerprint",
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

    // Resolved: what the Notes window actually paints (never writable).
    for (id, var, value, derived) in [
        (
            "resolved.notes.editor.baseFontSize",
            "--sk-notes-editor-font-size",
            notes_editor.base_font_size,
            "theme.fonts.monoSize via sync_gpui_component_theme → Input::text_size",
        ),
        (
            "resolved.notes.editor.lineBoxHeight",
            "--sk-notes-editor-line-box-height",
            notes_editor.line_box_height,
            "gpui-component Input line_height Rems(1.25) × 16px rem",
        ),
        (
            "resolved.notes.editor.caretWidth",
            "--sk-notes-caret-width",
            notes_editor.caret_width,
            "gpui-component blink_cursor::CURSOR_WIDTH",
        ),
        (
            "resolved.notes.editor.caretHeight",
            "--sk-notes-caret-height",
            notes_editor.caret_height,
            "resolved.notes.editor.lineBoxHeight × 0.85 (Size::Medium)",
        ),
        (
            "resolved.notes.editor.inputPaddingX",
            "--sk-notes-editor-input-padding-x",
            notes_editor.input_padding_x,
            "gpui_component::Size::Medium.input_px() — the REAL vendored accessor, not a copy",
        ),
        (
            "resolved.notes.editor.inputPaddingY",
            "--sk-notes-editor-input-padding-y",
            notes_editor.input_padding_y,
            "gpui_component::Size::Medium.input_py() — the REAL vendored accessor, not a copy",
        ),
        (
            "resolved.notes.footer.intrinsicHeight",
            "--sk-notes-footer-height",
            notes_footer_intrinsic,
            "footer_chrome::footer_button_height_in(HINT_STRIP_HEIGHT, footer.buttonPaddingY)",
        ),
        (
            "resolved.notes.titlebar.titleFontSize",
            "--sk-notes-titlebar-title-font-size",
            14.0,
            "gpui text_sm (0.875rem × 16px rem) in render_editor_titlebar",
        ),
        (
            "resolved.notes.footer.statusFontSize",
            "--sk-notes-footer-status-font-size",
            12.0,
            "gpui text_xs (0.75rem × 16px rem) in render_editor_footer",
        ),
    ] {
        b.add(
            id,
            TokenStage::Resolved,
            Some(var),
            TokenValue::Length {
                value: value as f64,
            },
            None,
            false,
            &[derived],
        );
    }
    // CSS-exposed since the Day Page slice: the editor's family is the theme
    // bridge's mono family, NOT list_item::FONT_MONO (--sk-font-mono). Both
    // say "JetBrains Mono" today, but the authorities differ.
    b.add(
        "resolved.notes.editor.fontFamily",
        TokenStage::Resolved,
        Some("--sk-notes-editor-font-family"),
        TokenValue::Text {
            value: notes_editor.base_font_family.clone(),
        },
        None,
        false,
        &["theme.fonts.monoFamily via sync_gpui_component_theme"],
    );

    // Resolved editor/link colors, read from the SAME theme bridge
    // (map_scriptkit_to_gpui_theme) the renderer's cx.theme() carries. The
    // link label accent and the markdown TITLE color are separate
    // authorities that happen to both be amber in the stock theme.
    b.add(
        "resolved.notes.editor.textColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-text-color"),
        hsla_color_value(notes_editor.text_color),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "window text_style — host roots install .text_color(text.primary)",
        ],
    );
    b.add(
        "resolved.notes.editor.caretColor",
        TokenStage::Resolved,
        Some("--sk-notes-caret-color"),
        hsla_color_value(notes_editor.caret_color),
        None,
        false,
        &[
            "theme.colors.text.primary",
            "map_scriptkit_to_gpui_theme → theme_color.caret (no focused-cursor override in script-kit-dark)",
        ],
    );
    b.add(
        "resolved.notes.editor.linkLabelColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-link-label"),
        hsla_color_value(notes_editor.link_label_color),
        None,
        false,
        &[
            "theme.colors.accent.selected",
            "map_scriptkit_to_gpui_theme → theme_color.accent (markdown link highlighter)",
        ],
    );
    b.add(
        "resolved.notes.editor.linkDestinationRestColor",
        TokenStage::Resolved,
        Some("--sk-notes-editor-link-destination-rest"),
        hsla_color_value(notes_editor.link_destination_rest_color),
        None,
        false,
        &[
            "resolved.notes.editor.linkLabelColor",
            "notesEditor.link.destinationCompactOpacity",
        ],
    );
    // Authored leaf behind the rest color — JSON-only (the mockup consumes
    // the resolved color above, never a browser opacity layer).
    b.add(
        "notesEditor.link.destinationCompactOpacity",
        TokenStage::Source,
        None,
        TokenValue::Number {
            value: notes_editor.link_destination_compact_opacity as f64,
        },
        Some("notes_editor::component::MARKDOWN_LINK_DESTINATION_COMPACT_OPACITY"),
        true,
        &[],
    );
    // Behavior fact: the destination is compact unless the selection
    // overlaps OR TOUCHES the link's full range (collapsed caret included).
    b.add(
        "notesEditor.link.destinationStateRule",
        TokenStage::Source,
        None,
        TokenValue::Text {
            value: "compactUnlessSelectionOverlapsOrTouchesFullRange".to_string(),
        },
        Some("notes_editor::component::markdown_link_destination_color"),
        false,
        &[],
    );

    // Resolved markdown capture styles, read from the SAME highlight theme
    // the Input paints with (build_markdown_highlight_theme). Copying color
    // literals here is forbidden; if the resolver ever loses access, emit
    // the notesMarkdown.exporterVisibilityMissing conflict instead.
    match (
        notes_markdown.title.color,
        notes_markdown.heading_marker.color,
        notes_markdown.list_marker.color,
    ) {
        (Some(title_color), Some(heading_marker_color), Some(list_marker_color)) => {
            b.add(
                "resolved.notes.editor.markdown.titleColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-title-color"),
                hsla_color_value(title_color),
                None,
                false,
                &[
                    "theme.colors.accent.selected",
                    "highlight_theme.syntax.title",
                ],
            );
            b.add(
                "resolved.notes.editor.markdown.headingMarkerColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-heading-marker-color"),
                hsla_color_value(heading_marker_color),
                None,
                false,
                &[
                    "theme.colors.text.muted",
                    "highlight_theme.syntax.punctuation_special",
                ],
            );
            b.add(
                "resolved.notes.editor.markdown.listMarkerColor",
                TokenStage::Resolved,
                Some("--sk-notes-markdown-list-marker-color"),
                hsla_color_value(list_marker_color),
                None,
                false,
                &[
                    "theme.colors.accent.selected",
                    "highlight_theme.syntax.punctuation_list_marker",
                ],
            );
            if let Some(weight) = notes_markdown.title.font_weight {
                b.add(
                    "resolved.notes.editor.markdown.titleFontWeight",
                    TokenStage::Resolved,
                    Some("--sk-notes-markdown-title-font-weight"),
                    TokenValue::FontWeight {
                        value: weight as f64,
                    },
                    None,
                    false,
                    &["highlight_theme.syntax.title"],
                );
            }
            if let Some(weight) = notes_markdown.list_marker.font_weight {
                b.add(
                    "resolved.notes.editor.markdown.listMarkerFontWeight",
                    TokenStage::Resolved,
                    Some("--sk-notes-markdown-list-marker-font-weight"),
                    TokenValue::FontWeight {
                        value: weight as f64,
                    },
                    None,
                    false,
                    &["highlight_theme.syntax.punctuation_list_marker"],
                );
            }
        }
        _ => {
            b.conflict(
                "notesMarkdown.exporterVisibilityMissing",
                &[
                    (
                        "query captures",
                        "title / punctuation.special / punctuation.list_marker".to_string(),
                    ),
                    (
                        "observed raster",
                        "accent bold title, dimmer # marker, accent list dashes".to_string(),
                    ),
                    (
                        "exporter access",
                        "highlight theme returned no color for a contract capture".to_string(),
                    ),
                ],
                "warning",
                "The markdown capture styles could not be resolved from the real highlight \
                 theme; the color tokens are intentionally OMITTED rather than copied from \
                 screenshots. Fix the resolver, never hardcode the bytes.",
            );
        }
    }

    // ── Notes conflicts (recorded, not collapsed) ───────────────────────
    b.conflict(
        "notesFooter.layoutReservationVsIntrinsicPaint",
        &[
            (
                "NotesLayoutMetrics.footer_height / autosize",
                format!("{}", notes_layout.footer_reservation_height),
            ),
            (
                "automation_layout_info footer bounds",
                format!("{}", notes_layout.footer_reservation_height),
            ),
            (
                "GPUI universal footer action row",
                format!("{notes_footer_intrinsic}"),
            ),
        ],
        "warning",
        "The layout model reserves 28px for the Notes footer while the painted \
         universal action-button row is 32px: autosize and the layout oracle \
         under-reserve the visible band by 4px. The 280px default-height fixture \
         masks it (initial-height floor). Mockups must paint the 32px resolved \
         truth; do NOT change NotesWindowStyle.footer_height here — that would be \
         an app behavior fix, not a contract record.",
    );
    b.conflict(
        "notesFooter.buttonHeightSourceDuplication",
        &[
            (
                "Notes renderer host band",
                format!(
                    "main_layout::HINT_STRIP_HEIGHT = {}",
                    crate::window_resize::main_layout::HINT_STRIP_HEIGHT
                ),
            ),
            (
                "main window native host",
                format!(
                    "NATIVE_MAIN_WINDOW_FOOTER_HEIGHT = {}",
                    crate::window_resize::main_layout::NATIVE_MAIN_WINDOW_FOOTER_HEIGHT
                ),
            ),
        ],
        "info",
        "Notes derives its 32px button row from HINT_STRIP_HEIGHT while the \
         exported shared --sk-footer-button-height derives from the main window's \
         native footer host. The numbers coincide (both 36-hosted) but the \
         provenance differs; Notes has NO native 36px footer host — its footer is \
         an in-window GPUI strip (notes.footer.presentation).",
    );
    b.conflict(
        "notesMarkdown.titleGlyphExtentsVsLineBox",
        &[
            (
                "Input painted line box",
                format!("{}", notes_editor.line_box_height),
            ),
            (
                "resolved title style",
                format!(
                    "bold (weight {}) at the shared {}px editor size — no capture font-size \
                     exists (gpui HighlightStyle is uniformly sized)",
                    notes_markdown
                        .title
                        .font_weight
                        .map(|w| w.to_string())
                        .unwrap_or_else(|| "unset".to_string()),
                    notes_editor.base_font_size
                ),
            ),
            (
                "observed raster",
                "heading glyph tops clipped in the 2026-07-11 reference capture".to_string(),
            ),
        ],
        "warning",
        "Bold markdown title runs paint inside the Input's fixed 20px line box and \
         their upper glyph area clips. Expected consequence of the Input primitive, \
         but a screen-level renderer defect — the heading is NOT a larger nominal \
         font size (same mono advance as body lines). Mockups must reproduce the \
         clip via the line box, never by inflating the heading font.",
    );
}
