#[cfg(test)]
mod dismiss_contract_tests {
    use super::*;

    /// One Escape owner per prompt kind, declared in the DismissPolicy table.
    ///
    /// Cancel-to-script prompts (select/path/drop/template + env/naming) call
    /// `submit_cancel()` from their entities so the script receives `None`;
    /// the launcher shell must NOT also close on Escape — two owners made
    /// Escape a race between "window gone" and "script got None". Editor and
    /// terminal child content likewise own Escape locally (guarded cancel /
    /// PTY pass-through).
    #[test]
    fn escape_ownership_is_declared_once_per_prompt_kind() {
        // Shell-owned: Escape closes the launcher window.
        for kind in [SurfaceKind::PromptEntity, SurfaceKind::Webcam] {
            assert!(
                kind.surface_contract()
                    .dismiss_policy
                    .closes_main_window_on(DismissTrigger::Escape),
                "{kind:?}: Escape should close the main window"
            );
        }
        // Entity-owned: the shell must defer on Escape.
        for kind in [
            SurfaceKind::PromptEntityCancelsToScript,
            SurfaceKind::ExplicitPromptEntity,
            SurfaceKind::PromptChildContent,
        ] {
            assert!(
                !kind
                    .surface_contract()
                    .dismiss_policy
                    .closes_main_window_on(DismissTrigger::Escape),
                "{kind:?}: Escape belongs to the prompt entity, not the shell"
            );
        }
        // Cancel-to-script prompts still blur-close like standard surfaces.
        assert!(SurfaceKind::PromptEntityCancelsToScript
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::WindowBlur));
        // Webcam: sticky on blur (live capture), but Escape/Cmd+W close.
        assert!(!SurfaceKind::Webcam
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::WindowBlur));
        assert!(SurfaceKind::Webcam
            .surface_contract()
            .dismiss_policy
            .closes_main_window_on(DismissTrigger::CmdW));
    }

    /// Flow sessions background instead of dying: clicking away (blur) hides
    /// the launcher while the turn keeps running, Escape stays view-owned
    /// (backgrounds to the desk, never kills the session), and Cmd+W closes
    /// the window. Pairs with the runtime probe proving `flow_sessions`
    /// survive `close_and_reset_window`.
    #[test]
    fn flow_session_backgrounds_on_blur_and_owns_escape() {
        let policy = SurfaceKind::FlowSession.surface_contract().dismiss_policy;
        assert!(
            policy.closes_main_window_on(DismissTrigger::WindowBlur),
            "unfocusing a flow must hide the launcher (session keeps running)"
        );
        assert!(
            !policy.closes_main_window_on(DismissTrigger::Escape),
            "Escape is view-owned: back to the desk, never window-close"
        );
        assert!(policy.closes_main_window_on(DismissTrigger::CmdW));
        assert_eq!(
            AppView::FlowSessionView { session_id: 1 }.surface_kind(),
            SurfaceKind::FlowSession,
            "the flow session view must map to its dedicated surface kind"
        );
    }

    /// The cancellable prompt views must map to the cancel-to-script surface
    /// kind — adding a new cancellable prompt without reclassifying it here
    /// silently reintroduces the double-Escape-owner race.
    #[test]
    fn cancellable_prompt_views_use_cancel_to_script_kind() {
        // Compile-time companion: surface_kind() is an exhaustive match, so
        // new AppView variants already force a classification decision. This
        // test locks the four known cancellable prompts to the right kind via
        // the variant list in surface_kind() — see SelectPrompt/PathPrompt/
        // DropPrompt/TemplatePrompt arm.
        let source = include_str!("app_view_state.rs");
        let arm_start = source
            .find("=> SurfaceKind::PromptEntityCancelsToScript")
            .expect("cancel-to-script arm should exist");
        let preceding = &source[arm_start.saturating_sub(400)..arm_start];
        for variant in [
            "AppView::SelectPrompt",
            "AppView::PathPrompt",
            "AppView::DropPrompt",
            "AppView::TemplatePrompt",
        ] {
            assert!(
                preceding.contains(variant),
                "{variant} must classify as PromptEntityCancelsToScript (entity owns Escape)"
            );
        }
    }
}
