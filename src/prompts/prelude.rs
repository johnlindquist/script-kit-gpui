//! Curated prompt API surface for explicit imports.
#![allow(unused_imports)]

pub use super::base::{DesignContext, PromptBase, ResolvedColors};
pub use super::chat::{
    ChatClaudeCodeCallback, ChatConfigureCallback, ChatPrompt, ChatPromptDismissBinding,
    ChatPromptDismissCallback, ChatPromptDismissRoute, ChatSubmitCallback,
};
pub use super::commands::{parse_command, CommandOption, SlashCommand, SlashCommandType};
pub use super::div::{ContainerOptions, ContainerPadding, DivPrompt};
pub use super::drop::DropPrompt;
pub use super::env::EnvPrompt;
pub use super::path::{PathInfo, PathPrompt, PathPromptEvent, ShowActionsCallback};
pub use super::select::SelectPrompt;
pub use super::template::TemplatePrompt;
pub use super::SubmitCallback;

#[cfg(target_os = "macos")]
pub use super::webcam::WebcamPrompt;
#[cfg(not(target_os = "macos"))]
pub use super::webcam_stub::WebcamPrompt;
