//! HTTP failure presentation and redaction shared by AI providers.

use anyhow::{anyhow, Result};

/// Extract a user-friendly error message from an API error response body.
///
/// Tries to parse JSON error responses from various AI providers and extract
/// the most useful error message for display to users.
pub(super) fn extract_api_error_message(body: &str) -> Option<String> {
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

pub(super) fn safe_provider_diagnostic_detail(raw: &str) -> String {
    crate::ai::reliability::redact_diagnostic(raw)
        .copyable_detail
        .unwrap_or_else(|| "Provider diagnostic details were redacted".to_string())
}

pub(super) fn provider_http_failure_message(
    status: u16,
    provider_name: &str,
    body: &str,
) -> String {
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
pub(super) fn handle_http_response(
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
    let diagnostic = crate::ai::reliability::redact_diagnostic(&body_str);

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
pub(super) fn simplify_auth_error(detail: &str) -> String {
    // Vercel OIDC errors are very verbose - simplify them
    if detail.contains("OIDC") || detail.contains("VERCEL_OIDC_TOKEN") {
        return "Vercel AI Gateway requires OIDC authentication. This is only available when running on Vercel. For local development, use direct API keys (SCRIPT_KIT_ANTHROPIC_API_KEY, SCRIPT_KIT_OPENAI_API_KEY).".to_string();
    }
    detail.to_string()
}
