//! Canonical trigger-builtin registry.
//!
//! Single source of truth for the aliases accepted by the stdin
//! `triggerBuiltin` verb. Every entry lists:
//!
//! * one canonical built-in command id (`builtin/clipboard-history`, ...)
//! * zero or more legacy aliases (`clipboard`, `clipboard-history`, ...)
//!
//! The registry is validated at startup via [`validate_trigger_registry`].
//! Duplicate aliases or duplicate command ids cause a loud panic BEFORE the
//! window ever appears — the Run 7 Pass #8 log-spam bug survived precisely
//! because a silent runtime no-op was the only feedback path for a missing
//! or typoed name.
//!
//! The [`TriggerBuiltin`] enum is the compiler-enforced exhaustive handle
//! used by `dispatch_trigger_builtin` — adding a new canonical built-in
//! means adding a variant, which means adding a dispatch arm, which means
//! the stdin ingress and the internal dispatch path can never drift apart.

use std::collections::BTreeMap;
use std::sync::OnceLock;

/// Exhaustive handle for every statically-registered trigger-builtin.
///
/// Adding a variant forces all dispatch callers to grow a matching arm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TriggerBuiltin {
    ClipboardHistory,
    AppLauncher,
    FileSearch,
    BrowserTabs,
    EmojiPicker,
    WindowSwitcher,
    TabAi,
    ProcessManager,
    CurrentAppCommands,
    NewScript,
    SdkReference,
    AiVault,
    Settings,
    BrowseKitStore,
    ManageInstalledKits,
    Favorites,
    SearchAiPresets,
    ScriptTemplateCatalog,
    Dictation,
    DictationHistory,
    ChooseTheme,
    ScriptKitSelfie,
    MainWindow,
    QuickTerminal,
    Webcam,
}

impl TriggerBuiltin {
    /// All variants, in a stable order — used by the registry builder and
    /// by uniqueness tests.
    pub const ALL: &'static [TriggerBuiltin] = &[
        TriggerBuiltin::ClipboardHistory,
        TriggerBuiltin::AppLauncher,
        TriggerBuiltin::FileSearch,
        TriggerBuiltin::BrowserTabs,
        TriggerBuiltin::EmojiPicker,
        TriggerBuiltin::WindowSwitcher,
        TriggerBuiltin::TabAi,
        TriggerBuiltin::ProcessManager,
        TriggerBuiltin::CurrentAppCommands,
        TriggerBuiltin::NewScript,
        TriggerBuiltin::SdkReference,
        TriggerBuiltin::AiVault,
        TriggerBuiltin::Settings,
        TriggerBuiltin::BrowseKitStore,
        TriggerBuiltin::ManageInstalledKits,
        TriggerBuiltin::Favorites,
        TriggerBuiltin::SearchAiPresets,
        TriggerBuiltin::ScriptTemplateCatalog,
        TriggerBuiltin::Dictation,
        TriggerBuiltin::DictationHistory,
        TriggerBuiltin::ChooseTheme,
        TriggerBuiltin::ScriptKitSelfie,
        TriggerBuiltin::MainWindow,
        TriggerBuiltin::QuickTerminal,
        TriggerBuiltin::Webcam,
    ];

    /// Canonical command id — matches the `builtin/...` ids used elsewhere
    /// in [`crate::builtins`] and [`crate::config::canonical_builtin_command_id`].
    pub const fn command_id(self) -> &'static str {
        match self {
            TriggerBuiltin::ClipboardHistory => "builtin/clipboard-history",
            TriggerBuiltin::AppLauncher => "builtin/app-launcher",
            TriggerBuiltin::FileSearch => "builtin/file-search",
            TriggerBuiltin::BrowserTabs => "builtin/browser-tabs",
            TriggerBuiltin::EmojiPicker => "builtin/emoji-picker",
            TriggerBuiltin::WindowSwitcher => "builtin/window-switcher",
            TriggerBuiltin::TabAi => "builtin/ai-chat",
            TriggerBuiltin::ProcessManager => "builtin/process-manager",
            TriggerBuiltin::CurrentAppCommands => "builtin/current-app-commands",
            TriggerBuiltin::NewScript => "builtin/new-script",
            TriggerBuiltin::SdkReference => "builtin/sdk-reference",
            TriggerBuiltin::AiVault => "builtin/vault",
            TriggerBuiltin::Settings => "builtin/settings",
            TriggerBuiltin::BrowseKitStore => "builtin/browse-kit-store",
            TriggerBuiltin::ManageInstalledKits => "builtin/manage-installed-kits",
            TriggerBuiltin::Favorites => "builtin/favorites",
            TriggerBuiltin::SearchAiPresets => "builtin/search-ai-presets",
            TriggerBuiltin::ScriptTemplateCatalog => "builtin/new-script-from-template",
            TriggerBuiltin::Dictation => "builtin/dictation",
            TriggerBuiltin::DictationHistory => "builtin/dictation-history",
            TriggerBuiltin::ChooseTheme => "builtin/choose-theme",
            TriggerBuiltin::ScriptKitSelfie => "builtin/script-kit-selfie",
            TriggerBuiltin::MainWindow => "builtin/main-window",
            TriggerBuiltin::QuickTerminal => "builtin/quick-terminal",
            TriggerBuiltin::Webcam => "builtin/webcam",
        }
    }

    /// Does this trigger-builtin also appear as a `BuiltInFeature` launcher
    /// entry (either top-level via [`crate::builtins::get_builtin_entries`]
    /// or hidden via `hidden_builtin_entry`)?
    ///
    /// Most variants do — their canonical command id is what the launcher
    /// uses. A few are intentionally internal-only:
    ///
    /// * [`TriggerBuiltin::AppLauncher`] — there is no single "open the
    ///   app launcher" entry because launcher apps are each individual
    ///   entries.
    /// * [`TriggerBuiltin::CurrentAppCommands`] — collapsed into
    ///   `builtin/do-in-current-app`; the `triggerBuiltin` route still
    ///   exists for menu-bar callers but the launcher no longer exposes
    ///   it directly.
    ///
    /// `tests/source_audits/trigger_builtin_registry_consistency.rs`
    /// uses this flag to skip internal-only variants when cross-checking
    /// registry ↔ `BuiltInFeature` drift. Keeping the flag explicit
    /// means the "why isn't it registered?" question is answered in the
    /// enum itself, not buried in a test skip list.
    pub const fn requires_builtin_feature_entry(self) -> bool {
        match self {
            // Launcher apps are each their own entry; no top-level
            // `builtin/app-launcher` is registered.
            TriggerBuiltin::AppLauncher => false,
            // Collapsed into `builtin/do-in-current-app` — see
            // `current_app_commands_builtin_is_no_longer_registered`.
            TriggerBuiltin::CurrentAppCommands => false,
            // Kit Store launcher stubs are intentionally pruned, but
            // triggerBuiltin still needs deterministic proof entry points.
            TriggerBuiltin::BrowseKitStore
            | TriggerBuiltin::ManageInstalledKits
            | TriggerBuiltin::SearchAiPresets => false,
            _ => true,
        }
    }

    /// Legacy / lowercased aliases accepted by the stdin `triggerBuiltin`
    /// verb. Aliases MUST be lowercase and are matched case-insensitively
    /// against the incoming `name`.
    pub const fn legacy_aliases(self) -> &'static [&'static str] {
        match self {
            TriggerBuiltin::ClipboardHistory => {
                &["clipboard", "clipboard-history", "clipboardhistory"]
            }
            TriggerBuiltin::AppLauncher => &["apps", "app-launcher", "applauncher"],
            TriggerBuiltin::FileSearch => &["file-search", "filesearch", "files", "searchfiles"],
            TriggerBuiltin::BrowserTabs => &["browser-tabs", "browsertabs", "tabs"],
            TriggerBuiltin::EmojiPicker => &["emoji", "emoji-picker", "emojipicker"],
            TriggerBuiltin::WindowSwitcher => &["window-switcher", "windowswitcher", "windows"],
            TriggerBuiltin::TabAi => &["tab-ai", "tabai"],
            TriggerBuiltin::ProcessManager => &["process-manager", "processmanager", "processes"],
            TriggerBuiltin::CurrentAppCommands => &[
                "current-app-commands",
                "currentappcommands",
                "current-app",
                "app-commands",
                "menu-commands",
            ],
            TriggerBuiltin::NewScript => &["new-script", "newscript"],
            TriggerBuiltin::SdkReference => &["sdk-reference", "sdkreference", "sdk-docs"],
            TriggerBuiltin::AiVault => &["vault", "ai-vault", "aivault"],
            TriggerBuiltin::Settings => &["settings", "kit-settings", "script-kit-settings"],
            TriggerBuiltin::BrowseKitStore => {
                &["browse-kit-store", "kit-store", "kit-store-browse", "kits"]
            }
            TriggerBuiltin::ManageInstalledKits => &[
                "manage-installed-kits",
                "installed-kits",
                "kit-store-installed",
                "manage-kits",
            ],
            TriggerBuiltin::Favorites => &["favorites", "favorite", "starred"],
            TriggerBuiltin::SearchAiPresets => &[
                "search-ai-presets",
                "ai-presets",
                "presets",
                "agent-presets",
            ],
            TriggerBuiltin::ScriptTemplateCatalog => &[
                "new-script-from-template",
                "script-template",
                "script-templates",
                "templates",
                "starter-template",
            ],
            TriggerBuiltin::Dictation => &["dictate", "voice-dictation", "start-dictation"],
            TriggerBuiltin::DictationHistory => &[
                "dictation-history",
                "dictationhistory",
                "dictation",
                "transcripts",
            ],
            TriggerBuiltin::ChooseTheme => &["choose-theme", "theme", "theme-designer"],
            TriggerBuiltin::ScriptKitSelfie => &[
                "script-kit-selfie",
                "scriptkitselfie",
                "selfie",
                "screenshot-selfie",
            ],
            TriggerBuiltin::MainWindow => &[
                "main-window",
                "launcher",
                "mini-main-window",
                "mini-launcher",
                "mini",
            ],
            TriggerBuiltin::QuickTerminal => &["quick-terminal", "quickterminal"],
            TriggerBuiltin::Webcam => &["webcam", "camera"],
        }
    }
}

/// Frozen alias and command-id table built once at startup.
pub struct TriggerBuiltinRegistry {
    by_command_id: BTreeMap<&'static str, TriggerBuiltin>,
    by_legacy_alias: BTreeMap<&'static str, TriggerBuiltin>,
}

impl TriggerBuiltinRegistry {
    fn build() -> Result<Self, String> {
        let mut by_command_id: BTreeMap<&'static str, TriggerBuiltin> = BTreeMap::new();
        let mut by_legacy_alias: BTreeMap<&'static str, TriggerBuiltin> = BTreeMap::new();

        for &id in TriggerBuiltin::ALL {
            let command_id = id.command_id();
            if let Some(prev) = by_command_id.insert(command_id, id) {
                return Err(format!(
                    "duplicate triggerBuiltin command_id '{command_id}' between {prev:?} and {id:?}"
                ));
            }

            for &alias in id.legacy_aliases() {
                if alias != alias.to_ascii_lowercase().as_str() {
                    return Err(format!(
                        "triggerBuiltin alias '{alias}' for {id:?} must be lowercase"
                    ));
                }
                if let Some(prev) = by_legacy_alias.insert(alias, id) {
                    return Err(format!(
                        "duplicate triggerBuiltin alias '{alias}' between {prev:?} and {id:?}"
                    ));
                }
            }
        }

        Ok(Self {
            by_command_id,
            by_legacy_alias,
        })
    }

    /// Look up by canonical command id. Accepts ids with or without the
    /// `builtin/` prefix — the input is canonicalized before lookup.
    pub fn lookup_command_id(&self, id: &str) -> Option<TriggerBuiltin> {
        let canonical = crate::config::canonical_builtin_command_id(id);
        self.by_command_id.get(canonical.as_str()).copied()
    }

    /// Look up by legacy alias. Case-insensitive and whitespace-trimmed.
    pub fn lookup_legacy_alias(&self, name: &str) -> Option<TriggerBuiltin> {
        let normalized = name.trim().to_ascii_lowercase();
        self.by_legacy_alias.get(normalized.as_str()).copied()
    }

    /// Try every resolution path (canonical id first, then legacy alias).
    pub fn resolve(&self, name: &str) -> Option<TriggerBuiltin> {
        self.lookup_command_id(name)
            .or_else(|| self.lookup_legacy_alias(name))
    }

    #[allow(dead_code)]
    pub fn command_ids(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_command_id.keys().copied()
    }

    #[allow(dead_code)]
    pub fn legacy_aliases(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.by_legacy_alias.keys().copied()
    }
}

/// Every canonical triggerBuiltin command id, in `TriggerBuiltin::ALL`
/// order. Single source of truth for drift audits of the
/// `kit://trigger-builtins` MCP resource. The unit test
/// `trigger_builtin_command_ids_cover_every_variant` below pins this slice
/// to the exhaustive variant list.
pub const TRIGGER_BUILTIN_COMMAND_IDS: &[&str] = &[
    "builtin/clipboard-history",
    "builtin/app-launcher",
    "builtin/file-search",
    "builtin/browser-tabs",
    "builtin/emoji-picker",
    "builtin/window-switcher",
    "builtin/ai-chat",
    "builtin/process-manager",
    "builtin/current-app-commands",
    "builtin/new-script",
    "builtin/sdk-reference",
    "builtin/vault",
    "builtin/settings",
    "builtin/browse-kit-store",
    "builtin/manage-installed-kits",
    "builtin/favorites",
    "builtin/search-ai-presets",
    "builtin/new-script-from-template",
    "builtin/dictation",
    "builtin/dictation-history",
    "builtin/choose-theme",
    "builtin/script-kit-selfie",
    "builtin/main-window",
    "builtin/quick-terminal",
    "builtin/webcam",
];

/// Accessor used by `tests/mcp_resource_drift.rs` and the
/// `kit://trigger-builtins` MCP resource so the documentation and the
/// runtime registry share one source of truth.
pub fn all_trigger_builtin_command_ids() -> &'static [&'static str] {
    TRIGGER_BUILTIN_COMMAND_IDS
}

static TRIGGER_REGISTRY: OnceLock<TriggerBuiltinRegistry> = OnceLock::new();

/// Global registry accessor. Initializes on first call and panics with a
/// descriptive error if the static tables contain duplicate command ids or
/// aliases — this is exactly the `unknown-name` fail-loud-at-startup
/// behavior that was previously a silent runtime no-op.
pub fn registry() -> &'static TriggerBuiltinRegistry {
    TRIGGER_REGISTRY.get_or_init(|| {
        TriggerBuiltinRegistry::build()
            .unwrap_or_else(|e| panic!("invalid triggerBuiltin registry: {e}"))
    })
}

/// Force-validate the registry at a well-defined startup point. Returns
/// `Ok(())` on success, `Err(message)` if a duplicate is detected. Call
/// this from `app_run_setup.rs` BEFORE stdin intake begins so a bad
/// registration fails loudly at startup.
pub fn validate_trigger_registry() -> Result<(), String> {
    TriggerBuiltinRegistry::build().map(|reg| {
        // Warm the OnceLock with the validated instance when it's still empty.
        let _ = TRIGGER_REGISTRY.set(reg);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_builds_without_duplicates() {
        let reg = TriggerBuiltinRegistry::build().expect("registry must build");
        assert_eq!(reg.by_command_id.len(), TriggerBuiltin::ALL.len());
        assert!(reg.by_legacy_alias.len() >= TriggerBuiltin::ALL.len());
    }

    #[test]
    fn every_variant_round_trips_by_command_id() {
        let reg = TriggerBuiltinRegistry::build().unwrap();
        for &id in TriggerBuiltin::ALL {
            assert_eq!(reg.lookup_command_id(id.command_id()), Some(id));
        }
    }

    #[test]
    fn every_alias_resolves_case_insensitively() {
        let reg = TriggerBuiltinRegistry::build().unwrap();
        for &id in TriggerBuiltin::ALL {
            for &alias in id.legacy_aliases() {
                assert_eq!(reg.lookup_legacy_alias(alias), Some(id));
                assert_eq!(
                    reg.lookup_legacy_alias(&alias.to_ascii_uppercase()),
                    Some(id),
                    "alias '{alias}' must resolve case-insensitively"
                );
            }
        }
    }

    #[test]
    fn resolve_accepts_both_paths() {
        let reg = TriggerBuiltinRegistry::build().unwrap();
        assert_eq!(
            reg.resolve("builtin/clipboard-history"),
            Some(TriggerBuiltin::ClipboardHistory)
        );
        assert_eq!(
            reg.resolve("clipboard"),
            Some(TriggerBuiltin::ClipboardHistory)
        );
        assert_eq!(reg.resolve("totally-fake-builtin"), None);
    }

    #[test]
    fn trigger_builtin_command_ids_cover_every_variant() {
        assert_eq!(
            TRIGGER_BUILTIN_COMMAND_IDS.len(),
            TriggerBuiltin::ALL.len(),
            "TRIGGER_BUILTIN_COMMAND_IDS must list exactly one id per variant"
        );
        for (idx, id) in TriggerBuiltin::ALL.iter().enumerate() {
            assert_eq!(
                TRIGGER_BUILTIN_COMMAND_IDS[idx],
                id.command_id(),
                "TRIGGER_BUILTIN_COMMAND_IDS[{idx}] must equal {id:?}.command_id()"
            );
        }
        let accessor: Vec<&'static str> = all_trigger_builtin_command_ids().to_vec();
        let expected: Vec<&'static str> = TRIGGER_BUILTIN_COMMAND_IDS.to_vec();
        assert_eq!(accessor, expected);
    }

    #[test]
    fn all_variants_have_at_least_one_alias() {
        for &id in TriggerBuiltin::ALL {
            assert!(
                !id.legacy_aliases().is_empty(),
                "{id:?} must have at least one legacy alias for stdin callers"
            );
            assert!(
                id.command_id().starts_with("builtin/"),
                "{id:?} command_id must use the 'builtin/' prefix"
            );
        }
    }
}
