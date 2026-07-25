//! Source audit for the Agent Chat / Agent Chat rename boundary.
//!
//! The project renamed the *feature* to "Agent Chat" while keeping "Agent Chat" as the
//! name of frozen compatibility contracts (action IDs, route IDs, serialized
//! surface IDs, `getAgentChatState`, telemetry labels). This audit proves two things
//! at once:
//!
//! 1. The canonical `agent_chat::ui` boundary exists and is wired into the
//!    launcher view state, so new code has a stable `AgentChat*` import surface.
//! 2. The frozen external contracts are still present verbatim. If a future
//!    rename pass deletes or edits one of these strings, this audit fails and
//!    forces the change to be deliberate (and paired with a contract migration).

use std::fs;
use std::path::{Path, PathBuf};

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

fn collect_rs_files(root: impl AsRef<Path>, files: &mut Vec<PathBuf>) {
    let root = root.as_ref();
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn agent_chat_ui_boundary_exists() {
    let mod_rs = read("src/ai/agent_chat/mod.rs");
    assert!(
        mod_rs.contains("pub(crate) mod ui;"),
        "agent_chat must expose the canonical `ui` boundary module"
    );
    assert!(
        mod_rs.contains("pub(crate) mod content;"),
        "agent_chat must expose the `content` boundary module"
    );

    let ui = read("src/ai/agent_chat/ui/mod.rs");
    let reexports = reexported_names(&ui);
    for name in [
        "AgentChatView",
        "AgentChatThread",
        "AgentChatEvent",
        "AgentChatSession",
        "AgentChatInlineSetupState",
        "AgentChatRetryRequest",
        "AgentChatPermissionBroker",
    ] {
        assert!(
            reexports.iter().any(|item| item == name),
            "agent_chat::ui must re-export `{name}` as part of the canonical boundary"
        );
    }
}

/// Names re-exported by a module's `pub(crate) use ...;` statements.
///
/// This used to be asserted as the literal text `"AgentChatView as AgentChatView"`.
/// That spelling is a redundant self-alias, so a routine cleanup deleted the
/// `as` halves and every one of these assertions went red while the boundary
/// itself was untouched — all seven types are still re-exported. The invariant
/// worth keeping is "the facade re-exports this type", not "the facade spells
/// it in the one style it happened to use in 2026".
///
/// Parsing the use statements also makes the check STRICTER than the old
/// substring test: a type merely mentioned in a doc comment or a function body
/// no longer counts, because only text inside `pub(crate) use ... ;` is
/// considered.
fn reexported_names(source: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = source;
    while let Some(start) = rest.find("pub(crate) use ") {
        let after = &rest[start + "pub(crate) use ".len()..];
        let Some(end) = after.find(';') else { break };
        let statement = &after[..end];
        // Keep only the leaf identifiers: drop path prefixes and braces, and
        // resolve `foo as bar` to the exported name `bar`.
        for piece in statement.split([',', '{', '}']) {
            let piece = piece.trim();
            if piece.is_empty() {
                continue;
            }
            let exported = match piece.rsplit_once(" as ") {
                Some((_, alias)) => alias.trim(),
                None => piece.rsplit("::").next().unwrap_or(piece).trim(),
            };
            if !exported.is_empty() && exported.chars().all(|c| c.is_alphanumeric() || c == '_') {
                names.push(exported.to_string());
            }
        }
        rest = &after[end..];
    }
    names
}

#[test]
fn app_view_state_uses_agent_chat_ui_boundary() {
    let app_view = read("src/main_sections/app_view_state.rs");
    assert!(
        app_view.contains("crate::ai::agent_chat::ui::AgentChatView"),
        "AgentChatView variant entity must flow through the agent_chat::ui boundary"
    );
}

/// The chat-runtime/UI types that MUST flow through the `agent_chat::ui`
/// facade once an outer consumer references them by an `agent_chat::` path.
const RUNTIME_TYPES: &[&str] = &[
    "AgentChatView",
    "AgentChatThread",
    "AgentChatThreadInit",
    "AgentChatThreadMessage",
    "AgentChatThreadStatus",
    "AgentChatEvent",
    "AgentChatEventRx",
    "AgentChatSession",
    "AgentChatInlineSetupState",
    "AgentChatSetupAction",
    "AgentChatRetryRequest",
    "AgentChatHistoryResumeRequest",
    "AgentChatPermissionBroker",
    "AgentChatLaunchBlocker",
    "AgentChatLaunchRequirements",
    "AgentChatLaunchResolution",
    "AgentChatToolCallState",
];

/// Returns the first runtime type that `content` reaches via an `agent_chat::` path
/// (optionally through a single submodule segment, e.g. `agent_chat::view::AgentChatThread`).
/// The frozen `AppView::AgentChatView` enum variant is NOT matched because it has
/// no `agent_chat::` path prefix.
fn first_agent_chat_runtime_reference(content: &str) -> Option<String> {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    for (idx, _) in content.match_indices("agent_chat::") {
        let rest = &content[idx + "agent_chat::".len()..];
        // Optionally skip one lowercase submodule segment like `view::`.
        let after_submodule = match rest.find("::") {
            Some(pos)
                if pos > 0
                    && rest[..pos]
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_') =>
            {
                &rest[pos + 2..]
            }
            _ => rest,
        };
        for ty in RUNTIME_TYPES {
            if let Some(tail) = after_submodule.strip_prefix(ty) {
                if !tail.chars().next().is_some_and(is_ident) {
                    return Some((*ty).to_string());
                }
            }
        }
    }
    None
}

#[test]
fn outer_consumers_reach_runtime_types_through_agent_chat_ui() {
    // Outer feature roots must import the chat-runtime/UI types via the
    // `agent_chat::ui` facade, not directly from `crate::ai::agent_chat::ui`. Non-runtime
    // Agent Chat compatibility surfaces (catalog/config/portal/history/surface_state/
    // actions-dialog/context builders) are allowed to stay on `crate::ai::agent_chat::ui`.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let outer_roots = [
        "src/app_impl",
        "src/notes",
        "src/render_builtins",
        "src/actions",
        "src/app_actions",
        "src/prompt_handler",
        "src/test_support",
    ];

    let mut offenders = Vec::new();
    for root in outer_roots {
        let mut files = Vec::new();
        collect_rs_files(manifest.join(root), &mut files);
        for file in files {
            let contents = fs::read_to_string(&file).unwrap_or_default();
            if let Some(ty) = first_agent_chat_runtime_reference(&contents) {
                offenders.push(format!("{} reaches agent_chat::{ty}", file.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "outer consumers must import chat-runtime types from \
         crate::ai::agent_chat::ui, not crate::ai::agent_chat::ui; offenders: {offenders:#?}"
    );
}

#[test]
fn agent_chat_ui_facade_stays_runtime_only() {
    // The facade is a runtime/UI boundary; it must NOT absorb non-runtime Agent Chat
    // compatibility surfaces. Those get their own future boundaries if needed.
    let ui = read("src/ai/agent_chat/ui/mod.rs");
    for forbidden in [
        "AgentChatAgentCatalogEntry",
        "AgentChatAgentConfig",
        "AgentChatAgentRuntimeState",
        "AgentChatActionsDialogContext",
        "AgentChatPortalLaunchContract",
        "AgentChatHistoryEntry",
        "portal_contract",
        "catalog::",
        "config::",
        "history::",
    ] {
        assert!(
            !ui.contains(forbidden),
            "agent_chat::ui must stay runtime-only; it must not re-export `{forbidden}`"
        );
    }
}

#[test]
fn frozen_serialized_surface_ids_are_unchanged() {
    let app_view = read("src/main_sections/app_view_state.rs");
    // Serialized view-type ids feed launcher surface contracts and automation;
    // these MUST stay stable even though the feature is now "Agent Chat".
    assert!(
        app_view.contains("Some(\"agent_chat\")"),
        "frozen serialized surface id `agent_chat` must remain"
    );
    assert!(
        app_view.contains("SurfaceKind::AgentChat"),
        "frozen SurfaceKind::AgentChat variant must remain"
    );
    assert!(
        app_view.contains("AppView::AgentChatView"),
        "frozen AppView::AgentChatView variant must remain"
    );
}

#[test]
fn frozen_action_and_route_ids_are_unchanged() {
    let script_context = read("src/actions/builders/script_context.rs");
    for id in [
        "agent_chat:root",
        "agent_chat:change_model",
        "agent_chat_switch_model:",
    ] {
        assert!(
            script_context.contains(id),
            "frozen action/route id `{id}` must remain in script_context.rs"
        );
    }
}

#[test]
fn frozen_get_agent_chat_state_protocol_contract_is_unchanged() {
    let agent_chat_state = read("src/protocol/types/agent_chat_state.rs");
    assert!(
        agent_chat_state.contains("AGENT_CHAT_STATE_SCHEMA_VERSION"),
        "frozen `AGENT_CHAT_STATE_SCHEMA_VERSION` automation contract must remain"
    );
    assert!(
        agent_chat_state.contains("AgentChatStateSnapshot"),
        "frozen `AgentChatStateSnapshot` automation type must remain"
    );
}

#[test]
fn agent_chat_user_facing_copy_uses_feature_name() {
    // Display copy must name the feature "Agent Chat", not "Agent Chat". Each pair
    // asserts the new copy is present and the old Agent Chat-as-feature label is gone.
    let cases: &[(&str, &str, &str)] = &[(
        "src/render_prompts/term.rs",
        "⌘W Agent Chat",
        "⌘W Agent Chat",
    )];
    for (path, expect_new, forbid_old) in cases {
        let src = read(path);
        assert!(
            src.contains(expect_new),
            "{path} must use the Agent Chat feature name (`{expect_new}`)"
        );
        assert!(
            !src.contains(forbid_old),
            "{path} must not keep the Agent Chat-as-feature display label (`{forbid_old}`)"
        );
    }
}

#[test]
fn agent_chat_contract_strings_remain_frozen_until_contract_rename_slice() {
    // Step 6 changes only display copy. Native window title, serialized surface
    // ids, automation snapshot, and detached-target wire ids stay frozen.
    assert!(
        read("src/platform/secondary_window_config.rs").contains("Script Kit Agent Chat"),
        "frozen native window title `Script Kit Agent Chat` must remain"
    );
    assert!(
        read("src/ai/agent_chat/ui/chat_window.rs").contains("agentChatDetached"),
        "frozen detached-target wire id `agentChatDetached` must remain"
    );
    let app_view = read("src/main_sections/app_view_state.rs");
    assert!(
        app_view.contains("Some(\"agent_chat_history\")"),
        "frozen serialized surface id `agent_chat_history` must remain"
    );
}
