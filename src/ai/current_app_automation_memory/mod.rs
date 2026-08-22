//! Persistent memory of frontmost-app automation capabilities for AI agents.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ai::GeneratedScriptReceipt;
use crate::builtins::BuiltInEntry;
use crate::menu_bar::current_app_commands::{
    build_replay_current_app_recipe_receipt, CurrentAppCommandRecipe, FrontmostMenuSnapshot,
    ReplayCurrentAppRecipeReceipt,
};

pub const CURRENT_APP_AUTOMATION_MEMORY_INDEX_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppAutomationMemoryIndexEntry {
    pub schema_version: u32,
    pub slug: String,
    pub script_path: String,
    pub receipt_path: String,
    pub bundle_id: String,
    pub app_name: String,
    pub effective_query: String,
    pub raw_query: String,
    pub prompt: String,
    pub provider_id: String,
    pub model_id: String,
    pub lookup_key: String,
    pub auto_replay_eligible: bool,
    pub written_at_unix_ms: u128,
    pub recipe: CurrentAppCommandRecipe,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CurrentAppAutomationMemoryDecision {
    pub schema_version: u32,
    pub action: String,
    pub query: String,
    pub bundle_id: String,
    pub considered: usize,
    pub best_score: f32,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matched: Option<CurrentAppAutomationMemoryIndexEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replay: Option<ReplayCurrentAppRecipeReceipt>,
}

pub fn current_app_automation_memory_index_path() -> Result<PathBuf> {
    let home = env::var("HOME").context("HOME is not set")?;
    Ok(Path::new(&home)
        .join(".scriptkit")
        .join("scripts")
        .join(".current-app-automation-memory.json"))
}

pub fn read_current_app_automation_memory_index(
) -> Result<Vec<CurrentAppAutomationMemoryIndexEntry>> {
    read_current_app_automation_memory_index_at(&current_app_automation_memory_index_path()?)
}

fn read_current_app_automation_memory_index_at(
    path: &Path,
) -> Result<Vec<CurrentAppAutomationMemoryIndexEntry>> {
    if !crate::atomic_file::inspect_private_file(path)
        .context("Failed inspecting current app automation memory index")?
    {
        return Ok(Vec::new());
    }

    let json = crate::atomic_file::read_private_file(path)
        .context("Failed reading current app automation memory index")?;

    serde_json::from_str(&json).context("Failed parsing current app automation memory index")
}

pub fn normalize_automation_memory_text(input: &str) -> String {
    let mut normalized = String::with_capacity(input.len());
    let mut last_was_space = false;

    for ch in input.chars() {
        let ch = if ch == '\u{2192}' { ' ' } else { ch };

        if ch.is_ascii_alphanumeric() {
            normalized.push(ch.to_ascii_lowercase());
            last_was_space = false;
        } else if !last_was_space {
            normalized.push(' ');
            last_was_space = true;
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn current_app_recipe_lookup_key(recipe: &CurrentAppCommandRecipe) -> String {
    format!(
        "{}::{}",
        normalize_automation_memory_text(&recipe.prompt_receipt.bundle_id),
        normalize_automation_memory_text(&recipe.effective_query)
    )
}

fn token_set(input: &str) -> BTreeSet<String> {
    normalize_automation_memory_text(input)
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn jaccard_similarity(left: &str, right: &str) -> f32 {
    let left_set = token_set(left);
    let right_set = token_set(right);

    if left_set.is_empty() || right_set.is_empty() {
        return 0.0;
    }

    let intersection = left_set.intersection(&right_set).count() as f32;
    let union = left_set.union(&right_set).count() as f32;

    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

fn score_candidate(query: &str, entry: &CurrentAppAutomationMemoryIndexEntry) -> f32 {
    let query_norm = normalize_automation_memory_text(query);
    let effective_norm = normalize_automation_memory_text(&entry.effective_query);
    let raw_norm = normalize_automation_memory_text(&entry.raw_query);

    let exact = if query_norm == effective_norm || query_norm == raw_norm {
        1.0
    } else {
        0.0
    };

    let effective_overlap = jaccard_similarity(&query_norm, &effective_norm);
    let raw_overlap = jaccard_similarity(&query_norm, &raw_norm);

    (exact * 0.70) + (effective_overlap * 0.20) + (raw_overlap * 0.10)
}

pub fn upsert_current_app_automation_memory_from_receipt(
    receipt: &GeneratedScriptReceipt,
) -> Result<()> {
    if receipt.current_app_recipe.is_none() {
        return Ok(());
    }
    upsert_current_app_automation_memory_from_receipt_at(
        &current_app_automation_memory_index_path()?,
        receipt,
    )
}

fn upsert_current_app_automation_memory_from_receipt_at(
    path: &Path,
    receipt: &GeneratedScriptReceipt,
) -> Result<()> {
    let Some(recipe) = receipt.current_app_recipe.clone() else {
        return Ok(());
    };

    let mut entries = read_current_app_automation_memory_index_at(path)?;
    let entry = CurrentAppAutomationMemoryIndexEntry {
        schema_version: CURRENT_APP_AUTOMATION_MEMORY_INDEX_SCHEMA_VERSION,
        slug: receipt.slug.clone(),
        script_path: receipt.script_path.clone(),
        receipt_path: receipt.receipt_path.clone(),
        bundle_id: recipe.prompt_receipt.bundle_id.clone(),
        app_name: recipe.prompt_receipt.app_name.clone(),
        effective_query: recipe.effective_query.clone(),
        raw_query: recipe.raw_query.clone(),
        prompt: receipt.prompt.clone(),
        provider_id: receipt.provider_id.clone(),
        model_id: receipt.model_id.clone(),
        lookup_key: current_app_recipe_lookup_key(&recipe),
        auto_replay_eligible: !receipt.shell_execution_warning,
        written_at_unix_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        recipe,
    };

    entries.retain(|existing| existing.receipt_path != entry.receipt_path);
    entries.push(entry.clone());

    entries.sort_by(|left, right| {
        left.lookup_key
            .cmp(&right.lookup_key)
            .then_with(|| right.written_at_unix_ms.cmp(&left.written_at_unix_ms))
    });

    let json = serde_json::to_string_pretty(&entries)
        .context("Failed to serialize current app automation memory index")?;
    crate::atomic_file::write_private_atomic(path, json.as_bytes())
        .context("Failed writing current app automation memory index")?;

    let safe_lookup_key = crate::logging::log_private_user_value(&entry.lookup_key);
    let safe_slug = crate::logging::log_private_user_value(&entry.slug);
    tracing::info!(
        category = "CURRENT_APP_AUTOMATION_MEMORY",
        action = "index_upsert",
        lookup_key_bytes = safe_lookup_key.raw_bytes,
        lookup_key_sha256 = %safe_lookup_key.sha256,
        slug_bytes = safe_slug.raw_bytes,
        slug_sha256 = %safe_slug.sha256,
        auto_replay_eligible = entry.auto_replay_eligible,
        "current_app_automation_memory.index_upserted"
    );

    Ok(())
}

fn build_generate_new_decision(
    query: &str,
    bundle_id: &str,
    considered: usize,
    reason: &str,
) -> CurrentAppAutomationMemoryDecision {
    CurrentAppAutomationMemoryDecision {
        schema_version: CURRENT_APP_AUTOMATION_MEMORY_INDEX_SCHEMA_VERSION,
        action: "generate_new".to_string(),
        query: query.to_string(),
        bundle_id: bundle_id.to_string(),
        considered,
        best_score: 0.0,
        reason: reason.to_string(),
        matched: None,
        replay: None,
    }
}

pub fn resolve_current_app_automation_from_memory(
    raw_query: &str,
    snapshot: &FrontmostMenuSnapshot,
    entries: &[BuiltInEntry],
    selected_text: Option<&str>,
    browser_url: Option<&str>,
) -> Result<CurrentAppAutomationMemoryDecision> {
    let query = raw_query.trim();
    if query.is_empty() {
        return Ok(build_generate_new_decision(
            query,
            &snapshot.bundle_id,
            0,
            "empty_query",
        ));
    }

    let bundle_id_norm = normalize_automation_memory_text(&snapshot.bundle_id);
    let all_entries = read_current_app_automation_memory_index()?;

    let mut candidates: Vec<(CurrentAppAutomationMemoryIndexEntry, f32)> = all_entries
        .into_iter()
        .filter(|entry| {
            entry.auto_replay_eligible
                && normalize_automation_memory_text(&entry.bundle_id) == bundle_id_norm
        })
        .map(|entry| {
            let score = score_candidate(query, &entry);
            (entry, score)
        })
        .collect();

    let considered = candidates.len();
    if candidates.is_empty() {
        return Ok(build_generate_new_decision(
            query,
            &snapshot.bundle_id,
            considered,
            "no_prior_automation_for_bundle_id",
        ));
    }

    candidates.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.lookup_key.cmp(&right.0.lookup_key))
    });

    let Some((best_entry, best_score)) = candidates.into_iter().next() else {
        return Ok(build_generate_new_decision(
            query,
            &snapshot.bundle_id,
            considered,
            "no_prior_automation_for_bundle_id",
        ));
    };

    if best_score < 0.55 {
        return Ok(build_generate_new_decision(
            query,
            &snapshot.bundle_id,
            considered,
            "no_sufficiently_similar_prior_automation",
        ));
    }

    let replay = build_replay_current_app_recipe_receipt(
        &best_entry.recipe,
        entries,
        snapshot.clone(),
        selected_text,
        browser_url,
    );

    let (action, reason) = if replay.verification.warning_count == 0 && best_score >= 0.90 {
        ("replay_recipe", "matched_verified_current_app_automation")
    } else if replay.verification.warning_count == 0 {
        (
            "repair_recipe",
            "matched_similar_current_app_automation_needs_prompt_refresh",
        )
    } else {
        (
            "repair_recipe",
            "matched_prior_automation_but_live_context_drifted",
        )
    };

    let safe_query = crate::logging::log_private_user_value(query);
    let safe_slug = crate::logging::log_private_user_value(&best_entry.slug);
    tracing::info!(
        category = "CURRENT_APP_AUTOMATION_MEMORY",
        action,
        best_score,
        considered,
        bundle_id = %snapshot.bundle_id,
        query_bytes = safe_query.raw_bytes,
        query_sha256 = %safe_query.sha256,
        slug_bytes = safe_slug.raw_bytes,
        slug_sha256 = %safe_slug.sha256,
        "current_app_automation_memory.resolved"
    );

    Ok(CurrentAppAutomationMemoryDecision {
        schema_version: CURRENT_APP_AUTOMATION_MEMORY_INDEX_SCHEMA_VERSION,
        action: action.to_string(),
        query: query.to_string(),
        bundle_id: snapshot.bundle_id.clone(),
        considered,
        best_score,
        reason: reason.to_string(),
        matched: Some(best_entry),
        replay: Some(replay),
    })
}

#[cfg(test)]
mod tests;
