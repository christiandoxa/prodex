use super::helpers::*;
use super::*;

fn usage(five_used: i64, week_used: i64, weekly_only: bool) -> serde_json::Value {
    let mut value: serde_json::Value = serde_json::from_str(&runtime_proxy_usage_body("fixture@example.com")).unwrap();
    value["rate_limit"]["primary_window"]["used_percent"] = five_used.into();
    value["rate_limit"]["primary_window"]["reset_at"] = (Local::now().timestamp() + 3600).into();
    value["rate_limit"]["secondary_window"]["used_percent"] = week_used.into();
    value["rate_limit"]["secondary_window"]["reset_at"] = (Local::now().timestamp() + 604800).into();
    if weekly_only {
        value["plan_type"] = "prolite".into();
        value["rate_limit"]["primary_window"] = serde_json::Value::Null;
    }
    value
}

fn seed_snapshot(harness: &RuntimeProxyProfileHarness, name: &str, raw: serde_json::Value, block_weekly: Option<bool>) {
    let usage:UsageResponse=serde_json::from_value(raw).unwrap();
    let mut snapshot=runtime_profile_usage_snapshot_from_usage(&usage);
    let now=Local::now().timestamp();snapshot.checked_at=now-2;
    let mut runtime=harness.shared().runtime.lock().unwrap();
    if let Some(weekly)=block_weekly {
        let until=if weekly {snapshot.weekly_reset_at} else {snapshot.five_hour_reset_at};
        runtime.profile_retry_backoff_until.insert(name.into(),until);
        runtime.profile_backoff_updated_at.insert(prodex_runtime_store::runtime_profile_retry_backoff_update_key(name),(now-2)*1000);
    }
    runtime.profile_probe_cache.insert(name.into(), RuntimeProfileProbeCacheEntry {
        checked_at:now,auth:AuthSummary{label:"chatgpt".into(),quota_compatible:true},result:Ok(usage),
    });
    runtime.profile_usage_snapshots.insert(name.into(),snapshot);
}

fn probe_profile(harness:&RuntimeProxyProfileHarness,name:&str) {
    let home=harness.shared().runtime.lock().unwrap().state.profiles[name].codex_home.clone();
    crate::run_runtime_probe_jobs_inline(harness.shared(),vec![(name.into(),home)],"transition_probe");
}

fn send_complete_history(harness:&RuntimeProxyProfileHarness,id:u64,expected:&str) {
    let request=RuntimeProxyRequest {
        method:"POST".into(),path_and_query:"/backend-api/codex/responses".into(),
        headers:vec![("Content-Type".into(),"application/json".into())],
        body:br#"{"model":"gpt-6.1-sol","input":[{"role":"user","content":"complete persisted history, no opaque continuation"}]}"#.to_vec(),
    };
    let (status,body)=match proxy_runtime_responses_request(id,&request,harness.shared()).unwrap() {
        RuntimeResponsesReply::Buffered(parts)=>(parts.status,String::from_utf8_lossy(&parts.body).into_owned()),
        RuntimeResponsesReply::Streaming(mut response)=>{let mut body=String::new();response.body.read_to_string(&mut body).unwrap();(response.status,body)},
    };
    assert_eq!(status,200,"{body}");assert!(body.contains(expected),"{body}");
}

#[test]
fn quota_transition_043714_three_profiles_rotate_and_rejoin_after_both_window_resets() {
    let backend=RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses,"main-account","before-block"),
        RuntimeProxyBackendFaultStep::quota_snapshot("third-account",usage(0,54,true)),
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses,"third-account","weekly-only-ready"),
        RuntimeProxyBackendFaultStep::success(RuntimeProxyBackendFaultRoute::Compact,"third-account"),
        RuntimeProxyBackendFaultStep::quota_snapshot("second-account",usage(10,20,false)),
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses,"second-account","five-hour-restored"),
        RuntimeProxyBackendFaultStep::quota_snapshot("main-account",usage(10,20,false)),
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses,"main-account","weekly-restored"),
    ]));
    let harness=RuntimeProxyProfileHarnessBuilder::new()
        .openai_profile("main","main-account",Some("main@example.com"))
        .openai_profile("second","second-account",Some("second@example.com"))
        .openai_profile("third","third-account",Some("third@example.com"))
        .active_profile("main").current_profile("main").upstream_base_url(backend.base_url()).build();
    for name in ["main","second","third"] {seed_snapshot(&harness,name,usage(10,20,false),None);}
    send_complete_history(&harness,4371410,"before-block");
    seed_snapshot(&harness,"main",usage(26,100,false),Some(true));
    seed_snapshot(&harness,"second",usage(100,20,false),Some(false));
    seed_snapshot(&harness,"third",usage(0,54,true),None);
    harness.shared().runtime.lock().unwrap().profile_health.insert(runtime_profile_auth_failure_key("third"),RuntimeProfileHealth{score:12,updated_at:Local::now().timestamp()-2});
    probe_profile(&harness,"third");
    send_complete_history(&harness,4371411,"weekly-only-ready");
    let compact=RuntimeProxyRequest {
        method:"POST".into(),path_and_query:"/backend-api/codex/responses/compact".into(),
        headers:vec![("Content-Type".into(),"application/json".into())],
        body:br#"{"model":"gpt-6.1-sol","input":[{"role":"user","content":"compact full history"}]}"#.to_vec(),
    };
    let (status,body)=tiny_http_response_status_and_body(proxy_runtime_standard_request(4371412,&compact,harness.shared()).unwrap());
    assert_eq!(status,200,"compaction must use the surviving ready profile: {body}");
    seed_snapshot(&harness,"third",usage(0,100,true),Some(true));
    probe_profile(&harness,"second");
    send_complete_history(&harness,4371413,"five-hour-restored");
    seed_snapshot(&harness,"second",usage(100,20,false),Some(false));
    probe_profile(&harness,"main");
    send_complete_history(&harness,4371414,"weekly-restored");
    assert_eq!(backend.responses_accounts(),["main-account","third-account","third-account","second-account","main-account"],
        "every request must execute exactly once on its eligible owner");
    let log=read_runtime_proxy_test_log(&harness.shared().log_path);
    assert!(!log.contains("precommit_budget_exhausted"),"no local retry exhaustion while a revalidated profile is ready: {log}");
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("main"),0);
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("second"),0);
    assert_eq!(harness.shared().lane_admission.profile_inflight_count("third"),0);
}
