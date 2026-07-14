//! Regression tests for the root launcher file-search boundary.
//!
//! Root search may append bounded file rows, but the unified matcher itself
//! must stay a pure in-memory ranker. Spotlight process ownership remains in
//! the file-search module, and the dedicated File Search view keeps its richer
//! directory-browser behavior.

#[cfg(test)]
mod tests {
    use std::fs;

    fn production_source(source: &str) -> &str {
        source.split("#[cfg(test)]").next().unwrap_or(source)
    }

    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("missing function signature {signature}"));
        let source = &source[start..];
        let body_start = source.find('{').expect("function body open brace");
        let mut depth = 0usize;

        for (offset, character) in source[body_start..].char_indices() {
            match character {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[body_start..body_start + offset + 1];
                    }
                }
                _ => {}
            }
        }

        panic!("unterminated function body for {signature}");
    }

    #[test]
    fn unified_search_module_does_not_call_file_search_processes() {
        let source = fs::read_to_string("src/scripts/search/unified.rs")
            .expect("read src/scripts/search/unified.rs");
        let production = production_source(&source);

        for forbidden in ["mdfind", "search_files(", "search_files_streaming"] {
            assert!(
                !production.contains(forbidden),
                "unified search should not call file search process APIs directly: {forbidden}"
            );
        }
    }

    #[test]
    fn dedicated_file_search_still_owns_file_search_view_navigation() {
        let view_source = fs::read_to_string("src/render_builtins/file_search.rs")
            .expect("read src/render_builtins/file_search.rs");

        assert!(
            view_source.contains("AppView::FileSearchView"),
            "dedicated File Search view should remain a distinct browser surface"
        );
        assert!(
            view_source.contains("Double-click: browse directory inline or open file")
                && view_source.contains("Tab/Shift+Tab handled by intercept_keystrokes"),
            "dedicated File Search should keep directory browsing and parent navigation"
        );
    }

    #[test]
    fn root_streaming_search_disables_filesystem_fallback() {
        let source = fs::read_to_string("src/file_search/mdfind.rs")
            .expect("read src/file_search/mdfind.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("pub fn root_search() -> Self { Self { skip_metadata: true, allow_filesystem_fallback: false"),
            "root search options should skip metadata and disable filesystem fallback"
        );
        assert!(
            normalized
                .contains("SearchFilesStreamingOptions::dedicated_file_search(skip_metadata)"),
            "existing streaming entry point should preserve dedicated File Search defaults"
        );
    }

    #[test]
    fn script_list_automation_reads_grouped_visible_rows() {
        let collect_source = fs::read_to_string("src/app_layout/collect_elements.rs")
            .expect("read src/app_layout/collect_elements.rs");
        let prompt_source = fs::read_to_string("src/prompt_handler/mod.rs")
            .expect("read src/prompt_handler/mod.rs");

        assert!(
            collect_source.contains("script_list_visible_row_labels_from_cache")
                && collect_source.contains("cached_grouped_results_snapshot()")
                && collect_source.contains("SearchResult::File"),
            "getElements should expose ScriptList grouped rows, including root file results"
        );
        assert!(
            prompt_source.contains("self.get_grouped_results_cached();")
                && prompt_source.contains("self.script_list_visible_row_labels_from_cache()"),
            "getState should refresh grouped rows before reporting ScriptList visible rows"
        );
    }

    #[test]
    fn root_file_search_receive_loop_handles_cancel_and_disconnect() {
        let source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains(
                "loop { if cancel.load(std::sync::atomic::Ordering::Relaxed) { return; }"
            ),
            "root file receive loop should keep honoring cancellation after the worker starts"
        );
        assert!(
            normalized.contains("Err(std::sync::mpsc::TryRecvError::Disconnected) => break"),
            "root file receive loop should exit if the worker channel disconnects before Done"
        );
    }

    #[test]
    fn root_unified_search_config_gates_provider_and_grouping() {
        let config_source =
            fs::read_to_string("src/config/types.rs").expect("read src/config/types.rs");
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let filtering_source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");

        assert!(
            config_source.contains("pub struct UnifiedSearchConfig")
                && config_source.contains("pub struct UnifiedSearchFilesConfig")
                && config_source.contains("pub enum RootFilePromotionConfig")
                && config_source.contains("pub fn root_file_section_options(&self)"),
            "config.ts should expose real unifiedSearch.files controls that map to root file section options"
        );
        assert!(
            root_source.contains("root_file_options.files_enabled")
                && root_source.contains("root_file_options.global_search_enabled")
                && root_source.contains("root_file_options.directory_browse_enabled")
                && root_source.contains("options.recent_files_enabled"),
            "root provider startup and recent hydration should honor unifiedSearch.files gates before launching work"
        );
        assert!(
            filtering_source.contains("let unified_search = self.config.get_unified_search();")
                && filtering_source.contains("unified_search.root_file_section_options()")
                && filtering_source.contains(
                    "get_grouped_results_with_validation_query_and_root_files_with_options"
                ),
            "main menu grouping should pass config-derived root file options into the Files section"
        );
        assert!(
            grouping_source.contains("!options.files_enabled")
                && grouping_source.contains("!options.recent_files_enabled")
                && grouping_source.contains("!options.global_search_enabled")
                && grouping_source.contains("!options.directory_browse_enabled"),
            "grouping should keep a defensive config gate even if provider state is stale"
        );
    }

    #[test]
    fn async_root_file_updates_preserve_selection_by_stable_key() {
        let state_source =
            fs::read_to_string("src/main_sections/app_state.rs").expect("read app_state.rs");
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");

        assert!(
            state_source.contains("struct MainMenuSelectionSnapshot")
                && state_source.contains("main_menu_selection_snapshot")
                && state_source.contains("restore_main_menu_selection_from_snapshot")
                && state_source.contains("stable_selection_key()")
                && state_source.contains("grouped_index_for_stable_selection_key"),
            "main menu selection preservation should snapshot and restore stable result keys, not grouped indexes"
        );
        assert!(
            root_source.contains("let selection_before =")
                && root_source.contains("self.main_menu_selection_snapshot()")
                && root_source.contains("self.restore_main_menu_selection_from_snapshot(snapshot)")
                && root_source.contains("self.sync_list_state_for_filter_replacement();")
                && root_source.contains("self.validate_selection_bounds(cx);")
                && root_source.contains("schedule_main_list_selection_reveal_above_footer(")
                && root_source.contains("root_file_active_publish_deferred"),
            "async root file publish path should rebuild rows, restore the previous key, validate bounds, then schedule the selected row reveal above the footer after viewport measurement"
        );
    }

    #[test]
    fn root_file_source_chip_pages_on_near_bottom_selection() {
        let filtering_source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");
        let movement_source = fs::read_to_string("src/app_navigation/impl_movement.rs")
            .expect("read src/app_navigation/impl_movement.rs");
        let file_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");

        assert!(
            file_source.contains("ROOT_FILE_SOURCE_CHIP_INITIAL_VISIBLE_ROWS")
                && file_source.contains("ROOT_FILE_SOURCE_CHIP_PAGE_SIZE"),
            "explicit Files source-chip paging should have separate initial and incremental budgets"
        );
        let normalized_filtering = filtering_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            filtering_source.contains("root_file_source_chip_visible_limit_for(")
                && normalized_filtering
                    .contains("root_file_options.source_chip_visible_limit = Some(visible_limit)")
                && normalized_filtering.contains(
                    "root_file_options.source_filter_browse_target_visible_rows = Some(visible_limit)"
                ),
            "explicit Files source filters should pass the current page budget into search and browse grouping"
        );
        assert!(
            movement_source.contains("fn maybe_expand_root_file_source_chip_page")
                && movement_source.contains("const PRELOAD_THRESHOLD: usize = 3")
                && movement_source.contains("ROOT_FILE_SOURCE_CHIP_PAGE_SIZE")
                && movement_source.contains("restore_main_menu_selection_from_snapshot(snapshot)")
                && movement_source.contains("self.sync_list_state();")
                && movement_source.contains("schedule_main_list_selection_reveal_above_footer")
                && movement_source.contains(
                    "reveal_main_list_selection_above_footer(\"root_file_source_chip_page_expand\")"
                ),
            "selection near the bottom of explicit Files rows should increment the page, sync the ListState row count, preserve selection, and reveal it above the footer after viewport measurement"
        );
    }

    #[test]
    fn root_source_filter_lazy_scroll_proof_is_state_first() {
        let script = fs::read_to_string("scripts/agentic/root-source-filter-lazy-scroll.ts")
            .expect("read lazy scroll proof script");

        assert!(
            script.contains("mainListScroll")
                && script.contains("selectedRowVisible")
                && script.contains("selectedRowAboveFooter")
                && script.contains("getState")
                && script.contains("getElements"),
            "lazy scroll proof should assert state and element receipts for selected-row visibility"
        );
        assert!(
            !script.contains("captureScreenshot") && !script.contains("simulateClick"),
            "lazy scroll proof should stay state-first and not rely on screenshots or mouse clicks"
        );
    }

    #[test]
    fn files_source_filter_one_character_path_is_not_hard_coded_in_app_layers() {
        for path in [
            "src/app_impl/filter_input_core.rs",
            "src/app_impl/root_file_search.rs",
            "src/scripts/grouping.rs",
        ] {
            let source =
                fs::read_to_string(path).unwrap_or_else(|err| panic!("read {path}: {err}"));
            assert!(
                !source.contains("new_text == \"f:s\"") && !source.contains("filter_text == \"f:s\""),
                "{path} should route one-character Files source filters through parser/intent contracts, not f:s string checks"
            );
            assert!(
                !source.contains("No files match"),
                "{path} should not hide the f:s regression behind a hard-coded no-result message"
            );
        }
    }

    #[test]
    fn root_global_file_search_caches_results_without_active_frame_publish() {
        let source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            !normalized.contains("publish_partial_results"),
            "root global file search should not stream partial batches into the active root frame"
        );
        assert!(
            normalized.contains("Ok(crate::file_search::SearchEvent::Result(result)) =>")
                && !normalized.contains("app.apply_root_file_search_results_for_generation( generation, snapshot, true, false, cx, );"),
            "SearchEvent::Result should collect into the batch without publishing a visible partial frame"
        );
        assert!(
            normalized.contains("Ok(crate::file_search::SearchEvent::Done) => break")
                && normalized.contains("app.cache_root_file_search_results_for_generation( generation, request_cache_key, batch, true, );"),
            "final global root file update should warm the cache instead of mutating the active frame"
        );
        assert!(
            normalized.contains("let publish_active_results =")
                && normalized.contains(
                    "matches!(&request, RootFileSearchRequest::DirectoryBrowse { .. })"
                )
                && normalized.contains("if publish_active_results"),
            "only explicit directory browse may publish its final collected rows into the active frame"
        );
    }

    #[test]
    fn root_file_ranking_stays_local_and_does_not_start_searches() {
        let source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let rank_source = source
            .split("pub fn rank_root_file_results(")
            .nth(1)
            .and_then(|section| section.split("/// Payload for file drag-out").next())
            .expect("rank_root_file_results source should be present");

        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !rank_source.contains(forbidden),
                "root ranking should only rank already-returned rows, not start searches: {forbidden}"
            );
        }
        assert!(
            rank_source.contains("file.name") && rank_source.contains("file.path"),
            "root ranking should continue scoring the existing FileResult name/path fields"
        );
    }

    #[test]
    fn root_file_directory_context_ranking_stays_bounded_and_below_filename_first() {
        let source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let production = production_source(&source);
        let provider_source = production
            .split("pub fn root_file_provider_query_for_user_query(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("/// Returns true when the root launcher")
                    .next()
            })
            .expect("root provider query builder should be present");
        let rank_source = production
            .split("pub fn rank_root_file_results(")
            .nth(1)
            .and_then(|section| section.split("/// Payload for file drag-out").next())
            .expect("rank_root_file_results source should be present");

        assert!(
            production.contains("const ROOT_FILE_PATH_CONTEXT_TIER: i32 = 3")
                && production.contains("const ROOT_FILE_PATH_CONTEXT_MAX_TERMS: usize = 4"),
            "directory-context ranking should stay below filename token tier 4 and bounded by term count"
        );
        assert!(
            provider_source.contains("root_file_path_context_mdquery_branches(&terms)")
                && provider_source.contains("kMDItemPath ==")
                && provider_source.contains("kMDItemFSName ==")
                && provider_source.contains("root_file_query_has_safe_global_length(term)"),
            "provider query should add bounded path+filename branches only for safe parent terms"
        );
        assert!(
            rank_source.contains("root_file_path_context_matches_query(file, &q)")
                && rank_source.contains("ROOT_FILE_PATH_CONTEXT_TIER")
                && rank_source.contains("root_file_name_relevance_tier(&file.name, &q, name_matched).max"),
            "root ranking should apply directory-context relevance without replacing filename-first relevance"
        );
    }

    #[test]
    fn root_file_renderer_uses_file_type_specific_svg_icons() {
        let source = fs::read_to_string("src/designs/core/render.rs")
            .expect("read src/designs/core/render.rs");
        let file_arm = source
            .split("SearchResult::File(fm) =>")
            .nth(1)
            .and_then(|section| section.split("SearchResult::Note").next())
            .expect("SearchResult::File arm should be present");

        assert!(
            source.contains("fn root_file_type_svg_icon(")
                && source.contains("FileType::Directory => \"FolderOpen\"")
                && source.contains("FileType::Image => \"file-image\"")
                && source.contains("FileType::Document => \"file-text\"")
                && source.contains("FileType::Audio => \"file-audio\"")
                && source.contains("FileType::Video => \"file-video\"")
                && source.contains("FileType::Application => \"package\""),
            "root file SVG icon mapping should live in a small named helper with type-specific icons"
        );
        assert!(
            file_arm.contains("root_file_type_svg_icon(fm.file.file_type)"),
            "root file rows should derive their SVG icon from FileResult.file_type"
        );
        assert!(
            !file_arm.contains("IconKind::Svg(\"File\".to_string())"),
            "root file rows should no longer hardcode the generic File icon"
        );
        assert!(
            !file_arm.contains("IconKind::Image")
                && !file_arm.contains("is_thumbnail_preview_supported"),
            "root launcher file rows should stay on static SVG icons, not thumbnails"
        );
    }

    #[test]
    fn root_file_handoff_row_uses_existing_search_files_fallback() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let types_source =
            fs::read_to_string("src/scripts/types.rs").expect("read src/scripts/types.rs");
        let builtins_source = fs::read_to_string("src/fallbacks/builtins.rs")
            .expect("read src/fallbacks/builtins.rs");

        assert!(
            builtins_source.contains("pub const SEARCH_FILES_FALLBACK_ID: &str = \"search-files\""),
            "Search Files fallback id should be exported instead of repeated as an inline literal"
        );
        assert!(
            grouping_source.contains("fn root_file_search_handoff_result(")
                && grouping_source.contains("SEARCH_FILES_FALLBACK_ID")
                && grouping_source.contains("Search Files for \\\"{query}\\\"")
                && grouping_source.contains("Open full File Search")
                && grouping_source.contains("SearchResult::Fallback("),
            "root file grouping should append a synthetic fallback row that opens the dedicated File Search view"
        );
        assert!(
            types_source.contains("title_override: Option<String>")
                && types_source.contains("description_override: Option<String>")
                && types_source.contains("with_display_overrides(")
                && types_source.contains("pub fn display_label(&self) -> String")
                && types_source.contains("pub fn display_description(&self) -> String"),
            "fallback matches should support dynamic display text without leaking static strings"
        );
        assert!(
            fs::read_to_string("src/app_layout/collect_elements.rs")
                .expect("read src/app_layout/collect_elements.rs")
                .contains("scripts::SearchResult::Fallback(m) => m.display_label()"),
            "automation element labels should expose the handoff row's dynamic title"
        );
    }

    #[test]
    fn root_file_handoff_row_does_not_start_file_search_processes() {
        let source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let handoff_source = source
            .split("fn root_file_search_handoff_result(")
            .nth(1)
            .and_then(|section| section.split("/// Incomplete menu-syntax hint row.").next())
            .expect("root_file_search_handoff_result source should be present");

        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !handoff_source.contains(forbidden),
                "root file handoff row should only reuse the fallback execution path, not start searches: {forbidden}"
            );
        }
    }

    #[test]
    fn root_file_handoff_row_groups_after_files_before_fallbacks() {
        let source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let append_source = source
            .split("fn append_root_file_section(")
            .nth(1)
            .and_then(|section| section.split("fn root_file_search_handoff_result(").next())
            .expect("append_root_file_section source should be present");

        let section_offset = append_source
            .find("ui_state.section_label.clone()")
            .expect("Files section header should be inserted");
        let file_offset = append_source
            .find("flat_results.push(SearchResult::File(file_match));")
            .expect("actual file rows should be inserted");
        let handoff_offset = append_source
            .find("flat_results.push(handoff);")
            .expect("handoff row should be inserted after actual file rows");
        let splice_offset = append_source
            .find("grouped.splice(insertion_index..insertion_index, file_group);")
            .expect("Files group should still be spliced before fallback rows");

        assert!(
            section_offset < file_offset && file_offset < handoff_offset && handoff_offset < splice_offset,
            "Files section should render real file rows, then the handoff row, before the group is inserted ahead of fallbacks"
        );
        assert!(
            append_source
                .contains("let handoff = if suppress_handoff")
                && append_source.contains("root_file_search_handoff_result(filter_text, mode, options.query_intent, &ui_state)")
                && append_source.contains("files.is_empty() && handoff.is_none()"),
            "Files section should still appear with the handoff row when Spotlight returns zero file rows"
        );
    }

    #[test]
    fn root_global_file_promotion_is_policy_gated_and_grouping_only() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let production = production_source(&grouping_source);
        let helper = production
            .split("fn root_file_section_should_promote(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn root_file_section_insertion_index(")
                    .next()
            })
            .expect("root file promotion helper should be present");

        assert!(
            helper.contains("RootFilePromotionPolicy::Never")
                && helper.contains("RootFileSectionMode::GlobalQuery")
                && helper.contains("filter_text.trim()")
                && helper.contains("root_file_global_query_is_eligible(query)")
                && helper.contains("flat_results.iter().any(is_primary_launcher_result)")
                && helper.contains("files.first()")
                && helper.contains("RootFilePromotionPolicy::ExactFilenameOnly")
                && helper.contains("root_file_name_exact_or_stem_matches_query("),
            "root file promotion should default to never and require exact filename/stem opt-in without primary launcher rows"
        );
        assert!(
            production.contains("root_file_section_insertion_index(grouped, flat_results, promote)")
                && production.contains("SearchResult::ScriptIssue(_)"),
            "promotion should route through a grouping insertion helper while preserving the script issue row"
        );
        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !helper.contains(forbidden),
                "root file promotion must not start providers: {forbidden}"
            );
        }
    }

    #[test]
    fn root_global_promotion_uses_exact_gate_while_recent_seeds_keep_token_gate() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let grouping_production = production_source(&grouping_source);

        assert!(
            file_search_production.contains("pub fn root_file_name_token_matches_query(")
                && file_search_production.contains("root_file_name_seed_matches_query")
                && file_search_production.contains("root_file_recent_seed_matches_query")
                && file_search_production
                    .contains("root_file_name_token_matches_query(name, query)"),
            "recent seed eligibility should preserve the shared filename-token gate"
        );

        let promote_helper = grouping_production
            .split("fn root_file_section_should_promote(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn root_file_section_insertion_index(")
                    .next()
            })
            .expect("root file promotion helper should be present");
        assert!(
            promote_helper.contains("RootFileSectionMode::GlobalQuery")
                && promote_helper.contains("root_file_global_query_is_eligible(query)")
                && promote_helper.contains("flat_results.iter().any(is_primary_launcher_result)")
                && promote_helper.contains("root_file_name_exact_or_stem_matches_query("),
            "promotion should be stricter than recent seeds: exact filename/stem only and blocked by primary launcher rows"
        );
        assert!(
            !promote_helper.contains("ROOT_FILE_MIN_QUERY_CHARS"),
            "promotion should share root file global query eligibility instead of keeping a stale raw length check"
        );
        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !promote_helper.contains(forbidden),
                "root file promotion must not start providers: {forbidden}"
            );
        }
    }

    #[test]
    fn root_recent_files_share_global_file_eligibility() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let grouping_production = production_source(&grouping_source);

        let hydrate_body = file_search_production
            .split("pub fn file_result_from_existing_path(")
            .nth(1)
            .and_then(|section| section.split("/// Convert directory browse results").next())
            .expect("file_result_from_existing_path body should be present");
        assert!(
            hydrate_body.contains("root_global_file_result_is_eligible(&result)"),
            "recent file hydration should use the shared global root file eligibility gate"
        );

        let recent_body = grouping_production
            .split("fn append_recent_root_file_section(")
            .nth(1)
            .and_then(|section| section.split("fn append_root_file_section(").next())
            .expect("append_recent_root_file_section body should be present");
        assert!(
            recent_body.contains("root_global_file_result_is_eligible(file)"),
            "empty-root Recent Files should defensively filter app bundles and bundle contents"
        );
    }

    #[test]
    fn root_file_token_gate_supports_developer_filename_boundaries() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let file_search_production = production_source(&file_search_source);
        let token_helper = file_search_production
            .split("fn contains_at_root_file_token_boundary(")
            .nth(1)
            .and_then(|section| section.split("fn is_root_file_boundary_char(").next())
            .expect("root filename token-boundary helper should be present");
        let boundary_helper = file_search_production
            .split("fn is_root_file_token_boundary_at(")
            .nth(1)
            .and_then(|section| section.split("fn is_root_file_boundary_char(").next())
            .expect("root token boundary classifier should be present");

        assert!(
            file_search_production.contains("contains_at_root_file_token_boundary(name, query)")
                && file_search_production
                    .contains("fn root_file_name_token_matches_single_term(")
                && file_search_production
                    .contains("root_file_name_token_matches_query(name, query)"),
            "ranking, promotion, and recent seed gates should all share the developer filename token helper"
        );
        assert!(
            token_helper.contains("haystack.to_lowercase()")
                && token_helper.contains("needle.to_lowercase()")
                && token_helper.contains("is_root_file_token_boundary_at(haystack, idx)"),
            "token matching should stay case-insensitive while preserving original casing for boundary classification"
        );
        assert!(
            boundary_helper.contains("previous.is_ascii_lowercase() && current.is_ascii_uppercase()")
                && boundary_helper.contains("previous.is_ascii_digit() && current.is_ascii_alphabetic()")
                && boundary_helper
                    .contains("previous.is_ascii_uppercase()")
                && boundary_helper
                    .contains("next.is_some_and(|ch| ch.is_ascii_lowercase())"),
            "root file token boundaries should include camel-case, digit-to-word, and acronym-to-word filename transitions"
        );
        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !token_helper.contains(forbidden) && !boundary_helper.contains(forbidden),
                "developer filename token matching must not start providers: {forbidden}"
            );
        }
    }

    #[test]
    fn root_global_multiword_provider_and_token_gate_stay_shared() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let root_production = production_source(&root_source);
        let root_normalized = root_production
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let grouping_production = production_source(&grouping_source);

        assert!(
            file_search_production.contains("pub fn root_file_provider_query_for_user_query(")
                && file_search_production.contains("fn root_file_query_terms(")
                && file_search_production.contains("root_file_text_matches_terms_in_order")
                && file_search_production
                    .contains("root_file_name_token_matches_single_term(name, &terms[0])"),
            "root file token matching should share one query-term parser across single and multi-word gates"
        );
        assert!(
            root_normalized.contains("root_file_provider_query_for_user_query(&query)")
                && root_normalized.contains("search_files_streaming_with_options( &provider_query,"),
            "root global provider search should expand safe multi-word filename queries before launching Spotlight"
        );

        let promote_helper = grouping_production
            .split("fn root_file_section_should_promote(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn root_file_section_insertion_index(")
                    .next()
            })
            .expect("root file promotion helper should be present");
        assert!(
            promote_helper.contains("root_file_name_exact_or_stem_matches_query("),
            "multi-word promotion should stay exact-only instead of using the broader filename-token gate"
        );
        assert!(
            file_search_production.contains("root_file_name_token_matches_query(name, query)"),
            "recent seeds should continue to delegate to the shared filename-token gate"
        );
    }

    #[test]
    fn root_global_short_digit_queries_share_search_and_promotion_eligibility() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let grouping_production = production_source(&grouping_source);

        assert!(
            file_search_production.contains("pub fn root_file_global_query_is_eligible(")
                && file_search_production.contains("fn root_file_query_has_safe_global_length(")
                && file_search_production.contains("fn root_file_short_digit_token_query(")
                && file_search_production.contains("root_file_global_query_is_eligible(query)"),
            "root file search should expose one shared global eligibility helper with a narrow short-digit exception"
        );

        let promote_helper = grouping_production
            .split("fn root_file_section_should_promote(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn root_file_section_insertion_index(")
                    .next()
            })
            .expect("root file promotion helper should be present");
        assert!(
            promote_helper.contains("root_file_global_query_is_eligible(query)")
                && promote_helper.contains("flat_results.iter().any(is_primary_launcher_result)")
                && promote_helper.contains("root_file_name_exact_or_stem_matches_query("),
            "promotion should share root global search eligibility before applying exact filename-only promotion"
        );
        assert!(
            !promote_helper.contains("ROOT_FILE_MIN_QUERY_CHARS"),
            "promotion must not keep a stale raw min-length gate after short digit tokens become eligible"
        );
    }

    #[test]
    fn root_global_file_promotion_respects_primary_launcher_rows() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let production = production_source(&grouping_source);
        let launcher_helper = production
            .split("fn is_primary_launcher_result(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn root_file_section_insertion_index(")
                    .next()
            })
            .expect("primary launcher helper should be present");

        assert!(
            launcher_helper.contains("SearchResult::Script(_)")
                && launcher_helper.contains("SearchResult::Scriptlet(_)")
                && launcher_helper.contains("SearchResult::Skill(_)")
                && launcher_helper.contains("SearchResult::BuiltIn(_)")
                && launcher_helper.contains("SearchResult::App(_)")
                && launcher_helper.contains("SearchResult::Window(_)"),
            "file promotion should be blocked by any primary launcher row"
        );
        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !launcher_helper.contains(forbidden),
                "primary launcher guard must stay grouping-only: {forbidden}"
            );
        }
    }

    #[test]
    fn root_directory_browse_provider_stays_in_app_layer() {
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let filtering_source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");
        let grouping_production = production_source(&grouping_source);

        assert!(
            root_source.contains("RootFileSearchRequest::DirectoryBrowse")
                && root_source.contains("list_directory_with_options(")
                && root_source.contains("ROOT_FILE_BROWSE_SOURCE_LIMIT"),
            "root directory browsing should collect directory rows in the root file app/provider layer"
        );
        assert!(
            grouping_production.contains("RootFileSectionMode::DirectoryBrowse")
                && grouping_production.contains("root_directory_file_matches(")
                && grouping_production.contains("root_directory_browse_child_filter(filter_text)")
                && grouping_production.contains("ROOT_FILE_BROWSE_RENDER_LIMIT"),
            "grouping should render already-collected directory rows without starting providers"
        );
        for forbidden in ["std::fs::read_dir", "list_directory_with_options("] {
            assert!(
                !grouping_production.contains(forbidden),
                "grouping should not start directory providers directly: {forbidden}"
            );
        }
        assert!(
            filtering_source.contains("self.root_search.root_file_search_mode")
                && filtering_source.contains("self.root_search.root_file_search_loading")
                && (filtering_source.contains("&self.root_search.root_file_results")
                    || filtering_source
                        .contains("self.root_search.root_file_results.as_slice()")),
            "filtering cache should pass the root file source mode, loading state, and collected rows"
        );
    }

    #[test]
    fn root_file_match_mode_state_drives_files_section_header() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let filtering_source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");

        assert!(
            file_search_source.contains("pub enum RootFileInlineMatchMode")
                && file_search_source.contains("Files · Phrase match")
                && file_search_source.contains("Files · Word match")
                && file_search_source.contains("Files · Folder"),
            "root file display labels should be modeled as deterministic match modes"
        );
        assert!(
            grouping_source.contains("struct RootFileSectionUiState")
                && grouping_source.contains("root_file_inline_match_mode_for_query")
                && grouping_source.contains("ui_state.section_label.clone()")
                && grouping_source.contains("ui_state.handoff_subtitle.clone()"),
            "root Files section header and handoff subtitle should share one derived UI state"
        );
        assert!(
            !grouping_source.contains("Files · Searching...")
                && !grouping_source.contains("Files · Loading folder..."),
            "ordinary root Files headers should not expose provider lifecycle as match mode copy"
        );
        assert!(
            filtering_source.contains("self.root_search.root_file_search_loading"),
            "filtering cache should still pass visible loading state into grouping receipts/status"
        );
    }

    #[test]
    fn root_global_recent_file_seed_stays_grouping_only() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let production = production_source(&grouping_source);
        let helper = production
            .split("fn merge_root_global_file_results_with_recent(")
            .nth(1)
            .and_then(|section| section.split("struct RootFileSectionUiState").next())
            .expect("recent merge helper should be present");

        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !helper.contains(forbidden),
                "recent seed merge must not start providers: {forbidden}"
            );
        }
        assert!(
            production.contains("root_recent_file_results")
                && production.contains("merge_root_global_file_results_with_recent(")
                && production.contains("root_file_recent_seed_matches_query")
                && production.contains("RootFileSectionMode::DirectoryBrowse"),
            "root grouping should pass eligible recent files into global ranking without changing directory browse mode"
        );
    }

    #[test]
    fn root_global_recent_file_seed_filters_path_only_matches() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let production = production_source(&grouping_source);
        let helper = production
            .split("fn merge_root_global_file_results_with_recent(")
            .nth(1)
            .and_then(|section| section.split("struct RootFileSectionUiState").next())
            .expect("recent merge helper should be present");
        let recent_seed_helper = file_search_production
            .split("pub fn root_file_recent_seed_matches_query(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn contains_at_root_file_token_boundary(")
                    .next()
            })
            .expect("recent seed helper should be present");

        assert!(
            helper.contains("filter_text: &str")
                && helper.contains("query_intent: crate::file_search::RootFileQueryIntent")
                && helper.contains("provider_results")
                && helper.contains("recent_results")
                && helper.contains("root_file_recent_seed_matches_query_for_intent("),
            "recent root file seeds should be filtered by the shared recent-seed predicate while provider rows stay unfiltered"
        );
        assert!(
            recent_seed_helper.contains("RootFileQueryIntent::OrdinaryRoot")
                && recent_seed_helper.contains("root_file_global_query_is_eligible_for_intent(query, intent)")
                && recent_seed_helper.contains("root_file_name_seed_matches_query(&file.name, query)")
                && recent_seed_helper.contains("root_file_path_context_matches_query(file, query)"),
            "recent seed eligibility should require a global query and allow filename-token or ordered directory-context matches"
        );
        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !helper.contains(forbidden),
                "recent seed filename filter must not start providers: {forbidden}"
            );
        }
    }

    /// Global-file merging is a pure ranking boundary: starting providers here
    /// would duplicate asynchronous search work during every grouping pass.
    #[test]
    fn root_global_file_merge_does_not_start_providers_or_scan_directories() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let merge_helper = function_body(
            production_source(&grouping_source),
            "fn merge_root_global_file_results_with_recent(",
        );

        for forbidden in [
            "mdfind",
            "search_files(",
            "search_files_streaming",
            "std::process::Command",
            "std::fs::read_dir",
            "list_directory",
        ] {
            assert!(
                !merge_helper.contains(forbidden),
                "global file merging must stay in-memory: {forbidden}"
            );
        }
    }

    #[test]
    fn nonempty_global_root_search_refreshes_recent_file_snapshot() {
        let source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("RootFileSectionMode::GlobalQuery")
                && normalized.contains("self.refresh_root_recent_file_results();"),
            "non-empty global root file search should refresh the frecency-backed recent file snapshot"
        );
    }

    #[test]
    fn root_file_direct_shortcuts_route_through_shared_action_executor() {
        let selection_source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let normalized = selection_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            normalized.contains("pub(crate) fn try_execute_root_file_action_shortcut")
                && normalized.contains("ROOT_FILE_QUICK_LOOK_ACTION_ID")
                && normalized.contains("ROOT_FILE_COPY_PATH_ACTION_ID")
                && normalized.contains("ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID")
                && normalized.contains("selected_root_file_result_owned()")
                && normalized.contains("self.execute_root_file_action(action_id, &file, window, cx)"),
            "root file direct shortcuts should route selected file rows through the shared root-file action executor"
        );

        let simulate_key_source = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
            .expect("read src/app_impl/simulate_key_dispatch.rs");
        let runtime_stdin =
            fs::read_to_string("src/main_entry/runtime_stdin_match_simulate_key.rs")
                .expect("read src/main_entry/runtime_stdin_match_simulate_key.rs");
        assert!(
            simulate_key_source.contains("try_execute_root_file_action_shortcut"),
            "central simulateKey dispatcher should offer direct root-file shortcuts"
        );
        assert!(
            runtime_stdin.contains("dispatch_simulate_key"),
            "stdin runtime entry point should delegate simulateKey to the shared dispatcher"
        );
    }

    #[test]
    fn root_directory_child_fragment_filtering_stays_direct_child_only() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let file_search_production = production_source(&file_search_source);
        let grouping_production = production_source(&grouping_source);

        assert!(
            file_search_production.contains("pub fn root_directory_query_base(")
                && file_search_production.contains("root_file_section_mode_for_query")
                && file_search_production.contains("RootFileSectionMode::DirectoryBrowse")
                && file_search_production.contains("root_directory_file_matches("),
            "root directory child-fragment queries should stay in directory-browse mode"
        );
        assert!(
            grouping_production.contains("fn root_directory_browse_child_filter(")
                && grouping_production.contains("root_directory_query_base(query)")
                && grouping_production.contains("child_filter.as_deref()"),
            "grouping should derive only a child-name filter and pass it to already-collected rows"
        );
        for forbidden in [
            "std::fs::read_dir",
            "list_directory_with_options(",
            "search_files_streaming",
            "mdfind",
        ] {
            assert!(
                !grouping_production.contains(forbidden),
                "filtered root directory grouping should not start providers directly: {forbidden}"
            );
        }
    }

    #[test]
    fn root_directory_child_fragment_edits_reuse_active_provider() {
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let normalized = root_source.split_whitespace().collect::<Vec<_>>().join(" ");
        let active_source_body = function_body(
            &root_source,
            "fn active_root_directory_browse_source_matches(",
        );

        assert!(
            file_search_source.contains("pub fn root_directory_browse_source_key(")
                && file_search_source.contains("parse_directory_path(query)?")
                && file_search_source.contains("Some((parsed.directory, parsed.show_hidden))"),
            "file search should expose a provider key based on directory plus hidden-file mode"
        );
        assert!(
            active_source_body.contains("root_directory_browse_source_key(")
                && active_source_body.contains("&self.root_search.root_file_search_query"),
            "root app layer should compare active directory-browse provider identity separately from the visible query"
        );
        assert!(
            normalized.contains(
                "RootFileSearchRequest::DirectoryBrowse { query, directory, show_hidden, } if self.active_root_directory_browse_source_matches(directory, *show_hidden)"
            ) && normalized.contains("self.root_search.root_file_search_query = query.clone();")
                && normalized.contains("self.refresh_root_file_grouping_after_query_only_change(cx);"),
            "directory child-fragment edits should only update the visible query and regroup cached rows"
        );
        assert!(
            !normalized.contains("root_file_search_query != query_for_task"),
            "directory listings should be allowed to complete after the visible child fragment changes"
        );
    }

    #[test]
    fn root_directory_tab_navigation_has_no_plain_tab_agent_chat_routing() {
        let simulate_key_source = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
            .expect("read src/app_impl/simulate_key_dispatch.rs");
        let runtime_stdin =
            fs::read_to_string("src/main_entry/runtime_stdin_match_simulate_key.rs")
                .expect("read src/main_entry/runtime_stdin_match_simulate_key.rs");
        assert!(
            simulate_key_source.contains("try_navigate_root_file_directory_with_tab"),
            "central simulateKey dispatcher should route root file directory Tab"
        );
        assert!(
            !simulate_key_source.contains("try_route_plain_tab_to_agent_chat_context_capture")
                && !runtime_stdin.contains("try_route_plain_tab_to_agent_chat_context_capture"),
            "ScriptList Tab simulateKey should not route plain Tab to Agent Chat"
        );

        let selection_source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        assert!(
            selection_source.contains("pub(crate) fn try_navigate_root_file_directory_with_tab")
                && selection_source.contains("selected_root_directory_query_owned")
                && selection_source.contains("root_file_parent_query_for_filter")
                && selection_source.contains("set_filter_text_immediate"),
            "ScriptList Tab navigation should be centralized in selection_fallback.rs"
        );
    }

    #[test]
    fn fallback_keeps_window_open_uses_search_files_constant() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let body = source
            .split("fn fallback_keeps_window_open(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("fn should_ignore_main_menu_open_carryover_input")
                    .next()
            })
            .expect("fallback_keeps_window_open source should be present");

        assert!(
            body.contains("SEARCH_FILES_FALLBACK_ID"),
            "search-files fallback window behavior should use the exported id constant"
        );
        assert!(
            !body.contains("\"search-files\""),
            "search-files id should not be repeated as a literal in fallback_keeps_window_open"
        );
    }

    #[test]
    fn fallback_mode_enter_prefers_visible_grouped_fallback_selection() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let body = source
            .split("pub fn execute_selected_fallback(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("/// Execute a built-in fallback action without window reference")
                    .next()
            })
            .expect("execute_selected_fallback source should be present");

        assert!(
            body.contains("self.selected_main_list_search_result_owned()")
                && body.contains("scripts::SearchResult::Fallback(fallback_match)")
                && body.contains("self.execute_fallback_item(&fallback_match.fallback, cx);")
                && body.contains("execute_selected_fallback.no_live_grouped_fallback")
                && !body.contains("main_menu_fallback_state.selected_item()"),
            "fallback-mode Enter should execute only the visible grouped fallback row"
        );
    }

    #[test]
    fn root_file_actions_are_main_list_only() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("read src/app_impl/actions_dialog.rs");
        let simulate_key_source = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
            .expect("read src/app_impl/simulate_key_dispatch.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
        let simulate_key_normalized = simulate_key_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            normalized.contains(
                "ActionsDialogHost::MainList => { if let Some(result) = self.selected_main_list_search_result_owned()"
            ) && normalized.contains("root_unified_action_owner_for_result(&result)")
                && normalized.contains("self.toggle_root_unified_result_actions(subject, window, cx);"),
            "MainList actions should branch to root-file actions for selected root file rows"
        );
        assert!(
            simulate_key_normalized.contains("view.simulate_key_requests_generic_actions_toggle")
                && simulate_key_normalized.contains("view.toggle_actions(ctx, window);"),
            "stdin simulateKey Cmd+K on ScriptList should use the shared dispatcher so root-file rows get their actions"
        );
        assert!(
            normalized.contains("ActionsDialogHost::FileSearch => { let selected = self.selected_file_search_result_owned();")
                && normalized.contains("self.toggle_file_search_actions("),
            "dedicated FileSearch actions should keep using the file-search action route"
        );
    }

    #[test]
    fn root_file_actions_do_not_expand_dedicated_file_search_browser() {
        let view_source = fs::read_to_string("src/render_builtins/file_search.rs")
            .expect("read src/render_builtins/file_search.rs");

        for forbidden in [
            "root_file_open",
            "root_file_reveal_in_finder",
            "root_file_copy_path",
            "root_file_copy_name",
            "root_file_quick_look",
            "root_file_search_in_folder",
            "root_file_browse_parent_folder",
        ] {
            assert!(
                !view_source.contains(forbidden),
                "root file action id should not be introduced into dedicated File Search render/navigation code: {forbidden}"
            );
        }
    }

    #[test]
    fn root_file_open_uses_shared_open_helper() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let actions_dialog_source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("read src/app_impl/actions_dialog.rs");
        let actions_toggle_source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("read src/app_impl/actions_toggle.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");
        let actions_dialog_normalized = actions_dialog_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let actions_toggle_normalized = actions_toggle_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            normalized.contains(
                "scripts::SearchResult::File(file_match) => { self.execute_root_file_open(&file_match.file, cx); }"
            ),
            "Enter on root file rows should call execute_root_file_open instead of inlining open_file"
        );
        assert!(
            normalized.contains("ROOT_FILE_OPEN_ACTION_ID => {")
                && normalized.contains("self.execute_root_file_open(file, cx);"),
            "root_file_open action should share execute_root_file_open with Enter"
        );
        assert!(
            actions_toggle_normalized
                .contains("self.pending_root_file_actions_file = Some(file.clone());"),
            "root file actions should capture the selected file when the palette opens"
        );
        assert!(
            actions_dialog_normalized.contains(
                "let root_file_context = if root_unified_context.is_none() && should_close && matches!(host, ActionsDialogHost::MainList) && crate::action_helpers::is_root_file_action_id(&action_id)"
            ) && actions_dialog_normalized.contains(
                "self.pending_root_file_actions_file .clone() .or_else(|| self.selected_root_file_result_owned())"
            ) && actions_dialog_normalized
                .contains("self.clear_actions_context_for_host(host);"),
            "root file action activation should capture context before close and clear it on MainList close"
        );
    }

    #[test]
    fn root_file_open_records_frecency_after_success_through_shared_open_helper() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        let record_body = function_body(&source, "fn record_root_file_open_use(");
        assert!(
            record_body.contains("root_file_open_frecency_keys(file)")
                && record_body.contains("self.frecency_store.record_use(")
                && record_body.contains("self.frecency_store.save()")
                && record_body.contains("self.invalidate_grouped_cache();"),
            "root-file recording should use the typed frecency-key plan, save, and invalidate"
        );

        let open_body = source
            .split("pub(crate) fn execute_root_file_open(")
            .nth(1)
            .and_then(|section| {
                section
                    .split("pub(crate) fn root_file_search_in_folder_query")
                    .next()
            })
            .expect("execute_root_file_open source should be present");
        let open_call = open_body
            .find("crate::file_search::open_file(&file.path)")
            .expect("execute_root_file_open should call open_file");
        let record_call = open_body
            .find("self.record_root_file_open_use(file);")
            .expect("execute_root_file_open should record frecency after successful open");
        let close_call = open_body
            .find("self.close_and_reset_window(cx);")
            .expect("execute_root_file_open should close after recording");

        assert!(
            open_call < record_call && record_call < close_call,
            "root file frecency should record after a successful OS open and before closing"
        );
        assert!(
            normalized.contains("scripts::SearchResult::File(_) => None"),
            "execute_selected should not pre-record root file frecency before open success"
        );
    }

    #[test]
    fn root_file_actions_prefer_captured_file_over_live_selection() {
        let source = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("read src/app_impl/actions_dialog.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains(
                "if crate::action_helpers::is_root_file_action_id(&action_id) { if let Some(file) = self .pending_root_file_actions_file .clone() .or_else(|| self.selected_root_file_result_owned())"
            ),
            "root file action execution should prefer the captured file over the current live selection"
        );
    }

    #[test]
    fn root_file_action_enter_routes_activation_before_close() {
        let actions_dialog = fs::read_to_string("src/app_impl/actions_dialog.rs")
            .expect("read src/app_impl/actions_dialog.rs");
        let app_view_state = fs::read_to_string("src/main_sections/app_view_state.rs")
            .expect("read src/main_sections/app_view_state.rs");
        let render_script_list = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("read src/render_script_list/mod.rs");
        let stdin_route = fs::read_to_string("src/app_impl/simulate_key_dispatch.rs")
            .expect("read src/app_impl/simulate_key_dispatch.rs");
        let normalized_actions = actions_dialog
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let normalized_state = app_view_state
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let normalized_script_list = render_script_list
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let normalized_stdin = stdin_route.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized_state.contains("Execute { action_id: String, should_close: bool, }"),
            "ActionsRoute::Execute should carry should_close so callers can execute before closing clears captured root subjects"
        );
        assert!(
            normalized_actions.contains("pub(crate) fn execute_actions_route_action")
                && normalized_actions.contains("ActionsDialogActivation::Executed { action_id, should_close, }")
                && normalized_actions.contains("self.handle_actions_dialog_activation("),
            "route executions should reuse the activation handler that captures root context before close"
        );
        assert!(
            normalized_script_list.contains(
                "ActionsRoute::Execute { action_id, should_close, } => { this.execute_actions_route_action( ActionsDialogHost::MainList, action_id, should_close, window, cx,"
            ),
            "MainList physical Enter should execute root-file actions through the activation route"
        );
        assert!(
            normalized_stdin.contains("crate::ActionsRoute::Execute { action_id, should_close, }")
                && normalized_stdin.contains(
                "view.execute_actions_route_action( host, action_id, should_close, window, ctx,"
            ),
            "simulated Enter should use the same activation route as physical Enter"
        );
    }

    #[test]
    fn root_file_actions_context_cleared_by_detached_on_close() {
        let source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("read src/app_impl/actions_toggle.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        let mark_closed = normalized
            .find("app.mark_actions_popup_closed();")
            .expect("detached actions-window on_close should mark popup closed");
        let clear_context = normalized
            .find("app.clear_actions_context_for_host(host);")
            .expect("detached actions-window on_close should clear host context");
        let mark_resync = normalized
            .find("app.mark_filter_resync_after_actions_if_needed();")
            .expect("detached actions-window on_close should mark filter resync");
        assert!(
            mark_closed < clear_context && clear_context < mark_resync,
            "detached actions-window on_close should clear any captured MainList root-file context"
        );
    }

    #[test]
    fn root_file_action_ids_are_reserved() {
        let source =
            fs::read_to_string("src/action_helpers.rs").expect("read src/action_helpers.rs");

        for action_id in [
            "root_file_open",
            "root_file_reveal_in_finder",
            "root_file_copy_path",
            "root_file_copy_name",
            "root_file_quick_look",
            "root_file_search_in_folder",
            "root_file_browse_parent_folder",
        ] {
            assert!(
                source.contains(action_id),
                "root file action id should be reserved: {action_id}"
            );
        }
        assert!(
            source.contains("ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID")
                && source.contains("| ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID"),
            "Search Inside Folder action id should be reserved and recognized as a captured root-file action"
        );
        assert!(
            source.contains("ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID")
                && source.contains("| ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID"),
            "Browse Parent Folder action id should be reserved and recognized as a captured root-file action"
        );
        assert!(
            source.contains("ROOT_FILE_COPY_NAME_ACTION_ID")
                && source.contains("| ROOT_FILE_COPY_NAME_ACTION_ID"),
            "Copy Name action id should be reserved and recognized as a captured root-file action"
        );
    }

    #[test]
    fn root_file_search_in_folder_action_is_directory_only() {
        let source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("read src/app_impl/actions_toggle.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("file.file_type == crate::file_search::FileType::Directory")
                && normalized.contains("if is_dir { actions.push( Action::new( crate::action_helpers::ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID"),
            "Search Inside Folder should only be added for directory root-file rows"
        );
    }

    #[test]
    fn root_file_search_in_folder_action_opens_dedicated_file_search() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID")
                && normalized.contains("ensure_trailing_slash(&file.path)")
                && normalized.contains("self.open_file_search(query, cx);"),
            "root folder action should hand off to dedicated File Search with a trailing-slashed directory path"
        );
    }

    #[test]
    fn root_file_browse_parent_folder_action_is_file_only_and_opens_dedicated_file_search() {
        let actions_source = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("read src/app_impl/actions_toggle.rs");
        let selection_source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let actions_normalized = actions_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let selection_normalized = selection_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            actions_normalized.contains("ROOT_FILE_BROWSE_PARENT_FOLDER_ACTION_ID")
                && actions_normalized.contains("!is_dir")
                && actions_normalized.contains("\"Browse Parent Folder\"")
                && actions_normalized.contains("shorten_path(&parent_query)"),
            "Browse Parent Folder should only be rendered for regular root file rows with a shortened parent display"
        );
        assert!(
            selection_normalized.contains("root_file_browse_parent_folder_query")
                && selection_normalized.contains("parent_folder_search_query(&file.path)")
                && selection_normalized
                    .contains("self.clear_main_list_selection_for_root_file_handoff();")
                && selection_normalized.contains("self.open_file_search(query, cx);"),
            "Browse Parent Folder should clear stale MainList selection and hand off to dedicated File Search at the containing folder"
        );
    }

    #[test]
    fn root_file_quick_look_uses_shared_os_helper_without_file_search_state() {
        let selection_source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let branch = selection_source
            .split("ROOT_FILE_QUICK_LOOK_ACTION_ID =>")
            .nth(1)
            .and_then(|rest| rest.split("ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID =>").next())
            .expect("root-file Quick Look branch");

        assert!(
            branch.contains("crate::file_search::quick_look(&file.path)"),
            "root-file Quick Look should use the shared OS helper against the captured file path"
        );
        for forbidden in [
            "file_search_actions_path",
            "quick_look_entry",
            "record_root_file_open_use",
            "execute_root_file_open",
            "close_and_reset_window",
            "open_file_search",
        ] {
            assert!(
                !branch.contains(forbidden),
                "root-file Quick Look must not use {forbidden}"
            );
        }
    }

    #[test]
    fn root_file_quick_look_helper_checks_missing_path_before_nonblocking_spawn() {
        let source = fs::read_to_string("src/file_search/os_open.rs")
            .expect("read src/file_search/os_open.rs");
        let branch = source
            .split("pub fn quick_look(path: &str) -> Result<(), String> {")
            .nth(1)
            .and_then(|rest| rest.split("/// Show the \"Open With\" dialog").next())
            .expect("quick_look helper body");
        let normalized = branch.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("if !Path::new(path).exists()")
                && normalized.contains("return Err(format!(\"Path does not exist: {}\", path));"),
            "quick_look should return a controlled error for missing paths"
        );
        assert!(
            normalized.contains("Command::new(\"qlmanage\")")
                && normalized.contains(".arg(\"-p\")")
                && normalized.contains(".arg(path)")
                && normalized.contains(".spawn()"),
            "macOS Quick Look should spawn qlmanage directly without shell quoting"
        );
        assert!(
            !normalized.contains(".wait()"),
            "Quick Look should not block waiting for qlmanage to exit"
        );
    }

    #[test]
    fn root_file_parent_folder_handoff_keeps_home_display_shortening_scoped() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let directory_source = fs::read_to_string("src/file_search/directory.rs")
            .expect("read src/file_search/directory.rs");
        let file_search_normalized = file_search_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let directory_normalized = directory_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            file_search_normalized.contains("Some(shorten_path(&ensure_trailing_slash(parent)))"),
            "Browse Parent Folder should hand off with a display-safe parent query"
        );
        assert!(
            directory_normalized.contains("shorten_home_prefix_for_display_with_home")
                && directory_normalized.contains("path == home")
                && directory_normalized.contains("home_with_slash"),
            "home-prefix shortening should require exact home or home path-boundary matches"
        );
    }

    #[test]
    fn root_file_copy_name_action_copies_basename_only() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            normalized.contains("ROOT_FILE_COPY_NAME_ACTION_ID")
                && normalized.contains("gpui::ClipboardItem::new_string(file.name.clone())")
                && normalized.contains("format!(\"Copied name: {}\", file.name)"),
            "Copy Name should copy only FileResult.name and show basename-only HUD feedback"
        );
    }

    #[test]
    fn root_file_copy_path_action_feedback_uses_full_path() {
        let source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");
        let branch = source
            .split("ROOT_FILE_COPY_PATH_ACTION_ID =>")
            .nth(1)
            .and_then(|section| section.split("ROOT_FILE_COPY_NAME_ACTION_ID =>").next())
            .expect("root-file Copy Path branch should be present");

        assert!(
            branch.contains("gpui::ClipboardItem::new_string(file.path.clone())")
                && branch.contains("format!(\"Copied path: {}\", file.path)"),
            "Copy Path should copy the full FileResult.path and show matching full-path HUD feedback"
        );
        assert!(
            !branch.contains("format!(\"Copied path: {}\", file.name)"),
            "Copy Path HUD feedback must not show only the basename while the clipboard receives the full path"
        );
    }

    #[test]
    fn root_recent_files_keep_grouping_search_pure() {
        for path in [
            "src/scripts/grouping.rs",
            "src/scripts/grouping/search_mode.rs",
            "src/scripts/types.rs",
        ] {
            let source = fs::read_to_string(path).unwrap_or_else(|_| panic!("read {path}"));
            let production = production_source(&source);
            for forbidden in [
                "mdfind",
                "search_files(",
                "search_files_streaming",
                "std::process::Command",
                "std::fs::read_dir",
                "list_directory",
            ] {
                assert!(
                    !production.contains(forbidden),
                    "{path} should not start filesystem providers while grouping or ranking: {forbidden}"
                );
            }
        }
    }

    #[test]
    fn root_recent_files_are_file_rows_not_fallbacks() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let recent_source = grouping_source
            .split("fn append_recent_root_file_section(")
            .nth(1)
            .and_then(|section| section.split("fn append_root_file_section(").next())
            .expect("append_recent_root_file_section source should be present");

        assert!(
            recent_source.contains("\"Recent Files\""),
            "empty root recent files should render under a Recent Files section"
        );
        assert!(
            recent_source.contains("flat_results.push(SearchResult::File(")
                && !recent_source.contains("SearchResult::Fallback("),
            "recent root files should be real file rows, not fallback rows"
        );
        assert!(
            !recent_source.contains("root_file_search_handoff_result"),
            "empty recent files should not create the Search Files continuation row"
        );
    }

    #[test]
    fn root_recent_files_hydrate_from_frecency_in_app_layer() {
        let file_search_source =
            fs::read_to_string("src/file_search/mod.rs").expect("read src/file_search/mod.rs");
        let root_source = fs::read_to_string("src/app_impl/root_file_search.rs")
            .expect("read src/app_impl/root_file_search.rs");
        let utility_source = fs::read_to_string("src/app_execute/utility_views.rs")
            .expect("read src/app_execute/utility_views.rs");
        let filtering_source = fs::read_to_string("src/app_impl/filtering_cache.rs")
            .expect("read src/app_impl/filtering_cache.rs");
        let root_normalized = root_source.split_whitespace().collect::<Vec<_>>().join(" ");
        let utility_normalized = utility_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let filtering_normalized = filtering_source
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");

        assert!(
            file_search_source.contains("ROOT_FILE_RECENT_RENDER_LIMIT")
                && file_search_source.contains("ROOT_FILE_RECENT_SEED_LIMIT")
                && file_search_source.contains("ROOT_FILE_RECENT_HYDRATE_LIMIT"),
            "root recent files should separate visible render cap from searchable seed pool"
        );
        assert!(
            utility_normalized.contains("top_file_paths(limit.saturating_mul(3).max(limit))")
                && utility_normalized.contains("file_result_from_existing_path(&path)")
                && utility_normalized.contains("take(limit)")
                && root_normalized.contains("recent_file_results_from_frecency(crate::file_search::ROOT_FILE_RECENT_SEED_LIMIT)")
                && root_normalized
                    .contains("self.root_search.root_recent_file_results = next_results"),
            "recent root files should hydrate known frecency paths into a deeper seed pool in the app layer"
        );
        assert!(
            filtering_normalized.contains("RootFileSectionMode::GlobalQuery")
                && filtering_normalized.contains("self.refresh_root_recent_file_results();")
                && (filtering_normalized.contains("&self.root_search.root_recent_file_results")
                    || filtering_normalized
                        .contains("self.root_search.root_recent_file_results.as_slice()")),
            "empty and non-empty global root grouping should refresh and pass recent file rows explicitly"
        );
    }

    #[test]
    fn root_recent_file_seed_pool_exceeds_empty_render_cap() {
        let grouping_source =
            fs::read_to_string("src/scripts/grouping.rs").expect("read src/scripts/grouping.rs");
        let production = production_source(&grouping_source);
        let recent_source = production
            .split("fn append_recent_root_file_section(")
            .nth(1)
            .and_then(|section| section.split("fn append_root_file_section(").next())
            .expect("append_recent_root_file_section source should be present");
        let merge_helper = production
            .split("fn merge_root_global_file_results_with_recent(")
            .nth(1)
            .and_then(|section| section.split("struct RootFileSectionUiState").next())
            .expect("merge_root_global_file_results_with_recent source should be present");

        assert!(
            recent_source.contains("source_filter_browse_target_visible_rows")
                && recent_source
                    .contains("unwrap_or(crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT)"),
            "empty-root Recent Files should stay capped while explicit source-filter browse can raise the cap"
        );
        assert!(
            !merge_helper.contains("ROOT_FILE_RECENT_RENDER_LIMIT"),
            "non-empty global recent seeds should search the deeper recent pool, not just visible empty-root rows"
        );
        assert!(
            merge_helper.contains("root_file_recent_seed_matches_query_for_intent("),
            "non-empty global recent seeds should keep bounded recent-seed eligibility"
        );
    }

    #[test]
    fn root_launcher_directory_browse_does_not_open_dedicated_file_search_directly() {
        let source = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("read src/render_script_list/mod.rs");
        let selection_source = fs::read_to_string("src/app_impl/selection_fallback.rs")
            .expect("read src/app_impl/selection_fallback.rs");

        assert!(
            !source.contains("ROOT_FILE_SEARCH_IN_FOLDER_ACTION_ID"),
            "root folder handoff should live in the action executor, not ScriptList key handling"
        );
        assert!(
            !source.contains("open_file_search("),
            "ScriptList render/key handling should not directly open File Search for root folder rows"
        );
        assert!(
            selection_source.contains("try_navigate_root_file_directory_with_tab")
                && selection_source.contains("set_filter_text_immediate"),
            "root directory Tab should update the ScriptList query inline instead of opening dedicated File Search"
        );
    }
}
