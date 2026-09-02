//! GPUI Prompt UI Components
//!
//! This module provides modular prompt components for Script Kit.
//! Each prompt type is implemented in its own submodule for parallel development.
//!
//! # Module Structure
//! - `base`: PromptBase - Shared base infrastructure (fields, DesignContext, macros)
//! - `chat`: ChatPrompt - Raycast-style chat interface with streaming support
//! - `div`: DivPrompt - HTML content display
//! - `path`: PathPrompt - File/folder picker with navigation, filtering, and action events
//! - `env`: EnvPrompt - Environment variable/secrets prompt with encrypted storage support
//! - `drop`: DropPrompt - Drag-and-drop file input with dropped file metadata submission
//! - `template`: TemplatePrompt - String template editor with placeholder navigation and preview
//! - `select`: SelectPrompt - Filterable multi-select list with keyboard-driven item toggling

#![allow(dead_code)]

pub mod base;
pub mod chat;
pub mod commands;
pub mod context;
mod creation_feedback;
pub mod div;
mod drop;
pub mod env;
pub mod markdown;
pub mod naming;
mod path;
pub mod prelude;
mod select;
mod template;
#[cfg(target_os = "macos")]
pub mod webcam;
#[cfg(not(target_os = "macos"))]
mod webcam_stub;

// Re-export prompt types for use when they're integrated into main.rs
// When integrating:
// 1. Create Entity<PromptType> in main.rs
// 2. Switch from inline rendering to entity-based rendering
// Note: ArgPrompt is implemented inline in render_prompts/arg.rs, not as a standalone component

// Base infrastructure for prompts - will be used as prompts adopt PromptBase
#[allow(unused_imports)]
pub use base::{DesignContext, PromptBase, ResolvedColors};
pub use chat::{
    ChatClaudeCodeCallback, ChatConfigureCallback, ChatPrompt, ChatPromptDismissBinding,
    ChatPromptDismissCallback, ChatPromptDismissRoute, ChatPromptHostMode,
    ChatPromptRecoveryBinding, ChatSubmitCallback, ChatTranscriptAlignment,
};
pub use creation_feedback::{
    CreationFeedbackPanel, CreationFeedbackPayload, CreationFeedbackReceiptStatus,
};
pub use div::{ContainerOptions, ContainerPadding, DivPrompt};
pub use naming::{
    NamingPrompt, NamingPromptConfig, NamingSubmitResult, NamingTarget, TemplateSelection,
};
#[cfg(target_os = "macos")]
pub use webcam::WebcamPrompt;
#[cfg(not(target_os = "macos"))]
pub use webcam_stub::WebcamPrompt;

// These exports are ready for use in main.rs when AppView variants are added
// The #[allow(unused_imports)] is temporary until main.rs integrations are complete
#[allow(unused_imports)]
pub use drop::{DropPrompt, DroppedFile};
#[allow(unused_imports)]
pub use env::EnvPrompt;
#[allow(unused_imports)]
pub use path::PathInfo;
#[allow(unused_imports)]
pub use path::PathPrompt;
#[allow(unused_imports)]
pub use path::PathPromptEvent;
#[allow(unused_imports)]
pub use path::ShowActionsCallback;
#[allow(unused_imports)]
pub(crate) use select::select_submission_is_allowed;
#[allow(unused_imports)]
pub use select::SelectPrompt;
#[allow(unused_imports)]
pub use template::TemplatePrompt;

// Re-export common types used by prompts
use std::sync::Arc;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;

#[cfg(test)]
mod prelude_tests;
