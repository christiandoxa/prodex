use super::*;

#[test]
fn runtime_doctor_parses_smart_context_autopilot_roi_fields() {
    let event = runtime_doctor_parse_smart_context_autopilot_line(
        "[2026-05-04T00:00:00Z] smart_context_autopilot request=42 transport=http route=responses tier=large decision=rewritten reasons=- token_usage_source=responses_api observed_context_tokens=12345 body_bytes_before=12000 body_bytes_after=3999 body_bytes_saved=8001 rewrite_ratio_percent=33 self_check=shrunk available_tokens=19000 budget_mode=artifact_condensed max_inline_tool_output_bytes=4096 max_rehydrate_tokens=1024 policy_reasons=tight_budget,missing_rehydrate_refs artifacts_stored=2 tool_outputs_condensed=1 duplicate_texts=3 cross_turn_duplicate_texts=1 repeat_tool_output_refs=5 blob_outputs_condensed=1 rehydrated_refs=4 static_context_deltas=6 repo_state_facts=7",
    )
    .expect("smart context autopilot line should parse");

    assert_eq!(event.request.as_deref(), Some("42"));
    assert_eq!(event.transport.as_deref(), Some("http"));
    assert_eq!(event.route.as_deref(), Some("responses"));
    assert_eq!(event.tier.as_deref(), Some("large"));
    assert_eq!(event.decision.as_deref(), Some("rewritten"));
    assert_eq!(event.reasons, Vec::<String>::new());
    assert_eq!(event.token_usage_source.as_deref(), Some("responses_api"));
    assert_eq!(event.observed_context_tokens, Some(12345));
    assert_eq!(event.body_bytes_before, Some(12000));
    assert_eq!(event.body_bytes_after, Some(3999));
    assert_eq!(event.body_bytes_saved, Some(8001));
    assert_eq!(event.estimated_tokens_saved, Some(2001));
    assert_eq!(event.rewrite_ratio_percent, Some(33));
    assert_eq!(event.self_check.as_deref(), Some("shrunk"));
    assert_eq!(event.available_tokens, Some(19000));
    assert_eq!(event.policy_mode.as_deref(), Some("artifact_condensed"));
    assert_eq!(
        event.policy_reasons,
        vec![
            "tight_budget".to_string(),
            "missing_rehydrate_refs".to_string()
        ]
    );
    assert_eq!(event.artifacts_stored, Some(2));
    assert_eq!(event.tool_outputs_condensed, Some(1));
    assert_eq!(event.duplicate_texts, Some(3));
    assert_eq!(event.cross_turn_duplicate_texts, Some(1));
    assert_eq!(event.repeat_tool_output_refs, Some(5));
    assert_eq!(event.blob_outputs_condensed, Some(1));
    assert_eq!(event.rehydrated_refs, Some(4));
    assert_eq!(event.static_context_deltas, Some(6));
    assert_eq!(event.repo_state_facts, Some(7));
}

#[test]
fn runtime_doctor_summarizes_smart_context_autopilot_roi_and_reasons() {
    let log = br#"
[2026-05-04T00:00:00Z] smart_context_autopilot request=1 transport=http route=responses tier=large decision=rewritten reasons=- token_usage_source=responses_api body_bytes_before=12000 body_bytes_after=4000 self_check=shrunk budget_mode=artifact_condensed policy_reasons=tight_budget artifacts_stored=1 repeat_tool_output_refs=2 static_context_deltas=3 repo_state_facts=4
[2026-05-04T00:00:01Z] smart_context_autopilot request=2 transport=http route=responses tier=condensed decision=self_check_passthrough reasons=- token_usage_source=responses_api body_bytes_before=8000 body_bytes_after=8000 self_check=critical_signal_loss budget_mode=minimal_refs_only policy_reasons=critical_budget repeat_tool_output_refs=5 static_context_deltas=7 repo_state_facts=11
[2026-05-04T00:00:02Z] smart_context_autopilot request=3 transport=websocket route=websocket tier=exact decision=require_exact reasons=previous_response,turn_state token_usage_source=unknown body_bytes_before=6000 body_bytes_after=6000 self_check=pass_through_exact budget_mode=exact_pass_through policy_reasons=exactness_required repeat_tool_output_refs=13 static_context_deltas=17 repo_state_facts=19
[2026-05-04T00:00:03Z] first_local_chunk request=3 transport=http
"#;

    let summary = runtime_doctor_summarize_smart_context_autopilot_tail(log);

    assert_eq!(summary.event_count, 3);
    assert_eq!(summary.rewrite_count, 1);
    assert_eq!(summary.fallback_count, 2);
    assert_eq!(summary.total_body_bytes_before, 26000);
    assert_eq!(summary.total_body_bytes_after, 18000);
    assert_eq!(summary.total_body_bytes_saved, 8000);
    assert_eq!(summary.estimated_tokens_saved, 2000);
    assert_eq!(summary.total_repeat_tool_output_refs, 20);
    assert_eq!(summary.total_static_context_deltas, 27);
    assert_eq!(summary.total_repo_state_facts, 34);
    assert_eq!(summary.decision_counts["rewritten"], 1);
    assert_eq!(summary.decision_counts["self_check_passthrough"], 1);
    assert_eq!(summary.decision_counts["require_exact"], 1);
    assert_eq!(summary.fallback_reason_counts["critical_signal_loss"], 1);
    assert_eq!(summary.fallback_reason_counts["previous_response"], 1);
    assert_eq!(summary.fallback_reason_counts["turn_state"], 1);
    assert_eq!(summary.self_check_counts["shrunk"], 1);
    assert_eq!(summary.self_check_counts["pass_through_exact"], 1);
    assert_eq!(summary.policy_mode_counts["artifact_condensed"], 1);
    assert_eq!(summary.policy_mode_counts["minimal_refs_only"], 1);
    assert_eq!(summary.policy_reason_counts["tight_budget"], 1);
    assert_eq!(summary.policy_reason_counts["critical_budget"], 1);
    assert_eq!(summary.policy_reason_counts["exactness_required"], 1);
    assert_eq!(summary.token_usage_source_counts["responses_api"], 2);
    assert_eq!(
        summary
            .latest_event
            .as_ref()
            .and_then(|event| event.request.as_deref()),
        Some("3")
    );
    assert_eq!(
        summary
            .latest_event
            .as_ref()
            .and_then(|event| event.repeat_tool_output_refs),
        Some(13)
    );
    assert_eq!(
        summary
            .latest_event
            .as_ref()
            .and_then(|event| event.static_context_deltas),
        Some(17)
    );
    assert_eq!(
        summary
            .latest_event
            .as_ref()
            .and_then(|event| event.repo_state_facts),
        Some(19)
    );

    let value = serde_json::to_value(&summary).expect("summary should serialize");
    assert_eq!(value["latest_event"]["repeat_tool_output_refs"], 13);
    assert_eq!(value["latest_event"]["static_context_deltas"], 17);
    assert_eq!(value["latest_event"]["repo_state_facts"], 19);
}

#[test]
fn runtime_doctor_parses_json_smart_context_autopilot_fields() {
    let line = r#"{"timestamp":"2026-05-04T00:00:00Z","message":"smart_context_autopilot request=9 decision=rewritten body_bytes_before=100 body_bytes_after=20","fields":{"body_bytes_saved":80,"budget_mode":"artifact_condensed","policy_reasons":"tight_budget,moderate_budget","self_check":"shrunk","available_tokens":2500,"repeat_tool_output_refs":3,"static_context_deltas":4,"repo_state_facts":5}}"#;

    let event = runtime_doctor_parse_smart_context_autopilot_line(line)
        .expect("JSON smart context autopilot line should parse");

    assert_eq!(event.request.as_deref(), Some("9"));
    assert_eq!(event.body_bytes_saved, Some(80));
    assert_eq!(event.estimated_tokens_saved, Some(20));
    assert_eq!(event.available_tokens, Some(2500));
    assert_eq!(event.policy_mode.as_deref(), Some("artifact_condensed"));
    assert_eq!(
        event.policy_reasons,
        vec!["tight_budget".to_string(), "moderate_budget".to_string()]
    );
    assert_eq!(event.self_check.as_deref(), Some("shrunk"));
    assert_eq!(event.repeat_tool_output_refs, Some(3));
    assert_eq!(event.static_context_deltas, Some(4));
    assert_eq!(event.repo_state_facts, Some(5));
}

#[test]
fn runtime_doctor_tail_summary_counts_smart_context_marker_and_facets() {
    let log = br#"
[2026-05-04T00:00:00Z] smart_context_autopilot request=1 transport=http route=responses tier=large decision=rewritten reasons=- token_usage_source=responses_api body_bytes_before=100 body_bytes_after=20 self_check=shrunk budget_mode=artifact_condensed policy_reasons=tight_budget
[2026-05-04T00:00:01Z] smart_context_autopilot request=2 transport=http route=responses tier=exact decision=require_exact reasons=previous_response token_usage_source=unknown body_bytes_before=100 body_bytes_after=100 self_check=pass_through_exact budget_mode=exact_pass_through policy_reasons=exactness_required
"#;

    let summary = summarize_runtime_log_tail(log);

    assert_eq!(
        summary.marker_counts["smart_context_autopilot"], 2,
        "runtime doctor marker registry should include smart context autopilot"
    );
    assert_eq!(summary.facet_counts["decision"]["rewritten"], 1);
    assert_eq!(summary.facet_counts["decision"]["require_exact"], 1);
    assert_eq!(summary.facet_counts["tier"]["large"], 1);
    assert_eq!(summary.facet_counts["budget_mode"]["exact_pass_through"], 1);
    assert_eq!(
        summary.marker_last_fields["smart_context_autopilot"]["self_check"],
        "pass_through_exact"
    );
}
