//! Alias persistence system for built-in commands and scriptlets.
//!
//! This module provides functionality for storing and retrieving user-defined
//! aliases for commands that don't have their own alias metadata.
//!
//! # Storage Format
//!
//! Aliases are stored in `~/.scriptkit/aliases.json` as a simple JSON object
//! mapping command IDs to their alias strings.
//!
//! # Example
//!
//! ```json
//! {
//!   "builtin/clipboard-history": "ch",
//!   "builtin/app-launcher": "apps",
//!   "app/com.apple.Safari": "safari"
//! }
//! ```

mod persistence;

// Re-export persistence functions
pub use persistence::{
    default_aliases_path, get_cached_alias_overrides, load_alias_overrides, remove_alias_override,
    save_alias_override,
};
