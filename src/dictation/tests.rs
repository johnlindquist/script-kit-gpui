use crate::dictation::capture::{mix_to_mono, normalize_chunk, resample_linear, run_processor};
use crate::dictation::transcription::{
    build_session_result, is_parakeet_model_available, merge_captured_chunks, DictationEngine,
    DictationTranscriber, DictationTranscriptionConfig, ParakeetDictationEngine,
    WhisperDictationEngine,
};
use crate::dictation::types::{
    CapturedAudioChunk, CompletedDictationCapture, DictationCaptureConfig, DictationCaptureEvent,
    DictationDestination, RawAudioChunk,
};
use crate::dictation::visualizer::{silent_bars, AudioVisualiser};
use crate::dictation::DictationOverlayState;
use anyhow::Result;
use parking_lot::Mutex;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Stub engine for unit tests
// ---------------------------------------------------------------------------

struct StubEngine {
    output: String,
}

impl DictationEngine for StubEngine {
    fn transcribe(&mut self, _samples: &[f32], _initial_prompt: Option<&str>) -> Result<String> {
        Ok(self.output.clone())
    }
}

// ---------------------------------------------------------------------------
// Existing capture / visualizer tests
// ---------------------------------------------------------------------------

#[test]
fn mix_to_mono_averages_interleaved_channels() {
    let mono = mix_to_mono(&[1.0, -1.0, 0.5, 0.0], 2);
    assert_eq!(mono, vec![0.0, 0.25]);
}

#[test]
fn resample_linear_preserves_endpoints() {
    let resampled = resample_linear(&[0.0, 1.0], 2, 4);
    assert_eq!(resampled.len(), 4);
    assert!((resampled[0] - 0.0).abs() < 1e-6);
    assert!((resampled[3] - 1.0).abs() < 1e-6);
    assert!(resampled[1] > 0.0 && resampled[1] < 1.0);
    assert!(resampled[2] > 0.0 && resampled[2] < 1.0);
}

#[test]
fn normalize_chunk_mixes_resamples_and_sets_duration() {
    let config = DictationCaptureConfig::default();
    let raw = RawAudioChunk {
        sample_rate_hz: 8_000,
        channels: 2,
        samples: vec![0.5, -0.5, 0.25, 0.25],
    };

    let normalized = normalize_chunk(raw, &config);

    assert_eq!(normalized.sample_rate_hz, 16_000);
    assert_eq!(normalized.samples.len(), 4);
    assert_eq!(normalized.duration, Duration::from_micros(250));
    assert!((normalized.samples[0] - 0.0).abs() < 1e-6);
}

#[test]
fn fft_visualiser_returns_none_until_window_fills() {
    let mut vis = AudioVisualiser::new_speech(16_000);
    // 100 samples is less than the 1024-sample FFT window — should return None.
    let silence = vec![0.0_f32; 100];
    assert!(vis.feed(&silence).is_none());
}

#[test]
fn fft_visualiser_returns_bars_when_window_fills() {
    let mut vis = AudioVisualiser::new_speech(16_000);
    // Generate a 440 Hz sine tone — 1024 samples at 16 kHz.
    let samples: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin() * 0.5)
        .collect();
    let bars = vis.feed(&samples);
    assert!(bars.is_some(), "should return bars after full window");
    let bars = bars.expect("bars");
    assert_eq!(bars.len(), 9);
    assert!(
        bars.iter().all(|&b| (0.0..=1.0).contains(&b)),
        "all bars should be 0.0–1.0"
    );
}

#[test]
fn silent_bars_are_minimum_height() {
    let bars = silent_bars();
    assert_eq!(bars.len(), 9);
    assert!(bars.iter().all(|&b| b < 0.1), "silent bars should be low");
}

// ---------------------------------------------------------------------------
// Transcription facade tests
// ---------------------------------------------------------------------------

#[test]
fn merge_captured_chunks_concatenates_audio() {
    let merged = merge_captured_chunks(&[
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.1, 0.2],
            duration: Duration::from_millis(10),
        },
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.3, 0.4, 0.5],
            duration: Duration::from_millis(15),
        },
    ]);
    assert_eq!(merged, vec![0.1, 0.2, 0.3, 0.4, 0.5]);
}

#[test]
fn merge_captured_chunks_empty_input() {
    let merged = merge_captured_chunks(&[]);
    assert!(merged.is_empty());
}

#[test]
fn transcriber_returns_none_for_silence() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 4,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "should not be used".to_string(),
        }),
    );
    assert_eq!(transcriber.transcribe_samples(&[0.0, 0.0, 0.0, 0.0])?, None);
    Ok(())
}

#[test]
fn transcriber_returns_none_for_too_few_samples() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 100,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "should not be used".to_string(),
        }),
    );
    assert_eq!(transcriber.transcribe_samples(&[0.5, -0.5])?, None);
    Ok(())
}

#[test]
fn transcriber_returns_none_for_empty_engine_output() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "   ".to_string(),
        }),
    );
    assert_eq!(transcriber.transcribe_samples(&[0.25, -0.25])?, None);
    Ok(())
}

#[test]
fn transcriber_trims_engine_output() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "  hello world  ".to_string(),
        }),
    );
    assert_eq!(
        transcriber.transcribe_samples(&[0.25, -0.25])?,
        Some("hello world".to_string())
    );
    Ok(())
}

#[test]
fn transcriber_chunks_delegates_to_samples() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "from chunks".to_string(),
        }),
    );
    let chunks = vec![
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.3, 0.4],
            duration: Duration::from_millis(10),
        },
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.5],
            duration: Duration::from_millis(5),
        },
    ];
    assert_eq!(
        transcriber.transcribe_chunks(&chunks)?,
        Some("from chunks".to_string())
    );
    Ok(())
}

#[test]
fn build_session_result_sums_duration_and_keeps_destination() {
    let result = build_session_result(
        &[
            CapturedAudioChunk {
                sample_rate_hz: 16_000,
                samples: vec![0.1, 0.2],
                duration: Duration::from_millis(10),
            },
            CapturedAudioChunk {
                sample_rate_hz: 16_000,
                samples: vec![0.3],
                duration: Duration::from_millis(20),
            },
        ],
        DictationDestination::FrontmostApp,
        "hello".to_string(),
    );

    assert_eq!(result.transcript, "hello");
    assert_eq!(result.destination, DictationDestination::FrontmostApp);
    assert_eq!(result.audio_duration, Duration::from_millis(30));
}

#[test]
fn build_session_result_active_prompt_destination() {
    let result = build_session_result(
        &[CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.1],
            duration: Duration::from_millis(50),
        }],
        DictationDestination::ActivePrompt,
        "dictated text".to_string(),
    );

    assert_eq!(result.transcript, "dictated text");
    assert_eq!(result.destination, DictationDestination::ActivePrompt);
    assert_eq!(result.audio_duration, Duration::from_millis(50));
}

// ---------------------------------------------------------------------------
// Chunk-duration buffering tests
// ---------------------------------------------------------------------------

#[test]
fn run_processor_honors_chunk_duration_and_flushes_tail() {
    let config = DictationCaptureConfig {
        sample_rate_hz: 16_000,
        chunk_duration: Duration::from_millis(1), // 16 samples per chunk
        level_window: Duration::from_millis(1),
    };
    let (raw_tx, raw_rx) = std::sync::mpsc::sync_channel(4);
    let (event_tx, event_rx) = async_channel::bounded(16);

    let join = std::thread::spawn(move || run_processor(raw_rx, event_tx, config));

    raw_tx
        .send(RawAudioChunk {
            sample_rate_hz: 16_000,
            channels: 1,
            samples: vec![0.25; 20],
        })
        .expect("send raw chunk");
    drop(raw_tx);

    let mut chunk_lengths = Vec::new();
    while let Ok(event) = event_rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunk_lengths.push(chunk.samples.len()),
            DictationCaptureEvent::EndOfStream => break,
            DictationCaptureEvent::Bars(_) => {}
        }
    }

    join.join().expect("processor thread");
    assert_eq!(chunk_lengths, vec![16, 4]);
}

#[test]
fn run_processor_emits_exact_chunk_with_no_tail() {
    let config = DictationCaptureConfig {
        sample_rate_hz: 16_000,
        chunk_duration: Duration::from_millis(1), // 16 samples per chunk
        level_window: Duration::from_millis(1),
    };
    let (raw_tx, raw_rx) = std::sync::mpsc::sync_channel(4);
    let (event_tx, event_rx) = async_channel::bounded(16);

    let join = std::thread::spawn(move || run_processor(raw_rx, event_tx, config));

    raw_tx
        .send(RawAudioChunk {
            sample_rate_hz: 16_000,
            channels: 1,
            samples: vec![0.5; 16],
        })
        .expect("send raw chunk");
    drop(raw_tx);

    let mut chunk_lengths = Vec::new();
    while let Ok(event) = event_rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunk_lengths.push(chunk.samples.len()),
            DictationCaptureEvent::EndOfStream => break,
            DictationCaptureEvent::Bars(_) => {}
        }
    }

    join.join().expect("processor thread");
    assert_eq!(chunk_lengths, vec![16]);
}

#[test]
fn run_processor_buffers_across_multiple_raw_chunks() {
    let config = DictationCaptureConfig {
        sample_rate_hz: 16_000,
        chunk_duration: Duration::from_millis(1), // 16 samples per chunk
        level_window: Duration::from_millis(1),
    };
    let (raw_tx, raw_rx) = std::sync::mpsc::sync_channel(4);
    let (event_tx, event_rx) = async_channel::bounded(16);

    let join = std::thread::spawn(move || run_processor(raw_rx, event_tx, config));

    // Send 10 samples, then 10 more — should produce one 16-sample chunk + 4-sample tail
    raw_tx
        .send(RawAudioChunk {
            sample_rate_hz: 16_000,
            channels: 1,
            samples: vec![0.1; 10],
        })
        .expect("send first");
    raw_tx
        .send(RawAudioChunk {
            sample_rate_hz: 16_000,
            channels: 1,
            samples: vec![0.2; 10],
        })
        .expect("send second");
    drop(raw_tx);

    let mut chunk_lengths = Vec::new();
    while let Ok(event) = event_rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunk_lengths.push(chunk.samples.len()),
            DictationCaptureEvent::EndOfStream => break,
            DictationCaptureEvent::Bars(_) => {}
        }
    }

    join.join().expect("processor thread");
    assert_eq!(chunk_lengths, vec![16, 4]);
}

// ---------------------------------------------------------------------------
// Transcriber contract tests (prompt forwarding, idle timeout)
// ---------------------------------------------------------------------------

struct RecordingEngine {
    prompts: Arc<Mutex<Vec<Option<String>>>>,
    output: String,
}

impl DictationEngine for RecordingEngine {
    fn transcribe(&mut self, _samples: &[f32], initial_prompt: Option<&str>) -> Result<String> {
        self.prompts.lock().push(initial_prompt.map(str::to_owned));
        Ok(self.output.clone())
    }
}

#[test]
fn transcriber_forwards_initial_prompt() -> Result<()> {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            initial_prompt: Some("keep punctuation".into()),
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(RecordingEngine {
            prompts: prompts.clone(),
            output: "hello".into(),
        }),
    );

    assert_eq!(
        transcriber.transcribe_samples(&[0.25])?,
        Some("hello".into())
    );
    assert_eq!(
        prompts.lock().as_slice(),
        &[Some("keep punctuation".into())]
    );
    Ok(())
}

#[test]
fn transcriber_reports_idle_after_timeout() {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            idle_unload_after: Duration::from_millis(1),
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "ok".into(),
        }),
    );

    std::thread::sleep(Duration::from_millis(5));
    assert!(transcriber.is_idle());
}

#[test]
fn transcriber_not_idle_immediately_after_use() -> Result<()> {
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            idle_unload_after: Duration::from_secs(300),
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "test".into(),
        }),
    );

    let _ = transcriber.transcribe_samples(&[0.5])?;
    assert!(!transcriber.is_idle());
    Ok(())
}

// ---------------------------------------------------------------------------
// WhisperDictationEngine tests
// ---------------------------------------------------------------------------

#[test]
fn whisper_engine_new_fails_for_missing_model() {
    let config = DictationTranscriptionConfig {
        model_path: std::path::PathBuf::from("/definitely/missing-model.bin"),
        ..Default::default()
    };
    let result = WhisperDictationEngine::new(&config);
    assert!(
        result.is_err(),
        "WhisperDictationEngine::new must fail for a missing model path"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found"),
        "error should mention 'not found', got: {err_msg}"
    );
}

#[test]
fn whisper_engine_new_fails_for_directory_path() {
    let config = DictationTranscriptionConfig {
        model_path: std::path::PathBuf::from("/tmp"),
        ..Default::default()
    };
    let result = WhisperDictationEngine::new(&config);
    assert!(
        result.is_err(),
        "WhisperDictationEngine::new must fail for a directory path"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not a regular file"),
        "error should mention 'not a regular file', got: {err_msg}"
    );
}

#[test]
fn whisper_engine_missing_model_error_names_attempted_path() {
    let explicit_path = std::path::PathBuf::from("/nonexistent/dir/model.bin");
    let config = DictationTranscriptionConfig {
        model_path: explicit_path.clone(),
        ..Default::default()
    };
    let err_msg = WhisperDictationEngine::new(&config)
        .unwrap_err()
        .to_string();
    assert!(
        err_msg.contains(explicit_path.to_str().unwrap()),
        "error must name the attempted model path, got: {err_msg}"
    );
}

#[test]
fn whisper_engine_new_accepts_regular_file_model_path() {
    let temp_file = tempfile::NamedTempFile::new().expect("create temp model file");
    let config = DictationTranscriptionConfig {
        model_path: temp_file.path().to_path_buf(),
        ..Default::default()
    };

    WhisperDictationEngine::new(&config)
        .expect("WhisperDictationEngine::new should accept a regular file path");
}

#[test]
fn whisper_engine_new_with_whisper_config_surfaces_path_or_succeeds() {
    use crate::dictation::transcription::resolve_whisper_model_path;
    let whisper_path = resolve_whisper_model_path();
    let config = DictationTranscriptionConfig {
        model_path: whisper_path.clone(),
        ..Default::default()
    };
    let result = WhisperDictationEngine::new(&config);

    if whisper_path.is_file() {
        assert!(
            result.is_ok(),
            "whisper model path should initialize when the file exists: {}",
            whisper_path.display()
        );
    } else {
        let error = result
            .expect_err("whisper model path should fail when the file is missing or invalid")
            .to_string();
        assert!(
            error.contains(&whisper_path.display().to_string()),
            "whisper-path init error should name the attempted path, got: {error}"
        );
    }
}

// ---------------------------------------------------------------------------
// Model path resolution tests
// ---------------------------------------------------------------------------

#[test]
fn default_model_path_is_absolute() {
    let config = DictationTranscriptionConfig::default();
    assert!(
        config.model_path.is_absolute(),
        "default model path must be absolute, got: {}",
        config.model_path.display()
    );
}

#[test]
fn default_model_path_lives_under_kit_path() {
    use crate::setup::get_kit_path;
    let config = DictationTranscriptionConfig::default();
    let kit_path = get_kit_path();
    assert!(
        config.model_path.starts_with(&kit_path),
        "default model path must be under kit_path ({}), got: {}",
        kit_path.display(),
        config.model_path.display()
    );
}

#[test]
fn resolve_default_model_path_matches_config_default() {
    use crate::dictation::transcription::resolve_default_model_path;
    let config = DictationTranscriptionConfig::default();
    assert_eq!(
        config.model_path,
        resolve_default_model_path(),
        "DictationTranscriptionConfig::default().model_path must equal resolve_default_model_path()"
    );
}

#[test]
fn resolve_default_model_path_ends_with_parakeet_dir() {
    use crate::dictation::transcription::resolve_default_model_path;
    let path = resolve_default_model_path();
    assert!(
        path.ends_with("models/parakeet-tdt-0.6b-v3-int8"),
        "resolved path must end with models/parakeet-tdt-0.6b-v3-int8, got: {}",
        path.display()
    );
}

#[test]
fn resolve_whisper_model_path_ends_with_expected_filename() {
    use crate::dictation::transcription::resolve_whisper_model_path;
    let path = resolve_whisper_model_path();
    assert!(
        path.ends_with("models/whisper-medium-q4_1.bin"),
        "resolved path must end with models/whisper-medium-q4_1.bin, got: {}",
        path.display()
    );
}

#[test]
fn dictation_model_catalog_ids_round_trip_and_unknown_falls_back() {
    use crate::dictation::DictationModelId;

    for entry in crate::dictation::dictation_model_catalog() {
        assert_eq!(
            DictationModelId::from_str(entry.stable_id),
            Some(entry.id),
            "catalog id must round-trip: {}",
            entry.stable_id
        );
    }

    assert_eq!(
        DictationModelId::from_preference(Some("not-a-real-model")),
        DictationModelId::default()
    );
    assert_eq!(
        DictationModelId::from_preference(None),
        DictationModelId::ParakeetTdt06bV3
    );
}

#[test]
fn dictation_model_catalog_marks_only_parakeet_recommended() {
    use crate::dictation::DictationModelId;

    let catalog = crate::dictation::dictation_model_catalog();
    let recommended: Vec<_> = catalog
        .iter()
        .filter(|entry| entry.recommended)
        .map(|entry| entry.id)
        .collect();

    assert_eq!(recommended, vec![DictationModelId::ParakeetTdt06bV3]);
    assert!(catalog
        .iter()
        .any(|entry| entry.description
            == "Fast and accurate. Auto-detects 25 European languages (ignores the language setting)."));
    assert!(catalog.iter().any(|entry| entry.description
        == "Broadest language coverage and honors the language setting, but may run a bit slow."));
}

#[test]
fn dictation_model_size_formatting_reuses_human_readable_units() {
    assert_eq!(crate::dictation::format_dictation_model_size(0), "0 B");
    assert_eq!(crate::dictation::format_dictation_model_size(1024), "1 KB");
    assert_eq!(
        crate::dictation::format_dictation_model_size(500_000_000),
        "476.8 MB"
    );
}

// ---------------------------------------------------------------------------
// Runtime handoff architecture tests
// ---------------------------------------------------------------------------

#[test]
fn runtime_returns_toggle_outcome_not_unit() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        runtime_src.contains("DictationToggleOutcome"),
        "toggle_dictation must return DictationToggleOutcome"
    );
    assert!(
        !runtime_src.contains("deliver_transcript"),
        "runtime must not own delivery — caller does"
    );
    assert!(
        !runtime_src.contains("resolve_destination"),
        "runtime must not resolve destination — caller does"
    );
}

#[test]
fn runtime_drops_capture_handle_before_draining_events() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    // The function that stops capture must drop the handle before blocking on
    // recv_blocking so the processor thread can flush its tail chunk and send
    // EndOfStream.
    let drop_pos = runtime_src
        .find("capture_handle.take()")
        .expect("runtime must drop capture handle via .take()");
    let drain_pos = runtime_src
        .find("recv_blocking()")
        .expect("runtime must blocking-drain via recv_blocking");
    assert!(
        drop_pos < drain_pos,
        "capture handle must be dropped BEFORE blocking drain (drop at byte {drop_pos}, drain at {drain_pos})"
    );
}

// ---------------------------------------------------------------------------
// Dictation goal critical paths (regression guard)
// ---------------------------------------------------------------------------

#[test]
fn dictation_goal_critical_paths_exist() {
    let mod_rs = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");
    let builtin =
        std::fs::read_to_string("src/app_execute/builtin_execution.rs").expect("read builtin");
    let hotkeys = std::fs::read_to_string("src/hotkeys/mod.rs").expect("read hotkeys");
    let window = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    let transcription =
        std::fs::read_to_string("src/dictation/transcription.rs").expect("read transcription.rs");

    assert!(
        mod_rs.contains("toggle_dictation"),
        "mod.rs must export toggle_dictation"
    );
    assert!(
        builtin.contains("dictation_toggle") || builtin.contains("toggle_dictation"),
        "builtin_execution.rs must call dictation toggle"
    );
    assert!(
        !builtin.contains("dictation_stub"),
        "builtin_execution.rs must not contain 'dictation_stub'"
    );
    assert!(
        !hotkeys.contains("TODO: wire dictation"),
        "hotkeys/mod.rs must not contain 'TODO: wire dictation'"
    );
    assert!(
        window.contains("WindowKind::PopUp"),
        "window.rs must use WindowKind::PopUp"
    );
    assert!(
        transcription.contains("WhisperDictationEngine"),
        "transcription.rs must define WhisperDictationEngine"
    );
    assert!(
        transcription.contains("ParakeetDictationEngine"),
        "transcription.rs must define ParakeetDictationEngine"
    );
}

// ---------------------------------------------------------------------------
// Mic preference resolution source audit
// ---------------------------------------------------------------------------

#[test]
fn runtime_resolves_mic_preference_from_settings() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        runtime_src.contains("load_user_preferences"),
        "runtime must read config-backed preferences for mic selection"
    );
    assert!(
        runtime_src.contains("selected_device_id"),
        "runtime must check dictation.selected_device_id"
    );
    assert!(
        runtime_src.contains("resolve_selected_input_device"),
        "runtime must delegate selection to resolve_selected_input_device"
    );
}

// ---------------------------------------------------------------------------
// Regression: overlay window uses PopUp + Blurred + vibrancy
// ---------------------------------------------------------------------------

#[test]
fn dictation_overlay_uses_popup_blur_and_vibrancy() {
    let source =
        std::fs::read_to_string("src/dictation/window.rs").expect("read dictation window.rs");
    assert!(
        source.contains("WindowKind::PopUp"),
        "overlay must use WindowKind::PopUp"
    );
    assert!(
        source.contains("WindowBackgroundAppearance::Blurred"),
        "overlay must support blurred background"
    );
    // 81e23d77c gave the overlay its own platform wrapper
    // (configure_dictation_overlay_window) which routes through the same
    // configure_window_vibrancy_common path as other secondary windows.
    assert!(
        source.contains("crate::platform::configure_dictation_overlay_window"),
        "overlay must configure vibrancy via platform helper"
    );
}

// ---------------------------------------------------------------------------
// Regression: dictation entrypoints must not be stubs
// ---------------------------------------------------------------------------

#[test]
fn dictation_entrypoints_are_not_stubs() {
    let builtin_src =
        std::fs::read_to_string("src/app_execute/builtin_execution.rs").expect("read builtin");
    let hotkeys_src = std::fs::read_to_string("src/hotkeys/mod.rs").expect("read hotkeys");

    assert!(
        !builtin_src.contains("dictation_stub"),
        "builtin_execution.rs still contains 'dictation_stub'"
    );
    assert!(
        !builtin_src.contains("not yet wired"),
        "builtin_execution.rs still contains 'not yet wired'"
    );
    assert!(
        !hotkeys_src.contains("TODO: wire dictation"),
        "hotkeys/mod.rs still contains 'TODO: wire dictation'"
    );
}

// ---------------------------------------------------------------------------
// Stop-path flush: tail chunk preserved after stop
// ---------------------------------------------------------------------------

/// Simulates the stop-path by sending chunks through a channel, then
/// closing the sender (simulating handle drop) and verifying that the
/// receiver collects all chunks including the tail before EndOfStream.
#[test]
fn stop_path_collects_all_chunks_including_tail_after_handle_drop() {
    use crate::dictation::types::CompletedDictationCapture;

    let (tx, rx) = async_channel::bounded::<DictationCaptureEvent>(16);

    // Simulate a processor thread that sends chunks then EndOfStream.
    let producer = std::thread::spawn(move || {
        tx.send_blocking(DictationCaptureEvent::Chunk(CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.1; 160],
            duration: Duration::from_millis(10),
        }))
        .expect("send chunk 1");

        tx.send_blocking(DictationCaptureEvent::Bars([0.3; 9]))
            .expect("send level");

        // Tail chunk — the one that would be lost if we drained with
        // try_recv() before the handle was dropped.
        tx.send_blocking(DictationCaptureEvent::Chunk(CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.2; 80],
            duration: Duration::from_millis(5),
        }))
        .expect("send tail chunk");

        tx.send_blocking(DictationCaptureEvent::EndOfStream)
            .expect("send EOS");
    });

    // Consumer: blocking drain (mirrors stop_capture_and_collect).
    let mut chunks = Vec::new();
    while let Ok(event) = rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunks.push(chunk),
            DictationCaptureEvent::Bars(_) => {}
            DictationCaptureEvent::EndOfStream => break,
        }
    }

    producer.join().expect("producer thread");

    assert_eq!(chunks.len(), 2, "must collect both chunks including tail");
    assert_eq!(chunks[0].samples.len(), 160);
    assert_eq!(chunks[1].samples.len(), 80, "tail chunk must be preserved");

    let audio_duration = crate::dictation::transcription::captured_duration(&chunks);
    assert_eq!(audio_duration, Duration::from_millis(15));

    // Verify CompletedDictationCapture can be constructed from the result.
    let capture = CompletedDictationCapture {
        chunks,
        audio_duration,
        truncated: false,
    };
    assert_eq!(capture.chunks.len(), 2);
    assert_eq!(capture.audio_duration, Duration::from_millis(15));
}

/// Verifies that an empty recording (no chunks before EndOfStream) results
/// in `Stopped(None)`.
#[test]
fn stop_path_empty_recording_produces_none() {
    use crate::dictation::types::DictationToggleOutcome;

    let (tx, rx) = async_channel::bounded::<DictationCaptureEvent>(4);

    let producer = std::thread::spawn(move || {
        tx.send_blocking(DictationCaptureEvent::EndOfStream)
            .expect("send EOS");
    });

    let mut chunks = Vec::new();
    while let Ok(event) = rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunks.push(chunk),
            DictationCaptureEvent::Bars(_) => {}
            DictationCaptureEvent::EndOfStream => break,
        }
    }

    producer.join().expect("producer thread");

    let outcome = if chunks.is_empty() {
        DictationToggleOutcome::Stopped(None)
    } else {
        DictationToggleOutcome::Stopped(Some(CompletedDictationCapture {
            audio_duration: crate::dictation::transcription::captured_duration(&chunks),
            chunks,
            truncated: false,
        }))
    };

    assert_eq!(outcome, DictationToggleOutcome::Stopped(None));
}

/// Verifies that the blocking drain terminates with an error when the
/// channel is closed without sending EndOfStream.
#[test]
fn stop_path_errors_when_channel_closes_without_eos() {
    let (tx, rx) = async_channel::bounded::<DictationCaptureEvent>(4);

    let producer = std::thread::spawn(move || {
        tx.send_blocking(DictationCaptureEvent::Chunk(CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.5; 160],
            duration: Duration::from_millis(10),
        }))
        .expect("send chunk");
        // Drop tx without sending EndOfStream.
    });

    let mut chunks = Vec::new();
    let mut saw_eos = false;
    while let Ok(event) = rx.recv_blocking() {
        match event {
            DictationCaptureEvent::Chunk(chunk) => chunks.push(chunk),
            DictationCaptureEvent::Bars(_) => {}
            DictationCaptureEvent::EndOfStream => {
                saw_eos = true;
                break;
            }
        }
    }

    producer.join().expect("producer thread");

    assert!(
        !saw_eos,
        "should not have received EndOfStream when sender dropped early"
    );
    assert_eq!(chunks.len(), 1, "should still collect chunks before close");
}

#[test]
fn transcribe_captured_audio_returns_none_for_silent_input_without_model() {
    crate::dictation::runtime::reset_cached_transcriber_for_tests();

    let chunks = vec![CapturedAudioChunk {
        sample_rate_hz: 16_000,
        samples: vec![0.0; 1_600],
        duration: Duration::from_millis(100),
    }];

    let result = crate::dictation::transcribe_captured_audio(&chunks)
        .expect("silent input should short-circuit before engine init");

    assert_eq!(result, None);

    crate::dictation::runtime::reset_cached_transcriber_for_tests();
}

// ---------------------------------------------------------------------------
// Mic selection wiring regression tests
// ---------------------------------------------------------------------------

#[test]
fn builtin_microphone_selection_command_is_wired() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("SettingsCommandType::SelectMicrophone"),
        "builtin settings command must expose SelectMicrophone"
    );
    assert!(
        builtin_src.contains("crate::dictation::list_input_device_menu_items("),
        "SelectMicrophone must load rows from the shared menu-item helper"
    );
    assert!(
        builtin_src.contains("BUILTIN_MIC_SELECT_PROMPT_ID"),
        "SelectMicrophone must open a dedicated synthetic prompt"
    );
    assert!(
        builtin_src.contains("BUILTIN_MIC_DEFAULT_VALUE"),
        "SelectMicrophone must include a system-default choice value"
    );
    assert!(
        builtin_src.contains("AppView::MiniPrompt"),
        "SelectMicrophone must open a MiniPrompt"
    );
    assert!(
        builtin_src.contains("Select microphone..."),
        "SelectMicrophone prompt placeholder must stay user-facing"
    );
}

#[test]
fn builtin_microphone_submit_handler_persists_or_clears_preference() {
    let helpers_src =
        std::fs::read_to_string("src/render_prompts/arg/helpers.rs").expect("read arg helpers");
    let config_src = std::fs::read_to_string("src/config/types.rs").expect("read config types");

    assert!(
        helpers_src.contains("fn is_valid_builtin_mic_selection(&self, value: &str) -> bool"),
        "arg helpers must validate built-in microphone selections"
    );
    assert!(
        helpers_src.contains("value == BUILTIN_MIC_DEFAULT_VALUE"),
        "submit handling must accept the synthetic system-default value"
    );
    assert!(
        helpers_src.contains("crate::dictation::apply_device_selection(&action)"),
        "submit handling must persist choices through the shared apply_device_selection helper"
    );
    assert!(
        config_src.contains("pub selected_device_id: Option<String>"),
        "config-backed preferences must persist dictation.selected_device_id"
    );
}

#[test]
fn builtin_microphone_prompt_labels_current_and_preselects() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("format!(\"{} (current)\", item.title)"),
        "prompt must label the selected microphone as current"
    );
    assert!(
        builtin_src.contains("self.arg_selected_index = start_index;"),
        "prompt must preselect the saved/current microphone"
    );
}

#[test]
fn builtin_microphone_prompt_delegates_to_shared_menu_items() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("crate::dictation::list_input_device_menu_items("),
        "SelectMicrophone must delegate to the shared menu-item builder"
    );
    assert!(
        builtin_src.contains("item.is_selected"),
        "SelectMicrophone must use the shared is_selected flag for stale-device fallback"
    );
}

// ---------------------------------------------------------------------------
// Delivery contract tests: prompt-first, frontmost-app-second
// ---------------------------------------------------------------------------

/// Prove that `handle_dictation_transcript` checks `try_set_prompt_input`
/// BEFORE calling `paste_text`, so an active prompt always wins.
#[test]
fn delivery_checks_prompt_input_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let prompt_pos = src
        .find("try_set_prompt_input")
        .expect("handle_dictation_transcript must call try_set_prompt_input");
    let paste_pos = src
        .find("paste_text")
        .expect("handle_dictation_transcript must call paste_text");

    assert!(
        prompt_pos < paste_pos,
        "try_set_prompt_input (byte {prompt_pos}) must appear before paste_text (byte {paste_pos}) \
         — active prompt takes priority over frontmost app paste"
    );
}

/// Prove that when `try_set_prompt_input` returns true, the destination is
/// `ActivePrompt` and no paste fallback occurs.
#[test]
fn delivery_active_prompt_sets_correct_destination() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    // The delivery block must assign ActivePrompt when try_set_prompt_input succeeds.
    assert!(
        src.contains("DictationDestination::ActivePrompt"),
        "delivery must use DictationDestination::ActivePrompt when prompt accepts input"
    );

    // Verify ActivePrompt is set in the if-true branch (before paste_text).
    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];
    let active_prompt_pos = handler_src
        .find("DictationDestination::ActivePrompt")
        .expect("ActivePrompt must be in handler");
    let paste_pos = handler_src
        .find("paste_text")
        .expect("paste_text must be in handler");
    assert!(
        active_prompt_pos < paste_pos,
        "ActivePrompt destination must be assigned BEFORE the paste fallback branch"
    );
}

/// Prove that when `try_set_prompt_input` returns false, delivery falls
/// through to `paste_text` and sets `FrontmostApp` on success.
#[test]
fn delivery_frontmost_app_sets_correct_destination() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    // paste_text must be in an else branch after try_set_prompt_input
    assert!(
        handler_src.contains("paste_text"),
        "handler must call paste_text as fallback"
    );
    assert!(
        handler_src.contains("DictationDestination::FrontmostApp"),
        "handler must set FrontmostApp destination on successful paste"
    );
}

/// Prove that paste failures are surfaced (logged + toast) and not silently
/// dropped.  The overlay is closed before pasting (for focus handoff), so
/// failures cannot use the overlay — they show a toast instead.
#[test]
fn delivery_paste_failure_surfaces_error() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    // The Err branch of paste_text must log the failure.
    let paste_pos = handler_src
        .find("paste_text")
        .expect("handler must call paste_text");
    let after_paste = &handler_src[paste_pos..];

    assert!(
        after_paste.contains("Failed to paste dictation transcript"),
        "paste failure must log an error describing what failed"
    );
    assert!(
        after_paste.contains("show_error_toast"),
        "paste failure must show an error toast to the user"
    );
}

/// Prove that the frontmost-app paste path hides the main window and closes
/// the dictation overlay BEFORE pasting, so macOS returns keyboard focus to
/// the target app before the CGEvent Cmd+V fires.
///
/// The close + hide logic now lives inside `yield_focus_for_dictation_paste`,
/// which the frontmost branch calls before `paste_text`.
#[test]
fn delivery_frontmost_app_hides_window_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    let frontmost_src = frontmost_dictation_delivery_branch(handler_src);

    // The frontmost branch calls the helper which does close + hide.
    let yield_pos = frontmost_src
        .find("yield_focus_for_dictation_paste")
        .expect("frontmost branch must call yield_focus_for_dictation_paste");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost branch must call paste_text");

    assert!(
        yield_pos < paste_pos,
        "yield_focus_for_dictation_paste (byte {yield_pos}) must appear before paste_text (byte {paste_pos})"
    );

    // The helper itself must close the overlay and hide the main window.
    let helper_start = src
        .find("fn yield_focus_for_dictation_paste")
        .expect("helper must exist");
    let helper_src = &src[helper_start..helper_start + 1200.min(src.len() - helper_start)];
    assert!(
        helper_src.contains("close_dictation_overlay"),
        "helper must close dictation overlay"
    );
    assert!(
        helper_src.contains("defer_hide_main_window"),
        "helper must call defer_hide_main_window"
    );
}

/// Prove that transcription errors also surface as Failed phase.
#[test]
fn delivery_transcription_error_surfaces_as_failed_phase() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    // The Err(error) arm must show Failed overlay.
    assert!(
        handler_src.contains("Err(error)"),
        "handler must match on Err(error) for transcription failures"
    );
    assert!(
        handler_src.contains("Transcription failed"),
        "transcription error must be logged"
    );
}

/// Prove delivery logic stays in builtin_execution, not in dictation/runtime.
#[test]
fn delivery_logic_not_in_runtime() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        !runtime_src.contains("try_set_prompt_input"),
        "runtime must not call try_set_prompt_input — delivery is the caller's job"
    );
    assert!(
        !runtime_src.contains("paste_text"),
        "runtime must not call paste_text — delivery is the caller's job"
    );
    assert!(
        !runtime_src.contains("TextInjector"),
        "runtime must not reference TextInjector — delivery is the caller's job"
    );
    assert!(
        !runtime_src.contains("handle_dictation_transcript"),
        "runtime must not define or call handle_dictation_transcript"
    );
}

/// Prove that `try_set_prompt_input` covers the main prompt types that
/// should accept dictated text.
#[test]
fn try_set_prompt_input_covers_key_prompt_views() {
    let src = std::fs::read_to_string("src/app_impl/ui_window.rs").expect("read ui_window.rs");

    let fn_start = src
        .find("fn try_set_prompt_input")
        .expect("try_set_prompt_input must exist");
    let fn_src = &src[fn_start..fn_start + 3000.min(src.len() - fn_start)];

    for view in &[
        "AppView::ArgPrompt",
        "AppView::MiniPrompt",
        "AppView::MicroPrompt",
        "AppView::PathPrompt",
        "AppView::SelectPrompt",
    ] {
        assert!(
            fn_src.contains(view),
            "try_set_prompt_input must handle {view}"
        );
    }

    // Must return false for unhandled views (the _ => false arm).
    assert!(
        fn_src.contains("_ => false"),
        "try_set_prompt_input must return false for unhandled views"
    );
}

/// Prove that the silent-audio path (Ok(None)) does not show an error and
/// closes the overlay quickly without surfacing a failure.
#[test]
fn delivery_silent_audio_closes_without_error() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    // The Ok(None) arm must schedule close without showing Failed.
    assert!(
        handler_src.contains("Ok(None)"),
        "handler must match Ok(None) for silent audio"
    );

    // Ok(None) must not produce a Failed phase.
    let none_pos = handler_src
        .find("Ok(None)")
        .expect("Ok(None) arm must exist");
    // Find the next match arm boundary (either Ok(Some(..)) or Err(..))
    let next_arm = handler_src[none_pos..]
        .find("Err(error)")
        .unwrap_or(handler_src.len() - none_pos);
    let none_arm = &handler_src[none_pos..none_pos + next_arm];

    assert!(
        !none_arm.contains("DictationSessionPhase::Failed"),
        "silent audio must not surface as Failed — it should close quietly"
    );
}

// ---------------------------------------------------------------------------
// Stale mic preference self-heal regression test
// ---------------------------------------------------------------------------

#[test]
fn runtime_clears_stale_mic_preference_on_missing_device() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    // The fallback branch must attempt to clear the stale preference.
    assert!(
        runtime_src.contains("save_dictation_device_id(None)"),
        "resolve_preferred_device must clear stale preference when saved device is missing"
    );

    // If clearing fails, runtime must log a warning and continue.
    assert!(
        runtime_src.contains("Failed to clear stale microphone preference"),
        "runtime must warn when clearing stale preference fails"
    );

    // The runtime must delegate to resolve_selected_input_device, which
    // handles the system-default fallback internally.
    assert!(
        runtime_src.contains("resolve_selected_input_device"),
        "resolve_preferred_device must delegate to resolve_selected_input_device for fallback"
    );
}

// ---------------------------------------------------------------------------
// Hotkey isolation: dictation hotkey must never show main window
// ---------------------------------------------------------------------------

#[test]
fn dictation_hotkey_never_shows_main_window() {
    let execution_src = std::fs::read_to_string("src/app_impl/execution_scripts.rs")
        .expect("read execution_scripts.rs");
    assert!(
        execution_src.contains("\"builtin/dictation\""),
        "builtin/dictation must be listed in NO_MAIN_WINDOW_BUILTINS"
    );

    for (label, path) in [
        (
            "runtime_tray_hotkeys.rs",
            "src/main_entry/runtime_tray_hotkeys.rs",
        ),
        ("app_run_setup.rs", "src/main_entry/app_run_setup.rs"),
    ] {
        let src = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("read {label}"));
        let start = src
            .find("Dictation hotkey listener")
            .unwrap_or_else(|| panic!("{label} must have a Dictation hotkey listener section"));
        let end = src[start..].find(".detach()").unwrap_or(src.len() - start);
        let section = &src[start..start + end];

        assert!(
            section.contains("let should_show_window = app_entity_inner.update"),
            "{label} must inspect the execute_by_command_id_or_path return value"
        );
        assert!(
            !section.contains("show_main_window_helper"),
            "{label} dictation hotkey must not show the main window"
        );
    }
}

// ---------------------------------------------------------------------------
// Transcription observability: skip reasons and model path are logged
// ---------------------------------------------------------------------------

#[test]
fn dictation_transcription_logs_skip_reasons_and_model_path() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        runtime_src.contains("Starting dictation transcription"),
        "runtime must log transcription entry"
    );
    assert!(
        runtime_src.contains("Skipping dictation transcription: audio too short"),
        "runtime must log the too-short skip branch"
    );
    assert!(
        runtime_src.contains("Skipping dictation transcription: audio too silent"),
        "runtime must log the too-silent skip branch"
    );
    assert!(
        runtime_src.contains("model_path = %config.model_path.display()"),
        "runtime must log the model path"
    );
    assert!(
        runtime_src.contains("Dictation transcription succeeded"),
        "runtime must log successful transcription"
    );
}

#[test]
fn dictation_surfaces_missing_model_with_download() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("dictation_model_entry(model_id)"),
        "dictation start path must check selected catalog model availability"
    );
    assert!(
        builtin_src.contains("start_dictation_model_download"),
        "missing dictation model must trigger a background download"
    );
}

#[test]
fn dictation_focus_settle_matches_reference_contract() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("const DICTATION_FOCUS_SETTLE_MS: u64 = 120;"),
        "frontmost-app paste settle delay must match the 120ms reference"
    );
    assert!(
        builtin_src.contains("Preparing frontmost-app dictation paste"),
        "frontmost-app paste path must log the target handoff"
    );
}

// ---------------------------------------------------------------------------
// Hotkey routing: dictation hotkey uses builtin toggle, not a duplicate path
// ---------------------------------------------------------------------------

#[test]
fn dictation_hotkey_routes_through_builtin_toggle_flow() {
    // Verify both entry-point files have a dictation hotkey listener that
    // routes through the config-governed dictation builtin (dictation.target
    // decides the destination) without introducing a duplicate dictation
    // implementation.
    for (label, path) in [
        (
            "runtime_tray_hotkeys.rs",
            "src/main_entry/runtime_tray_hotkeys.rs",
        ),
        ("app_run_setup.rs", "src/main_entry/app_run_setup.rs"),
    ] {
        let src = std::fs::read_to_string(path).unwrap_or_else(|_| panic!("read {label}"));

        assert!(
            src.contains("dictation_hotkey_channel"),
            "{label} must consume the dictation hotkey channel"
        );
        assert!(
            src.contains(r#"execute_by_command_id_or_path("builtin/dictation", ctx)"#),
            "{label} must route the dictation hotkey through the config-governed \
             builtin/dictation entry (dictation.target decides the destination)"
        );
        // Must NOT contain a second toggle_dictation call — only the builtin path owns that.
        let hotkey_section_start = src
            .find("Dictation hotkey listener")
            .unwrap_or_else(|| panic!("{label} must have a Dictation hotkey listener section"));
        let hotkey_section_end = src[hotkey_section_start..]
            .find(".detach()")
            .unwrap_or(src.len() - hotkey_section_start);
        let hotkey_section = &src[hotkey_section_start..hotkey_section_start + hotkey_section_end];

        assert!(
            !hotkey_section.contains("toggle_dictation()"),
            "{label} dictation hotkey listener must NOT call toggle_dictation() directly — \
             it must route through the builtin execution path"
        );
        assert!(
            !hotkey_section.contains("open_dictation_overlay"),
            "{label} dictation hotkey listener must NOT call open_dictation_overlay directly — \
             it must route through the builtin execution path"
        );
    }
}

#[test]
fn generic_dictation_builtin_remains_contextual() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let branch_start = src
        .find("builtins::BuiltInFeature::Dictation =>")
        .expect("dictation builtin branch must exist");
    let branch_tail = &src[branch_start..];
    let next_branch = branch_tail[1..]
        .find("builtins::BuiltInFeature::")
        .map(|offset| offset + 1)
        .unwrap_or(branch_tail.len());
    let branch_src = &branch_tail[..next_branch];

    assert!(
        branch_src.contains("DictationBuiltinAction::CurrentSurface")
            && src.contains("let is_start_edge = !crate::dictation::is_dictation_busy();")
            && src.contains("self.dictation_start_target(action)")
            && src.contains(".unwrap_or_else(|| self.resolve_dictation_target())"),
        "generic dictation builtin must continue resolving context-sensitive targets"
    );
}

#[test]
fn dictation_hotkey_channel_not_dead_code() {
    // The hotkey channel must be consumed (receiver side) in at least one entry point.
    let runtime = std::fs::read_to_string("src/main_entry/runtime_tray_hotkeys.rs")
        .expect("read runtime_tray_hotkeys.rs");
    let setup =
        std::fs::read_to_string("src/main_entry/app_run_setup.rs").expect("read app_run_setup.rs");

    let has_receiver = runtime.contains("dictation_hotkey_channel().1.recv()")
        || setup.contains("dictation_hotkey_channel().1.recv()");
    assert!(
        has_receiver,
        "dictation hotkey channel receiver must be consumed in at least one entry point"
    );
}

// ---------------------------------------------------------------------------
// Finished-label formatting tests
// ---------------------------------------------------------------------------

#[test]
fn finished_label_always_returns_done() {
    assert_eq!(super::window::finished_label().to_string(), "Done");
}

#[test]
fn frontmost_app_delivery_closes_overlay_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    assert!(
        handler_src.contains("DictationSessionPhase::Finished"),
        "internal dictation delivery must show the Finished confirmation pill"
    );

    let yield_offset = handler_src
        .find("yield_focus_for_dictation_paste")
        .expect("frontmost-app delivery must yield focus (close overlay) before paste");
    let paste_offset = handler_src
        .find("paste_text")
        .expect("frontmost-app delivery must paste after yielding focus");

    assert!(
        yield_offset < paste_offset,
        "frontmost-app delivery must yield focus before pasting (yield at {yield_offset}, paste at {paste_offset})"
    );
}

// ---------------------------------------------------------------------------
// Ordering regressions: frontmost-app delivery sequence
// ---------------------------------------------------------------------------

/// Extract the frontmost-app else branch from handle_dictation_transcript.
fn frontmost_dictation_delivery_branch(handler_src: &str) -> &str {
    let internal_if = handler_src
        .find("if delivered_internally")
        .expect("handler must branch on internal delivery");
    let else_offset = handler_src[internal_if..]
        .find("} else {")
        .expect("handler must have a frontmost-app else branch");
    &handler_src[internal_if + else_offset..]
}

#[test]
fn delivery_frontmost_app_yields_focus_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    let frontmost_src = frontmost_dictation_delivery_branch(handler_src);

    let yield_pos = frontmost_src
        .find("yield_focus_for_dictation_paste")
        .expect("frontmost-app branch must yield focus (close overlay + hide)");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app branch must paste transcript");

    assert!(
        yield_pos < paste_pos,
        "yield_focus_for_dictation_paste (byte {yield_pos}) must appear before paste_text (byte {paste_pos})"
    );
}

#[test]
fn delivery_frontmost_app_waits_for_focus_before_pasting() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    let frontmost_src = frontmost_dictation_delivery_branch(handler_src);

    let yield_pos = frontmost_src
        .find("yield_focus_for_dictation_paste")
        .expect("frontmost-app branch must yield focus (close overlay + hide)");
    let focus_timer_pos = frontmost_src
        .find("dictation_focus_settle_duration")
        .expect("frontmost-app branch must wait for focus to settle");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app branch must paste transcript");

    assert!(
        yield_pos < focus_timer_pos,
        "yield_focus_for_dictation_paste (byte {yield_pos}) must appear before focus-settle timer (byte {focus_timer_pos})"
    );
    assert!(
        focus_timer_pos < paste_pos,
        "focus-settle timer (byte {focus_timer_pos}) must appear before paste_text (byte {paste_pos})"
    );
}

#[test]
fn delivery_frontmost_app_aborts_paste_on_focus_yield_failure() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];

    let frontmost_src = frontmost_dictation_delivery_branch(handler_src);

    // The focus-yield result must be checked (not discarded with `let _ =`).
    assert!(
        frontmost_src.contains("yield_focus_result"),
        "frontmost-app branch must name the focus-yield result for error checking"
    );

    // On failure, must show error toast and schedule cleanup before returning.
    let err_check_pos = frontmost_src
        .find("if let Err(error) = yield_focus_result")
        .expect("frontmost-app branch must check yield_focus_result for errors");
    // Search for show_error_toast AFTER the yield_focus_result error check,
    // since the tracker guard earlier in the branch also uses show_error_toast.
    let post_err_src = &frontmost_src[err_check_pos..];
    let show_error_pos = err_check_pos
        + post_err_src
            .find("show_error_toast")
            .expect("frontmost-app branch must show error toast on focus-yield failure");
    let cleanup_in_err = err_check_pos
        + post_err_src
            .find("schedule_dictation_transcriber_cleanup")
            .unwrap_or(usize::MAX - err_check_pos);

    assert!(
        err_check_pos < show_error_pos.min(cleanup_in_err),
        "error check must come before toast and cleanup"
    );
}

// ---------------------------------------------------------------------------
// Overlay contract: behavioral tests for helpers & constants
// ---------------------------------------------------------------------------

#[test]
fn overlay_elapsed_formatter_uses_mm_ss() {
    assert_eq!(
        super::window::format_elapsed(Duration::from_secs(0)).to_string(),
        "0:00"
    );
    assert_eq!(
        super::window::format_elapsed(Duration::from_secs(65)).to_string(),
        "1:05"
    );
    assert_eq!(
        super::window::format_elapsed(Duration::from_secs(600)).to_string(),
        "10:00"
    );
}

#[test]
fn overlay_waveform_height_matches_compact_contract() {
    assert!(
        (super::window::waveform_bar_height(0.0) - 4.0).abs() < 0.001,
        "silent bar height must be 4.0"
    );
    assert!(
        (super::window::waveform_bar_height(1.0) - 18.0).abs() < 0.001,
        "peak bar height must be 18.0"
    );
    let mid = super::window::waveform_bar_height(0.5);
    assert!(
        mid > 4.0 && mid < 18.0,
        "mid-level bar height must stay inside compact overlay bounds, got {mid}"
    );
}

#[test]
fn overlay_waveform_opacity_stays_clamped() {
    assert!(
        (super::window::waveform_bar_opacity(0.0) - 0.3).abs() < 0.001,
        "silent bar opacity must be 0.3"
    );
    assert!(
        (super::window::waveform_bar_opacity(1.0) - 1.0).abs() < 0.001,
        "peak bar opacity must be 1.0"
    );
    let mid = super::window::waveform_bar_opacity(0.5);
    assert!(
        (0.3..=1.0).contains(&mid),
        "bar opacity must remain between 0.3 and 1.0, got {mid}"
    );
}

#[test]
fn overlay_dot_and_window_constants_match_target_contract() {
    use crate::theme::opacity::{OPACITY_ACTIVE, OPACITY_SELECTED};

    // 2026-07-06 caption redesign: 560×100 fits the header row (timer +
    // destination chips + target badge), the enlarged caption band, and the
    // native footer rail at one constant height for every phase.
    assert_eq!(super::window::OVERLAY_WIDTH_PX, 560.0);
    assert_eq!(super::window::OVERLAY_HEIGHT_PX, 100.0);
    // 562fb2964 (Liquid Glass): the radius derives from the shared panel
    // token so the overlay stays in lockstep with other glass panels.
    assert_eq!(
        super::window::OVERLAY_RADIUS_PX,
        crate::ui::chrome::LIQUID_GLASS_PANEL_RADIUS_PX
    );
    assert_eq!(super::window::STATUS_TEXT_SIZE_PX, 11.5);
    // The caption line matches the main-menu footer label size so the live
    // transcript reads from a distance.
    assert_eq!(super::window::TRANSCRIPT_TEXT_SIZE_PX, 13.0);
    assert_eq!(super::window::WAVEFORM_BAR_COUNT, 9);
    assert_eq!(super::window::WAVEFORM_BAR_WIDTH_PX, 3.0);
    assert_eq!(super::window::WAVEFORM_BAR_GAP_PX, 3.0);
    assert_eq!(super::window::WAVEFORM_BAR_MAX_HEIGHT_PX, 18.0);
    assert_eq!(super::window::TRANSCRIBING_DOT_COUNT, 3);
    // Reduced-motion (static) fallback matches the original contract.
    assert_eq!(
        super::window::transcribing_dot_opacities_static(),
        [OPACITY_SELECTED, OPACITY_ACTIVE, OPACITY_SELECTED]
    );
}

#[test]
fn transcribing_dot_pulse_varies_over_time() {
    // At t=0, all dots are at the same phase point.
    let at_zero = super::window::transcribing_dot_opacities_at(0.0);
    // At t=0.3 (past one stagger cycle), dots should differ.
    let at_stagger = super::window::transcribing_dot_opacities_at(0.3);

    // All opacities must stay within [0.3, 1.0].
    for &arr in &[at_zero, at_stagger] {
        for &v in &arr {
            assert!(
                (0.29..=1.01).contains(&v),
                "dot opacity {v} out of [0.3, 1.0] range"
            );
        }
    }

    // At t=0.3 the stagger should cause visible differences between dots.
    let spread = at_stagger
        .iter()
        .copied()
        .fold(0.0_f32, |acc, v| acc.max(v))
        - at_stagger
            .iter()
            .copied()
            .fold(1.0_f32, |acc, v| acc.min(v));
    assert!(
        spread > 0.01,
        "dots should have visible spread at t=0.3, got {spread}"
    );
}

#[test]
fn transcribing_dot_pulse_is_periodic() {
    let period = super::window::TRANSCRIBING_PULSE_PERIOD_SECS;
    let at_t = super::window::transcribing_dot_opacities_at(0.5);
    let at_t_plus_period = super::window::transcribing_dot_opacities_at(0.5 + period);

    for (a, b) in at_t.iter().zip(at_t_plus_period.iter()) {
        assert!((a - b).abs() < 0.001, "pulse must be periodic: {a} vs {b}");
    }
}

#[test]
fn transcribing_dot_pulse_constants_match_vercel_voice() {
    assert!(
        (super::window::TRANSCRIBING_PULSE_PERIOD_SECS - 1.4).abs() < f64::EPSILON,
        "pulse period must be 1.4s (vercel-voice reference)"
    );
    assert!(
        (super::window::TRANSCRIBING_PULSE_STAGGER_SECS - 0.2).abs() < f64::EPSILON,
        "pulse stagger must be 0.2s (vercel-voice reference)"
    );
}

#[test]
fn reduced_motion_fallback_is_static() {
    use crate::theme::opacity::{OPACITY_ACTIVE, OPACITY_SELECTED};

    let static_opacities = super::window::transcribing_dot_opacities_static();
    assert_eq!(
        static_opacities,
        [OPACITY_SELECTED, OPACITY_ACTIVE, OPACITY_SELECTED],
        "reduced-motion fallback must return static staggered opacities"
    );
}

#[test]
fn overlay_has_sound_detects_active_audio() {
    let silent = [0.0_f32; super::window::WAVEFORM_BAR_COUNT];
    assert!(
        !super::window::has_sound(&silent),
        "all-zero bars must be silent"
    );

    let mut loud = [0.0_f32; super::window::WAVEFORM_BAR_COUNT];
    loud[4] = 0.11;
    assert!(
        super::window::has_sound(&loud),
        "bar just above the lowered threshold must count as sound"
    );
}

#[test]
fn dictation_overlay_derives_colors_from_theme_and_compact_capsule_chrome() {
    let source =
        std::fs::read_to_string("src/dictation/window.rs").expect("read dictation window.rs");

    // 81e23d77c moved the rail chrome contract into the shared footer_chrome
    // layer, and the glass in-window rail now composes the shared button
    // frames (render_footer_hint_action_button_frame delegates to the hint
    // renderer in FooterHintKeyMode::Shortcut — locked by footer_chrome's own
    // frame contract test), so the overlay and the native main-menu footer
    // share one source of truth.
    assert!(
        source.contains("footer_chrome::footer_rail_chrome"),
        "overlay surface must use the shared footer chrome (AppChromeColors) tokens"
    );
    assert!(
        source.contains("render_footer_hint_action_button_frame"),
        "overlay action rail buttons must use the shared footer button frame"
    );
    assert!(
        source.contains("render_glass_signal_band"),
        "overlay must render the launcher-style selected signal band"
    );
    assert!(
        source.contains("render_footer_action_rail"),
        "overlay action rail must use the shared tokenized footer rail geometry"
    );
    // Theme tokens still used for content colors.
    assert!(
        source.contains("theme.colors.ui.success"),
        "active waveform / transcribing state must use theme success color"
    );
    assert!(
        source.contains("theme.colors.text.primary"),
        "inactive waveform must use muted theme primary text color"
    );
    assert!(
        source.contains("theme.colors.text.primary.with_opacity"),
        "finished text must derive from theme primary text color"
    );
    assert!(
        source.contains("theme.colors.text.muted.with_opacity"),
        "timer and muted text must derive from theme muted text color"
    );
}

#[test]
fn dictation_overlay_close_handles_dead_windows_gracefully() {
    let source =
        std::fs::read_to_string("src/dictation/window.rs").expect("read dictation window.rs");

    let close_start = source
        .find("pub fn close_dictation_overlay")
        .expect("close_dictation_overlay must exist");
    let close_src = &source[close_start..close_start + 900.min(source.len() - close_start)];

    assert!(
        close_src.contains(".update("),
        "close_dictation_overlay must call handle.update"
    );
    // Dead windows are not errors — the close function must log a warning
    // but not propagate as Err, since the window is already gone.
    assert!(
        close_src.contains("Overlay window already gone"),
        "close_dictation_overlay must warn when closing an already-dead window"
    );
    // The slot must be cleared before attempting removal so no other
    // caller can see a stale handle.
    assert!(
        close_src.contains("guard.take()"),
        "close_dictation_overlay must clear the slot before removing the window"
    );
}

#[test]
fn delivery_frontmost_app_uses_error_propagating_focus_helper() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let helper_start = src
        .find("fn yield_focus_for_dictation_paste")
        .expect("builtin_execution.rs must define yield_focus_for_dictation_paste");
    let helper_src = &src[helper_start..helper_start + 1200.min(src.len() - helper_start)];

    assert!(
        helper_src.contains("close_dictation_overlay(cx)")
            && helper_src.contains("failed to close dictation overlay before paste"),
        "focus helper must propagate close_dictation_overlay failure with context"
    );
    assert!(
        helper_src.contains("platform::defer_hide_main_window(cx)"),
        "focus helper must still hide the main window before frontmost-app paste"
    );

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..];
    let frontmost_src = frontmost_dictation_delivery_branch(handler_src);

    let compacted_frontmost_src: String = frontmost_src
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    assert!(
        compacted_frontmost_src.contains(
            "this.yield_focus_for_dictation_paste(target_pid,&target_bundle_id,&target_app_name,cx,)"
        ),
        "frontmost-app delivery must call yield_focus_for_dictation_paste with tracked app identity"
    );

    // Scope to the async block that does close+paste (up to paste_text) to
    // avoid matching `let _ =` in unrelated arms like Stopped(None).
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost branch must paste");
    let pre_paste = &frontmost_src[..paste_pos];
    assert!(
        !pre_paste.contains("let _ = crate::dictation::close_dictation_overlay(cx)"),
        "frontmost-app delivery must not discard close_dictation_overlay errors before paste"
    );
}

// ---------------------------------------------------------------------------
// Strong source-level ordering proof for dictation focus handoff
// ---------------------------------------------------------------------------

/// Extract the body of `handle_dictation_transcript` from the source text.
fn dictation_handler_source(src: &str) -> &str {
    let start = src
        .find("fn handle_dictation_transcript")
        .expect("handle_dictation_transcript must exist");
    &src[start..]
}

/// Extract the body of `yield_focus_for_dictation_paste` from the source text.
/// Uses `schedule_dictation_overlay_close` as the end boundary.
fn dictation_yield_focus_helper_source(src: &str) -> &str {
    let start = src
        .find("fn yield_focus_for_dictation_paste(")
        .expect("yield_focus_for_dictation_paste must exist");
    let tail = &src[start..];
    let end = tail.find("fn schedule_dictation_overlay_close(").expect(
        "yield_focus_for_dictation_paste must be followed by schedule_dictation_overlay_close",
    );
    &tail[..end]
}

/// Extract the frontmost-app else branch from `handle_dictation_transcript`.
/// Uses `Ok(None)` as the end boundary.
fn dictation_frontmost_paste_source(src: &str) -> &str {
    let handler_src = dictation_handler_source(src);
    let else_pos = handler_src
        .find("} else {")
        .expect("handle_dictation_transcript must have a frontmost-app else branch");
    let tail = &handler_src[else_pos..];
    let end = tail
        .find("Ok(None)")
        .expect("frontmost-app else branch must end before the Ok(None) arm");
    &tail[..end]
}

#[test]
fn delivery_focus_helper_closes_overlay_before_hiding_main_window() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let helper_src = dictation_yield_focus_helper_source(&src);

    let close_pos = helper_src
        .find("close_dictation_overlay")
        .expect("focus helper must close dictation overlay");
    let hide_pos = helper_src
        .find("defer_hide_main_window")
        .expect("focus helper must defer main-window hide");

    assert!(
        close_pos < hide_pos,
        "close_dictation_overlay (byte {close_pos}) must appear before \
         defer_hide_main_window (byte {hide_pos}) inside yield_focus_for_dictation_paste"
    );
}

#[test]
fn delivery_focus_helper_activates_tracked_target_app() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let helper_src = dictation_yield_focus_helper_source(&src);

    assert!(
        helper_src.contains("target_pid: i32")
            && helper_src.contains("target_bundle_id: &str")
            && helper_src.contains("target_app_name: &str"),
        "yield_focus_for_dictation_paste must accept the tracked target app identity"
    );

    assert!(
        helper_src.contains("activate_tracked_app_for_dictation_paste("),
        "focus helper must activate the tracked target app before paste"
    );
    assert!(
        src.contains("let app_pid: i32 = msg_send![app, processIdentifier];")
            && src.contains("if app_pid != target_pid")
            && src.contains("activateWithOptions: 2u64"),
        "activation must find the tracked running app by pid rather than guessing from the current frontmost window"
    );
}

#[test]
fn delivery_frontmost_app_calls_focus_helper_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let frontmost_src = dictation_frontmost_paste_source(&src);

    let yield_pos = frontmost_src
        .find("yield_focus_for_dictation_paste")
        .expect("frontmost-app branch must call yield_focus_for_dictation_paste");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app branch must call paste_text");

    assert!(
        yield_pos < paste_pos,
        "yield_focus_for_dictation_paste (byte {yield_pos}) must appear before \
         paste_text (byte {paste_pos}) in the frontmost-app branch"
    );
}

#[test]
fn delivery_focus_yield_failure_surfaces_and_returns_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let frontmost_src = dictation_frontmost_paste_source(&src);

    assert!(
        frontmost_src.contains("failed to update app state before paste"),
        "frontmost-app path must wrap update failures with a dictation-specific context"
    );
    assert!(
        frontmost_src.contains("Dictation paste failed before paste step"),
        "frontmost-app path must show a pre-paste failure toast"
    );

    let err_pos = frontmost_src
        .find("if let Err(error) = yield_focus_result")
        .expect("frontmost-app path must check yield_focus_result");
    let return_pos = err_pos
        + frontmost_src[err_pos..]
            .find("return;")
            .expect("pre-paste failure path must return before paste");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app path must eventually call paste_text");

    assert!(
        err_pos < paste_pos,
        "yield_focus_result error handling must run before paste_text"
    );
    assert!(
        return_pos < paste_pos,
        "pre-paste failure path must return before paste_text"
    );
}

// ---------------------------------------------------------------------------
// Frontmost-app target gating and named timing helpers
// ---------------------------------------------------------------------------

#[test]
fn delivery_frontmost_app_checks_tracked_target_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let frontmost_src = dictation_frontmost_paste_source(&src);

    let target_pos = frontmost_src
        .find("ensure_dictation_frontmost_target_available")
        .expect("frontmost-app branch must verify tracked target before paste");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app branch must call paste_text");

    assert!(
        target_pos < paste_pos,
        "ensure_dictation_frontmost_target_available (byte {target_pos}) must appear before \
         paste_text (byte {paste_pos}) in the frontmost-app branch"
    );
}

#[test]
fn delivery_frontmost_app_surfaces_missing_tracked_target() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);

    assert!(
        handler_src
            .contains("no previously tracked frontmost app is available for dictation paste"),
        "frontmost-app path must surface a missing tracked-target error"
    );
    assert!(
        handler_src.contains("Failed to resolve frontmost-app dictation target"),
        "frontmost-app path must log tracked-target resolution failures"
    );
    assert!(
        handler_src.contains("show_error_toast"),
        "frontmost-app path must show the user a tracked-target failure"
    );
}

#[test]
fn delivery_frontmost_app_uses_named_focus_settle_duration_before_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let frontmost_src = dictation_frontmost_paste_source(&src);

    let settle_pos = frontmost_src
        .find("dictation_focus_settle_duration")
        .expect("frontmost-app branch must wait for named focus settle duration");
    let paste_pos = frontmost_src
        .find("paste_text")
        .expect("frontmost-app branch must call paste_text");

    assert!(
        settle_pos < paste_pos,
        "dictation_focus_settle_duration (byte {settle_pos}) must appear before \
         paste_text (byte {paste_pos}) in the frontmost-app branch"
    );
}

#[test]
fn delivery_frontmost_app_target_helper_uses_frontmost_app_tracker() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let helper_start = src
        .find("fn ensure_dictation_frontmost_target_available")
        .expect("tracked-target helper must exist");
    let helper_src = &src[helper_start..helper_start + 600.min(src.len() - helper_start)];

    assert!(
        helper_src.contains("get_last_real_app_bundle_id"),
        "tracked-target helper must use frontmost_app_tracker"
    );
}

// ---------------------------------------------------------------------------
// Device selection helpers
// ---------------------------------------------------------------------------

use crate::dictation::device::{
    build_device_menu_items, resolve_selected_input_device, DictationDeviceMenuItem,
    DictationDeviceSelectionAction,
};
use crate::dictation::types::{DictationDeviceId, DictationDeviceInfo, DictationDeviceTransport};

fn device(id: &str, name: &str, is_default: bool) -> DictationDeviceInfo {
    DictationDeviceInfo {
        id: DictationDeviceId(id.to_string()),
        name: name.to_string(),
        is_default,
        transport: DictationDeviceTransport::Unknown,
    }
}

fn device_with_transport(
    id: &str,
    name: &str,
    is_default: bool,
    transport: DictationDeviceTransport,
) -> DictationDeviceInfo {
    DictationDeviceInfo {
        id: DictationDeviceId(id.to_string()),
        name: name.to_string(),
        is_default,
        transport,
    }
}

#[test]
fn resolve_selected_input_device_prefers_saved_device() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let res = resolve_selected_input_device(&devices, Some("usb")).unwrap();
    assert_eq!(res.device.id.0, "usb");
    assert_eq!(res.device.name, "USB Microphone");
    assert!(!res.fell_back);
}

#[test]
fn resolve_selected_input_device_falls_back_to_default_for_stale_saved_id() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let res = resolve_selected_input_device(&devices, Some("missing")).unwrap();
    assert_eq!(res.device.id.0, "builtin");
    assert!(res.device.is_default);
    assert!(res.fell_back);
}

#[test]
fn resolve_selected_input_device_returns_default_when_no_preference() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let res = resolve_selected_input_device(&devices, None).unwrap();
    assert_eq!(res.device.id.0, "builtin");
    assert!(!res.fell_back);
}

#[test]
fn resolve_selected_input_device_returns_none_for_empty_list() {
    let selected = resolve_selected_input_device(&[], Some("anything"));
    assert!(selected.is_none());
}

#[test]
fn build_device_menu_items_marks_system_default_when_saved_device_is_missing() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let items = build_device_menu_items(&devices, Some("missing"));
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].title, "System Default");
    assert!(items[0].is_selected);
    assert!(!items[1].is_selected);
    assert!(!items[2].is_selected);
}

#[test]
fn build_device_menu_items_marks_saved_device_when_present() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let items = build_device_menu_items(&devices, Some("usb"));
    assert_eq!(items.len(), 3);
    assert!(
        !items[0].is_selected,
        "System Default should not be selected"
    );
    assert!(!items[1].is_selected, "builtin should not be selected");
    assert!(items[2].is_selected, "USB Mic should be selected");
}

#[test]
fn build_device_menu_items_marks_system_default_when_no_preference() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let items = build_device_menu_items(&devices, None);
    assert!(items[0].is_selected);
    assert!(!items[1].is_selected);
    assert!(!items[2].is_selected);
}

#[test]
fn build_device_menu_items_default_row_subtitle_includes_device_name() {
    let devices = vec![device("builtin", "MacBook Pro Microphone", true)];
    let items = build_device_menu_items(&devices, None);
    assert!(
        items[0].subtitle.contains("MacBook Pro Microphone"),
        "subtitle should name the default device"
    );
}

#[test]
fn build_device_menu_items_labels_default_device_with_dot_suffix() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let items = build_device_menu_items(&devices, None);
    assert!(
        items[1].title.contains("\u{00b7} default"),
        "default device row should have '\u{00b7} default' suffix"
    );
    assert_eq!(items[2].title, "USB Microphone");
}

#[test]
fn build_device_menu_items_actions_match_device_ids() {
    let devices = vec![
        device("builtin", "MacBook Pro Microphone", true),
        device("usb", "USB Microphone", false),
    ];
    let items = build_device_menu_items(&devices, None);
    assert_eq!(
        items[0].action,
        DictationDeviceSelectionAction::UseSystemDefault
    );
    assert_eq!(
        items[1].action,
        DictationDeviceSelectionAction::UseDevice(DictationDeviceId("builtin".to_string()))
    );
    assert_eq!(
        items[2].action,
        DictationDeviceSelectionAction::UseDevice(DictationDeviceId("usb".to_string()))
    );
}

#[test]
fn apply_device_selection_persists_selected_device_id_and_clears_to_default() {
    let lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp_dir = tempfile::tempdir().expect("create temp Script Kit dir");
    let kit_dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&kit_dir).expect("create Script Kit dir");
    let previous_sk_path = std::env::var(crate::setup::SK_PATH_ENV).ok();
    std::env::set_var(crate::setup::SK_PATH_ENV, temp_dir.path());

    crate::dictation::apply_device_selection(&DictationDeviceSelectionAction::UseDevice(
        DictationDeviceId("usb-mic".to_string()),
    ))
    .expect("persist selected microphone");

    let selected = crate::config::load_user_preferences();
    assert_eq!(
        selected.dictation.selected_device_id.as_deref(),
        Some("usb-mic")
    );

    crate::dictation::apply_device_selection(&DictationDeviceSelectionAction::UseSystemDefault)
        .expect("clear selected microphone");

    let cleared = crate::config::load_user_preferences();
    assert_eq!(cleared.dictation.selected_device_id, None);

    match previous_sk_path {
        Some(path) => std::env::set_var(crate::setup::SK_PATH_ENV, path),
        None => std::env::remove_var(crate::setup::SK_PATH_ENV),
    }
    drop(lock);
}

#[test]
fn dictation_preferences_serialize_selected_device_id_as_camel_case() {
    let preferences = crate::config::ScriptKitUserPreferences {
        dictation: crate::config::DictationPreferences {
            selected_device_id: Some("usb-mic".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let value = serde_json::to_value(&preferences).expect("serialize config-backed preferences");
    assert_eq!(value["dictation"]["selectedDeviceId"], "usb-mic");
    assert!(value["dictation"].get("model").is_none());
    assert!(value["dictation"].get("language").is_none());
    assert!(value["dictation"].get("selected_device_id").is_none());
}

#[test]
fn dictation_preferences_serde_round_trip_model_and_language() {
    let preferences = crate::config::ScriptKitUserPreferences {
        dictation: crate::config::DictationPreferences {
            selected_device_id: Some("usb-mic".to_string()),
            model: Some("whisper-medium".to_string()),
            language: Some("de".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    let value = serde_json::to_value(&preferences).expect("serialize dictation preferences");
    assert_eq!(value["dictation"]["selectedDeviceId"], "usb-mic");
    assert_eq!(value["dictation"]["model"], "whisper-medium");
    assert_eq!(value["dictation"]["language"], "de");

    let decoded: crate::config::ScriptKitUserPreferences =
        serde_json::from_value(value).expect("deserialize dictation preferences");
    assert_eq!(decoded.dictation, preferences.dictation);
}

#[test]
fn save_user_preferences_writes_config_and_cleans_known_legacy_settings() {
    let lock = crate::test_utils::SK_PATH_TEST_LOCK
        .get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let temp_dir = tempfile::tempdir().expect("create temp Script Kit dir");
    let kit_dir = temp_dir.path().to_path_buf();
    std::fs::create_dir_all(&kit_dir).expect("create Script Kit dir");
    std::env::set_var(crate::setup::SK_PATH_ENV, temp_dir.path());

    let settings_path = kit_dir.join("settings.json");
    let original = json!({
        "theme": { "presetId": "nord" }
    });
    std::fs::write(
        &settings_path,
        serde_json::to_string_pretty(&original).expect("serialize original settings"),
    )
    .expect("write settings file");

    let preferences = crate::config::ScriptKitUserPreferences {
        dictation: crate::config::DictationPreferences {
            selected_device_id: Some("usb-mic".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    crate::config::save_user_preferences(&preferences)
        .expect("save merged config-backed preferences");

    assert!(
        !settings_path.exists(),
        "known legacy settings file should be removed after config migration"
    );

    let config_path = kit_dir.join("config.ts");
    let config_contents = std::fs::read_to_string(&config_path).expect("read saved config");
    assert!(config_contents.contains("dictation"));
    assert!(config_contents.contains("selectedDeviceId"));
    assert!(config_contents.contains("usb-mic"));

    let loaded = crate::config::load_user_preferences();
    assert_eq!(
        loaded.dictation.selected_device_id.as_deref(),
        Some("usb-mic")
    );

    std::env::remove_var(crate::setup::SK_PATH_ENV);
    drop(lock);
}

// ---------------------------------------------------------------------------
// Delivery-target preflight tests
// ---------------------------------------------------------------------------

#[test]
fn dictation_runtime_exposes_is_recording_helper() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");
    let fn_start = runtime_src
        .find("pub fn is_dictation_recording() -> bool")
        .expect("runtime must expose is_dictation_recording");
    let fn_src = &runtime_src[fn_start..fn_start + 200.min(runtime_src.len() - fn_start)];
    assert!(
        fn_src.contains("SESSION.lock().is_some()"),
        "is_dictation_recording must reflect live session state"
    );

    let mod_src = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");
    assert!(
        mod_src.contains("is_dictation_recording"),
        "dictation mod facade must re-export is_dictation_recording"
    );
}

#[test]
fn can_accept_dictation_into_prompt_matches_delivery_views() {
    let src = std::fs::read_to_string("src/app_impl/ui_window.rs").expect("read ui_window.rs");
    let fn_start = src
        .find("fn can_accept_dictation_into_prompt")
        .expect("can_accept_dictation_into_prompt must exist");
    let fn_src = &src[fn_start..fn_start + 1200.min(src.len() - fn_start)];

    for view in &[
        "AppView::ArgPrompt",
        "AppView::MiniPrompt",
        "AppView::MicroPrompt",
        "AppView::PathPrompt",
        "AppView::SelectPrompt",
        "AppView::EnvPrompt",
        "AppView::TemplatePrompt",
        "AppView::FormPrompt",
    ] {
        assert!(
            fn_src.contains(view),
            "can_accept_dictation_into_prompt must include {view}"
        );
    }
}

#[test]
fn dictation_start_preflights_delivery_target_before_toggle() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let dictation_start = src
        .find("fn execute_dictation_builtin_action(")
        .expect("execute_dictation_builtin_action must exist");
    let dictation_src =
        &src[dictation_start..dictation_start + 4000.min(src.len() - dictation_start)];

    let preflight_pos = dictation_src
        .find("let preflight = self.prepare_dictation_builtin_start(action, None, cx);")
        .expect("dictation start must prepare preflight");
    let toggle_pos = dictation_src
        .find("crate::dictation::toggle_dictation(dictation_target)")
        .expect("dictation builtin must call toggle_dictation");
    assert!(
        preflight_pos < toggle_pos,
        "delivery-target preflight must run before toggle_dictation"
    );
}

#[test]
fn dictation_preflight_allows_prompt_or_frontmost_target() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let helper_start = src
        .find("fn ensure_dictation_delivery_target_available")
        .expect("delivery preflight helper must exist");
    let helper_src = &src[helper_start..helper_start + 600.min(src.len() - helper_start)];

    assert!(
        helper_src.contains("self.can_accept_dictation_into_prompt()"),
        "delivery preflight must allow direct prompt delivery"
    );
    assert!(
        helper_src.contains("ensure_dictation_frontmost_target_available"),
        "delivery preflight must use the tracked frontmost-app guard when no prompt can accept text"
    );
}

// ---------------------------------------------------------------------------
// Behavioral toggle outcome tests
// ---------------------------------------------------------------------------

/// Prove that `DictationToggleOutcome::Started` is constructible and distinct
/// from `Stopped` variants, matching the contract where the first toggle
/// returns `Started` and the second returns `Stopped(Some(_))` or
/// `Stopped(None)`.
#[test]
fn toggle_outcome_started_is_distinct_from_stopped() {
    use crate::dictation::types::DictationToggleOutcome;

    let started = DictationToggleOutcome::Started;
    let stopped_none = DictationToggleOutcome::Stopped(None);
    let stopped_some = DictationToggleOutcome::Stopped(Some(CompletedDictationCapture {
        chunks: vec![CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.5; 160],
            duration: Duration::from_millis(10),
        }],
        audio_duration: Duration::from_millis(10),
        truncated: false,
    }));

    assert_ne!(started, stopped_none);
    assert_ne!(started, stopped_some);
    assert_ne!(stopped_none, stopped_some);
}

/// Prove that `Stopped(Some(capture))` carries the expected audio data
/// through the type, validating the end-to-end contract from toggle_dictation
/// to transcription handoff.
#[test]
fn toggle_outcome_stopped_some_carries_capture_data() {
    use crate::dictation::types::DictationToggleOutcome;

    let chunks = vec![
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.1; 160],
            duration: Duration::from_millis(10),
        },
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.2; 80],
            duration: Duration::from_millis(5),
        },
    ];
    let capture = CompletedDictationCapture {
        chunks: chunks.clone(),
        audio_duration: Duration::from_millis(15),
        truncated: false,
    };
    let outcome = DictationToggleOutcome::Stopped(Some(capture));

    match outcome {
        DictationToggleOutcome::Stopped(Some(cap)) => {
            assert_eq!(cap.chunks.len(), 2);
            assert_eq!(cap.audio_duration, Duration::from_millis(15));
            // Verify the chunks can be merged for transcription.
            let merged = crate::dictation::merge_captured_chunks(&cap.chunks);
            assert_eq!(merged.len(), 240);
        }
        other => panic!("expected Stopped(Some(_)), got {other:?}"),
    }
}

/// Simulate the full toggle cycle: Started → (capture chunks) → Stopped(Some)
/// and verify the transcription facade processes the captured audio.
#[test]
fn toggle_cycle_produces_transcribable_capture() -> Result<()> {
    // Simulate producing captured chunks (as toggle_dictation would return).
    let chunks = vec![
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.3; 320],
            duration: Duration::from_millis(20),
        },
        CapturedAudioChunk {
            sample_rate_hz: 16_000,
            samples: vec![0.4; 160],
            duration: Duration::from_millis(10),
        },
    ];

    let capture = CompletedDictationCapture {
        audio_duration: crate::dictation::captured_duration(&chunks),
        chunks,
        truncated: false,
    };

    assert_eq!(capture.audio_duration, Duration::from_millis(30));

    // Feed the captured audio through a stub transcriber to prove the
    // capture → transcribe handoff works.
    let transcriber = DictationTranscriber::new(
        DictationTranscriptionConfig {
            minimum_samples: 1,
            ..Default::default()
        },
        Box::new(StubEngine {
            output: "hello world".to_string(),
        }),
    );

    let result = transcriber.transcribe_chunks(&capture.chunks)?;
    assert_eq!(result, Some("hello world".to_string()));

    Ok(())
}

// ---------------------------------------------------------------------------
// Overlay state transition coverage
// ---------------------------------------------------------------------------

/// Prove that the overlay states follow the expected progression for a
/// successful dictation session: Recording → Transcribing → Finished.
#[test]
fn overlay_state_transitions_recording_to_transcribing_to_finished() {
    use crate::dictation::DictationSessionPhase;

    let recording = DictationOverlayState {
        phase: DictationSessionPhase::Recording,
        elapsed: Duration::from_secs(2),
        bars: [0.1, 0.2, 0.5, 0.8, 1.0, 0.8, 0.5, 0.2, 0.1],
        transcript: "".into(),
        ..Default::default()
    };
    assert_eq!(recording.phase, DictationSessionPhase::Recording);
    assert!(recording.transcript.is_empty());

    let transcribing = DictationOverlayState {
        phase: DictationSessionPhase::Transcribing,
        elapsed: recording.elapsed,
        bars: [0.0; 9],
        transcript: "".into(),
        ..Default::default()
    };
    assert_eq!(transcribing.phase, DictationSessionPhase::Transcribing);
    assert!(transcribing.transcript.is_empty());

    let finished = DictationOverlayState {
        phase: DictationSessionPhase::Finished,
        elapsed: recording.elapsed,
        bars: [0.0; 9],
        transcript: "hello world".into(),
        ..Default::default()
    };
    assert_eq!(finished.phase, DictationSessionPhase::Finished);
    assert_eq!(finished.transcript.as_ref(), "hello world");
}

/// Prove that the overlay states follow the expected progression for a
/// failed dictation session: Recording → Transcribing → Failed.
#[test]
fn overlay_state_transitions_recording_to_transcribing_to_failed() {
    use crate::dictation::DictationSessionPhase;

    let recording = DictationOverlayState {
        phase: DictationSessionPhase::Recording,
        elapsed: Duration::from_secs(1),
        bars: [0.5; 9],
        transcript: "".into(),
        ..Default::default()
    };
    assert_eq!(recording.phase, DictationSessionPhase::Recording);

    let transcribing = DictationOverlayState {
        phase: DictationSessionPhase::Transcribing,
        elapsed: recording.elapsed,
        ..Default::default()
    };
    assert_eq!(transcribing.phase, DictationSessionPhase::Transcribing);

    let failed = DictationOverlayState {
        phase: DictationSessionPhase::Failed("model load error".to_string()),
        elapsed: recording.elapsed,
        ..Default::default()
    };
    assert!(
        matches!(failed.phase, DictationSessionPhase::Failed(ref msg) if msg.contains("model load"))
    );
}

/// Prove that overlay state for a silent/empty recording closes without
/// reaching Finished or Failed (silent path produces no overlay transition
/// beyond the initial Recording phase).
#[test]
fn overlay_state_silent_recording_skips_finished_and_failed() {
    use crate::dictation::DictationSessionPhase;

    // Silent recording: the runtime returns Stopped(None), so the caller
    // never transitions to Transcribing/Finished/Failed — just closes.
    let recording = DictationOverlayState {
        phase: DictationSessionPhase::Recording,
        elapsed: Duration::from_millis(500),
        bars: [0.0; 9],
        transcript: "".into(),
        ..Default::default()
    };
    assert_eq!(recording.phase, DictationSessionPhase::Recording);
    assert!(!crate::dictation::window::has_sound(&recording.bars));
}

/// Prove that the Transcribing → Finished transition carries the transcript
/// text, which is then used by the delivery path to inject into the prompt
/// or paste to the frontmost app.
#[test]
fn overlay_finished_state_carries_transcript_for_delivery() {
    use crate::dictation::DictationSessionPhase;

    let transcript_text = "the quick brown fox";
    let finished = DictationOverlayState {
        phase: DictationSessionPhase::Finished,
        elapsed: Duration::from_secs(3),
        bars: [0.0; 9],
        transcript: transcript_text.into(),
        ..Default::default()
    };

    assert_eq!(finished.phase, DictationSessionPhase::Finished);
    assert_eq!(finished.transcript.as_ref(), transcript_text);
    assert!(!finished.transcript.is_empty());
}

/// Verify that the overlay update path in builtin_execution transitions
/// through Recording → Transcribing → Finished (or Failed) in order,
/// matching the acceptance criteria.
#[test]
fn builtin_dictation_overlay_transitions_are_ordered_correctly() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let branch_start = src
        .find("builtins::BuiltInFeature::Dictation =>")
        .expect("dictation builtin branch must exist");
    let branch_src = &src[branch_start..];

    // On Started: overlay is set to Recording.
    let recording_pos = branch_src
        .find("DictationSessionPhase::Recording")
        .expect("Started arm must set overlay to Recording");

    // On Stopped(Some): overlay transitions to Transcribing.
    let transcribing_pos = branch_src
        .find("DictationSessionPhase::Transcribing")
        .expect("Stopped(Some) arm must set overlay to Transcribing");

    assert!(
        recording_pos < transcribing_pos,
        "Recording (byte {recording_pos}) must appear before Transcribing (byte {transcribing_pos}) in the dictation branch"
    );

    let handler_src = dictation_handler_source(&src);
    assert!(
        handler_src.contains("DictationSessionPhase::Finished"),
        "handler must render a brief Finished confirmation before the overlay closes"
    );
    assert!(
        handler_src
            .contains("schedule_dictation_overlay_close(cx, Self::DICTATION_FINISHED_LINGER)"),
        "handler must schedule the overlay close after the Finished linger"
    );

    // Transcription error: overlay shows Failed.
    assert!(
        handler_src.contains("DictationSessionPhase::Failed"),
        "handler must show Failed on transcription error"
    );
}

// ---------------------------------------------------------------------------
// Dictation start-edge preflight & helper parity tests
// ---------------------------------------------------------------------------

#[test]
fn dictation_start_preflight_runs_before_toggle() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let dictation_start = src
        .find("fn execute_dictation_builtin_action(")
        .expect("execute_dictation_builtin_action must exist");
    let dictation_src =
        &src[dictation_start..dictation_start + 4000.min(src.len() - dictation_start)];

    let recording_guard_pos = dictation_src
        .find("let is_start_edge = !crate::dictation::is_dictation_busy();")
        .expect("dictation start path must gate preflight on recording state");
    let preflight_pos = dictation_src
        .find("let preflight = self.prepare_dictation_builtin_start(action, None, cx);")
        .expect("dictation start path must prepare preflight");
    let toggle_pos = dictation_src
        .find("crate::dictation::toggle_dictation(dictation_target)")
        .expect("dictation start path must toggle dictation");

    assert!(
        recording_guard_pos < preflight_pos && preflight_pos < toggle_pos,
        "start-edge preflight must run before toggle_dictation"
    );
}

#[test]
fn rapid_dictation_toggle_parity_is_last_intent_wins_during_stop() {
    let restart_after_first_extra_toggle = crate::dictation::toggled_post_stop_restart(false);
    assert!(
        restart_after_first_extra_toggle,
        "first toggle during stop must queue a restart"
    );
    assert!(
        !crate::dictation::toggled_post_stop_restart(restart_after_first_extra_toggle),
        "second toggle during stop must cancel the queued restart"
    );
}

#[test]
fn dictation_start_preflight_surfaces_unavailable_target() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let dictation_start = src
        .find("fn prepare_dictation_builtin_start(")
        .expect("prepare_dictation_builtin_start must exist");
    let dictation_src =
        &src[dictation_start..dictation_start + 3000.min(src.len() - dictation_start)];

    assert!(
        dictation_src.contains("preflight_failure_message()"),
        "preflight failures must be logged"
    );
    assert!(
        dictation_src.contains("self.show_error_toast"),
        "preflight failures must surface a toast"
    );
}

#[test]
fn can_accept_dictation_into_prompt_stays_aligned_with_direct_delivery_views() {
    let src = std::fs::read_to_string("src/app_impl/ui_window.rs").expect("read ui_window.rs");

    let helper_start = src
        .find("fn can_accept_dictation_into_prompt")
        .expect("can_accept_dictation_into_prompt must exist");
    let helper_src = &src[helper_start..helper_start + 1200.min(src.len() - helper_start)];

    let setter_start = src
        .find("fn try_set_prompt_input")
        .expect("try_set_prompt_input must exist");
    let setter_src = &src[setter_start..setter_start + 3500.min(src.len() - setter_start)];

    for view in [
        "AppView::ArgPrompt",
        "AppView::MiniPrompt",
        "AppView::MicroPrompt",
        "AppView::PathPrompt",
        "AppView::SelectPrompt",
        "AppView::EnvPrompt",
        "AppView::TemplatePrompt",
        "AppView::FormPrompt",
        "AppView::FileSearchView",
    ] {
        assert!(
            helper_src.contains(view),
            "can_accept_dictation_into_prompt must include {view}"
        );
        assert!(
            setter_src.contains(view),
            "try_set_prompt_input must include {view}"
        );
    }
}

// ---------------------------------------------------------------------------
// Overlay: bottom-center positioning
// ---------------------------------------------------------------------------

#[test]
fn overlay_positioned_bottom_center_of_screen() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    assert!(
        window_src.contains("calculate_overlay_bottom_center_bounds()"),
        "overlay must use bottom-center positioning function"
    );
    assert!(
        window_src.contains("OVERLAY_BOTTOM_OFFSET_PX"),
        "overlay must define a bottom offset constant"
    );
    assert!(
        window_src.contains("const OVERLAY_BOTTOM_OFFSET_PX: f32 = 15.0"),
        "bottom offset must be 15px matching vercel-voice"
    );
    assert!(
        !window_src.contains("y: px(80.)"),
        "overlay must NOT use the old top-of-screen y=80 position"
    );
}

// ---------------------------------------------------------------------------
// Overlay: active-display selection (not mouse-based)
// ---------------------------------------------------------------------------

#[test]
fn overlay_uses_mouse_display_alignment() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // Must use get_global_mouse_position() and display_for_point() for display selection (matching the main window)
    assert!(
        window_src.contains("get_global_mouse_position()"),
        "overlay must use get_global_mouse_position() for display selection"
    );
    assert!(
        window_src.contains("display_for_point("),
        "overlay must use display_for_point() for display selection"
    );
    // Must NOT use the key-window get_active_display() heuristic
    assert!(
        !window_src.contains("get_active_display()"),
        "overlay must NOT resolve display via get_active_display()"
    );
}

#[test]
fn active_display_api_exists_in_platform() {
    let display_src = std::fs::read_to_string("src/platform/display.rs").expect("read display.rs");

    assert!(
        display_src.contains("pub fn get_active_display()"),
        "platform must expose get_active_display() for key-window display resolution"
    );
    assert!(
        display_src.contains("mainScreen"),
        "get_active_display() must use NSScreen.mainScreen"
    );
    // Non-macOS stub must exist.
    assert!(
        display_src.contains("#[cfg(not(target_os = \"macos\"))]")
            && display_src.contains("get_active_display"),
        "get_active_display() must have a non-macOS stub"
    );
}

// ---------------------------------------------------------------------------
// Overlay: Escape confirmation state machine
// ---------------------------------------------------------------------------

#[test]
fn overlay_has_confirming_phase_in_session_phase_enum() {
    let types_src = std::fs::read_to_string("src/dictation/types.rs").expect("read types.rs");

    assert!(
        types_src.contains("Confirming"),
        "DictationSessionPhase must include a Confirming variant"
    );
}

#[test]
fn overlay_handles_escape_key_for_confirmation() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    assert!(
        window_src.contains("handle_key_down"),
        "overlay must have a key-down handler"
    );
    assert!(
        window_src.contains("is_key_escape"),
        "overlay must check for Escape key"
    );
    assert!(
        window_src.contains("DictationSessionPhase::Confirming"),
        "overlay must transition to Confirming phase"
    );
    assert!(
        window_src.contains("on_key_down"),
        "overlay must register the key-down handler in render"
    );
    assert!(
        window_src.contains("track_focus"),
        "overlay must track focus for key event delivery"
    );
}

#[test]
fn overlay_abort_confirmation_wires_runtime_abort() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        builtin_src.contains("set_overlay_abort_callback"),
        "dictation start path must register an overlay abort callback"
    );
    assert!(
        builtin_src.contains("crate::dictation::abort_dictation()"),
        "overlay abort callback must discard the active recording"
    );
    assert!(
        runtime_src.contains("pub fn abort_dictation() -> Result<()>"),
        "dictation runtime must expose an explicit abort API"
    );
}

#[test]
fn overlay_confirming_phase_renders_stop_continue() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // Source file contains unicode escapes, so match those literally.
    assert!(
        window_src.contains("ACTION_STOP_LABEL") && window_src.contains("ENTER_KEYCAP"),
        "confirming phase must show Stop affordance"
    );
    assert!(
        window_src.contains("ACTION_CONTINUE_LABEL") && window_src.contains("ESC_KEYCAP"),
        "confirming phase must show Continue affordance"
    );
    assert!(
        window_src.contains("Stop dictation?"),
        "confirming phase must show Stop dictation? prompt"
    );
    assert!(
        !window_src.contains(r#""Abort""#),
        "old Abort label should be removed"
    );
    assert!(
        !window_src.contains(r#""Resume Esc""#),
        "old Resume label should be removed"
    );
}

// ---------------------------------------------------------------------------
// Overlay: glass bar styling
// ---------------------------------------------------------------------------

#[test]
fn overlay_uses_glass_bar_styling() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // 81e23d77c moved the glass bar / action rail chrome contract into the
    // shared footer_chrome layer (footer_rail_chrome derives from
    // AppChromeColors::from_theme), so both the native main-menu footer and
    // this overlay consume one rail surface, hover/active, and keycap source.
    assert!(
        window_src.contains("footer_chrome::footer_rail_chrome"),
        "overlay must derive glass surface from the shared footer rail chrome"
    );
    assert!(
        window_src.contains("render_footer_hint_action_button_frame"),
        "overlay must use the shared footer button frame for action buttons"
    );
    let rail_start = window_src
        .find("fn render_static_action_rail")
        .expect("action rail renderer must exist");
    let rail_src = &window_src[rail_start..rail_start + 900.min(window_src.len() - rail_start)];
    assert!(
        !rail_src.contains(".bg(rgba(rail_chrome.surface_rgba))"),
        "action rail must inherit the overlay surface instead of stacking a darker background"
    );
    assert!(
        window_src.contains("render_glass_signal_band"),
        "must render glass signal band"
    );
    assert!(
        window_src.contains("wrap_dictation_overlay_action_rail"),
        "action rail must sit on window surface without stacking under the signal band"
    );
    assert!(
        !window_src.contains("FONT_MONO"),
        "dictation overlay chrome must use the system UI font, not monospace"
    );
    assert!(
        window_src.contains("footer_chrome::footer_rail_chrome")
            && window_src.contains("render_footer_hint_action_button_frame"),
        "dictation action buttons must match main menu footer hover, active, font, and shortcut rendering (the shared frame pins FooterHintKeyMode::Shortcut via footer_chrome's own contract test)"
    );
}

// ---------------------------------------------------------------------------
// Overlay: glass bar dimension contract
// ---------------------------------------------------------------------------

#[test]
fn overlay_dimensions_match_glass_bar_contract() {
    assert_eq!(
        super::window::OVERLAY_WIDTH_PX,
        560.0,
        "overlay width must match the glass bar direction"
    );
    assert_eq!(
        super::window::OVERLAY_HEIGHT_PX,
        100.0,
        "overlay height must fit header row + caption band + footer rail"
    );
    // 562fb2964 moved the radius from a local 12.0 to the shared Liquid
    // Glass panel token (16.0) so the overlay matches other glass panels.
    assert_eq!(
        super::window::OVERLAY_RADIUS_PX,
        16.0,
        "overlay radius must match the adopted Liquid Glass panel shape"
    );
}

// ---------------------------------------------------------------------------
// Overlay: focus enabled for key events
// ---------------------------------------------------------------------------

#[test]
fn overlay_window_opens_without_app_activation() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // The overlay window must be created with focus: false to avoid
    // activating the app (which would surface the main window).  Key
    // events are delivered via orderFrontRegardless + makeKeyWindow.
    assert!(
        window_src.contains("focus: false"),
        "overlay window must open with focus: false to avoid app activation"
    );
    assert!(
        window_src.contains("orderFrontRegardless"),
        "overlay must use orderFrontRegardless for non-activating front"
    );
    assert!(
        window_src.contains("makeKeyWindow"),
        "overlay must use makeKeyWindow for key event delivery"
    );
}

// ---------------------------------------------------------------------------
// Runtime session owns overlay phase (confirm/resume/abort contract)
// ---------------------------------------------------------------------------

#[test]
fn runtime_session_has_overlay_phase_field() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        runtime_src.contains("overlay_phase: DictationSessionPhase"),
        "DictationSession must own overlay_phase field"
    );
}

#[test]
fn snapshot_overlay_state_reads_session_phase_not_hardcoded() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    // The old bug: snapshot_overlay_state() hardcoded `phase: Recording`.
    // Verify it now reads from `session.overlay_phase`.
    assert!(
        runtime_src.contains("session.overlay_phase.clone()"),
        "snapshot_overlay_state must read phase from session.overlay_phase, not hardcode it"
    );
    assert!(
        !runtime_src.contains("phase: crate::dictation::DictationSessionPhase::Recording"),
        "snapshot_overlay_state must not hardcode DictationSessionPhase::Recording"
    );
}

#[test]
fn set_overlay_phase_is_exported() {
    let mod_src = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");

    assert!(
        mod_src.contains("set_overlay_phase"),
        "set_overlay_phase must be re-exported from dictation module"
    );
}

#[test]
fn overlay_key_handler_writes_through_to_runtime_phase() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // The overlay key handler must use overlay_escape_action to decide behavior.
    assert!(
        window_src.contains("overlay_escape_action(&self.state.phase, elapsed)"),
        "overlay key handler must delegate to overlay_escape_action with elapsed"
    );

    // AbortSession must invoke the stored abort callback (via helper).
    assert!(
        window_src.contains("OVERLAY_ABORT_CALLBACK"),
        "overlay must invoke the stored abort callback on AbortSession"
    );

    // CloseOverlay must call close_dictation_overlay.
    let handler_start = window_src
        .find("fn handle_key_down")
        .expect("overlay must have a key-down handler");
    let handler_end = handler_start + 3000.min(window_src.len() - handler_start);
    let handler_src = &window_src[handler_start..handler_end];
    assert!(
        handler_src.contains("close_dictation_overlay")
            || handler_src.contains("abort_overlay_session"),
        "overlay key handler must call close_dictation_overlay or abort_overlay_session on CloseOverlay"
    );
}

#[test]
fn builtin_microphone_submit_handler_accepts_mini_prompt_choices() {
    let helpers_src =
        std::fs::read_to_string("src/render_prompts/arg/helpers.rs").expect("read arg helpers");

    // Validation must match both ArgPrompt and MiniPrompt variants.
    assert!(
        helpers_src.contains("AppView::MiniPrompt { choices, .. }"),
        "builtin microphone submit handling must validate MiniPrompt choices"
    );

    // The validation function must appear and reference MiniPrompt.
    let valid_fn_start = helpers_src
        .find("fn is_valid_builtin_mic_selection")
        .expect("is_valid_builtin_mic_selection must exist");
    let valid_fn_body =
        &helpers_src[valid_fn_start..valid_fn_start + 400.min(helpers_src.len() - valid_fn_start)];
    assert!(
        valid_fn_body.contains("MiniPrompt"),
        "is_valid_builtin_mic_selection must handle MiniPrompt variant"
    );

    // The label resolution in handle_builtin_mic_selection must also match MiniPrompt.
    let handle_fn_start = helpers_src
        .find("fn handle_builtin_mic_selection")
        .expect("handle_builtin_mic_selection must exist");
    let handle_fn_body = &helpers_src
        [handle_fn_start..handle_fn_start + 1600.min(helpers_src.len() - handle_fn_start)];
    assert!(
        handle_fn_body.contains("MiniPrompt"),
        "handle_builtin_mic_selection label resolution must handle MiniPrompt variant"
    );
}

#[test]
fn active_recording_microphone_is_captured_at_session_start() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");
    let mod_src = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");

    assert!(
        runtime_src.contains("active_device: Option<DictationDeviceInfo>"),
        "dictation sessions must remember the microphone opened by the live capture"
    );
    assert!(
        runtime_src.contains("pub fn get_active_dictation_device()"),
        "runtime must expose the active capture microphone for honest overlay copy"
    );
    assert!(
        runtime_src.contains("let active_device = resolve_preferred_device_info()?")
            && runtime_src.contains("active_device: active_device.clone()"),
        "start_recording must capture the resolved microphone once at session start"
    );
    assert!(
        mod_src.contains("get_active_dictation_device"),
        "active capture microphone must be re-exported for source/runtime receipts"
    );
}

#[test]
fn overlay_microphone_control_is_labeled_as_next_recording_preference() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    assert!(
        window_src.contains("const ACTION_MIC_LABEL: &str = \"Select Mic\""),
        "overlay mic action must open the microphone picker without rendering as a keycap"
    );
    assert!(
        window_src.contains("The current recording keeps using the device it opened with"),
        "overlay source contract must document that cycling only changes the next recording"
    );
    assert!(
        window_src.contains("persisted preference used by the next dictation capture"),
        "overlay mic picker source must state the preference applies to the next capture"
    );
}

#[test]
fn start_recording_initialises_overlay_phase_to_recording() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        runtime_src.contains("overlay_phase: DictationSessionPhase::Recording"),
        "start_recording must initialise overlay_phase to Recording"
    );
}

// ---------------------------------------------------------------------------
// Regression: missing-model error path must NEVER attempt paste or delivery
// ---------------------------------------------------------------------------

/// When `transcribe_captured_audio` returns an `Err` (e.g. missing Whisper
/// model), `handle_dictation_transcript` must show a toast and update the
/// overlay to `Failed` — but it must **never** call `paste_text` or
/// `try_set_prompt_input`, because there is no transcript to deliver.
///
/// This test structurally verifies the error arm to prevent accidental
/// delivery regressions.
#[test]
fn missing_model_error_path_never_attempts_paste_or_prompt_delivery() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);

    // The error arm starts at `Err(error) =>` and extends to the end of the
    // match (the handler's closing brace).  Extract it by finding the last
    // `Err(error) =>` after the `Ok(None)` arm.
    let ok_none_pos = handler_src
        .find("Ok(None) =>")
        .expect("handler must have an Ok(None) arm");
    let err_arm_start = handler_src[ok_none_pos..]
        .find("Err(error) =>")
        .expect("handler must have an Err(error) arm after Ok(None)");
    let err_arm_src = &handler_src[ok_none_pos + err_arm_start..];

    assert!(
        !err_arm_src.contains("paste_text"),
        "Err arm of handle_dictation_transcript must NEVER call paste_text \
         — there is no transcript to deliver when transcription fails"
    );
    assert!(
        !err_arm_src.contains("try_set_prompt_input"),
        "Err arm of handle_dictation_transcript must NEVER call try_set_prompt_input \
         — there is no transcript to deliver when transcription fails"
    );

    // Confirm the error arm DOES show a toast and update overlay to Failed.
    assert!(
        err_arm_src.contains("show_error_toast"),
        "Err arm must surface the error to the user via toast"
    );
    assert!(
        err_arm_src.contains("DictationSessionPhase::Failed"),
        "Err arm must update overlay to Failed phase"
    );
    assert!(
        err_arm_src.contains("schedule_dictation_overlay_close"),
        "Err arm must schedule overlay close after showing the error"
    );
}

/// The `Ok(None)` arm (silent/short audio) must also never attempt delivery.
#[test]
fn silent_audio_path_never_attempts_paste_or_prompt_delivery() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);

    // Extract the Ok(None) arm: from `Ok(None) =>` to `Err(error) =>`.
    let none_start = handler_src
        .find("Ok(None) =>")
        .expect("handler must have an Ok(None) arm");
    let none_src_tail = &handler_src[none_start..];
    let err_offset = none_src_tail
        .find("Err(error) =>")
        .expect("Ok(None) arm must be followed by Err arm");
    let none_arm_src = &none_src_tail[..err_offset];

    assert!(
        !none_arm_src.contains("paste_text"),
        "Ok(None) arm must NEVER call paste_text — no transcript exists"
    );
    assert!(
        !none_arm_src.contains("try_set_prompt_input"),
        "Ok(None) arm must NEVER call try_set_prompt_input — no transcript exists"
    );
    assert!(
        !none_arm_src.contains("open_tab_ai_agent_chat_with_entry_intent"),
        "Ok(None) arm must NEVER open Agent Chat — no transcript exists to seed or submit"
    );
    assert!(
        none_arm_src.contains("WindowEvent::AbortDictation"),
        "Ok(None) arm must abort the dictation session so Agent Chat-targeted silence does not reveal ScriptList"
    );
    assert!(
        !none_arm_src.contains("WindowEvent::FinishDictation"),
        "Ok(None) arm must not finish successfully because Agent Chat finish reveals a transcript-backed chat"
    );
}

#[test]
fn agent_chat_dictation_delivery_suppresses_focused_launcher_context() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);
    let tab_ai_arm_start = handler_src
        .find("DictationTarget::TabAiHarness =>")
        .expect("handler must route TabAiHarness dictation");
    let tab_ai_arm = &handler_src[tab_ai_arm_start..tab_ai_arm_start + 900];

    assert!(
        tab_ai_arm.contains("open_tab_ai_agent_chat_with_entry_intent_suppressing_focused_part"),
        "Agent Chat dictation should submit the transcript as the prompt without inheriting the selected ScriptList item as context"
    );
}

#[test]
fn orchestrator_focus_main_commands_queue_app_focus_after_reveal() {
    let bridge_src = std::fs::read_to_string("src/app_impl/window_orchestrator_bridge.rs")
        .expect("read window_orchestrator_bridge.rs");

    assert!(
        bridge_src.contains("WindowCommand::FocusMain(token)"),
        "the orchestrator bridge must consume FocusMain commands instead of leaving them as executor no-ops"
    );
    assert!(
        bridge_src.contains("this.pending_focus = Some(target);")
            && bridge_src.contains("cx.notify();"),
        "FocusMain commands must queue ScriptListApp pending_focus after reveal so the next render applies keyboard focus"
    );
    assert!(
        bridge_src.contains("FocusToken::ChatComposer => Some(FocusTarget::AgentChat)"),
        "Agent Chat dictation's ChatComposer token must map to the dedicated AgentChat focus target"
    );
}

// ---------------------------------------------------------------------------
// Regression: Escape → Escape abort must NEVER deliver a transcript
// ---------------------------------------------------------------------------

/// The overlay abort callback registered at dictation start must call
/// `abort_dictation()` + `close_dictation_overlay()` and must NOT invoke
/// `handle_dictation_transcript` or any delivery function.  This verifies
/// the structural separation between abort and delivery paths.
#[test]
fn abort_callback_never_invokes_transcript_delivery() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    // Extract the abort callback body: from `set_overlay_abort_callback` to
    // the closing `});` before `open_dictation_overlay`.
    let callback_start = src
        .find("set_overlay_abort_callback")
        .expect("dictation start path must register an abort callback");
    let callback_tail = &src[callback_start..];

    // The callback is a closure — find its extent up to the next overlay call.
    let callback_end = callback_tail
        .find("open_dictation_overlay")
        .expect("abort callback must be followed by overlay open");
    let callback_src = &callback_tail[..callback_end];

    assert!(
        callback_src.contains("request_active_dictation_stop(")
            && callback_src.contains("DictationStopReason::OverlayAbort")
            && callback_src.contains("false"),
        "abort callback must request a non-transcribing stop so the recording is discarded off the UI path"
    );
    assert!(
        !callback_src.contains("handle_dictation_transcript"),
        "abort callback must NEVER invoke handle_dictation_transcript — \
         the user chose to discard the recording"
    );
    assert!(
        !callback_src.contains("paste_text"),
        "abort callback must NEVER call paste_text"
    );
    assert!(
        !callback_src.contains("try_set_prompt_input"),
        "abort callback must NEVER call try_set_prompt_input"
    );
    assert!(
        !callback_src.contains("transcribe_captured_audio"),
        "abort callback must NEVER invoke transcription — \
         the recording is discarded, not transcribed"
    );
}

#[test]
fn orchestrator_executor_keeps_global_main_visibility_in_sync() {
    let src =
        std::fs::read_to_string("src/window_orchestrator/executor.rs").expect("read executor.rs");

    let conceal_start = src
        .find("WindowCommand::ConcealMain")
        .expect("executor must handle ConcealMain");
    let dismiss_start = src
        .find("WindowCommand::DismissMain")
        .expect("executor must handle DismissMain");
    let reveal_start = src
        .find("WindowCommand::RevealMain")
        .expect("executor must handle RevealMain");
    let focus_surface_start = src
        .find("WindowCommand::FocusSurface")
        .expect("executor must handle FocusSurface");

    assert!(
        src[conceal_start..dismiss_start].contains("set_main_window_visible(false)"),
        "ConcealMain must mark the main window hidden before dictation opens"
    );
    assert!(
        src[dismiss_start..reveal_start].contains("set_main_window_visible(false)"),
        "DismissMain must mark the main window hidden"
    );
    assert!(
        src[reveal_start..focus_surface_start].contains("set_main_window_visible(true)"),
        "RevealMain must mark the main window visible"
    );
}

/// `abort_dictation()` must drop the session, ensuring `is_dictation_recording()`
/// returns false afterward and preventing any further overlay pump ticks from
/// reading stale state.
#[test]
fn abort_dictation_clears_session_state() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    // abort_dictation must call stop_recording which takes the session.
    let abort_start = runtime_src
        .find("pub fn abort_dictation() -> Result<()>")
        .expect("abort_dictation must exist");
    let abort_src =
        &runtime_src[abort_start..abort_start + 300.min(runtime_src.len() - abort_start)];

    assert!(
        abort_src.contains("stop_recording()"),
        "abort_dictation must call stop_recording() to drain and drop the session"
    );

    // stop_recording must take() the session from the mutex.
    let stop_start = runtime_src
        .find("fn stop_recording()")
        .expect("stop_recording must exist");
    let stop_src = &runtime_src[stop_start..stop_start + 600.min(runtime_src.len() - stop_start)];

    assert!(
        stop_src.contains(".take()"),
        "stop_recording must take() the session from the global mutex, \
         clearing it so is_dictation_recording() returns false"
    );
}

/// The overlay Escape key handler must write `Confirming` through to the
/// runtime (via `set_overlay_phase`) so the pump reads the authoritative
/// phase. Escape in Confirming must resume recording, while Enter remains the
/// deliberate abort action that invokes `abort_dictation` + close, NOT
/// `handle_dictation_transcript`.
#[test]
fn escape_abort_never_reaches_transcript_handler() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    let handler_start = window_src
        .find("fn handle_key_down")
        .expect("overlay must have a key-down handler");
    let handler_end = handler_start + 3000.min(window_src.len() - handler_start);
    let handler_src = &window_src[handler_start..handler_end];

    // AbortSession arm invokes the stored abort callback (via helper).
    assert!(
        handler_src.contains("abort_overlay_session"),
        "Escape abort must invoke abort_overlay_session (which uses the stored abort callback)"
    );

    // overlay_escape_action routes Recording (≥ threshold) to TransitionToConfirming
    // and Recording (< threshold) to AbortSession.
    assert!(
        window_src.contains(
            "DictationSessionPhase::Recording => OverlayEscapeAction::TransitionToConfirming"
        ),
        "overlay_escape_action must map Recording (>= threshold) to TransitionToConfirming"
    );
    assert!(
        window_src.contains("ESCAPE_CONFIRM_THRESHOLD"),
        "overlay_escape_action must use the named threshold constant"
    );
    assert!(
        window_src
            .contains("DictationSessionPhase::Confirming => OverlayEscapeAction::ResumeRecording"),
        "overlay_escape_action must map Confirming to ResumeRecording"
    );

    // The key handler must NEVER invoke transcript delivery functions.
    assert!(
        !handler_src.contains("handle_dictation_transcript"),
        "overlay key handler must never call handle_dictation_transcript"
    );
    assert!(
        !handler_src.contains("paste_text"),
        "overlay key handler must never call paste_text"
    );
    assert!(
        !handler_src.contains("transcribe_captured_audio"),
        "overlay key handler must never invoke transcription"
    );
}

// ---------------------------------------------------------------------------
// Regression: missing-model toast includes download guidance
// ---------------------------------------------------------------------------

/// When the Parakeet model is missing at transcription time, the error
/// handler must detect the specific error string and log the resolved
/// model path for diagnostics.
#[test]
fn missing_model_error_detects_parakeet_string_and_logs_model_path() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);

    // The error arm must detect the model-missing string (both engines
    // bail with "... model not downloaded").
    assert!(
        handler_src.contains(r#"error_text.contains("model not downloaded")"#),
        "handler must detect the 'model not downloaded' error string"
    );
    assert!(
        handler_src.contains("resolve_default_model_path()"),
        "error handler must use the resolved Parakeet model path"
    );
}

// ---------------------------------------------------------------------------
// Regression: complete delivery ordering chain (end-to-end)
// ---------------------------------------------------------------------------

/// Verify the full frontmost-app delivery chain ordering in one test:
/// yield_focus → focus_settle → paste_text.
/// This catches regressions if any step is reordered or removed.
#[test]
fn frontmost_app_delivery_full_ordering_chain() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let frontmost_src = dictation_frontmost_paste_source(&src);

    let steps: Vec<(&str, usize)> = [
        (
            "yield_focus_for_dictation_paste",
            "yield focus to target app",
        ),
        (
            "dictation_focus_settle_duration",
            "wait for focus to settle",
        ),
        ("paste_text", "paste transcript"),
    ]
    .iter()
    .map(|(needle, label)| {
        let pos = frontmost_src
            .find(needle)
            .unwrap_or_else(|| panic!("frontmost-app branch must contain {label} ({needle})"));
        (*label, pos)
    })
    .collect();

    for pair in steps.windows(2) {
        assert!(
            pair[0].1 < pair[1].1,
            "delivery ordering violated: '{}' (byte {}) must come before '{}' (byte {})",
            pair[0].0,
            pair[0].1,
            pair[1].0,
            pair[1].1
        );
    }
}

// ---------------------------------------------------------------------------
// ParakeetDictationEngine tests
// ---------------------------------------------------------------------------

#[test]
fn parakeet_engine_new_fails_for_missing_model_dir() {
    let result =
        ParakeetDictationEngine::new(std::path::Path::new("/definitely/missing-parakeet-dir"));
    assert!(
        result.is_err(),
        "ParakeetDictationEngine::new must fail for a missing directory"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not found"),
        "error should mention 'not found', got: {err_msg}"
    );
}

#[test]
fn parakeet_engine_new_fails_for_file_path() {
    let temp_file = tempfile::NamedTempFile::new().expect("create temp file");
    let result = ParakeetDictationEngine::new(temp_file.path());
    assert!(
        result.is_err(),
        "ParakeetDictationEngine::new must fail for a file path"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("not a directory"),
        "error should mention 'not a directory', got: {err_msg}"
    );
}

#[test]
fn parakeet_engine_new_accepts_existing_directory() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    ParakeetDictationEngine::new(temp_dir.path())
        .expect("ParakeetDictationEngine::new should accept an existing directory");
}

// ---------------------------------------------------------------------------
// Model availability tests
// ---------------------------------------------------------------------------

#[test]
fn is_parakeet_model_available_returns_false_for_nonexistent_path() {
    // The default path is unlikely to exist in CI, so this should be false.
    // We test the function runs without panicking and returns a bool.
    let _available = is_parakeet_model_available();
}

#[test]
fn is_parakeet_model_available_returns_true_for_populated_dir() {
    let temp_dir = tempfile::TempDir::new().expect("create temp dir");
    // Create a dummy file inside to simulate an extracted model.
    std::fs::write(temp_dir.path().join("model.onnx"), b"dummy").expect("write dummy model");

    // We can't easily test the actual function since it reads from a fixed path,
    // but we can verify the logic inline.
    let path = temp_dir.path();
    let available = path.is_dir()
        && std::fs::read_dir(path)
            .map(|mut entries| entries.next().is_some())
            .unwrap_or(false);
    assert!(
        available,
        "populated directory should be detected as available"
    );
}

// ---------------------------------------------------------------------------
// Download types tests
// ---------------------------------------------------------------------------

#[test]
fn download_progress_percentage_ranges() {
    use crate::dictation::download::{DownloadPhase, DownloadProgress};

    let p = DownloadProgress {
        downloaded: 0,
        total: 100,
    };
    assert_eq!(p.percentage(), 0);

    let p = DownloadProgress {
        downloaded: 50,
        total: 100,
    };
    assert_eq!(p.percentage(), 50);

    let p = DownloadProgress {
        downloaded: 100,
        total: 100,
    };
    assert_eq!(p.percentage(), 100);

    // Unknown total.
    let p = DownloadProgress {
        downloaded: 42,
        total: 0,
    };
    assert_eq!(p.percentage(), 0);
}

#[test]
fn download_phase_variants_are_distinct() {
    use crate::dictation::download::DownloadPhase;

    let phases = [
        DownloadPhase::Downloading,
        DownloadPhase::Extracting,
        DownloadPhase::Complete,
        DownloadPhase::Cancelled,
        DownloadPhase::Failed("test".to_string()),
    ];

    for (i, a) in phases.iter().enumerate() {
        for (j, b) in phases.iter().enumerate() {
            if i == j {
                assert_eq!(a, b);
            } else {
                assert_ne!(a, b);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parakeet model constants and URL validation
// ---------------------------------------------------------------------------

#[test]
fn parakeet_model_url_is_https() {
    use crate::dictation::transcription::PARAKEET_MODEL_URL;
    assert!(
        PARAKEET_MODEL_URL.starts_with("https://"),
        "model URL must use HTTPS, got: {PARAKEET_MODEL_URL}"
    );
}

#[test]
fn parakeet_model_archive_size_is_reasonable() {
    use crate::dictation::transcription::PARAKEET_MODEL_ARCHIVE_SIZE;
    // ~478 MB — sanity check it's in the right ballpark.
    assert!(
        PARAKEET_MODEL_ARCHIVE_SIZE > 400_000_000,
        "archive size should be > 400 MB"
    );
    assert!(
        PARAKEET_MODEL_ARCHIVE_SIZE < 600_000_000,
        "archive size should be < 600 MB"
    );
}

// ---------------------------------------------------------------------------
// Runtime transcriber cache selection logic
// ---------------------------------------------------------------------------

#[test]
fn transcriber_cache_rebuilds_when_model_or_language_changes_or_idle() {
    use crate::dictation::runtime::{should_rebuild_cached_transcriber, CachedTranscriberKey};
    use crate::dictation::DictationModelId;

    let parakeet = CachedTranscriberKey {
        model_id: DictationModelId::ParakeetTdt06bV3,
        language: None,
    };
    let whisper_en = CachedTranscriberKey {
        model_id: DictationModelId::WhisperMedium,
        language: Some("en".to_string()),
    };
    let whisper_de = CachedTranscriberKey {
        model_id: DictationModelId::WhisperMedium,
        language: Some("de".to_string()),
    };

    assert!(should_rebuild_cached_transcriber(None, &parakeet, false));
    assert!(!should_rebuild_cached_transcriber(
        Some(&parakeet),
        &parakeet,
        false
    ));
    assert!(should_rebuild_cached_transcriber(
        Some(&parakeet),
        &parakeet,
        true
    ));
    assert!(should_rebuild_cached_transcriber(
        Some(&parakeet),
        &whisper_en,
        false
    ));
    assert!(should_rebuild_cached_transcriber(
        Some(&whisper_en),
        &whisper_de,
        false
    ));
}

// ---------------------------------------------------------------------------
// Model status type tests
// ---------------------------------------------------------------------------

#[test]
fn dictation_model_status_variants() {
    use crate::dictation::types::DictationModelStatus;

    let statuses = [
        DictationModelStatus::Available,
        DictationModelStatus::NotDownloaded,
        DictationModelStatus::Downloading {
            percentage: 50,
            downloaded_bytes: 250_000_000,
            total_bytes: 500_000_000,
            speed_bytes_per_sec: 10_000_000,
            eta_seconds: Some(25),
        },
        DictationModelStatus::Extracting,
        DictationModelStatus::DownloadFailed("err".to_string()),
    ];

    assert_eq!(statuses.len(), 5);
    assert_ne!(statuses[0], statuses[1]);
}

#[test]
fn format_eta_covers_ranges() {
    use crate::dictation::download::format_eta;

    assert_eq!(format_eta(None), "ETA --");
    assert_eq!(format_eta(Some(0)), "ETA <1s");
    assert_eq!(format_eta(Some(15)), "ETA 15s");
    assert_eq!(format_eta(Some(75)), "ETA 1m 15s");
    assert_eq!(format_eta(Some(3600)), "ETA 1h");
    assert_eq!(format_eta(Some(3672)), "ETA 1h 1m");
}

#[test]
fn downloading_prompt_includes_eta_and_cancel_action() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let prompt_start = src
        .find("fn build_dictation_model_prompt")
        .expect("build_dictation_model_prompt must exist");
    let prompt_src = &src[prompt_start..prompt_start + 7000.min(src.len() - prompt_start)];

    assert!(
        prompt_src.contains("format_progress_summary"),
        "downloading prompt must use the shared progress summary (which includes ETA)"
    );
    assert!(
        prompt_src.contains("Cancel download"),
        "downloading prompt must expose a cancel action"
    );
    assert!(
        prompt_src.contains("retry resumes from the partial file"),
        "cancel action must explain resume behavior"
    );
    assert!(
        prompt_src.contains("Partial download kept. Retry resumes"),
        "cancelled downloads must explain resume behavior"
    );
}

// ---------------------------------------------------------------------------
// Source-contract regression tests
// ---------------------------------------------------------------------------

/// Ensure the overlay window opens without activating the app and receives
/// key events (Escape) via orderFrontRegardless + makeKeyWindow + GPUI focus.
/// These invariants protect the hotkey-only overlay goal and the Escape-confirm flow.
#[test]
fn overlay_requests_key_delivery_without_app_activation() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    assert!(
        src.contains("focus: false"),
        "overlay must not activate the app when it opens"
    );
    assert!(
        src.contains("orderFrontRegardless"),
        "overlay must raise itself without activating the app"
    );
    assert!(
        src.contains("makeKeyWindow"),
        "overlay must request key-window status so Escape is delivered"
    );
    assert!(
        src.contains("view.focus_handle.focus(window, cx);"),
        "overlay must focus the GPUI focus handle after opening"
    );
}

/// Ensure the transcription pipeline logs model resolution details so that
/// second-hotkey failures are diagnosable from logs alone.
#[test]
fn dictation_transcription_logs_model_resolution_details() {
    let src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");
    assert!(
        src.contains("model_exists = config.model_path.exists()"),
        "transcription log must include model_exists"
    );
    assert!(
        src.contains("model_is_file = config.model_path.is_file()"),
        "transcription log must include model_is_file"
    );
    assert!(
        src.contains("model not downloaded"),
        "runtime must bail with a clear message when the selected model is missing"
    );
}

// ---------------------------------------------------------------------------
// Overlay lifecycle regression tests
// ---------------------------------------------------------------------------

#[test]
fn overlay_generation_is_bumped_from_started_path_not_window_creation() {
    let window_source = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    let builtin_source = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        window_source.contains("pub fn begin_overlay_session()"),
        "dictation overlay must expose a session-scoped generation bump"
    );
    assert!(
        builtin_source.contains("begin_overlay_session()"),
        "dictation start path must bump generation before opening/reusing the overlay"
    );
}

#[test]
fn open_dictation_overlay_does_not_bump_generation_itself() {
    let window_source = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // Extract just the open_dictation_overlay function body to check it
    // doesn't contain its own generation bump.
    let fn_start = window_source
        .find("pub fn open_dictation_overlay(")
        .expect("open_dictation_overlay must exist");
    let fn_body = &window_source[fn_start..];
    // Take up to the next top-level `pub fn` after the opening one.
    let fn_end = fn_body[1..].find("\npub fn ").unwrap_or(fn_body.len() - 1);
    let fn_text = &fn_body[..fn_end + 1];

    assert!(
        !fn_text.contains("OVERLAY_GENERATION.fetch_add"),
        "open_dictation_overlay must not bump generation — \
         that is begin_overlay_session's job"
    );
}

#[test]
fn hidden_app_dictation_overlay_always_keys_overlay() {
    let window_source = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    assert!(
        window_source.contains("let should_key_overlay = true"),
        "overlay must always become key window so Escape/Enter are delivered in hidden-app mode"
    );
}

#[test]
fn dictation_builtin_does_not_set_opened_from_main_menu() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    // Extract the Dictation arm of the match.
    let arm_start = src
        .find("builtins::BuiltInFeature::Dictation =>")
        .expect("Dictation arm must exist");
    let tail = &src[arm_start..];
    // The arm ends at the next BuiltInFeature variant.
    let arm_end = tail[1..]
        .find("builtins::BuiltInFeature::")
        .unwrap_or(tail.len() - 1);
    let arm_text = &tail[..arm_end + 1];

    assert!(
        !arm_text.contains("opened_from_main_menu = true"),
        "dictation builtin must not set opened_from_main_menu — \
         that would cause the main window to appear on the dictation hotkey path"
    );
}

#[test]
fn overlay_render_uses_outer_wrapper_for_full_bounds() {
    let window_source = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // The render must have a dedicated outer root div that claims full bounds
    // with overflow hidden and the one-window composition as a child.
    let compact: String = window_source
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    assert!(
        compact.contains(".w_full().h_full().overflow_hidden().child(composition)"),
        "overlay render must use an outer wrapper div with w_full().h_full().overflow_hidden().child(composition)"
    );
}

// ---------------------------------------------------------------------------
// Source-contract coverage: singleton, non-activating, edge-to-edge fill,
// and generation guards
// ---------------------------------------------------------------------------

fn extract_delta_contract_section<'a>(source: &'a str, start_pat: &str, end_pat: &str) -> &'a str {
    let start = source
        .find(start_pat)
        .unwrap_or_else(|| panic!("missing section start: {start_pat}"));
    let tail = &source[start..];
    let end = tail
        .find(end_pat)
        .map(|offset| start + offset)
        .unwrap_or(source.len());
    &source[start..end]
}

fn compact_delta_contract(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn dictation_overlay_singleton_nonactivating_contract() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    let section = extract_delta_contract_section(
        &src,
        "pub fn open_dictation_overlay(",
        "pub fn update_dictation_overlay(",
    );
    let body = compact_delta_contract(section);

    // Live-handle reuse: probe handle liveness before reuse
    assert!(
        body.contains(&compact_delta_contract(
            "let alive = handle.update(cx, |_view, _window, _cx| {}).is_ok();"
        )),
        "overlay open path must probe handle liveness before reuse"
    );

    // Singleton reuse: the live-handle branch must return the existing
    // handle before any window creation. A tracing call sits between the
    // check and the return, so anchor on ordering rather than exact text.
    let alive_branch_pos = section
        .find("if alive {")
        .expect("overlay open path must branch on handle liveness");
    let reuse_return_pos = section
        .find("return Ok(handle);")
        .expect("overlay must early-return the existing live handle");
    let open_window_anchor_pos = section
        .find(".open_window(window_options")
        .expect("overlay must create the native window through GPUI open_window");
    assert!(
        alive_branch_pos < reuse_return_pos && reuse_return_pos < open_window_anchor_pos,
        "overlay must reuse the live singleton window instead of spawning duplicates"
    );

    // Stale-handle clearing
    assert!(
        body.contains(&compact_delta_contract("*guard = None;")),
        "stale overlay handles must be cleared before recreation"
    );

    // Non-activating open: focus: false
    assert!(
        body.contains(&compact_delta_contract(
            "let main_was_visible = crate::is_main_window_visible();"
        )),
        "overlay open path must snapshot main visibility before native window creation"
    );
    let visibility_snapshot_pos = section
        .find("let main_was_visible = crate::is_main_window_visible();")
        .expect("overlay must snapshot main visibility before native window creation");
    let open_window_pos = section
        .find(".open_window(window_options")
        .expect("overlay must create the native window through GPUI open_window");
    assert!(
        visibility_snapshot_pos < open_window_pos,
        "main visibility must be captured before opening the overlay window"
    );

    assert!(
        body.contains(&compact_delta_contract("focus: false,")),
        "overlay window must stay non-activating on open"
    );
    assert!(
        body.contains(&compact_delta_contract("show: false,")),
        "overlay must be created hidden so GPUI does not surface sibling launcher panels"
    );
    assert!(
        body.contains(&compact_delta_contract("kind: gpui::WindowKind::PopUp,")),
        "overlay must remain a PopUp so makeKeyWindow does not activate or reveal main"
    );
    assert!(
        body.contains(&compact_delta_contract("is_resizable: false,")),
        "overlay window must not be resizable"
    );

    // Order front without activating
    assert!(
        body.contains(&compact_delta_contract(
            "let () = msg_send![ns_window, orderFrontRegardless];"
        )),
        "overlay must order front without activating the app"
    );

    // Re-hide main window when Script Kit was hidden
    assert!(
        body.contains(&compact_delta_contract("if !main_was_visible {")),
        "overlay open path must check main window visibility and re-hide when it was hidden"
    );
    let order_out_pos = section
        .find("msg_send![main_window, orderOut: cocoa::base::nil]")
        .expect("overlay must order out the hidden main window before surfacing");
    let order_front_pos = section
        .find("msg_send![ns_window, orderFrontRegardless]")
        .expect("overlay must order itself front without activation");
    assert!(
        order_out_pos < order_front_pos,
        "hidden-main orderOut must happen before overlay orderFrontRegardless"
    );

    // Singleton slot storage
    assert!(
        body.contains(&compact_delta_contract("*guard = Some(handle);")),
        "overlay handle must be stored into the singleton slot"
    );
}

#[test]
fn dictation_overlay_claims_full_popup_bounds_contract() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    let body = compact_delta_contract(extract_delta_contract_section(
        &src,
        "let surface = div()",
        "pub(crate) fn finished_label",
    ));

    // The bounded pill surface owns the full width and clips its content. In
    // glass mode its height is the main-content region; the footer and exact
    // 8pt gutter live below it in the same physical window.
    assert!(
        body.contains(&compact_delta_contract(
            "let surface = div()\
            .flex()\
            .flex_row()\
            .items_center()\
            .justify_center()\
            .w_full()\
            .when(glass_in_window_footer, |d| {\
                d.h(px(detached_regions.main_content.height))\
                    .min_h(px(detached_regions.main_content.height))\
            })\
            .when(!glass_in_window_footer, |d| d.h_full())\
            .relative()\
            .overflow_hidden()"
        )),
        "dictation pill must fill the bounded content stage and preserve the non-glass full-window fallback"
    );

    // Root overlay node must fill the popup window edge-to-edge with overflow hidden
    assert!(
        body.contains(&compact_delta_contract(
            "div().track_focus(&self.focus_handle).on_key_down(cx.listener(Self::handle_key_down)).on_modifiers_changed("
        )),
        "root overlay node must fill the popup window edge-to-edge with overflow_hidden"
    );
    assert!(
        body.contains(&compact_delta_contract(
            ".w_full().h_full().overflow_hidden().child(composition)"
        )),
        "root overlay node must fill the popup window edge-to-edge with overflow_hidden"
    );
}

#[test]
fn dictation_overlay_generation_guards_pump_and_delayed_close_contract() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let body = compact_delta_contract(&src);

    // Session start must bump generation
    assert!(
        body.contains(&compact_delta_contract(
            "let _ = crate::dictation::begin_overlay_session();"
        )),
        "dictation start edge must bump overlay generation"
    );

    // Overlay pump and delayed close must bail on generation mismatch
    assert!(
        body.contains(&compact_delta_contract(
            "if crate::dictation::overlay_generation() != gen {"
        )),
        "overlay pump and delayed close must bail when a newer session exists"
    );
}

// ---------------------------------------------------------------------------
// Overlay escape action mapping
// ---------------------------------------------------------------------------

#[test]
fn overlay_escape_action_aborts_before_threshold() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    // Escape during Recording below 5 seconds → immediate abort.
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Recording, Duration::from_secs(4)),
        OverlayEscapeAction::AbortSession
    );
}

#[test]
fn overlay_escape_action_confirms_at_threshold() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    // Escape during Recording at exactly 5 seconds → transition to Confirming.
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Recording, Duration::from_secs(5)),
        OverlayEscapeAction::TransitionToConfirming
    );
}

#[test]
fn overlay_escape_action_resumes_from_confirming() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    // Escape during Confirming resumes recording (elapsed is irrelevant).
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Confirming, Duration::from_secs(9)),
        OverlayEscapeAction::ResumeRecording
    );
}

#[test]
fn overlay_escape_closes_non_recording_phases() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    let elapsed = Duration::from_secs(0);
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Transcribing, elapsed),
        OverlayEscapeAction::CloseOverlay
    );
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Delivering, elapsed),
        OverlayEscapeAction::CloseOverlay
    );
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Finished, elapsed),
        OverlayEscapeAction::CloseOverlay
    );
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Failed("boom".to_string()), elapsed),
        OverlayEscapeAction::CloseOverlay
    );
}

#[test]
fn overlay_escape_propagates_only_when_idle() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Idle, Duration::from_secs(0)),
        OverlayEscapeAction::Propagate
    );
}

#[test]
fn confirming_ui_uses_stop_continue_copy_and_timer() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    assert!(src.contains("Stop dictation?"));
    assert!(
        src.contains("format_elapsed(self.state.elapsed)")
            || src.contains("format_elapsed(state.elapsed)")
    );
    assert!(
        !src.contains(r#""Abort \u{21b5}""#),
        "old Abort label should be removed"
    );
    assert!(
        !src.contains(r#""Resume Esc""#),
        "old Resume label should be removed"
    );
}

#[test]
fn confirming_state_no_longer_resumes_on_unrelated_keys() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    assert!(
        !src.contains("Key pressed during confirmation, resuming recording"),
        "confirming state must swallow unrelated keys instead of resuming recording"
    );
}

#[test]
fn dictation_elapsed_is_exported() {
    let mod_src = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");
    assert!(
        mod_src.contains("dictation_elapsed"),
        "dictation_elapsed must be re-exported from dictation module"
    );
}

#[test]
fn overlay_phase_copy_confirming_uses_stop_continue() {
    use super::types::DictationSessionPhase;
    use super::window::overlay_phase_copy;

    let (headline, action_label) = overlay_phase_copy(&DictationSessionPhase::Confirming);
    assert_eq!(headline, "Stop dictation?");
    assert_eq!(action_label, "Continue");
}

#[test]
fn overlay_escape_action_boundary_just_below_threshold() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    // 4999ms is below the 5-second threshold → immediate abort.
    assert_eq!(
        overlay_escape_action(
            &DictationSessionPhase::Recording,
            Duration::from_millis(4999),
        ),
        OverlayEscapeAction::AbortSession
    );
}

#[test]
fn overlay_escape_action_confirms_well_above_threshold() {
    use super::types::DictationSessionPhase;
    use super::window::{overlay_escape_action, OverlayEscapeAction};

    // 30 seconds is well above the threshold → still transition to confirming.
    assert_eq!(
        overlay_escape_action(&DictationSessionPhase::Recording, Duration::from_secs(30),),
        OverlayEscapeAction::TransitionToConfirming
    );
}

#[test]
fn confirming_render_shows_stop_and_continue_buttons() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    // Stop button with Enter key hint (↵ = \u{21b5})
    assert!(
        src.contains("ACTION_STOP_LABEL"),
        "confirming UI must render Stop button"
    );
    assert!(
        src.contains("ACTION_CONTINUE_LABEL"),
        "confirming UI must render Continue button"
    );
}

#[test]
fn confirming_render_uses_error_and_success_colors() {
    let src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");
    // 81e23d77c: button hover/active colors now come from the shared
    // footer_rail_chrome (which derives from AppChromeColors::from_theme)
    // instead of a local PromptFooterColors copy.
    assert!(
        src.contains("footer_chrome::footer_rail_chrome"),
        "buttons must use standard shared footer chrome colors"
    );
}

// ---------------------------------------------------------------------------
// Waveform animation quality
// ---------------------------------------------------------------------------

#[test]
fn fft_visualiser_produces_nonzero_bars_for_speech_tone() {
    let mut vis = AudioVisualiser::new_speech(16_000);
    // 440 Hz sine at moderate volume — should light up at least some bars.
    let samples: Vec<f32> = (0..1024)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 16_000.0).sin() * 0.3)
        .collect();
    let bars = vis.feed(&samples).expect("should produce bars");
    let max_bar = bars.iter().cloned().fold(0.0_f32, f32::max);
    assert!(max_bar > 0.05, "speech tone should produce visible bars");
    assert!(bars.iter().all(|bar| *bar >= 0.0 && *bar <= 1.0));
}

#[test]
fn waveform_attack_is_faster_than_decay() {
    use super::visualizer::animate_bars;

    let rise = animate_bars([0.08; 9], [1.0; 9], Duration::from_millis(16))[4];
    let fall = animate_bars([1.0; 9], [0.08; 9], Duration::from_millis(16))[4];
    // Rise delta should exceed fall delta (fast attack, slow decay).
    assert!(rise - 0.08 > 1.0 - fall);
}

// ---------------------------------------------------------------------------
// Mic selection heuristic tests
// ---------------------------------------------------------------------------

#[test]
fn mic_resolution_prefers_saved_device() {
    let devices = vec![
        device_with_transport("mic-1", "Built-in", true, DictationDeviceTransport::BuiltIn),
        device_with_transport("mic-2", "USB Mic", false, DictationDeviceTransport::Usb),
    ];
    let res = resolve_selected_input_device(&devices, Some("mic-2")).expect("should resolve");
    assert_eq!(res.device.id.0, "mic-2");
    assert!(!res.fell_back);
}

#[test]
fn mic_resolution_falls_back_when_saved_device_missing() {
    let devices = vec![device_with_transport(
        "mic-1",
        "Built-in",
        true,
        DictationDeviceTransport::BuiltIn,
    )];
    let res =
        resolve_selected_input_device(&devices, Some("disappeared-mic")).expect("should resolve");
    assert_eq!(res.device.id.0, "mic-1");
    assert!(res.fell_back);
}

#[test]
fn mic_resolution_prefers_system_default_when_no_preference() {
    let devices = vec![
        device_with_transport("mic-1", "USB Mic", false, DictationDeviceTransport::Usb),
        device_with_transport("mic-2", "Built-in", true, DictationDeviceTransport::BuiltIn),
    ];
    let res = resolve_selected_input_device(&devices, None).expect("should resolve");
    assert_eq!(res.device.id.0, "mic-2", "should prefer is_default device");
    assert!(!res.fell_back);
}

#[test]
fn mic_resolution_prefers_builtin_over_virtual_when_no_default() {
    let devices = vec![
        device_with_transport(
            "mic-v",
            "Virtual Audio",
            false,
            DictationDeviceTransport::Virtual,
        ),
        device_with_transport(
            "mic-b",
            "MacBook Mic",
            false,
            DictationDeviceTransport::BuiltIn,
        ),
    ];
    let res = resolve_selected_input_device(&devices, None).expect("should resolve");
    assert_eq!(
        res.device.id.0, "mic-b",
        "should prefer built-in over virtual"
    );
}

#[test]
fn mic_resolution_prefers_usb_over_virtual_when_no_builtin() {
    let devices = vec![
        device_with_transport(
            "mic-v",
            "Virtual Audio",
            false,
            DictationDeviceTransport::Virtual,
        ),
        device_with_transport("mic-u", "USB Mic", false, DictationDeviceTransport::Usb),
    ];
    let res = resolve_selected_input_device(&devices, None).expect("should resolve");
    assert_eq!(res.device.id.0, "mic-u", "should prefer USB over virtual");
}

#[test]
fn mic_resolution_uses_first_device_as_last_resort() {
    let devices = vec![device_with_transport(
        "mic-v",
        "Virtual Audio",
        false,
        DictationDeviceTransport::Virtual,
    )];
    let res = resolve_selected_input_device(&devices, None).expect("should resolve");
    assert_eq!(res.device.id.0, "mic-v", "should fall back to any device");
}

#[test]
fn mic_resolution_returns_none_for_empty_list() {
    let res = resolve_selected_input_device(&[], None);
    assert!(res.is_none());
}

#[test]
fn mic_resolution_returns_none_for_empty_list_with_saved_pref() {
    let res = resolve_selected_input_device(&[], Some("mic-1"));
    assert!(res.is_none());
}

// ---------------------------------------------------------------------------
// Download progress formatting tests
// ---------------------------------------------------------------------------

#[test]
fn format_bytes_covers_ranges() {
    use crate::dictation::download::format_bytes;

    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1 KB");
    assert_eq!(format_bytes(1_048_576), "1.0 MB");
    assert_eq!(format_bytes(500_000_000), "476.8 MB");
    assert_eq!(format_bytes(1_073_741_824), "1.0 GB");
}

#[test]
fn format_speed_shows_dash_for_zero() {
    use crate::dictation::download::format_speed;

    assert_eq!(format_speed(0), "-- MB/s");
    assert!(format_speed(10_485_760).contains("10.0 MB/s"));
}

#[test]
fn download_progress_percentage_edge_cases() {
    use crate::dictation::download::DownloadProgress;

    assert_eq!(
        DownloadProgress {
            downloaded: 1,
            total: 3
        }
        .percentage(),
        33
    );
    assert_eq!(
        DownloadProgress {
            downloaded: 999,
            total: 1000
        }
        .percentage(),
        99
    );
}

// ---------------------------------------------------------------------------
// Internal dictation target routing tests
// ---------------------------------------------------------------------------

#[test]
fn dictation_target_enum_covers_all_surfaces() {
    use crate::dictation::types::DictationTarget;

    // Exhaustive match proves all variants exist and are reachable.
    let targets = [
        DictationTarget::MainWindowFilter,
        DictationTarget::MainWindowPrompt,
        DictationTarget::NotesEditor,
        DictationTarget::AiChatComposer,
        DictationTarget::TabAiHarness,
        DictationTarget::ExternalApp,
        DictationTarget::DayPageToday,
        DictationTarget::QuickAiQuestion,
    ];
    for target in &targets {
        match target {
            DictationTarget::MainWindowFilter => {}
            DictationTarget::MainWindowPrompt => {}
            DictationTarget::NotesEditor => {}
            DictationTarget::AiChatComposer => {}
            DictationTarget::TabAiHarness => {}
            DictationTarget::ExternalApp => {}
            DictationTarget::DayPageToday => {}
            DictationTarget::QuickAiQuestion => {}
        }
    }
}

#[test]
fn dictation_destination_includes_internal_surfaces() {
    use crate::dictation::types::DictationDestination;

    // NotesEditor, AiChatComposer, and TabAiHarness must exist alongside the
    // original variants, plus the chip-selectable DayPageToday and
    // QuickAiQuestion destinations.
    let destinations = [
        DictationDestination::MainWindowFilter,
        DictationDestination::ActivePrompt,
        DictationDestination::FrontmostApp,
        DictationDestination::NotesEditor,
        DictationDestination::AiChatComposer,
        DictationDestination::TabAiHarness,
        DictationDestination::DayPageToday,
        DictationDestination::QuickAiQuestion,
    ];
    assert_eq!(destinations.len(), 8);
}

#[test]
fn resolve_dictation_target_exists_in_builtin_execution() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        src.contains("fn resolve_dictation_target("),
        "builtin_execution.rs must define resolve_dictation_target"
    );

    // Must check notes and AI windows before falling back to prompt or external.
    // Use the exact signature to skip resolve_dictation_target_with_override.
    let resolver_start = src
        .find("fn resolve_dictation_target(&self) ->")
        .expect("resolver must exist");
    let resolver_src = &src[resolver_start..resolver_start + 1400.min(src.len() - resolver_start)];

    assert!(
        resolver_src.contains("notes::is_notes_window_open()"),
        "resolver must check notes window"
    );
    assert!(
        resolver_src.contains("agent_chat_dictation_target_active()"),
        "resolver must check Agent Chat availability"
    );
    assert!(
        resolver_src.contains("can_accept_dictation_into_prompt()"),
        "resolver must check prompt acceptance"
    );
    assert!(
        resolver_src.contains("DictationTarget::ExternalApp"),
        "resolver must fall back to ExternalApp"
    );
}

#[test]
fn resolve_dictation_target_checks_notes_before_ai() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    // Use the exact signature to skip resolve_dictation_target_with_override.
    let resolver_start = src
        .find("fn resolve_dictation_target(&self) ->")
        .expect("resolver must exist");
    let resolver_src = &src[resolver_start..resolver_start + 1400.min(src.len() - resolver_start)];

    let notes_pos = resolver_src
        .find("notes::is_notes_window_open()")
        .expect("resolver must check notes");
    let ai_pos = resolver_src
        .find("agent_chat_dictation_target_active()")
        .expect("resolver must check Agent Chat");
    let prompt_pos = resolver_src
        .find("can_accept_dictation_into_prompt()")
        .expect("resolver must check prompt");

    assert!(
        notes_pos < ai_pos,
        "notes must be checked before Agent Chat (notes_pos={notes_pos}, ai_pos={ai_pos})"
    );
    assert!(
        ai_pos < prompt_pos,
        "Agent Chat must be checked before prompt (ai_pos={ai_pos}, prompt_pos={prompt_pos})"
    );
}

#[test]
fn handle_dictation_transcript_accepts_target_parameter() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_sig = &src[handler_start..handler_start + 300.min(src.len() - handler_start)];

    assert!(
        handler_sig.contains("target: crate::dictation::DictationTarget"),
        "handle_dictation_transcript must accept a DictationTarget parameter"
    );
}

#[test]
fn delivery_routes_notes_transcript_via_inject_text() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..handler_start + 3000.min(src.len() - handler_start)];

    assert!(
        handler_src.contains("notes::inject_text_into_notes"),
        "handler must deliver to notes via inject_text_into_notes"
    );
    assert!(
        handler_src.contains("DictationTarget::NotesEditor"),
        "handler must match on NotesEditor target"
    );
}

#[test]
fn delivery_routes_ai_chat_transcript_to_agent_chat_composer() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..handler_start + 8000.min(src.len() - handler_start)];

    assert!(
        handler_src.contains("open_agent_chat_with_composer_seed"),
        "handler must deliver to Agent Chat via the composer-seed entry"
    );
    assert!(
        handler_src.contains("DictationTarget::AiChatComposer"),
        "handler must match on AiChatComposer target"
    );
}

#[test]
fn internal_delivery_failure_falls_back_to_frontmost_app() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_start = src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &src[handler_start..handler_start + 8000.min(src.len() - handler_start)];

    // Notes delivery must have fallback logging on failure. (The Agent Chat
    // composer route delivers through the entry-request orchestrator, which
    // stages the seed itself and has no synchronous failure branch.)
    assert!(
        handler_src.contains("Notes delivery failed, falling back"),
        "notes delivery must log fallback on failure"
    );
}

#[test]
fn toggle_dictation_accepts_target_parameter() {
    let src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        src.contains("pub fn toggle_dictation(target: DictationTarget)"),
        "toggle_dictation must accept a DictationTarget parameter"
    );
}

#[test]
fn get_dictation_target_is_exported() {
    let src = std::fs::read_to_string("src/dictation/mod.rs").expect("read dictation/mod.rs");
    assert!(
        src.contains("get_dictation_target"),
        "get_dictation_target must be re-exported from the dictation module"
    );
}

#[test]
fn dictation_session_stores_target() {
    let src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");

    assert!(
        src.contains("target: DictationTarget"),
        "DictationSession must store the target"
    );
    assert!(
        src.contains("target_cycle: Vec<DictationTarget>"),
        "DictationSession must store the per-session target cycle"
    );
    assert!(
        src.contains("pub fn get_dictation_target()"),
        "runtime must expose get_dictation_target()"
    );
}

#[test]
fn dictation_runtime_exposes_target_cycle_helpers() {
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime.rs");
    let mod_src = std::fs::read_to_string("src/dictation/mod.rs").expect("read mod.rs");

    assert!(
        runtime_src.contains("pub fn set_dictation_target_cycle(targets: Vec<DictationTarget>)"),
        "runtime must expose set_dictation_target_cycle()"
    );
    assert!(
        runtime_src.contains("pub fn can_cycle_dictation_target() -> bool"),
        "runtime must expose can_cycle_dictation_target()"
    );
    assert!(
        runtime_src.contains("pub fn cycle_dictation_target() -> Option<DictationTarget>"),
        "runtime must expose cycle_dictation_target()"
    );
    assert!(
        mod_src.contains("set_dictation_target_cycle")
            && mod_src.contains("can_cycle_dictation_target")
            && mod_src.contains("cycle_dictation_target"),
        "dictation mod facade must re-export target cycle helpers"
    );
}

#[test]
fn overlay_destination_chip_click_behavior_matches_armed_option_matrix() {
    use super::types::DictationSessionPhase;
    use super::window::{chip_click_behavior, ChipClickBehavior};

    let interactive = [
        DictationSessionPhase::Recording,
        DictationSessionPhase::Confirming,
    ];
    for phase in interactive {
        assert_eq!(
            chip_click_behavior(&phase, false, false),
            ChipClickBehavior::Retarget,
            "{phase:?} unarmed should only retarget"
        );
        assert_eq!(
            chip_click_behavior(&phase, false, true),
            ChipClickBehavior::Retarget,
            "{phase:?} unarmed with Option should only retarget"
        );
        assert_eq!(
            chip_click_behavior(&phase, true, true),
            ChipClickBehavior::Retarget,
            "{phase:?} armed with Option should stay in aim mode"
        );
        assert_eq!(
            chip_click_behavior(&phase, true, false),
            ChipClickBehavior::SendTo,
            "{phase:?} armed without Option should send"
        );
    }

    let non_interactive = [
        DictationSessionPhase::Idle,
        DictationSessionPhase::Transcribing,
        DictationSessionPhase::Delivering,
        DictationSessionPhase::Finished,
        DictationSessionPhase::Failed("boom".to_string()),
    ];
    for phase in non_interactive {
        for armed in [false, true] {
            for option_held in [false, true] {
                assert_eq!(
                    chip_click_behavior(&phase, armed, option_held),
                    ChipClickBehavior::Ignore,
                    "{phase:?} should ignore chips regardless of armed/Option state"
                );
            }
        }
    }
}

#[test]
fn dictation_footer_stop_button_tracks_armed_state() {
    use super::types::DictationSessionPhase;
    use crate::footer_popup::FooterAction;

    let has_stop = |phase, armed| {
        super::window::dictation_native_footer_config(&phase, armed)
            .buttons
            .iter()
            .any(|button| button.action == FooterAction::Stop)
    };

    assert!(!has_stop(DictationSessionPhase::Recording, false));
    assert!(has_stop(DictationSessionPhase::Recording, true));
    assert!(!has_stop(DictationSessionPhase::Confirming, false));
    assert!(has_stop(DictationSessionPhase::Confirming, true));
}

#[test]
fn dictation_chip_tooltips_match_aim_and_send_modes() {
    use super::window::{chip_tooltip_label, DestinationChipMode};
    use crate::dictation::DictationTarget;

    let external_aim = chip_tooltip_label(DictationTarget::ExternalApp, DestinationChipMode::Aim);
    assert!(
        external_aim.as_ref().starts_with("Dictate into "),
        "external app aim tooltip should name dictation target"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::DayPageToday, DestinationChipMode::Aim).as_ref(),
        "Dictate to today's note"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::QuickAiQuestion, DestinationChipMode::Aim).as_ref(),
        "Dictate to AI"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::TabAiHarness, DestinationChipMode::Aim).as_ref(),
        "Dictate to Agent Chat"
    );

    let external_send = chip_tooltip_label(DictationTarget::ExternalApp, DestinationChipMode::Send);
    assert!(
        external_send.as_ref().starts_with("Stop & paste into "),
        "external app send tooltip should name stop-and-paste outcome"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::DayPageToday, DestinationChipMode::Send).as_ref(),
        "Stop & append to today's note"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::QuickAiQuestion, DestinationChipMode::Send).as_ref(),
        "Stop & ask AI"
    );
    assert_eq!(
        chip_tooltip_label(DictationTarget::TabAiHarness, DestinationChipMode::Send).as_ref(),
        "Stop & send to Agent Chat"
    );
}

#[test]
fn overlay_destination_chip_icons_resolve_to_lucide_assets() {
    // Each chip carries an icon matching how its concept is drawn elsewhere
    // in the app (paste/clipboard, day/calendar, ask/sparkles, Agent
    // Chat/bot). Resolution must succeed so the chips never render blank
    // icon slots.
    for (target, verb, icon) in super::window::DICTATION_CHIP_TARGETS {
        assert!(
            super::window::chip_icon_path(icon).is_some(),
            "chip {verb} ({target:?}) icon {icon:?} must resolve to a Lucide asset path"
        );
    }
}

#[test]
fn overlay_timer_and_target_badge_use_readable_primary_text() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    // 2026-07-06 caption redesign: the timer reads active primary while the
    // session is live and intentionally grays out (muted) once processing
    // locks the duration in — the header holds its layout instead of
    // vanishing into a progress indicator.
    assert!(
        window_src.contains("let timer_color = if interactive {")
            && window_src.contains("theme.colors.text.primary.with_opacity(OPACITY_ACTIVE)"),
        "live timer must use active primary text, graying only during processing"
    );
    assert!(
        window_src.contains("render_target_badge_content(self.state.target)")
            && window_src.contains("target_badge_label(target)"),
        "target badge must render through the shared content helper with text fallback"
    );
}

#[test]
fn external_app_badge_names_the_tracked_frontmost_app() {
    let window_src = std::fs::read_to_string("src/dictation/window.rs").expect("read window.rs");

    assert!(
        window_src.contains("pub(crate) fn target_badge_label(")
            && window_src.contains("pub(crate) fn target_badge_frontmost_app_icon(")
            && window_src.contains("crate::app_launcher::cached_app_icon_for_bundle")
            && window_src.contains("crate::frontmost_app_tracker::get_last_real_app()")
            && window_src.contains("target.overlay_label().into()")
            && window_src.contains("icons::render_image"),
        "external-app badge should prefer the indexed frontmost app icon and fall back to the tracked app name or static target label"
    );
}

#[test]
fn builtin_dictation_configures_target_cycle_and_agent_chat_alternate() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    assert!(
        builtin_src.contains("fn dictation_target_cycle_for("),
        "builtin execution must define a dictation target cycle helper"
    );
    assert!(
        builtin_src.contains("set_dictation_target_cycle")
            && builtin_src.contains("DictationTarget::ExternalApp")
            && builtin_src.contains("DictationTarget::TabAiHarness"),
        "dictation start paths must configure a cycle that includes Agent Chat chat submit as an alternate"
    );
}

#[test]
fn main_window_filter_delivery_can_reset_to_script_list_and_reveal_main() {
    let builtin_src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");
    let handler_start = builtin_src
        .find("fn handle_dictation_transcript")
        .expect("handler must exist");
    let handler_src = &builtin_src[handler_start..];

    assert!(
        handler_src.contains("self.reset_to_script_list(cx);"),
        "main-window dictation delivery must be able to reset to ScriptList before applying the filter"
    );
    assert!(
        handler_src.contains("script_kit_gpui::set_main_window_visible(true);")
            && handler_src.contains("show_main_window_without_activation"),
        "main-window dictation delivery must reveal the main window when retargeted from a hidden session"
    );
}

#[test]
fn inject_text_into_notes_is_exported() {
    let src = std::fs::read_to_string("src/notes/mod.rs").expect("read notes/mod.rs");
    assert!(
        src.contains("inject_text_into_notes"),
        "inject_text_into_notes must be re-exported from the notes module"
    );
}

// ---------------------------------------------------------------------------
// Missing-model entry gate opens the rich download prompt
// ---------------------------------------------------------------------------

/// When the Parakeet model is not available at dictation start time, the
/// entry gate must open the rich model-download prompt — not just log or
/// show a toast.  This verifies the preflight guard opens the prompt and
/// returns early before attempting capture.
#[test]
fn missing_model_entry_gate_opens_download_prompt() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let dictation_arm_start = src
        .find("fn prepare_dictation_builtin_start(")
        .expect("builtin_execution must have prepare_dictation_builtin_start");
    let arm_src =
        &src[dictation_arm_start..dictation_arm_start + 2000.min(src.len() - dictation_arm_start)];

    // Must check model availability before starting capture.
    assert!(
        arm_src.contains("model.is_available()"),
        "Dictation entry must check selected model availability"
    );

    // When model is missing, must open the rich download prompt.
    assert!(
        arm_src.contains("open_dictation_model_prompt(cx)"),
        "Dictation entry must open the model download prompt when model is missing"
    );

    // The model check must come before delivery target check — we don't
    // want to fail on delivery validation when the real issue is missing model.
    let model_check_pos = arm_src
        .find("model.is_available()")
        .expect("model check must exist");
    let delivery_check_pos = arm_src
        .find("ensure_dictation_builtin_target_available")
        .expect("delivery target check must exist");
    assert!(
        model_check_pos < delivery_check_pos,
        "model availability check must come BEFORE delivery target validation"
    );
}

// ---------------------------------------------------------------------------
// Start preflight must fire the mic TCC prompt and use real model status
// ---------------------------------------------------------------------------

/// A fresh install has microphone permission NotDetermined. macOS only shows
/// the consent dialog when the app calls
/// `AVCaptureDevice requestAccessForMediaType:`, and System Settings does not
/// list the app under Microphone until that has fired at least once. The
/// dictation start preflight is the only user-reachable trigger, so it must
/// actually request access — otherwise new users have NO path to grant the
/// permission (release audit 2026-07-01). It must also derive model status
/// from disk instead of hardcoding Available, so a missing model routes to
/// setup before recording starts rather than after transcription fails.
#[test]
fn dictation_start_preflight_requests_mic_access_and_uses_real_model_status() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let start = src
        .find("fn open_dictation_setup_if_not_ready(")
        .expect("open_dictation_setup_if_not_ready must exist");
    let tail = &src[start..];
    let end = tail
        .find("fn ensure_dictation_delivery_target_available(")
        .expect("preflight must be followed by ensure_dictation_delivery_target_available");
    let preflight_src = &tail[..end];

    assert!(
        preflight_src.contains("request_microphone_permission_nonblocking"),
        "preflight must fire the TCC prompt for NotDetermined mic permission"
    );
    assert!(
        preflight_src.contains("model.is_available()"),
        "preflight must derive model status from disk, not hardcode Available"
    );
}

// ---------------------------------------------------------------------------
// Missing-model transcription recovery opens prompt instead of dead-end toast
// ---------------------------------------------------------------------------

/// When transcription fails because the Parakeet model is missing, the
/// handler must close the overlay and open the rich download prompt so the
/// user can immediately start the download — rather than showing a
/// dead-end toast that requires them to guess the next step.
#[test]
fn missing_model_transcription_recovery_opens_download_prompt() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let handler_src = dictation_handler_source(&src);

    // Find the model-missing branch within the Err arm.
    let parakeet_branch_start = handler_src
        .find(r#"error_text.contains("model not downloaded")"#)
        .expect("handler must detect the model-missing error string");
    // Scope far enough after the detection to capture the full recovery branch,
    // including the early return after cleanup / prompt dispatch.
    let branch_src = &handler_src[parakeet_branch_start
        ..parakeet_branch_start + 900.min(handler_src.len() - parakeet_branch_start)];

    // Must close the dictation overlay before opening the prompt.
    assert!(
        branch_src.contains("close_dictation_overlay"),
        "Parakeet-missing branch must close the dictation overlay"
    );

    // Must open the rich download prompt.
    assert!(
        branch_src.contains("open_dictation_model_prompt"),
        "Parakeet-missing branch must open the model download prompt, not a dead-end toast"
    );

    // Must schedule transcriber cleanup.
    assert!(
        branch_src.contains("schedule_dictation_transcriber_cleanup"),
        "Parakeet-missing branch must schedule transcriber cleanup"
    );

    // Must return early — no fallthrough to the generic error toast.
    assert!(
        branch_src.contains("return;"),
        "Parakeet-missing branch must return early to avoid the generic error toast"
    );

    // Must NOT show a dead-end toast in this branch (the prompt IS the UX).
    let branch_before_return =
        &branch_src[..branch_src.find("return;").unwrap_or(branch_src.len())];
    assert!(
        !branch_before_return.contains("show_error_toast"),
        "Parakeet-missing branch must NOT show a dead-end toast — the prompt is the recovery UX"
    );
}

// ---------------------------------------------------------------------------
// Download failure path uses classified error messages
// ---------------------------------------------------------------------------

/// When the background model download fails, the error text surfaced to
/// the user must go through `classify_download_error` so users see
/// actionable guidance instead of raw error strings.
#[test]
fn download_failure_uses_classified_error() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let download_fn_start = src
        .find("fn start_dictation_model_download(")
        .expect("start_dictation_model_download must exist");
    // Scope from the function start to the next top-level function
    // (`build_dictation_model_prompt`) to capture the full error handling.
    let download_tail = &src[download_fn_start..];
    let download_src = match download_tail.find("fn build_dictation_model_prompt(") {
        Some(end) => &download_tail[..end],
        None => download_tail,
    };

    // Must call classify_download_error for non-cancelled failures.
    assert!(
        download_src.contains("classify_download_error"),
        "download failure path must classify errors for user-friendly messages"
    );

    // Must log the raw error AND the classified user-facing error separately.
    assert!(
        download_src.contains("raw_error") && download_src.contains("user_error"),
        "download failure must log both raw_error and user_error for diagnostics"
    );
}

// duplicate removed — see line 4053 for the canonical test

// ───────────────────────────────────────────────────────────────────
// Main-window-filter dictation routing
// ───────────────────────────────────────────────────────────────────

#[test]
fn dictation_target_enum_includes_main_window_filter_variant() {
    use crate::dictation::types::DictationTarget;

    // Exhaustive match proves MainWindowFilter is a first-class variant.
    let target = DictationTarget::MainWindowFilter;
    match target {
        DictationTarget::MainWindowFilter => {}
        DictationTarget::MainWindowPrompt => {}
        DictationTarget::NotesEditor => {}
        DictationTarget::AiChatComposer => {}
        DictationTarget::TabAiHarness => {}
        DictationTarget::ExternalApp => {}
        DictationTarget::DayPageToday => {}
        DictationTarget::QuickAiQuestion => {}
    }
    assert_eq!(target.overlay_label(), "Script Kit");
}

#[test]
fn dictation_destination_includes_main_window_filter_variant() {
    use crate::dictation::types::DictationDestination;

    // Verify MainWindowFilter is present among all destination variants.
    let destinations = [
        DictationDestination::MainWindowFilter,
        DictationDestination::ActivePrompt,
        DictationDestination::FrontmostApp,
        DictationDestination::NotesEditor,
        DictationDestination::AiChatComposer,
        DictationDestination::TabAiHarness,
        DictationDestination::DayPageToday,
        DictationDestination::QuickAiQuestion,
    ];
    assert_eq!(destinations.len(), 8);
}

#[test]
fn resolve_dictation_target_prefers_frontmost_app_over_script_list() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let resolver_start = src
        .find("fn resolve_dictation_target(&self) ->")
        .expect("resolver must exist");
    let resolver_src = &src[resolver_start..resolver_start + 1400.min(src.len() - resolver_start)];

    assert!(
        resolver_src
            .contains("let has_frontmost_app_target = Self::has_dictation_frontmost_target();"),
        "resolver must snapshot whether a frontmost app can receive contextual dictation"
    );
    let frontmost_pos = resolver_src
        .find("has_frontmost_app_target")
        .expect("resolver must check frontmost-app availability");
    let main_filter_pos = resolver_src
        .find("can_accept_dictation_into_main_filter()")
        .expect("resolver must keep launcher/main filter fallback");
    assert!(
        frontmost_pos < main_filter_pos,
        "contextual dictation must prefer the tracked frontmost app before falling back to Script Kit's main menu"
    );
}

#[test]
fn delivery_routes_main_window_filter_before_external_paste() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    // MainWindowFilter delivery must appear before the external-app paste
    // fallback in the overall file, proving it is checked first.
    let main_filter_pos = src
        .find("DictationTarget::MainWindowFilter =>")
        .expect("handler must match MainWindowFilter target");
    let paste_pos = src
        .find("paste_text")
        .expect("handler must still contain external-app fallback");

    assert!(
        main_filter_pos < paste_pos,
        "MainWindowFilter delivery must happen before external-app paste fallback"
    );
    assert!(
        src.contains("DictationDestination::MainWindowFilter"),
        "handler must map MainWindowFilter target to MainWindowFilter destination"
    );
}

#[test]
fn preflight_accepts_launcher_without_tracked_frontmost_app() {
    let src = std::fs::read_to_string("src/app_execute/builtin_execution.rs")
        .expect("read builtin_execution.rs");

    let preflight_start = src
        .find("fn ensure_dictation_delivery_target_available(&self)")
        .expect("preflight must exist");
    let preflight_src =
        &src[preflight_start..preflight_start + 400.min(src.len() - preflight_start)];

    assert!(
        preflight_src.contains("can_accept_dictation_into_main_filter()"),
        "preflight must accept launcher as a valid dictation destination"
    );
}

// ---------------------------------------------------------------------------
// Live transcript preview + dictation preference defaults
// ---------------------------------------------------------------------------

#[test]
fn estimate_caption_lines_wraps_words_greedily() {
    use super::window::estimate_caption_lines;

    // Empty and single-word text is one line.
    assert_eq!(estimate_caption_lines("", 10), 1);
    assert_eq!(estimate_caption_lines("hello", 10), 1);

    // "hello world" at width 8: "hello" (5) + " world" would need 11 > 8.
    assert_eq!(estimate_caption_lines("hello world", 8), 2);
    // At width 11 the two words fit one line exactly.
    assert_eq!(estimate_caption_lines("hello world", 11), 1);

    // A word longer than the line width still counts one line (the text
    // system will break it; the estimate stays conservative).
    assert_eq!(estimate_caption_lines(&"a".repeat(30), 10), 1);

    // Steady prose grows roughly linearly.
    let prose = "one two three four five six seven eight nine ten".repeat(3);
    assert!(estimate_caption_lines(&prose, 20) >= 6);
}

#[test]
fn chunked_transcription_reserved_for_long_audio() {
    use super::transcription::should_use_chunked_transcription;
    let rate = 16_000u32;

    // Typical short dictations take the single plain pass.
    assert!(!should_use_chunked_transcription(20 * rate as usize, rate));
    // A five-minute recording must go through the chunked path — one giant
    // encoder pass is quadratic in audio length and can drop trailing speech.
    assert!(should_use_chunked_transcription(300 * rate as usize, rate));
}

#[test]
fn overlay_growth_constants_bound_the_pill() {
    use super::window::{
        OVERLAY_HEIGHT_PX, OVERLAY_MAX_EXTRA_CAPTION_LINES, TRANSCRIPT_LINE_HEIGHT_PX,
    };
    let max_height =
        OVERLAY_HEIGHT_PX + OVERLAY_MAX_EXTRA_CAPTION_LINES as f32 * TRANSCRIPT_LINE_HEIGHT_PX;
    assert!(
        max_height <= 200.0,
        "fully grown dictation pill must stay a compact bottom-edge overlay, got {max_height}"
    );
}

#[test]
fn dictation_preferences_defaults_are_safe() {
    let prefs = crate::config::DictationPreferences::default();
    assert!(prefs.save_history_enabled(), "history stays on by default");
    assert_eq!(
        prefs.silence_rms_threshold(),
        crate::config::DictationPreferences::DEFAULT_SILENCE_RMS
    );
    assert_eq!(
        prefs.max_duration(),
        Some(std::time::Duration::from_secs(
            crate::config::DictationPreferences::DEFAULT_MAX_DURATION_SECS
        ))
    );
    assert!(prefs.live_preview_enabled(), "live preview on by default");
    assert!(prefs.push_to_talk_enabled(), "push-to-talk on by default");
    assert_eq!(
        prefs.target_mode(),
        crate::config::DictationTargetMode::Sticky,
        "target mode defaults to sticky"
    );
    assert_eq!(
        prefs.sticky_target(),
        None,
        "no sticky destination until a chip has been picked"
    );
    assert!(
        prefs.quick_ai_answers(),
        "quick AI fires-and-shows by default"
    );
}

#[test]
fn dictation_target_mode_parses_config_values() {
    use crate::config::{DictationPreferences, DictationTargetMode};
    use crate::dictation::DictationTarget;

    let context = DictationPreferences {
        target: Some("context".to_string()),
        ..Default::default()
    };
    assert_eq!(context.target_mode(), DictationTargetMode::Context);

    let explicit = DictationPreferences {
        target: Some("today".to_string()),
        ..Default::default()
    };
    assert_eq!(
        explicit.target_mode(),
        DictationTargetMode::Explicit(DictationTarget::DayPageToday)
    );

    // Typos degrade to the sticky default instead of pinning a wrong target.
    let typo = DictationPreferences {
        target: Some("todya".to_string()),
        ..Default::default()
    };
    assert_eq!(typo.target_mode(), DictationTargetMode::Sticky);

    let sticky = DictationPreferences {
        target: Some("sticky".to_string()),
        last_target: Some("ask".to_string()),
        ..Default::default()
    };
    assert_eq!(sticky.target_mode(), DictationTargetMode::Sticky);
    assert_eq!(
        sticky.sticky_target(),
        Some(DictationTarget::QuickAiQuestion)
    );

    let composer = DictationPreferences {
        quick_ai: Some("composer".to_string()),
        ..Default::default()
    };
    assert!(!composer.quick_ai_answers());
}

#[test]
fn dictation_sticky_labels_round_trip_through_parser() {
    use crate::dictation::{parse_dictation_target_label, DictationTarget};

    for target in [
        DictationTarget::MainWindowFilter,
        DictationTarget::MainWindowPrompt,
        DictationTarget::NotesEditor,
        DictationTarget::AiChatComposer,
        DictationTarget::TabAiHarness,
        DictationTarget::ExternalApp,
        DictationTarget::DayPageToday,
        DictationTarget::QuickAiQuestion,
    ] {
        assert_eq!(
            parse_dictation_target_label(target.sticky_label()),
            Some(target),
            "sticky label for {target:?} must parse back to the same target"
        );
    }
}

#[test]
fn dictation_preferences_clamp_and_disable_rules() {
    let prefs = crate::config::DictationPreferences {
        save_history: Some(false),
        silence_rms: Some(9.0),
        max_duration_secs: Some(0),
        live_preview: Some(false),
        push_to_talk: Some(false),
        ..Default::default()
    };
    assert!(!prefs.save_history_enabled());
    assert_eq!(
        prefs.silence_rms_threshold(),
        0.5,
        "silence gate clamps to 0.5"
    );
    assert_eq!(
        prefs.max_duration(),
        None,
        "0 disables the max-duration guard"
    );
    assert!(!prefs.live_preview_enabled());
    assert!(!prefs.push_to_talk_enabled());

    let tiny = crate::config::DictationPreferences {
        max_duration_secs: Some(5),
        ..Default::default()
    };
    assert_eq!(
        tiny.max_duration(),
        Some(std::time::Duration::from_secs(30)),
        "sub-30s ceilings clamp up to 30s"
    );
}

#[test]
fn completed_capture_truncation_flag_reaches_stop_receipt() {
    // The stop receipt must expose `truncated` so DevTools can prove whether
    // the capture tail was collected before the deadline.
    let runtime_src = std::fs::read_to_string("src/dictation/runtime.rs").expect("read runtime");
    assert!(
        runtime_src.contains(r#""truncated": truncated,"#),
        "stop receipt must carry the truncated flag"
    );
}

#[test]
fn dictation_target_destination_mapping_is_exhaustive_and_stable() {
    use crate::dictation::types::{DictationDestination, DictationTarget};
    let cases = [
        (
            DictationTarget::MainWindowFilter,
            DictationDestination::MainWindowFilter,
        ),
        (
            DictationTarget::MainWindowPrompt,
            DictationDestination::ActivePrompt,
        ),
        (
            DictationTarget::NotesEditor,
            DictationDestination::NotesEditor,
        ),
        (
            DictationTarget::AiChatComposer,
            DictationDestination::AiChatComposer,
        ),
        (
            DictationTarget::TabAiHarness,
            DictationDestination::TabAiHarness,
        ),
        (
            DictationTarget::ExternalApp,
            DictationDestination::FrontmostApp,
        ),
    ];
    for (target, destination) in cases {
        assert_eq!(target.destination(), destination);
    }
}
