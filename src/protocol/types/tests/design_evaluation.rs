use super::*;
use crate::protocol::parse_owned_frame_cursor;
use serde_json::{json, Value};

fn valid_fixture_controls() -> Vec<Value> {
    vec![
        json!({"family":"search","operation":"prepare","scenario":"tab-domain-hoist"}),
        json!({"family":"search","operation":"release","runIds":[42]}),
        json!({"family":"search","operation":"release","runIds":[42,43]}),
        json!({"family":"search","operation":"advance","milliseconds":16}),
        json!({"family": "agentChat", "operation": "submit", "text": "fixture request"}),
        json!({"family": "agentChat", "operation": "retry"}),
        json!({"family": "agentChat", "operation": "stop"}),
        json!({"family": "agentChat", "operation": "emitText", "turnGeneration": 7, "text": "response"}),
        json!({"family": "agentChat", "operation": "complete", "turnGeneration": 7}),
        json!({"family": "agentChat", "operation": "fail", "turnGeneration": 7}),
        json!({"family": "agentChat", "operation": "openHistory"}),
        json!({"family": "agentChat", "operation": "openSlashPicker"}),
        json!({"family": "agentChat", "operation": "openProfilePicker"}),
        json!({"family": "flow", "sessionId": 11, "operation": "submit", "text": "fixture request"}),
        json!({"family": "flow", "sessionId": 11, "operation": "retry"}),
        json!({"family": "flow", "sessionId": 11, "operation": "stop"}),
        json!({"family": "flow", "sessionId": 11, "operation": "background"}),
        json!({"family": "flow", "sessionId": 11, "operation": "resume"}),
        json!({"family": "flow", "sessionId": 11, "operation": "emitText", "messageId": "turn-7", "text": "response"}),
        json!({"family": "flow", "sessionId": 11, "operation": "complete", "messageId": "turn-7"}),
        json!({"family": "flow", "sessionId": 11, "operation": "fail", "messageId": "turn-7"}),
        json!({"family": "sdkChat", "operation": "submit", "text": "fixture request"}),
        json!({"family": "sdkChat", "operation": "retry"}),
        json!({"family": "sdkChat", "operation": "stop"}),
        json!({"family": "sdkChat", "operation": "emitText", "messageId": "turn-7", "text": "response"}),
        json!({"family": "sdkChat", "operation": "complete", "messageId": "turn-7"}),
        json!({"family": "sdkChat", "operation": "fail", "messageId": "turn-7"}),
        json!({"family": "dictation", "operation": "begin", "destination": "mainFilter"}),
        json!({"family": "dictation", "operation": "recording", "text": "transcript", "bars": ([0.0; 9])}),
        json!({"family": "dictation", "operation": "confirm"}),
        json!({"family": "dictation", "operation": "resume"}),
        json!({"family": "dictation", "operation": "transcribe"}),
        json!({"family": "dictation", "operation": "deliver"}),
        json!({"family": "dictation", "operation": "retarget", "destination": "notes"}),
        json!({"family": "dictation", "operation": "openMicrophonePicker"}),
        json!({"family": "fault", "operation": "suppressThemeNotification", "target": {"type": "instance", "id": "main", "generation": 3}}),
    ]
}

#[test]
fn fixture_controls_round_trip_every_flat_operation() {
    for wire in valid_fixture_controls() {
        let control: FixtureControl = serde_json::from_str(&wire.to_string())
            .unwrap_or_else(|error| panic!("valid control {wire} rejected: {error}"));
        assert_eq!(
            serde_json::to_value(control).expect("serialize control"),
            wire
        );
    }
}

#[test]
fn fixture_controls_reject_unknown_fields_families_and_operations() {
    for wire in valid_fixture_controls() {
        for field in ["unexpected", "command"] {
            let mut unknown_field = wire.clone();
            unknown_field[field] = json!({"operation": "retry"});
            assert!(
                serde_json::from_value::<FixtureControl>(unknown_field.clone()).is_err(),
                "accepted unknown field in {unknown_field}"
            );
        }
        let mut unknown_family = wire.clone();
        unknown_family["family"] = json!("unknownFamily");
        assert!(serde_json::from_value::<FixtureControl>(unknown_family).is_err());
        let mut unknown_operation = wire;
        unknown_operation["operation"] = json!("unknownOperation");
        assert!(serde_json::from_value::<FixtureControl>(unknown_operation).is_err());
    }
    assert!(serde_json::from_value::<FixtureControl>(json!({
        "family": "agentChat", "command": {"operation": "retry"}
    }))
    .is_err());
    assert!(serde_json::from_value::<FixtureControl>(json!({"operation": "retry"})).is_err());
    assert!(serde_json::from_value::<FixtureControl>(json!({"family": "agentChat"})).is_err());
}

#[test]
fn flow_fixture_controls_require_session_identity_for_every_operation() {
    for wire in valid_fixture_controls()
        .into_iter()
        .filter(|wire| wire["family"] == "flow")
    {
        let mut missing_session = wire.clone();
        missing_session.as_object_mut().unwrap().remove("sessionId");
        assert!(serde_json::from_value::<FixtureControl>(missing_session).is_err());
        for session_id in [Value::Null, json!("11"), json!(-1), json!(1.5)] {
            let mut invalid_session = wire.clone();
            invalid_session["sessionId"] = session_id;
            assert!(serde_json::from_value::<FixtureControl>(invalid_session).is_err());
        }
    }
}

#[test]
fn design_fixture_control_envelope_preserves_flat_controls_and_rejects_unknown_fields() {
    for control in valid_fixture_controls() {
        let wire = json!({
            "operation": "fixtureControl",
            "target": {"type": "instance", "id": "main", "generation": 3},
            "expected": {
                "windowId": "main", "windowGeneration": 3, "appViewVariant": "ScriptList",
                "targetGeneration": 3, "surfaceGeneration": 4, "dataGeneration": 5
            },
            "control": control
        });
        let command: DesignCommand = serde_json::from_str(&wire.to_string())
            .unwrap_or_else(|error| panic!("valid fixture command {wire} rejected: {error}"));
        assert_eq!(
            serde_json::to_value(command).expect("serialize command"),
            wire
        );
        let mut unknown_outer_field = wire.clone();
        unknown_outer_field["unexpected"] = json!(true);
        assert!(serde_json::from_value::<DesignCommand>(unknown_outer_field).is_err());
        let mut unknown_inner_field = wire;
        unknown_inner_field["control"]["unexpected"] = json!(true);
        assert!(serde_json::from_value::<DesignCommand>(unknown_inner_field).is_err());
    }
}

#[test]
fn payload_free_design_commands_reject_unknown_fields() {
    for operation in ["catalog", "diagnose", "end"] {
        let wire = json!({"operation": operation});
        let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(command).unwrap(), wire);
        for field in ["unexpected", "command", "target"] {
            let mut extended = wire.clone();
            extended[field] = json!(true);
            assert!(serde_json::from_value::<DesignCommand>(extended).is_err());
        }
    }
}

#[test]
fn atomic_capture_frame_command_requires_target_and_image_flag_without_expected_revision() {
    for include_image in [true, false] {
        let wire = json!({
            "operation": "captureFrame",
            "target": {"type": "instance", "id": "main", "generation": 3},
            "includeImage": include_image
        });
        let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
        assert!(matches!(command, DesignCommand::CaptureFrame { .. }));
        assert_eq!(serde_json::to_value(command).unwrap(), wire);
        for required in ["target", "includeImage"] {
            let mut missing = wire.clone();
            missing.as_object_mut().unwrap().remove(required);
            assert!(serde_json::from_value::<DesignCommand>(missing).is_err());
        }
        for field in ["expected", "afterFrameGeneration", "hiDpi", "unexpected"] {
            let mut extended = wire.clone();
            extended[field] = json!(true);
            assert!(serde_json::from_value::<DesignCommand>(extended).is_err());
        }
        for invalid in [json!(null), json!("true"), json!(1)] {
            let mut malformed = wire.clone();
            malformed["includeImage"] = invalid;
            assert!(serde_json::from_value::<DesignCommand>(malformed).is_err());
        }
        for required in ["id", "generation"] {
            let mut missing = wire.clone();
            missing["target"].as_object_mut().unwrap().remove(required);
            assert!(serde_json::from_value::<DesignCommand>(missing).is_err());
        }
    }
}

#[test]
fn frame_acknowledgement_requires_explicit_identity_and_a_strict_cursor() {
    let wire = json!({
        "operation":"acknowledgeFrames",
        "target":{"type":"instance", "id":"main", "generation":3},
        "expected":{"windowId":"main", "windowGeneration":3, "appViewVariant":"ScriptList",
            "targetGeneration":3, "surfaceGeneration":4, "dataGeneration":5, "frameGeneration":19},
        "cursor":{"traceGeneration":7, "afterFrameGeneration":19}
    });
    let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
    assert!(matches!(&command, DesignCommand::AcknowledgeFrames { .. }));
    assert_eq!(serde_json::to_value(command).unwrap(), wire);
    for field in ["target", "expected", "cursor"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<DesignCommand>(missing).is_err());
    }
    for cursor in [
        Value::Null,
        json!({}),
        json!({"traceGeneration":7,"afterFrameGeneration":-1}),
        json!({"traceGeneration":7,"afterFrameGeneration":19,"force":true}),
    ] {
        let mut invalid = wire.clone();
        invalid["cursor"] = cursor;
        assert!(serde_json::from_value::<DesignCommand>(invalid).is_err());
    }
    for field in ["frameCursor", "draw", "clearError"] {
        let mut invalid = wire.clone();
        invalid[field] = json!(true);
        assert!(serde_json::from_value::<DesignCommand>(invalid).is_err());
    }
}

#[test]
fn search_fixture_controls_reject_payload_injection_and_malformed_run_ids() {
    for field in [
        "selectedIndex",
        "results",
        "path",
        "code",
        "source",
        "generation",
    ] {
        let mut wire = json!({"family":"search","operation":"release","runIds":[42]});
        wire[field] = json!(1);
        assert!(serde_json::from_value::<FixtureControl>(wire).is_err());
    }
    for id in [Value::Null, json!("42"), json!(-1), json!(1.5)] {
        assert!(serde_json::from_value::<FixtureControl>(
            json!({"family":"search","operation":"release","runIds":[id]})
        )
        .is_err());
    }
    assert!(serde_json::from_str::<FixtureControl>(
        r#"{"family":"search","operation":"release","runIds":[1],"runIds":[2]}"#
    )
    .is_err());
    assert!(serde_json::from_value::<FixtureControl>(
        json!({"family":"search","operation":"release","runId":42})
    )
    .is_err());
    for ids in [Vec::<u64>::new(), vec![1, 1], (1..=129).collect()] {
        assert!(serde_json::from_value::<FixtureControl>(
            json!({"family":"search","operation":"release","runIds":ids})
        )
        .is_err());
    }
    assert!(serde_json::from_value::<FixtureControl>(
        json!({"family":"search","operation":"release","runIds":(1..=128).collect::<Vec<u64>>()})
    )
    .is_ok());
    assert!(serde_json::from_value::<FixtureControl>(
        json!({"family":"search","operation":"advance","milliseconds":4_294_967_296_u64})
    )
    .is_err());
}

#[test]
fn scheduled_capture_requires_exact_observed_frame_and_notification_identity() {
    let wire = json!({"operation":"captureFrame","target":{"type":"instance","id":"main","generation":3},"includeImage":false,
        "scheduled":{"expected":{"windowId":"main","windowGeneration":3,"appViewVariant":"ScriptList","targetGeneration":3,"surfaceGeneration":4,"dataGeneration":5},
        "afterFrameGeneration":8,"afterNotificationEpoch":9}});
    let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(command).unwrap(), wire);
    for required in ["expected", "afterFrameGeneration", "afterNotificationEpoch"] {
        let mut missing = wire.clone();
        missing["scheduled"]
            .as_object_mut()
            .unwrap()
            .remove(required);
        assert!(serde_json::from_value::<DesignCommand>(missing).is_err());
    }
    let mut unknown = wire;
    unknown["scheduled"]["force"] = json!(true);
    assert!(serde_json::from_value::<DesignCommand>(unknown).is_err());
}

#[test]
fn owned_search_provider_wait_round_trips_all_declared_sources() {
    for source in [
        "files",
        "directory",
        "brain-lexical",
        "brain-semantic",
        "tabs",
        "history",
        "windows",
        "icons",
        "notes",
        "todos",
        "clipboard",
        "dictation",
        "conversations",
        "spine",
        "brain-inbox",
        "scripts",
        "apps",
        "skills",
        "validation",
        "flow-roster",
    ] {
        for after in [0, u64::MAX] {
            let wire = json!({"type":"searchProvider","source":source,"query":{"lifetime":7,"revision":19,"scopeRevision":2},"afterRunId":after});
            let condition: OwnedSearchProviderCondition =
                serde_json::from_value(wire.clone()).unwrap();
            let OwnedSearchProviderCondition::SearchProvider {
                source: parsed_source,
                query,
                after_run_id,
                accept_cached,
            } = condition;
            assert_eq!(parsed_source.as_str(), source);
            assert_eq!(
                query,
                OwnedSearchQueryStamp {
                    lifetime: 7,
                    revision: 19,
                    scope_revision: 2
                }
            );
            assert_eq!(after_run_id, after);
            assert!(!accept_cached);
            assert_eq!(serde_json::to_value(condition).unwrap(), wire);
        }
    }
}

#[test]
fn owned_search_provider_wait_cache_readiness_is_explicit() {
    for accept_cached in [false, true] {
        let wire = json!({"type":"searchProvider","source":"tabs","query":{"lifetime":7,"revision":19,"scopeRevision":2},"afterRunId":0,"acceptCached":accept_cached});
        let condition: OwnedSearchProviderCondition = serde_json::from_value(wire.clone()).unwrap();
        assert!(
            matches!(condition, OwnedSearchProviderCondition::SearchProvider { accept_cached: actual, .. } if actual == accept_cached)
        );
        let mut expected = wire;
        if !accept_cached {
            expected.as_object_mut().unwrap().remove("acceptCached");
        }
        assert_eq!(serde_json::to_value(condition).unwrap(), expected);
    }
}

#[test]
fn owned_search_provider_wait_rejects_malformed_and_widened_conditions() {
    let wire = json!({"type":"searchProvider","source":"brain-semantic","query":{"lifetime":7,"revision":19,"scopeRevision":2},"afterRunId":11});
    for field in ["type", "source", "query", "afterRunId"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<OwnedSearchProviderCondition>(missing).is_err());
    }
    for (field, invalid) in [
        ("source", json!("unknown")),
        ("source", Value::Null),
        ("type", json!("stateMatch")),
        ("query", Value::Null),
        ("afterRunId", Value::Null),
        ("afterRunId", json!(-1)),
        ("afterRunId", json!(1.5)),
        ("afterRunId", json!("11")),
        ("acceptCached", Value::Null),
        ("acceptCached", json!("true")),
        ("acceptCached", json!(1)),
    ] {
        let mut invalid_wire = wire.clone();
        invalid_wire[field] = invalid;
        assert!(serde_json::from_value::<OwnedSearchProviderCondition>(invalid_wire).is_err());
    }
    for field in ["lifetime", "revision", "scopeRevision"] {
        for invalid in [Value::Null, json!(-1), json!(1.5), json!("7"), json!(false)] {
            let mut invalid_wire = wire.clone();
            invalid_wire["query"][field] = invalid;
            assert!(serde_json::from_value::<OwnedSearchProviderCondition>(invalid_wire).is_err());
        }
        let mut missing = wire.clone();
        missing["query"].as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<OwnedSearchProviderCondition>(missing).is_err());
    }
    for field in ["generation", "runId", "release", "clock", "results"] {
        let mut widened = wire.clone();
        widened[field] = json!(true);
        assert!(serde_json::from_value::<OwnedSearchProviderCondition>(widened).is_err());
    }
    let mut widened_query = wire;
    widened_query["query"]["text"] = json!("old query");
    assert!(serde_json::from_value::<OwnedSearchProviderCondition>(widened_query).is_err());
}

#[test]
fn owned_search_provider_wait_does_not_expand_ordinary_wait_conditions() {
    let wire = json!({"type":"searchProvider","source":"files","query":{"lifetime":7,"revision":19,"scopeRevision":2},"afterRunId":0});
    assert!(serde_json::from_value::<OwnedSearchProviderCondition>(wire.clone()).is_ok());
    assert!(serde_json::from_value::<WaitCondition>(wire).is_err());
}

#[test]
fn owned_file_search_stream_wait_round_trips_only_its_exact_identity() {
    let wire = json!({"type":"fileSearchStream","generation":7,"query":"~/owned/資料"});
    let condition: OwnedFileSearchStreamCondition = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(condition).unwrap(), wire);
    assert!(serde_json::from_value::<WaitCondition>(wire.clone()).is_err());
    assert!(serde_json::from_value::<OwnedSearchProviderCondition>(wire).is_err());
}

#[test]
fn owned_file_search_stream_wait_rejects_malformed_and_widened_conditions() {
    let wire = json!({"type":"fileSearchStream","generation":7,"query":"~/owned/"});
    for field in ["type", "generation", "query"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<OwnedFileSearchStreamCondition>(missing).is_err());
    }
    for (field, invalid) in [
        ("type", json!("searchProvider")),
        ("generation", Value::Null),
        ("generation", json!(-1)),
        ("generation", json!(1.5)),
        ("generation", json!("7")),
        ("query", Value::Null),
        ("query", json!(false)),
        ("release", json!(true)),
        ("source", json!("files")),
    ] {
        let mut invalid_wire = wire.clone();
        invalid_wire[field] = invalid;
        assert!(serde_json::from_value::<OwnedFileSearchStreamCondition>(invalid_wire).is_err());
    }
}

#[test]
fn owned_file_search_preview_wait_round_trips_only_its_exact_identity() {
    let wire =
        json!({"type":"fileSearchPreview","generation":7,"query":"~/owned/資料","workSequence":19});
    let condition: OwnedFileSearchPreviewCondition = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(condition).unwrap(), wire);
    assert!(serde_json::from_value::<WaitCondition>(wire.clone()).is_err());
    assert!(serde_json::from_value::<OwnedFileSearchStreamCondition>(wire.clone()).is_err());
    assert!(serde_json::from_value::<OwnedSearchProviderCondition>(wire).is_err());
}

#[test]
fn owned_file_search_preview_wait_rejects_malformed_and_widened_conditions() {
    let wire =
        json!({"type":"fileSearchPreview","generation":7,"query":"~/owned/","workSequence":19});
    for field in ["type", "generation", "query", "workSequence"] {
        let mut missing = wire.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(serde_json::from_value::<OwnedFileSearchPreviewCondition>(missing).is_err());
    }
    for field in ["generation", "workSequence"] {
        for invalid in [
            Value::Null,
            json!(0),
            json!(-1),
            json!(1.5),
            json!("7"),
            json!(false),
        ] {
            let mut invalid_wire = wire.clone();
            invalid_wire[field] = invalid;
            assert!(
                serde_json::from_value::<OwnedFileSearchPreviewCondition>(invalid_wire).is_err()
            );
        }
    }
    for (field, invalid) in [
        ("type", json!("fileSearchStream")),
        ("query", Value::Null),
        ("query", json!(false)),
        ("query", json!({"text":"~/owned/"})),
        ("release", json!(true)),
        ("phase", json!("held")),
        ("contentHash", json!("supplied-hash")),
        ("path", json!("/outside")),
        ("source", json!("files")),
    ] {
        let mut invalid_wire = wire.clone();
        invalid_wire[field] = invalid;
        assert!(serde_json::from_value::<OwnedFileSearchPreviewCondition>(invalid_wire).is_err());
    }
}

#[test]
fn owned_response_encoding_requires_an_explicit_supported_version() {
    let wire = json!("zlib-json-base64-v1");
    let encoding: OwnedResponseEncoding = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(encoding).unwrap(), wire);
    for invalid in [
        Value::Null,
        json!("zlib-json-base64-v2"),
        json!(true),
        json!({"encoding":wire}),
    ] {
        assert!(serde_json::from_value::<OwnedResponseEncoding>(invalid).is_err());
    }
}

#[test]
fn capture_frame_cursor_preserves_omission_and_scheduled_authority() {
    let cursor = OwnedFrameCursor {
        trace_generation: 7,
        after_frame_generation: 19,
    };
    let scheduled = json!({
        "expected":{"windowId":"main","windowGeneration":3,"appViewVariant":"ScriptList","targetGeneration":3,"surfaceGeneration":4,"dataGeneration":5},
        "afterFrameGeneration":8,"afterNotificationEpoch":9
    });
    for requirement in [None, Some(scheduled)] {
        for include_image in [false, true] {
            for expected_cursor in [None, Some(cursor)] {
                let mut wire = json!({"operation":"captureFrame","target":{"type":"instance","id":"main","generation":3},"includeImage":include_image});
                if let Some(requirement) = &requirement {
                    wire["scheduled"] = requirement.clone();
                }
                if let Some(cursor) = expected_cursor {
                    wire["frameCursor"] = serde_json::to_value(cursor).unwrap();
                }
                let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
                assert!(
                    matches!(&command, DesignCommand::CaptureFrame { frame_cursor, .. } if *frame_cursor == expected_cursor)
                );
                assert_eq!(serde_json::to_value(command).unwrap(), wire);
            }
        }
    }
}

#[test]
fn capture_frame_cursor_rejects_null_malformed_and_unknown_fields() {
    let reject = |cursor: Value| {
        let wire = json!({"operation":"captureFrame","target":{"type":"instance","id":"main","generation":3},"includeImage":false,"frameCursor":cursor});
        let error = serde_json::from_value::<DesignCommand>(wire).unwrap_err();
        assert_eq!(error.to_string(), "frame_cursor_invalid");
    };
    for invalid in [
        Value::Null,
        json!(false),
        json!(1),
        json!("cursor"),
        json!([]),
        json!({}),
    ] {
        reject(invalid);
    }
    for field in ["traceGeneration", "afterFrameGeneration"] {
        for invalid in [
            Value::Null,
            json!(true),
            json!("1"),
            json!(-1),
            json!(1.5),
            json!(1.0),
            json!([]),
            json!({}),
        ] {
            let mut cursor = json!({"traceGeneration":7,"afterFrameGeneration":19});
            cursor[field] = invalid;
            reject(cursor);
        }
        let mut cursor = json!({"traceGeneration":7,"afterFrameGeneration":19});
        cursor.as_object_mut().unwrap().remove(field);
        reject(cursor);
    }
    reject(json!({"traceGeneration":7,"afterFrameGeneration":19,"force":true}));
}

#[test]
fn owned_frame_cursor_round_trips_without_mutating_the_request() {
    let mut request =
        json!({"type":"getState","target":{"type":"instance","id":"main","generation":3}});
    assert_eq!(parse_owned_frame_cursor(&request), Ok(None));
    let cursor = OwnedFrameCursor {
        trace_generation: 7,
        after_frame_generation: 19,
    };
    request["frameCursor"] = json!({"traceGeneration":7,"afterFrameGeneration":19});
    let before = request.clone();
    assert_eq!(parse_owned_frame_cursor(&request), Ok(Some(cursor)));
    assert_eq!(parse_owned_frame_cursor(&request), Ok(Some(cursor)));
    assert_eq!(
        serde_json::to_value(cursor).unwrap(),
        request["frameCursor"]
    );
    assert_eq!(request, before);
}

#[test]
fn owned_frame_cursor_rejects_null_malformed_and_unknown_fields() {
    for invalid in [
        Value::Null,
        json!(false),
        json!(0),
        json!("cursor"),
        json!([]),
        json!({}),
    ] {
        let request = json!({"type":"getState","frameCursor":invalid});
        assert_eq!(
            parse_owned_frame_cursor(&request),
            Err("frame_cursor_invalid")
        );
    }
    for field in ["traceGeneration", "afterFrameGeneration"] {
        for invalid in [
            Value::Null,
            json!(true),
            json!("1"),
            json!(-1),
            json!(1.5),
            json!(1.0),
            json!([]),
            json!({}),
        ] {
            let mut request = json!({"type":"getState","frameCursor":{"traceGeneration":7,"afterFrameGeneration":19}});
            request["frameCursor"][field] = invalid;
            assert_eq!(
                parse_owned_frame_cursor(&request),
                Err("frame_cursor_invalid")
            );
        }
        let mut request = json!({"type":"getState","frameCursor":{"traceGeneration":7,"afterFrameGeneration":19}});
        request["frameCursor"]
            .as_object_mut()
            .unwrap()
            .remove(field);
        assert_eq!(
            parse_owned_frame_cursor(&request),
            Err("frame_cursor_invalid")
        );
    }
    let request = json!({"type":"getState","frameCursor":{"traceGeneration":7,"afterFrameGeneration":19,"force":true}});
    assert_eq!(
        parse_owned_frame_cursor(&request),
        Err("frame_cursor_invalid")
    );
}

#[test]
fn owned_frame_cursor_is_only_admitted_on_owned_get_state() {
    for operation in [
        "getElements",
        "getLayoutInfo",
        "batch",
        "waitFor",
        "design",
        "",
    ] {
        let request =
            json!({"type":operation,"frameCursor":{"traceGeneration":7,"afterFrameGeneration":19}});
        assert_eq!(
            parse_owned_frame_cursor(&request),
            Err("frame_cursor_invalid")
        );
    }
}

#[test]
fn owned_frame_cursor_validates_lifetime_and_retained_history_boundaries() {
    let cursor = OwnedFrameCursor {
        trace_generation: 7,
        after_frame_generation: 19,
    };
    assert_eq!(cursor.validate(8, 20, 25), Err("frame_cursor_stale"));
    assert_eq!(cursor.validate(6, 0, 18), Err("frame_cursor_stale"));
    assert_eq!(cursor.validate(7, 20, 25), Err("frame_cursor_retired"));
    assert_eq!(cursor.validate(7, 10, 18), Err("frame_cursor_future"));
    assert_eq!(cursor.validate(7, 19, 25), Ok(()));
    assert_eq!(cursor.validate(7, 10, 19), Ok(()));
    assert_eq!(cursor.validate(7, 19, 19), Ok(()));
    assert_eq!(cursor.validate(7, 10, 25), Ok(()));
    assert_eq!(
        cursor,
        OwnedFrameCursor {
            trace_generation: 7,
            after_frame_generation: 19
        }
    );
    for generation in [0, u64::MAX] {
        let cursor = OwnedFrameCursor {
            trace_generation: generation,
            after_frame_generation: generation,
        };
        assert_eq!(cursor.validate(generation, generation, generation), Ok(()));
        let request = json!({"type":"getState","frameCursor":cursor});
        assert_eq!(parse_owned_frame_cursor(&request), Ok(Some(cursor)));
    }
}

#[test]
fn safety_probe_catalog_round_trips_through_strict_command_envelope() {
    let expected_names = [
        "invalidShow",
        "invalidFocus",
        "invalidDialog",
        "invalidTabbing",
        "invalidOversize",
        "nativeActivation",
        "nativeIme",
        "globalPointer",
        "clipboardRead",
        "clipboardWrite",
        "directAppActivation",
        "process",
        "provider",
        "credentials",
        "device",
        "openExternal",
        "notification",
        "blankReadback",
        "failedReadback",
        "missingRequiredImage",
        "missingRequiredSvg",
        "oversizedImage",
        "duplicateSemanticIdentity",
        "duplicateMeasurementIdentity",
        "deferredDispatch",
    ];
    assert_eq!(NativeSafetyProbe::ALL.len(), expected_names.len());
    for (probe, name) in NativeSafetyProbe::ALL.iter().zip(expected_names) {
        assert_eq!(serde_json::to_value(probe).unwrap(), json!(name));
        let wire = json!({
            "operation": "probeSafety",
            "target": {"type": "instance", "id": "main", "generation": 3},
            "expected": {
                "windowId": "main", "windowGeneration": 3, "appViewVariant": "ScriptList",
                "targetGeneration": 3, "surfaceGeneration": 4, "dataGeneration": 5,
                "presentationRevision": 6, "themeRevision": 7, "frameGeneration": 8
            },
            "probe": name
        });
        let command: DesignCommand = serde_json::from_value(wire.clone()).unwrap();
        assert_eq!(serde_json::to_value(command).unwrap(), wire);
        for required in ["target", "expected", "probe"] {
            let mut missing = wire.clone();
            missing.as_object_mut().unwrap().remove(required);
            assert!(serde_json::from_value::<DesignCommand>(missing).is_err());
        }
        for field in ["pixels", "path", "command", "unexpected"] {
            let mut extended = wire.clone();
            extended[field] = json!("untrusted input");
            assert!(serde_json::from_value::<DesignCommand>(extended).is_err());
        }
        for invalid in [
            json!("unknownProbe"),
            json!(false),
            json!([]),
            json!({"kind":name,"pixels":[0]}),
        ] {
            let mut extended = wire.clone();
            extended["probe"] = invalid;
            assert!(serde_json::from_value::<DesignCommand>(extended).is_err());
        }
    }
}
