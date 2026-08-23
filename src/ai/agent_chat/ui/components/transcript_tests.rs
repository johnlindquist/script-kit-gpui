#[cfg(test)]
mod tests {
    use super::*;

    fn message(
        role: AgentChatThreadMessageRole,
        body: impl Into<SharedString>,
    ) -> AgentChatThreadMessage {
        AgentChatThreadMessage {
            id: 1,
            role,
            body: body.into(),
            tool_call_id: None,
            tool_meta: None,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn transcript_fidelity_ids_are_unique_per_rendered_message() {
        let mut assistant = message(AgentChatThreadMessageRole::Assistant, "assistant");
        assistant.id = 17;
        let mut system = message(AgentChatThreadMessageRole::System, "system");
        system.id = 18;
        let mut tool = message(AgentChatThreadMessageRole::Tool, "tool");
        tool.id = 20;

        assert_eq!(
            transcript_row_fidelity_id(&assistant),
            "agent-chat-transcript-row-assistant-17"
        );
        assert_eq!(
            transcript_row_fidelity_id(&system),
            "agent-chat-transcript-row-system-18"
        );
        assert_eq!(
            transcript_row_fidelity_id(&tool),
            "agent-chat-transcript-row-tool-20"
        );
    }

    #[test]
    fn pending_activity_transfers_with_the_latest_user_response() {
        let prior_user = message(AgentChatThreadMessageRole::User, "Earlier");
        let prior_empty_assistant = message(AgentChatThreadMessageRole::Assistant, "");
        let latest_user = message(AgentChatThreadMessageRole::User, "Latest");
        let mut messages = vec![prior_user, prior_empty_assistant, latest_user];

        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::TailSentinel,
            "historical empty assistant rows must not capture current activity"
        );

        messages.push(message(AgentChatThreadMessageRole::Assistant, ""));
        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::EmptyAssistantRow(3)
        );

        messages[3].body = "First token".into();
        assert_eq!(
            pending_activity_placement(&messages, true),
            PendingActivityPlacement::Hidden,
            "first visible assistant text must replace pending activity"
        );

        assert_eq!(
            pending_activity_placement(&messages, false),
            PendingActivityPlacement::Hidden
        );
    }

    #[test]
    fn first_visible_assistant_text_parses_immediately() {
        let assistant = message(AgentChatThreadMessageRole::Assistant, "First token");
        assert!(AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some(""),
            "First token",
            true
        ));
        assert!(AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            None,
            "First token",
            true
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            None,
            "Historical answer",
            false
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some("First"),
            "First token",
            true
        ));
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &assistant,
            Some(""),
            "   ",
            true
        ));

        let user = message(AgentChatThreadMessageRole::User, "First token");
        assert!(!AgentChatTranscript::should_parse_message_immediately(
            &user,
            Some(""),
            "First token",
            true
        ));
    }

    #[test]
    fn heavy_markdown_stats_count_markdown_and_bare_links() {
        let body = [
            "[Calendar](scriptkit://run/add-to-google-calendar)",
            "[Docs](https://example.com/docs) and https://example.com/raw",
            "[empty]() [not a link]",
        ]
        .join("\n");

        let stats = HeavyMarkdownStats::from_text(&body);

        // Calendar: 1 markdown link (its scriptkit:// target is excluded from
        // bare counting). Docs: 1 markdown link + 1 bare URL outside the
        // target. Empty target and bracket-only text count as 0.
        assert_eq!(stats.link_like_spans, 3);
    }

    #[test]
    fn link_dense_user_messages_use_heavy_preview_path() {
        let body = (0..14)
            .map(|ix| format!("[Brain source {ix}](scriptkit://agent-chat/thread-{ix})"))
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::User, body);

        assert!(stats.is_scroll_heavy());
        assert!(AgentChatTranscript::should_use_heavy_markdown_preview(
            &msg, stats
        ));
    }

    #[test]
    fn heavy_markdown_preview_still_skips_tool_rows() {
        let body = (0..20)
            .map(|ix| format!("[Tool source {ix}](https://example.com/{ix})"))
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::Tool, body);

        assert!(stats.is_scroll_heavy());
        assert!(!AgentChatTranscript::should_use_heavy_markdown_preview(
            &msg, stats
        ));
    }

    #[test]
    fn quick_ai_link_policy_is_assistant_only() {
        let assistant = message(AgentChatThreadMessageRole::Assistant, "answer");
        assert_eq!(
            AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::QuickAi, &assistant),
            MarkdownLinkLabelPolicy::CompactLongBareHttp
        );
        assert_eq!(
            AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::Standard, &assistant),
            MarkdownLinkLabelPolicy::Preserve
        );

        for role in [
            AgentChatThreadMessageRole::User,
            AgentChatThreadMessageRole::Thought,
            AgentChatThreadMessageRole::Tool,
            AgentChatThreadMessageRole::System,
            AgentChatThreadMessageRole::Error,
        ] {
            let msg = message(role, "unchanged");
            assert_eq!(
                AgentChatTranscript::markdown_link_policy_for(AgentChatUiVariant::QuickAi, &msg),
                MarkdownLinkLabelPolicy::Preserve
            );
        }
    }

    #[test]
    fn quick_ai_compact_links_bypass_heavy_plain_preview() {
        let body = (0..14)
            .map(|ix| {
                format!(
                    "https://news.google.com/rss/articles/very-long-redirect-{ix}-{}",
                    "x".repeat(96)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let stats = HeavyMarkdownStats::from_text(&body);
        let msg = message(AgentChatThreadMessageRole::Assistant, body);

        assert!(stats.is_scroll_heavy());
        assert!(AgentChatTranscript::should_use_heavy_markdown_preview_for(
            AgentChatUiVariant::Standard,
            &msg,
            stats
        ));
        assert!(!AgentChatTranscript::should_use_heavy_markdown_preview_for(
            AgentChatUiVariant::QuickAi,
            &msg,
            stats
        ));
    }

    // =========================================================================
    // C-R6: `ListState` is the SOLE follow-tail authority.
    //
    // The transcript no longer keeps a duplicate `Cell<bool>` that could lie
    // about (or silently re-enable) tail-following after a manual scroll. These
    // tests pin the observable contract: every follow/manual metric is read
    // from `ListState::is_following_tail()`, wheel-up and scrollbar drags
    // disable following inside `ListState` (and the transcript reports it), and
    // switching the anchor preserves whatever follow/offset state was live.
    // =========================================================================
    mod follow_tail_authority {
        use super::*;
        use gpui::{
            point, size, AppContext as _, ListOffset, Pixels, ScrollDelta, ScrollWheelEvent, Size,
            TestAppContext, VisualTestContext,
        };

        fn seeded(role: AgentChatThreadMessageRole, id: u64, body: &str) -> AgentChatThreadMessage {
            let mut msg = message(role, body.to_string());
            msg.id = id;
            msg
        }

        /// A small idle transcript: alternating user/assistant rows with unique
        /// ids so the message-view map stays 1:1.
        fn conversation(count: usize) -> Vec<AgentChatThreadMessage> {
            (0..count)
                .map(|ix| {
                    let role = if ix % 2 == 0 {
                        AgentChatThreadMessageRole::User
                    } else {
                        AgentChatThreadMessageRole::Assistant
                    };
                    seeded(role, ix as u64 + 1, &format!("Message {ix}"))
                })
                .collect()
        }

        fn build(
            cx: &mut TestAppContext,
            count: usize,
            variant: AgentChatUiVariant,
        ) -> Entity<AgentChatTranscript> {
            // `TextViewState` (built during message reconciliation) reads the
            // gpui-component `Theme` global, so install component defaults first.
            cx.update(|cx| gpui_component::init(cx));
            cx.new(|cx| {
                AgentChatTranscript::new(conversation(count), cx).with_ui_variant(variant, cx)
            })
        }

        /// A minimal `Render` view that hosts a `list()` over a shared
        /// `ListState` with fixed-height rows. Drawing the list inside a real
        /// view is required because `List::paint` reads `window.current_view()`.
        struct ListProbe {
            state: ListState,
            row_height: f32,
        }

        impl Render for ListProbe {
            fn render(
                &mut self,
                _window: &mut Window,
                _cx: &mut Context<Self>,
            ) -> impl IntoElement {
                let row_height = self.row_height;
                list(self.state.clone(), move |_ix, _window, _cx| {
                    div().h(px(row_height)).w_full().into_any()
                })
                .with_sizing_behavior(ListSizingBehavior::Auto)
                .size_full()
            }
        }

        /// Draw a `list()` backed by the transcript's SHARED `ListState` so the
        /// real vendored layout/scroll machinery runs (registering the wheel
        /// handler and setting `last_layout_bounds`) without paying the cost of
        /// rendering the markdown transcript itself. Because the `ListState` is
        /// an `Rc<RefCell<..>>`, every follow-tail transition the list makes is
        /// observed by the transcript through `is_following_tail()`.
        fn draw_shared_list(
            vcx: &mut VisualTestContext,
            state: &ListState,
            viewport: Size<Pixels>,
            row_height: f32,
        ) {
            let state = state.clone();
            vcx.draw(point(px(0.0), px(0.0)), viewport, move |_window, cx| {
                cx.new(|_| ListProbe {
                    state: state.clone(),
                    row_height,
                })
                .into_any_element()
            });
        }

        /// (1) A short Standard/Quick AI transcript is Top-anchored and rests at
        /// the top (item 0) when the content fits the viewport.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_short_top_rests_at_top(
            cx: &mut TestAppContext,
        ) {
            for variant in [AgentChatUiVariant::Standard, AgentChatUiVariant::QuickAi] {
                let entity = build(cx, 3, variant);
                assert_eq!(
                    cx.read_entity(&entity, |t, _| t.anchor),
                    AgentChatTranscriptAnchor::Top,
                    "{variant:?} must be top-anchored",
                );
                let state = cx.read_entity(&entity, |t, _| t.list_state());
                let vcx = cx.add_empty_window();
                // Content (4 rows x 20px = 80px) is far shorter than the 400px
                // viewport, so the resting position is governed by alignment.
                draw_shared_list(vcx, &state, size(px(300.0), px(400.0)), 20.0);
                assert_eq!(
                    state.logical_scroll_top().item_ix,
                    0,
                    "{variant:?}: short top-anchored transcript must rest at the top",
                );
            }
        }

        /// (2) A short bottom-docked transcript is Bottom-anchored and rests at
        /// the bottom (past the last item) when the content fits.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_short_bottom_rests_at_bottom(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 3, AgentChatUiVariant::BottomDock);
            assert_eq!(
                cx.read_entity(&entity, |t, _| t.anchor),
                AgentChatTranscriptAnchor::Bottom,
                "BottomDock must be bottom-anchored",
            );
            let row_count = cx.read_entity(&entity, |t, _| t.row_count());
            let state = cx.read_entity(&entity, |t, _| t.list_state());
            let vcx = cx.add_empty_window();
            draw_shared_list(vcx, &state, size(px(300.0), px(400.0)), 20.0);
            assert_eq!(
                state.logical_scroll_top().item_ix,
                row_count,
                "short bottom-anchored transcript must rest past the last item",
            );
        }

        /// (3) While at the latest message, a Top-anchored transcript keeps
        /// following as new streaming chunks arrive.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_top_streams_at_latest(cx: &mut TestAppContext) {
            let entity = build(cx, 2, AgentChatUiVariant::Standard);
            assert!(
                cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail,
                "fresh transcript follows"
            );

            // Append a streaming chunk (tail-only splice).
            let mut next = conversation(2);
            next.push(seeded(AgentChatThreadMessageRole::Assistant, 99, "chunk"));
            cx.update(|cx| entity.update(cx, |t, cx| t.set_messages(next, cx)));

            let m = cx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(m.follow_tail, "an at-latest stream must keep following");
            assert!(!m.manual_scroll);
            assert_eq!(
                cx.read_entity(&entity, |t, _| t.logical_scroll_top().item_ix),
                cx.read_entity(&entity, |t, _| t.row_count()),
                "following pins the logical top to the tail",
            );
        }

        /// (4) Same at-latest streaming follow, for a Bottom-anchored transcript.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_bottom_streams_at_latest(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 2, AgentChatUiVariant::BottomDock);
            assert!(
                cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail
            );

            let mut next = conversation(2);
            next.push(seeded(AgentChatThreadMessageRole::Assistant, 99, "chunk"));
            cx.update(|cx| entity.update(cx, |t, cx| t.set_messages(next, cx)));

            let m = cx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(
                m.follow_tail,
                "bottom-anchored at-latest stream keeps following"
            );
            assert!(!m.manual_scroll);
        }

        /// (5) A wheel-up gesture disables follow inside `ListState`, and the
        /// transcript reports it through the shared authority.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_wheel_up_disables_follow(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 6, AgentChatUiVariant::Standard);
            let state = cx.read_entity(&entity, |t, _| t.list_state());
            let vcx = cx.add_empty_window();
            // Tall rows overflow the viewport so a wheel gesture is meaningful.
            draw_shared_list(vcx, &state, size(px(300.0), px(200.0)), 80.0);

            assert!(
                vcx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail,
                "transcript follows before the wheel gesture",
            );

            // Positive delta.y scrolls toward the top (wheel-up); the vendored
            // list disables follow on the first upward wheel event.
            vcx.simulate_event(ScrollWheelEvent {
                position: point(px(20.0), px(20.0)),
                delta: ScrollDelta::Pixels(point(px(0.0), px(80.0))),
                ..Default::default()
            });

            let m = vcx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(!m.follow_tail, "wheel-up must disable follow-tail");
            assert!(
                m.manual_scroll,
                "manual_scroll must equal !is_following_tail"
            );
        }

        /// (6) A scrollbar drag disables follow inside `ListState`, and the
        /// transcript reports it.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_scrollbar_drag_disables_follow(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 6, AgentChatUiVariant::Standard);
            let state = cx.read_entity(&entity, |t, _| t.list_state());
            let vcx = cx.add_empty_window();
            // A draw is required so `set_offset_from_scrollbar` has layout bounds.
            draw_shared_list(vcx, &state, size(px(300.0), px(200.0)), 80.0);
            assert!(
                vcx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail
            );

            // This is the exact call the scrollbar element makes while dragging.
            state.set_offset_from_scrollbar(point(px(0.0), px(50.0)));

            let m = vcx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(!m.follow_tail, "scrollbar drag must disable follow-tail");
            assert!(m.manual_scroll);
        }

        /// (7) New chunks do NOT snap a manually-scrolled reader back to the
        /// tail, and switching the anchor (Top↔Bottom) preserves both the
        /// manual (not-following) state and the logical offset.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_manual_scroll_survives_chunks_and_anchor_switch(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 4, AgentChatUiVariant::Standard);

            // Reader scrolls up into history: follow off, parked at item 1.
            let parked = ListOffset {
                item_ix: 1,
                offset_in_item: px(0.0),
            };
            cx.read_entity(&entity, |t, _| t.scroll_to(parked));
            let m = cx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(!m.follow_tail, "manual scroll clears follow");
            assert!(m.manual_scroll);

            // A streaming chunk arrives; it must not snap the reader.
            let mut next = conversation(4);
            next.push(seeded(AgentChatThreadMessageRole::Assistant, 99, "chunk"));
            cx.update(|cx| entity.update(cx, |t, cx| t.set_messages(next, cx)));
            assert!(
                !cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail,
                "a new chunk must not re-enable follow while manually scrolled",
            );
            assert_eq!(
                cx.read_entity(&entity, |t, _| t.logical_scroll_top().item_ix),
                1,
                "the manual offset survives an appended chunk",
            );

            // Switch Top -> Bottom: follow state and offset must be preserved.
            cx.update(|cx| {
                entity.update(cx, |t, _| t.apply_anchor(AgentChatTranscriptAnchor::Bottom))
            });
            assert!(
                !cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail,
                "an anchor change must not re-enable follow after a manual scroll",
            );
            assert_eq!(
                cx.read_entity(&entity, |t, _| t.logical_scroll_top().item_ix),
                1,
                "anchor switch preserves the manual logical offset",
            );

            // And back Bottom -> Top: still preserved.
            cx.update(|cx| {
                entity.update(cx, |t, _| t.apply_anchor(AgentChatTranscriptAnchor::Top))
            });
            assert!(
                !cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail
            );
            assert_eq!(
                cx.read_entity(&entity, |t, _| t.logical_scroll_top().item_ix),
                1,
            );
        }

        /// (8) `scroll_to_end` explicitly resumes following after a manual scroll.
        #[gpui::test]
        fn agent_chat_transcript_anchor_follow_tail_scroll_to_end_resumes_following(
            cx: &mut TestAppContext,
        ) {
            let entity = build(cx, 4, AgentChatUiVariant::Standard);

            cx.read_entity(&entity, |t, _| {
                t.scroll_to(ListOffset {
                    item_ix: 1,
                    offset_in_item: px(0.0),
                })
            });
            assert!(
                !cx.read_entity(&entity, |t, _| t.scroll_metrics())
                    .follow_tail,
                "manual scroll clears follow"
            );

            cx.read_entity(&entity, |t, _| t.scroll_to_end());
            let m = cx.read_entity(&entity, |t, _| t.scroll_metrics());
            assert!(m.follow_tail, "scroll_to_end resumes following");
            assert!(!m.manual_scroll);
        }
    }
}
