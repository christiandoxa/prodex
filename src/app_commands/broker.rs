use super::*;

pub(crate) fn handle_runtime_broker(args: RuntimeBrokerArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let proxy = start_runtime_rotation_proxy_with_listen_addr(
        &paths,
        &state,
        &args.current_profile,
        args.upstream_base_url.clone(),
        args.include_code_review,
        args.listen_addr.as_deref(),
    )?;
    if proxy.owner_lock.is_none() {
        return Ok(());
    }

    let metadata = RuntimeBrokerMetadata {
        broker_key: runtime_broker_key(&args.upstream_base_url, args.include_code_review),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: Local::now().timestamp(),
        current_profile: args.current_profile.clone(),
        include_code_review: args.include_code_review,
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
    };
    register_runtime_broker_metadata(&proxy.log_path, metadata.clone());
    let registry = RuntimeBrokerRegistry {
        pid: std::process::id(),
        listen_addr: proxy.listen_addr.to_string(),
        started_at: metadata.started_at,
        upstream_base_url: args.upstream_base_url.clone(),
        include_code_review: args.include_code_review,
        current_profile: args.current_profile.clone(),
        instance_token: args.instance_token.clone(),
        admin_token: args.admin_token.clone(),
        openai_mount_path: Some(RUNTIME_PROXY_OPENAI_MOUNT_PATH.to_string()),
    };
    save_runtime_broker_registry(&paths, &args.broker_key, &registry)?;
    runtime_proxy_log_to_path(
        &proxy.log_path,
        &format!(
            "runtime_broker_started listen_addr={} broker_key={} current_profile={} include_code_review={}",
            proxy.listen_addr, args.broker_key, args.current_profile, args.include_code_review
        ),
    );
    audit_log_event_best_effort(
        "runtime_broker",
        "start",
        "success",
        serde_json::json!({
            "broker_key": args.broker_key,
            "listen_addr": proxy.listen_addr.to_string(),
            "current_profile": args.current_profile,
            "include_code_review": args.include_code_review,
            "upstream_base_url": args.upstream_base_url,
        }),
    );

    let startup_grace_until = metadata
        .started_at
        .saturating_add(runtime_broker_startup_grace_seconds());
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    let lease_scan_interval = Duration::from_millis(
        RUNTIME_BROKER_LEASE_SCAN_INTERVAL_MS.max(RUNTIME_BROKER_POLL_INTERVAL_MS),
    );
    let mut idle_started_at = None::<i64>;
    let mut cached_live_leases = 0usize;
    let mut last_lease_scan_at = Instant::now() - lease_scan_interval;
    loop {
        let active_requests = proxy.active_request_count.load(Ordering::SeqCst);
        if active_requests == 0 && last_lease_scan_at.elapsed() >= lease_scan_interval {
            cached_live_leases = cleanup_runtime_broker_stale_leases(&paths, &args.broker_key);
            last_lease_scan_at = Instant::now();
        }
        if cached_live_leases > 0 || active_requests > 0 {
            idle_started_at = None;
        } else {
            let now = Local::now().timestamp();
            if now < startup_grace_until {
                idle_started_at = None;
                thread::sleep(poll_interval);
                continue;
            }
            let idle_since = idle_started_at.get_or_insert(now);
            if now.saturating_sub(*idle_since) >= RUNTIME_BROKER_IDLE_GRACE_SECONDS {
                runtime_proxy_log_to_path(
                    &proxy.log_path,
                    &format!(
                        "runtime_broker_idle_shutdown broker_key={} idle_seconds={}",
                        args.broker_key,
                        now.saturating_sub(*idle_since)
                    ),
                );
                break;
            }
        }
        thread::sleep(poll_interval);
    }

    drop(proxy);
    remove_runtime_broker_registry_if_token_matches(&paths, &args.broker_key, &args.instance_token);
    Ok(())
}
