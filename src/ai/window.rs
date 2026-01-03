//! AI Chat Window
//!
//! A separate floating window for AI chat, built with gpui-component.
//! This is completely independent from the main Script Kit launcher window.
//!
//! # Architecture
//!
//! The window follows a Raycast-style layout:
//! - Left sidebar: Chat history list with search, grouped by date (Today, Yesterday, This Week, Older)
//! - Right main panel: Welcome state ("Ask Anything") or chat messages
//! - Bottom: Input area + model picker + submit button

use anyhow::Result;
use chrono::{Datelike, NaiveDate, Utc};
use gpui::{
    div, hsla, point, prelude::*, px, rgb, size, svg, App, BoxShadow, Context, Entity, FocusHandle,
    Focusable, Hsla, IntoElement, KeyDownEvent, ParentElement, Render, ScrollHandle, SharedString,
    Styled, Subscription, Window, WindowBounds, WindowOptions,
};

// Import local IconName for SVG icons (has external_path() method)
use crate::designs::icon_variations::IconName as LocalIconName;

#[cfg(target_os = "macos")]
use cocoa::appkit::NSApp;
#[cfg(target_os = "macos")]
use cocoa::base::{id, nil};
use gpui_component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{Input, InputEvent, InputState},
    scroll::ScrollableElement,
    theme::{ActiveTheme, Theme as GpuiTheme, ThemeColor, ThemeMode},
    Icon, IconName, Root, Sizable,
};
#[cfg(target_os = "macos")]
use objc::{msg_send, sel, sel_impl};
use tracing::{debug, info};

use super::config::ModelInfo;
use super::model::{Chat, ChatId, Message, MessageRole};
use super::providers::ProviderRegistry;
use super::storage;
use crate::watcher::ThemeWatcher;

/// Events from the streaming thread
enum StreamingEvent {
    /// A chunk of text received
    Chunk(String),
    /// Streaming completed successfully
    Done,
    /// An error occurred
    Error(String),
}

/// Date group categories for sidebar organization
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateGroup {
    Today,
    Yesterday,
    ThisWeek,
    Older,
}

impl DateGroup {
    /// Get the display label for this group
    fn label(&self) -> &'static str {
        match self {
            DateGroup::Today => "Today",
            DateGroup::Yesterday => "Yesterday",
            DateGroup::ThisWeek => "This Week",
            DateGroup::Older => "Older",
        }
    }
}

/// Determine which date group a date belongs to
fn get_date_group(date: NaiveDate, today: NaiveDate) -> DateGroup {
    let days_ago = today.signed_duration_since(date).num_days();

    if days_ago == 0 {
        DateGroup::Today
    } else if days_ago == 1 {
        DateGroup::Yesterday
    } else if days_ago < 7
        && date.weekday().num_days_from_monday() < today.weekday().num_days_from_monday()
    {
        // Same week (and not earlier in a previous week)
        DateGroup::ThisWeek
    } else if days_ago < 7 {
        DateGroup::ThisWeek
    } else {
        DateGroup::Older
    }
}

/// Group chats by date categories
fn group_chats_by_date(chats: &[Chat]) -> Vec<(DateGroup, Vec<&Chat>)> {
    let today = Utc::now().date_naive();

    let mut today_chats: Vec<&Chat> = Vec::new();
    let mut yesterday_chats: Vec<&Chat> = Vec::new();
    let mut this_week_chats: Vec<&Chat> = Vec::new();
    let mut older_chats: Vec<&Chat> = Vec::new();

    for chat in chats {
        let chat_date = chat.updated_at.date_naive();
        match get_date_group(chat_date, today) {
            DateGroup::Today => today_chats.push(chat),
            DateGroup::Yesterday => yesterday_chats.push(chat),
            DateGroup::ThisWeek => this_week_chats.push(chat),
            DateGroup::Older => older_chats.push(chat),
        }
    }

    let mut groups = Vec::new();
    if !today_chats.is_empty() {
        groups.push((DateGroup::Today, today_chats));
    }
    if !yesterday_chats.is_empty() {
        groups.push((DateGroup::Yesterday, yesterday_chats));
    }
    if !this_week_chats.is_empty() {
        groups.push((DateGroup::ThisWeek, this_week_chats));
    }
    if !older_chats.is_empty() {
        groups.push((DateGroup::Older, older_chats));
    }

    groups
}

/// Generate a contextual mock AI response based on the user's message
/// Used for demo/testing when no AI providers are configured
fn generate_mock_response(user_message: &str) -> String {
    let msg_lower = user_message.to_lowercase();

    // Contextual responses based on common patterns
    if msg_lower.contains("hello") || msg_lower.contains("hi") || msg_lower.starts_with("hey") {
        return "Hello! I'm Script Kit's AI assistant running in demo mode. Since no API key is configured, I'm providing mock responses. To enable real AI, set `SCRIPT_KIT_ANTHROPIC_API_KEY` or `SCRIPT_KIT_OPENAI_API_KEY` in your environment.".to_string();
    }

    if msg_lower.contains("script") || msg_lower.contains("automation") {
        return "Script Kit is a powerful automation tool! Here are some things you can do:\n\n1. **Create scripts** - Write TypeScript/JavaScript to automate tasks\n2. **Use prompts** - `arg()`, `editor()`, `div()` for interactive UIs\n3. **Hotkeys** - Bind scripts to global keyboard shortcuts\n4. **Snippets** - Text expansion with dynamic content\n\nTry running a script with `Cmd+;` to see it in action!".to_string();
    }

    if msg_lower.contains("help") || msg_lower.contains("how") {
        return "I'm here to help! In demo mode, I can explain Script Kit concepts:\n\n• **Scripts** live in `~/.sk/kit/scripts/`\n• **SDK** provides `arg()`, `div()`, `editor()`, and more\n• **Hotkeys** are configured in script metadata\n• **This AI chat** works with Claude or GPT when you add an API key\n\nWhat would you like to know more about?".to_string();
    }

    if msg_lower.contains("code") || msg_lower.contains("example") {
        return "Here's a simple Script Kit example:\n\n```typescript\n// Name: Hello World\n// Shortcut: cmd+shift+h\n\nconst name = await arg(\"What's your name?\");\nawait div(`<h1>Hello, ${name}!</h1>`);\n```\n\nThis creates a script that:\n1. Asks for your name via a prompt\n2. Displays a greeting in an HTML view\n\nSave this to `~/.sk/kit/scripts/hello.ts` and run it!".to_string();
    }

    if msg_lower.contains("api") || msg_lower.contains("key") || msg_lower.contains("configure") {
        return "To enable real AI responses, configure an API key:\n\n**For Claude (Anthropic):**\n```bash\nexport SCRIPT_KIT_ANTHROPIC_API_KEY=\"sk-ant-...\"\n```\n\n**For GPT (OpenAI):**\n```bash\nexport SCRIPT_KIT_OPENAI_API_KEY=\"sk-...\"\n```\n\nAdd these to your `~/.zshrc` or `~/.sk/kit/.env` file, then restart Script Kit.".to_string();
    }

    // Default response for unrecognized queries
    format!(
        "I received your message: \"{}\"\n\n\
        I'm running in **demo mode** because no AI API key is configured. \
        My responses are pre-written examples.\n\n\
        To get real AI responses:\n\
        1. Get an API key from Anthropic or OpenAI\n\
        2. Set `SCRIPT_KIT_ANTHROPIC_API_KEY` or `SCRIPT_KIT_OPENAI_API_KEY`\n\
        3. Restart Script Kit\n\n\
        Try asking about \"scripts\", \"help\", or \"code examples\" to see more demo responses!",
        user_message.chars().take(50).collect::<String>()
    )
}

/// Global handle to the AI window
static AI_WINDOW: std::sync::OnceLock<std::sync::Mutex<Option<gpui::WindowHandle<Root>>>> =
    std::sync::OnceLock::new();

/// Global handle to the AiApp entity (for updating state from outside)
static AI_APP_ENTITY: std::sync::OnceLock<std::sync::Mutex<Option<Entity<AiApp>>>> =
    std::sync::OnceLock::new();

/// The main AI chat application view
pub struct AiApp {
    /// All chats (cached from storage)
    chats: Vec<Chat>,

    /// Currently selected chat ID
    selected_chat_id: Option<ChatId>,

    /// Cache of last message preview per chat (ChatId -> preview text)
    message_previews: std::collections::HashMap<ChatId, String>,

    /// Chat input state (using gpui-component's Input)
    input_state: Entity<InputState>,

    /// Search input state for sidebar
    search_state: Entity<InputState>,

    /// Current search query
    search_query: String,

    /// Whether the sidebar is collapsed
    sidebar_collapsed: bool,

    /// Provider registry with available AI providers
    provider_registry: ProviderRegistry,

    /// Available models from all providers
    available_models: Vec<ModelInfo>,

    /// Currently selected model for new chats
    selected_model: Option<ModelInfo>,

    /// Focus handle for keyboard navigation
    focus_handle: FocusHandle,

    /// Subscriptions to keep alive
    _subscriptions: Vec<Subscription>,

    // === Streaming State ===
    /// Whether we're currently streaming a response
    is_streaming: bool,

    /// Content accumulated during streaming
    streaming_content: String,

    /// Messages for the currently selected chat (cached for display)
    current_messages: Vec<Message>,

    /// Scroll handle for the messages area (for auto-scrolling during streaming)
    messages_scroll_handle: ScrollHandle,
}

impl AiApp {
    /// Create a new AiApp
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Initialize storage
        if let Err(e) = storage::init_ai_db() {
            tracing::error!(error = %e, "Failed to initialize AI database");
        }

        // Load chats from storage
        let chats = storage::get_all_chats().unwrap_or_default();
        let selected_chat_id = chats.first().map(|c| c.id);

        // Load message previews for each chat
        let mut message_previews = std::collections::HashMap::new();
        for chat in &chats {
            if let Ok(messages) = storage::get_recent_messages(&chat.id, 1) {
                if let Some(last_msg) = messages.first() {
                    // Truncate preview to ~60 chars
                    let preview: String = last_msg.content.chars().take(60).collect();
                    let preview = if preview.len() < last_msg.content.len() {
                        format!("{}...", preview.trim())
                    } else {
                        preview
                    };
                    message_previews.insert(chat.id, preview);
                }
            }
        }

        // Initialize provider registry from environment
        let provider_registry = ProviderRegistry::from_environment();
        let available_models = provider_registry.get_all_models();

        // Select default model (prefer Claude, then GPT-4o)
        let selected_model = available_models
            .iter()
            .find(|m| m.id.contains("claude-3-5-sonnet"))
            .or_else(|| available_models.iter().find(|m| m.id == "gpt-4o"))
            .or_else(|| available_models.first())
            .cloned();

        info!(
            providers = provider_registry.provider_ids().len(),
            models = available_models.len(),
            selected = selected_model
                .as_ref()
                .map(|m| m.display_name.as_str())
                .unwrap_or("none"),
            "AI providers initialized"
        );

        // Create input states
        let input_state = cx.new(|cx| InputState::new(window, cx).placeholder("Ask anything..."));

        let search_state = cx.new(|cx| InputState::new(window, cx).placeholder("Search chats..."));

        let focus_handle = cx.focus_handle();

        // Subscribe to input changes and Enter key
        let input_sub = cx.subscribe_in(&input_state, window, {
            move |this, _, ev: &InputEvent, window, cx| match ev {
                InputEvent::Change => this.on_input_change(cx),
                InputEvent::PressEnter { .. } => this.submit_message(window, cx),
                _ => {}
            }
        });

        // Subscribe to search changes
        let search_sub = cx.subscribe_in(&search_state, window, {
            move |this, _, ev: &InputEvent, _window, cx| {
                if matches!(ev, InputEvent::Change) {
                    this.on_search_change(cx);
                }
            }
        });

        // Load messages for the selected chat
        let current_messages = selected_chat_id
            .and_then(|id| storage::get_chat_messages(&id).ok())
            .unwrap_or_default();

        info!(chat_count = chats.len(), "AI app initialized");

        Self {
            chats,
            selected_chat_id,
            message_previews,
            input_state,
            search_state,
            search_query: String::new(),
            sidebar_collapsed: false,
            provider_registry,
            available_models,
            selected_model,
            focus_handle,
            _subscriptions: vec![input_sub, search_sub],
            // Streaming state
            is_streaming: false,
            streaming_content: String::new(),
            current_messages,
            messages_scroll_handle: ScrollHandle::new(),
        }
    }

    /// Handle input changes
    fn on_input_change(&mut self, _cx: &mut Context<Self>) {
        // TODO: Handle input changes (e.g., streaming, auto-complete)
    }

    /// Handle model selection change
    fn on_model_change(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(model) = self.available_models.get(index) {
            info!(
                model_id = model.id,
                model_name = model.display_name,
                provider = model.provider,
                "Model selected"
            );
            self.selected_model = Some(model.clone());
            cx.notify();
        }
    }

    /// Handle search query changes - filters chats in real-time as user types
    fn on_search_change(&mut self, cx: &mut Context<Self>) {
        let query = self.search_state.read(cx).value().to_string();
        self.search_query = query.clone();

        debug!(query = %query, "Search query changed");

        // If search is not empty, filter chats
        if !query.trim().is_empty() {
            // Use simple case-insensitive title matching for responsiveness
            // FTS search is available but can fail on special characters
            let query_lower = query.to_lowercase();
            let all_chats = storage::get_all_chats().unwrap_or_default();
            self.chats = all_chats
                .into_iter()
                .filter(|chat| chat.title.to_lowercase().contains(&query_lower))
                .collect();

            debug!(results = self.chats.len(), "Search filtered chats");

            // Always select first result when filtering
            if !self.chats.is_empty() {
                let first_id = self.chats[0].id;
                if self.selected_chat_id != Some(first_id) {
                    self.selected_chat_id = Some(first_id);
                    // Load messages for the selected chat
                    self.current_messages =
                        storage::get_chat_messages(&first_id).unwrap_or_default();
                }
            } else {
                self.selected_chat_id = None;
                self.current_messages = Vec::new();
            }
        } else {
            // Reload all chats when search is cleared
            self.chats = storage::get_all_chats().unwrap_or_default();
            // Keep current selection if it still exists, otherwise select first
            if let Some(id) = self.selected_chat_id {
                if !self.chats.iter().any(|c| c.id == id) {
                    self.selected_chat_id = self.chats.first().map(|c| c.id);
                    if let Some(new_id) = self.selected_chat_id {
                        self.current_messages =
                            storage::get_chat_messages(&new_id).unwrap_or_default();
                    }
                }
            }
        }

        cx.notify();
    }

    /// Create a new chat
    fn create_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Option<ChatId> {
        // Get model and provider from selected model, or use defaults
        let (model_id, provider) = self
            .selected_model
            .as_ref()
            .map(|m| (m.id.clone(), m.provider.clone()))
            .unwrap_or_else(|| {
                (
                    "claude-3-5-sonnet-20241022".to_string(),
                    "anthropic".to_string(),
                )
            });

        // Create a new chat with selected model
        let chat = Chat::new(&model_id, &provider);
        let id = chat.id;

        // Save to storage
        if let Err(e) = storage::create_chat(&chat) {
            tracing::error!(error = %e, "Failed to create chat");
            return None;
        }

        // Add to cache and select it
        self.chats.insert(0, chat);
        self.select_chat(id, window, cx);

        info!(chat_id = %id, model = model_id, "New chat created");
        Some(id)
    }

    /// Select a chat
    fn select_chat(&mut self, id: ChatId, _window: &mut Window, cx: &mut Context<Self>) {
        self.selected_chat_id = Some(id);

        // Load messages for this chat
        self.current_messages = storage::get_chat_messages(&id).unwrap_or_default();

        // Scroll to bottom to show latest messages
        self.messages_scroll_handle.scroll_to_bottom();

        // Clear any streaming state
        self.is_streaming = false;
        self.streaming_content.clear();

        cx.notify();
    }

    /// Delete the currently selected chat (soft delete)
    fn delete_selected_chat(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.selected_chat_id {
            if let Err(e) = storage::delete_chat(&id) {
                tracing::error!(error = %e, "Failed to delete chat");
                return;
            }

            // Remove from visible list and select next
            self.chats.retain(|c| c.id != id);
            self.selected_chat_id = self.chats.first().map(|c| c.id);

            cx.notify();
        }
    }

    /// Submit the current input as a message
    fn submit_message(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let content = self.input_state.read(cx).value().to_string();

        if content.trim().is_empty() {
            return;
        }

        // Don't allow new messages while streaming
        if self.is_streaming {
            return;
        }

        // If no chat selected, create a new one
        let chat_id = if let Some(id) = self.selected_chat_id {
            id
        } else {
            match self.create_chat(window, cx) {
                Some(id) => id,
                None => {
                    tracing::error!("Failed to create chat for message submission");
                    return;
                }
            }
        };

        // Update chat title if this is the first message
        if let Some(chat) = self.chats.iter_mut().find(|c| c.id == chat_id) {
            if chat.title == "New Chat" {
                let new_title = Chat::generate_title_from_content(&content);
                chat.set_title(&new_title);

                // Persist title update
                if let Err(e) = storage::update_chat_title(&chat_id, &new_title) {
                    tracing::error!(error = %e, "Failed to update chat title");
                }
            }
        }

        // Create and save user message
        let user_message = Message::user(chat_id, &content);
        if let Err(e) = storage::save_message(&user_message) {
            tracing::error!(error = %e, "Failed to save user message");
            return;
        }

        // Add to current messages for display
        self.current_messages.push(user_message);

        // Scroll to bottom to show the new message
        self.messages_scroll_handle.scroll_to_bottom();

        // Update message preview cache
        let preview: String = content.chars().take(60).collect();
        let preview = if preview.len() < content.len() {
            format!("{}...", preview.trim())
        } else {
            preview
        };
        self.message_previews.insert(chat_id, preview);

        // Clear the input
        self.input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });

        info!(
            chat_id = %chat_id,
            content_len = content.len(),
            "User message submitted"
        );

        // Start streaming response
        self.start_streaming_response(chat_id, cx);

        cx.notify();
    }

    /// Start streaming an AI response (or mock response if no providers configured)
    fn start_streaming_response(&mut self, chat_id: ChatId, cx: &mut Context<Self>) {
        // Check if we have a model selected - if not, use mock mode
        let use_mock_mode = self.selected_model.is_none() || self.available_models.is_empty();

        if use_mock_mode {
            info!(chat_id = %chat_id, "No AI providers configured - using mock mode");
            self.start_mock_streaming_response(chat_id, cx);
            return;
        }

        // Get the selected model
        let model = match &self.selected_model {
            Some(m) => m.clone(),
            None => {
                tracing::error!("No model selected for streaming");
                return;
            }
        };

        // Find the provider for this model
        let provider = match self.provider_registry.find_provider_for_model(&model.id) {
            Some(p) => p.clone(),
            None => {
                tracing::error!(model_id = model.id, "No provider found for model");
                return;
            }
        };

        // Build messages for the API call
        let api_messages: Vec<super::providers::ProviderMessage> = self
            .current_messages
            .iter()
            .map(|m| super::providers::ProviderMessage {
                role: m.role.to_string(),
                content: m.content.clone(),
            })
            .collect();

        // Set streaming state
        self.is_streaming = true;
        self.streaming_content.clear();

        info!(
            chat_id = %chat_id,
            model = model.id,
            provider = model.provider,
            message_count = api_messages.len(),
            "Starting AI streaming response"
        );

        // Use a shared buffer for streaming content
        let shared_content = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let shared_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let shared_error = std::sync::Arc::new(std::sync::Mutex::new(None::<String>));

        let model_id = model.id.clone();
        let content_clone = shared_content.clone();
        let done_clone = shared_done.clone();
        let error_clone = shared_error.clone();

        // Spawn background thread for streaming
        std::thread::spawn(move || {
            let result = provider.stream_message(
                &api_messages,
                &model_id,
                Box::new(move |chunk| {
                    if let Ok(mut content) = content_clone.lock() {
                        content.push_str(&chunk);
                    }
                }),
            );

            match result {
                Ok(()) => {
                    done_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                Err(e) => {
                    if let Ok(mut err) = error_clone.lock() {
                        *err = Some(e.to_string());
                    }
                    done_clone.store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });

        // Poll for streaming updates using background executor
        let content_for_poll = shared_content.clone();
        let done_for_poll = shared_done.clone();
        let error_for_poll = shared_error.clone();

        cx.spawn(async move |this, cx| {
            use gpui::Timer;
            loop {
                Timer::after(std::time::Duration::from_millis(50)).await;

                // Check if done or errored
                if done_for_poll.load(std::sync::atomic::Ordering::SeqCst) {
                    // Get final content
                    let final_content = content_for_poll.lock().ok().map(|c| c.clone());
                    let error = error_for_poll.lock().ok().and_then(|e| e.clone());

                    let _ = cx.update(|cx| {
                        this.update(cx, |app, cx| {
                            if let Some(err) = error {
                                tracing::error!(error = err, "Streaming error");
                                app.is_streaming = false;
                                app.streaming_content.clear();
                            } else if let Some(content) = final_content {
                                app.streaming_content = content;
                                app.finish_streaming(chat_id, cx);
                            }
                            cx.notify();
                        })
                    });
                    break;
                }

                // Update with current content
                if let Ok(content) = content_for_poll.lock() {
                    if !content.is_empty() {
                        let current = content.clone();
                        let _ = cx.update(|cx| {
                            this.update(cx, |app, cx| {
                                app.streaming_content = current;
                                // Auto-scroll to bottom as new content arrives
                                app.messages_scroll_handle.scroll_to_bottom();
                                cx.notify();
                            })
                        });
                    }
                }
            }
        })
        .detach();
    }

    /// Start a mock streaming response for testing/demo when no AI providers are configured
    fn start_mock_streaming_response(&mut self, chat_id: ChatId, cx: &mut Context<Self>) {
        // Set streaming state
        self.is_streaming = true;
        self.streaming_content.clear();

        // Get the last user message to generate a contextual mock response
        let user_message = self
            .current_messages
            .last()
            .map(|m| m.content.clone())
            .unwrap_or_default();

        // Generate a mock response based on the user's message
        let mock_response = generate_mock_response(&user_message);

        info!(
            chat_id = %chat_id,
            user_message_len = user_message.len(),
            mock_response_len = mock_response.len(),
            "Starting mock streaming response"
        );

        // Simulate streaming by revealing the response word by word
        let words: Vec<String> = mock_response
            .split_inclusive(char::is_whitespace)
            .map(|s| s.to_string())
            .collect();

        cx.spawn(async move |this, cx| {
            use gpui::Timer;

            let mut accumulated = String::new();
            let mut delay_counter = 0u64;

            for word in words {
                // Vary delay slightly based on word position (30-60ms range)
                delay_counter = delay_counter.wrapping_add(17); // Simple pseudo-variation
                let delay = 30 + (delay_counter % 30);
                Timer::after(std::time::Duration::from_millis(delay)).await;

                accumulated.push_str(&word);

                let current_content = accumulated.clone();
                let _ = cx.update(|cx| {
                    this.update(cx, |app, cx| {
                        app.streaming_content = current_content;
                        // Auto-scroll to bottom as new content arrives
                        app.messages_scroll_handle.scroll_to_bottom();
                        cx.notify();
                    })
                });
            }

            // Small delay before finishing
            Timer::after(std::time::Duration::from_millis(100)).await;

            // Finish streaming
            let _ = cx.update(|cx| {
                this.update(cx, |app, cx| {
                    app.finish_streaming(chat_id, cx);
                })
            });
        })
        .detach();
    }

    /// Finish streaming and save the assistant message
    fn finish_streaming(&mut self, chat_id: ChatId, cx: &mut Context<Self>) {
        if !self.streaming_content.is_empty() {
            // Create and save assistant message
            let assistant_message = Message::assistant(chat_id, &self.streaming_content);
            if let Err(e) = storage::save_message(&assistant_message) {
                tracing::error!(error = %e, "Failed to save assistant message");
            }

            // Add to current messages
            self.current_messages.push(assistant_message);

            // Update message preview
            let preview: String = self.streaming_content.chars().take(60).collect();
            let preview = if preview.len() < self.streaming_content.len() {
                format!("{}...", preview.trim())
            } else {
                preview
            };
            self.message_previews.insert(chat_id, preview);

            info!(
                chat_id = %chat_id,
                content_len = self.streaming_content.len(),
                "Streaming response complete"
            );
        }

        self.is_streaming = false;
        self.streaming_content.clear();
        cx.notify();
    }

    /// Get the currently selected chat
    fn get_selected_chat(&self) -> Option<&Chat> {
        self.selected_chat_id
            .and_then(|id| self.chats.iter().find(|c| c.id == id))
    }

    /// Render the search input
    fn render_search(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        // Fixed height container to prevent layout shift when typing
        div()
            .w_full()
            .h(px(36.)) // Fixed height to prevent layout shift
            .flex()
            .items_center()
            .child(
                Input::new(&self.search_state)
                    .w_full()
                    .small()
                    .focus_bordered(false), // Disable default focus border (too bright)
            )
    }

    /// Toggle sidebar visibility
    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    /// Render the sidebar toggle button using the Sidebar icon from our icon library
    fn render_sidebar_toggle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // Use opacity to indicate state - dimmed when collapsed
        let icon_color = if self.sidebar_collapsed {
            cx.theme().muted_foreground.opacity(0.5)
        } else {
            cx.theme().muted_foreground
        };

        div()
            .id("sidebar-toggle")
            .flex()
            .items_center()
            .justify_center()
            .size(px(24.))
            .rounded_md()
            .cursor_pointer()
            .hover(|s| s.bg(cx.theme().muted.opacity(0.3)))
            .on_mouse_down(
                gpui::MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.toggle_sidebar(cx);
                }),
            )
            .child(
                svg()
                    .external_path(LocalIconName::Sidebar.external_path())
                    .size(px(16.))
                    .text_color(icon_color),
            )
    }

    /// Render the chats sidebar with date groupings
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // If sidebar is collapsed, just show a thin strip with toggle button
        if self.sidebar_collapsed {
            return div()
                .flex()
                .flex_col()
                .w(px(48.))
                .h_full()
                .bg(cx.theme().sidebar)
                .border_r_1()
                .border_color(cx.theme().sidebar_border)
                .items_center()
                // Top row - aligned with traffic lights (h=28px to match window chrome)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .w_full()
                        .h(px(28.))
                        .px_2()
                        .child(self.render_sidebar_toggle(cx)),
                )
                // New chat button below
                .child(
                    div().pt_1().child(
                        Button::new("new-chat-collapsed")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.create_chat(window, cx);
                            })),
                    ),
                )
                .into_any_element();
        }

        let selected_id = self.selected_chat_id;
        let date_groups = group_chats_by_date(&self.chats);

        // Build a custom sidebar with date groupings using divs
        // This gives us more control over the layout than SidebarGroup
        div()
            .flex()
            .flex_col()
            .w(px(240.))
            .h_full()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().sidebar_border)
            // Top row - sidebar toggle aligned with traffic lights (right side of that row)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_end() // Push to right side (traffic lights are on left)
                    .w_full()
                    .h(px(28.)) // Match traffic light row height
                    .px_2()
                    .child(self.render_sidebar_toggle(cx)),
            )
            // Header with new chat button and search
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w_full()
                    .px_2()
                    .pb_2()
                    .gap_2()
                    // New chat button row
                    .child(
                        div().flex().items_center().justify_end().w_full().child(
                            Button::new("new-chat")
                                .ghost()
                                .xsmall()
                                .icon(IconName::Plus)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_chat(window, cx);
                                })),
                        ),
                    )
                    .child(self.render_search(cx)),
            )
            // Scrollable chat list with date groups
            // Note: overflow_y_scrollbar() wraps the element in a Scrollable container
            // that uses size_full(), so flex_1() goes on the wrapper, not the inner content
            .child(
                div()
                    .flex()
                    .flex_col()
                    .px_2()
                    .pb_2()
                    .gap_3()
                    .children(date_groups.into_iter().map(|(group, chats)| {
                        self.render_date_group(group, chats, selected_id, cx)
                    }))
                    .overflow_y_scrollbar()
                    .flex_1(),
            )
            .into_any_element()
    }

    /// Render a date group section (Today, Yesterday, This Week, Older)
    fn render_date_group(
        &self,
        group: DateGroup,
        chats: Vec<&Chat>,
        selected_id: Option<ChatId>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .w_full()
            .gap_1()
            // Group header
            .child(
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .px_1()
                    .py_1()
                    .child(group.label()),
            )
            // Chat items
            .children(
                chats
                    .into_iter()
                    .map(|chat| self.render_chat_item(chat, selected_id, cx)),
            )
    }

    /// Render a single chat item with title and preview
    fn render_chat_item(
        &self,
        chat: &Chat,
        selected_id: Option<ChatId>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chat_id = chat.id;
        let is_selected = selected_id == Some(chat_id);

        let title: SharedString = if chat.title.is_empty() {
            "New Chat".into()
        } else {
            chat.title.clone().into()
        };

        let preview = self.message_previews.get(&chat_id).cloned();

        // Create a custom chat item with title and preview
        div()
            .id(SharedString::from(format!("chat-{}", chat_id)))
            .flex()
            .flex_col()
            .w_full()
            .px_2()
            .py_1()
            .rounded_md()
            .cursor_pointer()
            .when(is_selected, |d| d.bg(cx.theme().sidebar_accent))
            .when(!is_selected, |d| {
                d.hover(|d| d.bg(cx.theme().sidebar_accent.opacity(0.5)))
            })
            .on_click(cx.listener(move |this, _, window, cx| {
                this.select_chat(chat_id, window, cx);
            }))
            .child(
                // Title
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(cx.theme().sidebar_foreground)
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(title),
            )
            .when_some(preview, |d, preview_text| {
                // Clean up preview: skip markdown headings, find actual content
                let clean_preview: String = preview_text
                    .lines()
                    .map(|line| line.trim())
                    .find(|line| {
                        // Skip empty lines
                        !line.is_empty()
                        // Skip markdown headings
                        && !line.starts_with('#')
                        // Skip code fence markers
                        && !line.starts_with("```")
                        // Skip horizontal rules
                        && !line.chars().all(|c| c == '-' || c == '*' || c == '_')
                    })
                    .unwrap_or("")
                    .chars()
                    .take(50)
                    .collect();

                d.child(
                    // Preview (muted, smaller text, single line only)
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(clean_preview),
                )
            })
    }

    /// Render the model picker button
    /// Clicking cycles to the next model; shows current model name
    fn render_model_picker(&self, cx: &mut Context<Self>) -> impl IntoElement {
        if self.available_models.is_empty() {
            // No models available - show message
            return div()
                .flex()
                .items_center()
                .px_2()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("No AI providers configured")
                .into_any_element();
        }

        // Get current model display name
        let model_label: SharedString = self
            .selected_model
            .as_ref()
            .map(|m| m.display_name.clone())
            .unwrap_or_else(|| "Select Model".to_string())
            .into();

        // Model picker button - clicking cycles through models
        Button::new("model-picker")
            .ghost()
            .xsmall()
            .icon(IconName::ChevronDown)
            .child(model_label)
            .on_click(cx.listener(|this, _, _window, cx| {
                this.cycle_model(cx);
            }))
            .into_any_element()
    }

    /// Cycle to the next model in the list
    fn cycle_model(&mut self, cx: &mut Context<Self>) {
        if self.available_models.is_empty() {
            return;
        }

        // Find current index
        let current_idx = self
            .selected_model
            .as_ref()
            .and_then(|sm| self.available_models.iter().position(|m| m.id == sm.id))
            .unwrap_or(0);

        // Cycle to next
        let next_idx = (current_idx + 1) % self.available_models.len();
        self.on_model_change(next_idx, cx);
    }

    /// Render the welcome state (no chat selected or empty chat)
    fn render_welcome(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_1()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .text_color(cx.theme().foreground)
                    .child("Ask Anything"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("Start a conversation with AI"),
            )
    }

    /// Render a single message bubble
    fn render_message(&self, message: &Message, cx: &mut Context<Self>) -> impl IntoElement {
        let is_user = message.role == MessageRole::User;

        div()
            .flex()
            .flex_col()
            .w_full()
            .mb_3()
            .child(
                // Role label
                div()
                    .text_xs()
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(cx.theme().muted_foreground)
                    .mb_1()
                    .child(if is_user { "You" } else { "Assistant" }),
            )
            .child(
                // Message content
                div()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .when(is_user, |d| d.bg(cx.theme().secondary))
                    .when(!is_user, |d| d.bg(cx.theme().muted.opacity(0.3)))
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().foreground)
                            .child(message.content.clone()),
                    ),
            )
    }

    /// Render streaming content (assistant response in progress)
    fn render_streaming_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w_full()
            .mb_3()
            .child(
                // Role label with streaming indicator
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .mb_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(cx.theme().muted_foreground)
                            .child("Assistant"),
                    )
                    .child(
                        // Streaming indicator
                        div().text_xs().text_color(cx.theme().accent).child("●"),
                    ),
            )
            .child(
                // Streaming content
                div()
                    .w_full()
                    .p_3()
                    .rounded_md()
                    .bg(cx.theme().muted.opacity(0.3))
                    .child(div().text_sm().text_color(cx.theme().foreground).child(
                        if self.streaming_content.is_empty() {
                            "...".to_string()
                        } else {
                            self.streaming_content.clone()
                        },
                    )),
            )
    }

    /// Render the messages area
    fn render_messages(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let streaming_element = if self.is_streaming {
            Some(self.render_streaming_content(cx))
        } else {
            None
        };

        // Messages list with vertical scrollbar
        // Note: The container (in render_main_panel) handles flex_1 for sizing
        // We use size_full() here to fill the bounded container
        div()
            .id("messages-scroll-container")
            .flex()
            .flex_col()
            .p_3()
            .gap_3()
            .size_full()
            // Render all messages
            .children(
                self.current_messages
                    .iter()
                    .map(|msg| self.render_message(msg, cx)),
            )
            // Show streaming content if streaming
            .children(streaming_element)
            .overflow_y_scroll()
            .track_scroll(&self.messages_scroll_handle)
    }

    /// Render the main chat panel
    fn render_main_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let has_selection = self.selected_chat_id.is_some();

        // Build titlebar
        let titlebar = div()
            .id("ai-titlebar")
            .flex()
            .items_center()
            .justify_between()
            .h(px(36.))
            .px_3()
            .bg(cx.theme().title_bar)
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                // Chat title (truncated)
                div()
                    .flex_1()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(
                        self.get_selected_chat()
                            .map(|c| {
                                if c.title.is_empty() {
                                    "New Chat".to_string()
                                } else {
                                    c.title.clone()
                                }
                            })
                            .unwrap_or_else(|| "AI Chat".to_string()),
                    ),
            )
            .when(has_selection, |d| {
                d.child(
                    div().flex().items_center().gap_1().child(
                        Button::new("delete-chat")
                            .ghost()
                            .xsmall()
                            .icon(IconName::Delete)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.delete_selected_chat(cx);
                            })),
                    ),
                )
            });

        // Build input area at bottom - Raycast-style layout:
        // Row 1: [+ icon] [input field with magenta border]
        // Row 2: [Model picker with spinner] ... [Submit ↵] | [Actions ⌘K]

        // Use theme accent color for input border (follows theme)
        let input_border_color = cx.theme().accent;

        let input_area = div()
            .flex()
            .flex_col()
            .w_full()
            .bg(cx.theme().title_bar)
            .px_3()
            .pt_3()
            .pb_2() // Reduced bottom padding
            .gap_2()
            // Input row with + icon and accent border
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .w_full()
                    // Plus button on the left using SVG icon (properly centered)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .size(px(28.))
                            .rounded_full()
                            .border_1()
                            .border_color(cx.theme().muted_foreground.opacity(0.4))
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted.opacity(0.3)))
                            .child(
                                svg()
                                    .external_path(LocalIconName::Plus.external_path())
                                    .size(px(14.))
                                    .text_color(cx.theme().muted_foreground),
                            ),
                    )
                    // Input field with subtle accent border
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .rounded_lg()
                            .border_1()
                            .border_color(input_border_color.opacity(0.6)) // Subtle border
                            .overflow_hidden()
                            .child(
                                Input::new(&self.input_state).w_full().focus_bordered(false), // Disable default focus ring
                            ),
                    ),
            )
            // Bottom row: Model picker left, actions right (reduced padding)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .w_full()
                    .overflow_hidden()
                    // Left side: Model picker with potential spinner
                    .child(self.render_model_picker(cx))
                    // Right side: Submit and Actions as text labels
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .flex_shrink_0()
                            // Submit ↵ - clickable text
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px_2()
                                    .py(px(2.)) // Reduced vertical padding
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().muted.opacity(0.3)))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(|this, _, window, cx| {
                                            this.submit_message(window, cx);
                                        }),
                                    )
                                    .child("Submit ↵"),
                            )
                            // Divider
                            .child(div().w(px(1.)).h(px(16.)).bg(cx.theme().border))
                            // Actions ⌘K - placeholder for future actions menu
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .px_2()
                                    .py(px(2.)) // Reduced vertical padding to match Submit
                                    .rounded_md()
                                    .cursor_pointer()
                                    .hover(|s| s.bg(cx.theme().muted.opacity(0.3)))
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Actions ⌘K"),
                            ),
                    ),
            );

        // Determine what to show in the content area
        let has_messages = !self.current_messages.is_empty() || self.is_streaming;

        // Build main layout
        // Structure: titlebar (fixed) -> content area (flex_1, scrollable) -> input area (fixed)
        div()
            .flex_1()
            .flex()
            .flex_col()
            .h_full()
            .overflow_hidden()
            // Titlebar (fixed height)
            .child(titlebar)
            // Content area - this wrapper gets flex_1 to fill remaining space
            // The scrollable content goes inside this bounded container
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(if has_messages {
                        self.render_messages(cx).into_any_element()
                    } else {
                        self.render_welcome(cx).into_any_element()
                    }),
            )
            // Input area (fixed height, always visible at bottom)
            .child(input_area)
    }

    /// Create box shadows from theme configuration
    /// Uses the same drop_shadow settings as the main window
    fn create_box_shadows(&self) -> Vec<BoxShadow> {
        let theme = crate::theme::load_theme();
        let shadow_config = theme.get_drop_shadow();

        if !shadow_config.enabled {
            return vec![];
        }

        // Convert hex color to HSLA
        let r = ((shadow_config.color >> 16) & 0xFF) as f32 / 255.0;
        let g = ((shadow_config.color >> 8) & 0xFF) as f32 / 255.0;
        let b = (shadow_config.color & 0xFF) as f32 / 255.0;

        // Simple RGB to HSL conversion
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let l = (max + min) / 2.0;

        let (h, s) = if max == min {
            (0.0, 0.0)
        } else {
            let d = max - min;
            let s = if l > 0.5 {
                d / (2.0 - max - min)
            } else {
                d / (max + min)
            };
            let h = if max == r {
                (g - b) / d + if g < b { 6.0 } else { 0.0 }
            } else if max == g {
                (b - r) / d + 2.0
            } else {
                (r - g) / d + 4.0
            };
            (h / 6.0, s)
        };

        vec![BoxShadow {
            color: hsla(h, s, l, shadow_config.opacity),
            offset: point(px(shadow_config.offset_x), px(shadow_config.offset_y)),
            blur_radius: px(shadow_config.blur_radius),
            spread_radius: px(shadow_config.spread_radius),
        }]
    }
}

impl Focusable for AiApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AiApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let box_shadows = self.create_box_shadows();

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(cx.theme().background)
            .shadow(box_shadows)
            .text_color(cx.theme().foreground)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                // Handle keyboard shortcuts
                let key = event.keystroke.key.as_str();
                let modifiers = &event.keystroke.modifiers;

                // platform modifier = Cmd on macOS, Ctrl on Windows/Linux
                if modifiers.platform {
                    match key {
                        "n" => {
                            this.create_chat(window, cx);
                        }
                        "enter" | "return" => this.submit_message(window, cx),
                        // Cmd+\ to toggle sidebar (like Raycast)
                        "\\" | "backslash" => this.toggle_sidebar(cx),
                        // Cmd+B also toggles sidebar (common convention)
                        "b" => this.toggle_sidebar(cx),
                        _ => {}
                    }
                }
            }))
            .child(self.render_sidebar(cx))
            .child(self.render_main_panel(cx))
    }
}

/// Convert a u32 hex color to Hsla
#[inline]
fn hex_to_hsla(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// Map Script Kit's ColorScheme to gpui-component's ThemeColor
///
/// NOTE: We intentionally do NOT apply the user's opacity.* values to theme colors here.
/// The opacity values are for window-level transparency (vibrancy effect),
/// not for making UI elements semi-transparent. UI elements should remain solid.
fn map_scriptkit_to_gpui_theme(sk_theme: &crate::theme::Theme) -> ThemeColor {
    let colors = &sk_theme.colors;

    // Get default dark theme as base and override with Script Kit colors
    let mut theme_color = *ThemeColor::dark();

    // Main background and foreground
    theme_color.background = hex_to_hsla(colors.background.main);
    theme_color.foreground = hex_to_hsla(colors.text.primary);

    // Accent colors (Script Kit yellow/gold)
    theme_color.accent = hex_to_hsla(colors.accent.selected);
    theme_color.accent_foreground = hex_to_hsla(colors.text.primary);

    // Border
    theme_color.border = hex_to_hsla(colors.ui.border);
    theme_color.input = hex_to_hsla(colors.ui.border);

    // List/sidebar colors
    theme_color.list = hex_to_hsla(colors.background.main);
    theme_color.list_active = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_active_border = hex_to_hsla(colors.accent.selected);
    theme_color.list_hover = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.list_even = hex_to_hsla(colors.background.main);
    theme_color.list_head = hex_to_hsla(colors.background.title_bar);

    // Sidebar (use slightly lighter background)
    theme_color.sidebar = hex_to_hsla(colors.background.title_bar);
    theme_color.sidebar_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_border = hex_to_hsla(colors.ui.border);
    theme_color.sidebar_accent = hex_to_hsla(colors.accent.selected_subtle);
    theme_color.sidebar_accent_foreground = hex_to_hsla(colors.text.primary);
    theme_color.sidebar_primary = hex_to_hsla(colors.accent.selected);
    theme_color.sidebar_primary_foreground = hex_to_hsla(colors.text.primary);

    // Primary (accent-colored buttons) - yellow/gold background with dark text/icons
    // Use explicit black Hsla for foreground to ensure icon visibility against yellow
    theme_color.primary = hex_to_hsla(colors.accent.selected);
    // Black with full opacity - using Hsla directly since rgb(0x000000).into() may have issues
    theme_color.primary_foreground = Hsla {
        h: 0.0,
        s: 0.0,
        l: 0.0, // Black (0% lightness)
        a: 1.0, // Full opacity
    };
    theme_color.primary_hover = hex_to_hsla(colors.accent.selected).opacity(0.9);
    theme_color.primary_active = hex_to_hsla(colors.accent.selected).opacity(0.8);

    // Secondary (muted buttons)
    theme_color.secondary = hex_to_hsla(colors.background.search_box);
    theme_color.secondary_foreground = hex_to_hsla(colors.text.primary);
    theme_color.secondary_hover = hex_to_hsla(colors.background.title_bar);
    theme_color.secondary_active = hex_to_hsla(colors.background.title_bar);

    // Muted (disabled states, subtle elements)
    theme_color.muted = hex_to_hsla(colors.background.search_box);
    theme_color.muted_foreground = hex_to_hsla(colors.text.muted);

    // Title bar
    theme_color.title_bar = hex_to_hsla(colors.background.title_bar);
    theme_color.title_bar_border = hex_to_hsla(colors.ui.border);

    // Popover
    theme_color.popover = hex_to_hsla(colors.background.main);
    theme_color.popover_foreground = hex_to_hsla(colors.text.primary);

    // Status colors
    theme_color.success = hex_to_hsla(colors.ui.success);
    theme_color.success_foreground = hex_to_hsla(colors.text.primary);
    theme_color.danger = hex_to_hsla(colors.ui.error);
    theme_color.danger_foreground = hex_to_hsla(colors.text.primary);
    theme_color.warning = hex_to_hsla(colors.ui.warning);
    theme_color.warning_foreground = hex_to_hsla(colors.text.primary);
    theme_color.info = hex_to_hsla(colors.ui.info);
    theme_color.info_foreground = hex_to_hsla(colors.text.primary);

    // Scrollbar
    theme_color.scrollbar = hex_to_hsla(colors.background.main);
    theme_color.scrollbar_thumb = hex_to_hsla(colors.text.dimmed);
    theme_color.scrollbar_thumb_hover = hex_to_hsla(colors.text.muted);

    // Caret (cursor) - cyan by default
    theme_color.caret = hex_to_hsla(0x00ffff);

    // Selection
    theme_color.selection = hex_to_hsla(colors.accent.selected_subtle);

    // Ring (focus ring) - use a more subtle version of the accent color
    // The full accent is too bright for focus borders
    let mut ring_color = hex_to_hsla(colors.accent.selected);
    ring_color.a = 0.5; // 50% opacity for a subtler focus ring
    theme_color.ring = ring_color;

    // Tab colors
    theme_color.tab = hex_to_hsla(colors.background.main);
    theme_color.tab_active = hex_to_hsla(colors.background.search_box);
    theme_color.tab_active_foreground = hex_to_hsla(colors.text.primary);
    theme_color.tab_foreground = hex_to_hsla(colors.text.secondary);
    theme_color.tab_bar = hex_to_hsla(colors.background.title_bar);

    debug!(
        background = format!("#{:06x}", colors.background.main),
        accent = format!("#{:06x}", colors.accent.selected),
        "Script Kit theme mapped to gpui-component (AI window)"
    );

    theme_color
}

/// Initialize gpui-component theme and sync with Script Kit theme
fn ensure_theme_initialized(cx: &mut App) {
    // First, initialize gpui-component (this sets up the default theme)
    gpui_component::init(cx);

    // Load Script Kit's theme
    let sk_theme = crate::theme::load_theme();

    // Map Script Kit colors to gpui-component ThemeColor
    let custom_colors = map_scriptkit_to_gpui_theme(&sk_theme);

    // Apply the custom colors to the global theme
    let theme = GpuiTheme::global_mut(cx);
    theme.colors = custom_colors;
    theme.mode = ThemeMode::Dark; // Script Kit uses dark mode by default

    info!("AI window theme synchronized with Script Kit");
}

/// Toggle the AI window (open if closed, bring to front if open)
///
/// The AI window behaves as a NORMAL window (not a floating panel):
/// - Can go behind other windows when it loses focus
/// - Hotkey brings it to front and focuses it
/// - Does NOT affect other windows (main window, notes window)
/// - Does NOT hide the app when closed
pub fn open_ai_window(cx: &mut App) -> Result<()> {
    use crate::logging;

    logging::log("AI", "open_ai_window called - checking state");

    // Ensure gpui-component theme is initialized before opening window
    ensure_theme_initialized(cx);

    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    // Check if window already exists and is valid
    if let Some(ref handle) = *guard {
        // Window exists - check if it's valid
        let window_valid = handle
            .update(cx, |_root, window, _cx| {
                // Window is valid - bring it to front and focus it
                window.activate_window();
            })
            .is_ok();

        if window_valid {
            logging::log("AI", "AI window exists - bringing to front and focusing");
            // Activate the app to ensure the window can receive focus
            cx.activate(true);
            return Ok(());
        }

        // Window handle was invalid, fall through to create new window
        logging::log("AI", "AI window handle was invalid - creating new");
        *guard = None;
    }

    // Create new window
    logging::log("AI", "Creating new AI window");
    info!("Opening new AI window");

    // Load theme to determine window background appearance (vibrancy)
    let theme = crate::theme::load_theme();
    let window_background = if theme.is_vibrancy_enabled() {
        gpui::WindowBackgroundAppearance::Blurred
    } else {
        gpui::WindowBackgroundAppearance::Opaque
    };

    let window_options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(gpui::Bounds::centered(
            None,
            size(px(900.), px(700.)),
            cx,
        ))),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Script Kit AI".into()),
            appears_transparent: true,
            ..Default::default()
        }),
        window_background,
        focus: true,
        show: true,
        // IMPORTANT: Use Normal window kind (not PopUp) so it behaves like a regular window
        // This allows it to go behind other windows and participate in normal window ordering
        kind: gpui::WindowKind::Normal,
        ..Default::default()
    };

    // Create a holder for the AiApp entity so we can store it
    let ai_app_holder: std::sync::Arc<std::sync::Mutex<Option<Entity<AiApp>>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    let ai_app_holder_clone = ai_app_holder.clone();

    let handle = cx.open_window(window_options, |window, cx| {
        let view = cx.new(|cx| AiApp::new(window, cx));
        // Store the AiApp entity for later access
        *ai_app_holder_clone.lock().unwrap() = Some(view.clone());
        cx.new(|cx| Root::new(view, window, cx))
    })?;

    // Store the AiApp entity globally
    let app_entity_holder = AI_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
    *app_entity_holder.lock().unwrap() = ai_app_holder.lock().unwrap().take();

    // Activate the app and window so user can immediately start typing
    cx.activate(true);
    let _ = handle.update(cx, |_root, window, _cx| {
        window.activate_window();
    });

    *guard = Some(handle);

    // NOTE: We do NOT configure as floating panel - this is a normal window
    // that can go behind other windows

    // Theme hot-reload watcher for AI window
    // Spawns a background task that watches ~/.sk/kit/theme.json for changes
    let app_entity_holder_ref = AI_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
    if let Some(ai_app) = app_entity_holder_ref.lock().unwrap().clone() {
        let ai_app_for_theme = ai_app.clone();
        cx.spawn(async move |cx: &mut gpui::AsyncApp| {
            let (mut theme_watcher, theme_rx) = ThemeWatcher::new();
            if theme_watcher.start().is_err() {
                return;
            }
            loop {
                gpui::Timer::after(std::time::Duration::from_millis(200)).await;
                if theme_rx.try_recv().is_ok() {
                    info!("AI window: theme.json changed, reloading");
                    let _ = cx.update(|cx| {
                        // Re-sync gpui-component theme with updated Script Kit theme
                        crate::theme::sync_gpui_component_theme(cx);
                        // Notify the AI window to re-render with new colors
                        ai_app_for_theme.update(cx, |_app, cx| {
                            cx.notify();
                        });
                    });
                }
            }
        })
        .detach();
    }

    Ok(())
}

/// Close the AI window
pub fn close_ai_window(cx: &mut App) {
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let mut guard = window_handle.lock().unwrap();

    if let Some(handle) = guard.take() {
        let _ = handle.update(cx, |_, window, _| {
            window.remove_window();
        });
    }

    // Also clear the AiApp entity reference
    let app_entity_holder = AI_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
    *app_entity_holder.lock().unwrap() = None;
}

/// Check if the AI window is currently open
///
/// Returns true if the AI window exists and is valid.
/// This is used by other parts of the app to check if AI is open
/// without affecting it.
pub fn is_ai_window_open() -> bool {
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let guard = window_handle.lock().unwrap();
    guard.is_some()
}

/// Set the search filter text in the AI window.
/// Used for testing the search functionality via stdin commands.
pub fn set_ai_search(cx: &mut App, query: &str) {
    use crate::logging;

    // Get the AI window handle for the window context
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let window_guard = window_handle.lock().unwrap();

    // Get the AiApp entity
    let app_entity_holder = AI_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
    let app_guard = app_entity_holder.lock().unwrap();

    if let (Some(handle), Some(app_entity)) = (window_guard.as_ref(), app_guard.as_ref()) {
        let query_owned = query.to_string();
        let app_entity_clone = app_entity.clone();

        let _ = handle.update(cx, |_root, window, cx| {
            app_entity_clone.update(cx, |app, cx| {
                // Set the search input value
                app.search_state.update(cx, |state, cx| {
                    state.set_value(query_owned.clone(), window, cx);
                });
                // Trigger the search change handler
                app.on_search_change(cx);
                logging::log("AI", &format!("Search filter set to: {}", query_owned));
            });
        });
    } else {
        logging::log("AI", "Cannot set search - AI window not open");
    }
}

/// Set the main input text in the AI window and optionally submit.
/// Used for testing the streaming functionality via stdin commands.
pub fn set_ai_input(cx: &mut App, text: &str, submit: bool) {
    use crate::logging;

    // Get the AI window handle for the window context
    let window_handle = AI_WINDOW.get_or_init(|| std::sync::Mutex::new(None));
    let window_guard = window_handle.lock().unwrap();

    // Get the AiApp entity
    let app_entity_holder = AI_APP_ENTITY.get_or_init(|| std::sync::Mutex::new(None));
    let app_guard = app_entity_holder.lock().unwrap();

    if let (Some(handle), Some(app_entity)) = (window_guard.as_ref(), app_guard.as_ref()) {
        let text_owned = text.to_string();
        let app_entity_clone = app_entity.clone();

        let _ = handle.update(cx, |_root, window, cx| {
            app_entity_clone.update(cx, |app, cx| {
                // Set the input value
                app.input_state.update(cx, |state, cx| {
                    state.set_value(text_owned.clone(), window, cx);
                });
                logging::log("AI", &format!("Input set to: {}", text_owned));

                // Optionally submit the message (triggers streaming)
                if submit {
                    app.submit_message(window, cx);
                    logging::log("AI", "Message submitted - streaming started");
                }
            });
        });
    } else {
        logging::log("AI", "Cannot set input - AI window not open");
    }
}

/// Configure the AI window as a floating panel (always on top).
///
/// This sets:
/// - NSFloatingWindowLevel (3) - floats above normal windows
/// - NSWindowCollectionBehaviorMoveToActiveSpace - moves to current space when shown
/// - Disabled window restoration - prevents macOS position caching
#[cfg(target_os = "macos")]
fn configure_ai_as_floating_panel() {
    use crate::logging;
    use std::ffi::CStr;

    unsafe {
        let app: id = NSApp();
        let windows: id = msg_send![app, windows];
        let count: usize = msg_send![windows, count];

        for i in 0..count {
            let window: id = msg_send![windows, objectAtIndex: i];
            let title: id = msg_send![window, title];

            if title != nil {
                let title_cstr: *const i8 = msg_send![title, UTF8String];
                if !title_cstr.is_null() {
                    let title_str = CStr::from_ptr(title_cstr).to_string_lossy();

                    if title_str == "Script Kit AI" {
                        // Found the AI window - configure it

                        // NSFloatingWindowLevel = 3
                        let floating_level: i32 = 3;
                        let _: () = msg_send![window, setLevel:floating_level];

                        // NSWindowCollectionBehaviorMoveToActiveSpace = 2
                        let collection_behavior: u64 = 2;
                        let _: () = msg_send![window, setCollectionBehavior:collection_behavior];

                        // Disable window restoration
                        let _: () = msg_send![window, setRestorable:false];

                        logging::log(
                            "PANEL",
                            "AI window configured as floating panel (level=3, MoveToActiveSpace)",
                        );
                        return;
                    }
                }
            }
        }

        logging::log(
            "PANEL",
            "Warning: AI window not found by title for floating panel config",
        );
    }
}

#[cfg(not(target_os = "macos"))]
fn configure_ai_as_floating_panel() {
    // No-op on non-macOS platforms
}
