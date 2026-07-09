fn collect_chat_prompt_elements(
    &self,
    chat_prompt: &prompts::ChatPrompt,
    limit: usize,
) -> (Vec<protocol::ElementInfo>, usize) {
    let total_count = chat_prompt.messages.len() + 3;
    let mut elements = Vec::with_capacity(limit.min(total_count));

    Self::push_limited_element(
        &mut elements,
        limit,
        Self::input_element(
            "chat-model",
            "Model",
            chat_prompt.model.clone(),
            false,
            Some(0),
        ),
    );
    Self::push_limited_element(
        &mut elements,
        limit,
        Self::input_element(
            "chat-input",
            chat_prompt
                .placeholder
                .clone()
                .unwrap_or_else(|| "Message".to_string()),
            Some(Self::preview_value(chat_prompt.input.text(), 240)),
            true,
            Some(1),
        ),
    );
    Self::push_limited_element(
        &mut elements,
        limit,
        protocol::ElementInfo::list("chat-messages", chat_prompt.messages.len()),
    );

    for (index, message) in chat_prompt.messages.iter().enumerate() {
        if elements.len() >= limit {
            break;
        }
        let sender = if message.is_user() {
            "User"
        } else {
            "Assistant"
        };
        let content = message.get_content();
        let text = if content.is_empty() {
            sender.to_string()
        } else {
            format!("{sender}: {}", Self::preview_value(content, 180))
        };
        elements.push(Self::choice_element(
            index,
            text,
            Self::preview_value(content, 180),
            index + 1 == chat_prompt.messages.len(),
        ));
    }

    (elements, total_count)
}

pub(crate) fn script_list_result_label(result: &scripts::SearchResult) -> String {
    match result {
        scripts::SearchResult::Script(m) => m.script.name.clone(),
        scripts::SearchResult::Scriptlet(m) => {
            let vars = crate::context_templates::ContextTemplateVars::from_frontmost_tracker();
            crate::context_templates::substitute_context_vars(&m.scriptlet.name, &vars)
                .into_owned()
        }
        scripts::SearchResult::BuiltIn(m) => m.entry.name.clone(),
        scripts::SearchResult::App(m) => m.app.name.clone(),
        scripts::SearchResult::Window(m) => m.window.title.clone(),
        scripts::SearchResult::File(m) => m.file.name.clone(),
        scripts::SearchResult::Note(m) => m.title.clone(),
        scripts::SearchResult::BrainHit(m) => m.hit.title.clone(),
        scripts::SearchResult::BrainInboxItem(m) => m.item.title.clone(),
        scripts::SearchResult::Todo(m) => m.hit.title.clone(),
        scripts::SearchResult::AgentChatHistory(m) => m.entry.title_display().to_string(),
        scripts::SearchResult::AiVault(m) => m.hit.safe_title.clone(),
        scripts::SearchResult::ClipboardHistory(m) => m.title.clone(),
        scripts::SearchResult::DictationHistory(m) => m.preview.clone(),
        scripts::SearchResult::BrowserTab(m) => m.hit.title.clone(),
        scripts::SearchResult::BrowserHistory(m) => m.hit.title.clone(),
        scripts::SearchResult::Agent(m) => m.agent.name.clone(),
        scripts::SearchResult::Skill(m) => m.skill.title.clone(),
        scripts::SearchResult::Fallback(m) => m.display_label(),
        scripts::SearchResult::ScriptIssue(m) => m.title.clone(),
        scripts::SearchResult::SpineProjection(row) => row.title.to_string(),
    }
}
