//! Script Kit environment setup and initialization.
//!
//! Ensures ~/.scriptkit exists with required directories and starter files.
//! The path can be overridden via the SK_PATH environment variable.
//! Idempotent: user-owned files are never overwritten; app-owned files may be refreshed.

// --- merged from part_000.rs ---
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{debug, info, instrument, warn};
/// Embedded config template (included at compile time)
const EMBEDDED_CONFIG_TEMPLATE: &str = include_str!("../../kit-init/config-template.ts");
/// Embedded SDK content (included at compile time)
const EMBEDDED_SDK: &str = include_str!("../../scripts/kit-sdk.ts");
/// Embedded user-facing command line shim
const EMBEDDED_SCRIPTKIT_CLI: &str = include_str!("../../scripts/mcp-cli.ts");
/// Optional theme example (included at compile time)
const EMBEDDED_THEME_EXAMPLE: &str = include_str!("../../kit-init/theme.example.json");
pub(crate) const EMBEDDED_TIPS: &str = include_str!("../../kit-init/tips.json");
/// Embedded package.json template for user's kit directory
/// The "type": "module" enables top-level await in all .ts scripts
const EMBEDDED_PACKAGE_JSON: &str = r#"{
  "name": "@scriptkit/kit",
  "type": "module",
  "private": true,
  "scripts": {
    "typecheck": "tsc --noEmit"
  }
}
"#;
/// Embedded GUIDE.md comprehensive user guide
const EMBEDDED_GUIDE_MD: &str = include_str!("../../kit-init/GUIDE.md");
/// Embedded CleanShot X extension (built-in extension that ships with the app)
const EMBEDDED_CLEANSHOT_EXTENSION: &str =
    include_str!("../../kit-init/scriptlets/cleanshot/main.md");
/// Embedded CleanShot X shared actions (built-in actions for all cleanshot scriptlets)
const EMBEDDED_CLEANSHOT_ACTIONS: &str =
    include_str!("../../kit-init/scriptlets/cleanshot/main.actions.md");
/// Embedded 1Password extension (built-in extension that ships with the app)
const EMBEDDED_1PASSWORD_EXTENSION: &str =
    include_str!("../../kit-init/scriptlets/1password/main.md");
/// Embedded Quick Links extension (built-in extension that ships with the app)
const EMBEDDED_QUICKLINKS_EXTENSION: &str =
    include_str!("../../kit-init/scriptlets/quicklinks/main.md");
/// Embedded Quick Links shared actions (built-in actions for all quicklinks scriptlets)
const EMBEDDED_QUICKLINKS_ACTIONS: &str =
    include_str!("../../kit-init/scriptlets/quicklinks/main.actions.md");
/// Embedded Window Management extension (built-in extension that ships with the app)
const EMBEDDED_WINDOW_MANAGEMENT_EXTENSION: &str =
    include_str!("../../kit-init/scriptlets/window-management/main.md");
/// Embedded AI Text Tools extension (built-in extension that ships with the app)
const EMBEDDED_AI_TEXT_TOOLS_EXTENSION: &str =
    include_str!("../../kit-init/scriptlets/ai-text-tools/main.md");
/// Root-level CLAUDE.md for the ~/.scriptkit workspace (the harness cwd)
const EMBEDDED_ROOT_CLAUDE_MD: &str = include_str!("../../kit-init/ROOT_CLAUDE.md");
/// Root-level AGENTS.md SDK reference for the ~/.scriptkit workspace
const EMBEDDED_ROOT_AGENTS_MD: &str = include_str!("../../kit-init/ROOT_AGENTS.md");
/// Skills README
const EMBEDDED_SKILLS_README: &str = include_str!("../../kit-init/skills/README.md");
/// Skill: new script
const EMBEDDED_SKILL_NEW_SCRIPT: &str = include_str!("../../kit-init/skills/new-script/SKILL.md");
/// Skill: new scriptlet
const EMBEDDED_SKILL_NEW_SCRIPTLET: &str =
    include_str!("../../kit-init/skills/new-scriptlet/SKILL.md");
/// Skill: config updates
const EMBEDDED_SKILL_UPDATE_CONFIG: &str =
    include_str!("../../kit-init/skills/update-config/SKILL.md");
/// Skill: external MCP server configuration
const EMBEDDED_SKILL_CONFIGURE_MCP: &str =
    include_str!("../../kit-init/skills/configure-mcp/SKILL.md");
/// Skill: troubleshooting
const EMBEDDED_SKILL_TROUBLESHOOTING: &str =
    include_str!("../../kit-init/skills/troubleshoot/SKILL.md");
/// Example script: Todo app
const EMBEDDED_EXAMPLE_TODO_APP: &str = include_str!("../../kit-init/examples/scripts/todo-app.ts");
/// Canonical menu syntax handler: todo inbox
const EMBEDDED_CANONICAL_CAPTURE_TODO_INBOX: &str =
    include_str!("../../scripts/examples/menu-syntax/capture-todo-inbox.ts");
/// Canonical menu syntax handler: calendar event
const EMBEDDED_CANONICAL_CREATE_CALENDAR_EVENT: &str =
    include_str!("../../scripts/examples/menu-syntax/create-calendar-event.ts");
/// Canonical menu syntax handler: macOS Calendar event
const EMBEDDED_CANONICAL_CREATE_MAC_CALENDAR_EVENT: &str =
    include_str!("../../scripts/examples/menu-syntax/create-mac-calendar-event.ts");
/// Canonical menu syntax handler: Google Calendar event
const EMBEDDED_CANONICAL_ADD_GOOGLE_CALENDAR_EVENT: &str =
    include_str!("../../scripts/examples/menu-syntax/add-google-calendar-event.ts");
/// Canonical menu syntax handler: reminder
const EMBEDDED_CANONICAL_CREATE_REMINDER: &str =
    include_str!("../../scripts/examples/menu-syntax/create-reminder.ts");
/// Canonical menu syntax handler: snooze task
const EMBEDDED_CANONICAL_SNOOZE_TASK: &str =
    include_str!("../../scripts/examples/menu-syntax/snooze-task.ts");
/// Canonical menu syntax handler: defer task
const EMBEDDED_CANONICAL_DEFER_TASK: &str =
    include_str!("../../scripts/examples/menu-syntax/defer-task.ts");
/// Canonical menu syntax handler: daily note
const EMBEDDED_CANONICAL_APPEND_DAILY_NOTE: &str =
    include_str!("../../scripts/examples/menu-syntax/append-daily-note.ts");
/// Canonical menu syntax handler: social draft
const EMBEDDED_CANONICAL_DRAFT_SOCIAL_POST: &str =
    include_str!("../../scripts/examples/menu-syntax/draft-social-post.ts");
/// Canonical menu syntax handler: tagged link
const EMBEDDED_CANONICAL_SAVE_TAGGED_LINK: &str =
    include_str!("../../scripts/examples/menu-syntax/save-tagged-link.ts");
/// Examples README
const EMBEDDED_EXAMPLES_README: &str = include_str!("../../kit-init/examples/README.md");
/// Examples START_HERE launchpad
const EMBEDDED_EXAMPLES_START_HERE: &str = include_str!("../../kit-init/examples/START_HERE.md");
/// Skill: notes — working with the Notes window and automation targets
const EMBEDDED_SKILL_MANAGE_NOTES: &str =
    include_str!("../../kit-init/skills/manage-notes/SKILL.md");
/// Skill: new agent (compatibility — skills are now the preferred reusable AI unit)
const EMBEDDED_SKILL_NEW_AGENT: &str = include_str!("../../kit-init/skills/new-agent/SKILL.md");
/// Skill: Agent Chat — programmatic chat flows, typed context parts, streaming, and lifecycle
const EMBEDDED_SKILL_START_CHAT: &str = include_str!("../../kit-init/skills/start-chat/SKILL.md");
/// Skill: custom actions — Actions Menu commands in scripts and companion .actions.md files
const EMBEDDED_SKILL_ADD_ACTIONS: &str = include_str!("../../kit-init/skills/add-actions/SKILL.md");
/// Default Agent Chat agent catalog (seeded on first run — provider/catalog selection, not plugin skills)
const EMBEDDED_AGENT_CHAT_AGENTS_JSON: &str = r#"{
  "schemaVersion": 1,
  "agents": [
    {
      "id": "opencode",
      "displayName": "OpenCode",
      "command": "opencode",
      "args": ["agent_chat"],
      "env": {},
      "models": [],
      "install": {
        "command": "npm",
        "args": ["install", "-g", "opencode-ai"]
      }
    },
    {
      "id": "codex-agent_chat",
      "displayName": "Codex",
      "command": "codex-agent_chat",
      "args": [],
      "env": {},
      "models": [],
      "auth": {
        "summary": "Authenticate with ChatGPT, CODEX_API_KEY, or OPENAI_API_KEY."
      }
    }
  ]
}"#;
// --- merged from part_001.rs ---
// (Old kit-level agents doc constant removed — canonical docs live at root level)
// --- merged from part_002.rs ---
// (Old kit-level claude doc constant removed — canonical docs live at root level)
/// Environment variable to override the default ~/.scriptkit path
pub const SK_PATH_ENV: &str = "SK_PATH";
/// Result of setup process
#[derive(Debug)]
pub struct SetupResult {
    /// Whether ~/.scriptkit didn't exist before this run
    pub is_fresh_install: bool,
    /// Path to ~/.scriptkit (or SK_PATH override, or fallback if home dir couldn't be resolved)
    pub kit_path: PathBuf,
    /// Whether bun looks discoverable on this machine
    pub bun_available: bool,
    /// Any warnings encountered during setup
    pub warnings: Vec<String>,
}
/// Get the kit path, respecting SK_PATH environment variable
///
/// Priority:
/// 1. SK_PATH environment variable (if set)
/// 2. ~/.scriptkit (default)
/// 3. Temp directory fallback (if home dir unavailable)
pub fn get_kit_path() -> PathBuf {
    // Check for SK_PATH override first
    if let Ok(sk_path) = std::env::var(SK_PATH_ENV) {
        if let Ok(expanded) = shellexpand::full(&sk_path) {
            return PathBuf::from(expanded.as_ref());
        }
        return PathBuf::from(shellexpand::tilde(&sk_path).as_ref());
    }

    // Default: ~/.scriptkit
    match dirs::home_dir() {
        Some(home) => home.join(".scriptkit"),
        None => std::env::temp_dir().join("script-kit"),
    }
}

/// Container for all Script Kit plugins under the active workspace.
pub fn plugins_path() -> PathBuf {
    get_kit_path().join("plugins")
}

/// User-owned TypeScript configuration file under the active workspace.
pub fn config_ts_path() -> PathBuf {
    get_kit_path().join("config.ts")
}

/// User-owned theme override file under the active workspace.
pub fn theme_json_path() -> PathBuf {
    get_kit_path().join("theme.json")
}

/// User-authored theme presets directory under the active workspace.
pub fn themes_dir() -> PathBuf {
    get_kit_path().join("themes")
}
/// Migrate from legacy ~/.kenv to new ~/.scriptkit structure
///
/// This function handles one-time migration from the old directory structure:
/// - Moves ~/.kenv contents to ~/.scriptkit
/// - Moves ~/.kenv/scripts to ~/.scriptkit/plugins/main/scripts
/// - Moves ~/.kenv/scriptlets to ~/.scriptkit/plugins/main/scriptlets
/// - Creates a symlink ~/.kenv -> ~/.scriptkit for backwards compatibility
///
/// Returns true if migration was performed, false if not needed
#[instrument(level = "info", name = "migrate_from_kenv")]
pub fn migrate_from_kenv() -> bool {
    let home = match dirs::home_dir() {
        Some(h) => h,
        None => return false,
    };

    let old_kenv = home.join(".kenv");
    let new_scriptkit = home.join(".scriptkit");

    // Only migrate if old path exists and new path doesn't
    if !old_kenv.exists() || new_scriptkit.exists() {
        return false;
    }

    info!(
        old_path = %old_kenv.display(),
        new_path = %new_scriptkit.display(),
        "Migrating from ~/.kenv to ~/.scriptkit"
    );

    let mut failures = Vec::<String>::new();

    // Create the new plugin structure.
    let main_scripts = new_scriptkit.join("plugins").join("main").join("scripts");
    let main_scriptlets = new_scriptkit
        .join("plugins")
        .join("main")
        .join("scriptlets");

    if let Err(e) = fs::create_dir_all(&main_scripts) {
        warn!(error = %e, "Failed to create main/scripts directory");
        return false;
    }

    if let Err(e) = fs::create_dir_all(&main_scriptlets) {
        warn!(error = %e, "Failed to create main/scriptlets directory");
        return false;
    }

    // Move scripts from ~/.kenv/scripts to ~/.scriptkit/plugins/main/scripts
    let old_scripts = old_kenv.join("scripts");
    if old_scripts.exists() && old_scripts.is_dir() {
        match fs::read_dir(&old_scripts) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let old_path = entry.path();
                            let file_name = old_path.file_name().unwrap_or_default();
                            let new_path = main_scripts.join(file_name);

                            if let Err(e) = fs::rename(&old_path, &new_path) {
                                let err_msg = format!(
                                    "Failed to move script from {} to {}: {}",
                                    old_path.display(),
                                    new_path.display(),
                                    e
                                );
                                warn!("{}", err_msg);
                                failures.push(err_msg);
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to read script entry: {}", e);
                            warn!("{}", err_msg);
                            failures.push(err_msg);
                        }
                    }
                }
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to read scripts directory ({}): {}",
                    old_scripts.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Move scriptlets from ~/.kenv/scriptlets to ~/.scriptkit/plugins/main/scriptlets
    let old_scriptlets = old_kenv.join("scriptlets");
    if old_scriptlets.exists() && old_scriptlets.is_dir() {
        match fs::read_dir(&old_scriptlets) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) => {
                            let old_path = entry.path();
                            let file_name = old_path.file_name().unwrap_or_default();
                            let new_path = main_scriptlets.join(file_name);

                            if let Err(e) = fs::rename(&old_path, &new_path) {
                                let err_msg = format!(
                                    "Failed to move scriptlet from {} to {}: {}",
                                    old_path.display(),
                                    new_path.display(),
                                    e
                                );
                                warn!("{}", err_msg);
                                failures.push(err_msg);
                            }
                        }
                        Err(e) => {
                            let err_msg = format!("Failed to read scriptlet entry: {}", e);
                            warn!("{}", err_msg);
                            failures.push(err_msg);
                        }
                    }
                }
            }
            Err(e) => {
                let err_msg = format!(
                    "Failed to read scriptlets directory ({}): {}",
                    old_scriptlets.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Move user config files into ~/.scriptkit/
    if let Err(e) = fs::create_dir_all(&new_scriptkit) {
        warn!(
            error = %e,
            path = %new_scriptkit.display(),
            "Failed to create ~/.scriptkit during migration"
        );
        return false;
    }

    let kit_files = [
        "config.ts",
        "theme.json",
        "tsconfig.json",
        "package.json",
        "settings.json",
    ];
    for file in kit_files {
        let old_path = old_kenv.join(file);
        let new_path = new_scriptkit.join(file);
        if old_path.exists() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                let err_msg = format!(
                    "Failed to move kit-owned config file from {} to {}: {}",
                    old_path.display(),
                    new_path.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Root-owned workspace files remain at ~/.scriptkit/
    let root_files = [".gitignore"];
    for file in root_files {
        let old_path = old_kenv.join(file);
        let new_path = new_scriptkit.join(file);
        if old_path.exists() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                let err_msg = format!(
                    "Failed to move root-owned workspace file from {} to {}: {}",
                    old_path.display(),
                    new_path.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Move data directories to new root
    let data_dirs = ["logs", "cache", "db", "sdk"];
    for dir in data_dirs {
        let old_path = old_kenv.join(dir);
        let new_path = new_scriptkit.join(dir);
        if old_path.exists() && old_path.is_dir() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                let err_msg = format!(
                    "Failed to move data directory {} from {} to {}: {}",
                    dir,
                    old_path.display(),
                    new_path.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Move data files to new root
    let data_files = [
        "frecency.json",
        "store.json",
        "server.json",
        "agent-token",
        "notes.db",
        "ai-chats.db",
        "clipboard-history.db",
    ];
    for file in data_files {
        let old_path = old_kenv.join(file);
        let new_path = new_scriptkit.join(file);
        if old_path.exists() && !new_path.exists() {
            if let Err(e) = fs::rename(&old_path, &new_path) {
                let err_msg = format!(
                    "Failed to move data file {} from {} to {}: {}",
                    file,
                    old_path.display(),
                    new_path.display(),
                    e
                );
                warn!("{}", err_msg);
                failures.push(err_msg);
            }
        }
    }

    // Remove the old ~/.kenv directory (should be mostly empty now)
    if failures.is_empty() {
        if let Err(e) = fs::remove_dir_all(&old_kenv) {
            warn!(error = %e, "Failed to remove old ~/.kenv directory, may have remaining files");
        }
    } else {
        tracing::error!(
            failures = ?failures,
            "Migration encountered failures. Preserving old ~/.kenv directory to prevent data loss."
        );
    }

    // Create symlink for backwards compatibility (Unix only)
    #[cfg(unix)]
    {
        if let Err(e) = std::os::unix::fs::symlink(&new_scriptkit, &old_kenv) {
            warn!(error = %e, "Failed to create ~/.kenv symlink for backwards compatibility");
        } else {
            info!("Created ~/.kenv -> ~/.scriptkit symlink for backwards compatibility");
        }
    }

    info!("Migration from ~/.kenv to ~/.scriptkit complete");
    true
}
// --- merged from part_003.rs ---
/// Ensure the ~/.scriptkit environment is properly set up.
///
/// This function is idempotent - it will create missing directories and files
/// without overwriting existing user configurations.
///
/// # Directory Structure Created
/// ```text
/// ~/.scriptkit/                  # Root (can be overridden via SK_PATH)
/// ├── kit/                       # All kits container (for easy version control)
/// │   ├── main/                  # Default user kit
/// │   │   ├── scripts/           # User scripts (.ts, .js files)
/// │   │   ├── scriptlets/         # Markdown scriptlet bundles
/// │   │   └── agents/             # AI agent definitions (.md)
/// │   └── custom-kit/            # Additional custom kits
/// │       ├── scripts/
/// │       ├── scriptlets/
/// │       └── agents/
/// │   ├── package.json           # Node.js module config (type: module for top-level await)
/// │   └── tsconfig.json          # TypeScript path mappings
/// │   ├── config.ts              # User configuration (created from template if missing)
/// │   ├── theme.json             # Theme configuration (created from example if missing)
/// │   ├── AGENTS.md              # Redirect stub → ../AGENTS.md
/// │   └── CLAUDE.md              # Redirect stub → ../CLAUDE.md
/// ├── sdk/                       # Runtime SDK (kit-sdk.ts)
/// ├── bin/                       # App-managed command shims
/// ├── db/                        # Databases
/// ├── logs/                      # Application logs
/// ├── cache/
/// │   └── app-icons/             # Cached application icons
/// ├── GUIDE.md                   # User guide
/// └── .gitignore                 # Ignore transient files
/// ```
///
/// # Environment Variables
/// - `SK_PATH`: Override the default ~/.scriptkit path
///
/// # Returns
/// `SetupResult` with information about the setup process.
#[instrument(level = "info", name = "ensure_kit_setup")]
pub fn ensure_kit_setup() -> SetupResult {
    let mut warnings = Vec::new();

    let kit_dir = get_kit_path();

    // Check if this is a fresh install before we create anything
    let is_fresh_install = !kit_dir.exists();

    // Log if using SK_PATH override
    if std::env::var(SK_PATH_ENV).is_ok() {
        info!(
            kit_path = %kit_dir.display(),
            "Using SK_PATH override"
        );
    }

    // Ensure root kit directory exists first
    if let Err(e) = fs::create_dir_all(&kit_dir) {
        warnings.push(format!(
            "Failed to create kit root {}: {}",
            kit_dir.display(),
            e
        ));
        // If we can't create the root, there's not much else we can safely do.
        return SetupResult {
            is_fresh_install,
            kit_path: kit_dir,
            bun_available: false,
            warnings,
        };
    }

    // Seed the user-themes directory so save/load operations don't race the
    // first-use creation on the hot path.
    if let Err(e) = crate::theme::user_themes::ensure_user_themes_dir() {
        warnings.push(format!("Failed to create user themes dir: {e}"));
    }

    // Plugin roots: each plugin gets {scripts,scriptlets,agents,skills} + plugin.json
    ensure_plugin_root(
        &kit_dir,
        "main",
        "Main",
        "Default personal plugin",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "cleanshot",
        "CleanShot X",
        "Built-in screenshot commands",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "1password",
        "1Password",
        "Built-in password-manager commands",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "quicklinks",
        "Quick Links",
        "Built-in quick link commands",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "window-management",
        "Window Management",
        "Built-in window commands",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "ai-text-tools",
        "AI Text Tools",
        "Built-in text tools",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "examples",
        "Examples",
        "Built-in examples",
        &mut warnings,
    );
    ensure_plugin_root(
        &kit_dir,
        "scriptkit",
        "Script Kit",
        "Built-in Script Kit skills and references",
        &mut warnings,
    );

    // Migrate legacy plugin-scoped extensions/ directories to scriptlets/.
    migrate_plugin_extensions_to_scriptlets(&kit_dir, &mut warnings);

    // Non-plugin infrastructure directories
    let required_dirs = [
        // Context bundle directory for Tab AI argv-based launch
        kit_dir.join("context"),
        // Root-level harness temp workspace used by kit://sdk-reference
        kit_dir.join("tmp").join("test-scripts"),
        kit_dir.join("tmp").join("test-scriptlets"),
        kit_dir.join("sdk"),
        kit_dir.join("bin"),
        kit_dir.join("db"),
        kit_dir.join("logs"),
        kit_dir.join("cache").join("app-icons"),
    ];

    for dir in required_dirs {
        ensure_dir(&dir, &mut warnings);
    }

    // App-managed: SDK (refresh if changed)
    let sdk_path = kit_dir.join("sdk").join("kit-sdk.ts");
    write_string_if_changed(&sdk_path, EMBEDDED_SDK, &mut warnings, "sdk/kit-sdk.ts");

    // App-managed: user-facing `scriptkit` command shim (refresh if changed)
    let scriptkit_cli_path = kit_dir.join("bin").join("scriptkit");
    write_executable_string_if_changed(
        &scriptkit_cli_path,
        EMBEDDED_SCRIPTKIT_CLI,
        &mut warnings,
        "bin/scriptkit",
    );

    let plugins_dir = kit_dir.join("plugins");

    // App-managed: Built-in CleanShot X extension (refresh if changed)
    // This extension ships with the app and provides screenshot/recording commands
    let cleanshot_path = kit_dir
        .join("plugins")
        .join("cleanshot")
        .join("scriptlets")
        .join("main.md");
    write_string_if_changed(
        &cleanshot_path,
        EMBEDDED_CLEANSHOT_EXTENSION,
        &mut warnings,
        "plugins/cleanshot/scriptlets/main.md",
    );

    // App-managed: Built-in CleanShot X shared actions (refresh if changed)
    // These actions are automatically available for all CleanShot scriptlets
    let cleanshot_actions_path = kit_dir
        .join("plugins")
        .join("cleanshot")
        .join("scriptlets")
        .join("main.actions.md");
    write_string_if_changed(
        &cleanshot_actions_path,
        EMBEDDED_CLEANSHOT_ACTIONS,
        &mut warnings,
        "plugins/cleanshot/scriptlets/main.actions.md",
    );

    // App-managed: Built-in 1Password extension (refresh if changed)
    // This extension ships with the app and provides password manager CLI commands
    let onepassword_path = kit_dir
        .join("plugins")
        .join("1password")
        .join("scriptlets")
        .join("main.md");
    write_string_if_changed(
        &onepassword_path,
        EMBEDDED_1PASSWORD_EXTENSION,
        &mut warnings,
        "plugins/1password/scriptlets/main.md",
    );

    // App-managed: Built-in Quick Links extension (refresh if changed)
    // This extension ships with the app and provides quick access to common websites
    let quicklinks_path = kit_dir
        .join("plugins")
        .join("quicklinks")
        .join("scriptlets")
        .join("main.md");
    write_string_if_changed(
        &quicklinks_path,
        EMBEDDED_QUICKLINKS_EXTENSION,
        &mut warnings,
        "plugins/quicklinks/scriptlets/main.md",
    );

    // App-managed: Built-in Quick Links shared actions (refresh if changed)
    // These actions are automatically available for all Quick Links scriptlets
    let quicklinks_actions_path = kit_dir
        .join("plugins")
        .join("quicklinks")
        .join("scriptlets")
        .join("main.actions.md");
    write_string_if_changed(
        &quicklinks_actions_path,
        EMBEDDED_QUICKLINKS_ACTIONS,
        &mut warnings,
        "plugins/quicklinks/scriptlets/main.actions.md",
    );

    // App-managed: Built-in Window Management extension (refresh if changed)
    // This extension ships with the app and provides window tiling and positioning
    let window_management_path = kit_dir
        .join("plugins")
        .join("window-management")
        .join("scriptlets")
        .join("main.md");
    write_string_if_changed(
        &window_management_path,
        EMBEDDED_WINDOW_MANAGEMENT_EXTENSION,
        &mut warnings,
        "plugins/window-management/scriptlets/main.md",
    );

    // App-managed: Built-in AI Text Tools extension (refresh if changed)
    // This extension ships with the app and provides AI-powered text transformations
    let ai_text_tools_path = kit_dir
        .join("plugins")
        .join("ai-text-tools")
        .join("scriptlets")
        .join("main.md");
    write_string_if_changed(
        &ai_text_tools_path,
        EMBEDDED_AI_TEXT_TOOLS_EXTENSION,
        &mut warnings,
        "plugins/ai-text-tools/scriptlets/main.md",
    );

    // User-owned: config.ts (only create if missing)
    let config_path = kit_dir.join("config.ts");
    write_string_if_missing(
        &config_path,
        EMBEDDED_CONFIG_TEMPLATE,
        &mut warnings,
        "config.ts",
    );

    // User-owned (optional): theme.json (only create if missing)
    let theme_path = kit_dir.join("theme.json");
    write_string_if_missing(
        &theme_path,
        EMBEDDED_THEME_EXAMPLE,
        &mut warnings,
        "theme.json",
    );

    // User-owned: local tips catalog (only create if missing).
    write_string_if_missing(
        &kit_dir.join("tips.json"),
        EMBEDDED_TIPS,
        &mut warnings,
        "tips.json",
    );

    // App-managed: tsconfig.json path mappings in the workspace root (merge-safe)
    ensure_tsconfig_paths(&kit_dir.join("tsconfig.json"), &mut warnings);

    // App-managed: package.json for top-level await support in plugin scripts
    let package_json_path = kit_dir.join("package.json");
    write_string_if_missing(
        &package_json_path,
        EMBEDDED_PACKAGE_JSON,
        &mut warnings,
        "package.json",
    );

    // User-owned: GUIDE.md (only create if missing)
    // Comprehensive user guide for learning Script Kit
    let guide_md_path = kit_dir.join("GUIDE.md");
    write_string_if_missing(&guide_md_path, EMBEDDED_GUIDE_MD, &mut warnings, "GUIDE.md");

    // User-owned: Agent Chat agent catalog (only create if missing)
    // Users add/edit Agent Chat agent entries here for multi-agent support
    let agent_chat_agents_path = kit_dir.join("agent_chat").join("agents.json");
    write_string_if_missing(
        &agent_chat_agents_path,
        EMBEDDED_AGENT_CHAT_AGENTS_JSON,
        &mut warnings,
        "agent_chat/agents.json",
    );

    // Root-level CLAUDE.md — the canonical agent instructions file.
    // Lives at ~/.scriptkit/CLAUDE.md so harnesses that cwd into ~/.scriptkit find it.
    let root_claude_md_path = kit_dir.join("CLAUDE.md");
    write_string_if_changed(
        &root_claude_md_path,
        EMBEDDED_ROOT_CLAUDE_MD,
        &mut warnings,
        "CLAUDE.md",
    );

    // Root-level AGENTS.md — SDK reference for all agents.
    let root_agents_md_path = kit_dir.join("AGENTS.md");
    write_string_if_changed(
        &root_agents_md_path,
        EMBEDDED_ROOT_AGENTS_MD,
        &mut warnings,
        "AGENTS.md",
    );

    // App-managed: Skills library — seeded into the Script Kit plugin (refresh if changed)
    let scriptkit_skills = plugins_dir.join("scriptkit").join("skills");
    write_string_if_changed(
        &scriptkit_skills.join("README.md"),
        EMBEDDED_SKILLS_README,
        &mut warnings,
        "plugins/scriptkit/skills/README.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("new-script").join("SKILL.md"),
        EMBEDDED_SKILL_NEW_SCRIPT,
        &mut warnings,
        "plugins/scriptkit/skills/new-script/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("new-scriptlet").join("SKILL.md"),
        EMBEDDED_SKILL_NEW_SCRIPTLET,
        &mut warnings,
        "plugins/scriptkit/skills/new-scriptlet/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("update-config").join("SKILL.md"),
        EMBEDDED_SKILL_UPDATE_CONFIG,
        &mut warnings,
        "plugins/scriptkit/skills/update-config/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("configure-mcp").join("SKILL.md"),
        EMBEDDED_SKILL_CONFIGURE_MCP,
        &mut warnings,
        "plugins/scriptkit/skills/configure-mcp/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("troubleshoot").join("SKILL.md"),
        EMBEDDED_SKILL_TROUBLESHOOTING,
        &mut warnings,
        "plugins/scriptkit/skills/troubleshoot/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("manage-notes").join("SKILL.md"),
        EMBEDDED_SKILL_MANAGE_NOTES,
        &mut warnings,
        "plugins/scriptkit/skills/manage-notes/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("new-agent").join("SKILL.md"),
        EMBEDDED_SKILL_NEW_AGENT,
        &mut warnings,
        "plugins/scriptkit/skills/new-agent/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("start-chat").join("SKILL.md"),
        EMBEDDED_SKILL_START_CHAT,
        &mut warnings,
        "plugins/scriptkit/skills/start-chat/SKILL.md",
    );
    write_string_if_changed(
        &scriptkit_skills.join("add-actions").join("SKILL.md"),
        EMBEDDED_SKILL_ADD_ACTIONS,
        &mut warnings,
        "plugins/scriptkit/skills/add-actions/SKILL.md",
    );

    // App-managed: Examples plugin. Keep this intentionally small: one real
    // Todo app example plus a short README/launchpad.
    let examples_plugin = plugins_dir.join("examples");
    prune_managed_examples_plugin(&examples_plugin, &mut warnings);
    write_string_if_changed(
        &examples_plugin.join("README.md"),
        EMBEDDED_EXAMPLES_README,
        &mut warnings,
        "plugins/examples/README.md",
    );
    write_string_if_changed(
        &examples_plugin.join("START_HERE.md"),
        EMBEDDED_EXAMPLES_START_HERE,
        &mut warnings,
        "plugins/examples/START_HERE.md",
    );
    write_string_if_changed(
        &examples_plugin.join("scripts").join("todo-app.ts"),
        EMBEDDED_EXAMPLE_TODO_APP,
        &mut warnings,
        "plugins/examples/scripts/todo-app.ts",
    );

    // App-managed: .gitignore (refresh if changed)
    let gitignore_path = kit_dir.join(".gitignore");
    let gitignore_content = r#"# Script Kit managed .gitignore
# This file is regenerated on app start - edit with caution

# =============================================================================
# Node.js / Bun dependencies
# =============================================================================
# Root node_modules (for package.json at ~/.scriptkit/plugins/)
node_modules/

# Kit-specific node_modules (e.g., main/node_modules, examples/node_modules)
*/node_modules/

# Package manager files
package-lock.json
yarn.lock
pnpm-lock.yaml
bun.lockb
.pnpm-store/

# =============================================================================
# Databases
# =============================================================================
# SQLite databases
*.db
*.db-journal
*.db-shm
*.db-wal

# Specific databases (redundant with *.db but explicit for clarity)
db/
clipboard-history.db
notes.db
ai-chats.db

# =============================================================================
# Runtime & Cache
# =============================================================================
# SDK is managed by the app, always regenerated
sdk/

# Command shims are managed by the app, always regenerated
bin/

# Application logs
logs/

# Cache files (app icons, etc.)
cache/

# Frecency tracking (regenerated from usage)
frecency.json

# Server state
server.json

# Authentication tokens
agent-token

# =============================================================================
# Build & Tooling
# =============================================================================
# TypeScript build output
*.tsbuildinfo
dist/
build/
.turbo/

# IDE
.idea/
.vscode/
*.swp
*.swo
*~

# macOS
.DS_Store
._*

# =============================================================================
# Secrets & Environment
# =============================================================================
.env
.env.local
.env.*.local
*.pem
*.key

# =============================================================================
# Temporary files
# =============================================================================
*.tmp
*.temp
*.log
tmp/
temp/
"#;
    write_string_if_changed(
        &gitignore_path,
        gitignore_content,
        &mut warnings,
        ".gitignore",
    );

    // Dependency check: bun (no process spawn; just path checks)
    let bun_available = bun_is_discoverable();
    if !bun_available {
        warnings.push(
            "bun not found (PATH/common install locations). Config/scripts may not run until bun is installed.".to_string(),
        );
    }

    // Optional "getting started" content only on truly fresh installs
    if is_fresh_install {
        create_sample_files(&kit_dir, &mut warnings);
    }

    info!(
        kit_path = %kit_dir.display(),
        is_fresh_install,
        bun_available,
        warning_count = warnings.len(),
        "Kit setup complete"
    );

    SetupResult {
        is_fresh_install,
        kit_path: kit_dir,
        bun_available,
        warnings,
    }
}
fn ensure_dir(path: &Path, warnings: &mut Vec<String>) {
    if path.exists() {
        return;
    }
    if let Err(e) = fs::create_dir_all(path) {
        warnings.push(format!(
            "Failed to create directory {}: {}",
            path.display(),
            e
        ));
    } else {
        debug!(path = %path.display(), "Created directory");
    }
}

/// Migrate legacy plugin-scoped `extensions/` directories to `scriptlets/`.
fn migrate_plugin_extensions_to_scriptlets(kit_dir: &Path, warnings: &mut Vec<String>) {
    let plugins_root = kit_dir.join("plugins");
    let Ok(entries) = fs::read_dir(&plugins_root) else {
        return;
    };

    for entry in entries.flatten() {
        let plugin_root = entry.path();
        if !plugin_root.is_dir() {
            continue;
        }

        let legacy_dir = plugin_root.join("extensions");
        if !legacy_dir.exists() || !legacy_dir.is_dir() {
            continue;
        }

        let scriptlets_dir = plugin_root.join("scriptlets");
        if !scriptlets_dir.exists() {
            match fs::rename(&legacy_dir, &scriptlets_dir) {
                Ok(()) => {
                    info!(
                        plugin_root = %plugin_root.display(),
                        "Migrated legacy extensions directory to scriptlets"
                    );
                    continue;
                }
                Err(error) => {
                    warnings.push(format!(
                        "Failed to rename legacy extensions directory {} to {}: {}",
                        legacy_dir.display(),
                        scriptlets_dir.display(),
                        error
                    ));
                    continue;
                }
            }
        }

        let Ok(legacy_entries) = fs::read_dir(&legacy_dir) else {
            warnings.push(format!(
                "Failed to read legacy extensions directory {} during migration",
                legacy_dir.display()
            ));
            continue;
        };

        for legacy_entry in legacy_entries.flatten() {
            let legacy_path = legacy_entry.path();
            let Some(file_name) = legacy_path.file_name() else {
                continue;
            };
            let target_path = scriptlets_dir.join(file_name);

            if target_path.exists() {
                warnings.push(format!(
                    "Skipped legacy scriptlet {} because {} already exists",
                    legacy_path.display(),
                    target_path.display()
                ));
                continue;
            }

            if let Err(error) = fs::rename(&legacy_path, &target_path) {
                warnings.push(format!(
                    "Failed to migrate legacy scriptlet {} to {}: {}",
                    legacy_path.display(),
                    target_path.display(),
                    error
                ));
            }
        }

        if let Err(error) = fs::remove_dir(&legacy_dir) {
            debug!(
                error = %error,
                path = %legacy_dir.display(),
                "Legacy extensions directory not removed after migration"
            );
        }
    }
}
/// Remove only files this app previously managed in the examples plugin.
///
/// The examples plugin may contain user experiments, so do not delete whole
/// artifact directories just because the managed starter pack changed.
fn prune_managed_examples_plugin(root: &Path, warnings: &mut Vec<String>) {
    const STALE_MANAGED_EXAMPLE_FILES: &[&str] = &[
        "agents/plan-feature.i.agy.md",
        "agents/review-pr.claude.md",
        "scripts/choose-from-list.ts",
        "scripts/clipboard-transform.ts",
        "scripts/generic-oauth-device-flow.ts",
        "scripts/github-device-login.ts",
        "scripts/google-calendar-device-login.ts",
        "scripts/hello-world.ts",
        "scripts/lib/oauth-device-flow.ts",
        "scripts/microsoft-graph-device-login.ts",
        "scripts/path-picker.ts",
        "scripts/power-syntax-capture-expense-ledger.ts",
        "scripts/power-syntax-capture-github-local.ts",
        "scripts/power-syntax-capture-snippet-library.ts",
        "scripts/power-syntax-command-env-dump.ts",
        "scripts/power-syntax-duplicate-command.ts",
        "scripts/power-syntax-payload-lab.ts",
        "scripts/power-syntax-refine-fixture.ts",
        "scripts/todoist-demo.ts",
        "scriptlets/agent_chat-chat/main.md",
        "scriptlets/advanced.md",
        "scriptlets/custom-actions/main.actions.md",
        "scriptlets/custom-actions/main.md",
        "scriptlets/howto.md",
        "scriptlets/main.md",
        "scriptlets/notes/main.md",
        "scriptlets/power-syntax.md",
        "scriptlets/starter.md",
        "skills/explain-code/SKILL.md",
        "skills/plan-feature/SKILL.md",
        "skills/review-pr/SKILL.md",
    ];

    for rel in STALE_MANAGED_EXAMPLE_FILES {
        let path = root.join(rel);
        if !path.is_file() {
            continue;
        }
        if let Err(error) = fs::remove_file(&path) {
            warnings.push(format!(
                "Failed to prune stale managed example {}: {}",
                path.display(),
                error
            ));
        }
    }

    for rel in [
        "scripts/lib",
        "scriptlets/agent_chat-chat",
        "scriptlets/custom-actions",
        "scriptlets/notes",
        "skills/explain-code",
        "skills/plan-feature",
        "skills/review-pr",
        "agents",
        "scriptlets",
        "skills",
    ] {
        let path = root.join(rel);
        if path.is_dir() {
            let _ = fs::remove_dir(&path);
        }
    }

    ensure_dir(&root.join("scripts"), warnings);
}

/// Ensure a plugin root exists with its standard subdirectories and a `plugin.json` manifest.
fn ensure_plugin_root(
    kit_dir: &Path,
    plugin_id: &str,
    title: &str,
    description: &str,
    warnings: &mut Vec<String>,
) {
    let root = kit_dir.join("plugins").join(plugin_id);
    ensure_dir(&root.join("scripts"), warnings);
    ensure_dir(&root.join("scriptlets"), warnings);
    ensure_dir(&root.join("agents"), warnings);
    ensure_dir(&root.join("skills"), warnings);
    ensure_dir(&root.join("profiles"), warnings);

    let manifest = format!(
        "{{\n  \"id\": \"{plugin_id}\",\n  \"title\": \"{title}\",\n  \"description\": \"{description}\"\n}}"
    );
    write_string_if_missing(
        &root.join("plugin.json"),
        &manifest,
        warnings,
        &format!("plugins/{plugin_id}/plugin.json"),
    );

    info!(plugin_id = %plugin_id, "plugin_root_ensured");
}

fn write_string_if_missing(path: &Path, contents: &str, warnings: &mut Vec<String>, label: &str) {
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warnings.push(format!(
                "Failed to create parent dir for {} ({}): {}",
                label,
                parent.display(),
                e
            ));
            return;
        }
    }
    if let Err(e) = fs::write(path, contents) {
        warnings.push(format!(
            "Failed to write {} ({}): {}",
            label,
            path.display(),
            e
        ));
    } else {
        info!(path = %path.display(), "Created {}", label);
    }
}
// --- merged from part_004.rs ---
/// Write string to path if content changed, using atomic rename for safety
///
/// This function uses an atomic write pattern to prevent race conditions and
/// partial writes:
/// 1. Write to a temporary file in the same directory
/// 2. Atomically rename temp file to target path
///
/// The rename is atomic on most filesystems, so readers will either see the
/// old content or the new content, never a partial write.
fn write_string_if_changed(path: &Path, contents: &str, warnings: &mut Vec<String>, label: &str) {
    if let Ok(existing) = fs::read_to_string(path) {
        if existing == contents {
            return;
        }
    }

    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            warnings.push(format!(
                "Failed to create parent dir for {} ({}): {}",
                label,
                parent.display(),
                e
            ));
            return;
        }
    }

    // Atomic write: write to temp file then rename
    // This prevents readers from seeing partial writes during concurrent access
    let temp_path = path.with_extension("tmp");

    if let Err(e) = fs::write(&temp_path, contents) {
        warnings.push(format!(
            "Failed to write temp file for {} ({}): {}",
            label,
            temp_path.display(),
            e
        ));
        return;
    }

    // Atomic rename - this is atomic on most filesystems
    if let Err(e) = fs::rename(&temp_path, path) {
        warnings.push(format!(
            "Failed to rename {} to {}: {}",
            temp_path.display(),
            path.display(),
            e
        ));
        // Clean up temp file on failure
        let _ = fs::remove_file(&temp_path);
    } else {
        debug!(path = %path.display(), "Updated {}", label);
    }
}

fn write_executable_string_if_changed(
    path: &Path,
    contents: &str,
    warnings: &mut Vec<String>,
    label: &str,
) {
    write_string_if_changed(path, contents, warnings, label);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        match fs::metadata(path) {
            Ok(metadata) => {
                let mut permissions = metadata.permissions();
                if permissions.mode() & 0o755 != 0o755 {
                    permissions.set_mode(0o755);
                    if let Err(error) = fs::set_permissions(path, permissions) {
                        warnings.push(format!(
                            "Failed to mark {} executable ({}): {}",
                            label,
                            path.display(),
                            error
                        ));
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "Failed to stat {} for executable permissions ({}): {}",
                label,
                path.display(),
                error
            )),
        }
    }
}
/// Ensure tsconfig.json has proper TypeScript/Bun settings (merge-safe)
/// The tsconfig lives at ~/.scriptkit/plugins/tsconfig.json, SDK at ~/.scriptkit/sdk/
///
/// Sets essential options while preserving user customizations:
/// - target: ESNext (for top-level await and modern features)
/// - module: ESNext (ES modules)
/// - moduleResolution: Bundler (optimal for Bun)
/// - paths: @scriptkit/sdk mapping
/// - noEmit: true (Bun runs .ts directly)
/// - skipLibCheck: true (faster)
/// - esModuleInterop: true (CommonJS compat)
fn ensure_tsconfig_paths(tsconfig_path: &Path, warnings: &mut Vec<String>) {
    use serde_json::{json, Value};

    // Path is relative from the workspace root to sdk/.
    let expected_sdk_path = json!(["./sdk/kit-sdk.ts"]);

    let mut config: Value = if tsconfig_path.exists() {
        match fs::read_to_string(tsconfig_path) {
            Ok(content) => match serde_json::from_str(&content) {
                Ok(v) => v,
                Err(e) => {
                    let err_msg = format!(
                        "tsconfig.json exists at {} but is not valid JSON (parsing failed: {}). Preserving original file to prevent data loss.",
                        tsconfig_path.display(),
                        e
                    );
                    warnings.push(err_msg.clone());
                    warn!("{}", err_msg);
                    return;
                }
            },
            Err(e) => {
                let err_msg = format!(
                    "Failed to read existing tsconfig.json at {}: {}. Preserving original file.",
                    tsconfig_path.display(),
                    e
                );
                warnings.push(err_msg.clone());
                warn!("{}", err_msg);
                return;
            }
        }
    } else {
        json!({})
    };

    // Ensure compilerOptions exists
    if config.get("compilerOptions").is_none() {
        config["compilerOptions"] = json!({});
    }

    let Some(compiler_options) = config["compilerOptions"].as_object_mut() else {
        warnings.push("tsconfig.json compilerOptions is not an object".to_string());
        return;
    };
    let mut changed = false;

    // Essential settings for Bun/TypeScript scripts (set if missing)
    let defaults = [
        ("target", json!("ESNext")),
        ("module", json!("ESNext")),
        ("moduleResolution", json!("Bundler")),
        ("noEmit", json!(true)),
        ("skipLibCheck", json!(true)),
        ("esModuleInterop", json!(true)),
        ("allowImportingTsExtensions", json!(true)),
        ("verbatimModuleSyntax", json!(true)),
    ];

    for (key, value) in defaults {
        if !compiler_options.contains_key(key) {
            compiler_options.insert(key.to_string(), value);
            changed = true;
        }
    }

    // Ensure paths exists
    if !compiler_options.contains_key("paths") {
        compiler_options.insert("paths".to_string(), json!({}));
        changed = true;
    }

    // Always ensure @scriptkit/sdk path is correct
    let Some(paths) = compiler_options
        .get_mut("paths")
        .and_then(|v| v.as_object_mut())
    else {
        warnings.push("tsconfig.json paths is not an object".to_string());
        return;
    };
    if paths.get("@scriptkit/sdk") != Some(&expected_sdk_path) {
        paths.insert("@scriptkit/sdk".to_string(), expected_sdk_path);
        changed = true;
    }

    if !changed {
        return;
    }

    match serde_json::to_string_pretty(&config) {
        Ok(json_str) => {
            if let Err(e) = fs::write(tsconfig_path, json_str) {
                warnings.push(format!(
                    "Failed to write tsconfig.json ({}): {}",
                    tsconfig_path.display(),
                    e
                ));
                warn!(error = %e, "Failed to write tsconfig.json");
            } else {
                info!("Updated tsconfig.json with TypeScript/Bun settings");
            }
        }
        Err(e) => {
            warnings.push(format!("Failed to serialize tsconfig.json: {}", e));
            warn!(error = %e, "Failed to serialize tsconfig.json");
        }
    }
}
/// Fast check: looks for bun in common locations and PATH without spawning a process.
fn bun_is_discoverable() -> bool {
    let mut candidates: Vec<PathBuf> = Vec::new();

    // Common install locations
    if let Some(home) = dirs::home_dir() {
        candidates.push(home.join(".bun").join("bin").join(bun_exe_name()));
    }
    candidates.push(PathBuf::from("/opt/homebrew/bin").join(bun_exe_name()));
    candidates.push(PathBuf::from("/usr/local/bin").join(bun_exe_name()));
    candidates.push(PathBuf::from("/usr/bin").join(bun_exe_name()));

    // PATH scan
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            candidates.push(dir.join(bun_exe_name()));
        }
    }

    candidates.into_iter().any(|p| p.exists())
}
fn bun_exe_name() -> &'static str {
    #[cfg(windows)]
    {
        "bun.exe"
    }
    #[cfg(not(windows))]
    {
        "bun"
    }
}
// --- merged from part_005.rs ---
fn create_sample_files(kit_dir: &Path, warnings: &mut Vec<String>) {
    // Create sample files in the main plugin.
    let main_scripts_dir = kit_dir.join("plugins").join("main").join("scripts");
    let main_scriptlets_dir = kit_dir.join("plugins").join("main").join("scriptlets");
    let main_agents_dir = kit_dir.join("plugins").join("main").join("agents");

    let canonical_menu_syntax_scripts = [
        (
            "capture-todo-inbox.ts",
            EMBEDDED_CANONICAL_CAPTURE_TODO_INBOX,
        ),
        (
            "create-calendar-event.ts",
            EMBEDDED_CANONICAL_CREATE_CALENDAR_EVENT,
        ),
        (
            "create-mac-calendar-event.ts",
            EMBEDDED_CANONICAL_CREATE_MAC_CALENDAR_EVENT,
        ),
        (
            "add-google-calendar-event.ts",
            EMBEDDED_CANONICAL_ADD_GOOGLE_CALENDAR_EVENT,
        ),
        ("create-reminder.ts", EMBEDDED_CANONICAL_CREATE_REMINDER),
        ("snooze-task.ts", EMBEDDED_CANONICAL_SNOOZE_TASK),
        ("defer-task.ts", EMBEDDED_CANONICAL_DEFER_TASK),
        ("append-daily-note.ts", EMBEDDED_CANONICAL_APPEND_DAILY_NOTE),
        ("draft-social-post.ts", EMBEDDED_CANONICAL_DRAFT_SOCIAL_POST),
        ("save-tagged-link.ts", EMBEDDED_CANONICAL_SAVE_TAGGED_LINK),
    ];
    for (filename, contents) in canonical_menu_syntax_scripts {
        write_string_if_missing(
            &main_scripts_dir.join(filename),
            contents,
            warnings,
            &format!("plugins/main/scripts/{filename}"),
        );
    }

    // Create hello-world.ts script
    let hello_script_path = main_scripts_dir.join("hello-world.ts");
    if !hello_script_path.exists() {
        let hello_script = r#"/*
# Hello World

A simple greeting script demonstrating Script Kit basics.

## Features shown:
- `arg()` - Prompt for user input with choices
- `div()` - Display HTML content with Tailwind CSS
*/

export const metadata = {
  name: "Hello World",
  description: "A simple greeting script",
  // shortcut: "cmd shift h",  // Uncomment to add a global hotkey
  sdkCapabilities: ["arg", "div"],
  executionTopology: "typescript-script",
};

// Prompt the user to select or type their name
const name = await arg("What's your name?", [
  "World",
  "Script Kit",
  "Friend",
]);

// Display a greeting using HTML with Tailwind CSS classes
await div(`
  <div class="flex flex-col items-center justify-center h-full p-8">
    <h1 class="text-4xl font-bold text-yellow-400 mb-4">
      Hello, ${name}! 👋
    </h1>
    <p class="text-gray-400 text-lg">
      Welcome to Script Kit
    </p>
    <div class="mt-6 text-sm text-gray-500">
      Press <kbd class="px-2 py-1 bg-gray-700 rounded">Escape</kbd> to close
    </div>
  </div>
`);
"#;
        if let Err(e) = fs::write(&hello_script_path, hello_script) {
            warnings.push(format!(
                "Failed to create sample script {}: {}",
                hello_script_path.display(),
                e
            ));
        } else {
            info!(path = %hello_script_path.display(), "Created sample script");
        }
    }

    // Create hello-world.md scriptlet bundle
    let hello_scriptlet_path = main_scriptlets_dir.join("hello-world.md");
    if !hello_scriptlet_path.exists() {
        let hello_scriptlet = r#"# Hello World Scriptlets

Quick shell commands you can run from Script Kit.
Each code block is a separate scriptlet that appears in the menu.

---

## Say Hello
<!-- 
name: Say Hello
description: Display a greeting notification
shortcut: ctrl h
-->

```bash
echo "Hello from Script Kit! 🎉"
```

---

## Current Date
<!-- 
name: Current Date
description: Copy today's date to clipboard
shortcut: ctrl d
-->

```bash
date +"%Y-%m-%d" | pbcopy
echo "Date copied: $(date +"%Y-%m-%d")"
```

---

## Open Downloads
<!-- 
name: Open Downloads
description: Open the Downloads folder in Finder
-->

```bash
open ~/Downloads
```

---

## Quick Note
<!-- 
name: Quick Note
description: Append a timestamped note to notes.txt
-->

```bash
echo "[$(date +"%Y-%m-%d %H:%M")] $1" >> ~/notes.txt
echo "Note saved!"
```

---

## System Info
<!-- 
name: System Info
description: Show basic system information
-->

```bash
echo "User: $(whoami)"
echo "Host: $(hostname)"
echo "OS: $(sw_vers -productName) $(sw_vers -productVersion)"
echo "Shell: $SHELL"
```
"#;
        if let Err(e) = fs::write(&hello_scriptlet_path, hello_scriptlet) {
            warnings.push(format!(
                "Failed to create sample scriptlet bundle {}: {}",
                hello_scriptlet_path.display(),
                e
            ));
        } else {
            info!(path = %hello_scriptlet_path.display(), "Created sample scriptlet bundle");
        }
    }

    // Create hello-world.claude.md agent
    let hello_agent_path = main_agents_dir.join("hello-world.claude.md");
    if !hello_agent_path.exists() {
        let hello_agent = r#"---
_sk_name: Hello World Assistant
_sk_description: A friendly assistant that helps with simple tasks
_sk_interactive: true
---

You are a friendly, helpful assistant. Keep responses concise and practical.

When the user asks for help:
1. Understand their request clearly
2. Provide a direct, actionable answer
3. Offer to help with follow-up questions

Be conversational but efficient. Focus on solving the user's immediate needs.
"#;
        if let Err(e) = fs::write(&hello_agent_path, hello_agent) {
            warnings.push(format!(
                "Failed to create sample agent {}: {}",
                hello_agent_path.display(),
                e
            ));
        } else {
            info!(path = %hello_agent_path.display(), "Created sample agent");
        }
    }

    // Create README.md at kit root
    let readme_path = kit_dir.join("README.md");
    if !readme_path.exists() {
        let readme = r##"# Script Kit

Welcome to Script Kit! This directory contains your scripts, configuration, and data.

## Directory Structure

```
~/.scriptkit/
├── kit/                    # All kits (version control friendly)
│   ├── main/               # Your default kit
│   │   ├── scripts/        # TypeScript/JavaScript scripts (.ts, .js)
│   │   ├── scriptlets/     # Markdown scriptlet bundles (.md)
│   │   └── agents/         # AI agent definitions (.md)
│   ├── package.json        # Node.js module config (enables top-level await)
│   └── tsconfig.json       # TypeScript path mappings
├── sdk/                    # Runtime SDK (managed by app)
├── db/                     # Databases (clipboard history, etc.)
├── logs/                   # Application logs
├── cache/                  # Cached data (app icons, etc.)
└── README.md               # This file
```

## File Watching

Script Kit watches these files and reloads automatically:

| File/Directory | What happens on change |
|----------------|------------------------|
| `config.ts` | Reloads configuration (hotkeys, settings) |
| `theme.json` | Applies new theme colors immediately |
| `plugins/main/scripts/*.ts` | Updates script list and metadata |
| `plugins/main/scriptlets/*.md` | Updates scriptlet list |

## Scripts

Scripts are TypeScript files in `plugins/main/scripts/`. They have full access to the Script Kit SDK.

### Example Script

```typescript
// plugins/main/scripts/my-script.ts

export const metadata = {
  name: "My Script",
  description: "Does something useful",
  shortcut: "cmd shift m",  // Optional global hotkey
};

// Prompt for input
const choice = await arg("Pick an option", ["Option 1", "Option 2"]);

// Show result
await div(`<div class="p-4">You chose: ${choice}</div>`);
```

### Script Metadata

Use the `metadata` export for type-safe configuration:

```typescript
export const metadata = {
  name: "Script Name",           // Display name in menu
  description: "What it does",   // Shown below the name
  shortcut: "cmd shift x",       // Global hotkey (optional)
  alias: "sn",                   // Quick search alias (optional)
};
```

## Scriptlets

Scriptlets are Markdown files containing quick shell commands. Each code block becomes a menu item.

### Example Scriptlet

```markdown
# My Scriptlets

## Open Project
<!-- shortcut: cmd shift p -->

\`\`\`bash
cd ~/projects/myapp && code .
\`\`\`

## Git Status
<!-- name: Check Git Status -->

\`\`\`bash
git status
\`\`\`
```

### Scriptlet Metadata

Add HTML comments before code blocks:

```markdown
<!-- 
name: Display Name
description: What this does
shortcut: cmd shift x
-->
```

## Configuration (config.ts)

Your `config.ts` controls Script Kit behavior:

```typescript
export default {
  // Global hotkey to open Script Kit
  hotkey: {
    key: "Semicolon",
    modifiers: ["meta"],  // cmd+;
  },
  
  // UI Settings
  editorFontSize: 16,
  terminalFontSize: 14,
  
  // Built-in features
  builtIns: {
    clipboardHistory: true,
    appLauncher: true,
  },
} satisfies Config;
```

## Theme (theme.json)

Customize colors in `theme.json`:

```json
{
  "colors": {
    "background": {
      "main": "#1E1E1E"
    },
    "text": {
      "primary": "#FFFFFF",
      "secondary": "#CCCCCC"
    },
    "accent": {
      "selected": "#FBBF24"
    }
  }
}
```

Colors can be specified as:
- Hex strings: `"#FBBF24"` or `"FBBF24"`
- RGB: `"rgb(251, 191, 36)"`
- RGBA: `"rgba(251, 191, 36, 1.0)"`

## Environment Variable

Set `SK_PATH` to use a different directory:

```bash
export SK_PATH=~/my-scripts
```

## Quick Tips

1. **Create a new script**: Add a `.ts` file to `plugins/main/scripts/`
2. **Add a hotkey**: Set `shortcut` in the metadata
3. **Test changes**: Scripts reload automatically on save
4. **View logs**: Check `logs/script-kit-gpui.jsonl` for debugging
5. **Complete guide**: See `GUIDE.md` for comprehensive tutorials and documentation

## Links

- Documentation: https://scriptkit.com/docs
- GitHub: https://github.com/johnlindquist/kit

---

Happy scripting! 🚀
"##;
        if let Err(e) = fs::write(&readme_path, readme) {
            warnings.push(format!(
                "Failed to create README {}: {}",
                readme_path.display(),
                e
            ));
        } else {
            info!(path = %readme_path.display(), "Created README.md");
        }
    }
}
// --- merged from part_006.rs ---
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Run a test body while holding the shared SK_PATH lock.
    /// Automatically sets SK_PATH to `kit_root` and removes it on exit.
    fn with_sk_path<F: FnOnce(&std::path::Path)>(f: F) {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());

        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().to_path_buf();
        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        f(&kit_root);

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    /// Test that plugin directories live under plugins/
    /// Expected structure: ~/.scriptkit/plugins/main/scripts, ~/.scriptkit/plugins/main/scriptlets
    #[test]
    fn test_plugin_directory_uses_plugins_subdirectory() {
        with_sk_path(|kit_root| {
            let result = ensure_kit_setup();

            let kit_main_scripts = kit_root.join("plugins").join("main").join("scripts");
            let kit_main_extensions = kit_root.join("plugins").join("main").join("scriptlets");

            assert!(
                kit_main_scripts.exists(),
                "Expected plugins/main/scripts to exist at {:?}",
                kit_main_scripts
            );
            assert!(
                kit_main_extensions.exists(),
                "Expected plugins/main/scriptlets to exist at {:?}",
                kit_main_extensions
            );

            let old_main_scripts = kit_root.join("main").join("scripts");
            assert!(
                !old_main_scripts.exists(),
                "Old structure main/scripts should NOT exist at {:?}",
                old_main_scripts
            );

            assert!(!result.warnings.iter().any(|w| w.contains("Failed")));
        });
    }

    /// Test that sample files are created in plugins/main/scripts
    #[test]
    fn test_sample_files_in_plugins_subdirectory() {
        with_sk_path(|kit_root| {
            let result = ensure_kit_setup();

            if result.is_fresh_install {
                let hello_script = kit_root
                    .join("plugins")
                    .join("main")
                    .join("scripts")
                    .join("hello-world.ts");
                assert!(
                    hello_script.exists(),
                    "Expected hello-world.ts at {:?}",
                    hello_script
                );
            }
        });
    }

    #[test]
    fn test_fresh_install_seeds_canonical_menu_syntax_handlers() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().join("scriptkit-fresh-menu-syntax");
        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        let result = ensure_kit_setup();
        assert!(result.is_fresh_install);

        let main_scripts = kit_root.join("plugins").join("main").join("scripts");
        for filename in [
            "capture-todo-inbox.ts",
            "create-calendar-event.ts",
            "create-mac-calendar-event.ts",
            "add-google-calendar-event.ts",
            "create-reminder.ts",
            "snooze-task.ts",
            "defer-task.ts",
            "append-daily-note.ts",
            "draft-social-post.ts",
            "save-tagged-link.ts",
        ] {
            let path = main_scripts.join(filename);
            assert!(path.exists(), "expected seeded handler at {:?}", path);
            let content = fs::read_to_string(&path).unwrap();
            assert!(
                content.contains("menuSyntax"),
                "{filename} should declare menuSyntax metadata"
            );
            assert!(
                content.contains(r#"family: "capture.v1""#),
                "{filename} should declare a capture.v1 handler"
            );
        }

        let hello_path = main_scripts.join("hello-world.ts");
        let hello_source = fs::read_to_string(&hello_path)
            .expect("the actual fresh-install Hello World script must be readable");
        let parsed = crate::metadata_parser::extract_typed_metadata(&hello_source);
        assert!(
            parsed.errors.is_empty(),
            "the seeded starter must have parseable host compatibility metadata: {:?}",
            parsed.errors
        );
        let metadata = parsed
            .metadata
            .expect("the seeded starter must expose typed metadata");
        assert_eq!(
            metadata.extra.get("sdkCapabilities"),
            Some(&serde_json::json!(["arg", "div"]))
        );
        assert_eq!(
            metadata.extra.get("executionTopology"),
            Some(&serde_json::json!("typescript-script"))
        );
        let hello_script = crate::scripts::Script {
            name: "Hello World".to_owned(),
            path: hello_path,
            extension: "ts".to_owned(),
            plugin_id: "main".to_owned(),
            typed_metadata: Some(metadata),
            ..crate::scripts::Script::default()
        };
        assert!(
            crate::scripts::validate_declared_sdk_capabilities(&hello_script).is_empty(),
            "the first-run starter must satisfy its actual interactive SDK contract"
        );

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_bun_is_discoverable() {
        // This test just verifies the function doesn't panic
        let _ = bun_is_discoverable();
    }

    #[test]
    fn test_bun_exe_name() {
        let name = bun_exe_name();
        #[cfg(windows)]
        assert_eq!(name, "bun.exe");
        #[cfg(not(windows))]
        assert_eq!(name, "bun");
    }

    #[test]
    fn test_get_kit_path_default() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::remove_var(SK_PATH_ENV);
        let path = get_kit_path();
        assert!(path.to_string_lossy().contains(".scriptkit"));
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_override() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::set_var(SK_PATH_ENV, "/custom/path");
        let path = get_kit_path();
        assert_eq!(path, PathBuf::from("/custom/path"));
        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_tilde() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        std::env::set_var(SK_PATH_ENV, "~/.config/kit");
        let path = get_kit_path();
        assert!(!path.to_string_lossy().contains("~"));
        assert!(path.to_string_lossy().contains(".config/kit"));
        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    #[test]
    fn test_get_kit_path_with_env_var_expansion() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let env_var = "SCRIPT_KIT_TEST_SK_PATH_ROOT";
        std::env::set_var(env_var, "/tmp/script-kit-env-root");
        std::env::set_var(SK_PATH_ENV, format!("${env_var}/kit"));

        let path = get_kit_path();
        assert_eq!(path, PathBuf::from("/tmp/script-kit-env-root/kit"));

        std::env::remove_var(SK_PATH_ENV);
        std::env::remove_var(env_var);
        drop(lock);
    }

    /// Comprehensive setup verification test
    /// Verifies the complete directory structure matches documentation:
    /// ```
    /// ~/.scriptkit/
    /// ├── kit/
    /// │   ├── main/
    /// │   │   ├── scripts/
    /// │   │   ├── scriptlets/
    /// │   │   └── agents/
    /// │   ├── config.ts
    /// │   ├── theme.json
    /// │   ├── package.json
    /// │   ├── tsconfig.json
    /// │   ├── AGENTS.md
    /// │   └── CLAUDE.md
    /// ├── sdk/
    /// │   └── kit-sdk.ts
    /// ├── db/
    /// ├── logs/
    /// ├── cache/
    /// └── GUIDE.md
    /// ```
    #[test]
    fn test_complete_setup_structure() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        // Use a subdirectory that definitely doesn't exist for fresh install detection
        let kit_root = temp_dir.path().join("scriptkit-test");

        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());

        let result = ensure_kit_setup();
        // Don't assert is_fresh_install - just verify the structure is correct
        assert!(
            result.warnings.is_empty() || !result.warnings.iter().any(|w| w.contains("Failed"))
        );

        // Verify plugins/ subdirectory structure
        let plugins_dir = kit_root.join("plugins");
        assert!(plugins_dir.exists(), "plugins/ directory should exist");

        // Verify main kit directories
        let main_dir = plugins_dir.join("main");
        assert!(
            main_dir.join("scripts").exists(),
            "plugins/main/scripts/ should exist"
        );
        assert!(
            main_dir.join("scriptlets").exists(),
            "plugins/main/scriptlets/ should exist"
        );
        assert!(
            main_dir.join("agents").exists(),
            "plugins/main/agents/ should exist"
        );

        // Verify user config files at workspace root
        assert!(
            kit_root.join("config.ts").exists(),
            "config.ts should exist"
        );
        assert!(
            kit_root.join("theme.json").exists(),
            "theme.json should exist"
        );
        assert!(
            kit_root.join("package.json").exists(),
            "package.json should exist"
        );
        assert!(
            kit_root.join("tsconfig.json").exists(),
            "tsconfig.json should exist"
        );
        assert!(
            kit_root.join("AGENTS.md").exists(),
            "AGENTS.md should exist"
        );
        assert!(
            kit_root.join("CLAUDE.md").exists(),
            "CLAUDE.md should exist"
        );

        // Verify SDK directory
        assert!(
            kit_root.join("sdk").join("kit-sdk.ts").exists(),
            "sdk/kit-sdk.ts should exist"
        );
        assert!(
            kit_root.join("bin").join("scriptkit").exists(),
            "bin/scriptkit should exist"
        );

        // Verify other directories
        assert!(kit_root.join("db").exists(), "db/ directory should exist");
        assert!(
            kit_root.join("logs").exists(),
            "logs/ directory should exist"
        );
        assert!(
            kit_root.join("cache").exists(),
            "cache/ directory should exist"
        );

        // Verify GUIDE.md at root
        assert!(
            kit_root.join("GUIDE.md").exists(),
            "GUIDE.md should exist at root"
        );

        // Verify sample script on fresh install
        let hello_script = main_dir.join("scripts").join("hello-world.ts");
        assert!(
            hello_script.exists(),
            "hello-world.ts sample script should exist"
        );

        // Verify config.ts content
        let config_content = fs::read_to_string(kit_root.join("config.ts")).unwrap();
        assert!(
            config_content.contains("@scriptkit/sdk"),
            "config.ts should import @scriptkit/sdk"
        );
        assert!(
            config_content.contains("hotkey"),
            "config.ts should have hotkey config"
        );

        // Verify package.json has correct name and type
        let package_content = fs::read_to_string(kit_root.join("package.json")).unwrap();
        assert!(
            package_content.contains("@scriptkit/kit"),
            "package.json should have @scriptkit/kit name"
        );
        assert!(
            package_content.contains("\"type\": \"module\""),
            "package.json should enable ESM"
        );

        // Root-level AGENTS.md has the actual SDK reference
        let agents_content = fs::read_to_string(kit_root.join("AGENTS.md")).unwrap();
        assert!(
            agents_content.contains("Script Kit"),
            "Root AGENTS.md should mention Script Kit"
        );
        assert!(
            agents_content.contains("~/.scriptkit/config.ts"),
            "Root AGENTS.md should have correct config path"
        );

        // Root-level CLAUDE.md has the actual agent instructions
        let claude_content = fs::read_to_string(kit_root.join("CLAUDE.md")).unwrap();
        assert!(
            claude_content.contains("Script Kit"),
            "Root CLAUDE.md should mention Script Kit"
        );
        assert!(
            claude_content.contains("@scriptkit/sdk"),
            "Root CLAUDE.md should reference the SDK"
        );

        // Verify CleanShot X built-in extension
        let cleanshot_dir = plugins_dir.join("cleanshot").join("scriptlets");
        assert!(
            cleanshot_dir.exists(),
            "plugins/cleanshot/scriptlets/ should exist"
        );
        let cleanshot_extension = cleanshot_dir.join("main.md");
        assert!(
            cleanshot_extension.exists(),
            "plugins/cleanshot/scriptlets/main.md should exist"
        );
        let cleanshot_content = fs::read_to_string(&cleanshot_extension).unwrap();
        assert!(
            cleanshot_content.contains("CleanShot X"),
            "CleanShot extension should have CleanShot X title"
        );
        assert!(
            cleanshot_content.contains("cleanshot://capture-area"),
            "CleanShot extension should have Capture Area command"
        );
        assert!(
            cleanshot_content.contains("cleanshot://record-screen"),
            "CleanShot extension should have Record Screen command"
        );

        // Verify 1Password built-in extension
        let onepassword_dir = plugins_dir.join("1password").join("scriptlets");
        assert!(
            onepassword_dir.exists(),
            "plugins/1password/scriptlets/ should exist"
        );
        let onepassword_extension = onepassword_dir.join("main.md");
        assert!(
            onepassword_extension.exists(),
            "plugins/1password/scriptlets/main.md should exist"
        );
        let onepassword_content = fs::read_to_string(&onepassword_extension).unwrap();
        assert!(
            onepassword_content.contains("1Password"),
            "1Password extension should have 1Password title"
        );
        assert!(
            onepassword_content.contains("op item list"),
            "1Password extension should have item list command"
        );
        assert!(
            onepassword_content.contains("op whoami"),
            "1Password extension should have whoami command"
        );

        // Verify Quick Links built-in extension
        let quicklinks_dir = plugins_dir.join("quicklinks").join("scriptlets");
        assert!(
            quicklinks_dir.exists(),
            "plugins/quicklinks/scriptlets/ should exist"
        );
        let quicklinks_extension = quicklinks_dir.join("main.md");
        assert!(
            quicklinks_extension.exists(),
            "plugins/quicklinks/scriptlets/main.md should exist"
        );
        let quicklinks_content = fs::read_to_string(&quicklinks_extension).unwrap();
        assert!(
            quicklinks_content.contains("Quick Links"),
            "Quick Links extension should have Quick Links title"
        );
        assert!(
            quicklinks_content.contains("https://github.com"),
            "Quick Links extension should have GitHub link"
        );
        assert!(
            quicklinks_content.contains("https://www.google.com"),
            "Quick Links extension should have Google link"
        );

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }

    /// Test that paths in AGENTS.md match actual setup paths
    #[test]
    fn test_agents_md_paths_match_setup() {
        let lock = crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let temp_dir = TempDir::new().unwrap();
        let kit_root = temp_dir.path().to_path_buf();

        std::env::set_var(SK_PATH_ENV, kit_root.to_str().unwrap());
        let _ = ensure_kit_setup();

        // Root-level AGENTS.md is the canonical location now
        let agents_content = fs::read_to_string(kit_root.join("AGENTS.md")).unwrap();

        // Verify documented paths actually exist
        let documented_paths = [
            ("plugins/main/scripts", "~/.scriptkit/plugins/main/scripts/"),
            (
                "plugins/main/scriptlets",
                "~/.scriptkit/plugins/main/scriptlets/",
            ),
            ("config.ts", "~/.scriptkit/config.ts"),
            ("theme.json", "~/.scriptkit/theme.json"),
            ("sdk/kit-sdk.ts", "~/.scriptkit/sdk/"),
            ("bin/scriptkit", "~/.scriptkit/bin/scriptkit"),
        ];

        for (relative_path, doc_path) in documented_paths {
            assert!(
                agents_content.contains(doc_path),
                "AGENTS.md should document path: {}",
                doc_path
            );

            let actual_path = kit_root.join(relative_path);
            // For directories, check they exist; for files, check the parent exists
            if relative_path.contains('.') {
                assert!(
                    actual_path.exists(),
                    "Documented path {} should exist as file: {:?}",
                    doc_path,
                    actual_path
                );
            } else {
                assert!(
                    actual_path.exists(),
                    "Documented path {} should exist as directory: {:?}",
                    doc_path,
                    actual_path
                );
            }
        }

        std::env::remove_var(SK_PATH_ENV);
        drop(lock);
    }
}

#[cfg(test)]
mod tab_ai_agent_doc_contract_tests {
    const ROOT_CLAUDE: &str = include_str!("../../kit-init/ROOT_CLAUDE.md");
    const ROOT_AGENTS: &str = include_str!("../../kit-init/ROOT_AGENTS.md");
    const AI_MOD_SOURCE: &str = include_str!("../ai/mod.rs");
    const TAB_CONTEXT_SOURCE: &str = include_str!("../ai/tab_context.rs");

    fn assert_tab_ai_doc_contract(source: &str, label: &str) {
        for needle in [
            "Quick Terminal with Flat Context Injection",
            "AppView::QuickTerminalView",
            "TermPrompt",
            "TabAiHarnessSubmissionMode",
            "PasteOnly",
            "Submit",
            "claudeCode",
            "CaptureContextOptions::tab_ai_submit()",
            "Cmd+W",
            "Escape",
            "Agent Chat",
            "open_tab_ai_agent_chat_with_entry_intent",
        ] {
            assert!(source.contains(needle), "{label} must contain `{needle}`");
        }

        assert!(
            source.contains("PTY") || source.contains("pty"),
            "{label} must describe the landed PTY-backed path"
        );

        // The universal AI entry migrated from plain Tab to Cmd+Enter
        // (commit b6c5752bb "Deprecate Tab ACP agent chat surface").
        assert!(
            source.contains(
                "Command+Enter in `AppView::ScriptList` routes through the Agent Chat entry path"
            ),
            "{label} must describe Cmd+Enter as the Agent Chat entry path"
        );

        assert!(
            !source.contains(
                "Plain `Tab` in `AppView::ScriptList` routes through the Agent Chat entry path"
            ),
            "{label} must not describe plain Tab as the Agent Chat entry path"
        );
        assert!(
            !source.contains("Plain `Tab` opens the harness terminal"),
            "{label} must not describe plain Tab as opening the harness terminal"
        );
        assert!(
            !source.contains("`Shift+Tab` in `AppView::ScriptList` with non-empty filter text"),
            "{label} must not describe Shift+Tab in ScriptList as the default quick-submit path"
        );
    }

    fn assert_tab_ai_schema_detail_contract(source: &str, label: &str) {
        for needle in [
            "pub const TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION: u32 = 2;",
            "pub struct TabAiExecutionRecord",
            "PanelOnlyElements",
            "CollectorFallback",
            "NoSemanticElements",
            "MissingFocusTarget",
            "InputNotExtractable",
            "InputNotApplicable",
        ] {
            assert!(source.contains(needle), "{label} must contain `{needle}`");
        }

        for stale in [
            "TAB_AI_EXECUTION_RECORD_SCHEMA_VERSION: u32 = 1",
            "record + status + output + duration",
            "Persisted memory: intent, script, target bundle_id, outcome",
            "PanelOnlyWarning",
            "MissingInput",
        ] {
            assert!(
                !source.contains(stale),
                "{label} contains stale Tab AI schema detail: {stale}"
            );
        }
    }

    #[test]
    fn root_claude_doc_matches_landed_tab_ai_contract() {
        assert_tab_ai_doc_contract(ROOT_CLAUDE, "kit-init/ROOT_CLAUDE.md");
    }

    #[test]
    fn root_agents_doc_matches_landed_tab_ai_contract() {
        assert_tab_ai_doc_contract(ROOT_AGENTS, "kit-init/ROOT_AGENTS.md");
    }

    #[test]
    fn tab_ai_schema_detail_matches_current_tab_ai_types() {
        assert_tab_ai_schema_detail_contract(TAB_CONTEXT_SOURCE, "src/ai/tab_context.rs");
    }

    #[test]
    fn ai_mod_docs_reflect_agent_chat_primary_path() {
        for needle in [
            "//! AI surfaces and shared contracts.",
            "//! - User-facing AI chat surface: Agent Chat",
            "//! - Entry points should route to `open_tab_ai_agent_chat_with_entry_intent(...)` when they need the canonical chat UI",
        ] {
            assert!(
                AI_MOD_SOURCE.contains(needle),
                "src/ai/mod.rs docs must contain `{needle}`"
            );
        }

        assert!(
            !AI_MOD_SOURCE.contains("//! - Submission shape: flat text-native"),
            "src/ai/mod.rs must not describe QuickTerminalView as the primary AI surface"
        );
    }
}

#[cfg(test)]
mod asset_destination_tests {
    use std::path::PathBuf;

    /// Resolve the relative destination path for an embedded kit-init asset.
    ///
    /// Skills map to the workspace root (`skills/…`), the config template
    /// maps to `config.ts`, and everything else passes through unchanged.
    fn embedded_asset_destination_relative(asset: &str) -> PathBuf {
        if let Some(_rest) = asset.strip_prefix("skills/") {
            // skills/ already carries the correct relative prefix
            return PathBuf::from(asset);
        }
        match asset {
            "config-template.ts" => PathBuf::from("config.ts"),
            other => PathBuf::from(other),
        }
    }

    #[test]
    fn skills_install_to_workspace_root_skills_directory() {
        assert_eq!(
            embedded_asset_destination_relative("skills/update-config/SKILL.md"),
            PathBuf::from("skills/update-config/SKILL.md")
        );
    }

    #[test]
    fn skills_readme_installs_to_workspace_root() {
        assert_eq!(
            embedded_asset_destination_relative("skills/README.md"),
            PathBuf::from("skills/README.md")
        );
    }

    #[test]
    fn config_template_installs_under_kit_directory() {
        assert_eq!(
            embedded_asset_destination_relative("config-template.ts"),
            PathBuf::from("config.ts")
        );
    }

    #[test]
    fn passthrough_asset_unchanged() {
        assert_eq!(
            embedded_asset_destination_relative("GUIDE.md"),
            PathBuf::from("GUIDE.md")
        );
    }

    /// Verify that `ensure_kit_setup` writes bundled skills under the Script Kit plugin.
    #[test]
    fn setup_creates_skills_under_scriptkit_plugin() {
        let source = include_str!("mod.rs");
        assert!(
            source.contains(r#".join("scriptkit").join("skills")"#),
            "ensure_kit_setup must create bundled skills under plugins/scriptkit/skills"
        );
        assert!(
            source.contains(r#".join("skills").join("update-config")"#),
            "ensure_kit_setup must create the update-config skill under the Script Kit plugin"
        );
        // Verify skills are NOT nested under kit/ in the non-test portion of the file.
        // We search only the first 900 lines (the setup logic) to avoid matching test code.
        let setup_portion: String = source.lines().take(900).collect::<Vec<_>>().join("\n");
        let bad_pattern = [".join(\"kit\")", ".join(\"skills\")"].concat();
        assert!(
            !setup_portion.contains(&bad_pattern),
            "skills must not be nested under kit/ — they belong at the workspace root"
        );
    }
}
