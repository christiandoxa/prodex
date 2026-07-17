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

impl RuntimeProxySmartContextRewriteBenchCase {
    pub fn new(tool_line_count: usize) -> Self {
        let tool_line_count = tool_line_count.max(32);
        let paths = bench_paths("smart-context-rewrite");
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
        let shared = bench_runtime_shared("smart-context-rewrite", state, 8);
        register_runtime_smart_context_proxy_state(&shared.log_path, true, Some(18_000), None);

        let output = (0..tool_line_count)
            .map(|index| {
                format!(
                    "line {index:04}: /repo/prodex/crates/prodex-app/src/runtime_proxy/smart_context.rs token-heavy-tool-output repeated-context payload={}",
                    "abcdef0123456789".repeat(8)
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5.1-codex",
            "input": [
                {
                    "type": "message",
                    "role": "user",
                    "content": "Summarize the failing runtime proxy tool output and keep artifact references."
                },
                {
                    "type": "function_call_output",
                    "call_id": "call_bench_large_tool_output",
                    "output": output
                }
            ]
        }))
        .expect("benchmark smart-context request body should serialize");
        let request = RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/responses".to_string(),
            headers: Vec::new(),
            body,
        };

        Self { shared, request }
    }

    pub fn rewrite_large_tool_output(&self) -> usize {
        prepare_runtime_smart_context_http_body_for_profile(
            bench_case_id(),
            &self.request,
            &self.shared,
            RuntimeRouteKind::Responses,
            Some("main"),
        )
        .len()
    }
}
