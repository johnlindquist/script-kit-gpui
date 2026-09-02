#[cfg(test)]
mod script_issue_catalog_tests {
    use super::*;
    use crate::mcp_resources::SdkCapabilityDiagnosticCode;
    use crate::scripts::{
        MetadataField, ScriptValidationIssue, ScriptValidationKind, ValidationSeverity,
    };

    fn capability_issue(severity: ValidationSeverity) -> ScriptValidationIssue {
        ScriptValidationIssue {
            severity,
            path: "/tmp/scriptlet-authoring.md".into(),
            script_name: "Repair Windows".to_string(),
            field: Some(MetadataField::Capability),
            message: "This capability is unavailable.".to_string(),
            kind: ScriptValidationKind::CapabilityUnavailable {
                capability: "moveWindow".to_string(),
                code: if severity == ValidationSeverity::Fatal {
                    SdkCapabilityDiagnosticCode::UnsupportedCapability
                } else {
                    SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable
                },
                alternatives: Vec::new(),
            },
            related: Vec::new(),
        }
    }

    fn report_for_retained(severity: ValidationSeverity) -> ValidationReport {
        ValidationReport {
            schema_version: crate::scripts::validation::VALIDATION_SCHEMA_VERSION,
            total_candidates: 1,
            valid_count: usize::from(severity == ValidationSeverity::Warning),
            fatal_count: usize::from(severity == ValidationSeverity::Fatal),
            warning_count: usize::from(severity == ValidationSeverity::Warning),
            failed_scripts: Arc::from(Vec::new()),
            warnings: Arc::from(Vec::new()),
            retained_issues: Arc::from(vec![capability_issue(severity)]),
        }
    }

    #[test]
    fn retained_fatal_scriptlets_receive_repair_row_without_fake_exclusion() {
        let mut grouped = Vec::new();
        let mut results = Vec::new();
        prepend_script_issues_row(
            &mut grouped,
            &mut results,
            &report_for_retained(ValidationSeverity::Fatal),
            None,
        );

        let SearchResult::ScriptIssue(issue) = &results[0] else {
            panic!("retained fatal command must expose the repair row");
        };
        assert_eq!(issue.title, "Script Issues (1)");
        assert_eq!(issue.failed_count, 0);
        assert_eq!(issue.fatal_count, 1);
        assert!(issue.description.as_ref().unwrap().contains("1 retained"));
        assert!(matches!(grouped[0], GroupedListItem::Item(0)));
    }

    #[test]
    fn pending_permission_warning_alone_still_exposes_repair_row() {
        let mut grouped = Vec::new();
        let mut results = Vec::new();
        prepend_script_issues_row(
            &mut grouped,
            &mut results,
            &report_for_retained(ValidationSeverity::Warning),
            None,
        );

        let SearchResult::ScriptIssue(issue) = &results[0] else {
            panic!("permission-pending command must expose the repair row");
        };
        assert_eq!(issue.failed_count, 0);
        assert_eq!(issue.fatal_count, 0);
        assert_eq!(issue.warning_count, 1);
        assert!(issue.description.as_ref().unwrap().contains("1 warning"));
    }
}

#[cfg(test)]
mod advanced_query_tests {
    use super::*;
    use crate::file_search::{FileResult, FileType};
    use crate::menu_syntax::{parse, AdvancedQuery, MenuSyntaxParse};
    use crate::scripts::types::BuiltInMatch;

    fn issue_row() -> SearchResult {
        SearchResult::ScriptIssue(ScriptIssueMatch {
            title: "Script Issues (1)".into(),
            description: None,
            failed_count: 1,
            fatal_count: 1,
            warning_count: 0,
            score: i32::MAX,
        })
    }

    fn advanced_query_from(raw: &str) -> AdvancedQuery {
        match parse(raw) {
            MenuSyntaxParse::AdvancedQuery(q) => q,
            other => panic!("expected AdvancedQuery for {raw:?}, got {other:?}"),
        }
    }

    /// Audit finding F2: a brain memory must outrank the generic
    /// "Search Files for …" handoff CTA, so when the files section holds
    /// nothing but that CTA the brain section inserts above it. With any
    /// non-CTA result present, brain keeps the default passive position.
    #[test]
    fn brain_insertion_index_promotes_above_cta_only_files_section() {
        let search_files = crate::fallbacks::builtins::get_builtin_fallbacks()
            .into_iter()
            .find(|f| f.id == crate::fallbacks::builtins::SEARCH_FILES_FALLBACK_ID)
            .expect("search files fallback");
        let handoff = SearchResult::Fallback(
            FallbackMatch::new(
                crate::fallbacks::FallbackItem::Builtin(search_files.clone()),
                0,
            )
            .with_stable_selection_key("fallback/root-file-search-handoff/global"),
        );
        let plain_fallback = SearchResult::Fallback(FallbackMatch::new(
            crate::fallbacks::FallbackItem::Builtin(search_files),
            0,
        ));

        // CTA-only files section: brain inserts above its header (index 0).
        let flat = vec![handoff.clone()];
        let grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        assert_eq!(root_brain_passive_insertion_index(&grouped, &flat), 0);

        // Section with a non-CTA row keeps the default (append) position.
        let flat = vec![handoff, plain_fallback];
        let grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];
        assert_eq!(
            root_brain_passive_insertion_index(&grouped, &flat),
            grouped.len()
        );
    }

    #[test]
    fn rejects_issue_under_type_script_predicate() {
        let query = advanced_query_from(":type:script git");
        assert!(advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn allows_issue_under_type_issue_predicate() {
        let query = advanced_query_from(":type:issue");
        assert!(!advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn no_advanced_query_never_rejects_issue() {
        assert!(!advanced_query_rejects_issue(None));
    }

    #[test]
    fn empty_predicates_never_reject_issue() {
        // Grammar pivot (2026-06): a bare `: git` no longer parses as an
        // AdvancedQuery — the leading-colon form stays Incomplete
        // (BareQueryPrefix) until a concrete filter is chosen. A source-filter
        // query like `f: git` is the current spelling that still yields an
        // AdvancedQuery with empty predicates, which is what this invariant
        // is about: no predicates means the ScriptIssue row is never rejected.
        let query = advanced_query_from("f: git");
        assert!(query.predicates.is_empty());
        assert!(!advanced_query_rejects_issue(Some(&query)));
    }

    #[test]
    fn apply_advanced_query_drops_issue_with_type_script() {
        let query = advanced_query_from(":type:script git");
        let results = vec![issue_row()];
        let filtered = crate::menu_syntax::apply_advanced_query(results, &query);
        assert!(
            filtered.is_empty(),
            ":type:script must not leak a ScriptIssue row through grouping"
        );
    }

    #[test]
    fn apply_advanced_query_keeps_issue_with_type_issue() {
        let query = advanced_query_from(":type:issue");
        let results = vec![issue_row()];
        let filtered = crate::menu_syntax::apply_advanced_query(results, &query);
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn pin_alias_match_first_moves_existing_result_to_top() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Main".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
            None,
        );

        assert!(
            matches!(grouped.first(), Some(GroupedListItem::Item(1))),
            "alias target must be the first grouped entry, got {grouped:?}"
        );
        assert_eq!(
            flat.len(),
            2,
            "no synthetic result when target already present"
        );
    }

    #[test]
    fn pin_alias_match_first_preserves_leading_results_header() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Results".to_string(), None),
            GroupedListItem::Item(0),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
            None,
        );

        assert!(matches!(
            grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Results"
        ));
        assert!(
            matches!(grouped.get(1), Some(GroupedListItem::Item(1))),
            "alias target must be pinned under Results, got {grouped:?}"
        );
    }

    #[test]
    fn pin_alias_match_first_inserts_fallback_when_target_missing() {
        let mut flat = vec![builtin_result("Other Command")];
        let mut grouped = vec![GroupedListItem::Item(0)];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
            None,
        );

        assert_eq!(flat.len(), 2, "fallback result must be appended");
        assert!(
            matches!(grouped.first(), Some(GroupedListItem::Item(1))),
            "fallback alias target must be pinned first, got {grouped:?}"
        );
        assert!(
            matches!(flat[1], SearchResult::BuiltIn(ref bm) if bm.entry.id == "builtin/aliased-command")
        );
    }

    #[test]
    fn pin_alias_match_first_drops_orphaned_section_header() {
        let mut flat = vec![
            builtin_result("Other Command"),
            builtin_result("Aliased Command"),
        ];
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Lonely".to_string(), None),
            GroupedListItem::Item(1),
        ];

        pin_alias_match_first(
            &mut grouped,
            &mut flat,
            &|result| matches!(result, SearchResult::BuiltIn(bm) if bm.entry.id == "builtin/aliased-command"),
            &|| builtin_result("Aliased Command"),
            None,
        );

        assert!(matches!(grouped.first(), Some(GroupedListItem::Item(1))));
        assert!(
            !grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, _) if label == "Lonely"
            )),
            "header left without rows must be dropped, got {grouped:?}"
        );
    }

    #[test]
    fn query_predicates_suppress_issue_row_prepend() {
        // Deep proof that `:type:script <text>` never prepends an issue row even
        // when validation reports failed scripts. We inspect the shared helper
        // to avoid spinning up a full frecency/scripts fixture.
        let query = advanced_query_from(":type:script something");
        assert!(advanced_query_rejects_issue(Some(&query)));
    }

    fn root_file(path: &str, name: &str) -> FileResult {
        root_file_with_type(path, name, FileType::Document)
    }

    fn root_file_with_type(path: &str, name: &str, file_type: FileType) -> FileResult {
        FileResult {
            path: path.to_string(),
            name: name.to_string(),
            size: 0,
            modified: 0,
            file_type,
        }
    }

    fn root_file_search_handoff_result_for_test(
        query: &str,
        mode: crate::file_search::RootFileSectionMode,
    ) -> Option<SearchResult> {
        let ui_state = RootFileSectionUiState::new(
            query,
            mode,
            crate::file_search::RootFileQueryIntent::OrdinaryRoot,
            false,
            false,
            false,
            0,
            0,
            0,
            true,
        );
        root_file_search_handoff_result(
            query,
            mode,
            crate::file_search::RootFileQueryIntent::OrdinaryRoot,
            &ui_state,
        )
    }

    fn builtin_result(name: &str) -> SearchResult {
        SearchResult::BuiltIn(BuiltInMatch {
            entry: BuiltInEntry {
                id: format!("builtin/{}", name.to_lowercase().replace(' ', "-")),
                name: name.to_string(),
                description: "Test built-in".to_string(),
                keywords: Vec::new(),
                feature: crate::builtins::BuiltInFeature::AppLauncher,
                icon: None,
                group: crate::builtins::BuiltInGroup::Core,
            },
            score: i32::MAX,
            match_evidence: None,
        })
    }

    fn builtin_entry(name: &str) -> BuiltInEntry {
        match builtin_result(name) {
            SearchResult::BuiltIn(bm) => bm.entry,
            _ => unreachable!("builtin_result always returns a BuiltIn row"),
        }
    }

    fn agent_chat_history_hit(
        session_id: &str,
        title: &str,
    ) -> crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit {
        crate::ai::agent_chat::ui::history::AgentChatHistorySearchHit {
            entry: crate::ai::agent_chat::ui::history::AgentChatHistoryEntry {
                timestamp: "2026-05-10T17:13:06Z".to_string(),
                first_message: title.to_string(),
                message_count: 3,
                session_id: session_id.to_string(),
                title: title.to_string(),
                custom_title: None,
                preview: "Prior assistant reply".to_string(),
                search_text: title.to_lowercase(),
            },
            score: 100,
            matched_field: crate::ai::agent_chat::ui::history::AgentChatHistorySearchField::Title,
            evidence: None,
        }
    }

    fn clipboard_history_entry(
        id: &str,
        preview: &str,
        pinned: bool,
    ) -> crate::clipboard_history::ClipboardEntryMeta {
        crate::clipboard_history::ClipboardEntryMeta {
            id: id.to_string(),
            content_type: crate::clipboard_history::ContentType::Text,
            timestamp: chrono::Utc::now().timestamp_millis(),
            pinned,
            text_preview: preview.to_string(),
            image_width: None,
            image_height: None,
            byte_size: preview.len(),
            ocr_text: None,
        }
    }

    fn root_note_hit(id: &str, title: &str, pinned: bool) -> crate::notes::RootNoteSearchHit {
        crate::notes::RootNoteSearchHit {
            id: crate::notes::NoteId::parse(id).unwrap_or_else(crate::notes::NoteId::new),
            title: title.to_string(),
            updated_at: chrono::Utc::now(),
            is_pinned: pinned,
            char_count: 42,
            score: 100,
        }
    }

    fn root_brain_hit(
        source: crate::brain::DocSource,
        source_id: &str,
        title: &str,
    ) -> crate::brain::RootBrainSearchHit {
        crate::brain::RootBrainSearchHit {
            title: title.to_string(),
            excerpt: "remembered context".to_string(),
            source_label: source.label(),
            source,
            source_id: source_id.to_string(),
        }
    }

    fn root_browser_tab_hit(
        stable_key: &str,
        title: &str,
    ) -> crate::browser_tabs::RootBrowserTabSearchHit {
        crate::browser_tabs::RootBrowserTabSearchHit {
            stable_key: stable_key.to_string(),
            tab: crate::browser_tabs::BrowserTabInfo {
                browser_name: "Safari".into(),
                browser_bundle_id: "com.apple.Safari".into(),
                window_index: 1,
                tab_index: 1,
                title: title.into(),
                url: "https://example.com/design".into(),
            },
            title: title.to_string(),
            url: "https://example.com/design".to_string(),
            domain: "example.com".to_string(),
            provider_label: "Safari".to_string(),
            score: 100.0,
        }
    }

    #[test]
    fn provider_ranking_receipt_preserves_scores_order_and_actual_budget() {
        let mut first = root_browser_tab_hit("tab/z", "Zebra");
        first.score = 91.5;
        let mut second = root_browser_tab_hit("tab/a", "Alpha");
        second.score = 7.25;
        let hits = [first, second];
        for (remaining_total, domain_intent, expected_count) in [(1, false, 1), (0, true, 2)] {
            let mut grouped = Vec::new();
            let mut results = Vec::new();
            let mut evidence = MainMenuRankingEvidenceMap::new();
            let mut budget = RootPassiveResultBudget {
                remaining_total,
                max_per_source: 2,
            };
            append_root_browser_tabs_section(
                &mut grouped,
                &mut results,
                "example.com",
                None,
                &hits,
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: true,
                    max_results: 2,
                    ..Default::default()
                },
                &mut budget,
                false,
                domain_intent,
                Some(&mut evidence),
            );
            assert_eq!(results.len(), expected_count);
            assert_eq!(
                results[0].name(),
                "Zebra",
                "provider order must not become alphabetical"
            );
            let first = &evidence["tab/z"];
            assert_eq!(first.provider_rank, Some(0));
            assert_eq!(first.provider_score, Some(91.5));
            assert_eq!(first.score, None, "rank-as-score is not relevance evidence");
            assert_eq!(first.budget_limit, Some(expected_count));
            assert_eq!(first.admitted_count, Some(expected_count));
            assert_eq!(
                first.pin_reason,
                domain_intent.then_some("bare-domain-tabs")
            );
            if domain_intent {
                assert_eq!(evidence["tab/a"].provider_rank, Some(1));
                assert_eq!(evidence["tab/a"].provider_score, Some(7.25));
            }
        }
    }

    #[test]
    fn bare_domain_hoists_browser_tabs_ahead_of_every_group() {
        let hits = vec![root_browser_tab_hit("tab/x", "X")];
        let options = crate::browser_tabs::RootBrowserTabsSectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut domain_grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        let mut domain_flat = vec![builtin_result("Existing result")];
        let mut exhausted_budget = RootPassiveResultBudget {
            remaining_total: 0,
            max_per_source: 0,
        };
        append_root_browser_tabs_section(
            &mut domain_grouped,
            &mut domain_flat,
            "x.com",
            None,
            &hits,
            options.clone(),
            &mut exhausted_budget,
            false,
            true,
            None,
        );

        assert!(matches!(
            domain_grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Browser Tabs"
        ));
        let first_item = domain_grouped
            .iter()
            .find_map(|item| match item {
                GroupedListItem::Item(index) => domain_flat.get(*index),
                _ => None,
            })
            .expect("first selectable row");
        assert!(matches!(first_item, SearchResult::BrowserTab(_)));

        let mut ordinary_grouped = vec![
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(0),
        ];
        let mut ordinary_flat = vec![builtin_result("Existing result")];
        append_root_browser_tabs_section(
            &mut ordinary_grouped,
            &mut ordinary_flat,
            "design",
            None,
            &hits,
            options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            false,
            None,
        );
        assert!(matches!(
            ordinary_grouped.first(),
            Some(GroupedListItem::SectionHeader(label, None)) if label == "Files"
        ));
    }

    #[test]
    fn missing_explicit_passive_source_gets_a_status_row() {
        use crate::menu_syntax::RootUnifiedSourceFilter;

        let mut filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
        filters.insert(RootUnifiedSourceFilter::BrowserTabs);
        filters.insert(RootUnifiedSourceFilter::BrowserHistory);

        // Tabs matched; history's section early-returned before its status.
        let flat = vec![SearchResult::BrowserTab(crate::scripts::BrowserTabMatch {
            hit: root_browser_tab_hit("tab/1", "Design Doc"),
            subtitle: "Safari".to_string(),
            score: 100,
        })];
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Browser Tabs".to_string(), None),
            GroupedListItem::Item(0),
        ];
        append_missing_explicit_source_status_rows(&mut grouped, &flat, &filters);

        let history_status = grouped.iter().any(|item| {
            matches!(
                item,
                GroupedListItem::Status(status)
                    if status.source == RootUnifiedSourceFilter::BrowserHistory && status.shown == 0
            )
        });
        assert!(
            history_status,
            "silently-empty explicit source must leave a status row, got {grouped:?}"
        );
        assert!(
            !grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::Status(status)
                    if status.source == RootUnifiedSourceFilter::BrowserTabs
            )),
            "sources represented by result rows must not get a duplicate status"
        );

        // With zero selectable rows the launcher empty state owns feedback.
        let mut empty: Vec<GroupedListItem> = Vec::new();
        append_missing_explicit_source_status_rows(&mut empty, &[], &filters);
        assert!(empty.is_empty());
    }

    fn root_browser_history_hit(
        stable_key: &str,
        title: &str,
    ) -> crate::browser_history::RootBrowserHistorySearchHit {
        crate::browser_history::RootBrowserHistorySearchHit {
            stable_key: stable_key.to_string(),
            provider_label: "Safari".to_string(),
            profile_label: "Default".to_string(),
            title: title.to_string(),
            url: "https://example.com/design-history".to_string(),
            domain: "example.com".to_string(),
            last_visit_unix_ms: chrono::Utc::now().timestamp_millis(),
            visit_count: 3,
        }
    }

    fn root_dictation_history_hit(
        id: &str,
        preview: &str,
    ) -> crate::dictation::RootDictationHistorySearchHit {
        crate::dictation::RootDictationHistorySearchHit {
            id: id.to_string(),
            preview: preview.to_string(),
            target: "Main Filter".to_string(),
            timestamp: "2026-05-10T17:13:06Z".to_string(),
            audio_duration_ms: 1200,
            score: 100,
            matched_field: crate::dictation::DictationHistorySearchField::Transcript,
            evidence: None,
        }
    }

    fn grouped_result_roles(
        grouped: &[GroupedListItem],
        flat: &[SearchResult],
    ) -> Vec<(usize, &'static str)> {
        grouped
            .iter()
            .enumerate()
            .filter_map(|(grouped_index, item)| {
                let GroupedListItem::Item(flat_index) = item else {
                    return None;
                };
                let role = match flat.get(*flat_index)? {
                    SearchResult::Flow(_)
                    | SearchResult::Script(_)
                    | SearchResult::Scriptlet(_)
                    | SearchResult::Skill(_)
                    | SearchResult::BuiltIn(_)
                    | SearchResult::App(_)
                    | SearchResult::Window(_) => "primary",
                    SearchResult::File(_) => "rootFile",
                    SearchResult::Note(_)
                    | SearchResult::BrainHit(_)
                    | SearchResult::Todo(_)
                    | SearchResult::AgentChatHistory(_)
                    | SearchResult::AiVault(_)
                    | SearchResult::ClipboardHistory(_)
                    | SearchResult::DictationHistory(_)
                    | SearchResult::BrowserTab(_)
                    | SearchResult::BrowserHistory(_) => "rootPassive",
                    SearchResult::Fallback(_) => "fallback",
                    SearchResult::ScriptIssue(_) => "scriptIssue",
                    SearchResult::BrainInboxItem(_) => "brainInbox",
                    SearchResult::Agent(_) => "agent",
                    SearchResult::SpineProjection(_) => "spine",
                };
                Some((grouped_index, role))
            })
            .collect()
    }

    /// First grouped index of a terminal fallback row (the "Use … with"
    /// section at the bottom of the list).
    ///
    /// The Files section appends its own "Search Files for …" handoff CTA,
    /// which is also a `SearchResult::Fallback` but belongs to the Files
    /// section by design (it must not split file rows; see
    /// `root_agent_chat_history_rows_do_not_split_files_section_or_file_handoff`
    /// and the F2 audit note on `root_brain_passive_insertion_index`).
    /// Ordering invariants about fallbacks must therefore compare against the
    /// first fallback that is NOT the Files handoff.
    fn first_terminal_fallback_index(
        grouped: &[GroupedListItem],
        flat: &[SearchResult],
    ) -> Option<usize> {
        grouped.iter().enumerate().find_map(|(index, item)| {
            let GroupedListItem::Item(flat_index) = item else {
                return None;
            };
            match flat.get(*flat_index)? {
                SearchResult::Fallback(fallback)
                    if !fallback
                        .stable_selection_key_override
                        .as_deref()
                        .is_some_and(|key| {
                            key.starts_with("fallback/root-file-search-handoff")
                        }) =>
                {
                    Some(index)
                }
                _ => None,
            }
        })
    }

    fn passive_source_counts(
        flat: &[SearchResult],
    ) -> std::collections::HashMap<&'static str, usize> {
        let mut counts = std::collections::HashMap::new();
        for result in flat {
            let source = match result {
                SearchResult::Note(_) => "Notes",
                SearchResult::Todo(_) => "Todos",
                SearchResult::AgentChatHistory(_) => "Agent Chat Conversations",
                SearchResult::AiVault(_) => "AI Vault",
                SearchResult::ClipboardHistory(_) => "Clipboard History",
                SearchResult::DictationHistory(_) => "Dictation History",
                SearchResult::BrowserTab(_) => "Browser Tabs",
                SearchResult::BrowserHistory(_) => "Browser History",
                _ => continue,
            };
            *counts.entry(source).or_insert(0) += 1;
        }
        counts
    }

    fn passive_result_count(flat: &[SearchResult]) -> usize {
        passive_source_counts(flat).values().sum()
    }

    #[test]
    fn root_file_rows_append_files_section_for_eligible_search() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/fix spelling.png",
            "fix spelling.png",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "fix",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "eligible root queries should append a Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/fix spelling.png"
            )),
            "Files section should point at the ranked root file row"
        );
    }

    #[test]
    fn root_file_match_mode_labels_and_handoff_metadata_stay_aligned() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/dev/node_modules/why-is-node-running/index.js",
            "why-is-node-running",
        )];

        let (phrase_grouped, phrase_flat) =
            get_grouped_results_with_validation_query_and_root_files(
                &[],
                &[],
                &[],
                &[],
                &[],
                &frecency_store,
                "why i",
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                Some(crate::file_search::RootFileSectionMode::GlobalQuery),
                true,
                &[],
                &[],
            );
        assert!(phrase_grouped.iter().any(|item| matches!(
            item,
            GroupedListItem::SectionHeader(label, None) if label == "Files · Phrase match"
        )));
        assert!(phrase_flat.iter().any(|result| matches!(
            result,
            SearchResult::Fallback(fallback)
                if fallback.display_label() == "Search Files for \"why i\""
                    && fallback.display_description()
                        == "Open full File Search · preview matches typed phrase"
        )));

        let (word_grouped, word_flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "why is",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &root_files,
            &[],
        );
        assert!(word_grouped.iter().any(|item| matches!(
            item,
            GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
        )));
        assert!(word_flat.iter().any(|result| matches!(
            result,
            SearchResult::Fallback(fallback)
                if fallback.display_label() == "Search Files for \"why is\""
                    && fallback.display_description()
                        == "Open full File Search · preview matches filename words"
        )));

        let roles = grouped_result_roles(&word_grouped, &word_flat);
        let first_root_file = roles
            .iter()
            .find_map(|(idx, role)| (*role == "rootFile").then_some(*idx))
            .expect("root file row");
        let first_handoff = roles
            .iter()
            .find_map(|(idx, role)| (*role == "fallback").then_some(*idx))
            .expect("file handoff row");
        assert!(
            first_root_file < first_handoff,
            "file rows should precede handoff rows"
        );
    }

    #[test]
    fn root_passive_sources_never_precede_primary_launcher_rows_for_same_query() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 12,
                max_results_per_source_when_primary_visible: 5,
            },
            None,
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .expect("collision fixture should include a primary launcher row");
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .expect("collision fixture should include a root file row");
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .expect("collision fixture should include a passive row");
        // The Files section carries its own "Search Files for …" handoff CTA
        // (a Fallback row that is part of the Files section by design), so the
        // passive-vs-fallback invariant is scoped to the terminal "Use … with"
        // fallback section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat)
            .expect("collision fixture should include a terminal fallback row");

        assert!(first_primary < first_root_file);
        assert!(first_primary < first_passive);
        assert!(first_root_file < first_fallback);
        assert!(first_passive < first_fallback);
        assert!(
            roles
                .iter()
                .all(|(index, role)| *role != "rootPassive" || *index > first_primary),
            "no passive root row should appear before the first primary launcher row"
        );

        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_)
                | GroupedListItem::ReservedSectionSlot => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            section_labels,
            vec![
                "Results",
                "Files",
                "Browser Tabs",
                "Notes",
                "Clipboard History",
                "Dictation History",
                "Agent Chat Conversations",
                "Browser History",
                "Use \"design\" with...",
            ]
        );
    }

    #[test]
    fn root_passive_source_order_reorders_only_passive_sections() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];
        let passive_order = [
            crate::config::UnifiedSearchPassiveSource::AgentChatHistory,
            crate::config::UnifiedSearchPassiveSource::BrowserHistory,
            crate::config::UnifiedSearchPassiveSource::Notes,
            crate::config::UnifiedSearchPassiveSource::BrowserTabs,
            crate::config::UnifiedSearchPassiveSource::ClipboardHistory,
            crate::config::UnifiedSearchPassiveSource::DictationHistory,
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                min_query_chars: 3,
                ..Default::default()
            },
            &passive_order,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 12,
                max_results_per_source_when_primary_visible: 5,
            },
            None,
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .expect("collision fixture should include a primary launcher row");
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .expect("collision fixture should include a root file row");
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .expect("collision fixture should include a passive row");
        // Scoped to the terminal "Use … with" fallback section; the Files
        // handoff CTA is a Fallback row owned by the Files section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat)
            .expect("collision fixture should include a terminal fallback row");

        assert!(first_primary < first_root_file);
        assert!(first_root_file < first_passive);
        assert!(first_passive < first_fallback);

        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_)
                | GroupedListItem::ReservedSectionSlot => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            section_labels,
            vec![
                "Results",
                "Files",
                "Agent Chat Conversations",
                "Browser History",
                "Notes",
                "Browser Tabs",
                "Clipboard History",
                "Dictation History",
                "Use \"design\" with...",
            ]
        );
    }

    #[test]
    fn root_brain_section_appends_only_when_enabled_with_hits() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let brain_hits = vec![root_brain_hit(
            crate::brain::DocSource::Note,
            "44444444-4444-4444-4444-444444444444",
            "design memory",
        )];

        let run = |hits: &[crate::brain::RootBrainSearchHit], enabled: bool| {
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[builtin_entry("Design Gallery")],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &frecency_store,
                query,
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
                None,
                false,
                &[],
                &[],
                crate::file_search::RootFileSectionOptions::default(),
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                hits,
                crate::brain::RootBrainSectionOptions {
                    enabled,
                    ..Default::default()
                },
                &[],
                crate::notes::RootNotesSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: false,
                    max_results: 0,
                    min_query_chars: usize::MAX,
                    scan_limit: 0,
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai_vault::RootAiVaultSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig {
                    max_total_results: 12,
                    max_total_results_when_primary_visible: 12,
                    max_results_per_source_when_primary_visible: 5,
                },
                None,
            )
        };

        let has_brain_header = |grouped: &[GroupedListItem]| {
            grouped.iter().any(|item| {
                matches!(
                    item,
                    GroupedListItem::SectionHeader(label, None) if label == "From Your Brain"
                )
            })
        };

        let (grouped, flat) = run(&brain_hits, true);
        assert!(
            has_brain_header(&grouped),
            "enabled brain section with hits should append a From Your Brain header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::BrainHit(bm) if bm.hit.title == "design memory"
            )),
            "From Your Brain section should surface the brain hit row"
        );

        let (grouped, flat) = run(&brain_hits, false);
        assert!(
            !has_brain_header(&grouped),
            "disabled brain section must not append a header"
        );
        assert!(
            !flat
                .iter()
                .any(|result| matches!(result, SearchResult::BrainHit(_))),
            "disabled brain section must not surface rows"
        );

        let (grouped, flat) = run(&[], true);
        assert!(
            !has_brain_header(&grouped),
            "empty brain hits must not append a header"
        );
        assert!(
            !flat
                .iter()
                .any(|result| matches!(result, SearchResult::BrainHit(_))),
            "empty brain hits must not surface rows"
        );
    }

    #[test]
    fn active_source_filters_select_matching_passive_sources() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let browser_tabs = vec![root_browser_tab_hit("tab/design", "design tab")];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];
        let clipboard = vec![clipboard_history_entry(
            "clip-design",
            "design copied text",
            false,
        )];
        let dictation = vec![root_dictation_history_hit(
            "dictation-design",
            "design transcript",
        )];
        let agent_chat = vec![agent_chat_history_hit(
            "session-design",
            "design conversation",
        )];
        let browser_history = vec![root_browser_history_hit(
            "history/design",
            "design history page",
        )];

        for (source, expected_section, expected_source) in [
            (
                crate::menu_syntax::RootUnifiedSourceFilter::BrowserTabs,
                "Browser Tabs",
                "Browser Tabs",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Notes,
                "Notes",
                "Notes",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::ClipboardHistory,
                "Clipboard History",
                "Clipboard History",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Dictation,
                "Dictation History",
                "Dictation History",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::Conversations,
                "Agent Chat Conversations",
                "Agent Chat Conversations",
            ),
            (
                crate::menu_syntax::RootUnifiedSourceFilter::BrowserHistory,
                "Browser History",
                "Browser History",
            ),
        ] {
            let mut source_filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
            source_filters.insert(source);

            let (grouped, flat) =
                get_grouped_results_with_validation_query_and_root_files_with_options(
                    &[],
                    &[],
                    &[builtin_entry("Design Gallery")],
                    &[],
                    &[],
                    crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                    &[],
                    &[],
                    None,
                    &frecency_store,
                    query,
                    &SuggestedConfig::default(),
                    &[],
                    None,
                    None,
                    None,
                    None,
                    &source_filters,
                    None,
                    false,
                    &[],
                    &[],
                    crate::file_search::RootFileSectionOptions::default(),
                    &[],
                    crate::menu_syntax::RootTodoSectionOptions {
                        enabled: false,
                        ..Default::default()
                    },
                    &[],
                    crate::brain::RootBrainSectionOptions {
                        enabled: false,
                        ..Default::default()
                    },
                    &notes,
                    crate::notes::RootNotesSectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &clipboard,
                    crate::clipboard_history::RootClipboardHistorySectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &dictation,
                    crate::dictation::RootDictationHistorySectionOptions {
                        enabled: true,
                        max_results: 3,
                        min_query_chars: 3,
                        scan_limit: 10,
                    },
                    &agent_chat,
                    crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(
                    ),
                    &[],
                    crate::ai_vault::RootAiVaultSectionOptions::default(),
                    &browser_tabs,
                    crate::browser_tabs::RootBrowserTabsSectionOptions {
                        enabled: true,
                        ..Default::default()
                    },
                    &browser_history,
                    crate::browser_history::RootBrowserHistorySectionOptions {
                        enabled: true,
                        min_query_chars: 3,
                        ..Default::default()
                    },
                    &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                    crate::config::UnifiedSearchPassiveResultLimitsConfig::default(),
                    None,
                );

            let section_labels = grouped
                .iter()
                .filter_map(|item| match item {
                    GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                    GroupedListItem::SectionHeader(_, Some(_))
                    | GroupedListItem::Item(_)
                    | GroupedListItem::Status(_)
                    | GroupedListItem::ReservedSectionSlot => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(section_labels, vec![expected_section], "{source:?}");
            assert!(
                flat.iter()
                    .all(|result| result.source_name() == Some(expected_source)),
                "{source:?}: unexpected rows {flat:?}"
            );
        }
    }

    #[test]
    fn root_passive_budget_caps_rows_when_primary_launcher_results_exist() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let browser_tabs = (0..3)
            .map(|i| root_browser_tab_hit(&format!("tab/design-{i}"), "design tab"))
            .collect::<Vec<_>>();
        let notes = vec![
            root_note_hit("33333333-3333-3333-3333-333333333331", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333332", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333333", "design note", false),
        ];
        let clipboard = (0..3)
            .map(|i| {
                clipboard_history_entry(&format!("clip-design-{i}"), "design copied text", false)
            })
            .collect::<Vec<_>>();
        let dictation = (0..3)
            .map(|i| {
                root_dictation_history_hit(&format!("dictation-design-{i}"), "design transcript")
            })
            .collect::<Vec<_>>();
        let agent_chat = (0..3)
            .map(|i| agent_chat_history_hit(&format!("session-design-{i}"), "design conversation"))
            .collect::<Vec<_>>();
        let browser_history = (0..3)
            .map(|i| {
                root_browser_history_hit(&format!("history/design-{i}"), "design history page")
            })
            .collect::<Vec<_>>();

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[builtin_entry("Design Gallery")],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &dictation,
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
            },
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &browser_history,
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 12,
                max_total_results_when_primary_visible: 4,
                max_results_per_source_when_primary_visible: 1,
            },
            None,
        );

        let roles = grouped_result_roles(&grouped, &flat);
        let first_primary = roles
            .iter()
            .find_map(|(index, role)| (*role == "primary").then_some(*index))
            .unwrap();
        let first_root_file = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootFile").then_some(*index))
            .unwrap();
        let first_passive = roles
            .iter()
            .find_map(|(index, role)| (*role == "rootPassive").then_some(*index))
            .unwrap();
        // Scoped to the terminal "Use … with" fallback section; the Files
        // handoff CTA is a Fallback row owned by the Files section.
        let first_fallback = first_terminal_fallback_index(&grouped, &flat).unwrap();
        assert!(first_primary < first_root_file);
        assert!(first_root_file < first_passive);
        assert!(first_passive < first_fallback);
        assert_eq!(passive_result_count(&flat), 4);
        assert!(passive_source_counts(&flat)
            .values()
            .all(|count| *count <= 1));
    }

    #[test]
    fn root_passive_budget_allows_larger_passive_set_without_primary_launcher_results() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let browser_tabs = (0..3)
            .map(|i| root_browser_tab_hit(&format!("tab/design-{i}"), "design tab"))
            .collect::<Vec<_>>();
        let notes = vec![
            root_note_hit("33333333-3333-3333-3333-333333333331", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333332", "design note", false),
            root_note_hit("33333333-3333-3333-3333-333333333333", "design note", false),
        ];
        let clipboard = (0..3)
            .map(|i| {
                clipboard_history_entry(&format!("clip-design-{i}"), "design copied text", false)
            })
            .collect::<Vec<_>>();

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files_with_options(
            &[],
            &[],
            &[],
            &[],
            &[],
            crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
            &[],
            &[],
            None,
            &frecency_store,
            query,
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
            None,
            false,
            &[],
            &[],
            crate::file_search::RootFileSectionOptions::default(),
            &[],
            crate::menu_syntax::RootTodoSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &[],
            crate::brain::RootBrainSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &clipboard,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &[],
            crate::dictation::RootDictationHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                scan_limit: 10,
            },
            &[],
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
            },
            &[],
            crate::ai_vault::RootAiVaultSectionOptions::default(),
            &browser_tabs,
            crate::browser_tabs::RootBrowserTabsSectionOptions {
                enabled: true,
                max_results: 3,
                ..Default::default()
            },
            &[],
            crate::browser_history::RootBrowserHistorySectionOptions {
                enabled: true,
                max_results: 3,
                min_query_chars: 3,
                ..Default::default()
            },
            &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
            crate::config::UnifiedSearchPassiveResultLimitsConfig {
                max_total_results: 5,
                max_total_results_when_primary_visible: 1,
                max_results_per_source_when_primary_visible: 1,
            },
            None,
        );

        assert_eq!(passive_result_count(&flat), 5);
        let section_labels = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, None) => Some(label.as_str()),
                GroupedListItem::SectionHeader(_, Some(_))
                | GroupedListItem::Item(_)
                | GroupedListItem::Status(_)
                | GroupedListItem::ReservedSectionSlot => None,
            })
            .collect::<Vec<_>>();
        // The passive budget is consumed greedily in passive-source order:
        // Browser Tabs takes 3 rows, Notes takes the remaining 2, and the
        // Clipboard History section is skipped once the total budget (5) is
        // exhausted. The invariant under test is the total cap, asserted via
        // passive_result_count above.
        assert_eq!(
            section_labels,
            vec!["Browser Tabs", "Notes", "Use \"design\" with..."]
        );
    }

    #[test]
    fn root_passive_budget_zero_hides_passive_rows_during_primary_collision() {
        let frecency_store = FrecencyStore::new();
        let query = "design";
        let root_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];
        let notes = vec![root_note_hit(
            "33333333-3333-3333-3333-333333333333",
            "design note",
            false,
        )];

        let (_grouped, flat) =
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[builtin_entry("Design Gallery")],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &frecency_store,
                query,
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &crate::menu_syntax::RootUnifiedSourceFilterSet::default(),
                Some(crate::file_search::RootFileSectionMode::GlobalQuery),
                false,
                &root_files,
                &[],
                crate::file_search::RootFileSectionOptions::default(),
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::brain::RootBrainSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &notes,
                crate::notes::RootNotesSectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: true,
                    max_results: 3,
                    min_query_chars: 3,
                    scan_limit: 10,
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
                &[],
                crate::ai_vault::RootAiVaultSectionOptions::default(),
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: true,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: true,
                    min_query_chars: 3,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig {
                    max_total_results: 12,
                    max_total_results_when_primary_visible: 0,
                    max_results_per_source_when_primary_visible: 1,
                },
                None,
            );

        assert!(flat.iter().any(is_primary_launcher_result));
        assert!(flat
            .iter()
            .any(|result| matches!(result, SearchResult::File(_))));
        assert!(flat
            .iter()
            .any(|result| matches!(result, SearchResult::Fallback(_))));
        assert_eq!(passive_result_count(&flat), 0);
    }

    #[test]
    fn root_agent_chat_history_rows_insert_after_primary_rows_before_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::AgentChatHistory(hit)) if hit.entry.session_id == "session-1"
        ));
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label.starts_with("Use \""))
        );
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_append_for_short_or_advanced_query() {
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "ai",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:agent_chat-history search");
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_append_when_disabled() {
        let hits = vec![agent_chat_history_hit("session-1", "search design notes")];
        let mut grouped = Vec::new();
        let mut flat = Vec::new();

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );

        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_clipboard_history_rows_insert_before_agent_chat_and_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            true,
        )];
        let agent_chat = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Clipboard History")
        );
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::ClipboardHistory(hit)) if hit.entry.id == "clip-1"
        ));
        assert!(matches!(
            flat.get(3),
            Some(SearchResult::AgentChatHistory(hit)) if hit.entry.session_id == "session-1"
        ));
    }

    #[test]
    fn root_notes_rows_insert_after_primary_before_clipboard_agent_chat_and_fallbacks() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Use \"search\" with...".to_string(), None),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            builtin_result("Search Files"),
            root_file_search_handoff_result_for_test(
                "search",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let notes = vec![root_note_hit(
            "11111111-1111-1111-1111-111111111111",
            "search note",
            true,
        )];
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            true,
        )];
        let agent_chat = vec![agent_chat_history_hit("session-1", "search design notes")];

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: true,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &agent_chat,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Notes")
        );
        assert!(
            matches!(&grouped[3], GroupedListItem::SectionHeader(label, None) if label == "Clipboard History")
        );
        assert!(
            matches!(&grouped[5], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations")
        );
        assert!(matches!(
            flat.get(2),
            Some(SearchResult::Note(hit)) if hit.title == "search note"
        ));
    }

    #[test]
    fn root_notes_rows_do_not_append_for_empty_short_disabled_or_advanced_query() {
        let notes = vec![root_note_hit(
            "22222222-2222-2222-2222-222222222222",
            "search note",
            false,
        )];
        let enabled_options = crate::notes::RootNotesSectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "",
            None,
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "no",
            None,
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &notes,
            crate::notes::RootNotesSectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:note search");
        append_root_notes_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &notes,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_clipboard_history_rows_do_not_append_for_empty_short_disabled_or_advanced_query() {
        let clips = vec![clipboard_history_entry(
            "clip-1",
            "search copied text",
            false,
        )];
        let enabled_options = crate::clipboard_history::RootClipboardHistorySectionOptions {
            enabled: true,
            ..Default::default()
        };

        let mut grouped = Vec::new();
        let mut flat = Vec::new();
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "",
            None,
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "se",
            None,
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            None,
            &clips,
            crate::clipboard_history::RootClipboardHistorySectionOptions {
                enabled: false,
                ..Default::default()
            },
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());

        let query = advanced_query_from(":type:clipboard search");
        append_root_clipboard_history_section(
            &mut grouped,
            &mut flat,
            "search",
            Some(&query),
            &clips,
            enabled_options,
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );
        assert!(grouped.is_empty());
        assert!(flat.is_empty());
    }

    #[test]
    fn root_agent_chat_history_rows_do_not_split_files_section_or_file_handoff() {
        let mut grouped = vec![
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Files".to_string(), None),
            GroupedListItem::Item(1),
            GroupedListItem::Item(2),
            GroupedListItem::SectionHeader("Use \"design\" with...".to_string(), None),
            GroupedListItem::Item(3),
        ];
        let mut flat = vec![
            builtin_result("Open Notes"),
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/design.md", "design.md"),
                score: 50,
            }),
            root_file_search_handoff_result_for_test(
                "design",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
            root_file_search_handoff_result_for_test(
                "design",
                crate::file_search::RootFileSectionMode::GlobalQuery,
            )
            .unwrap(),
        ];
        let hits = vec![agent_chat_history_hit("session-1", "design notes")];

        append_root_agent_chat_history_section(
            &mut grouped,
            &mut flat,
            "design",
            None,
            &hits,
            crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions::default(),
            &mut RootPassiveResultBudget::unbounded(),
            false,
            None,
        );

        assert!(
            matches!(&grouped[1], GroupedListItem::SectionHeader(label, None) if label == "Files")
        );
        assert!(matches!(&grouped[2], GroupedListItem::Item(1)));
        assert!(matches!(&grouped[3], GroupedListItem::Item(2)));
        assert!(
            matches!(&grouped[4], GroupedListItem::SectionHeader(label, None) if label == "Agent Chat Conversations"),
            "Agent Chat Conversations should insert after the Files handoff, not between file rows"
        );
    }

    #[test]
    fn root_global_file_rows_seed_matching_recent_files_while_provider_loading() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "global root search should keep the stable Files header while recent seeds render"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
            )),
            "matching recent files should seed non-empty global root file results before provider rows arrive"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "seeded global file rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_rejects_path_only_match_while_loading() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Design/archive/readme.md",
            "readme.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Design/archive/readme.md"
            )),
            "path-only recent files should not seed non-empty global root search"
        );
        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "the stable Files section should remain visible for the continuation row"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "path-only recent rejection should still keep the dedicated File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_accepts_ordered_directory_context() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/dev/script-kit/README.md",
            "README.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "script kit readme",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
            )),
            "directory-context recent seeds should render under the stable Word match Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/script-kit/README.md"
            )),
            "ordered directory-context recent files should seed non-empty global root results"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"script kit readme\""
            )),
            "seeded directory-context rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_recent_seed_rejects_path_only_directory_context() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/dev/script-kit/readme/archive.txt",
            "archive.txt",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "script kit readme",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/script-kit/readme/archive.txt"
            )),
            "path-only directory-context recents must not seed while the provider is loading"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"script kit readme\""
            )),
            "path-only rejection should still keep the dedicated File Search handoff"
        );
    }

    #[test]
    fn root_global_provider_path_only_match_still_renders() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Design/archive/readme.md",
            "readme.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Design/archive/readme.md"
            )),
            "provider-returned path-only matches should still render after the provider answers"
        );
    }

    #[test]
    fn root_global_file_rows_dedupe_provider_and_recent_by_path() {
        let frecency_store = FrecencyStore::new();
        let shared = root_file("/Users/example/Desktop/design-notes.md", "design-notes.md");
        let provider_files = vec![shared.clone()];
        let recent_files = vec![shared];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &provider_files,
            &recent_files,
        );

        let duplicate_count = flat
            .iter()
            .filter(|result| {
                matches!(
                    result,
                    SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
                )
            })
            .count();

        assert_eq!(
            duplicate_count, 1,
            "provider and recent rows with the same full path should render once"
        );
    }

    #[test]
    fn root_global_exact_stem_match_promotes_files_section_when_opted_in() {
        let files = vec![SearchResult::File(crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
            score: 100,
        })];
        let grouped = vec![GroupedListItem::SectionHeader("Commands".to_string(), None)];
        let file_matches = files
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design-notes",
            &file_matches,
            &[],
        ));
        assert_eq!(
            root_file_section_insertion_index(&grouped, &files, true),
            0,
            "exact filename/stem matches can insert Files above ordinary launcher groups only when opted in"
        );
    }

    #[test]
    fn root_directory_browse_never_promotes_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/dev/design-notes.md", "design-notes.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::DirectoryBrowse,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_boundary_filename_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/client-design-notes.md",
                "client-design-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_camel_case_filename_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/ClientDesignNotes.md",
                "ClientDesignNotes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_recent_seed_accepts_camel_case_filename_token() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/ClientDesignNotes.md",
            "ClientDesignNotes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files"
            )),
            "global root search should keep the stable Files header while camel-case recent seeds render"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/ClientDesignNotes.md"
            )),
            "camel-case filename token matches should seed non-empty global root file results before provider rows arrive"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"design\""
            )),
            "seeded global file rows should keep the full File Search handoff"
        );
    }

    #[test]
    fn root_global_multiword_recent_seed_uses_filename_tokens() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/client-design-notes.md",
            "client-design-notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "client notes",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files · Word match"
            )),
            "multi-word recent seeds should render under the stable Word match Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/client-design-notes.md"
            )),
            "ordered multi-word filename tokens should seed non-empty global root file results"
        );
    }

    #[test]
    fn root_global_multiword_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/client-design-notes.md",
                "client-design-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design notes",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_multiword_mid_token_match_does_not_promote() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/redesign-notes.md",
                "redesign-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design notes",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_recent_seed_directory_context_does_not_promote_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/dev/script-kit/README.md", "README.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "script kit readme",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_short_digit_recent_seed_uses_filename_tokens() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/2026-q2-report.xlsx",
            "2026-q2-report.xlsx",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "q2",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            grouped.iter().any(|item| matches!(
                item,
                GroupedListItem::SectionHeader(label, None) if label == "Files"
            )),
            "short digit recent seeds should render under the stable Files header"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.name == "2026-q2-report.xlsx"
            )),
            "short digit filename tokens should seed non-empty global root file results"
        );
    }

    #[test]
    fn root_global_short_digit_token_match_does_not_promote_exact_policy() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/Q2Report.pdf", "Q2Report.pdf"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "q2",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_two_letter_query_still_does_not_promote() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/ai-notes.md", "ai-notes.md"),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "ai",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_mid_token_contains_does_not_promote_files_section() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/redesign-notes.md",
                "redesign-notes.md",
            ),
            score: 100,
        }];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &[],
        ));
    }

    #[test]
    fn root_global_strong_launcher_match_blocks_file_section_promotion() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file(
                "/Users/example/Desktop/fix spelling.png",
                "fix spelling.png",
            ),
            score: 100,
        }];
        let launcher_results = vec![builtin_result("Fix Spelling and Grammar")];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "spelling",
            &files,
            &launcher_results,
        ));
    }

    #[test]
    fn root_global_weak_launcher_match_blocks_file_section_promotion() {
        let files = vec![crate::scripts::FileMatch {
            file: root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
            score: 100,
        }];
        let launcher_results = vec![builtin_result("Redesign Theme")];

        assert!(!root_file_section_should_promote(
            crate::file_search::RootFilePromotionPolicy::ExactFilenameOnly,
            crate::file_search::RootFileSectionMode::GlobalQuery,
            false,
            "design",
            &files,
            &launcher_results,
        ));
    }

    #[test]
    fn root_file_rows_precede_fallback_rows_for_file_only_search() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/unique report name.pdf",
            "unique report name.pdf",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "unique report name",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let file_grouped_index = grouped
            .iter()
            .position(|item| {
                matches!(
                    item,
                    GroupedListItem::Item(idx)
                        if matches!(flat.get(*idx), Some(SearchResult::File(_)))
                )
            })
            .expect("file result should be grouped");
        let fallback_grouped_index = grouped
            .iter()
            .position(|item| {
                matches!(
                    item,
                    GroupedListItem::Item(idx)
                        if matches!(flat.get(*idx), Some(SearchResult::Fallback(_)))
                )
            })
            .expect("fallback result should still be grouped");

        assert!(
            file_grouped_index < fallback_grouped_index,
            "root file results should appear before fallback actions so Enter opens the file first"
        );
    }

    #[test]
    fn root_file_rows_do_not_append_for_advanced_queries() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file(
            "/Users/example/Desktop/fix spelling.png",
            "fix spelling.png",
        )];
        let query = advanced_query_from(":type:file fix");

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "fix",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            Some(&query),
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")),
            "advanced query mode should not mix in root Spotlight file rows"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "advanced query mode should not append file results"
        );
    }

    #[test]
    fn root_global_file_rows_exclude_application_bundles() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type("/Applications/Zed.app", "Zed.app", FileType::Application),
            root_file_with_type(
                "/Users/example/Desktop/zed-notes.md",
                "zed-notes.md",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["zed-notes.md"],
            "global root Files should not duplicate app launcher results as .app file rows"
        );
    }

    #[test]
    fn root_global_file_rows_exclude_app_bundle_contents() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/Desktop/zed-notes.md",
                "zed-notes.md",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["/Users/example/Desktop/zed-notes.md"],
            "global root Files should not render files nested inside .app bundles"
        );
    }

    #[test]
    fn root_global_app_bundle_filter_keeps_search_files_handoff() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file_with_type(
            "/Applications/Zed.app",
            "Zed.app",
            FileType::Application,
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "zed",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::File(file) if file.file.name == "Zed.app"
            )),
            "filtered application bundles should not render as root global file rows"
        );
        assert!(
            grouped.iter().any(|item| {
                matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files")
            }),
            "the Files section should still be allowed to show the handoff row"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Search Files for \"zed\""
            )),
            "app-bundle filtering should not remove the full File Search handoff"
        );
    }

    #[test]
    fn root_directory_browse_keeps_app_bundle_contents_for_explicit_paths() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file_with_type(
            "/Applications/Zed.app/Contents/Info.plist",
            "Info.plist",
            FileType::Document,
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "/Applications/Zed.app/Contents/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Applications/Zed.app/Contents/Info.plist"
            )),
            "explicit directory browse should still render already-collected direct children inside .app bundles"
        );
    }

    #[test]
    fn root_directory_browse_rows_append_files_section_for_path_query() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type("/Users/example/dev/app", "app", FileType::Directory),
            root_file_with_type(
                "/Users/example/dev/Zed.app",
                "Zed.app",
                FileType::Application,
            ),
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Files · Folder")),
            "directory path queries should append the folder-specific Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/dev/Zed.app"
            )),
            "directory browse should render provider-ordered rows, including app bundles"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Open File Search in \"~/dev\""
            )),
            "directory browse should append a folder-scoped File Search handoff"
        );
    }

    #[test]
    fn root_directory_browse_does_not_mix_recent_files() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![root_file("/Users/example/dev/design.md", "design.md")];
        let recent_files = vec![root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            true,
            &root_files,
            &recent_files,
        );

        let rendered_paths = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_paths,
            vec!["/Users/example/dev/design.md"],
            "directory browse should render direct children only and ignore recent file seeds"
        );
    }

    #[test]
    fn root_directory_browse_rows_use_provider_order_without_fuzzy_filtering() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Users/example/dev/beta.txt",
                "beta.txt",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha.txt",
                "alpha.txt",
                FileType::Document,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["beta.txt", "alpha.txt"],
            "directory browse should preserve provider order instead of fuzzy re-ranking"
        );
    }

    #[test]
    fn root_directory_browse_rows_filter_by_child_fragment() {
        let frecency_store = FrecencyStore::new();
        let root_files = vec![
            root_file_with_type(
                "/Users/example/dev/beta-notes.md",
                "beta-notes.md",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha-report.md",
                "alpha-report.md",
                FileType::Document,
            ),
            root_file_with_type(
                "/Users/example/dev/alpha-folder",
                "alpha-folder",
                FileType::Directory,
            ),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "~/dev/al",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::DirectoryBrowse),
            false,
            &root_files,
            &[],
        );

        let rendered_files = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.name.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert_eq!(
            rendered_files,
            vec!["alpha-folder", "alpha-report.md"],
            "directory browse child fragments should filter direct children inline"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label() == "Open File Search in \"~/dev\""
            )),
            "filtered directory browse should keep the handoff scoped to the containing folder"
        );
    }

    #[test]
    fn empty_root_appends_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "empty root should append a Recent Files section"
        );
        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/recent design notes.md"
            )),
            "Recent Files should render real SearchResult::File rows"
        );
    }

    #[test]
    fn recent_files_grouping_filters_directories_and_app_bundle_contents() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![
            root_file_with_type(
                "/Users/example/Desktop/example-folder",
                "example-folder",
                FileType::Directory,
            ),
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
            root_file("/Users/example/Desktop/design-notes.md", "design-notes.md"),
        ];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        let rendered_paths = flat
            .iter()
            .filter_map(|result| match result {
                SearchResult::File(file) => Some(file.file.path.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rendered_paths,
            vec!["/Users/example/Desktop/design-notes.md"],
            "Recent Files should filter directories and app bundle internals"
        );
    }

    #[test]
    fn empty_root_recent_files_suppress_section_when_all_rows_ineligible() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![
            root_file_with_type("/Applications/Zed.app", "Zed.app", FileType::Application),
            root_file_with_type(
                "/Applications/Zed.app/Contents/Info.plist",
                "Info.plist",
                FileType::Document,
            ),
        ];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "empty-root Recent Files should omit the section when every row is ineligible"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "all-ineligible recent files should not render file rows"
        );
    }

    #[test]
    fn root_global_recent_seed_can_match_beyond_empty_recent_render_limit() {
        let frecency_store = FrecencyStore::new();
        let mut recent_files = Vec::new();
        for idx in 0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT {
            recent_files.push(root_file(
                &format!("/Users/example/Desktop/other-{idx}.md"),
                &format!("other-{idx}.md"),
            ));
        }
        recent_files.push(root_file(
            "/Users/example/Desktop/design-notes.md",
            "design-notes.md",
        ));

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "design",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            Some(crate::file_search::RootFileSectionMode::GlobalQuery),
            true,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().any(|result| matches!(
                result,
                SearchResult::File(file) if file.file.path == "/Users/example/Desktop/design-notes.md"
            )),
            "non-empty global Files should seed from the deeper recent pool, not only the empty-root render cap"
        );
    }

    #[test]
    fn empty_root_recent_files_stay_render_capped_with_deeper_recent_pool() {
        let frecency_store = FrecencyStore::new();
        let recent_files = (0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 3)
            .map(|idx| {
                root_file(
                    &format!("/Users/example/Desktop/recent-{idx}.md"),
                    &format!("recent-{idx}.md"),
                )
            })
            .collect::<Vec<_>>();

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        let file_count = flat
            .iter()
            .filter(|result| matches!(result, SearchResult::File(_)))
            .count();
        assert_eq!(
            file_count,
            crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT,
            "empty-root Recent Files should remain visually capped"
        );
    }

    #[test]
    fn source_filter_files_empty_browse_uses_browse_target_not_recent_render_cap() {
        let recent_files = (0..crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 8)
            .map(|idx| {
                root_file(
                    &format!("/Users/example/Desktop/recent-{idx}.md"),
                    &format!("recent-{idx}.md"),
                )
            })
            .collect::<Vec<_>>();
        let mut source_filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
        source_filters.insert(crate::menu_syntax::RootUnifiedSourceFilter::Files);
        let target = crate::file_search::ROOT_FILE_RECENT_RENDER_LIMIT + 8;

        let (_grouped, flat) =
            get_grouped_results_with_validation_query_and_root_files_with_options(
                &[],
                &[],
                &[],
                &[],
                &[],
                crate::window_control::RootWindowsProviderStatus::Ready { count: 0 },
                &[],
                &[],
                None,
                &FrecencyStore::new(),
                "",
                &SuggestedConfig::default(),
                &[],
                None,
                None,
                None,
                None,
                &source_filters,
                None,
                false,
                &[],
                &recent_files,
                crate::file_search::RootFileSectionOptions {
                    source_filter_browse_target_visible_rows: Some(target),
                    ..Default::default()
                },
                &[],
                crate::menu_syntax::RootTodoSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::brain::RootBrainSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::notes::RootNotesSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::clipboard_history::RootClipboardHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::dictation::RootDictationHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai::agent_chat::ui::history::RootAgentChatHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::ai_vault::RootAiVaultSectionOptions::default(),
                &[],
                crate::browser_tabs::RootBrowserTabsSectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &[],
                crate::browser_history::RootBrowserHistorySectionOptions {
                    enabled: false,
                    ..Default::default()
                },
                &crate::config::UnifiedSearchPassiveSource::DEFAULT_ORDER,
                crate::config::UnifiedSearchPassiveResultLimitsConfig::default(),
                None,
            );

        let file_count = flat
            .iter()
            .filter(|result| matches!(result, SearchResult::File(_)))
            .count();
        assert_eq!(
            file_count, target,
            "explicit Files source-only browse should use the source-filter target, not the empty-root cap"
        );
    }

    #[test]
    fn recent_files_insert_after_icon_suggested_section() {
        let mut grouped = vec![
            GroupedListItem::SectionHeader("Suggested".to_string(), Some("StarFilled".to_string())),
            GroupedListItem::Item(0),
            GroupedListItem::SectionHeader("Commands".to_string(), Some("Terminal".to_string())),
            GroupedListItem::Item(1),
        ];
        let mut flat = vec![
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/suggested.txt", "suggested.txt"),
                score: 10,
            }),
            SearchResult::File(crate::scripts::FileMatch {
                file: root_file("/Users/example/Desktop/command.txt", "command.txt"),
                score: 9,
            }),
        ];
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        append_recent_root_file_section(
            &mut grouped,
            &mut flat,
            &recent_files,
            "",
            None,
            crate::file_search::RootFileSectionOptions::default(),
            None,
        );

        assert!(
            matches!(&grouped[4], GroupedListItem::SectionHeader(label, None) if label == "Recent Files"),
            "Recent Files should insert after primary launcher groups"
        );
    }

    #[test]
    fn non_empty_search_does_not_append_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "recent",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "non-empty root search should use Files, not Recent Files"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "recent files should not leak into non-empty root search"
        );
    }

    #[test]
    fn advanced_query_does_not_append_recent_files_section() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];
        let query = advanced_query_from(":type:file");

        let (grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            Some(&query),
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            !grouped
                .iter()
                .any(|item| matches!(item, GroupedListItem::SectionHeader(label, None) if label == "Recent Files")),
            "advanced query mode should not mix in recent root files"
        );
        assert!(
            flat.iter()
                .all(|result| !matches!(result, SearchResult::File(_))),
            "advanced query mode should not append recent file rows"
        );
    }

    #[test]
    fn recent_files_do_not_create_search_files_handoff_row() {
        let frecency_store = FrecencyStore::new();
        let recent_files = vec![root_file(
            "/Users/example/Desktop/recent design notes.md",
            "recent design notes.md",
        )];

        let (_grouped, flat) = get_grouped_results_with_validation_query_and_root_files(
            &[],
            &[],
            &[],
            &[],
            &[],
            &frecency_store,
            "",
            &SuggestedConfig::default(),
            &[],
            None,
            None,
            None,
            None,
            None,
            false,
            &[],
            &recent_files,
        );

        assert!(
            flat.iter().all(|result| !matches!(
                result,
                SearchResult::Fallback(fallback) if fallback.display_label().starts_with("Search Files for")
            )),
            "empty recent file rows should not create a Search Files continuation row"
        );
    }
}

#[cfg(test)]
mod capture_mode_tests {
    use super::*;
    use crate::menu_syntax::{parse, CaptureInvocation, MenuSyntaxParse};
    use crate::metadata_parser::TypedMetadata;
    use serde_json::json;
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn capture_from(raw: &str) -> CaptureInvocation {
        match parse(raw) {
            MenuSyntaxParse::Capture(c) => c,
            other => panic!("expected Capture for {raw:?}, got {other:?}"),
        }
    }

    fn script_with_menu_syntax(name: &str, menu_syntax: serde_json::Value) -> Arc<Script> {
        let mut extra: HashMap<String, serde_json::Value> = HashMap::new();
        extra.insert("menuSyntax".to_string(), menu_syntax);
        let mut meta = TypedMetadata::default();
        meta.extra = extra;
        Arc::new(Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}.ts")),
            extension: "ts".to_string(),
            description: Some(format!("{name} description")),
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: Some(meta),
            schema: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            kit_name: None,
            body: None,
        })
    }

    fn plain_script(name: &str) -> Arc<Script> {
        Arc::new(Script {
            name: name.to_string(),
            path: PathBuf::from(format!("/tmp/{name}.ts")),
            extension: "ts".to_string(),
            description: None,
            icon: None,
            alias: None,
            shortcut: None,
            typed_metadata: None,
            schema: None,
            plugin_id: "main".to_string(),
            plugin_title: None,
            kit_name: None,
            body: None,
        })
    }

    #[test]
    fn zero_handlers_returns_single_help_header_and_no_flat_results() {
        let invocation = capture_from(";todo Renew passport");
        let scripts: Vec<Arc<Script>> = vec![plain_script("unrelated")];
        let (grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        assert_eq!(flat.len(), 0, "no selectable results");
        assert_eq!(grouped.len(), 1, "exactly one help header");
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert!(
                    label.contains("capture.v1/todo"),
                    "help header must name the target, got {label:?}"
                );
                assert!(
                    label.contains("No scripts opted"),
                    "help header must explain why"
                );
            }
            other => panic!("expected SectionHeader, got {other:?}"),
        }
    }

    #[test]
    fn only_opted_in_handlers_appear_and_shape_is_header_then_items() {
        let todo_handler = script_with_menu_syntax(
            "todo-handler",
            json!([
                { "family": "capture.v1", "targets": ["todo"] }
            ]),
        );
        let note_handler = script_with_menu_syntax(
            "note-handler",
            json!([
                { "family": "capture.v1", "targets": ["note"] }
            ]),
        );
        let wildcard_handler = script_with_menu_syntax(
            "wildcard-handler",
            json!([
                { "family": "capture.v1", "targets": ["*"] }
            ]),
        );
        let unrelated = plain_script("unrelated");

        let scripts = vec![todo_handler, note_handler, wildcard_handler, unrelated];
        let invocation = capture_from(";todo Renew passport");
        let (grouped, flat) = build_capture_mode_results(&scripts, &invocation);

        assert_eq!(
            flat.len(),
            2,
            "todo + wildcard must match, note and plain must not"
        );
        // First item in grouped must be the section header.
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert_eq!(label, "Capture todo");
            }
            other => panic!("first grouped entry must be the capture header, got {other:?}"),
        }
        // The rest must be Item rows in index-order.
        for (expected_idx, entry) in grouped.iter().skip(1).enumerate() {
            match entry {
                GroupedListItem::Item(i) => assert_eq!(*i, expected_idx),
                other => panic!("expected Item({expected_idx}), got {other:?}"),
            }
        }
        let names: Vec<&str> = flat
            .iter()
            .filter_map(|r| match r {
                SearchResult::Script(sm) => Some(sm.script.name.as_str()),
                _ => None,
            })
            .collect();
        assert!(names.contains(&"todo-handler"));
        assert!(names.contains(&"wildcard-handler"));
        assert!(!names.contains(&"note-handler"));
        assert!(!names.contains(&"unrelated"));
    }

    #[test]
    fn non_capture_family_never_matches_even_if_targets_include_target() {
        let impostor = script_with_menu_syntax(
            "impostor",
            json!([
                { "family": "query.v1", "targets": ["todo"] }
            ]),
        );
        let scripts = vec![impostor];
        let invocation = capture_from(";todo Renew passport");
        let (_grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        assert_eq!(
            flat.len(),
            0,
            "non-capture family must never match capture mode"
        );
    }

    #[test]
    fn keyword_alias_matches_same_handlers_as_plus_alias() {
        let handler = script_with_menu_syntax(
            "note-handler",
            json!([{ "family": "capture.v1", "targets": ["note"] }]),
        );
        let scripts = vec![handler];
        let plus = capture_from(";note buy batteries");
        let keyword = capture_from("note: buy batteries");
        let (_, flat_plus) = build_capture_mode_results(&scripts, &plus);
        let (_, flat_keyword) = build_capture_mode_results(&scripts, &keyword);
        assert_eq!(flat_plus.len(), 1);
        assert_eq!(flat_keyword.len(), 1);
    }

    #[test]
    fn incomplete_hint_row_is_single_non_selectable_header() {
        let (grouped, flat) =
            build_menu_syntax_hint_results("Type a capture target: todo, cal, note, social, link");
        assert!(
            flat.is_empty(),
            "incomplete rows never yield selectable results"
        );
        assert_eq!(grouped.len(), 1);
        match &grouped[0] {
            GroupedListItem::SectionHeader(label, None) => {
                assert!(label.contains("todo"));
            }
            other => panic!("expected SectionHeader, got {other:?}"),
        }
        for entry in grouped.iter() {
            assert!(
                !matches!(entry, GroupedListItem::Item(_)),
                "hint rows must never be Item entries (Item maps to a selectable flat result)"
            );
        }
    }

    #[test]
    fn menu_syntax_parse_incomplete_wires_into_hint_helper() {
        use crate::menu_syntax::MenuSyntaxParse;
        match parse("+") {
            MenuSyntaxParse::Incomplete(s) => {
                let (grouped, flat) = build_menu_syntax_hint_results(&s.hint);
                assert!(flat.is_empty());
                assert_eq!(grouped.len(), 1);
                let GroupedListItem::SectionHeader(label, None) = &grouped[0] else {
                    panic!("expected header")
                };
                assert_eq!(label, &s.hint);
            }
            other => panic!("expected Incomplete for '+' , got {other:?}"),
        }
    }

    #[test]
    fn every_result_carries_max_score_for_deterministic_order() {
        let a = script_with_menu_syntax(
            "a",
            json!([{ "family": "capture.v1", "targets": ["todo"] }]),
        );
        let b = script_with_menu_syntax(
            "b",
            json!([{ "family": "capture.v1", "targets": ["todo"] }]),
        );
        let scripts = vec![a, b];
        let invocation = capture_from(";todo something");
        let (_grouped, flat) = build_capture_mode_results(&scripts, &invocation);
        for r in flat {
            match r {
                SearchResult::Script(sm) => {
                    assert_eq!(sm.score, i32::MAX);
                    assert_eq!(sm.match_kind, ScriptMatchKind::Name);
                    assert!(sm.content_match.is_none());
                }
                other => panic!("expected Script, got {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod conversations_section_tests {
    use super::*;
    use crate::ai::conversations::{
        AgentChatSessionId, ConversationLiveness, ConversationRecord, ConversationSessionId,
        ConversationSurface, FlowSessionId, QuickAiSessionId,
    };
    use crate::brain::inbox::InboxKind;
    use crate::brain::{InboxItem, RootBrainInboxSectionOptions};
    use crate::flows::model::{FlowDescriptor, FlowSource};

    const NOW: i64 = 1_000_000;

    fn flow() -> FlowDescriptor {
        FlowDescriptor {
            id: "project:test".into(),
            path: "/tmp/flow-test.md".into(),
            source: FlowSource::Project,
            name: "flow-test".into(),
            description: Some("Test helper".into()),
            engine: "codex".into(),
            engine_source: None,
            inputs: vec![],
            is_workflow: false,
            interactive: true,
            mtime_ms: 0,
            origin: None,
            wrapper_command: None,
        }
    }

    fn at(secs: u64) -> std::time::SystemTime {
        std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs)
    }

    fn flow_record(id: u64, secs: u64) -> ConversationRecord {
        ConversationRecord {
            id: ConversationSessionId::Flow(FlowSessionId(id)),
            surface: ConversationSurface::Flow {
                flow_id: "project:test".into(),
            },
            title: "Release checklist".into(),
            subtitle: "Flow · codex".into(),
            last_activity: at(secs),
            liveness: ConversationLiveness::Idle,
        }
    }

    fn agent_chat_record(id: &str, secs: u64) -> ConversationRecord {
        ConversationRecord {
            id: ConversationSessionId::AgentChat(AgentChatSessionId(id.into())),
            surface: ConversationSurface::AgentChat {
                profile_id: "default".into(),
            },
            title: "Fix the footer color drift now".into(),
            subtitle: "Agent Chat · GPT-5.6".into(),
            last_activity: at(secs),
            liveness: ConversationLiveness::Live {
                turn_in_flight: true,
            },
        }
    }

    fn quick_ai_record(id: u64, secs: u64) -> ConversationRecord {
        ConversationRecord {
            id: ConversationSessionId::QuickAi(QuickAiSessionId(id)),
            surface: ConversationSurface::QuickAi,
            title: "Why is the tint moving?".into(),
            subtitle: "Quick AI · Spark".into(),
            last_activity: at(secs),
            liveness: ConversationLiveness::Idle,
        }
    }

    fn stable_keys(flat: &[SearchResult]) -> Vec<String> {
        flat.iter()
            .filter_map(SearchResult::stable_selection_key)
            .collect()
    }

    /// Oracle step 7: one flat Conversations section, interleaved strictly
    /// by activity across ALL THREE surfaces — surface kind is subtitle
    /// text, never a grouping key — pinned above Brain Inbox.
    #[test]
    fn mixed_surfaces_render_one_flat_section_above_brain_inbox() {
        let inbox = InboxItem {
            id: 1,
            kind: InboxKind::Commitment,
            title: "Brain row".into(),
            detail: String::new(),
            source: "test".into(),
            source_id: "test-1".into(),
            created_at: 1,
            resolved_at: None,
        };
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &[inbox],
            RootBrainInboxSectionOptions::default(),
            2,
            None,
        );
        let records = vec![
            flow_record(1, 100),
            agent_chat_record("a", 300),
            quick_ai_record(9, 200),
        ];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &records,
            &[flow()],
            NOW,
            None,
        );

        // Exactly ONE Conversations header, at the very top.
        let headers: Vec<&String> = grouped
            .iter()
            .filter_map(|item| match item {
                GroupedListItem::SectionHeader(label, _) if label == "Conversations" => Some(label),
                _ => None,
            })
            .collect();
        assert_eq!(headers.len(), 1, "one header for all three surfaces");
        assert!(
            matches!(&grouped[0], GroupedListItem::SectionHeader(label, _) if label == "Conversations")
        );

        // Interleaved by recency: agent chat (300), quick ai (200), flow (100).
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.conversation_id() == Some(&ConversationSessionId::AgentChat(AgentChatSessionId("a".into()))))
        );
        assert!(
            matches!(&flat[1], SearchResult::Flow(row) if row.conversation_id() == Some(&ConversationSessionId::QuickAi(QuickAiSessionId(9))))
        );
        assert!(matches!(&flat[2], SearchResult::Flow(row) if row.flow_session_id() == Some(1)));
        // Brain Inbox comes after every conversation.
        assert!(matches!(&flat[3], SearchResult::BrainInboxItem(_)));

        // No two rows share a stable key (tagged ids cannot collide).
        let keys = stable_keys(&flat);
        let mut deduped = keys.clone();
        deduped.sort();
        deduped.dedup();
        assert_eq!(keys.len(), deduped.len(), "stable keys must be unique");
    }

    /// No running-first pinning: a NEWER idle conversation outranks an OLDER
    /// running one. Running state is an indicator, not a sort key — a stale
    /// running operation must not stay pinned above rows the user touched.
    #[test]
    fn newer_idle_row_outranks_older_running_row() {
        let older_running = agent_chat_record("running", 100);
        let newer_idle = flow_record(2, 200);
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &[older_running, newer_idle],
            &[flow()],
            NOW,
            None,
        );
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.flow_session_id() == Some(2)),
            "idle-but-newer must lead"
        );
    }

    /// Running rows carry a VISIBLE, text-readable indicator; idle rows read
    /// Ready; failed-but-resumable rows read Needs attention and remain.
    #[test]
    fn liveness_lane_is_visible_text() {
        let mut failed = flow_record(3, 100);
        failed.liveness = ConversationLiveness::Failed {
            code: "ProviderOverloaded".into(),
        };
        let records = vec![agent_chat_record("a", 300), flow_record(1, 200), failed];
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &records,
            &[flow()],
            NOW,
            None,
        );
        let subtitle = |index: usize| match &flat[index] {
            SearchResult::Flow(row) => row.subtitle.clone(),
            other => panic!("expected conversation row, got {other:?}"),
        };
        assert!(subtitle(0).contains("● Working"), "{}", subtitle(0));
        assert!(subtitle(1).contains("Ready"), "{}", subtitle(1));
        assert!(
            subtitle(2).contains("Needs attention"),
            "failed-but-resumable row remains, marked: {}",
            subtitle(2)
        );
    }

    /// Empty store → no header, no placeholder row. Brain Inbox becomes the
    /// first section naturally.
    #[test]
    fn empty_store_renders_no_header() {
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(&mut grouped, &mut flat, "", &[], &[flow()], NOW, None);
        assert!(grouped.is_empty(), "no Conversations header when empty");
        assert!(flat.is_empty());
    }

    /// Oracle step 5 (submission 98cab5e5): the section orders by SEMANTIC
    /// recency, not creation order. Controlled clocks, no sleeping.
    #[test]
    fn returning_to_an_older_session_moves_it_back_to_the_top() {
        let older = flow_record(1, 100);
        let newer = flow_record(2, 200);

        // Creation order alone: newer (id 2) first.
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &[older.clone(), newer.clone()],
            &[flow()],
            NOW,
            None,
        );
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.flow_session_id() == Some(2)));

        // The user returns to the OLDER session: it must move to the top even
        // though its id is smaller.
        let mut touched_older = older;
        touched_older.last_activity = at(300);
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &[touched_older, newer],
            &[flow()],
            NOW,
            None,
        );
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.flow_session_id() == Some(1)),
            "resumed session must outrank a newer-created but untouched one"
        );
    }

    /// Stable selection follows the SESSION across a recency reorder: the
    /// same session id keeps the same stable key whichever row position it
    /// lands in, so selection restore tracks it rather than the old index.
    #[test]
    fn stable_keys_follow_sessions_across_reorder() {
        let build = |records: &[ConversationRecord]| {
            let mut grouped = vec![];
            let mut flat = vec![];
            prepend_root_conversations_section(
                &mut grouped,
                &mut flat,
                "",
                records,
                &[flow()],
                NOW,
                None,
            );
            stable_keys(&flat)
        };
        let before = build(&[flow_record(1, 100), flow_record(2, 200)]);
        let mut resumed = flow_record(1, 300);
        resumed.last_activity = at(300);
        let after = build(&[resumed, flow_record(2, 200)]);
        assert_eq!(before[0], after[1], "session 2 keeps its key");
        assert_eq!(before[1], after[0], "session 1 keeps its key");
    }

    #[test]
    fn equal_activity_falls_back_to_stable_id_order() {
        let a = flow_record(1, 500);
        let b = flow_record(2, 500);
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &[a, b],
            &[flow()],
            NOW,
            None,
        );
        assert!(matches!(&flat[0], SearchResult::Flow(row) if row.flow_session_id() == Some(2)));
        assert!(matches!(&flat[1], SearchResult::Flow(row) if row.flow_session_id() == Some(1)));
    }

    /// A flow conversation whose definition file vanished must NOT lose its
    /// row: resume only needs the session id, and hiding a live session
    /// would orphan its in-flight turn.
    #[test]
    fn missing_flow_descriptor_does_not_drop_the_row() {
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "",
            &[flow_record(7, 100)],
            &[], // no descriptors on disk
            NOW,
            None,
        );
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.flow_session_id() == Some(7) && row.flow.is_none())
        );
    }

    #[test]
    fn rows_match_typed_state_query() {
        let mut grouped = vec![];
        let mut flat = vec![];
        prepend_root_conversations_section(
            &mut grouped,
            &mut flat,
            "working",
            &[flow_record(1, 100), agent_chat_record("a", 200)],
            &[flow()],
            NOW,
            None,
        );
        assert_eq!(flat.len(), 1, "only the Working row matches");
        assert!(
            matches!(&flat[0], SearchResult::Flow(row) if row.conversation_id() == Some(&ConversationSessionId::AgentChat(AgentChatSessionId("a".into()))))
        );
    }
}

#[cfg(test)]
mod brain_inbox_section_tests {
    use super::*;
    use crate::brain::inbox::InboxKind;
    use crate::brain::{InboxItem, RootBrainInboxSectionOptions};

    const NOW: i64 = 1_000_000;

    fn inbox_item(id: i64, title: &str) -> InboxItem {
        InboxItem {
            id,
            kind: InboxKind::Commitment,
            title: title.to_string(),
            detail: String::new(),
            source: "chat_turn".to_string(),
            source_id: format!("thread-{id}#0"),
            created_at: NOW - 3_600,
            resolved_at: None,
        }
    }

    fn existing_row() -> SearchResult {
        SearchResult::ScriptIssue(ScriptIssueMatch {
            title: "Script Issues (1)".into(),
            description: None,
            failed_count: 1,
            fatal_count: 1,
            warning_count: 0,
            score: i32::MAX,
        })
    }

    fn base_view() -> (Vec<GroupedListItem>, Vec<SearchResult>) {
        (
            vec![
                GroupedListItem::SectionHeader("Main".to_string(), None),
                GroupedListItem::Item(0),
            ],
            vec![existing_row()],
        )
    }

    /// Asserts the view still looks exactly like [`base_view`] (no pin).
    fn assert_unpinned(grouped: &[GroupedListItem], flat: &[SearchResult], context: &str) {
        assert_eq!(grouped.len(), 2, "{context}: grouped length changed");
        assert!(
            matches!(&grouped[0], GroupedListItem::SectionHeader(label, None) if label == "Main"),
            "{context}: header changed: {:?}",
            grouped[0]
        );
        assert!(
            matches!(grouped[1], GroupedListItem::Item(0)),
            "{context}: item index shifted: {:?}",
            grouped[1]
        );
        assert_eq!(flat.len(), 1, "{context}: flat length changed");
        assert!(
            matches!(flat[0], SearchResult::ScriptIssue(_)),
            "{context}: flat row replaced"
        );
    }

    #[test]
    fn prepends_header_and_rows_at_top_and_shifts_existing_indices() {
        let (mut grouped, mut flat) = base_view();
        let items = vec![
            inbox_item(1, "follow up with sam"),
            inbox_item(2, "answer rust question"),
        ];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
            None,
        );

        assert!(
            matches!(
                &grouped[0],
                GroupedListItem::SectionHeader(label, Some(icon))
                    if label == "Brain Inbox" && icon == "inbox"
            ),
            "section header must be pinned at index 0, got {:?}",
            grouped[0]
        );
        assert!(matches!(grouped[1], GroupedListItem::Item(0)));
        assert!(matches!(grouped[2], GroupedListItem::Item(1)));
        // Existing rows keep pointing at the original results (shifted by 2).
        assert!(matches!(
            &grouped[3],
            GroupedListItem::SectionHeader(label, None) if label == "Main"
        ));
        assert!(matches!(grouped[4], GroupedListItem::Item(2)));
        assert!(matches!(flat[2], SearchResult::ScriptIssue(_)));

        // Rows preserve newest-first input order and carry inbox identity.
        match &flat[0] {
            SearchResult::BrainInboxItem(row) => {
                assert_eq!(row.item.id, 1);
                assert_eq!(
                    flat[0].history_result_key().as_deref(),
                    Some("brain-inbox/1")
                );
                assert!(
                    row.subtitle.starts_with("Commitment · "),
                    "subtitle should lead with the kind label, got {:?}",
                    row.subtitle
                );
            }
            other => panic!("expected BrainInboxItem at flat[0], got {other:?}"),
        }
        assert!(matches!(&flat[1], SearchResult::BrainInboxItem(row) if row.item.id == 2));
    }

    #[test]
    fn caps_rows_at_max_results() {
        let (mut grouped, mut flat) = base_view();
        let items: Vec<InboxItem> = (1..=5)
            .map(|id| inbox_item(id, &format!("item {id}")))
            .collect();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: true,
                max_results: 3,
            },
            NOW,
            None,
        );
        let inbox_rows = flat
            .iter()
            .filter(|row| matches!(row, SearchResult::BrainInboxItem(_)))
            .count();
        assert_eq!(inbox_rows, 3, "rows must be capped at max_results");
    }

    #[test]
    fn no_op_on_non_empty_query_disabled_section_or_empty_items() {
        let items = vec![inbox_item(1, "follow up with sam")];

        // Non-empty query (including whitespace-only being treated as empty).
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "git",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
            None,
        );
        assert_unpinned(&grouped, &flat, "non-empty query");

        // Disabled section.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: false,
                max_results: 3,
            },
            NOW,
            None,
        );
        assert_unpinned(&grouped, &flat, "disabled section");

        // max_results == 0.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &items,
            RootBrainInboxSectionOptions {
                enabled: true,
                max_results: 0,
            },
            NOW,
            None,
        );
        assert_unpinned(&grouped, &flat, "max_results=0");

        // No open items.
        let (mut grouped, mut flat) = base_view();
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "",
            &[],
            RootBrainInboxSectionOptions::default(),
            NOW,
            None,
        );
        assert_unpinned(&grouped, &flat, "empty items");
    }

    #[test]
    fn whitespace_only_query_counts_as_empty() {
        let (mut grouped, mut flat) = base_view();
        let items = vec![inbox_item(1, "follow up with sam")];
        prepend_root_brain_inbox_section(
            &mut grouped,
            &mut flat,
            "   ",
            &items,
            RootBrainInboxSectionOptions::default(),
            NOW,
            None,
        );
        assert!(
            matches!(
                &grouped[0],
                GroupedListItem::SectionHeader(label, _) if label == "Brain Inbox"
            ),
            "whitespace-only filter is the empty query"
        );
    }
}
