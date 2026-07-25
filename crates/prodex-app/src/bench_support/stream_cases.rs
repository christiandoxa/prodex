use super::*;

#[doc(hidden)]
pub struct RuntimeProxySseInspectBenchCase {
    buffer: Vec<u8>,
}

impl RuntimeProxySseInspectBenchCase {
    pub fn new(event_count: usize) -> Self {
        let event_count = event_count.max(1);
        let mut buffer = Vec::new();
        for index in 0..event_count {
            if index % 8 == 0 {
                buffer.extend_from_slice(b": keep-alive\r\n");
            }
            let event_type = match index % 6 {
                0 => "response.created",
                1 => "response.in_progress",
                2 => "response.output_item.added",
                3 => "response.content_part.added",
                4 => "response.output_text.delta",
                _ => "response.reasoning_summary_text.delta",
            };
            buffer.extend_from_slice(
                format!(
                    "event: {event_type}\r\ndata: {{\"type\":\"{event_type}\",\"response_id\":\"resp-{index:03}\",\"delta\":\"bench-token-{index:03}\"}}\r\n\r\n"
                )
                .as_bytes(),
            );
        }
        buffer.extend_from_slice(
            b"event: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp-tail\",\"turn_state\":\"turn-tail\"}}\r\n\r\n",
        );
        Self { buffer }
    }

    pub fn inspect(&self) -> usize {
        match inspect_runtime_sse_buffer(&self.buffer) {
            RuntimeSseInspectionProgress::Hold {
                response_ids,
                turn_state,
            }
            | RuntimeSseInspectionProgress::Commit {
                response_ids,
                turn_state,
            } => response_ids.len() + usize::from(turn_state.is_some()),
            RuntimeSseInspectionProgress::QuotaBlocked
            | RuntimeSseInspectionProgress::Overloaded
            | RuntimeSseInspectionProgress::PreviousResponseNotFound => 0,
        }
    }
}

#[doc(hidden)]
pub struct RuntimeProxyLineageCleanupBenchCase {
    shared: RuntimeRotationProxyShared,
    template: RuntimeRotationState,
    profile_name: String,
    response_ids: Vec<String>,
}

impl RuntimeProxyLineageCleanupBenchCase {
    pub fn new(turn_state_count: usize) -> Self {
        let turn_state_count = turn_state_count.max(2);
        let paths = bench_paths("lineage-cleanup");
        let now = Local::now().timestamp();
        let profile_name = "main".to_string();
        let target_response_id = "resp-target".to_string();
        let mut response_profile_bindings = BTreeMap::new();
        let mut turn_state_bindings = BTreeMap::new();

        response_profile_bindings.insert(
            target_response_id.clone(),
            ResponseProfileBinding {
                profile_name: profile_name.clone(),
                bound_at: now,
            },
        );

        for index in 0..turn_state_count {
            let turn_state = format!("turn-{index:03}");
            turn_state_bindings.insert(
                turn_state.clone(),
                ResponseProfileBinding {
                    profile_name: profile_name.clone(),
                    bound_at: now,
                },
            );
            response_profile_bindings.insert(
                runtime_response_turn_state_lineage_key(&target_response_id, &turn_state),
                ResponseProfileBinding {
                    profile_name: profile_name.clone(),
                    bound_at: now,
                },
            );
            if index % 2 == 0 {
                let survivor_response_id = format!("resp-survivor-{index:03}");
                response_profile_bindings.insert(
                    survivor_response_id.clone(),
                    ResponseProfileBinding {
                        profile_name: profile_name.clone(),
                        bound_at: now,
                    },
                );
                response_profile_bindings.insert(
                    runtime_response_turn_state_lineage_key(&survivor_response_id, &turn_state),
                    ResponseProfileBinding {
                        profile_name: profile_name.clone(),
                        bound_at: now,
                    },
                );
            }
        }

        let template = RuntimeRotationState {
            paths,
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles: BTreeMap::from([(
                    profile_name.clone(),
                    ProfileEntry {
                        codex_home: PathBuf::from("/tmp/prodex-bench/main"),
                        managed: true,
                        email: None,
                        provider: ProfileProvider::Openai,
                    },
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings,
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: profile_name.clone(),
            profile_usage_auth: BTreeMap::new(),
            turn_state_bindings,
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

        Self {
            shared: bench_runtime_shared("lineage-cleanup", template.clone(), 8),
            template,
            profile_name,
            response_ids: vec![target_response_id],
        }
    }

    pub fn clear_dead_response_bindings(&self) -> usize {
        let mut runtime = self
            .shared
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *runtime = self.template.clone();
        drop(runtime);

        clear_runtime_dead_response_bindings(
            &self.shared,
            &self.profile_name,
            &self.response_ids,
            "bench_cleanup",
        )
        .expect("benchmark lineage cleanup should succeed");

        let runtime = self
            .shared
            .runtime
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime.turn_state_bindings.len()
    }
}

#[doc(hidden)]
pub struct RuntimeProxySmartContextRewriteBenchCase {
    shared: RuntimeRotationProxyShared,
    request: RuntimeProxyRequest,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum RuntimeProxySmartContextBenchMode {
    Active,
    CanaryOut,
    Disabled,
    Exact,
    Shadow,
}

impl RuntimeProxySmartContextRewriteBenchCase {
    pub fn new(tool_line_count: usize) -> Self {
        let tool_line_count = tool_line_count.max(32);
        let output = (0..tool_line_count)
            .map(|index| {
                format!(
                    "line {index:04}: /repo/prodex/crates/prodex-app/src/runtime_proxy/smart_context.rs token-heavy-tool-output repeated-context payload={}",
                    "abcdef0123456789".repeat(8)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Self::from_body(
            runtime_smart_context_bench_duplicate_body(&output),
            RuntimeProxySmartContextBenchMode::Active,
        )
    }

    pub fn active(body_bytes: usize) -> Self {
        let output = "cargo test runtime_proxy_affinity ... ok; repeated exact output\n"
            .repeat(body_bytes.saturating_sub(512).saturating_div(128).max(16));
        Self::from_body(
            runtime_smart_context_bench_duplicate_body(&output),
            RuntimeProxySmartContextBenchMode::Active,
        )
    }

    pub fn canary_out(body_bytes: usize) -> Self {
        Self::pass_through(body_bytes, RuntimeProxySmartContextBenchMode::CanaryOut)
    }

    pub fn disabled(body_bytes: usize) -> Self {
        Self::pass_through(body_bytes, RuntimeProxySmartContextBenchMode::Disabled)
    }

    pub fn exact(body_bytes: usize) -> Self {
        Self::pass_through(body_bytes, RuntimeProxySmartContextBenchMode::Exact)
    }

    pub fn rejected_noop(body_bytes: usize) -> Self {
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.1-codex",
            "input": [{
                "type": "message",
                "role": "user",
                "content": "x".repeat(body_bytes.saturating_sub(128)),
            }],
        }))
        .expect("benchmark no-op request body should serialize");
        Self::from_body(body, RuntimeProxySmartContextBenchMode::Active)
    }

    pub fn shadow(body_bytes: usize) -> Self {
        let output = "cargo test runtime_proxy_affinity ... ok; shadow repeated exact output\n"
            .repeat(body_bytes.saturating_sub(512).saturating_div(144).max(16));
        Self::from_body(
            runtime_smart_context_bench_duplicate_body(&output),
            RuntimeProxySmartContextBenchMode::Shadow,
        )
    }

    fn pass_through(body_bytes: usize, mode: RuntimeProxySmartContextBenchMode) -> Self {
        Self::from_body(vec![b'x'; body_bytes.max(1)], mode)
    }

    fn from_body(body: Vec<u8>, mode: RuntimeProxySmartContextBenchMode) -> Self {
        let paths = bench_paths("smart-context");
        let profile_name = "main".to_string();
        let state = RuntimeRotationState {
            paths: paths.clone(),
            state: AppState {
                active_profile: Some(profile_name.clone()),
                profiles: BTreeMap::from([(
                    profile_name.clone(),
                    bench_profile_entry(&paths, &profile_name),
                )]),
                last_run_selected_at: BTreeMap::new(),
                response_profile_bindings: BTreeMap::new(),
                session_profile_bindings: BTreeMap::new(),
            },
            upstream_base_url: "https://chatgpt.com/backend-api".to_string(),
            include_code_review: false,
            current_profile: profile_name.clone(),
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
        let mut shared = bench_runtime_shared("smart-context", state, 8);
        let config = Arc::get_mut(&mut shared.runtime_config)
            .expect("benchmark runtime config must be uniquely owned");
        config.smart_context_canary_percent =
            if mode == RuntimeProxySmartContextBenchMode::CanaryOut {
                0
            } else {
                100
            };
        config.smart_context_shadow = mode == RuntimeProxySmartContextBenchMode::Shadow;
        if mode != RuntimeProxySmartContextBenchMode::Disabled {
            register_runtime_smart_context_proxy_state(&shared, true, Some(18_000), None);
        }
        let mut headers = Vec::new();
        if mode == RuntimeProxySmartContextBenchMode::Exact {
            headers.push(("x-prodex-smart-context".to_string(), "exact".to_string()));
        }
        if mode == RuntimeProxySmartContextBenchMode::Shadow {
            headers.push(("session_id".to_string(), "bench-4".to_string()));
        }
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/responses".to_string(),
            headers,
            body,
        };

        Self { shared, request }
    }

    pub fn prepare(&self) -> usize {
        prepare_runtime_smart_context_http_body_for_profile(
            bench_case_id(),
            &self.request,
            &self.shared,
            RuntimeRouteKind::Responses,
            Some("main"),
        )
        .expect("benchmark request must not contain unresolved artifact references")
        .len()
    }

    pub fn rewrite_large_tool_output(&self) -> usize {
        self.prepare()
    }
}

fn runtime_smart_context_bench_duplicate_body(output: &str) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "model": "gpt-5.1-codex",
        "input": [
            {
                "type": "message",
                "role": "user",
                "content": "Summarize the failing runtime proxy tool output and keep references."
            },
            {
                "type": "function_call_output",
                "call_id": "call_bench_first",
                "output": output
            },
            {
                "type": "function_call_output",
                "call_id": "call_bench_second",
                "output": output
            }
        ]
    }))
    .expect("benchmark Smart Context request body should serialize")
}

#[doc(hidden)]
pub struct RuntimeProxySmartContextRehydrateBenchCase {
    store: RuntimeSmartContextArtifactStore,
    value: serde_json::Value,
}

impl RuntimeProxySmartContextRehydrateBenchCase {
    pub fn new(line_count: usize) -> Self {
        let text = (0..line_count.max(32))
            .map(|index| format!("rehydrated compiler output line {index:04}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut store = RuntimeSmartContextArtifactStore::default();
        let artifact = store
            .insert_text(&text)
            .expect("benchmark artifact should fit store limits");
        let value = serde_json::json!({"output": format!("prodex-artifact:{}", artifact.id)});
        Self { store, value }
    }

    pub fn rehydrate(&self) -> usize {
        runtime_smart_context_rehydrate_for_benchmark(self.value.clone(), &self.store)
    }
}

#[cfg(test)]
mod smart_context_bench_tests {
    use super::*;

    #[test]
    fn smart_context_bench_cases_exercise_their_named_paths() {
        for case in [
            RuntimeProxySmartContextRewriteBenchCase::disabled(4 * 1024),
            RuntimeProxySmartContextRewriteBenchCase::exact(4 * 1024),
            RuntimeProxySmartContextRewriteBenchCase::canary_out(4 * 1024),
            RuntimeProxySmartContextRewriteBenchCase::rejected_noop(4 * 1024),
            RuntimeProxySmartContextRewriteBenchCase::shadow(4 * 1024),
        ] {
            assert_eq!(case.prepare(), case.request.body.len());
        }

        let active = RuntimeProxySmartContextRewriteBenchCase::active(4 * 1024);
        assert!(active.prepare() < active.request.body.len());

        let rehydrate = RuntimeProxySmartContextRehydrateBenchCase::new(64);
        assert!(rehydrate.rehydrate() > rehydrate.value.to_string().len());
    }
}
