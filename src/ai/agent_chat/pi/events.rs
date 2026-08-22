use serde_json::Value;

use crate::ai::agent_chat::events::{AgentChatEvent, AgentChatForkPoint};
use crate::ai::agent_chat::ui::config::AgentChatModelEntry;

use super::protocol::{PiRpcLine, PiRpcResponse};

pub(crate) const REVEAL_MAX_UNBROKEN_CHARS: usize = 32;

pub(crate) fn map_rpc_line_to_events(line: PiRpcLine) -> Vec<AgentChatEvent> {
    match line {
        PiRpcLine::Response(response) => map_rpc_response_to_events(&response),
        PiRpcLine::Event(event) => map_rpc_event_to_events(&event),
    }
}

pub(crate) fn map_rpc_response_to_events(response: &PiRpcResponse) -> Vec<AgentChatEvent> {
    if !response.success {
        return vec![AgentChatEvent::failed(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            response
                .error
                .clone()
                .unwrap_or_else(|| "Pi RPC command failed".to_string()),
        )];
    }

    if response.command.as_deref() == Some("get_available_models") {
        return vec![AgentChatEvent::ModelsAvailable {
            current_model_id: None,
            models: models_from_response_data(response.data.as_ref()),
        }];
    }

    if response.command.as_deref() == Some("get_fork_messages") {
        return vec![AgentChatEvent::ForkPointsAvailable {
            entries: fork_points_from_response_data(response.data.as_ref()),
        }];
    }

    if response.command.as_deref() == Some("fork") {
        return vec![AgentChatEvent::ForkCompleted {
            text: response
                .data
                .as_ref()
                .and_then(|data| data.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        }];
    }

    Vec::new()
}

/// Parse `{"messages":[{"entryId","text"}]}` from a `get_fork_messages`
/// response into fork points. Entries without an entryId cannot be forked
/// to and are dropped.
fn fork_points_from_response_data(data: Option<&Value>) -> Vec<AgentChatForkPoint> {
    let Some(messages) = data
        .and_then(|data| data.get("messages"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };

    messages
        .iter()
        .filter_map(|message| {
            let entry_id = message.get("entryId").and_then(Value::as_str)?;
            if entry_id.is_empty() {
                return None;
            }
            let text = message
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default();
            Some(AgentChatForkPoint {
                entry_id: entry_id.to_string(),
                text: text.to_string(),
            })
        })
        .collect()
}

pub(crate) fn map_rpc_event_to_events(event: &Value) -> Vec<AgentChatEvent> {
    match get_str(event, "type") {
        Some("message_update") => map_message_update(event),
        Some("tool_call_end") => vec![AgentChatEvent::ToolCallStarted {
            tool_call_id: tool_call_id(event).unwrap_or_else(|| "tool-call".to_string()),
            title: get_str(event, "toolName")
                .or_else(|| get_str(event, "name"))
                .unwrap_or("Tool")
                .to_string(),
            status: "pending".to_string(),
            tool_name: tool_name_from_event(event),
            raw_input: raw_input_from_event(event),
        }],
        Some("tool_execution_start") => vec![AgentChatEvent::ToolCallStarted {
            tool_call_id: tool_call_id(event).unwrap_or_else(|| "tool-call".to_string()),
            title: get_str(event, "toolName")
                .or_else(|| get_str(event, "name"))
                .unwrap_or("Tool")
                .to_string(),
            status: "running".to_string(),
            tool_name: tool_name_from_event(event),
            raw_input: raw_input_from_event(event),
        }],
        Some("tool_execution_update") => vec![AgentChatEvent::ToolCallUpdated {
            tool_call_id: tool_call_id(event).unwrap_or_else(|| "tool-call".to_string()),
            title: get_str(event, "toolName")
                .or_else(|| get_str(event, "name"))
                .map(str::to_string),
            status: Some("running".to_string()),
            body: body_from_event(event),
            raw_input: raw_input_from_event(event),
            diff: diff_from_event(event),
            is_error: false,
        }],
        Some("tool_execution_end") => {
            let failed = get_str(event, "status") == Some("failed")
                || event.get("error").and_then(Value::as_str).is_some()
                || event.get("isError").and_then(Value::as_bool) == Some(true);
            vec![AgentChatEvent::ToolCallUpdated {
                tool_call_id: tool_call_id(event).unwrap_or_else(|| "tool-call".to_string()),
                title: get_str(event, "toolName")
                    .or_else(|| get_str(event, "name"))
                    .map(str::to_string),
                status: Some(if failed { "failed" } else { "complete" }.to_string()),
                body: body_from_event(event)
                    .or_else(|| get_str(event, "error").map(str::to_string)),
                raw_input: raw_input_from_event(event),
                diff: diff_from_event(event),
                is_error: failed,
            }]
        }
        Some("agent_end") => {
            if let Some(error) = get_str(event, "error").filter(|error| !error.trim().is_empty()) {
                vec![AgentChatEvent::failed(
                    sk_protocol::ai_reliability::ProtocolComponent::Pi,
                    error,
                )]
            } else {
                vec![AgentChatEvent::completed("stop")]
            }
        }
        Some("event_serialize_error") | Some("extension_error") => vec![AgentChatEvent::failed(
            sk_protocol::ai_reliability::ProtocolComponent::Pi,
            get_str(event, "error")
                .or_else(|| get_str(event, "message"))
                .unwrap_or("Pi RPC event error"),
        )],
        _ => Vec::new(),
    }
}

fn map_message_update(event: &Value) -> Vec<AgentChatEvent> {
    let update = event
        .get("assistantMessageEvent")
        .or_else(|| event.get("messageEvent"))
        .unwrap_or(event);
    let delta = get_str(update, "delta")
        .or_else(|| get_str(update, "text"))
        .unwrap_or_default();

    match get_str(update, "type") {
        Some("text_delta") if !delta.is_empty() => split_text_delta_for_reveal(delta)
            .into_iter()
            .map(AgentChatEvent::AgentMessageDelta)
            .collect(),
        Some("thinking_delta") if !delta.is_empty() => split_text_delta_for_reveal(delta)
            .into_iter()
            .map(AgentChatEvent::AgentThoughtDelta)
            .collect(),
        Some("tool_call_delta") if !delta.is_empty() => vec![AgentChatEvent::ToolCallUpdated {
            tool_call_id: tool_call_id(update)
                .or_else(|| tool_call_id(event))
                .unwrap_or_else(|| "tool-call".to_string()),
            title: None,
            status: None,
            body: Some(delta.to_string()),
            raw_input: None,
            diff: None,
            is_error: false,
        }],
        _ => Vec::new(),
    }
}

pub(crate) fn split_text_delta_for_reveal(delta: &str) -> Vec<String> {
    if delta.is_empty() {
        return Vec::new();
    }

    let mut chunks = Vec::new();
    let mut line_start = 0;

    for (idx, ch) in delta.char_indices() {
        if ch == '\n' {
            push_line_reveal_chunks(&delta[line_start..idx + ch.len_utf8()], &mut chunks);
            line_start = idx + ch.len_utf8();
        }
    }

    if line_start < delta.len() {
        push_line_reveal_chunks(&delta[line_start..], &mut chunks);
    }

    if cfg!(debug_assertions) {
        let joined = chunks.concat();
        if joined != delta {
            tracing::warn!(
                target: "script_kit::agent_chat",
                event = "reveal_chunk_mismatch",
                delta_len = delta.len(),
                chunks_len = chunks.len(),
                joined_len = joined.len(),
            );
            chunks.clear();
            if !delta.is_empty() {
                chunks.push(delta.to_string());
            }
        }
        chunks.retain(|c| !c.is_empty());
    }
    chunks
}

fn push_line_reveal_chunks(line: &str, chunks: &mut Vec<String>) {
    if line.is_empty() {
        return;
    }
    if line.chars().all(char::is_whitespace) {
        if let Some(previous) = chunks.last_mut() {
            previous.push_str(line);
        } else {
            chunks.push(line.to_string());
        }
        return;
    }

    if is_markdown_fence_line(line) || is_markdown_table_line(line) {
        chunks.push(line.to_string());
        return;
    }

    let protected_prefix_len = markdown_structural_prefix_len(line).unwrap_or(0);
    push_word_reveal_chunks(line, chunks, protected_prefix_len);
}

fn is_markdown_fence_line(line: &str) -> bool {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    if leading_spaces > 3 {
        return false;
    }
    let trimmed = &line[leading_spaces..];
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn is_markdown_table_line(line: &str) -> bool {
    line.trim_start().starts_with('|')
}

fn markdown_structural_prefix_len(line: &str) -> Option<usize> {
    let leading_len = line
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(line.len());
    let rest = &line[leading_len..];
    if rest.is_empty() {
        return None;
    }

    if let Some(len) = markdown_heading_prefix_len(rest)
        .or_else(|| markdown_blockquote_prefix_len(rest))
        .or_else(|| markdown_task_prefix_len(rest))
        .or_else(|| markdown_list_prefix_len(rest))
    {
        return Some(leading_len + len);
    }

    (leading_len > 0).then_some(leading_len)
}

fn markdown_heading_prefix_len(rest: &str) -> Option<usize> {
    let hash_len = rest.chars().take_while(|ch| *ch == '#').count();
    if (1..=6).contains(&hash_len) && rest.as_bytes().get(hash_len) == Some(&b' ') {
        Some(hash_len + 1)
    } else {
        None
    }
}

fn markdown_blockquote_prefix_len(rest: &str) -> Option<usize> {
    let mut len = 0;
    let bytes = rest.as_bytes();
    while bytes.get(len) == Some(&b'>') {
        len += 1;
    }
    if len == 0 {
        return None;
    }
    if bytes.get(len) == Some(&b' ') {
        len += 1;
    }
    Some(len)
}

fn markdown_task_prefix_len(rest: &str) -> Option<usize> {
    for marker in ["- [ ] ", "- [x] ", "- [X] ", "* [ ] ", "* [x] ", "* [X] "] {
        if rest.starts_with(marker) {
            return Some(marker.len());
        }
    }
    None
}

fn markdown_list_prefix_len(rest: &str) -> Option<usize> {
    for marker in ["- ", "* ", "+ "] {
        if rest.starts_with(marker) {
            return Some(marker.len());
        }
    }

    let mut digit_end = 0;
    for (idx, ch) in rest.char_indices() {
        if ch.is_ascii_digit() {
            digit_end = idx + ch.len_utf8();
        } else {
            break;
        }
    }
    if digit_end > 0
        && matches!(rest.as_bytes().get(digit_end), Some(b'.' | b')'))
        && rest.as_bytes().get(digit_end + 1) == Some(&b' ')
    {
        return Some(digit_end + 2);
    }

    None
}

fn push_word_reveal_chunks(line: &str, chunks: &mut Vec<String>, protected_prefix_len: usize) {
    let mut pos = protected_prefix_len.min(line.len());
    let mut prefix_pending = protected_prefix_len > 0;

    while pos < line.len() {
        if let Some((offset, _)) = line[pos..]
            .char_indices()
            .find(|(_, ch)| !ch.is_whitespace())
        {
            let word_start = pos + offset;
            if word_start > pos && !prefix_pending {
                chunks.push(line[pos..word_start].to_string());
            }

            let word_end = line[word_start..]
                .char_indices()
                .find(|(_, ch)| ch.is_whitespace())
                .map(|(idx, _)| word_start + idx)
                .unwrap_or(line.len());
            let whitespace_end = line[word_end..]
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map(|(idx, _)| word_end + idx)
                .unwrap_or(line.len());
            let prefix = if prefix_pending {
                &line[..protected_prefix_len]
            } else {
                ""
            };
            push_word_with_suffix(
                prefix,
                &line[word_start..word_end],
                &line[word_end..whitespace_end],
                chunks,
            );
            prefix_pending = false;
            pos = whitespace_end;
        } else {
            if pos < line.len() && !prefix_pending {
                chunks.push(line[pos..].to_string());
            }
            break;
        }
    }
}

fn push_word_with_suffix(prefix: &str, word: &str, suffix: &str, chunks: &mut Vec<String>) {
    if word.chars().count() <= REVEAL_MAX_UNBROKEN_CHARS {
        chunks.push(format!("{prefix}{word}{suffix}"));
        return;
    }

    let mut current = String::new();
    current.push_str(prefix);
    let mut current_chars = 0usize;
    for ch in word.chars() {
        current.push(ch);
        current_chars += 1;
        if current_chars >= REVEAL_MAX_UNBROKEN_CHARS {
            chunks.push(std::mem::take(&mut current));
            current_chars = 0;
        }
    }
    current.push_str(suffix);
    if !current.is_empty() {
        chunks.push(current);
    }
}

fn models_from_response_data(data: Option<&Value>) -> Vec<AgentChatModelEntry> {
    let models = data
        .and_then(|data| data.get("models").or(Some(data)))
        .and_then(Value::as_array)
        .into_iter()
        .flatten();

    models
        .filter_map(|model| {
            let raw_id = get_str(model, "id").or_else(|| get_str(model, "modelId"))?;
            let provider = get_str(model, "provider");
            let id = provider
                .filter(|_| !raw_id.contains('/'))
                .map(|provider| format!("{provider}/{raw_id}"))
                .unwrap_or_else(|| raw_id.to_string());
            let context_window = model
                .get("contextWindow")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok());

            Some(AgentChatModelEntry {
                id,
                display_name: get_str(model, "name")
                    .or_else(|| get_str(model, "displayName"))
                    .map(str::to_string),
                context_window,
            })
        })
        .collect()
}

fn get_str<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn get_value<'a>(value: &'a Value, key: &str) -> Option<&'a Value> {
    value.get(key)
}

fn text_from_content_array(value: &Value) -> Option<String> {
    let parts: Vec<String> = value
        .as_array()?
        .iter()
        .filter_map(|item| {
            if item.get("type").and_then(Value::as_str) == Some("text") {
                return get_str(item, "text").map(str::to_string);
            }
            get_str(item, "content")
                .or_else(|| item.as_str())
                .map(str::to_string)
        })
        .filter(|text| !text.trim().is_empty())
        .collect();

    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn tool_call_id(value: &Value) -> Option<String> {
    get_str(value, "toolCallId")
        .or_else(|| get_str(value, "tool_call_id"))
        .or_else(|| get_str(value, "id"))
        .map(str::to_string)
}

/// Raw Pi tool name. `tool_call_end` (model-side) nests it under `toolCall.name`;
/// execution events carry it as a top-level `toolName`.
fn tool_name_from_event(event: &Value) -> Option<String> {
    get_str(event, "toolName")
        .or_else(|| get_str(event, "name"))
        .or_else(|| event.get("toolCall").and_then(|tc| get_str(tc, "name")))
        .map(str::to_string)
}

/// Raw tool input args. Execution events carry top-level `args`; model-side
/// `tool_call_end` nests `toolCall.arguments`.
fn raw_input_from_event(event: &Value) -> Option<Value> {
    event
        .get("args")
        .or_else(|| event.get("toolCall").and_then(|tc| tc.get("arguments")))
        .filter(|value| !value.is_null())
        .cloned()
}

/// Pre-rendered diff emitted by Pi edit/write tools in `result.details.diff`.
fn diff_from_event(event: &Value) -> Option<String> {
    for key in ["result", "partialResult"] {
        if let Some(diff) = event
            .get(key)
            .and_then(crate::ai::agent_chat::ui::tool_card::diff_from_tool_result)
        {
            return Some(diff);
        }
    }
    None
}

fn body_from_event(event: &Value) -> Option<String> {
    for key in ["partialResult", "result"] {
        let Some(value) = get_value(event, key) else {
            continue;
        };
        if let Some(text) = value.get("content").and_then(text_from_content_array) {
            return Some(text);
        }
        if let Some(text) = get_str(value, "text").filter(|text| !text.trim().is_empty()) {
            return Some(text.to_string());
        }
        if !value.is_null() {
            return Some(value.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn pi_rpc_text_delta_maps_to_agent_message_delta() {
        let events = map_rpc_event_to_events(&json!({
            "type": "message_update",
            "assistantMessageEvent": {"type": "text_delta", "delta": "hi"}
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::AgentMessageDelta(delta)] if delta == "hi"
        ));
    }

    #[test]
    fn pi_rpc_text_delta_splits_for_reveal_without_losing_whitespace() {
        let delta = "Hello world\n\nNext step";
        let chunks = split_text_delta_for_reveal(delta);

        assert_eq!(chunks, vec!["Hello ", "world\n\n", "Next ", "step"]);
        assert_eq!(chunks.concat(), delta);
    }

    #[test]
    fn pi_rpc_text_delta_splits_preserve_indentation_trailing_spaces_and_blank_lines() {
        let delta = "  - first item  \n\n    indented next  ";
        let chunks = split_text_delta_for_reveal(delta);

        assert_eq!(chunks.concat(), delta);
        assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
    }

    #[test]
    fn pi_rpc_text_delta_splits_code_fence_lines_atomically() {
        let delta = "```ts\nconst value = 1;\n```\n";
        let chunks = split_text_delta_for_reveal(delta);

        assert_eq!(chunks.concat(), delta);
        assert_eq!(chunks.first().map(String::as_str), Some("```ts\n"));
        assert!(chunks.iter().any(|chunk| chunk == "```\n"));
        assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
    }

    #[test]
    fn pi_rpc_text_delta_attaches_markdown_prefixes_to_first_content() {
        let delta = "# Heading one\n> quoted text\n- [ ] task item\n indented code\n";
        let chunks = split_text_delta_for_reveal(delta);

        assert_eq!(chunks.concat(), delta);
        assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
        for bad in ["# ", "> ", "- [ ] ", " "] {
            assert!(
                !chunks.iter().any(|chunk| chunk == bad),
                "must not emit standalone markdown prefix chunk {bad:?}: {chunks:?}"
            );
        }
        assert!(chunks.iter().any(|chunk| chunk.starts_with("# Heading")));
        assert!(chunks.iter().any(|chunk| chunk.starts_with("> quoted")));
        assert!(chunks.iter().any(|chunk| chunk.starts_with("- [ ] task")));
        assert!(chunks.iter().any(|chunk| chunk.starts_with(" indented")));
    }

    #[test]
    fn pi_rpc_text_delta_splits_markdown_tables_by_line() {
        let delta = "| Name | Value |\n| --- | --- |\n| Foo | Bar |\n";
        let chunks = split_text_delta_for_reveal(delta);

        assert_eq!(
            chunks,
            vec!["| Name | Value |\n", "| --- | --- |\n", "| Foo | Bar |\n"]
        );
        assert_eq!(chunks.concat(), delta);
    }

    #[test]
    fn pi_rpc_text_delta_splits_long_unbroken_words() {
        let long_word = "a".repeat(REVEAL_MAX_UNBROKEN_CHARS * 2 + 7);
        let chunks = split_text_delta_for_reveal(&long_word);

        assert!(chunks.len() > 1);
        assert_eq!(chunks.concat(), long_word);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.chars().count() <= REVEAL_MAX_UNBROKEN_CHARS));
    }

    #[test]
    fn pi_rpc_text_delta_complex_markdown_cases_preserve_exact_bytes() {
        let cases = [
            "\n\nNext paragraph",
            " - first item \n\n indented next ",
            "```ts\nconst message = \"hello world\";\n```\n",
            "> quote with **bold** text\n\n1. ordered item\n",
            "| A | B |\n|---|---|\n| 1 | 2 |\n",
        ];

        for delta in cases {
            let chunks = split_text_delta_for_reveal(delta);
            assert_eq!(chunks.concat(), delta, "failed case: {delta:?}");
            assert!(chunks.iter().all(|chunk| !chunk.is_empty()));
        }
    }

    #[test]
    fn pi_rpc_thinking_delta_maps_to_agent_thought_delta() {
        let events = map_rpc_event_to_events(&json!({
            "type": "message_update",
            "assistantMessageEvent": {"type": "thinking_delta", "delta": "hmm"}
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::AgentThoughtDelta(delta)] if delta == "hmm"
        ));
    }

    #[test]
    fn pi_rpc_tool_execution_start_maps_to_tool_call_started() {
        let events = map_rpc_event_to_events(&json!({
            "type": "tool_execution_start",
            "toolCallId": "tool-1",
            "toolName": "bash",
            "args": {"cmd": "printf hi"}
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ToolCallStarted { tool_call_id, title, status, tool_name: Some(tool_name), raw_input: Some(raw_input) }]
                if tool_call_id == "tool-1"
                    && title == "bash"
                    && status == "running"
                    && tool_name == "bash"
                    && raw_input["cmd"] == "printf hi"
        ));
    }

    #[test]
    fn pi_rpc_tool_execution_end_extracts_details_diff_and_error_flag() {
        let events = map_rpc_event_to_events(&json!({
            "type": "tool_execution_end",
            "toolCallId": "tool-1",
            "toolName": "edit",
            "args": {"path": "src/lib.rs"},
            "result": {
                "content": [{"type": "text", "text": "Successfully replaced text in src/lib.rs."}],
                "details": {"diff": "-1 old\n+1 new", "firstChangedLine": 1}
            }
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ToolCallUpdated { diff: Some(diff), is_error: false, raw_input: Some(raw_input), .. }]
                if diff == "-1 old\n+1 new" && raw_input["path"] == "src/lib.rs"
        ));

        let failed = map_rpc_event_to_events(&json!({
            "type": "tool_execution_end",
            "toolCallId": "tool-1",
            "toolName": "bash",
            "isError": true,
            "result": {"content": [{"type": "text", "text": "boom"}]}
        }));
        assert!(matches!(
            failed.as_slice(),
            [AgentChatEvent::ToolCallUpdated { is_error: true, status: Some(status), .. }]
                if status == "failed"
        ));
    }

    #[test]
    fn pi_rpc_tool_execution_update_maps_text_body() {
        let events = map_rpc_event_to_events(&json!({
            "type": "tool_execution_update",
            "toolCallId": "tool-1",
            "partialResult": {"content": [{"type": "text", "text": "line"}]}
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ToolCallUpdated { body: Some(body), status: Some(status), .. }]
                if body == "line" && status == "running"
        ));
    }

    #[test]
    fn pi_rpc_tool_execution_end_maps_complete_status() {
        let events = map_rpc_event_to_events(&json!({
            "type": "tool_execution_end",
            "toolCallId": "tool-1",
            "result": {"content": [{"type": "text", "text": "done"}]}
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ToolCallUpdated { body: Some(body), status: Some(status), .. }]
                if body == "done" && status == "complete"
        ));
    }

    #[test]
    fn pi_rpc_failed_response_maps_to_failed_event() {
        let response = PiRpcResponse {
            id: Some("1".to_string()),
            command: Some("prompt".to_string()),
            success: false,
            data: None,
            error: Some("nope".to_string()),
            raw: json!({}),
        };

        let events = map_rpc_response_to_events(&response);
        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::TurnFailed { failure }]
                if failure.failure.code == sk_protocol::ai_reliability::AiFailureCode::Unknown
        ));
    }

    #[test]
    fn pi_rpc_agent_end_error_maps_to_failed_event() {
        let events = map_rpc_event_to_events(&json!({
            "type": "agent_end",
            "error": "failed"
        }));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::TurnFailed { failure }]
                if failure.failure.code == sk_protocol::ai_reliability::AiFailureCode::Unknown
        ));
    }

    #[test]
    fn pi_rpc_agent_end_success_maps_to_turn_finished() {
        let events = map_rpc_event_to_events(&json!({"type": "agent_end"}));

        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::TurnCompleted {
                outcome: crate::ai::reliability::AiTurnRuntimeOutcome::Completed {
                    stop_reason: Some(stop_reason)
                }
            }] if stop_reason == "stop"
        ));
    }

    #[test]
    fn pi_rpc_get_fork_messages_response_maps_to_fork_points() {
        let response = PiRpcResponse {
            id: Some("fork-msgs-1".to_string()),
            command: Some("get_fork_messages".to_string()),
            success: true,
            data: Some(json!({
                "messages": [
                    {"entryId": "entry-1", "text": "first prompt"},
                    {"entryId": "", "text": "unforkable"},
                    {"entryId": "entry-2", "text": null},
                ]
            })),
            error: None,
            raw: json!({}),
        };

        let events = map_rpc_response_to_events(&response);
        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ForkPointsAvailable { entries }]
                if entries.len() == 2
                    && entries[0].entry_id == "entry-1"
                    && entries[0].text == "first prompt"
                    && entries[1].entry_id == "entry-2"
                    && entries[1].text.is_empty()
        ));
    }

    #[test]
    fn pi_rpc_fork_response_maps_to_fork_completed_with_selected_text() {
        let response = PiRpcResponse {
            id: Some("fork-1".to_string()),
            command: Some("fork".to_string()),
            success: true,
            data: Some(json!({"text": "original prompt", "cancelled": false})),
            error: None,
            raw: json!({}),
        };

        let events = map_rpc_response_to_events(&response);
        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ForkCompleted { text }] if text == "original prompt"
        ));
    }

    #[test]
    fn pi_rpc_failed_fork_response_maps_to_failed() {
        let response = PiRpcResponse {
            id: Some("fork-1".to_string()),
            command: Some("fork".to_string()),
            success: false,
            data: None,
            error: Some("entry not found".to_string()),
            raw: json!({}),
        };

        let events = map_rpc_response_to_events(&response);
        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::TurnFailed { failure }]
                if failure.failure.code == sk_protocol::ai_reliability::AiFailureCode::Unknown
        ));
    }

    #[test]
    fn pi_rpc_get_available_models_response_maps_to_models_available() {
        let response = PiRpcResponse {
            id: Some("models-1".to_string()),
            command: Some("get_available_models".to_string()),
            success: true,
            data: Some(json!({
                "models": [{"provider": "openai", "id": "gpt-5.4", "name": "GPT 5.4", "contextWindow": 256000}]
            })),
            error: None,
            raw: json!({}),
        };

        let events = map_rpc_response_to_events(&response);
        assert!(matches!(
            events.as_slice(),
            [AgentChatEvent::ModelsAvailable { models, .. }]
                if models.len() == 1
                    && models[0].id == "openai/gpt-5.4"
                    && models[0].display_name.as_deref() == Some("GPT 5.4")
                    && models[0].context_window == Some(256000)
        ));
    }
}
