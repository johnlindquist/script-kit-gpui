use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

const KIT_STORE_GITHUB_API_BASE: &str = "https://api.github.com";
const KIT_STORE_GITHUB_ACCEPT: &str = "application/vnd.github+json";
const KIT_STORE_GITHUB_VERSION: &str = "2022-11-28";
const KIT_STORE_GITHUB_USER_AGENT: &str = "script-kit-gpui-kit-store-view";
const KIT_STORE_GITHUB_TOPICS: [&str; 2] = ["scriptkit-kit", "script-kit"];
static KIT_STORE_BROWSE_QUERY_GENERATION: AtomicU64 = AtomicU64::new(0);

fn next_kit_store_browse_query_generation() -> u64 {
    KIT_STORE_BROWSE_QUERY_GENERATION
        .fetch_add(1, AtomicOrdering::AcqRel)
        .wrapping_add(1)
}

fn kit_store_browse_query_generation_is_current(generation: u64) -> bool {
    KIT_STORE_BROWSE_QUERY_GENERATION.load(AtomicOrdering::Acquire) == generation
}

/// A kit repository discovered from GitHub search results.
#[derive(Debug, Clone, Default)]
struct KitStoreSearchResult {
    name: String,
    full_name: String,
    description: String,
    stars: u64,
    #[allow(dead_code)] // Reserved for "Open in Browser" action
    html_url: String,
    clone_url: String,
}

#[derive(Debug, Default, Deserialize)]
struct KitStoreGithubSearchResponse {
    #[serde(default)]
    items: Vec<KitStoreGithubRepo>,
}

#[derive(Debug, Default, Deserialize)]
struct KitStoreGithubRepo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    full_name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    stargazers_count: u64,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    clone_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KitStorePluginMutation {
    Install,
    Update,
    Remove,
}

impl KitStorePluginMutation {
    fn action(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Update => "update",
            Self::Remove => "remove",
        }
    }

    fn progress_message(self, plugin_name: &str) -> String {
        match self {
            Self::Install => format!("Installing '{}'...", plugin_name),
            Self::Update => format!("Updating '{}'...", plugin_name),
            Self::Remove => format!("Removing '{}'...", plugin_name),
        }
    }

    fn success_message(self, plugin_name: &str) -> String {
        match self {
            Self::Install => format!("Installed plugin '{}' — commands are live now", plugin_name),
            Self::Update => format!("Updated plugin '{}' — launcher refreshed", plugin_name),
            Self::Remove => format!("Removed plugin '{}' from the launcher", plugin_name),
        }
    }

    fn failure_message(self, plugin_name: Option<&str>, error: &str) -> String {
        match (self, plugin_name) {
            (Self::Install, _) => format!("Failed to install plugin: {}", error),
            (Self::Update, Some(plugin_name)) => {
                format!("Failed to update '{}': {}", plugin_name, error)
            }
            (Self::Update, None) => format!("Failed to update plugin: {}", error),
            (Self::Remove, Some(plugin_name)) => {
                format!("Failed to remove '{}': {}", plugin_name, error)
            }
            (Self::Remove, None) => format!("Failed to remove plugin: {}", error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KitStoreOperationStep {
    ReadGitHead,
    SaveInstalledRegistry,
    PullRepository,
    SaveUpdatedRegistry,
    RemoveDirectory,
    RemoveRegistry,
}

impl KitStoreOperationStep {
    fn git_command(self) -> Option<&'static str> {
        match self {
            Self::ReadGitHead => Some("git rev-parse"),
            Self::PullRepository => Some("git pull"),
            Self::SaveInstalledRegistry
            | Self::SaveUpdatedRegistry
            | Self::RemoveDirectory
            | Self::RemoveRegistry => None,
        }
    }

    fn git_spawn_failure(self, error: impl std::fmt::Display) -> String {
        let command = self.git_command().unwrap_or("git");
        format!("Failed to run {}: {}", command, error)
    }

    fn git_status_failure(self, status: std::process::ExitStatus, stderr: &[u8]) -> String {
        let operation = self.git_command().unwrap_or("git");
        let stderr = String::from_utf8_lossy(stderr).trim().to_string();
        format!(
            "{} failed with status {}{}",
            operation,
            status,
            if stderr.is_empty() {
                String::new()
            } else {
                format!(": {}", stderr)
            }
        )
    }

    fn storage_failure(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::SaveInstalledRegistry => format!("Failed to update plugin registry: {}", error),
            Self::SaveUpdatedRegistry => format!("Failed to save updated kit registry: {}", error),
            Self::RemoveRegistry => format!("Failed to update kit registry: {}", error),
            Self::ReadGitHead | Self::PullRepository | Self::RemoveDirectory => {
                format!("Kit Store storage step failed: {}", error)
            }
        }
    }

    fn remove_directory_failure(self, error: impl std::fmt::Display) -> String {
        match self {
            Self::RemoveDirectory => format!("Failed to remove kit directory: {}", error),
            Self::ReadGitHead
            | Self::SaveInstalledRegistry
            | Self::PullRepository
            | Self::SaveUpdatedRegistry
            | Self::RemoveRegistry => format!("Kit Store remove step failed: {}", error),
        }
    }

    fn empty_git_hash_message(self) -> String {
        let command = self.git_command().unwrap_or("git");
        format!("{} returned empty hash", command)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KitStoreBrowseEmptyState {
    NoFeaturedKits,
    NoSearchResults,
}

impl KitStoreBrowseEmptyState {
    fn from_query(query: &str) -> Self {
        if query.trim().is_empty() {
            Self::NoFeaturedKits
        } else {
            Self::NoSearchResults
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::NoFeaturedKits => "No kits available",
            Self::NoSearchResults => "No kits found",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::NoFeaturedKits => "Check your network connection or try again",
            Self::NoSearchResults => "Try a different search query",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KitStoreInstalledEmptyState {
    Empty,
    NoSearchResults,
}

impl KitStoreInstalledEmptyState {
    fn title(self) -> &'static str {
        match self {
            Self::Empty => "No installed kits",
            Self::NoSearchResults => "No installed kits match your search",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Empty => "Use \"Browse Kit Store\" to install one",
            Self::NoSearchResults => "Try a different search query",
        }
    }
}

impl ScriptListApp {
    fn kit_store_search_results(query: &str) -> Vec<KitStoreSearchResult> {
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .https_only(true)
            .build()
            .new_agent();

        let normalized_query = query.split_whitespace().join("+");
        let mut results = Vec::new();
        let mut seen = HashSet::new();

        for topic in KIT_STORE_GITHUB_TOPICS {
            let url = if normalized_query.is_empty() {
                format!(
                    "{KIT_STORE_GITHUB_API_BASE}/search/repositories?q=topic:{topic}&sort=stars&order=desc"
                )
            } else {
                format!(
                    "{KIT_STORE_GITHUB_API_BASE}/search/repositories?q=topic:{topic}+{normalized_query}&sort=stars&order=desc"
                )
            };

            let response = match agent
                .get(&url)
                .header("Accept", KIT_STORE_GITHUB_ACCEPT)
                .header("X-GitHub-Api-Version", KIT_STORE_GITHUB_VERSION)
                .header("User-Agent", KIT_STORE_GITHUB_USER_AGENT)
                .call()
            {
                Ok(response) => response,
                Err(error) => {
                    tracing::warn!("Kit Store GitHub search request failed: {}", error);
                    continue;
                }
            };

            let status = response.status().as_u16();
            if !(200..300).contains(&status) {
                tracing::warn!("Kit Store GitHub search status: {}", status);
                continue;
            }

            let mut body = response.into_body();
            let parsed = match body.read_json::<KitStoreGithubSearchResponse>() {
                Ok(parsed) => parsed,
                Err(error) => {
                    tracing::warn!("Kit Store GitHub JSON parse failed: {}", error);
                    continue;
                }
            };

            for item in parsed.items {
                if item.full_name.is_empty() || !seen.insert(item.full_name.clone()) {
                    continue;
                }
                results.push(KitStoreSearchResult {
                    name: item.name,
                    full_name: item.full_name,
                    description: item.description.unwrap_or_default(),
                    stars: item.stargazers_count,
                    html_url: item.html_url,
                    clone_url: item.clone_url,
                });
            }
        }

        results.sort_by(|a, b| b.stars.cmp(&a.stars).then_with(|| a.name.cmp(&b.name)));
        results
    }

    pub(crate) fn kit_store_list_installed() -> Vec<script_kit_gpui::kit_store::InstalledKit> {
        script_kit_gpui::kit_store::storage::list_installed_kits().unwrap_or_else(|error| {
            tracing::warn!("Kit Store list installed kits failed: {}", error);
            Vec::new()
        })
    }

    fn kit_store_derive_name(repo_url: &str) -> Result<String, String> {
        let trimmed = repo_url.trim();
        if trimmed.is_empty() {
            return Err("Repository URL is empty".to_string());
        }

        let without_query = trimmed.split('?').next().unwrap_or(trimmed);
        let without_fragment = without_query.split('#').next().unwrap_or(without_query);
        let normalized = match without_fragment.rsplit_once(':') {
            Some((_, remainder)) if without_fragment.contains('@') && remainder.contains('/') => {
                remainder
            }
            _ => without_fragment,
        };
        let normalized = normalized.trim_end_matches('/');
        let normalized = normalized.strip_suffix(".git").unwrap_or(normalized);
        let name = normalized.rsplit('/').next().unwrap_or_default().trim();
        if name.is_empty() {
            return Err(format!(
                "Unable to derive kit name from repository URL '{}'",
                repo_url
            ));
        }
        Ok(name.to_string())
    }

    fn kit_store_git_hash(path: &Path) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(path)
            .arg("rev-parse")
            .arg("HEAD")
            .output()
            .map_err(|error| KitStoreOperationStep::ReadGitHead.git_spawn_failure(error))?;
        if !output.status.success() {
            return Err(KitStoreOperationStep::ReadGitHead
                .git_status_failure(output.status, &output.stderr));
        }
        let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if hash.is_empty() {
            return Err(KitStoreOperationStep::ReadGitHead.empty_git_hash_message());
        }
        Ok(hash)
    }

    fn kit_store_install(
        result: &KitStoreSearchResult,
    ) -> Result<script_kit_gpui::kit_store::InstalledKit, String> {
        let repo_url = if result.clone_url.is_empty() {
            return Err("Selected kit is missing clone URL".to_string());
        } else {
            result.clone_url.clone()
        };

        // Delegate clone + plugin.json synthesis to git_ops, which installs
        // directly into the canonical plugin root (`<kit_path>/kit/<plugin-id>/`).
        let (plugin_id, install_path) =
            script_kit_gpui::kit_store::git_ops::install_kit(&repo_url)?;

        let git_hash = Self::kit_store_git_hash(&install_path)?;
        let mut kits = Self::kit_store_list_installed();
        kits.retain(|kit| kit.name != plugin_id);

        let installed = script_kit_gpui::kit_store::InstalledKit {
            name: plugin_id,
            path: install_path,
            repo_url,
            git_hash,
            // LAT_WHITELIST_RFC3339_STORAGE: raw RFC3339 belongs in the kit registry only;
            // display surfaces must format timestamps through crate::formatting.
            installed_at: chrono::Utc::now().to_rfc3339(),
        };
        kits.push(installed.clone());

        script_kit_gpui::kit_store::storage::save_kit_registry(&kits)
            .map_err(|error| KitStoreOperationStep::SaveInstalledRegistry.storage_failure(error))?;

        Ok(installed)
    }

    fn kit_store_update(kit: &script_kit_gpui::kit_store::InstalledKit) -> Result<(), String> {
        let pull_output = Command::new("git")
            .arg("-C")
            .arg(&kit.path)
            .arg("pull")
            .arg("--ff-only")
            .output()
            .map_err(|error| KitStoreOperationStep::PullRepository.git_spawn_failure(error))?;
        if !pull_output.status.success() {
            return Err(KitStoreOperationStep::PullRepository
                .git_status_failure(pull_output.status, &pull_output.stderr));
        }

        let latest_hash = Self::kit_store_git_hash(&kit.path)?;
        let mut kits = Self::kit_store_list_installed();
        if let Some(existing) = kits.iter_mut().find(|existing| existing.name == kit.name) {
            existing.git_hash = latest_hash;
        }

        script_kit_gpui::kit_store::storage::save_kit_registry(&kits)
            .map_err(|error| KitStoreOperationStep::SaveUpdatedRegistry.storage_failure(error))
    }

    fn kit_store_remove(kit: &script_kit_gpui::kit_store::InstalledKit) -> Result<(), String> {
        if kit.path.exists() {
            std::fs::remove_dir_all(&kit.path).map_err(|error| {
                KitStoreOperationStep::RemoveDirectory.remove_directory_failure(error)
            })?;
        }
        script_kit_gpui::kit_store::storage::remove_kit(&kit.name)
            .map_err(|error| KitStoreOperationStep::RemoveRegistry.storage_failure(error))
    }

    fn request_plugin_runtime_refresh(
        &mut self,
        action: KitStorePluginMutation,
        plugin_name: &str,
        cx: &mut Context<Self>,
    ) {
        tracing::info!(
            action = %action.action(),
            plugin_id = %plugin_name,
            "plugin_runtime_refresh_requested"
        );
        self.refresh_scripts(cx);
    }

    fn kit_store_refresh_installed_view(
        &mut self,
        removed_kit: Option<&str>,
        _cx: &mut Context<Self>,
    ) {
        let Some((filter, selected_index, old_keys)) = (match &self.current_view {
            AppView::InstalledKitsView {
                filter,
                selected_index,
                kits,
            } => Some((
                filter.clone(),
                *selected_index,
                Self::kit_store_installed_visible_rows(kits, filter)
                    .into_iter()
                    .map(|(_, kit)| kit.name.clone())
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        }) else {
            return;
        };
        let snapshot = capture_dynamic_uniform_list_snapshot(
            &old_keys,
            selected_index,
            &self.list_scroll_handle,
        );
        let refreshed = Self::kit_store_list_installed();
        let new_keys: Vec<String> =
            Self::kit_store_installed_visible_rows(&refreshed, &filter)
                .into_iter()
                .map(|(_, kit)| kit.name.clone())
                .collect();
        let restore = reconcile_dynamic_uniform_list_snapshot(&snapshot, &new_keys);
        if let AppView::InstalledKitsView {
            selected_index,
            kits,
            ..
        } = &mut self.current_view
        {
            *kits = refreshed;
            *selected_index = restore.selected_index;
        }
        restore_dynamic_uniform_list_viewport(&self.list_scroll_handle, &snapshot, restore);

        let removed_selected = removed_kit.is_some_and(|removed| {
            snapshot.selection.key.as_deref() == Some(removed)
        });
        if removed_selected && !restore.selected_key_survived && !new_keys.is_empty() {
            self.list_scroll_handle
                .scroll_to_item(restore.selected_index, ScrollStrategy::Nearest);
        }
    }

    fn kit_store_browse_visible_rows(
        results: &[KitStoreSearchResult],
    ) -> Vec<(usize, &KitStoreSearchResult)> {
        results.iter().enumerate().collect()
    }

    fn kit_store_installed_visible_rows<'a>(
        kits: &'a [script_kit_gpui::kit_store::InstalledKit],
        filter: &str,
    ) -> Vec<(usize, &'a script_kit_gpui::kit_store::InstalledKit)> {
        let needle = filter.trim().to_lowercase();
        kits.iter()
            .enumerate()
            .filter(|(_, kit)| {
                needle.is_empty()
                    || kit.name.to_lowercase().contains(&needle)
                    || kit.repo_url.to_lowercase().contains(&needle)
                    || kit.git_hash.to_lowercase().contains(&needle)
            })
            .collect()
    }

    fn kit_store_browse_selected_visible_result(
        results: &[KitStoreSearchResult],
        selected_index: usize,
    ) -> Option<KitStoreSearchResult> {
        Self::kit_store_browse_visible_rows(results)
            .get(selected_index)
            .map(|(_, result)| (*result).clone())
    }

    fn kit_store_installed_selected_visible_kit(
        kits: &[script_kit_gpui::kit_store::InstalledKit],
        filter: &str,
        selected_index: usize,
    ) -> Option<script_kit_gpui::kit_store::InstalledKit> {
        Self::kit_store_installed_visible_rows(kits, filter)
            .get(selected_index)
            .map(|(_, kit)| (*kit).clone())
    }

    fn kit_store_browse_dataset_and_visible_counts(
        results: &[KitStoreSearchResult],
    ) -> (usize, usize) {
        (
            results.len(),
            Self::kit_store_browse_visible_rows(results).len(),
        )
    }

    fn kit_store_installed_dataset_and_visible_counts(
        kits: &[script_kit_gpui::kit_store::InstalledKit],
        filter: &str,
    ) -> (usize, usize) {
        (
            kits.len(),
            Self::kit_store_installed_visible_rows(kits, filter).len(),
        )
    }

    fn kit_store_browse_visible_row_labels(results: &[KitStoreSearchResult]) -> Vec<String> {
        Self::kit_store_browse_visible_rows(results)
            .into_iter()
            .map(|(_, result)| result.full_name.clone())
            .collect()
    }

    fn kit_store_installed_visible_row_labels(
        kits: &[script_kit_gpui::kit_store::InstalledKit],
        filter: &str,
    ) -> Vec<String> {
        Self::kit_store_installed_visible_rows(kits, filter)
            .into_iter()
            .map(|(_, kit)| kit.name.clone())
            .collect()
    }

    fn kit_store_browse_row_description(result: &KitStoreSearchResult) -> String {
        if result.description.is_empty() {
            "No description".to_string()
        } else {
            result.description.clone()
        }
    }

    fn kit_store_browse_row_title(result: &KitStoreSearchResult) -> String {
        result.name.clone()
    }

    fn kit_store_browse_row_source_hint(result: &KitStoreSearchResult) -> Option<String> {
        Some(format!("{} · ★ {}", result.full_name, result.stars))
    }

    fn kit_store_browse_row_semantic_id(result: &KitStoreSearchResult) -> String {
        format!("kit-store-browse-row:{}", result.full_name)
    }

    fn kit_store_browse_count_label(total_results: usize) -> String {
        let suffix = if total_results == 1 { "" } else { "s" };
        format!("{} kit{}", total_results, suffix)
    }

    fn kit_store_installed_row_commit_label(
        kit: &script_kit_gpui::kit_store::InstalledKit,
    ) -> String {
        format!("commit {}", kit.git_hash)
    }

    fn kit_store_installed_row_title(kit: &script_kit_gpui::kit_store::InstalledKit) -> String {
        kit.name.clone()
    }

    fn kit_store_installed_row_description(
        kit: &script_kit_gpui::kit_store::InstalledKit,
    ) -> String {
        kit.repo_url.clone()
    }

    fn kit_store_installed_row_source_hint(
        kit: &script_kit_gpui::kit_store::InstalledKit,
    ) -> Option<String> {
        Some(Self::kit_store_installed_row_commit_label(kit))
    }

    fn kit_store_installed_row_semantic_id(
        kit: &script_kit_gpui::kit_store::InstalledKit,
    ) -> String {
        format!("kit-store-installed-row:{}", kit.name)
    }

    fn kit_store_installed_empty_state_from_filter(filter: &str) -> KitStoreInstalledEmptyState {
        if filter.trim().is_empty() {
            KitStoreInstalledEmptyState::Empty
        } else {
            KitStoreInstalledEmptyState::NoSearchResults
        }
    }

    fn kit_store_installed_count_label(total_kits: usize) -> String {
        let suffix = if total_kits == 1 { "" } else { "s" };
        format!("{} installed kit{}", total_kits, suffix)
    }

    fn kit_store_install_selected_result(
        &mut self,
        selected: &KitStoreSearchResult,
        cx: &mut Context<Self>,
    ) {
        let selected = selected.clone();

        self.toast_manager.push(
            components::toast::Toast::info(
                KitStorePluginMutation::Install.progress_message(&selected.name),
                &self.theme,
            )
            .duration_ms(Some(TOAST_INFO_MS)),
        );
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { Self::kit_store_install(&selected) })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(installed) => {
                    tracing::info!(
                        plugin_id = %installed.name,
                        install_path = %installed.path.display(),
                        "plugin_store_installed"
                    );
                    this.kit_store_refresh_installed_view(None, cx);
                    this.request_plugin_runtime_refresh(
                        KitStorePluginMutation::Install,
                        &installed.name,
                        cx,
                    );
                    this.toast_manager.push(
                        components::toast::Toast::success(
                            KitStorePluginMutation::Install.success_message(&installed.name),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                    cx.notify();
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        "plugin_store_install_failed"
                    );
                    this.toast_manager.push(
                        components::toast::Toast::error(
                            KitStorePluginMutation::Install.failure_message(None, &error),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn kit_store_update_selected_kit(
        &mut self,
        kit: &script_kit_gpui::kit_store::InstalledKit,
        cx: &mut Context<Self>,
    ) {
        let kit = kit.clone();
        let kit_name = kit.name.clone();

        self.toast_manager.push(
            components::toast::Toast::info(
                KitStorePluginMutation::Update.progress_message(&kit_name),
                &self.theme,
            )
            .duration_ms(Some(TOAST_INFO_MS)),
        );
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { Self::kit_store_update(&kit) })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    tracing::info!(plugin_id = %kit_name, "plugin_store_updated");
                    this.kit_store_refresh_installed_view(None, cx);
                    this.request_plugin_runtime_refresh(KitStorePluginMutation::Update, &kit_name, cx);
                    this.toast_manager.push(
                        components::toast::Toast::success(
                            KitStorePluginMutation::Update.success_message(&kit_name),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                    cx.notify();
                }
                Err(error) => {
                    tracing::warn!(plugin_id = %kit_name, error = %error, "plugin_store_update_failed");
                    this.toast_manager.push(
                        components::toast::Toast::error(
                            KitStorePluginMutation::Update
                                .failure_message(Some(&kit_name), &error),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn kit_store_remove_selected_kit(
        &mut self,
        kit: &script_kit_gpui::kit_store::InstalledKit,
        cx: &mut Context<Self>,
    ) {
        let kit = kit.clone();
        let kit_name = kit.name.clone();

        self.toast_manager.push(
            components::toast::Toast::info(
                KitStorePluginMutation::Remove.progress_message(&kit_name),
                &self.theme,
            )
            .duration_ms(Some(TOAST_INFO_MS)),
        );
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { Self::kit_store_remove(&kit) })
                .await;
            let _ = this.update(cx, |this, cx| match result {
                Ok(()) => {
                    tracing::info!(plugin_id = %kit_name, "plugin_store_removed");
                    this.kit_store_refresh_installed_view(Some(&kit_name), cx);
                    this.request_plugin_runtime_refresh(KitStorePluginMutation::Remove, &kit_name, cx);
                    this.toast_manager.push(
                        components::toast::Toast::success(
                            KitStorePluginMutation::Remove.success_message(&kit_name),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                    cx.notify();
                }
                Err(error) => {
                    tracing::warn!(plugin_id = %kit_name, error = %error, "plugin_store_remove_failed");
                    this.toast_manager.push(
                        components::toast::Toast::error(
                            KitStorePluginMutation::Remove
                                .failure_message(Some(&kit_name), &error),
                            &this.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                    cx.notify();
                }
            });
        })
            .detach();
    }

    pub(crate) fn kit_store_install_current_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let selected = if let AppView::BrowseKitsView {
            selected_index,
            results,
            ..
        } = &self.current_view
        {
            Self::kit_store_browse_selected_visible_result(results, *selected_index)
        } else {
            None
        };

        if let Some(selected) = selected {
            self.kit_store_install_selected_result(&selected, cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn kit_store_update_current_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let selected = if let AppView::InstalledKitsView {
            filter,
            selected_index,
            kits,
            ..
        } = &self.current_view
        {
            Self::kit_store_installed_selected_visible_kit(kits, filter, *selected_index)
        } else {
            None
        };

        if let Some(selected) = selected {
            self.kit_store_update_selected_kit(&selected, cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn kit_store_remove_current_selection(&mut self, cx: &mut Context<Self>) -> bool {
        let selected = if let AppView::InstalledKitsView {
            filter,
            selected_index,
            kits,
            ..
        } = &self.current_view
        {
            Self::kit_store_installed_selected_visible_kit(kits, filter, *selected_index)
        } else {
            None
        };

        if let Some(selected) = selected {
            self.kit_store_remove_selected_kit(&selected, cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn dispatch_kit_store_primary_footer_action(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        match &self.current_view {
            AppView::BrowseKitsView { .. } => {
                self.kit_store_install_current_selection(cx);
                true
            }
            AppView::InstalledKitsView { .. } => {
                self.kit_store_update_current_selection(cx);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn dispatch_kit_store_remove_footer_action(
        &mut self,
        cx: &mut Context<Self>,
    ) -> bool {
        if matches!(self.current_view, AppView::InstalledKitsView { .. }) {
            self.kit_store_remove_current_selection(cx);
            true
        } else {
            false
        }
    }

    pub(crate) fn dispatch_kit_store_browse_back_footer_action(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let query = if let AppView::BrowseKitsView { query, .. } = &self.current_view {
            Some(query.clone())
        } else {
            None
        };

        let Some(query) = query else {
            return false;
        };

        if query.is_empty() {
            self.go_back_or_close(window, cx);
        } else {
            self.kit_store_set_browse_query(String::new(), cx);
        }
        true
    }

    pub(crate) fn kit_store_set_browse_query(
        &mut self,
        next_query: String,
        cx: &mut Context<Self>,
    ) {
        let generation = next_kit_store_browse_query_generation();
        if let AppView::BrowseKitsView {
            query,
            selected_index,
            results,
        } = &mut self.current_view
        {
            *query = next_query.clone();
            *selected_index = 0;
            results.clear();
            self.list_scroll_handle
                .scroll_to_item(0, ScrollStrategy::Top);
            self.begin_list_viewport_scroll(
                crate::scrolling::list_interaction::ListViewportInputSource::Filter,
                cx,
            );
            cx.notify();
        }

        let query_for_fetch = next_query.clone();
        let query_for_guard = next_query;
        cx.spawn(async move |this, cx| {
            let results = cx
                .background_executor()
                .spawn(async move { Self::kit_store_search_results(&query_for_fetch) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if !kit_store_browse_query_generation_is_current(generation) {
                    return;
                }
                if let AppView::BrowseKitsView {
                    query,
                    results: view_results,
                    ..
                } = &mut this.current_view
                {
                    if *query == query_for_guard {
                        *view_results = results;
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }

    fn render_browse_kits(
        &mut self,
        query: &str,
        selected_index: usize,
        results: Vec<KitStoreSearchResult>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal("kit_store_browse", 2, false, false),
        );
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_name = rgba((chrome.text_primary_hex << 8) | 0xff);
        let text_muted = rgba(chrome.text_muted_rgba);
        let text_hint = rgba(chrome.text_hint_rgba);

        let query_owned = query.to_string();
        let input_is_empty = query_owned.is_empty();
        let total_results = results.len();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                let mut install_selected = false;

                if let AppView::BrowseKitsView {
                    query,
                    selected_index,
                    results,
                } = &mut this.current_view
                {
                    let mut handled = true;
                    match key {
                        _ if is_key_escape(key) => {
                            if query.is_empty() {
                                this.go_back_or_close(window, cx);
                            } else {
                                this.kit_store_set_browse_query(String::new(), cx);
                            }
                        }
                        _ if is_key_up(key) => {
                            if !results.is_empty() {
                                *selected_index = selected_index.saturating_sub(1);
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        _ if is_key_down(key) => {
                            if !results.is_empty() {
                                *selected_index = (*selected_index + 1)
                                    .min(results.len().saturating_sub(1));
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        _ if is_key_enter(key) => {
                            install_selected = true;
                        }
                        _ => handled = false,
                    }
                    if handled {
                        cx.stop_propagation();
                    }
                }

                if install_selected {
                    let selected = if let AppView::BrowseKitsView {
                        selected_index,
                        results,
                        ..
                    } = &this.current_view
                    {
                        Self::kit_store_browse_selected_visible_result(results, *selected_index)
                    } else {
                        None
                    };
                    if let Some(selected) = selected {
                        this.kit_store_install_selected_result(&selected, cx);
                    }
                }
            },
        );

        let selected_row = selected_index;
        let click_entity = cx.entity().downgrade();
        let hover_entity = cx.entity().downgrade();
        let results_for_list = results.clone();
        let list_colors = ListItemColors::from_theme(&self.theme);
        let main_menu_theme = self.current_main_menu_theme;
        let hovered = self.hovered_index;

        let list: AnyElement = if results_for_list.is_empty() {
            let empty_state = KitStoreBrowseEmptyState::from_query(&query_owned);
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .text_color(text_muted)
                .child(empty_state.title())
                .child(
                    div()
                        .text_xs()
                        .text_color(text_hint)
                        .child(empty_state.message()),
                )
                .into_any_element()
        } else {
            uniform_list(
                "kit-store-browse-list",
                results_for_list.len(),
                move |visible, _window, _cx| {
                    visible
                        .map(|ix| {
                            if let Some(result) = results_for_list.get(ix) {
                                let is_selected = ix == selected_row;
                                let is_hovered = hovered == Some(ix);

                                let row_entity = click_entity.clone();
                                let hover_entity = hover_entity.clone();
                                let result_for_click = result.clone();

                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = row_entity.upgrade() {
                                            let selected_result = result_for_click.clone();
                                            entity.update(cx, |this, cx| {
                                                let should_submit = if let AppView::BrowseKitsView {
                                                    selected_index,
                                                    ..
                                                } = &mut this.current_view
                                                {
                                                    let was_selected = *selected_index == ix;
                                                    *selected_index = ix;
                                                    crate::ui_foundation::should_submit_selected_row_click(
                                                        was_selected,
                                                        event.click_count(),
                                                    )
                                                } else {
                                                    false
                                                };
                                                this.list_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                this.note_list_pointer_click(ix, cx);
                                                if should_submit {
                                                    this.kit_store_install_selected_result(
                                                        &selected_result,
                                                        cx,
                                                    );
                                                }
                                            });
                                        }
                                        cx.stop_propagation();
                                    };

                                let move_entity = hover_entity.clone();
                                let move_handler =
                                    move |_event: &gpui::MouseMoveEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = move_entity.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                this.note_list_pointer_move(ix, cx);
                                            });
                                        }
                                    };
                                let hover_handler =
                                    move |is_hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = hover_entity.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                if !*is_hovered {
                                                    this.note_list_pointer_leave(ix, cx);
                                                }
                                            });
                                        }
                                    };

                                div()
                                    .id(Self::kit_store_browse_row_semantic_id(result))
                                    .cursor_pointer()
                                    .on_click(click_handler)
                                    .on_mouse_move(move_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(
                                            Self::kit_store_browse_row_title(result),
                                            list_colors,
                                        )
                                        .description_opt(Some(Self::kit_store_browse_row_description(
                                            result,
                                        )))
                                        .source_hint_opt(Self::kit_store_browse_row_source_hint(
                                            result,
                                        ))
                                        .selected(is_selected)
                                        .hovered(is_hovered)
                                        .main_menu_theme(main_menu_theme)
                                        .semantic_id(Self::kit_store_browse_row_semantic_id(result)),
                                    )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.list_scroll_handle, total_results, 8);

        let footer_hints: Vec<gpui::SharedString> = vec![
            "↵ Install".into(),
            if input_is_empty {
                "Esc Back".into()
            } else {
                "Esc Clear Search".into()
            },
        ];
        crate::components::emit_surface_prompt_hint_audit(
            "kit_store_browse",
            &footer_hints,
            "kit_store_browse_footer",
        );
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            footer_hints,
            None,
        ));

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .py(px(design_spacing.padding_xs))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .on_scroll_wheel(cx.listener(
                        move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                            this.observe_builtin_native_list_scroll(event, cx);
                        },
                    ))
                    .child(list)
                    .child(list_scrollbar),
            );

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .font_family(design_typography.font_family)
                .text_color(text_name)
                .key_context("kit_store_browse")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(Self::kit_store_browse_count_label(
                        total_results,
                    )),
                ], cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: content.into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
    }

    fn render_installed_kits(
        &mut self,
        filter: &str,
        selected_index: usize,
        kits: Vec<script_kit_gpui::kit_store::InstalledKit>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        crate::components::emit_prompt_chrome_audit(
            &crate::components::PromptChromeAudit::minimal("kit_store_installed", 2, false, false),
        );
        let tokens = get_tokens(self.current_design);
        let design_spacing = tokens.spacing();
        let design_typography = tokens.typography();

        let chrome = crate::theme::AppChromeColors::from_theme(&self.theme);
        let text_name = rgba((chrome.text_primary_hex << 8) | 0xff);
        let text_muted = rgba(chrome.text_muted_rgba);
        let text_hint = rgba(chrome.text_hint_rgba);
        let filter_owned = filter.to_string();
        let visible_rows = Self::kit_store_installed_visible_rows(&kits, &filter_owned);
        let total_kits = visible_rows.len();
        let dataset_kits = kits.len();
        let input_is_empty = filter_owned.is_empty();

        let handle_key = cx.listener(
            move |this: &mut Self,
                  event: &gpui::KeyDownEvent,
                  window: &mut Window,
                  cx: &mut Context<Self>| {
                this.hide_mouse_cursor(cx);

                let key = event.keystroke.key.as_str();
                let has_cmd = event.keystroke.modifiers.platform;

                if is_key_escape(key) {
                    this.go_back_or_close(window, cx);
                    cx.stop_propagation();
                    return;
                }
                if has_cmd && key.eq_ignore_ascii_case("w") {
                    this.close_and_reset_window(cx);
                    cx.stop_propagation();
                    return;
                }

                if let AppView::InstalledKitsView {
                    filter,
                    selected_index,
                    kits,
                } = &mut this.current_view
                {
                    let visible_len = Self::kit_store_installed_visible_rows(kits, filter).len();
                    let mut handled = true;
                    match key {
                        _ if is_key_up(key) => {
                            if visible_len > 0 {
                                *selected_index = selected_index.saturating_sub(1);
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        _ if is_key_down(key) => {
                            if visible_len > 0 {
                                *selected_index = (*selected_index + 1)
                                    .min(visible_len.saturating_sub(1));
                                this.list_scroll_handle
                                    .scroll_to_item(*selected_index, ScrollStrategy::Nearest);
                                cx.notify();
                            }
                        }
                        _ if is_key_enter(key) => {
                            let selected = Self::kit_store_installed_selected_visible_kit(
                                kits,
                                filter,
                                *selected_index,
                            );
                            if let Some(selected) = selected {
                                this.kit_store_update_selected_kit(&selected, cx);
                            }
                        }
                        "delete" => {
                            let selected = Self::kit_store_installed_selected_visible_kit(
                                kits,
                                filter,
                                *selected_index,
                            );
                            if let Some(selected) = selected {
                                this.kit_store_remove_selected_kit(&selected, cx);
                            }
                        }
                        _ => {
                            handled = false;
                        }
                    }
                    if handled {
                        cx.stop_propagation();
                    }
                }
            },
        );

        let selected_row = selected_index;
        let click_entity = cx.entity().downgrade();
        let hover_entity = cx.entity().downgrade();
        let kits_for_list: Vec<script_kit_gpui::kit_store::InstalledKit> = visible_rows
            .into_iter()
            .map(|(_, kit)| kit.clone())
            .collect();
        let list_colors = ListItemColors::from_theme(&self.theme);
        let main_menu_theme = self.current_main_menu_theme;
        let hovered = self.hovered_index;

        let list: AnyElement = if kits_for_list.is_empty() {
            let empty_state = if dataset_kits == 0 {
                KitStoreInstalledEmptyState::Empty
            } else {
                Self::kit_store_installed_empty_state_from_filter(&filter_owned)
            };
            div()
                .w_full()
                .h_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(8.0))
                .text_color(text_muted)
                .child(empty_state.title())
                .child(
                    div()
                        .text_xs()
                        .text_color(text_hint)
                        .child(empty_state.message()),
                )
                .into_any_element()
        } else {
            uniform_list(
                "kit-store-installed-list",
                kits_for_list.len(),
                move |visible, _window, _cx| {
                    visible
                        .map(|ix| {
                            if let Some(kit) = kits_for_list.get(ix) {
                                let is_selected = ix == selected_row;
                                let is_hovered = hovered == Some(ix);

                                let row_entity = click_entity.clone();
                                let hover_entity = hover_entity.clone();
                                let kit_for_click = kit.clone();

                                let click_handler =
                                    move |event: &gpui::ClickEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = row_entity.upgrade() {
                                            let selected_kit = kit_for_click.clone();
                                            entity.update(cx, |this, cx| {
                                                let should_submit =
                                                    if let AppView::InstalledKitsView {
                                                        selected_index,
                                                        ..
                                                    } = &mut this.current_view
                                                    {
                                                        let was_selected = *selected_index == ix;
                                                        *selected_index = ix;
                                                        crate::ui_foundation::should_submit_selected_row_click(
                                                            was_selected,
                                                            event.click_count(),
                                                        )
                                                } else {
                                                    false
                                                };
                                                this.list_scroll_handle
                                                    .scroll_to_item(ix, ScrollStrategy::Nearest);
                                                this.note_list_pointer_click(ix, cx);
                                                if should_submit {
                                                    this.kit_store_update_selected_kit(
                                                        &selected_kit,
                                                        cx,
                                                    );
                                                }
                                            });
                                        }
                                        cx.stop_propagation();
                                    };

                                let move_entity = hover_entity.clone();
                                let move_handler =
                                    move |_event: &gpui::MouseMoveEvent,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = move_entity.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                this.note_list_pointer_move(ix, cx);
                                            });
                                        }
                                    };
                                let hover_handler =
                                    move |is_hovered: &bool,
                                          _window: &mut Window,
                                          cx: &mut gpui::App| {
                                        if let Some(entity) = hover_entity.upgrade() {
                                            entity.update(cx, |this, cx| {
                                                if !*is_hovered {
                                                    this.note_list_pointer_leave(ix, cx);
                                                }
                                            });
                                        }
                                    };

                                div()
                                    .id(Self::kit_store_installed_row_semantic_id(kit))
                                    .cursor_pointer()
                                    .on_click(click_handler)
                                    .on_mouse_move(move_handler)
                                    .on_hover(hover_handler)
                                    .child(
                                        ListItem::new(
                                            Self::kit_store_installed_row_title(kit),
                                            list_colors,
                                        )
                                        .description_opt(Some(
                                            Self::kit_store_installed_row_description(kit),
                                        ))
                                        .source_hint_opt(Self::kit_store_installed_row_source_hint(
                                            kit,
                                        ))
                                        .selected(is_selected)
                                        .hovered(is_hovered)
                                        .main_menu_theme(main_menu_theme)
                                        .semantic_id(Self::kit_store_installed_row_semantic_id(kit)),
                                    )
                            } else {
                                div().id(ix).h(px(LIST_ITEM_HEIGHT))
                            }
                        })
                        .collect()
                },
            )
            .h_full()
            .track_scroll(&self.list_scroll_handle)
            .into_any_element()
        };
        let list_scrollbar =
            self.builtin_uniform_list_scrollbar(&self.list_scroll_handle, total_kits, 8);

        let footer_hints: Vec<gpui::SharedString> = vec![
            "↵ Update".into(),
            "⌦ Remove".into(),
            if input_is_empty {
                "Esc Back".into()
            } else {
                "Esc Clear Search".into()
            },
        ];
        crate::components::emit_surface_prompt_hint_audit(
            "kit_store_installed",
            &footer_hints,
            "kit_store_installed_footer",
        );
        let footer = self.main_window_footer_slot(crate::components::render_simple_hint_strip(
            footer_hints,
            None,
        ));

        let content = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.0))
            .w_full()
            .py(px(design_spacing.padding_xs))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h_full()
                    .on_scroll_wheel(cx.listener(
                        move |this, event: &gpui::ScrollWheelEvent, _window, cx| {
                            this.observe_builtin_native_list_scroll(event, cx);
                        },
                    ))
                    .child(list)
                    .child(list_scrollbar),
            );

        let menu_def = self.current_main_menu_theme.def();
        let shell = menu_def.shell;

        crate::components::main_view_chrome::render_main_view_chrome_footer_flush(
            crate::components::main_view_chrome::render_main_view_shell()
                .font_family(design_typography.font_family)
                .text_color(text_name)
                .key_context("kit_store_installed")
                .track_focus(&self.focus_handle)
                .on_key_down(handle_key),
            &self.theme,
            menu_def,
            crate::components::main_view_chrome::MainViewChrome {
                header: self.render_builtin_main_input_header(vec![
                    self.render_builtin_main_input_count_label(
                        Self::kit_store_installed_count_label(dataset_kits),
                    ),
                ], cx),
                divider: crate::components::main_view_chrome::MainViewDividerChrome {
                    margin_x: shell.divider_margin_x,
                    height: shell.divider_height,
                    visible: shell.divider_height > 0.0,
                },
                main: content.into_any_element(),
                footer,
                overlays: Vec::new(),
            },
        )
    }
}

#[cfg(test)]
mod kit_store_dynamic_list_tests {
    use super::*;

    #[test]
    fn kit_store_browse_async_generation_fails_closed() {
        let stale = next_kit_store_browse_query_generation();
        let current = next_kit_store_browse_query_generation();
        assert!(!kit_store_browse_query_generation_is_current(stale));
        assert!(kit_store_browse_query_generation_is_current(current));
    }

    #[test]
    fn kit_store_semantic_ids_do_not_include_row_ordinals() {
        let result = KitStoreSearchResult {
            full_name: "owner/repository".to_string(),
            ..Default::default()
        };
        assert_eq!(
            ScriptListApp::kit_store_browse_row_semantic_id(&result),
            "kit-store-browse-row:owner/repository"
        );
    }
}
