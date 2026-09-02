use super::*;

impl ScriptListApp {
    /// Show the naming dialog for creating a new script or extension.
    ///
    /// This creates a NamingPrompt entity and switches to the NamingPrompt view.
    /// The user types a friendly name and sees a live kebab-case filename preview.
    /// On submit, the naming channel receives the payload; on cancel, it receives None.
    pub(crate) fn show_naming_dialog(
        &mut self,
        target: prompts::NamingTarget,
        cx: &mut Context<Self>,
    ) {
        self.present_naming_dialog(target, None, cx);
    }

    /// Show the naming dialog already seeded with a selected script template.
    ///
    /// Called from the Script Template Catalog view's Enter handler. The
    /// template identity is threaded through [`prompts::NamingPromptConfig`]
    /// → [`prompts::NamingSubmitResult::template_id`] so
    /// [`Self::handle_naming_dialog_completion`] can resolve it back via
    /// [`crate::mcp_resources::find_script_template`] and render its final
    /// script body with [`crate::mcp_resources::render_script_template_file`]
    /// before exclusively creating the file and opening the editor.
    pub(crate) fn show_naming_dialog_for_script_template(
        &mut self,
        template: crate::mcp_resources::ScriptTemplateRef,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let selection = prompts::TemplateSelection {
            id: template.id.clone(),
            label: template.title.clone(),
        };
        self.present_naming_dialog(prompts::NamingTarget::Script, Some(selection), cx);
    }

    fn present_naming_dialog(
        &mut self,
        target: prompts::NamingTarget,
        template: Option<prompts::TemplateSelection>,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = crate::runtime_policy::check(crate::runtime_policy::ExternalEffect::ExternalStorage) {
            self.show_error_toast(error.to_string(), cx);
            return;
        }
        let (target_directory, extension) = match target {
            prompts::NamingTarget::Script => (script_creation::scripts_dir(), "ts"),
            prompts::NamingTarget::Extension => (script_creation::scriptlets_dir(), "md"),
        };

        let id = format!("naming-{}", target.as_str());

        let mut config = prompts::NamingPromptConfig::new(target, target_directory, extension)
            .placeholder(format!("My Cool {}", target.display_name()))
            .design_variant(self.current_design);
        if let Some(selection) = template {
            config = config.template(selection.id, selection.label);
        }

        use crate::design_evaluation::prompt_fixtures::{PromptSeed, PromptSeedCommon, NamingPromptSeed};
        let seed = PromptSeed::Naming(NamingPromptSeed {
            common: PromptSeedCommon::naming(id, self.naming_submit_sender.clone()),
            config, input: String::new(),
        });
        if let Err(error) = self.construct_prompt_seed(seed, cx) {
            self.show_error_toast(error.to_string(), cx);
            return;
        }
        self.opened_from_main_menu = true;

        logging::log(
            "NAMING",
            &format!("Showing naming dialog for {}", target.as_str()),
        );
    }

    /// Handle the result from the naming dialog channel.
    ///
    /// - `None` → user cancelled (Esc) → go back to script list
    /// - `Some(json)` → user submitted → create file, open in editor, show feedback
    pub(crate) fn handle_naming_dialog_completion(
        &mut self,
        payload: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(json) = payload else {
            // User cancelled
            logging::log("NAMING", "Naming dialog cancelled - returning to main menu");
            self.go_back_or_close(window, cx);
            return;
        };

        let result: prompts::NamingSubmitResult = match serde_json::from_str(&json) {
            Ok(r) => r,
            Err(e) => {
                tracing::error!(error = %e, payload = %json, "Failed to parse naming payload");
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to parse naming result: {}", e),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
                self.go_back_or_close(window, cx);
                return;
            }
        };

        // Extract the stem (filename without extension) for the creation functions
        let filename_stem = match std::path::Path::new(&result.filename).file_stem() {
            Some(s) => s.to_string_lossy().to_string(),
            None => result.filename.clone(),
        };

        let item_type = result.target.display_name().to_lowercase();

        logging::log(
            "NAMING",
            &format!(
                "Creating {} with name '{}' (filename: {})",
                item_type, result.friendly_name, result.filename
            ),
        );

        let rendered_script_template = if result.target == prompts::NamingTarget::Script {
            result
                .template_id
                .as_deref()
                .and_then(crate::mcp_resources::find_script_template)
                .map(|template| {
                    crate::mcp_resources::render_script_template_file(
                        &template,
                        &result.friendly_name,
                    )
                })
        } else {
            None
        };

        let create_result = match result.target {
            prompts::NamingTarget::Script => match rendered_script_template.as_deref() {
                Some(contents) => {
                    script_creation::create_new_script_with_contents(&filename_stem, contents)
                }
                None => script_creation::create_new_script(&filename_stem),
            },
            prompts::NamingTarget::Extension => {
                script_creation::create_new_scriptlet(&filename_stem)
            }
        };

        match create_result {
            Ok(path) => {
                let created_file_path: std::path::PathBuf = if path.is_absolute() {
                    path.clone()
                } else {
                    match std::env::current_dir() {
                        Ok(cwd) => cwd.join(&path),
                        Err(_) => path.clone(),
                    }
                };

                logging::log(
                    "NAMING",
                    &format!("Created new {}: {:?}", item_type, created_file_path),
                );

                // Preserve the existing unknown-template fallback and error
                // while writing recognized templates only through their
                // original exclusively created file handle.
                if result.target == prompts::NamingTarget::Script {
                    if let Some(template_id) = result.template_id.as_deref() {
                        if rendered_script_template.is_none() {
                            logging::log(
                                "WARN",
                                &format!(
                                    "Naming payload referenced unknown template_id: {}",
                                    template_id
                                ),
                            );
                            self.toast_manager.push(
                                components::toast::Toast::error(
                                    format!("Unknown template: {}", template_id),
                                    &self.theme,
                                )
                                .duration_ms(Some(TOAST_ERROR_MS)),
                            );
                        }
                    }
                }

                if result.target == prompts::NamingTarget::Script {
                    let used_script_template = result.template_id.is_some();
                    let receipt_prompt = if let Some(template_id) = result.template_id.as_deref() {
                        format!(
                            "Created local script '{}' from Script Template Catalog template '{}'",
                            result.friendly_name, template_id
                        )
                    } else {
                        format!(
                            "Created local script '{}' from NamingPrompt",
                            result.friendly_name
                        )
                    };
                    let receipt_result =
                        crate::ai::script_generation::write_script_creation_receipt_for_path(
                            &created_file_path,
                            &receipt_prompt,
                            &result.friendly_name,
                            if used_script_template {
                                "script_template_catalog"
                            } else {
                                "naming_prompt"
                            },
                            if used_script_template {
                                "script-template"
                            } else {
                                "manual"
                            },
                            "script-creation",
                        );
                    if let Err(error) = receipt_result {
                        tracing::warn!(
                            target: "naming",
                            error = %error,
                            path = %created_file_path.display(),
                            "script_creation_receipt.unavailable"
                        );
                        self.toast_manager.push(
                            components::toast::Toast::warning(
                                "Created script, but verification receipt was unavailable",
                                &self.theme,
                            )
                            .duration_ms(Some(TOAST_ERROR_MS)),
                        );
                    }
                }

                if let Err(e) = script_creation::open_in_editor(&path, &self.config) {
                    logging::log("ERROR", &format!("Failed to open in editor: {}", e));
                    self.toast_manager.push(
                        components::toast::Toast::error(
                            format!("Created {} but failed to open editor: {}", item_type, e),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_ERROR_MS)),
                    );
                } else {
                    self.toast_manager.push(
                        components::toast::Toast::success(
                            format!("New {} created and opened in editor", item_type),
                            &self.theme,
                        )
                        .duration_ms(Some(TOAST_SUCCESS_MS)),
                    );
                }

                self.open_creation_feedback_payload(
                    prompts::CreationFeedbackPayload::local_artifact(created_file_path),
                    cx,
                );
            }
            Err(e) => {
                logging::log("ERROR", &format!("Failed to create {}: {}", item_type, e));
                self.toast_manager.push(
                    components::toast::Toast::error(
                        format!("Failed to create {}: {}", item_type, e),
                        &self.theme,
                    )
                    .duration_ms(Some(TOAST_ERROR_MS)),
                );
                self.go_back_or_close(window, cx);
            }
        }
    }
}
