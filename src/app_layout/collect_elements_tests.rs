#[cfg(test)]
mod info_state_semantic_tests {
    use super::*;

    #[test]
    fn info_state_elements_project_cue_kind_without_action_fiction() {
        let snapshot = crate::components::launcher_empty_or_no_results_spec("#work", false)
            .semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        let root = elements
            .iter()
            .find(|element| element.role.as_deref() == Some("info-state"))
            .expect("InfoState root");
        assert_eq!(root.kind.as_deref(), Some("help"));
        assert_eq!(root.status_kind.as_deref(), Some("help"));

        let syntax: Vec<_> = elements
            .iter()
            .filter(|element| element.role.as_deref() == Some("guidance-cue"))
            .collect();
        assert_eq!(syntax.len(), 3);
        assert!(syntax
            .iter()
            .all(|element| element.kind.as_deref() == Some("syntax")));
        assert!(syntax
            .iter()
            .all(|element| element.selectable == Some(false)));
        assert!(syntax
            .iter()
            .all(|element| element.action_disabled.is_none()));
        assert!(syntax
            .iter()
            .any(|element| element.text.as_deref() == Some(";todo")));
    }

    #[test]
    fn empty_agent_chat_elements_expose_trigger_and_shortcut_kinds() {
        let snapshot = crate::components::agent_chat_empty_guidance_spec().semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        assert!(elements.iter().any(|element| {
            element.kind.as_deref() == Some("trigger")
                && element.text.as_deref() == Some("/")
                && element.value.is_none()
        }));
        assert!(elements.iter().any(|element| {
            element.kind.as_deref() == Some("shortcut")
                && element.text.as_deref() == Some("⌘K")
                && element.value.as_deref() == Some("cmd+k")
                && element.semantic_id == "info-cue:agent-chat-open-actions"
        }));
    }

    #[test]
    fn simple_builtin_empty_elements_expose_info_state_owner_and_icon() {
        let snapshot = crate::components::simple_empty_state_spec(
            "favorites-empty",
            "No favorites yet",
            "star",
            None,
        )
        .semantic_snapshot();
        let elements = ScriptListApp::info_state_elements(&snapshot);
        assert_eq!(elements.len(), 1);
        let root = &elements[0];
        assert_eq!(root.semantic_id, "info-state:favorites-empty");
        assert_eq!(root.role.as_deref(), Some("info-state"));
        assert_eq!(root.source.as_deref(), Some("InfoState"));
        assert_eq!(root.source_name.as_deref(), Some("favorites-empty"));
        assert_eq!(root.value.as_deref(), Some("star"));
        assert_eq!(root.kind.as_deref(), Some("neutral"));
    }
}

#[cfg(test)]
mod app_layout_projection_tests {
    use super::*;

    #[test]
    fn complete_projection_has_no_degradation_reasons() {
        let outcome = ElementCollectionOutcome::complete(
            "settings",
            vec![protocol::ElementInfo::panel("settings")],
            1,
        );
        assert_eq!(outcome.semantic_surface, "settings");
        assert_eq!(outcome.version, 1);
        assert_eq!(
            outcome.projection_quality,
            protocol::ProjectionQuality::Complete
        );
        assert!(outcome.reason_codes.is_empty());
    }

    #[test]
    fn partial_and_unsupported_projections_are_typed() {
        let partial = ElementCollectionOutcome::partial(
            "flowSession",
            protocol::ProjectionReason::RuntimeEntityMissing,
            vec![protocol::ElementInfo::panel("flow-session")],
            1,
        );
        assert_eq!(
            partial.projection_quality,
            protocol::ProjectionQuality::Partial
        );
        assert_eq!(
            partial.reason_codes,
            vec![protocol::ProjectionReason::RuntimeEntityMissing]
        );

        let unsupported = ElementCollectionOutcome::unsupported(
            "divPrompt",
            protocol::ProjectionReason::UnsupportedCustomDocument,
            vec![protocol::ElementInfo::panel("div-prompt")],
            1,
        );
        assert_eq!(
            unsupported.projection_quality,
            protocol::ProjectionQuality::Unsupported
        );
        assert_eq!(
            unsupported.reason_codes,
            vec![protocol::ProjectionReason::UnsupportedCustomDocument]
        );
    }

    #[test]
    fn empty_surface_finalizer_cannot_fabricate_completeness() {
        let outcome = ScriptListApp::finalize_surface_outcome(
            "fixture",
            "fixture",
            "panel_only_fixture",
            10,
            Vec::new(),
            0,
        );
        assert_eq!(
            outcome.projection_quality,
            protocol::ProjectionQuality::Partial
        );
        assert_eq!(
            outcome.reason_codes,
            vec![protocol::ProjectionReason::PanelOnly]
        );
        assert_eq!(outcome.warnings, vec!["panel_only_fixture"]);
    }
}

#[cfg(test)]
mod recent_files_semantic_tests {
    use super::*;

    #[test]
    fn recent_files_semantic_kind_distinguishes_directories() {
        assert_eq!(
            root_file_semantic_kind(crate::file_search::FileType::Directory),
            "directory"
        );
        assert_eq!(
            root_file_semantic_kind(crate::file_search::FileType::Document),
            "file"
        );
    }
}
