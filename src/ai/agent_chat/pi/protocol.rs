use std::collections::HashMap;
use std::path::PathBuf;

use crate::ai::agent_chat::content::ContentBlock;
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PiRpcLaunchSpec {
    pub command: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub cwd: PathBuf,
    /// Which AI surface this connection serves, for the phase trace.
    ///
    /// Three surfaces (Agent Chat, Text, Mini) share this one transport, so a
    /// trace record without a surface label would pool three different
    /// user experiences into one median. The label rides on the launch spec
    /// rather than on `AgentChatTurnRequest` because the runtime is already
    /// created per resolved profile in `agent_chat/launch.rs`, which is the
    /// layer that knows the profile id. Carrying it per-turn instead would
    /// mean threading a field through the UI thread for a value that cannot
    /// change between turns on the same connection.
    pub surface: crate::ai::phase_trace::AiSurface,
}

impl PiRpcLaunchSpec {
    pub(crate) fn new(command: impl Into<PathBuf>, cwd: impl Into<PathBuf>) -> Self {
        Self {
            command: command.into(),
            surface: crate::ai::phase_trace::AiSurface::AgentChat,
            args: Vec::new(),
            env: HashMap::new(),
            cwd: cwd.into(),
        }
    }

    pub(crate) fn with_args(mut self, args: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.args = args.into_iter().map(Into::into).collect();
        self
    }

    pub(crate) fn with_env(
        mut self,
        env: impl IntoIterator<Item = (impl Into<String>, impl Into<String>)>,
    ) -> Self {
        self.env = env
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect();
        self
    }

    /// Label the AI surface this connection serves for the phase trace.
    pub(crate) fn with_surface(mut self, surface: crate::ai::phase_trace::AiSurface) -> Self {
        self.surface = surface;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PiRpcPromptPayload {
    pub message: String,
    pub images: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PiRpcModelSelection {
    pub provider: String,
    pub model_id: String,
}

impl PiRpcModelSelection {
    pub(crate) fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        let separator = raw.find('/').or_else(|| raw.find(':'));
        let Some(separator) = separator else {
            return Err(anyhow!("Pi model selection must include provider"));
        };
        let provider = raw[..separator].trim();
        let model_id = raw[separator + 1..].trim();
        if provider.is_empty() || model_id.is_empty() {
            return Err(anyhow!(
                "Pi model selection must include provider and model"
            ));
        }
        Ok(Self {
            provider: provider.to_string(),
            model_id: model_id.to_string(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PiRpcResponse {
    pub id: Option<String>,
    pub command: Option<String>,
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    pub raw: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PiRpcLine {
    Response(PiRpcResponse),
    Event(Value),
}

pub(crate) fn build_prompt_payload(blocks: &[ContentBlock]) -> Result<PiRpcPromptPayload> {
    let mut text_parts = Vec::new();
    let mut images = Vec::new();

    for block in blocks {
        match block {
            ContentBlock::Text(text) if !text.text.trim().is_empty() => {
                text_parts.push(text.text.clone());
            }
            ContentBlock::Image(image) => {
                images.push(json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "mediaType": image.mime_type,
                        "data": image.data,
                    }
                }));
            }
            _ => {}
        }
    }

    let message = text_parts.join("\n\n");
    if message.trim().is_empty() && images.is_empty() {
        return Err(anyhow!("Pi prompt requires text or supported images"));
    }

    Ok(PiRpcPromptPayload { message, images })
}

pub(crate) fn build_prompt_command(id: impl Into<String>, payload: PiRpcPromptPayload) -> Value {
    let mut command = json!({
        "id": id.into(),
        "type": "prompt",
        "message": payload.message,
    });
    if !payload.images.is_empty() {
        command["images"] = Value::Array(payload.images);
    }
    command
}

pub(crate) fn build_abort_command(id: impl Into<String>) -> Value {
    json!({
        "id": id.into(),
        "type": "abort",
    })
}

pub(crate) fn build_get_available_models_command(id: impl Into<String>) -> Value {
    json!({
        "id": id.into(),
        "type": "get_available_models",
    })
}

pub(crate) fn build_set_model_command(
    id: impl Into<String>,
    selection: &PiRpcModelSelection,
) -> Value {
    json!({
        "id": id.into(),
        "type": "set_model",
        "provider": selection.provider,
        "modelId": selection.model_id,
    })
}

pub(crate) fn build_get_fork_messages_command(id: impl Into<String>) -> Value {
    json!({
        "id": id.into(),
        "type": "get_fork_messages",
    })
}

pub(crate) fn build_fork_command(id: impl Into<String>, entry_id: &str) -> Value {
    json!({
        "id": id.into(),
        "type": "fork",
        "entryId": entry_id,
    })
}

pub(crate) fn encode_json_line(value: &Value) -> String {
    format!("{value}\n")
}

pub(crate) fn parse_rpc_line(line: &str) -> Result<PiRpcLine> {
    let raw: Value = serde_json::from_str(line)?;
    if raw.get("type").and_then(Value::as_str) == Some("response") {
        return Ok(PiRpcLine::Response(PiRpcResponse {
            id: raw.get("id").and_then(Value::as_str).map(str::to_string),
            command: raw
                .get("command")
                .or_else(|| raw.get("commandType"))
                .and_then(Value::as_str)
                .map(str::to_string),
            success: raw.get("success").and_then(Value::as_bool).unwrap_or(false),
            data: raw.get("data").cloned(),
            error: raw.get("error").and_then(Value::as_str).map(str::to_string),
            raw,
        }));
    }
    Ok(PiRpcLine::Event(raw))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::agent_chat::content::{ImageContent, TextContent};

    #[test]
    fn pi_rpc_prompt_command_uses_stdio_protocol_shape() {
        let command = build_prompt_command(
            "turn-1",
            PiRpcPromptPayload {
                message: "hello".to_string(),
                images: Vec::new(),
            },
        );

        assert_eq!(command["id"], "turn-1");
        assert_eq!(command["type"], "prompt");
        assert_eq!(command["message"], "hello");
    }

    #[test]
    fn pi_rpc_prompt_command_includes_images_only_when_present() {
        let text_only = build_prompt_command(
            "turn-1",
            PiRpcPromptPayload {
                message: "hello".to_string(),
                images: Vec::new(),
            },
        );
        assert!(text_only.get("images").is_none());

        let with_image = build_prompt_command(
            "turn-2",
            PiRpcPromptPayload {
                message: String::new(),
                images: vec![json!({"type": "image"})],
            },
        );
        assert_eq!(with_image["images"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn pi_rpc_abort_command_uses_abort_type() {
        let command = build_abort_command("abort-1");
        assert_eq!(command["id"], "abort-1");
        assert_eq!(command["type"], "abort");
    }

    #[test]
    fn pi_rpc_get_available_models_command_uses_rpc_command() {
        let command = build_get_available_models_command("models-1");
        assert_eq!(command["id"], "models-1");
        assert_eq!(command["type"], "get_available_models");
    }

    #[test]
    fn pi_rpc_fork_commands_use_pi_rpc_shapes() {
        let list = build_get_fork_messages_command("fork-msgs-1");
        assert_eq!(list["id"], "fork-msgs-1");
        assert_eq!(list["type"], "get_fork_messages");

        let fork = build_fork_command("fork-1", "entry-42");
        assert_eq!(fork["id"], "fork-1");
        assert_eq!(fork["type"], "fork");
        assert_eq!(fork["entryId"], "entry-42");
    }

    #[test]
    fn pi_rpc_set_model_command_uses_provider_and_model_id() {
        let command = build_set_model_command(
            "model-1",
            &PiRpcModelSelection {
                provider: "openai".to_string(),
                model_id: "gpt-5.4".to_string(),
            },
        );
        assert_eq!(command["type"], "set_model");
        assert_eq!(command["provider"], "openai");
        assert_eq!(command["modelId"], "gpt-5.4");
    }

    #[test]
    fn pi_rpc_model_selection_parses_provider_slash_model() {
        let selection = PiRpcModelSelection::parse("openai/gpt-5.4").unwrap();
        assert_eq!(selection.provider, "openai");
        assert_eq!(selection.model_id, "gpt-5.4");

        let selection = PiRpcModelSelection::parse("anthropic:claude").unwrap();
        assert_eq!(selection.provider, "anthropic");
        assert_eq!(selection.model_id, "claude");
    }

    #[test]
    fn pi_rpc_model_selection_rejects_bare_model_id() {
        assert!(PiRpcModelSelection::parse("gpt-5.4").is_err());
    }

    #[test]
    fn pi_rpc_line_parser_splits_response_from_event() {
        let response =
            parse_rpc_line(r#"{"type":"response","id":"1","command":"prompt","success":true}"#)
                .unwrap();
        assert!(matches!(response, PiRpcLine::Response(_)));

        let event = parse_rpc_line(r#"{"type":"agent_end"}"#).unwrap();
        assert!(matches!(event, PiRpcLine::Event(_)));
    }

    #[test]
    fn pi_rpc_launch_spec_preserves_command_args_env_and_cwd() {
        let spec = PiRpcLaunchSpec::new("/tmp/custom-pi", "/tmp/work")
            .with_args(["--mode", "rpc"])
            .with_env([("A", "B")]);

        assert_eq!(spec.command, PathBuf::from("/tmp/custom-pi"));
        assert_eq!(spec.args, vec!["--mode", "rpc"]);
        assert_eq!(spec.env.get("A").map(String::as_str), Some("B"));
        assert_eq!(spec.cwd, PathBuf::from("/tmp/work"));
    }

    #[test]
    fn pi_rpc_prompt_payload_extracts_text_and_images() {
        let payload = build_prompt_payload(&[
            ContentBlock::Text(TextContent::new("one")),
            ContentBlock::Text(TextContent::new("two")),
            ContentBlock::Image(ImageContent::new("abc", "image/png")),
        ])
        .unwrap();

        assert_eq!(payload.message, "one\n\ntwo");
        assert_eq!(payload.images[0]["source"]["mediaType"], "image/png");
        assert_eq!(payload.images[0]["source"]["data"], "abc");
    }
}
