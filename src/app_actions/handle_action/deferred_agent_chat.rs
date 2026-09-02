enum DeferredAgentChatAction {
    OpenOnly,
    SetInput {
        text: String,
        submit: bool,
    },
    SetInputWithImage {
        text: String,
        image_base64: String,
        submit: bool,
    },
    AddAttachment {
        path: String,
    },
    ApplyPreset {
        preset_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredAgentChatActionKind {
    OpenOnly,
    SetInput,
    SetInputSubmit,
    SetInputWithImage,
    SetInputWithImageSubmit,
    AddAttachment,
    ApplyPreset,
}

impl DeferredAgentChatActionKind {
    fn name(self) -> &'static str {
        match self {
            Self::OpenOnly => "open_only",
            Self::SetInputSubmit => "set_input_submit",
            Self::SetInput => "set_input",
            Self::SetInputWithImageSubmit => "set_input_with_image_submit",
            Self::SetInputWithImage => "set_input_with_image",
            Self::AddAttachment => "add_attachment",
            Self::ApplyPreset => "apply_preset",
        }
    }

    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::OpenOnly => format!("Failed to open Agent Chat: {error}"),
            Self::AddAttachment => format!("Failed to attach file to Agent Chat: {error}"),
            Self::ApplyPreset => format!("Failed to apply AI preset: {error}"),
            Self::SetInput
            | Self::SetInputSubmit
            | Self::SetInputWithImage
            | Self::SetInputWithImageSubmit => format!("Failed to send to Agent Chat: {error}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DeferredAiImageAttachmentStage {
    DecodeClipboardImage,
    WriteClipboardImage,
}

impl DeferredAiImageAttachmentStage {
    fn failure_message(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::DecodeClipboardImage => format!("Failed to decode image attachment: {error}"),
            Self::WriteClipboardImage => format!("Failed to write image attachment: {error}"),
        }
    }
}

impl DeferredAgentChatAction {
    fn kind(&self) -> DeferredAgentChatActionKind {
        match self {
            Self::OpenOnly => DeferredAgentChatActionKind::OpenOnly,
            Self::SetInput { submit: true, .. } => DeferredAgentChatActionKind::SetInputSubmit,
            Self::SetInput { submit: false, .. } => DeferredAgentChatActionKind::SetInput,
            Self::SetInputWithImage { submit: true, .. } => {
                DeferredAgentChatActionKind::SetInputWithImageSubmit
            }
            Self::SetInputWithImage { submit: false, .. } => {
                DeferredAgentChatActionKind::SetInputWithImage
            }
            Self::AddAttachment { .. } => DeferredAgentChatActionKind::AddAttachment,
            Self::ApplyPreset { .. } => DeferredAgentChatActionKind::ApplyPreset,
        }
    }

    fn apply_to_agent_chat(
        self,
        entity: Entity<crate::ai::agent_chat::ui::AgentChatView>,
        cx: &mut App,
    ) -> Result<&'static str, String> {
        entity.update(cx, move |chat, cx| match self {
            Self::OpenOnly => Ok("open_only"),
            Self::SetInput { text, submit } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }
                chat.set_input(text, cx);
                if submit {
                    let Some(thread) = chat.thread() else {
                        return Err("Agent Chat thread unavailable".to_string());
                    };
                    thread
                        .update(cx, |thread, cx| thread.submit_input(cx))
                        .map_err(|error| error.to_string())?;
                }
                Ok("set_input")
            }
            Self::SetInputWithImage {
                text,
                image_base64,
                submit,
            } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }

                use base64::Engine as _;

                let png_bytes = base64::engine::general_purpose::STANDARD
                    .decode(image_base64)
                    .map_err(|error| {
                        DeferredAiImageAttachmentStage::DecodeClipboardImage.failure_message(error)
                    })?;
                let temp_path = std::env::temp_dir().join(format!(
                    "script-kit-agent_chat-clipboard-{}.png",
                    uuid::Uuid::new_v4()
                ));
                std::fs::write(&temp_path, png_bytes).map_err(|error| {
                    DeferredAiImageAttachmentStage::WriteClipboardImage.failure_message(error)
                })?;
                let path = temp_path.to_string_lossy().into_owned();

                chat.live_thread()
                    .update(cx, |thread, cx| {
                        thread.add_context_part(
                            crate::ai::AiContextPart::FilePath {
                                path,
                                label: "Clipboard Image".to_string(),
                            },
                            cx,
                        );
                        thread.set_input(text, cx);
                        if submit {
                            thread.submit_input(cx)?;
                        }
                        Ok::<(), String>(())
                    })
                    .map_err(|error| error.to_string())?;

                Ok("set_input_with_image")
            }
            Self::AddAttachment { path } => {
                if chat.is_setup_mode() {
                    return Err("Agent Chat is in setup mode".to_string());
                }

                let label = std::path::Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| path.clone());

                chat.live_thread().update(cx, |thread, cx| {
                    thread.add_context_part(crate::ai::AiContextPart::FilePath { path, label }, cx);
                });

                Ok("add_attachment")
            }
            Self::ApplyPreset { preset_id } => {
                chat.apply_preset_by_id(&preset_id, cx)?;
                Ok("apply_preset")
            }
        })
    }
}
