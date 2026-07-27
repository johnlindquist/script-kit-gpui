use std::{fs, path::Path};

fn read(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
}

fn rust_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn agent_chat_entry_request_is_the_cmd_enter_handoff_choke_point() {
    let source = read("src/app_impl/agent_handoff/agent_chat_entry.rs");
    for needle in [
        "pub(crate) enum AgentChatEntryOrigin",
        "pub(crate) enum AgentChatThreadTarget",
        "pub(crate) enum AgentChatSeedPolicy",
        "pub(crate) struct AgentChatEntryRequest",
        "pub(crate) fn open_agent_chat_from_entry_request",
        "blocks_launcher_ai_entry()",
    ] {
        assert!(source.contains(needle), "missing {needle}");
    }
    let body_start = source
        .find("pub(crate) fn open_agent_chat_from_entry_request")
        .expect("entry request opener should exist");
    let body = &source[body_start..];
    assert!(
        body.contains("self.seed_agent_chat_return_origin_for_view(&source_view)")
            && body.contains("event = \"agent_chat_entry_request_open\""),
        "entry request opener must seed return origin and log the handoff before launch dispatch"
    );
    for forbidden in [
        "self.current_view = AppView::AgentChatView",
        "ensure_embedded_ai_window(",
        "transition_agent_chat_surface(",
    ] {
        assert!(
            !body.contains(forbidden),
            "entry request opener must not mutate the Agent Chat surface directly: {forbidden}"
        );
    }
}

#[test]
fn cmd_enter_origins_have_real_source_callers() {
    let tab_ai = read("src/app_impl/agent_handoff/mod.rs");
    let notes = read("src/notes/window/keyboard.rs");
    for needle in [
        "AgentChatEntryOrigin::MainLauncher",
        "AgentChatEntryOrigin::FileSearch",
        "AgentChatEntryOrigin::ActionsDialog",
        "AgentChatEntryOrigin::PluginSkill",
    ] {
        assert!(tab_ai.contains(needle), "missing caller for {needle}");
    }
    assert!(
        notes.contains("handoff_selected_note_to_main_agent_chat"),
        "Notes Cmd+Enter must route through the notes->main Agent Chat handoff"
    );
}

#[test]
fn file_search_and_actions_do_not_stage_context_parts_directly() {
    for dir in ["src/file_search", "src/actions"] {
        let mut files = Vec::new();
        rust_files(&Path::new(env!("CARGO_MANIFEST_DIR")).join(dir), &mut files);
        for path in files {
            let src = fs::read_to_string(&path).unwrap();
            assert!(
                !src.contains("thread.add_context_part"),
                "{} must route Agent Chat context through AgentChatEntryRequest/staging helpers",
                path.display()
            );
        }
    }
}
