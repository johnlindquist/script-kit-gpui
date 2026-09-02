//! Data-only fixture identities shared by the compiled catalogue and native constructors.

pub(crate) fn main_fixture_ids() -> &'static [&'static str] {
    &[
        "main.script-list",
        "main.root-search-frame-stability",
        "main-search-contract",
        "main.about",
        "main.clipboard-history",
        "main.app-launcher",
        "main.window-switcher",
        "main.browser-tabs",
        "main.file-search-mini",
        "main.file-search-full",
        "main.profile-search",
        "main.theme-chooser",
        "main.emoji-picker",
        "main.script-issues",
        "main.sdk-reference",
        "main.tips",
        "main.script-template-catalog",
        "main.browse-kits",
        "main.migrate-v1",
        "main.migrate-v1-scanning",
        "main.migrate-v1-porting",
        "main.migrate-v1-done",
        "main.migrate-v1-unavailable",
        "main.installed-kits",
        "main.process-manager",
        "main.search-ai-presets",
        "main.settings",
        "main.permissions-wizard",
        "main.favorites-browse",
        "main.current-app-commands",
        "main.agent-chat-history",
        "main.browser-history",
        "main.dictation-history",
        "main.notes-browse",
        "main.notes-browse-no-match",
        "main.notes-browse-loading",
        "main.notes-browse-failed",
        "main.notes-browse-empty",
        "main.menu-syntax-trigger",
        "main.menu-syntax-object",
        "main.menu-syntax-history",
        "main.browse-kits-loading",
        "main.browse-kits-failed",
    ]
}

pub(crate) const SHORTCUT_FIXTURE_IDS: &[&str] = &["secondary.shortcut-recorder"];

pub(crate) const MAIN_OVERLAY_FIXTURE_IDS: &[&str] = &[
    "main-overlay.alias",
    "main-overlay.tab-ai-save-offer",
    "main-overlay.root-dialog",
    "main-overlay.root-notification",
    "main-overlay.warning",
    "main-overlay.logs",
    "main-overlay.loading",
    "main-overlay.toast",
    "main-overlay.effects",
    "main-overlay.debug-grid",
];

pub(crate) const PROMPT_FIXTURE_IDS: &[&str] = &[
    "prompt.arg",
    "prompt.mini",
    "prompt.micro",
    "prompt.div",
    "prompt.form",
    "prompt.fields",
    "prompt.editor",
    "prompt.select",
    "prompt.path",
    "prompt.env",
    "prompt.drop",
    "prompt.template",
    "prompt.hotkey",
    "prompt.chat",
    "prompt.term",
    "prompt.naming",
    "prompt.confirm",
    "prompt.webcam",
    "prompt.scratch-pad",
    "prompt.quick-terminal",
    "prompt.create-preset",
    "prompt.creation-feedback",
];

pub(crate) const NOTES_FIXTURE_IDS: &[&str] = &["notes.editor"];
pub(crate) const NOTES_AUXILIARY_FIXTURE_IDS: &[&str] = &[
    "notes.actions",
    "notes.recent-switcher",
    "notes.root-dialog",
    "notes.root-notification",
];
pub(crate) const DAY_PAGE_FIXTURE_IDS: &[&str] = &[
    "day-page.today",
    "day-page.shelf",
    "day-page.fragment",
    "day-page.switcher",
];

pub(crate) const AGENT_CHAT_FIXTURE_IDS: &[&str] = &[
    "agent-chat.standard.empty",
    "agent-chat.standard.populated",
    "agent-chat.user-bold.awaiting-first-text",
    "agent-chat.role-split.streaming",
    "agent-chat.bottom-dock.stopped",
    "agent-chat.dense-log.retryable-failure",
    "agent-chat.sidecar.permission-pending",
    "agent-chat.focused-text-mini.populated",
    "agent-chat.quick-ai.empty",
    "agent-chat.standard.queued",
    "agent-chat.standard.picker-open",
    "agent-chat.initial-setup",
    "agent-chat.runtime-setup",
    "agent-chat.detached.retryable-failure",
];

pub(crate) const FLOW_FIXTURE_IDS: &[&str] = &[
    "flow.desk.flash",
    "flow.desk.dispatch",
    "flow.desk.lens",
    "flow.session",
];

pub(crate) const SDK_CHAT_FIXTURE_IDS: &[&str] = &[
    "sdk-chat.empty",
    "sdk-chat.streaming",
    "sdk-chat.retryable-failure",
];

pub(crate) const AGENT_CHAT_POPUP_FIXTURE_IDS: &[&str] = &[
    "agent-chat.popup.history",
    "agent-chat.popup.slash",
    "agent-chat.popup.profile",
];

pub(crate) const DICTATION_FIXTURE_IDS: &[&str] = &[
    "dictation.recording",
    "dictation.confirming",
    "dictation.transcribing",
    "dictation.delivering",
    "dictation.finished",
    "dictation.failed",
    "dictation.microphone-picker",
];

pub(crate) const SECONDARY_FIXTURE_IDS: &[&str] = &[
    "secondary.actions",
    "secondary.confirm",
    "secondary.confirm-three-button",
    "secondary.hud",
    "secondary.hud-action",
    "secondary.snap",
];

pub(crate) fn fixture_ids() -> impl Iterator<Item = &'static str> {
    main_fixture_ids()
        .iter()
        .copied()
        .chain(MAIN_OVERLAY_FIXTURE_IDS.iter().copied())
        .chain(PROMPT_FIXTURE_IDS.iter().copied())
        .chain(NOTES_FIXTURE_IDS.iter().copied())
        .chain(NOTES_AUXILIARY_FIXTURE_IDS.iter().copied())
        .chain(DAY_PAGE_FIXTURE_IDS.iter().copied())
        .chain(AGENT_CHAT_FIXTURE_IDS.iter().copied())
        .chain(FLOW_FIXTURE_IDS.iter().copied())
        .chain(SDK_CHAT_FIXTURE_IDS.iter().copied())
        .chain(AGENT_CHAT_POPUP_FIXTURE_IDS.iter().copied())
        .chain(DICTATION_FIXTURE_IDS.iter().copied())
        .chain(SECONDARY_FIXTURE_IDS.iter().copied())
        .chain(
            crate::footer_popup::OWNED_FOOTER_FIXTURE_IDS
                .iter()
                .copied(),
        )
        .chain(SHORTCUT_FIXTURE_IDS.iter().copied())
}
