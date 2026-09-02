use script_kit_gpui::clipboard_history::{
    add_entry, clear_history, get_clipboard_history_meta, get_entry_content, ContentType,
};
use script_kit_gpui::mcp_protocol::{
    handle_request_with_runtime_context, JsonRpcRequest, JsonRpcResponse, McpRuntimeContext,
};
use serde_json::{json, Value};
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

static CLIPBOARD_TEST_LOCK: Mutex<()> = Mutex::new(());
static CLIPBOARD_TEST_HOME: OnceLock<TempDir> = OnceLock::new();

fn handle_request(request: JsonRpcRequest, context: Option<&McpRuntimeContext>) -> JsonRpcResponse {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("build test runtime");
    runtime.block_on(handle_request_with_runtime_context(
        request,
        &[],
        &[],
        None,
        context,
    ))
}

fn test_home() -> &'static std::path::Path {
    CLIPBOARD_TEST_HOME
        .get_or_init(|| TempDir::new().expect("create temp home"))
        .path()
}

fn with_clipboard_home<F: FnOnce()>(f: F) {
    let _lock = CLIPBOARD_TEST_LOCK
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let previous_home = std::env::var_os("HOME");
    std::env::set_var("HOME", test_home());
    clear_history().expect("clear clipboard history before test");
    f();
    clear_history().expect("clear clipboard history after test");
    if let Some(home) = previous_home {
        std::env::set_var("HOME", home);
    } else {
        std::env::remove_var("HOME");
    }
}

fn call_tool(name: &str, arguments: Value, context: Option<&McpRuntimeContext>) -> Value {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!("tool-call"),
        method: "tools/call".to_string(),
        params: json!({
            "name": name,
            "arguments": arguments,
        }),
    };
    let response = handle_request(request, context);
    response
        .result
        .expect("tools/call should return a JSON-RPC success envelope")
}

fn first_tool_text(result: &Value) -> Value {
    let text = result["content"][0]["text"]
        .as_str()
        .expect("tool result should include text content");
    serde_json::from_str(text).expect("tool text should be structured JSON")
}

fn clipboard_write_context() -> McpRuntimeContext {
    McpRuntimeContext {
        token_scopes: vec!["mcp:read".to_string(), "clipboard:write".to_string()],
        ..McpRuntimeContext::default()
    }
}

#[test]
fn clipboard_tools_are_listed_before_generic_kit_fallback() {
    let request = JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        id: json!("tools-list"),
        method: "tools/list".to_string(),
        params: json!({}),
    };
    let response = handle_request(request, None);
    let tools = response.result.unwrap()["tools"]
        .as_array()
        .unwrap()
        .clone();
    let names: Vec<_> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();

    let clipboard_pin_idx = names
        .iter()
        .position(|name| *name == "kit/clipboard_pin")
        .expect("kit/clipboard_pin should be listed");
    let kit_show_idx = names
        .iter()
        .position(|name| *name == "kit/show")
        .expect("kit/show should be listed");
    assert!(
        clipboard_pin_idx < kit_show_idx,
        "clipboard mutation tools must be listed before generic kit tools"
    );
}

#[test]
fn clipboard_mutations_require_clipboard_write_scope() {
    let context = McpRuntimeContext {
        token_scopes: vec!["mcp:read".to_string()],
        ..McpRuntimeContext::default()
    };
    let result = call_tool(
        "kit/clipboard_pin",
        json!({ "id": "entry-1" }),
        Some(&context),
    );
    let payload = first_tool_text(&result);
    assert_eq!(result["isError"], true);
    assert_eq!(payload["action"], "kit/clipboard_pin");
    assert_eq!(payload["error"]["code"], "scope_denied");
    assert!(
        payload["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("clipboard:write"),
        "scope denial should name the missing clipboard:write scope"
    );
}

#[test]
fn clipboard_destructive_tools_require_confirm_true() {
    let context = clipboard_write_context();

    let delete_result = call_tool(
        "kit/clipboard_delete",
        json!({ "id": "entry-1" }),
        Some(&context),
    );
    let delete_payload = first_tool_text(&delete_result);
    assert_eq!(delete_result["isError"], true);
    assert_eq!(delete_payload["error"]["code"], "confirm_required");

    let clear_result = call_tool("kit/clipboard_clear_unpinned", json!({}), Some(&context));
    let clear_payload = first_tool_text(&clear_result);
    assert_eq!(clear_result["isError"], true);
    assert_eq!(clear_payload["error"]["code"], "confirm_required");
}

#[test]
fn clipboard_pin_unpin_delete_round_trips_against_history_db() {
    with_clipboard_home(|| {
        let context = clipboard_write_context();
        let id = add_entry("mcp clipboard pin target", ContentType::Text).expect("add entry");

        let pin = call_tool("kit/clipboard_pin", json!({ "id": id }), Some(&context));
        let pin_payload = first_tool_text(&pin);
        assert_eq!(pin_payload["ok"], true);
        assert_eq!(pin_payload["result"]["pinned"], true);
        assert!(
            get_clipboard_history_meta(10, 0)
                .expect("read pinned clipboard metadata")
                .iter()
                .any(|entry| entry.id == id && entry.pinned),
            "pin should mark the entry as pinned in history metadata"
        );

        let unpin = call_tool("kit/clipboard_unpin", json!({ "id": id }), Some(&context));
        let unpin_payload = first_tool_text(&unpin);
        assert_eq!(unpin_payload["ok"], true);
        assert_eq!(unpin_payload["result"]["unpinned"], true);
        assert!(
            get_clipboard_history_meta(10, 0)
                .expect("read unpinned clipboard metadata")
                .iter()
                .any(|entry| entry.id == id && !entry.pinned),
            "unpin should clear the pinned flag"
        );

        let delete = call_tool(
            "kit/clipboard_delete",
            json!({ "id": id, "confirm": true }),
            Some(&context),
        );
        let delete_payload = first_tool_text(&delete);
        assert_eq!(delete_payload["ok"], true);
        assert_eq!(delete_payload["result"]["deleted"], true);
        assert!(
            get_entry_content(delete_payload["result"]["id"].as_str().unwrap_or_default())
                .is_none(),
            "delete should remove the entry content"
        );
    });
}

#[test]
fn clipboard_clear_unpinned_preserves_pinned_entries() {
    with_clipboard_home(|| {
        let context = clipboard_write_context();
        let pinned_id = add_entry("mcp pinned clipboard", ContentType::Text).expect("add pinned");
        let unpinned_id =
            add_entry("mcp unpinned clipboard", ContentType::Text).expect("add unpinned");

        let pin = call_tool(
            "kit/clipboard_pin",
            json!({ "id": pinned_id }),
            Some(&context),
        );
        assert_eq!(first_tool_text(&pin)["ok"], true);

        let clear = call_tool(
            "kit/clipboard_clear_unpinned",
            json!({ "confirm": true }),
            Some(&context),
        );
        let clear_payload = first_tool_text(&clear);
        assert_eq!(clear_payload["ok"], true);
        assert_eq!(clear_payload["result"]["clearedUnpinned"], true);
        assert!(
            get_entry_content(clear_payload["result"]["id"].as_str().unwrap_or_default()).is_none(),
            "clear result should not report a target id"
        );
        assert_eq!(
            get_entry_content(&pinned_id).as_deref(),
            Some("mcp pinned clipboard")
        );
        assert!(
            get_entry_content(&unpinned_id).is_none(),
            "clear unpinned should remove unpinned entries"
        );
    });
}

#[test]
fn clipboard_copy_reports_not_found_for_missing_entry_without_touching_system_clipboard() {
    with_clipboard_home(|| {
        let context = clipboard_write_context();
        let copy = call_tool(
            "kit/clipboard_copy",
            json!({ "id": "missing-entry" }),
            Some(&context),
        );
        let payload = first_tool_text(&copy);
        assert_eq!(copy["isError"], true);
        assert_eq!(payload["error"]["code"], "not_found");
    });
}
