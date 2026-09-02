use gpui::{Context, Window};

use crate::actions::{Action, ActionCategory};
use crate::designs::icon_variations::IconName;
use crate::scripts::SearchResult;
use crate::ScriptListApp;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RootUnifiedResultAction {
    FileOpen,
    FileRevealInFinder,
    FileCopyPath,
    FileCopyName,
    FileQuickLook,
    NoteOpen,
    NoteCopyTitle,
    NoteCopyId,
    ClipboardPaste,
    ClipboardCopy,
    ClipboardAttachToAi,
    ClipboardPin,
    ClipboardUnpin,
    ClipboardQuickLook,
    ClipboardDelete,
    BrowserTabSwitch,
    BrowserTabCopyUrl,
    BrowserTabCopyTitle,
    BrowserTabCopyTitleUrl,
    BrowserTabOpenUrl,
    BrowserHistoryOpen,
    BrowserHistoryCopyUrl,
    BrowserHistoryCopyTitle,
    BrowserHistoryCopyTitleUrl,
    AgentChatHistoryResume,
    AgentChatHistoryCopyTitle,
    AgentChatHistoryCopySessionId,
    AgentChatHistoryCopyPreview,
    AiVaultPasteResumeCommand,
    AiVaultCopyResumeCommand,
    AiVaultResumeConfiguredTerminal,
    AiVaultConfigureTerminal,
    AiVaultResumeQuickTerminal,
    AiVaultCopySessionId,
    AiVaultCopyProvider,
    AiVaultCopyWorkspacePath,
    AiVaultCopyTitle,
    AiVaultRevealInCmux,
    DictationPaste,
    DictationCopyTranscript,
    DictationAttachToAi,
    DictationCreateNote,
    DictationDelete,
    AppLaunch,
    AppRevealInFinder,
    AppCopyPath,
    AppCopyName,
    AppCopyBundleId,
    WindowSwitch,
    WindowCopyTitle,
    WindowCopyAppName,
    WindowCopyDescriptor,
    CommandRun,
    CommandCopyId,
    SkillOpen,
    SkillCopyId,
    SkillCopyPluginId,
    ScriptIssueInspect,
    ScriptIssueCopySummary,
}

impl RootUnifiedResultAction {
    pub(crate) const ALL: &'static [RootUnifiedResultAction] = &[
        Self::FileOpen,
        Self::FileRevealInFinder,
        Self::FileCopyPath,
        Self::FileCopyName,
        Self::FileQuickLook,
        Self::NoteOpen,
        Self::NoteCopyTitle,
        Self::NoteCopyId,
        Self::ClipboardPaste,
        Self::ClipboardCopy,
        Self::ClipboardAttachToAi,
        Self::ClipboardPin,
        Self::ClipboardUnpin,
        Self::ClipboardQuickLook,
        Self::ClipboardDelete,
        Self::BrowserTabSwitch,
        Self::BrowserTabCopyUrl,
        Self::BrowserTabCopyTitle,
        Self::BrowserTabCopyTitleUrl,
        Self::BrowserTabOpenUrl,
        Self::BrowserHistoryOpen,
        Self::BrowserHistoryCopyUrl,
        Self::BrowserHistoryCopyTitle,
        Self::BrowserHistoryCopyTitleUrl,
        Self::AgentChatHistoryResume,
        Self::AgentChatHistoryCopyTitle,
        Self::AgentChatHistoryCopySessionId,
        Self::AgentChatHistoryCopyPreview,
        Self::AiVaultPasteResumeCommand,
        Self::AiVaultCopyResumeCommand,
        Self::AiVaultResumeConfiguredTerminal,
        Self::AiVaultConfigureTerminal,
        Self::AiVaultResumeQuickTerminal,
        Self::AiVaultCopySessionId,
        Self::AiVaultCopyProvider,
        Self::AiVaultCopyWorkspacePath,
        Self::AiVaultCopyTitle,
        Self::AiVaultRevealInCmux,
        Self::DictationPaste,
        Self::DictationCopyTranscript,
        Self::DictationAttachToAi,
        Self::DictationCreateNote,
        Self::DictationDelete,
        Self::AppLaunch,
        Self::AppRevealInFinder,
        Self::AppCopyPath,
        Self::AppCopyName,
        Self::AppCopyBundleId,
        Self::WindowSwitch,
        Self::WindowCopyTitle,
        Self::WindowCopyAppName,
        Self::WindowCopyDescriptor,
        Self::CommandRun,
        Self::CommandCopyId,
        Self::SkillOpen,
        Self::SkillCopyId,
        Self::SkillCopyPluginId,
        Self::ScriptIssueInspect,
        Self::ScriptIssueCopySummary,
    ];

    pub(crate) fn action_id(self) -> &'static str {
        match self {
            Self::FileOpen => crate::action_helpers::ROOT_FILE_OPEN_ACTION_ID,
            Self::FileRevealInFinder => crate::action_helpers::ROOT_FILE_REVEAL_IN_FINDER_ACTION_ID,
            Self::FileCopyPath => crate::action_helpers::ROOT_FILE_COPY_PATH_ACTION_ID,
            Self::FileCopyName => crate::action_helpers::ROOT_FILE_COPY_NAME_ACTION_ID,
            Self::FileQuickLook => crate::action_helpers::ROOT_FILE_QUICK_LOOK_ACTION_ID,
            Self::NoteOpen => "root_note_open",
            Self::NoteCopyTitle => "root_note_copy_title",
            Self::NoteCopyId => "root_note_copy_id",
            Self::ClipboardPaste => "root_clipboard_paste",
            Self::ClipboardCopy => "root_clipboard_copy",
            Self::ClipboardAttachToAi => "root_clipboard_attach_to_ai",
            Self::ClipboardPin => "root_clipboard_pin",
            Self::ClipboardUnpin => "root_clipboard_unpin",
            Self::ClipboardQuickLook => "root_clipboard_quick_look",
            Self::ClipboardDelete => "root_clipboard_delete",
            Self::BrowserTabSwitch => "root_browser_tab_switch",
            Self::BrowserTabCopyUrl => "root_browser_tab_copy_url",
            Self::BrowserTabCopyTitle => "root_browser_tab_copy_title",
            Self::BrowserTabCopyTitleUrl => "root_browser_tab_copy_title_url",
            Self::BrowserTabOpenUrl => "root_browser_tab_open_url",
            Self::BrowserHistoryOpen => "root_browser_history_open",
            Self::BrowserHistoryCopyUrl => "root_browser_history_copy_url",
            Self::BrowserHistoryCopyTitle => "root_browser_history_copy_title",
            Self::BrowserHistoryCopyTitleUrl => "root_browser_history_copy_title_url",
            Self::AgentChatHistoryResume => "root_agent_chat_history_resume",
            Self::AgentChatHistoryCopyTitle => "root_agent_chat_history_copy_title",
            Self::AgentChatHistoryCopySessionId => "root_agent_chat_history_copy_session_id",
            Self::AgentChatHistoryCopyPreview => "root_agent_chat_history_copy_preview",
            Self::AiVaultPasteResumeCommand => "root_ai_vault_paste_resume_command",
            Self::AiVaultCopyResumeCommand => "root_ai_vault_copy_resume_command",
            Self::AiVaultResumeConfiguredTerminal => "root_ai_vault_resume_configured_terminal",
            Self::AiVaultConfigureTerminal => "root_ai_vault_configure_terminal",
            Self::AiVaultResumeQuickTerminal => "root_ai_vault_resume_quick_terminal",
            Self::AiVaultCopySessionId => "root_ai_vault_copy_session_id",
            Self::AiVaultCopyProvider => "root_ai_vault_copy_provider",
            Self::AiVaultCopyWorkspacePath => "root_ai_vault_copy_workspace_path",
            Self::AiVaultCopyTitle => "root_ai_vault_copy_title",
            Self::AiVaultRevealInCmux => "root_ai_vault_reveal_in_cmux",
            Self::DictationPaste => "root_dictation_paste",
            Self::DictationCopyTranscript => "root_dictation_copy_transcript",
            Self::DictationAttachToAi => "root_dictation_attach_to_ai",
            Self::DictationCreateNote => "root_dictation_create_note",
            Self::DictationDelete => "root_dictation_delete",
            Self::AppLaunch => "root_app_launch",
            Self::AppRevealInFinder => "root_app_reveal_in_finder",
            Self::AppCopyPath => "root_app_copy_path",
            Self::AppCopyName => "root_app_copy_name",
            Self::AppCopyBundleId => "root_app_copy_bundle_id",
            Self::WindowSwitch => "root_window_switch",
            Self::WindowCopyTitle => "root_window_copy_title",
            Self::WindowCopyAppName => "root_window_copy_app_name",
            Self::WindowCopyDescriptor => "root_window_copy_descriptor",
            Self::CommandRun => "root_command_run",
            Self::CommandCopyId => "root_command_copy_id",
            Self::SkillOpen => "root_skill_open",
            Self::SkillCopyId => "root_skill_copy_id",
            Self::SkillCopyPluginId => "root_skill_copy_plugin_id",
            Self::ScriptIssueInspect => "root_script_issue_inspect",
            Self::ScriptIssueCopySummary => "root_script_issue_copy_summary",
        }
    }

    pub(crate) fn from_action_id(id: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|action| action.action_id() == id)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum RootUnifiedResultActionOwner {
    RootSubject(Box<RootUnifiedActionSubject>),
    ExistingScriptActions,
    None,
}

#[derive(Clone, Debug)]
pub(crate) enum RootUnifiedActionSubject {
    File(crate::file_search::FileResult),
    Note {
        id: crate::notes::NoteId,
        title: String,
    },
    Clipboard(crate::clipboard_history::ClipboardEntryMeta),
    BrowserTab(crate::browser_tabs::RootBrowserTabSearchHit),
    BrowserHistory(crate::browser_history::RootBrowserHistorySearchHit),
    AgentChatHistory(crate::ai::agent_chat::ui::history::AgentChatHistoryEntry),
    AiVault(crate::ai_vault::AiVaultHit),
    Dictation {
        id: String,
        preview: String,
    },
    App(crate::app_launcher::AppInfo),
    Window(crate::window_control::WindowInfo),
    BuiltIn(crate::builtins::BuiltInEntry),
    Skill(std::sync::Arc<crate::plugins::PluginSkill>),
    ScriptIssue(crate::scripts::ScriptIssueMatch),
}

impl RootUnifiedActionSubject {
    pub(crate) fn context_title(&self) -> String {
        match self {
            Self::File(file) => file.name.clone(),
            Self::Note { title, .. } => title.clone(),
            Self::Clipboard(entry) => entry.text_preview.clone(),
            Self::BrowserTab(hit) => hit.title.clone(),
            Self::BrowserHistory(hit) => hit.title.clone(),
            Self::AgentChatHistory(entry) => entry.title_display().to_string(),
            Self::AiVault(_) => "AI Vault Conversation".to_string(),
            Self::Dictation { preview, .. } => preview.clone(),
            Self::App(app) => app.name.clone(),
            Self::Window(window) => window.title.clone(),
            Self::BuiltIn(entry) => entry.name.clone(),
            Self::Skill(skill) => skill.title.clone(),
            Self::ScriptIssue(issue) => issue.title.clone(),
        }
    }

    pub(crate) fn stable_key(&self) -> Option<String> {
        match self {
            Self::File(file) => Some(format!("file/{}", file.path)),
            Self::Note { id, .. } => Some(format!("note/{}", id.as_str())),
            Self::Clipboard(entry) => Some(format!("clipboard-history/{}", entry.id)),
            Self::BrowserTab(hit) => Some(hit.stable_key.clone()),
            Self::BrowserHistory(hit) => Some(hit.stable_key.clone()),
            Self::AgentChatHistory(entry) => {
                Some(format!("agent_chat-history/{}", entry.session_id))
            }
            Self::AiVault(hit) => Some(hit.stable_key.clone()),
            Self::Dictation { id, .. } => Some(format!("dictation-history/{id}")),
            Self::App(app) => Some(
                app.bundle_id
                    .as_ref()
                    .map(|bundle_id| format!("app/{bundle_id}"))
                    .unwrap_or_else(|| {
                        format!("app/{}", app.name.to_lowercase().replace(' ', "-"))
                    }),
            ),
            Self::Window(window) => Some(format!("window:{}:{}", window.app, window.title)),
            Self::BuiltIn(entry) => {
                if entry.id.starts_with("builtin/") {
                    Some(entry.id.clone())
                } else {
                    Some(format!("builtin/{}", entry.id))
                }
            }
            Self::Skill(skill) => Some(format!("skill:{}:{}", skill.plugin_id, skill.skill_id)),
            Self::ScriptIssue(issue) => Some(format!(
                "script-issue/{}:{}:{}:{}",
                issue.title, issue.failed_count, issue.fatal_count, issue.warning_count
            )),
        }
    }

    pub(crate) fn source_name(&self) -> &'static str {
        match self {
            Self::File(_) => "Files",
            Self::Note { .. } => "Notes",
            Self::Clipboard(_) => "Clipboard History",
            Self::BrowserTab(_) => "Browser Tabs",
            Self::BrowserHistory(_) => "Browser History",
            Self::AgentChatHistory(_) => "Agent Chat Conversations",
            Self::AiVault(_) => "AI Vault",
            Self::Dictation { .. } => "Dictation History",
            Self::App(_) => "Apps",
            Self::Window(_) => "Windows",
            Self::BuiltIn(_) | Self::Skill(_) | Self::ScriptIssue(_) => "Commands",
        }
    }
}

pub(crate) fn root_unified_action_owner_for_result(
    result: &SearchResult,
) -> RootUnifiedResultActionOwner {
    match result {
        // App and BuiltIn rows deliberately delegate to the legacy script
        // actions menu: for apps it is a strict superset of the root subject
        // arm (Finder section, favorites, quit/restart/force-quit), and for
        // built-ins it carries Show Info plus the global actions. The App and
        // BuiltIn arms in `root_unified_action_subject_from_result` /
        // `root_unified_actions_for_subject` below stay as the complete
        // subject matrix (locked by
        // tests/source_audits/root_unified_source_actions_contract.rs) but
        // are not reachable from this owner routing.
        SearchResult::Script(_)
        | SearchResult::Scriptlet(_)
        | SearchResult::BuiltIn(_)
        | SearchResult::App(_) => RootUnifiedResultActionOwner::ExistingScriptActions,
        _ => root_unified_action_subject_from_result(result)
            .map(|subject| RootUnifiedResultActionOwner::RootSubject(Box::new(subject)))
            .unwrap_or(RootUnifiedResultActionOwner::None),
    }
}

pub(crate) fn root_unified_action_subject_from_result(
    result: &SearchResult,
) -> Option<RootUnifiedActionSubject> {
    match result {
        SearchResult::File(file) => Some(RootUnifiedActionSubject::File(file.file.clone())),
        SearchResult::Note(note) => Some(RootUnifiedActionSubject::Note {
            id: note.hit.id,
            title: note.title.clone(),
        }),
        SearchResult::ClipboardHistory(entry) => {
            Some(RootUnifiedActionSubject::Clipboard(entry.entry.clone()))
        }
        SearchResult::BrowserTab(tab) => {
            Some(RootUnifiedActionSubject::BrowserTab(tab.hit.clone()))
        }
        SearchResult::BrowserHistory(history) => Some(RootUnifiedActionSubject::BrowserHistory(
            history.hit.clone(),
        )),
        SearchResult::AgentChatHistory(history) => Some(
            RootUnifiedActionSubject::AgentChatHistory(history.entry.clone()),
        ),
        SearchResult::AiVault(ai_vault) => {
            Some(RootUnifiedActionSubject::AiVault(ai_vault.hit.clone()))
        }
        SearchResult::DictationHistory(dictation) => Some(RootUnifiedActionSubject::Dictation {
            id: dictation.id.clone(),
            preview: dictation.preview.clone(),
        }),
        SearchResult::App(app) => Some(RootUnifiedActionSubject::App(app.app.clone())),
        SearchResult::Window(window) => {
            Some(RootUnifiedActionSubject::Window(window.window.clone()))
        }
        SearchResult::BuiltIn(builtin) => {
            Some(RootUnifiedActionSubject::BuiltIn(builtin.entry.clone()))
        }
        SearchResult::Skill(skill) => Some(RootUnifiedActionSubject::Skill(skill.skill.clone())),
        SearchResult::ScriptIssue(issue) => {
            Some(RootUnifiedActionSubject::ScriptIssue(issue.clone()))
        }
        // Flows use the script-context action path (reveal/copy path work via
        // action_helpers); no dedicated root-unified subject yet.
        SearchResult::Flow(_)
        | SearchResult::Script(_)
        | SearchResult::Scriptlet(_)
        | SearchResult::Todo(_)
        | SearchResult::BrainHit(_)
        | SearchResult::BrainInboxItem(_)
        | SearchResult::Agent(_)
        | SearchResult::Fallback(_)
        | SearchResult::SpineProjection(_) => None,
    }
}

pub(crate) fn root_unified_actions_for_subject(subject: &RootUnifiedActionSubject) -> Vec<Action> {
    match subject {
        RootUnifiedActionSubject::File(file) => super::actions_toggle::root_file_actions_for(file),
        RootUnifiedActionSubject::Note { .. } => vec![
            action(RootUnifiedResultAction::NoteOpen, "Open Note", "Open"),
            action(
                RootUnifiedResultAction::NoteCopyTitle,
                "Copy Note Title",
                "Share",
            ),
            action(RootUnifiedResultAction::NoteCopyId, "Copy Note ID", "Share"),
        ],
        RootUnifiedActionSubject::Clipboard(entry) => {
            let pin_action = if entry.pinned {
                action(RootUnifiedResultAction::ClipboardUnpin, "Unpin", "Actions")
            } else {
                action(RootUnifiedResultAction::ClipboardPin, "Pin", "Actions")
            };
            let mut actions = vec![
                action(
                    RootUnifiedResultAction::ClipboardPaste,
                    "Paste Clipboard",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::ClipboardCopy,
                    "Copy to Clipboard",
                    "Share",
                ),
            ];
            if matches!(
                entry.content_type,
                crate::clipboard_history::ContentType::Text
                    | crate::clipboard_history::ContentType::Link
                    | crate::clipboard_history::ContentType::Color
            ) {
                actions.push(action(
                    RootUnifiedResultAction::ClipboardAttachToAi,
                    "Attach to Agent Chat",
                    "Share",
                ));
            }
            actions.extend([
                pin_action,
                action(
                    RootUnifiedResultAction::ClipboardQuickLook,
                    "Quick Look",
                    "Actions",
                ),
                action(
                    RootUnifiedResultAction::ClipboardDelete,
                    "Delete Clipboard Entry",
                    "Danger",
                ),
            ]);
            actions
        }
        RootUnifiedActionSubject::BrowserTab(_) => vec![
            action(
                RootUnifiedResultAction::BrowserTabSwitch,
                "Switch to Tab",
                "Open",
            ),
            action(
                RootUnifiedResultAction::BrowserTabOpenUrl,
                "Open URL in Browser",
                "Open",
            ),
            action(
                RootUnifiedResultAction::BrowserTabCopyUrl,
                "Copy URL",
                "Share",
            ),
            action(
                RootUnifiedResultAction::BrowserTabCopyTitle,
                "Copy Title",
                "Share",
            ),
            action(
                RootUnifiedResultAction::BrowserTabCopyTitleUrl,
                "Copy Title and URL",
                "Share",
            ),
        ],
        RootUnifiedActionSubject::BrowserHistory(_) => vec![
            action(
                RootUnifiedResultAction::BrowserHistoryOpen,
                "Open Page",
                "Open",
            ),
            action(
                RootUnifiedResultAction::BrowserHistoryCopyUrl,
                "Copy URL",
                "Share",
            ),
            action(
                RootUnifiedResultAction::BrowserHistoryCopyTitle,
                "Copy Title",
                "Share",
            ),
            action(
                RootUnifiedResultAction::BrowserHistoryCopyTitleUrl,
                "Copy Title and URL",
                "Share",
            ),
        ],
        RootUnifiedActionSubject::AgentChatHistory(entry) => {
            let mut actions = vec![
                action(
                    RootUnifiedResultAction::AgentChatHistoryResume,
                    "Resume Conversation",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::AgentChatHistoryCopyTitle,
                    "Copy Conversation Title",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AgentChatHistoryCopySessionId,
                    "Copy Session ID",
                    "Share",
                ),
            ];
            if !entry.preview_display().is_empty() {
                actions.push(action(
                    RootUnifiedResultAction::AgentChatHistoryCopyPreview,
                    "Copy Preview",
                    "Share",
                ));
            }
            actions
        }
        RootUnifiedActionSubject::AiVault(hit) => {
            let mut actions = vec![
                action(
                    RootUnifiedResultAction::AiVaultPasteResumeCommand,
                    "Paste Resume Command",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::AiVaultCopyResumeCommand,
                    "Copy Resume Command",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AiVaultResumeConfiguredTerminal,
                    "Resume in Configured Terminal",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::AiVaultConfigureTerminal,
                    "Configure Terminal",
                    "Settings",
                ),
                action(
                    RootUnifiedResultAction::AiVaultResumeQuickTerminal,
                    "Resume in Quick Terminal",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::AiVaultCopySessionId,
                    "Copy Session ID",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AiVaultCopyProvider,
                    "Copy Provider",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AiVaultCopyTitle,
                    "Copy Title",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AiVaultRevealInCmux,
                    "Reveal in cmux",
                    "Open",
                ),
            ];
            if hit.workspace_path.is_some() {
                actions.insert(
                    4,
                    action(
                        RootUnifiedResultAction::AiVaultCopyWorkspacePath,
                        "Copy Workspace Path",
                        "Share",
                    ),
                );
            }
            actions
        }
        RootUnifiedActionSubject::Dictation { .. } => vec![
            action(
                RootUnifiedResultAction::DictationPaste,
                "Paste Dictation",
                "Open",
            ),
            action(
                RootUnifiedResultAction::DictationCopyTranscript,
                "Copy Transcript",
                "Share",
            ),
            action(
                RootUnifiedResultAction::DictationAttachToAi,
                "Add to Agent Chat",
                "Share",
            ),
            action(
                RootUnifiedResultAction::DictationCreateNote,
                "Create Note from Transcript",
                "Actions",
            ),
            action(
                RootUnifiedResultAction::DictationDelete,
                "Delete Dictation",
                "Danger",
            ),
        ],
        RootUnifiedActionSubject::App(app) => {
            let mut actions = vec![
                action(RootUnifiedResultAction::AppLaunch, "Launch App", "Open"),
                action(
                    RootUnifiedResultAction::AppRevealInFinder,
                    "Reveal in Finder",
                    "Open",
                ),
                action(
                    RootUnifiedResultAction::AppCopyPath,
                    "Copy App Path",
                    "Share",
                ),
                action(
                    RootUnifiedResultAction::AppCopyName,
                    "Copy App Name",
                    "Share",
                ),
            ];
            if app.bundle_id.is_some() {
                actions.push(action(
                    RootUnifiedResultAction::AppCopyBundleId,
                    "Copy Bundle ID",
                    "Share",
                ));
            }
            actions
        }
        RootUnifiedActionSubject::Window(_) => vec![
            action(
                RootUnifiedResultAction::WindowSwitch,
                "Switch to Window",
                "Open",
            ),
            action(
                RootUnifiedResultAction::WindowCopyTitle,
                "Copy Window Title",
                "Share",
            ),
            action(
                RootUnifiedResultAction::WindowCopyAppName,
                "Copy App Name",
                "Share",
            ),
            action(
                RootUnifiedResultAction::WindowCopyDescriptor,
                "Copy Window Descriptor",
                "Share",
            ),
        ],
        RootUnifiedActionSubject::BuiltIn(_) => vec![
            action(RootUnifiedResultAction::CommandRun, "Run Command", "Open"),
            action(
                RootUnifiedResultAction::CommandCopyId,
                "Copy Command ID",
                "Share",
            ),
        ],
        RootUnifiedActionSubject::Skill(_) => vec![
            action(RootUnifiedResultAction::SkillOpen, "Open Skill", "Open"),
            action(
                RootUnifiedResultAction::SkillCopyId,
                "Copy Skill ID",
                "Share",
            ),
            action(
                RootUnifiedResultAction::SkillCopyPluginId,
                "Copy Plugin ID",
                "Share",
            ),
        ],
        RootUnifiedActionSubject::ScriptIssue(_) => vec![
            action(
                RootUnifiedResultAction::ScriptIssueInspect,
                "Inspect Issues",
                "Open",
            ),
            action(
                RootUnifiedResultAction::ScriptIssueCopySummary,
                "Copy Issue Summary",
                "Share",
            ),
        ],
    }
}

fn action(id: RootUnifiedResultAction, title: &str, section: &str) -> Action {
    let icon = match section {
        "Open" => IconName::PlayFilled,
        "Share" => IconName::Copy,
        "Danger" => IconName::Trash,
        _ => IconName::Settings,
    };
    Action::new(id.action_id(), title, None, ActionCategory::ScriptContext)
        .with_section(section)
        .with_icon(icon)
}

pub(crate) fn execute_root_unified_result_action(
    app: &mut ScriptListApp,
    action_id: &str,
    subject: &RootUnifiedActionSubject,
    window: &mut Window,
    cx: &mut Context<ScriptListApp>,
) -> bool {
    let Some(action) = RootUnifiedResultAction::from_action_id(action_id) else {
        tracing::warn!(
            target: "script_kit::actions",
            event = "unknown_root_unified_result_action",
            action_id,
            "Unknown root result action ignored"
        );
        return true;
    };

    match (action, subject) {
        (
            RootUnifiedResultAction::FileOpen
            | RootUnifiedResultAction::FileRevealInFinder
            | RootUnifiedResultAction::FileCopyPath
            | RootUnifiedResultAction::FileCopyName
            | RootUnifiedResultAction::FileQuickLook,
            RootUnifiedActionSubject::File(file),
        ) => app.execute_root_file_action(action_id, file, window, cx),
        (RootUnifiedResultAction::NoteOpen, RootUnifiedActionSubject::Note { id, .. }) => {
            let _ = app.execute_root_note_open(*id, cx);
            true
        }
        (RootUnifiedResultAction::NoteCopyTitle, RootUnifiedActionSubject::Note { title, .. }) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(title.clone()));
            true
        }
        (RootUnifiedResultAction::NoteCopyId, RootUnifiedActionSubject::Note { id, .. }) => {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(id.as_str()));
            true
        }
        (RootUnifiedResultAction::ClipboardPaste, RootUnifiedActionSubject::Clipboard(entry)) => {
            app.execute_root_clipboard_history_paste(&entry.id, cx);
            true
        }
        (RootUnifiedResultAction::ClipboardCopy, RootUnifiedActionSubject::Clipboard(entry)) => {
            if let Err(error) = crate::clipboard_history::copy_entry_to_clipboard(&entry.id) {
                app.show_hud(
                    format!("Failed to copy clipboard entry: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (
            RootUnifiedResultAction::ClipboardAttachToAi,
            RootUnifiedActionSubject::Clipboard(entry),
        ) => {
            let text = crate::clipboard_history::get_entry_content(&entry.id)
                .unwrap_or_else(|| entry.text_preview.clone());
            app.submit_to_current_or_new_tab_ai_harness_from_text(
                text,
                crate::ai::TabAiQuickSubmitSource::Fallback,
                cx,
            );
            true
        }
        (RootUnifiedResultAction::ClipboardPin, RootUnifiedActionSubject::Clipboard(entry)) => {
            if let Err(error) = crate::clipboard_history::pin_entry(&entry.id) {
                app.show_hud(
                    format!("Failed to pin clipboard entry: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (RootUnifiedResultAction::ClipboardUnpin, RootUnifiedActionSubject::Clipboard(entry)) => {
            if let Err(error) = crate::clipboard_history::unpin_entry(&entry.id) {
                app.show_hud(
                    format!("Failed to unpin clipboard entry: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (
            RootUnifiedResultAction::ClipboardQuickLook,
            RootUnifiedActionSubject::Clipboard(entry),
        ) => {
            if let Err(error) = crate::clipboard_history::quick_look_entry(entry) {
                app.show_hud(
                    format!("Failed to preview clipboard entry: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (RootUnifiedResultAction::ClipboardDelete, RootUnifiedActionSubject::Clipboard(entry)) => {
            if let Err(error) = crate::clipboard_history::remove_entry(&entry.id) {
                app.show_hud(
                    format!("Failed to delete clipboard entry: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (RootUnifiedResultAction::BrowserTabSwitch, RootUnifiedActionSubject::BrowserTab(hit)) => {
            app.execute_root_browser_tab_switch(hit, cx);
            true
        }
        (RootUnifiedResultAction::BrowserTabOpenUrl, RootUnifiedActionSubject::BrowserTab(hit)) => {
            app.execute_root_browser_history_open(&hit.url, cx);
            true
        }
        (RootUnifiedResultAction::BrowserTabCopyUrl, RootUnifiedActionSubject::BrowserTab(hit)) => {
            copy(hit.url.clone(), cx)
        }
        (
            RootUnifiedResultAction::BrowserTabCopyTitle,
            RootUnifiedActionSubject::BrowserTab(hit),
        ) => copy(hit.title.clone(), cx),
        (
            RootUnifiedResultAction::BrowserTabCopyTitleUrl,
            RootUnifiedActionSubject::BrowserTab(hit),
        ) => copy(format!("{} — {}", hit.title, hit.url), cx),
        (
            RootUnifiedResultAction::BrowserHistoryOpen,
            RootUnifiedActionSubject::BrowserHistory(hit),
        ) => {
            app.execute_root_browser_history_open(&hit.url, cx);
            true
        }
        (
            RootUnifiedResultAction::BrowserHistoryCopyUrl,
            RootUnifiedActionSubject::BrowserHistory(hit),
        ) => copy(hit.url.clone(), cx),
        (
            RootUnifiedResultAction::BrowserHistoryCopyTitle,
            RootUnifiedActionSubject::BrowserHistory(hit),
        ) => copy(hit.title.clone(), cx),
        (
            RootUnifiedResultAction::BrowserHistoryCopyTitleUrl,
            RootUnifiedActionSubject::BrowserHistory(hit),
        ) => copy(format!("{} — {}", hit.title, hit.url), cx),
        (
            RootUnifiedResultAction::AgentChatHistoryResume,
            RootUnifiedActionSubject::AgentChatHistory(entry),
        ) => {
            app.resume_agent_chat_conversation_from_history(
                &entry.session_id,
                entry.first_message.as_str(),
                cx,
            );
            true
        }
        (
            RootUnifiedResultAction::AgentChatHistoryCopyTitle,
            RootUnifiedActionSubject::AgentChatHistory(entry),
        ) => copy(entry.title_display().to_string(), cx),
        (
            RootUnifiedResultAction::AgentChatHistoryCopySessionId,
            RootUnifiedActionSubject::AgentChatHistory(entry),
        ) => copy(entry.session_id.clone(), cx),
        (
            RootUnifiedResultAction::AgentChatHistoryCopyPreview,
            RootUnifiedActionSubject::AgentChatHistory(entry),
        ) => copy(entry.preview_display().to_string(), cx),
        (
            RootUnifiedResultAction::AiVaultPasteResumeCommand,
            RootUnifiedActionSubject::AiVault(hit),
        ) => {
            app.execute_root_ai_vault_paste_resume_command(hit, cx);
            true
        }
        (
            RootUnifiedResultAction::AiVaultCopyResumeCommand,
            RootUnifiedActionSubject::AiVault(hit),
        ) => {
            app.execute_root_ai_vault_copy_resume_command(hit, cx);
            true
        }
        (
            RootUnifiedResultAction::AiVaultResumeConfiguredTerminal,
            RootUnifiedActionSubject::AiVault(hit),
        ) => {
            app.execute_root_ai_vault_resume_configured_terminal(hit, cx);
            true
        }
        (
            RootUnifiedResultAction::AiVaultConfigureTerminal,
            RootUnifiedActionSubject::AiVault(_),
        ) => {
            app.execute_root_ai_vault_configure_terminal(cx);
            true
        }
        (
            RootUnifiedResultAction::AiVaultResumeQuickTerminal,
            RootUnifiedActionSubject::AiVault(hit),
        ) => {
            app.execute_root_ai_vault_resume_quick_terminal(hit, cx);
            true
        }
        (RootUnifiedResultAction::AiVaultCopySessionId, RootUnifiedActionSubject::AiVault(hit)) => {
            copy(hit.session_id.clone(), cx)
        }
        (RootUnifiedResultAction::AiVaultCopyProvider, RootUnifiedActionSubject::AiVault(hit)) => {
            copy(hit.provider.clone(), cx)
        }
        (
            RootUnifiedResultAction::AiVaultCopyWorkspacePath,
            RootUnifiedActionSubject::AiVault(hit),
        ) => copy(hit.workspace_path.clone().unwrap_or_default(), cx),
        (RootUnifiedResultAction::AiVaultCopyTitle, RootUnifiedActionSubject::AiVault(hit)) => {
            copy(hit.safe_title.clone(), cx)
        }
        (RootUnifiedResultAction::AiVaultRevealInCmux, RootUnifiedActionSubject::AiVault(hit)) => {
            let receipt = crate::ai_vault::reveal_vault_session(hit);
            app.show_hud(
                vault_receipt_hud("AI Vault reveal", &receipt),
                Some(crate::HUD_MEDIUM_MS),
                cx,
            );
            true
        }
        (
            RootUnifiedResultAction::DictationPaste,
            RootUnifiedActionSubject::Dictation { id, .. },
        ) => {
            app.execute_root_dictation_history_paste(id, cx);
            true
        }
        (
            RootUnifiedResultAction::DictationCopyTranscript,
            RootUnifiedActionSubject::Dictation { id, .. },
        ) => {
            if let Some(entry) = crate::dictation::get_history_entry(id) {
                copy(entry.transcript, cx)
            } else {
                true
            }
        }
        (
            RootUnifiedResultAction::DictationAttachToAi,
            RootUnifiedActionSubject::Dictation { id, .. },
        ) => {
            if let Some(entry) = crate::dictation::get_history_entry(id) {
                app.open_tab_ai_agent_chat_with_context_part(
                    crate::ai::AiContextPart::ResourceUri {
                        uri: entry.resource_uri(),
                        label: format!("Dictation: {}", entry.preview),
                    },
                    "root_dictation_history_add",
                    cx,
                );
            }
            true
        }
        (
            RootUnifiedResultAction::DictationCreateNote,
            RootUnifiedActionSubject::Dictation { id, .. },
        ) => {
            if let Some(entry) = crate::dictation::get_history_entry(id) {
                if let Err(error) = crate::notes::save_note_with_content(cx, entry.transcript) {
                    app.show_hud(
                        format!("Failed to create note: {error}"),
                        Some(crate::HUD_MEDIUM_MS),
                        cx,
                    );
                }
            }
            true
        }
        (
            RootUnifiedResultAction::DictationDelete,
            RootUnifiedActionSubject::Dictation { id, .. },
        ) => {
            let pending = app.dictation_history_attachment_is_pending(id, cx);
            let body = crate::dictation::delete_history_confirmation_body(pending);
            let entry_id = id.clone();
            let owner = cx.entity().downgrade();
            let owner_for_confirm = owner.clone();
            app.was_window_focused = true;
            crate::confirm::open_parent_confirm_dialog_for_entity(
                window,
                cx,
                owner,
                crate::confirm::ParentConfirmOptions::destructive(
                    "Delete Dictation?",
                    body,
                    "Delete",
                ),
                move |_window, cx| {
                    if let Some(entity) = owner_for_confirm.upgrade() {
                        entity.update(cx, |app, cx| {
                            app.delete_dictation_history_entry_confirmed(&entry_id, cx);
                        });
                    }
                },
                |_window, _cx| {},
            );
            true
        }
        (RootUnifiedResultAction::AppLaunch, RootUnifiedActionSubject::App(app_info)) => {
            app.execute_app(app_info, cx);
            true
        }
        (RootUnifiedResultAction::AppRevealInFinder, RootUnifiedActionSubject::App(app_info)) => {
            if let Err(error) =
                crate::file_search::reveal_in_finder(&app_info.path.to_string_lossy())
            {
                app.show_hud(
                    format!("Failed to reveal app: {error}"),
                    Some(crate::HUD_MEDIUM_MS),
                    cx,
                );
            }
            true
        }
        (RootUnifiedResultAction::AppCopyPath, RootUnifiedActionSubject::App(app_info)) => {
            copy(app_info.path.to_string_lossy().to_string(), cx)
        }
        (RootUnifiedResultAction::AppCopyName, RootUnifiedActionSubject::App(app_info)) => {
            copy(app_info.name.clone(), cx)
        }
        (RootUnifiedResultAction::AppCopyBundleId, RootUnifiedActionSubject::App(app_info)) => {
            copy(app_info.bundle_id.clone().unwrap_or_default(), cx)
        }
        (RootUnifiedResultAction::WindowSwitch, RootUnifiedActionSubject::Window(window_info)) => {
            app.execute_window_focus(window_info, cx);
            true
        }
        (
            RootUnifiedResultAction::WindowCopyTitle,
            RootUnifiedActionSubject::Window(window_info),
        ) => copy(window_info.title.clone(), cx),
        (
            RootUnifiedResultAction::WindowCopyAppName,
            RootUnifiedActionSubject::Window(window_info),
        ) => copy(window_info.app.clone(), cx),
        (
            RootUnifiedResultAction::WindowCopyDescriptor,
            RootUnifiedActionSubject::Window(window_info),
        ) => copy(format!("{} — {}", window_info.app, window_info.title), cx),
        (RootUnifiedResultAction::CommandRun, RootUnifiedActionSubject::BuiltIn(entry)) => {
            let _outcome = app.execute_builtin(entry, cx);
            true
        }
        (RootUnifiedResultAction::CommandCopyId, RootUnifiedActionSubject::BuiltIn(entry)) => {
            copy(entry.id.clone(), cx)
        }
        (RootUnifiedResultAction::SkillOpen, RootUnifiedActionSubject::Skill(skill)) => {
            app.open_agent_chat_with_selected_skill(skill, cx);
            true
        }
        (RootUnifiedResultAction::SkillCopyId, RootUnifiedActionSubject::Skill(skill)) => {
            copy(skill.skill_id.clone(), cx)
        }
        (RootUnifiedResultAction::SkillCopyPluginId, RootUnifiedActionSubject::Skill(skill)) => {
            copy(skill.plugin_id.clone(), cx)
        }
        (RootUnifiedResultAction::ScriptIssueInspect, RootUnifiedActionSubject::ScriptIssue(_)) => {
            app.open_script_issues_view(cx);
            true
        }
        (
            RootUnifiedResultAction::ScriptIssueCopySummary,
            RootUnifiedActionSubject::ScriptIssue(issue),
        ) => copy(
            issue
                .description
                .clone()
                .unwrap_or_else(|| issue.title.clone()),
            cx,
        ),
        _ => {
            tracing::warn!(
                target: "script_kit::actions",
                event = "root_unified_result_action_subject_mismatch",
                action_id,
                subject_source = subject.source_name(),
                "Root result action ignored for mismatched subject"
            );
            true
        }
    }
}

fn copy(value: String, cx: &mut Context<ScriptListApp>) -> bool {
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(value));
    true
}

impl ScriptListApp {
    pub(crate) fn execute_root_ai_vault_paste_resume_command(
        &mut self,
        hit: &crate::ai_vault::AiVaultHit,
        cx: &mut Context<Self>,
    ) {
        let command = crate::ai_vault::resume_command_for_hit(
            hit,
            crate::ai_vault::AiVaultTerminalRouting::UserPreferred,
        );
        self.hide_main_and_reset(cx);
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            let injector = crate::text_injector::TextInjector::new();
            if let Err(error) = injector.paste_text(&command) {
                crate::logging::log(
                    "ERROR",
                    &format!("Failed to paste AI Vault resume command: {error}"),
                );
            } else {
                crate::logging::log("EXEC", "Pasted AI Vault resume command");
            }
        });
    }

    pub(crate) fn execute_root_ai_vault_copy_resume_command(
        &mut self,
        hit: &crate::ai_vault::AiVaultHit,
        cx: &mut Context<Self>,
    ) {
        let command = crate::ai_vault::resume_command_for_hit(
            hit,
            crate::ai_vault::AiVaultTerminalRouting::UserPreferred,
        );
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(command));
        self.show_hud(
            "Copied AI Vault resume command".to_string(),
            Some(crate::HUD_SHORT_MS),
            cx,
        );
    }

    pub(crate) fn execute_root_ai_vault_resume_configured_terminal(
        &mut self,
        hit: &crate::ai_vault::AiVaultHit,
        cx: &mut Context<Self>,
    ) {
        match self.config.get_unified_search().ai_vault.resume_terminal {
            crate::config::AiVaultResumeTerminal::Cmux => {
                self.execute_root_ai_vault_resume_cmux(
                    hit,
                    crate::ai_vault::AiVaultTerminalRouting::UserPreferred,
                    cx,
                );
            }
            crate::config::AiVaultResumeTerminal::QuickTerminal => {
                self.execute_root_ai_vault_resume_quick_terminal(hit, cx);
            }
        }
    }

    pub(crate) fn execute_root_ai_vault_configure_terminal(&mut self, cx: &mut Context<Self>) {
        let snippet = r#"unifiedSearch: {
  aiVault: {
    resumeTerminal: "quickTerminal", // or "cmux"
  },
},"#;
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(snippet.to_string()));
        self.show_hud(
            "Copied AI Vault terminal config snippet".to_string(),
            Some(crate::HUD_SHORT_MS),
            cx,
        );
    }

    pub(crate) fn execute_root_ai_vault_resume_quick_terminal(
        &mut self,
        hit: &crate::ai_vault::AiVaultHit,
        cx: &mut Context<Self>,
    ) {
        let command = crate::ai_vault::resume_command_for_hit(
            hit,
            crate::ai_vault::AiVaultTerminalRouting::UserPreferred,
        );
        let cwd = hit
            .workspace_path
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(std::path::PathBuf::from);
        self.open_quick_terminal_with_command(cwd, command, cx);
    }

    fn execute_root_ai_vault_resume_cmux(
        &mut self,
        hit: &crate::ai_vault::AiVaultHit,
        routing: crate::ai_vault::AiVaultTerminalRouting,
        cx: &mut Context<Self>,
    ) {
        let receipt = crate::ai_vault::resume_vault_session(hit, routing);
        self.show_hud(
            vault_receipt_hud("AI Vault resume", &receipt),
            Some(crate::HUD_MEDIUM_MS),
            cx,
        );
    }
}

fn vault_receipt_hud(prefix: &str, receipt: &crate::ai_vault::AiVaultResumeReceipt) -> String {
    match receipt.error.as_ref() {
        Some(error) if !error.is_empty() => format!("{prefix} failed: {error}"),
        _ => format!("{prefix}: {}", receipt.status),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(subject: &RootUnifiedActionSubject) -> Vec<String> {
        root_unified_actions_for_subject(subject)
            .into_iter()
            .map(|action| action.id)
            .collect()
    }

    #[test]
    fn every_action_id_round_trips() {
        for action in RootUnifiedResultAction::ALL {
            assert_eq!(
                RootUnifiedResultAction::from_action_id(action.action_id()),
                Some(*action)
            );
        }
    }

    #[test]
    fn action_ids_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for action in RootUnifiedResultAction::ALL {
            assert!(
                seen.insert(action.action_id()),
                "duplicate id {}",
                action.action_id()
            );
        }
    }

    #[test]
    fn notes_actions_match_expected_matrix() {
        let subject = RootUnifiedActionSubject::Note {
            id: crate::notes::NoteId::new(),
            title: "note".to_string(),
        };
        assert_eq!(
            ids(&subject),
            vec![
                "root_note_open",
                "root_note_copy_title",
                "root_note_copy_id"
            ]
        );
    }

    #[test]
    fn dictation_actions_offer_composer_add_and_confirmed_delete_verbs() {
        let subject = RootUnifiedActionSubject::Dictation {
            id: "dictation-id".to_string(),
            preview: "Dictation preview".to_string(),
        };
        let actions = root_unified_actions_for_subject(&subject);
        let action_ids = actions
            .iter()
            .map(|action| action.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            action_ids,
            vec![
                "root_dictation_paste",
                "root_dictation_copy_transcript",
                "root_dictation_attach_to_ai",
                "root_dictation_create_note",
                "root_dictation_delete",
            ]
        );
        let add = actions
            .iter()
            .find(|action| action.id == "root_dictation_attach_to_ai")
            .expect("Dictation Agent Chat action");
        assert_eq!(add.title, "Add to Agent Chat");
        assert!(actions.iter().all(|action| !action.title.contains("Ask")));
        assert!(actions.iter().all(|action| !action.title.contains("Send")));
    }

    #[test]
    fn clipboard_actions_match_expected_matrix_pinned_and_unpinned() {
        let mut entry = crate::clipboard_history::ClipboardEntryMeta {
            id: "clip".to_string(),
            content_type: crate::clipboard_history::ContentType::Text,
            timestamp: 0,
            pinned: false,
            text_preview: "clip".to_string(),
            image_width: None,
            image_height: None,
            byte_size: 4,
            ocr_text: None,
        };
        assert!(ids(&RootUnifiedActionSubject::Clipboard(entry.clone()))
            .contains(&"root_clipboard_pin".to_string()));
        entry.pinned = true;
        assert!(ids(&RootUnifiedActionSubject::Clipboard(entry))
            .contains(&"root_clipboard_unpin".to_string()));
    }

    #[test]
    fn clipboard_attach_action_only_shows_for_text_submit_content() {
        fn entry(content_type: crate::clipboard_history::ContentType) -> RootUnifiedActionSubject {
            RootUnifiedActionSubject::Clipboard(crate::clipboard_history::ClipboardEntryMeta {
                id: "clip".to_string(),
                content_type,
                timestamp: 0,
                pinned: false,
                text_preview: "clip".to_string(),
                image_width: None,
                image_height: None,
                byte_size: 4,
                ocr_text: None,
            })
        }

        for content_type in [
            crate::clipboard_history::ContentType::Text,
            crate::clipboard_history::ContentType::Link,
            crate::clipboard_history::ContentType::Color,
        ] {
            assert!(
                ids(&entry(content_type)).contains(&"root_clipboard_attach_to_ai".to_string()),
                "root clipboard attach should be visible for {content_type:?}"
            );
        }

        for content_type in [
            crate::clipboard_history::ContentType::File,
            crate::clipboard_history::ContentType::Image,
        ] {
            assert!(
                !ids(&entry(content_type)).contains(&"root_clipboard_attach_to_ai".to_string()),
                "root clipboard attach should be hidden for {content_type:?}"
            );
        }
    }

    #[test]
    fn script_rows_delegate_to_existing_script_actions_owner() {
        let script = crate::scripts::Script {
            name: "Build".to_string(),
            path: std::path::PathBuf::from("/tmp/build.ts"),
            plugin_id: "main".to_string(),
            ..Default::default()
        };
        let result = SearchResult::Script(crate::scripts::ScriptMatch {
            script: std::sync::Arc::new(script),
            score: 1,
            filename: "build.ts".to_string(),
            match_indices: Default::default(),
            match_kind: Default::default(),
            content_match: None,
            match_evidence: None,
        });
        assert!(matches!(
            root_unified_action_owner_for_result(&result),
            RootUnifiedResultActionOwner::ExistingScriptActions
        ));
    }

    #[test]
    fn config_backed_command_rows_delegate_to_existing_script_actions_owner() {
        let builtin = SearchResult::BuiltIn(crate::scripts::BuiltInMatch {
            entry: crate::builtins::BuiltInEntry {
                id: "builtin/clipboard-history".to_string(),
                name: "Clipboard History".to_string(),
                description: "Browse clipboard history".to_string(),
                keywords: vec![],
                feature: crate::builtins::BuiltInFeature::ClipboardHistory,
                icon: None,
                group: crate::builtins::BuiltInGroup::Core,
            },
            score: 1,
            match_evidence: None,
        });
        let app = SearchResult::App(crate::scripts::AppMatch {
            app: crate::app_launcher::AppInfo {
                name: "Safari".to_string(),
                path: std::path::PathBuf::from("/Applications/Safari.app"),
                bundle_id: Some("com.apple.Safari".to_string()),
                icon: None,
            },
            score: 1,
            match_evidence: None,
        });

        for result in [builtin, app] {
            assert!(matches!(
                root_unified_action_owner_for_result(&result),
                RootUnifiedResultActionOwner::ExistingScriptActions
            ));
        }
    }
}
