#![allow(unexpected_cfgs)]

//! Script Kit GPUI - A GPUI-based launcher for Script Kit
//!
//! This library provides the core functionality for executing scripts
//! with bidirectional JSONL communication.

pub mod components;
pub mod config;
pub mod debug_grid;
pub mod designs;
pub mod editor;
pub mod error;
pub mod executor;
pub mod form_prompt;
pub mod hotkeys;
pub mod list_item;
pub mod logging;
pub mod navigation;
pub mod panel;
pub mod perf;
pub mod platform;
pub mod prompts;
pub mod protocol;
pub mod scripts;
pub mod selected_text;
pub mod shortcuts;
pub mod syntax;
pub mod term_prompt;
pub mod terminal;
pub mod theme;
pub mod toast_manager;
#[cfg(not(test))]
pub mod tray;
pub mod warning_banner;
pub mod utils;
pub mod window_manager;
pub mod window_resize;

// Phase 1 system API modules
pub mod clipboard_history;
pub mod file_search;
pub mod window_control;

// System actions - macOS AppleScript-based system commands
#[cfg(target_os = "macos")]
pub mod system_actions;

// Script creation - Create new scripts and scriptlets
pub mod script_creation;

// Permissions wizard - Check and request macOS permissions
pub mod permissions_wizard;

// Menu bar reader - macOS Accessibility API for reading app menus
// Provides get_frontmost_menu_bar() with recursive parsing up to 3 levels
#[cfg(target_os = "macos")]
pub mod menu_bar;

// Menu executor - Execute menu actions via Accessibility API
// Navigates AX hierarchy and performs AXPress on menu items
#[cfg(target_os = "macos")]
pub mod menu_executor;

// Menu cache - SQLite-backed menu bar data caching
// Caches application menu hierarchies by bundle_id to avoid expensive rescans
#[cfg(target_os = "macos")]
pub mod menu_cache;

// Built-in features registry
pub mod app_launcher;
pub mod builtins;

// Frecency tracking for script usage
pub mod frecency;

// Process management for tracking bun script processes
pub mod process_manager;

// Scriptlet parsing and variable substitution
pub mod scriptlets;

// Scriptlet cache for tracking per-file state with change detection
// Used by file watchers to diff scriptlet changes and update registrations incrementally
pub mod scriptlet_cache;

// Typed metadata parser for new `metadata = {}` global syntax
pub mod metadata_parser;

// Schema parser for `schema = { input: {}, output: {} }` definitions
pub mod schema_parser;

// Scriptlet codefence metadata parser for ```metadata and ```schema blocks
pub mod scriptlet_metadata;

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

// Kenv environment setup and initialization
// Ensures ~/.sk/kit exists with required directories and starter files
pub mod setup;

// Storybook - Component preview system for development
pub mod storybook;

// Stories - Component story definitions for the storybook
pub mod stories;

// MCP Server - HTTP server for Model Context Protocol integration
// Provides localhost:43210 endpoint with Bearer token auth
pub mod mcp_server;

// MCP Streaming - Server-Sent Events (SSE) and audit logging
// Provides real-time event streaming and tool call audit logs
pub mod mcp_streaming;

// MCP Protocol - JSON-RPC 2.0 protocol handler for MCP
// Handles request parsing, method routing, and response generation
pub mod mcp_protocol;

// MCP Kit Tools - kit/* namespace tools for app control
// Provides kit/show, kit/hide, kit/state tools
pub mod mcp_kit_tools;

// MCP Script Tools - scripts/* namespace auto-generated tools
// Scripts with schema.input become MCP tools automatically
pub mod mcp_script_tools;

// MCP Resources - read-only data resources for MCP clients
// Provides kit://state, scripts://, and scriptlets:// resources
pub mod mcp_resources;

// Stdin commands - external command handling via stdin
// Provides JSON command protocol for testing and automation
pub mod stdin_commands;

// Notes - Raycast Notes feature parity
// Separate floating window for note-taking with gpui-component
pub mod notes;

// AI Chat - Separate floating window for AI conversations
// BYOK (Bring Your Own Key) with SQLite storage at ~/.sk/kit/ai-chats.db
pub mod ai;

// macOS launch-at-login via SMAppService
// Uses SMAppService on macOS 13+ for modern login item management
pub mod login_item;

// UI transitions/animations (self-contained module, no external crate dependency)
// Provides TransitionColor, Opacity, SlideOffset, AppearTransition, HoverState
// and easing functions (ease_out_quad, ease_in_quad, etc.)
// Used for smooth hover effects, toast animations, and other UI transitions
pub mod transitions;

// File watchers for theme, config, scripts, and system appearance
pub mod watcher;

// Shared window visibility state
// Used to track main window visibility across the app
// Notes/AI windows use this to decide whether to hide the app after closing
use std::sync::atomic::{AtomicBool, Ordering};

/// Global state tracking whether the main window is visible
/// - Used by hotkey toggle to show/hide main window
/// - Used by Notes/AI to prevent main window from appearing when they close
pub static MAIN_WINDOW_VISIBLE: AtomicBool = AtomicBool::new(false);

/// Check if the main window is currently visible
pub fn is_main_window_visible() -> bool {
    MAIN_WINDOW_VISIBLE.load(Ordering::SeqCst)
}

/// Set the main window visibility state
pub fn set_main_window_visible(visible: bool) {
    MAIN_WINDOW_VISIBLE.store(visible, Ordering::SeqCst);
}
