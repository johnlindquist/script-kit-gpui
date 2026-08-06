//! Source-level contract for dictation overlay focus and hidden-main behavior.
//!
//! The dictation overlay must be able to appear while Script Kit's main panel
//! remains hidden, without activating the app or briefly flashing the launcher.

const DICTATION_WINDOW: &str = include_str!("../src/dictation/window.rs");
const FOOTER_CHROME: &str = include_str!("../src/components/footer_chrome.rs");
const FOOTER_POPUP: &str = include_str!("../src/footer_popup.rs");

fn section_between<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    let start_idx = source.find(start).expect("section start must exist");
    let tail = &source[start_idx..];
    let end_idx = tail
        .find(end)
        .map(|idx| start_idx + idx)
        .unwrap_or(source.len());
    &source[start_idx..end_idx]
}

fn compact(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn dictation_overlay_opens_without_revealing_hidden_main_panel() {
    let section = section_between(
        DICTATION_WINDOW,
        "pub fn open_dictation_overlay(",
        "pub fn update_dictation_overlay(",
    );
    let compact_section = compact(section);

    assert!(
        compact_section.contains(&compact(
            "let main_was_visible = crate::is_main_window_visible();"
        )),
        "overlay must snapshot main-window visibility before native window operations"
    );
    let visibility_snapshot_pos = section
        .find("let main_was_visible = crate::is_main_window_visible();")
        .expect("overlay must snapshot main visibility before native window creation");
    let open_window_pos = section
        .find(".open_window(window_options")
        .expect("overlay must create the native window through GPUI open_window");
    assert!(
        visibility_snapshot_pos < open_window_pos,
        "main visibility must be captured before opening the overlay window"
    );
    assert!(
        compact_section.contains(&compact("focus: false,")),
        "overlay must not activate the app when it opens"
    );
    assert!(
        compact_section.contains(&compact("show: false,")),
        "overlay must be created hidden so GPUI does not surface sibling launcher panels"
    );
    assert!(
        compact_section.contains(&compact("kind: gpui::WindowKind::PopUp,")),
        "overlay must stay on GPUI's nonactivating popup window path"
    );

    let order_out_pos = section
        .find("msg_send![main_window, orderOut: cocoa::base::nil]")
        .expect("overlay must order out the hidden main panel before surfacing");
    let order_front_pos = section
        .find("msg_send![ns_window, orderFrontRegardless]")
        .expect("overlay must order itself front without activation");
    assert!(
        order_out_pos < order_front_pos,
        "hidden-main orderOut must happen before overlay orderFrontRegardless"
    );
}

#[test]
fn dictation_overlay_renders_visible_shortcut_rail() {
    assert!(
        DICTATION_WINDOW.contains("pub(crate) const OVERLAY_WIDTH_PX: f32 = 560.0;")
            && DICTATION_WINDOW.contains("pub(crate) const OVERLAY_HEIGHT_PX: f32 = 100.0;"),
        "dictation overlay must reserve enough room for the header row, caption band, and action chips"
    );
    assert!(
        DICTATION_WINDOW.contains("const ACTION_STOP_LABEL: &str = \"Stop\";")
            && DICTATION_WINDOW.contains("const ACTION_MIC_LABEL: &str = \"Select Mic\";")
            && DICTATION_WINDOW.contains("const ACTION_CANCEL_LABEL: &str = \"Cancel\";")
            && DICTATION_WINDOW.contains("const ACTION_CONTINUE_LABEL: &str = \"Continue\";")
            && DICTATION_WINDOW.contains("const ACTION_CLOSE_LABEL: &str = \"Close\";")
            && DICTATION_WINDOW.contains("const ESC_KEYCAP: &str = \"esc\";")
            && DICTATION_WINDOW.contains("const ENTER_KEYCAP: &str = \"\\u{21b5}\";")
            && DICTATION_WINDOW.contains(
                "const MIC_KEYCAP: &str = crate::components::footer_chrome::FOOTER_MIC_ICON_TOKEN;"
            ),
        "recording, confirming, and terminal phases must use compact action labels plus keycaps"
    );
    assert!(
        DICTATION_WINDOW.contains("fn dictation_native_footer_config(")
            && DICTATION_WINDOW.contains(".id(\"dictation-action-rail\")")
            && DICTATION_WINDOW.contains("native_footer_spacer()")
            && DICTATION_WINDOW.contains("render_footer_hint_action_button_frame(")
            && DICTATION_WINDOW
                .contains("render_footer_action_rail(\"dictation-action-rail\", frames)")
            && DICTATION_WINDOW
                .contains("render_static_action_rail(footer_config.buttons, Some(cx))"),
        "runtime must reserve the native footer slot while preview keeps the compact action rail"
    );
    assert!(
        DICTATION_WINDOW.contains("crate::components::footer_chrome::footer_rail_chrome")
            && DICTATION_WINDOW.contains("sync_window_footer_popup(")
            && DICTATION_WINDOW.contains("dictation_native_footer_config(")
            && !DICTATION_WINDOW.contains(".bg(rgba(rail_chrome.surface_rgba))")
            && !DICTATION_WINDOW.contains("rgba(rail_chrome.divider_rgba)")
            && DICTATION_WINDOW.contains("fn native_footer_spacer()"),
        "dictation action rail must reserve shared footer height without painting a local material or divider"
    );
    assert!(
        DICTATION_WINDOW.contains("render_footer_hint_action_button_frame(")
            && DICTATION_WINDOW.contains("FooterHintActionButtonFrameSpec")
            && DICTATION_WINDOW.contains("footer_button_height(")
            && DICTATION_WINDOW.contains("render_footer_action_rail(")
            && !DICTATION_WINDOW.contains("render_inline_shortcut_keys("),
        "dictation footer actions must render entirely through the shared footer frame and rail owners"
    );
    assert!(
        DICTATION_WINDOW.contains(
            "pub(crate) const DICTATION_OVERLAY_FOOTER_SURFACE: &str = \"dictation_overlay\";"
        ),
        "live dictation overlay should keep its own local footer/debug surface identity"
    );
    assert!(
        DICTATION_WINDOW.contains("dictation_footer_action_channel")
            && DICTATION_WINDOW.contains("MainWindowFooterConfig")
            && !DICTATION_WINDOW.contains("active_main_window_footer_surface")
            && DICTATION_WINDOW.contains("FooterAction::Stop")
            && DICTATION_WINDOW.contains("FooterAction::Close"),
        "dictation overlay must reuse the native footer renderer while keeping action routing on a dictation-specific channel"
    );
    // FOOTER_HINT_FONT_SIZE_PX was retired when hint font sizing moved to
    // per-spec `label_font_size_px`/`keycap_font_size_px` options; the mic
    // icon path became a plain literal. The invariant is the shared tokens,
    // not their historical shapes.
    assert!(
        FOOTER_CHROME
            .contains("pub(crate) const FOOTER_HINT_FONT_WEIGHT_APPKIT: f64 = 0.14;")
            && FOOTER_CHROME.contains(
                "pub(crate) const FOOTER_HINT_FONT_WEIGHT_GPUI: FontWeight = FontWeight(500.0);"
            )
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_KEYCAP_HEIGHT_PX: f32 = 20.0;")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_KEY_GLYPH_NUDGE_Y_PX: f32 = 1.0;")
            && FOOTER_CHROME
                .contains("pub(crate) const FOOTER_RETURN_GLYPH_NUDGE_Y_PX: f32 = 1.0;")
            && FOOTER_CHROME
                .contains("pub(crate) const FOOTER_SEMICOLON_GLYPH_NUDGE_Y_PX: f32 = -1.0;")
            && FOOTER_CHROME
                .contains("pub(crate) const FOOTER_BUTTON_VERTICAL_INSET_PX: f32 = 2.0;")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_CHIP_BORDER_ALPHA: f32 = 0.18;")
            && FOOTER_CHROME
                .contains("pub(crate) const FOOTER_CHIP_BORDER_HOVER_ALPHA: f32 = 0.34;")
            && FOOTER_CHROME
                .contains("pub(crate) const FOOTER_KEY_ANCHORED_CONTENT_PADDING_X_PX: f32 = 6.0;")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_MIC_ICON_TOKEN: &str = \"mic\";")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_MIC_ICON_PATH: &str =")
            && FOOTER_CHROME.contains("opacity.selected.max(FOOTER_CHIP_BORDER_SELECTED_ALPHA)")
            && FOOTER_CHROME.contains("pub(crate) fn footer_button_height(footer_height: f32)")
            && FOOTER_CHROME.contains("pub(crate) fn footer_rail_chrome(theme: &Theme)")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_ACTION_ITEM_GAP_PX")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_ACTION_CONTENT_GAP_PX")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_ACTION_CONTENT_PADDING_X_PX")
            && FOOTER_CHROME.contains("pub(crate) const FOOTER_ACTION_BUTTON_RADIUS_PX")
            && FOOTER_CHROME
                .contains("metrics.key_glyph_nudge_y + metrics.return_glyph_nudge_y"),
        "shared footer chrome tokens must pin native font, keycap, rail, and button chrome contracts"
    );
    assert!(
        FOOTER_CHROME.contains(
            "crate::components::hint_strip::shortcut_tokens_from_hint(shortcut)"
        ) && FOOTER_CHROME.contains("fn render_footer_labelcap(")
            && !FOOTER_CHROME.contains("footer_labelcap_border_color(theme)")
            && !FOOTER_CHROME.contains("FOOTER_LABELCAP_BORDER_ALPHA")
            && !FOOTER_CHROME.contains(".bg(footer_keycap_bg_color(theme))")
            && !FOOTER_CHROME.contains("FOOTER_KEYCAP_BG_ALPHA")
            && FOOTER_CHROME.contains(".flex_none()")
            && FOOTER_CHROME.contains(".min_h(px(metrics.keycap_height))")
            && FOOTER_CHROME.contains(".line_height(px(metrics.keycap_height))")
            && FOOTER_CHROME.contains(".group_hover(\"footer-action-button\""),
        "shared footer keycaps must use the escape glyph, runtime footer metrics, labelcap balance, hover foreground, and no steady-state fill"
    );
    assert!(
        DICTATION_WINDOW.contains("FooterAction::Stop,")
            && DICTATION_WINDOW.contains("FooterAction::Ai,")
            && DICTATION_WINDOW.contains("MIC_KEYCAP,")
            && DICTATION_WINDOW.contains("active_microphone_footer_label(),")
            && DICTATION_WINDOW.contains("fn active_microphone_footer_label() -> SharedString")
            && DICTATION_WINDOW.contains("crate::dictation::get_active_dictation_device()")
            && DICTATION_WINDOW.contains("FooterAction::Close,")
            && DICTATION_WINDOW.contains("self.submit_overlay_session(window, cx)")
            && DICTATION_WINDOW.contains("self.open_microphone_picker(window, cx)")
            && DICTATION_WINDOW.contains("self.abort_overlay_session(window, cx)"),
        "recording Stop, Mic, and Cancel controls must be native footer buttons routed into the overlay"
    );
    let footer_config_source = &DICTATION_WINDOW[DICTATION_WINDOW
        .find("pub(crate) fn dictation_native_footer_config")
        .expect("dictation footer config must exist")..];
    let recording_footer = section_between(
        footer_config_source,
        "DictationSessionPhase::Recording => {",
        "DictationSessionPhase::Confirming => {",
    );
    let mic_pos = recording_footer
        .find("FooterAction::Ai,")
        .expect("recording footer must include the mic action");
    let stop_pos = recording_footer
        .find("FooterAction::Stop,")
        .expect("recording footer must include Stop after mic");
    let cancel_pos = recording_footer
        .find("FooterAction::Close,")
        .expect("recording footer must include Cancel after Stop");
    assert!(
        mic_pos < stop_pos && stop_pos < cancel_pos,
        "recording footer order must keep the mic glyph button at the far left"
    );
    assert!(
        FOOTER_POPUP.contains("fn is_footer_left_pinned_button(")
            && FOOTER_POPUP.contains(
                "matches!(button_cfg.placement, FooterPlacement::Leading)"
            )
            && !FOOTER_POPUP.contains("&& button_cfg.label.as_ref().is_empty()")
            && FOOTER_POPUP.contains("fn footer_hint_content_layout_for_button(")
            && FOOTER_POPUP.contains("let label_x = (key_x + key_width + gap_width).round();")
            && FOOTER_POPUP.contains("if is_footer_left_pinned_button(button_cfg)")
            && FOOTER_POPUP.contains("setContentTintColor: text_color")
            && FOOTER_POPUP.contains("setAlphaValue: 1.0_f64")
            && FOOTER_POPUP.contains("setImageScaling: 0usize"),
        "native footer must pin the dictation mic glyph+label to x=0, render icon before text, scale it down into the keycap, and tint the icon like other footer glyphs"
    );
    assert!(
        FOOTER_POPUP.contains("isKindOfClass: class!(NSImageView)")
            && FOOTER_POPUP.contains("setContentTintColor: color"),
        "native footer image glyphs must use the same recursive opacity/tint updates as text key glyphs"
    );
    assert!(
        FOOTER_POPUP.contains("fn resolve_native_footer_visual_theme(")
            && FOOTER_POPUP.contains("footer_keycap_border_alpha_for_state(theme, state)")
            && FOOTER_POPUP.contains("visual_theme.border_alpha(state)")
            && FOOTER_POPUP.contains("set_footer_button_border(")
            && FOOTER_POPUP.contains("setBorderColor: cg_border"),
        "native footer chip borders must be visible at rest and strengthen on hover/selected states"
    );
    assert!(
        DICTATION_WINDOW.contains("fn open_microphone_picker")
            && DICTATION_WINDOW.contains("list_input_device_menu_items(selected_device_id)")
            && DICTATION_WINDOW.contains("build_dictation_microphone_popup_snapshot")
            && DICTATION_WINDOW.contains("sync_dictation_microphone_popup_window"),
        "overlay Select Mic control must use the shared microphone menu items and attached popup window"
    );
    assert!(
        DICTATION_WINDOW.contains("fn dictation_stop_keycap()")
            && DICTATION_WINDOW.contains("fn dictation_hotkey_keycap(")
            && DICTATION_WINDOW.contains(".get_dictation_hotkey()")
            && DICTATION_WINDOW.contains(".replace(\"Semicolon\", \";\")")
            && DICTATION_WINDOW.contains("FooterButtonConfig::new(")
            && DICTATION_WINDOW.contains("FooterAction::Stop,")
            && DICTATION_WINDOW.contains("dictation_stop_keycap(),")
            && DICTATION_WINDOW.contains("format!(\"Stop & {}\", target.descriptor().delivery_verb)")
            && !DICTATION_WINDOW.contains(
                "FooterButtonConfig::new(FooterAction::Stop, \"click\", ACTION_STOP_LABEL)"
            )
            && !DICTATION_WINDOW.contains("unwrap_or_else(|| \"again\".to_string())"),
        "recording Stop must show the configured dictation hotkey in the native footer instead of stale fallback labels"
    );

    let runtime_render = section_between(
        DICTATION_WINDOW,
        "impl Render for DictationOverlay",
        "/// Format the finished overlay state label.",
    );
    let preview_render = section_between(
        DICTATION_WINDOW,
        "pub(crate) fn render_dictation_overlay_state_preview",
        "fn render_static_target_badge_slot",
    );
    assert!(
        runtime_render.contains("DictationSessionPhase::Delivering")
            && runtime_render.contains("native_footer_spacer()")
            && DICTATION_WINDOW
                .contains("FooterButtonConfig::new(\n                FooterAction::Close"),
        "runtime Delivering state must reserve the shared native footer Close + esc action"
    );
    assert!(
        preview_render
            .contains("dictation_native_footer_config(&state.phase, armed, state.target)")
            && preview_render.contains("render_static_action_rail(footer_config.buttons, None)"),
        "preview phases must derive their footer actions from the same shared config as runtime"
    );
}
