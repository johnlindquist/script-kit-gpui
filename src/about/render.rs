#![allow(dead_code)]

use std::{
    rc::Rc,
    sync::{Arc, RwLock},
};

use gpui::{
    div, prelude::*, px, rgb, rgba, svg, App, ClickEvent, Div, FocusHandle, FontWeight,
    IntoElement, KeyDownEvent, Window,
};
use gpui_component::scroll::ScrollableElement;

use crate::{
    about::AboutState,
    branding,
    components::{
        NonListComposition, NonListCompositionOwner, NonListDensity, NonListMetrics, NonListPalette,
    },
    theme,
    ui::chrome,
    updates::UpdateState,
};

pub type AboutClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
pub type AboutKeyHandler = Rc<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>;

#[derive(Clone)]
pub struct AboutSurfaceActions {
    pub dismiss: AboutClickHandler,
    pub open_github: AboutClickHandler,
    pub open_discord: AboutClickHandler,
    pub follow_x: AboutClickHandler,
    pub check_updates: AboutClickHandler,
    pub open_release: AboutClickHandler,
    pub toggle_acknowledgements: AboutClickHandler,
    pub key_down: AboutKeyHandler,
}

fn is_about_activation_key(key: &str) -> bool {
    key == " "
        || key.eq_ignore_ascii_case("space")
        || key.eq_ignore_ascii_case("enter")
        || key.eq_ignore_ascii_case("return")
}

/// Render the launcher-native About surface opened from the tray menu.
pub(crate) fn render_about_surface(
    state: &AboutState,
    update_state: Arc<RwLock<UpdateState>>,
    focus: &FocusHandle,
    actions: AboutSurfaceActions,
    _window: &mut Window,
    _cx: &mut App,
) -> impl IntoElement {
    render_about_surface_inner(state, update_state, Some(focus), actions)
}

#[cfg(feature = "storybook")]
pub(crate) fn render_about_surface_preview(
    state: &AboutState,
    update_state: Arc<RwLock<UpdateState>>,
    actions: AboutSurfaceActions,
) -> gpui::AnyElement {
    render_about_surface_inner(state, update_state, None, actions).into_any_element()
}

fn render_about_surface_inner(
    state: &AboutState,
    update_state: Arc<RwLock<UpdateState>>,
    focus: Option<&FocusHandle>,
    actions: AboutSurfaceActions,
) -> impl IntoElement {
    let theme = theme::get_cached_theme();
    let chrome = theme::AppChromeColors::from_theme(&theme);
    let composition = NonListComposition::new(
        NonListCompositionOwner::About,
        NonListDensity::Comfortable,
        &theme,
    );
    let palette = composition.palette();
    let metrics = composition.metrics();
    let snapshot = update_state
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| UpdateState::Error {
            message: "update state unavailable".into(),
            failure: crate::updates::UpdateFailure::InvalidResponse,
        });
    let key_down = actions.key_down.clone();

    div()
        .id("about-surface")
        .when_some(focus, |surface, focus| surface.track_focus(focus))
        .size_full()
        .flex()
        .flex_col()
        .bg(rgba(chrome.surface_rgba))
        .capture_key_down(move |event, window, cx| key_down(event, window, cx))
        .child(render_header(chrome, actions.dismiss.clone()))
        .child(
            div()
                .id("about-content-scroll")
                .flex_1()
                .min_h(px(0.0))
                .px(px(32.0))
                .py(px(14.0))
                .flex()
                .flex_col()
                .items_center()
                .overflow_y_scrollbar()
                .child(
                    composition
                        .content_stack("about-non-list-content", 560.0, metrics.item_gap)
                        .items_center()
                        .child(render_logo_block(palette, metrics))
                        .child(render_title_version(chrome, palette))
                        .child(render_tagline(palette, metrics))
                        .child(render_creator_row(palette, metrics))
                        .child(render_quick_actions(
                            composition,
                            palette,
                            metrics,
                            &actions,
                        ))
                        .child(render_update_card(
                            composition,
                            palette,
                            metrics,
                            snapshot,
                            &actions,
                        ))
                        .child(render_acknowledgements(
                            composition,
                            palette,
                            metrics,
                            state,
                            &actions,
                        ))
                        .child(
                            composition
                                .footer_note("© John Lindquist · Built with GPUI")
                                .mt(px(20.0))
                                .h(px(28.0))
                                .flex()
                                .items_center(),
                        ),
                ),
        )
}

fn render_header(chrome: theme::AppChromeColors, dismiss: AboutClickHandler) -> Div {
    div()
        .h(px(52.0))
        .w_full()
        .px(px(16.0))
        .flex()
        .items_center()
        .justify_between()
        .border_b_1()
        .border_color(rgba(chrome.border_rgba))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(chrome.text_primary_hex))
                .child("About Script Kit"),
        )
        .child(
            div()
                .id("about-close-button")
                .tab_index(0)
                .size(px(28.0))
                .rounded(px(chrome::LIQUID_GLASS_CONTROL_RADIUS_PX))
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(16.0))
                .text_color(rgba(chrome.text_icon_rgba))
                .cursor_pointer()
                .hover(|style| style.bg(rgba(chrome.hover_rgba)))
                .focus_visible(|style| style.bg(rgba(chrome.hover_rgba)))
                .child("×")
                .on_click({
                    let dismiss = dismiss.clone();
                    move |event, window, cx| dismiss(event, window, cx)
                })
                .on_key_down(move |event: &KeyDownEvent, window, cx| {
                    if is_about_activation_key(event.keystroke.key.as_str()) {
                        let click_event = ClickEvent::default();
                        dismiss(&click_event, window, cx);
                        cx.stop_propagation();
                    } else {
                        cx.propagate();
                    }
                }),
        )
}

fn render_logo_block(palette: NonListPalette, metrics: NonListMetrics) -> Div {
    div()
        .size(px(56.0))
        .mb(px(metrics.item_gap))
        .rounded(px(metrics.card_radius))
        .border_1()
        .border_color(palette.border)
        .bg(palette.panel)
        .flex()
        .items_center()
        .justify_center()
        .child(
            svg()
                .path("logo.svg")
                .size(px(36.0))
                .text_color(palette.accent),
        )
}

fn render_title_version(chrome: theme::AppChromeColors, palette: NonListPalette) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .child(
            div()
                .text_size(px(28.0))
                .line_height(px(34.0))
                .font_weight(FontWeight::BOLD)
                .text_color(palette.title)
                .child(branding::APP_NAME),
        )
        .child(
            div()
                .h(px(20.0))
                .mt(px(6.0))
                .px(px(10.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(rgba(chrome.accent_badge_border_rgba))
                .bg(rgba(chrome.accent_badge_bg_rgba))
                .flex()
                .items_center()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(rgb(chrome.accent_badge_text_hex))
                .child(format!("v{}", env!("CARGO_PKG_VERSION"))),
        )
}

fn render_tagline(palette: NonListPalette, metrics: NonListMetrics) -> Div {
    div()
        .w(px(440.0))
        .max_w_full()
        .mt(px(6.0))
        .text_size(px(metrics.body_size))
        .line_height(px(metrics.body_line))
        .text_center()
        .text_color(palette.body)
        .child(branding::TAGLINE)
}

fn render_creator_row(palette: NonListPalette, metrics: NonListMetrics) -> Div {
    div()
        .mt(px(metrics.item_gap))
        .h(px(30.0))
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(
            div()
                .size(px(32.0))
                .rounded(px(999.0))
                .border_1()
                .border_color(palette.border)
                .bg(palette.panel)
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.0))
                .font_weight(FontWeight::BOLD)
                .text_color(palette.accent)
                .child("JL"),
        )
        .child(
            div()
                .text_size(px(14.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.body)
                .child("Created by John Lindquist"),
        )
}

fn render_quick_actions(
    composition: NonListComposition,
    palette: NonListPalette,
    metrics: NonListMetrics,
    actions: &AboutSurfaceActions,
) -> Div {
    composition
        .action_row(vec![
            action_button_with_min_width(
                "about-open-github",
                "Open GitHub repo",
                palette,
                metrics,
                actions.open_github.clone(),
                true,
                128.0,
            )
            .into_any_element(),
            action_button_with_min_width(
                "about-open-discord",
                "Open Discord",
                palette,
                metrics,
                actions.open_discord.clone(),
                true,
                128.0,
            )
            .into_any_element(),
            action_button_with_min_width(
                "about-follow-x",
                "Follow on X",
                palette,
                metrics,
                actions.follow_x.clone(),
                true,
                128.0,
            )
            .into_any_element(),
        ])
        .mt(px(metrics.item_gap))
        .max_w(px(metrics.max_width))
        .justify_center()
        .flex_wrap()
}

fn render_update_card(
    composition: NonListComposition,
    palette: NonListPalette,
    metrics: NonListMetrics,
    update_state: UpdateState,
    actions: &AboutSurfaceActions,
) -> impl IntoElement {
    let (status, label, enabled, handler) = match update_state {
        UpdateState::Idle => (
            format!("Version v{}", env!("CARGO_PKG_VERSION")),
            "Check for Updates",
            true,
            actions.check_updates.clone(),
        ),
        UpdateState::Checking { .. } => (
            "Checking…".to_string(),
            "Checking…",
            false,
            actions.check_updates.clone(),
        ),
        UpdateState::UpToDate => (
            "Up to date".to_string(),
            "Check for Updates",
            true,
            actions.check_updates.clone(),
        ),
        UpdateState::Available { release } => (
            format!("Update Available: v{}", release.version),
            "Open Release Page",
            true,
            actions.open_release.clone(),
        ),
        UpdateState::ReleaseNotReady {
            version, reason, ..
        } => (
            format!("Update v{version} found, but {}", reason.label()),
            "Open Release Page",
            true,
            actions.open_release.clone(),
        ),
        // KNOWN: Uses generic color; a proper semantic success design token requires a theme-API addition.
        UpdateState::Error { .. } => (
            "Couldn't check for updates".to_string(),
            "Check for Updates",
            true,
            actions.check_updates.clone(),
        ),
    };

    composition
        .card("about-update-card")
        .mt(px(metrics.item_gap))
        .max_w(px(metrics.max_width))
        .min_h(px(60.0))
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .flex_1()
                .min_w(px(0.0))
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(
                    div()
                        .text_size(px(14.0))
                        .line_height(px(20.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(palette.title)
                        .child("Updates"),
                )
                .child(
                    div()
                        .text_size(px(metrics.body_size))
                        .line_height(px(metrics.body_line))
                        .min_w(px(0.0))
                        .overflow_hidden()
                        .text_ellipsis()
                        .whitespace_nowrap()
                        .text_color(palette.body)
                        .child(status),
                ),
        )
        .child(action_button(
            "about-update-button",
            label,
            palette,
            metrics,
            handler,
            enabled,
        ))
}

fn render_acknowledgements(
    composition: NonListComposition,
    palette: NonListPalette,
    metrics: NonListMetrics,
    state: &AboutState,
    actions: &AboutSurfaceActions,
) -> impl IntoElement {
    composition
        .card("about-acknowledgements")
        .mt(px(metrics.item_gap))
        .max_w(px(metrics.max_width))
        .px(px(0.0))
        .py(px(0.0))
        .child(
            div()
                .id("about-acknowledgements-toggle")
                .tab_index(0)
                .h(px(34.0))
                .px(px(metrics.card_padding_x))
                .rounded(px(metrics.card_radius))
                .flex()
                .items_center()
                .justify_between()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .text_color(palette.title)
                .cursor_pointer()
                .hover(move |style| style.bg(palette.hover))
                .focus_visible(move |style| style.bg(palette.hover))
                .child("Acknowledgements")
                .child(if state.acks_open { "−" } else { "+" })
                .on_click({
                    let toggle = actions.toggle_acknowledgements.clone();
                    move |event, window, cx| toggle(event, window, cx)
                })
                .on_key_down({
                    let toggle = actions.toggle_acknowledgements.clone();
                    move |event: &KeyDownEvent, window, cx| {
                        if is_about_activation_key(event.keystroke.key.as_str()) {
                            let click_event = ClickEvent::default();
                            toggle(&click_event, window, cx);
                            cx.stop_propagation();
                        } else {
                            cx.propagate();
                        }
                    }
                }),
        )
        .when(state.acks_open, |container| {
            container.child(
                div()
                    .pt(px(8.0))
                    .px(px(metrics.card_padding_x))
                    .pb(px(10.0))
                    .text_size(px(metrics.body_size))
                    .line_height(px(metrics.body_line))
                    .text_color(palette.body)
                    .child("Powered by GPUI, ureq, tray-icon, resvg, and the Rust ecosystem."),
            )
        })
}

fn action_button(
    id: &'static str,
    label: &'static str,
    palette: NonListPalette,
    metrics: NonListMetrics,
    handler: AboutClickHandler,
    enabled: bool,
) -> impl IntoElement {
    action_button_with_min_width(id, label, palette, metrics, handler, enabled, 142.0)
}

fn action_button_with_min_width(
    id: &'static str,
    label: &'static str,
    palette: NonListPalette,
    _metrics: NonListMetrics,
    handler: AboutClickHandler,
    enabled: bool,
    min_width: f32,
) -> impl IntoElement {
    let mut label_element = div()
        .min_w(px(0.0))
        .overflow_hidden()
        .text_ellipsis()
        .whitespace_nowrap()
        .child(label);
    if enabled {
        label_element = label_element.cursor_pointer();
    }
    let mut button = div()
        .id(id)
        .h(px(34.0))
        .min_w(px(min_width))
        .px(px(12.0))
        .rounded(px(chrome::LIQUID_GLASS_COMPACT_RADIUS_PX))
        .border_1()
        .border_color(palette.border)
        .bg(palette.input)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::MEDIUM)
        .text_color(if enabled { palette.title } else { palette.hint })
        .child(label_element);

    if enabled {
        button = button
            .tab_index(0)
            .cursor_pointer()
            .hover(move |style| style.bg(palette.hover))
            .focus_visible(move |style| style.bg(palette.hover))
            .on_click({
                let handler = handler.clone();
                move |event, window, cx| handler(event, window, cx)
            })
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                if is_about_activation_key(event.keystroke.key.as_str()) {
                    let click_event = ClickEvent::default();
                    handler(&click_event, window, cx);
                    cx.stop_propagation();
                } else {
                    cx.propagate();
                }
            });
    } else {
        button = button.cursor_default();
    }

    button
}
