//! Source-level contracts for the Current App Commands built-in route.

const SOURCE: &str = include_str!("../src/render_builtins/current_app_commands.rs");

fn production_source() -> &'static str {
    SOURCE
        .split("#[cfg(test)]")
        .next()
        .expect("production source should exist")
}

#[test]
fn current_app_commands_uses_shared_chrome_text_tokens() {
    let source = production_source();
    assert!(
        source.contains("AppChromeColors::from_theme(&self.theme)"),
        "Current App Commands should resolve route chrome through shared AppChromeColors"
    );
    for needle in [
        "self.theme.colors.text.dimmed",
        "self.theme.colors.text.muted",
        "let text_dimmed",
        "let text_muted",
    ] {
        assert!(
            !source.contains(needle),
            "Current App Commands should not use pre-dimmed local text colors: {needle}"
        );
    }
    assert!(source.contains("chrome.text_primary_hex"));
    assert!(source.contains("chrome.text_hint_rgba"));
}

#[test]
fn current_app_commands_routes_actions_before_local_keys_and_consumes_handled_routes() {
    let source = production_source();
    let route = source
        .find("route_key_to_actions_dialog")
        .expect("Current App Commands should route actions popup keys");
    let escape = source
        .find("if is_key_escape(key)")
        .expect("Current App Commands should handle Escape");
    let cmd_w = source
        .find("key.eq_ignore_ascii_case(\"w\")")
        .expect("Current App Commands should handle Cmd+W");

    assert!(
        route < escape && route < cmd_w,
        "Current App Commands should route popup-owned keys before local Escape/Cmd+W"
    );
    for needle in [
        "ActionsRoute::Handled =>",
        "ActionsRoute::Execute {\n                        action_id,\n                        should_close,\n                    } =>",
        "cx.stop_propagation();\n                        return;",
    ] {
        assert!(
            source.contains(needle),
            "Current App Commands actions-popup route should consume handled branches: {needle}"
        );
    }
}

#[test]
fn current_app_commands_row_clicks_follow_launcher_activation_contract() {
    let source = production_source();
    for needle in [
        "event.click_count()",
        "should_submit_selected_row_click",
        "*selected_index = ix;",
        "cx.stop_propagation();",
    ] {
        assert!(
            source.contains(needle),
            "Current App Commands row clicks should select first and submit through launcher helper: {needle}"
        );
    }
}

#[test]
fn current_app_commands_header_uses_bounded_flex_for_filter_and_count() {
    let source = production_source();
    for needle in [
        ".flex_1()",
        ".min_w(px(0.))",
        ".flex_none()",
        ".whitespace_nowrap()",
        "chrome.text_hint_rgba",
    ] {
        assert!(
            source.contains(needle),
            "Current App Commands header should keep filter/count layout bounded: {needle}"
        );
    }
}

#[test]
fn current_app_commands_uses_vendor_scrollbar() {
    let source = production_source();
    assert!(
        source.contains("builtin_uniform_list_scrollbar("),
        "Current App Commands should attach the shared vendor scrollbar"
    );
}
