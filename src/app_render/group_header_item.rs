fn preview_keyword_tags(keywords: &[String]) -> Vec<String> {
    let mut tags = Vec::new();

    for keyword in keywords {
        let normalized = keyword.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        if tags.iter().any(|tag| tag == &normalized) {
            continue;
        }
        tags.push(normalized);
        if tags.len() >= 6 {
            break;
        }
    }

    tags
}

fn builtin_feature_annotation(feature: &builtins::BuiltInFeature) -> String {
    match feature {
        builtins::BuiltInFeature::ClipboardHistory => "Clipboard History Manager".to_string(),
        builtins::BuiltInFeature::PasteSequentially => "Paste Next Clipboard Item".to_string(),
        builtins::BuiltInFeature::Favorites => "Favorites".to_string(),
        builtins::BuiltInFeature::AppLauncher => "Application Launcher".to_string(),
        builtins::BuiltInFeature::App(name) => name.clone(),
        builtins::BuiltInFeature::WindowSwitcher => "Window Manager".to_string(),
        builtins::BuiltInFeature::BrowserTabs => "Browser Tabs".to_string(),
        builtins::BuiltInFeature::SyncToGithub => "Sync to GitHub".to_string(),
        builtins::BuiltInFeature::AiChat => "Agent Chat".to_string(),
        builtins::BuiltInFeature::AiChatVariant(_) => "Agent Chat".to_string(),
        builtins::BuiltInFeature::Notes => "Notes & Scratchpad".to_string(),
        builtins::BuiltInFeature::EmojiPicker => "Emoji Picker".to_string(),
        builtins::BuiltInFeature::MenuBarAction(_) => "Menu Bar Action".to_string(),
        builtins::BuiltInFeature::SystemAction(_) => "System Action".to_string(),
        builtins::BuiltInFeature::NotesCommand(_) => "Notes Command".to_string(),
        builtins::BuiltInFeature::AiCommand(_) => "Agent Chat Command".to_string(),
        builtins::BuiltInFeature::ScriptCommand(_) => "Script Creation".to_string(),
        builtins::BuiltInFeature::NewScriptFromTemplate => "Script Creation".to_string(),
        builtins::BuiltInFeature::NewFlow => "Flow Creation".to_string(),
        builtins::BuiltInFeature::PermissionCommand(_) => "Permission Management".to_string(),
        builtins::BuiltInFeature::FrecencyCommand(_) => "Suggested Items".to_string(),
        builtins::BuiltInFeature::UtilityCommand(_) => "Quick Utility".to_string(),
        builtins::BuiltInFeature::SettingsCommand(_) => "Settings".to_string(),
        builtins::BuiltInFeature::FileSearch => "File Browser".to_string(),
        builtins::BuiltInFeature::Webcam => "Webcam Capture".to_string(),
        builtins::BuiltInFeature::Dictation => "Dictate to Current App".to_string(),
        builtins::BuiltInFeature::DictationToAiHarness => {
            "Voice Dictation to Agent Chat".to_string()
        }
        builtins::BuiltInFeature::DictationToFrontmostApp => "Voice Dictation to App".to_string(),
        builtins::BuiltInFeature::DictationToNotes => "Voice Dictation to Notes".to_string(),
        builtins::BuiltInFeature::AgentChatHistory => "Agent Chat History".to_string(),
        builtins::BuiltInFeature::AiVault => "AI Vault".to_string(),
        builtins::BuiltInFeature::DictationHistory => "Dictation History".to_string(),
        builtins::BuiltInFeature::Settings => "Script Kit Settings".to_string(),
        builtins::BuiltInFeature::SdkReference => "SDK Reference".to_string(),
        builtins::BuiltInFeature::Tips => "Tips".to_string(),
        builtins::BuiltInFeature::MigrateV1Scripts => "Migration".to_string(),
        builtins::BuiltInFeature::FlowUxVariant(_) => "Flow Launcher".to_string(),
        builtins::BuiltInFeature::BackgroundEffectNext
        | builtins::BuiltInFeature::BackgroundEffectPrevious
        | builtins::BuiltInFeature::BackgroundEffectOff => "Background Effect".to_string(),
    }
}

fn group_header_section_name(name: &str) -> String {
    name.to_uppercase()
}

fn should_render_group_header_divider(ix: usize) -> bool {
    ix > 0
}

/// Helper function to render a group header style item with actual visual styling
fn render_group_header_item(
    ix: usize,
    is_selected: bool,
    style: &designs::group_header_variations::GroupHeaderStyle,
    spacing: &designs::DesignSpacing,
    _typography: &designs::DesignTypography,
    _visual: &designs::DesignVisual,
    colors: &designs::DesignColors,
) -> AnyElement {
    use crate::list_item::{ALPHA_SEPARATOR, SECTION_HEADER_HEIGHT, SECTION_PADDING_TOP};
    #[allow(unused_imports)]
    use designs::group_header_variations::GroupHeaderStyle;

    let name_owned = group_header_section_name(style.name());
    let divider_color = rgba((colors.text_secondary << 8) | ALPHA_SEPARATOR);

    let mut item_div = div()
        .id(ElementId::NamedInteger("gallery-header".into(), ix as u64))
        .w_full()
        .h(px(SECTION_HEADER_HEIGHT))
        .px(px(spacing.padding_lg))
        .pt(px(SECTION_PADDING_TOP))
        .flex()
        .flex_col()
        .justify_end();

    if should_render_group_header_divider(ix) {
        item_div = item_div.border_t_1().border_color(divider_color);
    }

    if is_selected {
        // Keep selected tint subtle so headers stay non-row-like.
        item_div = item_div.bg(rgba((colors.background_selected << 8) | ALPHA_SEPARATOR));
    }

    item_div
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(rgb(colors.text_secondary))
                .child(name_owned),
        )
        .into_any_element()
}

#[cfg(test)]
mod group_header_item_tests {
    use super::*;

    #[test]
    fn test_group_header_section_name_uppercases_labels() {
        assert_eq!(group_header_section_name("Main"), "MAIN");
    }

    #[test]
    fn test_should_render_group_header_divider_only_after_first_item() {
        assert!(!should_render_group_header_divider(0));
        assert!(should_render_group_header_divider(1));
    }
}
