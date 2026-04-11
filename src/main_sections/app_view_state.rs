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
#[derive(Debug, Clone)]
enum AppView {
    /// Showing the script list
    ScriptList,
    /// Showing the actions dialog (mini searchable popup)
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
    /// Showing design gallery (separator and icon variations)
    DesignGalleryView {
        filter: String,
        selected_index: usize,
    },
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
    /// Showing file search results
    FileSearchView {
        query: String,
        selected_index: usize,
        presentation: FileSearchPresentation,
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
    /// Showing creation feedback with file path and quick actions after script/extension creation.
    /// Requires explicit dismiss (Enter/Escape/button) — non-dismissable by click-outside.
    CreationFeedback { path: std::path::PathBuf },
    /// Browsing the Kit Store (GitHub search for installable kits)
    BrowseKitsView {
        query: String,
        selected_index: usize,
        results: Vec<KitStoreSearchResult>,
    },
    /// Managing locally installed kits (update/remove)
    InstalledKitsView {
        selected_index: usize,
        kits: Vec<script_kit_gpui::kit_store::InstalledKit>,
    },
    /// Showing process manager (running background scripts)
    /// Data comes from cached_processes field populated on open
    ProcessManagerView {
        filter: String,
        selected_index: usize,
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
    /// Lists categories: API Keys, Theme, Window Positions, Feature Toggles, Hotkeys
    SettingsView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing favorites with search/filter
    /// Supports Enter to run, D to remove, U/J to reorder, Esc to go back
    FavoritesBrowseView {
        filter: String,
        selected_index: usize,
    },
    /// Showing menu bar commands from the frontmost application
    /// Data comes from cached_current_app_entries populated on open
    CurrentAppCommandsView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing ACP conversation history with search and preview
    AcpHistoryView {
        filter: String,
        selected_index: usize,
    },
    /// Browsing notes from ACP as an attachment portal
    NotesBrowseView {
        filter: String,
        selected_index: usize,
    },
    /// Showing the ACP-backed Tab AI chat surface for the default Tab path.
    ///
    /// Verification-bearing script-authoring requests deliberately route to
    /// `QuickTerminalView` so the agent can run Bun verification inside the
    /// live harness terminal session before reporting success.
    AcpChatView {
        entity: Entity<crate::ai::acp::view::AcpChatView>,
    },
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
    /// Actions in emoji picker (restore focus to emoji search input)
    EmojiPicker,
    /// Actions in app launcher (restore focus to app launcher input)
    AppLauncher,
    /// Actions in built-in list and gallery surfaces (restore focus to main filter)
    BuiltinList,
    /// Actions in webcam prompt (restore focus to None - webcam has no input)
    WebcamPrompt,
    /// Actions in ACP chat (restore focus to ACP chat input)
    AcpChat,
    /// Actions in ACP history browser (restore focus to history search input)
    AcpHistory,
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
    /// User selected an action - caller should execute it via trigger_action_by_name
    Execute { action_id: String },
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
