use super::*;
use anyhow::{Context, Result, anyhow, bail};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

static SMART_CONTEXT_REPLAY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[cfg(feature = "allocation-bench-support")]
const SMART_CONTEXT_REPLAY_EVIDENCE_LEVEL: &str =
    "deterministic_correctness_with_tokenizer_and_allocation_counts";
#[cfg(not(feature = "allocation-bench-support"))]
const SMART_CONTEXT_REPLAY_EVIDENCE_LEVEL: &str = "deterministic_correctness_with_tokenizer_counts";
#[cfg(feature = "allocation-bench-support")]
const SMART_CONTEXT_REPLAY_COMMAND: &str = "cargo run --locked -q --features allocation-bench-support --bin prodex -- context replay-report <corpus.json> --json --strict";
#[cfg(not(feature = "allocation-bench-support"))]
const SMART_CONTEXT_REPLAY_COMMAND: &str =
    "cargo run --locked -q --bin prodex -- context replay-report <corpus.json> --json --strict";

struct RuntimeSmartContextReplayHarness {
    shared: Option<RuntimeRotationProxyShared>,
    marker: Option<crate::RuntimeProxyMarkerGuard>,
    root: PathBuf,
    mode: runtime_proxy_crate::SmartContextReplayMode,
}

impl RuntimeSmartContextReplayHarness {
    fn new(
        scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
        mode: runtime_proxy_crate::SmartContextReplayMode,
    ) -> Result<Self> {
        let sequence = SMART_CONTEXT_REPLAY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "prodex-smart-context-replay-{}-{sequence}",
            std::process::id()
        ));
        let (shared, marker) = runtime_smart_context_replay_shared_at_root(scenario, mode, &root)?;
        Ok(Self {
            shared: Some(shared),
            marker: Some(marker),
            root,
            mode,
        })
    }

    fn restart(
        &mut self,
        scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    ) -> Result<()> {
        self.shared.take();
        self.marker.take();
        let (shared, marker) =
            runtime_smart_context_replay_shared_at_root(scenario, self.mode, &self.root)?;
        self.shared = Some(shared);
        self.marker = Some(marker);
        Ok(())
    }

    fn shared(&self) -> Result<&RuntimeRotationProxyShared> {
        self.shared
            .as_ref()
            .context("replay harness has no shared runtime")
    }
}

impl Drop for RuntimeSmartContextReplayHarness {
    fn drop(&mut self) {
        self.shared.take();
        self.marker.take();
        let _ = fs::remove_dir_all(&self.root);
    }
}

enum RuntimeSmartContextReplayPreparedBody {
    Body(Vec<u8>),
    MissingArtifact(usize),
}

pub(crate) fn run_runtime_smart_context_replay_json(
    text: &str,
) -> Result<runtime_proxy_crate::SmartContextReplayReport> {
    let corpus = runtime_proxy_crate::smart_context_parse_replay_corpus_json(text)
        .map_err(|error| anyhow!(error))?;
    let mut scenario_results = vec![None; corpus.scenarios.len()];
    let mut concurrent_groups = BTreeMap::<&str, Vec<usize>>::new();
    for (index, scenario) in corpus.scenarios.iter().enumerate() {
        if let Some(group) = scenario.concurrent_group.as_deref() {
            concurrent_groups.entry(group).or_default().push(index);
        } else {
            scenario_results[index] = Some(
                run_runtime_smart_context_replay_scenario(scenario, false).with_context(|| {
                    format!("failed Smart Context replay scenario {}", scenario.id)
                })?,
            );
        }
    }
    for indexes in concurrent_groups.values() {
        let results = std::thread::scope(|scope| {
            let handles = indexes
                .iter()
                .map(|index| {
                    let scenario = &corpus.scenarios[*index];
                    (
                        *index,
                        scope.spawn(move || {
                            run_runtime_smart_context_replay_scenario(scenario, false)
                        }),
                    )
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|(index, handle)| {
                    let result = handle
                        .join()
                        .map_err(|_| anyhow!("Smart Context replay worker panicked"))??;
                    Ok((index, result))
                })
                .collect::<Result<Vec<_>>>()
        })?;
        for (index, result) in results {
            scenario_results[index] = Some(result);
        }
    }
    let scenario_results = scenario_results
        .into_iter()
        .enumerate()
        .map(|(index, result)| {
            result.with_context(|| {
                format!(
                    "missing Smart Context replay result for {}",
                    corpus.scenarios[index].id
                )
            })
        })
        .collect::<Result<Vec<_>>>()?;
    #[cfg(feature = "allocation-bench-support")]
    let mut scenario_results = scenario_results;
    #[cfg(feature = "allocation-bench-support")]
    for (scenario, result) in corpus.scenarios.iter().zip(scenario_results.iter_mut()) {
        let measured = run_runtime_smart_context_replay_scenario(scenario, true)
            .with_context(|| format!("failed allocation replay scenario {}", scenario.id))?;
        for (turn, measured_turn) in result.turns.iter_mut().zip(measured.turns) {
            turn.allocation_bytes = measured_turn.allocation_bytes;
        }
    }
    let mut failures = Vec::new();
    for result in &scenario_results {
        if !result.passed {
            failures.push(result.id.clone());
        }
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
        evidence_level: SMART_CONTEXT_REPLAY_EVIDENCE_LEVEL,
        provenance: runtime_proxy_crate::SmartContextReplayProvenance {
            package_version: env!("CARGO_PKG_VERSION"),
            commit_sha: replay_commit_sha(),
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            rust_toolchain: std::env::var("PRODEX_RUST_TOOLCHAIN")
                .or_else(|_| std::env::var("RUSTUP_TOOLCHAIN"))
                .ok(),
            tokenizer_source: "tiktoken-rs@0.12.0",
            token_measurement: "tokenizer_counted",
            command: SMART_CONTEXT_REPLAY_COMMAND,
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
    measure_allocation: bool,
) -> Result<runtime_proxy_crate::SmartContextReplayScenarioResult> {
    let mut exact = RuntimeSmartContextReplayHarness::new(
        scenario,
        runtime_proxy_crate::SmartContextReplayMode::Exact,
    )?;
    let mut optimized = RuntimeSmartContextReplayHarness::new(scenario, scenario.mode)?;
    let mut turns = Vec::with_capacity(scenario.turns.len());

    for (index, turn) in scenario.turns.iter().enumerate() {
        let turn_index = index + 1;
        if scenario.restart_before_turns.contains(&turn_index) {
            exact.restart(scenario)?;
            optimized.restart(scenario)?;
        }
        turns.push(run_runtime_smart_context_replay_turn(
            scenario,
            turn,
            turn_index,
            exact.shared()?,
            optimized.shared()?,
            measure_allocation,
        )?);
    }

    let exact_input_tokens = turns.iter().map(|turn| turn.exact_input_tokens).sum();
    let optimized_input_tokens = turns.iter().map(|turn| turn.optimized_input_tokens).sum();
    let net_saved_tokens = replay_token_difference(exact_input_tokens, optimized_input_tokens);
    let passed = turns.iter().all(|turn| turn.failures.is_empty())
        && optimized_input_tokens <= exact_input_tokens;

    Ok(runtime_proxy_crate::SmartContextReplayScenarioResult {
        id: scenario.id.clone(),
        transport: scenario.transport,
        route: scenario.route,
        provider: scenario.provider.clone(),
        model: scenario.model.clone(),
        context_window_tokens: scenario.context_window_tokens,
        mode: scenario.mode,
        tags: scenario.tags.clone(),
        concurrent_group: scenario.concurrent_group.clone(),
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
    measure_allocation: bool,
) -> Result<runtime_proxy_crate::SmartContextReplayTurnResult> {
    let body = serde_json::to_vec(&turn.request).context("failed to serialize replay request")?;
    let exact_generation_before = replay_state_generation(exact)?;
    let exact_body = match replay_prepare_body(
        scenario,
        runtime_proxy_crate::SmartContextReplayMode::Exact,
        turn_index,
        &body,
        exact,
    )? {
        RuntimeSmartContextReplayPreparedBody::Body(body) => body,
        RuntimeSmartContextReplayPreparedBody::MissingArtifact(count) => {
            bail!("exact replay was blocked by {count} missing artifact(s)")
        }
    };
    let exact_state_mutations =
        replay_state_generation(exact)?.saturating_sub(exact_generation_before);

    let optimized_generation_before = replay_state_generation(optimized)?;
    #[cfg(feature = "allocation-bench-support")]
    let allocation_before =
        measure_allocation.then(crate::allocation_bench_support::runtime_allocation_snapshot);
    #[cfg(not(feature = "allocation-bench-support"))]
    let allocation_before: Option<()> = {
        let _ = measure_allocation;
        None
    };
    let started_at = Instant::now();
    let optimized_prepared =
        replay_prepare_body(scenario, scenario.mode, turn_index, &body, optimized)?;
    let rewrite_duration_ns = u64::try_from(started_at.elapsed().as_nanos()).unwrap_or(u64::MAX);
    #[cfg(feature = "allocation-bench-support")]
    let allocation_bytes = allocation_before.map(|before| {
        let after = crate::allocation_bench_support::runtime_allocation_snapshot();
        after
            .allocated_bytes
            .saturating_sub(before.allocated_bytes)
            .saturating_add(
                after
                    .reallocated_bytes
                    .saturating_sub(before.reallocated_bytes),
            )
    });
    #[cfg(not(feature = "allocation-bench-support"))]
    let allocation_bytes = allocation_before.map(|()| 0);
    let optimized_state_mutations =
        replay_state_generation(optimized)?.saturating_sub(optimized_generation_before);
    let (optimized_body, blocked_before_upstream, missing_artifact_count) = match optimized_prepared
    {
        RuntimeSmartContextReplayPreparedBody::Body(body) => (body, false, 0),
        RuntimeSmartContextReplayPreparedBody::MissingArtifact(count) => {
            (body.clone(), true, count)
        }
    };

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
    let unresolved_artifact_references = if blocked_before_upstream {
        Vec::new()
    } else {
        optimized_value
            .as_ref()
            .map(runtime_smart_context_collect_artifact_refs)
            .unwrap_or_default()
            .into_iter()
            .map(|reference| reference.id)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    };
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
    match turn.expected_outcome {
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::Rewrite if !rewrite_applied => {
            failures.push("expected_rewrite_not_applied".to_string());
        }
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::PassThrough
            if rewrite_applied || blocked_before_upstream =>
        {
            failures.push("expected_pass_through_not_observed".to_string());
        }
        runtime_proxy_crate::SmartContextReplayExpectedOutcome::MissingArtifactFailure
            if !blocked_before_upstream =>
        {
            failures.push("expected_missing_artifact_failure_not_observed".to_string());
        }
        _ => {}
    }
    if rewrite_applied && net_saved_tokens <= 0 {
        failures.push("rewrite_not_token_positive".to_string());
    }
    if optimized_input_tokens > exact_input_tokens {
        failures.push("aggregate_input_tokens_increased".to_string());
    }

    let validation_passed = failures.is_empty();
    let fallback_reason = if blocked_before_upstream {
        Some("missing_artifact")
    } else if rewrite_applied {
        None
    } else {
        Some(match scenario.mode {
            runtime_proxy_crate::SmartContextReplayMode::Exact => "explicit_exact",
            runtime_proxy_crate::SmartContextReplayMode::Shadow => "shadow",
            runtime_proxy_crate::SmartContextReplayMode::CanaryOut => "canary_out",
            runtime_proxy_crate::SmartContextReplayMode::Active => match scenario.route {
                runtime_proxy_crate::SmartContextReplayRoute::Responses
                | runtime_proxy_crate::SmartContextReplayRoute::Websocket => "no_op",
                runtime_proxy_crate::SmartContextReplayRoute::Compact
                | runtime_proxy_crate::SmartContextReplayRoute::Standard => "unsupported_route",
            },
        })
    };

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
        blocked_before_upstream,
        missing_artifact_count,
        validation_passed,
        fallback_reason,
        allocation_bytes,
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
) -> Result<RuntimeSmartContextReplayPreparedBody> {
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
    let (path_and_query, route_kind) = match scenario.route {
        runtime_proxy_crate::SmartContextReplayRoute::Responses => {
            ("/responses", RuntimeRouteKind::Responses)
        }
        runtime_proxy_crate::SmartContextReplayRoute::Compact => {
            ("/responses/compact", RuntimeRouteKind::Compact)
        }
        runtime_proxy_crate::SmartContextReplayRoute::Standard => {
            ("/v1/chat/completions", RuntimeRouteKind::Standard)
        }
        runtime_proxy_crate::SmartContextReplayRoute::Websocket => {
            ("/responses", RuntimeRouteKind::Websocket)
        }
    };
    let request = RuntimeProxyRequest {
        method: "POST".to_string(),
        path_and_query: path_and_query.to_string(),
        headers,
        body: body.to_vec(),
    };
    let request_id = u64::try_from(turn_index).unwrap_or(u64::MAX);
    match scenario.transport {
        runtime_proxy_crate::SmartContextReplayTransport::Http => {
            match prepare_runtime_smart_context_http_body_for_profile(
                request_id,
                &request,
                shared,
                route_kind,
                Some("replay"),
            ) {
                Ok(body) => Ok(RuntimeSmartContextReplayPreparedBody::Body(
                    body.into_owned(),
                )),
                Err(error) => Ok(RuntimeSmartContextReplayPreparedBody::MissingArtifact(
                    error.missing_artifact_count,
                )),
            }
        }
        runtime_proxy_crate::SmartContextReplayTransport::Websocket => {
            let request_text = std::str::from_utf8(body).context("replay request is not UTF-8")?;
            match prepare_runtime_smart_context_websocket_text(
                request_id,
                request_text,
                &request,
                shared,
                "replay",
            ) {
                Ok(body) => Ok(RuntimeSmartContextReplayPreparedBody::Body(
                    body.into_owned().into_bytes(),
                )),
                Err(error) => Ok(RuntimeSmartContextReplayPreparedBody::MissingArtifact(
                    error.missing_artifact_count,
                )),
            }
        }
    }
}

fn runtime_smart_context_replay_shared_at_root(
    scenario: &runtime_proxy_crate::SmartContextReplayScenarioInput,
    mode: runtime_proxy_crate::SmartContextReplayMode,
    root: &Path,
) -> Result<(RuntimeRotationProxyShared, crate::RuntimeProxyMarkerGuard)> {
    fs::create_dir_all(root)
        .with_context(|| format!("failed to create replay root {}", root.display()))?;
    let paths = AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared"),
        legacy_shared_codex_root: root.join("legacy-shared"),
        root: root.to_path_buf(),
    };
    let profile_name = "replay".to_string();
    let provider = prodex_provider_core::ProviderId::parse(&scenario.provider)
        .with_context(|| format!("unsupported replay provider {}", scenario.provider))?;
    let upstream_base_url = format!("https://{}.example.com/v1", provider.label());
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
        upstream_base_url,
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
        Some(root.join("runtime-smart-context-artifacts.json")),
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
    Ok((shared, marker))
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
