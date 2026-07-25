use super::*;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

static SMART_CONTEXT_REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(crate) fn run_runtime_smart_context_replay_json(
    text: &str,
) -> Result<runtime_proxy_crate::SmartContextReplayReport> {
    let corpus = runtime_proxy_crate::smart_context_parse_replay_corpus_json(text)
        .map_err(|error| anyhow!(error))?;
    let mut scenario_results = Vec::with_capacity(corpus.scenarios.len());
    let mut failures = Vec::new();

    for scenario in &corpus.scenarios {
        let result = run_runtime_smart_context_replay_scenario(scenario)
            .with_context(|| format!("failed Smart Context replay scenario {}", scenario.id))?;
        if !result.passed {
            failures.push(scenario.id.clone());
        }
        scenario_results.push(result);
    }

    let exact_input_tokens = scenario_results
        .iter()
        .map(|scenario| scenario.exact_input_tokens)
        .sum();
    let optimized_input_tokens = scenario_results
        .iter()
        .map(|scenario| scenario.optimized_input_tokens)
        .sum();
    let net_saved_tokens = replay_token_difference(exact_input_tokens, optimized_input_tokens);

    Ok(runtime_proxy_crate::SmartContextReplayReport {
        schema_version: runtime_proxy_crate::SMART_CONTEXT_REPLAY_REPORT_SCHEMA_VERSION,
        corpus_schema_version: corpus.schema_version,
        evidence_level: "deterministic_correctness_with_tokenizer_counts",
        provenance: runtime_proxy_crate::SmartContextReplayProvenance {
            package_version: env!("CARGO_PKG_VERSION"),
            commit_sha: replay_commit_sha(),
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            rust_toolchain: std::env::var("RUSTUP_TOOLCHAIN").ok(),
            tokenizer_source: "tiktoken-rs@0.12.0",
            token_measurement: "tokenizer_counted",
            command: "cargo run -q --bin prodex -- context replay-report <corpus.json> --json --strict",
        },
        scenarios: scenario_results,
        exact_input_tokens,
        optimized_input_tokens,
        net_saved_tokens,
        passed: failures.is_empty(),
        failures,
    })
}

pub(crate) fn render_runtime_smart_context_replay_markdown(
    report: &runtime_proxy_crate::SmartContextReplayReport,
) -> String {
    let mut output = String::from("# Smart Context Deterministic Replay\n\n");
    output.push_str(&format!("- evidence_level: {}\n", report.evidence_level));
    output.push_str(&format!(
        "- token_measurement: {} ({})\n",
        report.provenance.token_measurement, report.provenance.tokenizer_source
    ));
    output.push_str(&format!(
        "- exact_input_tokens: {}\n",
        report.exact_input_tokens
    ));
    output.push_str(&format!(
        "- optimized_input_tokens: {}\n",
        report.optimized_input_tokens
    ));
    output.push_str(&format!(
        "- net_saved_tokens: {}\n",
        report.net_saved_tokens
    ));
    output.push_str(&format!("- passed: {}\n", report.passed));
    output.push_str("\n## Scenarios\n\n");
    for scenario in &report.scenarios {
        output.push_str(&format!(
            "- {}: passed={}, turns={}, net_saved_tokens={}\n",
            scenario.id,
            scenario.passed,
            scenario.turns.len(),
            scenario.net_saved_tokens
        ));
        for turn in &scenario.turns {
            if !turn.failures.is_empty() {
                output.push_str(&format!(
                    "  - turn {} failures: {}\n",
                    turn.turn,
                    turn.failures.join(", ")
                ));
            }
        }
    }
    output
}

fn run_runtime_smart_context_replay_scenario(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
) -> Result<runtime_proxy_crate::SmartContextReplayScenarioResult> {
    if !scenario.provider.eq_ignore_ascii_case("openai") {
        bail!(
            "unsupported replay provider {}; the first executable slice supports openai",
            scenario.provider
        );
    }
    let (exact, exact_marker, exact_root) = runtime_smart_context_replay_shared(
        scenario,
        runtime_proxy_crate::SmartContextReplayMode::Exact,
    )?;
    let (optimized, optimized_marker, optimized_root) =
        runtime_smart_context_replay_shared(scenario, scenario.mode)?;
    let mut turns = Vec::with_capacity(scenario.turns.len());

    for (index, turn) in scenario.turns.iter().enumerate() {
        turns.push(run_runtime_smart_context_replay_turn(
            scenario,
            turn,
            index + 1,
            &exact,
            &optimized,
        )?);
    }

    drop(exact);
    drop(optimized);
    drop(exact_marker);
    drop(optimized_marker);
    remove_runtime_smart_context_replay_root(&exact_root)?;
    remove_runtime_smart_context_replay_root(&optimized_root)?;

    let exact_input_tokens = turns.iter().map(|turn| turn.exact_input_tokens).sum();
    let optimized_input_tokens = turns.iter().map(|turn| turn.optimized_input_tokens).sum();
    let net_saved_tokens = replay_token_difference(exact_input_tokens, optimized_input_tokens);
    let passed = turns.iter().all(|turn| turn.failures.is_empty())
        && optimized_input_tokens <= exact_input_tokens;

    Ok(runtime_proxy_crate::SmartContextReplayScenarioResult {
        id: scenario.id.clone(),
        transport: scenario.transport,
        provider: scenario.provider.clone(),
        model: scenario.model.clone(),
        context_window_tokens: scenario.context_window_tokens,
        mode: scenario.mode,
        exact_input_tokens,
        optimized_input_tokens,
        net_saved_tokens,
        turns,
        passed,
    })
}

fn run_runtime_smart_context_replay_turn(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    turn: &runtime_proxy_crate::SmartContextReplayTurnInput,
    turn_index: usize,
    exact: &RuntimeRotationProxyShared,
    optimized: &RuntimeRotationProxyShared,
) -> Result<runtime_proxy_crate::SmartContextReplayTurnResult> {
    let body = serde_json::to_vec(&turn.request).context("failed to serialize replay request")?;
    let exact_generation_before = replay_state_generation(exact)?;
    let exact_body = replay_prepare_body(
        scenario,
        runtime_proxy_crate::SmartContextReplayMode::Exact,
        turn_index,
        &body,
        exact,
    )?;
    let exact_state_mutations =
        replay_state_generation(exact)?.saturating_sub(exact_generation_before);

    let optimized_generation_before = replay_state_generation(optimized)?;
    let started_at = Instant::now();
    let optimized_body =
        replay_prepare_body(scenario, scenario.mode, turn_index, &body, optimized)?;
    let rewrite_duration_ns = u64::try_from(started_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
    let optimized_state_mutations =
        replay_state_generation(optimized)?.saturating_sub(optimized_generation_before);

    let exact_count = runtime_proxy_crate::smart_context_count_serialized_request(
        &exact_body,
        Some(&scenario.model),
    );
    let optimized_count = runtime_proxy_crate::smart_context_count_serialized_request(
        &optimized_body,
        Some(&scenario.model),
    );
    if !exact_count.is_proven() || !optimized_count.is_proven() {
        bail!(
            "model {} has no supported tokenizer; deterministic replay cannot claim token savings",
            scenario.model
        );
    }
    let exact_input_tokens = exact_count.tokens;
    let optimized_input_tokens = optimized_count.tokens;
    let net_saved_tokens = replay_token_difference(exact_input_tokens, optimized_input_tokens);
    let optimized_value = serde_json::from_slice::<serde_json::Value>(&optimized_body).ok();
    let valid_json = optimized_value.is_some();
    let required_text_preserved = optimized_value.as_ref().is_some_and(|value| {
        turn.required_text
            .iter()
            .all(|required| replay_value_contains_text(value, required))
    });
    let protocol_fields_preserved = optimized_value.as_ref().is_some_and(|value| {
        turn.preserve_json_pointers
            .iter()
            .all(|pointer| turn.request.pointer(pointer) == value.pointer(pointer))
    });
    let unresolved_artifact_references = optimized_value
        .as_ref()
        .map(runtime_smart_context_collect_artifact_refs)
        .unwrap_or_default()
        .into_iter()
        .map(|reference| reference.id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let rewrite_applied = optimized_body != body;
    let exact_byte_identity = exact_body == body;
    let selected_transforms = replay_selected_transforms(&optimized_body);
    let mut failures = Vec::new();
    if !exact_byte_identity {
        failures.push("exact_body_changed".to_string());
    }
    if exact_state_mutations != 0 {
        failures.push("exact_state_mutated".to_string());
    }
    if matches!(
        scenario.mode,
        runtime_proxy_crate::SmartContextReplayMode::Exact
            | runtime_proxy_crate::SmartContextReplayMode::Shadow
            | runtime_proxy_crate::SmartContextReplayMode::CanaryOut
    ) && optimized_state_mutations != 0
    {
        failures.push("pass_through_mode_state_mutated".to_string());
    }
    if !valid_json {
        failures.push("optimized_body_invalid_json".to_string());
    }
    if !required_text_preserved {
        failures.push("required_text_missing".to_string());
    }
    if !protocol_fields_preserved {
        failures.push("protocol_field_changed".to_string());
    }
    if !unresolved_artifact_references.is_empty() {
        failures.push("unresolved_artifact_reference".to_string());
    }
    if turn.expect_rewrite != rewrite_applied {
        failures.push("rewrite_expectation_mismatch".to_string());
    }
    if rewrite_applied && net_saved_tokens <= 0 {
        failures.push("rewrite_not_token_positive".to_string());
    }
    if optimized_input_tokens > exact_input_tokens {
        failures.push("aggregate_input_tokens_increased".to_string());
    }

    Ok(runtime_proxy_crate::SmartContextReplayTurnResult {
        turn: turn_index,
        exact_body_bytes: exact_body.len(),
        optimized_body_bytes: optimized_body.len(),
        exact_input_tokens,
        optimized_input_tokens,
        net_saved_tokens,
        token_count_source: optimized_count.source.as_str(),
        tokenizer_family: optimized_count.tokenizer_family.unwrap_or("unknown"),
        token_confidence_basis_points: optimized_count.confidence_basis_points,
        token_error_bound_tokens: optimized_count.error_bound_tokens,
        rewrite_applied,
        exact_byte_identity,
        valid_json,
        required_text_preserved,
        protocol_fields_preserved,
        unresolved_artifact_references,
        selected_transforms,
        exact_state_mutations,
        optimized_state_mutations,
        rewrite_duration_ns,
        exact_body_sha256: replay_body_sha256(&exact_body),
        optimized_body_sha256: replay_body_sha256(&optimized_body),
        failures,
    })
}

fn replay_prepare_body(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    mode: runtime_proxy_crate::SmartContextReplayMode,
    turn_index: usize,
    body: &[u8],
    shared: &RuntimeRotationProxyShared,
) -> Result<Vec<u8>> {
    let mut headers = vec![
        ("session_id".to_string(), format!("replay-{}", scenario.id)),
        (
            "x-codex-turn-metadata".to_string(),
            serde_json::json!({
                "session_id": format!("replay-{}", scenario.id),
                "cwd": "/workspace/replay"
            })
            .to_string(),
        ),
    ];
    if mode == runtime_proxy_crate::SmartContextReplayMode::Exact {
        headers.push(("x-prodex-smart-context".to_string(), "exact".to_string()));
    }
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: "/responses".to_string(),
        headers,
        body: body.to_vec(),
    };
    let request_id = u64::try_from(turn_index).unwrap_or(u64::MAX);
    match scenario.transport {
        runtime_proxy_crate::SmartContextReplayTransport::Http => {
            Ok(prepare_runtime_smart_context_http_body_for_profile(
                request_id,
                &request,
                shared,
                RuntimeRouteKind::Responses,
                Some("replay"),
            )?
            .into_owned())
        }
        runtime_proxy_crate::SmartContextReplayTransport::Websocket => {
            let request_text = std::str::from_utf8(body).context("replay request is not UTF-8")?;
            Ok(prepare_runtime_smart_context_websocket_text(
                request_id,
                request_text,
                &request,
                shared,
                "replay",
            )?
            .into_owned()
            .into_bytes())
        }
    }
}

fn runtime_smart_context_replay_shared(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    mode: runtime_proxy_crate::SmartContextReplayMode,
) -> Result<(
    RuntimeRotationProxyShared,
    crate::RuntimeProxyMarkerGuard,
    PathBuf,
)> {
    let sequence = SMART_CONTEXT_REPLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "prodex-smart-context-replay-{}-{sequence}",
        std::process::id()
    ));
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root: root.clone(),
    };
    let profile_name = "replay".to_string();
    let state = RuntimeRotationState {
        paths: paths.clone(),
        state: AppState {
            active_profile: Some(profile_name.clone()),
            profiles: BTreeMap::from([(
                profile_name.clone(),
                ProfileEntry {
                    codex_home: paths.managed_profiles_root.join(&profile_name),
                    managed: true,
                    email: None,
                    provider: ProfileProvider::Openai,
                },
            )]),
            last_run_selected_at: BTreeMap::new(),
            response_profile_bindings: BTreeMap::new(),
            session_profile_bindings: BTreeMap::new(),
        },
        upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
        include_code_review: false,
        current_profile: profile_name,
        profile_usage_auth: BTreeMap::new(),
        turn_state_bindings: BTreeMap::new(),
        session_id_bindings: BTreeMap::new(),
        continuation_statuses: RuntimeContinuationStatuses::default(),
        profile_probe_cache: BTreeMap::new(),
        profile_usage_snapshots: BTreeMap::new(),
        profile_retry_backoff_until: BTreeMap::new(),
        profile_transport_backoff_until: BTreeMap::new(),
        profile_route_circuit_open_until: BTreeMap::new(),
        profile_backoff_updated_at: BTreeMap::new(),
        profile_health: BTreeMap::new(),
    };
    let mut config =
        RuntimeConfig::offline_default(&paths).map_err(|errors| anyhow!(errors.to_string()))?;
    config.smart_context_shadow = mode == runtime_proxy_crate::SmartContextReplayMode::Shadow;
    config.smart_context_canary_percent =
        if mode == runtime_proxy_crate::SmartContextReplayMode::CanaryOut {
            0
        } else {
            100
        };
    let lane_limits = RuntimeProxyLaneLimits {
        responses: config.tuning.lane_limits.responses,
        compact: config.tuning.lane_limits.compact,
        websocket: config.tuning.lane_limits.websocket,
        standard: config.tuning.lane_limits.standard,
    };
    let shared = RuntimeRotationProxyShared {
        runtime_config: Arc::new(config),
        smart_context_engine: Arc::new(RuntimeSmartContextEngine::default()),
        upstream_no_proxy: false,
        auto_redeem_enabled: false,
        compact_client: reqwest::Client::new(),
        async_client: reqwest::Client::builder()
            .build()
            .context("failed to build replay HTTP client")?,
        async_runtime: Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .context("failed to build replay async runtime")?,
        ),
        runtime: Arc::new(Mutex::new(state)),
        log_path: root.join("runtime.log"),
        request_sequence: Arc::new(AtomicU64::new(1)),
        state_save_revision: Arc::new(AtomicU64::new(0)),
        local_overload_backoff_until: Arc::new(AtomicU64::new(0)),
        active_request_count: Arc::new(AtomicUsize::new(0)),
        active_request_limit: 8,
        runtime_state_lock_wait_counters:
            RuntimeRotationProxyShared::new_runtime_state_lock_wait_counters(),
        lane_admission: RuntimeProxyLaneAdmission::new(lane_limits),
    };
    register_runtime_proxy_persistence_mode(&shared.log_path, false);
    let marker = crate::RuntimeProxyMarkerGuard::new(&shared.log_path);
    register_runtime_smart_context_proxy_state(
        &shared,
        true,
        Some(scenario.context_window_tokens),
        None,
    );
    if let Some(observed_context_tokens) = scenario.observed_context_tokens {
        observe_runtime_smart_context_token_usage_for_bucket(
            &shared,
            RuntimeTokenUsage {
                input_tokens: observed_context_tokens,
                cached_input_tokens: 0,
                output_tokens: 0,
                reasoning_tokens: 0,
            },
            None,
        );
    }
    Ok((shared, marker, root))
}

fn replay_state_generation(shared: &RuntimeRotationProxyShared) -> Result<u64> {
    Ok(runtime_smart_context_proxy_state_snapshot(shared).map_or(0, |(generation, _)| generation))
}

fn replay_value_contains_text(value: &serde_json::Value, required: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.contains(required),
        serde_json::Value::Array(values) => values
            .iter()
            .any(|value| replay_value_contains_text(value, required)),
        serde_json::Value::Object(object) => object
            .values()
            .any(|value| replay_value_contains_text(value, required)),
        _ => false,
    }
}

fn replay_selected_transforms(body: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(body);
    let mut transforms = Vec::new();
    if text.contains("[prodex-context-ref v=1 ") {
        transforms.push("within_request_duplicate".to_string());
    }
    transforms
}

fn replay_body_sha256(body: &[u8]) -> String {
    runtime_proxy_crate::smart_context_hash_text(&String::from_utf8_lossy(body))
        .strip_prefix("sc2:")
        .unwrap_or_default()
        .to_string()
}

fn replay_token_difference(exact: u64, optimized: u64) -> i64 {
    let difference = i128::from(exact) - i128::from(optimized);
    difference.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn replay_commit_sha() -> Option<String> {
    ["PRODEX_GIT_COMMIT", "GITHUB_SHA"]
        .into_iter()
        .filter_map(|key| std::env::var(key).ok())
        .map(|value| value.trim().to_ascii_lowercase())
        .find(|value| {
            (7..=64).contains(&value.len()) && value.chars().all(|ch| ch.is_ascii_hexdigit())
        })
}

fn remove_runtime_smart_context_replay_root(root: &PathBuf) -> Result<()> {
    if root.exists() {
        fs::remove_dir_all(root)
            .with_context(|| format!("failed to remove replay workspace {}", root.display()))?;
    }
    Ok(())
}
