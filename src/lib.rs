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
pub mod prompts;
pub mod protocol;
pub mod scripts;
pub mod selected_text;
pub mod syntax;
pub mod term_prompt;
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
pub mod file_search;
pub mod window_control;

// Built-in features registry
pub mod app_launcher;
pub mod builtins;

// Frecency tracking for script usage
pub mod frecency;

// Process management for tracking bun script processes
pub mod process_manager;

// Scriptlet parsing and variable substitution
pub mod scriptlets;

// VSCode snippet syntax parser for template() SDK function
pub mod snippet;

// HTML form parsing for form() prompt
pub mod form_parser;

// Centralized template variable substitution system
// Used by expand_manager, template prompts, and future template features
pub mod template_variables;

// Text injection for text expansion/snippet systems
pub mod text_injector;

// Expand trigger matching for text expansion
pub mod expand_matcher;

// Global keyboard monitoring for system-wide keystroke capture
// Required for text expansion triggers typed in any application
#[cfg(target_os = "macos")]
pub mod keyboard_monitor;

// Expand manager - ties together keyboard monitoring, trigger matching,
// and text injection for the complete text expansion system
#[cfg(target_os = "macos")]
pub mod expand_manager;

// OCR module - macOS Vision framework integration
#[cfg(feature = "ocr")]
pub mod ocr;

// Script scheduling with cron expressions and natural language
pub mod scheduler;

// Storybook - Component preview system for development
// NOTE: Temporarily disabled due to GPUI API changes (overflow_scroll, when_some, etc.)
// pub mod storybook;

// Stories - Component story definitions for the storybook
// pub mod stories;
