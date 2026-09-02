#[test]
fn test_get_grouped_results_selection_priority_with_frecency() {
    // This test verifies the SELECTION behavior, not just grouping.
    //
    // Bug: When user opens Script Kit, the FIRST SELECTABLE item should be
    // the most recently used item (from SUGGESTED), not the first item in MAIN.
    //
    // The grouped list structure determines what gets selected initially.
    // With frecency, the first Item (not SectionHeader) should be the
    // frecency script, which means selected_index=0 should point to
    // the frecency script when we skip headers.

    let scripts = wrap_scripts(vec![
        Script {
            name: "alpha-script".to_string(),
            path: PathBuf::from("/alpha-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "zebra-script".to_string(),
            path: PathBuf::from("/zebra-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins = create_test_builtins(); // Clipboard History, App Launcher
    let apps: Vec<AppInfo> = vec![];

    let mut frecency_store = FrecencyStore::new();
    frecency_store.record_use("/zebra-script.ts"); // Give frecency to zebra

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    // Find the first Item (not SectionHeader) - this is what gets selected
    let first_selectable_idx = grouped
        .iter()
        .find_map(|item| {
            if let GroupedListItem::Item(idx) = item {
                Some(*idx)
            } else {
                None
            }
        })
        .expect("Should have at least one selectable item");

    let first_result = &results[first_selectable_idx];

    // The first selectable item MUST be the frecency script
    // NOT Clipboard History (which would be first alphabetically in MAIN)
    assert_eq!(
        first_result.name(),
        "zebra-script",
        "First selectable item should be the frecency script 'zebra-script', got '{}'. \
             This bug causes Clipboard History to appear first regardless of user's frecency.",
        first_result.name()
    );

    // Verify the structure explicitly
    // grouped[0] = SectionHeader("Suggested")
    // grouped[1] = Item(zebra-script) <- THIS should be first selection
    // grouped[2] = SectionHeader("SCRIPTS") or next type-based section
    // grouped[3+] = Other items sorted alphabetically within their sections

    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s, _) => {
                let name = s.split(" · ").next().unwrap_or(s);
                format!("[{}]", name)
            }
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
            GroupedListItem::ReservedSectionSlot => "[reserved section slot]".to_string(),
            GroupedListItem::Status(status) => format!("[{}]", status.label),
        })
        .collect();

    // First 3 items should be: Suggested header, frecency item, Main header (kit-based section)
    // Scripts without kit_name default to "main" kit
    assert_eq!(
        &grouped_names[..3],
        &["[Suggested]", "zebra-script", "[Main]"],
        "First 3 items should be: Suggested header, frecency item, Main header. Got: {:?}",
        grouped_names
    );
}

#[test]
fn test_get_grouped_results_no_frecency_items_in_type_sections() {
    // This test verifies the type-based sectioning behavior.
    //
    // When there's NO frecency data, items are grouped into type-based sections:
    // - SCRIPTS: user scripts
    // - COMMANDS: builtins
    //
    // Items are sorted alphabetically within each section.
    // Sections appear in order: SCRIPTS, SCRIPTLETS, COMMANDS, APPS

    let scripts = wrap_scripts(vec![
        Script {
            name: "alpha-script".to_string(),
            path: PathBuf::from("/alpha-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "zebra-script".to_string(),
            path: PathBuf::from("/zebra-script.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins = create_test_builtins(); // Clipboard History, App Launcher
    let apps: Vec<AppInfo> = vec![];

    // No frecency - fresh start
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    // With no frecency and empty frecency store, items matching DEFAULT_SUGGESTED_ITEMS
    // appear in SUGGESTED section, remaining items go to their type-based sections.
    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s, _) => {
                let name = s.split(" · ").next().unwrap_or(s);
                format!("[{}]", name)
            }
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
            GroupedListItem::ReservedSectionSlot => "[reserved section slot]".to_string(),
            GroupedListItem::Status(status) => format!("[{}]", status.label),
        })
        .collect();

    // With default suggestions enabled and empty frecency, "Clipboard History" (which is in
    // DEFAULT_SUGGESTED_ITEMS) appears in SUGGESTED. App Launcher is not in defaults, stays in COMMANDS.
    // Expected structure:
    // [Suggested], Clipboard History (matches DEFAULT_SUGGESTED_ITEMS)
    // [Main], alpha-script, zebra-script
    // [Commands], App Launcher
    assert_eq!(
        grouped_names[0], "[Suggested]",
        "First item should be Suggested header when frecency is empty but defaults match. Got: {:?}",
        grouped_names
    );

    assert_eq!(
        grouped_names,
        vec![
            "[Suggested]",
            "Clipboard History",
            "[Main]",
            "alpha-script",
            "zebra-script",
            "[Commands]",
            "App Launcher",
        ],
        "Items matching DEFAULT_SUGGESTED_ITEMS should be in SUGGESTED, rest in type-based sections. Got: {:?}",
        grouped_names
    );
}

#[test]
fn test_get_grouped_results_prefers_last_selected_result_for_exact_query() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "open-alpha".to_string(),
            path: PathBuf::from("/open-alpha.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Open the alpha flow".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "open-zeta".to_string(),
            path: PathBuf::from("/open-zeta.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: Some("Open the zeta flow".to_string()),
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (baseline_grouped, baseline_results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "open",
        &SuggestedConfig::default(),
        &[],
        None,
    );
    let baseline_first = baseline_grouped
        .iter()
        .find_map(|item| match item {
            GroupedListItem::Item(idx) => Some(baseline_results[*idx].name().to_string()),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .expect("baseline search should have a selectable result");
    assert_eq!(baseline_first, "open-alpha");

    let mut input_history = crate::input_history::InputHistory::new();
    input_history.add_entry_with_selection("open", Some("script/main:open-zeta".to_string()));

    let (grouped, results) = get_grouped_results_with_input_history(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "open",
        &SuggestedConfig::default(),
        &[],
        None,
        Some(&input_history),
    );

    let remembered_first = grouped
        .iter()
        .find_map(|item| match item {
            GroupedListItem::Item(idx) => Some(results[*idx].name().to_string()),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .expect("remembered search should have a selectable result");
    assert_eq!(remembered_first, "open-zeta");
}

#[test]
fn test_get_grouped_results_prefers_last_selected_builtin_for_exact_query() {
    use crate::builtins::{BuiltInFeature, BuiltInGroup};

    let scripts = wrap_scripts(vec![]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins = vec![
        BuiltInEntry {
            id: "builtin/clipboard-history".to_string(),
            name: "Clipboard History".to_string(),
            description: "Browse clipboard history".to_string(),
            keywords: vec!["history".to_string(), "clipboard".to_string()],
            feature: BuiltInFeature::ClipboardHistory,
            icon: None,
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/agent_chat-history".to_string(),
            name: "Agent Chat History".to_string(),
            description: "Browse Agent Chat conversations".to_string(),
            keywords: vec!["history".to_string(), "conversation".to_string()],
            feature: BuiltInFeature::AgentChatHistory,
            icon: None,
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/dictation-history".to_string(),
            name: "Dictation History".to_string(),
            description: "Browse saved dictations".to_string(),
            keywords: vec!["history".to_string(), "dictation".to_string()],
            feature: BuiltInFeature::DictationHistory,
            icon: None,
            group: BuiltInGroup::Core,
        },
    ];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (baseline_grouped, baseline_results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "history",
        &SuggestedConfig::default(),
        &[],
        None,
    );
    let baseline_first = baseline_grouped
        .iter()
        .find_map(|item| match item {
            GroupedListItem::Item(idx) => Some(baseline_results[*idx].name().to_string()),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .expect("baseline builtin search should have a selectable result");
    assert_eq!(baseline_first, "Clipboard History");

    let mut input_history = crate::input_history::InputHistory::new();
    input_history
        .add_entry_with_selection("history", Some("builtin/dictation-history".to_string()));

    let (grouped, results) = get_grouped_results_with_input_history(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "history",
        &SuggestedConfig::default(),
        &[],
        None,
        Some(&input_history),
    );

    let remembered_first = grouped
        .iter()
        .find_map(|item| match item {
            GroupedListItem::Item(idx) => Some(results[*idx].name().to_string()),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .expect("remembered builtin search should have a selectable result");
    assert_eq!(remembered_first, "Dictation History");
}

#[test]
fn test_get_grouped_results_ai_vault_builtin_beats_stale_script_history() {
    use crate::builtins::{BuiltInFeature, BuiltInGroup};

    let scripts = wrap_scripts(vec![Script {
        name: "Vault".to_string(),
        path: PathBuf::from("/vault.ts"),
        extension: "ts".to_string(),
        icon: None,
        description: Some("Search past Agent Chat conversations".to_string()),
        alias: None,
        shortcut: None,
        ..Default::default()
    }]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins = vec![BuiltInEntry {
        id: "builtin/vault".to_string(),
        name: "Vault".to_string(),
        description: "Search AI Vault sessions...".to_string(),
        keywords: vec![
            "vault".to_string(),
            "ai-vault".to_string(),
            "aivault".to_string(),
        ],
        feature: BuiltInFeature::AiVault,
        icon: None,
        group: BuiltInGroup::Core,
    }];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();
    let mut input_history = crate::input_history::InputHistory::new();
    input_history.add_entry_with_selection("vault", Some("script/main:Vault".to_string()));

    let (grouped, results) = get_grouped_results_with_input_history(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "vault",
        &SuggestedConfig::default(),
        &[],
        None,
        Some(&input_history),
    );

    let remembered_first = grouped
        .iter()
        .find_map(|item| match item {
            GroupedListItem::Item(idx) => Some(&results[*idx]),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .expect("vault search should have a selectable result");
    assert_eq!(
        remembered_first.history_result_key().as_deref(),
        Some("builtin/vault"),
        "exact AI Vault aliases must route to the built-in even when stale input history points at the old Vault script"
    );
}

#[test]
fn test_get_grouped_results_excludes_legacy_vault_script_for_unrelated_query() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "Amazon".to_string(),
            path: PathBuf::from("/amazon.ts"),
            extension: "ts".to_string(),
            description: Some("Amazon helper".to_string()),
            ..Default::default()
        },
        Script {
            name: "Vault".to_string(),
            path: PathBuf::from("/vault.ts"),
            extension: "ts".to_string(),
            description: Some(
                "Search past AI conversations and resume the selected one in Script Kit"
                    .to_string(),
            ),
            alias: Some("vault".to_string()),
            body: Some("const amazon = 'poison';".into()),
            ..Default::default()
        },
    ]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &FrecencyStore::new(),
        "amazon",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    let visible_names: Vec<&str> = grouped
        .iter()
        .filter_map(|item| match item {
            GroupedListItem::Item(idx) => Some(results[*idx].name()),
            GroupedListItem::SectionHeader(_, _)
            | GroupedListItem::ReservedSectionSlot
            | GroupedListItem::Status(_) => None,
        })
        .collect();

    assert!(visible_names.contains(&"Amazon"));
    assert!(!visible_names.contains(&"Vault"));
}

#[test]
fn test_get_grouped_results_default_suggestions_for_new_users() {
    // When frecency store is empty (new user), items matching DEFAULT_SUGGESTED_ITEMS
    // should appear in the SUGGESTED section to help users discover features.
    use crate::builtins::{
        BuiltInFeature, BuiltInGroup, NotesCommandType, ScriptCommandType, UtilityCommandType,
    };

    // Create builtins that match DEFAULT_SUGGESTED_ITEMS plus one non-default command.
    let builtins = vec![
        BuiltInEntry {
            id: "builtin/ai-chat".to_string(),
            name: "Agent Chat".to_string(),
            description: "Chat with AI assistants".to_string(),
            keywords: vec!["ai".to_string(), "chat".to_string()],
            feature: BuiltInFeature::AiChat,
            icon: Some("🤖".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/do-in-current-app".to_string(),
            name: "Safari Commands".to_string(),
            description: "Use the current app's menu commands or generate automation".to_string(),
            keywords: vec!["current".to_string(), "app".to_string()],
            feature: BuiltInFeature::UtilityCommand(UtilityCommandType::DoInCurrentApp),
            icon: Some("target".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/new-script".to_string(),
            name: "New Script".to_string(),
            description: "Create a blank Script Kit script".to_string(),
            keywords: vec!["new".to_string(), "script".to_string()],
            feature: BuiltInFeature::ScriptCommand(ScriptCommandType::NewScript),
            icon: Some("plus".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/clipboard-history".to_string(),
            name: "Clipboard History".to_string(),
            description: "View clipboard history".to_string(),
            keywords: vec!["clipboard".to_string()],
            feature: BuiltInFeature::ClipboardHistory,
            icon: Some("📋".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/open-notes".to_string(),
            name: "Open Notes".to_string(),
            description: "Open the Notes window".to_string(),
            keywords: vec!["notes".to_string()],
            feature: BuiltInFeature::NotesCommand(NotesCommandType::OpenNotes),
            icon: Some("notebook-pen".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/file-search".to_string(),
            name: "Search Files".to_string(),
            description: "Browse directories, search files, and open results".to_string(),
            keywords: vec!["file".to_string(), "search".to_string()],
            feature: BuiltInFeature::FileSearch,
            icon: Some("folder-search".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/browser-tabs".to_string(),
            name: "Search Browser Tabs".to_string(),
            description: "Search open browser tabs".to_string(),
            keywords: vec!["browser".to_string(), "tabs".to_string()],
            feature: BuiltInFeature::BrowserTabs,
            icon: Some("globe".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/quick-terminal".to_string(),
            name: "Quick Terminal".to_string(),
            description: "Open a quick terminal".to_string(),
            keywords: vec!["terminal".to_string()],
            feature: BuiltInFeature::UtilityCommand(UtilityCommandType::QuickTerminal),
            icon: Some("square-terminal".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/sdk-reference".to_string(),
            name: "SDK Reference".to_string(),
            description: "Browse Script Kit SDK functions".to_string(),
            keywords: vec!["sdk".to_string(), "reference".to_string()],
            feature: BuiltInFeature::SdkReference,
            icon: Some("book-open".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/other".to_string(),
            name: "Some Other Command".to_string(),
            description: "Not in defaults".to_string(),
            keywords: vec!["other".to_string()],
            feature: BuiltInFeature::AppLauncher,
            icon: Some("🔧".to_string()),
            group: BuiltInGroup::Core,
        },
    ];

    let scripts: Vec<Arc<Script>> = vec![];
    let scriptlets: Vec<Arc<Scriptlet>> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new(); // Empty frecency

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s, _) => {
                let name = s.split(" · ").next().unwrap_or(s);
                format!("[{}]", name)
            }
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
            GroupedListItem::ReservedSectionSlot => "[reserved section slot]".to_string(),
            GroupedListItem::Status(status) => format!("[{}]", status.label),
        })
        .collect();

    // Default suggestions should appear in order from DEFAULT_SUGGESTED_ITEMS
    // (reordered by observed user frequency; "Do in Current App" leads and is
    // rendered with its dynamic per-app name, here "Safari Commands").
    // "SDK Reference" is no longer a default suggestion, so it lands in
    // COMMANDS alongside "Some Other Command".
    assert_eq!(
        grouped_names,
        vec![
            "[Suggested]",
            "Safari Commands",
            "Agent Chat",
            "Search Files",
            "Clipboard History",
            "Search Browser Tabs",
            "Quick Terminal",
            "Open Notes",
            "New Script",
            "[Commands]",
            "SDK Reference",
            "Some Other Command",
        ],
        "Default suggested items should appear in SUGGESTED for new users, others in COMMANDS. Got: {:?}",
        grouped_names
    );
}

#[test]
fn test_default_suggested_items_match_real_builtin_catalog() {
    let config = crate::config::BuiltInConfig::default();
    let entries = crate::builtins::get_builtin_entries(&config);
    let builtin_names: std::collections::HashSet<&str> =
        entries.iter().map(|entry| entry.name.as_str()).collect();

    let missing: Vec<&str> = super::grouping::DEFAULT_SUGGESTED_ITEMS
        .iter()
        .copied()
        .filter(|name| !builtin_names.contains(name))
        .collect();

    assert!(
        missing.is_empty(),
        "DEFAULT_SUGGESTED_ITEMS must use exact built-in names. Missing: {:?}",
        missing
    );
}

#[test]
fn test_get_grouped_results_no_default_suggestions_when_frecency_exists() {
    // When frecency store has data, don't use default suggestions
    use crate::builtins::{BuiltInFeature, BuiltInGroup, NotesCommandType};

    let builtins = vec![
        BuiltInEntry {
            id: "builtin/ai-chat".to_string(),
            name: "Agent Chat".to_string(),
            description: "Chat with AI assistants".to_string(),
            keywords: vec!["ai".to_string()],
            feature: BuiltInFeature::AiChat,
            icon: Some("🤖".to_string()),
            group: BuiltInGroup::Core,
        },
        BuiltInEntry {
            id: "builtin/open-notes".to_string(),
            name: "Open Notes".to_string(),
            description: "Open the Notes window".to_string(),
            keywords: vec!["notes".to_string()],
            feature: BuiltInFeature::NotesCommand(NotesCommandType::OpenNotes),
            icon: Some("notebook-pen".to_string()),
            group: BuiltInGroup::Core,
        },
    ];

    let scripts: Vec<Arc<Script>> = vec![];
    let scriptlets: Vec<Arc<Scriptlet>> = vec![];
    let apps: Vec<AppInfo> = vec![];

    // Create frecency store with some data (user is not new)
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!(
        "frecency_test_defaults_{}.json",
        uuid::Uuid::new_v4()
    ));
    let mut frecency_store = FrecencyStore::with_path(temp_path.clone());
    frecency_store.record_use("builtin:builtin/open-notes"); // Record usage for Open Notes

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    let grouped_names: Vec<String> = grouped
        .iter()
        .map(|item| match item {
            GroupedListItem::SectionHeader(s, _) => {
                let name = s.split(" · ").next().unwrap_or(s);
                format!("[{}]", name)
            }
            GroupedListItem::Item(idx) => results[*idx].name().to_string(),
            GroupedListItem::ReservedSectionSlot => "[reserved section slot]".to_string(),
            GroupedListItem::Status(status) => format!("[{}]", status.label),
        })
        .collect();

    // With frecency data, SUGGESTED shows frecency-based items (Open Notes has usage)
    // Agent Chat has no usage, goes to COMMANDS
    assert_eq!(
        grouped_names,
        vec!["[Suggested]", "Open Notes", "[Commands]", "Agent Chat",],
        "With frecency data, should use frecency-based suggestions, not defaults. Got: {:?}",
        grouped_names
    );

    // Cleanup temp file
    let _ = std::fs::remove_file(temp_path);
}

#[test]
fn test_get_grouped_results_empty_inputs() {
    let scripts: Vec<Arc<Script>> = wrap_scripts(vec![]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    // Both should be empty when no inputs
    assert!(results.is_empty());
    assert!(grouped.is_empty());
}

#[test]
fn test_get_grouped_results_items_reference_correct_indices() {
    let scripts = wrap_scripts(vec![
        Script {
            name: "first".to_string(),
            path: PathBuf::from("/first.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
        Script {
            name: "second".to_string(),
            path: PathBuf::from("/second.ts"),
            extension: "ts".to_string(),
            icon: None,
            description: None,
            alias: None,
            shortcut: None,
            ..Default::default()
        },
    ]);
    let scriptlets: Vec<Arc<Scriptlet>> = wrap_scriptlets(vec![]);
    let builtins: Vec<BuiltInEntry> = vec![];
    let apps: Vec<AppInfo> = vec![];
    let frecency_store = FrecencyStore::new();

    let (grouped, results) = get_grouped_results(
        &scripts,
        &scriptlets,
        &builtins,
        &apps,
        &[],
        &frecency_store,
        "",
        &SuggestedConfig::default(),
        &[],
        None,
    );

    // All Item indices should be valid indices into results
    for item in &grouped {
        if let GroupedListItem::Item(idx) = item {
            assert!(
                *idx < results.len(),
                "Index {} out of bounds for results len {}",
                idx,
                results.len()
            );
        }
    }
}
