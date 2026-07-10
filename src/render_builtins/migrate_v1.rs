#[derive(Debug, Clone, PartialEq, Eq)]
enum MigrateV1EngineUnavailable {
    InvalidOverride(std::path::PathBuf),
    Missing,
}

impl MigrateV1EngineUnavailable {
    fn board_message(&self) -> String {
        match self {
            Self::InvalidOverride(path) => format!(
                "SK_MIGRATE_CLI does not point to an existing migration CLI file: {}",
                path.display()
            ),
            Self::Missing => "The v1 migration engine is unavailable in this build".to_string(),
        }
    }
}

fn bundled_migrate_v1_engine(executable: &std::path::Path) -> Option<std::path::PathBuf> {
    let macos_dir = executable.parent()?;
    if macos_dir.file_name()? != "MacOS" {
        return None;
    }
    let contents_dir = macos_dir.parent()?;
    if contents_dir.file_name()? != "Contents" {
        return None;
    }
    Some(
        contents_dir
            .join("Resources")
            .join("scripts")
            .join("migrate")
            .join("cli.ts"),
    )
}

fn resolve_migrate_v1_engine(
    override_path: Option<std::path::PathBuf>,
    executable: Option<&std::path::Path>,
    development_engine: Option<std::path::PathBuf>,
) -> Result<std::path::PathBuf, MigrateV1EngineUnavailable> {
    if let Some(path) = override_path {
        return path
            .is_file()
            .then_some(path.clone())
            .ok_or(MigrateV1EngineUnavailable::InvalidOverride(path));
    }

    if let Some(path) = executable.and_then(bundled_migrate_v1_engine) {
        if path.is_file() {
            return Ok(path);
        }
    }

    if let Some(path) = development_engine {
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(MigrateV1EngineUnavailable::Missing)
}

#[cfg(debug_assertions)]
fn development_migrate_v1_engine() -> Option<std::path::PathBuf> {
    Some(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("scripts")
            .join("migrate")
            .join("cli.ts"),
    )
}

#[cfg(not(debug_assertions))]
fn development_migrate_v1_engine() -> Option<std::path::PathBuf> {
    None
}

fn migrate_v1_engine_path() -> Result<std::path::PathBuf, MigrateV1EngineUnavailable> {
    let executable = std::env::current_exe().ok();
    resolve_migrate_v1_engine(
        std::env::var_os("SK_MIGRATE_CLI")
            .filter(|path| !path.is_empty())
            .map(std::path::PathBuf::from),
        executable.as_deref(),
        development_migrate_v1_engine(),
    )
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigrateScanFinding {
    #[serde(default)]
    api: String,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigrateScanEntry {
    #[serde(default)]
    file: String,
    #[serde(default)]
    path: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    findings: Vec<MigrateScanFinding>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(tag = "event")]
enum MigrateProgressEvent {
    #[serde(rename = "start")]
    Start {
        #[serde(default)]
        files: Vec<String>,
    },
    #[serde(rename = "phase")]
    Phase {
        #[serde(default)]
        file: String,
        #[serde(default)]
        phase: String,
    },
    #[serde(rename = "result")]
    Result {
        #[serde(default)]
        result: MigratePortResult,
    },
    #[serde(rename = "done")]
    Done {},
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigratePortResult {
    #[serde(default)]
    file: String,
    #[serde(default)]
    bucket: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    note: Option<MigratePortNote>,
    #[serde(default)]
    failure: Option<String>,
    #[serde(default)]
    attempts: Vec<MigratePortAttempt>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigratePortNote {
    #[serde(default)]
    summary: String,
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigratePortAttempt {
    #[serde(default)]
    attempt: usize,
    #[serde(default)]
    verdicts: Vec<MigratePortVerdict>,
}

#[derive(Debug, Default, serde::Deserialize)]
struct MigratePortVerdict {
    #[serde(default)]
    id: String,
    #[serde(default)]
    outcome: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    detail: String,
}

impl ScriptListApp {
    fn migrate_v1_new_board() -> MigrateBoardState {
        let v1_dir = std::env::var("SK_MIGRATE_V1_DIR")
            .ok()
            .filter(|dir| !dir.trim().is_empty())
            .unwrap_or_else(|| {
                std::env::var("HOME")
                    .map(|home| format!("{home}/.kenv/scripts"))
                    .unwrap_or_else(|_| "~/.kenv/scripts".to_string())
            });
        if !std::path::Path::new(&v1_dir).exists() {
            return MigrateBoardState {
                phase: MigrateBoardPhase::Unavailable(
                    "No Script Kit v1 scripts found at ~/.kenv/scripts".to_string(),
                ),
                v1_dir,
                ..Default::default()
            };
        }

        let engine_path = match migrate_v1_engine_path() {
            Ok(path) => path,
            Err(error) => {
                return MigrateBoardState {
                    phase: MigrateBoardPhase::Unavailable(error.board_message()),
                    v1_dir,
                    ..Default::default()
                };
            }
        };

        MigrateBoardState {
            phase: MigrateBoardPhase::Scanning,
            v1_dir,
            engine_path: Some(engine_path.to_string_lossy().into_owned()),
            ..Default::default()
        }
    }

    pub(crate) fn open_migrate_v1_view(&mut self, cx: &mut Context<Self>) {
        let board = Self::migrate_v1_new_board();
        self.open_builtin_filterable_view(
            AppView::MigrateV1View {
                filter: String::new(),
                selected_index: 0,
                board,
            },
            "Search scripts…",
            true,
            cx,
        );
        self.migrate_v1_start_scan(cx);
    }

    pub(crate) fn migrate_v1_start_scan(&mut self, cx: &mut Context<Self>) {
        let (engine, v1_dir) = match &self.current_view {
            AppView::MigrateV1View { board, .. } => {
                let Some(engine) = board.engine_path.clone() else {
                    return;
                };
                (engine, board.v1_dir.clone())
            }
            _ => return,
        };
        cx.spawn(async move |this, cx| {
            let output = cx
                .background_executor()
                .spawn(async move {
                    std::process::Command::new("bun")
                        .arg(&engine)
                        .arg("scan")
                        .arg(&v1_dir)
                        .arg("--json")
                        .output()
                })
                .await;
            let rows = output
                .ok()
                .and_then(|out| String::from_utf8(out.stdout).ok())
                .and_then(|stdout| serde_json::from_str::<Vec<MigrateScanEntry>>(&stdout).ok())
                .map(Self::migrate_rows_from_scan)
                .unwrap_or_default();

            let _ = this.update(cx, |this, cx| {
                if let AppView::MigrateV1View {
                    selected_index,
                    board,
                    ..
                } = &mut this.current_view
                {
                    board.rows = rows;
                    board.phase = MigrateBoardPhase::Report;
                    *selected_index = 0;
                    this.list_scroll_handle
                        .scroll_to_item(0, ScrollStrategy::Nearest);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn migrate_rows_from_scan(mut scan: Vec<MigrateScanEntry>) -> Vec<MigrateScriptRow> {
        fn bucket_rank(bucket: &str) -> usize {
            match bucket {
                "ready" => 0,
                "needs-changes" => 1,
                "needs-rewrite" => 2,
                _ => 3,
            }
        }
        scan.sort_by(|a, b| {
            bucket_rank(&a.bucket)
                .cmp(&bucket_rank(&b.bucket))
                .then_with(|| a.file.to_lowercase().cmp(&b.file.to_lowercase()))
        });
        scan.into_iter()
            .map(|entry| MigrateScriptRow {
                file: entry.file,
                path: entry.path,
                bucket: entry.bucket,
                incompatible_apis: entry
                    .findings
                    .into_iter()
                    .filter(|finding| finding.status != "supported")
                    .map(|finding| finding.api)
                    .filter(|api| !api.is_empty())
                    .collect(),
                phase: "queued".to_string(),
                ..Default::default()
            })
            .collect()
    }

    fn migrate_visible_rows(rows: &[MigrateScriptRow], filter: &str) -> Vec<usize> {
        let needle = filter.trim().to_lowercase();
        rows.iter()
            .enumerate()
            .filter_map(|(ix, row)| {
                (needle.is_empty() || row.file.to_lowercase().contains(&needle)).then_some(ix)
            })
            .collect()
    }

    pub(crate) fn dispatch_migrate_v1_primary_footer_action(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(self.current_view, AppView::MigrateV1View { .. }) {
            return false;
        }
        self.migrate_v1_primary_action(cx);
        true
    }

    fn migrate_v1_primary_action(&mut self, cx: &mut Context<Self>) {
        let action = match &self.current_view {
            AppView::MigrateV1View {
                filter,
                selected_index,
                board,
            } => match board.phase {
                MigrateBoardPhase::Report => Some(("port".to_string(), None)),
                MigrateBoardPhase::Done => {
                    let visible = Self::migrate_visible_rows(&board.rows, filter);
                    visible.get(*selected_index).and_then(|row_ix| {
                        let row = board.rows.get(*row_ix)?;
                        (row.status.as_deref() == Some("needs-review"))
                            .then(|| ("handoff".to_string(), Some(row.clone())))
                    })
                }
                _ => None,
            },
            _ => None,
        };

        match action {
            Some((kind, _)) if kind == "port" => self.migrate_v1_start_port(cx),
            Some((_, Some(row))) => self.migrate_v1_handoff_to_ai(row, cx),
            _ => {}
        }
    }

    fn migrate_v1_start_port(&mut self, cx: &mut Context<Self>) {
        let (engine, v1_dir, files) = match &mut self.current_view {
            AppView::MigrateV1View { board, .. } => {
                if board.port_started {
                    return;
                }
                let Some(engine) = board.engine_path.clone() else {
                    return;
                };
                board.port_started = true;
                board.phase = MigrateBoardPhase::Porting;
                for row in &mut board.rows {
                    row.phase = "queued".to_string();
                }
                cx.notify();
                (
                    engine,
                    board.v1_dir.clone(),
                    board.rows.iter().map(|row| row.file.clone()).collect::<Vec<_>>(),
                )
            }
            _ => return,
        };

        let (tx, rx) = async_channel::unbounded::<MigrateProgressEvent>();
        std::thread::spawn(move || {
            let child = std::process::Command::new("bun")
                .arg(engine)
                .arg("port")
                .arg(v1_dir)
                .arg("--progress-jsonl")
                .stdout(std::process::Stdio::piped())
                .spawn();
            let Ok(mut child) = child else {
                return;
            };
            if let Some(stdout) = child.stdout.take() {
                let reader = std::io::BufReader::new(stdout);
                for line in std::io::BufRead::lines(reader).map_while(Result::ok) {
                    if let Ok(event) = serde_json::from_str::<MigrateProgressEvent>(&line) {
                        let _ = tx.send_blocking(event);
                    }
                }
            }
            let _ = child.wait();
        });

        cx.spawn(async move |this, cx| {
            while let Ok(event) = rx.recv().await {
                let _ = this.update(cx, |this, cx| {
                    if let AppView::MigrateV1View { board, .. } = &mut this.current_view {
                        Self::migrate_v1_apply_event(board, event, &files);
                        cx.notify();
                    }
                });
            }
        })
        .detach();
    }

    fn migrate_v1_apply_event(
        board: &mut MigrateBoardState,
        event: MigrateProgressEvent,
        files: &[String],
    ) {
        match event {
            MigrateProgressEvent::Start { files: event_files } => {
                let files = if event_files.is_empty() { files } else { &event_files };
                for file in files {
                    if let Some(row) = board.rows.iter_mut().find(|row| row.file == *file) {
                        row.phase = "queued".to_string();
                    }
                }
            }
            MigrateProgressEvent::Phase { file, phase } => {
                if let Some(row) = board.rows.iter_mut().find(|row| row.file == file) {
                    row.phase = phase;
                }
            }
            MigrateProgressEvent::Result { result } => {
                if let Some(row) = board.rows.iter_mut().find(|row| row.file == result.file) {
                    row.bucket = result.bucket;
                    row.status = Some(result.status.clone());
                    row.phase = result.status;
                    row.note_summary = result.note.map(|note| note.summary);
                    row.failure = result.failure;
                    row.attempts = result
                        .attempts
                        .into_iter()
                        .map(|attempt| MigrateAttemptReceipt {
                            attempt: attempt.attempt,
                            verdicts: attempt
                                .verdicts
                                .into_iter()
                                .map(|verdict| MigrateVerdictReceipt {
                                    id: verdict.id,
                                    outcome: verdict.outcome,
                                    summary: verdict.summary,
                                    detail: verdict.detail,
                                })
                                .collect(),
                        })
                        .collect();
                }
            }
            MigrateProgressEvent::Done {} => {
                board.phase = MigrateBoardPhase::Done;
            }
        }
    }

    fn migrate_v1_handoff_to_ai(&mut self, row: MigrateScriptRow, cx: &mut Context<Self>) {
        let source = std::fs::read_to_string(&row.path).unwrap_or_default();
        let attempts = row.attempts.len();
        let apis = if row.incompatible_apis.is_empty() {
            "known incompatible APIs".to_string()
        } else {
            row.incompatible_apis.join(", ")
        };
        let mut receipts = String::new();
        if let Some(last) = row.attempts.last() {
            for verdict in &last.verdicts {
                receipts.push_str(&format!(
                    "- attempt {} {} {}: {}\n{}\n",
                    last.attempt, verdict.id, verdict.outcome, verdict.summary, verdict.detail
                ));
            }
        }
        let prompt = format!(
            "Port this Script Kit v1 script to v2. The automated migration pipeline failed after {attempts} attempts; validator receipts below. The v2 SDK has no {apis}. Produce a corrected v2 script.\n\nScript: {}\n\nFailure:\n{}\n\nReceipts:\n{}\n\nv1 source:\n```ts\n{}\n```",
            row.file,
            row.failure.unwrap_or_default(),
            receipts,
            source
        );
        self.open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part(Some(prompt), cx);
    }

    fn render_migrate_v1(
        &mut self,
        filter: &str,
        selected_index: usize,
        board: MigrateBoardState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal("migrate_v1", 2, false, false),
        );
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();
        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_name = rgba((chrome.text_primary_hex << 8) | 0xff);
        let text_muted = rgba(chrome.text_muted_rgba);
        let text_hint = rgba(chrome.text_hint_rgba);
        let visible_rows = Self::migrate_visible_rows(&board.rows, filter);
        let total_results = visible_rows.len();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);
                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }
                let mut primary = false;
                if let AppView::MigrateV1View {
                    filter,
                    selected_index,
                    board,
                } = &mut this.current_view
                {
                    let total = Self::migrate_visible_rows(&board.rows, filter).len();
                    let mut handled = true;
                    match key {
                        _ if is_key_escape(key) => {
                            if matches!(board.phase, MigrateBoardPhase::Porting) {
                                crate::platform::defer_hide_main_window(cx);
                            } else if filter.is_empty() {
                                this.go_back_or_close(window, cx);
                            } else {
                                filter.clear();
                                this.filter_text.clear();
                                this.pending_filter_sync = true;
                                *selected_index = 0;
                            }
                        }
                        _ if is_key_up(key) => {
                            if *selected_index > 0 {
                                *selected_index -= 1;
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            }
                        }
                        _ if is_key_down(key) => {
                            if *selected_index < total.saturating_sub(1) {
                                *selected_index += 1;
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                            }
                        }
                        _ if is_key_enter(key) => primary = true,
                        _ => handled = false,
                    }
                    if handled {
                        cx.notify();
                        cx.stop_propagation();
                    }
                }
                if primary {
                    this.migrate_v1_primary_action(cx);
                }
            },
        );

        let list: AnyElement = if let MigrateBoardPhase::Unavailable(message) = &board.phase {
            crate::components::info_state::render_info_state(
                crate::components::info_state::InfoStateSpec::new("migrate-v1-unavailable")
                    .title("Migrate v1 Scripts")
                    .body(message.clone()),
                &self.theme,
                cx,
            )
            .into_any_element()
        } else if board.rows.is_empty() {
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .text_color(text_muted)
                .child(match board.phase {
                    MigrateBoardPhase::Scanning => "Scanning v1 scripts…",
                    _ => "No matching scripts",
                })
                .child(div().text_xs().text_color(text_hint).child("~/.kenv/scripts"))
                .into_any_element()
        } else {
            let rows = board.rows.clone();
            let indices = visible_rows.clone();
            let phase_for_list = board.phase.clone();
            let click_entity = cx.entity().downgrade();
            let hover_entity = cx.entity().downgrade();
            let list_colors = ListItemColors::from_theme(&self.theme);
            let main_menu_theme = self.current_main_menu_theme;
            let hovered = self.hovered_index;
            uniform_list("migrate-v1-list", indices.len(), move |visible, _window, _cx| {
                visible
                    .map(|visible_ix| {
                        let Some(row_ix) = indices.get(visible_ix).copied() else {
                            return div().id(visible_ix).h(px(LIST_ITEM_HEIGHT)).into_any_element();
                        };
                        let Some(row) = rows.get(row_ix) else {
                            return div().id(visible_ix).h(px(LIST_ITEM_HEIGHT)).into_any_element();
                        };
                        let row_entity = click_entity.clone();
                        let hover_entity = hover_entity.clone();
                        let is_selected = visible_ix == selected_index;
                        let is_hovered = hovered == Some(visible_ix);
                        let click_handler = move |_event: &gpui::ClickEvent,
                                                  _window: &mut Window,
                                                  cx: &mut gpui::App| {
                            if let Some(entity) = row_entity.upgrade() {
                                entity.update(cx, |this, cx| {
                                    if let AppView::MigrateV1View { selected_index, .. } =
                                        &mut this.current_view
                                    {
                                        *selected_index = visible_ix;
                                    }
                                    cx.notify();
                                });
                            }
                            cx.stop_propagation();
                        };
                        let hover_handler =
                            move |is_hovered: &bool, _window: &mut Window, cx: &mut gpui::App| {
                                if let Some(entity) = hover_entity.upgrade() {
                                    entity.update(cx, |this, cx| {
                                        if *is_hovered {
                                            this.input_mode = InputMode::Mouse;
                                            this.hovered_index = Some(visible_ix);
                                        } else if this.hovered_index == Some(visible_ix) {
                                            this.hovered_index = None;
                                        }
                                        cx.notify();
                                    });
                                }
                            };
                        let glyph = match &phase_for_list {
                            MigrateBoardPhase::Report => match row.bucket.as_str() {
                                "ready" => "✓",
                                "needs-changes" => "~",
                                "needs-rewrite" => "✗",
                                _ => "?",
                            },
                            MigrateBoardPhase::Done => match row.status.as_deref() {
                                Some("verified") => "✓",
                                Some("verified-with-warnings") => "⚠",
                                Some("needs-review") => "!",
                                Some("error") => "✗",
                                _ => "·",
                            },
                            _ => "·",
                        };
                        let title = format!("{glyph} {}", row.file);
                        let description = match &phase_for_list {
                            MigrateBoardPhase::Report => row.incompatible_apis.join(", "),
                            MigrateBoardPhase::Porting => row.phase.clone(),
                            MigrateBoardPhase::Done => row
                                .note_summary
                                .clone()
                                .or_else(|| row.failure.clone())
                                .unwrap_or_else(|| row.phase.clone()),
                            _ => row.phase.clone(),
                        };
                        div()
                            .id(visible_ix)
                            .cursor_pointer()
                            .on_click(click_handler)
                            .on_hover(hover_handler)
                            .child(
                                ListItem::new(title, list_colors)
                                    .description_opt((!description.is_empty()).then_some(description))
                                    .source_hint_opt(Some(row.bucket.clone()))
                                    .selected(is_selected)
                                    .hovered(is_hovered)
                                    .main_menu_theme(main_menu_theme)
                                    .semantic_id(format!("migrate-v1-row-{visible_ix}"))
                                    .with_accent_bar(true),
                            )
                            .into_any_element()
                    })
                    .collect()
            })
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };

        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.list_scroll_handle, total_results, 8);
        let footer_hints: Vec<gpui::SharedString> = match board.phase {
            MigrateBoardPhase::Report => vec!["↵ Port all".into(), "Esc Back".into()],
            MigrateBoardPhase::Porting => vec!["Porting…".into(), "Esc Hide (keeps running)".into()],
            MigrateBoardPhase::Done => {
                vec!["↵ Port with AI (needs-review row)".into(), "Esc Back".into()]
            }
            _ => vec!["Esc Back".into()],
        };
        crate::components::emit_surface_prompt_hint_audit(
            "migrate_v1",
            &footer_hints,
            "migrate_v1_footer",
        );
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            footer_hints,
            None,
        ));

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .py(px(design_spacing.padding_xs))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .child(list)
                    .child(list_scrollbar),
            );

        let menu_def = self.current_main_menu_theme.def();
        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .font_family(design_typography.font_family)
                .text_color(text_name)
                .key_context("migrate_v1")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(format!(
                        "{} scripts",
                        total_results
                    )),
                ], cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: menu_def.shell.divider_margin_x,
                    height: menu_def.shell.divider_height,
                    visible: menu_def.shell.divider_height > 0.0,
                },
                main: content.into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
    }
}

#[cfg(test)]
mod migrate_v1_engine_tests {
    use super::{
        bundled_migrate_v1_engine, resolve_migrate_v1_engine, MigrateV1EngineUnavailable,
    };

    static NEXT_FIXTURE: std::sync::atomic::AtomicUsize =
        std::sync::atomic::AtomicUsize::new(0);

    fn temp_root(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "script-kit-migrate-engine-{name}-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    fn touch(path: &std::path::Path) {
        std::fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture");
        std::fs::write(path, "// fixture").expect("write fixture");
    }

    #[test]
    fn derives_engine_from_a_macos_app_executable_only() {
        let app_executable = std::path::Path::new(
            "/Applications/Script Kit.app/Contents/MacOS/script-kit-gpui",
        );
        assert_eq!(
            bundled_migrate_v1_engine(app_executable),
            Some(std::path::PathBuf::from(
                "/Applications/Script Kit.app/Contents/Resources/scripts/migrate/cli.ts"
            ))
        );
        assert_eq!(
            bundled_migrate_v1_engine(std::path::Path::new("/tmp/script-kit-gpui")),
            None
        );
    }

    #[test]
    fn valid_override_wins_over_bundle_and_development_engine() {
        let root = temp_root("precedence");
        let override_engine = root.join("override/cli.ts");
        let executable = root.join("Script Kit.app/Contents/MacOS/script-kit-gpui");
        let bundle_engine = root.join("Script Kit.app/Contents/Resources/scripts/migrate/cli.ts");
        let development_engine = root.join("repo/scripts/migrate/cli.ts");
        touch(&override_engine);
        touch(&bundle_engine);
        touch(&development_engine);

        assert_eq!(
            resolve_migrate_v1_engine(
                Some(override_engine.clone()),
                Some(&executable),
                Some(development_engine),
            ),
            Ok(override_engine)
        );

        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn invalid_explicit_override_fails_closed() {
        let root = temp_root("invalid-override");
        let missing_override = root.join("missing/cli.ts");
        let executable = root.join("Script Kit.app/Contents/MacOS/script-kit-gpui");
        let bundle_engine = root.join("Script Kit.app/Contents/Resources/scripts/migrate/cli.ts");
        let development_engine = root.join("repo/scripts/migrate/cli.ts");
        touch(&bundle_engine);
        touch(&development_engine);

        assert_eq!(
            resolve_migrate_v1_engine(
                Some(missing_override.clone()),
                Some(&executable),
                Some(development_engine),
            ),
            Err(MigrateV1EngineUnavailable::InvalidOverride(
                missing_override.clone()
            ))
        );
        assert!(
            MigrateV1EngineUnavailable::InvalidOverride(missing_override)
                .board_message()
                .contains("SK_MIGRATE_CLI")
        );

        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn bundle_precedes_development_and_release_fails_without_bundle() {
        let root = temp_root("fallbacks");
        let executable = root.join("Script Kit.app/Contents/MacOS/script-kit-gpui");
        let bundle_engine = root.join("Script Kit.app/Contents/Resources/scripts/migrate/cli.ts");
        let development_engine = root.join("repo/scripts/migrate/cli.ts");
        touch(&bundle_engine);
        touch(&development_engine);

        assert_eq!(
            resolve_migrate_v1_engine(
                None,
                Some(&executable),
                Some(development_engine.clone()),
            ),
            Ok(bundle_engine.clone())
        );
        std::fs::remove_file(&bundle_engine).expect("remove bundled fixture");
        assert_eq!(
            resolve_migrate_v1_engine(None, Some(&executable), Some(development_engine.clone())),
            Ok(development_engine)
        );
        assert_eq!(
            resolve_migrate_v1_engine(None, Some(&executable), None),
            Err(MigrateV1EngineUnavailable::Missing)
        );

        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
