impl ScriptListApp {
    fn collect_creation_feedback_elements(
        &self,
        payload: &prompts::CreationFeedbackPayload,
        limit: usize,
    ) -> ElementCollectionOutcome {
        let mut all_elements = Vec::new();
        let mut field = |semantic_id: &str,
                         element_type: protocol::ElementType,
                         text: Option<String>,
                         value: Option<String>,
                         role: Option<&str>,
                         kind: Option<&str>,
                         status_kind: Option<&str>,
                         action_disabled: Option<&str>| {
            all_elements.push(protocol::ElementInfo {
                semantic_id: semantic_id.to_string(),
                element_type,
                text,
                value,
                content: None,
                selected: None,
                focused: None,
                index: None,
                role: role.map(str::to_string),
                kind: kind.map(str::to_string),
                source: None,
                source_name: None,
                selectable: Some(false),
                status_kind: status_kind.map(str::to_string),
                action_disabled: action_disabled.map(str::to_string),
                style: None,
            });
        };

        field(
            "creation-feedback:panel",
            protocol::ElementType::Panel,
            Some("Creation feedback".to_string()),
            None,
            Some("panel"),
            Some("creation_feedback"),
            None,
            None,
        );
        field(
            "creation-feedback:artifact-kind",
            protocol::ElementType::Panel,
            Some(payload.artifact_kind_label().to_string()),
            Some(payload.artifact_kind.kind().to_string()),
            Some("status"),
            Some("artifact_kind"),
            Some(payload.artifact_kind.kind()),
            None,
        );
        field(
            "creation-feedback:artifact-path",
            protocol::ElementType::Input,
            Some("Artifact path".to_string()),
            Some(payload.artifact_path.to_string_lossy().to_string()),
            Some("readonly_path"),
            Some("artifact_path"),
            None,
            None,
        );
        field(
            "creation-feedback:verification-status",
            protocol::ElementType::Panel,
            Some(payload.verification_status_label().to_string()),
            Some(payload.verification_status_kind().to_string()),
            Some("status"),
            Some("verification_status"),
            Some(payload.verification_status_kind()),
            None,
        );
        field(
            "creation-feedback:receipt-status",
            protocol::ElementType::Panel,
            Some(payload.receipt_status_label().to_string()),
            Some(payload.receipt_status_kind().to_string()),
            Some("status"),
            Some("receipt_status"),
            Some(payload.receipt_status_kind()),
            None,
        );
        field(
            "creation-feedback:receipt-path",
            protocol::ElementType::Input,
            Some("Receipt path".to_string()),
            Some(payload.receipt_path_text().to_string()),
            Some("readonly_path"),
            Some("receipt_path"),
            Some(payload.receipt_status_kind()),
            None,
        );

        for (index, semantic_id, label, disabled) in [
            (
                0,
                "button:creation-feedback:reveal-artifact",
                "Reveal in Finder",
                None,
            ),
            (
                1,
                "button:creation-feedback:copy-artifact-path",
                "Copy Path",
                None,
            ),
            (2, "button:creation-feedback:edit-artifact", "Edit", None),
            (
                3,
                "button:creation-feedback:run-artifact",
                "Run",
                Some(payload.run_disabled_reason()),
            ),
            (
                4,
                "button:creation-feedback:copy-receipt-path",
                "Copy Receipt Path",
                payload
                    .receipt_path()
                    .is_none()
                    .then_some("receipt_not_applicable"),
            ),
            (
                5,
                "button:creation-feedback:open-receipt",
                "Open Receipt",
                payload
                    .receipt_path()
                    .is_none()
                    .then_some("receipt_not_applicable"),
            ),
        ] {
            all_elements.push(protocol::ElementInfo {
                semantic_id: semantic_id.to_string(),
                element_type: protocol::ElementType::Button,
                text: Some(label.to_string()),
                value: None,
                content: None,
                selected: None,
                focused: None,
                index: Some(index),
                role: Some("action".to_string()),
                kind: Some("creation_feedback_action".to_string()),
                source: None,
                source_name: None,
                selectable: Some(disabled.is_none()),
                status_kind: disabled.map(str::to_string),
                action_disabled: disabled.map(str::to_string),
                style: None,
            });
        }

        let total_count = all_elements.len();
        let elements = all_elements.into_iter().take(limit).collect();
        ElementCollectionOutcome::complete("creationFeedback", elements, total_count)
    }
}
