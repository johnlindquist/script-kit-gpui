// Day Page vertical layout budget — MOVED to the lib at
// `script_kit_gpui::day_page::layout` so the design-contract exporter and
// `cargo test --lib` share the renderer's truth (2026-07-11 Day Page design
// slice). This include!-target stays as a thin compatibility re-export for
// the binary's bare-name callers (day_page_view.rs, app_layout dispatch).
pub(crate) use script_kit_gpui::day_page::layout::{
    day_page_layout_budget, DAY_PAGE_CLIPBOARD_SHELF_GAP_PX,
    DAY_PAGE_CLIPBOARD_SHELF_ROW_HEIGHT_PX, DAY_PAGE_CLIPBOARD_SHELF_TOGGLE_HEIGHT_PX,
    DAY_PAGE_CLIPBOARD_SHELF_TOP_PADDING_PX, DAY_PAGE_MIN_EDITOR_HEIGHT_PX,
};
