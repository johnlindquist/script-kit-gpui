#![allow(dead_code)]

use gpui::{div, prelude::*, px, rgb, rgba, svg, AnyElement, Div, FontWeight, Rgba, SharedString};

use crate::theme::{self, AppChromeColors};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InfoTextMetric {
    pub size: f32,
    pub line: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InfoTypeScale {
    pub micro: InfoTextMetric,
    pub caption: InfoTextMetric,
    pub body: InfoTextMetric,
    pub subhead: InfoTextMetric,
    pub title: InfoTextMetric,
    pub hero: InfoTextMetric,
    pub brand: InfoTextMetric,
}

pub(crate) const INFO_TYPE_SCALE: InfoTypeScale = InfoTypeScale {
    micro: InfoTextMetric {
        size: 11.0,
        line: 14.0,
    },
    caption: InfoTextMetric {
        size: 12.0,
        line: 16.0,
    },
    body: InfoTextMetric {
        size: 13.0,
        line: 18.0,
    },
    subhead: InfoTextMetric {
        size: 14.0,
        line: 20.0,
    },
    title: InfoTextMetric {
        size: 16.0,
        line: 22.0,
    },
    hero: InfoTextMetric {
        size: 20.0,
        line: 26.0,
    },
    brand: InfoTextMetric {
        size: 22.0,
        line: 28.0,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InfoSpacing {
    pub xxs: f32,
    pub xs: f32,
    pub sm: f32,
    pub md: f32,
    pub lg: f32,
    pub xl: f32,
    pub xxl: f32,
}

pub(crate) const INFO_SPACING: InfoSpacing = InfoSpacing {
    xxs: 4.0,
    xs: 8.0,
    sm: 12.0,
    md: 16.0,
    lg: 20.0,
    xl: 24.0,
    xxl: 32.0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InfoStateDensity {
    Compact,
    Comfortable,
    Hero,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InfoStateLayout {
    Centered,
    AnchoredTop,
    MainViewColumns,
    ComposerEmpty,
    InlineRow,
    InlinePanel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InfoStateTone {
    Neutral,
    Help,
    Setup,
    Permission,
    Recovery,
    About,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InfoTonePresentation {
    pub semantic_kind: &'static str,
    pub accessible_prefix: Option<&'static str>,
    pub default_icon_hint: Option<&'static str>,
    pub accent_foreground: bool,
    pub background_wash: bool,
}

pub(crate) const fn resolve_info_tone(tone: InfoStateTone) -> InfoTonePresentation {
    match tone {
        InfoStateTone::Neutral => InfoTonePresentation {
            semantic_kind: "neutral",
            accessible_prefix: None,
            default_icon_hint: None,
            accent_foreground: false,
            background_wash: false,
        },
        InfoStateTone::Help => InfoTonePresentation {
            semantic_kind: "help",
            accessible_prefix: Some("Help"),
            default_icon_hint: Some("circle-help"),
            accent_foreground: true,
            background_wash: false,
        },
        InfoStateTone::Setup => InfoTonePresentation {
            semantic_kind: "setup",
            accessible_prefix: Some("Setup"),
            default_icon_hint: Some("settings"),
            accent_foreground: true,
            background_wash: false,
        },
        InfoStateTone::Permission => InfoTonePresentation {
            semantic_kind: "permission",
            accessible_prefix: Some("Permission required"),
            default_icon_hint: Some("shield-check"),
            accent_foreground: true,
            background_wash: false,
        },
        InfoStateTone::Recovery => InfoTonePresentation {
            semantic_kind: "recovery",
            accessible_prefix: Some("Recovery"),
            default_icon_hint: Some("circle-alert"),
            accent_foreground: true,
            background_wash: false,
        },
        InfoStateTone::About => InfoTonePresentation {
            semantic_kind: "about",
            accessible_prefix: Some("About"),
            default_icon_hint: Some("info"),
            accent_foreground: true,
            background_wash: false,
        },
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InfoMetrics {
    pub max_width: f32,
    pub icon_size: f32,
    pub row_min_h: f32,
    pub radius: f32,
    pub block_gap: f32,
    pub item_gap: f32,
    pub pad_x: f32,
    pub pad_y: f32,
}

pub(crate) fn info_metrics(density: InfoStateDensity) -> InfoMetrics {
    match density {
        InfoStateDensity::Compact => InfoMetrics {
            max_width: 380.0,
            icon_size: 28.0,
            row_min_h: 28.0,
            radius: 8.0,
            block_gap: 12.0,
            item_gap: 6.0,
            pad_x: 12.0,
            pad_y: 10.0,
        },
        InfoStateDensity::Comfortable => InfoMetrics {
            max_width: 500.0,
            icon_size: 36.0,
            row_min_h: 34.0,
            radius: 9.0,
            block_gap: 16.0,
            item_gap: 8.0,
            pad_x: 16.0,
            pad_y: 14.0,
        },
        InfoStateDensity::Hero => InfoMetrics {
            max_width: 560.0,
            icon_size: 44.0,
            row_min_h: 36.0,
            radius: 10.0,
            block_gap: 20.0,
            item_gap: 10.0,
            pad_x: 20.0,
            pad_y: 16.0,
        },
    }
}

#[derive(Clone, Copy)]
pub(crate) struct InfoPalette {
    pub title: Rgba,
    pub body: Rgba,
    pub hint: Rgba,
    pub strong: Rgba,
    pub placeholder: Rgba,
    pub icon: Rgba,
    pub accent: Rgba,
    pub hover: Rgba,
    pub selected: Rgba,
    pub border: Rgba,
    pub whisper: Rgba,
    pub panel: Rgba,
}

pub(crate) fn info_palette(theme: &theme::Theme) -> InfoPalette {
    let chrome = AppChromeColors::from_theme(theme);
    InfoPalette {
        title: rgb(chrome.text_primary_hex),
        body: rgba(chrome.text_muted_rgba),
        hint: rgba(chrome.text_hint_rgba),
        strong: rgba(chrome.text_strong_rgba),
        placeholder: rgba(chrome.placeholder_text_rgba),
        icon: rgba(chrome.text_icon_rgba),
        accent: rgb(chrome.accent_hex),
        hover: rgba(chrome.hover_rgba),
        selected: rgba(chrome.selection_rgba),
        border: rgba(chrome.whisper_border_rgba),
        whisper: rgba(chrome.whisper_surface_rgba),
        panel: rgba(chrome.panel_surface_rgba),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InfoCue {
    Shortcut {
        raw: SharedString,
        tokens: Vec<String>,
        canonical: String,
        action_id: &'static str,
    },
    Trigger {
        text: SharedString,
        semantic_id: &'static str,
    },
    Syntax {
        text: SharedString,
        semantic_id: &'static str,
    },
    Label {
        text: SharedString,
        semantic_id: &'static str,
    },
}

impl InfoCue {
    pub(crate) const fn kind(&self) -> &'static str {
        match self {
            Self::Shortcut { .. } => "shortcut",
            Self::Trigger { .. } => "trigger",
            Self::Syntax { .. } => "syntax",
            Self::Label { .. } => "label",
        }
    }

    pub(crate) fn text(&self) -> &str {
        match self {
            Self::Shortcut { raw, .. } => raw.as_ref(),
            Self::Trigger { text, .. } | Self::Syntax { text, .. } | Self::Label { text, .. } => {
                text.as_ref()
            }
        }
    }

    pub(crate) const fn semantic_id(&self) -> &'static str {
        match self {
            Self::Shortcut { action_id, .. } => action_id,
            Self::Trigger { semantic_id, .. }
            | Self::Syntax { semantic_id, .. }
            | Self::Label { semantic_id, .. } => semantic_id,
        }
    }

    pub(crate) fn canonical_shortcut(&self) -> Option<&str> {
        match self {
            Self::Shortcut { canonical, .. } => Some(canonical),
            Self::Trigger { .. } | Self::Syntax { .. } | Self::Label { .. } => None,
        }
    }

    pub(crate) const fn action_id(&self) -> Option<&'static str> {
        match self {
            Self::Shortcut { action_id, .. } => Some(action_id),
            Self::Trigger { .. } | Self::Syntax { .. } | Self::Label { .. } => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InfoCueValidationError {
    EmptyShortcut,
    NonShortcutCue,
    MissingActionId,
}

fn is_non_shortcut_info_cue(raw: &str) -> bool {
    let trimmed = raw.trim();
    matches!(trimmed, "/" | "@")
        || trimmed.eq_ignore_ascii_case("filter")
        || trimmed.starts_with(':')
        || trimmed.starts_with(';')
        || trimmed.starts_with("type:")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoGuidanceItem {
    pub cue: InfoCue,
    pub label: SharedString,
    pub detail: Option<SharedString>,
}

impl InfoGuidanceItem {
    pub(crate) fn try_shortcut(
        raw: &'static str,
        action_id: &'static str,
        label: impl Into<SharedString>,
    ) -> Result<Self, InfoCueValidationError> {
        if raw.trim().is_empty() {
            return Err(InfoCueValidationError::EmptyShortcut);
        }
        if is_non_shortcut_info_cue(raw) {
            return Err(InfoCueValidationError::NonShortcutCue);
        }
        if action_id.trim().is_empty() {
            return Err(InfoCueValidationError::MissingActionId);
        }
        let tokens = crate::components::hint_strip::shortcut_tokens_from_hint(raw);
        let canonical = crate::components::hint_strip::canonical_shortcut_hint(raw);
        if tokens.is_empty() || canonical.is_empty() {
            return Err(InfoCueValidationError::EmptyShortcut);
        }
        Ok(Self {
            cue: InfoCue::Shortcut {
                raw: raw.into(),
                tokens,
                canonical,
                action_id,
            },
            label: label.into(),
            detail: None,
        })
    }

    pub(crate) fn shortcut(
        raw: &'static str,
        action_id: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self::try_shortcut(raw, action_id, label)
            .expect("InfoGuidanceItem::shortcut requires an executable keyboard cue")
    }

    pub(crate) fn trigger(
        text: &'static str,
        semantic_id: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            cue: InfoCue::Trigger {
                text: text.into(),
                semantic_id,
            },
            label: label.into(),
            detail: None,
        }
    }

    pub(crate) fn syntax(
        text: &'static str,
        semantic_id: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            cue: InfoCue::Syntax {
                text: text.into(),
                semantic_id,
            },
            label: label.into(),
            detail: None,
        }
    }

    pub(crate) fn label(
        text: &'static str,
        semantic_id: &'static str,
        label: impl Into<SharedString>,
    ) -> Self {
        Self {
            cue: InfoCue::Label {
                text: text.into(),
                semantic_id,
            },
            label: label.into(),
            detail: None,
        }
    }

    pub(crate) fn detail(mut self, detail: impl Into<SharedString>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoSection {
    pub title: Option<SharedString>,
    pub items: Vec<InfoGuidanceItem>,
}

impl InfoSection {
    pub(crate) fn new(items: Vec<InfoGuidanceItem>) -> Self {
        Self { title: None, items }
    }

    pub(crate) fn titled(title: impl Into<SharedString>, items: Vec<InfoGuidanceItem>) -> Self {
        Self {
            title: Some(title.into()),
            items,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoShortcutNote {
    pub shortcut: &'static str,
    pub text: SharedString,
}

impl InfoShortcutNote {
    pub(crate) fn new(shortcut: &'static str, text: impl Into<SharedString>) -> Self {
        Self {
            shortcut,
            text: text.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoStateSpec {
    pub id: &'static str,
    pub layout: InfoStateLayout,
    pub density: InfoStateDensity,
    pub tone: InfoStateTone,
    pub icon_hint: Option<&'static str>,
    pub eyebrow: Option<SharedString>,
    pub title: Option<SharedString>,
    pub body: Option<SharedString>,
    pub sections: Vec<InfoSection>,
    pub footer_note: Option<SharedString>,
    pub footer_shortcut_note: Option<InfoShortcutNote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoCueSemanticSnapshot {
    pub semantic_id: &'static str,
    pub cue_kind: &'static str,
    pub cue_text: String,
    pub canonical_shortcut: Option<String>,
    pub action_id: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InfoStateSemanticSnapshot {
    pub id: &'static str,
    pub layout: InfoStateLayout,
    pub density: InfoStateDensity,
    pub tone: InfoStateTone,
    pub semantic_kind: &'static str,
    pub accessible_prefix: Option<&'static str>,
    pub default_icon_hint: Option<&'static str>,
    pub title_present: bool,
    pub title_byte_len: usize,
    pub body_present: bool,
    pub body_byte_len: usize,
    pub cues: Vec<InfoCueSemanticSnapshot>,
    pub actions: Vec<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InfoEmptySurface {
    ActionsSearch,
    ArgChoices,
    NotesBrowse,
}

impl InfoStateSpec {
    pub(crate) fn new(id: &'static str) -> Self {
        Self {
            id,
            layout: InfoStateLayout::Centered,
            density: InfoStateDensity::Compact,
            tone: InfoStateTone::Neutral,
            icon_hint: None,
            eyebrow: None,
            title: None,
            body: None,
            sections: Vec::new(),
            footer_note: None,
            footer_shortcut_note: None,
        }
    }

    pub(crate) fn semantic_snapshot(&self) -> InfoStateSemanticSnapshot {
        let tone = resolve_info_tone(self.tone);
        let cues = self
            .sections
            .iter()
            .flat_map(|section| section.items.iter())
            .map(|item| InfoCueSemanticSnapshot {
                semantic_id: item.cue.semantic_id(),
                cue_kind: item.cue.kind(),
                cue_text: item.cue.text().to_string(),
                canonical_shortcut: item.cue.canonical_shortcut().map(str::to_string),
                action_id: item.cue.action_id(),
            })
            .collect();
        InfoStateSemanticSnapshot {
            id: self.id,
            layout: self.layout,
            density: self.density,
            tone: self.tone,
            semantic_kind: tone.semantic_kind,
            accessible_prefix: tone.accessible_prefix,
            default_icon_hint: self.icon_hint.or(tone.default_icon_hint),
            title_present: self.title.is_some(),
            title_byte_len: self.title.as_ref().map_or(0, |title| title.len()),
            body_present: self.body.is_some(),
            body_byte_len: self.body.as_ref().map_or(0, |body| body.len()),
            cues,
            actions: Vec::new(),
        }
    }

    pub(crate) fn layout(mut self, layout: InfoStateLayout) -> Self {
        self.layout = layout;
        self
    }

    pub(crate) fn density(mut self, density: InfoStateDensity) -> Self {
        self.density = density;
        self
    }

    pub(crate) fn tone(mut self, tone: InfoStateTone) -> Self {
        self.tone = tone;
        self
    }

    pub(crate) fn icon_hint(mut self, icon_hint: &'static str) -> Self {
        self.icon_hint = Some(icon_hint);
        self
    }

    pub(crate) fn eyebrow(mut self, eyebrow: impl Into<SharedString>) -> Self {
        self.eyebrow = Some(eyebrow.into());
        self
    }

    pub(crate) fn title(mut self, title: impl Into<SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub(crate) fn body(mut self, body: impl Into<SharedString>) -> Self {
        self.body = Some(body.into());
        self
    }

    pub(crate) fn section(mut self, section: InfoSection) -> Self {
        self.sections.push(section);
        self
    }

    pub(crate) fn footer_note(mut self, note: impl Into<SharedString>) -> Self {
        self.footer_note = Some(note.into());
        self.footer_shortcut_note = None;
        self
    }

    pub(crate) fn footer_shortcut_note(
        mut self,
        shortcut: &'static str,
        text: impl Into<SharedString>,
    ) -> Self {
        self.footer_shortcut_note = Some(InfoShortcutNote::new(shortcut, text));
        self.footer_note = None;
        self
    }
}

pub(crate) fn shared_empty_state_spec(surface: InfoEmptySurface, query: &str) -> InfoStateSpec {
    let query = query.trim();
    match surface {
        InfoEmptySurface::ActionsSearch => {
            if query.is_empty() {
                InfoStateSpec::new("actions-empty-no-actions")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Help)
                    .body("No actions available")
            } else {
                InfoStateSpec::new("actions-empty-no-matches")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Recovery)
                    .body("No actions match your search")
            }
        }
        InfoEmptySurface::ArgChoices => {
            if query.is_empty() {
                InfoStateSpec::new("arg-empty-no-choices")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Help)
                    .body("No choices · Enter to submit typed value")
            } else {
                InfoStateSpec::new("arg-empty-no-matches")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Recovery)
                    .body("No matches · Enter to submit typed value")
            }
        }
        InfoEmptySurface::NotesBrowse => {
            if query.is_empty() {
                InfoStateSpec::new("notes-browse-empty-no-notes")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Help)
                    .body("No notes yet")
            } else {
                InfoStateSpec::new("notes-browse-empty-no-matches")
                    .layout(InfoStateLayout::InlineRow)
                    .density(InfoStateDensity::Compact)
                    .tone(InfoStateTone::Recovery)
                    .body("No notes match your filter")
            }
        }
    }
}

pub(crate) fn render_shared_empty_state(
    surface: InfoEmptySurface,
    query: &str,
    theme: &theme::Theme,
    cx: &gpui::App,
) -> AnyElement {
    render_info_state(shared_empty_state_spec(surface, query), theme, cx)
}

/// Semantic replacement for the retired list-local EmptyState renderer.
pub(crate) fn simple_empty_state_spec(
    id: &'static str,
    message: impl Into<SharedString>,
    icon_hint: &'static str,
    hint: Option<&str>,
) -> InfoStateSpec {
    let mut spec = InfoStateSpec::new(id)
        .layout(InfoStateLayout::Centered)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Neutral)
        .icon_hint(icon_hint)
        .body(message);
    if let Some(hint) = hint.filter(|hint| !hint.trim().is_empty()) {
        spec = spec.footer_note(hint.to_string());
    }
    spec
}

pub(crate) fn render_simple_empty_state(
    id: &'static str,
    message: impl Into<SharedString>,
    icon_hint: &'static str,
    hint: Option<&str>,
    theme: &theme::Theme,
    cx: &gpui::App,
) -> AnyElement {
    render_info_state(
        simple_empty_state_spec(id, message, icon_hint, hint),
        theme,
        cx,
    )
}

pub(crate) fn permission_onboarding_intro_spec(
    granted_count: usize,
    row_count: usize,
    all_required_granted: bool,
) -> InfoStateSpec {
    let footer = if all_required_granted {
        "All required permissions granted. Press Esc to finish."
    } else {
        "Press Enter on a row to grant it. Status updates automatically."
    };
    InfoStateSpec::new("permissions-onboarding-intro")
        .layout(InfoStateLayout::InlinePanel)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Permission)
        .eyebrow(format!("{granted_count} of {row_count} granted"))
        .body(
            "Script Kit uses macOS permissions to read selected text, paste into other apps, run shortcuts, and capture context.",
        )
        .section(InfoSection::titled(
            "Actions",
            vec![
                InfoGuidanceItem::shortcut(
                    "↵",
                    "permissions-grant-selected",
                    "Grant selected permission",
                ),
                InfoGuidanceItem::shortcut("Esc", "permissions-finish", "Finish setup"),
            ],
        ))
        .footer_note(footer)
}

pub(crate) fn agent_setup_info_spec(
    title: impl Into<SharedString>,
    body: impl Into<SharedString>,
    selected_agent: Option<impl Into<SharedString>>,
) -> InfoStateSpec {
    let mut spec = InfoStateSpec::new("agent-setup-guidance")
        .layout(InfoStateLayout::InlineRow)
        .density(InfoStateDensity::Comfortable)
        .tone(InfoStateTone::Setup)
        .title(title)
        .body(body);
    if let Some(agent) = selected_agent {
        let agent: SharedString = agent.into();
        spec = spec.eyebrow(format!("Selected · {agent}"));
    }
    spec
}

pub(crate) fn agent_chat_empty_guidance_spec() -> InfoStateSpec {
    InfoStateSpec::new("agent_chat-empty-composer-guidance")
        .layout(InfoStateLayout::ComposerEmpty)
        .density(InfoStateDensity::Comfortable)
        .tone(InfoStateTone::Help)
        .title("Ask with context")
        .body("Describe the result you want. Use / for skills or @ to attach context before you send.")
        .section(InfoSection::new(vec![
            InfoGuidanceItem::trigger(
                "/",
                "agent-chat-trigger-skills",
                "Use a skill or agent command",
            ),
            InfoGuidanceItem::trigger(
                "@",
                "agent-chat-trigger-context",
                "Attach files, scripts, clipboard, or history",
            ),
            InfoGuidanceItem::shortcut("⇧↵", "agent-chat-add-newline", "Add a newline"),
            InfoGuidanceItem::shortcut("⌘P", "agent-chat-open-history", "Open previous chats"),
            InfoGuidanceItem::shortcut("⌘K", "agent-chat-open-actions", "Show every chat action"),
        ]))
}

pub(crate) fn render_agent_chat_empty_guidance(theme: &theme::Theme, cx: &gpui::App) -> AnyElement {
    render_info_state(agent_chat_empty_guidance_spec(), theme, cx)
}

pub(crate) fn launcher_empty_or_no_results_spec(
    filter_text_for_render: &str,
    has_active_filter: bool,
) -> InfoStateSpec {
    if filter_text_for_render.is_empty() {
        return launcher_no_scripts_spec();
    }
    if has_active_filter {
        return launcher_active_filter_no_results_spec(filter_text_for_render);
    }
    if launcher_plain_hash_search(filter_text_for_render) {
        return launcher_plain_hash_no_results_spec(filter_text_for_render);
    }
    launcher_generic_no_results_spec(filter_text_for_render)
}

pub(crate) fn render_launcher_empty_or_no_results(
    filter_text_for_render: &str,
    has_active_filter: bool,
    theme: &theme::Theme,
    cx: &gpui::App,
) -> AnyElement {
    render_info_state(
        launcher_empty_or_no_results_spec(filter_text_for_render, has_active_filter),
        theme,
        cx,
    )
}

fn launcher_no_scripts_spec() -> InfoStateSpec {
    InfoStateSpec::new("launcher-empty-no-scripts")
        .layout(InfoStateLayout::Centered)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Help)
        .title("No scripts yet")
        .body("This launcher opens your Script Kit scripts and snippets. Create one now, ask Agent Chat to draft the workflow, or open Actions for setup and install options.")
        .section(InfoSection::new(vec![
            InfoGuidanceItem::shortcut("⌘N", "launcher-create-script", "Create a script")
                .detail("Start a new automation in your scripts folder."),
            InfoGuidanceItem::shortcut("⌘↵", "launcher-ask-agent-chat", "Ask Agent Chat")
                .detail("Describe the workflow you want and let AI draft it."),
            InfoGuidanceItem::shortcut("⌘K", "launcher-open-actions", "Open Actions")
                .detail("Find reload, install, and setup commands."),
        ]))
        .footer_note("After scripts exist, type here to search and run them.")
}

fn launcher_active_filter_no_results_spec(filter_text: &str) -> InfoStateSpec {
    let filter_display = launcher_filter_display(filter_text);
    InfoStateSpec::new("launcher-empty-active-filter")
        .layout(InfoStateLayout::Centered)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Recovery)
        .title(format!("No matches for \"{filter_display}\""))
        .body("The search is working, but an active filter is narrowing the launcher to zero results. Remove a filter chip or loosen the query to widen the set.")
        .section(InfoSection::new(vec![
            InfoGuidanceItem::shortcut("Esc", "launcher-clear-search", "Clear the search"),
            InfoGuidanceItem::label(
                "Filter",
                "launcher-filter-chip-label",
                "Remove a filter chip",
            )
            .detail("Source and type filters apply before fuzzy matching."),
            InfoGuidanceItem::shortcut("⌘K", "launcher-open-actions", "Open Actions")
                .detail("Use actions if you meant to manage scripts or filters."),
        ]))
        .footer_note("Filters narrow the library before the launcher ranks results.")
}

fn launcher_plain_hash_no_results_spec(filter_text: &str) -> InfoStateSpec {
    let filter_display = launcher_filter_display(filter_text);
    InfoStateSpec::new("launcher-empty-plain-hash")
        .layout(InfoStateLayout::Centered)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Help)
        .title("Tags need a syntax prefix")
        .body(format!("Plain {filter_display} is treated as launcher text search. Use :#tag to filter existing tags, or add #tag after a capture like ;todo when you are creating one."))
        .section(InfoSection::titled(
            "Examples",
            vec![
                InfoGuidanceItem::syntax(":#", "launcher-syntax-tag-prefix", "Filter tagged items")
                    .detail("Example: :#work"),
                InfoGuidanceItem::syntax(
                    ":tag:",
                    "launcher-syntax-tag-name",
                    "Filter by tag name",
                )
                .detail("Example: :tag:work"),
                InfoGuidanceItem::syntax(
                    ";todo",
                    "launcher-syntax-todo-capture",
                    "Create a tagged capture",
                )
                .detail("Example: ;todo Buy milk #errands"),
            ],
        ))
        .footer_note("Keep #tag plain only when you want text search, not tag filtering.")
}

fn launcher_generic_no_results_spec(filter_text: &str) -> InfoStateSpec {
    let filter_display = launcher_filter_display(filter_text);
    InfoStateSpec::new("launcher-empty-generic-no-results")
        .layout(InfoStateLayout::Centered)
        .density(InfoStateDensity::Compact)
        .tone(InfoStateTone::Recovery)
        .title(format!("No results for \"{filter_display}\""))
        .body("The launcher searches scripts, scriptlets, snippets, and built-in commands by name and metadata. Try fewer words, use a structured filter, capture the thought, or ask Agent Chat to turn it into a script.")
        .section(InfoSection::new(vec![
            InfoGuidanceItem::shortcut("Esc", "launcher-clear-search", "Clear the search"),
            InfoGuidanceItem::syntax(":", "launcher-syntax-source", "Search other sources")
                .detail(source_prefix_examples_detail()),
            InfoGuidanceItem::syntax(
                "type:",
                "launcher-syntax-metadata",
                "Search by metadata",
            )
            .detail("Examples: type:script · type:scriptlet · shortcut:cmd+k"),
            InfoGuidanceItem::syntax(";todo", "launcher-syntax-capture", "Capture instead")
                .detail("Examples: ;todo · ;note"),
            InfoGuidanceItem::shortcut("⌘N", "launcher-create-script", "Create a script")
                .detail("Start a new automation in your scripts folder."),
            InfoGuidanceItem::shortcut("⌘↵", "launcher-ask-agent-chat", "Ask Agent Chat")
                .detail("Turn this search into a script request."),
        ]))
        .footer_note("Structured filters work best for metadata; plain words work best for names.")
}

/// Teach the source-head syntax with real heads from the canonical
/// descriptor table, so this hint can never drift from the parser.
/// Featured sources are the ones users most often don't know exist
/// (browser history and todos surface only via their prefixes).
fn source_prefix_examples_detail() -> String {
    use crate::menu_syntax::payload::RootUnifiedSourceFilter as Source;

    let featured = [
        Source::Files,
        Source::BrowserHistory,
        Source::Todo,
        Source::ClipboardHistory,
    ];
    let examples = crate::menu_syntax::payload::SOURCE_HEAD_SPECS
        .iter()
        .filter(|spec| featured.contains(&spec.source))
        .map(|spec| spec.canonical)
        .collect::<Vec<_>>()
        .join(" · ");
    format!("Examples: {examples} — type : to browse every source")
}

fn launcher_plain_hash_search(filter_text: &str) -> bool {
    filter_text.starts_with('#') && filter_text.chars().skip(1).all(|ch| !ch.is_whitespace())
}

fn launcher_filter_display(filter_text: &str) -> String {
    if filter_text.chars().count() > 30 {
        format!("{}...", crate::utils::truncate_str_chars(filter_text, 27))
    } else {
        filter_text.to_string()
    }
}

pub(crate) fn render_info_state(
    spec: InfoStateSpec,
    theme: &theme::Theme,
    cx: &gpui::App,
) -> AnyElement {
    let def = crate::designs::current_main_menu_theme().def();
    render_info_state_with_main_view_def(spec, theme, def, cx)
}

pub(crate) fn render_info_state_with_main_view_def(
    spec: InfoStateSpec,
    theme: &theme::Theme,
    def: crate::designs::MainMenuThemeDef,
    cx: &gpui::App,
) -> AnyElement {
    let palette = info_palette(theme);
    let metrics = info_metrics(spec.density);
    let uses_main_view_columns = matches!(
        spec.layout,
        InfoStateLayout::ComposerEmpty | InfoStateLayout::MainViewColumns
    );
    let content = render_info_content(&spec, theme, palette, metrics, !uses_main_view_columns, cx);

    match spec.layout {
        InfoStateLayout::Centered => div()
            .id(spec.id)
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .flex()
            .items_center()
            .justify_center()
            .px(px(INFO_SPACING.xl))
            .child(content)
            .into_any_element(),
        InfoStateLayout::ComposerEmpty | InfoStateLayout::MainViewColumns => {
            let cols = crate::components::main_view_chrome::main_view_content_columns(def);
            div()
                .id(spec.id)
                .w_full()
                .h_full()
                .min_h(px(0.0))
                .flex()
                .items_start()
                .justify_start()
                .pl(px(cols.text_column_x))
                .pr(px(cols.content_right_inset_x))
                .pt(px(cols.top_inset_y))
                .pb(px(def.shell.content_inset_bottom))
                .child(content)
                .into_any_element()
        }
        InfoStateLayout::AnchoredTop => div()
            .id(spec.id)
            .w_full()
            .h_full()
            .min_h(px(0.0))
            .flex()
            .items_start()
            .justify_center()
            .px(px(INFO_SPACING.xl))
            .py(px(INFO_SPACING.xl))
            .child(content)
            .into_any_element(),
        InfoStateLayout::InlineRow => div().id(spec.id).w_full().child(content).into_any_element(),
        InfoStateLayout::InlinePanel => div()
            .id(spec.id)
            .w_full()
            .child(
                content
                    .rounded(px(metrics.radius))
                    .border_1()
                    .border_color(palette.border)
                    .bg(palette.whisper)
                    .px(px(metrics.pad_x))
                    .py(px(metrics.pad_y)),
            )
            .into_any_element(),
    }
}

/// Render an InlinePanel whose outer surface fills its owning flow while its
/// compact prose measure remains bounded. This separates panel measure from
/// text measure without changing compact InlinePanel call sites elsewhere.
pub(crate) fn render_info_state_full_width_panel(
    spec: InfoStateSpec,
    theme: &theme::Theme,
    text_inset_x: f32,
    cx: &gpui::App,
) -> AnyElement {
    debug_assert_eq!(spec.layout, InfoStateLayout::InlinePanel);
    let palette = info_palette(theme);
    let metrics = info_metrics(spec.density);
    let content = render_info_content(&spec, theme, palette, metrics, true, cx);

    div()
        .id(spec.id)
        .w_full()
        .rounded(px(metrics.radius))
        .border_1()
        .border_color(palette.border)
        .bg(palette.whisper)
        .pl(px(text_inset_x.max(metrics.pad_x)))
        .pr(px(metrics.pad_x))
        .py(px(metrics.pad_y))
        .child(content)
        .into_any_element()
}

fn render_info_tone_header(spec: &InfoStateSpec, palette: InfoPalette) -> Option<AnyElement> {
    use crate::icons::IconNamed;

    let presentation = resolve_info_tone(spec.tone);
    let eyebrow = spec.eyebrow.clone().or_else(|| {
        presentation
            .accessible_prefix
            .map(|prefix| SharedString::from(prefix.to_string()))
    });
    let icon = spec
        .icon_hint
        .or(presentation.default_icon_hint)
        .and_then(crate::icons::lucide_from_str);
    if eyebrow.is_none() && icon.is_none() {
        return None;
    }

    let color = if presentation.accent_foreground {
        palette.accent
    } else {
        palette.strong
    };
    let mut row = div()
        .flex()
        .flex_row()
        .items_center()
        .gap(px(INFO_SPACING.xs))
        .text_color(color);
    if let Some(icon) = icon {
        row = row.child(
            svg()
                .path(icon.path())
                .size(px(INFO_TYPE_SCALE.subhead.size))
                .flex_none()
                .text_color(color),
        );
    }
    if let Some(eyebrow) = eyebrow {
        row = row.child(
            div()
                .text_size(px(INFO_TYPE_SCALE.micro.size))
                .line_height(px(INFO_TYPE_SCALE.micro.line))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color)
                .child(eyebrow),
        );
    }
    Some(row.into_any_element())
}

fn render_info_content(
    spec: &InfoStateSpec,
    theme: &theme::Theme,
    palette: InfoPalette,
    metrics: InfoMetrics,
    cap_width: bool,
    cx: &gpui::App,
) -> Div {
    let title_metric = match spec.density {
        InfoStateDensity::Compact => INFO_TYPE_SCALE.subhead,
        InfoStateDensity::Comfortable => INFO_TYPE_SCALE.title,
        InfoStateDensity::Hero => INFO_TYPE_SCALE.hero,
    };
    let body_metric = INFO_TYPE_SCALE.body;

    let mut stack = div().w_full().flex().flex_col().gap(px(metrics.block_gap));
    if cap_width {
        stack = stack.max_w(px(metrics.max_width));
    }

    if let Some(tone_header) = render_info_tone_header(spec, palette) {
        stack = stack.child(tone_header);
    }

    if spec.title.is_some() || spec.body.is_some() {
        let mut intro = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(INFO_SPACING.xs * 0.5));
        if let Some(title) = spec.title.clone() {
            intro = intro.child(
                div()
                    .text_size(px(title_metric.size))
                    .line_height(px(title_metric.line))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(palette.title)
                    .child(title),
            );
        }
        if let Some(body) = spec.body.clone() {
            intro = intro.child(
                div()
                    .text_size(px(body_metric.size))
                    .line_height(px(body_metric.line))
                    .text_color(palette.body)
                    .child(body),
            );
        }
        stack = stack.child(intro);
    }

    for (index, section) in spec.sections.iter().enumerate() {
        stack = stack.child(render_info_section(
            section,
            format!("{}-section-{index}", spec.id),
            spec.density,
            theme,
            palette,
            metrics,
            cx,
        ));
    }

    if let Some(note) = spec.footer_shortcut_note.clone() {
        // Align the footer shortcut keycap into the same fixed-width slot the
        // guidance rows use, so its trailing text lines up with the guidance
        // labels above it instead of starting at the keycap's natural width.
        let guidance_items: Vec<InfoGuidanceItem> = spec
            .sections
            .iter()
            .flat_map(|section| section.items.iter().cloned())
            .collect();
        let shortcut_slot_width_px = info_guidance_cue_slot_width_px(&guidance_items, cx);
        stack = stack.child(render_info_shortcut_note(
            note,
            metrics,
            theme,
            palette,
            shortcut_slot_width_px,
        ));
    } else if let Some(note) = spec.footer_note.clone() {
        stack = stack.child(render_info_plain_footer_note(note, palette));
    }

    stack
}

fn render_info_plain_footer_note(note: SharedString, palette: InfoPalette) -> AnyElement {
    div()
        .text_size(px(INFO_TYPE_SCALE.caption.size))
        .line_height(px(INFO_TYPE_SCALE.caption.line))
        .text_color(palette.hint)
        .child(note)
        .into_any_element()
}

fn render_info_shortcut_note(
    note: InfoShortcutNote,
    metrics: InfoMetrics,
    theme: &theme::Theme,
    palette: InfoPalette,
    shortcut_slot_width_px: f32,
) -> AnyElement {
    let keycaps = crate::components::footer_chrome::render_footer_shortcut_keycaps(
        note.shortcut.to_string(),
        theme,
    );
    // Use the same fixed-width keycap slot + row gap as `render_guidance_row` so
    // the note text aligns horizontally with the guidance labels above it.
    let keycap_slot = if shortcut_slot_width_px > 0.0 {
        div()
            .w(px(shortcut_slot_width_px))
            .flex_none()
            .flex()
            .items_center()
            .child(keycaps)
    } else {
        div().flex().items_center().child(keycaps)
    };
    div()
        .w_full()
        .min_h(px(metrics.row_min_h))
        .flex()
        .items_center()
        .gap(px(INFO_SPACING.sm))
        .child(keycap_slot)
        .child(render_info_guidance_text(note.text, None, palette))
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_info_section(
    section: &InfoSection,
    id: String,
    density: InfoStateDensity,
    theme: &theme::Theme,
    palette: InfoPalette,
    metrics: InfoMetrics,
    cx: &gpui::App,
) -> AnyElement {
    let mut stack = div()
        .id(id)
        .w_full()
        .flex()
        .flex_col()
        .gap(px(metrics.item_gap));

    if let Some(title) = section.title.clone() {
        stack = stack.child(
            div()
                .text_size(px(INFO_TYPE_SCALE.micro.size))
                .line_height(px(INFO_TYPE_SCALE.micro.line))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.strong)
                .child(title),
        );
    }

    stack
        .child(render_info_guidance_items(
            "info-guidance-items",
            &section.items,
            density,
            theme,
            palette,
            cx,
        ))
        .into_any_element()
}

pub(crate) fn render_info_guidance_items(
    id: &'static str,
    items: &[InfoGuidanceItem],
    density: InfoStateDensity,
    theme: &theme::Theme,
    palette: InfoPalette,
    cx: &gpui::App,
) -> AnyElement {
    let metrics = info_metrics(density);
    let shortcut_slot_width_px = info_guidance_cue_slot_width_px(items, cx);
    div()
        .id(id)
        .w_full()
        .flex()
        .flex_col()
        .gap(px(INFO_SPACING.xs * 0.5))
        .children(items.iter().map(|item| {
            render_guidance_row(item, metrics, theme, palette, shortcut_slot_width_px)
                .into_any_element()
        }))
        .into_any_element()
}

/// Shared column width for every cue kind. Shortcut widths use footer keycap
/// metrics; trigger, syntax, and label widths use the text system directly and
/// never pass through shortcut/keycap measurement.
fn info_guidance_cue_slot_width_px(items: &[InfoGuidanceItem], cx: &gpui::App) -> f32 {
    info_guidance_cue_slot_width_from_widths(
        items
            .iter()
            .map(|item| info_cue_measured_width_px(&item.cue, cx)),
    )
}

fn info_cue_text_width_px(text: &str, font_family: &'static str, cx: &gpui::App) -> f32 {
    let text_system = cx.text_system();
    let font_id = text_system.resolve_font(&gpui::font(font_family));
    let font_size = px(INFO_TYPE_SCALE.caption.size);
    text.chars()
        .map(|ch| f32::from(text_system.layout_width(font_id, font_size, ch)))
        .sum::<f32>()
        .ceil()
}

fn info_cue_measured_width_px(cue: &InfoCue, cx: &gpui::App) -> f32 {
    match cue {
        InfoCue::Shortcut { tokens, .. } => {
            crate::components::footer_chrome::footer_shortcut_keycaps_measured_width_from_tokens(
                tokens.iter().map(String::as_str),
                cx,
            )
        }
        InfoCue::Trigger { text, .. } => {
            info_cue_text_width_px(text, crate::list_item::FONT_MONO, cx)
        }
        InfoCue::Syntax { text, .. } => {
            info_cue_text_width_px(text, crate::list_item::FONT_MONO, cx) + INFO_SPACING.xs
        }
        InfoCue::Label { text, .. } => {
            info_cue_text_width_px(text, crate::list_item::FONT_SYSTEM_UI, cx)
        }
    }
}

fn info_guidance_cue_slot_width_from_widths(widths: impl Iterator<Item = f32>) -> f32 {
    let min_cue_width = crate::components::footer_chrome::FOOTER_KEYCAP_HEIGHT_PX * 2.0
        + crate::components::footer_chrome::FOOTER_ACTION_CONTENT_GAP_PX;
    let max_width = widths.fold(0.0, f32::max);

    if max_width > 0.0 {
        max_width.max(min_cue_width)
    } else {
        0.0
    }
}

fn render_info_cue(cue: &InfoCue, theme: &theme::Theme, palette: InfoPalette) -> AnyElement {
    let semantic_id = SharedString::from(format!("info-cue-{}", cue.semantic_id()));
    match cue {
        InfoCue::Shortcut { tokens, .. } => div()
            .id(semantic_id)
            .flex()
            .items_center()
            .child(
                crate::components::footer_chrome::render_footer_shortcut_keycaps_from_tokens(
                    tokens.iter().map(String::as_str),
                    theme,
                ),
            )
            .into_any_element(),
        InfoCue::Trigger { text, .. } => div()
            .id(semantic_id)
            .font_family(crate::list_item::FONT_MONO)
            .text_size(px(INFO_TYPE_SCALE.caption.size))
            .line_height(px(INFO_TYPE_SCALE.caption.line))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(palette.accent)
            .child(text.clone())
            .into_any_element(),
        InfoCue::Syntax { text, .. } => div()
            .id(semantic_id)
            .font_family(crate::list_item::FONT_MONO)
            .text_size(px(INFO_TYPE_SCALE.caption.size))
            .line_height(px(INFO_TYPE_SCALE.caption.line))
            .text_color(palette.accent)
            .bg(palette.whisper)
            .rounded(px(INFO_SPACING.xxs))
            .px(px(INFO_SPACING.xxs))
            .child(text.clone())
            .into_any_element(),
        InfoCue::Label { text, .. } => div()
            .id(semantic_id)
            .text_size(px(INFO_TYPE_SCALE.caption.size))
            .line_height(px(INFO_TYPE_SCALE.caption.line))
            .text_color(palette.strong)
            .child(text.clone())
            .into_any_element(),
    }
}

fn render_guidance_row(
    item: &InfoGuidanceItem,
    metrics: InfoMetrics,
    theme: &theme::Theme,
    palette: InfoPalette,
    cue_slot_width_px: f32,
) -> Div {
    div()
        .w_full()
        .min_h(px(metrics.row_min_h))
        .flex()
        .items_center()
        .gap(px(INFO_SPACING.sm))
        .child(
            div()
                .w(px(cue_slot_width_px))
                .flex_none()
                .flex()
                .items_center()
                .child(render_info_cue(&item.cue, theme, palette)),
        )
        .child(render_info_guidance_text(
            item.label.clone(),
            item.detail.clone(),
            palette,
        ))
}

fn render_info_guidance_text(
    label: SharedString,
    detail: Option<SharedString>,
    palette: InfoPalette,
) -> Div {
    let mut text = div()
        .flex_1()
        .min_w(px(0.0))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .child(
            div()
                .text_size(px(INFO_TYPE_SCALE.caption.size))
                .line_height(px(INFO_TYPE_SCALE.caption.line))
                .text_color(palette.body)
                .child(label),
        );

    if let Some(detail) = detail {
        text = text.child(
            div()
                .text_size(px(INFO_TYPE_SCALE.micro.size))
                .line_height(px(INFO_TYPE_SCALE.micro.line))
                .text_color(palette.hint)
                .child(detail),
        );
    }

    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_type_scale_matches_compact_palette_math() {
        assert_eq!(
            INFO_TYPE_SCALE.micro,
            InfoTextMetric {
                size: 11.0,
                line: 14.0
            }
        );
        assert_eq!(
            INFO_TYPE_SCALE.caption,
            InfoTextMetric {
                size: 12.0,
                line: 16.0
            }
        );
        assert_eq!(
            INFO_TYPE_SCALE.body,
            InfoTextMetric {
                size: 13.0,
                line: 18.0
            }
        );
        assert_eq!(
            INFO_TYPE_SCALE.title,
            InfoTextMetric {
                size: 16.0,
                line: 22.0
            }
        );
        assert!(INFO_TYPE_SCALE.brand.size <= 22.0);
    }

    #[test]
    fn info_metrics_use_four_pixel_rhythm_where_visible() {
        for metrics in [
            info_metrics(InfoStateDensity::Compact),
            info_metrics(InfoStateDensity::Comfortable),
            info_metrics(InfoStateDensity::Hero),
        ] {
            for value in [
                metrics.icon_size,
                metrics.row_min_h,
                metrics.block_gap,
                metrics.pad_x,
                metrics.pad_y,
            ] {
                assert_eq!(value.rem_euclid(2.0), 0.0);
            }
        }
    }

    #[test]
    fn info_cues_classify_shortcuts_triggers_syntax_and_labels_explicitly() {
        let agent = agent_chat_empty_guidance_spec().semantic_snapshot();
        assert!(agent
            .cues
            .iter()
            .any(|cue| cue.cue_kind == "trigger" && cue.cue_text == "/"));
        assert!(agent
            .cues
            .iter()
            .any(|cue| cue.cue_kind == "trigger" && cue.cue_text == "@"));
        assert!(agent.cues.iter().any(|cue| {
            cue.cue_kind == "shortcut"
                && cue.cue_text == "⌘K"
                && cue.canonical_shortcut.as_deref() == Some("cmd+k")
                && cue.action_id == Some("agent-chat-open-actions")
        }));

        let active =
            launcher_empty_or_no_results_spec("type:script nope", true).semantic_snapshot();
        assert!(active.cues.iter().any(|cue| {
            cue.cue_kind == "label"
                && cue.cue_text == "Filter"
                && cue.canonical_shortcut.is_none()
                && cue.action_id.is_none()
        }));

        let tag = launcher_empty_or_no_results_spec("#work", false).semantic_snapshot();
        for syntax in [":#", ":tag:", ";todo"] {
            assert!(tag.cues.iter().any(|cue| {
                cue.cue_kind == "syntax"
                    && cue.cue_text == syntax
                    && cue.canonical_shortcut.is_none()
                    && cue.action_id.is_none()
            }));
        }
        assert_eq!(
            tag.cues
                .iter()
                .filter(|cue| cue.cue_kind == "syntax" && cue.cue_text == ";todo")
                .count(),
            1,
            ";todo must stay one semantic syntax element"
        );

        let generic = launcher_empty_or_no_results_spec("zzz", false).semantic_snapshot();
        for syntax in [":", "type:", ";todo"] {
            assert!(generic
                .cues
                .iter()
                .any(|cue| cue.cue_kind == "syntax" && cue.cue_text == syntax));
        }

        let permission = permission_onboarding_intro_spec(0, 2, false);
        for item in permission
            .sections
            .iter()
            .flat_map(|section| &section.items)
        {
            match &item.cue {
                InfoCue::Shortcut {
                    tokens,
                    canonical,
                    action_id,
                    ..
                } => {
                    assert!(!tokens.is_empty());
                    assert!(!canonical.is_empty());
                    assert!(!action_id.is_empty());
                }
                other => panic!("permission cue must be executable shortcut, got {other:?}"),
            }
        }
    }

    #[test]
    fn info_shortcut_constructor_rejects_syntax_triggers_labels_and_missing_ids() {
        for raw in [";todo", ":#", ":tag:", "type:", "/", "@", "Filter"] {
            assert_eq!(
                InfoGuidanceItem::try_shortcut(raw, "bad-action", "Bad cue"),
                Err(InfoCueValidationError::NonShortcutCue),
                "raw={raw}"
            );
        }
        assert_eq!(
            InfoGuidanceItem::try_shortcut("", "bad-action", "Empty"),
            Err(InfoCueValidationError::EmptyShortcut)
        );
        assert_eq!(
            InfoGuidanceItem::try_shortcut("⌘K", "", "Missing action"),
            Err(InfoCueValidationError::MissingActionId)
        );
    }

    #[test]
    fn info_tones_resolve_to_distinct_semantics_without_background_washes() {
        let cases = [
            (InfoStateTone::Neutral, "neutral", None, None),
            (
                InfoStateTone::Help,
                "help",
                Some("Help"),
                Some("circle-help"),
            ),
            (
                InfoStateTone::Setup,
                "setup",
                Some("Setup"),
                Some("settings"),
            ),
            (
                InfoStateTone::Permission,
                "permission",
                Some("Permission required"),
                Some("shield-check"),
            ),
            (
                InfoStateTone::Recovery,
                "recovery",
                Some("Recovery"),
                Some("circle-alert"),
            ),
            (InfoStateTone::About, "about", Some("About"), Some("info")),
        ];
        for (tone, kind, prefix, icon) in cases {
            let presentation = resolve_info_tone(tone);
            assert_eq!(presentation.semantic_kind, kind);
            assert_eq!(presentation.accessible_prefix, prefix);
            assert_eq!(presentation.default_icon_hint, icon);
            assert!(!presentation.background_wash);
        }
        assert_ne!(
            resolve_info_tone(InfoStateTone::Help).semantic_kind,
            resolve_info_tone(InfoStateTone::Recovery).semantic_kind
        );
    }

    #[test]
    fn semantic_snapshot_is_redacted_and_does_not_manufacture_actions() {
        let query = "private-search-canary";
        let spec = launcher_empty_or_no_results_spec(query, false);
        let snapshot = spec.semantic_snapshot();
        assert_eq!(snapshot.semantic_kind, "recovery");
        assert!(snapshot.title_present);
        assert!(snapshot.title_byte_len > query.len());
        assert!(snapshot.body_present);
        assert!(snapshot.actions.is_empty());
        let debug = format!("{snapshot:?}");
        assert!(!debug.contains(query));
    }

    #[test]
    fn agent_chat_empty_guidance_teaches_starting_context_not_window_management() {
        let spec = agent_chat_empty_guidance_spec();
        let copy = format!("{spec:?}");
        assert!(copy.contains("Ask with context"));
        assert!(copy.contains("Use a skill or agent command"));
        assert!(copy.contains("Attach files, scripts, clipboard, or history"));
        assert!(copy.contains("Add a newline"));
        assert!(copy.contains("Open previous chats"));
        assert!(copy.contains("Show every chat action"));
        assert!(spec.footer_shortcut_note.is_none());
        assert!(!copy.contains("Type / for skills"));
        assert!(!copy.contains(&format!("{} new", "⌘N")));
        assert!(!copy.contains(&format!("{} close", "⌘W")));
    }

    #[test]
    fn guidance_shortcut_slot_width_tracks_widest_measured_run() {
        let min_shortcut_width = crate::components::footer_chrome::FOOTER_KEYCAP_HEIGHT_PX * 2.0
            + crate::components::footer_chrome::FOOTER_ACTION_CONTENT_GAP_PX;

        // The widest measured keycap run wins.
        let wide = min_shortcut_width + 24.0;
        let width = info_guidance_cue_slot_width_from_widths([wide, 18.0, 30.0].into_iter());
        assert_eq!(width, wide);

        // Narrow runs are floored at two square keycaps plus a gap so rows
        // with short shortcuts still align with wider neighbors.
        let width = info_guidance_cue_slot_width_from_widths([10.0].into_iter());
        assert_eq!(width, min_shortcut_width);

        // No shortcuts at all collapses the slot entirely.
        assert_eq!(
            info_guidance_cue_slot_width_from_widths(std::iter::empty()),
            0.0
        );
    }

    #[test]
    fn agent_chat_empty_guidance_keeps_actions_shortcut_in_guidance_rows() {
        // Regression: keeping ⌘K as a separate footer note made the actions row
        // sit lower than the rest of the guidance list. It belongs in the same
        // guidance item stack so every row shares one spacing contract.
        let spec = agent_chat_empty_guidance_spec();
        assert!(
            spec.footer_shortcut_note.is_none(),
            "agent_chat guidance should not render a separate footer shortcut note"
        );
        let guidance_items: Vec<InfoGuidanceItem> = spec
            .sections
            .iter()
            .flat_map(|section| section.items.iter().cloned())
            .collect();
        assert!(
            guidance_items.iter().any(|item| {
                matches!(
                    &item.cue,
                    InfoCue::Shortcut { raw, action_id, .. }
                        if raw.as_ref() == "⌘K" && *action_id == "agent-chat-open-actions"
                ) && item.label.as_ref() == "Show every chat action"
            }),
            "⌘K actions guidance must be a normal guidance row"
        );
    }

    #[test]
    fn launcher_empty_guidance_teaches_library_and_next_actions() {
        let spec = launcher_empty_or_no_results_spec("", false);
        let copy = format!("{spec:?}");
        assert!(copy.contains("No scripts yet"));
        assert!(copy.contains("This launcher opens your Script Kit scripts and snippets"));
        assert!(copy.contains("Create a script"));
        assert!(copy.contains("Ask Agent Chat"));
        assert!(copy.contains("Open Actions"));
        assert!(!copy.contains("No scripts or snippets found"));
        assert!(!copy.contains("Press ⌘N to create a new script"));
    }

    #[test]
    fn launcher_no_results_preserves_active_filter_plain_hash_and_generic_cases() {
        let active = format!(
            "{:?}",
            launcher_empty_or_no_results_spec("type:script nope", true)
        );
        assert!(active.contains("No matches for"));
        assert!(active.contains("active filter is narrowing"));
        assert!(active.contains("Remove a filter chip"));
        assert!(active.contains("Source and type filters apply before fuzzy matching"));

        let tag = format!("{:?}", launcher_empty_or_no_results_spec("#work", false));
        assert!(tag.contains("Tags need a syntax prefix"));
        assert!(tag.contains("Plain #work is treated as launcher text search"));
        assert!(tag.contains("Example: :#work"));
        assert!(tag.contains("Example: :tag:work"));
        assert!(tag.contains("Example: ;todo Buy milk #errands"));

        let generic = format!("{:?}", launcher_empty_or_no_results_spec("zzz", false));
        assert!(generic.contains("No results for"));
        assert!(generic.contains("zzz"));
        assert!(generic.contains("scripts, scriptlets, snippets, and built-in commands"));
        assert!(generic.contains("type:script"));
        assert!(generic.contains("shortcut:cmd+k"));
        assert!(generic.contains("Ask Agent Chat"));
    }

    #[test]
    fn launcher_generic_no_results_offers_create_script_shortcut_without_old_prose() {
        let spec = launcher_empty_or_no_results_spec("zzz", false);
        let snapshot = spec.semantic_snapshot();

        assert!(snapshot.cues.iter().any(|cue| {
            cue.cue_kind == "shortcut"
                && cue.cue_text == "⌘N"
                && cue.canonical_shortcut.as_deref() == Some("cmd+n")
                && cue.action_id == Some("launcher-create-script")
        }));

        let copy = format!("{spec:?}");
        assert!(copy.contains("Create a script"));
        assert!(!copy.contains("Press ⌘N to create a new script"));
    }

    #[test]
    fn launcher_no_results_truncates_long_utf8_filter_display() {
        let input = "é".repeat(45);
        let spec = launcher_empty_or_no_results_spec(&input, false);
        let copy = format!("{spec:?}");
        assert!(copy.contains("..."));
        assert!(!copy.contains(&"é".repeat(45)));
    }

    #[test]
    fn launcher_empty_state_routes_through_info_state() {
        let source = std::fs::read_to_string("src/render_script_list/mod.rs")
            .expect("failed to read src/render_script_list/mod.rs");
        let old_empty_title = concat!("No scripts or ", "snippets found");
        let old_empty_hint = concat!("Press ", "⌘N", " to create a new script");
        let old_generic_fallback =
            concat!("Try a different search term or press ", "⌘↵", " to ask AI");

        assert!(
            source.contains("render_launcher_empty_or_no_results"),
            "launcher empty/no-results must render through shared InfoState"
        );
        assert!(
            !source.contains(old_empty_title),
            "old launcher empty title must not return"
        );
        assert!(
            !source.contains(old_empty_hint),
            "old launcher empty hint must not return"
        );
        assert!(
            !source.contains(old_generic_fallback),
            "old generic no-results fallback must not return"
        );
    }

    #[test]
    fn shared_empty_specs_cover_actions_arg_and_notes_without_surface_local_copy() {
        let actions_empty = format!(
            "{:?}",
            shared_empty_state_spec(InfoEmptySurface::ActionsSearch, "")
        );
        assert!(actions_empty.contains("No actions available"));

        let actions_filtered = format!(
            "{:?}",
            shared_empty_state_spec(InfoEmptySurface::ActionsSearch, "open")
        );
        assert!(actions_filtered.contains("No actions match your search"));

        let arg_filtered = shared_empty_state_spec(InfoEmptySurface::ArgChoices, "abc");
        assert_eq!(arg_filtered.layout, InfoStateLayout::InlineRow);
        let arg_copy = format!("{arg_filtered:?}");
        assert!(arg_copy.contains("No matches"));
        assert!(arg_copy.contains("Enter to submit typed value"));

        let notes_empty = format!(
            "{:?}",
            shared_empty_state_spec(InfoEmptySurface::NotesBrowse, "")
        );
        assert!(notes_empty.contains("No notes yet"));

        let notes_filtered = format!(
            "{:?}",
            shared_empty_state_spec(InfoEmptySurface::NotesBrowse, "meeting")
        );
        assert!(notes_filtered.contains("No notes match your filter"));
    }

    #[test]
    fn simple_builtin_empty_state_is_semantic_and_keeps_optional_hint() {
        let spec = simple_empty_state_spec(
            "process-manager-empty",
            "No running processes",
            "terminal",
            Some("Start a script to see it here"),
        );
        assert_eq!(spec.layout, InfoStateLayout::Centered);
        assert_eq!(spec.tone, InfoStateTone::Neutral);
        assert_eq!(spec.icon_hint, Some("terminal"));
        assert_eq!(
            spec.body.as_ref().map(|body| body.as_ref()),
            Some("No running processes")
        );
        assert_eq!(
            spec.footer_note.as_ref().map(|note| note.as_ref()),
            Some("Start a script to see it here")
        );
        let semantics = spec.semantic_snapshot();
        assert_eq!(semantics.semantic_kind, "neutral");
        assert_eq!(semantics.default_icon_hint, Some("terminal"));
        assert!(semantics.actions.is_empty());
    }

    #[test]
    fn permission_intro_spec_models_progress_and_completion_footer() {
        let pending = permission_onboarding_intro_spec(2, 5, false);
        let pending_copy = format!("{pending:?}");
        assert_eq!(pending.tone, InfoStateTone::Permission);
        assert!(pending_copy.contains("2 of 5 granted"));
        assert!(pending_copy.contains("Press Enter on a row to grant it"));
        assert!(pending_copy.contains("Grant selected permission"));
        assert!(pending_copy.contains("Finish setup"));

        let complete = permission_onboarding_intro_spec(5, 5, true);
        let complete_copy = format!("{complete:?}");
        assert!(complete_copy.contains("All required permissions granted"));
        assert!(complete_copy.contains("Press Esc to finish"));
    }

    #[test]
    fn agent_setup_spec_uses_shared_setup_anatomy_and_optional_selection_context() {
        let without_agent = agent_setup_info_spec(
            "Agent required",
            "Choose an agent to continue.",
            None::<SharedString>,
        );
        assert_eq!(without_agent.layout, InfoStateLayout::InlineRow);
        assert_eq!(without_agent.density, InfoStateDensity::Comfortable);
        assert_eq!(without_agent.tone, InfoStateTone::Setup);
        assert!(without_agent.eyebrow.is_none());

        let with_agent = agent_setup_info_spec(
            "Authentication required",
            "Authenticate, then retry.",
            Some("Claude Code"),
        );
        assert_eq!(
            with_agent.eyebrow.as_ref().map(|eyebrow| eyebrow.as_ref()),
            Some("Selected · Claude Code")
        );
    }
}
