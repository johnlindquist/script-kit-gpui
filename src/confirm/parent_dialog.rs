use std::rc::Rc;
use std::sync::Mutex;

use gpui::{px, AnyView, App, AppContext as _, Pixels, SharedString, WeakEntity, Window};
use gpui_component::button::ButtonVariant;

use super::window::{
    open_confirm_popup_window, ConfirmPopupParentWindow, ConfirmWindowOptions, ParentDialogResult,
};
use crate::components::confirm_modal_shell::PARENT_MODAL_WIDTH_PX;

type ConfirmCallback = Rc<dyn Fn(&mut Window, &mut App)>;

/// Routing hook for in-window confirm. The binary registers a closure during
/// startup that knows how to push `AppView::ConfirmPrompt` onto the
/// `ScriptListApp` entity. Returns `true` when it accepted the route.
type InWindowRouter = Box<
    dyn Fn(AnyView, ParentConfirmOptions, async_channel::Sender<bool>, &mut App) -> bool
        + Send
        + Sync,
>;

static IN_WINDOW_ROUTER: Mutex<Option<InWindowRouter>> = Mutex::new(None);

#[allow(dead_code)]
pub(crate) fn register_in_window_router(router: InWindowRouter) {
    if let Ok(mut guard) = IN_WINDOW_ROUTER.lock() {
        *guard = Some(router);
        tracing::info!(
            target: "script_kit::confirm",
            event = "in_window_confirm_router_registered",
            "Registered in-window confirm router"
        );
    }
}

#[derive(Clone)]
pub(crate) struct ParentConfirmOptions {
    pub title: SharedString,
    pub body: SharedString,
    pub confirm_text: SharedString,
    pub cancel_text: SharedString,
    pub confirm_variant: ButtonVariant,
    pub width: Pixels,
}

#[derive(Clone, Debug)]
pub(crate) struct ParentActionDialogOptions {
    pub title: SharedString,
    pub body: SharedString,
    pub primary_text: SharedString,
    pub secondary_text: Option<SharedString>,
    pub dismiss_text: SharedString,
    pub width: Pixels,
}

impl Default for ParentActionDialogOptions {
    fn default() -> Self {
        Self {
            title: "Action required".into(),
            body: "".into(),
            primary_text: "Continue".into(),
            secondary_text: None,
            dismiss_text: "Back".into(),
            width: px(PARENT_MODAL_WIDTH_PX),
        }
    }
}

impl std::fmt::Debug for ParentConfirmOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ParentConfirmOptions")
            .field("title", &self.title)
            .field("confirm_text", &self.confirm_text)
            .field("cancel_text", &self.cancel_text)
            .field(
                "is_danger",
                &matches!(self.confirm_variant, ButtonVariant::Danger),
            )
            .finish()
    }
}

impl Default for ParentConfirmOptions {
    fn default() -> Self {
        Self {
            title: "Confirm".into(),
            body: "".into(),
            confirm_text: "OK".into(),
            cancel_text: "Cancel".into(),
            confirm_variant: ButtonVariant::Primary,
            width: px(PARENT_MODAL_WIDTH_PX),
        }
    }
}

// ── Design-contract resolvers for the in-window confirm surface ────────────
// Shared between `render_confirm_prompt` (render_prompts/other.rs) and the
// token exporter (src/design_contract) so the two can never drift. The
// legacy popup-window path (src/confirm/window.rs) is a different surface
// and deliberately does not use these.

/// Title font size in the in-window confirm surface.
pub(crate) const CONFIRM_PROMPT_TITLE_FONT_SIZE_PX: f32 = 20.0;
/// Body font size in the in-window confirm surface.
pub(crate) const CONFIRM_PROMPT_BODY_FONT_SIZE_PX: f32 = 14.0;
/// Reading-width cap for the body copy.
pub(crate) const CONFIRM_PROMPT_BODY_MAX_WIDTH_PX: f32 = 560.0;

/// The implicit GPUI line height: `TextStyle::default()` uses `phi()` and
/// `line_height_in_pixels` rounds (vendor/gpui/src/style.rs). The confirm
/// renderer never overrides it, so 14pt body → 23, 20pt title → 32.
/// Pixel-validated against a live capture on 2026-07-11 (body line spacing
/// measured at exactly 23.0 logical pt).
pub(crate) fn confirm_prompt_line_height_px(font_size: f32) -> f32 {
    const PHI: f32 = 1.618_034;
    (font_size * PHI).round()
}

/// Layout inputs `render_confirm_prompt` actually consumes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ConfirmPromptMetrics {
    /// Padding on the centered content region (`design_spacing.padding_xl`).
    pub content_padding: f32,
    /// Gap between title and body (`design_spacing.padding_md`). The protocol
    /// layout model claims 16; the renderer truth is this field (12 under the
    /// Default design) — recorded as `confirmGap.rendererSpacingVsLayoutOracle`.
    pub stack_gap: f32,
    pub title_font_size: f32,
    pub title_line_height: f32,
    pub body_font_size: f32,
    pub body_line_height: f32,
    pub body_max_width: f32,
    /// Height of the in-shell native-footer spacer (the shared footer rail
    /// height). The confirm shell is a fixed `STANDARD_HEIGHT` column, so the
    /// centered region is `STANDARD_HEIGHT - footer_spacer_height` tall
    /// starting below the main context header.
    pub footer_spacer_height: f32,
}

pub(crate) fn resolved_confirm_prompt_metrics(
    spacing: crate::designs::DesignSpacing,
    footer_rail_height: f32,
) -> ConfirmPromptMetrics {
    ConfirmPromptMetrics {
        content_padding: spacing.padding_xl,
        stack_gap: spacing.padding_md,
        title_font_size: CONFIRM_PROMPT_TITLE_FONT_SIZE_PX,
        title_line_height: confirm_prompt_line_height_px(CONFIRM_PROMPT_TITLE_FONT_SIZE_PX),
        body_font_size: CONFIRM_PROMPT_BODY_FONT_SIZE_PX,
        body_line_height: confirm_prompt_line_height_px(CONFIRM_PROMPT_BODY_FONT_SIZE_PX),
        body_max_width: CONFIRM_PROMPT_BODY_MAX_WIDTH_PX,
        footer_spacer_height: footer_rail_height,
    }
}

/// What the in-window confirm surface paints for its two text blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedConfirmPromptColors {
    pub title_rgba: u32,
    pub body_rgba: u32,
}

pub(crate) fn resolved_confirm_prompt_colors(
    theme: &crate::theme::Theme,
    is_danger: bool,
) -> ResolvedConfirmPromptColors {
    ResolvedConfirmPromptColors {
        title_rgba: ((if is_danger {
            theme.colors.ui.error
        } else {
            theme.colors.text.primary
        }) << 8)
            | 0xFF,
        body_rgba: (theme.colors.text.secondary << 8) | 0xFF,
    }
}

impl ParentConfirmOptions {
    #[allow(dead_code)]
    pub(crate) fn destructive(
        title: impl Into<SharedString>,
        body: impl Into<SharedString>,
        confirm_text: impl Into<SharedString>,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            confirm_text: confirm_text.into(),
            cancel_text: "Cancel".into(),
            confirm_variant: ButtonVariant::Danger,
            width: px(PARENT_MODAL_WIDTH_PX),
        }
    }
}

// Used by include!() code in app_actions/handle_action/scripts.rs — clippy
// cannot trace through include!() and reports a false-positive dead_code lint.
#[allow(dead_code)]
pub(crate) fn open_parent_confirm_dialog(
    window: &mut Window,
    cx: &mut App,
    options: ParentConfirmOptions,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) {
    open_parent_confirm_dialog_with_lifecycle(window, cx, options, || true, on_confirm, on_cancel);
}

/// Open a confirm popup whose parent window is identified explicitly by
/// automation id (e.g. `"notes"`). Use this for non-main parent windows so the
/// popup is pinned and AppKit-attached to the *intended* window even when
/// focus has drifted.
#[allow(dead_code)]
pub(crate) fn open_parent_confirm_dialog_for_automation_parent(
    window: &mut Window,
    cx: &mut App,
    parent_automation_id: impl Into<String>,
    options: ParentConfirmOptions,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) {
    open_parent_confirm_dialog_with_lifecycle_and_parent(
        window,
        cx,
        options,
        Some(parent_automation_id.into()),
        || true,
        Rc::new(on_confirm),
        None,
        None,
        Rc::new(on_cancel),
    );
}

#[allow(dead_code)]
pub(crate) fn open_parent_confirm_dialog_for_entity<T: 'static>(
    window: &mut Window,
    cx: &mut App,
    owner: WeakEntity<T>,
    options: ParentConfirmOptions,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) {
    let owner_for_lifecycle = owner.clone();
    open_parent_confirm_dialog_with_lifecycle(
        window,
        cx,
        options,
        move || owner_for_lifecycle.upgrade().is_some(),
        on_confirm,
        on_cancel,
    );
}

pub(crate) fn open_parent_confirm_dialog_with_lifecycle(
    window: &mut Window,
    cx: &mut App,
    options: ParentConfirmOptions,
    keep_open_while: impl Fn() -> bool + 'static,
    on_confirm: impl Fn(&mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&mut Window, &mut App) + 'static,
) {
    open_parent_confirm_dialog_with_lifecycle_and_parent(
        window,
        cx,
        options,
        None,
        keep_open_while,
        Rc::new(on_confirm),
        None,
        None,
        Rc::new(on_cancel),
    );
}

pub(crate) fn open_parent_action_dialog(
    window: &mut Window,
    cx: &mut App,
    options: ParentActionDialogOptions,
    on_primary: impl Fn(&mut Window, &mut App) + 'static,
    on_secondary: impl Fn(&mut Window, &mut App) + 'static,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
) {
    let secondary_text = options.secondary_text;
    let popup = ParentConfirmOptions {
        title: options.title,
        body: options.body,
        confirm_text: options.primary_text,
        cancel_text: options.dismiss_text,
        confirm_variant: ButtonVariant::Primary,
        width: options.width,
    };
    open_parent_confirm_dialog_with_lifecycle_and_parent(
        window,
        cx,
        popup,
        None,
        || true,
        Rc::new(on_primary),
        secondary_text.clone(),
        secondary_text.map(|_| Rc::new(on_secondary) as ConfirmCallback),
        Rc::new(on_dismiss),
    );
}

fn open_parent_confirm_dialog_with_lifecycle_and_parent(
    window: &mut Window,
    cx: &mut App,
    options: ParentConfirmOptions,
    explicit_parent_automation_id: Option<String>,
    keep_open_while: impl Fn() -> bool + 'static,
    on_confirm: ConfirmCallback,
    secondary_text: Option<SharedString>,
    on_secondary: Option<ConfirmCallback>,
    on_cancel: ConfirmCallback,
) {
    let has_lifecycle_predicate = true;
    tracing::info!(
        event = "parent_confirm_dialog_opened",
        title = %options.title,
        has_lifecycle_predicate,
        "parent_confirm_dialog_opened"
    );

    let width_value: f32 = options.width.into();
    tracing::info!(
        event = "parent_confirm_dialog_building",
        title = %options.title,
        width = width_value,
        "parent_confirm_dialog_building"
    );

    window.activate_window();

    let keep_open_while: Rc<dyn Fn() -> bool> = Rc::new(keep_open_while);
    let parent_bounds = window.bounds();
    let display_id = window.display(cx).map(|d| d.id());
    let parent_window_handle = window.window_handle();

    let (result_tx, result_rx) = async_channel::bounded::<ParentDialogResult>(1);

    let on_confirm_for_task = on_confirm.clone();
    let on_cancel_for_task = on_cancel.clone();
    let on_secondary_for_task = on_secondary.clone();

    tracing::info!(
        target: "script_kit::confirm",
        event = "parent_confirm_spawning_result_task",
        "Spawning async task to await confirm popup result"
    );

    cx.spawn(async move |cx: &mut gpui::AsyncApp| {
        tracing::info!(
            target: "script_kit::confirm",
            event = "parent_confirm_result_task_waiting",
            "Result task: waiting for confirm popup result..."
        );

        let result = result_rx
            .recv()
            .await
            .unwrap_or(ParentDialogResult::Dismiss);

        tracing::info!(
            target: "script_kit::confirm",
            event = "parent_confirm_result_received",
            result = ?result,
            "Result task: received result from confirm popup"
        );

        let update_result = cx.update_window(parent_window_handle, move |_, parent_window, cx| {
            tracing::info!(
                target: "script_kit::confirm",
                event = "parent_confirm_activating_parent",
                result = ?result,
                "Result task: re-activating parent window and calling callback"
            );

            parent_window.activate_window();

            match result {
                ParentDialogResult::Primary => on_confirm_for_task(parent_window, cx),
                ParentDialogResult::Secondary => {
                    if let Some(callback) = on_secondary_for_task {
                        callback(parent_window, cx);
                    } else {
                        on_cancel_for_task(parent_window, cx);
                    }
                }
                ParentDialogResult::Dismiss => on_cancel_for_task(parent_window, cx),
                ParentDialogResult::ProgrammaticClose => {}
            }
        });

        if let Err(error) = update_result {
            tracing::error!(
                target: "script_kit::confirm",
                event = "parent_confirm_update_window_failed",
                result = ?result,
                error = ?error,
                "Result task: failed to update parent window"
            );
        }
    })
    .detach();

    let popup_options = ConfirmWindowOptions {
        title: options.title,
        body: options.body,
        confirm_text: options.confirm_text,
        cancel_text: options.cancel_text,
        secondary_text,
        confirm_variant: options.confirm_variant,
        width: options.width,
    };

    let parent_automation_id =
        explicit_parent_automation_id.or_else(crate::windows::focused_automation_window_id);
    match open_confirm_popup_window(
        cx,
        ConfirmPopupParentWindow {
            handle: parent_window_handle,
            bounds: parent_bounds,
            display_id,
            automation_id: parent_automation_id,
        },
        popup_options,
        keep_open_while,
        result_tx,
    ) {
        Ok(_handle) => {
            tracing::info!(
                target: "script_kit::confirm",
                event = "parent_confirm_popup_opened_ok",
                "Confirm popup window opened successfully"
            );
        }
        Err(error) => {
            tracing::error!(
                target: "script_kit::confirm",
                event = "parent_confirm_dialog_open_failed",
                error = ?error,
                "parent_confirm_dialog_open_failed"
            );
        }
    }
}

/// Open a native confirmation popup centered over the caller window and return
/// whether the user confirmed. Uses the global main window handle from async
/// contexts that only have `&mut AsyncApp`.
///
/// Returns `Ok(true)` if the user confirms, `Ok(false)` if they cancel, close,
/// or the dialog cannot be opened.
#[allow(dead_code)]
pub(crate) async fn confirm_with_parent_dialog(
    cx: &mut gpui::AsyncApp,
    options: ParentConfirmOptions,
    trace_id: &str,
) -> anyhow::Result<bool> {
    tracing::info!(
        category = "UI",
        trace_id = %trace_id,
        event = "confirm_modal_open",
        title = %options.title,
        "Opening confirmation modal"
    );

    let (confirm_tx, confirm_rx) = async_channel::bounded::<bool>(1);

    let window_handle = crate::get_main_window_handle()
        .ok_or_else(|| anyhow::anyhow!("Main window handle not available"))?;

    let sender_ok = confirm_tx.clone();
    let sender_cancel = confirm_tx.clone();

    cx.update_window(window_handle, move |_any_view, window, cx| {
        tracing::info!(
            target: "script_kit::confirm",
            event = "confirm_route_decision",
            "Opening native parent-attached confirm popup"
        );

        open_parent_confirm_dialog(
            window,
            cx,
            options,
            move |_window, _cx| {
                let _ = sender_ok.try_send(true);
            },
            move |_window, _cx| {
                let _ = sender_cancel.try_send(false);
            },
        );
    })?;

    let confirmed = confirm_rx.recv().await.unwrap_or(false);
    tracing::info!(
        category = "UI",
        trace_id = %trace_id,
        event = "confirm_modal_result",
        confirmed,
        "Confirmation modal resolved"
    );
    Ok(confirmed)
}

#[cfg(test)]
mod tests {
    use std::fs;

    fn normalize_ws(source: &str) -> String {
        source.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn parent_confirm_dialog_activates_window_before_opening_native_popup() {
        let source = fs::read_to_string("src/confirm/parent_dialog.rs")
            .expect("Failed to read src/confirm/parent_dialog.rs");
        let normalized = normalize_ws(&source);

        let activate_idx = normalized
            .find("window.activate_window();")
            .expect("parent confirm dialog should activate the parent window");
        let open_idx = normalized
            .find("open_confirm_popup_window(")
            .expect("parent confirm dialog should open the native confirm popup window");

        assert!(
            activate_idx < open_idx,
            "parent confirm dialog should activate the window before opening the native popup"
        );
    }

    #[test]
    fn parent_confirm_dialog_no_longer_uses_window_open_dialog() {
        let source = fs::read_to_string("src/confirm/parent_dialog.rs")
            .expect("Failed to read src/confirm/parent_dialog.rs");
        // Only scan production code — strip everything after #[cfg(test)]
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(&source);
        let normalized = normalize_ws(production_source);

        let legacy_marker = ["window", ".open_dialog("].concat();
        assert!(
            !normalized.contains(&legacy_marker),
            "parent confirm dialog should no longer use in-window open_dialog"
        );
    }

    #[test]
    fn async_confirm_route_no_longer_uses_in_window_confirm_prompt_router() {
        let source = fs::read_to_string("src/confirm/parent_dialog.rs")
            .expect("Failed to read src/confirm/parent_dialog.rs");
        let production_source = source.split("#[cfg(test)]").next().unwrap_or(&source);
        let function_body = production_source
            .split("pub(crate) async fn confirm_with_parent_dialog")
            .nth(1)
            .and_then(|section| section.split("let confirmed = confirm_rx.recv()").next())
            .expect("expected confirm_with_parent_dialog body");
        let normalized = normalize_ws(function_body);

        assert!(
            !normalized.contains("crate::is_main_window_visible()"),
            "async confirms should no longer choose AppView::ConfirmPrompt based on main-window visibility"
        );
        assert!(
            !normalized.contains("IN_WINDOW_ROUTER.lock()"),
            "async confirms should no longer route through the in-window ConfirmPrompt router"
        );
        assert!(
            normalized.contains("open_parent_confirm_dialog("),
            "async confirms should open the shared native parent confirm popup"
        );
    }

    #[test]
    fn parent_confirm_dialog_passes_parent_bounds_to_popup_window() {
        let source = fs::read_to_string("src/confirm/parent_dialog.rs")
            .expect("Failed to read src/confirm/parent_dialog.rs");
        let normalized = normalize_ws(&source);

        assert!(
            normalized.contains("let parent_bounds = window.bounds();"),
            "parent confirm dialog should capture parent bounds for centering"
        );
        assert!(
            normalized.contains("open_confirm_popup_window("),
            "parent confirm dialog should open the native popup window"
        );
    }

    #[test]
    fn async_confirmation_callers_use_shared_parent_dialog_helper() {
        let clipboard_source = fs::read_to_string("src/app_actions/handle_action/clipboard.rs")
            .expect("Failed to read clipboard.rs");
        let chat_source = fs::read_to_string("src/app_impl/chat_actions.rs")
            .expect("Failed to read chat_actions.rs");
        let execution_source = fs::read_to_string("src/app_impl/execution_paths.rs")
            .expect("Failed to read execution_paths.rs");
        let builtin_source = fs::read_to_string("src/app_execute/builtin_execution.rs")
            .expect("Failed to read builtin_execution.rs");

        let clipboard = normalize_ws(&clipboard_source);
        let chat = normalize_ws(&chat_source);
        let execution = normalize_ws(&execution_source);
        let builtin = normalize_ws(&builtin_source);

        assert!(
            clipboard.contains("crate::confirm::confirm_with_parent_dialog(")
                && !clipboard.contains("confirm_with_modal("),
            "clipboard.rs should call confirm_with_parent_dialog directly, not confirm_with_modal"
        );

        assert!(
            chat.contains("crate::confirm::confirm_with_parent_dialog(")
                && chat.contains("ParentConfirmOptions::destructive(")
                && chat.contains("\"Clear Conversation\"")
                && chat.contains("\"Clear\"")
                && !chat.contains("async_channel::bounded::<bool>(1)"),
            "chat_actions should use the shared destructive async confirm helper"
        );

        assert!(
            execution.contains("crate::confirm::confirm_with_parent_dialog(")
                && execution.contains("ParentConfirmOptions::destructive(")
                && execution.contains("\"Move to Trash\"")
                && !execution.contains("async_channel::bounded::<bool>(1)"),
            "execution_paths should use the shared destructive async confirm helper"
        );

        assert!(
            builtin.contains("crate::confirm::confirm_with_parent_dialog(")
                && !builtin.contains("confirm_with_modal("),
            "builtin_execution.rs should call confirm_with_parent_dialog directly, not confirm_with_modal"
        );
    }

    #[test]
    fn parent_confirm_dialog_logs_build_width_before_render() {
        let source = fs::read_to_string("src/confirm/parent_dialog.rs")
            .expect("Failed to read src/confirm/parent_dialog.rs");
        let normalized = normalize_ws(&source);

        assert!(
            normalized.contains("event = \"parent_confirm_dialog_building\"")
                && normalized.contains("width = width_value,"),
            "parent confirm dialog should log the concrete width used to build the dialog"
        );
    }

    fn extract_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        source
            .split(start)
            .nth(1)
            .and_then(|section| section.split(end).next())
            .expect("expected section to exist")
    }

    #[test]
    fn script_list_confirm_actions_bind_dialog_lifecycle_to_script_list_entity() {
        let source = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
            .expect("Failed to read scripts.rs");

        let remove_branch = normalize_ws(extract_section(
            &source,
            "\"remove_script\" | \"delete_script\" => {",
            "\"reload_scripts\" => {",
        ));
        assert!(
            remove_branch.contains("let weak_entity = cx.entity().downgrade();")
                && remove_branch.contains("crate::confirm::open_parent_confirm_dialog_for_entity(")
                && remove_branch.contains("this.refresh_scripts(cx);")
                && !remove_branch.contains("crate::confirm::open_parent_confirm_dialog("),
            "script removal should use the entity-owned parent confirm helper and keep refreshing the list after confirmation"
        );

        let quit_branch = normalize_ws(extract_section(
            &source,
            "\"quit\" => {",
            "\"copy_content\" => {",
        ));
        assert!(
            quit_branch.contains("let owner = cx.entity().downgrade();")
                && quit_branch.contains("crate::confirm::open_parent_confirm_dialog_for_entity(")
                && quit_branch.contains("Self::quit_script_kit_confirm_options()")
                && quit_branch.contains("Self::prepare_script_kit_shutdown();")
                && !quit_branch.contains("crate::confirm::open_parent_confirm_dialog("),
            "quit should use the entity-owned parent confirm helper while preserving the shared copy and shutdown cleanup"
        );
    }

    // NOTE: prompt_handler's SDK confirm() route no longer uses
    // confirm_with_parent_dialog — 192d7d647 ("Route SDK confirm through
    // shared prompt") intentionally migrated it onto the shared in-window
    // ConfirmPrompt surface via Self::open_confirm_prompt. That contract is
    // locked by tests/source_audits/confirm_modal_shared_shell.rs::
    // sdk_confirm_host_route_uses_shared_confirm_prompt_surface, so the old
    // prompt_handler_confirm_uses_shared_async_confirm_helper audit here was
    // removed instead of being rewritten into a duplicate.

    #[test]
    fn quit_action_and_builtin_quit_share_shutdown_cleanup() {
        let scripts_source = fs::read_to_string("src/app_actions/handle_action/scripts.rs")
            .expect("Failed to read scripts.rs");
        let builtin_source = fs::read_to_string("src/app_execute/builtin_execution.rs")
            .expect("Failed to read builtin_execution.rs");

        let scripts = normalize_ws(&scripts_source);
        let builtins = normalize_ws(&builtin_source);

        assert!(
            scripts.contains("Self::quit_script_kit_confirm_options()"),
            "direct quit action should keep using the shared quit confirm copy"
        );

        let helper_idx = builtins
            .find("fn prepare_script_kit_shutdown()")
            .expect("Expected shared quit shutdown helper");
        let kill_idx = builtins
            .find("PROCESS_MANAGER.kill_all_processes();")
            .expect("Expected helper to stop running processes");
        let remove_pid_idx = builtins
            .find("PROCESS_MANAGER.remove_main_pid();")
            .expect("Expected helper to clear the main pid");

        assert!(
            helper_idx < kill_idx && kill_idx < remove_pid_idx,
            "shared quit shutdown helper should stop processes before clearing the main pid"
        );

        let action_helper_idx = scripts
            .find("Self::prepare_script_kit_shutdown();")
            .expect("Expected direct quit action to use the shared shutdown helper");
        let action_quit_idx = scripts[action_helper_idx..]
            .find("cx.quit();")
            .map(|idx| action_helper_idx + idx)
            .expect("Expected direct quit action to quit after shutdown cleanup");

        assert!(
            action_helper_idx < action_quit_idx,
            "direct quit action should run shutdown cleanup before quitting"
        );

        let builtin_branch_idx = builtins
            .find("SystemActionType::QuitScriptKit => {")
            .expect("Expected builtin quit system action branch");
        let builtin_helper_idx = builtins[builtin_branch_idx..]
            .find("Self::prepare_script_kit_shutdown();")
            .map(|idx| builtin_branch_idx + idx)
            .expect("Expected builtin quit to use the shared shutdown helper");
        let builtin_quit_idx = builtins[builtin_helper_idx..]
            .find("cx.quit();")
            .map(|idx| builtin_helper_idx + idx)
            .expect("Expected builtin quit to call cx.quit() after cleanup");

        assert!(
            builtin_helper_idx < builtin_quit_idx,
            "builtin quit should run shutdown cleanup before quitting"
        );
    }
}
