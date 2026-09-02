//! Scriptlet loading and parsing
//!
//! This module provides functions for loading scriptlets from markdown files
//! in the ~/.scriptkit/plugins/*/scriptlets/ directories.

mod loading;
#[cfg(test)]
mod parsing;

pub use loading::{load_scriptlets, read_scriptlets_from_file, ScriptletCatalogue};

pub(crate) use loading::extract_kit_from_path;
#[cfg(test)]
pub(crate) use parsing::parse_scriptlet_section;

#[cfg(test)]
pub(crate) use loading::build_scriptlet_file_path;
#[cfg(test)]
pub(crate) use parsing::{extract_code_block, extract_html_comment_metadata};

#[cfg(test)]
mod tests;
