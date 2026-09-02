//! Inline dropdown selector component for form fields.

mod component;
mod render;
mod row;
#[cfg(test)]
mod tests;
mod types;

pub(crate) use component::{
    inline_dropdown_clamp_selected_index, inline_dropdown_visible_range,
    inline_dropdown_visible_range_from_start, InlineDropdown,
};
pub(crate) use row::{
    render_compact_synopsis_strip, render_dense_monoline_picker_row,
    render_dense_monoline_picker_row_with_accessory,
    render_dense_monoline_picker_row_with_leading_visual, render_highlighted_meta,
    render_highlighted_text, render_soft_compact_picker_row, COMMAND_OPACITY,
    CONTEXT_SELECTOR_ROW_HEIGHT, CONTEXT_SELECTOR_SYNOPSIS_HEIGHT, GHOST, HINT, MUTED_OP,
    SOFT_COMPACT_PICKER_ROW_HEIGHT,
};
pub(crate) use types::{InlineDropdownColors, InlineDropdownEmptyState, InlineDropdownSynopsis};
