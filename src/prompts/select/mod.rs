//! SelectPrompt - Multi-select from choices
//!
//! Features:
//! - Select multiple items from a list
//! - Toggle selection with Cmd/Ctrl+Space
//! - Filter choices by typing
//! - Submit selected items

use gpui::{
    div, prelude::*, px, rgb, rgba, uniform_list, AnyElement, Context, FocusHandle, Focusable,
    MouseButton, Render, ScrollStrategy, SharedString, UniformListScrollHandle, Window,
};
use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;

use crate::components::{
    Density, ItemState, LeadingContent, TextContent, TrailingContent, UnifiedListItem,
    UnifiedListItemColors,
};
use crate::designs::{get_tokens, DesignColors, DesignVariant};
use crate::list_item::IconKind;
use crate::logging;
use crate::protocol::Choice;
use crate::scripts;
use crate::theme;

use super::SubmitCallback;

mod prompt;
mod render;
mod search;
#[cfg(test)]
mod tests;
mod types;

pub use prompt::SelectPrompt;
pub(crate) use search::select_submission_is_allowed;
use search::*;
use types::*;
