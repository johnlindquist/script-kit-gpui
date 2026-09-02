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

use anyhow::{anyhow, Context, Result};
use itertools::Itertools;
use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::sync::Arc;
use std::time::Duration;

use super::config::{default_models, DetectedKeys, ModelInfo, ProviderConfig};

mod diagnostics;

#[cfg(test)]
use diagnostics::{extract_api_error_message, provider_http_failure_message, simplify_auth_error};
use diagnostics::{handle_http_response, safe_provider_diagnostic_detail};

/// Default timeouts for API requests
const CONNECT_TIMEOUT_SECS: u64 = 10;
const SEND_TIMEOUT_SECS: u64 = 30;
const RESPONSE_TIMEOUT_SECS: u64 = 30;
const READ_TIMEOUT_SECS: u64 = 120;
const GLOBAL_TIMEOUT_SECS: u64 = 180;
const HTTP_MAX_ATTEMPTS: usize = 3;
const HTTP_RETRY_BASE_DELAY_MS: u64 = 250;

fn should_retry_http_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500..=599)
}

fn should_retry_transport_error(error: &ureq::Error) -> bool {
    matches!(
        error,
        ureq::Error::Timeout(_)
            | ureq::Error::Io(_)
            | ureq::Error::HostNotFound
            | ureq::Error::ConnectionFailed
            | ureq::Error::Protocol(_)
            | ureq::Error::BodyStalled
    )
}

fn retry_delay_for_attempt(attempt: usize) -> Duration {
    let exponent = (attempt.saturating_sub(1)).min(5);
    let multiplier = 1_u64 << exponent;
    Duration::from_millis(HTTP_RETRY_BASE_DELAY_MS.saturating_mul(multiplier))
}

fn send_json_with_retry(
    provider_name: &str,
    operation: &str,
    make_request: impl Fn() -> std::result::Result<ureq::http::Response<ureq::Body>, ureq::Error>,
) -> Result<ureq::http::Response<ureq::Body>> {
    crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
    let correlation_id = crate::logging::current_correlation_id();

    for attempt in 1..=HTTP_MAX_ATTEMPTS {
        match make_request() {
            Ok(response) => {
                let status = response.status().as_u16();
                if should_retry_http_status(status) && attempt < HTTP_MAX_ATTEMPTS {
                    let delay = retry_delay_for_attempt(attempt);
                    tracing::warn!(
                        correlation_id = %correlation_id,
                        provider = provider_name,
                        operation = operation,
                        attempt,
                        max_attempts = HTTP_MAX_ATTEMPTS,
                        status,
                        retry_in_ms = delay.as_millis() as u64,
                        "Retrying AI API request after retryable HTTP status"
                    );
                    std::thread::sleep(delay);
                    continue;
                }

                return Ok(response);
            }
            Err(error) => {
                let diagnostic = super::reliability::redact_diagnostic(&error.to_string());
                if should_retry_transport_error(&error) && attempt < HTTP_MAX_ATTEMPTS {
                    let delay = retry_delay_for_attempt(attempt);
                    tracing::warn!(
                        correlation_id = %correlation_id,
                        provider = provider_name,
                        operation = operation,
                        attempt,
                        max_attempts = HTTP_MAX_ATTEMPTS,
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        retry_in_ms = delay.as_millis() as u64,
                        "Retrying AI API request after transient transport error"
                    );
                    std::thread::sleep(delay);
                    continue;
                }

                let safe_detail = diagnostic.copyable_detail.unwrap_or_else(|| {
                    "Provider transport diagnostic details were redacted".to_string()
                });
                return Err(anyhow!(safe_detail)).context(format!(
                    "{} request failed (attempted={} attempt={}/{})",
                    provider_name, operation, attempt, HTTP_MAX_ATTEMPTS
                ));
            }
        }
    }

    Err(anyhow!(
        "{} request failed before sending (attempted={} state=unexpected_retry_exit)",
        provider_name,
        operation
    ))
}

/// Create a ureq::Agent with standard timeouts for API requests.
fn create_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .http_status_as_error(false)
        .https_only(true)
        .timeout_global(Some(Duration::from_secs(GLOBAL_TIMEOUT_SECS)))
        .timeout_connect(Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)))
        .timeout_send_request(Some(Duration::from_secs(SEND_TIMEOUT_SECS)))
        .timeout_send_body(Some(Duration::from_secs(SEND_TIMEOUT_SECS)))
        .timeout_recv_response(Some(Duration::from_secs(RESPONSE_TIMEOUT_SECS)))
        .timeout_recv_body(Some(Duration::from_secs(READ_TIMEOUT_SECS)))
        .build()
        .new_agent()
}

/// Parse SSE (Server-Sent Events) stream and process data lines.
///
/// This helper handles:
/// - CRLF line endings (trims trailing \r)
/// - Multi-line data accumulation
/// - [DONE] termination marker
///
/// # Arguments
///
/// * `reader` - A BufRead implementation (typically from response body)
/// * `on_data` - Callback invoked for each complete data payload; returns true to continue, false to stop
// Maximum size for a single SSE event data buffer (16 MB).
// Prevents unbounded memory growth from malicious or misbehaving servers.
const SSE_MAX_EVENT_SIZE: usize = 16 * 1024 * 1024;

fn stream_sse_lines<R: BufRead>(
    reader: R,
    mut on_data: impl FnMut(&str) -> Result<bool>,
) -> Result<()> {
    let mut data_buf = String::new();

    for line in reader.lines() {
        let mut line = line.context("Failed to read SSE line")?;
        // Handle CRLF endings
        if line.ends_with('\r') {
            line.pop();
        }

        // Blank line: end of event
        if line.is_empty() {
            if data_buf.is_empty() {
                continue;
            }
            if data_buf == "[DONE]" {
                break;
            }

            // on_data returns true to continue, false to stop
            if !on_data(&data_buf)? {
                break;
            }
            data_buf.clear();
            continue;
        }

        // Collect data lines
        if let Some(d) = line.strip_prefix("data: ") {
            if data_buf.len().saturating_add(d.len()) > SSE_MAX_EVENT_SIZE {
                anyhow::bail!(
                    "SSE event exceeded maximum size of {} bytes",
                    SSE_MAX_EVENT_SIZE
                );
            }
            if !data_buf.is_empty() {
                data_buf.push('\n');
            }
            data_buf.push_str(d);
        }
    }
    Ok(())
}

/// Image data for multimodal API calls
#[derive(Debug, Clone)]
pub struct ProviderImage {
    /// Base64 encoded image data
    pub data: String,
    /// MIME type of the image (e.g., "image/png", "image/jpeg")
    pub media_type: String,
}

impl ProviderImage {
    /// Create a new image from base64 data
    pub fn new(data: String, media_type: String) -> Self {
        Self { data, media_type }
    }

    /// Create a PNG image
    pub fn png(data: String) -> Self {
        Self::new(data, "image/png".to_string())
    }

    /// Create a JPEG image
    pub fn jpeg(data: String) -> Self {
        Self::new(data, "image/jpeg".to_string())
    }
}

/// Message for AI provider API calls.
#[derive(Debug, Clone)]
pub struct ProviderMessage {
    /// Role of the message sender: "user", "assistant", or "system"
    pub role: String,
    /// Text content of the message
    pub content: String,
    /// Image attachments for multimodal messages
    pub images: Vec<ProviderImage>,
}

impl ProviderMessage {
    /// Create a new user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            images: Vec::new(),
        }
    }

    /// Create a new user message with images.
    pub fn user_with_images(content: impl Into<String>, images: Vec<ProviderImage>) -> Self {
        Self {
            role: "user".to_string(),
            content: content.into(),
            images,
        }
    }

    /// Create a new assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: "assistant".to_string(),
            content: content.into(),
            images: Vec::new(),
        }
    }

    /// Create a new system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: "system".to_string(),
            content: content.into(),
            images: Vec::new(),
        }
    }

    /// Check if this message has images attached
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }
}

/// Callback type for streaming responses.
pub type StreamCallback = Box<dyn Fn(String) -> bool + Send + Sync>;

fn forward_unstreamed_claude_response(
    response: &str,
    streamed_chunk_count: usize,
    on_chunk: &StreamCallback,
) -> bool {
    streamed_chunk_count > 0 || on_chunk(response.to_string())
}

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
    /// * `session_id` - Optional session ID for conversation continuity (used by Claude Code CLI)
    ///
    /// # Returns
    ///
    /// Ok(()) on success, or an error.
    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
        session_id: Option<&str>,
    ) -> Result<()>;
}

/// OpenAI provider implementation with real API calls.
pub struct OpenAiProvider {
    config: ProviderConfig,
    agent: ureq::Agent,
}

/// OpenAI API constants
const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

impl OpenAiProvider {
    /// Create a new OpenAI provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("openai", "OpenAI", api_key),
            agent: create_agent(),
        }
    }

    /// Create with a custom base URL (for Azure OpenAI or proxies).
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("openai", "OpenAI", api_key).with_base_url(base_url),
            agent: create_agent(),
        }
    }

    /// Get the API URL (uses custom base_url if set)
    fn api_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(OPENAI_API_URL)
    }

    /// Build the request body for OpenAI API
    ///
    /// Supports multimodal messages with images using OpenAI's content array format:
    /// ```json
    /// {
    ///   "role": "user",
    ///   "content": [
    ///     {"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}},
    ///     {"type": "text", "text": "What's in this image?"}
    ///   ]
    /// }
    /// ```
    fn build_request_body(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        stream: bool,
    ) -> serde_json::Value {
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                // If message has images, use content array format
                if m.has_images() {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                    // Add images (OpenAI uses data URL format)
                    for img in &m.images {
                        let data_url = format!("data:{};base64,{}", img.media_type, img.data);
                        content_blocks.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": data_url
                            }
                        }));
                    }

                    // Add text content if not empty
                    if !m.content.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": m.content
                        }));
                    }

                    serde_json::json!({
                        "role": m.role,
                        "content": content_blocks
                    })
                } else {
                    // Text-only message (simpler format)
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content
                    })
                }
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

        let json_str = &line["data: ".len()..]; // Skip "data: "

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
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to OpenAI"
        );

        let response = send_json_with_retry("OpenAI", "send_message", || {
            self.agent
                .post(self.api_url())
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .send_json(&body)
        })
        .context("Network error connecting to OpenAI")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "OpenAI")?;

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
        _session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to OpenAI"
        );

        let response = send_json_with_retry("OpenAI", "stream_message", || {
            self.agent
                .post(self.api_url())
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .header("Accept", "text/event-stream")
                .send_json(&body)
        })
        .context("Network error connecting to OpenAI")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "OpenAI")?;

        // Read the SSE stream using the helper
        let reader = BufReader::new(response.into_body().into_reader());

        stream_sse_lines(reader, |data| {
            // Parse OpenAI streaming format
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|choice| choice.get("delta"))
                    .and_then(|delta| delta.get("content"))
                    .and_then(|c| c.as_str())
                {
                    if !on_chunk(content.to_string()) {
                        return Ok(false);
                    }
                }
            }
            Ok(true) // continue processing
        })?;

        tracing::debug!("Completed streaming response from OpenAI");

        Ok(())
    }
}

/// Anthropic provider implementation with real API calls.
pub struct AnthropicProvider {
    config: ProviderConfig,
    agent: ureq::Agent,
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
            agent: create_agent(),
        }
    }

    /// Create with a custom base URL (for proxies).
    pub fn with_base_url(api_key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("anthropic", "Anthropic", api_key).with_base_url(base_url),
            agent: create_agent(),
        }
    }

    /// Get the API URL (uses custom base_url if set)
    fn api_url(&self) -> &str {
        self.config.base_url.as_deref().unwrap_or(ANTHROPIC_API_URL)
    }

    /// Build the request body for Anthropic API
    ///
    /// Supports multimodal messages with images using Anthropic's content array format:
    /// ```json
    /// {
    ///   "role": "user",
    ///   "content": [
    ///     {"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": "..."}},
    ///     {"type": "text", "text": "What's in this image?"}
    ///   ]
    /// }
    /// ```
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

        // Filter out system messages and build multimodal content for the messages array
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                // If message has images, use content array format
                if m.has_images() {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                    // Add images first (Anthropic recommends images before text)
                    for img in &m.images {
                        content_blocks.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": img.media_type,
                                "data": img.data
                            }
                        }));
                    }

                    // Add text content if not empty
                    if !m.content.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": m.content
                        }));
                    }

                    serde_json::json!({
                        "role": m.role,
                        "content": content_blocks
                    })
                } else {
                    // Text-only message (simpler format)
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content
                    })
                }
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

        let json_str = &line["data: ".len()..]; // Skip "data: "

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
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to Anthropic"
        );

        let response = send_json_with_retry("Anthropic", "send_message", || {
            self.agent
                .post(self.api_url())
                .header("Content-Type", "application/json")
                .header("x-api-key", self.config.api_key())
                .header("anthropic-version", ANTHROPIC_VERSION)
                .send_json(&body)
        })
        .context("Network error connecting to Anthropic")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "Anthropic")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse Anthropic response")?;

        // Extract content from response - join ALL content blocks, not just first
        // Response format: {"content": [{"type": "text", "text": "..."}, ...], ...}
        let content = response_json
            .get("content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .join("")
            })
            .unwrap_or_default();

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
        _session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to Anthropic"
        );

        let response = send_json_with_retry("Anthropic", "stream_message", || {
            self.agent
                .post(self.api_url())
                .header("Content-Type", "application/json")
                .header("x-api-key", self.config.api_key())
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("Accept", "text/event-stream")
                .send_json(&body)
        })
        .context("Network error connecting to Anthropic")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "Anthropic")?;

        // Read the SSE stream using the helper
        let reader = BufReader::new(response.into_body().into_reader());

        stream_sse_lines(reader, |data| {
            // Parse Anthropic streaming format
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                // Anthropic streaming format:
                // content_block_delta events: {"type": "content_block_delta", "delta": {"type": "text_delta", "text": "..."}}
                if parsed.get("type").and_then(|t| t.as_str()) == Some("content_block_delta") {
                    if let Some(delta) = parsed.get("delta") {
                        if delta.get("type").and_then(|t| t.as_str()) == Some("text_delta") {
                            if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                if !on_chunk(text.to_string()) {
                                    return Ok(false);
                                }
                            }
                        }
                    }
                }
            }
            Ok(true) // continue processing
        })?;

        tracing::debug!("Completed streaming response from Anthropic");

        Ok(())
    }
}

/// Google (Gemini) provider implementation with real API calls.
///
/// Uses the Gemini `streamGenerateContent` endpoint for streaming and
/// `generateContent` for non-streaming requests via the `generativelanguage.googleapis.com` API.
pub struct GoogleProvider {
    config: ProviderConfig,
    agent: ureq::Agent,
}

/// Google Gemini API constants
const GOOGLE_API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";

impl GoogleProvider {
    /// Create a new Google provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("google", "Google Gemini", api_key),
            agent: create_agent(),
        }
    }

    /// Build the non-streaming API URL for a given model.
    fn api_url(&self, model_id: &str) -> String {
        format!("{}/{}:generateContent", GOOGLE_API_BASE, model_id)
    }

    /// Build the streaming API URL for a given model.
    fn stream_api_url(&self, model_id: &str) -> String {
        format!(
            "{}/{}:streamGenerateContent?alt=sse",
            GOOGLE_API_BASE, model_id
        )
    }

    /// Build the request body for Gemini API.
    ///
    /// Gemini uses a different message format from OpenAI:
    /// ```json
    /// {
    ///   "contents": [
    ///     {"role": "user", "parts": [{"text": "Hello"}]},
    ///     {"role": "model", "parts": [{"text": "Hi there!"}]}
    ///   ],
    ///   "systemInstruction": {"parts": [{"text": "You are helpful."}]}
    /// }
    /// ```
    fn build_request_body(&self, messages: &[ProviderMessage]) -> serde_json::Value {
        // Extract system message
        let system_msg = messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone());

        // Convert messages to Gemini format (skip system messages)
        let contents: Vec<serde_json::Value> = messages
            .iter()
            .filter(|m| m.role != "system")
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };

                let mut parts: Vec<serde_json::Value> = Vec::new();

                // Add images as inline_data parts
                for img in &m.images {
                    parts.push(serde_json::json!({
                        "inline_data": {
                            "mime_type": img.media_type,
                            "data": img.data
                        }
                    }));
                }

                // Add text part
                if !m.content.is_empty() {
                    parts.push(serde_json::json!({"text": m.content}));
                }

                serde_json::json!({
                    "role": role,
                    "parts": parts
                })
            })
            .collect();

        let mut body = serde_json::json!({"contents": contents});

        // Add system instruction if present
        if let Some(system) = system_msg {
            body["systemInstruction"] = serde_json::json!({
                "parts": [{"text": system}]
            });
        }

        body
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
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages);
        let url = self.api_url(model_id);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to Google Gemini"
        );

        let response = send_json_with_retry("Google Gemini", "send_message", || {
            self.agent
                .post(&url)
                .header("Content-Type", "application/json")
                .header("x-goog-api-key", self.config.api_key())
                .send_json(&body)
        })
        .context("Network error connecting to Google Gemini")?;

        let response = handle_http_response(response, "Google Gemini")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse Google Gemini response")?;

        // Extract text from: {"candidates": [{"content": {"parts": [{"text": "..."}]}}]}
        let content = response_json
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|candidate| candidate.get("content"))
            .and_then(|content| content.get("parts"))
            .and_then(|parts| parts.as_array())
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                    .join("")
            })
            .unwrap_or_default();

        tracing::debug!(
            content_len = content.len(),
            "Received non-streaming response from Google Gemini"
        );

        Ok(content)
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
        _session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages);
        let url = self.stream_api_url(model_id);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to Google Gemini"
        );

        let response = send_json_with_retry("Google Gemini", "stream_message", || {
            self.agent
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Accept", "text/event-stream")
                .header("x-goog-api-key", self.config.api_key())
                .send_json(&body)
        })
        .context("Network error connecting to Google Gemini")?;

        let response = handle_http_response(response, "Google Gemini")?;

        let reader = BufReader::new(response.into_body().into_reader());

        stream_sse_lines(reader, |data| {
            // Gemini streaming format (SSE with alt=sse):
            // {"candidates": [{"content": {"parts": [{"text": "..."}]}}]}
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(text) = parsed
                    .get("candidates")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|candidate| candidate.get("content"))
                    .and_then(|content| content.get("parts"))
                    .and_then(|parts| parts.as_array())
                    .and_then(|parts| parts.first())
                    .and_then(|part| part.get("text"))
                    .and_then(|t| t.as_str())
                {
                    if !on_chunk(text.to_string()) {
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        })?;

        tracing::debug!("Completed streaming response from Google Gemini");

        Ok(())
    }
}

/// Groq provider implementation with real API calls.
///
/// Groq uses an OpenAI-compatible API at `api.groq.com/openai/v1`, so the
/// request/response format is identical to the OpenAI provider.
pub struct GroqProvider {
    config: ProviderConfig,
    agent: ureq::Agent,
}

/// Groq API constants
const GROQ_API_URL: &str = "https://api.groq.com/openai/v1/chat/completions";

impl GroqProvider {
    /// Create a new Groq provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("groq", "Groq", api_key),
            agent: create_agent(),
        }
    }

    /// Build the request body for Groq API (OpenAI-compatible format).
    fn build_request_body(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        stream: bool,
    ) -> serde_json::Value {
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                if m.has_images() {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();
                    for img in &m.images {
                        let data_url = format!("data:{};base64,{}", img.media_type, img.data);
                        content_blocks.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {"url": data_url}
                        }));
                    }
                    if !m.content.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": m.content
                        }));
                    }
                    serde_json::json!({
                        "role": m.role,
                        "content": content_blocks
                    })
                } else {
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content
                    })
                }
            })
            .collect();

        serde_json::json!({
            "model": model_id,
            "stream": stream,
            "messages": api_messages
        })
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
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Sending non-streaming request to Groq"
        );

        let response = send_json_with_retry("Groq", "send_message", || {
            self.agent
                .post(GROQ_API_URL)
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .send_json(&body)
        })
        .context("Network error connecting to Groq")?;

        let response = handle_http_response(response, "Groq")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse Groq response")?;

        // OpenAI-compatible format: {"choices": [{"message": {"content": "..."}}]}
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
            "Received non-streaming response from Groq"
        );

        Ok(content)
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
        _session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            message_count = messages.len(),
            "Starting streaming request to Groq"
        );

        let response = send_json_with_retry("Groq", "stream_message", || {
            self.agent
                .post(GROQ_API_URL)
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .header("Accept", "text/event-stream")
                .send_json(&body)
        })
        .context("Network error connecting to Groq")?;

        let response = handle_http_response(response, "Groq")?;

        let reader = BufReader::new(response.into_body().into_reader());

        stream_sse_lines(reader, |data| {
            // OpenAI-compatible streaming format:
            // {"choices": [{"delta": {"content": "..."}}]}
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|choice| choice.get("delta"))
                    .and_then(|delta| delta.get("content"))
                    .and_then(|c| c.as_str())
                {
                    if !on_chunk(content.to_string()) {
                        return Ok(false);
                    }
                }
            }
            Ok(true)
        })?;

        tracing::debug!("Completed streaming response from Groq");

        Ok(())
    }
}

/// Vercel AI Gateway URL
const VERCEL_GATEWAY_URL: &str = "https://ai-gateway.vercel.sh/v1";

/// Vercel AI Gateway provider implementation.
///
/// Routes requests through Vercel's AI Gateway, which supports multiple providers
/// through namespaced model IDs (e.g., "openai/gpt-4o", "anthropic/claude-sonnet-4.5").
pub struct VercelGatewayProvider {
    config: ProviderConfig,
    agent: ureq::Agent,
}

impl VercelGatewayProvider {
    /// Create a new Vercel Gateway provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            config: ProviderConfig::new("vercel", "Vercel AI Gateway", api_key),
            agent: create_agent(),
        }
    }

    /// Get the chat completions API URL
    fn api_url(&self) -> String {
        format!("{}/chat/completions", VERCEL_GATEWAY_URL)
    }

    /// Normalize a model ID to include provider prefix if missing.
    ///
    /// Vercel Gateway expects namespaced model IDs like "openai/gpt-4o".
    /// If no prefix is provided, defaults to "openai/".
    fn normalize_model_id(model_id: &str) -> String {
        if model_id.contains('/') {
            model_id.to_string()
        } else {
            format!("openai/{}", model_id)
        }
    }

    /// Build the request body for Vercel Gateway (OpenAI-compatible format)
    ///
    /// Supports multimodal messages with images using OpenAI's content array format:
    /// ```json
    /// {
    ///   "role": "user",
    ///   "content": [
    ///     {"type": "image_url", "image_url": {"url": "data:image/png;base64,..."}},
    ///     {"type": "text", "text": "What's in this image?"}
    ///   ]
    /// }
    /// ```
    fn build_request_body(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        stream: bool,
    ) -> serde_json::Value {
        let api_messages: Vec<serde_json::Value> = messages
            .iter()
            .map(|m| {
                // If message has images, use content array format
                if m.has_images() {
                    let mut content_blocks: Vec<serde_json::Value> = Vec::new();

                    // Add images (OpenAI-compatible data URL format)
                    for img in &m.images {
                        let data_url = format!("data:{};base64,{}", img.media_type, img.data);
                        content_blocks.push(serde_json::json!({
                            "type": "image_url",
                            "image_url": {
                                "url": data_url
                            }
                        }));
                    }

                    // Add text content if not empty
                    if !m.content.is_empty() {
                        content_blocks.push(serde_json::json!({
                            "type": "text",
                            "text": m.content
                        }));
                    }

                    serde_json::json!({
                        "role": m.role,
                        "content": content_blocks
                    })
                } else {
                    // Text-only message (simpler format)
                    serde_json::json!({
                        "role": m.role,
                        "content": m.content
                    })
                }
            })
            .collect();

        serde_json::json!({
            "model": Self::normalize_model_id(model_id),
            "stream": stream,
            "messages": api_messages
        })
    }
}

impl AiProvider for VercelGatewayProvider {
    fn provider_id(&self) -> &str {
        &self.config.provider_id
    }

    fn display_name(&self) -> &str {
        &self.config.display_name
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        // Vercel Gateway supports various models from different providers.
        // These are curated defaults; the full list is available via GET https://ai-gateway.vercel.sh/v1/models
        // Model IDs are namespaced: provider/model (e.g., "openai/gpt-4o", "anthropic/claude-haiku-4.5")
        // These MUST match the exact IDs from https://ai-gateway.vercel.sh/v1/models
        // NOTE: The FIRST model in this list is the default model for new chats
        vec![
            // Default model: Claude Haiku 4.5 (fast, cheap, good quality)
            ModelInfo::new(
                "anthropic/claude-haiku-4.5",
                "Claude Haiku 4.5 (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            ModelInfo::new(
                "anthropic/claude-3.5-haiku",
                "Claude 3.5 Haiku (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            // Other Anthropic models
            ModelInfo::new(
                "anthropic/claude-sonnet-4.5",
                "Claude Sonnet 4.5 (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            ModelInfo::new(
                "anthropic/claude-opus-4.5",
                "Claude Opus 4.5 (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            ModelInfo::new(
                "anthropic/claude-sonnet-4",
                "Claude Sonnet 4 (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            // OpenAI models
            ModelInfo::new("openai/gpt-5", "GPT-5 (via Vercel)", "vercel", true, 400000),
            ModelInfo::new(
                "openai/gpt-5-mini",
                "GPT-5 mini (via Vercel)",
                "vercel",
                true,
                400000,
            ),
            ModelInfo::new(
                "openai/gpt-4o",
                "GPT-4o (via Vercel)",
                "vercel",
                true,
                128000,
            ),
            ModelInfo::new("openai/o3", "o3 (via Vercel)", "vercel", true, 200000),
            ModelInfo::new(
                "openai/gpt-4o-mini",
                "GPT-4o mini (via Vercel)",
                "vercel",
                true,
                128000,
            ),
            ModelInfo::new(
                "openai/o3-mini",
                "o3 mini (via Vercel)",
                "vercel",
                true,
                200000,
            ),
            // Google models
            ModelInfo::new(
                "google/gemini-2.5-pro",
                "Gemini 2.5 Pro (via Vercel)",
                "vercel",
                true,
                1048576,
            ),
            ModelInfo::new(
                "google/gemini-2.5-flash",
                "Gemini 2.5 Flash (via Vercel)",
                "vercel",
                true,
                1048576,
            ),
            // xAI models
            ModelInfo::new("xai/grok-3", "Grok 3 (via Vercel)", "vercel", true, 131072),
            // DeepSeek models
            ModelInfo::new(
                "deepseek/deepseek-r1",
                "DeepSeek R1 (via Vercel)",
                "vercel",
                true,
                160000,
            ),
        ]
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, false);

        tracing::debug!(
            model = model_id,
            normalized_model = Self::normalize_model_id(model_id),
            message_count = messages.len(),
            "Sending non-streaming request to Vercel Gateway"
        );

        let response = send_json_with_retry("Vercel AI Gateway", "send_message", || {
            self.agent
                .post(&self.api_url())
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .send_json(&body)
        })
        .context("Network error connecting to Vercel AI Gateway")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "Vercel AI Gateway")?;

        let response_json: serde_json::Value = response
            .into_body()
            .read_json()
            .context("Failed to parse Vercel Gateway response")?;

        // OpenAI-compatible response format
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
            "Received non-streaming response from Vercel Gateway"
        );

        Ok(content)
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
        _session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        let body = self.build_request_body(messages, model_id, true);

        tracing::debug!(
            model = model_id,
            normalized_model = Self::normalize_model_id(model_id),
            message_count = messages.len(),
            "Starting streaming request to Vercel Gateway"
        );

        let response = send_json_with_retry("Vercel AI Gateway", "stream_message", || {
            self.agent
                .post(&self.api_url())
                .header("Content-Type", "application/json")
                .header(
                    "Authorization",
                    &format!("Bearer {}", self.config.api_key()),
                )
                .header("Accept", "text/event-stream")
                .send_json(&body)
        })
        .context("Network error connecting to Vercel AI Gateway")?;

        // Check HTTP status and extract meaningful error if not 2xx
        let response = handle_http_response(response, "Vercel AI Gateway")?;

        // Read the SSE stream using the helper (OpenAI-compatible format)
        let reader = BufReader::new(response.into_body().into_reader());

        stream_sse_lines(reader, |data| {
            // Parse OpenAI-compatible streaming format
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(content) = parsed
                    .get("choices")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|choice| choice.get("delta"))
                    .and_then(|delta| delta.get("content"))
                    .and_then(|c| c.as_str())
                {
                    if !on_chunk(content.to_string()) {
                        return Ok(false);
                    }
                }
            }
            Ok(true) // continue processing
        })?;

        tracing::debug!("Completed streaming response from Vercel Gateway");

        Ok(())
    }
}

/// Claude Code CLI provider implementation.
///
/// This provider wraps the local `claude` CLI in headless mode, speaking JSONL
/// over stdin/stdout. It allows Script Kit to use Claude Code as a first-class
/// AI provider with session persistence and tool access.
///
/// # Configuration
///
/// The provider is configured via environment variables:
/// - `SCRIPT_KIT_CLAUDE_CODE_ENABLED`: Set to "1" or "true" to enable
/// - `SCRIPT_KIT_CLAUDE_PATH`: Path to `claude` binary (default: "claude")
/// - `SCRIPT_KIT_CLAUDE_PERMISSION_MODE`: Permission mode (default: "plan")
/// - `SCRIPT_KIT_CLAUDE_ALLOWED_TOOLS`: Comma-separated tools (optional)
/// - `SCRIPT_KIT_CLAUDE_ADD_DIRS`: Comma-separated workspace paths (optional)
///
/// # Protocol
///
/// Uses Claude Code's stream-json protocol:
/// - Spawns `claude` with `--print --input-format stream-json --output-format stream-json`
/// - Writes one JSON object per line to stdin for user messages
/// - Reads JSON objects from stdout, streaming text from `stream_event` deltas
///
/// # Session Persistence
///
/// Each conversation gets a UUID session ID passed via `--session-id`, allowing
/// Claude Code to maintain context across messages within a chat.
pub struct ClaudeCodeProvider {
    claude_path: String,
    permission_mode: String,
    allowed_tools: Option<String>,
    add_dirs: Vec<std::path::PathBuf>,
}

impl Clone for ClaudeCodeProvider {
    fn clone(&self) -> Self {
        Self {
            claude_path: self.claude_path.clone(),
            permission_mode: self.permission_mode.clone(),
            allowed_tools: self.allowed_tools.clone(),
            add_dirs: self.add_dirs.clone(),
        }
    }
}

impl ClaudeCodeProvider {
    /// Create a ClaudeCodeProvider from a config file configuration.
    ///
    /// This is the preferred method when using `~/.scriptkit/config.ts`.
    ///
    /// Returns `Some(provider)` if:
    /// 1. `config.enabled` is true
    /// 2. The `claude` CLI is available in PATH (or at custom path)
    ///
    /// Returns `None` if the provider is not enabled or `claude` is not found.
    pub fn from_config(config: &crate::config::ClaudeCodeConfig) -> Option<Self> {
        if let Err(error) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
        {
            tracing::warn!(%error, "CLI provider discovery refused");
            return None;
        }
        if !config.enabled {
            tracing::debug!("Claude Code CLI provider not enabled in config");
            return None;
        }

        let claude_path = config.path.clone().unwrap_or_else(|| "claude".to_string());

        // Verify `claude` is available
        if !Self::is_available(&claude_path) {
            tracing::warn!(
                path = %claude_path,
                "Claude Code CLI not found at configured path - provider disabled"
            );
            return None;
        }

        let permission_mode = config.permission_mode.clone();
        let allowed_tools = config.allowed_tools.clone();
        let add_dirs: Vec<std::path::PathBuf> = config
            .add_dirs
            .iter()
            .map(std::path::PathBuf::from)
            .collect();

        tracing::info!(
            path = %claude_path,
            permission_mode = %permission_mode,
            add_dirs_count = add_dirs.len(),
            "Claude Code CLI provider initialized from config"
        );

        Some(Self {
            claude_path,
            permission_mode,
            allowed_tools,
            add_dirs,
        })
    }

    /// Attempt to create a ClaudeCodeProvider from environment variables.
    ///
    /// This is the fallback method when config is not available.
    /// Prefer `from_config()` when loading from `~/.scriptkit/config.ts`.
    ///
    /// Returns `Some(provider)` if:
    /// 1. `SCRIPT_KIT_CLAUDE_CODE_ENABLED` is set to "1" or "true"
    /// 2. The `claude` CLI is available in PATH (or at custom path)
    ///
    /// Returns `None` if the provider is not enabled or `claude` is not found.
    pub fn detect_from_env() -> Option<Self> {
        if let Err(error) =
            crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)
        {
            tracing::warn!(%error, "CLI provider discovery refused");
            return None;
        }
        use super::config::env_vars;

        // Check if explicitly enabled
        let enabled = std::env::var(env_vars::CLAUDE_CODE_ENABLED)
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);

        if !enabled {
            tracing::debug!(
                "Claude Code CLI provider not enabled (set SCRIPT_KIT_CLAUDE_CODE_ENABLED=1)"
            );
            return None;
        }

        let claude_path =
            std::env::var(env_vars::CLAUDE_CODE_PATH).unwrap_or_else(|_| "claude".to_string());

        // Verify `claude` is available
        if !Self::is_available(&claude_path) {
            tracing::warn!(
                path = %claude_path,
                "Claude Code CLI not found - provider disabled"
            );
            return None;
        }

        let permission_mode = std::env::var(env_vars::CLAUDE_CODE_PERMISSION_MODE)
            .unwrap_or_else(|_| "plan".to_string());

        let allowed_tools = std::env::var(env_vars::CLAUDE_CODE_ALLOWED_TOOLS).ok();

        let add_dirs = std::env::var(env_vars::CLAUDE_CODE_ADD_DIRS)
            .ok()
            .map(|v| {
                v.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(std::path::PathBuf::from)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        tracing::info!(
            path = %claude_path,
            permission_mode = %permission_mode,
            add_dirs_count = add_dirs.len(),
            "Claude Code CLI provider initialized from environment"
        );

        Some(Self {
            claude_path,
            permission_mode,
            allowed_tools,
            add_dirs,
        })
    }

    /// Check if the `claude` CLI is available at the given path.
    fn is_available(path: &str) -> bool {
        if crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Process).is_err() {
            return false;
        }
        use std::process::{Command, Stdio};

        Command::new(path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Extract the system prompt from messages (if any).
    fn extract_system_prompt(messages: &[ProviderMessage]) -> Option<String> {
        messages
            .iter()
            .find(|m| m.role == "system")
            .map(|m| m.content.clone())
    }

    /// Extract the last user message text.
    fn extract_last_user_text(messages: &[ProviderMessage]) -> Result<String> {
        let last_user = messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .ok_or_else(|| anyhow!("No user message found"))?;

        if !last_user.images.is_empty() {
            return Err(anyhow!(
                "Claude Code CLI provider currently does not support image messages"
            ));
        }

        Ok(last_user.content.clone())
    }

    /// Build a user message JSON for the stream-json protocol.
    fn make_user_message_json(content: &str) -> serde_json::Value {
        // The Agent SDK stream-json format: a per-line user message
        serde_json::json!({
            "type": "user",
            "message": {
                "role": "user",
                "content": content
            }
        })
    }

    /// Execute a single streaming request to Claude Code CLI.
    ///
    /// # Arguments
    /// * `session_id` - UUID for session persistence
    /// * `model_id` - Model to use ("sonnet", "opus", "default")
    /// * `system_prompt` - Optional system prompt
    /// * `user_prompt` - The user's message
    /// * `on_chunk` - Callback for streaming text chunks
    ///
    /// # Returns
    /// The final result text from the `type:"result"` message.
    fn stream_claude_once(
        &self,
        session_id: &str,
        model_id: &str,
        system_prompt: Option<&str>,
        user_prompt: &str,
        on_chunk: &StreamCallback,
        is_resuming: bool,
    ) -> Result<String> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        use std::io::{BufRead, Write};
        use std::process::{Command, Stdio};

        let mut cmd = Command::new(&self.claude_path);

        // ASSISTANT MODE: Disable all coding features, act as a helpful assistant
        // Full isolation prevents project settings from loading. Credentials stay
        // in the child environment; inline `--settings` contains no auth data.
        cmd.arg("--setting-sources").arg("");
        let launch_auth = super::session::apply_safe_claude_launch_settings(
            &mut cmd,
            &super::session::read_user_credential_settings(),
        )?;

        // Only allow safe, non-destructive tools
        cmd.arg("--tools").arg("WebSearch, WebFetch, Read");

        // Disable Chrome integration and slash commands
        cmd.arg("--no-chrome");
        cmd.arg("--disable-slash-commands");

        // Core headless mode flags
        // NOTE: --verbose is REQUIRED when using --output-format stream-json with --print
        // --include-partial-messages enables real-time streaming chunks
        cmd.arg("--print")
            .arg("--verbose")
            .arg("--include-partial-messages")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json");

        // Session persistence: use --session-id for new sessions, --resume for continuing
        // This is CRITICAL for conversation continuity:
        // - --session-id creates a NEW session and saves it to disk
        // - --resume loads an EXISTING session from disk and continues it
        if is_resuming {
            tracing::debug!(session_id = %session_id, "Resuming existing Claude Code session");
            cmd.arg("--resume").arg(session_id);
        } else {
            tracing::debug!(session_id = %session_id, "Creating new Claude Code session");
            cmd.arg("--session-id").arg(session_id);
        }

        // Model selection (if not default)
        if !model_id.is_empty() && model_id != "default" {
            cmd.arg("--model").arg(model_id);
        }

        // System prompt - use provided or default to helpful assistant
        // Note: System prompt is only applied on new sessions; resumed sessions use the original
        let system_prompt_transport =
            super::session::prepare_private_claude_system_prompt(&mut cmd, system_prompt)?;

        // Clear CLAUDECODE env var so the spawned CLI doesn't think it's a nested session
        cmd.env_remove("CLAUDECODE");

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        tracing::debug!(
            session_id = %session_id,
            model_id = %model_id,
            credential_env_count = launch_auth.credential_env_count,
            api_key_configured = launch_auth.api_key_configured,
            oauth_token_configured = launch_auth.oauth_token_configured,
            "Spawning Claude Code CLI"
        );

        let mut child = cmd.spawn().context("Failed to spawn `claude` CLI")?;
        system_prompt_transport.deliver_after_spawn(&mut child)?;

        // Drain stderr in a separate thread to prevent deadlock
        // Use Arc<Mutex<>> to capture stderr content for error reporting
        let stderr_content = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
        let stderr_capture = stderr_content.clone();
        let stderr_handle = child.stderr.take().map(|stderr| {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    // Log stderr for debugging (but don't spam)
                    if !line.trim().is_empty() {
                        let diagnostic = super::reliability::redact_diagnostic(&line);
                        tracing::trace!(
                            diagnostic_fingerprint = %diagnostic.fingerprint.0,
                            stderr_bytes = line.len(),
                            "Claude CLI stderr"
                        );
                        // Capture stderr for error messages
                        if let Ok(mut content) = stderr_capture.lock() {
                            if !content.is_empty() {
                                content.push('\n');
                            }
                            content.push_str(&line);
                        }
                    }
                }
            })
        });

        // Send one user message line, then close stdin (EOF ends the query)
        {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| anyhow!("No stdin handle"))?;
            let msg = Self::make_user_message_json(user_prompt);
            let line = serde_json::to_string(&msg)?;

            tracing::trace!(message_bytes = line.len(), "Sending to Claude CLI stdin");

            stdin.write_all(line.as_bytes())?;
            stdin.write_all(b"\n")?;
            // stdin drops here, sending EOF
        }

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("No stdout handle"))?;
        let reader = BufReader::new(stdout);

        let mut saw_text_delta = false;
        let mut final_result: Option<String> = None;

        for line in reader.lines() {
            let line = line.context("Failed to read Claude CLI stdout")?;

            if line.trim().is_empty() {
                continue;
            }

            let v: serde_json::Value = match serde_json::from_str(&line) {
                Ok(v) => v,
                Err(_) => {
                    // Ignore non-JSON lines (e.g., debug output)
                    let diagnostic = super::reliability::redact_diagnostic(&line);
                    tracing::trace!(
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        stdout_bytes = line.len(),
                        "Non-JSON line from Claude CLI"
                    );
                    continue;
                }
            };

            let msg_type = v.get("type").and_then(|x| x.as_str()).unwrap_or("");

            match msg_type {
                "stream_event" => {
                    // Anthropic-style streaming deltas wrapped by Claude Code
                    // Look for content_block_delta with text_delta
                    let event = &v["event"];
                    let event_type = event.get("type").and_then(|x| x.as_str());

                    if event_type == Some("content_block_delta") {
                        let delta_type = event["delta"].get("type").and_then(|x| x.as_str());

                        if delta_type == Some("text_delta") {
                            if let Some(text) = event["delta"].get("text").and_then(|x| x.as_str())
                            {
                                saw_text_delta = true;
                                if !on_chunk(text.to_string()) {
                                    let _ = child.kill();
                                    let _ = child.wait();
                                    return Ok(final_result.unwrap_or_default());
                                }
                            }
                        }
                    }
                }
                "assistant" => {
                    // Fallback: extract text from assistant message if no streaming deltas
                    if !saw_text_delta {
                        if let Some(content) =
                            v.pointer("/message/content").and_then(|x| x.as_array())
                        {
                            let mut text = String::new();
                            for block in content {
                                if block.get("type").and_then(|x| x.as_str()) == Some("text") {
                                    if let Some(t) = block.get("text").and_then(|x| x.as_str()) {
                                        text.push_str(t);
                                    }
                                }
                            }
                            if !text.is_empty() && !on_chunk(text.clone()) {
                                let _ = child.kill();
                                let _ = child.wait();
                                return Ok(final_result.unwrap_or_default());
                            }
                        }
                    }
                }
                "result" => {
                    // Final result message - check for errors
                    let is_error = v.get("is_error").and_then(|x| x.as_bool()).unwrap_or(false);
                    if is_error {
                        // Extract human-readable message from errors JSON
                        let error_msg = v
                            .get("errors")
                            .and_then(|e| e.as_array())
                            .and_then(|arr| {
                                arr.iter()
                                    .filter_map(|e| {
                                        e.get("message")
                                            .or_else(|| e.get("error"))
                                            .and_then(|m| m.as_str())
                                    })
                                    .next()
                            })
                            .or_else(|| v.get("errors").and_then(|e| e.as_str()))
                            .unwrap_or("Unknown error");
                        return Err(anyhow!(
                            "Claude Code error: {}",
                            safe_provider_diagnostic_detail(error_msg)
                        ));
                    }
                    if let Some(r) = v.get("result").and_then(|x| x.as_str()) {
                        final_result = Some(r.to_string());
                    }
                    break;
                }
                _ => {
                    // Ignore other message types (e.g., "init", "system", etc.)
                    tracing::trace!(msg_type = %msg_type, "Ignoring Claude CLI message type");
                }
            }
        }

        // Wait for the process to finish
        let status = child.wait().context("Failed to wait for Claude CLI")?;
        if !status.success() {
            // Wait for stderr thread to finish capturing
            if let Some(handle) = stderr_handle {
                let _ = handle.join();
            }
            let stderr_msg = stderr_content.lock().map(|s| s.clone()).unwrap_or_default();
            if stderr_msg.is_empty() {
                return Err(anyhow!("`claude` CLI exited with status: {}", status));
            } else {
                let diagnostic = super::reliability::redact_diagnostic(&stderr_msg);
                tracing::error!(
                    diagnostic_fingerprint = %diagnostic.fingerprint.0,
                    stderr_bytes = stderr_msg.len(),
                    status = %status,
                    "Claude CLI failed with stderr output"
                );
                // Surface the meaningful part of stderr directly
                let clean_msg = if stderr_msg
                    .contains("cannot be launched inside another Claude Code session")
                {
                    "Claude Code cannot be launched inside another Claude Code session. \
                     Nested sessions share runtime resources and will crash all active sessions."
                        .to_string()
                } else if stderr_msg.contains("command not found") {
                    "Claude Code CLI is not installed".to_string()
                } else {
                    // Strip common prefixes like "Error: " for cleaner display
                    let detail = stderr_msg
                        .trim()
                        .strip_prefix("Error: ")
                        .unwrap_or(stderr_msg.trim());
                    safe_provider_diagnostic_detail(detail)
                };
                return Err(anyhow!("{}", clean_msg));
            }
        }

        tracing::debug!(
            session_id = %session_id,
            saw_streaming = saw_text_delta,
            "Claude Code CLI request completed"
        );

        super::session::completed_claude_response(final_result)
    }
}

impl AiProvider for ClaudeCodeProvider {
    fn provider_id(&self) -> &str {
        "claude_code"
    }

    fn display_name(&self) -> &str {
        "Claude Code (CLI)"
    }

    fn available_models(&self) -> Vec<ModelInfo> {
        vec![
            ModelInfo::new(
                "sonnet",
                "Claude Code - Sonnet",
                "claude_code",
                true,
                200_000,
            ),
            ModelInfo::new("opus", "Claude Code - Opus", "claude_code", true, 200_000),
            ModelInfo::new("haiku", "Claude Code - Haiku", "claude_code", true, 200_000),
            ModelInfo::new(
                "default",
                "Claude Code - Default",
                "claude_code",
                true,
                200_000,
            ),
        ]
    }

    fn send_message(&self, messages: &[ProviderMessage], model_id: &str) -> Result<String> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        // Generate a new session ID for this standalone request
        let session_id = uuid::Uuid::new_v4().to_string();
        let system_prompt = Self::extract_system_prompt(messages);
        let user_prompt = Self::extract_last_user_text(messages)?;

        // Use a no-op callback since we don't need streaming for send_message
        // send_message is always a new session (no persistence)
        let noop: StreamCallback = Box::new(|_| true);
        self.stream_claude_once(
            &session_id,
            model_id,
            system_prompt.as_deref(),
            &user_prompt,
            &noop,
            false, // is_resuming: always false for one-off send_message
        )
    }

    fn stream_message(
        &self,
        messages: &[ProviderMessage],
        model_id: &str,
        on_chunk: StreamCallback,
        session_id: Option<&str>,
    ) -> Result<()> {
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Provider)?;
        // Use provided session ID for conversation continuity, or generate a new one
        let effective_session_id = session_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let system_prompt = Self::extract_system_prompt(messages);
        let user_prompt = Self::extract_last_user_text(messages)?;

        // Check if persistent sessions are enabled (default: true)
        // Set SCRIPT_KIT_CLAUDE_PERSISTENT_SESSION=0 to disable
        let use_persistent = std::env::var("SCRIPT_KIT_CLAUDE_PERSISTENT_SESSION")
            .map(|v| v != "0" && v.to_lowercase() != "false")
            .unwrap_or(true);

        if use_persistent && session_id.is_some() {
            // Try persistent session manager first
            tracing::info!(
                session_id = %effective_session_id,
                model_id = %model_id,
                message_count = messages.len(),
                user_prompt_len = user_prompt.len(),
                "Using persistent Claude session"
            );

            let manager = super::session::ClaudeSessionManager::global();
            let chunk_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let chunk_count_clone = chunk_count.clone();

            match manager.send_message(
                &effective_session_id,
                &user_prompt,
                model_id,
                system_prompt.as_deref(),
                |chunk| {
                    let count =
                        chunk_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    tracing::trace!(
                        chunk_num = count,
                        chunk_len = chunk.len(),
                        "Persistent session chunk received"
                    );
                    on_chunk(chunk.to_string())
                },
            ) {
                Ok(result) => {
                    let total_chunks = chunk_count.load(std::sync::atomic::Ordering::Relaxed);
                    if !forward_unstreamed_claude_response(&result, total_chunks, &on_chunk) {
                        return Ok(());
                    }
                    tracing::info!(
                        session_id = %effective_session_id,
                        total_chunks,
                        result_len = result.len(),
                        "Persistent session message completed"
                    );
                    return Ok(());
                }
                Err(e) => {
                    if super::session::is_claude_session_cancelled(&e) {
                        tracing::info!(
                            session_id = %effective_session_id,
                            "Persistent Claude request cancelled without retry"
                        );
                        return Ok(());
                    }

                    let total_chunks = chunk_count.load(std::sync::atomic::Ordering::Relaxed);
                    if !super::session::should_retry_claude_session_failure(&e, total_chunks) {
                        return Err(e);
                    }
                    let diagnostic = super::reliability::redact_diagnostic(&e.to_string());
                    tracing::warn!(
                        session_id = %effective_session_id,
                        diagnostic_fingerprint = %diagnostic.fingerprint.0,
                        "Persistent session failed, falling back to spawn-per-message"
                    );
                    // Fall through to spawn-per-message
                }
            }
        }

        // Fallback: spawn-per-message approach (original implementation)
        // Detect if we're resuming an existing session by checking for assistant messages
        // If there are any assistant messages in history, this is a follow-up message
        // and we should use --resume instead of --session-id
        let has_assistant_messages = messages.iter().any(|m| m.role == "assistant");
        let is_resuming = session_id.is_some() && has_assistant_messages;

        tracing::debug!(
            session_id = %effective_session_id,
            has_session_id = session_id.is_some(),
            has_assistant_messages = has_assistant_messages,
            is_resuming = is_resuming,
            message_count = messages.len(),
            "Claude Code spawn-per-message mode"
        );

        let _ = self.stream_claude_once(
            &effective_session_id,
            model_id,
            system_prompt.as_deref(),
            &user_prompt,
            &on_chunk,
            is_resuming,
        )?;

        Ok(())
    }
}

/// Registry of available AI providers.
///
/// The registry automatically discovers available providers based on
/// environment variables and provides a unified interface to access them.
#[derive(Clone)]
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

    /// Create a registry populated from environment variables only.
    ///
    /// Scans for `SCRIPT_KIT_*_API_KEY` environment variables and
    /// creates providers for each detected key.
    ///
    /// For Claude Code CLI, uses environment variables only.
    /// Prefer `from_environment_with_config` when loading from `~/.scriptkit/config.ts`.
    pub fn from_environment() -> Self {
        Self::from_environment_with_config(None)
    }

    /// Create a registry populated from environment variables and optional config.
    ///
    /// Scans for `SCRIPT_KIT_*_API_KEY` environment variables and
    /// creates providers for each detected key.
    ///
    /// For Claude Code CLI:
    /// - If config is provided and has `claudeCode.enabled = true`, uses config settings
    /// - Otherwise falls back to environment variables (`SCRIPT_KIT_CLAUDE_CODE_ENABLED=1`)
    ///
    /// # Arguments
    ///
    /// * `config` - Optional Script Kit configuration from `~/.scriptkit/config.ts`
    pub fn from_environment_with_config(config: Option<&crate::config::Config>) -> Self {
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

        if let Some(key) = keys.vercel {
            registry.register(Arc::new(VercelGatewayProvider::new(key)));
        }

        let claude_provider = config
            .map(|c| c.get_claude_code())
            .and_then(|claude_config| ClaudeCodeProvider::from_config(&claude_config))
            .or_else(ClaudeCodeProvider::detect_from_env);

        if let Some(claude_cli) = claude_provider {
            registry.register(Arc::new(claude_cli));
        }

        // Log which providers are available (without exposing keys)
        let available: Vec<_> = registry.providers.keys().collect();
        if !available.is_empty() {
            tracing::info!(
                providers = ?available,
                "AI providers initialized"
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
include!("providers_tests.rs");
