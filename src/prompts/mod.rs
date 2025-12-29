//! GPUI Prompt UI Components
//!
//! This module provides modular prompt components for Script Kit.
//! Each prompt type is implemented in its own submodule for parallel development.
//!
//! # Module Structure
//! - `arg`: ArgPrompt - Selectable list with search/filtering
//! - `div`: DivPrompt - HTML content display
//! - `path`: PathPrompt - File/folder picker (skeleton)
//! - `env`: EnvPrompt - Environment variable/secrets (skeleton)
//! - `drop`: DropPrompt - Drag and drop (skeleton)
//! - `template`: TemplatePrompt - String templates with placeholders (skeleton)
//! - `select`: SelectPrompt - Multi-select with checkboxes (skeleton)

#![allow(dead_code)]

mod arg;
mod div;
mod drop;
mod env;
mod path;
mod select;
mod template;

// Re-export prompt types for use when they're integrated into main.rs
// Currently these are scaffolding - the actual prompt rendering is still in main.rs
// When integrating:
// 1. Create Entity<PromptType> in main.rs
// 2. Switch from inline rendering to entity-based rendering
// pub use arg::ArgPrompt;
// pub use div::DivPrompt;
// pub use drop::DropPrompt;
// pub use env::EnvPrompt;
// pub use path::PathPrompt;
// pub use select::SelectPrompt;
// pub use template::TemplatePrompt;

// Re-export common types used by prompts
use std::sync::Arc;

/// Callback for prompt submission
/// Signature: (id: String, value: Option<String>)
pub type SubmitCallback = Arc<dyn Fn(String, Option<String>) + Send + Sync>;
