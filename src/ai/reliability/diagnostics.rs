use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use sk_protocol::ai_reliability::{
    DiagnosticAvailability, DiagnosticDescriptor, DiagnosticId, DiagnosticRedaction,
    DiagnosticVisibility, Fingerprint,
};
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

const MAX_DIAGNOSTIC_BYTES: usize = 2_048;
const ALLOWLISTED_JSON_KEYS: [&str; 7] = [
    "type",
    "status",
    "code",
    "message",
    "model",
    "component",
    "error",
];
type DiagnosticRedactionRules = Vec<(regex::Regex, &'static str)>;
static DIAGNOSTIC_REDACTION_RULES: OnceLock<Option<DiagnosticRedactionRules>> = OnceLock::new();

/// Safe secondary detail retained for Copy Details.
///
/// `copyable_detail` is already redacted and length-bounded. The raw input is
/// deliberately not retained by this type or by [`DiagnosticVault`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedactedDiagnostic {
    pub fingerprint: Fingerprint,
    pub copyable_detail: Option<String>,
    pub truncated: bool,
    pub suppressed: bool,
}

#[derive(Debug, Default)]
pub struct DiagnosticVault {
    entries: Mutex<HashMap<DiagnosticId, RedactedDiagnostic>>,
}

impl DiagnosticVault {
    pub fn capture(&self, raw: &str) -> DiagnosticDescriptor {
        let diagnostic = redact_diagnostic(raw);
        let id = DiagnosticId(format!("ai-{}", &diagnostic.fingerprint.0[..16]));
        let availability = if diagnostic.copyable_detail.is_some() {
            DiagnosticAvailability::Available
        } else {
            DiagnosticAvailability::FingerprintOnly
        };
        self.entries.lock().insert(id.clone(), diagnostic.clone());
        DiagnosticDescriptor {
            id,
            fingerprint: diagnostic.fingerprint,
            availability,
            visibility: DiagnosticVisibility::SecondaryOnly,
            redaction: if diagnostic.copyable_detail.is_some() {
                DiagnosticRedaction::AllowlistedFieldsV1
            } else {
                DiagnosticRedaction::HashOnly
            },
        }
    }

    pub fn get(&self, id: &DiagnosticId) -> Option<RedactedDiagnostic> {
        self.entries.lock().get(id).cloned()
    }
}

pub fn redact_diagnostic(raw: &str) -> RedactedDiagnostic {
    let fingerprint = Fingerprint(hex_sha256(raw));
    let allowlisted = match serde_json::from_str::<serde_json::Value>(raw) {
        Ok(value) => allowlist_json(&value)
            .and_then(|value| serde_json::to_string(&value).ok())
            .unwrap_or_else(|| "[REDACTED]".to_string()),
        Err(_) => raw.to_string(),
    };
    let redacted = redact_secrets_and_paths(&allowlisted);
    let compact = redacted.trim();
    let suppressed = compact.is_empty() || contains_only_secret_placeholder(compact);
    let (copyable_detail, truncated) = if suppressed {
        (None, false)
    } else {
        let (bounded, was_truncated) = truncate_utf8(compact, MAX_DIAGNOSTIC_BYTES);
        (Some(bounded), was_truncated)
    };
    RedactedDiagnostic {
        fingerprint,
        copyable_detail,
        truncated,
        suppressed,
    }
}

fn allowlist_json(value: &serde_json::Value) -> Option<serde_json::Value> {
    match value {
        serde_json::Value::Object(object) => {
            let mut safe = serde_json::Map::new();
            for key in ALLOWLISTED_JSON_KEYS {
                let Some(value) = object.get(key) else {
                    continue;
                };
                let retained = match value {
                    serde_json::Value::Object(_) => allowlist_json(value),
                    serde_json::Value::Array(_) | serde_json::Value::Null => None,
                    serde_json::Value::Bool(_)
                    | serde_json::Value::Number(_)
                    | serde_json::Value::String(_) => Some(value.clone()),
                };
                if let Some(retained) = retained {
                    safe.insert(key.to_string(), retained);
                }
            }
            if safe.is_empty() {
                None
            } else {
                Some(serde_json::Value::Object(safe))
            }
        }
        serde_json::Value::Array(_) | serde_json::Value::Null => None,
        serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => Some(value.clone()),
    }
}

fn redact_secrets_and_paths(input: &str) -> String {
    let mut output = input.to_string();
    if let Some(home) = dirs::home_dir().and_then(|path| path.to_str().map(str::to_owned)) {
        output = output.replace(&home, "~");
    }

    let rules = DIAGNOSTIC_REDACTION_RULES.get_or_init(|| {
        [
            (
                r#"(?i)(authorization\s*[:=]\s*)(?:bearer\s+)?[^\s,;"}]+"#,
                "${1}[REDACTED]",
            ),
            (r#"(?i)(bearer\s+)[^\s,;"}]+"#, "${1}[REDACTED]"),
            (
                r#"(?i)(cookie\s*[:=]\s*)[^\r\n,;"}]+"#,
                "${1}[REDACTED]",
            ),
            (
                r#"(?i)((?:api[_-]?key|token|secret|password|passwd|passphrase|credential)\s*[:=]\s*)["']?[^"'\s,;}]+["']?"#,
                "${1}[REDACTED]",
            ),
            (
                r#"(?i)("(?:api[_-]?key|token|secret|password|passwd|passphrase|credential|authorization|cookie)"\s*:\s*)"[^"]*""#,
                "${1}[REDACTED]",
            ),
            (
                r#"(?s)-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----.*?-----END [A-Z0-9 ]*PRIVATE KEY-----"#,
                "[REDACTED]",
            ),
            (
                r#"(?i)\b(?:sk-(?:proj-[a-z0-9_-]{8,}|ant-api\d{2}-[a-z0-9_-]{8,}|[a-z0-9_-]{20,})|gsk_[a-z0-9_-]{8,}|gh[pousr]_[a-z0-9_]{8,}|github_pat_[a-z0-9_]{8,}|xox[baprs]-[a-z0-9-]{8,}|AIza[a-z0-9_-]{20,})\b"#,
                "[REDACTED]",
            ),
            (r#"/Users/[^/\s"\\]+"#, "~"),
            (r#"/home/[^/\s"\\]+"#, "~"),
        ]
        .into_iter()
        .map(|(pattern, replacement)| {
            regex::Regex::new(pattern).map(|regex| (regex, replacement))
        })
        .collect::<Result<DiagnosticRedactionRules, _>>()
        .ok()
    });
    let Some(rules) = rules else {
        return "[REDACTED]".to_string();
    };
    for (regex, replacement) in rules {
        output = regex.replace_all(&output, *replacement).into_owned();
    }
    output
}

fn contains_only_secret_placeholder(value: &str) -> bool {
    let stripped: String = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    stripped.eq_ignore_ascii_case("redacted")
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (String, bool) {
    if value.len() <= max_bytes {
        return (value.to_string(), false);
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}…", &value[..end]), true)
}

fn hex_sha256(value: &str) -> String {
    const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";
    let bytes = Sha256::digest(value.as_bytes());
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX_DIGITS[(byte >> 4) as usize]));
        output.push(char::from(HEX_DIGITS[(byte & 0x0f) as usize]));
    }
    output
}

pub(crate) fn safe_parameters(
    pairs: impl IntoIterator<Item = (&'static str, Option<String>)>,
) -> BTreeMap<String, String> {
    pairs
        .into_iter()
        .filter_map(|(key, value)| value.map(|value| (key.to_string(), value)))
        .collect()
}
