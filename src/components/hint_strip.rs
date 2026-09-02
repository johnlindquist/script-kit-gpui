#![allow(dead_code)]

use std::{
    collections::HashSet,
    rc::Rc,
    sync::{Mutex, OnceLock},
};

use gpui::{
    div, prelude::*, px, rgba, svg, AnyElement, App, ClickEvent, FontWeight, IntoElement,
    MouseButton, MouseMoveEvent, RenderOnce, SharedString, Window,
};

use crate::ui::chrome::{
    alpha_from_opacity, HINT_STRIP_HEIGHT, HINT_STRIP_PADDING_X, HINT_STRIP_PADDING_Y,
    HINT_TEXT_OPACITY,
};
use crate::ui_foundation::HexColorExt;

const HINT_STRIP_CONTENT_GAP: f32 = 8.0;

/// Padding inside each clickable hint button.
const HINT_BUTTON_PADDING_X: f32 = 4.0;
const HINT_BUTTON_PADDING_Y: f32 = 2.0;

/// Corner radius for hint button hover highlight — the canonical
/// hover-button radius shared by every hover-pill button.
const HINT_BUTTON_RADIUS: f32 = crate::ui::chrome::ACTION_BUTTON_RADIUS_PX;

/// Size for keyboard glyph icons in the hint strip.
/// Slightly larger than text_xs (12px) for visual clarity at hint opacity.
const KEY_ICON_SIZE: f32 = 14.0;
/// Optical Y nudge for the return icon in native/footer hint strips.
const RETURN_ICON_NUDGE_Y_PX: f32 = 6.0;

/// Gap between a key icon and its label text within a single hint.
const KEY_ICON_LABEL_GAP: f32 = 3.0;

/// Embedded asset paths for keyboard glyph SVGs, resolved by AppAssets via
/// `svg().path()`. Compile-time CARGO_MANIFEST_DIR filesystem paths broke in
/// released bundles — they point at the CI runner (P0 2026-06-11).
const RETURN_ICON_PATH: &str = "icons/return.svg";
const TAB_ICON_PATH: &str = "icons/tab.svg";
const COMMAND_ICON_PATH: &str = "icons/command.svg";
const SHIFT_ICON_PATH: &str = "icons/shift.svg";
const ESCAPE_ICON_PATH: &str = "icons/escape.svg";

const KEYCAP_PADDING_X: f32 = 6.0;
const KEYCAP_PADDING_Y: f32 = 1.0;
const KEYCAP_RADIUS: f32 = 5.0;
const KEYCAP_BG_OPACITY: f32 = 0.12;
const FOOTER_HINT_TEXT_SIZE: f32 = 12.5;

/// A click handler for a single hint action.
pub(crate) type HintClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

struct HintAction {
    action_id: SharedString,
    on_click: HintClickHandler,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct HintInteractionSnapshot {
    pub action_id: Option<SharedString>,
    pub pointer: bool,
    pub hover: bool,
    pub active: bool,
    pub clickable: bool,
}

fn required_hint_action_id(action_id: impl Into<SharedString>) -> SharedString {
    let action_id = action_id.into();
    assert!(
        !action_id.trim().is_empty(),
        "interactive hints require a non-empty stable action ID"
    );
    action_id
}

fn hint_interaction_snapshot(action: Option<&HintAction>) -> HintInteractionSnapshot {
    let interactive = action.is_some();
    HintInteractionSnapshot {
        action_id: action.map(|action| action.action_id.clone()),
        pointer: interactive,
        hover: interactive,
        active: interactive,
        clickable: interactive,
    }
}

#[derive(IntoElement)]
pub struct HintStrip {
    hints: Vec<SharedString>,
    leading: Option<AnyElement>,
    /// Interactive hints carry both a stable action identity and a required callback.
    /// Static hints keep this slot empty and receive no pointer-event chrome.
    actions: Vec<Option<HintAction>>,
}

impl HintStrip {
    pub fn new(hints: impl IntoHints) -> Self {
        let hints = hints.into_hints();
        let len = hints.len();
        Self {
            hints,
            leading: None,
            actions: std::iter::repeat_with(|| None).take(len).collect(),
        }
    }

    pub fn leading(mut self, leading: impl IntoElement) -> Self {
        self.leading = Some(leading.into_any_element());
        self
    }

    /// Make the hint at `index` interactive.
    ///
    /// Interactive hints require a stable action ID and a real callback. A hint
    /// without both remains static and receives no pointer/hover/active affordance.
    pub fn on_hint_click(
        mut self,
        index: usize,
        action_id: impl Into<SharedString>,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        if index < self.actions.len() {
            self.actions[index] = Some(HintAction {
                action_id: required_hint_action_id(action_id),
                on_click: Rc::new(handler),
            });
        }
        self
    }

    pub(crate) fn interaction_snapshots(&self) -> Vec<HintInteractionSnapshot> {
        self.actions
            .iter()
            .map(|action| hint_interaction_snapshot(action.as_ref()))
            .collect()
    }
}

pub trait IntoHints {
    fn into_hints(self) -> Vec<SharedString>;
}

impl IntoHints for Vec<SharedString> {
    fn into_hints(self) -> Vec<SharedString> {
        self
    }
}

impl IntoHints for SharedString {
    fn into_hints(self) -> Vec<SharedString> {
        vec![self]
    }
}

impl IntoHints for &str {
    fn into_hints(self) -> Vec<SharedString> {
        vec![self.to_string().into()]
    }
}

impl IntoHints for String {
    fn into_hints(self) -> Vec<SharedString> {
        vec![self.into()]
    }
}

fn text_color_with_opacity(primary: u32, opacity: f32) -> u32 {
    // Theme text colors are stored as 0xAARRGGBB; strip the original alpha, shift RGB into
    // RRGGBB00, then inject the requested alpha byte for gpui::rgba.
    ((primary & 0x00FF_FFFF) << 8) | alpha_from_opacity(opacity)
}

/// A parsed hint: either a text+shortcut pair or plain text.
enum HintElement {
    /// A text label paired with one or more trailing keyboard glyph icons or keycaps.
    KeyHint {
        parts: Vec<KeyHintPart>,
        label: SharedString,
    },
    /// Plain text (no icon).
    Text(SharedString),
}

enum KeyHintPart {
    Icon(&'static str),
    Keycap(SharedString),
}

fn icon_nudge_y(icon_path: &str) -> f32 {
    if icon_path == RETURN_ICON_PATH {
        RETURN_ICON_NUDGE_Y_PX
    } else {
        0.0
    }
}

// ─── Shared compact shortcut renderer ────────────────────────────────

const INLINE_SHORTCUT_GAP: f32 = 3.0;
const INLINE_SHORTCUT_ICON_SIZE: f32 = 12.0;
const INLINE_SHORTCUT_TEXT_SIZE: f32 = 11.0;
const INLINE_SHORTCUT_KEYCAP_PADDING_X: f32 = 4.0;
const INLINE_SHORTCUT_KEYCAP_PADDING_Y: f32 = 1.0;
const INLINE_SHORTCUT_KEYCAP_RADIUS: f32 = 4.0;

#[derive(Clone, Copy, Debug)]
pub(crate) struct InlineShortcutColors {
    pub glyph: gpui::Hsla,
    pub keycap_bg: gpui::Hsla,
    pub keycap_border: Option<gpui::Hsla>,
}

/// Shared whisper-chrome preset for compact inline shortcuts.
///
/// Produces ultra-low-opacity keycap backgrounds (0.08) with faint borders (0.18),
/// matching the whisper-chrome design language used in the footer hint strip.
/// All primary surfaces should use this instead of per-surface keycap opacity tuning.
#[inline]
pub(crate) fn whisper_inline_shortcut_colors(
    glyph: gpui::Hsla,
    chrome: gpui::Hsla,
    show_border: bool,
) -> InlineShortcutColors {
    let mut bg = chrome;
    bg.a = 0.08;
    let border = if show_border {
        let mut b = chrome;
        b.a = 0.18;
        Some(b)
    } else {
        None
    };
    InlineShortcutColors {
        glyph,
        keycap_bg: bg,
        keycap_border: border,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ShortcutChromeAudit {
    surface: &'static str,
    mode: &'static str,
}

fn seen_shortcut_chrome_audits() -> &'static Mutex<HashSet<ShortcutChromeAudit>> {
    static SEEN: OnceLock<Mutex<HashSet<ShortcutChromeAudit>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn emit_shortcut_chrome_audit(surface: &'static str, mode: &'static str) {
    let audit = ShortcutChromeAudit { surface, mode };
    let mut seen = seen_shortcut_chrome_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if seen.insert(audit.clone()) {
        tracing::info!(surface = surface, mode = mode, "shortcut_chrome_audit");
    }
}

enum InlineShortcutToken {
    Icon(&'static str),
    Text(SharedString),
    Keycap(SharedString),
}

fn is_symbol_shortcut_char(ch: char) -> bool {
    matches!(
        ch,
        '⌘' | '⌃'
            | '⌥'
            | '⇧'
            | '↵'
            | '↩'
            | '⏎'
            | '⎋'
            | '⇥'
            | '⌫'
            | '␣'
            | '↑'
            | '↓'
            | '←'
            | '→'
            | '⇞'
            | '⇟'
            | '↖'
            | '↘'
    )
}

fn normalize_shortcut_part(part: &str) -> String {
    match part.to_lowercase().as_str() {
        "cmd" | "command" | "meta" | "super" => "⌘".to_string(),
        "ctrl" | "control" => "⌃".to_string(),
        "alt" | "option" | "opt" => "⌥".to_string(),
        "shift" => "⇧".to_string(),
        "enter" | "return" => "↵".to_string(),
        "escape" | "esc" => "⎋".to_string(),
        "tab" => "⇥".to_string(),
        "space" => "␣".to_string(),
        "backspace" | "delete" => "⌫".to_string(),
        "up" | "arrowup" => "↑".to_string(),
        "down" | "arrowdown" => "↓".to_string(),
        "left" | "arrowleft" => "←".to_string(),
        "right" | "arrowright" => "→".to_string(),
        "pageup" => "⇞".to_string(),
        "pagedown" => "⇟".to_string(),
        "home" => "↖".to_string(),
        "end" => "↘".to_string(),
        "click" => "click".to_string(),
        other if other.chars().all(is_symbol_shortcut_char) => other.to_string(),
        other => other.to_uppercase(),
    }
}

fn plus_notation_parts(shortcut: &str) -> Vec<String> {
    let chars: Vec<char> = shortcut.chars().collect();
    let mut parts = Vec::new();
    let mut current = String::new();

    for (index, ch) in chars.iter().copied().enumerate() {
        if ch != '+' {
            current.push(ch);
            continue;
        }

        if !current.trim().is_empty() {
            parts.push(current.trim().to_string());
            current.clear();
        }

        // A plus with no following key is the key itself. This covers both
        // "⌘+" and the second plus in "cmd++" without manufacturing an
        // empty token.
        if index + 1 == chars.len() {
            parts.push("+".to_string());
        }
    }

    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }

    parts
}

pub(crate) fn shortcut_tokens_from_hint(shortcut: &str) -> Vec<String> {
    let trimmed = shortcut.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    // Config-style notation. Parse separators without replacing them so a
    // trailing or doubled plus remains a literal Plus keycap.
    if trimmed.contains('+') {
        return plus_notation_parts(trimmed)
            .into_iter()
            .map(|part| normalize_shortcut_part(&part))
            .collect();
    }

    // Space-delimited config notation ("cmd shift k").
    if trimmed.chars().any(char::is_whitespace) {
        return trimmed
            .split_whitespace()
            .map(normalize_shortcut_part)
            .collect();
    }

    // Display-style input: "⌘F1", "⌘PAGEUP", "⌃⌘↑".
    // Preserve contiguous text runs so multi-character keys stay grouped.
    if trimmed.chars().any(is_symbol_shortcut_char) {
        let mut tokens = Vec::new();
        let mut text_run = String::new();
        for ch in trimmed.chars() {
            if is_symbol_shortcut_char(ch) {
                if !text_run.is_empty() {
                    tokens.push(normalize_shortcut_part(&text_run));
                    text_run.clear();
                }
                tokens.push(ch.to_string());
            } else {
                text_run.push(ch);
            }
        }
        if !text_run.is_empty() {
            tokens.push(normalize_shortcut_part(&text_run));
        }
        return tokens;
    }

    vec![normalize_shortcut_part(trimmed)]
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct ShortcutNormalizationAudit {
    surface: &'static str,
    input: String,
    output: String,
}

fn seen_shortcut_normalization_audits() -> &'static Mutex<HashSet<ShortcutNormalizationAudit>> {
    static SEEN: OnceLock<Mutex<HashSet<ShortcutNormalizationAudit>>> = OnceLock::new();
    SEEN.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(crate) fn emit_shortcut_normalization_audit(surface: &'static str, input: &str, output: &str) {
    let audit = ShortcutNormalizationAudit {
        surface,
        input: input.to_string(),
        output: output.to_string(),
    };
    let mut seen = seen_shortcut_normalization_audits()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if seen.insert(audit.clone()) {
        tracing::info!(
            surface = surface,
            input = %input,
            output = %output,
            "shortcut_normalized"
        );
    }
}

#[inline]
pub(crate) fn compact_shortcut_display_string(shortcut: &str) -> String {
    shortcut_tokens_from_hint(shortcut).join("")
}

fn canonical_shortcut_part(part: &str) -> String {
    match part.trim().to_lowercase().as_str() {
        "⌘" | "cmd" | "command" | "meta" | "super" => "cmd".to_string(),
        "⌃" | "ctrl" | "control" => "ctrl".to_string(),
        "⌥" | "alt" | "option" | "opt" => "alt".to_string(),
        "⇧" | "shift" => "shift".to_string(),
        "↵" | "↩" | "⏎" | "enter" | "return" => "enter".to_string(),
        "⎋" | "escape" | "esc" => "escape".to_string(),
        "⇥" | "tab" => "tab".to_string(),
        "⌫" | "backspace" | "delete" => "backspace".to_string(),
        "␣" | "space" => "space".to_string(),
        "↑" | "up" | "arrowup" => "up".to_string(),
        "↓" | "down" | "arrowdown" => "down".to_string(),
        "←" | "left" | "arrowleft" => "left".to_string(),
        "→" | "right" | "arrowright" => "right".to_string(),
        "⇞" | "pageup" => "pageup".to_string(),
        "⇟" | "pagedown" => "pagedown".to_string(),
        "↖" | "home" => "home".to_string(),
        "↘" | "end" => "end".to_string(),
        other => other.to_string(),
    }
}

pub(crate) fn canonical_shortcut_hint(shortcut: &str) -> String {
    let tokens = shortcut_tokens_from_hint(shortcut);
    let mut modifiers = Vec::new();
    let mut key: Option<String> = None;

    for token in tokens {
        let canonical = canonical_shortcut_part(&token);
        match canonical.as_str() {
            "cmd" | "ctrl" | "alt" | "shift" => modifiers.push(canonical),
            _ => key = Some(canonical),
        }
    }

    modifiers.sort();
    if let Some(key) = key {
        modifiers.push(key);
    }
    modifiers.join("+")
}

fn inline_shortcut_token(token: &str) -> InlineShortcutToken {
    match token {
        "⌘" => InlineShortcutToken::Icon(COMMAND_ICON_PATH),
        "⇧" => InlineShortcutToken::Icon(SHIFT_ICON_PATH),
        "↵" | "↩" | "⏎" => InlineShortcutToken::Icon(RETURN_ICON_PATH),
        "⇥" => InlineShortcutToken::Icon(TAB_ICON_PATH),
        value if value.chars().count() > 1 => InlineShortcutToken::Keycap(value.to_string().into()),
        value => InlineShortcutToken::Text(value.to_uppercase().into()),
    }
}

pub(crate) fn render_inline_shortcut_keys<'a>(
    keys: impl IntoIterator<Item = &'a str>,
    colors: InlineShortcutColors,
) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(INLINE_SHORTCUT_GAP));
    let mut has_keys = false;

    for key in keys {
        has_keys = true;
        row = row.child(match inline_shortcut_token(key) {
            InlineShortcutToken::Icon(icon_path) => svg()
                .path(icon_path)
                .size(px(INLINE_SHORTCUT_ICON_SIZE))
                .flex_shrink_0()
                .mt(px(icon_nudge_y(icon_path)))
                .text_color(colors.glyph)
                .into_any_element(),
            InlineShortcutToken::Text(text) => div()
                .text_size(px(INLINE_SHORTCUT_TEXT_SIZE))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(colors.glyph)
                .child(text)
                .into_any_element(),
            InlineShortcutToken::Keycap(text) => {
                let mut chip = div()
                    .px(px(INLINE_SHORTCUT_KEYCAP_PADDING_X))
                    .py(px(INLINE_SHORTCUT_KEYCAP_PADDING_Y))
                    .rounded(px(INLINE_SHORTCUT_KEYCAP_RADIUS))
                    .bg(colors.keycap_bg)
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.glyph)
                    .child(text);
                if let Some(border) = colors.keycap_border {
                    chip = chip.border_1().border_color(border);
                }
                chip.into_any_element()
            }
        });
    }

    if has_keys {
        row.into_any_element()
    } else {
        div().into_any_element()
    }
}

#[cfg(test)]
mod inline_shortcut_tests {
    use super::{
        canonical_shortcut_hint, icon_nudge_y, shortcut_tokens_from_hint, COMMAND_ICON_PATH,
        RETURN_ICON_NUDGE_Y_PX, RETURN_ICON_PATH,
    };

    #[test]
    fn shortcut_tokens_handle_raw_symbol_and_literal_key_inputs() {
        let cases = [
            ("cmd+shift+k", vec!["⌘", "⇧", "K"]),
            ("cmd shift k", vec!["⌘", "⇧", "K"]),
            ("cmd+k", vec!["⌘", "K"]),
            ("cmd k", vec!["⌘", "K"]),
            ("⌘K", vec!["⌘", "K"]),
            ("⌃⌘↑", vec!["⌃", "⌘", "↑"]),
            ("cmd+pageup", vec!["⌘", "⇞"]),
            ("cmd+home", vec!["⌘", "↖"]),
            ("cmd++", vec!["⌘", "+"]),
            ("⌘+", vec!["⌘", "+"]),
            ("cmd+-", vec!["⌘", "-"]),
            ("ctrl+\\", vec!["⌃", "\\"]),
            ("F12", vec!["F12"]),
            ("Escape", vec!["⎋"]),
            ("Enter", vec!["↵"]),
        ];

        for (input, expected) in cases {
            assert_eq!(shortcut_tokens_from_hint(input), expected, "input={input}");
        }
    }

    #[test]
    fn shortcut_tokens_preserve_grouped_multi_char_keys() {
        // F-keys must stay grouped, not split into individual characters
        assert_eq!(shortcut_tokens_from_hint("⌘F1"), vec!["⌘", "F1"]);
        assert_eq!(shortcut_tokens_from_hint("⌘F12"), vec!["⌘", "F12"]);
        // PAGEUP normalizes to compact glyph
        assert_eq!(shortcut_tokens_from_hint("⌘PAGEUP"), vec!["⌘", "⇞"]);
        assert_eq!(shortcut_tokens_from_hint("⌘PAGEDOWN"), vec!["⌘", "⇟"]);
        assert_eq!(shortcut_tokens_from_hint("⌘HOME"), vec!["⌘", "↖"]);
        assert_eq!(shortcut_tokens_from_hint("⌘END"), vec!["⌘", "↘"]);
    }

    #[test]
    fn canonical_shortcut_hint_normalizes_display_strings() {
        assert_eq!(canonical_shortcut_hint("⌘⇞"), "cmd+pageup");
        assert_eq!(canonical_shortcut_hint("⌘F1"), "cmd+f1");
        assert_eq!(canonical_shortcut_hint("⌃⇧↵"), "ctrl+shift+enter");
        assert_eq!(canonical_shortcut_hint("cmd+shift+k"), "cmd+shift+k");
        assert_eq!(canonical_shortcut_hint("cmd++"), "cmd++");
        assert_eq!(canonical_shortcut_hint("ctrl+\\"), "ctrl+\\");
    }

    #[test]
    fn shortcut_consumers_share_one_alias_and_literal_key_stream() {
        use crate::actions::{Action, ActionCategory, ActionsDialog};
        use crate::components::button::Button;
        use crate::components::unified_list_item::TrailingContent;

        for input in [
            "Cmd+K",
            "cmd+k",
            "⌘K",
            "cmd++",
            "ctrl+\\",
            "⌘F12",
            "cmd+pageup",
            "cmd+home",
            "Escape",
            "Enter",
        ] {
            let expected = shortcut_tokens_from_hint(input);
            assert_eq!(
                crate::components::footer_chrome::split_footer_shortcut(input),
                expected,
                "footer input={input}"
            );
            assert_eq!(
                Button::resolve_shortcut_tokens(input),
                expected,
                "button input={input}"
            );
            assert_eq!(
                ActionsDialog::parse_shortcut_keycaps(input),
                expected,
                "Actions input={input}"
            );

            let action =
                Action::new("test", "Test", None, ActionCategory::GlobalOps).with_shortcut(input);
            assert_eq!(
                action.shortcut_tokens.as_deref(),
                Some(expected.as_slice()),
                "action model input={input}"
            );

            match TrailingContent::shortcut(input) {
                TrailingContent::Shortcut { tokens, .. } => assert_eq!(
                    tokens.as_ref(),
                    expected.as_slice(),
                    "unified row input={input}"
                ),
                _ => unreachable!("shortcut constructor always returns Shortcut"),
            }
        }

        let recorder = crate::components::shortcut_recorder::RecordedShortcut {
            cmd: true,
            key: Some("+".to_string()),
            ..Default::default()
        };
        assert_eq!(recorder.to_keycaps(), vec!["⌘", "+"]);
    }

    #[test]
    fn return_icon_gets_footer_optical_nudge() {
        assert_eq!(RETURN_ICON_NUDGE_Y_PX, 6.0);
        assert_eq!(icon_nudge_y(RETURN_ICON_PATH), 6.0);
        assert_eq!(icon_nudge_y(COMMAND_ICON_PATH), 0.0);
    }
}

// ─── End shared compact shortcut renderer ────────────────────────────

fn is_boundary_or_end(rest: &str) -> bool {
    rest.is_empty() || rest.chars().next().is_some_and(char::is_whitespace)
}

/// Parse a hint string and extract a leading keyboard glyph if present.
///
/// Recognized patterns (all map to SVG icons, rendered after the label):
/// - `"↵ Run"`, `"⏎ Send"`, `"↩ Send"` → label + Return icon
/// - `"⌘K Actions"`, `"⌘⇧↵ Send"` → label + icon sequence
/// - `"⌘↵ AI"` → label + command + return icons
/// - `"Esc Back"` → label + Esc text keycap
fn parse_hint(hint: &str) -> HintElement {
    let mut rest = hint;
    let mut parts = Vec::new();

    loop {
        if let Some(next) = rest.strip_prefix('⌘') {
            parts.push(KeyHintPart::Icon(COMMAND_ICON_PATH));
            rest = next;
            continue;
        }

        if let Some(next) = rest.strip_prefix('⇧') {
            parts.push(KeyHintPart::Icon(SHIFT_ICON_PATH));
            rest = next;
            continue;
        }

        if let Some(next) = rest.strip_prefix("Tab") {
            if is_boundary_or_end(next) {
                parts.push(KeyHintPart::Icon(TAB_ICON_PATH));
                rest = next;
                continue;
            }
        }

        if let Some(next) = rest.strip_prefix("Esc") {
            if is_boundary_or_end(next) {
                parts.push(KeyHintPart::Keycap("Esc".into()));
                rest = next;
                continue;
            }
        }

        if let Some(next) = rest.strip_prefix('↵') {
            parts.push(KeyHintPart::Icon(RETURN_ICON_PATH));
            rest = next;
            continue;
        }

        if let Some(next) = rest.strip_prefix('\u{23CE}') {
            parts.push(KeyHintPart::Icon(RETURN_ICON_PATH));
            rest = next;
            continue;
        }

        if let Some(next) = rest.strip_prefix('\u{21A9}') {
            parts.push(KeyHintPart::Icon(RETURN_ICON_PATH));
            rest = next;
            continue;
        }

        // After a modifier (⌘, ⇧, etc.), a single uppercase letter followed by
        // a space or end-of-string is a key character, not part of the label.
        // e.g. "⌘K Actions" → [⌘ icon, K keycap] + label "Actions"
        if !parts.is_empty() {
            if let Some(ch) = rest.chars().next() {
                if ch.is_ascii_uppercase() {
                    let after = &rest[ch.len_utf8()..];
                    if after.is_empty() || after.starts_with(' ') {
                        parts.push(KeyHintPart::Keycap(ch.to_string().into()));
                        rest = after;
                        continue;
                    }
                }
            }
        }

        break;
    }

    if parts.is_empty() {
        return HintElement::Text(hint.to_string().into());
    }

    HintElement::KeyHint {
        parts,
        label: rest.trim_start().to_string().into(),
    }
}

/// Render a single hint element (text+shortcut or plain text) with a pre-computed RGBA color.
fn render_hint_element(element: HintElement, text_rgba: u32) -> AnyElement {
    render_hint_element_hsla(element, rgba(text_rgba).into())
}

/// Render a single hint element with an HSLA color.
fn render_hint_element_hsla(element: HintElement, color: gpui::Hsla) -> AnyElement {
    match element {
        HintElement::KeyHint { parts, label } => {
            let theme = crate::theme::get_cached_theme();
            let keycap_bg = theme.colors.text.primary.with_opacity(KEYCAP_BG_OPACITY);

            let mut hint_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(KEY_ICON_LABEL_GAP));

            if !label.is_empty() {
                hint_row = hint_row.child(
                    div()
                        .text_size(px(FOOTER_HINT_TEXT_SIZE))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(color)
                        .child(label),
                );
            }

            let mut keys_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(KEY_ICON_LABEL_GAP));

            for part in parts {
                keys_row = keys_row.child(match part {
                    KeyHintPart::Icon(icon_path) => svg()
                        .path(icon_path)
                        .size(px(KEY_ICON_SIZE))
                        .flex_shrink_0()
                        .mt(px(icon_nudge_y(icon_path)))
                        .text_color(color)
                        .into_any_element(),
                    KeyHintPart::Keycap(text) => div()
                        .px(px(KEYCAP_PADDING_X))
                        .py(px(KEYCAP_PADDING_Y))
                        .rounded(px(KEYCAP_RADIUS))
                        .bg(keycap_bg)
                        .text_size(px(FOOTER_HINT_TEXT_SIZE))
                        .font_weight(FontWeight::NORMAL)
                        .text_color(color)
                        .child(text)
                        .into_any_element(),
                });
            }

            hint_row = hint_row.child(keys_row);

            hint_row.into_any_element()
        }
        HintElement::Text(text) => div()
            .text_size(px(FOOTER_HINT_TEXT_SIZE))
            .font_weight(FontWeight::NORMAL)
            .text_color(color)
            .child(text)
            .into_any_element(),
    }
}

impl RenderOnce for HintStrip {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let theme = crate::theme::get_cached_theme();
        let chrome = crate::theme::AppChromeColors::from_theme(&theme);
        let text_rgba = text_color_with_opacity(theme.colors.text.primary, HINT_TEXT_OPACITY);
        let hover_bg = rgba(chrome.hover_rgba);
        let active_bg = rgba(chrome.selection_rgba);

        let has_interactive_hints = self.actions.iter().any(Option::is_some);
        let mut row = div()
            .id("hint-strip-footer")
            .w_full()
            .h(px(HINT_STRIP_HEIGHT))
            .px(px(HINT_STRIP_PADDING_X))
            .py(px(HINT_STRIP_PADDING_Y))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(HINT_STRIP_CONTENT_GAP));

        if has_interactive_hints {
            row = row
                .on_mouse_move(|_: &MouseMoveEvent, _window, cx| cx.stop_propagation())
                .on_mouse_down(MouseButton::Left, |_, _window, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _window, cx| cx.stop_propagation());
        }

        if let Some(leading) = self.leading {
            row = row.child(leading);
        }

        // Build the right-aligned hints container with icon-aware rendering.
        let mut hints_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(HINT_STRIP_CONTENT_GAP));

        for (index, (hint, action)) in self.hints.iter().zip(self.actions).enumerate() {
            let element = parse_hint(hint.as_ref());
            let hint_content = render_hint_element(element, text_rgba);

            if let Some(action) = action {
                let HintAction {
                    action_id,
                    on_click,
                } = action;
                let debug_selector = action_id.clone();
                let button = div()
                    .id(action_id)
                    .debug_selector(move || debug_selector.to_string())
                    .cursor_pointer()
                    .px(px(HINT_BUTTON_PADDING_X))
                    .py(px(HINT_BUTTON_PADDING_Y))
                    .rounded(px(HINT_BUTTON_RADIUS))
                    .hover(move |s| s.bg(hover_bg))
                    .active(move |s| s.bg(active_bg))
                    .on_click(move |event, window, cx| (on_click)(event, window, cx))
                    .child(hint_content);
                hints_row = hints_row.child(button);
            } else {
                let debug_selector = format!("hint-static-{index}");
                hints_row = hints_row.child(
                    div()
                        .debug_selector(move || debug_selector.clone())
                        .child(hint_content),
                );
            }
        }

        row.child(div().flex_1()).child(hints_row)
    }
}

/// Render static icon-aware hint content in a flex row.
///
/// Static hints explain keyboard grammar but are not controls: they intentionally
/// have no pointer cursor, hover/active paint, click handler, or action identity.
pub fn render_static_hint_icons(hints: &[&str], text_rgba: u32) -> AnyElement {
    render_static_hint_icons_hsla(hints, rgba(text_rgba).into())
}

/// Like [`render_static_hint_icons`] but accepts an HSLA color directly.
pub fn render_static_hint_icons_hsla(hints: &[&str], color: gpui::Hsla) -> AnyElement {
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HINT_STRIP_CONTENT_GAP));

    for hint in hints {
        let element = parse_hint(hint);
        row = row.child(render_hint_element_hsla(element, color));
    }

    row.into_any_element()
}

/// An interactive hint with stable action identity and a required callback.
pub struct ClickableHint {
    pub action_id: SharedString,
    pub label: SharedString,
    pub on_click: HintClickHandler,
}

impl ClickableHint {
    pub fn new(
        action_id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            action_id: required_hint_action_id(action_id),
            label: label.into(),
            on_click: Rc::new(on_click),
        }
    }

    pub(crate) fn interaction_snapshot(&self) -> HintInteractionSnapshot {
        HintInteractionSnapshot {
            action_id: Some(self.action_id.clone()),
            pointer: true,
            hover: true,
            active: true,
            clickable: true,
        }
    }
}

/// Render clickable hint icons with per-hint click handlers.
///
/// Each [`ClickableHint`] renders as a clickable button with ghost-bg hover.
pub fn render_hint_icons_clickable(hints: Vec<ClickableHint>, text_rgba: u32) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
    let hover_bg = rgba(chrome.hover_rgba);
    let active_bg = rgba(chrome.selection_rgba);
    let color: gpui::Hsla = rgba(text_rgba).into();

    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HINT_STRIP_CONTENT_GAP));

    for hint in hints {
        let element = parse_hint(hint.label.as_ref());
        let hint_content = render_hint_element_hsla(element, color);
        let on_click = hint.on_click;
        let debug_selector = hint.action_id.clone();

        row = row.child(
            div()
                .id(hint.action_id)
                .debug_selector(move || debug_selector.to_string())
                .cursor_pointer()
                .px(px(HINT_BUTTON_PADDING_X))
                .py(px(HINT_BUTTON_PADDING_Y))
                .rounded(px(HINT_BUTTON_RADIUS))
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
                .on_click(move |event, window, cx| (on_click)(event, window, cx))
                .child(hint_content),
        );
    }

    row.into_any_element()
}

/// An interactive hint with stable identity, required callback, and toggle state.
pub struct SelectableHint {
    pub action_id: SharedString,
    pub label: SharedString,
    pub on_click: HintClickHandler,
    pub selected: bool,
}

impl SelectableHint {
    pub fn new(
        action_id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            action_id: required_hint_action_id(action_id),
            label: label.into(),
            on_click: Rc::new(on_click),
            selected: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub(crate) fn interaction_snapshot(&self) -> HintInteractionSnapshot {
        HintInteractionSnapshot {
            action_id: Some(self.action_id.clone()),
            pointer: true,
            hover: true,
            active: true,
            clickable: true,
        }
    }
}

/// Render hint icons with per-hint click handlers and optional selected state.
///
/// Selected hints show a persistent `selection_rgba` background (like the Agent Chat
/// chat Actions toggle). Uses the same icon-aware rendering as the Agent Chat footer.
pub fn render_selectable_hint_icons(hints: Vec<SelectableHint>, text_rgba: u32) -> AnyElement {
    let theme = crate::theme::get_cached_theme();
    let chrome = crate::theme::AppChromeColors::from_theme(&theme);
    let hover_bg = rgba(chrome.hover_rgba);
    let active_bg = rgba(chrome.selection_rgba);
    let color: gpui::Hsla = rgba(text_rgba).into();

    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(HINT_STRIP_CONTENT_GAP));

    for hint in hints {
        let element = parse_hint(hint.label.as_ref());
        let hint_content = render_hint_element_hsla(element, color);
        let is_selected = hint.selected;
        let on_click = hint.on_click;
        let debug_selector = hint.action_id.clone();

        row = row.child(
            div()
                .id(hint.action_id)
                .debug_selector(move || debug_selector.to_string())
                .cursor_pointer()
                .px(px(HINT_BUTTON_PADDING_X))
                .py(px(HINT_BUTTON_PADDING_Y))
                .rounded(px(HINT_BUTTON_RADIUS))
                .when(is_selected, |d| d.bg(active_bg))
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
                .on_click(move |event, window, cx| (on_click)(event, window, cx))
                .child(hint_content),
        );
    }

    row.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Context, Modifiers, Render, TestAppContext, VisualTestContext};
    use std::cell::Cell;

    struct HintStripPointerProbe {
        clicks: Rc<Cell<usize>>,
    }

    impl Render for HintStripPointerProbe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let clicks = self.clicks.clone();
            HintStrip::new(vec!["Type to Search".into(), "↵ Run".into()]).on_hint_click(
                1,
                "probe-run-action",
                move |_, _, _| clicks.set(clicks.get() + 1),
            )
        }
    }

    fn assert_static(snapshot: &HintInteractionSnapshot) {
        assert_eq!(snapshot.action_id, None);
        assert!(!snapshot.pointer);
        assert!(!snapshot.hover);
        assert!(!snapshot.active);
        assert!(!snapshot.clickable);
    }

    fn assert_interactive(snapshot: &HintInteractionSnapshot, action_id: &str) {
        assert!(snapshot
            .action_id
            .as_ref()
            .is_some_and(|id| id.as_ref() == action_id));
        assert!(snapshot.pointer);
        assert!(snapshot.hover);
        assert!(snapshot.active);
        assert!(snapshot.clickable);
    }

    #[test]
    fn static_hint_strip_has_no_pointer_or_action_semantics() {
        let strip = HintStrip::new(vec!["Type to Search".into(), "↑↓ Navigate".into()]);
        let snapshots = strip.interaction_snapshots();
        assert_eq!(snapshots.len(), 2);
        snapshots.iter().for_each(assert_static);
    }

    #[test]
    fn real_gpui_dispatch_keeps_static_hint_inert_and_fires_action_once() {
        let clicks = Rc::new(Cell::new(0));
        let probe_clicks = clicks.clone();
        let mut cx = TestAppContext::single();
        let window = cx.add_window(move |_, _| HintStripPointerProbe {
            clicks: probe_clicks,
        });
        let mut vcx = VisualTestContext::from_window(window.into(), &cx);
        vcx.run_until_parked();

        let static_bounds = vcx
            .debug_bounds("hint-static-0")
            .expect("static hint should publish measurement bounds");
        vcx.simulate_click(static_bounds.center(), Modifiers::default());
        assert_eq!(clicks.get(), 0, "static hint must not dispatch");

        let action_bounds = vcx
            .debug_bounds("probe-run-action")
            .expect("interactive hint should publish its stable action bounds");
        vcx.simulate_click(action_bounds.center(), Modifiers::default());
        assert_eq!(clicks.get(), 1, "one click must dispatch exactly once");
    }

    #[test]
    fn hint_strip_only_adds_pointer_semantics_with_stable_action_id_and_callback() {
        let strip = HintStrip::new(vec!["↵ Run".into(), "Esc Dismiss".into()]).on_hint_click(
            0,
            "prompt-footer-run",
            |_, _, _| {},
        );
        let snapshots = strip.interaction_snapshots();
        assert_interactive(&snapshots[0], "prompt-footer-run");
        assert_static(&snapshots[1]);
    }

    #[test]
    #[should_panic(expected = "interactive hints require a non-empty stable action ID")]
    fn clickable_hint_rejects_empty_action_identity() {
        let _hint = ClickableHint::new("", "⌘C Copy URI", |_, _, _| {});
    }

    #[test]
    fn clickable_and_selectable_hints_require_action_identity() {
        let clickable = ClickableHint::new("preview-copy-uri", "⌘C Copy URI", |_, _, _| {});
        assert_interactive(&clickable.interaction_snapshot(), "preview-copy-uri");

        let selectable =
            SelectableHint::new("agent-chat-actions", "⌘K Actions", |_, _, _| {}).selected(true);
        assert_interactive(&selectable.interaction_snapshot(), "agent-chat-actions");
        assert!(selectable.selected);
    }
}
