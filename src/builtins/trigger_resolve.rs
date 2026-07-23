//! Oracle-Session `protocol-builtin-boundary-refactor-plan` PR3:
//! pure resolver for `triggerBuiltin` stdin bodies.
//!
//! The stdin dispatcher previously interleaved name-normalization,
//! alias lookup, `builtinId` lookup, logging, and dispatch in one
//! function. That made it impossible to test the routing decisions
//! without spinning a whole app. This module extracts the pure
//! `JSON body -> TriggerBuiltinResolution` function. It takes a
//! `&serde_json::Value`, reads the two recognized keys (`name`,
//! `builtinId`), and returns a structured outcome.
//!
//! No logging, no side effects, no global state. The dispatcher
//! layer turns the outcome into user-visible feedback; golden-file
//! tests at `tests/trigger_builtin_resolve_golden.rs` pin the
//! resolver's decisions against a small JSONL transcript so the
//! routing table can be audited without a real window.

use serde_json::Value;

use crate::builtins::trigger_registry::{registry, TriggerBuiltin};

/// Which recognized resolution path produced the hit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedVia {
    /// Matched a `builtin/...` canonical command id through the
    /// `builtinId` JSON key.
    BuiltinIdField,
    /// The `name` key happened to be a canonical `builtin/...` id.
    NameAsCommandId,
    /// The `name` key matched one of the registry's legacy aliases.
    NameAlias,
    /// Both `name` and `builtinId` were supplied and resolved to the
    /// same variant.
    BothAgree,
}

/// What was supplied on the wire, summarized for error reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuppliedKeys {
    Neither,
    NameOnly { name: String },
    BuiltinIdOnly { builtin_id: String },
    Both { name: String, builtin_id: String },
}

/// Outcome of resolving a `triggerBuiltin` JSON body. Distinguishing
/// every arm lets dispatch callers return a tailored error (e.g.
/// "you supplied name but not builtinId, and the name didn't match
/// anything") instead of a generic "unknown".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TriggerBuiltinResolution {
    /// Body had neither `name` nor `builtinId` (or both were blank).
    MissingKey,
    /// One or both keys were supplied but nothing resolved.
    Unknown { supplied: SuppliedKeys },
    /// Both keys resolved but they referred to different variants.
    Conflict {
        from_name: TriggerBuiltin,
        from_builtin_id: TriggerBuiltin,
    },
    /// Clean hit.
    Resolved {
        id: TriggerBuiltin,
        via: ResolvedVia,
    },
}

/// Extract an optional string-typed field, trimming whitespace and
/// treating the empty string as "missing" to match the runtime
/// dispatcher. Non-string values (numbers, objects, null) are also
/// treated as missing so malformed payloads fall into the
/// `MissingKey` / `Unknown` arms rather than panicking.
fn read_trimmed_string(body: &Value, key: &str) -> Option<String> {
    let s = body.as_object()?.get(key)?.as_str()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Resolve one `triggerBuiltin` body. Pure — safe to call from
/// tests, from the dispatcher, and from the golden-file harness.
pub fn resolve_trigger_builtin(body: &Value) -> TriggerBuiltinResolution {
    let name = read_trimmed_string(body, "name");
    let builtin_id = read_trimmed_string(body, "builtinId");
    let reg = registry();

    match (name, builtin_id) {
        (None, None) => TriggerBuiltinResolution::MissingKey,

        (None, Some(id)) => match reg.lookup_command_id(&id) {
            Some(hit) => TriggerBuiltinResolution::Resolved {
                id: hit,
                via: ResolvedVia::BuiltinIdField,
            },
            None => TriggerBuiltinResolution::Unknown {
                supplied: SuppliedKeys::BuiltinIdOnly { builtin_id: id },
            },
        },

        (Some(name), None) => {
            // Try canonical id first, then legacy alias. Mirrors the
            // ordering in `TriggerBuiltinRegistry::resolve`.
            if let Some(hit) = reg.lookup_command_id(&name) {
                TriggerBuiltinResolution::Resolved {
                    id: hit,
                    via: ResolvedVia::NameAsCommandId,
                }
            } else if let Some(hit) = reg.lookup_legacy_alias(&name) {
                TriggerBuiltinResolution::Resolved {
                    id: hit,
                    via: ResolvedVia::NameAlias,
                }
            } else {
                TriggerBuiltinResolution::Unknown {
                    supplied: SuppliedKeys::NameOnly { name },
                }
            }
        }

        (Some(name), Some(id)) => {
            let from_name = reg
                .lookup_command_id(&name)
                .or_else(|| reg.lookup_legacy_alias(&name));
            let from_builtin_id = reg.lookup_command_id(&id);
            match (from_name, from_builtin_id) {
                (Some(a), Some(b)) if a == b => TriggerBuiltinResolution::Resolved {
                    id: a,
                    via: ResolvedVia::BothAgree,
                },
                (Some(a), Some(b)) => TriggerBuiltinResolution::Conflict {
                    from_name: a,
                    from_builtin_id: b,
                },
                _ => TriggerBuiltinResolution::Unknown {
                    supplied: SuppliedKeys::Both {
                        name,
                        builtin_id: id,
                    },
                },
            }
        }
    }
}

/// Render an outcome as a stable, line-oriented string for
/// golden-file comparison. Matches one-for-one with the enum shape
/// — do not add fields without updating the golden fixture at
/// `tests/golden/trigger_builtin/basic.jsonl`.
pub fn render_resolution(outcome: &TriggerBuiltinResolution) -> String {
    match outcome {
        TriggerBuiltinResolution::MissingKey => "MissingKey".to_string(),
        TriggerBuiltinResolution::Unknown { supplied } => match supplied {
            SuppliedKeys::Neither => "Unknown::Neither".to_string(),
            SuppliedKeys::NameOnly { name } => format!("Unknown::NameOnly::{name}"),
            SuppliedKeys::BuiltinIdOnly { builtin_id } => {
                format!("Unknown::BuiltinIdOnly::{builtin_id}")
            }
            SuppliedKeys::Both { name, builtin_id } => {
                format!("Unknown::Both::{name}::{builtin_id}")
            }
        },
        TriggerBuiltinResolution::Conflict {
            from_name,
            from_builtin_id,
        } => format!(
            "Conflict::{}::{}",
            render_variant(*from_name),
            render_variant(*from_builtin_id)
        ),
        TriggerBuiltinResolution::Resolved { id, via } => {
            format!("Resolved::{}::{}", render_variant(*id), render_via(*via))
        }
    }
}

fn render_variant(id: TriggerBuiltin) -> &'static str {
    // Hand-rolled to avoid relying on `Debug` output formatting.
    match id {
        TriggerBuiltin::ClipboardHistory => "ClipboardHistory",
        TriggerBuiltin::AppLauncher => "AppLauncher",
        TriggerBuiltin::FileSearch => "FileSearch",
        TriggerBuiltin::BrowserTabs => "BrowserTabs",
        TriggerBuiltin::EmojiPicker => "EmojiPicker",
        TriggerBuiltin::WindowSwitcher => "WindowSwitcher",
        TriggerBuiltin::TabAi => "TabAi",
        TriggerBuiltin::ProcessManager => "ProcessManager",
        TriggerBuiltin::CurrentAppCommands => "CurrentAppCommands",
        TriggerBuiltin::NewScript => "NewScript",
        TriggerBuiltin::SdkReference => "SdkReference",
        TriggerBuiltin::AiVault => "AiVault",
        TriggerBuiltin::Settings => "Settings",
        TriggerBuiltin::BrowseKitStore => "BrowseKitStore",
        TriggerBuiltin::ManageInstalledKits => "ManageInstalledKits",
        TriggerBuiltin::Favorites => "Favorites",
        TriggerBuiltin::SearchAiPresets => "SearchAiPresets",
        TriggerBuiltin::ScriptTemplateCatalog => "ScriptTemplateCatalog",
        TriggerBuiltin::Dictation => "Dictation",
        TriggerBuiltin::DictationHistory => "DictationHistory",
        TriggerBuiltin::ChooseTheme => "ChooseTheme",
        TriggerBuiltin::ScriptKitSelfie => "ScriptKitSelfie",
        TriggerBuiltin::MainWindow => "MainWindow",
        TriggerBuiltin::QuickTerminal => "QuickTerminal",
        TriggerBuiltin::Webcam => "Webcam",
    }
}

fn render_via(via: ResolvedVia) -> &'static str {
    match via {
        ResolvedVia::BuiltinIdField => "BuiltinIdField",
        ResolvedVia::NameAsCommandId => "NameAsCommandId",
        ResolvedVia::NameAlias => "NameAlias",
        ResolvedVia::BothAgree => "BothAgree",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn missing_both_keys() {
        let body = json!({});
        assert!(matches!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::MissingKey
        ));
    }

    #[test]
    fn blank_string_counts_as_missing() {
        let body = json!({ "name": "   ", "builtinId": "" });
        assert!(matches!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::MissingKey
        ));
    }

    #[test]
    fn non_string_value_counts_as_missing() {
        let body = json!({ "name": 42, "builtinId": null });
        assert!(matches!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::MissingKey
        ));
    }

    #[test]
    fn resolves_legacy_alias() {
        let body = json!({ "name": "clipboard" });
        assert_eq!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::Resolved {
                id: TriggerBuiltin::ClipboardHistory,
                via: ResolvedVia::NameAlias
            }
        );
    }

    #[test]
    fn resolves_name_as_command_id() {
        let body = json!({ "name": "builtin/clipboard-history" });
        assert_eq!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::Resolved {
                id: TriggerBuiltin::ClipboardHistory,
                via: ResolvedVia::NameAsCommandId
            }
        );
    }

    #[test]
    fn resolves_builtin_id_field() {
        let body = json!({ "builtinId": "builtin/clipboard-history" });
        assert_eq!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::Resolved {
                id: TriggerBuiltin::ClipboardHistory,
                via: ResolvedVia::BuiltinIdField
            }
        );
    }

    #[test]
    fn both_agreeing_uses_both_agree() {
        let body = json!({
            "name": "clipboard",
            "builtinId": "builtin/clipboard-history",
        });
        assert_eq!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::Resolved {
                id: TriggerBuiltin::ClipboardHistory,
                via: ResolvedVia::BothAgree
            }
        );
    }

    #[test]
    fn both_disagreeing_is_conflict() {
        let body = json!({
            "name": "clipboard",
            "builtinId": "builtin/emoji-picker",
        });
        assert_eq!(
            resolve_trigger_builtin(&body),
            TriggerBuiltinResolution::Conflict {
                from_name: TriggerBuiltin::ClipboardHistory,
                from_builtin_id: TriggerBuiltin::EmojiPicker,
            }
        );
    }

    #[test]
    fn unknown_name_reports_supplied() {
        let body = json!({ "name": "totally-fake" });
        match resolve_trigger_builtin(&body) {
            TriggerBuiltinResolution::Unknown {
                supplied: SuppliedKeys::NameOnly { name },
            } => {
                assert_eq!(name, "totally-fake");
            }
            other => panic!("expected Unknown::NameOnly, got {other:?}"),
        }
    }

    #[test]
    fn unknown_builtin_id_reports_supplied() {
        let body = json!({ "builtinId": "builtin/not-a-thing" });
        match resolve_trigger_builtin(&body) {
            TriggerBuiltinResolution::Unknown {
                supplied: SuppliedKeys::BuiltinIdOnly { builtin_id },
            } => {
                assert_eq!(builtin_id, "builtin/not-a-thing");
            }
            other => panic!("expected Unknown::BuiltinIdOnly, got {other:?}"),
        }
    }

    #[test]
    fn both_unresolvable_reports_both() {
        let body = json!({ "name": "bad", "builtinId": "builtin/also-bad" });
        match resolve_trigger_builtin(&body) {
            TriggerBuiltinResolution::Unknown {
                supplied: SuppliedKeys::Both { name, builtin_id },
            } => {
                assert_eq!(name, "bad");
                assert_eq!(builtin_id, "builtin/also-bad");
            }
            other => panic!("expected Unknown::Both, got {other:?}"),
        }
    }

    #[test]
    fn render_is_stable_for_all_resolved_vias() {
        for via in [
            ResolvedVia::BuiltinIdField,
            ResolvedVia::NameAsCommandId,
            ResolvedVia::NameAlias,
            ResolvedVia::BothAgree,
        ] {
            let outcome = TriggerBuiltinResolution::Resolved {
                id: TriggerBuiltin::ClipboardHistory,
                via,
            };
            let s = render_resolution(&outcome);
            assert!(s.starts_with("Resolved::ClipboardHistory::"));
        }
    }
}
