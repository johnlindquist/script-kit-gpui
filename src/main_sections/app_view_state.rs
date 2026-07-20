/// Controls whether the file search view renders as a full split-view (list + preview)
/// or a compact list-only surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileSearchPresentation {
    /// Full split-view with list + preview panel (opened via builtin "Search Files")
    Full,
    /// Compact list-only surface (opened via `~` trigger from ScriptList)
    Mini,
}

/// Application state - what view are we currently showing
pub(crate) const ABOUT_SURFACE_EXEMPTION: &str =
    "about is a static content surface with no list selection owner";

/// Prompt id of the internal (sessionless) brain-memory preview DivPrompt.
/// Enter/Escape on a DivPrompt with this id return to the script list instead
/// of routing through the protocol prompt machinery — checked by the root
/// capture_key_down handler and the simulateKey dispatcher.
pub(crate) const BRAIN_MEMORY_PREVIEW_PROMPT_ID: &str = "brain-memory-preview";

#[derive(Debug, Clone, PartialEq, Eq)]
enum MigrateBoardPhase {
    Unavailable(String),
    Scanning,
    Report,
    Porting,
    Done,
}

impl Default for MigrateBoardPhase {
    fn default() -> Self {
        Self::Scanning
    }
}

#[derive(Debug, Clone, Default)]
struct MigrateBoardState {
    phase: MigrateBoardPhase,
    rows: Vec<MigrateScriptRow>,
    v1_dir: String,
    engine_path: Option<String>,
    port_started: bool,
}

#[derive(Debug, Clone, Default)]
struct MigrateScriptRow {
    file: String,
    path: String,
    bucket: String,
    incompatible_apis: Vec<String>,
    phase: String,
    status: Option<String>,
    note_summary: Option<String>,
    failure: Option<String>,
    attempts: Vec<MigrateAttemptReceipt>,
}

#[derive(Debug, Clone, Default)]
struct MigrateAttemptReceipt {
    attempt: usize,
    verdicts: Vec<MigrateVerdictReceipt>,
}

#[derive(Debug, Clone, Default)]
struct MigrateVerdictReceipt {
    id: String,
    outcome: String,
    summary: String,
    detail: String,
}

#[derive(Debug, Clone)]
enum AppView {
    /// Showing the script list
    ScriptList,
    /// Showing the launcher-native About surface opened from the tray menu.
    About {
        previous: Box<AppView>,
        state: crate::about::AboutState,
        update_state: std::sync::Arc<std::sync::RwLock<crate::updates::UpdateState>>,
    },
    /// Legacy full-view placeholder for the actions dialog. The live Cmd+K
    /// route is an attached popup and deliberately keeps its host AppView
    /// current so the host's native footer remains visible underneath.
    #[allow(dead_code)]
    ActionsDialog,
    /// Showing an arg prompt from a script
    ArgPrompt {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
        actions: Option<Vec<ProtocolAction>>,
    },
    /// Showing a div prompt from a script
    DivPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<DivPrompt>,
    },
    /// Showing a form prompt from a script (HTML form with submit button)
    FormPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<FormPromptState>,
    },
    /// Showing a terminal prompt from a script
    TermPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<term_prompt::TermPrompt>,
    },
    /// Showing an editor prompt from a script (gpui-component based with Find/Replace)
    EditorPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<EditorPrompt>,
        /// Separate focus handle for the editor (not shared with parent)
        /// Note: This is kept for API compatibility but focus is managed via entity.focus()
        #[allow(dead_code)]
        focus_handle: FocusHandle,
    },
    /// Showing a select prompt from a script (multi-select)
    SelectPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<SelectPrompt>,
    },
    /// Showing a path prompt from a script (file/folder picker)
    PathPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<PathPrompt>,
        focus_handle: FocusHandle,
    },
    /// Showing env prompt for environment variable input with keyring storage
    EnvPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<EnvPrompt>,
    },
    /// Showing drop prompt for drag and drop file handling
    DropPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<DropPrompt>,
    },
    /// Showing template prompt for string template editing
    TemplatePrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<TemplatePrompt>,
    },
    /// Showing transient SDK hotkey capture prompt
    HotkeyPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<crate::components::shortcut_recorder::ShortcutRecorder>,
    },
    /// Showing chat prompt for conversational interfaces
    ChatPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<prompts::ChatPrompt>,
    },
    /// Compact single-line arg prompt (mini variant)
    MiniPrompt {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },
    /// Ultra-compact inline arg prompt (micro variant)
    MicroPrompt {
        id: String,
        placeholder: String,
        choices: Vec<Choice>,
    },
    /// Showing clipboard history
    /// P0 FIX: View state only - data comes from clipboard_history module cache
    ClipboardHistoryView {
        filter: String,
        selected_index: usize,
    },
    /// Showing app launcher
    /// P0 FIX: View state only - data comes from ScriptListApp.apps or app_launcher module
    AppLauncherView {
        filter: String,
        selected_index: usize,
    },
    /// Showing window switcher
    /// P0 FIX: View state only - windows stored in ScriptListApp.cached_windows
    WindowSwitcherView {
        filter: String,
        selected_index: usize,
    },
    /// Showing searchable open browser tabs across supported browsers
    BrowserTabsView {
        filter: String,
        selected_index: usize,
    },
    /// Showing design gallery (separator and icon variations)
    DesignGalleryView {
        filter: String,
        selected_index: usize,
    },
    /// Showing footer gallery (shortcut glyph and font variations)
    FooterGalleryView {
        filter: String,
        selected_index: usize,
    },
    /// Showing one main-window non-list state design language example
    NonListStatesView { selected_index: usize },
    /// Showing the in-app storybook compare view for design exploration
    #[cfg(feature = "storybook")]
    DesignExplorerView {
        entity: Entity<script_kit_gpui::storybook::StoryBrowser>,
    },
    /// Showing webcam prompt
    WebcamView {
        entity: Entity<prompts::WebcamPrompt>,
    },
    /// Showing scratch pad editor (auto-saves to disk)
    ScratchPadView {
        entity: Entity<EditorPrompt>,
        #[allow(dead_code)]
        focus_handle: FocusHandle,
    },
    /// Showing quick terminal
    QuickTerminalView {
        entity: Entity<term_prompt::TermPrompt>,
    },
    /// Conversing with a flow agent: the session's PTY (running the real
    /// `flow-*` wrapper interactively) fills the main window. The entity
    /// lives in `flow_sessions`; leaving this view NEVER kills the process.
    FlowSessionView { session_id: u64 },
    /// Showing file search results
    FileSearchView {
        query: String,
        selected_index: usize,
        presentation: FileSearchPresentation,
    },
    /// Showing Agent Chat profiles with split-pane preview
    ProfileSearchView {
        filter: String,
        selected_index: usize,
    },
    /// Showing theme chooser with live preview and search
    ThemeChooserView {
        filter: String,
        selected_index: usize,
    },
    /// Showing emoji picker grid with category sections
    EmojiPickerView {
        filter: String,
        selected_index: usize,
        selected_category: Option<crate::emoji::EmojiCategory>,
    },
    /// Showing naming dialog for script/extension creation.
    /// Non-dismissable — requires explicit submit or cancel.
    NamingPrompt {
        #[allow(dead_code)]
        id: String,
        entity: Entity<prompts::NamingPrompt>,
    },
    /// Showing creation feedback with artifact path, receipt status, and quick actions after creation.
    /// Requires explicit dismiss (Enter/Escape/button) — non-dismissable by click-outside.
    CreationFeedback {
        payload: prompts::CreationFeedbackPayload,
    },
    /// Read-only diagnostic view listing scripts excluded by validation.
    /// Populated from the Arc<ValidationReport> already held by ScriptListApp;
    /// Escape returns to ScriptList, Cmd+C copies diagnostics to clipboard.
    ScriptIssuesView {
        report: std::sync::Arc<crate::scripts::ValidationReport>,
    },
    /// Browseable SDK reference sourced from the same data that powers
    /// `kit://sdk-reference`. Filterable by name/signature/description;
    /// Enter / Cmd+C copy the selected entry as markdown. Escape returns
    /// to the script list. Entries live behind an Arc to keep view clones
    /// cheap — the UI never re-parses the MCP JSON payload.
    SdkReferenceView {
        filter: String,
        selected_index: usize,
        entries: std::sync::Arc<[crate::mcp_resources::SdkFunctionRef]>,
    },
    TipsView {
        filter: String,
        selected_index: usize,
        entries: Vec<script_kit_gpui::tips::Tip>,
    },
    /// Browseable starter-template catalog sourced from the same data that
    /// powers `kit://script-templates`. Enter transitions into the naming
    /// prompt with the selected template threaded through so
    /// [`crate::mcp_resources::render_script_template_file`] can overwrite
    /// the newly-created script body before the editor opens. Cmd+C copies
    /// the template's markdown card. Escape returns to the script list.
    /// Templates live behind an Arc to keep view clones cheap — the UI
    /// never re-builds the catalog from scratch on each render.
    ScriptTemplateCatalogView {
        filter: String,
        selected_index: usize,
        templates: std::sync::Arc<[crate::mcp_resources::ScriptTemplateRef]>,
    },
    /// Browsing the Kit Store (GitHub search for installable kits)
    BrowseKitsView {
        query: String,
        selected_index: usize,
        results: Vec<KitStoreSearchResult>,
    },
    /// Live board for scanning and porting Script Kit v1 scripts.
    MigrateV1View {
        filter: String,
        selected_index: usize,
        board: MigrateBoardState,
    },
    /// Managing locally installed kits (update/remove)
    InstalledKitsView {
        filter: String,
        selected_index: usize,
        kits: Vec<script_kit_gpui::kit_store::InstalledKit>,
    },
    /// Showing process manager (running background scripts)
    /// Data comes from cached_processes field populated on open
    ProcessManagerView {
        filter: String,
        selected_index: usize,
    },
    /// Flow UX exploration surfaces (Flash / Dispatch / Lens) over the
    /// mdflow roster. One view, parameterized by variant, so run lifecycle
    /// stays in crate::flows and variants remain thin renderers.
    /// `inline_run` is Flash's engaged run (Esc backgrounds it).
    FlowUxView {
        variant: crate::flows::model::FlowUxVariant,
        filter: String,
        selected_index: usize,
        inline_run: Option<u64>,
    },
    /// Showing searchable list of saved AI presets
    /// Selecting a preset opens AI chat with its system prompt and model
    SearchAiPresetsView {
        filter: String,
        selected_index: usize,
    },
    /// Showing create AI preset form
    /// Name + system prompt + model fields
    CreateAiPresetView {
        name: String,
        system_prompt: String,
        model: String,
        active_field: usize,
    },
    /// Showing settings hub with configuration panels
    /// Lists categories: Theme, Window Positions, Feature Toggles, and Hotkeys
    SettingsView {
        filter: String,
        selected_index: usize,
    },
    /// Guided macOS permission grant flow (onboarding wizard).
    /// Cards for each `crate::permissions_wizard::PermissionKind` with live
    /// TCC status; Enter fires the native prompt or opens System Settings.
    PermissionsWizardView { selected_index: usize },
    /// Browsing favorites with search/filter
    /// Supports Enter to run, D to remove, U/J to reorder, Esc to go back
    FavoritesBrowseView {
        filter: String,
        selected_index: usize,
    },
    /// Showing menu bar commands from the frontmost application
    /// Data comes from a session-backed capture and cached_current_app_entries
    CurrentAppCommandsView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing Agent Chat conversation history with search and preview
    AgentChatHistoryView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing recent browser history as an Agent Chat attachment portal
    BrowserHistoryView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing saved dictation transcripts with search and preview
    DictationHistoryView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing notes from Agent Chat as an attachment portal
    NotesBrowseView {
        filter: String,
        selected_index: usize,
    },
    /// Showing the Agent Chat surface for the default Tab path.
    ///
    /// The `AgentChatView` variant name and `"agent_chat"` serialized id are kept
    /// for launcher surface-contract compatibility; the entity now flows
    /// through the canonical `agent_chat::ui` boundary.
    ///
    /// Verification-bearing new-script requests deliberately route to
    /// `QuickTerminalView` so the agent can run Bun verification inside the
    /// live harness terminal session before reporting success.
    AgentChatView {
        entity: Entity<crate::ai::agent_chat::ui::AgentChatView>,
    },
    /// Today's brain day page in the main launcher window (same frame as ScriptList).
    DayPage { entity: Entity<DayPageView> },
    /// In-window confirm state — replaces the popup dialog when the main window
    /// is the active context. Restored to `previous` when the user confirms or
    /// cancels via Esc / Enter / Tab + ↵ / footer Apply / Close.
    ConfirmPrompt {
        options: crate::confirm::ParentConfirmOptions,
        sender: async_channel::Sender<bool>,
        focused_button: ConfirmFocusedButton,
        previous: Box<AppView>,
    },
}

/// Exhaustive host-level header/input ownership for every main-window view.
///
/// This is deliberately an enum instead of independent booleans: a new
/// `AppView` must choose one valid chrome anatomy before it can compile.
/// Surface renderers may own their body input, but only the canonical input
/// variants may reserve the shared `MainViewInput` lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MainViewHeaderInputPolicy {
    /// The view renders the shared context zone and one-line main input.
    ViewOwnedCanonicalInput,
    /// The view renders the shared context zone and a multiline input whose
    /// first line exactly matches the canonical one-line input geometry.
    /// Agent Chat may add its trailing Send action inside that shared shell;
    /// the action never changes the shell, leading inset, or text baseline.
    ViewOwnedCanonicalMultilineInput,
    /// The view renders only the shared context zone. Day Page intentionally
    /// keeps its editor in the main slot instead of pretending it is a search
    /// input.
    ViewOwnedContextOnly,
    /// The view deliberately owns a compact, non-canonical full-window shell.
    /// Focused Text Mini's input row is only 44px tall, so it cannot
    /// contain the canonical 58px header anatomy without defeating its purpose.
    /// It must expose its own honest layout receipt and reserve any native
    /// footer safe area instead of emitting fake MainViewHeader/Input nodes.
    ViewOwnedIntentionalCompact,
    /// The root renders only the shared context zone above a legacy, prompt,
    /// or child surface whose editable controls belong to its body.
    RootContextOnly,
}

impl MainViewHeaderInputPolicy {
    pub(crate) const fn view_owns_main_window_shell(self) -> bool {
        matches!(
            self,
            Self::ViewOwnedCanonicalInput
                | Self::ViewOwnedCanonicalMultilineInput
                | Self::ViewOwnedContextOnly
                | Self::ViewOwnedIntentionalCompact
        )
    }

    pub(crate) const fn has_main_input(self) -> bool {
        matches!(
            self,
            Self::ViewOwnedCanonicalInput | Self::ViewOwnedCanonicalMultilineInput
        )
    }
}

/// Which button has Tab focus inside an in-window [`AppView::ConfirmPrompt`].
///
/// Default is `Confirm` so a bare ↵ activates the primary action, mirroring
/// the popup confirm window's initial focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ConfirmFocusedButton {
    #[default]
    Confirm,
    Cancel,
}

impl ConfirmFocusedButton {
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Confirm => Self::Cancel,
            Self::Cancel => Self::Confirm,
        }
    }
}

/// Stable behavior identity for each top-level launcher surface contract.
///
/// `AppView` carries runtime payloads and child entities. `SurfaceKind` is the
/// payload-free identity that agents, generated docs, and proof matrices can
/// use without reverse-engineering enum fields or render files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SurfaceKind {
    ScriptList,
    About,
    ActionsDialog,
    PromptEntity,
    /// Prompt entities whose Escape is owned by the entity itself: the entity
    /// calls `submit_cancel()` so the script receives `None` and can run its
    /// cancel path. The launcher shell must NOT also close on Escape — two
    /// owners made Escape a race between "window gone" and "script got None".
    PromptEntityCancelsToScript,
    PromptChildContent,
    ExplicitPromptEntity,
    Webcam,
    ClipboardHistory,
    AppLauncher,
    WindowSwitcher,
    BrowserTabs,
    GenericFilterableList,
    Settings,
    KitStoreBrowse,
    MigrateV1,
    KitStoreInstalled,
    ProcessManager,
    FlowUx,
    CurrentAppCommands,
    PermissionsWizard,
    DesignGallery,
    FooterGallery,
    NonListStates,
    #[cfg(feature = "storybook")]
    DesignExplorer,
    UtilityChildContent,
    /// Main-window Threadline flow conversation. Split from
    /// `UtilityChildContent` because its blur semantics differ: clicking
    /// away hides the launcher and resets to the main menu while the flow
    /// session keeps running in the background (Escape stays view-owned —
    /// it backgrounds to the flow desk, never closes the window).
    FlowSession,
    FileSearchMini,
    FileSearchFull,
    ProfileSearch,
    ThemeChooser,
    EmojiPicker,
    Feedback,
    SdkReference,
    Tips,
    ScriptTemplateCatalog,
    AgentChatHistory,
    AttachmentPortalBrowser,
    AgentChat,
    DayPage,
    ConfirmPrompt,
}

/// First-pass vocabulary for describing what kind of launcher surface an
/// [`AppView`] represents.
///
/// These names are intentionally behavior-oriented instead of renderer-oriented:
/// future contract registries should say "this is a filterable launcher list",
/// not "this happens to render in file X".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceFamily {
    /// The normal Script Kit home list and its main-menu command shell.
    MainMenu,
    /// Script-owned prompt surfaces created from SDK prompt messages.
    ScriptPrompt,
    /// Searchable launcher-owned rows such as clipboard history and settings.
    FilterableLauncherList,
    /// Tool-like workspaces that own richer interaction than a plain row list.
    UtilityWorkspace,
    /// Temporary picker surfaces opened to attach context back to Agent Chat.
    AttachmentPortal,
    /// Embedded assistant/chat surfaces and their transcript/history variants.
    AssistantWorkspace,
    /// Completion or diagnostic surfaces that explain a one-shot result.
    FeedbackSurface,
}

/// Names which component owns keyboard text input for a launcher surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceInputOwnership {
    /// The launcher shell owns the shared filter input and row navigation.
    LauncherFilter,
    /// A prompt entity owns text entry, validation, and focus behavior.
    PromptEntity,
    /// A child view/entity owns focus and interprets keys locally.
    ChildView,
    /// The surface has no normal editable input.
    NoEditableInput,
}

/// Names how much preview/detail UI is part of a surface's expected shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfacePreviewRole {
    /// A compact list or prompt with no persistent preview region.
    NoPersistentPreview,
    /// A preview/info region exists but is revealed by explicit user intent.
    OptionalInfoPanel,
    /// A split preview is central to selecting the right item.
    RequiredSplitPreview,
    /// The primary content is a transcript, editor, or terminal pane.
    ContentPane,
    /// The surface explains a completed action or diagnostic result.
    FeedbackPanel,
}

/// Names the focus owner a surface expects after entry or restoration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceFocusPolicy {
    /// Focus should land on the shared launcher filter/input chrome.
    LauncherFilterFocus,
    /// Focus should land on a script prompt entity.
    PromptEntityFocus,
    /// Focus should land inside a child content view such as editor, terminal, or Agent Chat.
    ChildViewFocus,
    /// The surface has no normal editable focus target.
    NoEditableFocus,
}

/// Names the keyboard dispatcher family a surface expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceKeyboardPolicy {
    /// Shared launcher list/filter keys own navigation and activation.
    LauncherListKeyboard,
    /// Prompt entities own text entry, validation, and prompt-local keys.
    PromptEntityKeyboard,
    /// Child content views own local keyboard interpretation.
    ChildViewKeyboard,
    /// Actions-dialog routing owns popup list keys before host shortcuts.
    ActionsDialogKeyboard,
    /// The surface only expects explicit shell commands such as dismiss/close.
    NoEditableKeyboard,
}

/// Names which layer owns visible/shared actions for a surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceActionsPolicy {
    /// The main menu owns global launcher actions.
    MainMenuActions,
    /// Row-aware host actions can be opened from the surface.
    HostRowActions,
    /// A prompt entity owns any local actions.
    PromptEntityActions,
    /// A child content view owns any local actions.
    ChildViewActions,
    /// The actions dialog owns its own filtered action rows.
    ActionsDialogActions,
    /// The surface has no normal shared action surface.
    NoSurfaceActions,
}

/// Names the preferred first proof for a surface contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceProofPolicy {
    /// Use a state receipt before any screenshot.
    StateReceiptProof,
    /// Use state plus element list/count receipts.
    StateAndElementsProof,
    /// Use child-view state receipts before visual proof.
    ChildViewStateProof,
    /// Use popup-scoped state/visibility receipts.
    PopupStateProof,
}

/// Names the expected visual shape of a launcher surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) enum LauncherSurfaceVisualPolicy {
    /// Compact launcher shell with no always-visible split preview.
    CompactLauncherVisual,
    /// Split list/detail surface where preview is part of the contract.
    SplitPreviewVisual,
    /// Child/content pane such as terminal, editor, Agent Chat, or About.
    ContentPaneVisual,
    /// Attached/modal popup visual contract.
    PopupVisual,
    /// Completion or diagnostic feedback surface.
    FeedbackVisual,
}

/// Shared vocabulary tuple for the future exhaustive AppView behavior registry.
///
/// AURP-03 adds the names only. AURP-04 should wire every [`AppView`] variant
/// through an exhaustive registry that returns this vocabulary beside concrete
/// behavior such as dismiss policy, focus restoration, and automation tags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct LauncherSurfaceContractVocabulary {
    pub(crate) family: LauncherSurfaceFamily,
    pub(crate) input_ownership: LauncherSurfaceInputOwnership,
    pub(crate) preview_role: LauncherSurfacePreviewRole,
}

#[allow(dead_code)]
impl LauncherSurfaceContractVocabulary {
    pub(crate) const fn new(
        family: LauncherSurfaceFamily,
        input_ownership: LauncherSurfaceInputOwnership,
        preview_role: LauncherSurfacePreviewRole,
    ) -> Self {
        Self {
            family,
            input_ownership,
            preview_role,
        }
    }
}

/// The user/system action that may dismiss the current main-window view.
///
/// Oracle-Session `shortcuts-hud-grid-dismiss-logic` — kept separate from
/// popup-specific dismissal (confirm/actions overlays have their own close
/// paths through `close_actions_popup_for_current_view` / `confirm::*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DismissTrigger {
    /// Main launcher window lost focus — the `focus_lost` click-outside path.
    WindowBlur,
    /// Explicit backdrop / shield click for a main-surface view.
    #[allow(dead_code)]
    BackdropClick,
    /// Global Escape handled by the launcher shell.
    Escape,
    /// Platform close chord — currently Cmd+W on macOS.
    CmdW,
}

/// What the launcher shell should do for a given dismiss trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DismissEffect {
    /// Shell does not dismiss; trigger is ignored at this layer.
    Ignore,
    /// Shell closes / resets the main launcher window.
    CloseMainWindow,
    /// The focused view/entity consumes the trigger.
    ///
    /// Used for terminal / editor / chat surfaces where Escape is meaningful
    /// inside the child view but should not close the launcher shell.
    #[allow(dead_code)]
    LetViewHandle,
}

/// Per-[`AppView`] dismiss contract.
///
/// Intentionally does **not** implement `Default`. A default would recreate
/// the original bug: new variants would silently inherit dismissal behavior.
/// New `AppView` variants must declare a policy explicitly — rustc
/// exhaustiveness in [`AppView::dismiss_policy`] is the compile-time guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DismissPolicy {
    window_blur: DismissEffect,
    backdrop_click: DismissEffect,
    escape: DismissEffect,
    cmd_w: DismissEffect,
}

impl DismissPolicy {
    pub(crate) const fn new(
        window_blur: DismissEffect,
        backdrop_click: DismissEffect,
        escape: DismissEffect,
        cmd_w: DismissEffect,
    ) -> Self {
        Self {
            window_blur,
            backdrop_click,
            escape,
            cmd_w,
        }
    }

    /// Raycast-style surfaces: blur, backdrop, Escape and Cmd+W all close.
    pub(crate) const fn standard_launcher_surface() -> Self {
        Self::new(
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
        )
    }

    /// Sticky surfaces: shell ignores blur/backdrop, Escape is consumed by
    /// the view itself, but Cmd+W still closes the main window.
    pub(crate) const fn explicit_cmd_w_only() -> Self {
        Self::new(
            DismissEffect::Ignore,
            DismissEffect::Ignore,
            DismissEffect::LetViewHandle,
            DismissEffect::CloseMainWindow,
        )
    }

    /// Cancel-to-script prompts: blur/backdrop/Cmd+W close like a standard
    /// launcher surface, but Escape belongs to the prompt entity, which calls
    /// `submit_cancel()` so the running script receives `None`. The shell
    /// must never also close on Escape — one key, one owner.
    pub(crate) const fn cancel_to_script_prompt() -> Self {
        Self::new(
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
            DismissEffect::LetViewHandle,
            DismissEffect::CloseMainWindow,
        )
    }

    /// Sticky capture surfaces (webcam): blur/backdrop are ignored so an
    /// accidental click-away cannot kill a live capture session, but Escape
    /// and Cmd+W close. This is the behavior the webcam key handler always
    /// implemented; the contract previously claimed LetViewHandle for Escape.
    pub(crate) const fn sticky_escape_closes() -> Self {
        Self::new(
            DismissEffect::Ignore,
            DismissEffect::Ignore,
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
        )
    }

    /// Background-on-blur surfaces (flow sessions): clicking away hides the
    /// launcher (work continues in the background), Escape stays view-owned
    /// (it backgrounds to the flow desk), Cmd+W closes.
    pub(crate) const fn blur_closes_escape_view_owned() -> Self {
        Self::new(
            DismissEffect::CloseMainWindow,
            DismissEffect::CloseMainWindow,
            DismissEffect::LetViewHandle,
            DismissEffect::CloseMainWindow,
        )
    }

    pub(crate) const fn effect_for(self, trigger: DismissTrigger) -> DismissEffect {
        match trigger {
            DismissTrigger::WindowBlur => self.window_blur,
            DismissTrigger::BackdropClick => self.backdrop_click,
            DismissTrigger::Escape => self.escape,
            DismissTrigger::CmdW => self.cmd_w,
        }
    }

    pub(crate) const fn closes_main_window_on(self, trigger: DismissTrigger) -> bool {
        matches!(self.effect_for(trigger), DismissEffect::CloseMainWindow)
    }
}

/// Behavior declaration for an [`AppView`] variant.
///
/// This is the first registry layer: every top-level surface declares its
/// vocabulary, dismiss policy, and automation semantic surface tag in one
/// exhaustive match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub(crate) struct LauncherSurfaceContract {
    pub(crate) vocabulary: LauncherSurfaceContractVocabulary,
    pub(crate) focus_policy: LauncherSurfaceFocusPolicy,
    pub(crate) keyboard_policy: LauncherSurfaceKeyboardPolicy,
    pub(crate) actions_policy: LauncherSurfaceActionsPolicy,
    pub(crate) proof_policy: LauncherSurfaceProofPolicy,
    pub(crate) visual_policy: LauncherSurfaceVisualPolicy,
    pub(crate) dismiss_policy: DismissPolicy,
    pub(crate) automation_semantic_surface: &'static str,
}

impl LauncherSurfaceContract {
    pub(crate) const fn new(
        vocabulary: LauncherSurfaceContractVocabulary,
        focus_policy: LauncherSurfaceFocusPolicy,
        keyboard_policy: LauncherSurfaceKeyboardPolicy,
        actions_policy: LauncherSurfaceActionsPolicy,
        proof_policy: LauncherSurfaceProofPolicy,
        visual_policy: LauncherSurfaceVisualPolicy,
        dismiss_policy: DismissPolicy,
        automation_semantic_surface: &'static str,
    ) -> Self {
        Self {
            vocabulary,
            focus_policy,
            keyboard_policy,
            actions_policy,
            proof_policy,
            visual_policy,
            dismiss_policy,
            automation_semantic_surface,
        }
    }
}

impl AppView {
    /// Stable variant name for DevTools target identity receipts.
    pub(crate) fn app_view_variant(&self) -> &'static str {
        match self {
            AppView::ScriptList => "ScriptList",
            AppView::About { .. } => "About",
            AppView::ActionsDialog => "ActionsDialog",
            AppView::ArgPrompt { .. } => "ArgPrompt",
            AppView::DivPrompt { .. } => "DivPrompt",
            AppView::FormPrompt { .. } => "FormPrompt",
            AppView::TermPrompt { .. } => "TermPrompt",
            AppView::EditorPrompt { .. } => "EditorPrompt",
            AppView::SelectPrompt { .. } => "SelectPrompt",
            AppView::PathPrompt { .. } => "PathPrompt",
            AppView::EnvPrompt { .. } => "EnvPrompt",
            AppView::DropPrompt { .. } => "DropPrompt",
            AppView::TemplatePrompt { .. } => "TemplatePrompt",
            AppView::HotkeyPrompt { .. } => "HotkeyPrompt",
            AppView::ChatPrompt { .. } => "ChatPrompt",
            AppView::MiniPrompt { .. } => "MiniPrompt",
            AppView::MicroPrompt { .. } => "MicroPrompt",
            AppView::NamingPrompt { .. } => "NamingPrompt",
            AppView::CreateAiPresetView { .. } => "CreateAiPresetView",
            AppView::WebcamView { .. } => "WebcamView",
            AppView::ClipboardHistoryView { .. } => "ClipboardHistoryView",
            AppView::AppLauncherView { .. } => "AppLauncherView",
            AppView::WindowSwitcherView { .. } => "WindowSwitcherView",
            AppView::BrowserTabsView { .. } => "BrowserTabsView",
            AppView::BrowseKitsView { .. } => "BrowseKitsView",
            AppView::MigrateV1View { .. } => "MigrateV1View",
            AppView::InstalledKitsView { .. } => "InstalledKitsView",
            AppView::SettingsView { .. } => "SettingsView",
            AppView::PermissionsWizardView { .. } => "PermissionsWizardView",
            AppView::SearchAiPresetsView { .. } => "SearchAiPresetsView",
            AppView::FavoritesBrowseView { .. } => "FavoritesBrowseView",
            AppView::ProcessManagerView { .. } => "ProcessManagerView",
            AppView::FlowUxView { .. } => "FlowUxView",
            AppView::CurrentAppCommandsView { .. } => "CurrentAppCommandsView",
            AppView::DesignGalleryView { .. } => "DesignGalleryView",
            AppView::FooterGalleryView { .. } => "FooterGalleryView",
            AppView::NonListStatesView { .. } => "NonListStatesView",
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => "DesignExplorerView",
            AppView::ScratchPadView { .. } => "ScratchPadView",
            AppView::QuickTerminalView { .. } => "QuickTerminalView",
            AppView::FlowSessionView { .. } => "FlowSessionView",
            AppView::FileSearchView { .. } => "FileSearchView",
            AppView::ProfileSearchView { .. } => "ProfileSearchView",
            AppView::ThemeChooserView { .. } => "ThemeChooserView",
            AppView::EmojiPickerView { .. } => "EmojiPickerView",
            AppView::CreationFeedback { .. } => "CreationFeedback",
            AppView::ScriptIssuesView { .. } => "ScriptIssuesView",
            AppView::SdkReferenceView { .. } => "SdkReferenceView",
            AppView::TipsView { .. } => "TipsView",
            AppView::ScriptTemplateCatalogView { .. } => "ScriptTemplateCatalogView",
            AppView::AgentChatHistoryView { .. } => "AgentChatHistoryView",
            AppView::BrowserHistoryView { .. } => "BrowserHistoryView",
            AppView::DictationHistoryView { .. } => "DictationHistoryView",
            AppView::NotesBrowseView { .. } => "NotesBrowseView",
            AppView::AgentChatView { .. } => "AgentChatView",
            AppView::DayPage { .. } => "DayPage",
            AppView::ConfirmPrompt { .. } => "ConfirmPrompt",
        }
    }

    /// Payload-free surface identity for the active top-level launcher view.
    ///
    /// Do **not** add `_ => ...` here. The point is to make rustc fail when a
    /// new [`AppView`] variant is added without an explicit surface identity.
    pub(crate) fn surface_kind(&self) -> SurfaceKind {
        match self {
            AppView::ScriptList => SurfaceKind::ScriptList,
            AppView::About { .. } => SurfaceKind::About,
            AppView::ActionsDialog => SurfaceKind::ActionsDialog,
            AppView::ArgPrompt { .. }
            | AppView::DivPrompt { .. }
            | AppView::FormPrompt { .. }
            | AppView::HotkeyPrompt { .. }
            | AppView::ChatPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. } => SurfaceKind::PromptEntity,
            // These prompt entities call submit_cancel() on Escape themselves;
            // classifying them as PromptEntity (escape = CloseMainWindow) made
            // the shell race the entity for the same key.
            AppView::SelectPrompt { .. }
            | AppView::PathPrompt { .. }
            | AppView::DropPrompt { .. }
            | AppView::TemplatePrompt { .. } => SurfaceKind::PromptEntityCancelsToScript,
            AppView::TermPrompt { .. } | AppView::EditorPrompt { .. } => {
                SurfaceKind::PromptChildContent
            }
            AppView::EnvPrompt { .. }
            | AppView::NamingPrompt { .. }
            | AppView::CreateAiPresetView { .. } => SurfaceKind::ExplicitPromptEntity,
            AppView::WebcamView { .. } => SurfaceKind::Webcam,
            AppView::ClipboardHistoryView { .. } => SurfaceKind::ClipboardHistory,
            AppView::AppLauncherView { .. } => SurfaceKind::AppLauncher,
            AppView::WindowSwitcherView { .. } => SurfaceKind::WindowSwitcher,
            AppView::BrowserTabsView { .. } => SurfaceKind::BrowserTabs,
            AppView::BrowseKitsView { .. } => SurfaceKind::KitStoreBrowse,
            AppView::MigrateV1View { .. } => SurfaceKind::MigrateV1,
            AppView::InstalledKitsView { .. } => SurfaceKind::KitStoreInstalled,
            AppView::SettingsView { .. } => SurfaceKind::Settings,
            AppView::PermissionsWizardView { .. } => SurfaceKind::PermissionsWizard,
            AppView::SearchAiPresetsView { .. } | AppView::FavoritesBrowseView { .. } => {
                SurfaceKind::GenericFilterableList
            }
            AppView::ProcessManagerView { .. } => SurfaceKind::ProcessManager,
            AppView::FlowUxView { .. } => SurfaceKind::FlowUx,
            AppView::CurrentAppCommandsView { .. } => SurfaceKind::CurrentAppCommands,
            AppView::DesignGalleryView { .. } => SurfaceKind::DesignGallery,
            AppView::FooterGalleryView { .. } => SurfaceKind::FooterGallery,
            AppView::NonListStatesView { .. } => SurfaceKind::NonListStates,
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => SurfaceKind::DesignExplorer,
            AppView::ScratchPadView { .. } | AppView::QuickTerminalView { .. } => {
                SurfaceKind::UtilityChildContent
            }
            AppView::FlowSessionView { .. } => SurfaceKind::FlowSession,
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Mini,
                ..
            } => SurfaceKind::FileSearchMini,
            AppView::FileSearchView {
                presentation: FileSearchPresentation::Full,
                ..
            } => SurfaceKind::FileSearchFull,
            AppView::ProfileSearchView { .. } => SurfaceKind::ProfileSearch,
            AppView::ThemeChooserView { .. } => SurfaceKind::ThemeChooser,
            AppView::EmojiPickerView { .. } => SurfaceKind::EmojiPicker,
            AppView::CreationFeedback { .. } | AppView::ScriptIssuesView { .. } => {
                SurfaceKind::Feedback
            }
            AppView::SdkReferenceView { .. } => SurfaceKind::SdkReference,
            AppView::TipsView { .. } => SurfaceKind::Tips,
            AppView::ScriptTemplateCatalogView { .. } => SurfaceKind::ScriptTemplateCatalog,
            AppView::AgentChatHistoryView { .. } => SurfaceKind::AgentChatHistory,
            AppView::BrowserHistoryView { .. }
            | AppView::DictationHistoryView { .. }
            | AppView::NotesBrowseView { .. } => SurfaceKind::AttachmentPortalBrowser,
            AppView::AgentChatView { .. } => SurfaceKind::AgentChat,
            AppView::DayPage { .. } => SurfaceKind::DayPage,
            AppView::ConfirmPrompt { .. } => SurfaceKind::ConfirmPrompt,
        }
    }

    /// Exhaustive behavior contract for every top-level launcher view.
    pub(crate) fn surface_contract(&self) -> LauncherSurfaceContract {
        self.surface_kind().surface_contract()
    }

    /// Exhaustive main-window header/input contract for this view.
    ///
    /// Do not add a wildcard arm: new views must declare whether they own a
    /// canonical input, the canonical multiline composer, or a context-only
    /// exception. This compiler-enforced inventory replaces drift-prone lists
    /// in render and automation layout code.
    pub(crate) fn main_view_header_input_policy(&self) -> MainViewHeaderInputPolicy {
        match self {
            AppView::ScriptList
            | AppView::ClipboardHistoryView { .. }
            | AppView::AppLauncherView { .. }
            | AppView::WindowSwitcherView { .. }
            | AppView::BrowserTabsView { .. }
            | AppView::DesignGalleryView { .. }
            | AppView::FooterGalleryView { .. }
            | AppView::FileSearchView { .. }
            | AppView::ProfileSearchView { .. }
            | AppView::ThemeChooserView { .. }
            | AppView::EmojiPickerView { .. }
            | AppView::SdkReferenceView { .. }
            | AppView::TipsView { .. }
            | AppView::ScriptTemplateCatalogView { .. }
            | AppView::BrowseKitsView { .. }
            | AppView::MigrateV1View { .. }
            | AppView::InstalledKitsView { .. }
            | AppView::ProcessManagerView { .. }
            | AppView::FlowUxView { .. }
            | AppView::FlowSessionView { .. }
            | AppView::SearchAiPresetsView { .. }
            | AppView::SettingsView { .. }
            | AppView::FavoritesBrowseView { .. }
            | AppView::CurrentAppCommandsView { .. }
            | AppView::AgentChatHistoryView { .. }
            | AppView::BrowserHistoryView { .. }
            | AppView::DictationHistoryView { .. }
            | AppView::NotesBrowseView { .. } => MainViewHeaderInputPolicy::ViewOwnedCanonicalInput,
            AppView::AgentChatView { .. } => {
                MainViewHeaderInputPolicy::ViewOwnedCanonicalMultilineInput
            }
            AppView::DayPage { .. } => MainViewHeaderInputPolicy::ViewOwnedContextOnly,
            AppView::About { .. }
            | AppView::ActionsDialog
            | AppView::ArgPrompt { .. }
            | AppView::DivPrompt { .. }
            | AppView::FormPrompt { .. }
            | AppView::TermPrompt { .. }
            | AppView::EditorPrompt { .. }
            | AppView::SelectPrompt { .. }
            | AppView::PathPrompt { .. }
            | AppView::EnvPrompt { .. }
            | AppView::DropPrompt { .. }
            | AppView::TemplatePrompt { .. }
            | AppView::HotkeyPrompt { .. }
            | AppView::ChatPrompt { .. }
            | AppView::MiniPrompt { .. }
            | AppView::MicroPrompt { .. }
            | AppView::NamingPrompt { .. }
            | AppView::CreateAiPresetView { .. }
            | AppView::WebcamView { .. }
            | AppView::NonListStatesView { .. }
            | AppView::ScratchPadView { .. }
            | AppView::QuickTerminalView { .. }
            | AppView::CreationFeedback { .. }
            | AppView::ScriptIssuesView { .. }
            | AppView::PermissionsWizardView { .. }
            | AppView::ConfirmPrompt { .. } => MainViewHeaderInputPolicy::RootContextOnly,
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => MainViewHeaderInputPolicy::RootContextOnly,
        }
    }

    /// Resolve stateful sub-variants that cannot be classified from the
    /// `AppView` discriminant alone. Standard Agent Chat owns the canonical
    /// multiline header; Focused Text Mini intentionally owns compact chrome.
    pub(crate) fn resolved_main_view_header_input_policy(
        &self,
        cx: &gpui::App,
    ) -> MainViewHeaderInputPolicy {
        match self {
            AppView::AgentChatView { entity } if entity.read(cx).is_focused_text_mini() => {
                MainViewHeaderInputPolicy::ViewOwnedIntentionalCompact
            }
            _ => self.main_view_header_input_policy(),
        }
    }

    /// True when the active view renderer owns the entire main-window shell.
    pub(crate) fn uses_view_owned_main_window_shell(&self, cx: &gpui::App) -> bool {
        self.resolved_main_view_header_input_policy(cx)
            .view_owns_main_window_shell()
    }

    /// Dismiss policy for the active top-level launcher view.
    ///
    /// The policy is stored in [`SurfaceKind::surface_contract`] so behavior
    /// names, dismissal, and automation tags stay declared together.
    pub(crate) fn dismiss_policy(&self) -> DismissPolicy {
        self.surface_contract().dismiss_policy
    }

    /// Native main-window footer surface id for the active view.
    ///
    /// This remains AppView-specific rather than SurfaceKind-specific because
    /// several stable `SurfaceKind`s intentionally group payload-bearing
    /// routes that still need distinct footer ownership strings.
    pub(crate) fn native_footer_surface(&self) -> Option<&'static str> {
        match self {
            AppView::ScriptList => Some("script_list"),
            AppView::SelectPrompt { .. } => Some("select_prompt"),
            AppView::DivPrompt { .. } => Some("div_prompt"),
            AppView::FormPrompt { .. } => Some("form_prompt"),
            AppView::EditorPrompt { .. } => Some("editor_prompt"),
            AppView::EnvPrompt { .. } => Some("env_prompt"),
            AppView::DropPrompt { .. } => Some("drop_prompt"),
            AppView::TemplatePrompt { .. } => Some("template_prompt"),
            AppView::HotkeyPrompt { .. } => Some("hotkey_prompt"),
            AppView::MiniPrompt { .. } => Some("mini_prompt"),
            AppView::ClipboardHistoryView { .. } => Some("clipboard_history"),
            AppView::FileSearchView { .. } => Some("file_search"),
            AppView::ProfileSearchView { .. } => Some("profile_search"),
            AppView::WebcamView { .. } => Some("webcam_prompt"),
            AppView::NamingPrompt { .. } => Some("naming_prompt"),
            AppView::CreationFeedback { .. } => Some("creation_feedback"),
            AppView::ScriptIssuesView { .. } => Some("script_issues"),
            AppView::ArgPrompt { .. } => Some("arg_prompt"),
            AppView::EmojiPickerView { .. } => Some("emoji_picker"),
            AppView::AgentChatHistoryView { .. } => Some("agent_chat_history"),
            AppView::BrowserHistoryView { .. } => Some("browser_history"),
            AppView::DictationHistoryView { .. } => Some("dictation_history"),
            AppView::AgentChatView { .. } => Some("agent_chat"),
            AppView::DayPage { .. } => Some("day_page"),
            AppView::ChatPrompt { .. } => Some("chat_prompt"),
            AppView::QuickTerminalView { .. } => Some("quick_terminal"),
            AppView::FlowSessionView { .. } => Some("flow_session"),
            AppView::PathPrompt { .. } => Some("path_prompt"),
            AppView::AppLauncherView { .. } => Some("app_launcher"),
            AppView::WindowSwitcherView { .. } => Some("window_switcher"),
            AppView::BrowserTabsView { .. } => Some("browser_tabs"),
            AppView::DesignGalleryView { .. } => Some("design_gallery"),
            AppView::FooterGalleryView { .. } => Some("footer_gallery"),
            AppView::NonListStatesView { .. } => Some("non_list_states"),
            AppView::ScratchPadView { .. } => Some("scratch_pad"),
            AppView::ThemeChooserView { .. } => Some("theme_chooser"),
            AppView::ProcessManagerView { .. } => Some("process_manager"),
            AppView::FlowUxView { .. } => Some("flow_ux"),
            AppView::CurrentAppCommandsView { .. } => Some("current_app_commands"),
            AppView::SearchAiPresetsView { .. } => Some("search_ai_presets"),
            AppView::SettingsView { .. } => Some("settings"),
            AppView::PermissionsWizardView { .. } => Some("permissions_wizard"),
            AppView::BrowseKitsView { .. } => Some("kit_store_browse"),
            AppView::MigrateV1View { .. } => Some("migrate_v1"),
            AppView::InstalledKitsView { .. } => Some("kit_store_installed"),
            AppView::FavoritesBrowseView { .. } => Some("favorites"),
            AppView::ConfirmPrompt { .. } => Some("confirm_prompt"),
            AppView::TipsView { .. } => Some("tips"),
            AppView::TermPrompt { .. } => Some("term_prompt"),
            AppView::About { .. } => Some("about"),
            AppView::MicroPrompt { .. } => Some("micro_prompt"),
            AppView::SdkReferenceView { .. } => Some("sdk_reference"),
            AppView::ScriptTemplateCatalogView { .. } => Some("script_template_catalog"),
            AppView::CreateAiPresetView { .. } => Some("create_ai_preset"),
            AppView::NotesBrowseView { .. } => Some("notes_browse"),
            // The live Cmd+K path is an attached popup that keeps the host
            // AppView current (and therefore keeps the host footer visible).
            // This dead full-view variant stores no host identity, so it stays
            // deferred rather than advertising a decorative dialog footer.
            AppView::ActionsDialog => None,
            #[cfg(feature = "storybook")]
            AppView::DesignExplorerView { .. } => None,
        }
    }
}

impl SurfaceKind {
    /// Exhaustive behavior contract for every stable launcher surface kind.
    ///
    /// Do **not** add `_ => ...` here. The point is to make rustc fail when
    /// a new [`SurfaceKind`] is added without explicit behavior decisions.
    pub(crate) fn surface_contract(self) -> LauncherSurfaceContract {
        use LauncherSurfaceActionsPolicy::{
            ActionsDialogActions, ChildViewActions, HostRowActions, MainMenuActions,
            NoSurfaceActions, PromptEntityActions,
        };
        use LauncherSurfaceFamily::{
            AssistantWorkspace, AttachmentPortal, FeedbackSurface, FilterableLauncherList,
            MainMenu, ScriptPrompt, UtilityWorkspace,
        };
        use LauncherSurfaceFocusPolicy::{
            ChildViewFocus, LauncherFilterFocus, NoEditableFocus, PromptEntityFocus,
        };
        use LauncherSurfaceInputOwnership::{
            ChildView, LauncherFilter, NoEditableInput, PromptEntity,
        };
        use LauncherSurfaceKeyboardPolicy::{
            ActionsDialogKeyboard, ChildViewKeyboard, LauncherListKeyboard, NoEditableKeyboard,
            PromptEntityKeyboard,
        };
        use LauncherSurfacePreviewRole::{
            ContentPane, FeedbackPanel, NoPersistentPreview, OptionalInfoPanel,
            RequiredSplitPreview,
        };
        use LauncherSurfaceProofPolicy::{
            ChildViewStateProof, PopupStateProof, StateAndElementsProof, StateReceiptProof,
        };
        use LauncherSurfaceVisualPolicy::{
            CompactLauncherVisual, ContentPaneVisual, FeedbackVisual, PopupVisual,
            SplitPreviewVisual,
        };

        let standard = DismissPolicy::standard_launcher_surface();
        let explicit = DismissPolicy::explicit_cmd_w_only();

        match self {
            SurfaceKind::ScriptList => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(MainMenu, LauncherFilter, OptionalInfoPanel),
                LauncherFilterFocus,
                LauncherListKeyboard,
                MainMenuActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::About => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FeedbackSurface,
                    NoEditableInput,
                    ContentPane,
                ),
                NoEditableFocus,
                NoEditableKeyboard,
                NoSurfaceActions,
                StateReceiptProof,
                ContentPaneVisual,
                explicit,
                "about",
            ),
            SurfaceKind::ActionsDialog => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                ActionsDialogKeyboard,
                ActionsDialogActions,
                PopupStateProof,
                PopupVisual,
                standard,
                "scriptList",
            ),

            SurfaceKind::PromptEntity => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    ScriptPrompt,
                    PromptEntity,
                    NoPersistentPreview,
                ),
                PromptEntityFocus,
                PromptEntityKeyboard,
                PromptEntityActions,
                StateReceiptProof,
                CompactLauncherVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::PromptEntityCancelsToScript => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    ScriptPrompt,
                    PromptEntity,
                    NoPersistentPreview,
                ),
                PromptEntityFocus,
                PromptEntityKeyboard,
                PromptEntityActions,
                StateReceiptProof,
                CompactLauncherVisual,
                DismissPolicy::cancel_to_script_prompt(),
                "scriptList",
            ),

            SurfaceKind::PromptChildContent => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(ScriptPrompt, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::ExplicitPromptEntity => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    ScriptPrompt,
                    PromptEntity,
                    NoPersistentPreview,
                ),
                PromptEntityFocus,
                PromptEntityKeyboard,
                PromptEntityActions,
                StateReceiptProof,
                CompactLauncherVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::Webcam => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(ScriptPrompt, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                DismissPolicy::sticky_escape_closes(),
                "scriptList",
            ),

            SurfaceKind::ClipboardHistory => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                standard,
                "clipboardHistory",
            ),
            SurfaceKind::AppLauncher => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "appLauncher",
            ),
            SurfaceKind::WindowSwitcher => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "windowSwitcher",
            ),
            SurfaceKind::BrowserTabs => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "browserTabs",
            ),
            SurfaceKind::GenericFilterableList => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::Settings => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "settings",
            ),
            SurfaceKind::KitStoreBrowse => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "kitStoreBrowse",
            ),
            SurfaceKind::MigrateV1 => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "migrate_v1",
            ),
            SurfaceKind::KitStoreInstalled => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "kitStoreInstalled",
            ),
            SurfaceKind::ProcessManager => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                explicit,
                "processManager",
            ),
            // Flow UX exploration surfaces (Flash/Dispatch/Lens). Escape is
            // view-owned (`explicit`): it backgrounds Flash's inline run or
            // clears the filter before ever closing the window, and launched
            // flow runs must survive window blur untouched.
            SurfaceKind::FlowUx => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                explicit,
                "flowUx",
            ),
            SurfaceKind::CurrentAppCommands => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                explicit,
                "currentAppCommands",
            ),

            // Wizard intentionally uses the explicit dismiss policy: the grant
            // flow sends users to System Settings and back, so window blur must
            // not tear the wizard down mid-flow.
            SurfaceKind::PermissionsWizard => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    NoEditableInput,
                    NoPersistentPreview,
                ),
                NoEditableFocus,
                NoEditableKeyboard,
                NoSurfaceActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                explicit,
                "permissionsWizard",
            ),

            SurfaceKind::DesignGallery => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "designGallery",
            ),
            SurfaceKind::FooterGallery => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "footerGallery",
            ),
            SurfaceKind::NonListStates => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(UtilityWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                standard,
                "nonListStates",
            ),
            #[cfg(feature = "storybook")]
            SurfaceKind::DesignExplorer => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(UtilityWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::UtilityChildContent => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(UtilityWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                explicit,
                "scriptList",
            ),
            // Flow sessions background on blur: the conversation keeps
            // running while the launcher hides and resets to the main menu,
            // so the next hotkey press lands on the main menu with the
            // session still live under the desk's Active rows.
            SurfaceKind::FlowSession => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(UtilityWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                DismissPolicy::blur_closes_escape_view_owned(),
                "flowSession",
            ),
            SurfaceKind::FileSearchMini => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                standard,
                "fileSearch",
            ),
            SurfaceKind::FileSearchFull => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                standard,
                "fileSearch",
            ),
            SurfaceKind::ProfileSearch => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                standard,
                "profileSearch",
            ),
            SurfaceKind::ThemeChooser => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::EmojiPicker => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    UtilityWorkspace,
                    LauncherFilter,
                    NoPersistentPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                CompactLauncherVisual,
                explicit,
                "emojiPicker",
            ),

            SurfaceKind::Feedback => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FeedbackSurface,
                    NoEditableInput,
                    FeedbackPanel,
                ),
                NoEditableFocus,
                NoEditableKeyboard,
                NoSurfaceActions,
                StateReceiptProof,
                FeedbackVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::SdkReference => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::Tips => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                explicit,
                "scriptList",
            ),
            SurfaceKind::ScriptTemplateCatalog => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FilterableLauncherList,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                explicit,
                "scriptTemplateCatalog",
            ),

            SurfaceKind::AgentChatHistory => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    AssistantWorkspace,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::AttachmentPortalBrowser => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    AttachmentPortal,
                    LauncherFilter,
                    RequiredSplitPreview,
                ),
                LauncherFilterFocus,
                LauncherListKeyboard,
                HostRowActions,
                StateAndElementsProof,
                SplitPreviewVisual,
                standard,
                "scriptList",
            ),
            SurfaceKind::AgentChat => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(AssistantWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                ChildViewActions,
                ChildViewStateProof,
                ContentPaneVisual,
                explicit,
                "agentChatChat",
            ),
            SurfaceKind::DayPage => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(AssistantWorkspace, ChildView, ContentPane),
                ChildViewFocus,
                ChildViewKeyboard,
                MainMenuActions,
                ChildViewStateProof,
                ContentPaneVisual,
                standard,
                "dayPage",
            ),
            SurfaceKind::ConfirmPrompt => LauncherSurfaceContract::new(
                LauncherSurfaceContractVocabulary::new(
                    FeedbackSurface,
                    NoEditableInput,
                    FeedbackPanel,
                ),
                NoEditableFocus,
                NoEditableKeyboard,
                NoSurfaceActions,
                PopupStateProof,
                PopupVisual,
                explicit,
                "confirmPrompt",
            ),
        }
    }
}

/// Map an [`AppView`] variant to the automation `semanticSurface` tag.
///
/// Callers feed this value into
/// [`crate::windows::update_automation_semantic_surface`] after a subview
/// transition so `listAutomationWindows.windows[0].semanticSurface` re-keys
/// on the active surface rather than reporting the host kind.
fn semantic_surface_for_main_view(view: &AppView) -> Option<String> {
    Some(
        view.surface_contract()
            .automation_semantic_surface
            .to_string(),
    )
}

/// Wrapper to hold a script session that can be shared across async boundaries
/// Uses parking_lot::Mutex which doesn't poison on panic, avoiding .unwrap() calls
type SharedSession = Arc<ParkingMutex<Option<executor::ScriptSession>>>;

/// Tracks which input field currently has focus for cursor display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusedInput {
    /// Main script list filter input
    MainFilter,
    /// Actions dialog search input
    ActionsSearch,
    /// Arg prompt input (when running a script)
    ArgPrompt,
    /// No input focused (e.g., terminal prompt)
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum MainWindowMode {
    Full,
    #[default]
    Mini,
}

/// Pending focus target - identifies which element should receive focus
/// when window access becomes available. This prevents the "perpetual focus
/// enforcement in render()" anti-pattern that causes focus thrash.
///
/// Focus is applied once when pending_focus is set, then cleared.
/// This mechanism allows non-render code paths (like handle_prompt_message)
/// to request focus changes that are applied on the next render.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusTarget {
    /// Focus the main filter input (gpui_input_state)
    MainFilter,
    /// Focus the app root (self.focus_handle)
    AppRoot,
    /// Focus the actions dialog (if open)
    ActionsDialog,
    /// Focus the path prompt's focus handle
    PathPrompt,
    /// Focus the form prompt (delegates to active field)
    FormPrompt,
    /// Focus the editor prompt
    EditorPrompt,
    /// Focus the select prompt
    SelectPrompt,
    /// Focus the env prompt
    EnvPrompt,
    /// Focus the drop prompt
    DropPrompt,
    /// Focus the template prompt
    TemplatePrompt,
    /// Focus the term prompt
    TermPrompt,
    /// Focus the chat prompt
    ChatPrompt,
    /// Focus the launcher Agent Chat chat composer
    AgentChat,
    /// Focus the naming prompt
    NamingPrompt,
}

/// Identifies which prompt type is hosting the actions dialog.
///
/// This determines focus restoration behavior when the dialog closes,
/// since different prompt types have different focus targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // MainList variant reserved for render_script_list.rs refactoring
enum ActionsDialogHost {
    /// Actions in arg prompt (restore focus to ArgPrompt input)
    ArgPrompt,
    /// Actions in div prompt (restore focus to None - div has no input)
    DivPrompt,
    /// Actions in editor prompt (restore focus to None - editor handles its own focus)
    EditorPrompt,
    /// Actions in template prompt (restore focus to TemplatePrompt)
    TemplatePrompt,
    /// Actions in term prompt (restore focus to None - terminal handles its own focus)
    TermPrompt,
    /// Actions in form prompt (restore focus to None - form handles field focus)
    FormPrompt,
    /// Actions in chat prompt (restore focus to ChatPrompt input)
    ChatPrompt,
    /// Actions in main script list (restore focus to MainFilter)
    MainList,
    /// Actions in file search (restore focus to file search input)
    FileSearch,
    /// Actions in clipboard history (restore focus to clipboard search input)
    ClipboardHistory,
    /// Actions in dictation history (restore focus to dictation search input)
    DictationHistory,
    /// Actions in favorites browser (restore focus to favorites search input)
    Favorites,
    /// Actions in theme chooser / theme designer (restore focus to theme search input)
    ThemeChooser,
    /// Actions in emoji picker (restore focus to emoji search input)
    EmojiPicker,
    /// Actions in app launcher (restore focus to app launcher input)
    AppLauncher,
    /// Actions in built-in list and gallery surfaces (restore focus to main filter)
    BuiltinList,
    /// Actions in the Flow Desk / a flow session (focused flow or session
    /// verbs: converse, run once, edit, reveal, stop, dismiss)
    FlowDesk,
    /// Actions in webcam prompt (restore focus to None - webcam has no input)
    WebcamPrompt,
    /// Actions in Agent Chat chat (restore focus to Agent Chat chat input)
    AgentChat,
    /// Actions in Agent Chat history browser (restore focus to history search input)
    AgentChatHistory,
    /// Actions in the detached Agent Chat chat window (routes to the detached
    /// window's own dispatcher via `dispatch_action_to_detached`; focus
    /// restoration is handled by the detached window, not the main view)
    AgentChatDetached,
}

/// Input mode for list navigation - tracks whether user is using keyboard or mouse.
/// When in Keyboard mode, hover effects are disabled to prevent dual-highlight.
/// Mouse movement switches back to Mouse mode, re-enabling hover.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InputMode {
    #[default]
    Mouse,
    Keyboard,
}

/// Result of routing a key event to the actions dialog.
///
/// Returned by `route_key_to_actions_dialog` to indicate how the caller
/// should proceed after routing.
#[derive(Debug, Clone)]
enum ActionsRoute {
    /// Actions popup is not open - key was not handled, caller should process normally
    NotHandled,
    /// Key was handled by the actions dialog - caller should return/stop propagation
    Handled,
    /// User selected an action - caller should execute the activation.
    Execute {
        action_id: String,
        should_close: bool,
    },
}

/// File-search preview thumbnail lifecycle state.
///
/// Tracks async thumbnail loading for the right-side FileSearch preview panel.
/// The `path` field on non-idle variants guards against stale async updates.
#[derive(Clone)]
enum FileSearchThumbnailPreviewState {
    /// No thumbnail should be rendered (no selection or non-image selection).
    Idle,
    /// Thumbnail load is in-flight for this path.
    Loading { path: String },
    /// Thumbnail loaded successfully with decoded image and dimensions.
    Ready {
        path: String,
        image: Arc<gpui::RenderImage>,
        width: u32,
        height: u32,
    },
    /// Thumbnail not available for this path (size/format/decode constraints).
    Unavailable { path: String, message: String },
}

/// State for the inline shortcut recorder overlay.
///
/// When this is Some, the ShortcutRecorder modal is displayed.
/// Used for configuring keyboard shortcuts without opening an external editor.
#[derive(Debug, Clone)]
struct ShortcutRecorderState {
    /// The unique command identifier (e.g., "scriptlet/my-script", "builtin/clipboard-history")
    command_id: String,
    /// Human-readable name of the command being configured
    command_name: String,
}

/// State for the inline alias input overlay.
///
/// When this is Some, the alias input modal is displayed.
/// Used for configuring command aliases.
#[derive(Debug, Clone)]
struct AliasInputState {
    /// The unique command identifier (e.g., "builtin/clipboard-history", "app/com.apple.Safari")
    command_id: String,
    /// Human-readable name of the command being configured
    command_name: String,
    /// Current alias text being edited
    alias_text: String,
}

/// State for the Tab AI save-offer overlay, shown after a successful ephemeral
/// script execution when the user is prompted to persist the script.
#[derive(Debug, Clone)]
struct TabAiSaveOfferState {
    /// The execution record for the completed Tab AI run.
    record: crate::ai::TabAiExecutionRecord,
    /// Derived filename stem (e.g. "force-quit-this-app").
    filename_stem: String,
    /// Error message if save attempt failed, shown inline in the overlay.
    error: Option<SharedString>,
}

#[cfg(test)]
mod dismiss_contract_tests {
    use super::*;

    /// One Escape owner per prompt kind, declared in the DismissPolicy table.
    ///
    /// Cancel-to-script prompts (select/path/drop/template + env/naming) call
    /// `submit_cancel()` from their entities so the script receives `None`;
    /// the launcher shell must NOT also close on Escape — two owners made
    /// Escape a race between "window gone" and "script got None". Editor and
    /// terminal child content likewise own Escape locally (guarded cancel /
    /// PTY pass-through).
    #[test]
    fn escape_ownership_is_declared_once_per_prompt_kind() {
        // Shell-owned: Escape closes the launcher window.
        for kind in [SurfaceKind::PromptEntity, SurfaceKind::Webcam] {
            assert!(
                kind.surface_contract()
                    .dismiss_policy
                    .closes_main_window_on(DismissTrigger::Escape),
                "{kind:?}: Escape should close the main window"
            );
        }
        // Entity-owned: the shell must defer on Escape.
        for kind in [
            SurfaceKind::PromptEntityCancelsToScript,
            SurfaceKind::ExplicitPromptEntity,
            SurfaceKind::PromptChildContent,
        ] {
            assert!(
                !kind
                    .surface_contract()
                    .dismiss_policy
                    .closes_main_window_on(DismissTrigger::Escape),
                "{kind:?}: Escape belongs to the prompt entity, not the shell"
            );
        }
        // Cancel-to-script prompts still blur-close like standard surfaces.
        assert!(SurfaceKind::PromptEntityCancelsToScript
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::WindowBlur));
        // Webcam: sticky on blur (live capture), but Escape/Cmd+W close.
        assert!(!SurfaceKind::Webcam
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::WindowBlur));
        assert!(SurfaceKind::Webcam
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::CmdW));
    }

    /// Flow sessions background instead of dying: clicking away (blur) hides
    /// the launcher while the turn keeps running, Escape stays view-owned
    /// (backgrounds to the desk, never kills the session), and Cmd+W closes
    /// the window. Pairs with the runtime probe proving `flow_sessions`
    /// survive `close_and_reset_window`.
    #[test]
    fn flow_session_backgrounds_on_blur_and_owns_escape() {
        let policy = SurfaceKind::FlowSession.surface_contract().dismiss_policy;
        assert!(
            policy.closes_main_window_on(DismissTrigger::WindowBlur),
            "unfocusing a flow must hide the launcher (session keeps running)"
        );
        assert!(
            !policy.closes_main_window_on(DismissTrigger::Escape),
            "Escape is view-owned: back to the desk, never window-close"
        );
        assert!(policy.closes_main_window_on(DismissTrigger::CmdW));
        assert_eq!(
            AppView::FlowSessionView { session_id: 1 }.surface_kind(),
            SurfaceKind::FlowSession,
            "the flow session view must map to its dedicated surface kind"
        );
    }

    /// The cancellable prompt views must map to the cancel-to-script surface
    /// kind — adding a new cancellable prompt without reclassifying it here
    /// silently reintroduces the double-Escape-owner race.
    #[test]
    fn cancellable_prompt_views_use_cancel_to_script_kind() {
        // Compile-time companion: surface_kind() is an exhaustive match, so
        // new AppView variants already force a classification decision. This
        // test locks the four known cancellable prompts to the right kind via
        // the variant list in surface_kind() — see SelectPrompt/PathPrompt/
        // DropPrompt/TemplatePrompt arm.
        let source = include_str!("app_view_state.rs");
        let arm_start = source
            .find("=> SurfaceKind::PromptEntityCancelsToScript")
            .expect("cancel-to-script arm should exist");
        let preceding = &source[arm_start.saturating_sub(400)..arm_start];
        for variant in [
            "AppView::SelectPrompt",
            "AppView::PathPrompt",
            "AppView::DropPrompt",
            "AppView::TemplatePrompt",
        ] {
            assert!(
                preceding.contains(variant),
                "{variant} must classify as PromptEntityCancelsToScript (entity owns Escape)"
            );
        }
    }
}
