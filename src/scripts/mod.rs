//! Scripts module - Script and scriptlet management
//!
//! This module provides functionality for:
//! - Loading scripts from ~/.scriptkit/plugins/*/scripts/
//! - Loading scriptlets from ~/.scriptkit/plugins/*/scriptlets/
//! - Fuzzy search across scripts, scriptlets, built-ins, apps, and windows
//! - Grouping results by frecency and type
//! - Registering scheduled scripts
//!
//! # Module Structure
//!
//! - `types` - Core data types (Script, Scriptlet, SearchResult, etc.)
//! - `metadata` - Metadata extraction from script files
//! - `loader` - Script loading from file system
//! - `scriptlet_loader` - Scriptlet loading and parsing
//! - `search` - Fuzzy search functionality
//! - `grouping` - Result grouping for the main menu
//! - `scheduling` - Script scheduling registration

#![allow(dead_code)]

mod command_contract;
mod grouping;
pub(crate) mod input_detection;
mod loader;
mod metadata;
pub(crate) mod root_search_contract;
mod scheduling;
mod scriptlet_loader;
pub(crate) mod search;
mod types;
mod validation;

pub use self::command_contract::LauncherCommandReceipt;
#[allow(unused_imports)]
pub(crate) use self::grouping::build_capture_mode_results;
#[allow(unused_imports)]
pub(crate) use self::grouping::build_menu_syntax_hint_results;
#[allow(unused_imports)]
pub use self::grouping::get_grouped_results;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_input_history;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_input_history_and_query;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_validation;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_validation_and_query;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_validation_query_and_root_files;
#[allow(unused_imports)]
pub(crate) use self::grouping::get_grouped_results_with_validation_query_and_root_files_with_options;
#[allow(unused_imports)]
pub(crate) use self::grouping::grouped_view::FlowDiscoveryNote;
#[allow(unused_imports)]
pub(crate) use self::grouping::pin_alias_match_first;
#[allow(unused_imports)]
pub(crate) use self::grouping::prepend_root_brain_inbox_section;
#[allow(unused_imports)]
pub(crate) use self::grouping::prepend_root_conversations_section;
#[allow(unused_imports)]
pub use self::loader::{read_scripts, read_scripts_report};
pub use self::scheduling::register_scheduled_scripts;
pub use self::scriptlet_loader::{load_scriptlets, read_scriptlets_from_file};
#[allow(unused_imports)]
pub use self::search::{
    compute_match_indices_for_result, fuzzy_search_builtins, fuzzy_search_flows,
    fuzzy_search_skills, fuzzy_search_unified, fuzzy_search_unified_all,
    fuzzy_search_unified_all_with_skills, fuzzy_search_unified_all_with_skills_and_flows,
    NucleoCtx,
};
#[allow(unused_imports)]
pub use self::types::{
    preview_cache_is_valid, preview_match_signature, AgentChatHistoryMatch, AgentMatch,
    AiVaultMatch, AppMatch, BrainInboxMatch, BrainMatch, BrowserHistoryMatch, BrowserTabMatch,
    BuiltInMatch, ClipboardHistoryMatch, ConversationRowTarget, DictationHistoryMatch,
    FallbackConfig, FallbackMatch, FileMatch, FlowMatch, MatchEvidence, MatchEvidenceField,
    MatchIndices, NoteMatch, RootWindowEntry, Script, ScriptContentMatch, ScriptIssueMatch,
    ScriptMatch, ScriptMatchKind, Scriptlet, ScriptletMatch, SearchResult, SkillMatch, TodoMatch,
    WindowMatch,
};
#[allow(unused_imports)]
pub use self::validation::{
    detect_binding_collisions, format_script_validation_diagnostics,
    format_script_validation_issue_detail, merge_registered_scriptlet_validation_issues,
    merge_scriptlet_validation_issues, scriptlet_capability_registry_generation,
    scriptlet_declared_sdk_capabilities, validate_declared_sdk_capabilities,
    validate_declared_sdk_capabilities_with_host_availability,
    validate_legacy_scriptlet_capabilities, validate_script_catalog,
    validate_scriptlet_capabilities, validate_scriptlet_capabilities_with_host_availability,
    BindingKind, FailedScript, MetadataField, RelatedScript, ScriptCatalogReport,
    ScriptValidationIssue, ScriptValidationKind, ValidationReport, ValidationSeverity,
    VALIDATION_SCHEMA_VERSION,
};

#[cfg(test)]
pub(crate) use self::metadata::{
    extract_full_metadata, extract_script_metadata, parse_metadata_line,
};

#[allow(unused_imports)] // Re-exported for the library target and integration tests.
pub use self::search::fuzzy_search_scripts;

#[cfg(test)]
pub(crate) use self::search::{
    fuzzy_search_scriptlets, fuzzy_search_unified_with_builtins, fuzzy_search_unified_with_windows,
    fuzzy_search_windows,
};

// Re-export external types needed by tests via super::*
#[cfg(test)]
pub(crate) use crate::app_launcher::AppInfo;
#[cfg(test)]
pub(crate) use crate::builtins::BuiltInEntry;
#[cfg(test)]
pub(crate) use crate::frecency::FrecencyStore;
#[cfg(test)]
pub(crate) use crate::list_item::GroupedListItem;
#[cfg(test)]
pub(crate) use std::path::PathBuf;

// Internal re-exports for tests
#[cfg(test)]
pub(crate) use scriptlet_loader::{
    build_scriptlet_file_path, extract_code_block, extract_html_comment_metadata,
    extract_kit_from_path, parse_scriptlet_section,
};
#[cfg(test)]
pub(crate) use search::{
    contains_ignore_ascii_case, extract_filename, extract_scriptlet_display_path,
    find_ignore_ascii_case, fuzzy_match_with_indices, fuzzy_match_with_indices_ascii,
    is_fuzzy_match,
};

#[cfg(test)]
mod tests;
