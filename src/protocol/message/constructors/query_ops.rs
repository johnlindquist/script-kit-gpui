use super::*;

impl Message {
    // ============================================================
    // Constructor methods for file search
    // ============================================================

    /// Create a file search request
    pub fn file_search(request_id: String, query: String, only_in: Option<String>) -> Self {
        Message::FileSearch {
            request_id,
            query,
            only_in,
        }
    }

    /// Create a file search result response
    pub fn file_search_result(request_id: String, files: Vec<FileSearchResultEntry>) -> Self {
        Message::FileSearchResult { request_id, files }
    }

    // ============================================================
    // Constructor methods for screenshot capture
    // ============================================================

    /// Create a capture screenshot request
    pub fn capture_screenshot(request_id: String) -> Self {
        Message::CaptureScreenshot {
            request_id,
            hi_dpi: None,
            target: None,
        }
    }

    /// Create a capture screenshot request with hi_dpi option
    pub fn capture_screenshot_with_options(request_id: String, hi_dpi: Option<bool>) -> Self {
        Message::CaptureScreenshot {
            request_id,
            hi_dpi,
            target: None,
        }
    }

    /// Create a screenshot result response
    pub fn screenshot_result(request_id: String, data: String, width: u32, height: u32) -> Self {
        Message::ScreenshotResult {
            request_id,
            data,
            width,
            height,
            error: None,
        }
    }

    /// Create a screenshot error result response
    pub fn screenshot_error(request_id: String, error: String) -> Self {
        Message::ScreenshotResult {
            request_id,
            data: String::new(),
            width: 0,
            height: 0,
            error: Some(error),
        }
    }

    // ============================================================
    // Constructor methods for state query
    // ============================================================

    /// Create a get state request
    pub fn get_state(request_id: String) -> Self {
        Message::GetState {
            request_id,
            target: None,
            summary_only: false,
        }
    }

    /// Create a state result response
    #[allow(clippy::too_many_arguments)]
    pub fn state_result(
        request_id: String,
        prompt_type: String,
        prompt_id: Option<String>,
        surface_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>,
        active_popup_contract: Option<crate::protocol::LauncherSurfaceContractSnapshot>,
        active_footer: Option<crate::protocol::ActiveFooterSnapshot>,
        submit_diagnostics: Option<serde_json::Value>,
        placeholder: Option<String>,
        input_value: String,
        choice_count: usize,
        visible_choice_count: usize,
        selected_index: i32,
        selected_value: Option<String>,
        is_focused: bool,
        window_visible: bool,
        mini_ai: Option<serde_json::Value>,
        focused_text_agent_chat: Option<serde_json::Value>,
        filter_input_decorations: Option<serde_json::Value>,
        filter_input_diagnostics: Option<serde_json::Value>,
        menu_syntax_main_hint: Option<crate::menu_syntax::MenuSyntaxMainHintSnapshot>,
        capture_history_picker: Option<
            crate::menu_syntax::capture_history_picker::HistoryPickerSnapshot,
        >,
        main_window_preflight: Option<serde_json::Value>,
        actions_dialog: Option<serde_json::Value>,
        root_file_search: Option<serde_json::Value>,
        main_list_scroll: Option<serde_json::Value>,
        active_list_scroll: Option<serde_json::Value>,
        screenshot_identity: Option<String>,
        drop_state: Option<serde_json::Value>,
        path_state: Option<serde_json::Value>,
        notes_state: Option<serde_json::Value>,
        day_page_state: Option<serde_json::Value>,
        dictation_state: Option<serde_json::Value>,
        ghost_prediction: Option<serde_json::Value>,
        flow_ux: Option<serde_json::Value>,
        backgrounded_sessions: Option<serde_json::Value>,
    ) -> Self {
        Message::StateResult {
            request_id,
            prompt_type,
            prompt_id,
            surface_contract,
            active_popup_contract,
            active_footer,
            submit_diagnostics,
            placeholder,
            input_value,
            choice_count,
            visible_choice_count,
            selected_index,
            selected_value,
            is_focused,
            window_visible,
            mini_ai,
            focused_text_agent_chat,
            filter_input_decorations,
            filter_input_diagnostics,
            menu_syntax_main_hint,
            capture_history_picker,
            main_window_preflight,
            actions_dialog,
            root_file_search,
            main_list_scroll,
            active_list_scroll,
            screenshot_identity,
            drop_state,
            path_state,
            notes_state,
            day_page_state,
            dictation_state,
            ghost_prediction,
            flow_ux,
            backgrounded_sessions,
        }
    }

    // ============================================================
    // Constructor methods for element query
    // ============================================================

    /// Create a get elements request
    pub fn get_elements(request_id: String) -> Self {
        Message::GetElements {
            request_id,
            limit: None,
            target: None,
            include_headers: None,
        }
    }

    /// Create a get elements request with limit
    pub fn get_elements_with_limit(request_id: String, limit: usize) -> Self {
        Message::GetElements {
            request_id,
            limit: Some(limit),
            target: None,
            include_headers: None,
        }
    }

    /// Create a legacy elements result. It is deliberately marked unsupported:
    /// callers that can prove a projection must use `elements_result_with_projection`.
    pub fn elements_result(
        request_id: String,
        elements: Vec<ElementInfo>,
        total_count: usize,
        focused_semantic_id: Option<String>,
        selected_semantic_id: Option<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self::elements_result_with_projection(
            request_id,
            "legacyUnknown".to_string(),
            1,
            ProjectionQuality::Unsupported,
            vec![ProjectionReason::CollectorUnavailable],
            elements,
            total_count,
            focused_semantic_id,
            selected_semantic_id,
            warnings,
        )
    }

    /// Create an elements result response with observation receipt metadata.
    #[expect(
        clippy::too_many_arguments,
        reason = "the public wire-compatible constructor preserves each stable projection-envelope field"
    )]
    pub fn elements_result_with_projection(
        request_id: String,
        semantic_surface: String,
        projection_version: u32,
        projection_quality: ProjectionQuality,
        reason_codes: Vec<ProjectionReason>,
        elements: Vec<ElementInfo>,
        total_count: usize,
        focused_semantic_id: Option<String>,
        selected_semantic_id: Option<String>,
        warnings: Vec<String>,
    ) -> Self {
        let truncated = elements.len() < total_count;
        Message::ElementsResult {
            request_id,
            semantic_surface,
            projection_version,
            projection_quality,
            reason_codes,
            elements,
            total_count,
            truncated,
            focused_semantic_id,
            selected_semantic_id,
            warnings,
        }
    }

    // ============================================================
    // Constructor methods for layout info
    // ============================================================

    /// Create a get layout info request
    pub fn get_layout_info(request_id: String) -> Self {
        Message::GetLayoutInfo {
            request_id,
            target: None,
        }
    }

    /// Create a get layout info request targeting a specific window
    pub fn get_layout_info_targeted(request_id: String, target: AutomationWindowTarget) -> Self {
        Message::GetLayoutInfo {
            request_id,
            target: Some(target),
        }
    }

    /// Create a layout info result response
    pub fn layout_info_result(request_id: String, info: LayoutInfo) -> Self {
        Message::LayoutInfoResult { request_id, info }
    }

    // ============================================================
    // Constructor methods for error reporting
    // ============================================================

    /// Create a script error message from ScriptErrorData
    pub fn set_error(error_data: ScriptErrorData) -> Self {
        Message::SetError {
            error_message: error_data.error_message,
            stderr_output: error_data.stderr_output,
            exit_code: error_data.exit_code,
            stack_trace: error_data.stack_trace,
            script_path: error_data.script_path,
            suggestions: error_data.suggestions,
            timestamp: error_data.timestamp,
        }
    }

    /// Create a simple script error message with just the message and path
    pub fn script_error(error_message: String, script_path: String) -> Self {
        Message::SetError {
            error_message,
            stderr_output: None,
            exit_code: None,
            stack_trace: None,
            script_path,
            suggestions: Vec::new(),
            timestamp: None,
        }
    }

    /// Create a full script error message with all optional fields
    pub fn script_error_full(
        error_message: String,
        script_path: String,
        stderr_output: Option<String>,
        exit_code: Option<i32>,
        stack_trace: Option<String>,
        suggestions: Vec<String>,
        timestamp: Option<String>,
    ) -> Self {
        Message::SetError {
            error_message,
            stderr_output,
            exit_code,
            stack_trace,
            script_path,
            suggestions,
            timestamp,
        }
    }

    // ============================================================
    // Constructor methods for scriptlet operations
    // ============================================================

    /// Create a run scriptlet request
    pub fn run_scriptlet(
        request_id: String,
        scriptlet: ScriptletData,
        inputs: Option<serde_json::Value>,
        args: Vec<String>,
    ) -> Self {
        Message::RunScriptlet {
            request_id,
            scriptlet,
            inputs,
            args,
        }
    }

    /// Create a get scriptlets request
    pub fn get_scriptlets(request_id: String) -> Self {
        Message::GetScriptlets {
            request_id,
            kit: None,
            group: None,
        }
    }

    /// Create a get scriptlets request with filters
    pub fn get_scriptlets_filtered(
        request_id: String,
        kit: Option<String>,
        group: Option<String>,
    ) -> Self {
        Message::GetScriptlets {
            request_id,
            kit,
            group,
        }
    }

    /// Create a scriptlet list response
    pub fn scriptlet_list(request_id: String, scriptlets: Vec<ScriptletData>) -> Self {
        Message::ScriptletList {
            request_id,
            scriptlets,
        }
    }

    /// Create a successful scriptlet result
    pub fn scriptlet_result_success(
        request_id: String,
        output: Option<String>,
        exit_code: Option<i32>,
    ) -> Self {
        Message::ScriptletResult {
            request_id,
            success: true,
            output,
            error: None,
            exit_code,
        }
    }

    /// Create a failed scriptlet result
    pub fn scriptlet_result_error(
        request_id: String,
        error: String,
        exit_code: Option<i32>,
    ) -> Self {
        Message::ScriptletResult {
            request_id,
            success: false,
            output: None,
            error: Some(error),
            exit_code,
        }
    }

    // ============================================================
    // Constructor methods for Agent Chat state query
    // ============================================================

    /// Create a getAgentChatState request
    pub fn get_agent_chat_state(request_id: String) -> Self {
        Message::GetAgentChatState {
            request_id,
            target: None,
        }
    }

    /// Create an Agent Chat state result response
    pub fn agent_chat_state_result(request_id: String, state: AgentChatStateSnapshot) -> Self {
        Message::AgentChatStateResult { request_id, state }
    }

    // ============================================================
    // Constructor methods for Agent Chat test probe
    // ============================================================

    /// Create a resetAgentChatTestProbe request
    pub fn reset_agent_chat_test_probe(request_id: String) -> Self {
        Message::ResetAgentChatTestProbe {
            request_id,
            target: None,
        }
    }

    /// Create a resetAgentChatTestProbe request targeting a specific Agent Chat surface
    pub fn reset_agent_chat_test_probe_targeted(
        request_id: String,
        target: AutomationWindowTarget,
    ) -> Self {
        Message::ResetAgentChatTestProbe {
            request_id,
            target: Some(target),
        }
    }

    /// Create a getAgentChatTestProbe request
    pub fn get_agent_chat_test_probe(request_id: String, tail: Option<usize>) -> Self {
        Message::GetAgentChatTestProbe {
            request_id,
            tail,
            target: None,
        }
    }

    /// Create an Agent Chat test probe result response
    pub fn agent_chat_test_probe_result(
        request_id: String,
        probe: AgentChatTestProbeSnapshot,
    ) -> Self {
        Message::AgentChatTestProbeResult { request_id, probe }
    }

    // ============================================================
    // Constructor methods for Agent Chat setup actions
    // ============================================================

    /// Create a performAgentChatSetupAction request
    pub fn perform_agent_chat_setup_action(
        request_id: String,
        action: AgentChatSetupActionKind,
        agent_id: Option<String>,
    ) -> Self {
        Message::PerformAgentChatSetupAction {
            request_id,
            action,
            agent_id,
            target: None,
        }
    }

    /// Create a successful Agent Chat setup action result
    pub fn agent_chat_setup_action_result_success(
        request_id: String,
        state: AgentChatStateSnapshot,
    ) -> Self {
        Message::AgentChatSetupActionResult {
            request_id,
            success: true,
            error: None,
            state: Some(state),
        }
    }

    /// Create a failed Agent Chat setup action result
    pub fn agent_chat_setup_action_result_error(request_id: String, error: String) -> Self {
        Message::AgentChatSetupActionResult {
            request_id,
            success: false,
            error: Some(error),
            state: None,
        }
    }

    // ============================================================
    // Constructor methods for wait/batch transaction layer
    // ============================================================

    /// Create a waitFor request
    pub fn wait_for(
        request_id: String,
        condition: WaitCondition,
        timeout: Option<u64>,
        poll_interval: Option<u64>,
    ) -> Self {
        Message::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            trace: TransactionTraceMode::Off,
            target: None,
        }
    }

    /// Create a waitFor request with trace mode
    pub fn wait_for_with_trace(
        request_id: String,
        condition: WaitCondition,
        timeout: Option<u64>,
        poll_interval: Option<u64>,
        trace: TransactionTraceMode,
    ) -> Self {
        Message::WaitFor {
            request_id,
            condition,
            timeout,
            poll_interval,
            trace,
            target: None,
        }
    }

    /// Create a waitFor result
    pub fn wait_for_result(
        request_id: String,
        success: bool,
        elapsed: u64,
        error: Option<TransactionError>,
    ) -> Self {
        Message::WaitForResult {
            request_id,
            success,
            elapsed,
            error,
            trace: None,
        }
    }

    /// Create a waitFor result with embedded trace receipt
    pub fn wait_for_result_with_trace(
        request_id: String,
        success: bool,
        elapsed: u64,
        error: Option<TransactionError>,
        trace: Option<TransactionTrace>,
    ) -> Self {
        Message::WaitForResult {
            request_id,
            success,
            elapsed,
            error,
            trace,
        }
    }

    /// Create a batch request
    pub fn batch(
        request_id: String,
        commands: Vec<BatchCommand>,
        options: Option<BatchOptions>,
    ) -> Self {
        Message::Batch {
            request_id,
            commands,
            options,
            trace: TransactionTraceMode::Off,
            target: None,
        }
    }

    /// Create a batch request with trace mode
    pub fn batch_with_trace(
        request_id: String,
        commands: Vec<BatchCommand>,
        options: Option<BatchOptions>,
        trace: TransactionTraceMode,
    ) -> Self {
        Message::Batch {
            request_id,
            commands,
            options,
            trace,
            target: None,
        }
    }

    /// Create a batch result
    pub fn batch_result(
        request_id: String,
        success: bool,
        results: Vec<BatchResultEntry>,
        failed_at: Option<usize>,
        total_elapsed: u64,
    ) -> Self {
        Message::BatchResult {
            request_id,
            success,
            results,
            failed_at,
            total_elapsed,
            trace: None,
        }
    }

    // ============================================================
    // Constructor methods for automation window targeting
    // ============================================================

    /// Create a listAutomationWindows request
    pub fn list_automation_windows(request_id: String) -> Self {
        Message::ListAutomationWindows { request_id }
    }

    /// Create an automation window list result response
    pub fn automation_window_list_result(
        request_id: String,
        windows: Vec<AutomationWindowInfo>,
        focused_window_id: Option<String>,
    ) -> Self {
        Message::AutomationWindowListResult {
            request_id,
            windows,
            focused_window_id,
        }
    }

    // ============================================================
    // Constructor methods for GPUI event simulation
    // ============================================================

    /// Create a simulateGpuiEvent request
    pub fn simulate_gpui_event(
        request_id: String,
        event: SimulatedGpuiEvent,
        target: Option<AutomationWindowTarget>,
    ) -> Self {
        Message::SimulateGpuiEvent {
            request_id,
            target,
            event,
        }
    }

    /// Create a simulateGpuiEvent success result with dispatch metadata.
    pub fn simulate_gpui_event_result_success(
        request_id: String,
        dispatch_path: Option<String>,
        resolved_window_id: Option<String>,
        dispatch_completed: bool,
        dispatch_scheduled: bool,
        activation_proof: Option<String>,
    ) -> Self {
        Message::SimulateGpuiEventResult {
            request_id,
            success: true,
            error_code: None,
            error: None,
            dispatch_path,
            resolved_window_id,
            dispatch_completed,
            dispatch_scheduled,
            activation_proof,
        }
    }

    /// Create a simulateGpuiEvent error result with a machine-readable error code.
    ///
    /// Error codes: `target_not_found`, `target_ambiguous`, `handle_unavailable`, `dispatch_failed`.
    pub fn simulate_gpui_event_result_error(
        request_id: String,
        error_code: String,
        error: String,
        dispatch_path: Option<String>,
        resolved_window_id: Option<String>,
    ) -> Self {
        Message::SimulateGpuiEventResult {
            request_id,
            success: false,
            error_code: Some(error_code),
            error: Some(error),
            dispatch_path,
            resolved_window_id,
            dispatch_completed: false,
            dispatch_scheduled: false,
            activation_proof: None,
        }
    }

    /// Create a batch result with embedded trace receipt
    pub fn batch_result_with_trace(
        request_id: String,
        success: bool,
        results: Vec<BatchResultEntry>,
        failed_at: Option<usize>,
        total_elapsed: u64,
        trace: Option<TransactionTrace>,
    ) -> Self {
        Message::BatchResult {
            request_id,
            success,
            results,
            failed_at,
            total_elapsed,
            trace,
        }
    }

    // ============================================================
    // Constructor methods for exact-window inspection
    // ============================================================

    /// Create an inspectAutomationWindow request
    pub fn inspect_automation_window(
        request_id: String,
        target: Option<AutomationWindowTarget>,
        hi_dpi: Option<bool>,
        probes: Vec<crate::protocol::types::automation_inspect::PixelProbe>,
    ) -> Self {
        Message::InspectAutomationWindow {
            request_id,
            target,
            hi_dpi,
            probes,
        }
    }

    /// Create an automationInspectResult response
    pub fn automation_inspect_result(
        request_id: String,
        snapshot: crate::protocol::types::automation_inspect::AutomationInspectSnapshot,
    ) -> Self {
        Message::AutomationInspectResult {
            request_id,
            snapshot,
        }
    }
}
