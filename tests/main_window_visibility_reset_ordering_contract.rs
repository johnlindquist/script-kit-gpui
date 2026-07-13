//! Source-level contract for main-window show/hide stale-view flash ordering.
//!
//! The main panel hide is deferred to avoid GPUI/AppKit re-entrancy. While that
//! deferred hide is pending, the app must not switch the visible route to
//! ScriptList; otherwise users can see a one-frame main-menu flash on close.

const LIFECYCLE_RESET: &str = include_str!("../src/app_impl/lifecycle_reset.rs");
const WINDOW_VISIBILITY: &str = include_str!("../src/main_sections/window_visibility.rs");
const POSITIONING: &str = include_str!("../src/platform/positioning.rs");
const RENDER_IMPL: &str = include_str!("../src/main_sections/render_impl.rs");
const RENDER_SCRIPT_LIST: &str = include_str!("../src/render_script_list/mod.rs");

fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("missing function signature: {signature}"));
    let after_start = &source[start..];
    let open = after_start
        .find('{')
        .unwrap_or_else(|| panic!("missing function body for: {signature}"));
    let mut depth = 0usize;
    for (offset, ch) in after_start[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &after_start[..open + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated function body: {signature}");
}

#[test]
fn close_path_hides_before_deferred_script_list_reset() {
    let preparation = function_body(LIFECYCLE_RESET, "fn prepare_main_window_close");
    let close = function_body(LIFECYCLE_RESET, "pub(crate) fn close_and_reset_window");

    let hidden = preparation
        .find("script_kit_gpui::set_main_window_visible(false)")
        .expect("close path must mark main window logically hidden");
    let automation_hidden = preparation
        .find("crate::windows::set_automation_visibility(\"main\", false)")
        .expect("close path must mark automation hidden");
    let prepared = close
        .find(".prepare_main_window_close(cx, \"close_and_reset_window\", true, true)")
        .expect("close path must run shared preparation before native hide");
    let hide = close
        .find("platform::defer_hide_main_window(cx);")
        .expect("close path must enqueue main-panel-only hide");
    let deferred_reset = close
        .find("self.defer_reset_to_script_list_after_main_window_hidden")
        .expect("close path must schedule hidden reset after native hide is enqueued");

    assert!(
        prepared < hide,
        "shared close preparation must complete before native hide is enqueued"
    );
    assert!(
        automation_hidden < preparation.len(),
        "shared close preparation must mark automation hidden"
    );
    assert!(
        hide < deferred_reset,
        "hidden ScriptList reset must be scheduled after native hide is enqueued"
    );
    assert!(
        !preparation[hidden..].contains("reset_to_script_list(cx);"),
        "shared close preparation must not reset to ScriptList while the native window can still be visible"
    );
    assert!(
        !preparation.contains("self.cancel_script_execution(cx);"),
        "close path must cancel scripts without using the reset-owning cancellation helper"
    );
    assert!(
        !preparation.contains("update_automation_semantic_surface(\"main\", Some(\"scriptList\""),
        "close path must not rekey automation to ScriptList before hidden reset"
    );
}

#[test]
fn hidden_reset_rekeys_after_reset() {
    let body = function_body(
        LIFECYCLE_RESET,
        "pub(crate) fn reset_hidden_main_window_to_script_list",
    );
    let reset = body
        .find("self.reset_to_script_list(cx);")
        .expect("hidden reset helper must reset to ScriptList");
    let rekey = body
        .find("self.rekey_main_automation_surface_from_current_view();")
        .expect("hidden reset helper must rekey automation from the current view");
    let hidden = body
        .find("crate::windows::set_automation_visibility(\"main\", false)")
        .expect("hidden reset helper must keep automation hidden");

    assert!(
        reset < rekey,
        "automation rekey must follow the real view reset"
    );
    assert!(
        rekey < hidden,
        "hidden reset helper must preserve hidden automation after rekeying"
    );
}

#[test]
fn hide_helper_defers_script_list_reset_until_after_native_hide() {
    let body = function_body(WINDOW_VISIBILITY, "fn hide_main_window_helper");
    let hidden = body
        .find("set_main_window_visible(false)")
        .expect("hide helper must mark main window logically hidden");
    let hide = body
        .find("platform::defer_hide_main_window(cx);")
        .expect("hide helper must enqueue main-panel-only hide");
    let deferred_reset = body
        .find("view.defer_reset_to_script_list_after_main_window_hidden")
        .expect("hide helper must schedule hidden reset after native hide is enqueued");

    assert!(
        hidden < hide,
        "logical hidden state must precede native hide"
    );
    assert!(
        hide < deferred_reset,
        "hide helper must schedule the ScriptList reset after native hide is enqueued"
    );
    assert!(
        !body[hidden..hide].contains("reset_to_script_list(ctx);"),
        "hide helper must not reset to ScriptList while the native window can still be visible"
    );
    assert!(
        !body.contains("view.cancel_script_execution(ctx);"),
        "hide helper must cancel scripts without using the reset-owning cancellation helper"
    );
    assert!(
        !body[..hide].contains("update_automation_semantic_surface(\"main\", Some(\"scriptList\""),
        "hide helper must not rekey automation to ScriptList before hidden reset"
    );
}

#[test]
fn hidden_reset_runs_next_turn_without_timer_and_skips_stale_visibility() {
    let body = function_body(
        LIFECYCLE_RESET,
        "pub(crate) fn defer_reset_to_script_list_after_main_window_hidden",
    );
    let scheduled_generation = body
        .find("let scheduled_generation = script_kit_gpui::main_window_visibility_generation();")
        .expect("hidden reset must snapshot the hide generation when scheduled");
    let spawn = body
        .find("cx.spawn(async move |this, cx|")
        .expect("hidden reset must run on the next foreground turn");
    let stale_visible = body
        .find("script_kit_gpui::is_main_window_visible()")
        .expect("hidden reset must not mutate a re-shown main window");
    let generation_check = body
        .find("!= scheduled_generation")
        .expect("hidden reset must skip stale work after a newer visibility transition");
    let reset = body
        .find("app.reset_hidden_main_window_to_script_list(cx, reason)")
        .expect("hidden reset must still reset the hidden main window");

    assert!(
        scheduled_generation < spawn,
        "hidden reset must capture the hide generation before deferring"
    );
    assert!(
        stale_visible < reset && generation_check < reset,
        "hidden reset must prove it is still hidden/current before resetting"
    );
    assert!(
        !body.contains("timer(") && !body.contains("Duration::from_millis(16)"),
        "hidden reset must not wait on an arbitrary 16ms timer; it should prepare the hidden window as soon as the hide turn has been queued"
    );
}

#[test]
fn show_path_prepares_script_list_before_visible_true() {
    let body = function_body(WINDOW_VISIBILITY, "fn show_main_window_helper");
    let reset = body
        .find("view.reset_to_script_list(ctx);")
        .expect("show helper must be able to prepare ScriptList before reveal");
    let visible = body
        .find("set_main_window_visible(true);")
        .expect("show helper must mark the main window visible");
    let native_show = body
        .find("platform::show_main_window_without_activation();")
        .expect("show helper must perform native reveal");

    assert!(
        reset < visible,
        "show helper must prepare ScriptList before visible=true"
    );
    assert!(
        visible < native_show,
        "visible=true still precedes the native reveal/focus phase"
    );
}

#[test]
fn show_path_clamps_saved_main_position_before_native_move() {
    let body = function_body(WINDOW_VISIBILITY, "fn show_main_window_helper");
    let saved_restore = body
        .find("window_state::get_main_position_for_mouse_display")
        .expect("show helper must restore saved per-display main positions");
    let restored_bounds = body
        .find("let restored_bounds = gpui::Bounds")
        .expect("show helper must rebuild saved bounds with current show size");
    let clamp = body
        .find("clamp_restored_main_window_bounds_to_visible_area")
        .expect("show helper must clamp restored saved bounds to the display visible area");
    let native_move = body
        .find("platform::move_first_window_to_bounds(&bounds);")
        .expect("show helper must move native main window to computed bounds");

    assert!(
        saved_restore < restored_bounds && restored_bounds < clamp && clamp < native_move,
        "saved main-window position must be resized and visible-area clamped before native move"
    );

    let clamp_body = function_body(
        WINDOW_VISIBILITY,
        "fn clamp_restored_main_window_bounds_to_visible_area",
    );
    assert!(
        clamp_body.contains("platform::clamp_to_visible(bounds, &candidate.visible_area)"),
        "saved main-window restores must use the selected display's visible area"
    );
}

#[test]
fn show_path_resyncs_gpui_bounds_after_native_show_move() {
    let show_body = function_body(WINDOW_VISIBILITY, "fn show_main_window_helper");
    let native_move = show_body
        .find("platform::move_first_window_to_bounds(&bounds);")
        .expect("show helper must move native window before reveal");
    let pre_reveal_resync = show_body
        .find("before_pre_reveal_bounds_changed")
        .expect("show helper must mark GPUI bounds dirty while the moved window is still hidden");
    let native_show = show_body
        .find("platform::show_main_window_without_activation_with_geometry_trace")
        .expect("show helper must perform native reveal");
    let native_back = show_body
        .find("platform::send_ai_window_to_back();")
        .expect("show helper must complete native post-show window ordering");
    let post_reveal_resync = show_body
        .rfind("resync_main_window_gpui_bounds_after_native_show_move_for_trace")
        .expect("show helper should keep a post-reveal GPUI bounds snapshot for verification");
    let automation = show_body
        .find("sync_main_automation_window(None, true, true);")
        .expect("show helper must sync automation after focus restore");

    assert!(
        native_move < pre_reveal_resync && pre_reveal_resync < native_show,
        "GPUI bounds must be marked dirty after the native move and before orderFront reveals the panel"
    );
    assert!(
        native_show < native_back && native_back < post_reveal_resync && post_reveal_resync < automation,
        "post-reveal GPUI bounds snapshot must remain after native show ordering and before automation sync"
    );

    let resync_body = function_body(
        WINDOW_VISIBILITY,
        "fn resync_main_window_gpui_bounds_after_native_show_move_with_phases",
    );
    assert!(
        resync_body.contains("win.bounds_changed(cx);"),
        "resync helper must force GPUI to reread platform bounds"
    );
}

#[test]
fn native_move_syncs_content_layer_scale_before_reveal() {
    let move_body = function_body(POSITIONING, "pub fn move_first_window_to");
    let set_frame = move_body
        .find("setFrame:new_frame display:true animate:false")
        .expect("native move must set the NSWindow frame");
    let sync_scale = move_body
        .find("sync_window_content_layer_scale(window);")
        .expect("native move must sync content layer scale after moving displays");
    let verify = move_body
        .find("Window moved: actual=")
        .expect("native move should log the verified final frame");

    assert!(
        set_frame < sync_scale && sync_scale < verify,
        "content layer scale must be synced after the native move and before the moved window can be revealed"
    );

    let sync_body = function_body(POSITIONING, "unsafe fn sync_window_content_layer_scale");
    assert!(
        sync_body.contains("backingScaleFactor")
            && sync_body.contains("contentView")
            && sync_body.contains("sync_view_layer_tree_contents_scale"),
        "scale sync must use the destination NSWindow backing scale and apply it to the content view tree"
    );
}

#[test]
fn render_paths_do_not_mutate_visibility_or_route() {
    assert!(
        !RENDER_SCRIPT_LIST.contains("self.reset_to_script_list(")
            && !RENDER_SCRIPT_LIST.contains("script_kit_gpui::set_main_window_visible(")
            && !RENDER_SCRIPT_LIST.contains("update_automation_semantic_surface("),
        "ScriptList render must not own route/visibility/automation mutations"
    );
    assert!(
        !RENDER_IMPL.contains("defer_reset_to_script_list_after_main_window_hidden")
            && !RENDER_IMPL.contains("reset_hidden_main_window_to_script_list"),
        "hidden reset ordering must stay in lifecycle helpers, not render focus-loss code"
    );
}
