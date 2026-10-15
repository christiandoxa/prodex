use super::helpers::*;
use super::*;

fn probe(harness: &RuntimeProxyProfileHarness) {
    let home = harness.shared().runtime.lock().unwrap().state.profiles["main"].codex_home.clone();
    crate::run_runtime_probe_jobs_inline(harness.shared(), vec![("main".into(), home)], "revalidation_test");
}

#[test]
fn probe_recovery_043714_authenticated_success_clears_stale_auth_penalty() {
    let backend = RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([
        RuntimeProxyBackendFaultStep::sse_success(RuntimeProxyBackendFaultRoute::Responses, "main-account", "recovered-once"),
        RuntimeProxyBackendFaultStep::success(RuntimeProxyBackendFaultRoute::Compact, "main-account"),
    ]));
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(46, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile("main", "main-account", "main@example.com")
        .upstream_base_url(backend.base_url()).profile_usage_snapshot("main", ready).build();
    let now = Local::now().timestamp();
    {
        let mut runtime = harness.shared().runtime.lock().unwrap();
        runtime.profile_health.insert(runtime_profile_auth_failure_key("main"), RuntimeProfileHealth { score: 12, updated_at: now - 2 });
    }
    probe(&harness);
    let runtime = harness.shared().runtime.lock().unwrap();
    assert_eq!(runtime.profile_health[&runtime_profile_auth_failure_key("main")].score, 0,
        "a completed authenticated quota probe must supersede an older credential penalty even when the credential bytes did not change");
    drop(runtime);
    assert_eq!(backend.usage_accounts(), ["main-account"]);
    let request = RuntimeProxyRequest { method: "POST".into(), path_and_query: "/backend-api/codex/responses".into(), headers: vec![("Content-Type".into(), "application/json".into())], body: br#"{"model":"gpt-6.1-sol","input":[{"role":"user","content":"continue complete history"}]}"#.to_vec() };
    let reply = proxy_runtime_responses_request(4371401, &request, harness.shared()).unwrap();
    let (status, body) = match reply {
        RuntimeResponsesReply::Buffered(parts) => (parts.status, String::from_utf8_lossy(&parts.body).into_owned()),
        RuntimeResponsesReply::Streaming(mut reply) => { let mut body=String::new(); reply.body.read_to_string(&mut body).unwrap(); (reply.status, body) },
    };
    assert_eq!(status, 200, "{body}");
    assert!(body.contains("recovered-once"), "{body}");
    let compact = RuntimeProxyRequest { path_and_query: "/backend-api/codex/responses/compact".into(), ..request };
    let response = proxy_runtime_standard_request(4371402, &compact, harness.shared()).unwrap();
    let (status, body) = tiny_http_response_status_and_body(response);
    assert_eq!(status, 200, "compaction must remain usable after revalidation: {body}");
    assert_eq!(backend.responses_accounts(), ["main-account", "main-account"]);
}

#[test]
fn probe_recovery_043714_ready_snapshot_releases_only_quota_quarantine() {
    for weekly in [false, true] {
        let backend = RuntimeProxyBackend::start();
        let mut blocked = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(80, 86400));
        let now = Local::now().timestamp();
        blocked.checked_at = now - 2;
        let reset = if weekly { blocked.weekly_status=RuntimeQuotaWindowStatus::Exhausted; blocked.weekly_remaining_percent=0; blocked.weekly_reset_at }
            else { blocked.five_hour_status=RuntimeQuotaWindowStatus::Exhausted; blocked.five_hour_remaining_percent=0; blocked.five_hour_reset_at };
        let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile("main", "main-account", "main@example.com")
            .upstream_base_url(backend.base_url()).profile_usage_snapshot("main", blocked).build();
        {
            let mut runtime=harness.shared().runtime.lock().unwrap();
            runtime.profile_retry_backoff_until.insert("main".into(), reset);
            runtime.profile_backoff_updated_at.insert(prodex_runtime_store::runtime_profile_retry_backoff_update_key("main"), (now-2)*1000);
        }
        probe(&harness);
        let runtime=harness.shared().runtime.lock().unwrap();
        assert!(!runtime.profile_retry_backoff_until.contains_key("main"), "confirmed usable quota must replace stale weekly={weekly} quarantine");
        assert!(runtime.profile_backoff_updated_at.contains_key(&prodex_runtime_store::runtime_profile_retry_backoff_update_key("main")), "clearing quarantine needs a merge-safe tombstone");
    }
}

#[test]
fn probe_recovery_043714_restart_revalidates_auth_despite_fresh_ready_snapshot() {
    let backend = RuntimeProxyBackend::start();
    let ready = runtime_usage_snapshot(quota_window_ready(80, 3600), quota_window_ready(46, 86400));
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile("main", "main-account", "main@example.com")
        .upstream_base_url(backend.base_url()).profile_usage_snapshot("main", ready).build();
    harness.shared().runtime.lock().unwrap().profile_health.insert(
        runtime_profile_auth_failure_key("main"), RuntimeProfileHealth { score:12, updated_at:Local::now().timestamp()-2 });
    schedule_runtime_startup_probe_warmup(harness.shared());
    assert_eq!(backend.usage_accounts(), ["main-account"], "fresh quota is not proof that an inherited auth penalty is still valid");
    assert_eq!(harness.shared().runtime.lock().unwrap().profile_health[&runtime_profile_auth_failure_key("main")].score,0);
}

#[test]
fn probe_recovery_043714_cached_ready_dto_cannot_clear_auth_or_newer_failures() {
    let backend = RuntimeProxyBackend::start();
    let ready = runtime_usage_snapshot(quota_window_ready(80,3600),quota_window_ready(46,86400));
    let harness = RuntimeProxyProfileHarnessBuilder::single_openai_profile("main","main-account","main@example.com")
        .upstream_base_url(backend.base_url()).profile_usage_snapshot("main",ready).build();
    let now=Local::now().timestamp();
    harness.shared().runtime.lock().unwrap().profile_health.insert(runtime_profile_auth_failure_key("main"),RuntimeProfileHealth{score:12,updated_at:now+30});
    probe(&harness);
    assert_eq!(harness.shared().runtime.lock().unwrap().profile_health[&runtime_profile_auth_failure_key("main")].score,12,
        "a probe started before a later authentication failure cannot acknowledge it");
    let usage:UsageResponse=serde_json::from_str(&runtime_proxy_usage_body("main@example.com")).unwrap();
    apply_runtime_profile_probe_result(harness.shared(),"main",AuthSummary{label:"chatgpt".into(),quota_compatible:true},Ok(usage)).unwrap();
    assert_eq!(harness.shared().runtime.lock().unwrap().profile_health[&runtime_profile_auth_failure_key("main")].score,12,
        "ordinary cache insertion is not an authenticated HTTP receipt");
}

#[test]
fn probe_recovery_043714_ready_probe_preserves_independent_backoffs() {
    let backend=RuntimeProxyBackend::start();
    let mut blocked=runtime_usage_snapshot(quota_window_exhausted(3600),quota_window_ready(70,86400));
    let now=Local::now().timestamp();blocked.checked_at=now-2;
    let unrelated_retry=blocked.five_hour_reset_at+10;
    let harness=RuntimeProxyProfileHarnessBuilder::single_openai_profile("main","main-account","main@example.com")
        .upstream_base_url(backend.base_url()).profile_usage_snapshot("main",blocked).build();
    {
        let mut runtime=harness.shared().runtime.lock().unwrap();
        runtime.profile_retry_backoff_until.insert("main".into(),unrelated_retry);
        runtime.profile_transport_backoff_until.insert("main".into(),now+120);
    }
    probe(&harness);
    let runtime=harness.shared().runtime.lock().unwrap();
    assert_eq!(runtime.profile_retry_backoff_until.get("main"),Some(&unrelated_retry));
    assert_eq!(runtime.profile_transport_backoff_until.get("main"),Some(&(now+120)));
}

#[test]
fn probe_recovery_043714_failed_or_uninformative_probe_cannot_clear_auth() {
    for invalid_json in [false,true] {
        let fault=if invalid_json {
            RuntimeProxyBackendFaultStep::quota_snapshot("main-account",serde_json::json!({"error":"invalid quota body"}))
        } else {
            RuntimeProxyBackendFaultStep::invalid_request(RuntimeProxyBackendFaultRoute::Usage,"main-account")
        };
        let backend=RuntimeProxyBackend::start_with_fault_script(RuntimeProxyBackendFaultScript::new([fault]));
        let ready=runtime_usage_snapshot(quota_window_ready(80,3600),quota_window_ready(46,86400));
        let harness=RuntimeProxyProfileHarnessBuilder::single_openai_profile("main","main-account","main@example.com")
            .upstream_base_url(backend.base_url()).profile_usage_snapshot("main",ready).build();
        harness.shared().runtime.lock().unwrap().profile_health.insert(runtime_profile_auth_failure_key("main"),RuntimeProfileHealth{score:12,updated_at:Local::now().timestamp()-2});
        probe(&harness);
        assert_eq!(harness.shared().runtime.lock().unwrap().profile_health[&runtime_profile_auth_failure_key("main")].score,12,
            "failed or empty HTTP observations are not affirmative recovery evidence");
    }
}

#[test]
fn probe_recovery_043714_delayed_success_keeps_newer_quarantine_until_fresh_proof() {
    let backend=RuntimeProxyBackend::start();
    let mut blocked=runtime_usage_snapshot(quota_window_exhausted(3600),quota_window_ready(70,86400));
    let now=Local::now().timestamp();blocked.checked_at=now+30;
    let until=blocked.five_hour_reset_at;
    let harness=RuntimeProxyProfileHarnessBuilder::single_openai_profile("main","main-account","main@example.com")
        .upstream_base_url(backend.base_url()).profile_usage_snapshot("main",blocked).build();
    let update_key=prodex_runtime_store::runtime_profile_retry_backoff_update_key("main");
    {
        let mut runtime=harness.shared().runtime.lock().unwrap();
        runtime.profile_retry_backoff_until.insert("main".into(),until);
        runtime.profile_backoff_updated_at.insert(update_key.clone(),(now+30)*1000);
    }
    probe(&harness);
    {
        let mut runtime=harness.shared().runtime.lock().unwrap();
        assert_eq!(runtime.profile_usage_snapshots["main"].five_hour_status,RuntimeQuotaWindowStatus::Exhausted,
            "a delayed successful probe must not erase the newer quarantine evidence needed for later recovery");
        assert_eq!(runtime.profile_retry_backoff_until.get("main"),Some(&until));
        runtime.profile_usage_snapshots.get_mut("main").unwrap().checked_at=now-2;
        runtime.profile_backoff_updated_at.insert(update_key,(now-2)*1000);
    }
    probe(&harness);
    let runtime=harness.shared().runtime.lock().unwrap();
    assert!(!runtime.profile_retry_backoff_until.contains_key("main"));
    assert_eq!(runtime.profile_usage_snapshots["main"].five_hour_status,RuntimeQuotaWindowStatus::Ready);
}
