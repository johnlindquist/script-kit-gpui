use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, Render,
    Window, WindowBounds, WindowOptions, SharedString, FocusHandle, Focusable,
    WindowHandle,
};

mod scripts;
mod executor;
mod logging;

struct ScriptListApp {
    scripts: Vec<scripts::Script>,
    selected_index: usize,
    filter_text: String,
    last_output: Option<SharedString>,
    focus_handle: FocusHandle,
    show_logs: bool,
}

impl ScriptListApp {
    fn new(cx: &mut Context<Self>) -> Self {
        let scripts = scripts::read_scripts();
        logging::log("APP", &format!("Loaded {} scripts from ~/.kenv/scripts", scripts.len()));
        ScriptListApp {
            scripts,
            selected_index: 0,
            filter_text: String::new(),
            last_output: None,
            focus_handle: cx.focus_handle(),
            show_logs: false,
        }
    }

    fn filtered_scripts(&self) -> Vec<scripts::Script> {
        if self.filter_text.is_empty() {
            self.scripts.clone()
        } else {
            let filter_lower = self.filter_text.to_lowercase();
            self.scripts.iter()
                .filter(|s| s.name.to_lowercase().contains(&filter_lower))
                .cloned()
                .collect()
        }
    }

    fn move_selection_up(&mut self, cx: &mut Context<Self>) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            let filtered = self.filtered_scripts();
            if let Some(script) = filtered.get(self.selected_index) {
                logging::log("NAV", &format!("Selected: {} (index {})", script.name, self.selected_index));
            }
            cx.notify();
        }
    }

    fn move_selection_down(&mut self, cx: &mut Context<Self>) {
        let filtered_len = self.filtered_scripts().len();
        if self.selected_index < filtered_len.saturating_sub(1) {
            self.selected_index += 1;
            let filtered = self.filtered_scripts();
            if let Some(script) = filtered.get(self.selected_index) {
                logging::log("NAV", &format!("Selected: {} (index {})", script.name, self.selected_index));
            }
            cx.notify();
        }
    }

    fn execute_selected(&mut self, cx: &mut Context<Self>) {
        let filtered = self.filtered_scripts();
        if let Some(script) = filtered.get(self.selected_index).cloned() {
            logging::log("EXEC", &format!("Executing script: {}", script.name));
            let start = std::time::Instant::now();
            
            match executor::execute_script(&script.path) {
                Ok(output) => {
                    let elapsed = start.elapsed().as_millis();
                    let first_line = output.lines().next().unwrap_or("");
                    let msg = format!("✓ {}: {}", script.name, first_line);
                    self.last_output = Some(SharedString::from(msg.clone()));
                    logging::log("EXEC", &format!("SUCCESS in {}ms: {} -> {}", elapsed, script.name, first_line));
                }
                Err(err) => {
                    let elapsed = start.elapsed().as_millis();
                    self.last_output = Some(SharedString::from(format!("✗ Error: {}", err)));
                    logging::log("EXEC", &format!("FAILED in {}ms: {} -> {}", elapsed, script.name, err));
                }
            }
            cx.notify();
        }
    }

    fn update_filter(&mut self, new_char: Option<char>, backspace: bool, clear: bool, cx: &mut Context<Self>) {
        if clear {
            self.filter_text.clear();
            self.selected_index = 0;
            logging::log("FILTER", "Cleared filter");
        } else if backspace && !self.filter_text.is_empty() {
            self.filter_text.pop();
            self.selected_index = 0;
            logging::log("FILTER", &format!("Backspace, filter now: '{}'", self.filter_text));
        } else if let Some(ch) = new_char {
            self.filter_text.push(ch);
            self.selected_index = 0;
            let count = self.filtered_scripts().len();
            logging::log("FILTER", &format!("Added '{}', filter: '{}', showing {} scripts", ch, self.filter_text, count));
        }
        cx.notify();
    }
    
    fn toggle_logs(&mut self, cx: &mut Context<Self>) {
        self.show_logs = !self.show_logs;
        logging::log("UI", &format!("Logs panel: {}", if self.show_logs { "shown" } else { "hidden" }));
        cx.notify();
    }
}

impl Focusable for ScriptListApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for ScriptListApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let filtered = self.filtered_scripts();
        let filtered_len = filtered.len();
        let total_len = self.scripts.len();

        // Build the script list items
        let mut list_container = div()
            .flex()
            .flex_col()
            .w_full();

        if filtered_len == 0 {
            list_container = list_container.child(
                div()
                    .w_full()
                    .py(px(32.))
                    .px(px(16.))
                    .text_center()
                    .text_color(rgb(0x999999))
                    .child(if self.filter_text.is_empty() {
                        "No scripts found in ~/.kenv/scripts/".to_string()
                    } else {
                        format!("No scripts match '{}'", self.filter_text)
                    }),
            );
        } else {
            for (idx, script) in filtered.iter().enumerate() {
                let is_selected = idx == self.selected_index;
                let bg_color = if is_selected {
                    rgb(0x007acc)
                } else {
                    rgb(0x1e1e1e)
                };
                let text_color = if is_selected {
                    rgb(0xffffff)
                } else {
                    rgb(0xe0e0e0)
                };

                list_container = list_container.child(
                    div()
                        .w_full()
                        .px(px(16.))
                        .py(px(8.))
                        .bg(bg_color)
                        .text_color(text_color)
                        .border_b_1()
                        .border_color(rgb(0x464647))
                        .child(format!(
                            "{} {}.{}",
                            if is_selected { "▶" } else { " " },
                            script.name,
                            script.extension
                        )),
                );
            }
        }

        // Build log panel if visible
        let log_panel = if self.show_logs {
            let logs = logging::get_last_logs(10);
            let mut log_container = div()
                .flex()
                .flex_col()
                .w_full()
                .bg(rgb(0x0d0d0d))
                .border_t_1()
                .border_color(rgb(0x464647))
                .p(px(8.))
                .max_h(px(150.));
            
            log_container = log_container.child(
                div()
                    .text_color(rgb(0x808080))
                    .text_xs()
                    .pb(px(4.))
                    .child("─── Logs (Cmd+L to toggle) ───")
            );
            
            for log_line in logs.iter().rev() {
                log_container = log_container.child(
                    div()
                        .text_color(rgb(0x00ff00))
                        .text_xs()
                        .child(log_line.clone())
                );
            }
            
            Some(log_container)
        } else {
            None
        };

        let filter_display = if self.filter_text.is_empty() {
            SharedString::from("Type to search...")
        } else {
            SharedString::from(self.filter_text.clone())
        };
        let filter_is_empty = self.filter_text.is_empty();

        let handle_key = cx.listener(move |this: &mut Self, event: &gpui::KeyDownEvent, window: &mut Window, cx: &mut Context<Self>| {
            let key_str = event.keystroke.key.to_lowercase();
            let has_cmd = event.keystroke.modifiers.platform;
            
            logging::log("KEY", &format!("Key: '{}', cmd: {}, key_char: {:?}", key_str, has_cmd, event.keystroke.key_char));
            
            // Handle Cmd+key combinations
            if has_cmd {
                match key_str.as_str() {
                    "l" => {
                        this.toggle_logs(cx);
                        return;
                    }
                    _ => {}
                }
            }
            
            match key_str.as_str() {
                "up" | "arrowup" => this.move_selection_up(cx),
                "down" | "arrowdown" => this.move_selection_down(cx),
                "enter" => this.execute_selected(cx),
                "escape" => {
                    // If filter has text, clear it. Otherwise minimize window.
                    if !this.filter_text.is_empty() {
                        this.update_filter(None, false, true, cx);
                    } else {
                        logging::log("APP", "Escape pressed - minimizing window");
                        window.minimize_window();
                    }
                }
                "backspace" => this.update_filter(None, true, false, cx),
                _ => {
                    if let Some(ref key_char) = event.keystroke.key_char {
                        if let Some(ch) = key_char.chars().next() {
                            if ch.is_alphanumeric() || ch == '-' || ch == '_' || ch == ' ' {
                                this.update_filter(Some(ch), false, false, cx);
                            }
                        }
                    } else if event.keystroke.key.len() == 1 {
                        if let Some(ch) = event.keystroke.key.chars().next() {
                            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                                this.update_filter(Some(ch), false, false, cx);
                            }
                        }
                    }
                }
            }
        });

        let mut main_div = div()
            .flex()
            .flex_col()
            .bg(rgb(0x1e1e1e))
            .w_full()
            .h_full()
            .text_color(rgb(0xffffff))
            .key_context("script_list")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key)
            // Title bar
            .child(
                div()
                    .bg(rgb(0x2d2d30))
                    .w_full()
                    .px(px(16.))
                    .py(px(12.))
                    .border_b_1()
                    .border_color(rgb(0x464647))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .text_lg()
                            .child("Script Kit (GPUI PoC)")
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x808080))
                            .child("Esc to hide • Cmd+L logs")
                    ),
            )
            // Search box
            .child(
                div()
                    .bg(rgb(0x3c3c3c))
                    .w_full()
                    .px(px(16.))
                    .py(px(8.))
                    .border_b_1()
                    .border_color(rgb(0x464647))
                    .flex()
                    .flex_row()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child("🔍")
                            .child(
                                div()
                                    .text_color(if filter_is_empty { rgb(0x808080) } else { rgb(0xffffff) })
                                    .child(filter_display)
                            )
                    )
                    .child(
                        div()
                            .text_color(rgb(0x808080))
                            .child(format!("{} of {} scripts", filtered_len, total_len))
                    ),
            )
            // Scripts list
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .w_full()
                    .child(list_container),
            );
        
        // Add log panel if visible
        if let Some(panel) = log_panel {
            main_div = main_div.child(panel);
        }
        
        // Status bar
        main_div = main_div.child(
            div()
                .bg(rgb(0x2d2d30))
                .w_full()
                .px(px(16.))
                .py(px(8.))
                .border_t_1()
                .border_color(rgb(0x464647))
                .text_color(rgb(0x999999))
                .flex()
                .flex_row()
                .justify_between()
                .child(
                    if let Some(output) = &self.last_output {
                        output.clone()
                    } else {
                        SharedString::from("Type to filter • ↑/↓ navigate • Enter execute")
                    }
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(0x666666))
                        .child("Cmd+L logs")
                ),
        );
        
        main_div
    }
}

fn main() {
    logging::init();
    
    Application::new().run(move |cx: &mut App| {
        logging::log("APP", "GPUI Application starting");
        let bounds = Bounds::centered(None, size(px(500.), px(700.0)), cx);
        
        let window: WindowHandle<ScriptListApp> = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                logging::log("APP", "Window opened, creating ScriptListApp");
                cx.new(|cx| ScriptListApp::new(cx))
            },
        )
        .unwrap();
        
        // Focus the window
        window
            .update(cx, |view: &mut ScriptListApp, window: &mut Window, cx: &mut Context<ScriptListApp>| {
                let focus_handle = view.focus_handle(cx);
                window.focus(&focus_handle, cx);
                logging::log("APP", "Focus set on ScriptListApp");
            })
            .unwrap();
        
        cx.activate(true);
        
        logging::log("APP", "Application ready - Escape to hide, Cmd+L for logs");
    });
}
