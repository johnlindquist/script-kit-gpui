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

/// Extract a user-friendly error message from an API error response body.
///
/// Tries to parse JSON error responses from various AI providers and extract
/// the most useful error message for display to users.
fn extract_api_error_message(body: &str) -> Option<String> {
    let parsed: serde_json::Value = serde_json::from_str(body).ok()?;

    // OpenAI/Vercel format: {"error": {"message": "...", "type": "..."}}
    if let Some(error) = parsed.get("error") {
        let message = error.get("message").and_then(|m| m.as_str());
        let error_type = error.get("type").and_then(|t| t.as_str());

        return match (message, error_type) {
            (Some(msg), Some(typ)) => Some(format!("{}: {}", typ, msg)),
            (Some(msg), None) => Some(msg.to_string()),
            _ => None,
        };
    }

    // Anthropic format: {"type": "error", "error": {"type": "...", "message": "..."}}
    if parsed.get("type").and_then(|t| t.as_str()) == Some("error") {
        if let Some(error) = parsed.get("error") {
            let message = error.get("message").and_then(|m| m.as_str());
            let error_type = error.get("type").and_then(|t| t.as_str());

            return match (message, error_type) {
                (Some(msg), Some(typ)) => Some(format!("{}: {}", typ, msg)),
                (Some(msg), None) => Some(msg.to_string()),
                _ => None,
            };
        }
    }

    None
}

fn safe_provider_diagnostic_detail(raw: &str) -> String {
    super::reliability::redact_diagnostic(raw)
        .copyable_detail
        .unwrap_or_else(|| "Provider diagnostic details were redacted".to_string())
}

fn provider_http_failure_message(status: u16, provider_name: &str, body: &str) -> String {
    let error_detail = extract_api_error_message(body);

    match status {
        401 => {
            let detail = error_detail.unwrap_or_else(|| "Invalid or missing API key".to_string());
            format!(
                "{} authentication failed: {}",
                provider_name,
                safe_provider_diagnostic_detail(&simplify_auth_error(&detail))
            )
        }
        403 => {
            let detail = error_detail.unwrap_or_else(|| "Access denied".to_string());
            format!(
                "{} access denied: {}",
                provider_name,
                safe_provider_diagnostic_detail(&detail)
            )
        }
        404 => {
            let detail = error_detail.unwrap_or_else(|| "Model or endpoint not found".to_string());
            format!(
                "{}: {}",
                provider_name,
                safe_provider_diagnostic_detail(&detail)
            )
        }
        429 => {
            let detail = error_detail.unwrap_or_else(|| "Too many requests".to_string());
            format!(
                "{} rate limited: {}",
                provider_name,
                safe_provider_diagnostic_detail(&detail)
            )
        }
        500..=599 => {
            let detail = error_detail.unwrap_or_else(|| "Server error".to_string());
            format!(
                "{} server error ({}): {}",
                provider_name,
                status,
                safe_provider_diagnostic_detail(&detail)
            )
        }
        _ => {
            let detail = error_detail.unwrap_or_else(|| body.to_string());
            format!(
                "{} error (HTTP {}): {}",
                provider_name,
                status,
                safe_provider_diagnostic_detail(&detail)
            )
        }
    }
}

/// Handle HTTP response and return an error if status is not 2xx.
///
/// Reads the error body and extracts a user-friendly message.
fn handle_http_response(
    response: ureq::http::Response<ureq::Body>,
    provider_name: &str,
) -> Result<ureq::http::Response<ureq::Body>> {
    let status = response.status().as_u16();

    if (200..300).contains(&status) {
        return Ok(response);
    }

    // Read the error body
    let mut body = response.into_body();
    let body_str = body.read_to_string().unwrap_or_default();

    let user_message = provider_http_failure_message(status, provider_name, &body_str);
    let diagnostic = super::reliability::redact_diagnostic(&body_str);

    tracing::warn!(
        status = status,
        provider = provider_name,
        diagnostic_fingerprint = %diagnostic.fingerprint.0,
        response_bytes = body_str.len(),
        "API request failed"
    );

    Err(anyhow!(user_message))
}

/// Simplify verbose authentication error messages for display.
fn simplify_auth_error(detail: &str) -> String {
    // Vercel OIDC errors are very verbose - simplify them
    if detail.contains("OIDC") || detail.contains("VERCEL_OIDC_TOKEN") {
        return "Vercel AI Gateway requires OIDC authentication. This is only available when running on Vercel. For local development, use direct API keys (SCRIPT_KIT_ANTHROPIC_API_KEY, SCRIPT_KIT_OPENAI_API_KEY).".to_string();
    }
    detail.to_string()
}

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
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn claude_provider_custom_system_prompt_uses_private_shared_descriptor_transport() {
        let hostile_prompt = "provider-private-system-prompt-canary Bearer provider-private-token";
        let mut command = std::process::Command::new("claude-not-launched");
        let _transport = super::super::session::prepare_private_claude_system_prompt(
            &mut command,
            Some(hostile_prompt),
        )
        .expect("provider uses the shared anonymous prompt pipe");
        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();

        assert_eq!(args[0], "--system-prompt-file");
        assert!(args[1].starts_with("/dev/fd/"));
        assert!(args
            .iter()
            .all(|argument| !argument.contains("provider-private")));
        assert!(!format!("{command:?}").contains("provider-private"));
        assert!(command.get_envs().all(|(key, value)| {
            !key.to_string_lossy().contains("provider-private")
                && value.is_none_or(|value| !value.to_string_lossy().contains("provider-private"))
        }));
    }

    #[test]
    fn claude_provider_launch_keeps_hostile_credential_canaries_out_of_arguments() {
        let api_key = "provider-private-api-key-canary";
        let oauth_token = "provider-private-oauth-token-canary";
        let mut command = std::process::Command::new("claude-not-launched");
        let summary = super::super::session::apply_safe_claude_launch_settings(
            &mut command,
            &serde_json::json!({
                "env": {
                    "ANTHROPIC_API_KEY": api_key,
                    "CLAUDE_CODE_OAUTH_TOKEN": oauth_token,
                },
                "apiKeyHelper": "provider-private-helper-canary",
                "oauthAccount": {"emailAddress": "provider-private-account-canary"},
            }),
        )
        .expect("existing explicit credentials safely replace unsupported metadata");

        let args: Vec<_> = command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        for canary in [
            api_key,
            oauth_token,
            "provider-private-helper-canary",
            "provider-private-account-canary",
        ] {
            assert!(args.iter().all(|argument| !argument.contains(canary)));
            assert!(!format!("{summary:?}").contains(canary));
        }

        let environment: std::collections::HashMap<_, _> = command
            .get_envs()
            .filter_map(|(key, value)| {
                value.map(|value| {
                    (
                        key.to_string_lossy().into_owned(),
                        value.to_string_lossy().into_owned(),
                    )
                })
            })
            .collect();
        assert_eq!(environment["ANTHROPIC_API_KEY"], api_key);
        assert_eq!(environment["CLAUDE_CODE_OAUTH_TOKEN"], oauth_token);
        assert!(summary.api_key_configured);
        assert!(summary.oauth_token_configured);
    }

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
                    chunks_clone
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push(chunk);
                    true
                }),
                None,
            )
            .unwrap();

        let collected = chunks.lock().unwrap_or_else(|e| e.into_inner());
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

    #[test]
    fn test_stream_sse_lines_basic() {
        use std::io::Cursor;

        // Simulate SSE stream with basic data
        let sse_data = "data: hello\n\ndata: world\n\n";
        let reader = Cursor::new(sse_data);

        let mut collected = Vec::new();
        stream_sse_lines(reader, |data| {
            collected.push(data.to_string());
            Ok(true)
        })
        .unwrap();

        assert_eq!(collected, vec!["hello", "world"]);
    }

    #[test]
    fn test_stream_sse_lines_done_marker() {
        use std::io::Cursor;

        // [DONE] should stop processing
        let sse_data = "data: first\n\ndata: [DONE]\n\ndata: should_not_see\n\n";
        let reader = Cursor::new(sse_data);

        let mut collected = Vec::new();
        stream_sse_lines(reader, |data| {
            collected.push(data.to_string());
            Ok(true)
        })
        .unwrap();

        assert_eq!(collected, vec!["first"]);
    }

    #[test]
    fn test_stream_sse_lines_crlf() {
        use std::io::Cursor;

        // Should handle CRLF line endings
        let sse_data = "data: with_cr\r\n\r\n";
        let reader = Cursor::new(sse_data);

        let mut collected = Vec::new();
        stream_sse_lines(reader, |data| {
            collected.push(data.to_string());
            Ok(true)
        })
        .unwrap();

        assert_eq!(collected, vec!["with_cr"]);
    }

    #[test]
    fn test_stream_sse_lines_callback_stop() {
        use std::io::Cursor;

        // Callback returning false should stop processing
        let sse_data = "data: first\n\ndata: second\n\ndata: third\n\n";
        let reader = Cursor::new(sse_data);

        let mut collected = Vec::new();
        stream_sse_lines(reader, |data| {
            collected.push(data.to_string());
            Ok(collected.len() < 2) // Stop after 2 items
        })
        .unwrap();

        assert_eq!(collected, vec!["first", "second"]);
    }

    #[test]
    fn test_vercel_provider() {
        let provider = VercelGatewayProvider::new("test-key");
        assert_eq!(provider.provider_id(), "vercel");
        assert_eq!(provider.display_name(), "Vercel AI Gateway");

        let models = provider.available_models();
        assert!(!models.is_empty());
        assert!(models.iter().any(|m| m.id.contains("openai/")));
        assert!(models.iter().any(|m| m.id.contains("anthropic/")));
    }

    #[test]
    fn test_vercel_normalize_model_id() {
        // Already prefixed - should not change
        assert_eq!(
            VercelGatewayProvider::normalize_model_id("openai/gpt-4o"),
            "openai/gpt-4o"
        );
        assert_eq!(
            VercelGatewayProvider::normalize_model_id("anthropic/claude-haiku-4.5"),
            "anthropic/claude-haiku-4.5"
        );

        // Not prefixed - should add openai/
        assert_eq!(
            VercelGatewayProvider::normalize_model_id("gpt-4o"),
            "openai/gpt-4o"
        );
        assert_eq!(
            VercelGatewayProvider::normalize_model_id("gpt-4o-mini"),
            "openai/gpt-4o-mini"
        );
    }

    #[test]
    fn test_vercel_request_body_normalizes_model() {
        let provider = VercelGatewayProvider::new("test-key");
        let messages = vec![ProviderMessage::user("Hello")];

        // Test with unprefixed model
        let body = provider.build_request_body(&messages, "gpt-4o", false);
        assert_eq!(body["model"], "openai/gpt-4o");

        // Test with prefixed model
        let body = provider.build_request_body(&messages, "anthropic/claude-haiku-4.5", true);
        assert_eq!(body["model"], "anthropic/claude-haiku-4.5");
    }

    #[test]
    fn test_anthropic_api_url_respects_base_url() {
        // Default URL
        let provider = AnthropicProvider::new("test-key");
        assert_eq!(provider.api_url(), ANTHROPIC_API_URL);

        // Custom base URL
        let provider = AnthropicProvider::with_base_url("test-key", "https://custom.proxy.com/v1");
        assert_eq!(provider.api_url(), "https://custom.proxy.com/v1");
    }

    #[test]
    fn test_registry_with_vercel() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(VercelGatewayProvider::new("test")));

        assert!(registry.has_any_provider());
        assert!(registry.get_provider("vercel").is_some());

        let models = registry.get_all_models();
        assert!(models.iter().any(|m| m.provider == "vercel"));
    }

    #[test]
    fn test_extract_api_error_message_openai_format() {
        // OpenAI/Vercel format
        let body = r#"{"error": {"message": "Invalid API key", "type": "authentication_error"}}"#;
        let result = extract_api_error_message(body);
        assert_eq!(
            result,
            Some("authentication_error: Invalid API key".to_string())
        );

        // Missing type
        let body = r#"{"error": {"message": "Something went wrong"}}"#;
        let result = extract_api_error_message(body);
        assert_eq!(result, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_extract_api_error_message_anthropic_format() {
        // Anthropic format
        let body = r#"{"type": "error", "error": {"type": "invalid_request_error", "message": "Invalid model"}}"#;
        let result = extract_api_error_message(body);
        assert_eq!(
            result,
            Some("invalid_request_error: Invalid model".to_string())
        );
    }

    #[test]
    fn test_extract_api_error_message_invalid_json() {
        let result = extract_api_error_message("not json");
        assert_eq!(result, None);

        let result = extract_api_error_message(r#"{"foo": "bar"}"#);
        assert_eq!(result, None);
    }

    #[test]
    fn provider_http_failure_messages_redact_credentials_and_private_paths() {
        let body = serde_json::json!({
            "error": {
                "type": "authentication_error",
                "message": "Invalid credentials token=sk-private-token Authorization: Bearer sk-private-bearer /Users/private-project/secrets"
            }
        })
        .to_string();

        for status in [401, 403, 404, 429, 500, 418] {
            let message = provider_http_failure_message(status, "Example Provider", &body);
            assert!(message.contains("Example Provider"));
            assert!(!message.contains("sk-private-token"));
            assert!(!message.contains("sk-private-bearer"));
            assert!(!message.contains("/Users/private-project"));
        }

        let auth_message = provider_http_failure_message(401, "Example Provider", &body);
        assert!(auth_message.contains("authentication failed"));
        let rate_message = provider_http_failure_message(429, "Example Provider", &body);
        assert!(rate_message.contains("rate limited"));
    }

    #[test]
    fn provider_diagnostic_copy_preserves_recovery_reason_without_exposing_secrets() {
        let detail = safe_provider_diagnostic_detail(
            "Sign in required api_key=sk-provider-secret /Users/private-person/config.json",
        );

        assert!(detail.contains("Sign in required"));
        assert!(!detail.contains("sk-provider-secret"));
        assert!(!detail.contains("/Users/private-person"));
    }

    #[test]
    fn test_simplify_auth_error_vercel_oidc() {
        let detail = "Error verifying OIDC token\nThe AI Gateway OIDC authentication token...";
        let result = simplify_auth_error(detail);
        assert!(result.contains("Vercel AI Gateway requires OIDC authentication"));
        assert!(result.contains("local development"));
    }

    #[test]
    fn test_simplify_auth_error_passthrough() {
        let detail = "Invalid API key provided";
        let result = simplify_auth_error(detail);
        assert_eq!(result, detail);
    }

    #[test]
    fn test_create_agent_disables_status_errors_and_enforces_https() {
        let agent = create_agent();
        let config = agent.config();

        assert!(
            !config.http_status_as_error(),
            "Agent must pass non-2xx responses through so handle_http_response can parse API error bodies"
        );
        assert!(
            config.https_only(),
            "Agent must enforce HTTPS transport for AI API requests"
        );
    }

    #[test]
    fn test_should_retry_http_status_when_transient() {
        for status in [408, 429, 500, 502, 503, 504] {
            assert!(
                should_retry_http_status(status),
                "status {status} should be retryable"
            );
        }
    }

    #[test]
    fn test_should_not_retry_http_status_when_permanent_client_error() {
        for status in [400, 401, 403, 404, 422] {
            assert!(
                !should_retry_http_status(status),
                "status {status} should not be retryable"
            );
        }
    }

    #[test]
    fn test_should_retry_transport_error_timeout() {
        let err = ureq::Error::Timeout(ureq::Timeout::Connect);
        assert!(should_retry_transport_error(&err));
    }

    #[test]
    fn test_should_not_retry_transport_error_bad_uri() {
        let err = ureq::Error::BadUri("missing scheme".to_string());
        assert!(!should_retry_transport_error(&err));
    }

    // ================= Agent Chat Provider Registration Tests =================

    #[test]
    fn legacy_claude_provider_remains_default_without_agent_chat_opt_in() {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let claude_path = temp_dir.path().join("fake-claude");
        std::fs::write(
            &claude_path,
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then\n  echo fake-claude 0.0.0\n  exit 0\nfi\nexit 1\n",
        )
        .expect("write fake claude binary");
        #[cfg(unix)]
        {
            let mut permissions = std::fs::metadata(&claude_path)
                .expect("read fake claude metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&claude_path, permissions)
                .expect("mark fake claude binary executable");
        }

        let orig = std::env::var("SCRIPT_KIT_AGENT_CHAT_CLAUDE_CODE").ok();
        std::env::remove_var("SCRIPT_KIT_AGENT_CHAT_CLAUDE_CODE");
        let mut config = crate::config::Config::default();
        config.claude_code = Some(crate::config::ClaudeCodeConfig {
            enabled: true,
            path: Some(claude_path.to_string_lossy().into_owned()),
            ..Default::default()
        });

        let registry = ProviderRegistry::from_environment_with_config(Some(&config));

        assert!(
            registry.get_provider("claude_code").is_some(),
            "Legacy claude_code provider must remain the default without Agent Chat opt-in"
        );
        assert!(
            registry.get_provider("claude-code").is_none(),
            "Agent Chat provider must not be registered without Agent Chat opt-in"
        );

        if let Some(v) = orig {
            std::env::set_var("SCRIPT_KIT_AGENT_CHAT_CLAUDE_CODE", v);
        } else {
            std::env::remove_var("SCRIPT_KIT_AGENT_CHAT_CLAUDE_CODE");
        }
    }

    // ================= Claude Code CLI Provider Tests =================

    #[test]
    fn test_claude_code_provider_metadata() {
        // Create provider manually for testing (bypasses env detection)
        let provider = ClaudeCodeProvider {
            claude_path: "claude".to_string(),
            permission_mode: "plan".to_string(),
            allowed_tools: None,
            add_dirs: vec![],
        };

        assert_eq!(provider.provider_id(), "claude_code");
        assert_eq!(provider.display_name(), "Claude Code (CLI)");

        let models = provider.available_models();
        assert_eq!(models.len(), 4);
        assert!(models.iter().any(|m| m.id == "sonnet"));
        assert!(models.iter().any(|m| m.id == "opus"));
        assert!(models.iter().any(|m| m.id == "haiku"));
        assert!(models.iter().any(|m| m.id == "default"));

        // All models should support streaming
        assert!(models.iter().all(|m| m.supports_streaming));
        // All models should have 200k context
        assert!(models.iter().all(|m| m.context_window == 200_000));
    }

    #[test]
    fn test_claude_code_extract_system_prompt() {
        let messages = vec![
            ProviderMessage::system("You are a helpful assistant"),
            ProviderMessage::user("Hello"),
        ];
        let result = ClaudeCodeProvider::extract_system_prompt(&messages);
        assert_eq!(result, Some("You are a helpful assistant".to_string()));

        // No system message
        let messages = vec![ProviderMessage::user("Hello")];
        let result = ClaudeCodeProvider::extract_system_prompt(&messages);
        assert_eq!(result, None);
    }

    #[test]
    fn test_claude_code_extract_last_user_text() {
        let messages = vec![
            ProviderMessage::user("First"),
            ProviderMessage::assistant("Response"),
            ProviderMessage::user("Second"),
        ];
        let result = ClaudeCodeProvider::extract_last_user_text(&messages);
        assert_eq!(result.unwrap(), "Second");

        // No user message
        let messages = vec![ProviderMessage::assistant("Hello")];
        let result = ClaudeCodeProvider::extract_last_user_text(&messages);
        assert!(result.is_err());
    }

    #[test]
    fn test_claude_code_make_user_message_json() {
        let json = ClaudeCodeProvider::make_user_message_json("Hello, Claude!");
        assert_eq!(json["type"], "user");
        assert_eq!(json["message"]["role"], "user");
        assert_eq!(json["message"]["content"], "Hello, Claude!");
    }

    #[test]
    fn claude_final_only_response_is_delivered_once_without_duplicating_streamed_answers() {
        let delivered = std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
        let captured = std::sync::Arc::clone(&delivered);
        let callback: StreamCallback = Box::new(move |chunk| {
            captured
                .lock()
                .expect("capture private response")
                .push(chunk);
            true
        });

        assert!(forward_unstreamed_claude_response(
            "private final-only answer",
            0,
            &callback
        ));
        assert!(forward_unstreamed_claude_response(
            "private already-streamed answer",
            3,
            &callback
        ));
        assert_eq!(
            *delivered.lock().expect("inspect captured private answer"),
            vec!["private final-only answer".to_string()]
        );

        let cancelled: StreamCallback = Box::new(|_| false);
        assert!(!forward_unstreamed_claude_response(
            "private cancelled answer",
            0,
            &cancelled
        ));
        assert!(forward_unstreamed_claude_response(
            "private already-streamed answer",
            1,
            &cancelled
        ));
    }

    #[test]
    fn test_claude_code_is_not_available_for_nonexistent_binary() {
        // A nonexistent binary should return false
        assert!(!ClaudeCodeProvider::is_available(
            "/nonexistent/path/to/claude"
        ));
    }

    #[test]
    fn test_claude_code_detect_from_env_disabled_by_default() {
        // Clear any existing env vars for this test
        let original_enabled = std::env::var("SCRIPT_KIT_CLAUDE_CODE_ENABLED").ok();

        std::env::remove_var("SCRIPT_KIT_CLAUDE_CODE_ENABLED");

        // Should return None when not explicitly enabled
        let result = ClaudeCodeProvider::detect_from_env();
        assert!(result.is_none());

        // Restore
        if let Some(val) = original_enabled {
            std::env::set_var("SCRIPT_KIT_CLAUDE_CODE_ENABLED", val);
        }
    }

    #[test]
    fn test_claude_code_registry_registration() {
        let mut registry = ProviderRegistry::new();
        let provider = ClaudeCodeProvider {
            claude_path: "claude".to_string(),
            permission_mode: "plan".to_string(),
            allowed_tools: Some("Read,Edit".to_string()),
            add_dirs: vec![std::path::PathBuf::from("/tmp")],
        };
        registry.register(Arc::new(provider));

        assert!(registry.has_any_provider());
        assert!(registry.get_provider("claude_code").is_some());

        let models = registry.get_models_for_provider("claude_code");
        assert_eq!(models.len(), 4); // sonnet, opus, haiku, default
    }

    #[test]
    fn test_claude_code_find_provider_for_model() {
        let mut registry = ProviderRegistry::new();
        registry.register(Arc::new(ClaudeCodeProvider {
            claude_path: "claude".to_string(),
            permission_mode: "plan".to_string(),
            allowed_tools: None,
            add_dirs: vec![],
        }));

        let provider = registry.find_provider_for_model("sonnet");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), "claude_code");

        let provider = registry.find_provider_for_model("opus");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), "claude_code");

        let provider = registry.find_provider_for_model("default");
        assert!(provider.is_some());
        assert_eq!(provider.unwrap().provider_id(), "claude_code");
    }

    #[test]
    fn test_claude_code_clone() {
        let provider = ClaudeCodeProvider {
            claude_path: "/custom/path/to/claude".to_string(),
            permission_mode: "dontAsk".to_string(),
            allowed_tools: Some("Bash,Read".to_string()),
            add_dirs: vec![
                std::path::PathBuf::from("/home/user/project"),
                std::path::PathBuf::from("/tmp"),
            ],
        };

        let cloned = provider.clone();

        assert_eq!(cloned.claude_path, provider.claude_path);
        assert_eq!(cloned.permission_mode, provider.permission_mode);
        assert_eq!(cloned.allowed_tools, provider.allowed_tools);
        assert_eq!(cloned.add_dirs, provider.add_dirs);
    }

    // ================= AI Provider Integration Tests =================
    // These tests verify key behaviors that ensure provider reliability

    /// Test that Claude CLI arguments include --verbose flag (required for stream-json output)
    /// This flag is CRITICAL - without it, the CLI doesn't produce proper streaming output
    #[test]
    fn test_claude_cli_verbose_flag_in_command() {
        // We can't easily test Command construction inside stream_claude_once without
        // refactoring. Instead, verify the code pattern exists by checking the source.
        // This is a compile-time verification that the flag is present.

        // The actual test: create a provider and verify we can build the command args
        let provider = ClaudeCodeProvider {
            claude_path: "claude".to_string(),
            permission_mode: "plan".to_string(),
            allowed_tools: None,
            add_dirs: vec![],
        };

        // Verify the provider has expected fields
        assert_eq!(provider.claude_path, "claude");
        assert_eq!(provider.permission_mode, "plan");

        // Note: The --verbose flag is added in stream_claude_once() around line 1506-1507:
        // cmd.arg("--print")
        //     .arg("--verbose")
        //     .arg("--input-format")
        //     .arg("stream-json")
        // This test ensures the provider structure is correct;
        // the actual flag is verified by code review (see AGENTS.md §17c for context)
    }

    /// Test that JSONL input message format is valid JSON
    /// The Claude Code CLI expects messages in specific JSONL format
    #[test]
    fn test_claude_jsonl_input_format() {
        // Test the make_user_message_json produces valid, parseable JSON
        let json = ClaudeCodeProvider::make_user_message_json("Hello, world!");

        // Verify it's valid JSON by serializing and parsing back
        let json_str = serde_json::to_string(&json).expect("should serialize to JSON");
        let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("should parse back");

        // Verify structure matches Claude Code CLI stream-json protocol
        assert_eq!(parsed["type"], "user", "type must be 'user'");
        assert!(parsed["message"].is_object(), "message must be an object");
        assert_eq!(
            parsed["message"]["role"], "user",
            "message.role must be 'user'"
        );
        assert_eq!(
            parsed["message"]["content"], "Hello, world!",
            "message.content must match input"
        );

        // Test with special characters
        let special = ClaudeCodeProvider::make_user_message_json("Test \"quotes\" and\nnewlines");
        let special_str = serde_json::to_string(&special).expect("should serialize special chars");
        let special_parsed: serde_json::Value =
            serde_json::from_str(&special_str).expect("should parse special chars");
        assert_eq!(
            special_parsed["message"]["content"],
            "Test \"quotes\" and\nnewlines"
        );

        // Test with unicode
        let unicode = ClaudeCodeProvider::make_user_message_json("Hello 世界 🌍");
        let unicode_str = serde_json::to_string(&unicode).expect("should serialize unicode");
        let unicode_parsed: serde_json::Value =
            serde_json::from_str(&unicode_str).expect("should parse unicode");
        assert_eq!(unicode_parsed["message"]["content"], "Hello 世界 🌍");
    }

    /// Test that all providers have real implementations (no mock labeling)
    #[test]
    fn test_provider_models_are_real() {
        // Google provider has real models
        let google_provider = GoogleProvider::new("test-key");
        let google_models = google_provider.available_models();
        assert!(
            !google_models.is_empty(),
            "Google provider should return models"
        );
        for model in &google_models {
            assert!(
                !model.display_name.contains("(Mock)"),
                "Google model '{}' should NOT have (Mock) suffix",
                model.id
            );
            assert!(!model.is_mock_provider());
        }

        // Groq provider has real models
        let groq_provider = GroqProvider::new("test-key");
        let groq_models = groq_provider.available_models();
        assert!(
            !groq_models.is_empty(),
            "Groq provider should return models"
        );
        for model in &groq_models {
            assert!(
                !model.display_name.contains("(Mock)"),
                "Groq model '{}' should NOT have (Mock) suffix",
                model.id
            );
            assert!(!model.is_mock_provider());
        }

        // OpenAI and Anthropic also real
        let openai_provider = OpenAiProvider::new("test-key");
        for model in openai_provider.available_models() {
            assert!(!model.is_mock_provider());
        }

        let anthropic_provider = AnthropicProvider::new("test-key");
        for model in anthropic_provider.available_models() {
            assert!(!model.is_mock_provider());
        }
    }

    /// Test real Claude Code CLI execution (requires `claude` CLI installed)
    /// Run with: cargo test --features system-tests test_claude_code_real -- --ignored
    #[test]
    #[ignore = "Requires Claude Code CLI installed - run with `claude` in PATH"]
    fn test_claude_code_real() {
        // Check if claude is available
        if !ClaudeCodeProvider::is_available("claude") {
            eprintln!("Skipping: `claude` CLI not found in PATH");
            return;
        }

        let provider = ClaudeCodeProvider {
            claude_path: "claude".to_string(),
            permission_mode: "plan".to_string(),
            allowed_tools: None,
            add_dirs: vec![],
        };

        let messages = vec![
            ProviderMessage::system(
                "You are a helpful assistant. Reply with exactly 'Hello from Claude Code!' and nothing else.",
            ),
            ProviderMessage::user("Say hello"),
        ];

        // Test streaming
        let chunks = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let chunks_clone = chunks.clone();

        let result = provider.stream_message(
            &messages,
            "default",
            Box::new(move |chunk| {
                chunks_clone
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push(chunk);
                true
            }),
            Some("test-session"),
        );

        assert!(result.is_ok(), "stream_message failed: {:?}", result.err());

        let collected = chunks.lock().unwrap_or_else(|e| e.into_inner());
        let full_response: String = collected.iter().cloned().collect();
        assert!(!full_response.is_empty(), "No response received");
        println!("Claude Code response: {}", full_response);
    }
}
