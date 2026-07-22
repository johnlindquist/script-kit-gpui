const FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_BYTES: u64 = 20 * 1024 * 1024;
const FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_DIMENSION: u32 = 8_000;
const FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_SIDE_PX: f32 = 280.0;

fn file_search_list_pane_top_inset(spacing: crate::designs::DesignSpacing) -> f32 {
    spacing.padding_xs
}

const FILE_SEARCH_PREVIEW_PATH_SELECTOR: &str = "file-search-preview-path";
const FILE_SEARCH_PREVIEW_PATH_TEXT_SELECTOR: &str = "file-search-preview-path-text";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileSearchPreviewPathTruncation {
    constrain_width: bool,
    hide_overflow: bool,
    ellipsis: bool,
    nowrap: bool,
}

const fn file_search_preview_path_truncation() -> FileSearchPreviewPathTruncation {
    FileSearchPreviewPathTruncation {
        constrain_width: true,
        hide_overflow: true,
        ellipsis: true,
        nowrap: true,
    }
}

fn file_search_preview_path(
    value: impl Into<SharedString>,
    color: gpui::Rgba,
) -> gpui::Stateful<Div> {
    let truncation = file_search_preview_path_truncation();
    let text = div()
        .debug_selector(|| FILE_SEARCH_PREVIEW_PATH_TEXT_SELECTOR.to_string())
        .text_xs()
        .text_color(color)
        .when(truncation.constrain_width, |element| {
            element.w_full().min_w(px(0.0))
        })
        .when(truncation.hide_overflow, |element| {
            element.overflow_hidden()
        })
        .when(truncation.ellipsis, |element| element.text_ellipsis())
        .when(truncation.nowrap, |element| element.whitespace_nowrap())
        .child(value.into());

    div()
        .id(FILE_SEARCH_PREVIEW_PATH_SELECTOR)
        .debug_selector(|| FILE_SEARCH_PREVIEW_PATH_SELECTOR.to_string())
        .w_full()
        .when(truncation.constrain_width, |element| element.min_w(px(0.0)))
        .when(truncation.hide_overflow, |element| {
            element.overflow_hidden()
        })
        .child(text)
}

static FILE_SEARCH_NATIVE_DRAG_AWAITING_APP_REACTIVATE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchEmptyState {
    NoFilesFound,
}

impl FileSearchEmptyState {
    fn from_query(_query: &str) -> Self {
        Self::NoFilesFound
    }

    fn audit_state(self) -> &'static str {
        match self {
            Self::NoFilesFound => "no_files_found",
        }
    }

    fn render_state(self) -> &'static str {
        match self {
            Self::NoFilesFound => "empty_no_results",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::NoFilesFound => "No files found",
        }
    }
}

fn mark_file_search_native_drag_awaiting_app_reactivate() {
    FILE_SEARCH_NATIVE_DRAG_AWAITING_APP_REACTIVATE
        .store(true, std::sync::atomic::Ordering::SeqCst);
}

fn take_file_search_native_drag_awaiting_app_reactivate() -> bool {
    FILE_SEARCH_NATIVE_DRAG_AWAITING_APP_REACTIVATE.swap(false, std::sync::atomic::Ordering::SeqCst)
}

#[derive(Debug)]
struct FileSearchThumbnailPreviewImage {
    image: Arc<gpui::RenderImage>,
    width: u32,
    height: u32,
}

#[derive(Debug)]
enum FileSearchThumbnailLoadFailure {
    FileTooLarge {
        size_bytes: u64,
    },
    ResolutionTooLarge {
        width: u32,
        height: u32,
        max_dimension: u32,
    },
    UnsupportedFormat,
    UnableToGenerate {
        reason: String,
    },
}

impl FileSearchThumbnailLoadFailure {
    fn preview_message(&self) -> String {
        match self {
            FileSearchThumbnailLoadFailure::FileTooLarge { size_bytes } => {
                let size_mb = (*size_bytes as f64) / (1024.0 * 1024.0);
                format!("File too large for thumbnail preview ({size_mb:.1} MB)")
            }
            FileSearchThumbnailLoadFailure::ResolutionTooLarge {
                width,
                height,
                max_dimension,
            } => {
                format!(
                    "Image resolution too large for preview ({}x{}, max {}x{})",
                    width, height, max_dimension, max_dimension
                )
            }
            FileSearchThumbnailLoadFailure::UnsupportedFormat => {
                "Preview not available for this format".to_string()
            }
            FileSearchThumbnailLoadFailure::UnableToGenerate { reason } => {
                format!("Unable to generate preview: {reason}")
            }
        }
    }
}

/// Shared helper for file-search native drag. Initiates the macOS drag
/// session and schedules GPUI's internal active-drag state to clear after
/// GPUI finishes storing the row drag preview.
fn begin_file_search_native_drag(
    drag_path: &str,
    window: &mut gpui::Window,
    cx: &mut gpui::App,
) -> gpui::Entity<file_search::FileDragPayload> {
    crate::logging::log(
        "FOCUS",
        &format!("FileSearch native drag start: path={drag_path}"),
    );
    if let Err(error) = crate::platform::begin_native_file_drag(drag_path) {
        crate::logging::log(
            "FOCUS",
            &format!(
                "FileSearch native drag platform handoff failed: path={drag_path} error={error}"
            ),
        );
        tracing::warn!(
            path = %drag_path,
            error = %error,
            "failed to start native file drag"
        );
    } else {
        mark_file_search_native_drag_awaiting_app_reactivate();
        crate::logging::log(
            "FOCUS",
            &format!("FileSearch native drag platform handoff succeeded: path={drag_path}"),
        );
    }
    // GPUI sets `active_drag` after this `.on_drag(...)` callback returns.
    // Defer cleanup so the AppKit handoff cannot leave GPUI thinking a row
    // drag is still active after the file was dropped into another app.
    let drag_path_for_log = drag_path.to_string();
    window.defer(cx, move |window, cx| {
        let stopped_drag = cx.stop_active_drag(window);
        crate::logging::log(
            "FOCUS",
            &format!(
                "FileSearch deferred drag cleanup: stopped_drag={} main_window_focused={}",
                stopped_drag,
                crate::platform::is_main_window_focused()
            ),
        );
        tracing::debug!(
            target: "script_kit::keyboard",
            event = "file_search_native_drag_gpui_state_cleared",
            path = %drag_path_for_log,
            stopped_drag,
            "Cleared GPUI file-search drag state after native drag handoff"
        );
    });
    let name = std::path::Path::new(drag_path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    cx.new(|_| file_search::FileDragPayload { name })
}

fn file_search_thumbnail_extension(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_ascii_lowercase)
}

fn file_search_thumbnail_is_decodable_extension(path: &str) -> bool {
    matches!(
        file_search_thumbnail_extension(path).as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico")
    )
}

fn file_search_thumbnail_display_size(width: u32, height: u32, max_side_px: f32) -> (f32, f32) {
    let width_f = width as f32;
    let height_f = height as f32;

    if width_f <= 0.0 || height_f <= 0.0 {
        return (max_side_px, max_side_px);
    }

    let scale = (max_side_px / width_f).min(max_side_px / height_f).min(1.0);
    (width_f * scale, height_f * scale)
}

fn load_file_search_thumbnail_preview(
    path: &str,
    max_bytes: u64,
    max_dimension: u32,
) -> Result<FileSearchThumbnailPreviewImage, FileSearchThumbnailLoadFailure> {
    use anyhow::Context as _;
    use image::GenericImageView as _;

    if !file_search_thumbnail_is_decodable_extension(path) {
        return Err(FileSearchThumbnailLoadFailure::UnsupportedFormat);
    }

    let metadata = std::fs::metadata(path)
        .with_context(|| format!("failed to read metadata for '{}'", path))
        .map_err(|error| FileSearchThumbnailLoadFailure::UnableToGenerate {
            reason: error.to_string(),
        })?;

    let size_bytes = metadata.len();
    if size_bytes > max_bytes {
        return Err(FileSearchThumbnailLoadFailure::FileTooLarge { size_bytes });
    }

    let image_bytes = std::fs::read(path)
        .with_context(|| format!("failed to read image bytes for '{}'", path))
        .map_err(|error| FileSearchThumbnailLoadFailure::UnableToGenerate {
            reason: error.to_string(),
        })?;

    let decoded_image = match image::load_from_memory(&image_bytes) {
        Ok(image) => image,
        Err(image_error) => {
            if matches!(image_error, image::ImageError::Unsupported(_)) {
                return Err(FileSearchThumbnailLoadFailure::UnsupportedFormat);
            }

            let reason = anyhow::Error::new(image_error)
                .context(format!("failed to decode image data for '{}'", path))
                .to_string();
            return Err(FileSearchThumbnailLoadFailure::UnableToGenerate { reason });
        }
    };

    let (width, height) = decoded_image.dimensions();
    if width > max_dimension || height > max_dimension {
        return Err(FileSearchThumbnailLoadFailure::ResolutionTooLarge {
            width,
            height,
            max_dimension,
        });
    }

    let mut bgra = decoded_image.to_rgba8();
    for pixel in bgra.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }

    let frame = image::Frame::new(bgra);
    let render_image = gpui::RenderImage::new(smallvec::smallvec![frame]);

    Ok(FileSearchThumbnailPreviewImage {
        image: Arc::new(render_image),
        width,
        height,
    })
}

fn file_search_row_trailing_metadata(
    size: u64,
    modified: u64,
    list_colors: &crate::list_item::ListItemColors,
    selected: bool,
) -> AnyElement {
    let metadata_color = rgba(crate::list_item::row_description_text_rgba(
        list_colors,
        selected,
    ));
    let summary = format!(
        "{} · {}",
        file_search::format_file_size(size),
        file_search::format_relative_time(modified)
    );
    div()
        .max_w(px(88.0))
        .overflow_hidden()
        .text_xs()
        .whitespace_nowrap()
        .text_ellipsis()
        .text_color(metadata_color)
        .child(summary)
        .into_any_element()
}

fn render_file_search_loading_skeleton(
    list_colors: &crate::list_item::ListItemColors,
    ui_border: u32,
    text_dimmed: u32,
    compact: bool,
) -> AnyElement {
    let skeleton_bg = rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
        ui_border,
        crate::theme::opacity::OPACITY_GHOST,
    ));
    let skeleton_strong = rgba(crate::ui_foundation::hex_to_rgba_with_opacity(
        ui_border,
        crate::theme::opacity::OPACITY_SUBTLE,
    ));
    let row_height = LIST_ITEM_HEIGHT;
    let icon_size = 20.0;
    let row_specs = [
        (156.0, 246.0, 52.0, 70.0),
        (214.0, 302.0, 44.0, 62.0),
        (182.0, 274.0, 58.0, 78.0),
        (238.0, 326.0, 48.0, 66.0),
        (168.0, 256.0, 56.0, 72.0),
        (206.0, 288.0, 42.0, 60.0),
    ];

    div()
        .w_full()
        .h_full()
        .flex()
        .flex_col()
        .py(px(6.0))
        .children(row_specs.into_iter().enumerate().map(
            |(ix, (title_w, path_w, size_w, age_w))| {
                div()
                    .id(ix)
                    .w_full()
                    .h(px(row_height))
                    .flex()
                    .flex_row()
                    .items_center()
                    .px(px(12.0))
                    .gap(px(8.0))
                    .when(ix == 0, |row| {
                        row.bg(rgba(crate::list_item::row_selected_background_rgba(
                            list_colors,
                        )))
                    })
                    .child(
                        div()
                            .w(px(icon_size))
                            .h(px(icon_size))
                            .rounded(px(6.0))
                            .border_1()
                            .border_color(skeleton_strong)
                            .bg(skeleton_bg),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.0))
                            .flex()
                            .flex_col()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .w(px(title_w))
                                    .max_w_full()
                                    .h(px(10.0))
                                    .rounded(px(5.0))
                                    .bg(skeleton_strong),
                            )
                            .child(
                                div()
                                    .w(px(path_w))
                                    .max_w_full()
                                    .h(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(skeleton_bg),
                            ),
                    )
                    .child(
                        div()
                            .w(px(if compact { 76.0 } else { 104.0 }))
                            .flex()
                            .flex_col()
                            .items_end()
                            .gap(px(2.0))
                            .child(
                                div()
                                    .w(px(size_w))
                                    .h(px(8.0))
                                    .rounded(px(4.0))
                                    .bg(skeleton_bg),
                            )
                            .child(div().w(px(age_w)).h(px(8.0)).rounded(px(4.0)).bg(rgba(
                                crate::ui_foundation::hex_to_rgba_with_opacity(
                                    text_dimmed,
                                    crate::theme::opacity::OPACITY_GHOST,
                                ),
                            ))),
                    )
            },
        ))
        .into_any_element()
}

impl ScriptListApp {
    fn ensure_file_search_preview_thumbnail(
        &mut self,
        selected_file: Option<&file_search::FileResult>,
        cx: &mut Context<Self>,
    ) {
        let thumbnail_path = selected_file
            .filter(|file| file_search::is_thumbnail_preview_supported(&file.path))
            .map(|file| file.path.clone());

        let Some(path) = thumbnail_path else {
            if !matches!(
                self.file_search_preview_thumbnail,
                FileSearchThumbnailPreviewState::Idle
            ) {
                tracing::debug!("file_search_thumbnail_preview_state_transition: idle");
                self.file_search_preview_thumbnail = FileSearchThumbnailPreviewState::Idle;
                cx.notify();
            }
            return;
        };

        let already_loaded_for_path = match &self.file_search_preview_thumbnail {
            FileSearchThumbnailPreviewState::Loading { path: current_path }
            | FileSearchThumbnailPreviewState::Ready {
                path: current_path, ..
            }
            | FileSearchThumbnailPreviewState::Unavailable {
                path: current_path, ..
            } => current_path == &path,
            FileSearchThumbnailPreviewState::Idle => false,
        };

        if already_loaded_for_path {
            return;
        }

        tracing::debug!(
            path = %path,
            "file_search_thumbnail_preview_state_transition: loading"
        );
        self.file_search_preview_thumbnail =
            FileSearchThumbnailPreviewState::Loading { path: path.clone() };
        cx.notify();

        cx.spawn(async move |this, cx| {
            let (tx, rx) = std::sync::mpsc::channel();
            let path_for_decode = path.clone();
            std::thread::spawn(move || {
                let result = load_file_search_thumbnail_preview(
                    &path_for_decode,
                    FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_BYTES,
                    FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_DIMENSION,
                );
                let _ = tx.send(result);
            });

            let decode_result = loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;
                match rx.try_recv() {
                    Ok(result) => break result,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {}
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        break Err(FileSearchThumbnailLoadFailure::UnableToGenerate {
                            reason: "thumbnail worker disconnected".to_string(),
                        });
                    }
                }
            };

            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    let is_still_current_request = matches!(
                        &app.file_search_preview_thumbnail,
                        FileSearchThumbnailPreviewState::Loading { path: current_path }
                            if current_path == &path
                    );

                    if !is_still_current_request {
                        tracing::debug!(
                            path = %path,
                            "file_search_thumbnail_preview_stale_result_ignored"
                        );
                        return;
                    }

                    match decode_result {
                        Ok(loaded) => {
                            tracing::debug!(
                                path = %path,
                                width = loaded.width,
                                height = loaded.height,
                                "file_search_thumbnail_preview_state_transition: ready"
                            );
                            app.file_search_preview_thumbnail =
                                FileSearchThumbnailPreviewState::Ready {
                                    path: path.clone(),
                                    image: loaded.image,
                                    width: loaded.width,
                                    height: loaded.height,
                                };
                        }
                        Err(error) => {
                            let message = error.preview_message();
                            tracing::warn!(
                                path = %path,
                                reason = %message,
                                "file_search_thumbnail_preview_state_transition: unavailable"
                            );
                            app.file_search_preview_thumbnail =
                                FileSearchThumbnailPreviewState::Unavailable {
                                    path: path.clone(),
                                    message,
                                };
                        }
                    }

                    cx.notify();
                })
            });
        })
        .detach();
    }

    /// Render file search view with 50/50 split (list + preview) or mini list-only
    pub(crate) fn render_file_search(
        &mut self,
        query: &str,
        selected_index: usize,
        presentation: FileSearchPresentation,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        use crate::file_search::{self, FileType};

        let is_mini = matches!(presentation, FileSearchPresentation::Mini);
        let chrome_audit = if is_mini {
            crate::components::PromptChromeAudit::minimal_list("file_search", true)
        } else {
            crate::components::PromptChromeAudit::expanded("file_search", true)
        };
        crate::components::emit_prompt_chrome_audit(&chrome_audit);

        // Use design tokens for spacing/visual, theme for colors
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let _design_typography = tokens.typography();
        let design_visual = tokens.visual();
        let color_resolver =
            crate::theme::ColorResolver::new_for_shell(&self.theme, self.current_design);
        let typography_resolver =
            crate::theme::TypographyResolver::new_theme_first(&self.theme, self.current_design);
        let empty_text_color = color_resolver.empty_text_color();
        let empty_font_family = typography_resolver.primary_font().to_string();
        let _opacity = self.theme.get_opacity();
        // bg_with_alpha removed - let vibrancy show through from Root (matches main menu)
        // Removed: box_shadows - shadows on transparent elements block vibrancy
        let _box_shadows = self.create_box_shadows();

        // Color values for use in closures
        let ui_border = self.theme.colors.ui.border;
        let list_colors = crate::list_item::ListItemColors::from_theme(&self.theme);
        let text_primary = self.theme.colors.text.primary;
        let text_muted = self.theme.colors.text.muted;
        let text_dimmed = self.theme.colors.text.dimmed;

        // Get selected file for preview (if any)
        // Clamp the display index so a stale selected_index from a shrinking
        // result set still resolves to a valid visible row.
        let selection = self.file_search_selection_binding(selected_index);
        let selected_file = selection.file.clone();
        let clamped_selected_index = selection
            .projection
            .map(|projection| projection.display_index);
        self.ensure_file_search_preview_thumbnail(selected_file.as_ref(), cx);

        // Use pre-computed display indices instead of running Nucleo in render
        // This is CRITICAL for animation performance - render must be cheap
        // The display_indices are computed in recompute_file_search_display_indices()
        // which is called when:
        // 1. Results change (new directory listing or search results)
        // 2. Filter pattern changes (user types in existing directory)
        // 3. Loading completes
        let display_indices = &self.file_search_display_indices;
        let filtered_len = display_indices.len();

        // Log render state (throttled - only when state changes meaningfully)
        static LAST_LOG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let last = LAST_LOG.load(std::sync::atomic::Ordering::Relaxed);
        if now_ms.saturating_sub(last) > 500 {
            // Log at most every 500ms
            LAST_LOG.store(now_ms, std::sync::atomic::Ordering::Relaxed);
            logging::log(
                "SEARCH",
                &format!(
                    "render_file_search: query='{}' loading={} cached={} display={} selected={}",
                    query,
                    self.file_search_loading,
                    self.cached_file_results.len(),
                    filtered_len,
                    selected_index
                ),
            );
        }

        // Key handler for file search
        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                // Hide cursor while typing - automatically shows when mouse moves
                this.hide_mouse_cursor(cx);

                // If the shortcut recorder is active, don't process any key events.
                // The recorder has its own key handlers and should receive all key events.
                if this.shortcut_recorder_state.is_some() {
                    return;
                }

                let key = event.keystroke.key.as_str();
                let key_char = event.keystroke.key_char.as_deref();
                let has_cmd = event.keystroke.modifiers.platform;

                let modifiers = &event.keystroke.modifiers;

                // Route keys to actions dialog first if it's open
                match this.route_key_to_actions_dialog(
                    key,
                    key_char,
                    modifiers,
                    ActionsDialogHost::FileSearch,
                    window,
                    cx,
                ) {
                    ActionsRoute::NotHandled => {
                        // Actions dialog not open - continue to file search key handling
                    }
                    ActionsRoute::Handled => {
                        // Key was consumed by actions dialog
                        return;
                    }
                    ActionsRoute::Execute {
                        action_id,
                        should_close,
                    } => {
                        this.execute_actions_route_action(
                            ActionsDialogHost::FileSearch,
                            action_id,
                            should_close,
                            window,
                            cx,
                        );
                        return;
                    }
                }

                // ESC: In portal mode, cancel and return to Agent Chat chat.
                // Otherwise, clear query first; if empty, go back/close.
                if is_key_escape(key) {
                    if this.is_in_attachment_portal() {
                        this.close_attachment_portal_cancel(cx);
                        return;
                    }
                    if !this.clear_builtin_view_filter(cx) {
                        this.go_back_or_close(window, cx);
                    }
                    return;
                }

                // Cmd+W closes window
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    logging::log("KEY", "Cmd+W - closing window");
                    this.close_and_reset_window(cx);
                    return;
                }

                if let AppView::FileSearchView {
                    query: _,
                    selected_index,
                    ..
                } = &mut this.current_view
                {
                    // Copy the index so the mutable borrow on current_view is
                    // released before the closure borrows `this` immutably.
                    let sel_idx = *selected_index;

                    let get_selected_file = || this.file_search_selection_binding(sel_idx).file;

                    // Space (unmodified) triggers Quick Look for non-directory files
                    let is_space = key.eq_ignore_ascii_case("space") || key_char == Some(" ");
                    if is_space
                        && !event.keystroke.modifiers.platform
                        && !event.keystroke.modifiers.shift
                        && !event.keystroke.modifiers.control
                        && !event.keystroke.modifiers.alt
                    {
                        if let Some(file) = get_selected_file() {
                            if file.file_type != FileType::Directory {
                                this.file_search_actions_path = Some(file.path.clone());
                                this.handle_action("quick_look".to_string(), window, cx);
                                cx.stop_propagation();
                                return;
                            }
                        }
                    }

                    match key {
                        // Arrow keys are handled by arrow_interceptor in app_impl.rs
                        // which calls stop_propagation(). This is the single source of truth
                        // for arrow key handling in FileSearchView.
                        _ if is_key_up(key) || is_key_down(key) => {
                            // Already handled by interceptor, no-op here
                        }
                        // Tab/Shift+Tab handled by intercept_keystrokes in app_impl.rs
                        // (interceptor fires BEFORE input component can capture Tab)
                        _ if is_key_enter(key) => {
                            if has_cmd {
                                let has_shift = event.keystroke.modifiers.shift;
                                // Snapshot selection status before mutable borrows.
                                let has_selection = get_selected_file().is_some();

                                // Plain ⌘↵ (no Shift): prefer the shared launcher
                                // route so FileSearch converges on the same Agent Chat
                                // context-capture path as other main-window surfaces.
                                if !has_shift {
                                    tracing::info!(
                                        target: "script_kit::tab_ai",
                                        event = "file_search_cmd_enter_global_route_attempted",
                                        has_selection,
                                    );

                                    if this
                                        .try_route_global_cmd_enter_to_agent_chat_context_capture(
                                            cx,
                                        )
                                    {
                                        cx.stop_propagation();
                                        return;
                                    }
                                }

                                // ⌘⇧↵ or shared-route fallback: use the local
                                // selection-or-query helper for file-specific AI.
                                if has_shift {
                                    tracing::info!(
                                        target: "script_kit::tab_ai",
                                        event = "file_search_cmd_shift_enter_local_ai",
                                        has_selection,
                                    );
                                }

                                let ai_args = if let AppView::FileSearchView {
                                    ref query,
                                    selected_index,
                                    ..
                                } = this.current_view
                                {
                                    Some((query.clone(), selected_index))
                                } else {
                                    None
                                };
                                if let Some((query, sel_idx)) = ai_args {
                                    if this.open_file_search_selection_or_query_in_tab_ai(
                                        &query, sel_idx, has_shift, cx,
                                    ) {
                                        cx.stop_propagation();
                                    }
                                }
                            } else if let Some(file) = get_selected_file() {
                                // cwd-pick mode: Enter on a directory sets
                                // spine_cwd and returns to ScriptList. Enter
                                // on a file is a no-op so the user can keep
                                // navigating until they pick a directory.
                                if this.cwd_pick_mode {
                                    if file.file_type == FileType::Directory {
                                        // Footer chip shows the full path
                                        // (~/dev rather than just "dev") so
                                        // the user can tell sibling-named
                                        // directories apart.
                                        let label = file_search::shorten_path(&file.path)
                                            .trim_end_matches('/')
                                            .to_string();
                                        tracing::info!(
                                            target: "script_kit::spine",
                                            event = "cwd_pick_resolve",
                                            path = %file.path,
                                            label = %label,
                                            "Enter in cwd-pick FileSearchView set spine_cwd"
                                        );
                                        this.spine_cwd =
                                            Some(std::path::PathBuf::from(&file.path));
                                        this.spine_cwd_label = Some(label);
                                        this.spine_cwd_revision =
                                            this.spine_cwd_revision.wrapping_add(1);
                                        this.cwd_pick_mode = false;
                                        this.invalidate_grouped_cache();
                                        // Warm a Pi session for this cwd now
                                        // so the next Cmd+Enter launches the
                                        // agent in the chosen directory
                                        // without a cold-start "try again".
                                        this.prewarm_agent_chat_for_spine_cwd(cx);
                                        this.persist_spine_cwd();
                                        this.reset_to_script_list(cx);
                                        this.clear_filter(window, cx);
                                        // Sentinel: prevents the shared
                                        // gpui input's `InputEvent::PressEnter`
                                        // subscriber from firing
                                        // `execute_selected` on the freshly
                                        // restored ScriptList view.
                                        this.record_return_to_script_list_submit(
                                            "cwd_pick",
                                            "file_search_enter_keydown",
                                            Some(&file.path),
                                        );
                                    }
                                    cx.stop_propagation();
                                    return;
                                }

                                // Portal mode: attach file to Agent Chat chat and return.
                                if this.is_in_attachment_portal() {
                                    if file.file_type == FileType::Directory {
                                        let next_query = format!(
                                            "{}/",
                                            file_search::shorten_path(&file.path)
                                                .trim_end_matches('/')
                                        );
                                        let next_presentation = match &this.current_view {
                                            AppView::FileSearchView {
                                                presentation, ..
                                            } => *presentation,
                                            _ => FileSearchPresentation::Full,
                                        };
                                        this.open_file_search_view_preserving_current_results(
                                            next_query,
                                            next_presentation,
                                            cx,
                                        );
                                        cx.stop_propagation();
                                        return;
                                    }

                                    let part =
                                        crate::ai::message_parts::AiContextPart::FilePath {
                                            path: file.path.clone(),
                                            label: std::path::Path::new(&file.path)
                                                .file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_else(|| file.path.clone()),
                                        };
                                    this.close_attachment_portal_with_part(part, cx);
                                    cx.stop_propagation();
                                    return;
                                }

                                // Standard file search: open with the default app and close,
                                // even when the selected item is a directory.
                                // Journal the query → choice pair so the brain
                                // can answer "what did I just search for?".
                                let query = this.filter_text.trim();
                                if !query.is_empty() {
                                    crate::brain::record_activity(
                                        "searched files for",
                                        &format!("\"{}\" and opened {}", query, file.path),
                                    );
                                }
                                let _ = file_search::open_file(&file.path);
                                this.close_and_reset_window(cx);
                            }
                        }
                        _ => {
                            let has_shift = event.keystroke.modifiers.shift;

                            // Handle Cmd+K (toggle actions)
                            if has_cmd && key.eq_ignore_ascii_case("k") && !has_shift {
                                let selected = get_selected_file();
                                this.toggle_file_search_actions(selected.as_ref(), window, cx);
                                return;
                            }
                            // Handle Cmd+F (focus the search input) — this chord is
                            // advertised in the footer hint set ("⌘F Focus"), so it
                            // must have a live handler.
                            if has_cmd && !has_shift && key.eq_ignore_ascii_case("f") {
                                this.focus_main_filter(window, cx);
                                return;
                            }
                            // Handle Cmd+Shift+F (Reveal in Finder) — kept explicit
                            // because it is not a secondary command in the shared contract.
                            if has_cmd && has_shift && key.eq_ignore_ascii_case("f") {
                                if let Some(file) = get_selected_file() {
                                    this.file_search_actions_path = Some(file.path.clone());
                                    this.handle_action("reveal_in_finder".to_string(), window, cx);
                                }
                                return;
                            }
                            // All other Cmd shortcuts resolve through the shared
                            // secondary-command contract so the footer, action sheet,
                            // and keyboard dispatch stay in sync.
                            if let Some(file) = get_selected_file() {
                                let file_info = crate::file_search::FileInfo::from_result(&file);
                                if let Some(action_id) =
                                    crate::actions::resolve_file_search_secondary_action_id(
                                        key, has_cmd, has_shift, &file_info,
                                    )
                                {
                                    this.file_search_actions_path = Some(file.path.clone());
                                    this.handle_action(action_id.to_string(), window, cx);
                                }
                            }
                        }
                    }
                }
            },
        );

        // Clone data for the uniform_list closure
        // Use display_indices to get files in the correct order (filtered + sorted)
        // Include the original result index for animation timestamp lookup
        let files_for_closure: Vec<(usize, file_search::FileResult)> = display_indices
            .iter()
            .filter_map(|&idx| self.cached_file_results.get(idx).map(|f| (idx, f.clone())))
            .collect();
        let current_selected = clamped_selected_index.unwrap_or(usize::MAX);
        let file_hovered = self.hovered_index;
        let is_loading = self.file_search_loading;
        let click_entity_handle = cx.entity().downgrade();
        let hover_entity_handle = cx.entity().downgrade();

        // Use uniform_list for virtualized scrolling
        // Skeleton loading: show placeholder rows while loading and no results yet
        tracing::info!(
            target: "script_kit::prompt_chrome",
            surface = "file_search",
            loading_state = if is_loading && filtered_len == 0 { "skeleton" } else { "content" },
            empty_state = if !is_loading && filtered_len == 0 {
                FileSearchEmptyState::from_query(query).audit_state()
            } else { "" },
            "file_search_state_audit"
        );
        let list_element = if is_loading && filtered_len == 0 {
            // Loading with no results yet - show static skeleton rows.
            // Render 6 skeleton rows so the list pane stays stable while choices load.
            render_file_search_loading_skeleton(&list_colors, ui_border, text_dimmed, is_mini)
        } else if filtered_len == 0 {
            // No results and not loading - show empty state message
            let state = FileSearchEmptyState::from_query(query);
            let icon = crate::designs::icon_variations::IconName::Folder;
            crate::list_item::EmptyState::new(state.title(), empty_text_color, &empty_font_family)
                .icon(icon)
                .into_element()
        } else {
            uniform_list(
                "file-search-list",
                filtered_len,
                move |visible_range, _window, _cx| {
                    visible_range
                        .map(|ix| {
                            if let Some((_result_idx, file)) = files_for_closure.get(ix) {
                                let is_selected = ix == current_selected;
                                let is_hovered = file_hovered == Some(ix);

                                // Click handler: select on click, open/browse on double-click
                                let click_entity = click_entity_handle.clone();
                                let file_path = file.path.clone();
                                let file_type = file.file_type;
                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = click_entity.upgrade() {
                                            let file_path = file_path.clone();
                                            app.update(cx, |this, cx| {
                                                this.lock_file_search_selection_to_user_choice();
                                                if let AppView::FileSearchView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    *selected_index = ix;
                                                }
                                                cx.notify();

                                                // Double-click: browse directory inline or open file
                                                if let gpui::ClickEvent::Mouse(mouse_event) = event
                                                {
                                                    if mouse_event.down.click_count == 2 {
                                                        if file_type == FileType::Directory {
                                                            let next_query = format!(
                                                                "{}/",
                                                                file_search::shorten_path(
                                                                    &file_path
                                                                )
                                                                .trim_end_matches('/')
                                                            );
                                                            let next_presentation = match &this
                                                                .current_view
                                                            {
                                                                AppView::FileSearchView {
                                                                    presentation,
                                                                    ..
                                                                } => *presentation,
                                                                _ => FileSearchPresentation::Full,
                                                            };
                                                            this
                                                                .open_file_search_view_preserving_current_results(
                                                                next_query,
                                                                next_presentation,
                                                                cx,
                                                            );
                                                        } else {
                                                            logging::log(
                                                                "UI",
                                                                &format!(
                                                                    "Double-click opening file: {}",
                                                                    file_path
                                                                ),
                                                            );
                                                            let _ =
                                                                file_search::open_file(&file_path);
                                                            this.close_and_reset_window(cx);
                                                        }
                                                    }
                                                }
                                            });
                                        }
                                    };

                                let hover_entity = hover_entity_handle.clone();
                                let hover_handler =
                                    move |hov: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(app) = hover_entity.upgrade() {
                                            app.update(cx, |this, cx| {
                                                if *hov {
                                                    this.input_mode = InputMode::Mouse;
                                                    if this.hovered_index != Some(ix) {
                                                        this.hovered_index = Some(ix);
                                                        cx.notify();
                                                    }
                                                } else if this.hovered_index == Some(ix) {
                                                    this.hovered_index = None;
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    };

                                // Drag payload for native file drag-out
                                let drag_payload = file_search::FileDragPayload::from_result(file);
                                let drag_path_for_native = file.path.clone();

                                let item = ListItem::new(file.name.clone(), list_colors)
                                    .description(file_search::shorten_path(&file.path))
                                    .icon(file_search::file_type_icon(file.file_type))
                                    .selected(is_selected)
                                    .hovered(is_hovered)
                                    .trailing_accessory(file_search_row_trailing_metadata(
                                        file.size,
                                        file.modified,
                                        &list_colors,
                                        is_selected,
                                    ));

                                div()
                                    .id(ix)
                                    .cursor_pointer()
                                    .on_click(click_handler)
                                    .on_hover(hover_handler)
                                    .on_drag(
                                        drag_payload,
                                        move |_payload, _position, window, cx| {
                                            begin_file_search_native_drag(
                                                &drag_path_for_native,
                                                window,
                                                cx,
                                            )
                                        },
                                    )
                                    .child(item)
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.file_search_scroll_handle)
            .into_any_element()
        };
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.file_search_scroll_handle, filtered_len, 8);

        // Build preview panel content - matching main menu labeled section pattern
        let preview_content = if let Some(file) = &selected_file {
            let file_type_str = match file.file_type {
                FileType::Directory => "Folder",
                FileType::Image => "Image",
                FileType::Audio => "Audio",
                FileType::Video => "Video",
                FileType::Document => "Document",
                FileType::Application => "Application",
                FileType::File => "File",
                FileType::Other => "File",
            };
            let preview_supports_thumbnail =
                file_search::is_thumbnail_preview_supported(&file.path);
            let thumbnail_section = if preview_supports_thumbnail {
                let preview_body = match &self.file_search_preview_thumbnail {
                    FileSearchThumbnailPreviewState::Ready {
                        path,
                        image,
                        width,
                        height,
                    } if path == &file.path => {
                        let (display_width, display_height) = file_search_thumbnail_display_size(
                            *width,
                            *height,
                            FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_SIDE_PX,
                        );
                        let image_for_render = image.clone();
                        div()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(design_spacing.gap_sm))
                            .child(
                                gpui::img(move |_window: &mut Window, _cx: &mut App| {
                                    Some(Ok(image_for_render.clone()))
                                })
                                .w(px(display_width))
                                .h(px(display_height))
                                .object_fit(gpui::ObjectFit::Contain)
                                .rounded(px(design_visual.radius_sm)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(text_muted))
                                    .child(format!("{}×{} px", width, height)),
                            )
                            .into_any_element()
                    }
                    FileSearchThumbnailPreviewState::Unavailable { path, message }
                        if path == &file.path =>
                    {
                        div()
                            .w_full()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child(message.clone())
                            .into_any_element()
                    }
                    FileSearchThumbnailPreviewState::Loading { path } if path == &file.path => {
                        div()
                            .w_full()
                            .text_sm()
                            .text_color(rgb(text_dimmed))
                            .child("Loading thumbnail...")
                            .into_any_element()
                    }
                    _ => div()
                        .w_full()
                        .text_sm()
                        .text_color(rgb(text_dimmed))
                        .child("Loading thumbnail...")
                        .into_any_element(),
                };

                Some(
                    div()
                        .flex()
                        .flex_col()
                        .pb(px(design_spacing.padding_md))
                        // Chromeless preview: no header label, no card wrapper
                        .child(
                            div()
                                .w_full()
                                .min_h(px(FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_SIDE_PX + 24.0))
                                .flex()
                                .items_center()
                                .justify_center()
                                .overflow_hidden()
                                .child(preview_body),
                        ),
                )
            } else {
                None
            };

            {
                let meta_row = |label: &str, value: String| {
                    div()
                        .flex()
                        .flex_row()
                        .gap(px(design_spacing.gap_md))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(text_muted))
                                .child(label.to_string()),
                        )
                        .child(
                            div()
                                .ml_auto()
                                .text_xs()
                                .text_color(rgb(text_dimmed))
                                .child(value),
                        )
                };

                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .p(px(design_spacing.padding_lg))
                    .gap(px(design_spacing.gap_md))
                    .overflow_y_hidden()
                    .when_some(thumbnail_section, |container, section| {
                        container.child(section)
                    })
                    // Name (chromeless — no type badge pill)
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(text_primary))
                            .child(file.name.clone()),
                    )
                    // Truncated path (no section label — content is self-evident)
                    .child(file_search_preview_path(
                        file.path.clone(),
                        rgb(text_dimmed),
                    ))
                    // Meta rows (no divider, no section label — spacing defines groups)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(design_spacing.gap_sm))
                            .child(meta_row("Size", file_search::format_file_size(file.size)))
                            .child(meta_row(
                                "Modified",
                                file_search::format_relative_time(file.modified),
                            ))
                            .child(meta_row("Type", file_type_str.to_string())),
                    )
            }
        } else if is_loading {
            div().flex_1().flex().items_center().justify_center().child(
                div()
                    .text_sm()
                    .text_color(rgb(text_muted))
                    .child("Loading preview\u{2026}"),
            )
        } else {
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(6.0))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(text_dimmed))
                        .child("No file selected"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted))
                        .child("Use \u{2191}\u{2193} to inspect results"),
                )
        };

        let is_directory_query = crate::file_search::parse_directory_path(query).is_some();
        let is_advanced_query = crate::file_search::looks_like_advanced_mdquery(query);

        let empty_state = FileSearchEmptyState::from_query(query);
        let (empty_title, empty_subtitle) = if is_directory_query && query.ends_with('/') {
            (
                "Folder is empty",
                "Try another path, or press \u{2318}\u{21b5} for AI help deciding what to inspect next",
            )
        } else if is_directory_query {
            (
                "No matches in folder",
                "Keep typing to narrow the current directory, or press \u{2318}\u{21b5} for AI help refining the browse",
            )
        } else if is_advanced_query {
            (
                "No Spotlight matches",
                "Try a broader predicate, or press \u{2318}\u{21b5} for AI help rewriting the query",
            )
        } else {
            (
                "No files found",
                "Try a broader query, or press \u{2318}\u{21b5} to ask AI how to refine this search",
            )
        };
        let render_empty_list_state = || {
            div()
                .max_w(px(320.0))
                .px(px(18.0))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.0))
                .child(
                    div()
                        .text_size(px(19.0))
                        .line_height(px(24.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(rgb(text_primary))
                        .text_center()
                        .child(empty_title),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .line_height(px(19.0))
                        .text_color(rgba((self.theme.colors.text.secondary << 8) | 0xD9))
                        .text_center()
                        .child(empty_subtitle),
                )
        };

        // Footer: strict three-key pattern. The AI entry chords
        // ⌘↵ (Explain) / ⌘⇧↵ (Plan) are now visible in the footer
        // so users can discover them without guessing.
        let ai_hint = "⌘↵ Explain with AI · ⌘⇧↵ Plan with AI";
        let file_search_hints = if self.is_in_attachment_portal() {
            // Portal mode: simplified footer indicating attach context.
            let primary = if selected_file
                .as_ref()
                .map(|f| matches!(f.file_type, file_search::FileType::Directory))
                .unwrap_or(false)
            {
                "\u{21b5} Browse"
            } else {
                "\u{21b5} Attach"
            };
            vec![
                primary.into(),
                "Esc Cancel".into(),
                "Attaching to Agent Chat".into(),
            ]
        } else if selected_file.is_some() {
            // Tab browses into the selected directory (Shift+Tab goes up) —
            // the surface's signature interaction, so it must be advertised.
            let selected_is_directory = selected_file
                .as_ref()
                .map(|f| matches!(f.file_type, file_search::FileType::Directory))
                .unwrap_or(false);
            let primary = if selected_is_directory {
                "\u{21b5} Open · \u{21e5} Browse"
            } else {
                "\u{21b5} Open"
            };
            vec![primary.into(), ai_hint.into(), "\u{2318}K Actions".into()]
        } else if self.file_search_current_dir.is_some() {
            vec![
                "\u{21b5} Open".into(),
                ai_hint.into(),
                "\u{2318}K Actions".into(),
            ]
        } else if is_loading {
            vec![ai_hint.into(), "Searching".into(), "\u{2318}F Focus".into()]
        } else {
            let primary = if is_directory_query {
                "\u{21b5} Open"
            } else {
                "\u{21b5} Run"
            };
            vec![primary.into(), ai_hint.into(), "\u{2318}F Focus".into()]
        };

        // Header: shared main-view input shell. Search metadata stays out of
        // the input row so main-window chrome keeps identical geometry.
        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;
        let header_gap = shell.header_gap;
        let input_body = div()
            .flex_1()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(header_gap))
            .occlude()
            .capture_any_mouse_down(cx.listener(|this, _event, window, cx| {
                if matches!(this.current_view, AppView::FileSearchView { .. }) {
                    let needs_app_reactivate =
                        take_file_search_native_drag_awaiting_app_reactivate();
                    let stopped_drag = cx.stop_active_drag(window);
                    if needs_app_reactivate {
                        crate::platform::activate_main_window();
                    }
                    window.activate_window();
                    let input_state = this.gpui_input_state.clone();
                    this.focus_main_filter(window, cx);
                    logging::log(
                        "FOCUS",
                        &format!(
                            "FileSearch header mouse capture: stopped_drag={} restored_main_filter_focus=true activated_window=true needs_app_reactivate={} main_window_focused={}",
                            stopped_drag,
                            needs_app_reactivate,
                            crate::platform::is_main_window_focused()
                        ),
                    );
                    if needs_app_reactivate {
                        window.defer(cx, move |window, cx| {
                            crate::platform::activate_main_window();
                            window.activate_window();
                            input_state.update(cx, |state, cx| {
                                state.focus(window, cx);
                            });
                            logging::log(
                                "FOCUS",
                                &format!(
                                    "FileSearch deferred header refocus: rekeyed_panel=true activated_app=true main_window_focused={}",
                                    crate::platform::is_main_window_focused()
                                ),
                            );
                        });
                    }
                }
            }))
            .on_mouse_move(cx.listener(|this, _: &MouseMoveEvent, _window, cx| {
                if this.hovered_index.is_some() {
                    this.hovered_index = None;
                    cx.notify();
                }
                cx.stop_propagation();
            }))
            .child(
                div().flex_1().flex().flex_row().items_center().child(
                    self.render_search_input(),
                ),
            );
        let input = crate::components::main_view_chrome::render_main_view_input_shell(
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewInputChrome {
                body: input_body.into_any_element(),
                trailing: Vec::new(),
            },
        );
        let header = crate::components::main_view_chrome::MainViewHeaderChrome::canonical(
            menu_def,
            self.render_clickable_main_view_context_zone(menu_def, cx),
            input,
        );
        let divider = crate::components::main_view_chrome::MainViewDividerChrome {
            margin_x: shell.divider_margin_x,
            height: shell.divider_height,
            visible: false,
        };

        // List pane: loading/empty/results with scrollbar overlay.
        let list_pane = if is_loading && filtered_len == 0 {
            tracing::info!(
                target: "script_kit::prompt_chrome",
                surface = "file_search",
                state = "loading_skeleton",
                filtered_len,
                "file_search_state_rendered"
            );
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .child(list_element)
        } else if filtered_len == 0 {
            tracing::info!(
                target: "script_kit::prompt_chrome",
                surface = "file_search",
                state = empty_state.render_state(),
                filtered_len,
                "file_search_state_rendered"
            );
            div()
                .w_full()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .child(render_empty_list_state())
        } else {
            tracing::info!(
                target: "script_kit::prompt_chrome",
                surface = "file_search",
                state = "results",
                filtered_len,
                "file_search_state_rendered"
            );
            div()
                .relative()
                .w_full()
                .h_full()
                .child(list_element)
                .child(list_scrollbar)
        };
        let list_pane = div()
            .w_full()
            .h_full()
            .flex()
            .flex_col()
            .pt(px(file_search_list_pane_top_inset(design_spacing)))
            .child(
                crate::components::builtin_leading_separator::render_builtin_leading_separator(
                    if query.trim().is_empty() {
                        "Files"
                    } else {
                        "Results"
                    },
                    is_loading.then_some("Indexing files"),
                    list_colors,
                ),
            )
            .child(div().relative().flex_1().min_h(px(0.0)).child(list_pane));

        // Preview pane: file detail or placeholder
        let preview_pane = preview_content;

        // Emit the audit from the live renderer so the report reads
        // the real surface, not a stale layout helper.
        if is_mini {
            crate::components::emit_prompt_hint_audit("file_search", &file_search_hints);
        }

        // Glass fallback uses the same rail geometry, button frames, and
        // optically nudged keycaps as the native footer.
        let gpui_footer =
            crate::components::footer_chrome::render_static_footer_hint_action_rail(
                "file-search-footer-action-rail",
                file_search_hints,
            );
        let footer = self.main_window_footer_slot(gpui_footer);

        let root = crate::components::main_view_chrome::render_main_view_shell()
            .font_family(self.theme_font_family())
            .key_context("FileSearchView")
            .track_focus(&self.focus_handle)
            .on_key_down(handle_key);

        let main = if is_mini {
            div()
                .w_full()
                .h_full()
                .min_h(px(0.))
                .overflow_hidden()
                .child(list_pane)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .h_full()
                .min_h(px(0.))
                .w_full()
                .overflow_hidden()
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(list_pane),
                )
                .child(
                    div()
                        .flex_1()
                        .h_full()
                        .min_h(px(0.))
                        .overflow_hidden()
                        .child(preview_pane),
                )
                .into_any_element()
        };

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            root,
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header,
                divider,
                main,
                footer,
                overlays: Vec::new(),
            },
        )
    }
}

#[cfg(test)]
mod file_search_thumbnail_tests {
    use super::*;
    use image::{Rgba, RgbaImage};
    use tempfile::tempdir;

    #[test]
    fn file_search_preview_path_contract_is_width_constrained_nowrap_with_ellipsis() {
        let truncation = file_search_preview_path_truncation();

        assert!(truncation.constrain_width);
        assert!(truncation.hide_overflow);
        assert!(truncation.ellipsis);
        assert!(truncation.nowrap);
    }

    #[test]
    fn test_file_search_thumbnail_display_size_scales_longest_side_when_over_limit() {
        let (width, height) = file_search_thumbnail_display_size(4000, 1000, 280.0);
        assert!((width - 280.0).abs() < f32::EPSILON);
        assert!((height - 70.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_file_search_thumbnail_is_decodable_extension_matches_supported_decoder_inputs() {
        assert!(file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.png"
        ));
        assert!(file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.JPG"
        ));
        assert!(file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.webp"
        ));
        assert!(file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.tiff"
        ));
        assert!(file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.ico"
        ));
        assert!(!file_search_thumbnail_is_decodable_extension(
            "/tmp/sample.svg"
        ));
    }

    #[test]
    fn test_load_file_search_thumbnail_preview_returns_file_too_large_when_size_exceeds_limit() {
        let temp_dir = tempdir().expect("tempdir should be created");
        let image_path = temp_dir.path().join("too-big.png");
        std::fs::write(&image_path, vec![0_u8; 128]).expect("image bytes should be written");

        let result = load_file_search_thumbnail_preview(
            image_path
                .to_str()
                .expect("temp image path should be valid utf-8"),
            32,
            FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_DIMENSION,
        );

        match result {
            Err(FileSearchThumbnailLoadFailure::FileTooLarge { size_bytes }) => {
                assert_eq!(size_bytes, 128);
            }
            other => panic!("Expected FileTooLarge error, got {:?}", other),
        }
    }

    #[test]
    fn test_load_file_search_thumbnail_preview_returns_resolution_too_large_when_dimension_exceeds_limit(
    ) {
        let temp_dir = tempdir().expect("tempdir should be created");
        let image_path = temp_dir.path().join("oversized.png");
        let img = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        img.save(&image_path).expect("test image should be written");

        let result = load_file_search_thumbnail_preview(
            image_path
                .to_str()
                .expect("temp image path should be valid utf-8"),
            FILE_SEARCH_PREVIEW_THUMBNAIL_MAX_BYTES,
            1,
        );

        match result {
            Err(FileSearchThumbnailLoadFailure::ResolutionTooLarge {
                width,
                height,
                max_dimension,
            }) => {
                assert_eq!(width, 2);
                assert_eq!(height, 2);
                assert_eq!(max_dimension, 1);
            }
            other => panic!("Expected ResolutionTooLarge error, got {:?}", other),
        }
    }
}

#[cfg(test)]
mod file_search_chrome_audit {
    fn production_source() -> &'static str {
        include_str!("file_search.rs")
            .split("#[cfg(test)]")
            .next()
            .expect("production source should exist")
    }

    #[test]
    fn file_search_has_no_hardcoded_alpha_fills() {
        let source = production_source();
        assert!(
            !source.contains("| 0x24)"),
            "file_search should not contain hardcoded alpha fill 0x24"
        );
        assert!(
            !source.contains("| 0x40)"),
            "file_search should not contain hardcoded alpha fill 0x40"
        );
    }

    #[test]
    fn file_search_header_has_no_feature_local_count_adornment() {
        let source = production_source();

        assert!(
            !source.contains("filtered_len.to_string()")
                && !source.contains("files_label")
                && !source.contains(".child(if is_mini"),
            "file_search header should not add feature-local count/adornment text beside MainMenuInput"
        );
    }

    #[test]
    fn file_search_uses_shared_list_item_rows() {
        let source = production_source();
        assert!(
            source.contains("ListItem::new(file.name.clone(), list_colors)"),
            "file_search must render rows through the shared ListItem component"
        );
        assert!(
            source.contains("LIST_ITEM_HEIGHT"),
            "file_search uniform_list fallback rows must use the shared list item height"
        );
        assert!(
            source.contains("let row_height = LIST_ITEM_HEIGHT")
                && source.contains(".h(px(row_height))")
                && source.contains("div().id(ix).h(px(LIST_ITEM_HEIGHT))"),
            "file_search skeleton and fallback rows must both derive height from LIST_ITEM_HEIGHT"
        );
        assert!(
            source.contains("row_description_text_rgba"),
            "file_search trailing metadata should derive text color from shared list_item helpers"
        );
        assert!(
            !source.contains("h(px(52.)") && !source.contains("h(px(52.0)"),
            "file_search must not hardcode taller 52px rows"
        );
    }
}

#[cfg(test)]
mod file_search_list_pane_paint_tests {
    use super::*;

    const PANE_SELECTOR: &str = "file-search-list-pane-inset-test";
    const CONTENT_SELECTOR: &str = "file-search-list-pane-content-test";

    struct TestFileSearchListPaneInset;

    impl gpui::Render for TestFileSearchListPaneInset {
        fn render(
            &mut self,
            _window: &mut gpui::Window,
            _cx: &mut gpui::Context<Self>,
        ) -> impl gpui::IntoElement {
            let spacing = crate::designs::DesignSpacing::default();
            div()
                .debug_selector(|| PANE_SELECTOR.to_string())
                .w_full()
                .h_full()
                .pt(px(file_search_list_pane_top_inset(spacing)))
                .child(
                    div()
                        .debug_selector(|| CONTENT_SELECTOR.to_string())
                        .w_full()
                        .h(px(32.0)),
                )
        }
    }

    #[gpui::test]
    fn file_search_list_pane_top_inset_tracks_padding_xs(cx: &mut gpui::TestAppContext) {
        use gpui::px;

        let window = cx.update(|cx| {
            let mut options = gpui::WindowOptions::default();
            options.window_bounds = Some(gpui::WindowBounds::Windowed(gpui::Bounds::new(
                gpui::point(px(0.0), px(0.0)),
                gpui::size(px(480.0), px(120.0)),
            )));
            cx.open_window(options, |_, cx| cx.new(|_| TestFileSearchListPaneInset))
                .expect("file-search inset paint test window should open")
        });

        cx.run_until_parked();

        window
            .update(cx, |_, window, _| {
                let pane = window
                    .debug_bounds()
                    .get(PANE_SELECTOR)
                    .copied()
                    .expect("file search list pane should paint");
                let content = window
                    .debug_bounds()
                    .get(CONTENT_SELECTOR)
                    .copied()
                    .expect("file search list pane content should paint");
                let spacing = crate::designs::DesignSpacing::default();

                assert_eq!(
                    content.origin.y - pane.origin.y,
                    px(spacing.padding_xs),
                    "the painted top inset must derive from DesignSpacing.padding_xs"
                );
            })
            .expect("file-search inset paint test window should remain available");
    }
}
