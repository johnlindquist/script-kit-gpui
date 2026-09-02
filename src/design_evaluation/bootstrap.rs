use std::collections::{BTreeSet, HashSet};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::{ensure, Context as _, Result};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use flate2::{write::ZlibEncoder, Compression};
use sha2::{Digest, Sha256};

use crate::protocol::{
    EvaluationLimits, OwnedResponseEncoding, OwnedRuntimeIdentity, OWNED_EVALUATION_LIMITS,
    OWNED_RESPONSE_CODEC,
};
use crate::runtime_policy::{install_owned_evaluation, OwnedEvaluationPolicy};

pub(super) const GUARDS: [&str; 8] = [
    "earlyBootstrap",
    "applicationEffects",
    "nativePlatform",
    "hiddenWindows",
    "localInput",
    "ownedStorage",
    "boundedProgress",
    "renderReadback",
];

pub(super) struct Bootstrap {
    pub identity: OwnedRuntimeIdentity,
    pub root: PathBuf,
    pub launch_nonce: String,
    pub policy_sha256: String,
    pub fixture_ids: BTreeSet<String>,
    pub limits: EvaluationLimits,
}

fn required(name: &str) -> Result<String> {
    let value = std::env::var(name).with_context(|| format!("missing_{name}"))?;
    ensure!(!value.is_empty(), "empty_evaluation_setting");
    Ok(value)
}

fn sha256_file(path: &std::path::Path) -> Result<String> {
    let mut source = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = [0u8; 64 * 1024];
    loop {
        let count = source.read(&mut bytes)?;
        if count == 0 {
            break;
        }
        digest.update(&bytes[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn hash_value(name: &str) -> Result<String> {
    let value = required(name)?;
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "invalid_evaluation_digest"
    );
    Ok(value)
}

impl Bootstrap {
    fn install() -> Result<Self> {
        ensure!(
            std::env::args_os().skip(1).collect::<Vec<_>>() == ["--owned-ui-evaluation"],
            "evaluation_arguments_invalid"
        );
        ensure!(
            required("SCRIPT_KIT_NONINTERACTIVE")? == "1",
            "evaluation_requires_noninteractive"
        );
        ensure!(
            required("SCRIPT_KIT_OWNED_EVALUATION")? == "1",
            "evaluation_mode_invalid"
        );
        ensure!(
            required("PATH")? == "/usr/bin:/bin:/usr/sbin:/sbin",
            "evaluation_path_invalid"
        );
        for (name, value) in std::env::vars() {
            if name.starts_with("SCRIPT_KIT_ALLOW_") {
                ensure!(value == "0", "evaluation_unsafe_opt_in");
            }
            ensure!(
                ![
                    "OPENAI_API_KEY",
                    "ANTHROPIC_API_KEY",
                    "GEMINI_API_KEY",
                    "CODEX_API_KEY",
                    "GITHUB_TOKEN",
                    "GH_TOKEN"
                ]
                .contains(&name.as_str()),
                "evaluation_credentials_present"
            );
        }
        let root = PathBuf::from(required("SCRIPT_KIT_OWNED_EVALUATION_ROOT")?);
        let instance = required("SCRIPT_KIT_PROCESS_INSTANCE_ID")?;
        let generation = required("SCRIPT_KIT_SESSION_GENERATION")?;
        uuid::Uuid::parse_str(&instance).context("evaluation_process_identity_invalid")?;
        uuid::Uuid::parse_str(&generation).context("evaluation_session_identity_invalid")?;
        let policy = OwnedEvaluationPolicy::new(&root, instance.clone(), generation.clone())?;
        for name in [
            "HOME",
            "SK_PATH",
            "CODEX_HOME",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
            "XDG_CACHE_HOME",
            "TMPDIR",
        ] {
            let path = PathBuf::from(required(name)?);
            policy.require_owned_path(&path)?;
            ensure!(path.is_dir(), "evaluation_directory_missing");
        }
        let launch_nonce = required("SCRIPT_KIT_OWNED_EVALUATION_NONCE")?;
        uuid::Uuid::parse_str(&launch_nonce).context("evaluation_nonce_invalid")?;
        let limits: EvaluationLimits =
            serde_json::from_str(&required("SCRIPT_KIT_OWNED_EVALUATION_LIMITS")?)?;
        let mut maximum_limits = limits;
        maximum_limits.max_lifetime_ms = OWNED_EVALUATION_LIMITS.max_lifetime_ms;
        ensure!(
            maximum_limits == OWNED_EVALUATION_LIMITS
                && limits.max_lifetime_ms > 0
                && limits.max_lifetime_ms <= OWNED_EVALUATION_LIMITS.max_lifetime_ms,
            "evaluation_limits_mismatch"
        );
        let policy_text = format!(
            "{{\"version\":1,\"limits\":{},\"guards\":{}}}",
            serde_json::to_string(&limits)?,
            serde_json::to_string(&GUARDS)?
        );
        let policy_sha256 = format!("{:x}", Sha256::digest(policy_text.as_bytes()));
        ensure!(
            policy_sha256 == hash_value("SCRIPT_KIT_OWNED_EVALUATION_POLICY_SHA256")?,
            "evaluation_policy_mismatch"
        );
        let binary_sha256 = hash_value("SCRIPT_KIT_OWNED_EVALUATION_BINARY_SHA256")?;
        ensure!(
            sha256_file(&std::env::current_exe()?)? == binary_sha256,
            "evaluation_binary_mismatch"
        );
        let manifest_sha256 = hash_value("SCRIPT_KIT_OWNED_EVALUATION_MANIFEST_SHA256")?;
        let requested: Vec<String> =
            serde_json::from_str(&required("SCRIPT_KIT_OWNED_EVALUATION_FIXTURES")?)?;
        let fixture_ids: BTreeSet<_> = requested.iter().cloned().collect();
        ensure!(
            requested.len() <= 512 && requested.len() == fixture_ids.len(),
            "evaluation_fixture_subset_invalid"
        );
        for id in &fixture_ids {
            ensure!(
                !id.is_empty()
                    && id.len() <= 160
                    && id
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || b"._/-".contains(&byte)),
                "evaluation_fixture_id_invalid"
            );
        }
        let identity = OwnedRuntimeIdentity {
            pid: std::process::id(),
            // The supervisor observes this PID before gated exec. Driver compares
            // this value with its independently retained process identity.
            process_start_time: required("SCRIPT_KIT_PROCESS_START_TIME")?,
            process_instance_id: instance,
            session_generation: generation,
            binary_sha256,
            manifest_sha256,
        };
        install_owned_evaluation(policy)?;
        Ok(Self {
            identity,
            root,
            launch_nonce,
            policy_sha256,
            fixture_ids,
            limits,
        })
    }
}

fn input_channel() -> mpsc::Receiver<Result<serde_json::Value, &'static str>> {
    let (sender, receiver) = mpsc::sync_channel(16);
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        let mut input = stdin.lock();
        let mut buffer = Vec::with_capacity(16 * 1024);
        loop {
            let value = match crate::stdin_commands::read_stdin_line_bounded(
                &mut input,
                &mut buffer,
                16 * 1024,
            ) {
                Ok(crate::stdin_commands::StdinLineRead::Eof) => break,
                Ok(crate::stdin_commands::StdinLineRead::Line(line)) if line.trim().is_empty() => {
                    continue
                }
                Ok(crate::stdin_commands::StdinLineRead::Line(line)) => {
                    serde_json::from_str(&line).map_err(|_| "invalid_json")
                }
                Ok(crate::stdin_commands::StdinLineRead::TooLong { .. }) => {
                    Err("stdin_line_too_long")
                }
                Err(_) => Err("stdin_read_failed"),
            };
            let terminal = value.is_err();
            if sender.send(value).is_err() || terminal {
                break;
            }
        }
    });
    receiver
}

fn encode_response(
    reply: &serde_json::Value,
    encoding: Option<OwnedResponseEncoding>,
) -> Result<Vec<u8>> {
    let decoded = serde_json::to_vec(reply)?;
    ensure!(
        decoded.len() <= OWNED_RESPONSE_CODEC.max_decoded_bytes,
        "evaluation_response_too_large"
    );
    match encoding {
        None => return Ok(decoded),
        Some(OwnedResponseEncoding::ZlibJsonBase64V1) => {}
    }
    let decoded_bytes = decoded.len();
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&decoded)?;
    let compressed = encoder
        .finish()
        .context("evaluation_response_compression_failed")?;
    ensure!(
        compressed.len() <= OWNED_RESPONSE_CODEC.max_compressed_bytes,
        "evaluation_compressed_response_too_large"
    );
    drop(decoded);
    let encoded = serde_json::to_vec(&serde_json::json!({
        "type": OWNED_RESPONSE_CODEC.response_type,
        "version": OWNED_RESPONSE_CODEC.version,
        "encoding": encoding,
        "protocolVersion": reply["protocolVersion"],
        "requestId": reply["requestId"],
        "responseType": reply["type"].as_str().context("evaluation_response_type_required")?,
        "decodedBytes": decoded_bytes,
        "compressedBytes": compressed.len(),
        "payload": STANDARD.encode(&compressed),
    }))?;
    ensure!(
        encoded.len() < OWNED_RESPONSE_CODEC.max_decoded_bytes,
        "evaluation_response_too_large"
    );
    Ok(encoded)
}

pub(super) fn run() -> Result<()> {
    let bootstrap = Bootstrap::install()?;
    let deadline = Instant::now() + Duration::from_millis(bootstrap.limits.max_lifetime_ms);
    let max_requests = bootstrap.limits.max_requests;
    let mut evaluator = super::runtime::Evaluator::new(bootstrap)?;
    let input = input_channel();
    let stdout = std::io::stdout();
    let mut request_ids = HashSet::new();
    let mut shutdown_reason = "error";
    let result = (|| -> Result<()> {
        loop {
            if Instant::now() >= deadline {
                shutdown_reason = "lifetimeExpired";
                // Lifetime expiry is a requested resource boundary, like EOF,
                // not an evaluator failure. Cleanup and reference teardown below
                // still must succeed before the final observation and zero exit.
                break;
            }
            let mut value = match input.recv_timeout(Duration::from_millis(2)) {
                Ok(value) => value.map_err(anyhow::Error::msg)?,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    // Advance timers, tasks and effects, but reserve paints for
                    // explicit progress commands. A completed-frame token must
                    // not be invalidated by an idle repaint before readback.
                    evaluator.tick(false)?;
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    shutdown_reason = "inputEof";
                    break;
                }
            };
            crate::protocol::version::read_wire_version(&value)?;
            ensure!(
                value["protocolVersion"].as_u64()
                    == Some(u64::from(
                        crate::protocol::version::CURRENT_PROTOCOL_VERSION
                    )),
                "evaluation_protocol_version_required"
            );
            let request_id = value["requestId"]
                .as_str()
                .filter(|value| !value.is_empty())
                .context("evaluation_request_id_required")?
                .to_owned();
            ensure!(
                request_ids.len() < max_requests as usize,
                "evaluation_request_budget_exhausted"
            );
            ensure!(
                request_ids.insert(request_id.clone()),
                "evaluation_duplicate_request_id"
            );
            // Strip the transport option before command deserialization or any application effect.
            let encoding = value
                .as_object_mut()
                .and_then(|fields| fields.remove("responseEncoding"))
                .map(serde_json::from_value::<OwnedResponseEncoding>)
                .transpose();
            let (encoding, mut reply) = match encoding {
                Ok(encoding) => (encoding, evaluator.request(&request_id, value)),
                Err(_) => (
                    None,
                    serde_json::json!({
                        "type": "error", "code": "response_encoding_invalid",
                        "message": "Invalid owned response encoding",
                    }),
                ),
            };
            reply["protocolVersion"] = crate::protocol::version::CURRENT_PROTOCOL_VERSION.into();
            reply["requestId"] = request_id.into();
            let encoded = encode_response(&reply, encoding)?;
            let mut output = stdout.lock();
            output.write_all(&encoded)?;
            output.write_all(b"\n")?;
            output.flush()?;
            drop(output);
            if evaluator.ended() {
                shutdown_reason = "explicitEnd";
                break;
            }
        }
        Ok(())
    })();
    let cleanup = evaluator.close();
    let reply = evaluator.lifecycle_observation(shutdown_reason, cleanup.is_ok());
    // Match GPUI's test teardown: keep the leak authority alive until every
    // context/executor-owned reference has been dropped. A leak must prevent
    // publication of successful lifecycle evidence, not panic after it.
    let entity_refcounts = evaluator.cx.app.borrow().ref_counts_drop_handle();
    drop(evaluator);
    drop(entity_refcounts);
    let final_observation = (|| -> Result<()> {
        let encoded = serde_json::to_vec(&reply)?;
        ensure!(
            encoded.len() <= 16 * 1024,
            "evaluation_lifecycle_response_too_large"
        );
        let mut output = stdout.lock();
        output.write_all(&encoded)?;
        output.write_all(b"\n")?;
        output.flush()?;
        Ok(())
    })();
    result.and(cleanup).and(final_observation)
}
