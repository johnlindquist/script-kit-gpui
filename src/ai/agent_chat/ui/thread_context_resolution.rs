impl AgentChatThread {
    /// Mark pending context as ready for the next submit.
    fn arm_pending_context(&mut self, reason: &'static str) {
        self.pending_context_consumed = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_armed",
            reason,
            pending_part_count = self.pending_context_items.len(),
            pending_block_count = self.pending_context_blocks.len(),
            ambient_enabled = self.pending_ambient_context_enabled,
        );
    }

    /// Clear hidden ambient context blocks and disable the ambient flag.
    fn clear_pending_ambient_context(&mut self, reason: &'static str) {
        let cleared_block_count = self.pending_context_blocks.len();
        self.pending_context_blocks.clear();
        self.pending_ambient_context_enabled = false;
        self.pending_context_consumed = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_ambient_context_cleared",
            reason,
            cleared_block_count,
            pending_part_count = self.pending_context_items.len(),
        );
    }

    /// Clear all pending context state (parts, blocks, flags).
    fn clear_all_pending_context(&mut self, reason: &'static str) {
        let cleared_part_count = self.pending_context_items.len();
        let cleared_block_count = self.pending_context_blocks.len();
        self.pending_context_items.clear();
        self.pending_context_blocks.clear();
        self.pending_context_consumed = false;
        self.pending_ambient_context_enabled = false;
        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_pending_context_cleared",
            reason,
            cleared_part_count,
            cleared_block_count,
        );
    }

    /// Returns `true` for the explicit `@screenshot` resource chip.
    ///
    /// Agent Chat follow-up submits should attach this as a real image block instead
    /// of only resolving the text-only `kit://context?...` snapshot JSON.
    fn is_explicit_screenshot_part(part: &crate::ai::message_parts::AiContextPart) -> bool {
        matches!(
            part,
            crate::ai::message_parts::AiContextPart::ResourceUri { uri, label }
                if label == "Screenshot" && uri.contains("screenshot=1")
        )
    }

    /// Capture the explicit screenshot chip as an Agent Chat image block.
    ///
    /// `@screenshot` captures the active desktop — the display the Script Kit
    /// panel is on — with Script Kit's own windows excluded so the chat panel
    /// does not cover the content being asked about.
    ///
    /// Returns `Ok(None)` for non-screenshot parts so the normal prompt-block
    /// resolver can handle them. On capture failure the caller falls back to
    /// the canonical `kit://context?...` resource path.
    fn capture_special_context_block_for_part(
        part: &crate::ai::message_parts::AiContextPart,
    ) -> Result<Option<ContentBlock>, String> {
        if !Self::is_explicit_screenshot_part(part) {
            return Ok(None);
        }

        let (png_data, width, height) =
            crate::platform::capture_screen_screenshot().map_err(|error| error.to_string())?;
        if png_data.is_empty() {
            return Err("Active desktop screenshot was empty".to_string());
        }

        use base64::Engine as _;

        tracing::info!(
            target: "script_kit::tab_ai",
            event = "agent_chat_inline_screenshot_attachment_captured",
            width,
            height,
            title = "Active desktop",
            bytes = png_data.len(),
        );

        let base64_png = base64::engine::general_purpose::STANDARD.encode(&png_data);
        Ok(Some(ContentBlock::Image(ImageContent::new(
            base64_png,
            "image/png",
        ))))
    }

    /// Resolve pending context parts into Agent Chat blocks plus a standard receipt.
    ///
    /// Most parts resolve into text prompt blocks. Explicit screenshot chips
    /// are upgraded to real Agent Chat attachment blocks first, with the canonical
    /// resource resolver kept as a fallback if image capture fails.
    fn resolve_pending_context_items_with<F>(
        items: &[StagedContextItem],
        mut special_block_resolver: F,
    ) -> ResolvedPendingContext
    where
        F: FnMut(&crate::ai::message_parts::AiContextPart) -> Result<Option<ContentBlock>, String>,
    {
        let mut blocks = Vec::new();
        let mut prompt_blocks = Vec::new();
        let mut failures = Vec::new();
        let mut transition = PreparedContextTransition {
            attempted_items: items.to_vec(),
            attempted_ids: items.iter().map(|item| item.id.clone()).collect(),
            ..Default::default()
        };

        for item in items {
            let part = &item.part;
            match special_block_resolver(part) {
                Ok(Some(block)) => {
                    tracing::info!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_part_resolved_to_special_block",
                        context_item_id = %item.id.as_str(),
                        source_kind = ?part.source_kind(),
                    );
                    blocks.push(block);
                    transition.resolved_ids.push(item.id.clone());
                    continue;
                }
                Ok(None) => {}
                Err(error) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_special_block_capture_failed",
                        context_item_id = %item.id.as_str(),
                        source_kind = ?part.source_kind(),
                        error_len = error.len(),
                    );
                }
            }

            match crate::ai::message_parts::resolve_context_item_to_prompt_block(
                part,
                item.role.preparation_role(),
                &[],
                &[],
            ) {
                Ok(block) => {
                    transition.resolved_ids.push(item.id.clone());
                    if block.trim().is_empty() {
                        tracing::info!(
                            target: "script_kit::tab_ai",
                            event = "agent_chat_context_part_prompt_block_empty",
                            context_item_id = %item.id.as_str(),
                            source_kind = ?part.source_kind(),
                        );
                    } else {
                        prompt_blocks.push(block);
                    }
                }
                Err(failure) => {
                    tracing::warn!(
                        target: "script_kit::tab_ai",
                        event = "agent_chat_context_part_prompt_resolution_failed",
                        context_item_id = %item.id.as_str(),
                        source_kind = ?part.source_kind(),
                        role = item.role.as_str(),
                        failure_code = ?failure.failure.code,
                    );
                    transition.failures.push((item.id.clone(), failure.clone()));
                    failures.push(crate::ai::message_parts::ContextResolutionFailure {
                        part_id: item.id.0.clone(),
                        source_kind: part.source_kind(),
                        role: item.role.preparation_role(),
                        failure,
                    });
                }
            }
        }

        let prompt_prefix = prompt_blocks.join("\n\n");
        ResolvedPendingContext {
            blocks,
            receipt: crate::ai::message_parts::ContextResolutionReceipt {
                attempted: items.len(),
                resolved: transition.resolved_ids.len(),
                failures,
                prompt_prefix,
            },
            transition,
        }
    }

    /// Snapshot and consume staged context without resolving any part.
    ///
    /// This is the only pending-context operation called by `submit_input`;
    /// the returned job is resolved by `prepare_captured_turn_in_background`.
    fn take_pending_context_for_background_resolution(
        &mut self,
    ) -> Option<PendingContextResolutionJob> {
        self.enforce_zero_context_before_turn("take_pending_context_for_background_resolution");
        let has_pending_parts = !self.pending_context_items.is_empty();
        let has_pending_blocks = !self.pending_context_blocks.is_empty();
        if self.pending_context_consumed || (!has_pending_parts && !has_pending_blocks) {
            return None;
        }

        let blocks = self.pending_context_blocks.clone();
        let items = self.pending_context_items.clone();
        let attachments = items
            .iter()
            .map(|item| AgentChatMessageAttachment::from_part(&item.part))
            .collect();
        for pending in &mut self.pending_context_items {
            if items.iter().any(|item| item.id == pending.id) {
                pending.state = ContextLifecycleState::Resolving;
            }
        }

        Some(PendingContextResolutionJob {
            blocks,
            items,
            attachments,
        })
    }

    fn prepare_captured_turn_in_background(
        input: &str,
        context_job: Option<PendingContextResolutionJob>,
        should_stage_brain_recall: bool,
    ) -> PreparedTurnBlocks {
        let mut blocks = Vec::new();
        if should_stage_brain_recall {
            if let Some(recall) = crate::brain::recall_context_block(input).ok().flatten() {
                tracing::info!(
                    target: "script_kit::brain",
                    event = "agent_chat_brain_recall_staged",
                    chars = recall.len(),
                );
                blocks.push(ContentBlock::Text(TextContent::new(recall)));
            }
            crate::brain::record_ask_signals(input);
        }

        if let Some(job) = context_job {
            let consumed_part_count = job.items.len();
            let consumed_hidden_block_count = job.blocks.len();
            let attachments = job.attachments;
            blocks.extend(job.blocks);
            let resolved = Self::resolve_pending_context_items_with(
                &job.items,
                Self::capture_special_context_block_for_part,
            );
            let consumed_special_block_count = resolved.blocks.len();
            let receipt = resolved.receipt;
            let context_transition = resolved.transition;
            blocks.extend(resolved.blocks);

            tracing::info!(
                target: "script_kit::tab_ai",
                event = "agent_chat_pending_context_consumed",
                consumed_part_count,
                consumed_hidden_block_count,
                consumed_special_block_count,
                resolved_part_count = receipt.resolved,
                failed_part_count = receipt.failures.len(),
            );
            if !receipt.prompt_prefix.is_empty() {
                blocks.push(ContentBlock::Text(TextContent::new(
                    receipt.prompt_prefix.clone(),
                )));
            }
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "--- USER REQUEST ---\n{input}"
            ))));
            return PreparedTurnBlocks {
                blocks,
                receipt: Some(receipt),
                attachments,
                context_transition,
            };
        }

        if blocks.is_empty() {
            blocks.push(ContentBlock::Text(TextContent::new(input)));
        } else {
            blocks.push(ContentBlock::Text(TextContent::new(format!(
                "--- USER REQUEST ---\n{input}"
            ))));
        }
        PreparedTurnBlocks {
            blocks,
            receipt: None,
            attachments: Vec::new(),
            context_transition: PreparedContextTransition::default(),
        }
    }
}
