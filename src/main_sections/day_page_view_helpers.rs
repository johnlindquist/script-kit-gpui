fn should_normalize_day_page_references_after_edit(
    content: &str,
    previous_len: usize,
    cursor: usize,
) -> bool {
    if content.len() <= previous_len {
        return false;
    }
    let growth = content.len().saturating_sub(previous_len);
    if growth > 1 {
        return true;
    }
    let mut cursor = cursor.min(content.len());
    while cursor > 0 && !content.is_char_boundary(cursor) {
        cursor -= 1;
    }
    content[..cursor]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
}

fn automation_scroll_handle_metrics(
    handle: &gpui::ScrollHandle,
    source: &'static str,
) -> serde_json::Value {
    let offset = handle.offset();
    let max_offset = handle.max_offset();
    let viewport = handle.bounds().size;
    let max_scroll_top = max_offset.y.as_f32().max(0.0);
    let max_scroll_left = max_offset.x.as_f32().max(0.0);
    let scroll_top = (-offset.y.as_f32()).clamp(0.0, max_scroll_top);
    let scroll_left = (-offset.x.as_f32()).clamp(0.0, max_scroll_left);

    serde_json::json!({
        "schemaVersion": 1,
        "source": source,
        "available": true,
        "offsetUnit": "logicalPx",
        "scrollTop": scroll_top,
        "scrollLeft": scroll_left,
        "rawOffsetX": offset.x.as_f32(),
        "rawOffsetY": offset.y.as_f32(),
        "scrollHeight": viewport.height.as_f32() + max_scroll_top,
        "scrollWidth": viewport.width.as_f32() + max_scroll_left,
        "clientHeight": viewport.height.as_f32(),
        "clientWidth": viewport.width.as_f32(),
        "maxScrollTop": max_scroll_top,
        "maxScrollLeft": max_scroll_left,
        "canScrollY": max_scroll_top > 0.0,
        "canScrollX": max_scroll_left > 0.0,
    })
}

fn day_page_task_stats(content: &str) -> serde_json::Value {
    let mut total = 0_usize;
    let mut checked = 0_usize;
    let mut unchecked = 0_usize;
    for line in content.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("- [ ] ") {
            total += 1;
            unchecked += 1;
        } else if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [X] ") {
            total += 1;
            checked += 1;
        }
    }

    serde_json::json!({
        "schemaVersion": 1,
        "total": total,
        "checked": checked,
        "unchecked": unchecked,
    })
}
