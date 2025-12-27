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
#[cfg(not(test))]
pub mod tray;
pub mod utils;
pub mod window_resize;
