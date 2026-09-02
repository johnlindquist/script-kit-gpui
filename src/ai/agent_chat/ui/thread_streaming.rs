impl AgentChatThread {
    fn start_streaming_text_drain_if_needed(&mut self, cx: &mut Context<Self>) {
        if self.streaming_text_buffer.is_empty() || self.streaming_text_drain_task.is_some() {
            return;
        }

        let generation = self.transcript_generation;
        let stream_generation = self.stream_generation;
        let turn_id = self.current_turn_id;
        let thread_id = self.ui_thread_id.clone();
        let fixture_gate = self
            .fixture_control()
            .and_then(|control| control.take_drain_gate(stream_generation));
        let controlled_drain = fixture_gate.is_some();
        self.streaming_text_drain_task = Some(cx.spawn(async move |this, cx| {
            if let Some(gate) = fixture_gate {
                if gate.recv().await.is_err() {
                    return;
                }
            }
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(16))
                    .await;

                let should_continue = this
                    .update(cx, |this, cx| {
                        // Only the bounded fixture retains a pre-cancellation task.
                        // Both fixture and product execute the exact same drain owner.
                        let before = controlled_drain.then(|| {
                            (
                                this.streaming_text_buffer.clone(),
                                this.streaming_text_drain_task.is_some(),
                                this.messages
                                    .iter()
                                    .map(|message| message.body.clone())
                                    .collect::<Vec<_>>(),
                            )
                        });
                        let stale = this.transcript_generation != generation
                            || this.stream_generation != stream_generation
                            || this.current_turn_id != turn_id
                            || this.ui_thread_id != thread_id;
                        let keep_draining = this.drain_streaming_text_for_identity(
                            generation,
                            stream_generation,
                            turn_id,
                            &thread_id,
                            cx,
                        );
                        if let (Some((buffer, task_present, messages)), Some(control)) =
                            (before, this.fixture_control())
                        {
                            control.record_drain_callback(
                                super::mock_fixture::FixtureDrainReceipt {
                                    stream_generation,
                                    stale_rejected: stale,
                                    replacement_stream_generation: this.stream_generation,
                                    replacement_buffer_unchanged: buffer
                                        == this.streaming_text_buffer,
                                    replacement_task_present: task_present,
                                    replacement_task_preserved: task_present
                                        == this.streaming_text_drain_task.is_some(),
                                    replacement_transcript_unchanged: messages
                                        .iter()
                                        .eq(this.messages.iter().map(|message| &message.body)),
                                    ..Default::default()
                                },
                            );
                        }
                        keep_draining
                    })
                    .unwrap_or(false);

                if !should_continue {
                    break;
                }
            }
        }));
    }

    fn drain_streaming_text_for_identity(
        &mut self,
        generation: u64,
        stream_generation: u64,
        turn_id: u64,
        thread_id: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.transcript_generation != generation
            || self.stream_generation != stream_generation
            || self.current_turn_id != turn_id
            || self.ui_thread_id != thread_id
        {
            // A stale callback owns neither the replacement buffer nor its task.
            return false;
        }
        if self.drain_streaming_text_once() {
            self.notify_semantic_change(cx);
        }
        if self.streaming_text_buffer.is_empty() {
            self.streaming_text_drain_task = None;
            false
        } else {
            true
        }
    }

    fn drain_streaming_text_once(&mut self) -> bool {
        let budget = self.streaming_text_buffer.drain_budget_for_tick();
        let Some(delta) = self.streaming_text_buffer.drain_next(budget) else {
            return false;
        };
        let delta_bytes = delta.len();
        let changed = self.append_assistant_stream_delta(delta);
        if changed {
            // WP-B3: a drained delta that actually mutated a visible row, plus
            // the committed byte count.
            crate::chat_hot_counters::record_agent_assistant_commit(delta_bytes);
        }
        changed
    }

    fn flush_streaming_text_buffer(&mut self) -> bool {
        let delta = self.streaming_text_buffer.flush_all();
        self.streaming_text_drain_task = None;
        if delta.is_empty() {
            return false;
        }
        let delta_bytes = delta.len();
        let changed = self.append_assistant_stream_delta(delta);
        if changed {
            // WP-B3: a synchronous flush that committed buffered text to a row.
            crate::chat_hot_counters::record_agent_assistant_commit(delta_bytes);
        }
        changed
    }
}
