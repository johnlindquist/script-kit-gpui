//! AI provider abstraction layer.
//!
//! This module provides a trait-based abstraction for AI providers, allowing
//! Script Kit to work with multiple AI services (OpenAI, Anthropic, etc.) through
//! a unified interface.
//!
//! # Architecture
//!
//! - `AiProvider` trait defines the interface all providers must implement
//! - `ProviderRegistry` manages available providers based on detected API keys
//! - Individual provider implementations (OpenAI, Anthropic, etc.) implement the trait
//!

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;

use super::config::{default_models, DetectedKeys, ModelInfo, ProviderConfig};

/// Message for AI provider API calls.
#[derive(Debug, Clone)]
pub struct ProviderMessage {
    /// Role of the message sender: "user", "assistant", or "system"
    pub role: String,
    /// Content of the message
    pub content: String,
}

impl ProviderMessage {
    /// Create a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
        }
    }

    /// Create a new system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
        }
    }
}

/// Callback type for streaming responses.
pub type StreamCallback = Box<dyn Fn(String) + Send + Sync>;

/// Trait defining the interface for AI providers.
///
/// All AI providers (OpenAI, Anthropic, etc.) implement this trait to provide
/// a consistent interface for the AI window.
///
/// # Note on Async
///
/// Currently methods are synchronous for simplicity. When real HTTP integration
/// is added, these will become async using the `async_trait` crate.
pub trait AiProvider: Send + Sync {
    /// Unique identifier for this provider (e.g., "openai", "anthropic").
    fn provider_id(&self) -> &str;

    /// Human-readable display name (e.g., "OpenAI", "Anthropic").
    fn display_name(&self) -> &str;

    /// Get the list of available models for this provider.
    fn available_models(&self) -> Vec<ModelInfo>;

    /// Send a message and get a response (non-streaming).
    ///
    /// # Arguments
    ///
    /// * `messages` - The conversation history
    /// * `model_id` - The model to use for generation
    ///
    /// # Returns
    ///
    /// The generated response text, or an error.
    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String>;

    /// Send a message with streaming response.
    ///
    /// # Arguments
    ///
    /// * `messages` - The conversation history
    /// * `model_id` - The model to use for generation
    /// * `on_chunk` - Callback invoked for each chunk of the response
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error.
    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
    ) -> Result<()>;
}

/// OpenAI provider implementation with real API calls.
pub struct OpenAiProvider {
    config: ProviderConfig,
}

/// OpenAI API constants
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

impl OpenAiProvider {
    /// Create a new OpenAI provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("openai", "OpenAI", api_key),
        }
    }

    /// Create with a custom base URL (for Azure OpenAI or proxies).
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("openai", "OpenAI", api_key).with_base_url(base_url),
        }
    }

    /// Get the API URL (uses custom base_url if set)
    fn api_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(OPENAI_API_URL)
    }

    /// Build the request body for OpenAI API
    fn build_request_body(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        stream: bool,
    ) -> serde_json::Value {
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        serde_json::json!({
            "model": model_id,
            "stream": stream,
            "messages": api_messages
        })
    }

    /// Parse an SSE line and extract content delta (OpenAI format)
    fn parse_sse_line(line: &str) -> Option<String> {
        // SSE format: "data: {json}"
        if !line.starts_with("data: ") {
            return None;
        }

        let json_str = &line[6..]; // Skip "data: "

        // Check for stream end
        if json_str == "[DONE]" {
            return None;
        }

        // Parse the JSON
        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;

        // OpenAI streaming format:
        // {"choices": [{"delta": {"content": "..."}}]}
        parsed
            .get("choices")?
            .as_array()?
            .first()?
            .get("delta")?
            .get("content")?
            .as_str()
            .map(|s| s.to_string())
    }
}

impl AiProvider for OpenAiProvider {
    fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    fn display_name(&self) -> &str {
        &self.config.display_name
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        default_models::openai()
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to OpenAI"
        );

        let response = ureq::post(self.api_url())
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                &format!("Bearer {}", self.config.api_key()),
            )
            .send_json(&body)
            .context("Failed to send request to OpenAI API")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse OpenAI response")?;

        // Extract content from response
        // Response format: {"choices": [{"message": {"content": "..."}}]}
        let content = response_json
            .get("choices")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|choice| choice.get("message"))
            .and_then(|msg| msg.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();

        tracing::debug!(
            content_len = content.len(),
            "Received non-streaming response from OpenAI"
        );

        Ok(content)
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
    ) -> Result<()> {
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to OpenAI"
        );

        let response = ureq::post(self.api_url())
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                &format!("Bearer {}", self.config.api_key()),
            )
            .header("Accept", "text/event-stream")
            .send_json(&body)
            .context("Failed to send streaming request to OpenAI API")?;

        // Read the SSE stream
        let reader = BufReader::new(response.into_body().into_reader());

        for line in reader.lines() {
            let line = line.context("Failed to read SSE line")?;

            // Skip empty lines
            if line.is_empty() {
                continue;
            }

            // Parse and extract content delta
            if let Some(text) = Self::parse_sse_line(&line) {
                on_chunk(text);
            }
        }

        tracing::debug!("Completed streaming response from OpenAI");

        Ok(())
    }
}

/// Anthropic provider implementation with real API calls.
pub struct AnthropicProvider {
    config: ProviderConfig,
}

/// Anthropic API constants
const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const DEFAULT_MAX_TOKENS: u32 = 4096;

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("anthropic", "Anthropic", api_key),
        }
    }

    /// Build the request body for Anthropic API
    fn build_request_body(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        stream: bool,
    ) -> serde_json::Value {
        // Separate system message from conversation messages
        let system_msg = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        // Filter out system messages for the messages array
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content
                })
            })
            .collect();

        let mut body = serde_json::json!({
            "model": model_id,
            "max_tokens": DEFAULT_MAX_TOKENS,
            "stream": stream,
            "messages": api_messages
        });

        // Add system message if present
        if let Some(system) = system_msg {
            body["system"] = serde_json::Value::String(system);
        }

        body
    }

    /// Parse an SSE line and extract content delta
    fn parse_sse_line(line: &str) -> Option<String> {
        // SSE format: "data: {json}"
        if !line.starts_with("data: ") {
            return None;
        }

        let json_str = &line[6..]; // Skip "data: "

        // Check for stream end
        if json_str == "[DONE]" {
            return None;
        }

        // Parse the JSON
        let parsed: serde_json::Value = serde_json::from_str(json_str).ok()?;

        // Anthropic streaming format:
        // - content_block_delta events contain: {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "..."}}
        if parsed.get("type")?.as_str()? == "content_block_delta" {
            if let Some(delta) = parsed.get("delta") {
                if delta.get("type")?.as_str()? == "text_delta" {
                    return delta.get("text")?.as_str().map(|s| s.to_string());
                }
            }
        }

        None
    }
}

impl AiProvider for AnthropicProvider {
    fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    fn display_name(&self) -> &str {
        &self.config.display_name
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        default_models::anthropic()
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to Anthropic"
        );

        let response = ureq::post(ANTHROPIC_API_URL)
            .header("Content-Type", "application/json")
            .header("x-api-key", self.config.api_key())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send_json(&body)
            .context("Failed to send request to Anthropic API")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse Anthropic response")?;

        // Extract content from response
        // Response format: {"content": [{"type": "text", "text": "..."}], ...}
        let content = response_json
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or("")
            .to_string();

        tracing::debug!(
            content_len = content.len(),
            "Received non-streaming response from Anthropic"
        );

        Ok(content)
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
    ) -> Result<()> {
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to Anthropic"
        );

        let response = ureq::post(ANTHROPIC_API_URL)
            .header("Content-Type", "application/json")
            .header("x-api-key", self.config.api_key())
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("Accept", "text/event-stream")
            .send_json(&body)
            .context("Failed to send streaming request to Anthropic API")?;

        // Read the SSE stream
        let reader = BufReader::new(response.into_body().into_reader());

        for line in reader.lines() {
            let line = line.context("Failed to read SSE line")?;

            // Skip empty lines (SSE uses blank lines as event separators)
            if line.is_empty() {
                continue;
            }

            // Parse and extract content delta
            if let Some(text) = Self::parse_sse_line(&line) {
                on_chunk(text);
            }
        }

        tracing::debug!("Completed streaming response from Anthropic");

        Ok(())
    }
}

/// Google (Gemini) provider implementation.
pub struct GoogleProvider {
    config: ProviderConfig,
}

impl GoogleProvider {
    /// Create a new Google provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("google", "Google", api_key),
        }
    }
}

impl AiProvider for GoogleProvider {
    fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    fn display_name(&self) -> &str {
        &self.config.display_name
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        default_models::google()
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        let last_user_msg = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("(no message)");

        Ok(format!(
            "[Mock Google Response]\nModel: {}\nProvider: {}\n\nI received your message: \"{}\"",
            model_id,
            self.display_name(),
            last_user_msg
        ))
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
    ) -> Result<()> {
        let response = self.send_message(messages, model_id)?;

        for word in response.split_whitespace() {
            on_chunk(format!("{} ", word));
        }

        Ok(())
    }
}

/// Groq provider implementation.
pub struct GroqProvider {
    config: ProviderConfig,
}

impl GroqProvider {
    /// Create a new Groq provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("groq", "Groq", api_key),
        }
    }
}

impl AiProvider for GroqProvider {
    fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    fn display_name(&self) -> &str {
        &self.config.display_name
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        default_models::groq()
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        let last_user_msg = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("(no message)");

        Ok(format!(
            "[Mock Groq Response]\nModel: {}\nProvider: {}\n\nI received your message: \"{}\"",
            model_id,
            self.display_name(),
            last_user_msg
        ))
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
    ) -> Result<()> {
        let response = self.send_message(messages, model_id)?;

        for word in response.split_whitespace() {
            on_chunk(format!("{} ", word));
        }

        Ok(())
    }
}

/// Registry of available AI providers.
///
/// The registry automatically discovers available providers based on
/// environment variables and provides a unified interface to access them.
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
}

impl ProviderRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Create a registry populated from environment variables.
    ///
    /// Scans for `SCRIPT_KIT_*_API_KEY` environment variables and
    /// creates providers for each detected key.
    pub fn from_environment() -> Self {
        let keys = DetectedKeys::from_environment();
        let mut registry = Self::new();

        if let Some(key) = keys.openai {
            registry.register(Arc::new(OpenAiProvider::new(key)));
        }

        if let Some(key) = keys.anthropic {
            registry.register(Arc::new(AnthropicProvider::new(key)));
        }

        if let Some(key) = keys.google {
            registry.register(Arc::new(GoogleProvider::new(key)));
        }

        if let Some(key) = keys.groq {
            registry.register(Arc::new(GroqProvider::new(key)));
        }

        // Log which providers are available (without exposing keys)
        let available: Vec<_> = registry.providers.keys().collect();
        if !available.is_empty() {
            tracing::info!(
                providers = ?available,
                "AI providers initialized from environment"
            );
        } else {
            tracing::debug!("No AI provider API keys found in environment");
        }

        registry
    }

    /// Register a provider with the registry.
    pub fn register(&mut self, provider: Arc<dyn AiProvider>) {
        self.providers
            .insert(provider.provider_id().to_string(), provider);
    }

    /// Check if any providers are available.
    pub fn has_any_provider(&self) -> bool {
        !self.providers.is_empty()
    }

    /// Get a provider by ID.
    pub fn get_provider(&self, id: &str) -> Option<&Arc<dyn AiProvider>> {
        self.providers.get(id)
    }

    /// Get all registered provider IDs.
    pub fn provider_ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Get all available models from all providers.
    pub fn get_all_models(&self) -> Vec<ModelInfo> {
        let mut models = Vec::new();
        for provider in self.providers.values() {
            models.extend(provider.available_models());
        }
        models
    }

    /// Get models for a specific provider.
    pub fn get_models_for_provider(&self, provider_id: &str) -> Vec<ModelInfo> {
        self.providers
            .get(provider_id)
            .map(|p| p.available_models())
            .unwrap_or_default()
    }

    /// Find the provider that owns a specific model.
    pub fn find_provider_for_model(&self, model_id: &str) -> Option<&Arc<dyn AiProvider>> {
        self.providers
            .values()
            .find(|provider| provider.available_models().iter().any(|m| m.id == model_id))
    }
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_message_constructors() {
        let user = ProviderMessage::user("Hello");
        assert_eq!(user.role, "user");
        assert_eq!(user.content, "Hello");

        let assistant = ProviderMessage::assistant("Hi there");
        assert_eq!(assistant.role, "assistant");
        assert_eq!(assistant.content, "Hi there");

        let system = ProviderMessage::system("You are helpful");
        assert_eq!(system.role, "system");
        assert_eq!(system.content, "You are helpful");
    }

    #[test]
    fn test_openai_provider() {
        let provider = OpenAiProvider::new("test-key");
        assert_eq!(provider.provider_id(), "openai");
        assert_eq!(provider.display_name(), "OpenAI");

        let models = provider.available_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id == "gpt-4o"));
    }

    #[test]
    fn test_anthropic_provider() {
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.provider_id(), "anthropic");
        assert_eq!(provider.display_name(), "Anthropic");

        let models = provider.available_models();
        assert!(!models.is_empty());
    }

    /// Test send_message with real API calls (requires API key)
    /// Run with: cargo test --features system-tests test_send_message_real -- --ignored
    #[test]
    #[ignore = "Requires real API key - run with SCRIPT_KIT_OPENAI_API_KEY set"]
    fn test_send_message_real() {
        let api_key = std::env::var("SCRIPT_KIT_OPENAI_API_KEY")
            .expect("SCRIPT_KIT_OPENAI_API_KEY must be set for this test");
        let provider = OpenAiProvider::new(api_key);
        let messages = vec![
            ProviderMessage::system("You are helpful"),
            ProviderMessage::user("Say hello"),
        ];

        let response = provider.send_message(&messages, "gpt-4o-mini").unwrap();
        assert!(!response.is_empty());
    }

    /// Test stream_message with real API calls (requires API key)
    /// Run with: cargo test --features system-tests test_stream_message_real -- --ignored
    #[test]
    #[ignore = "Requires real API key - run with SCRIPT_KIT_OPENAI_API_KEY set"]
    fn test_stream_message_real() {
        let api_key = std::env::var("SCRIPT_KIT_OPENAI_API_KEY")
            .expect("SCRIPT_KIT_OPENAI_API_KEY must be set for this test");
        let provider = OpenAiProvider::new(api_key);
        let messages = vec![ProviderMessage::user("Say hello")];

        let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();

        provider
            .stream_message(
                &messages,
                "gpt-4o-mini",
                Box::new(move |chunk| {
                    chunks_clone.lock().unwrap().push(chunk);
                }),
            )
            .unwrap();

        let collected = chunks.lock().unwrap();
        assert!(!collected.is_empty());
    }

    #[test]
    fn test_request_body_construction() {
        let provider = OpenAiProvider::new("test-key");
        let messages = vec![
            ProviderMessage::system("You are helpful"),
            ProviderMessage::user("Hello"),
        ];

        let body = provider.build_request_body(&messages, "gpt-4o", false);

        assert_eq!(body["model"], "gpt-4o");
        assert_eq!(body["stream"], false);
        assert!(body["messages"].is_array());
        assert_eq!(body["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_anthropic_request_body_construction() {
        let provider = AnthropicProvider::new("test-key");
        let messages = vec![
            ProviderMessage::system("You are helpful"),
            ProviderMessage::user("Hello"),
        ];

        let body = provider.build_request_body(&messages, "claude-3-5-sonnet-20241022", true);

        assert_eq!(body["model"], "claude-3-5-sonnet-20241022");
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], "You are helpful");
        // Messages array should NOT contain the system message
        assert_eq!(body["messages"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_sse_parsing_openai() {
        // Test OpenAI SSE format
        let line = r#"data: {"choices": [{"delta": {"content": "Hello"}}]}"#;
        let result = OpenAiProvider::parse_sse_line(line);
        assert_eq!(result, Some("Hello".to_string()));

        // Empty delta
        let line = r#"data: {"choices": [{"delta": {}}]}"#;
        let result = OpenAiProvider::parse_sse_line(line);
        assert_eq!(result, None);

        // [DONE] marker
        let line = "data: [DONE]";
        let result = OpenAiProvider::parse_sse_line(line);
        assert_eq!(result, None);

        // Non-data line
        let line = "event: message";
        let result = OpenAiProvider::parse_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_sse_parsing_anthropic() {
        // Test Anthropic SSE format
        let line = r#"data: {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "World"}}"#;
        let result = AnthropicProvider::parse_sse_line(line);
        assert_eq!(result, Some("World".to_string()));

        // Other event types should be ignored
        let line = r#"data: {"type": "message_start", "message": {}}"#;
        let result = AnthropicProvider::parse_sse_line(line);
        assert_eq!(result, None);

        // [DONE] marker
        let line = "data: [DONE]";
        let result = AnthropicProvider::parse_sse_line(line);
        assert_eq!(result, None);
    }

    #[test]
    fn test_registry_empty() {
        let registry = ProviderRegistry::new();
        assert!(!registry.has_any_provider());
        assert!(registry.get_all_models().is_empty());
    }

    #[test]
    fn test_registry_register() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(OpenAiProvider::new("test-key")));

        assert!(registry.has_any_provider());
        assert!(registry.get_provider("openai").is_some());
        assert!(registry.get_provider("anthropic").is_none());
    }

    #[test]
    fn test_registry_get_all_models() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(OpenAiProvider::new("test")));
        registry.register(Arc::new(AnthropicProvider::new("test")));

        let models = registry.get_all_models();
        assert!(models.iter().any(|m| m.provider == "openai"));
        assert!(models.iter().any(|m| m.provider == "anthropic"));
    }

    #[test]
    fn test_registry_find_provider_for_model() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(OpenAiProvider::new("test")));
        registry.register(Arc::new(AnthropicProvider::new("test")));

        let provider = registry.find_provider_for_model("gpt-4o");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), "openai");

        let provider = registry.find_provider_for_model("claude-3-5-sonnet-20241022");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), "anthropic");

        let provider = registry.find_provider_for_model("nonexistent");
        assert!(provider.is_none());
    }
}
