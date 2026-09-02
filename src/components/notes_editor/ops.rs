use std::ops::Range;

use gpui::{Context, Window};
use itertools::Itertools;
use tracing::info;

use super::NotesEditor;

/// Prove a replacement from the observed buffer and caret, not requested length.
pub(crate) fn observed_replacement_range(
    before: &str,
    after: &str,
    replaced: Range<usize>,
    inserted: &str,
    selection: Range<usize>,
) -> Option<Range<usize>> {
    if replaced.start > replaced.end {
        return None;
    }
    let prefix = before.get(..replaced.start)?;
    let suffix = before.get(replaced.end..)?;
    let observed_len = after
        .len()
        .checked_sub(prefix.len().checked_add(suffix.len())?)?;
    let end = replaced.start.checked_add(observed_len)?;
    (after.starts_with(prefix)
        && after.ends_with(suffix)
        && after.get(replaced.start..end)? == inserted
        && selection == (end..end))
        .then_some(replaced.start..end)
}

impl NotesEditor {
    pub fn toggle_checklist(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);
            let line = &value[line_start..line_end];

            let (new_line, cursor_delta): (String, isize) =
                if let Some(rest) = line.strip_prefix("- [x] ") {
                    (format!("- [ ] {}", rest), 0)
                } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                    (format!("- [x] {}", rest), 0)
                } else if let Some(rest) = line.strip_prefix("- ") {
                    (format!("- [ ] {}", rest), 4)
                } else {
                    (format!("- [ ] {}", line), 6)
                };

            let new_value = format!("{}{}{}", &value[..line_start], new_line, &value[line_end..]);
            let new_cursor = (cursor as isize + cursor_delta).max(0) as usize;
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Toggled checklist on current line");
        cx.notify();
    }

    pub fn toggle_task_marker_at(
        &mut self,
        marker_range: Range<usize>,
        currently_checked: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let mut toggled = false;
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            if marker_range.start > marker_range.end
                || marker_range.end > value.len()
                || !value.is_char_boundary(marker_range.start)
                || !value.is_char_boundary(marker_range.end)
            {
                return;
            }
            let start = marker_range.start;
            let end = marker_range.end;
            let slice = &value[start..end];

            let needle = if currently_checked {
                ["[x]", "[X]"]
            } else {
                ["[ ]", "[ ]"]
            };
            let found = needle
                .iter()
                .find_map(|n| slice.find(n).map(|i| (i, n.len())));
            let Some((offset, len)) = found else {
                return;
            };

            let replacement = if currently_checked { "[ ]" } else { "[x]" };
            let abs = start + offset;
            let selection = state.selection();
            // Selection edits focus InputState, but preview replaces that input in
            // the dispatch tree. Keep the caller's keyboard owner after the edit.
            let focused = window.focused(cx);
            state.set_selection(abs, abs + len, window, cx);
            state.replace(replacement, window, cx);
            state.set_selection(selection.start, selection.end, window, cx);
            if let Some(focused) = focused {
                focused.focus(window, cx);
            } else {
                window.blur();
            }
            toggled = true;
        });

        if toggled {
            info!("Toggled task checkbox from preview");
            cx.notify();
        }
        toggled
    }

    pub fn insert_horizontal_rule(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let needs_newline =
                cursor > 0 && value.as_bytes().get(cursor - 1).is_none_or(|&b| b != b'\n');
            let rule = if needs_newline {
                "\n\n---\n\n"
            } else {
                "\n---\n\n"
            };

            let new_value = format!("{}{}{}", &value[..cursor], rule, &value[cursor..]);
            let new_cursor = cursor + rule.len();
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Inserted horizontal rule");
        cx.notify();
    }

    pub fn cycle_heading(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);
            let line = &value[line_start..line_end];

            let (new_line, cursor_delta): (String, isize) =
                if let Some(rest) = line.strip_prefix("### ") {
                    (rest.to_string(), -4)
                } else if let Some(rest) = line.strip_prefix("## ") {
                    (format!("### {}", rest), 1)
                } else if let Some(rest) = line.strip_prefix("# ") {
                    (format!("## {}", rest), 1)
                } else {
                    (format!("# {}", line), 2)
                };

            let new_value = format!("{}{}{}", &value[..line_start], new_line, &value[line_end..]);
            let new_cursor = (cursor as isize + cursor_delta).max(line_start as isize) as usize;
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Cycled heading level on current line");
        cx.notify();
    }

    pub fn move_line_up(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);

            if line_start == 0 {
                return;
            }

            let prev_line_start = value[..line_start - 1].rfind('\n').map_or(0, |p| p + 1);

            let current_line = &value[line_start..line_end];
            let prev_line = &value[prev_line_start..line_start - 1];

            let new_value = format!(
                "{}{}\n{}{}",
                &value[..prev_line_start],
                current_line,
                prev_line,
                &value[line_end..]
            );

            let offset_in_line = cursor - line_start;
            let new_cursor = prev_line_start + offset_in_line;

            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        cx.notify();
    }

    pub fn move_line_down(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);

            if line_end >= value.len() {
                return;
            }

            let next_line_end = value[line_end + 1..]
                .find('\n')
                .map_or(value.len(), |p| line_end + 1 + p);

            let current_line = &value[line_start..line_end];
            let next_line = &value[line_end + 1..next_line_end];

            let new_value = format!(
                "{}{}\n{}{}",
                &value[..line_start],
                next_line,
                current_line,
                &value[next_line_end..]
            );

            let offset_in_line = cursor - line_start;
            let new_line_start = line_start + next_line.len() + 1;
            let new_cursor = new_line_start + offset_in_line;

            state.set_value(&new_value, window, cx);
            state.set_selection(
                new_cursor.min(new_value.len()),
                new_cursor.min(new_value.len()),
                window,
                cx,
            );
        });
        cx.notify();
    }

    pub fn select_current_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);

            state.set_selection(line_start, line_end, window, cx);
        });
        cx.notify();
    }

    pub fn try_smart_paste(
        &mut self,
        clipboard: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let trimmed = clipboard.trim();

        if !trimmed.starts_with("http://") && !trimmed.starts_with("https://") {
            return false;
        }

        let selection = self.input_state.read(cx).selection();
        if selection.start == selection.end {
            return false;
        }

        let value = self.input_state.read(cx).value().to_string();
        let start = selection.start.min(value.len());
        let end = selection.end.min(value.len());
        let (start, end) = if start > end {
            (end, start)
        } else {
            (start, end)
        };
        let selected_text = &value[start..end];
        let link = format!("[{}]({})", selected_text, trimmed);
        let new_value = format!("{}{}{}", &value[..start], link, &value[end..]);
        let new_cursor = start + link.len();

        self.input_state.update(cx, |state, cx| {
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Smart paste: wrapped selection as markdown link");
        cx.notify();
        true
    }

    pub fn toggle_blockquote(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let sel_start = selection.start.min(value.len());
            let sel_end = selection.end.min(value.len());
            let (sel_start, sel_end) = if sel_start > sel_end {
                (sel_end, sel_start)
            } else {
                (sel_start, sel_end)
            };

            let region_start = value[..sel_start].rfind('\n').map_or(0, |p| p + 1);
            let region_end = value[sel_end..]
                .find('\n')
                .map_or(value.len(), |p| sel_end + p);

            let region = &value[region_start..region_end];
            let lines: Vec<&str> = region.split('\n').collect();

            let all_quoted = lines.iter().all(|l| l.starts_with("> "));

            let new_lines: Vec<String> = if all_quoted {
                lines
                    .iter()
                    .map(|l| l.strip_prefix("> ").unwrap_or(l).to_string())
                    .collect()
            } else {
                lines.iter().map(|l| format!("> {}", l)).collect()
            };

            let new_region = new_lines.join("\n");
            let new_value = format!(
                "{}{}{}",
                &value[..region_start],
                new_region,
                &value[region_end..]
            );

            let new_cursor = region_start + new_region.len();
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Toggled blockquote on selected lines");
        cx.notify();
    }

    pub fn duplicate_line(
        &mut self,
        direction_down: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);
            let line = &value[line_start..line_end];

            let (new_value, new_cursor) = if direction_down {
                let new_value = format!("{}\n{}{}", &value[..line_end], line, &value[line_end..]);
                let offset_in_line = cursor - line_start;
                let new_cursor = line_end + 1 + offset_in_line;
                (new_value, new_cursor)
            } else {
                let new_value =
                    format!("{}{}\n{}", &value[..line_start], line, &value[line_start..]);
                let offset_in_line = cursor - line_start;
                let new_cursor = line_start + offset_in_line;
                (new_value, new_cursor)
            };

            state.set_value(&new_value, window, cx);
            state.set_selection(
                new_cursor.min(new_value.len()),
                new_cursor.min(new_value.len()),
                window,
                cx,
            );
        });
        info!(
            direction = if direction_down { "down" } else { "up" },
            "Duplicated current line"
        );
        cx.notify();
    }

    pub fn delete_current_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);

            let (remove_start, remove_end) = if line_end < value.len() {
                (line_start, line_end + 1)
            } else if line_start > 0 {
                (line_start - 1, line_end)
            } else {
                (0, value.len())
            };

            let new_value = format!("{}{}", &value[..remove_start], &value[remove_end..]);
            let new_cursor = remove_start.min(new_value.len());
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Deleted current line");
        cx.notify();
    }

    pub fn indent_at_cursor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());
            let indent = "  ";
            let new_value = format!("{}{}{}", &value[..cursor], indent, &value[cursor..]);
            let new_cursor = cursor + indent.len();
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        cx.notify();
    }

    pub fn outdent_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line = &value[line_start..];

            let remove_count = if line.starts_with("  ") {
                2
            } else if line.starts_with(' ') {
                1
            } else {
                return;
            };

            let new_value = format!(
                "{}{}",
                &value[..line_start],
                &value[line_start + remove_count..]
            );
            let new_cursor = cursor.saturating_sub(remove_count).max(line_start);
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        cx.notify();
    }

    pub fn toggle_bullet_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);
            let line = &value[line_start..line_end];

            let (new_line, cursor_delta): (String, isize) =
                if let Some(rest) = line.strip_prefix("- [x] ") {
                    (format!("- {}", rest), -4)
                } else if let Some(rest) = line.strip_prefix("- [ ] ") {
                    (format!("- {}", rest), -4)
                } else if let Some(rest) = line.strip_prefix("- ") {
                    (rest.to_string(), -2)
                } else {
                    (format!("- {}", line), 2)
                };

            let new_value = format!("{}{}{}", &value[..line_start], new_line, &value[line_end..]);
            let new_cursor = (cursor as isize + cursor_delta).max(line_start as isize) as usize;
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Toggled bullet list on current line");
        cx.notify();
    }

    pub fn toggle_numbered_list(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_start = value[..cursor].rfind('\n').map_or(0, |p| p + 1);
            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);
            let line = &value[line_start..line_end];

            let numbered_prefix_len = Self::numbered_list_prefix_len(line);

            let (new_line, cursor_delta): (String, isize) = if numbered_prefix_len > 0 {
                let rest = &line[numbered_prefix_len..];
                (rest.to_string(), -(numbered_prefix_len as isize))
            } else {
                let num = Self::detect_next_list_number(&value, line_start);
                let prefix = format!("{}. ", num);
                let prefix_len = prefix.len() as isize;
                (format!("{}{}", prefix, line), prefix_len)
            };

            let new_value = format!("{}{}{}", &value[..line_start], new_line, &value[line_end..]);
            let new_cursor = (cursor as isize + cursor_delta).max(line_start as isize) as usize;
            state.set_value(&new_value, window, cx);
            state.set_selection(new_cursor, new_cursor, window, cx);
        });
        info!("Toggled numbered list on current line");
        cx.notify();
    }

    pub fn numbered_list_prefix_len(line: &str) -> usize {
        let mut chars = line.chars().peekable();
        let mut digit_count = 0;

        while let Some(&ch) = chars.peek() {
            if ch.is_ascii_digit() {
                digit_count += 1;
                chars.next();
            } else {
                break;
            }
        }

        if digit_count == 0 {
            return 0;
        }

        if chars.next() == Some('.') && chars.next() == Some(' ') {
            digit_count + 2
        } else {
            0
        }
    }

    pub fn detect_next_list_number(value: &str, current_line_start: usize) -> usize {
        if current_line_start == 0 {
            return 1;
        }
        let prev_line_end = current_line_start - 1;
        let prev_line_start = value[..prev_line_end].rfind('\n').map_or(0, |p| p + 1);
        let prev_line = &value[prev_line_start..prev_line_end];

        let prefix_len = Self::numbered_list_prefix_len(prev_line);
        if prefix_len > 0 {
            let num_str: String = prev_line
                .chars()
                .take_while(|c| c.is_ascii_digit())
                .collect();
            num_str.parse::<usize>().unwrap_or(1) + 1
        } else {
            1
        }
    }

    pub fn join_lines(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let cursor = selection.start.min(value.len());

            let line_end = value[cursor..]
                .find('\n')
                .map_or(value.len(), |p| cursor + p);

            if line_end >= value.len() {
                return;
            }

            let next_content_start = value[line_end + 1..]
                .find(|c: char| !c.is_whitespace() || c == '\n')
                .map_or(value.len(), |p| line_end + 1 + p);

            let next_char = value.as_bytes().get(next_content_start);
            let (new_value, join_cursor) = if next_char == Some(&b'\n') || next_char.is_none() {
                let end = if next_content_start < value.len() {
                    next_content_start + 1
                } else {
                    next_content_start
                };
                let new_value = format!("{}{}", &value[..line_end], &value[end..]);
                (new_value, line_end)
            } else {
                let new_value = format!("{} {}", &value[..line_end], &value[next_content_start..]);
                (new_value, line_end)
            };

            state.set_value(&new_value, window, cx);
            state.set_selection(join_cursor, join_cursor, window, cx);
        });
        info!("Joined current line with next");
        cx.notify();
    }

    pub fn transform_case(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.input_state.update(cx, |state, cx| {
            let value = state.value().to_string();
            let selection = state.selection();
            let start = selection.start.min(value.len());
            let end = selection.end.min(value.len());
            let (start, end) = if start > end {
                (end, start)
            } else {
                (start, end)
            };

            if start == end {
                return;
            }

            let selected = &value[start..end];

            let transformed = if selected == selected.to_lowercase() {
                selected.to_uppercase()
            } else if selected == selected.to_uppercase() {
                Self::to_title_case(selected)
            } else {
                selected.to_lowercase()
            };

            let new_value = format!("{}{}{}", &value[..start], transformed, &value[end..]);
            let new_end = start + transformed.len();
            state.set_value(&new_value, window, cx);
            state.set_selection(start, new_end, window, cx);
        });
        info!("Transformed case of selected text");
        cx.notify();
    }

    pub fn to_title_case(s: &str) -> String {
        s.split_whitespace()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    None => String::new(),
                    Some(first) => {
                        format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase())
                    }
                }
            })
            .join(" ")
    }
}

#[cfg(test)]
mod observed_replacement_tests {
    use super::observed_replacement_range;

    #[test]
    fn observed_replacement_requires_actual_span_surroundings_and_caret() {
        assert_eq!(
            observed_replacement_range("café old!", "café new!", 6..9, "new", 9..9),
            Some(6..9)
        );
        assert_eq!(
            observed_replacement_range("café old!", "café old!", 6..9, "new", 9..9),
            None
        );
        assert_eq!(
            observed_replacement_range("café old!", "café new?", 6..9, "new", 9..9),
            None
        );
        assert_eq!(
            observed_replacement_range("café old!", "café new!", 6..9, "new", 6..6),
            None
        );
        assert_eq!(
            observed_replacement_range("café old!", "café new!", 4..9, "new", 9..9),
            None
        );
        assert_eq!(
            observed_replacement_range("before", "before", 3..usize::MAX, "", 3..3),
            None
        );
    }
}
