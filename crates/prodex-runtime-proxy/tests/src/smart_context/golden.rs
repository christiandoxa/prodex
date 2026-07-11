use super::*;

#[test]
fn golden_prodex_s_and_super_rewrite_corpus_saves_tokens_preserves_signals_and_refs() {
    for trace in smart_context_golden_prodex_traces() {
        let rewrite = trace.rewrite();
        let before_text = std::str::from_utf8(&rewrite.before_body).unwrap();
        let after_text = std::str::from_utf8(&rewrite.after_body).unwrap();
        let before_body_tokens = smart_context_estimate_tokens_from_body(&rewrite.before_body);
        let after_body_tokens = smart_context_estimate_tokens_from_body(&rewrite.after_body);
        let before_context_tokens =
            smart_context_estimate_tokens_from_body(rewrite.before_context.as_bytes());
        let after_context_tokens =
            smart_context_estimate_tokens_from_body(rewrite.after_context.as_bytes());
        let before_output_tokens =
            smart_context_estimate_tokens_from_body(rewrite.before_tool_output.as_bytes());
        let after_output_tokens =
            smart_context_estimate_tokens_from_body(rewrite.after_tool_output.as_bytes());

        assert!(
            rewrite.after_body.len() < rewrite.before_body.len(),
            "{}: rewritten body should be smaller, before={} after={}",
            trace.name,
            rewrite.before_body.len(),
            rewrite.after_body.len()
        );
        assert!(
            after_body_tokens < before_body_tokens,
            "{}: rewritten body should save estimated tokens, before={before_body_tokens} after={after_body_tokens}",
            trace.name
        );
        assert!(
            after_body_tokens.saturating_mul(100)
                <= before_body_tokens.saturating_mul(trace.max_after_token_ratio_percent),
            "{}: rewritten body token ratio regressed, before={before_body_tokens} after={after_body_tokens} max_ratio={}%",
            trace.name,
            trace.max_after_token_ratio_percent
        );
        assert!(
            after_context_tokens < before_context_tokens,
            "{}: rewritten static context should save estimated tokens, before={before_context_tokens} after={after_context_tokens}",
            trace.name
        );
        assert!(
            after_output_tokens < before_output_tokens,
            "{}: rewritten tool output should save estimated tokens, before={before_output_tokens} after={after_output_tokens}",
            trace.name
        );

        for signal in &trace.critical_signals {
            assert_eq!(
                smart_context_golden_occurrences(before_text, signal),
                smart_context_golden_occurrences(after_text, signal),
                "{}: critical signal changed: {signal}",
                trace.name
            );
        }
        for reference in &trace.rehydrate_refs {
            assert_eq!(
                smart_context_golden_occurrences(before_text, reference),
                smart_context_golden_occurrences(after_text, reference),
                "{}: rehydrate ref changed: {reference}",
                trace.name
            );
        }

        let missing_rehydrate_refs = trace
            .rehydrate_refs
            .iter()
            .filter(|reference| !after_text.contains(**reference))
            .map(|reference| (*reference).to_string())
            .collect::<Vec<_>>();
        let regression =
            smart_context_regression_self_check(SmartContextRegressionSelfCheckInput {
                exactness_guard: smart_context_exactness_guard(
                    SmartContextExactnessInput::default(),
                ),
                before_hash: smart_context_hash_text(before_text),
                after_hash: smart_context_hash_text(after_text),
                before_estimated_tokens: before_body_tokens,
                after_estimated_tokens: after_body_tokens,
                before_critical_signal_count: trace.critical_signal_count(before_text),
                after_critical_signal_count: trace.critical_signal_count(after_text),
                missing_rehydrate_refs,
                unresolved_rehydrate_refs_are_segment_local: false,
            });

        assert_eq!(
            regression.decision,
            SmartContextRegressionSelfCheckDecision::Pass,
            "{}: regression self-check failed: {:?}",
            trace.name,
            regression.reasons
        );
        assert!(
            regression.saved_tokens >= trace.min_saved_tokens,
            "{}: saved token floor regressed, saved={} floor={}",
            trace.name,
            regression.saved_tokens,
            trace.min_saved_tokens
        );
    }
}

struct SmartContextGoldenProdexTrace {
    name: &'static str,
    model: &'static str,
    call_id: &'static str,
    static_artifact_id: &'static str,
    tool_artifact_id: &'static str,
    user_request: String,
    static_context: String,
    static_summary: String,
    tool_output: String,
    tool_summary: String,
    critical_signals: Vec<&'static str>,
    rehydrate_refs: Vec<&'static str>,
    max_after_token_ratio_percent: u64,
    min_saved_tokens: u64,
}

struct SmartContextGoldenProdexRewrite {
    before_body: Vec<u8>,
    after_body: Vec<u8>,
    before_context: String,
    after_context: String,
    before_tool_output: String,
    after_tool_output: String,
}

impl SmartContextGoldenProdexTrace {
    fn rewrite(&self) -> SmartContextGoldenProdexRewrite {
        let static_artifact =
            smart_context_golden_artifact(self.static_artifact_id, &self.static_context);
        let tool_artifact = smart_context_golden_artifact(self.tool_artifact_id, &self.tool_output);
        let after_context = smart_context_artifact_marker(&static_artifact, &self.static_summary);
        let after_tool_output = smart_context_artifact_marker(&tool_artifact, &self.tool_summary);
        let before_value = serde_json::json!({
            "model": self.model,
            "input": [
                {
                    "role": "system",
                    "content": self.static_context.as_str(),
                },
                {
                    "role": "user",
                    "content": self.user_request.as_str(),
                },
                {
                    "type": "function_call_output",
                    "call_id": self.call_id,
                    "output": self.tool_output.as_str(),
                },
            ],
            "store": true,
        });
        let after_value = serde_json::json!({
            "model": self.model,
            "input": [
                {
                    "role": "system",
                    "content": after_context.as_str(),
                },
                {
                    "role": "user",
                    "content": self.user_request.as_str(),
                },
                {
                    "type": "function_call_output",
                    "call_id": self.call_id,
                    "output": after_tool_output.as_str(),
                },
            ],
            "store": true,
        });

        SmartContextGoldenProdexRewrite {
            before_body: serde_json::to_vec(&before_value).unwrap(),
            after_body: serde_json::to_vec(&after_value).unwrap(),
            before_context: self.static_context.clone(),
            after_context,
            before_tool_output: self.tool_output.clone(),
            after_tool_output,
        }
    }

    fn critical_signal_count(&self, text: &str) -> usize {
        self.critical_signals
            .iter()
            .map(|signal| smart_context_golden_occurrences(text, signal))
            .sum()
    }
}

fn smart_context_golden_prodex_traces() -> Vec<SmartContextGoldenProdexTrace> {
    vec![
        smart_context_golden_prodex_s_trace(),
        smart_context_golden_prodex_super_trace(),
        smart_context_golden_prodex_s_runtime_stall_trace(),
        smart_context_golden_prodex_super_quota_trace(),
    ]
}

fn smart_context_golden_prodex_s_trace() -> SmartContextGoldenProdexTrace {
    let static_context = format!(
        "Prodex S runtime proxy instructions.\n{}\nKeep profile affinity and preserve upstream metadata.",
        "Never print while Codex TUI is active. ".repeat(180)
    );
    let static_summary =
        "Prodex S static context stored as artifact. Keep profile affinity and upstream metadata."
            .to_string();
    let diagnostics = [
        "Compiling prodex v0.0.0 (/workspace/prodex)",
        "error[E0425]: cannot find value `MISSING_CONTEXT` in this scope",
        "  --> crates/prodex-runtime-proxy/src/smart_context.rs:1710:9",
        "   |",
        "1710 |         MISSING_CONTEXT",
        "   |         ^^^^^^^^^^^^^^^ not found in this scope",
        "---- smart_context::golden_prodex_s_trace stdout ----",
        "thread 'smart_context::golden_prodex_s_trace' panicked at crates/prodex-runtime-proxy/src/smart_context.rs:1710:9",
    ]
    .join("\n");
    let tool_output = format!(
        "{diagnostics}\n{}",
        (0..220)
            .map(|index| format!("line {index}: noisy cargo build progress for prodex s profile"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SmartContextGoldenProdexTrace {
        name: "prodex-s-build-trace",
        model: "gpt-5",
        call_id: "call-prodex-s-build",
        static_artifact_id: "sc:prodex-s-static-context",
        tool_artifact_id: "sc:prodex-s-build-output",
        user_request: "Run focused runtime proxy test for Prodex s and inspect psc:prodex-s-build#L12-L18 only if needed.".to_string(),
        static_context,
        static_summary,
        tool_output,
        tool_summary: diagnostics,
        critical_signals: vec![
            "error[E0425]: cannot find value `MISSING_CONTEXT` in this scope",
            "crates/prodex-runtime-proxy/src/smart_context.rs:1710:9",
            "smart_context::golden_prodex_s_trace",
        ],
        rehydrate_refs: vec!["psc:prodex-s-build#L12-L18"],
        max_after_token_ratio_percent: 25,
        min_saved_tokens: 2_000,
    }
}

fn smart_context_golden_prodex_super_trace() -> SmartContextGoldenProdexTrace {
    let static_context = format!(
        "Prodex Super runtime context.\n{}\nSmart Context Autopilot remains transport-transparent.",
        "Preserve auto-rotate boundaries, runtime hot paths, and terminal silence. ".repeat(360)
    );
    let static_summary =
        "Prodex Super static context stored as artifact. Preserve rotate boundaries and terminal silence."
            .to_string();
    let runtime_signals = [
        "runtime_proxy_queue_overloaded lane=responses active=32 limit=32",
        "profile_inflight_saturated profile=super-a route=responses in_flight=4 cap=4",
        "prodex-runtime-latest.path=/tmp/prodex-runtime-latest.path",
        "thread 'runtime_proxy::super_golden_trace' panicked at crates/prodex-app/src/runtime_proxy/smart_context.rs:4702:17",
        "error: upstream stream ended before first_local_chunk",
    ]
    .join("\n");
    let tool_output = format!(
        "{runtime_signals}\n{}",
        (0..520)
            .map(|index| format!("2026-05-04T00:{:02}:00Z trace_id=req-super-{index:03} websocket keepalive and scheduler noise", index % 60))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SmartContextGoldenProdexTrace {
        name: "prodex-super-runtime-trace",
        model: "gpt-5.2",
        call_id: "call-prodex-super-runtime",
        static_artifact_id: "sc:prodex-super-static-context",
        tool_artifact_id: "sc:prodex-super-runtime-output",
        user_request: "Diagnose Prodex Super runtime pressure. Rehydrate psc:prodex-super-runtime#L31-L44 only if exact log lines are needed.".to_string(),
        static_context,
        static_summary,
        tool_output,
        tool_summary: runtime_signals,
        critical_signals: vec![
            "runtime_proxy_queue_overloaded lane=responses active=32 limit=32",
            "profile_inflight_saturated profile=super-a route=responses in_flight=4 cap=4",
            "prodex-runtime-latest.path=/tmp/prodex-runtime-latest.path",
            "runtime_proxy::super_golden_trace",
            "crates/prodex-app/src/runtime_proxy/smart_context.rs:4702:17",
            "error: upstream stream ended before first_local_chunk",
        ],
        rehydrate_refs: vec!["psc:prodex-super-runtime#L31-L44"],
        max_after_token_ratio_percent: 15,
        min_saved_tokens: 12_000,
    }
}

fn smart_context_golden_prodex_s_runtime_stall_trace() -> SmartContextGoldenProdexTrace {
    let static_context = format!(
        "Prodex S runtime stall triage context.\n{}\nInspect runtime logs before changing proxy selection behavior.",
        "Preserve previous_response_id affinity, x-codex-turn-state affinity, session_id compact affinity, and terminal silence. ".repeat(240)
    );
    let static_summary =
        "Prodex S stall context stored as artifact. Preserve affinity and inspect runtime logs first."
            .to_string();
    let critical_block = [
        "$ prodex doctor --runtime --json",
        r#"{"log_path":"/tmp/prodex-runtime-20260505-031455.log","latest_pointer":"/tmp/prodex-runtime-latest.path","profile_count":6}"#,
        "$ tail -n 160 /tmp/prodex-runtime-20260505-031455.log",
        "2026-05-05T03:14:58.102Z INFO route=responses profile=s-east request_id=req_s_7a1 first_upstream_chunk elapsed_ms=821",
        "2026-05-05T03:14:59.447Z WARN runtime_proxy_lane_limit_reached lane=responses active=8 limit=8 request_id=req_s_7a2",
        "2026-05-05T03:14:59.450Z WARN profile_transport_backoff profile=s-east route=responses ttl_ms=1500 error=stream_read_error",
        "2026-05-05T03:15:01.036Z ERROR stream_read_error route=responses profile=s-east request_id=req_s_7a1 error=connection reset before first_local_chunk",
        "critical: no first_local_chunk emitted for req_s_7a1",
        "artifact psc:prodex-s-runtime-stall#L88-L104 should be rehydrated before editing selection",
        "artifact psc:prodex-s-runtime-stall#L88-L104 referenced again by failing terminal transcript",
        "terminal transcript: /workspace/prodex/crates/prodex-runtime-proxy/src/selection.rs:392:21 still pending",
        "test failure: runtime_proxy_stall_preserves_affinity panicked at crates/prodex-runtime-proxy/tests/src/smart_context.rs:2044:9",
    ]
    .join("\n");
    let tool_output = format!(
        "{critical_block}\n{}",
        (0..360)
            .map(|index| format!(
                "2026-05-05T03:{:02}:{:02}.{:03}Z DEBUG pid={} cwd=/workspace/prodex cmd='cargo test -q -p prodex-runtime-proxy runtime_proxy_ -- --test-threads=1' target/debug/deps/prodex_runtime_proxy-{} lane=responses route=responses poll={} path=crates/prodex-runtime-proxy/src/runtime_proxy.rs:{}",
                15 + (index / 60),
                index % 60,
                (index * 17) % 1000,
                42000 + index,
                index % 17,
                index,
                310 + (index % 90)
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SmartContextGoldenProdexTrace {
        name: "prodex-s-runtime-stall-real-trace",
        model: "gpt-5",
        call_id: "call-prodex-s-runtime-stall",
        static_artifact_id: "sc:prodex-s-runtime-stall-static-context",
        tool_artifact_id: "sc:prodex-s-runtime-stall",
        user_request: "Investigate the Prodex S stall. Keep psc:prodex-s-runtime-stall#L88-L104 and psc:prodex-s-runtime-stall#L88-L104 available for exact rehydrate if selection logic is touched.".to_string(),
        static_context,
        static_summary,
        tool_output,
        tool_summary: critical_block,
        critical_signals: vec![
            "runtime_proxy_lane_limit_reached lane=responses active=8 limit=8",
            "profile_transport_backoff profile=s-east route=responses",
            "stream_read_error route=responses profile=s-east",
            "critical: no first_local_chunk emitted for req_s_7a1",
            "/workspace/prodex/crates/prodex-runtime-proxy/src/selection.rs:392:21",
            "runtime_proxy_stall_preserves_affinity",
        ],
        rehydrate_refs: vec!["psc:prodex-s-runtime-stall#L88-L104"],
        max_after_token_ratio_percent: 18,
        min_saved_tokens: 8_000,
    }
}

fn smart_context_golden_prodex_super_quota_trace() -> SmartContextGoldenProdexTrace {
    let static_context = format!(
        "Prodex Super quota and compaction trace context.\n{}\nGeneric 429 bodies must pass through unless an explicit quota code is present.",
        "Remote compact may rotate only before commit, but hard session affinity and previous_response_id ownership must stay authoritative. ".repeat(420)
    );
    let static_summary =
        "Prodex Super quota context stored as artifact. Preserve explicit quota classification and hard affinity."
            .to_string();
    let critical_block = [
        "$ cargo test -q -p prodex-runtime-proxy smart_context -- --test-threads=1",
        "---- runtime_proxy::compact_quota_real_trace stdout ----",
        "request POST /responses/compact session_id=sess_super_492 previous_response_id=resp_super_abcd",
        "selected profile=super-west route=compact affinity=session_id source=session_profile_bindings",
        "HTTP/1.1 429 Too Many Requests",
        r#"body={"error":{"type":"insufficient_quota","code":"insufficient_quota","message":"quota exceeded for account"}}"#,
        "quota_classification=account_quota route=compact profile=super-west rotate_allowed=true committed=false",
        "critical: generic_429_passthrough=false explicit_quota_code=true",
        "artifact psc:prodex-super-quota#L41-L63 contains sanitized upstream response",
        "artifact psc:prodex-super-quota#L41-L63 repeated in pytest-style failure footer",
        "thread 'runtime_proxy::compact_quota_real_trace' panicked at crates/prodex-runtime-proxy/tests/src/smart_context.rs:2121:13",
        "left: Synthetic429 right: UpstreamPassThrough at /workspace/prodex/crates/prodex-runtime-proxy/src/quota.rs:277:18",
    ]
    .join("\n");
    let tool_output = format!(
        "{critical_block}\n{}",
        (0..640)
            .map(|index| format!(
                "running test case {:04}: profile=super-west session_id=sess_super_492 previous_response_id=resp_super_abcd artifact=psc:prodex-super-quota-noise#L{}-L{} file=/workspace/prodex/crates/prodex-runtime-proxy/tests/fixtures/sanitized/compact_quota_{:04}.json stdout='poll compact retry {}' stderr='warning: unused retry budget sample'",
                index,
                200 + index,
                201 + index,
                index % 23,
                index % 5
            ))
            .collect::<Vec<_>>()
            .join("\n")
    );

    SmartContextGoldenProdexTrace {
        name: "prodex-super-quota-real-trace",
        model: "gpt-5.2",
        call_id: "call-prodex-super-quota",
        static_artifact_id: "sc:prodex-super-quota-static-context",
        tool_artifact_id: "sc:prodex-super-quota",
        user_request: "Debug Prodex Super compact quota behavior. Preserve psc:prodex-super-quota#L41-L63 and psc:prodex-super-quota#L41-L63 for exact rehydrate before changing 429 classification.".to_string(),
        static_context,
        static_summary,
        tool_output,
        tool_summary: critical_block,
        critical_signals: vec![
            "request POST /responses/compact session_id=sess_super_492 previous_response_id=resp_super_abcd",
            "selected profile=super-west route=compact affinity=session_id source=session_profile_bindings",
            r#""code":"insufficient_quota""#,
            "quota_classification=account_quota route=compact profile=super-west",
            "critical: generic_429_passthrough=false explicit_quota_code=true",
            "/workspace/prodex/crates/prodex-runtime-proxy/src/quota.rs:277:18",
        ],
        rehydrate_refs: vec!["psc:prodex-super-quota#L41-L63"],
        max_after_token_ratio_percent: 12,
        min_saved_tokens: 18_000,
    }
}

fn smart_context_golden_artifact(id: &str, text: &str) -> SmartContextArtifactRef {
    SmartContextArtifactRef {
        id: id.to_string(),
        byte_len: text.len(),
        content_hash: smart_context_hash_text(text),
    }
}

fn smart_context_golden_occurrences(haystack: &str, needle: &str) -> usize {
    haystack.match_indices(needle).count()
}
