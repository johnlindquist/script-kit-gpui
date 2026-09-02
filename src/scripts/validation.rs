//! Startup-time validation for loaded scripts.
//!
//! Oracle-Session `script-metadata-validation-fail-fast` PR1: introduce the
//! validation surface + collision detection. The goal is to make metadata
//! problems — especially duplicate `shortcut`, `alias`, `keyword`, or
//! `trigger` declarations — visible at load time instead of silently racing
//! at dispatch.
//!
//! This PR is the foundation: it defines the report types and a single
//! `validate_script_catalog` entry point that takes an already-loaded
//! `Vec<Arc<Script>>` and produces a [`ScriptCatalogReport`]. Follow-ups
//! plumb typed-metadata parse errors through the loader and expose a
//! `kit://failed-scripts` MCP resource + menu-bar badge count.
//!
//! Usage:
//!
//! ```ignore
//! let scripts = scripts::read_scripts()?;
//! let report = scripts::validate_script_catalog(scripts);
//! if !report.validation.failed_scripts.is_empty() {
//!     tracing::warn!(
//!         fatal = report.validation.fatal_count,
//!         warnings = report.validation.warning_count,
//!         "script_validation_found_failures",
//!     );
//! }
//! ```

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};

use serde::{Deserialize, Serialize};

use super::types::{Script, Scriptlet};
use crate::metadata_parser::TypedMetadata;

/// Current schema version of the `ValidationReport` payload.
pub const VALIDATION_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ValidationSeverity {
    Fatal,
    Warning,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum BindingKind {
    Shortcut,
    Alias,
    Keyword,
    Trigger,
}

impl BindingKind {
    pub fn as_metadata_field(self) -> MetadataField {
        match self {
            BindingKind::Shortcut => MetadataField::Shortcut,
            BindingKind::Alias => MetadataField::Alias,
            BindingKind::Keyword => MetadataField::Keyword,
            BindingKind::Trigger => MetadataField::Trigger,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MetadataField {
    Metadata,
    Schema,
    Name,
    Alias,
    Keyword,
    Trigger,
    Shortcut,
    Cron,
    Schedule,
    Watch,
    Capability,
    ExecutionTopology,
    Unknown,
}

/// Discriminated failure kind. Serialized tag-first so operator tooling can
/// switch on `kind.kind` without knowing the full enum shape.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ScriptValidationKind {
    /// Typed-metadata parser reported a non-fatal error but failed to produce
    /// a usable `TypedMetadata`. The raw detail is the parser message.
    MetadataParse { detail: String },
    /// Schema parser error. Pulls straight from `schema_parser::SchemaParseResult`.
    SchemaParse { detail: String },
    /// The field declared a value that failed shape/grammar validation.
    InvalidValue { value: String, reason: String },
    /// Two or more scripts declared the same binding (shortcut/alias/keyword/trigger).
    DuplicateBinding { binding: BindingKind, value: String },
    /// A capability is unknown, unsupported, unavailable in this execution
    /// topology, or blocked by already-known host compatibility facts.
    CapabilityUnavailable {
        capability: String,
        code: crate::mcp_resources::SdkCapabilityDiagnosticCode,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        alternatives: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RelatedScript {
    pub path: PathBuf,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptValidationIssue {
    pub severity: ValidationSeverity,
    pub path: PathBuf,
    pub script_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<MetadataField>,
    pub message: String,
    pub kind: ScriptValidationKind,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub related: Vec<RelatedScript>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FailedScript {
    pub path: PathBuf,
    pub name: String,
    pub fatal: Arc<[ScriptValidationIssue]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub schema_version: u32,
    pub total_candidates: usize,
    pub valid_count: usize,
    pub fatal_count: usize,
    pub warning_count: usize,
    pub failed_scripts: Arc<[FailedScript]>,
    pub warnings: Arc<[ScriptValidationIssue]>,
    /// Diagnostics for commands deliberately retained as visible, disabled
    /// launcher rows. Fatal entries here are not excluded `failed_scripts`,
    /// and warnings here never contaminate the script-only warning channel.
    #[serde(default)]
    pub retained_issues: Arc<[ScriptValidationIssue]>,
}

/// Stable, user-safe issue detail shared by the binary renderer and library
/// behavior tests. Raw script bodies, environment variables, and credentials
/// never enter the existing typed issue contract.
pub fn format_script_validation_issue_detail(issue: &ScriptValidationIssue) -> String {
    let field = issue
        .field
        .map(|field| format!("[{field:?}] "))
        .unwrap_or_default();
    let detail = match &issue.kind {
        ScriptValidationKind::MetadataParse { detail }
        | ScriptValidationKind::SchemaParse { detail } => detail.clone(),
        ScriptValidationKind::InvalidValue { value, reason } => {
            format!("value={value:?} — {reason}")
        }
        ScriptValidationKind::DuplicateBinding { binding, value } => {
            format!("{binding:?} duplicate: {value:?}")
        }
        ScriptValidationKind::CapabilityUnavailable {
            capability,
            code,
            alternatives,
        } => {
            let repair = if alternatives.is_empty() {
                String::new()
            } else {
                format!(" — try {}", alternatives.join(", "))
            };
            format!("{capability} ({code:?}){repair}")
        }
    };
    format!("{field}{detail}").trim().to_string()
}

/// Pure author-facing repair document. Excluded scripts, retained blocked
/// scriptlets, and pending warnings remain explicitly distinct.
pub fn format_script_validation_diagnostics(report: &ValidationReport) -> String {
    let mut output = format!(
        "Script Issues — {} excluded · {} retained issue(s) · {} fatal · {} warning(s)\n",
        report.failed_scripts.len(),
        report.retained_issues.len(),
        report.fatal_count,
        report.warning_count,
    );
    if report.failed_scripts.is_empty()
        && report.retained_issues.is_empty()
        && report.warnings.is_empty()
    {
        output.push_str("No failing scripts in this report.\n");
        return output;
    }

    for failed in report.failed_scripts.iter() {
        output.push_str(&format!(
            "\n## {}\n  path: {}\n",
            failed.name,
            failed.path.display(),
        ));
        for issue in failed.fatal.iter() {
            let field = issue
                .field
                .map(|field| format!("[{field:?}] "))
                .unwrap_or_default();
            output.push_str(&format!("  - {field}{}\n", issue.message));
            output.push_str(&format!(
                "      kind: {}\n",
                format_script_validation_issue_detail(issue)
            ));
            for related in &issue.related {
                output.push_str(&format!(
                    "      ↔ {} — {}\n",
                    related.name,
                    related.path.display()
                ));
            }
        }
    }

    for issue in report.retained_issues.iter().chain(report.warnings.iter()) {
        let status = match issue.severity {
            ValidationSeverity::Fatal => "blocked, retained in launcher",
            ValidationSeverity::Warning => "warning, retained in launcher",
        };
        output.push_str(&format!(
            "\n## {}\n  path: {}\n  status: {status}\n  - {}\n",
            issue.script_name,
            issue.path.display(),
            issue.message,
        ));
        output.push_str(&format!(
            "      kind: {}\n",
            format_script_validation_issue_detail(issue)
        ));
        for related in &issue.related {
            output.push_str(&format!(
                "      ↔ {} — {}\n",
                related.name,
                related.path.display()
            ));
        }
    }

    output
}

/// Bundles the kept scripts + the validation report into one immutable
/// artifact the startup/index publisher can consume atomically.
#[derive(Clone, Debug)]
pub struct ScriptCatalogReport {
    pub scripts: Arc<[Arc<Script>]>,
    pub validation: Arc<ValidationReport>,
}

/// Normalize a binding value for collision comparison.
///
/// Shortcuts: lowercase + collapse internal whitespace so `"Cmd Shift K"` and
/// `"cmd  shift k"` collide. Alias/keyword: lowercase + trim. This is
/// deliberately loose — we'd rather false-positive a collision than miss
/// one. Per-binding grammar validation is a follow-up.
fn normalize_binding(kind: BindingKind, raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    match kind {
        BindingKind::Shortcut => {
            let mut out = String::with_capacity(trimmed.len());
            let mut first = true;
            for token in trimmed.split_whitespace() {
                if !first {
                    out.push(' ');
                }
                first = false;
                for ch in token.chars() {
                    out.extend(ch.to_lowercase());
                }
            }
            Some(out)
        }
        BindingKind::Alias | BindingKind::Keyword | BindingKind::Trigger => {
            Some(trimmed.to_lowercase())
        }
    }
}

fn script_bindings(script: &Script) -> Vec<(BindingKind, &str)> {
    let mut out = Vec::with_capacity(4);
    if let Some(v) = script.shortcut.as_deref() {
        out.push((BindingKind::Shortcut, v));
    }
    if let Some(v) = script.alias.as_deref() {
        out.push((BindingKind::Alias, v));
    }
    if let Some(meta) = script.typed_metadata.as_ref() {
        if let Some(v) = meta.keyword.as_deref() {
            out.push((BindingKind::Keyword, v));
        }
        if let Some(v) = meta.extra.get("trigger").and_then(|v| v.as_str()) {
            out.push((BindingKind::Trigger, v));
        }
    }
    out
}

/// Detect duplicate shortcut/alias/keyword/trigger declarations across
/// the catalog. Emits one `DuplicateBinding` issue per offending script so
/// both sides show up in the failure report with pointers at each other.
pub fn detect_binding_collisions(scripts: &[Arc<Script>]) -> Vec<ScriptValidationIssue> {
    let mut buckets: HashMap<(BindingKind, String), Vec<RelatedScript>> = HashMap::new();
    for script in scripts {
        for (kind, raw) in script_bindings(script) {
            if let Some(value) = normalize_binding(kind, raw) {
                buckets
                    .entry((kind, value))
                    .or_default()
                    .push(RelatedScript {
                        path: script.path.clone(),
                        name: script.name.clone(),
                    });
            }
        }
    }

    let mut out = Vec::new();
    for ((binding, value), owners) in buckets {
        if owners.len() < 2 {
            continue;
        }
        for owner in &owners {
            let related: Vec<RelatedScript> = owners
                .iter()
                .filter(|peer| peer.path != owner.path)
                .cloned()
                .collect();
            out.push(ScriptValidationIssue {
                severity: ValidationSeverity::Fatal,
                path: owner.path.clone(),
                script_name: owner.name.clone(),
                field: Some(binding.as_metadata_field()),
                message: format!(
                    "{:?} `{}` is declared by {} scripts",
                    binding,
                    value,
                    owners.len()
                ),
                kind: ScriptValidationKind::DuplicateBinding {
                    binding,
                    value: value.clone(),
                },
                related,
            });
        }
    }
    // Sort for deterministic output — buckets iterate in hash order.
    out.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| format!("{:?}", a.kind).cmp(&format!("{:?}", b.kind)))
    });
    out
}

struct CapabilityValidationSubject<'a> {
    path: PathBuf,
    name: &'a str,
    runtime_topology: crate::mcp_resources::SdkExecutionTopology,
    enforce_scriptlet_topology: bool,
}

fn capability_validation_issue(
    subject: &CapabilityValidationSubject<'_>,
    field: MetadataField,
    message: String,
    kind: ScriptValidationKind,
) -> ScriptValidationIssue {
    let severity = if matches!(
        &kind,
        ScriptValidationKind::CapabilityUnavailable {
            code: crate::mcp_resources::SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable,
            ..
        }
    ) {
        // Unknown permission state must stay visible as pending. Removing the
        // row would falsely imply denial; execution still consults its issue.
        ValidationSeverity::Warning
    } else {
        ValidationSeverity::Fatal
    };
    ScriptValidationIssue {
        severity,
        path: subject.path.clone(),
        script_name: subject.name.to_owned(),
        field: Some(field),
        message,
        kind,
        related: Vec::new(),
    }
}

fn declared_execution_topology(
    subject: &CapabilityValidationSubject<'_>,
    value: Option<&serde_json::Value>,
) -> Result<crate::mcp_resources::SdkExecutionTopology, Box<ScriptValidationIssue>> {
    use crate::mcp_resources::SdkExecutionTopology;

    let Some(value) = value else {
        return Ok(subject.runtime_topology);
    };

    let topology = serde_json::from_value::<SdkExecutionTopology>(value.clone()).map_err(|_| {
        let rendered = value.to_string();
        Box::new(capability_validation_issue(
            subject,
            MetadataField::ExecutionTopology,
            "`executionTopology` must be typescript-script, typescript-scriptlet, typescript-scriptlet-interactive, shell-scriptlet, or python-scriptlet.".to_string(),
            ScriptValidationKind::InvalidValue {
                value: rendered,
                reason: "unknown_sdk_execution_topology".to_string(),
            },
        ))
    })?;

    let incompatible = match subject.runtime_topology {
        SdkExecutionTopology::ShellScriptlet | SdkExecutionTopology::PythonScriptlet => {
            topology != subject.runtime_topology
        }
        SdkExecutionTopology::TypeScriptScriptletInteractive
            if subject.enforce_scriptlet_topology =>
        {
            !matches!(
                topology,
                SdkExecutionTopology::TypeScriptScriptletInteractive
                    | SdkExecutionTopology::TypeScriptScriptlet
            )
        }
        SdkExecutionTopology::TypeScriptScriptlet if subject.enforce_scriptlet_topology => {
            topology != SdkExecutionTopology::TypeScriptScriptlet
        }
        _ => false,
    };
    if incompatible {
        return Err(Box::new(capability_validation_issue(
            subject,
            MetadataField::ExecutionTopology,
            "`executionTopology` does not match this command's actual execution transport."
                .to_string(),
            ScriptValidationKind::InvalidValue {
                value: value.to_string(),
                reason: "sdk_execution_topology_mismatch".to_string(),
            },
        )));
    }

    Ok(topology)
}

fn validate_metadata_capabilities(
    subject: &CapabilityValidationSubject<'_>,
    metadata: &TypedMetadata,
    availability: Option<&crate::mcp_resources::SdkHostAvailability>,
) -> Vec<ScriptValidationIssue> {
    // A topology is an independent author declaration. Validate it even when
    // this command does not currently enumerate any SDK capabilities.
    let topology =
        match declared_execution_topology(subject, metadata.extra.get("executionTopology")) {
            Ok(topology) => topology,
            Err(issue) => return vec![*issue],
        };
    let Some(declared) = metadata.extra.get("sdkCapabilities") else {
        return Vec::new();
    };

    let Some(capabilities) = declared.as_array() else {
        return vec![capability_validation_issue(
            subject,
            MetadataField::Capability,
            "`sdkCapabilities` must be an array of SDK capability names.".to_string(),
            ScriptValidationKind::InvalidValue {
                value: declared.to_string(),
                reason: "sdk_capabilities_must_be_array".to_string(),
            },
        )];
    };

    let mut seen = HashSet::with_capacity(capabilities.len());
    let mut issues = Vec::new();
    for capability in capabilities {
        let Some(name) = capability.as_str() else {
            issues.push(capability_validation_issue(
                subject,
                MetadataField::Capability,
                "Each SDK capability declaration must be a nonempty string.".to_string(),
                ScriptValidationKind::InvalidValue {
                    value: capability.to_string(),
                    reason: "sdk_capability_must_be_string".to_string(),
                },
            ));
            continue;
        };
        if name.trim().is_empty() || name != name.trim() {
            issues.push(capability_validation_issue(
                subject,
                MetadataField::Capability,
                "SDK capability names must be nonempty and cannot include surrounding whitespace."
                    .to_string(),
                ScriptValidationKind::InvalidValue {
                    value: name.to_string(),
                    reason: "invalid_sdk_capability_name".to_string(),
                },
            ));
            continue;
        }
        if !seen.insert(name) {
            issues.push(capability_validation_issue(
                subject,
                MetadataField::Capability,
                format!("SDK capability `{name}` is declared more than once."),
                ScriptValidationKind::InvalidValue {
                    value: name.to_string(),
                    reason: "duplicate_sdk_capability".to_string(),
                },
            ));
            continue;
        }

        let diagnostic = if let Some(availability) = availability {
            crate::mcp_resources::diagnose_sdk_capability_with_context(name, topology, availability)
        } else {
            crate::mcp_resources::diagnose_sdk_capability_for_current_host(name, topology)
        };
        if let Some(diagnostic) = diagnostic {
            issues.push(capability_validation_issue(
                subject,
                MetadataField::Capability,
                diagnostic.message,
                ScriptValidationKind::CapabilityUnavailable {
                    capability: diagnostic.capability,
                    code: diagnostic.code,
                    alternatives: diagnostic.alternatives,
                },
            ));
        }
    }
    issues
}

fn script_validation_subject(script: &Script) -> CapabilityValidationSubject<'_> {
    use crate::mcp_resources::SdkExecutionTopology;

    CapabilityValidationSubject {
        path: script.path.clone(),
        name: &script.name,
        runtime_topology: match script.extension.as_str() {
            "sh" | "bash" | "zsh" | "fish" => SdkExecutionTopology::ShellScriptlet,
            "py" | "python" | "python3" => SdkExecutionTopology::PythonScriptlet,
            _ => SdkExecutionTopology::TypeScriptScript,
        },
        enforce_scriptlet_topology: false,
    }
}

/// Validate explicit author metadata without heuristic parsing, OS permission
/// probes, provider calls, or assumptions about unknown permission grants.
pub fn validate_declared_sdk_capabilities(script: &Script) -> Vec<ScriptValidationIssue> {
    let Some(metadata) = script.typed_metadata.as_ref() else {
        return Vec::new();
    };
    validate_metadata_capabilities(&script_validation_subject(script), metadata, None)
}

/// Validate using only host facts explicitly supplied by an existing inventory.
/// Unlike the no-probe default, this can distinguish granted from denied access.
pub fn validate_declared_sdk_capabilities_with_host_availability(
    script: &Script,
    availability: &crate::mcp_resources::SdkHostAvailability,
) -> Vec<ScriptValidationIssue> {
    let Some(metadata) = script.typed_metadata.as_ref() else {
        return Vec::new();
    };
    validate_metadata_capabilities(
        &script_validation_subject(script),
        metadata,
        Some(availability),
    )
}

#[derive(Clone)]
struct RegisteredScriptletCapabilities {
    source_path: PathBuf,
    metadata: TypedMetadata,
    issues: Vec<ScriptValidationIssue>,
}

#[derive(Default)]
struct ScriptletCapabilityRegistry {
    generation: u64,
    entries: HashMap<String, RegisteredScriptletCapabilities>,
    loaded_sources: HashMap<String, PathBuf>,
}

static SCRIPTLET_CAPABILITY_REGISTRY: OnceLock<RwLock<ScriptletCapabilityRegistry>> =
    OnceLock::new();

fn scriptlet_capability_registry() -> &'static RwLock<ScriptletCapabilityRegistry> {
    SCRIPTLET_CAPABILITY_REGISTRY
        .get_or_init(|| RwLock::new(ScriptletCapabilityRegistry::default()))
}

fn scriptlet_source_path(scriptlet: &Scriptlet) -> PathBuf {
    scriptlet
        .file_path
        .as_deref()
        .map(|path| path.split_once('#').map_or(path, |(source, _)| source))
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from("plugins")
                .join(&scriptlet.plugin_id)
                .join("scriptlets")
        })
}

fn scriptlet_capability_identity(scriptlet: &Scriptlet) -> String {
    let command = scriptlet
        .command
        .as_deref()
        .filter(|command| !command.trim().is_empty())
        .or_else(|| {
            scriptlet
                .file_path
                .as_deref()
                .and_then(|path| path.split_once('#').map(|(_, command)| command))
        })
        .unwrap_or(&scriptlet.name);
    format!("{}#{command}", scriptlet_source_path(scriptlet).display())
}

fn scriptlet_validation_subject(scriptlet: &Scriptlet) -> CapabilityValidationSubject<'_> {
    use crate::mcp_resources::SdkExecutionTopology;

    let normalized = crate::scriptlets::normalize_scriptlet_tool(&scriptlet.tool);
    let runtime_topology = match normalized.as_str() {
        "kit" | "ts" | "bun" | "deno" | "js" => {
            SdkExecutionTopology::TypeScriptScriptletInteractive
        }
        "python" | "py" | "python3" => SdkExecutionTopology::PythonScriptlet,
        _ => SdkExecutionTopology::ShellScriptlet,
    };
    CapabilityValidationSubject {
        path: scriptlet_source_path(scriptlet),
        name: &scriptlet.name,
        runtime_topology,
        enforce_scriptlet_topology: true,
    }
}

/// Merge supported legacy HTML-comment declarations without treating ordinary
/// source text, custom metadata, or `Array.prototype.find` as SDK usage.
pub(crate) fn merge_scriptlet_capability_metadata(
    typed: Option<&TypedMetadata>,
    legacy: &HashMap<String, String>,
) -> Option<TypedMetadata> {
    let mut metadata = typed.cloned().unwrap_or_default();
    let mut has_metadata = typed.is_some();
    for (key, value) in legacy {
        let canonical = if key.eq_ignore_ascii_case("sdkCapabilities") {
            "sdkCapabilities"
        } else if key.eq_ignore_ascii_case("executionTopology") {
            "executionTopology"
        } else {
            continue;
        };
        metadata
            .extra
            .entry(canonical.to_string())
            .or_insert_with(|| {
                serde_json::from_str(value)
                    .unwrap_or_else(|_| serde_json::Value::String(value.clone()))
            });
        has_metadata = true;
    }
    has_metadata.then_some(metadata)
}

/// Preserve codefence metadata independently of the legacy public Scriptlet
/// shape, which is initialized by many pre-existing scriptlet fixtures.
#[cfg(test)]
pub(crate) fn register_scriptlet_capabilities(
    scriptlet: &Scriptlet,
    metadata: Option<&TypedMetadata>,
) {
    let identity = scriptlet_capability_identity(scriptlet);
    let source_path = scriptlet_source_path(scriptlet);
    let entry = metadata.map(|metadata| RegisteredScriptletCapabilities {
        source_path: source_path.clone(),
        metadata: metadata.clone(),
        issues: validate_metadata_capabilities(
            &scriptlet_validation_subject(scriptlet),
            metadata,
            None,
        ),
    });
    let mut registry = scriptlet_capability_registry()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry
        .loaded_sources
        .insert(identity.clone(), source_path);
    match entry {
        Some(entry) => {
            registry.entries.insert(identity, entry);
        }
        None => {
            registry.entries.remove(&identity);
        }
    }
}

pub(crate) fn publish_scriptlet_capability_snapshot(
    source: Option<&Path>,
    parsed: Vec<(Arc<Scriptlet>, Option<TypedMetadata>)>,
) -> Vec<Arc<Scriptlet>> {
    let mut rows = Vec::with_capacity(parsed.len());
    let mut entries = HashMap::new();
    let mut loaded_sources = HashMap::with_capacity(parsed.len());
    for (scriptlet, metadata) in parsed {
        let identity = scriptlet_capability_identity(&scriptlet);
        let source_path = scriptlet_source_path(&scriptlet);
        loaded_sources.insert(identity.clone(), source_path.clone());
        if let Some(metadata) = metadata {
            let issues = validate_metadata_capabilities(
                &scriptlet_validation_subject(&scriptlet),
                &metadata,
                None,
            );
            entries.insert(
                identity,
                RegisteredScriptletCapabilities {
                    source_path,
                    metadata,
                    issues,
                },
            );
        }
        rows.push(scriptlet);
    }
    let mut registry = scriptlet_capability_registry()
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    registry.generation = registry
        .generation
        .checked_add(1)
        .expect("scriptlet capability generation exhausted");
    if let Some(source) = source {
        registry
            .entries
            .retain(|_, entry| entry.source_path != source);
        registry.loaded_sources.retain(|_, path| path != source);
        registry.entries.extend(entries);
        registry.loaded_sources.extend(loaded_sources);
    } else {
        registry.entries = entries;
        registry.loaded_sources = loaded_sources;
    }
    rows
}

/// Return stable, typed diagnostics without removing the scriptlet from its
/// launcher. Dispatch owners must reject a nonempty result before side effects.
pub fn validate_scriptlet_capabilities(scriptlet: &Scriptlet) -> Vec<ScriptValidationIssue> {
    scriptlet_capability_registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entries
        .get(&scriptlet_capability_identity(scriptlet))
        .map(|entry| entry.issues.clone())
        .unwrap_or_default()
}

/// Return only explicitly declared, syntactically string-valued SDK capability
/// names. Malformed values remain available through the typed issue channel;
/// this never scans source or invents claims from unrelated custom fields.
pub fn scriptlet_declared_sdk_capabilities(scriptlet: &Scriptlet) -> Vec<String> {
    scriptlet_capability_registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entries
        .get(&scriptlet_capability_identity(scriptlet))
        .and_then(|entry| entry.metadata.extra.get("sdkCapabilities"))
        .and_then(serde_json::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Recheck a retained scriptlet against an explicitly known permission snapshot.
/// This can safely resolve a pending grant without probing or opening Settings.
pub fn validate_scriptlet_capabilities_with_host_availability(
    scriptlet: &Scriptlet,
    availability: &crate::mcp_resources::SdkHostAvailability,
) -> Vec<ScriptValidationIssue> {
    let metadata = scriptlet_capability_registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .entries
        .get(&scriptlet_capability_identity(scriptlet))
        .map(|entry| entry.metadata.clone());
    metadata.map_or_else(Vec::new, |metadata| {
        validate_metadata_capabilities(
            &scriptlet_validation_subject(scriptlet),
            &metadata,
            Some(availability),
        )
    })
}

/// Validate the separate synchronous legacy executor directly against its rich
/// parsed scriptlet. Unlike launcher execution, its Bun `.output()` transport
/// has no interactive response pipe, so prompt APIs must fail before spawn.
pub fn validate_legacy_scriptlet_capabilities(
    scriptlet: &crate::scriptlets::Scriptlet,
) -> Vec<ScriptValidationIssue> {
    use crate::mcp_resources::SdkExecutionTopology;

    let Some(metadata) = merge_scriptlet_capability_metadata(
        scriptlet.typed_metadata.as_ref(),
        &scriptlet.metadata.extra,
    ) else {
        return Vec::new();
    };
    let normalized = crate::scriptlets::normalize_scriptlet_tool(&scriptlet.tool);
    let runtime_topology = match normalized.as_str() {
        "kit" | "ts" | "bun" | "deno" => SdkExecutionTopology::TypeScriptScriptlet,
        "python" | "py" | "python3" => SdkExecutionTopology::PythonScriptlet,
        _ => SdkExecutionTopology::ShellScriptlet,
    };
    let subject = CapabilityValidationSubject {
        path: scriptlet
            .source_path
            .as_deref()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(format!("scriptlet/{}", scriptlet.command))),
        name: &scriptlet.name,
        runtime_topology,
        enforce_scriptlet_topology: true,
    };
    validate_metadata_capabilities(&subject, &metadata, None)
}

/// Current complete-load generation for deterministic refresh evidence.
pub fn scriptlet_capability_registry_generation() -> u64 {
    scriptlet_capability_registry()
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .generation
}

fn merge_scriptlet_issue_snapshot(
    report: &ValidationReport,
    candidate_count: usize,
    mut retained_issues: Vec<ScriptValidationIssue>,
) -> ValidationReport {
    retained_issues.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.script_name.cmp(&right.script_name))
            .then_with(|| left.message.cmp(&right.message))
    });
    retained_issues.dedup();

    let fatal_count = retained_issues
        .iter()
        .filter(|issue| issue.severity == ValidationSeverity::Fatal)
        .count();
    let warning_count = retained_issues
        .iter()
        .filter(|issue| issue.severity == ValidationSeverity::Warning)
        .count();
    let blocked_candidates: HashSet<_> = retained_issues
        .iter()
        .filter(|issue| issue.severity == ValidationSeverity::Fatal)
        .map(|issue| (&issue.path, &issue.script_name))
        .collect();

    ValidationReport {
        schema_version: report.schema_version,
        total_candidates: report.total_candidates.saturating_add(candidate_count),
        valid_count: report
            .valid_count
            .saturating_add(candidate_count.saturating_sub(blocked_candidates.len())),
        fatal_count: report.fatal_count.saturating_add(fatal_count),
        warning_count: report.warning_count.saturating_add(warning_count),
        failed_scripts: Arc::clone(&report.failed_scripts),
        warnings: Arc::clone(&report.warnings),
        retained_issues: Arc::from(retained_issues),
    }
}

/// Merge exact currently loaded scriptlets into the author report. Retained
/// fatal rows remain visible/disabled, while warning-only rows stay valid but
/// may still be waiting for an explicitly known permission inventory.
pub fn merge_scriptlet_validation_issues(
    report: &ValidationReport,
    scriptlets: &[Arc<Scriptlet>],
) -> ValidationReport {
    let retained_issues = scriptlets
        .iter()
        .flat_map(|scriptlet| validate_scriptlet_capabilities(scriptlet))
        .collect();
    merge_scriptlet_issue_snapshot(report, scriptlets.len(), retained_issues)
}

/// Build the same mixed-catalog report from the already-loaded sidecar without
/// reading markdown files, rerunning startup, requesting privacy access, or
/// replacing the existing scriptlet generation.
pub fn merge_registered_scriptlet_validation_issues(report: &ValidationReport) -> ValidationReport {
    let (candidate_count, retained_issues) = {
        let registry = scriptlet_capability_registry()
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            registry.loaded_sources.len(),
            registry
                .entries
                .values()
                .flat_map(|entry| entry.issues.iter().cloned())
                .collect(),
        )
    };
    merge_scriptlet_issue_snapshot(report, candidate_count, retained_issues)
}

/// Validate a catalog of already-loaded scripts. This is the entry point
/// the loader wraps via `read_scripts_report()`.
///
/// Fatal issues (currently: duplicate bindings) move a script into
/// `failed_scripts` and exclude it from the returned `scripts` slice so the
/// index never sees ambiguous dispatch. Warning issues stay in the kept set
/// but surface in the report for the MCP resource + menu-bar badge.
pub fn validate_script_catalog(scripts: Vec<Arc<Script>>) -> ScriptCatalogReport {
    let mut by_path: HashMap<PathBuf, Vec<ScriptValidationIssue>> = HashMap::new();
    for issue in detect_binding_collisions(&scripts) {
        by_path.entry(issue.path.clone()).or_default().push(issue);
    }
    for script in &scripts {
        for issue in validate_declared_sdk_capabilities(script) {
            by_path.entry(issue.path.clone()).or_default().push(issue);
        }
    }

    let total_candidates = scripts.len();
    let mut kept: Vec<Arc<Script>> = Vec::with_capacity(scripts.len());
    let mut failed: Vec<FailedScript> = Vec::new();
    let mut warnings: Vec<ScriptValidationIssue> = Vec::new();

    for script in scripts {
        let issues = by_path.remove(&script.path).unwrap_or_default();
        let (fatal_issues, warn_issues): (Vec<_>, Vec<_>) = issues
            .into_iter()
            .partition(|i| i.severity == ValidationSeverity::Fatal);
        warnings.extend(warn_issues);

        if fatal_issues.is_empty() {
            kept.push(script);
        } else {
            failed.push(FailedScript {
                path: script.path.clone(),
                name: script.name.clone(),
                fatal: Arc::from(fatal_issues),
            });
        }
    }

    failed.sort_by(|a, b| a.path.cmp(&b.path));

    let fatal_count: usize = failed.iter().map(|f| f.fatal.len()).sum();
    let warning_count = warnings.len();
    let valid_count = kept.len();

    let validation = Arc::new(ValidationReport {
        schema_version: VALIDATION_SCHEMA_VERSION,
        total_candidates,
        valid_count,
        fatal_count,
        warning_count,
        failed_scripts: Arc::from(failed),
        warnings: Arc::from(warnings),
        retained_issues: Arc::from(Vec::new()),
    });

    ScriptCatalogReport {
        scripts: Arc::from(kept),
        validation,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata_parser::TypedMetadata;

    fn make_script(name: &str, path: &str) -> Script {
        Script {
            name: name.to_string(),
            path: PathBuf::from(path),
            extension: "ts".to_string(),
            ..Script::default()
        }
    }

    fn scriptlet_registry_test_guard() -> std::sync::MutexGuard<'static, ()> {
        crate::test_utils::SK_PATH_TEST_LOCK
            .get_or_init(|| std::sync::Mutex::new(()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn with_shortcut(mut script: Script, shortcut: &str) -> Script {
        script.shortcut = Some(shortcut.to_string());
        script
    }

    fn with_alias(mut script: Script, alias: &str) -> Script {
        script.alias = Some(alias.to_string());
        script
    }

    fn with_keyword(mut script: Script, keyword: &str) -> Script {
        script.typed_metadata = Some(TypedMetadata {
            keyword: Some(keyword.to_string()),
            ..TypedMetadata::default()
        });
        script
    }

    fn arc(script: Script) -> Arc<Script> {
        Arc::new(script)
    }

    fn with_sdk_capabilities(mut script: Script, capabilities: serde_json::Value) -> Script {
        let metadata = script
            .typed_metadata
            .get_or_insert_with(TypedMetadata::default);
        metadata
            .extra
            .insert("sdkCapabilities".to_string(), capabilities);
        script
    }

    fn make_scriptlet(name: &str, source: &str, tool: &str) -> Scriptlet {
        let command = name.to_ascii_lowercase().replace(' ', "-");
        Scriptlet {
            name: name.to_string(),
            description: None,
            code: "items.find(item => item.ready)".to_string(),
            tool: tool.to_string(),
            shortcut: None,
            keyword: None,
            group: None,
            plugin_id: "main".to_string(),
            plugin_title: Some("Main".to_string()),
            file_path: Some(format!("{source}#{command}")),
            command: Some(command),
            alias: None,
            icon: None,
        }
    }

    fn capability_metadata(capabilities: serde_json::Value) -> TypedMetadata {
        TypedMetadata {
            extra: HashMap::from([("sdkCapabilities".to_string(), capabilities)]),
            ..TypedMetadata::default()
        }
    }

    #[test]
    fn empty_catalog_reports_zero_issues() {
        let report = validate_script_catalog(Vec::new());
        assert_eq!(report.validation.total_candidates, 0);
        assert_eq!(report.validation.valid_count, 0);
        assert_eq!(report.validation.fatal_count, 0);
        assert_eq!(report.validation.warning_count, 0);
        assert!(report.validation.failed_scripts.is_empty());
        assert!(report.scripts.is_empty());
    }

    #[test]
    fn single_script_with_bindings_passes() {
        let s = arc(with_shortcut(
            make_script("solo", "/tmp/solo.ts"),
            "cmd shift k",
        ));
        let report = validate_script_catalog(vec![s]);
        assert_eq!(report.validation.valid_count, 1);
        assert_eq!(report.validation.fatal_count, 0);
        assert!(report.validation.failed_scripts.is_empty());
    }

    #[test]
    fn duplicate_shortcut_excludes_both_scripts() {
        let a = arc(with_shortcut(make_script("a", "/tmp/a.ts"), "cmd shift k"));
        let b = arc(with_shortcut(make_script("b", "/tmp/b.ts"), "Cmd Shift K"));
        let report = validate_script_catalog(vec![a, b]);
        assert_eq!(report.validation.total_candidates, 2);
        assert_eq!(report.validation.valid_count, 0);
        assert_eq!(report.validation.fatal_count, 2);
        assert_eq!(report.validation.failed_scripts.len(), 2);

        let first = &report.validation.failed_scripts[0];
        assert_eq!(first.fatal.len(), 1);
        assert_eq!(first.fatal[0].related.len(), 1);
        assert!(matches!(
            first.fatal[0].kind,
            ScriptValidationKind::DuplicateBinding {
                binding: BindingKind::Shortcut,
                ..
            }
        ));
    }

    #[test]
    fn duplicate_alias_normalizes_case() {
        let a = arc(with_alias(make_script("a", "/tmp/a.ts"), "GC"));
        let b = arc(with_alias(make_script("b", "/tmp/b.ts"), "gc"));
        let report = validate_script_catalog(vec![a, b]);
        assert_eq!(report.validation.fatal_count, 2);
        assert!(report
            .validation
            .failed_scripts
            .iter()
            .all(|f| f.fatal.iter().any(|i| matches!(
                i.kind,
                ScriptValidationKind::DuplicateBinding {
                    binding: BindingKind::Alias,
                    ..
                }
            ))));
    }

    #[test]
    fn duplicate_keyword_from_typed_metadata_collides() {
        let a = arc(with_keyword(make_script("a", "/tmp/a.ts"), "!note"));
        let b = arc(with_keyword(make_script("b", "/tmp/b.ts"), "!note"));
        let report = validate_script_catalog(vec![a, b]);
        assert_eq!(report.validation.fatal_count, 2);
    }

    #[test]
    fn unique_bindings_across_kinds_do_not_collide() {
        let a = arc(with_shortcut(make_script("a", "/tmp/a.ts"), "cmd shift k"));
        // Alias "cmd shift k" should NOT collide with shortcut "cmd shift k"
        // because the (kind, value) bucket is kind-scoped.
        let b = arc(with_alias(make_script("b", "/tmp/b.ts"), "cmd shift k"));
        let report = validate_script_catalog(vec![a, b]);
        assert_eq!(report.validation.valid_count, 2);
        assert_eq!(report.validation.fatal_count, 0);
    }

    #[test]
    fn empty_binding_values_are_skipped() {
        let a = arc(with_shortcut(make_script("a", "/tmp/a.ts"), "   "));
        let b = arc(with_shortcut(make_script("b", "/tmp/b.ts"), ""));
        // Both shortcuts normalize to None — no collision, both kept.
        let report = validate_script_catalog(vec![a, b]);
        assert_eq!(report.validation.valid_count, 2);
        assert_eq!(report.validation.fatal_count, 0);
    }

    #[test]
    fn trigger_collision_from_extra_field() {
        let mut a = make_script("a", "/tmp/a.ts");
        let mut extra_a = std::collections::HashMap::new();
        extra_a.insert(
            "trigger".to_string(),
            serde_json::Value::String("open".into()),
        );
        a.typed_metadata = Some(TypedMetadata {
            extra: extra_a,
            ..TypedMetadata::default()
        });

        let mut b = make_script("b", "/tmp/b.ts");
        let mut extra_b = std::collections::HashMap::new();
        extra_b.insert(
            "trigger".to_string(),
            serde_json::Value::String("OPEN".into()),
        );
        b.typed_metadata = Some(TypedMetadata {
            extra: extra_b,
            ..TypedMetadata::default()
        });

        let report = validate_script_catalog(vec![arc(a), arc(b)]);
        assert_eq!(report.validation.fatal_count, 2);
        assert!(report
            .validation
            .failed_scripts
            .iter()
            .all(|f| f.fatal.iter().any(|i| matches!(
                i.kind,
                ScriptValidationKind::DuplicateBinding {
                    binding: BindingKind::Trigger,
                    ..
                }
            ))));
    }

    #[test]
    fn three_way_shortcut_collision_lists_all_peers() {
        let a = arc(with_shortcut(make_script("a", "/tmp/a.ts"), "cmd k"));
        let b = arc(with_shortcut(make_script("b", "/tmp/b.ts"), "cmd k"));
        let c = arc(with_shortcut(make_script("c", "/tmp/c.ts"), "cmd k"));
        let report = validate_script_catalog(vec![a, b, c]);
        assert_eq!(report.validation.fatal_count, 3);
        for failed in report.validation.failed_scripts.iter() {
            assert_eq!(
                failed.fatal[0].related.len(),
                2,
                "each failure should list the 2 peers it collides with"
            );
        }
    }

    #[test]
    fn repair_diagnostics_distinguish_excluded_retained_pending_and_repair_hints() {
        let excluded_issue = ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: PathBuf::from("/tmp/excluded.ts"),
            script_name: "Excluded Script".to_string(),
            field: Some(MetadataField::Shortcut),
            message: "Shortcut collides with another command".to_string(),
            kind: ScriptValidationKind::DuplicateBinding {
                binding: BindingKind::Shortcut,
                value: "cmd k".to_string(),
            },
            related: vec![RelatedScript {
                path: PathBuf::from("/tmp/other.ts"),
                name: "Other Script".to_string(),
            }],
        };
        let retained_issue = ScriptValidationIssue {
            severity: ValidationSeverity::Fatal,
            path: PathBuf::from("/tmp/retained.md"),
            script_name: "Retained Scriptlet".to_string(),
            field: Some(MetadataField::Capability),
            message: "Shell scriptlets do not receive SDK globals".to_string(),
            kind: ScriptValidationKind::CapabilityUnavailable {
                capability: "readFile".to_string(),
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
                alternatives: vec!["Move this command into a TypeScript script".to_string()],
            },
            related: Vec::new(),
        };
        let pending_issue = ScriptValidationIssue {
            severity: ValidationSeverity::Warning,
            path: PathBuf::from("/tmp/pending.ts"),
            script_name: "Pending Permission".to_string(),
            field: Some(MetadataField::Capability),
            message: "Permission inventory has not been supplied".to_string(),
            kind: ScriptValidationKind::CapabilityUnavailable {
                capability: "moveWindow".to_string(),
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable,
                alternatives: vec!["Supply an already-known permission inventory".to_string()],
            },
            related: Vec::new(),
        };
        let report = ValidationReport {
            schema_version: VALIDATION_SCHEMA_VERSION,
            total_candidates: 3,
            valid_count: 1,
            fatal_count: 2,
            warning_count: 1,
            failed_scripts: Arc::from(vec![FailedScript {
                path: excluded_issue.path.clone(),
                name: excluded_issue.script_name.clone(),
                fatal: Arc::from(vec![excluded_issue]),
            }]),
            warnings: Arc::from(vec![pending_issue]),
            retained_issues: Arc::from(vec![retained_issue]),
        };

        let text = format_script_validation_diagnostics(&report);
        assert!(text.contains("1 excluded · 1 retained issue(s) · 2 fatal · 1 warning(s)"));
        assert!(text.contains("## Excluded Script"));
        assert!(text.contains("↔ Other Script — /tmp/other.ts"));
        assert!(text.contains("## Retained Scriptlet"));
        assert!(text.contains("status: blocked, retained in launcher"));
        assert!(text.contains("readFile (MissingSdkTransport) — try Move this command"));
        assert!(text.contains("## Pending Permission"));
        assert!(text.contains("status: warning, retained in launcher"));
        assert!(text.contains("Supply an already-known permission inventory"));
    }

    #[test]
    fn empty_repair_diagnostics_are_explicit() {
        let report = validate_script_catalog(Vec::new());
        let text = format_script_validation_diagnostics(&report.validation);
        assert!(text.contains("0 excluded · 0 retained issue(s)"));
        assert!(text.contains("No failing scripts in this report."));
    }

    #[test]
    fn report_is_serializable() {
        let a = arc(with_shortcut(make_script("a", "/tmp/a.ts"), "cmd k"));
        let b = arc(with_shortcut(make_script("b", "/tmp/b.ts"), "cmd k"));
        let report = validate_script_catalog(vec![a, b]);
        let json = serde_json::to_string(&*report.validation)
            .expect("validation report must serialize cleanly");
        assert!(json.contains("\"schemaVersion\":1"));
        assert!(json.contains("\"duplicateBinding\""));
        assert!(json.contains("\"shortcut\""));
    }

    #[test]
    fn supported_explicit_sdk_capabilities_remain_in_the_catalog() {
        let script = with_sdk_capabilities(
            make_script("supported", "/tmp/supported.ts"),
            serde_json::json!(["arg", "readFile", "writeFile", "exec"]),
        );
        let report = validate_script_catalog(vec![arc(script)]);

        assert_eq!(report.validation.valid_count, 1);
        assert_eq!(report.validation.fatal_count, 0);
    }

    #[test]
    fn unsupported_sdk_capability_fails_before_a_script_can_be_indexed() {
        let script = with_sdk_capabilities(
            make_script("widget-script", "/tmp/widget.ts"),
            serde_json::json!(["widget"]),
        );
        let report = validate_script_catalog(vec![arc(script)]);
        let issue = &report.validation.failed_scripts[0].fatal[0];

        assert!(report.scripts.is_empty());
        assert_eq!(issue.field, Some(MetadataField::Capability));
        assert!(matches!(
            issue.kind,
            ScriptValidationKind::CapabilityUnavailable {
                ref capability,
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::UnsupportedCapability,
                ..
            } if capability == "widget"
        ));
        assert!(!issue.message.is_empty());
    }

    #[test]
    fn unknown_sdk_capability_is_reported_with_a_typed_diagnostic() {
        let script = with_sdk_capabilities(
            make_script("unknown", "/tmp/unknown.ts"),
            serde_json::json!(["imaginaryCapability"]),
        );
        let report = validate_script_catalog(vec![arc(script)]);

        assert!(matches!(
            report.validation.failed_scripts[0].fatal[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::UnknownCapability,
                ..
            }
        ));
    }

    #[test]
    fn typescript_scriptlet_cannot_claim_an_interactive_prompt() {
        let mut script = with_sdk_capabilities(
            make_script("scriptlet", "/tmp/scriptlet.ts"),
            serde_json::json!(["arg"]),
        );
        script
            .typed_metadata
            .as_mut()
            .expect("fixture metadata")
            .extra
            .insert(
                "executionTopology".to_string(),
                serde_json::json!("typescript-scriptlet"),
            );
        let report = validate_script_catalog(vec![arc(script)]);

        assert!(matches!(
            report.validation.failed_scripts[0].fatal[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code:
                    crate::mcp_resources::SdkCapabilityDiagnosticCode::InteractivePromptUnavailable,
                ..
            }
        ));
    }

    #[test]
    fn shell_scripts_cannot_claim_a_typescript_sdk_transport() {
        let mut script = make_script("shell", "/tmp/scriptlet.sh");
        script.extension = "sh".to_string();
        let script = with_sdk_capabilities(script, serde_json::json!(["readFile"]));
        let report = validate_script_catalog(vec![arc(script)]);

        assert!(matches!(
            report.validation.failed_scripts[0].fatal[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
                ..
            }
        ));
    }

    #[test]
    fn malformed_duplicate_and_unknown_topology_declarations_fail_closed() {
        let malformed = with_sdk_capabilities(
            make_script("malformed", "/tmp/malformed.ts"),
            serde_json::json!("arg"),
        );
        let duplicate = with_sdk_capabilities(
            make_script("duplicate", "/tmp/duplicate.ts"),
            serde_json::json!(["arg", "arg"]),
        );
        let mut unknown_topology = with_sdk_capabilities(
            make_script("topology", "/tmp/topology.ts"),
            serde_json::json!(["arg"]),
        );
        unknown_topology
            .typed_metadata
            .as_mut()
            .expect("fixture metadata")
            .extra
            .insert("executionTopology".to_string(), serde_json::json!("magic"));
        let report =
            validate_script_catalog(vec![arc(malformed), arc(duplicate), arc(unknown_topology)]);

        assert_eq!(report.validation.valid_count, 0);
        assert_eq!(report.validation.fatal_count, 3);
        assert!(report
            .validation
            .failed_scripts
            .iter()
            .any(|failed| { failed.fatal[0].field == Some(MetadataField::ExecutionTopology) }));
    }

    #[test]
    fn execution_topology_is_validated_without_a_capability_declaration() {
        let mut script = make_script("standalone-topology", "/tmp/standalone-topology.ts");
        script.typed_metadata = Some(TypedMetadata {
            extra: HashMap::from([(
                "executionTopology".to_string(),
                serde_json::json!("ruby-scriptlet"),
            )]),
            ..TypedMetadata::default()
        });

        let report = validate_script_catalog(vec![arc(script)]);
        assert_eq!(report.validation.fatal_count, 1);
        let issue = &report.validation.failed_scripts[0].fatal[0];
        assert_eq!(issue.field, Some(MetadataField::ExecutionTopology));
        assert!(matches!(
            &issue.kind,
            ScriptValidationKind::InvalidValue { reason, .. }
                if reason == "unknown_sdk_execution_topology"
        ));
    }

    #[test]
    fn explicit_known_host_facts_enforce_version_platform_and_permission() {
        let script = with_sdk_capabilities(
            make_script("native", "/tmp/native.ts"),
            serde_json::json!(["moveWindow"]),
        );
        let mut host = crate::mcp_resources::SdkHostAvailability {
            host_version: "not-semver".to_string(),
            platform: "macos".to_string(),
            granted_permissions: vec!["accessibility".to_string()],
        };

        let malformed = validate_declared_sdk_capabilities_with_host_availability(&script, &host);
        assert!(matches!(
            malformed[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::InvalidHostVersion,
                ..
            }
        ));

        host.host_version = "0.0.0".to_string();
        let outdated = validate_declared_sdk_capabilities_with_host_availability(&script, &host);
        assert!(matches!(
            outdated[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::HostVersionTooOld,
                ..
            }
        ));

        host.host_version = env!("CARGO_PKG_VERSION").to_string();
        host.platform = "linux".to_string();
        let wrong_platform =
            validate_declared_sdk_capabilities_with_host_availability(&script, &host);
        assert!(matches!(
            wrong_platform[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::UnsupportedPlatform,
                ..
            }
        ));

        host.platform = "macos".to_string();
        host.granted_permissions.clear();
        let denied = validate_declared_sdk_capabilities_with_host_availability(&script, &host);
        assert!(matches!(
            denied[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingPermission,
                ..
            }
        ));

        host.granted_permissions.push("accessibility".to_string());
        assert!(
            validate_declared_sdk_capabilities_with_host_availability(&script, &host).is_empty()
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn unknown_permission_inventory_keeps_script_visible_with_pending_warning() {
        let script = with_sdk_capabilities(
            make_script("pending-native", "/tmp/pending-native.ts"),
            serde_json::json!(["moveWindow"]),
        );
        let report = validate_script_catalog(vec![arc(script)]);

        assert_eq!(report.validation.valid_count, 1);
        assert_eq!(report.validation.fatal_count, 0);
        assert_eq!(report.validation.warning_count, 1);
        assert!(matches!(
            report.validation.warnings[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code:
                    crate::mcp_resources::SdkCapabilityDiagnosticCode::PermissionInventoryUnavailable,
                ..
            }
        ));
    }

    #[test]
    fn interactive_launcher_scriptlet_prompts_stay_supported() {
        let _registry = scriptlet_registry_test_guard();
        let scriptlet =
            make_scriptlet("Interactive Prompt", "/tmp/sdk-interactive-prompt.md", "ts");
        register_scriptlet_capabilities(
            &scriptlet,
            Some(&capability_metadata(serde_json::json!(["arg", "fields"]))),
        );

        assert!(validate_scriptlet_capabilities(&scriptlet).is_empty());
    }

    #[test]
    fn explicitly_noninteractive_scriptlet_rejects_prompt_without_hiding_row() {
        let _registry = scriptlet_registry_test_guard();
        let scriptlet = make_scriptlet("Legacy Prompt", "/tmp/sdk-noninteractive-prompt.md", "ts");
        let mut metadata = capability_metadata(serde_json::json!(["arg"]));
        metadata.extra.insert(
            "executionTopology".to_string(),
            serde_json::json!("typescript-scriptlet"),
        );
        register_scriptlet_capabilities(&scriptlet, Some(&metadata));

        let issues = validate_scriptlet_capabilities(&scriptlet);
        assert_eq!(issues.len(), 1);
        assert_eq!(
            issues[0].path,
            PathBuf::from("/tmp/sdk-noninteractive-prompt.md")
        );
        assert!(matches!(
            issues[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code:
                    crate::mcp_resources::SdkCapabilityDiagnosticCode::InteractivePromptUnavailable,
                ..
            }
        ));
    }

    #[test]
    fn shell_and_python_scriptlets_cannot_claim_or_spoof_sdk_transport() {
        let _registry = scriptlet_registry_test_guard();
        for (tool, path) in [
            ("bash", "/tmp/sdk-shell-topology.md"),
            ("python", "/tmp/sdk-python-topology.md"),
        ] {
            let scriptlet = make_scriptlet("Non SDK", path, tool);
            let metadata = capability_metadata(serde_json::json!(["readFile"]));
            register_scriptlet_capabilities(&scriptlet, Some(&metadata));
            assert!(matches!(
                validate_scriptlet_capabilities(&scriptlet)[0].kind,
                ScriptValidationKind::CapabilityUnavailable {
                    code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
                    ..
                }
            ));

            let mut spoofed = metadata.clone();
            spoofed.extra.insert(
                "executionTopology".to_string(),
                serde_json::json!("typescript-scriptlet-interactive"),
            );
            register_scriptlet_capabilities(&scriptlet, Some(&spoofed));
            assert!(matches!(
                &validate_scriptlet_capabilities(&scriptlet)[0].kind,
                ScriptValidationKind::InvalidValue { reason, .. }
                    if reason == "sdk_execution_topology_mismatch"
            ));
        }
    }

    #[test]
    fn scriptlet_refresh_removes_only_stale_source_diagnostics() {
        let _registry = scriptlet_registry_test_guard();
        let stale = make_scriptlet("Stale", "/tmp/sdk-stale-source.md", "bash");
        let retained = make_scriptlet("Retained", "/tmp/sdk-retained-source.md", "bash");
        let metadata = capability_metadata(serde_json::json!(["readFile"]));
        register_scriptlet_capabilities(&stale, Some(&metadata));
        register_scriptlet_capabilities(&retained, Some(&metadata));

        publish_scriptlet_capability_snapshot(
            Some(Path::new("/tmp/sdk-stale-source.md")),
            Vec::new(),
        );
        assert!(validate_scriptlet_capabilities(&stale).is_empty());
        assert_eq!(validate_scriptlet_capabilities(&retained).len(), 1);
    }

    #[test]
    fn blocked_scriptlet_stays_blocked_until_complete_snapshot_is_published() {
        let _registry = scriptlet_registry_test_guard();
        let blocked = Arc::new(make_scriptlet(
            "Blocked During Refresh",
            "/tmp/sdk-refresh-atomic.md",
            "bash",
        ));
        let metadata = capability_metadata(serde_json::json!(["readFile"]));
        register_scriptlet_capabilities(&blocked, Some(&metadata));
        let replacement: Vec<(Arc<Scriptlet>, Option<TypedMetadata>)> =
            vec![(Arc::clone(&blocked), None)];
        assert_eq!(validate_scriptlet_capabilities(&blocked).len(), 1);
        drop(replacement);
        assert_eq!(
            validate_scriptlet_capabilities(&blocked).len(),
            1,
            "discarded local reads retain active diagnostics"
        );
        publish_scriptlet_capability_snapshot(None, vec![(Arc::clone(&blocked), None)]);
        assert!(validate_scriptlet_capabilities(&blocked).is_empty());
    }

    #[test]
    fn mixed_validation_report_retains_blocked_rows_without_fabricating_exclusion() {
        let _registry = scriptlet_registry_test_guard();
        let report = validate_script_catalog(vec![arc(make_script("Safe", "/tmp/safe-report.ts"))]);
        let allowed = Arc::new(make_scriptlet(
            "Allowed Interactive",
            "/tmp/sdk-report-allowed.md",
            "ts",
        ));
        register_scriptlet_capabilities(
            &allowed,
            Some(&capability_metadata(serde_json::json!(["arg"]))),
        );
        let blocked = Arc::new(make_scriptlet(
            "Blocked Shell",
            "/tmp/sdk-report-blocked.md",
            "bash",
        ));
        register_scriptlet_capabilities(
            &blocked,
            Some(&capability_metadata(serde_json::json!(["readFile"]))),
        );

        let mixed = merge_scriptlet_validation_issues(&report.validation, &[allowed, blocked]);
        assert_eq!(mixed.total_candidates, 3);
        assert_eq!(mixed.valid_count, 2);
        assert_eq!(mixed.fatal_count, 1);
        assert_eq!(mixed.warning_count, 0);
        assert!(mixed.failed_scripts.is_empty());
        assert!(mixed.warnings.is_empty());
        assert_eq!(mixed.retained_issues.len(), 1);
        assert_eq!(mixed.retained_issues[0].severity, ValidationSeverity::Fatal);

        let json = serde_json::to_value(&mixed).expect("serialize mixed report");
        assert_eq!(json["retainedIssues"][0]["severity"], "fatal");
        assert_eq!(json["failedScripts"], serde_json::json!([]));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn pending_permission_scriptlet_remains_valid_but_visible_in_retained_issues() {
        let _registry = scriptlet_registry_test_guard();
        let report = validate_script_catalog(Vec::new());
        let pending = Arc::new(make_scriptlet(
            "Pending Permission",
            "/tmp/sdk-report-pending.md",
            "ts",
        ));
        register_scriptlet_capabilities(
            &pending,
            Some(&capability_metadata(serde_json::json!(["moveWindow"]))),
        );

        let mixed = merge_scriptlet_validation_issues(&report.validation, &[pending]);
        assert_eq!(mixed.total_candidates, 1);
        assert_eq!(mixed.valid_count, 1);
        assert_eq!(mixed.fatal_count, 0);
        assert_eq!(mixed.warning_count, 1);
        assert!(mixed.failed_scripts.is_empty());
        assert!(mixed.warnings.is_empty());
        assert_eq!(
            mixed.retained_issues[0].severity,
            ValidationSeverity::Warning
        );
    }

    #[test]
    fn legacy_validation_reports_deserialize_without_retained_issue_field() {
        let current = validate_script_catalog(Vec::new());
        let mut legacy = serde_json::to_value(&*current.validation).expect("serialize report");
        legacy
            .as_object_mut()
            .expect("report object")
            .remove("retainedIssues");

        let restored: ValidationReport =
            serde_json::from_value(legacy).expect("older validation reports must remain readable");
        assert!(restored.retained_issues.is_empty());
    }

    #[test]
    fn scriptlet_known_host_inventory_can_resolve_a_pending_permission() {
        let _registry = scriptlet_registry_test_guard();
        let scriptlet = make_scriptlet("Native", "/tmp/sdk-known-host.md", "ts");
        register_scriptlet_capabilities(
            &scriptlet,
            Some(&capability_metadata(serde_json::json!(["moveWindow"]))),
        );
        let host = crate::mcp_resources::SdkHostAvailability {
            host_version: env!("CARGO_PKG_VERSION").to_string(),
            platform: "macos".to_string(),
            granted_permissions: vec!["accessibility".to_string()],
        };

        assert!(
            validate_scriptlet_capabilities_with_host_availability(&scriptlet, &host).is_empty()
        );
    }

    #[test]
    fn legacy_executor_scriptlet_rejects_prompt_but_safe_noninteractive_api_works() {
        let mut legacy = crate::scriptlets::Scriptlet::new(
            "Legacy Interactive".to_string(),
            "ts".to_string(),
            "await arg('Prompt')".to_string(),
        );
        legacy.source_path = Some("/tmp/legacy-executor.md".to_string());
        legacy.typed_metadata = Some(capability_metadata(serde_json::json!(["arg"])));

        let issues = validate_legacy_scriptlet_capabilities(&legacy);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].path, PathBuf::from("/tmp/legacy-executor.md"));
        assert!(matches!(
            issues[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code:
                    crate::mcp_resources::SdkCapabilityDiagnosticCode::InteractivePromptUnavailable,
                ..
            }
        ));

        legacy.typed_metadata = Some(capability_metadata(serde_json::json!(["home"])));
        assert!(validate_legacy_scriptlet_capabilities(&legacy).is_empty());
    }

    #[test]
    fn legacy_executor_rejects_claimed_interactive_transport_and_keeps_old_scripts() {
        let mut legacy = crate::scriptlets::Scriptlet::new(
            "Existing".to_string(),
            "ts".to_string(),
            "items.find(item => item.ready)".to_string(),
        );
        assert!(validate_legacy_scriptlet_capabilities(&legacy).is_empty());

        legacy.typed_metadata = Some(TypedMetadata {
            extra: HashMap::from([(
                "executionTopology".to_string(),
                serde_json::json!("typescript-scriptlet-interactive"),
            )]),
            ..TypedMetadata::default()
        });
        let issues = validate_legacy_scriptlet_capabilities(&legacy);
        assert!(matches!(
            &issues[0].kind,
            ScriptValidationKind::InvalidValue { reason, .. }
                if reason == "sdk_execution_topology_mismatch"
        ));
    }

    #[test]
    fn legacy_html_capability_metadata_is_recognized_without_scanning_code() {
        let mut legacy = crate::scriptlets::Scriptlet::new(
            "Legacy Metadata".to_string(),
            "bash".to_string(),
            "items.find(item => item.ready)".to_string(),
        );
        legacy
            .metadata
            .extra
            .insert("sdkcapabilities".to_string(), "[\"readFile\"]".to_string());

        assert!(matches!(
            validate_legacy_scriptlet_capabilities(&legacy)[0].kind,
            ScriptValidationKind::CapabilityUnavailable {
                code: crate::mcp_resources::SdkCapabilityDiagnosticCode::MissingSdkTransport,
                ..
            }
        ));
    }

    #[test]
    fn ordinary_array_find_calls_and_undeclared_custom_metadata_are_not_sdk_claims() {
        let mut script = make_script("ordinary", "/tmp/ordinary.ts");
        script.body = Some("items.find(item => item.name === 'find')".to_string());
        script.typed_metadata = Some(TypedMetadata {
            extra: HashMap::from([(
                "capabilities".to_string(),
                serde_json::json!({ "unrelated": true }),
            )]),
            ..TypedMetadata::default()
        });

        let report = validate_script_catalog(vec![arc(script)]);
        assert_eq!(report.validation.valid_count, 1);
        assert_eq!(report.validation.fatal_count, 0);
    }

    #[test]
    fn capability_failure_serializes_code_and_migration_alternatives() {
        let script = with_sdk_capabilities(
            make_script("widget", "/tmp/widget.ts"),
            serde_json::json!(["widget"]),
        );
        let report = validate_script_catalog(vec![arc(script)]);
        let json = serde_json::to_value(&*report.validation).expect("serialize capability report");
        let issue = &json["failedScripts"][0]["fatal"][0];

        assert_eq!(issue["field"], "capability");
        assert_eq!(issue["kind"]["kind"], "capabilityUnavailable");
        assert_eq!(issue["kind"]["code"], "unsupported_capability");
        assert_eq!(issue["kind"]["capability"], "widget");
        assert!(issue["kind"]["alternatives"].is_array());
    }
}
