use super::*;

pub(crate) fn runtime_broker_process_args(config: RuntimeBrokerSpawnConfig<'_>) -> Vec<OsString> {
    prodex_runtime_broker::runtime_broker_process_args(config)
}

pub(crate) fn wait_for_existing_runtime_broker_recovery_or_exit(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        let Some(existing) = load_runtime_broker_registry(paths, broker_key)? else {
            return Ok(None);
        };

        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                broker_key,
                &existing.instance_token,
            );
            return Ok(None);
        }

        let health = probe_runtime_broker_health(client, &existing)?;
        match replace_runtime_broker_if_version_mismatch_with_health(
            paths,
            broker_key,
            &existing,
            health.as_ref(),
        ) {
            RuntimeBrokerVersionGuardOutcome::Compatible => {}
            RuntimeBrokerVersionGuardOutcome::Replaced => return Ok(None),
            RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests => {
                return Ok(None);
            }
        }

        if existing.matches_launch_config(upstream_base_url, include_code_review, upstream_no_proxy)
            && let Some(health) = health
            && health.matches_registry_instance(&existing)
        {
            return Ok(Some(existing));
        }

        thread::sleep(poll_interval);
    }

    Ok(None)
}

pub(crate) fn find_compatible_runtime_broker_registry(
    client: &Client,
    paths: &AppPaths,
    excluded_broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Result<Option<(String, RuntimeBrokerRegistry)>> {
    for broker_key in runtime_broker_registry_keys(paths) {
        if broker_key == excluded_broker_key {
            continue;
        }

        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if !registry.matches_launch_config(
            upstream_base_url,
            include_code_review,
            upstream_no_proxy,
        ) {
            continue;
        }
        if !runtime_process_pid_alive(registry.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &registry.instance_token,
            );
            continue;
        }
        let health = probe_runtime_broker_health(client, &registry)?;
        match replace_runtime_broker_if_version_mismatch_with_health(
            paths,
            &broker_key,
            &registry,
            health.as_ref(),
        ) {
            RuntimeBrokerVersionGuardOutcome::Compatible => {}
            RuntimeBrokerVersionGuardOutcome::Replaced
            | RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests => continue,
        }
        if let Some(health) = health
            && health.matches_registry_instance(&registry)
        {
            return Ok(Some((broker_key, registry)));
        }
    }

    Ok(None)
}

pub(crate) fn wait_for_runtime_broker_ready(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    expected_instance_token: &str,
) -> Result<RuntimeBrokerRegistry> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        if let Some(registry) = load_runtime_broker_registry(paths, broker_key)?
            && registry.instance_token == expected_instance_token
            && let Some(health) = probe_runtime_broker_health(client, &registry)?
            && health.matches_registry_instance(&registry)
        {
            return Ok(registry);
        }
        thread::sleep(poll_interval);
    }
    bail!("timed out waiting for runtime broker readiness");
}

pub(crate) type RuntimeBrokerSpawnConfig<'a> = prodex_runtime_broker::RuntimeBrokerSpawnConfig<'a>;

pub(crate) fn spawn_runtime_broker_process(
    paths: &AppPaths,
    config: RuntimeBrokerSpawnConfig<'_>,
) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current prodex binary")?;
    Command::new(current_exe)
        .args(runtime_broker_process_args(config))
        .env("PRODEX_HOME", &paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn runtime broker process")?;
    Ok(())
}

pub(crate) fn preferred_runtime_broker_listen_addr(
    paths: &AppPaths,
    broker_key: &str,
) -> Result<Option<String>> {
    Ok(
        load_runtime_broker_registry(paths, broker_key)?.and_then(|registry| {
            (!runtime_process_pid_alive(registry.pid)).then_some(registry.listen_addr)
        }),
    )
}

pub(crate) fn ensure_runtime_rotation_proxy_endpoint(
    paths: &AppPaths,
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Result<RuntimeProxyEndpoint> {
    let broker_key = runtime_broker_key(upstream_base_url, include_code_review, upstream_no_proxy);
    let ensure_lock_path = runtime_broker_ensure_lock_path(paths, &broker_key);
    let _ensure_lock = acquire_json_file_lock(&ensure_lock_path)?;
    let preferred_listen_addr = preferred_runtime_broker_listen_addr(paths, &broker_key)?;
    let broker_client = runtime_broker_client()?;

    if let Some(existing) = wait_for_existing_runtime_broker_recovery_or_exit(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
    )? {
        activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
        return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
    }

    if let Some(existing) = load_runtime_broker_registry(paths, &broker_key)? {
        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &existing.instance_token,
            );
        } else {
            let health = probe_runtime_broker_health(&broker_client, &existing)?;
            match replace_runtime_broker_if_version_mismatch_with_health(
                paths,
                &broker_key,
                &existing,
                health.as_ref(),
            ) {
                RuntimeBrokerVersionGuardOutcome::Compatible => {
                    if existing.matches_launch_config(
                        upstream_base_url,
                        include_code_review,
                        upstream_no_proxy,
                    ) && let Some(health) = health
                        && health.matches_registry_instance(&existing)
                    {
                        activate_runtime_broker_profile(
                            &broker_client,
                            &existing,
                            current_profile,
                        )?;
                        return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
                    }
                }
                RuntimeBrokerVersionGuardOutcome::Replaced
                | RuntimeBrokerVersionGuardOutcome::DeferredActiveRequests => {}
            }
        }
    }

    if let Some((existing_broker_key, existing)) = find_compatible_runtime_broker_registry(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
    )? {
        activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
        return runtime_proxy_endpoint_from_registry(paths, &existing_broker_key, &existing);
    }

    let instance_token = runtime_random_token("broker");
    let admin_token = runtime_random_token("admin");
    spawn_runtime_broker_process(
        paths,
        RuntimeBrokerSpawnConfig {
            current_profile,
            upstream_base_url,
            include_code_review,
            upstream_no_proxy,
            broker_key: &broker_key,
            instance_token: &instance_token,
            admin_token: &admin_token,
            listen_addr: preferred_listen_addr.as_deref(),
        },
    )?;
    let registry =
        wait_for_runtime_broker_ready(&broker_client, paths, &broker_key, &instance_token)?;
    activate_runtime_broker_profile(&broker_client, &registry, current_profile)?;
    runtime_proxy_endpoint_from_registry(paths, &broker_key, &registry)
}
