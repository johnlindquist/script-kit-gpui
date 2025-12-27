#![allow(unexpected_cfgs)]

//! Script Kit GPUI - A GPUI-based launcher for Script Kit
//!
//! This library provides the core functionality for executing scripts
//! with bidirectional JSONL communication.

pub mod components;
pub mod config;
pub mod designs;
pub mod editor;
pub mod error;
pub mod executor;
pub mod list_item;
pub mod logging;
pub mod panel;
pub mod perf;
pub mod protocol;
pub mod prompts;
pub mod scripts;
pub mod selected_text;
pub mod term_prompt;
pub mod syntax;
pub mod terminal;
pub mod theme;
pub mod toast_manager;
#[cfg(not(test))]
pub mod tray;
pub mod utils;
pub mod window_manager;
pub mod window_resize;

// Phase 1 system API modules
pub mod clipboard_history;
pub mod window_control;
pub mod file_search;

// Built-in features registry
pub mod builtins;
pub mod app_launcher;
