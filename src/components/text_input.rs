//! TextInput - Single-line text input with selection and clipboard support
//!
//! A reusable component for text input fields that supports:
//! - Text selection (shift+arrows, cmd+a, mouse drag)
//! - Clipboard operations (cmd+c, cmd+v, cmd+x)
//! - Word navigation (alt+arrows)
//! - Standard cursor movement (arrows, home/end)
//!

#[path = "text_input/core.rs"]
pub(crate) mod core;
#[path = "text_input/render.rs"]
mod render;
#[cfg(test)]
#[path = "text_input/tests.rs"]
mod tests;

pub use core::{TextInputState, TextSelection};
// OF-17 boundary: callers in app_impl reach the sanitizer through this module
// path; removing this re-export breaks the bin target (E0364 lineage).
pub(crate) use core::normalize_single_line_text;
#[allow(unused_imports)]
pub(crate) use render::{
    placeholder_cursor_anchor, pulse_cursor_bar, render_compact_search_text,
    render_text_input_cursor_selection, CompactSearchTextConfig, TextHighlightRange,
    TextInlinePillRange, TextInputRenderConfig, TextInputRenderIndicator,
};
