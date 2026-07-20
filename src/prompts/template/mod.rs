//! TemplatePrompt - String template with {{placeholder}} syntax
//!
//! Features:
//! - Parse template strings with {{name}} placeholders
//! - Tab through placeholders to fill them in
//! - Live preview of filled template
//! - Submit returns the filled template string

use gpui::{
    div, prelude::*, px, rgb, Context, FocusHandle, Focusable, Render, SharedString, Window,
};
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;

use crate::designs::{get_tokens, DesignVariant};
use crate::logging;
use crate::template_variables;
use crate::theme;

use super::SubmitCallback;

mod prompt;
mod render;
#[cfg(test)]
mod tests;
mod types;

pub use prompt::TemplatePrompt;
pub use types::TemplateInput;
