#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_conflict_has_complete_lifecycle_identity() {
        let bundle = checked_in_design_bundle().expect("bundle builds");
        assert_eq!(bundle.conflicts.len(), 34);
        for conflict in &bundle.conflicts {
            let lifecycle = &conflict.lifecycle;
            assert!(!lifecycle.owner.is_empty(), "{} owner", conflict.id);
            assert!(
                lifecycle.model_measurement_id.is_some(),
                "{} model measurement",
                conflict.id
            );
            assert!(
                lifecycle.render_measurement_id.is_some(),
                "{} render measurement",
                conflict.id
            );
            assert!(!lifecycle.task.is_empty(), "{} task", conflict.id);
            assert!(
                lifecycle.last_receipt.is_some(),
                "{} last receipt",
                conflict.id
            );
            assert!(
                !lifecycle.removal_condition.is_empty(),
                "{} removal condition",
                conflict.id
            );
        }
    }

    /// Locks the renderer-resolved bytes the HTML mockups depend on. If this
    /// test moves, regenerate design/mockups/generated and re-verify the
    /// published mockups before shipping the visual change.
    #[test]
    fn checked_in_bundle_matches_renderer_resolution() {
        let bundle = checked_in_design_bundle().expect("bundle builds");

        let rgba8 = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Color { rgba8, .. } => rgba8.clone(),
            other => panic!("{id} is not a color: {other:?}"),
        };
        let length = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Length { value } => *value,
            other => panic!("{id} is not a length: {other:?}"),
        };

        // Selected row: text_primary (#ffffff) at the component byte 0x20.
        assert_eq!(
            rgba8("resolved.mainMenu.row.selectedBackground"),
            "#FFFFFF20"
        );
        // Hover: max(theme hover 0.06*255=15, component 0x12=18) = 0x12.
        assert_eq!(rgba8("resolved.mainMenu.row.hoverBackground"), "#FFFFFF12");
        // Icon tile: accent #fbbf24 at max(def 0x80, IconTile floor 0xF2).
        assert_eq!(rgba8("resolved.mainMenu.icon.tileBackground"), "#FBBF24F2");
        // Themed geometry (44px rows), not the legacy 40px constant.
        assert_eq!(length("mainMenu.list.rowHeight"), 44.0);
        assert_eq!(length("mainMenu.row.radius"), 14.0);
        assert_eq!(length("window.width"), 750.0);
        assert_eq!(length("window.height"), 480.0);

        // The drift this system exists to expose stays recorded.
        assert!(bundle
            .conflicts
            .iter()
            .any(|c| c.id == "selectedFill.componentVsTheme"));

        // ── Actions dialog ──────────────────────────────────────────────
        // Shell + fixture composition (5 actions, 3 headers, footerless).
        assert_eq!(length("actionsDialog.shell.width"), 340.0);
        assert_eq!(length("actionsDialog.shell.maxHeight"), 400.0);
        assert_eq!(length("actionsDialog.shell.radius"), 18.0);
        assert_eq!(length("actionsDialog.shell.borderHeight"), 2.0);
        assert_eq!(length("actionsDialog.search.height"), 40.0);
        assert_eq!(length("actionsDialog.list.sectionHeaderHeight"), 24.0);
        assert_eq!(length("actionsDialog.list.rowHeight"), 36.0);
        assert_eq!(length("actionsDialog.list.paddingTop"), 0.0);
        assert_eq!(length("actionsDialog.list.paddingBottom"), 6.0);
        assert_eq!(
            length("resolved.actionsDialog.shell.bottomResidualHeight"),
            8.0
        );
        assert_eq!(length("resolved.actionsDialog.shell.fixtureHeight"), 300.0);
        // The height formula itself, not just its output snapshot.
        let popup = crate::designs::base_actions_popup_theme();
        assert_eq!(
            crate::actions::resolved_actions_popup_height(
                &popup,
                (5, 3),
                false,
                false,
                false,
                400.0,
                36.0
            ),
            300.0
        );

        // Search chrome.
        assert_eq!(length("actionsDialog.search.paddingX"), 12.0);
        assert_eq!(length("resolved.actionsDialog.search.paddingY"), 10.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.search.caretColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.search.placeholderColor"),
            "#FFFFFF66"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.search.textColor"),
            "#FFFFFFFF"
        );

        // Section chrome: centered 24px slot, muted label.
        assert_eq!(length("actionsDialog.section.paddingX"), 12.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.section.textColor"),
            "#FFFFFFA5"
        );

        // Row geometry and paint: shared ListItem seeded from InfoBarBase.
        assert_eq!(length("actionsDialog.row.wrapperInsetX"), 8.0);
        assert_eq!(length("resolved.actionsDialog.row.outerPaddingX"), 4.0);
        assert_eq!(length("resolved.actionsDialog.row.innerPaddingX"), 14.0);
        assert_eq!(length("resolved.actionsDialog.row.surfaceInsetX"), 12.0);
        assert_eq!(length("resolved.actionsDialog.row.textOriginX"), 26.0);
        assert_eq!(length("resolved.actionsDialog.row.radius"), 14.0);
        assert_eq!(length("actionsDialog.row.titleFontSize"), 14.0);
        assert_eq!(length("resolved.actionsDialog.row.titleLineHeight"), 16.0);
        assert_eq!(
            rgba8("resolved.actionsDialog.row.selectedBackground"),
            "#FFFFFF20"
        );
        assert_eq!(
            rgba8("resolved.actionsDialog.row.hoverBackground"),
            "#FFFFFF12"
        );

        // Contract flags stay footerless / top-search / header-grouped.
        let text = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Text { value } => value.clone(),
            other => panic!("{id} is not text: {other:?}"),
        };
        assert_eq!(text("actionsDialog.contract.searchPosition"), "top");
        assert_eq!(text("actionsDialog.contract.sectionMode"), "headers");
        assert_eq!(text("actionsDialog.contract.footerVisible"), "false");

        // Action-specific drift stays recorded.
        for conflict_id in [
            "actionsRow.radiusConfiguredVsPainted",
            "actionsRow.selectionConfiguredVsPainted",
            "actionsRow.compactSlotVsInheritedItemHeight",
            "actionsShortcut.popupTokensVsFooterRenderer",
            "actionsFooter.legacyHeightVsFooterlessContract",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }

        // ── Confirm prompt (in-window) ──────────────────────────────────
        // Geometry pixel-validated 2026-07-11 (see module comment).
        assert_eq!(length("confirmPrompt.window.height"), 500.0);
        assert_eq!(length("confirmPrompt.content.padding"), 24.0);
        assert_eq!(length("confirmPrompt.stack.gap"), 12.0);
        assert_eq!(length("confirmPrompt.title.fontSize"), 20.0);
        assert_eq!(length("confirmPrompt.body.fontSize"), 14.0);
        assert_eq!(length("confirmPrompt.stack.maxWidth"), 560.0);
        // GPUI's implicit phi() line heights, rounded like line_height_in_pixels.
        assert_eq!(length("resolved.confirmPrompt.title.lineHeight"), 32.0);
        assert_eq!(length("resolved.confirmPrompt.body.lineHeight"), 23.0);
        assert_eq!(length("resolved.confirmPrompt.footerSpacerHeight"), 32.0);
        // HEADER_PADDING_Y*2 + context height 22 = 38 (min 28 not binding).
        assert_eq!(length("resolved.confirmPrompt.headerHeight"), 38.0);
        assert_eq!(rgba8("resolved.confirmPrompt.titleDanger"), "#EF4444FF");
        assert_eq!(rgba8("resolved.confirmPrompt.titleDefault"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.confirmPrompt.bodyText"), "#FFFFFFFF");
        for conflict_id in [
            "confirmLayout.protocolModelVsRendererTruth",
            "confirmGap.rendererSpacingVsLayoutOracle",
            "confirmTypography.implicitLineHeightVsModeledSlots",
            "confirmFooter.heightLadder",
            "confirmFooter.slotVsInnerFrame",
            "confirmStack.rendererIntrinsicVsLayoutModel",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }

        // ── Notes window ────────────────────────────────────────────────
        let text = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Text { value } => value.clone(),
            other => panic!("{id} is not text: {other:?}"),
        };
        let number = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::Number { value } => *value,
            other => panic!("{id} is not a number: {other:?}"),
        };
        let weight = |id: &str| match &bundle.tokens.get(id).expect(id).value {
            TokenValue::FontWeight { value } => *value,
            other => panic!("{id} is not a font weight: {other:?}"),
        };
        let record = |id: &str| bundle.tokens.get(id).expect(id);

        // App-authored chrome (source stage, writable).
        assert_eq!(length("notes.window.defaultWidth"), 350.0);
        assert_eq!(length("notes.window.defaultHeight"), 280.0);
        assert_eq!(length("notes.window.defaultEdgePadding"), 20.0);
        assert_eq!(length("notes.titlebar.height"), 36.0);
        assert_eq!(length("notes.titlebar.paddingX"), 12.0);
        assert_eq!(length("notes.titlebar.leadingReserveWidth"), 60.0);
        assert_eq!(length("notes.titlebar.trailingReserveWidth"), 100.0);
        assert_eq!(length("notes.titlebar.trafficLightOriginX"), 8.0);
        assert_eq!(length("notes.titlebar.trafficLightOriginY"), 7.0);
        assert_eq!(length("notes.editor.paddingX"), 16.0);
        assert_eq!(length("notes.editor.paddingY"), 12.0);
        assert_eq!(length("notes.footer.statusMinWidth"), 24.0);
        assert_eq!(length("notes.footer.contentInsetX"), 14.0);
        assert_eq!(length("notes.footer.actionGap"), 2.0);
        // Opacity numbers cross the f32→f64 bridge (0.7f32 is not exactly
        // 0.7), matching the existing exported-number precedent
        // (--sk-main-menu-context-opacity).
        assert_eq!(number("notes.titlebar.titleRestOpacity"), 0.7f32 as f64);
        assert_eq!(number("notes.footer.restOpacity"), 0.5);
        for id in ["notes.window.defaultWidth", "notes.titlebar.height"] {
            let r = record(id);
            assert!(matches!(r.stage, TokenStage::Source), "{id} must be source");
            assert!(r.writable, "{id} must be writable");
        }

        // Layout MODEL: honest 28px reservation, distinct from paint.
        assert_eq!(length("notes.layout.footerReservationHeight"), 28.0);
        assert_eq!(length("notes.layout.autoResize.maxHeight"), 600.0);
        assert_eq!(length("notes.layout.autoResize.assumedLineHeight"), 20.0);
        assert_eq!(length("notes.layout.autoResize.applyThreshold"), 5.0);
        assert!(
            record("notes.layout.footerReservationHeight")
                .css_var
                .is_none(),
            "the 28px model reservation must not leak into mockup CSS"
        );
        assert!(!record("notes.layout.autoResize.assumedLineHeight").writable);

        // Footer presentation facts.
        assert_eq!(text("notes.footer.presentation"), "inWindowGpui");
        assert_eq!(text("notes.footer.nativeOverlay"), "false");
        assert_eq!(text("notes.footer.visibility"), "selectedNoteOnly");
        assert_eq!(text("notes.editor.markdown.language"), "markdown");
        for id in [
            "notes.editor.markdown.highlightQueryFingerprint",
            "notes.editor.markdown.injectionQueryFingerprint",
            "notes.editor.markdown.inlineHighlightQueryFingerprint",
        ] {
            assert!(
                text(id).starts_with("fnv1a64:"),
                "{id} must be a stable query fingerprint"
            );
            assert!(!record(id).writable);
        }

        // Resolved editor paint metrics (theme bridge + Input internals).
        assert_eq!(length("resolved.notes.editor.baseFontSize"), 16.0);
        assert_eq!(length("resolved.notes.editor.lineBoxHeight"), 20.0);
        assert_eq!(length("resolved.notes.editor.caretWidth"), 2.0);
        assert_eq!(length("resolved.notes.editor.caretHeight"), 17.0);
        assert_eq!(text("resolved.notes.editor.fontFamily"), "JetBrains Mono");
        assert_eq!(length("resolved.notes.titlebar.titleFontSize"), 14.0);
        assert_eq!(length("resolved.notes.footer.statusFontSize"), 12.0);
        // Painted footer band (32) vs the 28px model above.
        assert_eq!(length("resolved.notes.footer.intrinsicHeight"), 32.0);
        for id in [
            "resolved.notes.editor.baseFontSize",
            "resolved.notes.editor.lineBoxHeight",
            "resolved.notes.footer.intrinsicHeight",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // Markdown capture styles from the real highlight theme: accent bold
        // title, muted (separate) heading marker, accent bold list marker —
        // and NO heading font-size token (gpui HighlightStyle is uniformly
        // sized; the heading clips instead).
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.titleColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.headingMarkerColor"),
            "#FFFFFFFF"
        );
        assert_eq!(
            rgba8("resolved.notes.editor.markdown.listMarkerColor"),
            "#FBBF24FF"
        );
        assert_eq!(
            weight("resolved.notes.editor.markdown.titleFontWeight"),
            700.0
        );
        assert_eq!(
            weight("resolved.notes.editor.markdown.listMarkerFontWeight"),
            700.0
        );
        assert!(
            !bundle
                .tokens
                .keys()
                .any(|k| k.contains("markdown.titleFontSize")
                    || k.contains("markdown.headingFontSize")),
            "no markdown heading font-size token may exist"
        );
        assert!(
            !bundle
                .conflicts
                .iter()
                .any(|c| c.id == "notesMarkdown.exporterVisibilityMissing"),
            "the highlight theme is reachable; the visibility conflict must not fire"
        );

        // Notes drift stays recorded.
        for conflict_id in [
            "notesFooter.layoutReservationVsIntrinsicPaint",
            "notesFooter.buttonHeightSourceDuplication",
            "notesMarkdown.titleGlyphExtentsVsLineBox",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "notesFooter.layoutReservationVsIntrinsicPaint")
                .expect("reservation conflict")
                .severity,
            "warning"
        );

        // ── Settings hub ────────────────────────────────────────────────
        // Canonical shared owners only — settings mints NO alias tokens
        // (2026-07-11 Oracle correction). The profile records the design
        // variant the exporter resolves spacing with.
        assert_eq!(bundle.profile.design_variant, "default");
        assert_eq!(length("design.spacing.paddingXs"), 4.0);
        {
            let r = record("design.spacing.paddingXs");
            assert!(matches!(r.stage, TokenStage::Source));
            assert!(r.writable);
        }
        // Shared builtin-input count-label typography: text_sm size, gpui
        // default phi line height, NORMAL weight (never search 430).
        assert_eq!(
            length("resolved.builtinMainInput.countLabel.fontSize"),
            14.0
        );
        assert_eq!(
            length("resolved.builtinMainInput.countLabel.lineHeight"),
            23.0
        );
        assert_eq!(
            weight("resolved.builtinMainInput.countLabel.fontWeight"),
            400.0
        );
        // The first "Settings" separator paints the LEGACY list-item default
        // path (26/6) while the themed InfoBarBase pair stays 28/4.
        assert_eq!(
            length("resolved.listItem.default.firstSectionSlotHeight"),
            26.0
        );
        assert_eq!(
            length("resolved.listItem.default.firstSectionPaddingTop"),
            6.0
        );
        assert_eq!(length("mainMenu.list.firstSectionSlotHeight"), 28.0);
        assert_eq!(length("mainMenu.section.firstPaddingTop"), 4.0);
        for id in [
            "resolved.builtinMainInput.countLabel.fontSize",
            "resolved.builtinMainInput.countLabel.lineHeight",
            "resolved.builtinMainInput.countLabel.fontWeight",
            "resolved.listItem.default.firstSectionSlotHeight",
            "resolved.listItem.default.firstSectionPaddingTop",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only settings facts (no CSS role, never writable).
        assert_eq!(text("settingsHub.section.emptyFilterLabel"), "Settings");
        assert_eq!(text("settingsHub.section.filteredLabel"), "Results");
        assert_eq!(text("settingsHub.countLabel.counts"), "visibleFilteredRows");
        assert_eq!(
            text("settingsHub.countLabel.pluralization"),
            "1 setting / 2 settings"
        );
        assert_eq!(number("settingsHub.census.baseCount"), 11.0);
        assert_eq!(number("settingsHub.census.customPositionsCount"), 12.0);
        assert_eq!(
            text("settingsHub.census.optionalRow"),
            "Reset Window Positions"
        );
        assert_eq!(
            text("settingsHub.census.optionalPredicate"),
            "windowState.hasCustomPositions"
        );
        assert_eq!(number("settingsHub.icons.resolvedRowIconCount"), 0.0);
        for id in [
            "settingsHub.section.emptyFilterLabel",
            "settingsHub.section.filteredLabel",
            "settingsHub.countLabel.counts",
            "settingsHub.countLabel.pluralization",
            "settingsHub.census.baseCount",
            "settingsHub.census.customPositionsCount",
            "settingsHub.census.optionalRow",
            "settingsHub.census.optionalPredicate",
            "settingsHub.icons.resolvedRowIconCount",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // The rejected settings.* alias family must not exist: the count
        // inset reuses --sk-main-menu-search-text-inset-x and the count
        // color reuses --sk-text-hint directly.
        assert!(
            bundle.tokens.keys().all(|k| !k.starts_with("settings.")),
            "settings must not mint alias tokens under settings.*"
        );

        // Settings drift stays recorded.
        for conflict_id in [
            "settingsSection.firstSlotLegacyVsThemed",
            "settingsRows.authoredIconHintsVsResolvedNone",
            "settingsFooter.nativeRunVsGpuiOpenHint",
        ] {
            assert!(
                bundle.conflicts.iter().any(|c| c.id == conflict_id),
                "missing conflict {conflict_id}"
            );
        }
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "settingsRows.authoredIconHintsVsResolvedNone")
                .expect("icon conflict")
                .severity,
            "warning"
        );

        // ── Day Page (2026-07-11 Oracle-corrected slice) ────────────────
        // The five Day-owned geometry tokens (source, writable) — the ONLY
        // --sk-day-page-* CSS variables allowed to exist.
        assert_eq!(length("dayPage.editor.minHeight"), 180.0);
        assert_eq!(length("dayPage.shelf.topPadding"), 6.0);
        assert_eq!(length("dayPage.shelf.toggleHeight"), 20.0);
        assert_eq!(length("dayPage.shelf.expandedListGap"), 4.0);
        assert_eq!(length("dayPage.shelf.rowSlotHeight"), 24.0);
        for id in [
            "dayPage.editor.minHeight",
            "dayPage.shelf.topPadding",
            "dayPage.shelf.toggleHeight",
            "dayPage.shelf.expandedListGap",
            "dayPage.shelf.rowSlotHeight",
        ] {
            let r = record(id);
            assert!(matches!(r.stage, TokenStage::Source), "{id} must be source");
            assert!(r.writable, "{id} must be writable");
        }
        assert_eq!(number("dayPage.shelf.maxBodyFraction"), 0.4f32 as f64);
        assert!(record("dayPage.shelf.maxBodyFraction").css_var.is_none());

        // Shared owners the Day Page consumes (NO Day copies).
        assert_eq!(length("resolved.mainView.contentRightInsetX"), 2.0);
        assert_eq!(length("resolved.notes.editor.inputPaddingX"), 12.0);
        assert_eq!(length("resolved.notes.editor.inputPaddingY"), 8.0);
        assert_eq!(rgba8("resolved.notes.editor.textColor"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.notes.editor.caretColor"), "#FFFFFFFF");
        assert_eq!(rgba8("resolved.notes.editor.linkLabelColor"), "#FBBF24FF");
        // Rest destination: accent through the ACTUAL highlighter helper
        // (accent.opacity(0.45)) — 0.45 × 255 rounds to 0x73, resolved by
        // the color conversion, never a hand-entered byte.
        assert_eq!(
            rgba8("resolved.notes.editor.linkDestinationRestColor"),
            "#FBBF2473"
        );
        assert_eq!(
            number("notesEditor.link.destinationCompactOpacity"),
            0.45f32 as f64
        );
        assert!(record("notesEditor.link.destinationCompactOpacity")
            .css_var
            .is_none());
        assert_eq!(
            text("notesEditor.link.destinationStateRule"),
            "compactUnlessSelectionOverlapsOrTouchesFullRange"
        );
        // muted_foreground = text.primary @ opacity.text_placeholder (0.40).
        assert_eq!(
            rgba8("resolved.componentTheme.mutedForeground"),
            "#FFFFFF66"
        );
        assert_eq!(rgba8("resolved.componentTheme.foreground"), "#FFFFFFFF");
        assert_eq!(length("resourcePreview.compactRow.paddingX"), 8.0);
        assert_eq!(length("resourcePreview.compactRow.paddingY"), 4.0);
        assert_eq!(length("resolved.resourcePreview.compactRow.gap"), 8.0);
        assert_eq!(length("resolved.framework.textXsFontSize"), 12.0);
        assert_eq!(length("resolved.framework.gap1"), 4.0);
        for id in [
            "resolved.mainView.contentRightInsetX",
            "resolved.notes.editor.inputPaddingX",
            "resolved.notes.editor.inputPaddingY",
            "resolved.notes.editor.textColor",
            "resolved.notes.editor.caretColor",
            "resolved.notes.editor.linkLabelColor",
            "resolved.notes.editor.linkDestinationRestColor",
            "resolved.componentTheme.mutedForeground",
            "resolved.componentTheme.foreground",
            "resolved.resourcePreview.compactRow.gap",
            "resolved.framework.textXsFontSize",
            "resolved.framework.gap1",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only Day Page facts (no CSS role, never writable).
        assert_eq!(text("dayPage.header.contextInteraction"), "inert");
        assert_eq!(text("dayPage.header.inputSlot"), "none");
        assert_eq!(text("dayPage.header.dividerVisible"), "false");
        assert_eq!(text("dayPage.editor.spine.localOverlay"), "disabled");
        assert_eq!(
            text("dayPage.editor.spine.contextMentions"),
            "mainMenuRoundTrip"
        );
        assert_eq!(text("dayPage.shelf.defaultExpanded"), "false");
        assert_eq!(text("dayPage.shelf.hiddenWhenEmpty"), "true");
        assert_eq!(text("dayPage.shelf.hiddenDuringKitPreview"), "true");
        assert_eq!(text("dayPage.shelf.sourceLines"), "liftedFromEditor");
        assert_eq!(
            text("dayPage.footer.presentation"),
            "gpuiSpacerPlusNativeOverlay"
        );
        assert_eq!(text("dayPage.footer.defaultAction"), "actions");
        for id in [
            "dayPage.header.contextInteraction",
            "dayPage.header.inputSlot",
            "dayPage.header.dividerVisible",
            "dayPage.editor.spine.localOverlay",
            "dayPage.editor.spine.contextMentions",
            "dayPage.shelf.defaultExpanded",
            "dayPage.shelf.hiddenWhenEmpty",
            "dayPage.shelf.hiddenDuringKitPreview",
            "dayPage.shelf.sourceLines",
            "dayPage.footer.presentation",
            "dayPage.footer.defaultAction",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // No Day-prefixed duplicates of shared editor/link/caret/footer
        // tokens: the ONLY --sk-day-page-* variables are the five geometry
        // tokens above.
        let day_vars: Vec<&str> = bundle
            .tokens
            .values()
            .filter_map(|r| r.css_var.as_deref())
            .filter(|v| v.starts_with("--sk-day-page-"))
            .collect();
        let mut day_vars_sorted = day_vars.clone();
        day_vars_sorted.sort_unstable();
        assert_eq!(
            day_vars_sorted,
            vec![
                "--sk-day-page-editor-min-height",
                "--sk-day-page-shelf-expanded-list-gap",
                "--sk-day-page-shelf-row-slot-height",
                "--sk-day-page-shelf-toggle-height",
                "--sk-day-page-shelf-top-padding",
            ],
            "Day Page may only own its five geometry variables"
        );

        // Every CSS variable has exactly ONE token owner (bundle-wide).
        {
            let mut seen = std::collections::BTreeMap::new();
            for (id, r) in &bundle.tokens {
                if let Some(var) = &r.css_var {
                    if let Some(previous) = seen.insert(var.clone(), id.clone()) {
                        panic!("css var {var} owned by both {previous} and {id}");
                    }
                }
            }
        }

        // Canonical reference-fixture geometry (750×480, context-only header
        // 30, footer 32, one kept entry): collapsed 418/380/38/0 — the formula itself,
        // not just a snapshot.
        let collapsed = crate::day_page::layout::day_page_layout_budget(
            length("window.height") as f32,
            30.0,
            length("resolved.confirmPrompt.footerSpacerHeight") as f32,
            1,
            false,
            length("notes.editor.paddingY") as f32,
        );
        assert_eq!(collapsed.body_height, 418.0);
        assert_eq!(collapsed.editor_height, 380.0);
        assert_eq!(collapsed.shelf_height, 38.0);
        assert_eq!(collapsed.shelf_list_height, 0.0);

        // Day Page drift stays recorded (the footer height ladder).
        assert_eq!(
            bundle
                .conflicts
                .iter()
                .find(|c| c.id == "dayPageFooter.spacerVsNativeHostBand")
                .expect("day page footer conflict")
                .severity,
            "warning"
        );

        // CSS renders every var exactly once.
        let css = render_css(&bundle);
        assert_eq!(css.matches("--sk-main-menu-row-height:").count(), 1);
        assert!(css
            .contains("--sk-main-menu-row-selected-background: rgb(255 255 255 / 0.1254901961);"));
        assert_eq!(
            css.matches("--sk-actions-dialog-row-selected-background:")
                .count(),
            1
        );
        assert!(css.contains(
            "--sk-actions-dialog-row-selected-background: rgb(255 255 255 / 0.1254901961);"
        ));
        assert!(css.contains("--sk-actions-dialog-height: 300px;"));
        assert!(!css.contains("--sk-actions-dialog-footer-height:"));
        assert!(css.contains("--sk-confirm-window-height: 500px;"));
        assert!(css.contains("--sk-confirm-stack-max-width: 560px;"));
        assert!(css.contains("--sk-confirm-title-danger: rgb(239 68 68);"));
        assert!(css.contains("--sk-confirm-body-line-height: 23px;"));

        // Every --sk-notes-* var appears exactly once, with resolved values.
        for var in [
            "--sk-notes-window-width",
            "--sk-notes-window-height",
            "--sk-notes-titlebar-height",
            "--sk-notes-titlebar-padding-x",
            "--sk-notes-titlebar-traffic-width",
            "--sk-notes-titlebar-icons-width",
            "--sk-notes-titlebar-title-font-size",
            "--sk-notes-titlebar-title-rest-opacity",
            "--sk-notes-traffic-x",
            "--sk-notes-traffic-y",
            "--sk-notes-editor-padding-x",
            "--sk-notes-editor-padding-y",
            "--sk-notes-editor-font-size",
            "--sk-notes-editor-font-family",
            "--sk-notes-editor-line-box-height",
            "--sk-notes-editor-input-padding-x",
            "--sk-notes-editor-input-padding-y",
            "--sk-notes-editor-text-color",
            "--sk-notes-editor-link-label",
            "--sk-notes-editor-link-destination-rest",
            "--sk-notes-caret-width",
            "--sk-notes-caret-height",
            "--sk-notes-caret-color",
            "--sk-notes-footer-height",
            "--sk-notes-footer-content-inset-x",
            "--sk-notes-footer-rest-opacity",
            "--sk-notes-footer-status-min-width",
            "--sk-notes-footer-status-font-size",
            "--sk-notes-footer-action-gap",
            "--sk-notes-markdown-title-color",
            "--sk-notes-markdown-title-font-weight",
            "--sk-notes-markdown-heading-marker-color",
            "--sk-notes-markdown-list-marker-color",
            "--sk-notes-markdown-list-marker-font-weight",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-notes-footer-height: 32px;"));
        assert!(css.contains("--sk-notes-editor-line-box-height: 20px;"));
        assert!(css.contains("--sk-notes-markdown-title-color: rgb(251 191 36);"));
        assert!(!css.contains("--sk-notes-editor-line-height:"));
        assert!(!css.contains("--sk-notes-markdown-heading-font-size"));

        // Settings-slice vars render exactly once, under their shared
        // owners; NO --sk-settings-* alias vars may exist.
        for var in [
            "--sk-spacing-padding-xs",
            "--sk-builtin-main-input-count-font-size",
            "--sk-builtin-main-input-count-line-height",
            "--sk-builtin-main-input-count-font-weight",
            "--sk-list-item-default-first-section-slot-height",
            "--sk-list-item-default-first-section-padding-top",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-spacing-padding-xs: 4px;"));
        assert!(css.contains("--sk-builtin-main-input-count-font-size: 14px;"));
        assert!(css.contains("--sk-builtin-main-input-count-line-height: 23px;"));
        assert!(css.contains("--sk-builtin-main-input-count-font-weight: 400;"));
        assert!(css.contains("--sk-list-item-default-first-section-slot-height: 26px;"));
        assert!(css.contains("--sk-list-item-default-first-section-padding-top: 6px;"));
        assert!(!css.contains("--sk-settings-"));

        // Day Page slice vars render exactly once; NO rejected Day aliases.
        for var in [
            "--sk-day-page-editor-min-height",
            "--sk-day-page-shelf-top-padding",
            "--sk-day-page-shelf-toggle-height",
            "--sk-day-page-shelf-expanded-list-gap",
            "--sk-day-page-shelf-row-slot-height",
            "--sk-main-view-content-right-inset-x",
            "--sk-component-theme-muted-foreground",
            "--sk-component-theme-foreground",
            "--sk-compact-resource-row-padding-x",
            "--sk-compact-resource-row-padding-y",
            "--sk-compact-resource-row-gap",
            "--sk-framework-text-xs-font-size",
            "--sk-framework-gap-1",
        ] {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert!(css.contains("--sk-day-page-editor-min-height: 180px;"));
        assert!(css.contains("--sk-main-view-content-right-inset-x: 2px;"));
        assert!(css
            .contains("--sk-notes-editor-link-destination-rest: rgb(251 191 36 / 0.4509803922);"));
        assert!(css.contains("--sk-notes-editor-font-family: \"JetBrains Mono\";"));
        // Rejected Day-prefixed duplicates must never exist.
        for rejected in [
            "--sk-day-page-content-inset-x:",
            "--sk-day-page-editor-padding",
            "--sk-day-page-editor-input-padding",
            "--sk-day-page-editor-font-size:",
            "--sk-day-page-editor-line-height:",
            "--sk-day-page-editor-text:",
            "--sk-day-page-link-",
            "--sk-day-page-caret-",
            "--sk-day-page-shelf-gap:",
            "--sk-day-page-shelf-row-height:",
            "--sk-day-page-shelf-row-padding",
            "--sk-day-page-shelf-font-size:",
            "--sk-day-page-shelf-muted:",
            "--sk-day-page-footer-spacer-height:",
        ] {
            assert!(
                !css.contains(rejected),
                "rejected Day Page alias {rejected} must not exist"
            );
        }

        // ── Agent Chat (2026-07-11 Oracle-corrected slice) ──────────────
        // Source geometry (writable) straight off production_agent_chat_style.
        assert_eq!(length("agentChat.transcript.rowPaddingX"), 16.0);
        assert_eq!(length("agentChat.transcript.rowPaddingBottom"), 4.0);
        assert_eq!(length("agentChat.markdown.bodyFontSize"), 14.0);
        assert_eq!(length("agentChat.markdown.codeFontSize"), 13.0);
        assert_eq!(length("agentChat.block.borderWidth"), 2.0);
        assert_eq!(length("agentChat.block.headerGap"), 4.0);
        // Embedded default composer aliases the canonical main-menu search.
        assert_eq!(length("agentChat.composer.fontSize"), 20.0);
        assert_eq!(length("agentChat.composer.lineHeight"), 26.0);
        assert_eq!(length("agentChat.send.size"), 24.0);
        assert_eq!(length("agentChat.send.radius"), 6.0);
        {
            let r = record("agentChat.transcript.rowPaddingX");
            assert!(matches!(r.stage, TokenStage::Source));
            assert!(r.writable);
        }

        // Thought and tool header opacities stay SEPARATE tokens (both 0.75).
        assert_eq!(number("agentChat.block.thoughtHeaderOpacity"), 0.75);
        assert_eq!(number("agentChat.block.toolHeaderOpacity"), 0.75);
        assert_ne!(
            record("agentChat.block.thoughtHeaderOpacity").css_var,
            record("agentChat.block.toolHeaderOpacity").css_var,
            "thought/tool header opacities must not collapse into one var"
        );

        // Authored alpha leaves: JSON-only, and the decimal-50 foot-gun
        // stays authored decimal (0x32 only after the shared packer).
        assert_eq!(number("agentChat.error.bgAlpha"), 50.0);
        assert_eq!(number("agentChat.user.bgAlpha"), 6.0);
        assert_eq!(number("agentChat.block.toolBorderAlpha"), 127.0);
        assert_eq!(number("agentChat.diff.tintAlpha"), 20.0);
        assert_eq!(
            number("agentChat.markdown.paragraphGapRems"),
            0.28f32 as f64
        );
        for id in [
            "agentChat.transcript.turnDividerAlpha",
            "agentChat.markdown.codeBgAlpha",
            "agentChat.markdown.codeBorderAlpha",
            "agentChat.markdown.blockquoteBgAlpha",
            "agentChat.markdown.blockquoteBorderAlpha",
            "agentChat.user.bgAlpha",
            "agentChat.block.thoughtBorderAlpha",
            "agentChat.block.toolBorderAlpha",
            "agentChat.tool.statusPendingAlpha",
            "agentChat.diff.tintAlpha",
            "agentChat.system.borderAlpha",
            "agentChat.error.bgAlpha",
            "agentChat.error.borderAlpha",
            "agentChat.send.disabledBgAlpha",
            "agentChat.send.enabledBgAlpha",
            "agentChat.send.queueBgAlpha",
            "agentChat.markdown.paragraphGapRems",
            "agentChat.composer.paddingX",
            "agentChat.composer.paddingY",
        ] {
            assert!(
                record(id).css_var.is_none(),
                "{id} must stay JSON-only (no CSS variable)"
            );
        }

        // Resolved paint bytes — the SAME resolver output the renderer packs.
        assert_eq!(
            rgba8("resolved.agentChat.transcript.turnDivider"),
            "#34343418"
        );
        assert_eq!(rgba8("resolved.agentChat.user.bg"), "#FFFFFF06");
        assert_eq!(rgba8("resolved.agentChat.markdown.codeBg"), "#2A2A2AA0");
        assert_eq!(rgba8("resolved.agentChat.markdown.codeBorder"), "#34343440");
        assert_eq!(rgba8("resolved.agentChat.thought.border"), "#FFFFFF7F");
        assert_eq!(rgba8("resolved.agentChat.tool.border"), "#FBBF247F");
        assert_eq!(rgba8("resolved.agentChat.tool.borderError"), "#EF44447F");
        assert_eq!(rgba8("resolved.agentChat.tool.statusPending"), "#FFFFFF80");
        assert_eq!(rgba8("resolved.agentChat.tool.statusComplete"), "#00FF00FF");
        assert_eq!(rgba8("resolved.agentChat.tool.statusFailed"), "#EF4444FF");
        assert_eq!(rgba8("resolved.agentChat.diff.addedBg"), "#00FF0014");
        assert_eq!(rgba8("resolved.agentChat.diff.removedBg"), "#EF444414");
        assert_eq!(rgba8("resolved.agentChat.system.border"), "#34343430");
        // Decimal 50 → 0x32 through the shared pack_rgb_alpha owner.
        assert_eq!(rgba8("resolved.agentChat.error.bg"), "#EF444432");
        assert_eq!(rgba8("resolved.agentChat.error.border"), "#EF444480");
        assert_eq!(rgba8("resolved.agentChat.send.disabledBg"), "#FFFFFF06");
        assert_eq!(rgba8("resolved.agentChat.send.enabledBg"), "#FBBF2430");
        assert_eq!(rgba8("resolved.agentChat.send.queueBg"), "#FBBF2424");

        // Resolved typography/geometry through the shared app helpers.
        assert_eq!(length("resolved.agentChat.markdown.bodyLineHeight"), 23.0);
        assert_eq!(length("resolved.agentChat.composer.singleLineHeight"), 26.0);
        assert_eq!(length("resolved.framework.textSmFontSize"), 14.0);
        for id in [
            "resolved.agentChat.transcript.turnDivider",
            "resolved.agentChat.markdown.bodyLineHeight",
            "resolved.agentChat.composer.singleLineHeight",
            "resolved.framework.textSmFontSize",
        ] {
            let r = record(id);
            assert!(
                matches!(r.stage, TokenStage::Resolved),
                "{id} must be resolved"
            );
            assert!(!r.writable, "{id} must not be writable");
        }

        // JSON-only Agent Chat facts.
        assert_eq!(
            text("agentChat.composer.placeholderEmpty"),
            "Ask anything\u{2026}"
        );
        assert_eq!(
            text("agentChat.composer.placeholderFollowUp"),
            "Follow up\u{2026}"
        );
        assert_eq!(
            text("agentChat.composer.fontFamily"),
            crate::list_item::FONT_SYSTEM_UI
        );
        assert_eq!(text("agentChat.legacyComposer.fontFamily"), ".SystemUIFont");
        let embedded_composer_family = record("agentChat.composer.fontFamily");
        assert!(matches!(
            embedded_composer_family.stage,
            TokenStage::Resolved
        ));
        assert_eq!(
            embedded_composer_family.derived_from,
            vec!["mainMenu.type.uiFontFamily".to_string()]
        );
        assert_eq!(
            text("agentChat.transcript.alignment"),
            "bottomFollowTailWithSyntheticActivityTail"
        );
        assert_eq!(
            text("agentChat.footer.presentation"),
            "gpuiSpacerPlusNativeOverlay"
        );
        assert_eq!(
            text("agentChat.tool.defaultExpansion"),
            "collapsedExceptDiffOrError"
        );
        // Variant-limited numbers stay JSON-only facts.
        assert_eq!(number("agentChat.user.maxWidthRoleSplitOnly"), 520.0);
        assert_eq!(number("agentChat.assistant.maxWidthRoleSplitOnly"), 620.0);
        assert_eq!(number("agentChat.assistant.radius"), 0.0);
        assert_eq!(number("agentChat.assistant.bgAlpha"), 0.0);
        assert_eq!(number("agentChat.activity.dotSize"), 7.0);
        assert_eq!(number("agentChat.activity.gap"), 8.0);
        assert_eq!(number("agentChat.activity.labelAlpha"), 176.0);
        for id in [
            "agentChat.composer.placeholderEmpty",
            "agentChat.composer.placeholderFollowUp",
            "agentChat.composer.fontFamily",
            "agentChat.legacyComposer.fontFamily",
            "agentChat.transcript.alignment",
            "agentChat.footer.presentation",
            "agentChat.tool.defaultExpansion",
            "agentChat.user.maxWidthRoleSplitOnly",
            "agentChat.assistant.maxWidthRoleSplitOnly",
            "agentChat.assistant.radius",
            "agentChat.assistant.bgAlpha",
            "agentChat.activity.dotSize",
            "agentChat.activity.gap",
            "agentChat.activity.labelAlpha",
        ] {
            let r = record(id);
            assert!(r.css_var.is_none(), "{id} is a JSON-only fact");
            assert!(!r.writable, "{id} must not be writable");
        }

        // Remaining Agent Chat drift stays recorded as explicit conflicts.
        for (conflict_id, severity) in [
            ("agentChat.error.bgAlphaUnits", "info"),
            ("agentChat.standard.roleSplitOnlyFields", "info"),
        ] {
            let conflict = bundle
                .conflicts
                .iter()
                .find(|c| c.id == conflict_id)
                .unwrap_or_else(|| panic!("missing conflict {conflict_id}"));
            assert_eq!(conflict.severity, severity, "{conflict_id} severity");
        }

        // The explicit Agent Chat CSS-variable manifest: every var renders
        // exactly once, and nothing outside this list may exist.
        let agent_chat_manifest = [
            "--sk-agent-chat-row-padding-x",
            "--sk-agent-chat-row-padding-bottom",
            "--sk-agent-chat-response-start-margin-top",
            "--sk-agent-chat-turn-margin-top",
            "--sk-agent-chat-turn-padding-top",
            "--sk-agent-chat-turn-divider",
            "--sk-agent-chat-md-body-font-size",
            "--sk-agent-chat-md-body-line-height",
            "--sk-agent-chat-md-h1-font-size",
            "--sk-agent-chat-md-h2-font-size",
            "--sk-agent-chat-md-h3-font-size",
            "--sk-agent-chat-md-code-font-size",
            "--sk-agent-chat-md-code-padding-x",
            "--sk-agent-chat-md-code-padding-y",
            "--sk-agent-chat-md-code-radius",
            "--sk-agent-chat-md-code-bg",
            "--sk-agent-chat-md-code-border",
            "--sk-agent-chat-md-blockquote-padding-x",
            "--sk-agent-chat-md-blockquote-padding-y",
            "--sk-agent-chat-md-blockquote-radius",
            "--sk-agent-chat-md-blockquote-bg",
            "--sk-agent-chat-md-blockquote-border",
            "--sk-agent-chat-user-padding-x",
            "--sk-agent-chat-user-padding-y",
            "--sk-agent-chat-user-radius",
            "--sk-agent-chat-user-bg",
            "--sk-agent-chat-assistant-padding-x",
            "--sk-agent-chat-assistant-padding-y",
            "--sk-agent-chat-block-padding-x",
            "--sk-agent-chat-block-padding-y",
            "--sk-agent-chat-block-body-padding-top",
            "--sk-agent-chat-block-max-body-height",
            "--sk-agent-chat-block-border-width",
            "--sk-agent-chat-block-header-gap",
            "--sk-agent-chat-thought-header-opacity",
            "--sk-agent-chat-tool-header-opacity",
            "--sk-agent-chat-block-status-opacity",
            "--sk-agent-chat-thought-border",
            "--sk-agent-chat-tool-border",
            "--sk-agent-chat-tool-border-error",
            "--sk-agent-chat-tool-status-pending",
            "--sk-agent-chat-tool-status-complete",
            "--sk-agent-chat-tool-status-failed",
            "--sk-agent-chat-diff-added-bg",
            "--sk-agent-chat-diff-removed-bg",
            "--sk-agent-chat-diff-context-opacity",
            "--sk-agent-chat-system-padding-x",
            "--sk-agent-chat-system-padding-y",
            "--sk-agent-chat-system-opacity",
            "--sk-agent-chat-system-border",
            "--sk-agent-chat-error-padding-x",
            "--sk-agent-chat-error-padding-y",
            "--sk-agent-chat-error-radius",
            "--sk-agent-chat-error-bg",
            "--sk-agent-chat-error-border",
            "--sk-agent-chat-error-label-opacity",
            "--sk-agent-chat-error-hint-opacity",
            "--sk-agent-chat-composer-font-size",
            "--sk-agent-chat-composer-font-weight",
            "--sk-agent-chat-composer-line-height",
            "--sk-agent-chat-composer-single-line-height",
            "--sk-agent-chat-send-size",
            "--sk-agent-chat-send-radius",
            "--sk-agent-chat-send-disabled-bg",
            "--sk-agent-chat-send-disabled-opacity",
            "--sk-agent-chat-send-enabled-bg",
            "--sk-agent-chat-send-enabled-opacity",
            "--sk-agent-chat-send-queue-bg",
            "--sk-agent-chat-send-queue-opacity",
            "--sk-agent-chat-send-streaming-opacity",
        ];
        for var in agent_chat_manifest {
            assert_eq!(
                css.matches(&format!("{var}:")).count(),
                1,
                "{var} must render exactly once"
            );
        }
        assert_eq!(
            css.matches("--sk-agent-chat-").count(),
            agent_chat_manifest.len(),
            "no --sk-agent-chat-* variable may exist outside the manifest"
        );
        assert_eq!(css.matches("--sk-framework-text-sm-font-size:").count(), 1);
        assert!(css.contains("--sk-agent-chat-md-body-font-size: 14px;"));
        assert!(css.contains("--sk-agent-chat-md-body-line-height: 23px;"));
        assert!(css.contains("--sk-agent-chat-composer-font-size: 20px;"));
        assert!(css.contains("--sk-agent-chat-composer-font-weight: 430;"));
        assert!(css.contains("--sk-agent-chat-composer-line-height: 26px;"));
        assert!(css.contains("--sk-agent-chat-composer-single-line-height: 26px;"));
        assert!(css.contains("--sk-agent-chat-thought-header-opacity: 0.75;"));
        assert!(css.contains("--sk-agent-chat-tool-header-opacity: 0.75;"));
        assert!(css.contains("--sk-agent-chat-tool-status-complete: rgb(0 255 0);"));
        assert!(css.contains("--sk-agent-chat-turn-divider: rgb(52 52 52 / 0.0941176471);"));
        assert!(css.contains("--sk-framework-text-sm-font-size: 14px;"));
        // Rejected Agent Chat vars must never exist.
        for rejected in [
            "--sk-agent-chat-footer-dot",
            "--sk-agent-chat-composer-height:",
            "--sk-agent-chat-block-header-opacity:",
            "--sk-agent-chat-md-paragraph-gap",
            "--sk-agent-chat-user-max-width",
            "--sk-agent-chat-assistant-max-width",
            "--sk-emulator-",
        ] {
            assert!(
                !css.contains(rejected),
                "rejected Agent Chat var {rejected} must not exist"
            );
        }
    }
}
