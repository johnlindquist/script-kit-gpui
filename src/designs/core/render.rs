use super::metadata::{
    auto_description_for_script, code_preview_for_scriptlet, grouped_view_hint_for_script,
    grouped_view_hint_for_scriptlet,
};
use super::variant::DesignVariant;
use crate::designs::{MinimalColors, MinimalRenderer, RetroTerminalRenderer};
use crate::list_item::ListItemColors;
use crate::scripts::SearchResult;
use gpui::{AnyElement, IntoElement};

fn substitute_context_vars(text: &str) -> String {
    let vars = crate::context_templates::ContextTemplateVars::from_frontmost_tracker();
    crate::context_templates::substitute_context_vars(text, &vars).into_owned()
}

/// Map a script's file extension to a more appropriate default icon.
/// Returns an icon name like "Terminal" for shell scripts, "Code" for everything else.
/// Only used when the script has no explicit `// Icon:` metadata.
pub(crate) fn extension_default_icon(extension: &str) -> &'static str {
    match extension {
        "sh" | "bash" | "zsh" => "Terminal",
        "applescript" | "scpt" => "Terminal",
        _ => "Code",
    }
}

/// Map root launcher file-result types to static SVG icons.
pub(crate) fn root_file_type_svg_icon(file_type: crate::file_search::FileType) -> &'static str {
    match file_type {
        crate::file_search::FileType::Directory => "FolderOpen",
        crate::file_search::FileType::Application => "package",
        crate::file_search::FileType::Image => "file-image",
        crate::file_search::FileType::Document => "file-text",
        crate::file_search::FileType::Audio => "file-audio",
        crate::file_search::FileType::Video => "file-video",
        crate::file_search::FileType::File | crate::file_search::FileType::Other => "File",
    }
}

pub(crate) fn root_file_display_path(path: &str) -> String {
    crate::file_search::shorten_path(path)
}

pub(crate) fn ai_vault_provider_svg_icon(provider: &str, display_name: &str) -> &'static str {
    let normalized_provider = provider
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();
    let normalized_display = display_name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase();

    match (normalized_provider.as_str(), normalized_display.as_str()) {
        ("codex", _) | (_, "codex") | (_, "openaicodex") => "assets/icons/ai_provider_openai.svg",
        ("claude", _) | (_, "claudecode") | (_, "claude") | (_, "anthropic") => {
            "assets/icons/ai_provider_claude.svg"
        }
        ("rovodev", _) | (_, "rovodev") | (_, "atlassianrovo") | (_, "atlassian") => {
            "assets/icons/ai_provider_atlassian.svg"
        }
        ("hermesagent", _) | (_, "hermesagent") => "Terminal",
        _ => "Vault",
    }
}

#[derive(Debug, Default)]
pub(crate) struct SearchAccessories {
    pub(crate) type_accessory: Option<crate::list_item::TypeAccessory>,
    pub(crate) source_hint: Option<String>,
}

pub(crate) fn resolve_search_accessories(
    result: &SearchResult,
    filter_text: &str,
) -> SearchAccessories {
    if filter_text.is_empty() {
        return SearchAccessories::default();
    }
    if matches!(result, SearchResult::AiVault(_) | SearchResult::File(_)) {
        return SearchAccessories::default();
    }

    // Search rows should stay calm: keep only a quiet type icon.
    // Category/match-reason metadata is intentionally hidden.
    let (label, icon_name) = result.type_accessory_info();
    SearchAccessories {
        type_accessory: Some(crate::list_item::TypeAccessory { label, icon_name }),
        source_hint: None,
    }
}

pub(crate) fn resolve_tool_badge(result: &SearchResult, is_filtering: bool) -> Option<String> {
    if is_filtering {
        // Action/tool badges ("paste", "open", etc.) are too noisy during search.
        return None;
    }

    match result {
        SearchResult::Script(sm) => sm.script.typed_metadata.as_ref().and_then(|meta| {
            if meta.cron.is_some() || meta.schedule.is_some() {
                Some("cron".to_string())
            } else if !meta.watch.is_empty() {
                Some("watch".to_string())
            } else if meta.background {
                Some("bg".to_string())
            } else if meta.system {
                Some("sys".to_string())
            } else {
                None
            }
        }),
        SearchResult::Scriptlet(sm) => Some(sm.scriptlet.tool.clone()),
        _ => None,
    }
}

/// Render a single list item for the given design variant
///
/// This is the main dispatch function for design-specific item rendering.
/// It renders a single item based on the current design, with proper styling.
///
/// # Arguments
/// * `variant` - The design variant to render
/// * `result` - The search result to render
/// * `index` - The item index (for element ID and alternating styles)
/// * `is_selected` - Whether this item is currently selected (full focus styling)
/// * `is_hovered` - Whether this item is currently hovered (subtle visual feedback)
/// * `list_colors` - Pre-computed theme colors for the default design
/// * `filter_text` - Current search filter text (empty when not filtering; used for fuzzy match highlighting)
///
/// # Returns
/// An `AnyElement` containing the rendered item
#[allow(clippy::too_many_arguments)]
pub fn render_design_item(
    variant: DesignVariant,
    main_menu_theme: crate::designs::MainMenuThemeVariant,
    result: &SearchResult,
    index: usize,
    is_selected: bool,
    is_hovered: bool,
    list_colors: ListItemColors,
    filter_text: &str,
) -> AnyElement {
    // NOTE: Removed per-item DEBUG log that was causing log spam.
    // This function is called for every visible list item on every render frame.
    // With cursor blink triggering renders every 530ms, this logged 8-9 items × 2 renders/sec.

    match variant {
        DesignVariant::Minimal => {
            let colors = MinimalColors {
                text_primary: list_colors.text_primary,
                accent_selected: list_colors.accent_selected,
            };
            MinimalRenderer::new()
                .render_item(result, index, is_selected, colors)
                .into_any_element()
        }
        DesignVariant::RetroTerminal => RetroTerminalRenderer::new()
            .render_item(result, index, is_selected)
            .into_any_element(),
        // All other variants use the default ListItem renderer
        _ => {
            use crate::list_item::{IconKind, ListItem};

            // Compute fuzzy match indices for highlighting when actively filtering
            let (highlight_indices, description_highlight_indices) = if !filter_text.is_empty() {
                let indices =
                    crate::scripts::search::compute_match_indices_for_result(result, filter_text);
                let name_hi = if indices.name_indices.is_empty() {
                    None
                } else {
                    Some(indices.name_indices)
                };
                let desc_hi = if indices.description_indices.is_empty() {
                    None
                } else {
                    Some(indices.description_indices)
                };
                (name_hi, desc_hi)
            } else {
                (None, None)
            };

            // Extract name, description, shortcut, and icon based on result type
            let (name, description, shortcut, icon_kind) = match result {
                SearchResult::Script(sm) => {
                    // Use script's icon metadata if present, otherwise default to "Code" SVG
                    let icon = match &sm.script.icon {
                        Some(icon_name) => IconKind::Svg(icon_name.clone()),
                        None => {
                            IconKind::Svg(extension_default_icon(&sm.script.extension).to_string())
                        }
                    };
                    // Scripts: show shortcut or alias as badge (matching scriptlet pattern)
                    let badge = sm
                        .script
                        .shortcut
                        .clone()
                        .or_else(|| sm.script.alias.clone());
                    let description = auto_description_for_script(&sm.script);
                    (sm.script.name.clone(), description, badge, Some(icon))
                }
                SearchResult::Scriptlet(sm) => {
                    // Scriptlets: show shortcut, keyword, or alias as badge
                    let badge = sm
                        .scriptlet
                        .shortcut
                        .clone()
                        .or_else(|| sm.scriptlet.keyword.clone())
                        .or_else(|| sm.scriptlet.alias.clone());
                    // Prefer the scriptlet's own icon (from `icon:` metadata or bundle
                    // frontmatter), falling back to a tool-type default
                    let icon = match &sm.scriptlet.icon {
                        Some(icon_name) => IconKind::Svg(icon_name.clone()),
                        None => match sm.scriptlet.tool.as_str() {
                            "bash" | "sh" | "zsh" => IconKind::Svg("Terminal".to_string()),
                            "paste" | "snippet" => IconKind::Svg("Copy".to_string()),
                            "open" => IconKind::Svg("PlayFilled".to_string()),
                            _ => IconKind::Svg("BoltFilled".to_string()),
                        },
                    };
                    // Auto-generate description: prefer code preview, fall back to tool name
                    // Code preview gives users immediate insight into what the scriptlet does
                    let description = sm.scriptlet.description.clone().or_else(|| {
                        code_preview_for_scriptlet(&sm.scriptlet)
                            .or_else(|| Some(sm.scriptlet.tool_display_name().to_string()))
                    });
                    let name = substitute_context_vars(&sm.scriptlet.name);
                    let description = description.map(|d| substitute_context_vars(&d));
                    (name, description, badge, Some(icon))
                }
                SearchResult::BuiltIn(bm) => {
                    // Built-ins: pass icon name through directly (Lucide kebab-case)
                    let icon = match &bm.entry.icon {
                        Some(name) => IconKind::Svg(name.clone()),
                        None => IconKind::Svg("settings".to_string()),
                    };
                    (
                        bm.entry.name.clone(),
                        Some(bm.entry.description.clone()),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::App(am) => {
                    // Apps use pre-decoded icons, fallback to File SVG
                    let icon = match &am.app.icon {
                        Some(img) => IconKind::Image(img.clone()),
                        None => IconKind::Svg("File".to_string()),
                    };
                    (am.app.name.clone(), None, None, Some(icon))
                }
                SearchResult::Window(wm) => {
                    let icon = match &wm.app_icon {
                        Some(img) => IconKind::Image(img.clone()),
                        None => IconKind::Svg("panel-top".to_string()),
                    };
                    (
                        wm.window.title.clone(),
                        Some(wm.subtitle.clone()),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::File(fm) => (
                    fm.file.name.clone(),
                    Some(root_file_display_path(&fm.file.path)),
                    None,
                    Some(IconKind::Svg(
                        root_file_type_svg_icon(fm.file.file_type).to_string(),
                    )),
                ),
                SearchResult::Note(nm) => (
                    nm.title.clone(),
                    Some(nm.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("NotebookText".to_string())),
                ),
                SearchResult::BrainHit(bm) => (
                    bm.hit.title.clone(),
                    Some(bm.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("brain".to_string())),
                ),
                SearchResult::BrainInboxItem(bm) => (
                    bm.item.title.clone(),
                    Some(bm.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("inbox".to_string())),
                ),
                SearchResult::Todo(tm) => (
                    tm.hit.title.clone(),
                    Some(tm.hit.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("ListTodo".to_string())),
                ),
                SearchResult::AgentChatHistory(am) => (
                    am.entry.title_display().to_string(),
                    Some(am.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("MessageCircle".to_string())),
                ),
                SearchResult::AiVault(am) => (
                    am.hit.safe_title.clone(),
                    Some(am.subtitle.clone()),
                    None,
                    Some(IconKind::Svg(
                        ai_vault_provider_svg_icon(&am.hit.provider, &am.hit.provider_display_name)
                            .to_string(),
                    )),
                ),
                SearchResult::ClipboardHistory(cm) => (
                    cm.title.clone(),
                    Some(cm.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("Clipboard".to_string())),
                ),
                SearchResult::DictationHistory(dm) => (
                    dm.preview.clone(),
                    Some(dm.subtitle.clone()),
                    None,
                    Some(IconKind::Svg("Mic".to_string())),
                ),
                SearchResult::BrowserTab(bm) => {
                    let icon = if let Some(img) = crate::favicons::cached_favicon(&bm.hit.url) {
                        IconKind::Image(img)
                    } else {
                        IconKind::Svg("PanelTop".to_string())
                    };
                    (
                        bm.hit.title.clone(),
                        Some(bm.subtitle.clone()),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::BrowserHistory(bm) => {
                    let icon = if let Some(img) = crate::favicons::cached_favicon(&bm.hit.url) {
                        IconKind::Image(img)
                    } else {
                        IconKind::Svg("Globe".to_string())
                    };
                    (
                        bm.hit.title.clone(),
                        Some(bm.subtitle.clone()),
                        None,
                        Some(icon),
                    )
                }
                SearchResult::Skill(sm) => {
                    // Skills use a star icon (gold accent theme)
                    let description = if sm.skill.description.is_empty() {
                        Some(format!("{} skill", sm.skill.plugin_title))
                    } else {
                        Some(sm.skill.description.clone())
                    };
                    (
                        sm.skill.title.clone(),
                        description,
                        None,
                        Some(IconKind::Svg("StarFilled".to_string())),
                    )
                }
                SearchResult::Agent(am) => {
                    // Agents use backend-specific icons, with backend label in description
                    let icon_name = am
                        .agent
                        .icon
                        .clone()
                        .unwrap_or_else(|| am.agent.backend.icon().to_string());
                    let backend_label = am.agent.backend.label();
                    let description = am
                        .agent
                        .description
                        .clone()
                        .or_else(|| Some(format!("{} Agent", backend_label)));
                    (
                        am.agent.name.clone(),
                        description,
                        am.agent.shortcut.clone(),
                        Some(IconKind::Svg(icon_name)),
                    )
                }
                SearchResult::Fallback(fm) => {
                    let fallback_label = fm.display_label();
                    let fallback_description = fm.display_description();
                    // Fallback commands from "Use with..." section
                    // Map fallback icon names to SVG icons

                    let is_open_url = match &fm.fallback {
                        crate::fallbacks::FallbackItem::Builtin(b) => b.id == "open-url",
                        crate::fallbacks::FallbackItem::Script(_) => false,
                    };

                    let icon = if is_open_url {
                        if let Some(img) = crate::favicons::get_or_fetch_favicon(filter_text) {
                            IconKind::Image(img)
                        } else {
                            IconKind::Svg("ExternalLink".to_string())
                        }
                    } else {
                        let icon_name = match fm.fallback.icon() {
                            "external-link" => "ExternalLink",
                            "calculator" => "Calculator",
                            "file" => "File",
                            "terminal" => "Terminal",
                            "sticky-note" => "StickyNote",
                            "clipboard-copy" => "Copy",
                            "search" => "MagnifyingGlass",
                            other => other, // Pass through if already a valid icon name
                        };
                        IconKind::Svg(icon_name.to_string())
                    };

                    (fallback_label, Some(fallback_description), None, Some(icon))
                }
                SearchResult::ScriptIssue(issue) => (
                    issue.title.clone(),
                    issue.description.clone(),
                    None,
                    Some(IconKind::Svg("ExclamationTriangle".to_string())),
                ),
                SearchResult::SpineProjection(row) => (
                    row.title.to_string(),
                    row.subtitle.as_ref().map(|s| s.to_string()),
                    None,
                    Some(IconKind::Svg(
                        row.icon
                            .as_ref()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| "list".to_string()),
                    )),
                ),
                SearchResult::Flow(fm) => (
                    fm.display_name.clone(),
                    Some(fm.subtitle.clone()),
                    None,
                    // Chevron-fade is the flows brand mark (icon brainstorm
                    // 2026-07-11); resolves via IconName::Flow.
                    Some(IconKind::Svg("flow".to_string())),
                ),
            };

            // During search mode, keep only quiet type icons.
            // During grouped mode, use discoverability hints.
            let (type_accessory, source_hint) = if !filter_text.is_empty() {
                let accessories = resolve_search_accessories(result, filter_text);
                (accessories.type_accessory, accessories.source_hint)
            } else {
                // Grouped view: use extracted helpers for discoverability hints
                let hint = match result {
                    SearchResult::Script(sm) => grouped_view_hint_for_script(&sm.script),
                    SearchResult::Scriptlet(sm) => grouped_view_hint_for_scriptlet(&sm.scriptlet),
                    _ => None,
                };
                (None, hint)
            };

            // Tool/language badge for scriptlets (e.g., "ts", "bash", "paste")
            // For scripts: show property indicator if the script has special runtime behavior
            // (cron/schedule, file watch, background, system)
            let tool_badge = resolve_tool_badge(result, !filter_text.is_empty());

            ListItem::new(name, list_colors)
                .index(index)
                .icon_kind_opt(icon_kind)
                .shortcut_opt(shortcut)
                .description_opt(description)
                .selected(is_selected)
                .hovered(is_hovered)
                .main_menu_theme(main_menu_theme)
                .highlight_indices_opt(highlight_indices)
                .description_highlight_indices_opt(description_highlight_indices)
                .type_accessory_opt(type_accessory)
                .source_hint_opt(source_hint)
                .tool_badge_opt(tool_badge)
                .into_any_element()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ai_vault_provider_svg_icon, resolve_search_accessories};
    use crate::ai_vault::{AiVaultHit, AiVaultMatchedField};
    use crate::scripts::{AiVaultMatch, SearchResult};

    #[test]
    fn ai_vault_provider_icons_track_originating_tool() {
        assert_eq!(
            ai_vault_provider_svg_icon("codex", "Codex"),
            "assets/icons/ai_provider_openai.svg"
        );
        assert_eq!(
            ai_vault_provider_svg_icon("claude", "Claude Code"),
            "assets/icons/ai_provider_claude.svg"
        );
        assert_eq!(
            ai_vault_provider_svg_icon("rovoDev", "Rovo Dev"),
            "assets/icons/ai_provider_atlassian.svg"
        );
        assert_eq!(ai_vault_provider_svg_icon("hermes-agent", ""), "Terminal");
        assert_eq!(ai_vault_provider_svg_icon("unknown", ""), "Vault");
    }

    #[test]
    fn ai_vault_search_rows_do_not_render_right_type_accessory() {
        let hit: AiVaultHit = serde_json::from_value(serde_json::json!({
            "provider": "codex",
            "providerDisplayName": "Codex",
            "sessionId": "session-1",
            "sourceKind": "cli",
            "safeTitle": "Investigate launcher filtering",
            "matchedField": "title",
            "stableKey": "ai-vault/codex/cli/session-1",
            "score": 0
        }))
        .expect("AI Vault hit fixture should deserialize");
        assert_eq!(hit.matched_field, AiVaultMatchedField::Title);

        let result = SearchResult::AiVault(AiVaultMatch {
            hit,
            subtitle: "Codex".to_string(),
            score: 1,
        });

        let accessories = resolve_search_accessories(&result, "vault");
        assert!(accessories.type_accessory.is_none());
        assert!(accessories.source_hint.is_none());
    }
}
