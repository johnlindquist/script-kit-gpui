//! Regression tests for actions button visibility in the main script list.
//!
//! These source-audit tests ensure:
//! 1. The native footer bridge routes through canonical ScriptListApp dispatch.
//! 2. The full ScriptList footer uses the universal three-key hint strip.
//! 3. The Actions button click is gated by `has_actions()`.

#[cfg(test)]
mod tests {
    use std::fs;

    // -----------------------------------------------------------------------
    // Helper: read all .rs files from a directory into one string
    // -----------------------------------------------------------------------
    fn read_all_rs_in_dir(dir: &str) -> String {
        let mut content = String::new();
        for entry in fs::read_dir(dir).unwrap_or_else(|_| panic!("Failed to read dir {}", dir)) {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "rs") {
                content.push_str(
                    &fs::read_to_string(&path)
                        .unwrap_or_else(|_| panic!("Failed to read {}", path.display())),
                );
                content.push('\n');
            }
        }
        content
    }

    // -----------------------------------------------------------------------
    // Helpers: structural scoping (function bodies and match arms) instead of
    // fixed character windows, per the source-audit enforcement ladder.
    // -----------------------------------------------------------------------
    fn function_body<'a>(source: &'a str, signature: &str) -> &'a str {
        let start = source
            .find(signature)
            .unwrap_or_else(|| panic!("{signature} function not found"));
        let after_start = &source[start..];
        let open_brace = after_start
            .find('{')
            .unwrap_or_else(|| panic!("{signature} function body not found"));
        let body_start = start + open_brace;
        let mut depth = 0usize;
        for (offset, ch) in source[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &source[body_start..body_start + offset + 1];
                    }
                }
                _ => {}
            }
        }
        panic!("{signature} function body did not close")
    }

    /// Extract the block body of a `match` arm (`Pattern => { ... }`) from an
    /// already-scoped function body. Panics if the arm is missing or does not
    /// use a block, so assertions stay pinned to the arm they audit.
    fn match_arm_body<'a>(function_body: &'a str, arm_marker: &str) -> &'a str {
        let arm_start = function_body
            .find(arm_marker)
            .unwrap_or_else(|| panic!("{arm_marker} arm must exist"));
        let after_arm = &function_body[arm_start..];
        let open_brace = after_arm
            .find('{')
            .unwrap_or_else(|| panic!("{arm_marker} arm must use a block body"));
        let body_start = arm_start + open_brace;
        let mut depth = 0usize;
        for (offset, ch) in function_body[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &function_body[body_start..body_start + offset + 1];
                    }
                }
                _ => {}
            }
        }
        panic!("{arm_marker} arm body did not close")
    }

    // =======================================================================
    // Existing tests (preserved)
    // =======================================================================

    #[test]
    fn test_has_actions_helper_exists_and_handles_clipboard() {
        let content = read_all_rs_in_dir("src/app_actions/handle_action");

        assert!(
            content.contains("fn has_actions"),
            "Expected ScriptListApp::has_actions() helper to exist"
        );

        assert!(
            content.contains("ClipboardHistoryView"),
            "has_actions() should account for ClipboardHistoryView selection"
        );

        assert!(
            content.contains("selected_clipboard_entry"),
            "has_actions() should use selected_clipboard_entry() for clipboard history"
        );
    }

    #[test]
    fn test_footer_uses_config_driven_footer_rail() {
        let content = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        // The glass in-window footer renders the surface's resolved
        // MainWindowFooterConfig through the one shared rail renderer, so the
        // GPUI fallback cannot drift from the native footer language.
        assert!(
            content.contains("render_main_window_footer_config_rail"),
            "render_script_list footer must render the shared config-driven rail"
        );
        assert!(
            content.contains("emit_prompt_hint_audit(\"render_script_list::full\""),
            "render_script_list footer must emit a prompt hint audit"
        );
    }

    #[test]
    fn test_cmd_k_routes_through_shared_dispatcher() {
        let content = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        let cmdk_pos = content
            .find("Cmd+K")
            .expect("Cmd+K handler not found in render_script_list.rs");

        let after_cmdk = &content[cmdk_pos..content.len().min(cmdk_pos + 300)];

        assert!(
            after_cmdk.contains("handle_cmd_k_actions_toggle"),
            "Cmd+K handling should route through the shared dispatcher (handle_cmd_k_actions_toggle). Found section:\n{}",
            after_cmdk
        );
        // Must NOT directly call toggle_actions — that bypasses the shared dispatcher
        assert!(
            !after_cmdk.contains("toggle_actions(cx"),
            "Cmd+K must not directly call toggle_actions(); it should use handle_cmd_k_actions_toggle. Found section:\n{}",
            after_cmdk
        );
    }

    #[test]
    fn test_agent_hint_is_clickable_button() {
        let content = fs::read_to_string("src/components/launcher_ask_ai_hint.rs")
            .expect("Failed to read src/components/launcher_ask_ai_hint.rs");

        let hint_pos = content
            .find(".id(\"agent-hint-button\")")
            .expect("Agent hint container not found in src/components/launcher_ask_ai_hint.rs");
        let hint_section = &content[hint_pos..content.len().min(hint_pos + 1200)];

        assert!(
            hint_section.contains(".cursor_pointer()"),
            "Agent hint should be clickable (cursor_pointer). Section:\n{}",
            hint_section
        );
        assert!(
            hint_section.contains(".on_click("),
            "Agent hint should have an on_click handler. Section:\n{}",
            hint_section
        );
    }

    #[test]
    fn test_mini_mode_branch_keeps_shared_ask_ai_hint_and_skips_preview_footer() {
        let content = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");
        let render_impl = fs::read_to_string("src/main_sections/render_impl.rs")
            .expect("Failed to read src/main_sections/render_impl.rs");
        let footer_popup =
            fs::read_to_string("src/footer_popup.rs").expect("Failed to read src/footer_popup.rs");

        assert!(
            content.contains("let is_mini = self.main_window_mode == MainWindowMode::Mini;"),
            "mini mode flag should be computed from main_window_mode"
        );
        // The Agent affordance lives in the native footer (FooterAction::Ai);
        // the script list must not grow a local agent hint element again.
        assert!(
            !content.contains("agent-hint-button"),
            "Script list must not render a local agent hint — the Agent button lives in the native footer"
        );
        assert!(
            content.contains("if is_mini {")
                && content
                    .contains("// Mini mode: single column, toggle between list and info panel")
                && content.contains("mode = \"mini\""),
            "mini mode branch should render the single-column layout and mini perf log"
        );
        assert!(
            render_impl.contains("self.sync_main_footer_popup(window, cx);"),
            "render loop should sync the popup footer when mini mode visibility changes"
        );
        assert!(
            footer_popup.contains("struct FooterButtonConfig")
                && footer_popup.contains("struct MainWindowFooterConfig")
                && footer_popup.contains("FooterAction::Run")
                && footer_popup.contains("FooterAction::Actions")
                && footer_popup.contains("FooterAction::Ai"),
            "popup footer should define config types and all three canonical actions"
        );
    }

    // =======================================================================
    // NEW: Native footer bridge dispatch regression tests
    // =======================================================================

    /// Fails if the native footer bridge listener is removed from ui_window.rs
    /// or stops draining the canonical `footer_action_channel()`.
    #[test]
    fn test_native_footer_bridge_exists_and_uses_canonical_channel() {
        let content = fs::read_to_string("src/app_impl/ui_window.rs")
            .expect("Failed to read src/app_impl/ui_window.rs");

        // The bridge must exist as a method on ScriptListApp
        assert!(
            content.contains("fn ensure_main_footer_action_listener"),
            "Native footer bridge listener must exist in ui_window.rs"
        );

        // It must drain the canonical footer_action_channel (not a duplicate channel)
        assert!(
            content.contains("footer_popup::footer_action_channel()"),
            "Native footer bridge must use the canonical footer_action_channel()"
        );

        // It must delegate to handle_main_footer_action (not inline dispatch)
        assert!(
            content.contains("handle_main_footer_action(action"),
            "Native footer bridge must route through handle_main_footer_action()"
        );
    }

    /// Fails if handle_main_footer_action stops dispatching through the
    /// canonical dispatch_main_window_footer_action method or the dispatcher
    /// drops one of the native footer action contracts.
    #[test]
    fn test_native_footer_dispatches_canonical_actions() {
        let content = fs::read_to_string("src/app_impl/ui_window.rs")
            .expect("Failed to read src/app_impl/ui_window.rs");

        // The handler must route through the shared dispatcher.
        let handler = function_body(&content, "fn handle_main_footer_action");
        assert!(
            handler.contains("dispatch_main_window_footer_action(action"),
            "handle_main_footer_action must route through dispatch_main_window_footer_action()"
        );

        // Scope every arm assertion to the dispatcher's actual match arm so
        // unrelated growth elsewhere in the function can never mask (or fake)
        // a routing regression.
        let dispatcher = function_body(&content, "fn dispatch_main_window_footer_action");

        let run_arm = match_arm_body(dispatcher, "crate::footer_popup::FooterAction::Run =>");
        assert!(
            run_arm.contains("try_run_ready_agent_chat_script"),
            "FooterAction::Run must check for a ready Agent Chat script before falling back"
        );
        assert!(
            run_arm.contains("execute_selected"),
            "FooterAction::Run must fall back to execute_selected() for non-Agent Chat views"
        );

        let actions_arm =
            match_arm_body(dispatcher, "crate::footer_popup::FooterAction::Actions =>");
        assert!(
            actions_arm.contains("dispatch_actions_toggle_for_current_view("),
            "FooterAction::Actions must dispatch through dispatch_actions_toggle_for_current_view(), \
             which owns the has_actions() gating"
        );
        assert!(
            !actions_arm.contains("self.toggle_actions("),
            "FooterAction::Actions must not bypass dispatch_actions_toggle_for_current_view()"
        );

        let stop_arm = match_arm_body(dispatcher, "crate::footer_popup::FooterAction::Stop =>");
        assert!(
            stop_arm.contains("cancel_streaming_from_escape"),
            "FooterAction::Stop must dispatch through the Agent Chat streaming cancellation path"
        );

        let paste_arm = match_arm_body(
            dispatcher,
            "crate::footer_popup::FooterAction::PasteResponse =>",
        );
        assert!(
            paste_arm.contains("paste_latest_agent_chat_response_to_frontmost"),
            "FooterAction::PasteResponse must paste the latest Agent Chat assistant response"
        );

        let ai_arm = match_arm_body(dispatcher, "crate::footer_popup::FooterAction::Ai =>");
        assert!(
            ai_arm.contains("open_tab_ai_agent_chat_with_entry_intent"),
            "FooterAction::Ai must dispatch to open_tab_ai_agent_chat_with_entry_intent()"
        );
    }

    /// The Agent affordance lives in the standard native footer buttons for
    /// the ScriptList view (FooterAction::Ai with the ⌘↵ shortcut). The shared
    /// launcher hint component must keep using the shared footer hint renderer
    /// so it cannot drift visually if it is re-adopted.
    #[test]
    fn test_agent_button_lives_in_native_footer() {
        let footer_content = fs::read_to_string("src/app_impl/ui_window.rs")
            .expect("Failed to read src/app_impl/ui_window.rs");

        let hint_content = fs::read_to_string("src/components/launcher_ask_ai_hint.rs")
            .expect("Failed to read src/components/launcher_ask_ai_hint.rs");

        // Agent button is part of the standard native footer buttons
        let standard_fn = footer_content
            .find("fn standard_main_window_footer_buttons")
            .expect("standard_main_window_footer_buttons must exist");
        let standard_section =
            &footer_content[standard_fn..footer_content.len().min(standard_fn + 2500)];
        assert!(
            standard_section.contains("FooterAction::Ai") && standard_section.contains("\"Agent\""),
            "Agent button should be in the standard native footer buttons"
        );
        assert!(
            standard_section.contains("\"⌘↵\""),
            "Agent footer button should advertise the ⌘↵ shortcut"
        );

        // The shared hint component renders through the shared footer hint renderer
        assert!(
            hint_content.contains("render_footer_hint_content"),
            "Agent hint should use the shared footer hint renderer"
        );
        assert!(
            hint_content.contains("\"Agent\".into()"),
            "Agent hint should show the Agent label"
        );
    }

    /// Fails if the native footer bridge stops guarding against views that
    /// don't support the native footer. The handler must check the view-driven
    /// footer config resolver before dispatching.
    #[test]
    fn test_native_footer_guards_mini_scriptlist_surface() {
        let content = fs::read_to_string("src/app_impl/ui_window.rs")
            .expect("Failed to read src/app_impl/ui_window.rs");

        let handler_pos = content
            .find("fn handle_main_footer_action")
            .expect("handle_main_footer_action must exist");
        let handler_section = &content[handler_pos..content.len().min(handler_pos + 1500)];

        assert!(
            handler_section.contains("main_window_footer_config_with_cx("),
            "Native footer handler must check the view-driven footer config to guard dispatch"
        );
        assert!(
            handler_section.contains("is_main_window_visible()"),
            "Native footer handler must check window visibility"
        );
    }

    /// Fails if sync_main_footer_popup stops calling ensure_main_footer_action_listener.
    /// Without this, the async bridge is never started and native clicks are silently dropped.
    #[test]
    fn test_sync_footer_popup_starts_listener() {
        let content = fs::read_to_string("src/app_impl/ui_window.rs")
            .expect("Failed to read src/app_impl/ui_window.rs");

        let sync_pos = content
            .find("fn sync_main_footer_popup")
            .expect("sync_main_footer_popup must exist in ui_window.rs");
        let sync_section = &content[sync_pos..content.len().min(sync_pos + 400)];

        assert!(
            sync_section.contains("ensure_main_footer_action_listener"),
            "sync_main_footer_popup must call ensure_main_footer_action_listener to start the bridge"
        );
    }

    /// Fails if the shared actions dispatcher stops falling back to the
    /// generic shared-dialog handler for views that advertise SharedDialog
    /// support but don't have a dedicated branch.
    #[test]
    fn test_dispatcher_has_shared_dialog_fallback() {
        let content = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("Failed to read src/app_impl/actions_toggle.rs");

        assert!(
            content.contains("dispatch_shared_actions_toggle_fallback"),
            "Dispatcher must have a shared-dialog fallback method"
        );
        assert!(
            content.contains("current_view_supports_shared_actions()"),
            "Dispatcher must check current_view_supports_shared_actions() before the unsupported-view fallthrough"
        );
        assert!(
            content.contains("actions_toggle_dispatch_routed_shared_dialog_fallback"),
            "Shared-dialog fallback must emit a structured tracing event"
        );
    }

    /// Fails if handle_cmd_k_actions_toggle stops routing through the shared
    /// dispatcher, which would allow keyboard and footer click behavior to
    /// drift apart again.
    #[test]
    fn test_cmd_k_handler_uses_shared_dispatcher() {
        let content = fs::read_to_string("src/app_impl/actions_toggle.rs")
            .expect("Failed to read src/app_impl/actions_toggle.rs");

        let handler_pos = content
            .find("fn handle_cmd_k_actions_toggle")
            .expect("handle_cmd_k_actions_toggle must exist in actions_toggle.rs");
        let handler_section = &content[handler_pos..content.len().min(handler_pos + 600)];

        assert!(
            handler_section.contains("dispatch_actions_toggle_for_current_view"),
            "handle_cmd_k_actions_toggle must delegate to dispatch_actions_toggle_for_current_view"
        );
        assert!(
            handler_section.contains("cmd_k_actions_routed"),
            "handle_cmd_k_actions_toggle must emit cmd_k_actions_routed tracing event"
        );
    }

    /// Fails if the startup interceptor reintroduces host-specific Cmd+K
    /// dispatch instead of delegating through the named actions key intent
    /// after popup-owned key routing.
    #[test]
    fn test_startup_cmd_k_uses_shared_dispatcher_after_popup_router() {
        let content = fs::read_to_string("src/app_impl/startup.rs")
            .expect("Failed to read src/app_impl/startup.rs");

        let route_pos = content
            .find("match this.route_key_to_actions_dialog(")
            .expect("startup.rs must route popup-owned keys through route_key_to_actions_dialog()");
        let intent_pos = content[route_pos..]
            .find("main_window_actions_key_intent(&this.current_view, event)")
            .map(|offset| route_pos + offset)
            .expect("startup.rs must classify local actions key intent after popup routing");
        let intent_section = &content[intent_pos..content.len().min(intent_pos + 500)];

        assert!(
            intent_section.contains("handle_main_window_actions_key_intent(intent, window, cx)"),
            "startup actions key intent must route through the named intent handler. Found:\n{}",
            intent_section
        );
        assert!(
            content.contains("self.handle_cmd_k_actions_toggle(window, cx)"),
            "ToggleActions must still route through handle_cmd_k_actions_toggle()"
        );
        assert!(
            !intent_section.contains("toggle_actions_for_host("),
            "startup actions key intent must not bypass the shared dispatcher with toggle_actions_for_host(). Found:\n{}",
            intent_section
        );
        assert!(
            route_pos < intent_pos,
            "startup must check route_key_to_actions_dialog() before local actions key intent dispatch"
        );
    }

    /// Fails if the native footer buttons stop using the ScriptKitFooterButton
    /// subclass that accepts first mouse, which would break single-click
    /// behavior when the main window is inactive.
    #[test]
    fn test_native_footer_buttons_use_first_mouse_subclass() {
        let content =
            fs::read_to_string("src/footer_popup.rs").expect("Failed to read src/footer_popup.rs");

        assert!(
            content.contains("ScriptKitFooterButton"),
            "Footer buttons must use the ScriptKitFooterButton subclass"
        );
        assert!(
            content.contains("footer_button_class()"),
            "make_footer_hint_item must allocate from footer_button_class()"
        );
        assert!(
            content.contains("footer_button_accepts_first_mouse"),
            "ScriptKitFooterButton must implement acceptsFirstMouse:"
        );
        assert!(
            content.contains("footer_button_mouse_down_can_move_window"),
            "ScriptKitFooterButton must implement mouseDownCanMoveWindow"
        );
        // Must NOT allocate plain NSButton for footer hint items.
        // (Plain NSButton is fine for the transparent cwd/left-info chip hit
        // targets elsewhere in the file — scope the check to the hint item fn.)
        let hint_item_pos = content
            .find("unsafe fn make_footer_hint_item")
            .expect("make_footer_hint_item must exist");
        let hint_item_end = content[hint_item_pos..]
            .find("\nunsafe fn ")
            .or_else(|| content[hint_item_pos..].find("\nfn "))
            .map(|offset| hint_item_pos + offset)
            .unwrap_or(content.len());
        let hint_item_section = &content[hint_item_pos..hint_item_end];
        assert!(
            !hint_item_section.contains("msg_send![class!(NSButton), alloc]"),
            "Footer hint items must not allocate plain NSButton — use footer_button_class()"
        );
    }

    /// Fails if native footer hit testing only returns the NSButton's own tiny
    /// hit region. Clicks on the label/key text or empty slot area must resolve
    /// to the sibling button so the whole visible footer item is clickable.
    #[test]
    fn test_native_footer_hit_test_resolves_whole_item_to_button() {
        let content =
            fs::read_to_string("src/footer_popup.rs").expect("Failed to read src/footer_popup.rs");

        assert!(
            content.contains("fn footer_item_button_at_point"),
            "Native footer must resolve visible item slots by geometry, not only from super hitTest"
        );
        assert!(
            content.contains("fn footer_button_in_subviews"),
            "Footer hit testing must be able to find a button inside an item wrapper"
        );
        assert!(
            content.contains("fn nearest_footer_item_button"),
            "Footer hit testing must map text-field/wrapper hits to the sibling button"
        );

        let hit_test_pos = content
            .find("extern \"C\" fn footer_hit_test")
            .expect("footer_hit_test must exist");
        let hit_test_section = &content[hit_test_pos..content.len().min(hit_test_pos + 2200)];
        let geometry_pos = hit_test_section
            .find("footer_item_button_at_point(this_id, point)")
            .expect("footer_hit_test must call footer_item_button_at_point(this_id, point)");
        let super_pos = hit_test_section
            .find("super(this_id, class!(NSVisualEffectView)), hitTest: point")
            .expect("footer_hit_test must retain the AppKit super fallback");
        assert!(
            geometry_pos < super_pos,
            "Footer item geometry must be checked before super hitTest so front sibling overlays cannot steal item-slot clicks. Found:\n{}",
            hit_test_section
        );
        assert!(
            hit_test_section.contains("nearest_footer_button(hit)"),
            "footer_hit_test must preserve direct button hits. Found:\n{}",
            hit_test_section
        );
        assert!(
            hit_test_section.contains("nearest_footer_item_button(hit)"),
            "footer_hit_test must resolve footer item wrapper hits to the sibling button. Found:\n{}",
            hit_test_section
        );

        let helper_pos = content
            .find("fn footer_item_button_at_point")
            .expect("footer_item_button_at_point must exist");
        let helper_section = &content[helper_pos..content.len().min(helper_pos + 3000)];
        assert!(
            helper_section.contains("FOOTER_HINTS_ID")
                && helper_section.contains("convertPoint")
                && helper_section.contains("footer_button_in_subviews(item)"),
            "Geometry helper must search the native hints view, convert points into item bounds, and return the contained button. Found:\n{}",
            helper_section
        );
    }

    /// Fails if passive native footer chrome can cover the hint strip and steal
    /// AppKit hit tests before item-slot geometry sees the click.
    #[test]
    fn test_native_footer_left_info_view_is_passthrough() {
        let content =
            fs::read_to_string("src/footer_popup.rs").expect("Failed to read src/footer_popup.rs");

        assert!(
            content.contains("fn footer_passthrough_view_class")
                && content.contains("footer_passthrough_hit_test"),
            "Passive native footer overlays need a pass-through view class"
        );

        // Scope to the function that actually allocates `left_info_view`.
        //
        // This audit spent time red and, worse, unenforcing: it searched for
        // `unsafe fn ensure_main_footer_host`, which a rename turned into
        // `ensure_main_window_footer_host` before the allocation itself moved
        // out into `install_footer_host_view`. A source audit that cannot find
        // its anchor does not fail safe — it panics on the `expect` and stops
        // checking the invariant it exists to protect, which here is that a
        // passive footer overlay cannot steal AppKit hit tests from the hint
        // strip.
        //
        // If this `expect` fires again, repoint it at whichever function owns
        // the `left_info_view` allocation rather than deleting the assertion.
        let ensure_pos = content.find("unsafe fn install_footer_host_view").expect(
            "install_footer_host_view must exist — if it was renamed, repoint this audit at \
                 the function that allocates left_info_view",
        );
        let ensure_section = &content[ensure_pos..content.len().min(ensure_pos + 5000)];
        assert!(
            ensure_section.contains("msg_send![footer_passthrough_view_class(), alloc]"),
            "left_info_view must use a hit-test pass-through class so it cannot cover footer item slots. Found:\n{}",
            ensure_section
        );
        assert!(
            !ensure_section.contains("let left_info_view: id = msg_send![class!(NSView), alloc];"),
            "left_info_view must not be a plain NSView because it can steal super hitTest over the hint strip"
        );
    }

    /// Fails if the ScriptList footer rail stops dispatching clicks through
    /// the canonical footer action authority. The old clickable strip
    /// hand-rolled per-button callbacks (toggle_actions, open_tab_ai…, a
    /// local has_actions() gate); the config-driven rail routes every button
    /// through dispatch_main_window_footer_action, the same authority the
    /// native footer and Cmd+K use — gating and view-aware routing are locked
    /// there (see test_native_footer_dispatches_canonical_actions and
    /// test_cmd_k_handler_uses_shared_dispatcher).
    /// Complements test_footer_uses_config_driven_footer_rail by verifying
    /// the actual callback wiring, not just the function name.
    #[test]
    fn test_full_footer_clickable_strip_dispatches_canonical_actions() {
        let content = fs::read_to_string("src/render_script_list/mod.rs")
            .expect("Failed to read src/render_script_list/mod.rs");

        let mut search_from = 0usize;
        let mut rail_sites = 0usize;
        while let Some(found) = content[search_from..].find("render_main_window_footer_config_rail")
        {
            let strip_pos = search_from + found;
            let strip_section = &content[strip_pos..content.len().min(strip_pos + 2000)];
            assert!(
                strip_section.contains("dispatch_main_window_footer_action"),
                "every footer rail click handler must route through dispatch_main_window_footer_action()"
            );
            rail_sites += 1;
            search_from = strip_pos + "render_main_window_footer_config_rail".len();
        }
        assert!(
            rail_sites >= 1,
            "the config-driven footer rail must exist in render_script_list"
        );
    }
}
