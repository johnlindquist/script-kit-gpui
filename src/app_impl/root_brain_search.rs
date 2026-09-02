//! Query-owned Brain reads. Lexical IO runs once before first paint; the bounded
//! semantic worker keeps the native debounce and embedding-availability policy.
//! Grouping consumes only accepted, query-stamped source snapshots.

use super::*;
use crate::design_evaluation::search_fixtures as F;

const ROOT_BRAIN_SEMANTIC_DEBOUNCE_MS: u64 = 60;

impl ScriptListApp {
    fn root_brain_search_plan_for_query(
        &self,
        value: &str,
    ) -> (
        crate::brain::RootBrainQueryPlan,
        crate::brain::RootBrainSectionOptions,
    ) {
        let search_text = crate::menu_syntax::free_text_for_search(&self.menu_syntax_mode, value);
        let trimmed = search_text.trim();
        let search_needle = super::filtering_cache::root_passive_search_needle(trimmed);
        let advanced = self.menu_syntax_mode.advanced_query_for(value);
        let empty_filters = crate::menu_syntax::RootUnifiedSourceFilterSet::default();
        let source_filters = advanced
            .map(|query| &query.source_filters)
            .unwrap_or(&empty_filters);
        let explicit = source_filters.includes(crate::menu_syntax::RootUnifiedSourceFilter::Brain);
        let unified_search = self.config.get_unified_search();
        let mut options = unified_search.brain_section_options();
        if explicit {
            options.enabled = true;
            options.min_query_chars = 0;
            options.max_results = options
                .max_results
                .max(unified_search.passive_result_limits().max_total_results);
        }
        let can_collect = matches!(self.current_view, AppView::ScriptList)
            && !self.menu_syntax_object_selector_state.owns_main_list()
            && !self.menu_syntax_trigger_picker_state.owns_main_list()
            && !self
                .menu_syntax_mode
                .capture_composer_owns_input_for(trimmed)
            && !self.menu_syntax_mode.command_owns_input_for(trimmed);
        let plan = if can_collect && options.max_results > 0 {
            crate::brain::root_brain_query_plan(
                search_needle,
                explicit,
                advanced.is_some_and(|query| query.has_predicates()),
                source_filters.allows(crate::menu_syntax::RootUnifiedSourceFilter::Brain),
                options,
            )
        } else {
            crate::brain::RootBrainQueryPlan::Skip
        };
        (plan, options)
    }

    /// `false` joins the caller's query publication; `true` publishes a source
    /// change atomically against the already committed interaction snapshot.
    pub(crate) fn refresh_root_brain_lexical_for_query(
        &mut self,
        value: &str,
        allow_reorder: bool,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || value != self.computed_filter_text.as_str()
            || self.root_search.root_brain_lexical_request_is_current()
        {
            return;
        }
        let (plan, options) = self.root_brain_search_plan_for_query(value);
        let work_query = match &plan {
            crate::brain::RootBrainQueryPlan::Skip => return,
            crate::brain::RootBrainQueryPlan::RecentsOnly => "",
            crate::brain::RootBrainQueryPlan::Search(query) => query.as_str(),
        };
        let generation = self
            .root_search
            .allocate_named_provider_generation("brain-lexical");
        let owned_gate = self.main_services.search_gate();
        let owned_run = if let Some(gate) = &owned_gate {
            let Some(run) = gate.begin(
                "brain-lexical",
                work_query,
                generation,
                RootProviderPublicationPolicy::VisibleSynchronous,
            ) else {
                return;
            };
            Some(run)
        } else {
            None
        };
        let result = if let Some(run) = &owned_run {
            let Ok(result) =
                run.read_synchronously(|outcome, run| F::brain_result(outcome, run, false))
            else {
                // A refused control has no native IO outcome or publication policy.
                return;
            };
            result
        } else if let Some(sources) = self.main_services.owned_sources() {
            Ok(sources.brain_hits.clone())
        } else {
            match &plan {
                crate::brain::RootBrainQueryPlan::RecentsOnly => {
                    crate::brain::recent_root_brain_hits(options.max_results)
                }
                crate::brain::RootBrainQueryPlan::Search(query) => {
                    crate::brain::search_root_brain_direct(query, &options)
                }
                crate::brain::RootBrainQueryPlan::Skip => {
                    unreachable!("ineligible read returned before admission")
                }
            }
        };
        self.root_search.begin_named_provider(
            "brain-lexical",
            generation,
            work_query,
            "",
            RootProviderPublicationPolicy::VisibleSynchronous,
            true,
        );
        let terminal = F::ProviderTerminal::for_result(&result);
        let apply = |app: &mut Self, _cx: &mut Context<Self>| {
            let changed = app.root_search.install_root_brain_lexical_results(
                generation,
                result,
                options.max_results,
            );
            app.root_search
                .finish_named_provider("brain-lexical", generation, terminal.into());
            if let Some(run) = &owned_run {
                run.finish(terminal, RootProviderPublicationPolicy::VisibleSynchronous);
            }
            if changed {
                app.invalidate_root_passive_and_grouped_cache();
            }
            changed
        };
        if allow_reorder {
            self.commit_main_menu_results_refresh(
                "brain_lexical_refresh_complete",
                Some(("brain-lexical", generation)),
                cx,
                apply,
            );
        } else {
            apply(self, cx);
        }
    }

    pub(crate) fn maybe_start_root_brain_semantic_search(
        &mut self,
        value: &str,
        cx: &mut Context<Self>,
    ) {
        if !matches!(self.current_view, AppView::ScriptList)
            || !self.root_search.query_is_current()
            || value != self.computed_filter_text.as_str()
        {
            return;
        }
        let (plan, options) = self.root_brain_search_plan_for_query(value);
        let crate::brain::RootBrainQueryPlan::Search(query) = plan else {
            self.root_search.invalidate_root_brain_semantic_freshness();
            return;
        };
        if self
            .root_search
            .root_brain_semantic_request_matches(&query, options)
        {
            return;
        }
        self.root_search.note_desired_provider(
            "brain-semantic",
            value,
            "",
            RootProviderPublicationPolicy::Visible,
        );
        let Some(generation) = self
            .root_search
            .begin_root_brain_semantic_request(query.clone(), options)
        else {
            return;
        };
        let services = self.main_services.clone();
        let owned_gate = services.search_gate();
        cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(
                    ROOT_BRAIN_SEMANTIC_DEBOUNCE_MS,
                ))
                .await;
            let current = this
                .update(cx, |app, cx| {
                    if matches!(app.current_view, AppView::ScriptList)
                        && app
                            .root_search
                            .root_brain_semantic_generation_matches(generation)
                    {
                        return true;
                    }
                    let released = app.root_search.finish_root_brain_semantic_request(
                        generation,
                        RootProviderTerminal::Cancelled,
                    );
                    if released
                        && app
                            .root_search
                            .take_named_provider_desired("brain-semantic")
                    {
                        let value = app.computed_filter_text.clone();
                        app.maybe_start_root_brain_semantic_search(&value, cx);
                    }
                    false
                })
                .unwrap_or(false);
            if !current {
                return;
            }

            // Admit at production's IO boundary, after the real debounce fence.
            let owned_run = if let Some(gate) = &owned_gate {
                let Some(run) = gate.begin(
                    "brain-semantic",
                    &query,
                    generation,
                    RootProviderPublicationPolicy::Visible,
                ) else {
                    let _ = this.update(cx, |app, _cx| {
                        app.root_search.finish_root_brain_semantic_request(
                            generation,
                            RootProviderTerminal::Cancelled,
                        );
                    });
                    return;
                };
                Some(run)
            } else {
                None
            };
            let (tx, rx) = async_channel::bounded(1);
            if let Some(run) = &owned_run {
                run.deliver(
                    move |result| tx.try_send(result),
                    |outcome, run| F::brain_result(outcome, run, true).map(Some),
                )
                .await;
            } else if let Some(sources) = services.owned_sources() {
                cx.background_executor().timer(sources.file_delay).await;
                let _ = tx.try_send(Ok(Some(sources.brain_hits.clone())));
            } else {
                let worker_query = query.clone();
                std::thread::spawn(move || {
                    let _ = tx.send_blocking(crate::brain::search_root_brain_semantic(
                        &worker_query,
                        &options,
                    ));
                });
            }
            let result = rx.recv().await;
            let updated = this.update(cx, |app, cx| {
                app.apply_root_brain_semantic_completion(
                    generation,
                    query,
                    result,
                    owned_run.as_ref(),
                    cx,
                );
            });
            if updated.is_err() {
                if let Some(run) = &owned_run {
                    run.finish(
                        F::ProviderTerminal::StaleDiscarded,
                        RootProviderPublicationPolicy::Visible,
                    );
                }
            }
        })
        .detach();
    }

    fn apply_root_brain_semantic_completion(
        &mut self,
        generation: u64,
        query: String,
        result: std::result::Result<
            anyhow::Result<Option<Vec<crate::brain::RootBrainSearchHit>>>,
            async_channel::RecvError,
        >,
        owned_run: Option<&F::SearchRun>,
        cx: &mut Context<Self>,
    ) {
        let released = if !matches!(self.current_view, AppView::ScriptList)
            || !self
                .root_search
                .root_brain_semantic_generation_matches(generation)
        {
            let released = self.root_search.finish_root_brain_semantic_request(
                generation,
                RootProviderTerminal::StaleDiscarded,
            );
            if let Some(run) = owned_run {
                run.finish(
                    F::ProviderTerminal::StaleDiscarded,
                    RootProviderPublicationPolicy::Visible,
                );
            }
            released
        } else {
            let terminal = match &result {
                Ok(Ok(Some(hits))) => F::ProviderTerminal::Completed { count: hits.len() },
                Ok(Ok(None)) => F::ProviderTerminal::Unavailable,
                Ok(Err(error)) => F::ProviderTerminal::for_error(error),
                Err(_) => F::ProviderTerminal::Disconnected,
            };
            let mut released = false;
            self.commit_main_menu_results_refresh(
                "brain_semantic_refresh_complete",
                Some(("brain-semantic", generation)),
                cx,
                |app, _cx| {
                    let changed = match result {
                        Ok(Ok(Some(hits))) => app
                            .root_search
                            .install_root_brain_semantic_results(generation, query, hits),
                        _ => false,
                    };
                    released = app
                        .root_search
                        .finish_root_brain_semantic_request(generation, terminal.into());
                    if let Some(run) = owned_run {
                        run.finish(terminal, RootProviderPublicationPolicy::Visible);
                    }
                    if changed {
                        app.invalidate_root_passive_and_grouped_cache();
                    }
                    changed
                },
            );
            released
        };
        if released
            && self
                .root_search
                .take_named_provider_desired("brain-semantic")
        {
            let value = self.computed_filter_text.clone();
            self.maybe_start_root_brain_semantic_search(&value, cx);
        }
    }
}
