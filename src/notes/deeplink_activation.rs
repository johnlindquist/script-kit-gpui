use std::path::PathBuf;

use crate::config::{build_command_id, command_id_from_deeplink, CommandCategory};
use crate::spine::catalog_subsearch::ContextSubsearchSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationSurface {
    NotesWindow,
    DayPage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Activation {
    OpenExternalUrl {
        href: String,
    },
    OpenFile {
        path: PathBuf,
        raw_href: String,
    },
    OpenNote {
        note_id: crate::notes::NoteId,
    },
    ScopedSearch {
        source: ContextSubsearchSource,
        query: String,
    },
    KitResourcePreview {
        uri: String,
        allow_agent_chat_action: bool,
    },
    ConfirmBeforeRun {
        command_id: String,
        raw_href: String,
    },
    Error(ActivationError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActivationError {
    pub raw_href: String,
    pub reason: ActivationErrorReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ActivationErrorReason {
    EmptyHref,
    UnknownScheme {
        scheme: String,
    },
    UnknownSpinePrefix {
        prefix: String,
        supported: Vec<&'static str>,
    },
    EmptySpineValue {
        prefix: String,
    },
    MalformedUri {
        message: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KitResourcePreview {
    pub uri: String,
    pub title: String,
    pub mime_type: String,
    pub text: String,
    pub truncated: bool,
}

pub(crate) fn resolve_activation(href: &str, surface: ActivationSurface) -> Activation {
    let raw_href = href.trim();
    if raw_href.is_empty() {
        return error(raw_href, ActivationErrorReason::EmptyHref);
    }

    if raw_href.starts_with("http://") || raw_href.starts_with("https://") {
        return Activation::OpenExternalUrl {
            href: raw_href.to_string(),
        };
    }

    if raw_href.starts_with("file://") {
        let path = raw_href
            .strip_prefix("file://")
            .map(percent_decode_lossy)
            .filter(|path| !path.trim().is_empty());
        return match path {
            Some(path) => Activation::OpenFile {
                path: expand_tilde_path(&path),
                raw_href: raw_href.to_string(),
            },
            None => error(
                raw_href,
                ActivationErrorReason::MalformedUri {
                    message: "file:// link is missing a path".to_string(),
                },
            ),
        };
    }

    if is_probable_local_path(raw_href) {
        return Activation::OpenFile {
            path: expand_tilde_path(raw_href),
            raw_href: raw_href.to_string(),
        };
    }

    if let Some(path) = raw_href.strip_prefix("scriptkit://") {
        return resolve_scriptkit(path, raw_href);
    }

    if raw_href.starts_with("kit://") {
        return Activation::KitResourcePreview {
            uri: raw_href.to_string(),
            allow_agent_chat_action: matches!(surface, ActivationSurface::DayPage),
        };
    }

    let scheme = raw_href
        .split_once(':')
        .map(|(scheme, _)| scheme.to_string())
        .unwrap_or_else(|| "none".to_string());
    error(raw_href, ActivationErrorReason::UnknownScheme { scheme })
}

/// Honest verb for the editor hover hint chip: what a single click on the
/// hovered deeplink will actually do, per [`resolve_activation`]. `None` for
/// activations that only raise an error dialog — hovering a broken link must
/// not advertise an action.
pub(crate) fn hover_hint_verb(activation: &Activation) -> Option<&'static str> {
    match activation {
        Activation::KitResourcePreview { .. } => Some("Preview"),
        Activation::OpenNote { .. } => Some("Open note"),
        Activation::OpenFile { .. } => Some("Open file"),
        Activation::OpenExternalUrl { .. } => Some("Open in browser"),
        Activation::ScopedSearch { .. } => Some("Search"),
        Activation::ConfirmBeforeRun { .. } => Some("Run (asks first)"),
        Activation::Error(_) => None,
    }
}

/// Hover hint chip model: the honest verb plus the hovered href, or `None`
/// when no chip should render (nothing hovered, or the activation would only
/// error). Shared by the render path and the automation render receipt so
/// probes assert exactly what the chip gating logic decided.
pub(crate) fn hover_hint_model(
    hovered_href: Option<String>,
    surface: ActivationSurface,
) -> Option<(&'static str, String)> {
    let href = hovered_href?;
    let verb = hover_hint_verb(&resolve_activation(&href, surface))?;
    Some((verb, href))
}

pub(crate) fn read_cheap_kit_resource_preview(uri: &str) -> Result<KitResourcePreview, String> {
    if crate::runtime_policy::is_owned_evaluation() && !uri.starts_with("kit://notes") {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage)
            .map_err(|error| error.to_string())?;
    }
    if !is_cheap_text_kit_resource_uri(uri) {
        return Err(format!(
            "Resource preview supports kit://notes, kit://scripts, kit://clipboard-history, and kit://dictation-history in this slice: {uri}"
        ));
    }

    let scripts = if uri == "kit://scripts" {
        crate::scripts::read_scripts().map_err(|error| error.to_string())?
    } else {
        Vec::new()
    };
    let scriptlets = Vec::new();
    let resource = crate::mcp_resources::read_resource(uri, &scripts, &scriptlets, None)?;
    if !resource.mime_type.starts_with("text/") && resource.mime_type != "application/json" {
        return Err(format!(
            "Resource is not a cheap text preview: {} ({})",
            resource.uri, resource.mime_type
        ));
    }

    const MAX_PREVIEW_CHARS: usize = 120_000;
    let mut text: String = resource.text.chars().take(MAX_PREVIEW_CHARS).collect();
    let truncated = resource.text.chars().count() > MAX_PREVIEW_CHARS;
    if truncated {
        text.push_str("\n\n[… resource preview truncated …]");
    }

    Ok(KitResourcePreview {
        title: kit_resource_preview_title(&resource.uri),
        uri: resource.uri,
        mime_type: resource.mime_type,
        text,
        truncated,
    })
}

/// Parse the note id out of a `kit://notes/{id}` resource URI.
///
/// Shared by the Day Page and Notes window previews so both surfaces agree
/// on which previewed resources have an editable source note.
pub(crate) fn kit_note_source_id(uri: &str) -> Option<crate::notes::NoteId> {
    let rest = uri.strip_prefix("kit://notes/")?;
    let id = rest.split(['?', '#']).next().unwrap_or_default();
    crate::notes::NoteId::parse(id)
}

fn is_cheap_text_kit_resource_uri(uri: &str) -> bool {
    uri == "kit://notes"
        || uri.starts_with("kit://notes?")
        || uri.starts_with("kit://notes/")
        || uri == "kit://scripts"
        || uri == "kit://clipboard-history"
        || uri.starts_with("kit://clipboard-history?")
        || uri == "kit://dictation-history"
        || uri.starts_with("kit://dictation-history?")
}

fn kit_resource_preview_title(uri: &str) -> String {
    if uri.starts_with("kit://notes") {
        "Notes resource preview".to_string()
    } else if uri == "kit://scripts" {
        "Scripts resource preview".to_string()
    } else if uri.starts_with("kit://clipboard-history") {
        "Clipboard history resource preview".to_string()
    } else if uri.starts_with("kit://dictation-history") {
        "Dictation history resource preview".to_string()
    } else {
        "Script Kit resource preview".to_string()
    }
}

pub(crate) fn run_deeplink_confirm_options(
    command_id: &str,
    raw_href: &str,
) -> crate::confirm::ParentConfirmOptions {
    crate::confirm::ParentConfirmOptions {
        title: "Run Script Kit command?".into(),
        body: format!(
            "This note link wants to run `{}`.\n\n{}",
            command_id, raw_href
        )
        .into(),
        confirm_text: "Run".into(),
        cancel_text: "Cancel".into(),
        confirm_variant: gpui_component::button::ButtonVariant::Danger,
        width: gpui::px(crate::confirm::PARENT_MODAL_WIDTH_PX),
    }
}

fn resolve_scriptkit(path: &str, raw_href: &str) -> Activation {
    if let Some(note_id) = path.strip_prefix("notes/") {
        return match crate::notes::NoteId::parse(note_id) {
            Some(note_id) => Activation::OpenNote { note_id },
            None => error(
                raw_href,
                ActivationErrorReason::MalformedUri {
                    message: "notes deeplink must include a valid note id".to_string(),
                },
            ),
        };
    }

    if let Some(script_name) = path.strip_prefix("run/") {
        return match build_command_id(CommandCategory::Script, script_name) {
            Ok(command_id) => Activation::ConfirmBeforeRun {
                command_id,
                raw_href: raw_href.to_string(),
            },
            Err(err) => error(
                raw_href,
                ActivationErrorReason::MalformedUri {
                    message: err.to_string(),
                },
            ),
        };
    }

    if path.starts_with("commands/") {
        return match command_id_from_deeplink(raw_href) {
            Ok(command_id) => Activation::ConfirmBeforeRun {
                command_id,
                raw_href: raw_href.to_string(),
            },
            Err(err) => error(
                raw_href,
                ActivationErrorReason::MalformedUri {
                    message: err.to_string(),
                },
            ),
        };
    }

    if let Some(rest) = path.strip_prefix("spine/") {
        return resolve_spine(rest, raw_href);
    }

    error(
        raw_href,
        ActivationErrorReason::UnknownScheme {
            scheme: "scriptkit".to_string(),
        },
    )
}

fn resolve_spine(rest: &str, raw_href: &str) -> Activation {
    let (prefix, encoded_value) = match rest.split_once('/') {
        Some(parts) => parts,
        None => {
            return error(
                raw_href,
                ActivationErrorReason::MalformedUri {
                    message: "spine deeplink must include a context type and value".to_string(),
                },
            );
        }
    };

    let Some(source) = ContextSubsearchSource::from_trigger(prefix) else {
        return error(
            raw_href,
            ActivationErrorReason::UnknownSpinePrefix {
                prefix: prefix.to_string(),
                supported: supported_spine_prefixes(),
            },
        );
    };

    let query = percent_decode_lossy(encoded_value).trim().to_string();
    if query.is_empty() {
        return error(
            raw_href,
            ActivationErrorReason::EmptySpineValue {
                prefix: prefix.to_string(),
            },
        );
    }

    Activation::ScopedSearch { source, query }
}

fn error(raw_href: &str, reason: ActivationErrorReason) -> Activation {
    Activation::Error(ActivationError {
        raw_href: raw_href.to_string(),
        reason,
    })
}

fn supported_spine_prefixes() -> Vec<&'static str> {
    vec![
        "file",
        "files",
        "project",
        "projects",
        "notes",
        "scripts",
        "scriptlets",
        "skills",
        "browser-history",
        "clipboard",
        "history",
        "dictation",
        "calendar",
        "notifications",
    ]
}

fn is_probable_local_path(value: &str) -> bool {
    value.starts_with('/') || value.starts_with("~/")
}

fn expand_tilde_path(value: &str) -> PathBuf {
    if let Some(rest) = value.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

fn percent_decode_lossy(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hi = hex_value(bytes[index + 1]);
            let lo = hex_value(bytes[index + 2]);
            if let (Some(hi), Some(lo)) = (hi, lo) {
                output.push((hi << 4) | lo);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resolve(href: &str) -> Activation {
        resolve_activation(href, ActivationSurface::NotesWindow)
    }

    #[test]
    fn hover_hint_verb_matches_activation_semantics() {
        assert_eq!(hover_hint_verb(&resolve("kit://notes")), Some("Preview"));
        assert_eq!(
            hover_hint_verb(&resolve("https://example.com")),
            Some("Open in browser")
        );
        assert_eq!(
            hover_hint_verb(&resolve("file:///tmp/example.txt")),
            Some("Open file")
        );
        assert_eq!(
            hover_hint_verb(&resolve("scriptkit://run/example-script")),
            Some("Run (asks first)")
        );
    }

    #[test]
    fn hover_hint_verb_is_silent_for_error_activations() {
        assert_eq!(
            hover_hint_verb(&resolve("mailto:someone@example.com")),
            None
        );
        assert_eq!(hover_hint_verb(&resolve("")), None);
    }

    #[test]
    fn resolves_notes_deeplink_to_notes_window_activation() {
        let note_id = crate::notes::NoteId::new();
        assert_eq!(
            resolve(&format!("scriptkit://notes/{note_id}")),
            Activation::OpenNote { note_id }
        );
    }

    #[test]
    fn resolves_run_deeplink_to_confirm_before_run() {
        assert_eq!(
            resolve("scriptkit://run/example-script"),
            Activation::ConfirmBeforeRun {
                command_id: "script/example-script".to_string(),
                raw_href: "scriptkit://run/example-script".to_string(),
            }
        );
    }

    #[test]
    fn resolves_command_deeplink_to_confirm_before_run() {
        assert_eq!(
            resolve("scriptkit://commands/builtin/clipboard-history"),
            Activation::ConfirmBeforeRun {
                command_id: "builtin/clipboard-history".to_string(),
                raw_href: "scriptkit://commands/builtin/clipboard-history".to_string(),
            }
        );
    }

    #[test]
    fn spine_file_value_consumes_full_remaining_path() {
        assert_eq!(
            resolve("scriptkit://spine/file/src/main.rs"),
            Activation::ScopedSearch {
                source: ContextSubsearchSource::File,
                query: "src/main.rs".to_string(),
            }
        );
    }

    #[test]
    fn spine_value_percent_decodes_spaces_slashes_and_utf8() {
        assert_eq!(
            resolve("scriptkit://spine/project/my%20project%2Fnotes%20%E2%9C%A8.md"),
            Activation::ScopedSearch {
                source: ContextSubsearchSource::Project,
                query: "my project/notes ✨.md".to_string(),
            }
        );
    }

    #[test]
    fn empty_spine_value_is_error() {
        assert_eq!(
            resolve("scriptkit://spine/notes/"),
            Activation::Error(ActivationError {
                raw_href: "scriptkit://spine/notes/".to_string(),
                reason: ActivationErrorReason::EmptySpineValue {
                    prefix: "notes".to_string(),
                },
            })
        );
    }

    #[test]
    fn unknown_spine_prefix_is_error_with_supported_prefixes() {
        let activation = resolve("scriptkit://spine/nope/value");
        let Activation::Error(error) = activation else {
            panic!("expected error");
        };
        assert_eq!(error.raw_href, "scriptkit://spine/nope/value");
        match error.reason {
            ActivationErrorReason::UnknownSpinePrefix { prefix, supported } => {
                assert_eq!(prefix, "nope");
                assert!(supported.contains(&"notes"));
                assert!(supported.contains(&"browser-history"));
            }
            other => panic!("unexpected reason: {other:?}"),
        }
    }

    #[test]
    fn spine_scripts_never_resolves_to_run() {
        assert_eq!(
            resolve("scriptkit://spine/scripts/foo"),
            Activation::ScopedSearch {
                source: ContextSubsearchSource::Scripts,
                query: "foo".to_string(),
            }
        );
    }

    #[test]
    fn kit_resources_resolve_to_preview_with_surface_affordance() {
        assert_eq!(
            resolve_activation("kit://scripts", ActivationSurface::NotesWindow),
            Activation::KitResourcePreview {
                uri: "kit://scripts".to_string(),
                allow_agent_chat_action: false,
            }
        );
        assert_eq!(
            resolve_activation("kit://scripts", ActivationSurface::DayPage),
            Activation::KitResourcePreview {
                uri: "kit://scripts".to_string(),
                allow_agent_chat_action: true,
            }
        );
    }

    #[test]
    fn cheap_text_resource_preview_allowlist_is_narrow() {
        assert!(is_cheap_text_kit_resource_uri("kit://notes"));
        assert!(is_cheap_text_kit_resource_uri("kit://notes?limit=1"));
        assert!(is_cheap_text_kit_resource_uri("kit://scripts"));
        assert!(is_cheap_text_kit_resource_uri("kit://clipboard-history"));
        assert!(is_cheap_text_kit_resource_uri(
            "kit://clipboard-history?limit=1"
        ));
        assert!(is_cheap_text_kit_resource_uri(
            "kit://clipboard-history?id=abc"
        ));
        assert!(is_cheap_text_kit_resource_uri("kit://dictation-history"));
        assert!(is_cheap_text_kit_resource_uri(
            "kit://dictation-history?id=abc"
        ));
        assert!(!is_cheap_text_kit_resource_uri("kit://context"));
        assert!(!is_cheap_text_kit_resource_uri("kit://git-diff"));
    }

    #[test]
    fn kit_note_source_id_parses_only_single_note_uris() {
        let id = "35dfb389-4931-4d80-b079-4fa74f738ce7";
        let parsed = kit_note_source_id(&format!("kit://notes/{id}")).expect("valid note uri");
        assert_eq!(parsed.as_str(), id);
        assert_eq!(
            kit_note_source_id(&format!("kit://notes/{id}?x=1")).expect("query stripped"),
            parsed
        );
        assert_eq!(
            kit_note_source_id(&format!("kit://notes/{id}#frag")).expect("fragment stripped"),
            parsed
        );
        assert!(kit_note_source_id("kit://notes").is_none());
        assert!(kit_note_source_id("kit://notes?limit=1").is_none());
        assert!(kit_note_source_id("kit://notes/not-a-uuid").is_none());
        assert!(kit_note_source_id("kit://scripts").is_none());
        assert!(kit_note_source_id("kit://clipboard-history?id=abc").is_none());
    }

    #[test]
    fn unsupported_kit_resource_preview_errors_before_read() {
        let error = read_cheap_kit_resource_preview("kit://context").expect_err("unsupported");
        assert!(error.contains("kit://notes"));
        assert!(error.contains("kit://scripts"));
        assert!(error.contains("kit://clipboard-history"));
        assert!(error.contains("kit://context"));
    }

    #[test]
    fn unknown_scheme_is_error() {
        assert_eq!(
            resolve("unknown://thing"),
            Activation::Error(ActivationError {
                raw_href: "unknown://thing".to_string(),
                reason: ActivationErrorReason::UnknownScheme {
                    scheme: "unknown".to_string(),
                },
            })
        );
    }
}
