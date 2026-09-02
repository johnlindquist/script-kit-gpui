pub mod action_effects;
pub mod actions;
pub mod ai;
pub mod artifacts;
pub mod capture;
pub mod capture_gate;
pub mod capture_history_picker;
pub mod capture_schema;
pub mod command;
pub mod date;
pub mod doctor;
pub mod execute;
pub mod filter;
pub mod form;
pub mod fragments;
pub mod grammar_payload;
pub mod handler_index;
pub mod has_fields;
pub mod history;
pub mod ics;
pub mod link_scriptlet;
pub mod main_hint;
pub mod metadata;
pub mod mode;
pub mod nl_anchor;
pub mod nl_duration;
pub mod nl_phrase;
pub mod nl_recurrence;
pub mod nl_time;
pub mod object_selector;
pub mod parse;
pub mod payload;
pub mod query;
pub mod quote;
pub mod retention;
pub mod schema_overrides;
pub mod skill;
pub mod snippet_scriptlet;
pub mod templates;
pub mod trigger_picker;
pub mod trigger_picker_keys;

#[allow(unused_imports)]
pub use actions::{
    current_actions as current_menu_syntax_actions, MenuSyntaxAction, MenuSyntaxActionKind,
    MenuSyntaxActionState,
};
pub(crate) use artifacts::{
    discard_root_todos_snapshot_refresh, finish_root_todos_snapshot_refresh,
    invalidate_owned_root_todos_freshness, owned_root_todos_snapshot, read_root_todos_snapshot,
    reset_owned_root_todos, root_todos_fresh_cache_status, root_todos_snapshot_is_fresh,
    try_begin_root_todos_snapshot_refresh, RootTodoRefreshSnapshot,
};
#[allow(unused_imports)]
pub use artifacts::{
    read_all_artifacts, read_jsonl_artifact, read_payload_dir, root_todo_query_is_eligible,
    search_root_object_candidates_direct, search_root_object_candidates_in_sk_path,
    search_root_todos_cached, search_root_todos_direct, search_root_todos_in_sk_path,
    CaptureArtifact, CaptureArtifactKind, ReadArtifactReport, RootTodoSearchHit,
    RootTodoSectionOptions,
};
pub use capture_gate::{decide_capture_gate_for_script, CaptureGateDecision};
#[allow(unused_imports)]
pub use capture_history_picker::{
    build_history_picker_snapshot, build_history_picker_snapshot_with_overrides,
    detect_history_picker_context, snapshot_from_filter_text,
    snapshot_from_filter_text_with_overrides, HistoryPickerKind, HistoryPickerRow,
    HistoryPickerSnapshot,
};
pub use capture_schema::builtin_schema;
pub use command::{
    command_head_matches, command_slug, script_command_head, scriptlet_command_head,
};
pub use date::MenuSyntaxClock;
#[allow(unused_imports)]
pub use doctor::{
    validate as doctor_validate, validate_at_path as doctor_validate_at_path, DoctorIssue,
    DoctorReport, DoctorSeverity,
};
pub use execute::{
    build_capture_payload, command_env, payload_env, write_payload_tempfile, MenuSyntaxHandlerKind,
    MenuSyntaxHandlerRef,
};
pub use filter::{
    apply_advanced_query, capture_accepts_for_target_from_scripts, first_command_head_for_script,
    first_concrete_capture_target_for_script, matches_predicate,
    registered_capture_targets_from_scripts, script_menu_syntax_specs,
};
#[allow(unused_imports)]
pub use form::{
    apply_capture_form_field_edit, apply_menu_syntax_form_suggestion, build_capture_form_snapshot,
    empty_capture_invocation, MenuSyntaxFormFieldKind, MenuSyntaxFormFieldSnapshot,
    MenuSyntaxFormSnapshot, MenuSyntaxFormSuggestion, MenuSyntaxFormSuggestionApplication,
    MenuSyntaxFormSuggestionPools,
};
#[allow(unused_imports)]
pub use fragments::{MenuSyntaxFragment, MenuSyntaxFragmentRole, MenuSyntaxFragmentStatus};
#[allow(unused_imports)]
pub use grammar_payload::{
    DateEntry as GrammarDateEntry, FieldEntry as GrammarFieldEntry, FieldKind as GrammarFieldKind,
    GrammarPayload, GrammarSurface, GrammarVerb, TagEntry as GrammarTagEntry,
    GRAMMAR_PAYLOAD_VERSION,
};
#[allow(unused_imports)]
pub use handler_index::{
    explain_capture_handler_ranking, rank_handlers_for_target, rank_scripts_handling_capture,
    ranked_handler_is_user_authored, CaptureHandlerRankingExplanation, CaptureHandlerRankingRow,
    HandlerScore, RankedHandler,
};
#[allow(unused_imports)]
pub use history::{
    build_argv_pool, build_tag_pool, build_value_pool, read_argv_pool, read_key_pool,
    read_tag_pool, record_argv, record_tags, ArgvFrequency, ArgvHistoryEntry, CommandHistoryStore,
    HistoryStore, TagFrequency, TagHistoryEntry, ValueFrequency, ValueHistoryEntry,
    ARGV_HISTORY_FILENAME, COMMANDS_DIR, KEYS_DIR, KEY_HISTORY_SUFFIX, TAG_HISTORY_FILENAME,
};
#[allow(unused_imports)]
pub use link_scriptlet::{
    active_link_capture_field_selector_for_input, canonical_link_metadata_key,
    link_metadata_field_specs, normalize_link_capture_invocation, parse_link_scriptlet_capture,
    LinkLookup, LinkScriptletDraft, LinkScriptletOperation,
};
#[allow(unused_imports)]
pub use main_hint::{
    active_filter_head_owns_main_list, build_menu_syntax_main_hint,
    MenuSyntaxCaptureValidationSnapshot, MenuSyntaxCaptureValidationStatus,
    MenuSyntaxFragmentPreviewRow, MenuSyntaxMainHintChip, MenuSyntaxMainHintContext,
    MenuSyntaxMainHintKind, MenuSyntaxMainHintRow, MenuSyntaxMainHintSnapshot,
    MenuSyntaxMainHintTone,
};
pub use mode::{
    free_text_for_search, input_span_role_name, input_spans_for_input_with_targets,
    list_filter_segment_is_terminal, prefix_span_for_input, MenuSyntaxMode,
};
#[allow(unused_imports)]
pub use object_selector::{
    apply_object_selector_intent, build_object_selector_snapshot,
    object_selector_candidate_matches, object_selector_row_to_main_list_row,
    object_selector_visible_start_for_selection, plan_object_selector_transition,
    MenuSyntaxObjectSelectorState, ObjectSelectorCandidate, ObjectSelectorContext,
    ObjectSelectorIntentOutcome, ObjectSelectorRow, ObjectSelectorSnapshot,
    ObjectSelectorTransition,
};
#[allow(unused_imports)]
pub use parse::{parse, parse_with_capture_targets, MenuSyntaxParse};
#[allow(unused_imports)]
pub use payload::{
    object_refs_for_raw_capture, source_for_head, ActiveObjectSelector, AdvancedQuery,
    ArgvInvocation, CanonicalCaptureTarget, CaptureInvocation, CaptureObjectKind, CaptureObjectRef,
    CaptureOperation, CaptureTargetResolution, FilterIndicator, FilterIndicatorTone,
    MenuSyntaxHandlerSpec, RootUnifiedSourceFilter, RootUnifiedSourceFilterSet, SourceHeadSpec,
    SOURCE_HEAD_SPECS,
};
#[allow(unused_imports)]
pub use query::parse_advanced_query;
pub use quote::quote_for_filter_value;
#[allow(unused_imports)]
pub use retention::{
    apply_retention_plan, plan_retention, AppliedRetention, PayloadListing, RetentionConfig,
    RetentionPlan, AGE_CUTOFF_DAYS_DEFAULT, HARD_CAP_DEFAULT, KEEP_NEWEST_DEFAULT,
};
#[allow(unused_imports)]
pub use schema_overrides::{
    capture_kv_enum_values_for_specs, merge_enum_with_history, RankedSlotValue, SlotValueSource,
};
#[allow(unused_imports)]
pub use skill::{skill_specs_from_value, SkillSpec};
#[allow(unused_imports)]
pub use snippet_scriptlet::{
    active_capture_field_selector_for_input, normalize_snippet_capture_invocation,
    parse_snippet_scriptlet_capture, snippet_metadata_field_specs, ActiveCaptureFieldSelector,
    SnippetLookup, SnippetMetadataFieldSpec, SnippetMetadataValueKind, SnippetScriptletDraft,
    SnippetScriptletOperation,
};
#[allow(unused_imports)]
pub use trigger_picker::{
    build_trigger_picker_snapshot, create_capture_handler_scaffold, CaptureHandlerScaffoldEffects,
    TriggerPickerAction, TriggerPickerContext, TriggerPickerMode, TriggerPickerRow,
    TriggerPickerRowKind, TriggerPickerSnapshot,
};
#[allow(unused_imports)]
pub use trigger_picker_keys::{
    apply_intent, first_selectable_index, last_selectable_index, next_selectable_index,
    prev_selectable_index, InlinePickerKeyIntent, TriggerPickerIntentOutcome,
};
