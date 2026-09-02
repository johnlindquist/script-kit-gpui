//! AI provider configuration and environment variable detection.
//!
//! This module handles automatic discovery of AI provider API keys from environment
//! variables and the system keyring using the `SCRIPT_KIT_*_API_KEY` pattern for security.
//!
//! # Key Detection Order
//!
//! API keys are detected in the following order (first found wins):
//! 1. Environment variable (`SCRIPT_KIT_*_API_KEY`)
//! 2. System keyring (macOS Keychain via `com.scriptkit.env` service)
//!
//! This allows users to either set environment variables in their shell profile
//! or use the built-in "Configure API Key" commands which store in the keyring.
//!
//! # Environment Variable Pattern
//!
//! API keys are detected with the `SCRIPT_KIT_` prefix:
//! - `SCRIPT_KIT_OPENAI_API_KEY` -> OpenAI provider
//! - `SCRIPT_KIT_ANTHROPIC_API_KEY` -> Anthropic provider
//! - `SCRIPT_KIT_VERCEL_API_KEY` -> Vercel AI Gateway
//!
//! This prefix ensures users explicitly configure keys for Script Kit,
//! rather than accidentally exposing keys from other applications.

use std::env;
use std::fmt;

use crate::secrets::get_secret;

/// Wrapper type for secret values that always redacts in logs.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SecretString(String);

impl SecretString {
    /// Create a new secret string.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Expose the underlying secret value for API usage only.
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// Return true when the underlying secret is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Consume and return the underlying string.
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<SecretString> for String {
    fn from(value: SecretString) -> Self {
        value.0
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        self.expose()
    }
}

/// Represents a detected AI provider configuration.
#[derive(Clone)]
pub struct ProviderConfig {
    /// Unique identifier for the provider (e.g., "openai", "anthropic")
    pub provider_id: String,
    /// Human-readable name (e.g., "OpenAI", "Anthropic")
    pub display_name: String,
    /// The API key (should never be logged or displayed)
    api_key: SecretString,
    /// Base URL for the API (for custom endpoints)
    pub base_url: Option<String>,
}

impl ProviderConfig {
    /// Create a new provider configuration.
    pub fn new(
        provider_id: impl Into<String>,
        display_name: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            display_name: display_name.into(),
            api_key: SecretString::new(api_key),
            base_url: None,
        }
    }

    /// Create a provider configuration with a custom base URL.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = Some(base_url.into());
        self
    }

    /// Get the API key for making requests.
    ///
    /// # Security Note
    /// This method intentionally returns a reference to prevent accidental
    /// copies of the API key. Never log or display the returned value.
    pub fn api_key(&self) -> &str {
        self.api_key.expose()
    }

    /// Check if this provider has a valid (non-empty) API key.
    pub fn has_valid_key(&self) -> bool {
        !self.api_key.is_empty()
    }
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderConfig")
            .field("provider_id", &self.provider_id)
            .field("display_name", &self.display_name)
            .field("api_key", &"<redacted>")
            .field("base_url", &self.base_url)
            .finish()
    }
}

/// Information about an AI model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Unique identifier for the model (e.g., "gpt-4o", "claude-3-5-sonnet")
    pub id: String,
    /// Human-readable display name
    pub display_name: String,
    /// Provider this model belongs to
    pub provider: String,
    /// Whether this model supports streaming responses
    pub supports_streaming: bool,
    /// Context window size in tokens
    pub context_window: u32,
}

impl ModelInfo {
    /// Create a new model info.
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        provider: impl Into<String>,
        supports_streaming: bool,
        context_window: u32,
    ) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            provider: provider.into(),
            supports_streaming,
            context_window,
        }
    }

    /// Check if this model is from a mock/placeholder provider.
    ///
    /// Currently all providers have real API implementations, so this always returns false.
    /// Kept for backwards compatibility in case future providers are added as stubs.
    pub fn is_mock_provider(&self) -> bool {
        false
    }

    /// Log a warning if this is a mock provider model.
    /// Currently a no-op since all providers have real implementations.
    pub fn warn_if_mock(&self) {
        // No-op: all providers are real
    }
}

/// Environment variable names for API keys.
pub mod env_vars {
    /// OpenAI API key environment variable
    pub const OPENAI_API_KEY: &str = "SCRIPT_KIT_OPENAI_API_KEY";
    /// Anthropic API key environment variable
    pub const ANTHROPIC_API_KEY: &str = "SCRIPT_KIT_ANTHROPIC_API_KEY";
    /// Google AI (Gemini) API key environment variable
    pub const GOOGLE_API_KEY: &str = "SCRIPT_KIT_GOOGLE_API_KEY";
    /// Groq API key environment variable
    pub const GROQ_API_KEY: &str = "SCRIPT_KIT_GROQ_API_KEY";
    /// Vercel API key environment variable
    pub const VERCEL_API_KEY: &str = "SCRIPT_KIT_VERCEL_API_KEY";

    // Claude Code CLI configuration
    /// Path to the `claude` CLI binary (default: "claude")
    pub const CLAUDE_CODE_PATH: &str = "SCRIPT_KIT_CLAUDE_PATH";
    /// Permission mode for Claude Code: "plan", "dontAsk", etc. (default: "plan")
    pub const CLAUDE_CODE_PERMISSION_MODE: &str = "SCRIPT_KIT_CLAUDE_PERMISSION_MODE";
    /// Comma-separated list of allowed tools (e.g., "Bash(git:*),Read,Edit")
    pub const CLAUDE_CODE_ALLOWED_TOOLS: &str = "SCRIPT_KIT_CLAUDE_ALLOWED_TOOLS";
    /// Comma-separated paths to add to the Claude Code workspace
    pub const CLAUDE_CODE_ADD_DIRS: &str = "SCRIPT_KIT_CLAUDE_ADD_DIRS";
    /// Enable Claude Code CLI provider (set to "1" or "true" to enable)
    pub const CLAUDE_CODE_ENABLED: &str = "SCRIPT_KIT_CLAUDE_CODE_ENABLED";
}

/// Read an environment variable, trimming whitespace and filtering empty values.
///
/// Returns `None` if the variable is not set, is empty after trimming, or contains only whitespace.
fn read_env_nonempty(name: &str) -> Option<String> {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Credentials)
    {
        tracing::warn!(%error, "Provider environment access refused");
        return None;
    }
    env::var(name)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read an API key from environment variable or system keyring.
///
/// Checks in order:
/// 1. Environment variable (for users who set keys in shell profile)
/// 2. System keyring (for users who use built-in "Configure API Key" commands)
///
/// Returns the first non-empty value found, or `None` if not configured.
fn read_key_env_or_keyring(name: &str) -> Option<String> {
    if let Err(error) =
        crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::Credentials)
    {
        tracing::warn!(%error, "Provider credential discovery refused");
        return None;
    }
    // First check environment variable
    if let Some(value) = read_env_nonempty(name) {
        tracing::info!(target: "ai::config", key_name = %name, source = "env", "Found API key");
        return Some(value);
    }

    // Fall back to keyring
    let keyring_result = get_secret(name);
    if keyring_result.is_some() {
        tracing::info!(target: "ai::config", key_name = %name, source = "keyring", "Found API key");
    }
    keyring_result
}

/// Detected API keys from environment.
#[derive(Default)]
pub struct DetectedKeys {
    pub(crate) openai: Option<SecretString>,
    pub(crate) anthropic: Option<SecretString>,
    pub(crate) google: Option<SecretString>,
    pub(crate) groq: Option<SecretString>,
    pub(crate) vercel: Option<SecretString>,
}

impl std::fmt::Debug for DetectedKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DetectedKeys")
            .field("openai", &self.openai.is_some())
            .field("anthropic", &self.anthropic.is_some())
            .field("google", &self.google.is_some())
            .field("groq", &self.groq.is_some())
            .field("vercel", &self.vercel.is_some())
            .finish()
    }
}

impl DetectedKeys {
    /// Scan environment variables and system keyring for API keys.
    ///
    /// Looks for the `SCRIPT_KIT_*_API_KEY` pattern in:
    /// 1. Environment variables (for users who set keys in shell profile)
    /// 2. System keyring (for users who use built-in "Configure API Key" commands)
    ///
    /// This allows the SDK's `await env("SCRIPT_KIT_VERCEL_API_KEY")` to store keys
    /// in the keyring, and have Agent Chat automatically pick them up.
    pub fn from_environment() -> Self {
        Self {
            openai: read_key_env_or_keyring(env_vars::OPENAI_API_KEY).map(SecretString::from),
            anthropic: read_key_env_or_keyring(env_vars::ANTHROPIC_API_KEY).map(SecretString::from),
            google: read_key_env_or_keyring(env_vars::GOOGLE_API_KEY).map(SecretString::from),
            groq: read_key_env_or_keyring(env_vars::GROQ_API_KEY).map(SecretString::from),
            vercel: read_key_env_or_keyring(env_vars::VERCEL_API_KEY).map(SecretString::from),
        }
    }

    /// Check if any API keys were detected.
    pub fn has_any(&self) -> bool {
        self.openai.is_some()
            || self.anthropic.is_some()
            || self.google.is_some()
            || self.groq.is_some()
            || self.vercel.is_some()
    }

    /// Get a summary of which providers are available (for logging).
    ///
    /// Returns a list of provider names that have API keys configured.
    /// Does NOT include the actual keys.
    pub fn available_providers(&self) -> Vec<&'static str> {
        let mut providers = Vec::new();
        if self.openai.is_some() {
            providers.push("OpenAI");
        }
        if self.anthropic.is_some() {
            providers.push("Anthropic");
        }
        if self.google.is_some() {
            providers.push("Google");
        }
        if self.groq.is_some() {
            providers.push("Groq");
        }
        if self.vercel.is_some() {
            providers.push("Vercel");
        }
        providers
    }
}

/// Default models for each provider.
pub mod default_models {
    use super::ModelInfo;

    /// Get default OpenAI models.
    pub fn openai() -> Vec<ModelInfo> {
        vec![
            ModelInfo::new("gpt-4o", "GPT-4o", "openai", true, 128_000),
            ModelInfo::new("gpt-4o-mini", "GPT-4o Mini", "openai", true, 128_000),
            ModelInfo::new("gpt-4-turbo", "GPT-4 Turbo", "openai", true, 128_000),
            ModelInfo::new("gpt-3.5-turbo", "GPT-3.5 Turbo", "openai", true, 16_385),
        ]
    }

    /// Get default Anthropic models.
    /// NOTE: The FIRST model in this list is the default model for new chats
    pub fn anthropic() -> Vec<ModelInfo> {
        vec![
            // Default: Claude Haiku 4.5 (fast, cheap, good quality)
            ModelInfo::new(
                "claude-haiku-4-5-20250514",
                "Claude Haiku 4.5",
                "anthropic",
                true,
                200_000,
            ),
            ModelInfo::new(
                "claude-3-5-sonnet-20241022",
                "Claude 3.5 Sonnet",
                "anthropic",
                true,
                200_000,
            ),
            ModelInfo::new(
                "claude-3-opus-20240229",
                "Claude 3 Opus",
                "anthropic",
                true,
                200_000,
            ),
        ]
    }

    /// Get default Google (Gemini) models.
    pub fn google() -> Vec<ModelInfo> {
        vec![
            ModelInfo::new(
                "gemini-2.0-flash",
                "Gemini 2.0 Flash",
                "google",
                true,
                1_000_000,
            ),
            ModelInfo::new(
                "gemini-2.0-flash-lite",
                "Gemini 2.0 Flash Lite",
                "google",
                true,
                1_000_000,
            ),
            ModelInfo::new(
                "gemini-1.5-pro",
                "Gemini 1.5 Pro",
                "google",
                true,
                2_000_000,
            ),
            ModelInfo::new(
                "gemini-1.5-flash",
                "Gemini 1.5 Flash",
                "google",
                true,
                1_000_000,
            ),
        ]
    }

    /// Get default Groq models.
    pub fn groq() -> Vec<ModelInfo> {
        vec![
            ModelInfo::new(
                "llama-3.3-70b-versatile",
                "Llama 3.3 70B Versatile",
                "groq",
                true,
                128_000,
            ),
            ModelInfo::new(
                "llama-3.1-8b-instant",
                "Llama 3.1 8B Instant",
                "groq",
                true,
                128_000,
            ),
            ModelInfo::new("mixtral-8x7b-32768", "Mixtral 8x7B", "groq", true, 32_768),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_config_creation() {
        let config = ProviderConfig::new("openai", "OpenAI", "sk-test-key");
        assert_eq!(config.provider_id, "openai");
        assert_eq!(config.display_name, "OpenAI");
        assert_eq!(config.api_key(), "sk-test-key");
        assert!(config.has_valid_key());
    }

    #[test]
    fn test_provider_config_empty_key() {
        let config = ProviderConfig::new("openai", "OpenAI", "");
        assert!(!config.has_valid_key());
    }

    #[test]
    fn test_provider_config_with_base_url() {
        let config = ProviderConfig::new("openai", "OpenAI", "sk-test")
            .with_base_url("https://api.custom.com");
        assert_eq!(config.base_url, Some("https://api.custom.com".to_string()));
    }

    #[test]
    fn test_model_info_creation() {
        let model = ModelInfo::new("gpt-4o", "GPT-4o", "openai", true, 128_000);
        assert_eq!(model.id, "gpt-4o");
        assert_eq!(model.display_name, "GPT-4o");
        assert_eq!(model.provider, "openai");
        assert!(model.supports_streaming);
        assert_eq!(model.context_window, 128_000);
    }

    #[test]
    fn test_detected_keys_empty() {
        // Clear any existing env vars for this test
        let keys = DetectedKeys::default();
        assert!(!keys.has_any());
        assert!(keys.available_providers().is_empty());
    }

    #[test]
    fn test_detected_keys_with_provider() {
        // Manually construct to avoid env dependency in test
        let keys = DetectedKeys {
            openai: Some("sk-test".into()),
            anthropic: None,
            google: None,
            groq: None,
            vercel: None,
        };
        assert!(keys.has_any());
        assert_eq!(keys.available_providers(), vec!["OpenAI"]);
    }

    #[test]
    fn test_default_models() {
        let openai_models = default_models::openai();
        assert!(!openai_models.is_empty());
        assert!(openai_models.iter().any(|m| m.id == "gpt-4o"));

        let anthropic_models = default_models::anthropic();
        assert!(!anthropic_models.is_empty());
        assert!(anthropic_models.iter().any(|m| m.id.contains("claude")));
    }

    #[test]
    fn test_anthropic_default_models_have_unique_model_ids() {
        let models = default_models::anthropic();
        let mut unique_ids = std::collections::HashSet::new();

        for model in &models {
            assert!(
                unique_ids.insert(model.id.clone()),
                "Duplicate Anthropic model ID found: {}",
                model.id
            );
        }
    }

    #[test]
    fn test_provider_config_debug_redacts_api_key() {
        let config = ProviderConfig::new("openai", "OpenAI", "sk-super-secret-key-12345");
        let debug_output = format!("{:?}", config);

        // The API key should NOT appear in debug output
        assert!(!debug_output.contains("sk-super-secret-key-12345"));
        // Instead, it should show <redacted>
        assert!(debug_output.contains("<redacted>"));
        // Other fields should still be visible
        assert!(debug_output.contains("openai"));
        assert!(debug_output.contains("OpenAI"));
    }

    #[test]
    fn test_detected_keys_debug_shows_only_presence() {
        let keys = DetectedKeys {
            openai: Some("sk-secret-openai-key".into()),
            anthropic: Some("sk-ant-secret-key".into()),
            google: None,
            groq: None,
            vercel: Some("vk-vercel-key".into()),
        };
        let debug_output = format!("{:?}", keys);

        // Actual key values should NOT appear
        assert!(!debug_output.contains("sk-secret-openai-key"));
        assert!(!debug_output.contains("sk-ant-secret-key"));
        assert!(!debug_output.contains("vk-vercel-key"));
        // Should show boolean presence indicators
        assert!(debug_output.contains("openai: true"));
        assert!(debug_output.contains("anthropic: true"));
        assert!(debug_output.contains("google: false"));
        assert!(debug_output.contains("vercel: true"));
    }

    #[test]
    fn test_read_env_nonempty_trims_whitespace() {
        // Set up test env var with whitespace
        std::env::set_var("TEST_READ_ENV_WHITESPACE", "  test-value  ");
        let result = read_env_nonempty("TEST_READ_ENV_WHITESPACE");
        assert_eq!(result, Some("test-value".to_string()));
        std::env::remove_var("TEST_READ_ENV_WHITESPACE");
    }

    #[test]
    fn test_read_env_nonempty_filters_empty() {
        // Set up test env var that's empty
        std::env::set_var("TEST_READ_ENV_EMPTY", "");
        let result = read_env_nonempty("TEST_READ_ENV_EMPTY");
        assert_eq!(result, None);
        std::env::remove_var("TEST_READ_ENV_EMPTY");
    }

    #[test]
    fn test_read_env_nonempty_filters_whitespace_only() {
        // Set up test env var with only whitespace
        std::env::set_var("TEST_READ_ENV_WS_ONLY", "   ");
        let result = read_env_nonempty("TEST_READ_ENV_WS_ONLY");
        assert_eq!(result, None);
        std::env::remove_var("TEST_READ_ENV_WS_ONLY");
    }

    #[test]
    fn test_detected_keys_with_vercel() {
        let keys = DetectedKeys {
            openai: None,
            anthropic: None,
            google: None,
            groq: None,
            vercel: Some("vk-test".into()),
        };
        assert!(keys.has_any());
        assert_eq!(keys.available_providers(), vec!["Vercel"]);
    }

    #[test]
    fn test_secret_string_debug_masks_value() {
        let secret = SecretString::new("sk-test-super-secret");
        assert_eq!(format!("{:?}", secret), "***");
    }

    #[test]
    fn test_secret_string_display_masks_value() {
        let secret = SecretString::new("sk-test-super-secret");
        assert_eq!(format!("{}", secret), "***");
    }

    #[test]
    fn test_secret_string_expose_returns_inner_value() {
        let secret = SecretString::new("sk-test-super-secret");
        assert_eq!(secret.expose(), "sk-test-super-secret");
    }

    #[test]
    fn test_is_not_mock_provider_google() {
        let model = ModelInfo::new(
            "gemini-2.0-flash",
            "Gemini 2.0 Flash",
            "google",
            true,
            1_000_000,
        );
        assert!(!model.is_mock_provider());
    }

    #[test]
    fn test_is_not_mock_provider_groq() {
        let model = ModelInfo::new(
            "llama-3.3-70b-versatile",
            "Llama 3.3 70B Versatile",
            "groq",
            true,
            128_000,
        );
        assert!(!model.is_mock_provider());
    }

    #[test]
    fn test_is_not_mock_provider_openai() {
        let model = ModelInfo::new("gpt-4o", "GPT-4o", "openai", true, 128_000);
        assert!(!model.is_mock_provider());
    }

    #[test]
    fn test_is_not_mock_provider_anthropic() {
        let model = ModelInfo::new(
            "claude-3-5-sonnet",
            "Claude 3.5 Sonnet",
            "anthropic",
            true,
            200_000,
        );
        assert!(!model.is_mock_provider());
    }

    #[test]
    fn test_google_models_always_available() {
        let google_models = default_models::google();
        assert!(
            !google_models.is_empty(),
            "Google models should always be available"
        );
        assert!(google_models.iter().any(|m| m.id == "gemini-2.0-flash"));
    }

    #[test]
    fn test_groq_models_always_available() {
        let groq_models = default_models::groq();
        assert!(
            !groq_models.is_empty(),
            "Groq models should always be available"
        );
        assert!(groq_models
            .iter()
            .any(|m| m.id == "llama-3.3-70b-versatile"));
    }
}
