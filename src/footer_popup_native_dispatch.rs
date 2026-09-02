
#[cfg(target_os = "macos")]
unsafe fn bind_native_footer_buttons(view: id, token: u64) {
    if view == nil { return; }
    let is_button: cocoa::base::BOOL = msg_send![view, isKindOfClass: footer_button_class()];
    if is_button == YES {
        if let Some(button) = view.as_mut() { button.set_ivar::<u64>("_footerBindingToken", token); }
    }
    let children: id = msg_send![view, subviews];
    if children == nil { return; }
    let count: usize = msg_send![children, count];
    for index in 0..count {
        let child: id = msg_send![children, objectAtIndex: index];
        bind_native_footer_buttons(child, token);
    }
}

fn footer_action_key(action: FooterAction) -> &'static str {
    action.semantic_key()
}

#[cfg(target_os = "macos")]
fn ns_string(text: &str) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let Ok(c_string) = std::ffi::CString::new(text) else {
        return nil;
    };

    // SAFETY: The CString is NUL-terminated and lives for the duration of the call.
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c_string.as_ptr()] }
}

#[cfg(target_os = "macos")]
unsafe fn ns_color_from_rgba(rgba: u32) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let red = ((rgba >> 24) & 0xFF) as f64 / 255.0;
    let green = ((rgba >> 16) & 0xFF) as f64 / 255.0;
    let blue = ((rgba >> 8) & 0xFF) as f64 / 255.0;
    let alpha = (rgba & 0xFF) as f64 / 255.0;

    // SAFETY: Standard AppKit color construction on the main thread.
    msg_send![
        class!(NSColor),
        colorWithSRGBRed: red
        green: green
        blue: blue
        alpha: alpha
    ]
}

#[cfg(target_os = "macos")]
unsafe fn ns_color_from_hex_with_alpha(hex: u32, alpha: f64) -> id {
    use objc::{class, msg_send, sel, sel_impl};

    let red = ((hex >> 16) & 0xFF) as f64 / 255.0;
    let green = ((hex >> 8) & 0xFF) as f64 / 255.0;
    let blue = (hex & 0xFF) as f64 / 255.0;

    // SAFETY: Standard AppKit color construction on the main thread.
    msg_send![
        class!(NSColor),
        colorWithSRGBRed: red
        green: green
        blue: blue
        alpha: alpha
    ]
}

fn native_footer_visual_event_changed(
    button_id: &str,
    state_signature: usize,
    color_signature: usize,
    keycap_hex: u32,
) -> bool {
    type FooterVisualStateSignature = (usize, usize, u32);
    type FooterVisualStateCache = std::collections::HashMap<String, FooterVisualStateSignature>;
    static LAST_REPORTED: std::sync::OnceLock<std::sync::Mutex<FooterVisualStateCache>> =
        std::sync::OnceLock::new();
    let reported =
        LAST_REPORTED.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()));
    let Ok(mut reported) = reported.lock() else {
        // A poisoned diagnostics cache must never suppress real rendering.
        return true;
    };
    let signature = (state_signature, color_signature, keycap_hex);
    if reported.get(button_id).copied() == Some(signature) {
        false
    } else {
        reported.insert(button_id.to_string(), signature);
        true
    }
}

#[cfg(target_os = "macos")]
unsafe fn refresh_footer_button_visual_state_group(button: id) {
    use objc::{msg_send, sel, sel_impl};

    if button == nil {
        return;
    }
    let group_root: id = msg_send![button, superview];
    if group_root != nil {
        refresh_footer_button_visual_states(group_root);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_down(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    event: id,
) {
    use objc::{class, msg_send, sel, sel_impl};

    // SAFETY: `this` is our NSButton subclass. Actions opens a persistent popup,
    // so it owns selected visuals on mouse down instead of waiting for AppKit's
    // mouse-up action cycle to briefly clear and restore the state.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            let this_id = this as *const _ as id;
            let _: () = msg_send![super(this_id, class!(NSButton)), mouseDown: event];
            return;
        }

        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        if is_actions != YES {
            let this_id = this as *const _ as id;
            let _: () = msg_send![super(this_id, class!(NSButton)), mouseDown: event];
            return;
        }

        let button_id = this as *const _ as id;
        if let Some(obj) = button_id.as_mut() {
            obj.set_ivar::<cocoa::base::BOOL>("_selected", YES);
        }
        refresh_footer_button_visual_state_group(button_id);

        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_actions_mouse_down_selected",
            "Selected native footer Actions on mouse down"
        );
        let this_id = this as *const _ as id;
        send_footer_action_from_sender(this_id, FooterAction::Actions);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_entered(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    // SAFETY: Set hover background on the parent container's layer.
    // Recompute color from theme each time to avoid dangling CGColor pointers.
    unsafe {
        let enabled: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_enabled");
        if enabled != YES {
            return;
        }
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        if let Some(object) = (this as *const _ as id).as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", YES);
        }
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_entered",
            is_actions_button = is_actions == YES,
            "Native footer button hover entered"
        );

        refresh_footer_button_visual_state_group(this as *const _ as id);
    }
}

#[cfg(target_os = "macos")]
extern "C" fn footer_button_mouse_exited(
    this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    _event: id,
) {
    // SAFETY: Clear hover background on the parent container's layer.
    // If this button has _selected set, restore the selected color instead
    // of clearing.
    unsafe {
        let selected: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_selected");
        let is_actions: cocoa::base::BOOL = *this.get_ivar::<cocoa::base::BOOL>("_isActionsButton");
        let actions_window_open = crate::actions::is_actions_window_open();
        if let Some(object) = (this as *const _ as id).as_mut() {
            object.set_ivar::<cocoa::base::BOOL>("_hovered", NO);
        }
        tracing::debug!(
            target: "script_kit::footer_popup",
            event = "native_footer_button_hover_exited",
            is_actions_button = is_actions == YES,
            selected = selected == YES,
            actions_window_open,
            "Native footer button hover exited"
        );

        refresh_footer_button_visual_state_group(this as *const _ as id);
    }
}

#[cfg(target_os = "macos")]
fn footer_action_target() -> id {
    use std::sync::OnceLock;

    use objc::{msg_send, sel, sel_impl};

    static TARGET: OnceLock<usize> = OnceLock::new();

    // SAFETY: Creates the singleton footer action target via ObjC `new`; stored
    // for process lifetime in `OnceLock`.
    *TARGET.get_or_init(|| unsafe {
        let target: id = msg_send![footer_action_target_class(), new];
        target as usize
    }) as id
}

#[cfg(target_os = "macos")]
fn footer_action_selector(action: FooterAction) -> objc::runtime::Sel {
    use objc::{sel, sel_impl};

    match action {
        FooterAction::Run => sel!(runFooterAction:),
        FooterAction::Actions => sel!(actionsFooterAction:),
        FooterAction::Ai => sel!(aiFooterAction:),
        FooterAction::Apply => sel!(applyFooterAction:),
        FooterAction::Replace => sel!(replaceFooterAction:),
        FooterAction::Append => sel!(appendFooterAction:),
        FooterAction::Copy => sel!(copyFooterAction:),
        FooterAction::Expand => sel!(expandFooterAction:),
        FooterAction::Retry => sel!(retryFooterAction:),
        FooterAction::Close => sel!(closeFooterAction:),
        FooterAction::Stop => sel!(stopFooterAction:),
        FooterAction::PasteResponse => sel!(pasteResponseFooterAction:),
        FooterAction::Cwd => sel!(cwdFooterAction:),
        FooterAction::AgentModel => sel!(agentModelFooterAction:),
        FooterAction::Tips => sel!(tipsFooterAction:),
    }
}

#[cfg(target_os = "macos")]
fn footer_action_target_class() -> *const objc::runtime::Class {
    use std::sync::OnceLock;

    use objc::declare::ClassDecl;
    use objc::runtime::{Object, Sel};
    use objc::{class, sel, sel_impl};

    static CLASS: OnceLock<usize> = OnceLock::new();

    // SAFETY: ObjC class registration is serialized by `OnceLock`. Superclass
    // is `NSObject`; installed action methods match AppKit target/action ABI.
    *CLASS.get_or_init(|| unsafe {
        let superclass = class!(NSObject);
        let Some(mut decl) = ClassDecl::new("ScriptKitFooterActionTarget", superclass) else {
            return class!(NSObject) as *const _ as usize;
        };
        decl.add_method(
            sel!(runFooterAction:),
            footer_run_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(actionsFooterAction:),
            footer_actions_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(aiFooterAction:),
            footer_ai_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(applyFooterAction:),
            footer_apply_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(replaceFooterAction:),
            footer_replace_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(appendFooterAction:),
            footer_append_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(copyFooterAction:),
            footer_copy_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(expandFooterAction:),
            footer_expand_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(retryFooterAction:),
            footer_retry_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(closeFooterAction:),
            footer_close_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(stopFooterAction:),
            footer_stop_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(pasteResponseFooterAction:),
            footer_paste_response_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(cwdFooterAction:),
            footer_cwd_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(agentModelFooterAction:),
            footer_agent_model_action as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(tipsFooterAction:),
            footer_tips_action as extern "C" fn(&Object, Sel, id),
        );
        decl.register() as *const _ as usize
    }) as *const objc::runtime::Class
}

#[cfg(target_os = "macos")]
extern "C" fn footer_run_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Run);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_actions_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Actions);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_ai_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Ai);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_apply_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Apply);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_replace_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Replace);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_append_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Append);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_copy_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Copy);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_expand_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Expand);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_retry_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Retry);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_close_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::Close);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_stop_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Stop);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_paste_response_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::PasteResponse);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_cwd_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Cwd);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_agent_model_action(
    _this: &objc::runtime::Object,
    _: objc::runtime::Sel,
    sender: id,
) {
    send_footer_action_from_sender(sender, FooterAction::AgentModel);
}

#[cfg(target_os = "macos")]
extern "C" fn footer_tips_action(_this: &objc::runtime::Object, _: objc::runtime::Sel, sender: id) {
    send_footer_action_from_sender(sender, FooterAction::Tips);
}
