use super::*;
use crate::components::{FocusablePrompt, FocusablePromptInterceptedKey};
use crate::ui_foundation::{
    is_key_backspace, is_key_down, is_key_enter, is_key_left, is_key_right, is_key_tab, is_key_up,
    printable_char,
};

fn path_prompt_hints() -> Vec<gpui::SharedString> {
    vec!["↵ Select".into(), "⌘K Actions".into()]
}

impl Focusable for PathPrompt {
    fn focus_handle(&self, _cx: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<PathPromptEvent> for PathPrompt {}

impl Render for PathPrompt {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Use ListItemColors for consistent theming - always use theme
        let list_colors = ListItemColors::from_theme(&self.theme);

        // Clone cached render rows (Arc clone — no per-render allocation)
        let render_rows = self.render_rows.clone();
        let filtered_count = render_rows.len();
        let selected_index = self.selected_index;
        let status_message = self.visible_status_message();

        // Build list items using ListItem component for consistent styling
        let list = uniform_list(
            "path-list",
            filtered_count,
            move |visible_range: std::ops::Range<usize>, _window, _cx| {
                visible_range
                    .map(|ix| {
                        let entry = &render_rows[ix];
                        let is_selected = ix == selected_index;

                        // Choose icon based on entry type
                        let icon = if entry.is_symlink {
                            IconKind::Emoji("↪".to_string())
                        } else if entry.is_dir {
                            IconKind::Emoji("📁".to_string())
                        } else {
                            IconKind::Emoji("📄".to_string())
                        };

                        // Use ListItem component for consistent styling with main menu
                        ListItem::new(entry.name.clone(), list_colors)
                            .index(ix)
                            .icon_kind(icon)
                            .selected(is_selected)
                            .into_any_element()
                    })
                    .collect()
            },
        )
        .track_scroll(&self.list_scroll_handle)
        .flex_1()
        .w_full();

        let text = crate::components::prompt_text_palette(&self.theme);

        // Minimal chrome header: path prefix (muted) + filter text (primary), no buttons
        let path_prefix = self.path_prefix.clone();
        let filter_text = self.filter_text.clone();
        let filter_is_empty = filter_text.is_empty();

        let header = div()
            .id(gpui::ElementId::Name("input:path-filter".into()))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(gpui::px(8.0))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .text_size(gpui::px(16.0))
                    .overflow_x_hidden()
                    .child(
                        div()
                            .text_color(text.help)
                            .flex_shrink_0()
                            .max_w(gpui::px(200.0))
                            .overflow_x_hidden()
                            .child(gpui::SharedString::from(path_prefix)),
                    )
                    .child(
                        div()
                            .text_color(if filter_is_empty {
                                text.placeholder
                            } else {
                                text.primary
                            })
                            .child(if filter_is_empty {
                                gpui::SharedString::from("Type to filter...")
                            } else {
                                gpui::SharedString::from(filter_text)
                            }),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_xs()
                    .text_color(text.help)
                    .child(format!("{filtered_count} items")),
            );

        // Content wrapper
        let content = div()
            .id(gpui::ElementId::Name("list:path-entries".into()))
            .flex()
            .flex_col()
            .flex_1()
            .w_full()
            .px(gpui::px(8.0))
            .when(filtered_count == 0, |d| {
                d.child(
                    div()
                        .id(gpui::ElementId::Name("status:path-load".into()))
                        .flex()
                        .items_center()
                        .justify_center()
                        .h_full()
                        .w_full()
                        .text_sm()
                        .text_color(text.help)
                        .child(gpui::SharedString::from(status_message)),
                )
            })
            .child(list);

        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal_list("prompts::path", true),
        );

        let hints = path_prompt_hints();
        // Two surface-owned hints (`↵ Select | ⌘K Actions`), inside the
        // three-affordance budget: a script-owned file picker has no agent
        // handoff, so the universal `⌘↵ Agent` slot would advertise a verb
        // Enter's context can't honor. Routing this through the universal
        // audit channel logged a false prompt_hint_contract_violation on
        // every path prompt activation (chaos battery 08).
        crate::components::emit_surface_prompt_hint_audit(
            "prompts::path",
            &hints,
            "path picker: Select + Actions only; no agent handoff in a script-owned file picker",
        );

        tracing::info!(
            target: "script_kit::prompt_chrome",
            surface = "prompts::path",
            layout_mode = "mini",
            filtered_count,
            has_actions = true,
            footer_mode = "hint_strip",
            "path_prompt_chrome_checkpoint"
        );

        let footer =
            crate::components::prompt_layout_shell::main_window_footer_slot_for_prompt_surface(
                "path_prompt",
                || crate::components::render_simple_hint_strip(hints, None),
            );

        let container = crate::components::render_minimal_list_prompt_shell_with_footer(
            0.0, None, header, content, footer,
        )
        .id(gpui::ElementId::Name("window:path".into()))
        .text_color(text.primary);

        FocusablePrompt::new(container)
            .key_context("path_prompt")
            .focus_handle(self.focus_handle.clone())
            .build(
                window,
                cx,
                |this, intercepted_key, _event, _window, cx| match intercepted_key {
                    FocusablePromptInterceptedKey::Escape => {
                        let actions_showing = match this.actions_showing.lock() {
                            Ok(guard) => *guard,
                            Err(poison) => {
                                tracing::error!(
                                    "path_prompt_actions_showing_mutex_poisoned_in_app_key_handler"
                                );
                                *poison.into_inner()
                            }
                        };

                        if actions_showing {
                            return false;
                        }

                        logging::log(
                            "PROMPTS",
                            "PathPrompt: Escape key pressed - calling submit_cancel()",
                        );
                        this.submit_cancel();
                        true
                    }
                    FocusablePromptInterceptedKey::CmdK => {
                        this.toggle_actions(cx);
                        true
                    }
                    FocusablePromptInterceptedKey::CmdW => false,
                },
                |this, event, _window, cx| {
                    let key = event.keystroke.key.as_str();

                    // When actions are showing, let the ActionsDialog handle keys in parent routing.
                    let actions_showing = match this.actions_showing.lock() {
                        Ok(guard) => *guard,
                        Err(poison) => {
                            tracing::error!(
                                "path_prompt_actions_showing_mutex_poisoned_in_entity_key_handler"
                            );
                            *poison.into_inner()
                        }
                    };
                    if actions_showing {
                        return;
                    }

                    if is_key_up(key) {
                        this.move_up(cx);
                    } else if is_key_down(key) {
                        this.move_down(cx);
                    } else if is_key_left(key) {
                        this.navigate_to_parent(cx);
                    } else if is_key_right(key) {
                        this.navigate_into_selected(cx);
                    } else if is_key_tab(key) {
                        if event.keystroke.modifiers.shift {
                            this.navigate_to_parent(cx);
                        } else {
                            this.navigate_into_selected(cx);
                        }
                    } else if is_key_enter(key) {
                        this.handle_enter(cx);
                    } else if is_key_backspace(key) {
                        this.handle_backspace(cx);
                    } else if let Some(ch) = printable_char(event.keystroke.key_char.as_deref()) {
                        this.handle_char(ch, cx);
                    }
                },
            )
    }
}
