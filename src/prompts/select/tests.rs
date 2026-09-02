use super::prompt::collect_select_prompt_elements;
use super::*;

fn choice(name: &str, value: &str, description: Option<&str>) -> Choice {
    let mut choice = Choice::new(name.to_string(), value.to_string());
    choice.description = description.map(str::to_string);
    choice
}

#[test]
fn metadata_parses_shortcut_type_and_last_run() {
    let choice = choice(
        "Deploy API",
        "/Users/me/.scriptkit/scripts/deploy.ts",
        Some("Shortcut: cmd+shift+d • script • Last run 2h ago"),
    );

    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(metadata.shortcut.as_deref(), Some("⌘⇧D"));
    assert_eq!(metadata.item_type.as_deref(), Some("Script"));
    assert_eq!(metadata.last_run.as_deref(), Some("Last run 2h ago"));
    assert!(metadata.description.is_none());
}

#[test]
fn infer_script_from_ts_hash_fragment() {
    let choice = choice("Demo", "/tmp/demo.ts#section", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(
        metadata
            .item_type
            .expect("expected .ts# fragment to infer Script"),
        "Script"
    );
}

#[test]
fn infer_scriptlet_from_md_hash() {
    let choice = choice("Readme Section", "/tmp/readme.md#foo", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(
        metadata
            .item_type
            .expect("expected .md# fragment to infer Scriptlet"),
        "Scriptlet"
    );
}

#[test]
fn infer_extension_from_path() {
    let choice = choice("My Scriptlet", "/home/user/scriptlets/my-ext", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(
        metadata
            .item_type
            .expect("expected /scriptlets/ path to infer Scriptlet"),
        "Scriptlet"
    );
}

#[test]
fn infer_agent_from_name() {
    let choice = choice("My Agent", "/tools/helper", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(
        metadata
            .item_type
            .expect("expected name containing agent to infer Agent"),
        "Agent"
    );
}

#[test]
fn infer_script_from_ts_extension() {
    let choice = choice("Run Task", "/scripts/run.ts", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert_eq!(
        metadata
            .item_type
            .expect("expected .ts extension to infer Script"),
        "Script"
    );
}

#[test]
fn infer_none_no_signals() {
    let choice = choice("Settings", "settings", None);
    let metadata = ChoiceDisplayMetadata::from_choice(&choice);

    assert!(metadata.item_type.is_none());
}

#[test]
fn score_choice_matches_description_and_value() {
    let choice = choice(
        "Deploy",
        "/Users/me/.scriptkit/scripts/deploy-api.ts",
        Some("Publish service to production"),
    );

    let mut description_ctx = scripts::NucleoCtx::new("production");
    let mut value_ctx = scripts::NucleoCtx::new("deploy-api.ts");
    let indexed_choice = SelectChoiceIndex::from_choice(&choice, 0);

    assert!(
        score_choice_for_filter(&choice, &indexed_choice, "production", &mut description_ctx)
            .is_some()
    );
    assert!(
        score_choice_for_filter(&choice, &indexed_choice, "deploy-api.ts", &mut value_ctx)
            .is_some()
    );

    let mut miss_ctx = scripts::NucleoCtx::new("zzzzzz-no-match");
    assert!(
        score_choice_for_filter(&choice, &indexed_choice, "zzzzzz-no-match", &mut miss_ctx)
            .is_none()
    );
}

#[test]
fn score_choice_prefers_name_over_description_only_matches() {
    let name_match = choice(
        "Open Logs",
        "/tmp/open-logs.ts",
        Some("Tail runtime output"),
    );
    let description_match = choice("Tail Runtime", "/tmp/tail-runtime.ts", Some("Open logs"));
    let query = "open logs";

    let mut name_ctx = scripts::NucleoCtx::new(query);
    let mut description_ctx = scripts::NucleoCtx::new(query);
    let indexed_name_match = SelectChoiceIndex::from_choice(&name_match, 0);
    let indexed_description_match = SelectChoiceIndex::from_choice(&description_match, 1);

    let name_score =
        score_choice_for_filter(&name_match, &indexed_name_match, query, &mut name_ctx).unwrap();
    let description_score = score_choice_for_filter(
        &description_match,
        &indexed_description_match,
        query,
        &mut description_ctx,
    )
    .unwrap();

    assert!(
        name_score > description_score,
        "expected name match score ({name_score}) to beat description-only score ({description_score})"
    );
}

#[test]
fn char_indices_to_byte_ranges_handles_utf8_boundaries() {
    let text = "a😀b";
    // Indices for 😀 and b
    let ranges = char_indices_to_byte_ranges(text, &[1, 2]);
    assert_eq!(ranges, vec![1..6]);
}

#[test]
fn test_select_prompt_accepts_space_and_rejects_control_chars_in_filter_query() {
    assert!(should_append_to_filter(' '));
    assert!(should_append_to_filter('x'));
    assert!(!should_append_to_filter('\n'));
    assert!(!should_append_to_filter('\u{0000}'));
}

#[test]
fn test_select_prompt_submit_uses_focused_item_in_single_mode_when_none_toggled() {
    let selected_indices = Vec::new();
    let resolved = resolve_submission_indices(false, &selected_indices, Some(4));
    assert_eq!(resolved, vec![4]);
}

#[test]
fn test_select_prompt_submit_prefers_focused_item_in_single_mode_when_selection_exists() {
    let selected_indices = vec![2];
    let resolved = resolve_submission_indices(false, &selected_indices, Some(4));
    assert_eq!(resolved, vec![4]);
}

#[test]
fn test_select_prompt_submit_uses_explicit_selection_in_multiple_mode() {
    let selected_indices = vec![2, 7];
    let resolved = resolve_submission_indices(true, &selected_indices, Some(4));
    assert_eq!(resolved, vec![2, 7]);
}

#[test]
fn test_select_prompt_zero_selection_is_blocked_only_in_multiple_mode() {
    assert!(!select_submission_is_allowed(true, 0));
    assert!(select_submission_is_allowed(true, 1));
    assert!(select_submission_is_allowed(false, 0));
}

#[test]
fn test_select_prompt_cmd_a_toggles_only_when_all_filtered_items_are_selected() {
    let mut selected_indices = std::collections::HashSet::from([1, 7]);
    let filtered_indices = vec![1, 2, 3];

    assert!(!are_all_filtered_selected(
        &selected_indices,
        &filtered_indices
    ));
    toggle_filtered_selection(&mut selected_indices, &filtered_indices);
    assert_eq!(
        selected_indices,
        std::collections::HashSet::from([1, 2, 3, 7])
    );

    assert!(are_all_filtered_selected(
        &selected_indices,
        &filtered_indices
    ));
    toggle_filtered_selection(&mut selected_indices, &filtered_indices);
    assert_eq!(selected_indices, std::collections::HashSet::from([7]));
}

#[test]
fn test_select_prompt_select_all_preserves_existing_off_filter_selection() {
    let mut selected_indices = std::collections::HashSet::from([9]);
    let filtered_indices = vec![1, 2, 3];

    toggle_filtered_selection(&mut selected_indices, &filtered_indices);

    assert_eq!(
        selected_indices,
        std::collections::HashSet::from([1, 2, 3, 9])
    );
}

#[test]
fn test_select_prompt_emits_minimal_list_chrome_audit() {
    const SOURCE: &str = include_str!("prompt.rs");
    assert!(
        SOURCE.contains("emit_prompt_chrome_audit(")
            && SOURCE.contains("PromptChromeAudit::minimal_list(\"prompts::select\", true)"),
        "SelectPrompt::new should emit the shared minimal-list chrome audit contract"
    );
}

#[test]
fn test_select_prompt_semantic_id_honors_explicit_protocol_id() {
    let mut choice = choice("Demo", "scripts/demo.ts", None);
    choice.set_semantic_id("choice:custom-demo".to_string());

    assert_eq!(select_choice_semantic_id(&choice, 17), "choice:custom-demo");
}

#[test]
fn test_select_prompt_semantic_id_uses_source_index_when_filter_order_changes() {
    let choice = choice("Demo", "scripts/demo.ts", None);
    let stable_id = select_choice_semantic_id(&choice, 17);

    assert_eq!(stable_id, select_choice_semantic_id(&choice, 17));
    assert_ne!(stable_id, select_choice_semantic_id(&choice, 3));
}

#[test]
fn test_select_prompt_resolves_search_box_bg_by_design_variant() {
    let mut theme = theme::Theme::default();
    theme.colors.background.search_box = 0x112233;

    let design_colors = DesignColors {
        background_secondary: 0x445566,
        ..Default::default()
    };

    assert_eq!(
        resolve_search_box_bg_hex(&theme, DesignVariant::Default, &design_colors),
        0x112233
    );
    assert_eq!(
        resolve_search_box_bg_hex(&theme, DesignVariant::Minimal, &design_colors),
        0x445566
    );
}

#[test]
fn test_select_prompt_extracts_icon_hint_from_choice_metadata() {
    let description = Some("Shortcut: cmd+k • icon: terminal • script");
    assert_eq!(
        render::extract_choice_icon_hint(description),
        Some("terminal")
    );
}

#[test]
fn test_select_prompt_icon_kind_falls_back_to_code_icon_without_hint() {
    let choice = choice("Deploy API", "/Users/me/.scriptkit/scripts/deploy.ts", None);

    match render::icon_kind_from_choice(&choice) {
        IconKind::Svg(name) => assert_eq!(name, "Code"),
        _ => panic!("expected fallback Code SVG icon"),
    }
}

#[test]
fn test_select_prompt_compute_row_state_keeps_focus_and_selection_independent() {
    let selected = std::collections::HashSet::from([5]);
    let focused_but_unselected = render::compute_row_state(2, 2, 1, &selected, Some(1));
    assert!(focused_but_unselected.is_focused);
    assert!(!focused_but_unselected.is_selected);
    assert!(!focused_but_unselected.is_hovered);

    let selected_but_unfocused = render::compute_row_state(1, 2, 5, &selected, Some(1));
    assert!(!selected_but_unfocused.is_focused);
    assert!(selected_but_unfocused.is_selected);
    assert!(selected_but_unfocused.is_hovered);
}

#[test]
fn test_select_prompt_single_mode_visual_state_ignores_stale_selection() {
    let row_state = render::SelectRowState {
        is_focused: false,
        is_selected: true,
        is_hovered: false,
    };

    assert_eq!(
        render::visual_row_state_for_selection_mode(row_state, false),
        render::SelectRowState {
            is_focused: false,
            is_selected: false,
            is_hovered: false,
        }
    );
    assert_eq!(
        render::visual_row_state_for_selection_mode(row_state, true),
        row_state
    );
}

#[test]
fn test_select_prompt_toggle_selection_is_multi_select_only() {
    let mut selected = std::collections::HashSet::from([2]);

    assert!(!toggle_choice_selection(&mut selected, 4, false));
    assert_eq!(selected, std::collections::HashSet::from([2]));

    assert!(toggle_choice_selection(&mut selected, 4, true));
    assert_eq!(selected, std::collections::HashSet::from([2, 4]));

    assert!(toggle_choice_selection(&mut selected, 4, true));
    assert_eq!(selected, std::collections::HashSet::from([2]));
}

// ============================================================
// SelectPrompt collect_elements tests
// ============================================================

#[test]
fn test_select_prompt_get_elements_returns_visible_choices() {
    let choices = vec![
        Choice::new("Apple".to_string(), "apple".to_string()),
        Choice::new("Banana".to_string(), "banana".to_string()),
        Choice::new("Cherry".to_string(), "cherry".to_string()),
    ];
    let filtered: Vec<usize> = (0..choices.len()).collect();
    let selected: HashSet<usize> = HashSet::new();

    let (elements, total_count) =
        collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 50);

    // total_count = 3 choices + 1 input + 1 list = 5
    assert_eq!(total_count, 5);
    assert_eq!(elements.len(), 5);

    // First element: input
    assert_eq!(elements[0].semantic_id, "input:select-filter");
    assert_eq!(
        elements[0].element_type,
        crate::protocol::ElementType::Input
    );

    // Second element: list container
    assert_eq!(elements[1].semantic_id, "list:select-choices");
    assert_eq!(elements[1].element_type, crate::protocol::ElementType::List);

    // Choice rows
    assert_eq!(
        elements[2].element_type,
        crate::protocol::ElementType::Choice
    );
    assert_eq!(elements[2].text.as_deref(), Some("Apple"));
    assert_eq!(elements[2].value.as_deref(), Some("apple"));
    assert_eq!(elements[2].selected, Some(false));
    assert_eq!(elements[2].focused, Some(true)); // focused_index == 0
    assert_eq!(elements[2].index, Some(0));

    assert_eq!(elements[3].text.as_deref(), Some("Banana"));
    assert_eq!(elements[3].focused, Some(false));

    assert_eq!(elements[4].text.as_deref(), Some("Cherry"));
}

#[test]
fn test_select_prompt_get_elements_respects_limit() {
    let choices = vec![
        Choice::new("Apple".to_string(), "apple".to_string()),
        Choice::new("Banana".to_string(), "banana".to_string()),
        Choice::new("Cherry".to_string(), "cherry".to_string()),
    ];
    let filtered: Vec<usize> = (0..choices.len()).collect();
    let selected: HashSet<usize> = HashSet::new();

    // limit=1 should return only the input element
    let (elements, total_count) =
        collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 1);
    assert_eq!(elements.len(), 1);
    assert_eq!(total_count, 5);
    assert_eq!(elements[0].semantic_id, "input:select-filter");

    // limit=3 should return input + list + first choice
    let (elements, total_count) =
        collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 3);
    assert_eq!(elements.len(), 3);
    assert_eq!(total_count, 5);
    assert_eq!(
        elements[2].element_type,
        crate::protocol::ElementType::Choice
    );
}

#[test]
fn test_select_prompt_get_elements_uses_stable_key_semantic_id() {
    let choices = vec![
        Choice::new("Apple".to_string(), "apple".to_string()).with_key("fruit-apple".to_string()),
        Choice::new("Banana".to_string(), "banana".to_string()),
    ];
    let filtered: Vec<usize> = (0..choices.len()).collect();
    let selected: HashSet<usize> = HashSet::new();

    let (elements, _) = collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 50);

    // Keyed choice uses choice:key format
    assert_eq!(elements[2].semantic_id, "choice:fruit-apple");
    // Non-keyed choice uses an index plus a privacy-safe value fingerprint.
    assert_eq!(elements[3].semantic_id, choices[1].generate_id(1));
    assert!(!elements[3].semantic_id.contains("banana"));
}

#[test]
fn test_select_prompt_get_elements_uses_explicit_semantic_id() {
    let mut custom = Choice::new("Apple".to_string(), "apple".to_string());
    custom.set_semantic_id("choice:explicit-apple".to_string());
    let choices = vec![custom];
    let filtered = vec![0];
    let selected: HashSet<usize> = HashSet::new();

    let (elements, _) = collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 50);

    assert_eq!(elements[2].semantic_id, "choice:explicit-apple");
}

#[test]
fn test_select_prompt_get_elements_uses_source_index_after_filter_order_changes() {
    let choices = vec![
        Choice::new("Apple".to_string(), "apple".to_string()),
        Choice::new("Banana".to_string(), "banana".to_string()),
        Choice::new("Cherry".to_string(), "cherry".to_string()),
    ];
    let filtered = vec![2, 0];
    let selected: HashSet<usize> = HashSet::new();

    let (elements, _) = collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 50);

    assert_eq!(elements[2].semantic_id, choices[2].generate_id(2));
    assert_eq!(elements[3].semantic_id, choices[0].generate_id(0));
    assert!(!elements[2].semantic_id.contains("cherry"));
    assert!(!elements[3].semantic_id.contains("apple"));
}

#[test]
fn test_select_prompt_get_elements_reflects_selection_state() {
    let choices = vec![
        Choice::new("Apple".to_string(), "apple".to_string()),
        Choice::new("Banana".to_string(), "banana".to_string()),
    ];
    let filtered: Vec<usize> = (0..choices.len()).collect();
    let mut selected = HashSet::new();
    selected.insert(1); // Select Banana

    let (elements, _) = collect_select_prompt_elements("", &choices, &filtered, &selected, 0, 50);

    assert_eq!(elements[2].selected, Some(false)); // Apple not selected
    assert_eq!(elements[3].selected, Some(true)); // Banana selected
}

#[gpui::test]
fn real_select_filters_toggles_and_refuses_disabled_values(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let submitted = Arc::new(parking_lot::Mutex::new(Vec::new()));
    let output = submitted.clone();
    let prompt = cx.new(|cx| {
        SelectPrompt::new(
            "select-local".into(),
            None,
            vec![
                choice("Alpha", "a", None),
                choice("Beta", "b", None),
                choice("Unavailable", "c", None),
            ],
            true,
            cx.focus_handle(),
            Arc::new(move |_, value| output.lock().push(value)),
            Arc::new(theme::Theme::default()),
        )
        .with_disabled_choices([2])
    });
    prompt.update(cx, |prompt, cx| {
        assert!(!prompt.submit(cx));
        prompt.set_input("Beta".into(), cx);
        assert_eq!(prompt.filtered_choices, vec![1]);
        prompt.toggle_selection(cx);
        prompt.set_input(String::new(), cx);
        prompt.set_focused_index(2);
        prompt.toggle_selection(cx);
        assert!(!prompt.selected.contains(&2));
        let (elements, _) = prompt.collect_elements(20);
        assert_eq!(
            elements.last().unwrap().action_disabled,
            Some("choice_disabled".to_string())
        );
        assert!(prompt.submit(cx));
    });
    assert_eq!(*submitted.lock(), vec![Some("[\"b\"]".into())]);
}

#[gpui::test]
fn select_revision_tracks_text_and_selection_aba_not_no_ops(cx: &mut gpui::TestAppContext) {
    use gpui::AppContext as _;
    let prompt = cx.new(|cx| {
        SelectPrompt::new(
            "epoch".into(),
            None,
            vec![choice("Alpha", "a", None), choice("Beta", "b", None)],
            true,
            cx.focus_handle(),
            Arc::new(|_, _| {}),
            Arc::new(theme::Theme::default()),
        )
    });
    prompt.update(cx, |prompt, cx| {
        let initial = prompt.dictation_input_revision(cx);
        prompt.set_input(String::new(), cx);
        prompt.handle_backspace(cx);
        prompt.move_up(cx);
        assert_eq!(prompt.dictation_input_revision(cx), initial);
        prompt.set_input("a".into(), cx);
        let filtered = prompt.dictation_input_revision(cx);
        prompt.set_input("b".into(), cx);
        assert!(prompt.dictation_input_revision(cx) > filtered);
        prompt.set_input("a".into(), cx);
        assert!(prompt.dictation_input_revision(cx) > filtered);
        prompt.set_input(String::new(), cx);
        let before_focus = prompt.dictation_input_revision(cx);
        prompt.move_down(cx);
        prompt.move_up(cx);
        assert!(prompt.dictation_input_revision(cx) > before_focus);
        let before_toggle = prompt.dictation_input_revision(cx);
        prompt.toggle_selection(cx);
        prompt.toggle_selection(cx);
        assert!(prompt.selected.is_empty());
        assert!(prompt.dictation_input_revision(cx) > before_toggle);
        let before_read = prompt.dictation_input_revision(cx);
        prompt.collect_elements(20);
        assert_eq!(prompt.dictation_input_revision(cx), before_read);
    });
}
