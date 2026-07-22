use std::fs;

fn read(path: &str) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("failed to read {path}: {error}"))
}

fn function_body(source: &str, signature: &str) -> String {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} not found"));
    let rest = &source[start..];
    let open = rest.find('{').expect("function body open brace");
    let mut depth = 0usize;

    for (index, ch) in rest[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return rest[open..open + index + 1].to_string();
                }
            }
            _ => {}
        }
    }

    panic!("unterminated function body for {signature}");
}

#[test]
fn confirm_modal_shell_is_registered_as_shared_component() {
    let components_mod = read("src/components/mod.rs");
    let shell = read("src/components/confirm_modal_shell.rs");

    assert!(
        components_mod.contains("pub(crate) mod confirm_modal_shell;"),
        "shared confirm modal shell must be registered in src/components/mod.rs"
    );
    assert!(
        components_mod.contains("confirm_modal_shell")
            && components_mod.contains("ConfirmModalShellConfig")
            && components_mod.contains("CONFIRM_MODAL_SHELL_ID"),
        "shared shell exports must be available to modal routes"
    );
    assert!(
        shell.contains("CONFIRM_MODAL_SHELL_ID")
            && shell.contains("\"modal-shell:confirm\"")
            && shell.contains("confirm_modal_header")
            && shell.contains("confirm_modal_shell("),
        "shared shell must own the common confirm modal marker, header, and panel builder"
    );
}

#[test]
fn shortcut_recorder_and_confirm_popup_use_the_same_shell() {
    let shortcut = read("src/components/shortcut_recorder/render.rs");
    let confirm = read("src/confirm/window.rs");
    let shell = read("src/components/confirm_modal_shell.rs");

    for (label, source, content_id) in [
        (
            "shortcut recorder",
            shortcut.as_str(),
            "shortcut-modal-content",
        ),
        ("confirm popup", confirm.as_str(), "confirm-modal-content"),
    ] {
        assert!(
            source.contains("confirm_modal_shell(")
                && source.contains("ConfirmModalShellConfig")
                && source.contains(content_id),
            "{label} must render through the shared confirm modal shell while preserving route content id {content_id}"
        );
        assert!(
            source.contains("confirm_modal_header("),
            "{label} must use the shared confirm modal header"
        );
        assert!(
            source.contains("modal_action_row")
                && source.contains("ModalActionRowButton")
                && source.contains("FooterHintButtonLayoutOverrides"),
            "{label} must route modal actions through the shared modal action row"
        );
        assert!(
            !source.contains("Button::new(")
                && !source.contains("ButtonVariant::Ghost")
                && !source.contains("ButtonVariant::Primary"),
            "{label} must not render modal actions through the generic/local button component"
        );
    }
    assert!(
        shortcut.contains("shortcut-cancel-button")
            && shortcut.contains("shortcut-save-button")
            && shell.contains("key_first: false"),
        "shortcut recorder buttons must follow confirm modal action ordering"
    );
}

#[test]
fn confirm_popup_uses_shortcut_modal_button_and_chrome_tokens() {
    let footer = read("src/components/footer_chrome.rs");
    let confirm = read("src/confirm/window.rs");
    let render_block = confirm
        .split("impl Render for ConfirmPopupWindow")
        .nth(1)
        .expect("ConfirmPopupWindow render implementation should be present");

    assert!(
        footer.contains("FooterHintButtonLayoutOverrides")
            && footer.contains("FooterHintActionButtonFrameSpec")
            && footer.contains("render_footer_hint_button_like_with_layout")
            && footer.contains("render_footer_hint_action_button_frame")
            && footer.contains("footer_centered_action_edge_padding_x")
            && footer.contains("edge_padding_x_px")
            && footer.contains("shrink_frame_to_content_px")
            && footer.contains("footer-action-button-slot")
            && footer.contains(".group_hover(\"footer-action-button-slot\"")
            && footer.contains(".items_center()")
            && footer.contains(".justify_center()")
            && footer.contains("themed_footer_button_hover_rgba")
            && footer.contains("themed_footer_button_active_rgba"),
        "footer chrome must own the reusable action button frame, centering, hover, and active state used by confirm modal actions"
    );
    assert!(
        confirm.contains("modal_action_row")
            && confirm.contains("ModalActionRowButton")
            && confirm.contains("FooterHintButtonLayoutOverrides")
            && confirm.contains("footer_action_slot_width")
            && confirm.contains("FooterActionSlot::Close")
            && confirm.contains("FooterActionSlot::Run")
            && confirm.contains("footer_button_height")
            && confirm.contains("current_main_menu_footer_height")
            && confirm.contains("current_main_menu_footer_metrics().item_gap_px")
            && confirm.contains("edge_padding_x_px")
            && confirm.contains("footer_centered_action_button_layout")
            && confirm.contains("shrink_frame_to_content_px")
            && confirm.contains("confirm_action_button_layout")
            && confirm.contains("confirm_modal_stack_gaps")
            && confirm.contains("confirm_modal_spacer")
            && confirm.contains("confirm_anatomy_header_body_gap")
            && confirm.contains("confirm_anatomy_body_actions_gap")
            && confirm.contains("confirm_body_line_height"),
        "confirm popup must reuse the shared modal action row and expose action/anatomy modal designer overrides"
    );
    assert!(
        render_block.contains("modal_action_row")
            && render_block.contains("ModalActionRowButton")
            && render_block.contains("confirm-cancel-button")
            && render_block.contains("confirm-ok-button")
            && render_block.contains("label: self.cancel_text.clone()")
            && render_block.contains("key: \"Esc\".into()")
            && render_block.contains("label: self.confirm_text.clone()")
            && render_block.contains("key: \"↵\".into()")
            && render_block.contains("confirm-modal-stack")
            && render_block.contains("confirm-modal-gap:after-header")
            && render_block.contains("confirm-modal-gap:after-body"),
        "confirm popup actions must use the shared modal action-row primitive"
    );
    assert!(
        render_block.contains("gpui::rgb(chrome.accent_hex)")
            && render_block.contains("gpui::rgb(chrome.text_primary_hex)")
            && render_block.contains("gpui::rgba(chrome.border_rgba)")
            && render_block.contains("gpui::rgba(chrome.popup_surface_rgba)"),
        "confirm popup shell/header must use chrome theme tokens instead of local danger colors"
    );
    assert!(
        !render_block.contains("theme.colors.ui.error.with_opacity")
            && !render_block.contains("border_color = if is_danger")
            && !render_block.contains("accent_color = if is_danger")
            && !render_block.contains("Button::new(self.cancel_text")
            && !render_block.contains("Button::new(self.confirm_text")
            && !render_block.contains("on_mouse_down(MouseButton::Left")
            && !render_block.contains("render_footer_hint_button_like(")
            && !render_block.contains(".hover(move |style| style.bg(footer_button_hover_bg))")
            && !render_block.contains(".active(move |style| style.bg(footer_button_active_bg))")
            && !render_block.contains("FOOTER_ACTION_BUTTON_RADIUS_PX")
            && !render_block.contains("Button::new(")
            && !render_block.contains("key_first: true")
            && !render_block.contains(".mt(px(confirm_anatomy_header_body_gap()))")
            && !render_block.contains(".mt(px(if self.body.is_empty()")
            && !confirm.contains("components::button::BUTTON_GHOST_HEIGHT")
            && !confirm.contains("BUTTON_GAP"),
        "confirm popup must not reintroduce red danger shell styling, generic action buttons, hand-built mini buttons, or modal-local button dimensions"
    );
}

#[test]
fn confirm_popup_native_background_matches_actions_popup_not_footer_flush_strip() {
    let confirm = read("src/confirm/window.rs");
    let actions = read("src/actions/window.rs");
    let platform = read("src/platform/secondary_window_config.rs");
    let confirm_options = confirm
        .split("let handle = cx.open_window(")
        .nth(1)
        .and_then(|tail| tail.split("move |_window, cx|").next())
        .expect("confirm popup WindowOptions should be present");
    let actions_options = actions
        .split("let window_options = WindowOptions")
        .nth(1)
        .and_then(|tail| tail.split("};").next())
        .expect("actions popup WindowOptions should be present");
    let confirm_config = platform
        .split("pub unsafe fn configure_confirm_popup_window(window: id, is_dark: bool)")
        .nth(1)
        .and_then(|tail| tail.split("#[cfg(not(target_os = \"macos\"))]").next())
        .expect("macOS confirm popup config should be present");
    let footer_config = platform
        .split("pub unsafe fn configure_footer_popup_window(window: id, is_dark: bool)")
        .nth(1)
        .and_then(|tail| tail.split("#[cfg(not(target_os = \"macos\"))]").next())
        .expect("macOS footer popup config should be present");

    assert!(
        confirm_options.contains("kind: WindowKind::PopUp")
            && actions_options.contains("kind: WindowKind::PopUp"),
        "confirm and actions popup must stay on the same native GPUI WindowKind::PopUp surface"
    );
    assert!(
        confirm_config.contains("configure_actions_popup_window(window, is_dark)")
            && !confirm_config.contains("setHasShadow: false")
            && !confirm_config.contains("setCornerRadius: 0.0_f64"),
        "centered confirm popup must keep the same native background/depth configuration as actions popup"
    );
    assert!(
        footer_config.contains("configure_actions_popup_window(window, is_dark)")
            && footer_config.contains("setIgnoresMouseEvents: true")
            && footer_config.contains("setHasShadow: false")
            && footer_config.contains("setCornerRadius: 0.0_f64"),
        "flush footer picker owns the no-shadow/no-corner exception instead of confirm popup"
    );
}

/// Confirm popup must not become key during creation or native attach.
///
/// AppKit child-window ordering keeps the popup visually above the parent while
/// the main window routes live Enter/Tab/Escape handling. Promoting the popup
/// NSWindow to key after attach can drop it behind the main window.
#[test]
fn confirm_popup_does_not_promote_native_window_to_key() {
    let confirm = read("src/confirm/window.rs");
    let body = function_body(confirm.as_str(), "pub(crate) fn open_confirm_popup_window(");
    let options = body
        .split("let handle = cx.open_window(")
        .nth(1)
        .and_then(|tail| tail.split("move |_window, cx|").next())
        .expect("confirm popup WindowOptions should be present");

    assert!(
        !options.contains("focus: true"),
        "confirm popup must not open with focus: true; that can make the popup key before AppKit attaches it to the parent and visually demote the parent shadow"
    );
    assert!(
        options.contains("focus: false"),
        "confirm popup must explicitly open with focus: false"
    );
    assert!(
        body.contains("addChildWindow:confirm_ns_window ordered:NS_WINDOW_ABOVE")
            && body.contains("orderFrontRegardless"),
        "confirm popup must keep AppKit child attachment plus explicit front ordering"
    );
    assert!(
        !body.contains("makeKeyWindow"),
        "confirm popup must not call makeKeyWindow; main-window routing handles live confirm keys without native key promotion"
    );
}

#[test]
fn destructive_confirm_routes_use_shared_parent_confirm_helpers() {
    let scripts = read("src/app_actions/handle_action/scripts.rs");
    let files = read("src/app_actions/handle_action/files.rs");
    let clipboard = read("src/app_actions/handle_action/clipboard.rs");
    let notes = read("src/notes/window/notes.rs");

    assert!(
        scripts.contains("crate::confirm::open_parent_confirm_dialog_for_entity(")
            && scripts.contains("remove_script_confirmed")
            && scripts.contains("quit_script_kit_confirm_options()"),
        "script removal and quit confirmations must use the entity-owned shared parent confirm helper"
    );
    assert!(
        files.contains("crate::confirm::ParentConfirmOptions::destructive(")
            && files.contains("crate::confirm::confirm_with_parent_dialog(")
            && files.contains("\"Async action cancelled: move_to_trash\""),
        "file move-to-trash confirmation must use the shared parent confirm dialog and handle cancel"
    );
    assert!(
        clipboard.contains("clipboard_delete_multiple")
            && clipboard.contains("clipboard_delete_all")
            && clipboard.contains("crate::confirm::ParentConfirmOptions::destructive(")
            && clipboard.contains("crate::confirm::confirm_with_parent_dialog("),
        "clipboard bulk delete confirmations must use the shared parent confirm dialog"
    );
    assert!(
        notes.contains("crate::confirm::open_parent_confirm_dialog_for_automation_parent(")
            && notes.contains("\"notes\"")
            && notes.contains("notes_delete_cancelled")
            && notes.contains("restore_primary_focus_after_dialog"),
        "notes delete confirmation must use the parent-id-aware shared confirm helper and restore focus on cancel"
    );
}

#[test]
fn confirm_modal_inventory_tracks_routes_and_excludes_non_modals() {
    let inventory = read(".goals/receipts/modal-inventory.md");

    for route in [
        "Add Shortcut / shortcut recorder",
        "Quit Script Kit",
        "Remove/delete script",
        "Move file to trash",
        "Clipboard bulk delete / clear unpinned",
        "SDK `confirm`",
        "`openConfirmPrompt` stdin fixture",
        "`showShortcutRecorder` stdin fixture",
        "Notes delete confirmation",
    ] {
        assert!(
            inventory.contains(route),
            "modal inventory must track confirm route: {route}"
        );
    }

    for excluded in [
        "Actions Menu",
        "Trigger picker",
        "Hover/dropdown menus",
        "Browse panels",
        "Editor choice popup",
    ] {
        assert!(
            inventory.contains(excluded),
            "modal inventory must explicitly exclude non-modal surface: {excluded}"
        );
    }
}

#[test]
fn actions_and_trigger_pickers_are_not_classified_as_confirm_modals() {
    let inventory = read(".goals/receipts/modal-inventory.md");

    assert!(
        inventory.contains("Actions Menu")
            && inventory.contains("not a confirm/deny modal")
            && inventory.contains("Trigger picker"),
        "actions menu and trigger picker must stay out of the confirm modal migration"
    );
}

#[test]
fn sdk_confirm_api_stays_single_word_and_avoids_modal_namespace() {
    let sdk = read("scripts/kit-sdk.ts");
    let goal = read(".goals/modal-consistency-design-system.md");
    let inventory = read(".goals/receipts/modal-inventory.md");

    assert!(
        sdk.contains("export interface ConfirmConfig")
            && sdk.contains("(globalThis as any).confirm = async function confirm")
            && sdk.contains("type: 'confirm'"),
        "SDK confirm exposure must keep the typed global confirm() implementation"
    );
    assert!(
        !sdk.contains("modal.confirm") && !sdk.contains("globalThis.modal"),
        "SDK confirm exposure must not introduce a modal.confirm namespace"
    );
    assert!(
        goal.contains("API as `confirm`") && !goal.contains("modal.confirm"),
        "goal contract must preserve the single-word SDK confirm API"
    );
    assert!(
        inventory.contains("SDK `confirm`"),
        "inventory must track the script-facing SDK confirm route"
    );
}

#[test]
fn destructive_confirm_safety_scenario_uses_dry_run_confirm_fixture() {
    let scenario = read("scripts/agentic/scenario.ts");

    assert!(
        scenario.contains("runDestructiveConfirmModalSafetyStressScenario")
            && scenario.contains("dryRunOnlyRequired")
            && scenario.contains("type: \"openConfirmPrompt\"")
            && scenario.contains("agentic-destructive-confirm-open"),
        "destructive confirm safety stress must drive a dry-run confirm fixture instead of staying a static missing-receipt stub"
    );
    assert!(
        scenario.contains("destructiveCommandExecuted: false")
            && scenario.contains("systemCommandRequested: false")
            && scenario.contains("trashMutationRequested: false"),
        "dry-run destructive confirm proof must explicitly guard against real destructive commands"
    );
}

#[test]
fn sdk_confirm_runtime_proof_uses_real_script_run_route() {
    let scenario = read("scripts/agentic/scenario.ts");
    let index = read("scripts/agentic/index.ts");
    let smoke = read("tests/smoke/test-confirm-sdk-runtime.ts");
    let inventory = read(".goals/receipts/modal-inventory.md");

    assert!(
        scenario.contains("runSdkConfirmRuntimeProofScenario")
            && scenario.contains("tests/smoke/test-confirm-sdk-runtime.ts")
            && scenario.contains("type: \"run\"")
            && scenario.contains("sdk-confirm-run-")
            && scenario.contains("SCRIPT_KIT_SESSION_DIR")
            && scenario.contains("SCRIPT_KIT_GPUI_BINARY")
            && scenario.contains("SCRIPT_KIT_DISABLE_AUTOMATIC_UPDATE_CHECK")
            && scenario.contains("expectedProtocolType: \"confirm\"")
            && scenario.contains("expectedSurface: \"ConfirmPrompt\""),
        "SDK confirm runtime proof must drive the real script run route into the shared ConfirmPrompt surface from an isolated session"
    );
    assert!(
        scenario.contains("simulateKey")
            && scenario.contains("sdk-confirm-escape-cancel")
            && scenario.contains("scriptResult.result === false")
            && scenario.contains("processTree")
            && scenario.contains("failureArtifacts")
            && scenario.contains("confirm-modal-sdk-confirm-runtime-artifacts")
            && !scenario
                .split("runSdkConfirmRuntimeProofScenario")
                .nth(1)
                .expect("SDK confirm runtime proof scenario should be present")
                .contains("openConfirmPrompt"),
        "SDK confirm runtime proof must resolve safely through Escape, preserve blocker artifacts on failure, and must not fall back to the stdin confirm fixture"
    );
    assert!(
        index.contains("sdk-confirm-runtime-proof")
            && index.contains("runSdkConfirmRuntimeProofScenario"),
        "agentic index must expose a stable top-level SDK confirm runtime proof recipe"
    );
    assert!(
        smoke.contains("await confirm({")
            && smoke.contains("SDK confirm runtime proof?")
            && smoke.contains("Confirm SDK")
            && smoke.contains("Cancel SDK")
            && smoke.contains("resultType: typeof result"),
        "SDK confirm smoke script must call global confirm() and persist the boolean result receipt"
    );
    assert!(
        inventory.contains("SDK `confirm`"),
        "inventory must track the SDK confirm runtime proof status"
    );
}

#[test]
fn confirm_prompt_simulate_key_routes_confirm_and_cancel() {
    let source = read("src/app_impl/simulate_key_dispatch.rs");

    assert!(
        source.contains("AppView::ConfirmPrompt { .. }")
            && source.contains("SimulateKey: Escape - cancel ConfirmPrompt")
            && source.contains("view.resolve_confirm_prompt(false, window, ctx)")
            && source.contains("SimulateKey: Enter - confirm ConfirmPrompt")
            && source.contains("view.resolve_confirm_prompt(confirmed, window, ctx)")
            && source.contains("view.toggle_confirm_prompt_focus(ctx)"),
        "stdin simulateKey must route ConfirmPrompt Tab, Enter, and Escape through the shared confirm resolver"
    );
}

#[test]
fn prompt_popup_batch_confirm_selection_closes_confirm_popup() {
    let confirm = read("src/confirm/window.rs");
    let prompt_handler = read("src/prompt_handler/mod.rs");
    let by_value = confirm
        .split("pub(crate) fn batch_select_confirm_button_by_value")
        .nth(1)
        .and_then(|section| {
            section
                .split("pub(crate) fn batch_select_confirm_button_by_semantic_id")
                .next()
        })
        .expect("batch_select_confirm_button_by_value should be present");

    assert!(
        by_value.contains("send_confirm_result(confirmed)")
            && by_value.contains("close_confirm_window(cx)"),
        "PromptPopup batch activation must match mouse/key confirm behavior by closing the confirm popup after sending the result"
    );
    assert!(
        prompt_handler.contains("batch_select_confirm_button_by_value(&value, cx)")
            && prompt_handler
                .contains("batch_select_confirm_button_by_semantic_id(&semantic_id, cx)"),
        "PromptPopup batch routing must pass App context so confirm selection can close the popup"
    );
}

#[test]
fn sdk_confirm_host_route_uses_shared_confirm_prompt_surface() {
    let source = read("src/prompt_handler/mod.rs");
    let show_confirm = source
        .split("PromptMessage::ShowConfirm {\n                id,")
        .nth(1)
        .and_then(|section| section.split("PromptMessage::ShowChat").next())
        .expect("ShowConfirm handler block should be present");

    assert!(
        show_confirm.contains("self.prepare_window_for_prompt(\"UI\", \"confirm\", \"\")")
            && show_confirm.contains("self.open_confirm_prompt(")
            && show_confirm.contains("async_channel::bounded::<bool>(1)")
            && show_confirm.contains("Message::Submit"),
        "SDK confirm() host route must open the shared in-window ConfirmPrompt and submit the bool result back to the script"
    );
    assert!(
        !show_confirm.contains("confirm_with_parent_dialog"),
        "SDK confirm() host route must not bypass the shared ConfirmPrompt surface via the native parent popup"
    );
}

#[test]
fn notes_delete_confirm_route_proof_uses_real_notes_route() {
    let scenario = read("scripts/agentic/scenario.ts");
    let index = read("scripts/agentic/index.ts");
    let notes = read("src/notes/window/notes.rs");
    let keyboard = read("src/notes/window/keyboard.rs");

    let scenario_block = scenario
        .split("runNotesDeleteConfirmRouteProofScenario")
        .nth(1)
        .and_then(|section| {
            section
                .split("runActionsCommandDiscoverabilityNoopStressScenario")
                .next()
        })
        .expect("Notes delete confirm route proof scenario should be present");

    assert!(
        scenario_block.contains("SCRIPT_KIT_TEST_NOTES_DB_PATH")
            && scenario_block.contains("/tmp/confirm-modal-notes-delete-proof/")
            && scenario_block.contains("type: \"openNotes\"")
            && scenario_block.contains("notes-delete-confirm-cmd-n")
            && scenario_block.contains("notes-delete-cmd-shift-backspace")
            && scenario_block.contains("target: notesTarget")
            && scenario_block.contains("button:1:cancel")
            && scenario_block.contains("parentWindowId")
            && scenario_block.contains("notes")
            && scenario_block.contains("confirmDialog")
            && scenario_block.contains("Move note to Trash")
            && scenario_block.contains("Delete")
            && scenario_block.contains("Cancel")
            && scenario_block.contains("sandboxNoteDeleted")
            && !scenario_block.contains("openConfirmPrompt")
            && !scenario_block.contains("delete_note_by_id("),
        "Notes delete route proof must use a sandboxed real Notes shortcut path into the attached confirm popup, then cancel without direct modal/deletion fallbacks"
    );
    assert!(
        index.contains("notes-delete-confirm-route-proof")
            && index.contains("runNotesDeleteConfirmRouteProofScenario"),
        "agentic index must expose the Notes delete confirm route proof recipe"
    );
    assert!(
        notes.contains("crate::confirm::open_parent_confirm_dialog_for_automation_parent(")
            && notes.contains("\"notes\"")
            && notes.contains("\"Move note to Trash\"")
            && notes.contains("\"Delete\"")
            && notes.contains("cancel_text: \"Cancel\".into()"),
        "Notes delete confirmation must route through the shared parent confirm helper with Notes as the automation parent"
    );
    assert!(
        keyboard.contains("handle_platform_delete_shortcut")
            && keyboard.contains("request_delete_selected_note(window, cx)")
            && keyboard.contains("notes-delete") == false,
        "Notes keyboard delete shortcut should own the real route without adding test-only shortcut branches"
    );
}
