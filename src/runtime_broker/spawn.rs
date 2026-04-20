use super::*;

pub(crate) fn runtime_broker_process_args(
    current_profile: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    broker_key: &str,
    instance_token: &str,
    admin_token: &str,
    listen_addr: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("__runtime-broker"),
        OsString::from("--current-profile"),
        OsString::from(current_profile),
        OsString::from("--upstream-base-url"),
        OsString::from(upstream_base_url),
    ];
    if include_code_review {
        args.push(OsString::from("--include-code-review"));
    }
    args.extend([
        OsString::from("--broker-key"),
        OsString::from(broker_key),
        OsString::from("--instance-token"),
        OsString::from(instance_token),
        OsString::from("--admin-token"),
        OsString::from(admin_token),
    ]);
    if let Some(listen_addr) = listen_addr {
        args.push(OsString::from("--listen-addr"));
        args.push(OsString::from(listen_addr));
    }
    args
}

pub(crate) fn wait_for_existing_runtime_broker_recovery_or_exit(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < Duration::from_millis(runtime_broker_ready_timeout_ms()) {
        let Some(existing) = load_runtime_broker_registry(paths, broker_key)? else {
            return Ok(None);
        };

        if replace_runtime_broker_if_version_mismatch(paths, broker_key, &existing) {
            return Ok(None);
        }

        if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(client, &existing)?
            && health.instance_token == existing.instance_token
        {
            return Ok(Some(existing));
        }

        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                broker_key,
                &existing.instance_token,
            );
            return Ok(None);
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
) -> Result<Option<(String, RuntimeBrokerRegistry)>> {
    for broker_key in runtime_broker_registry_keys(paths) {
        if broker_key == excluded_broker_key {
            continue;
        }

        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if registry.upstream_base_url != upstream_base_url
            || registry.include_code_review != include_code_review
        {
            continue;
        }
        if replace_runtime_broker_if_version_mismatch(paths, &broker_key, &registry) {
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
        if let Some(health) = probe_runtime_broker_health(client, &registry)?
            && health.instance_token == registry.instance_token
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
            && health.instance_token == expected_instance_token
        {
            return Ok(registry);
        }
        thread::sleep(poll_interval);
    }
    bail!("timed out waiting for runtime broker readiness");
}

pub(crate) struct RuntimeBrokerSpawnConfig<'a> {
    current_profile: &'a str,
    upstream_base_url: &'a str,
    include_code_review: bool,
    broker_key: &'a str,
    instance_token: &'a str,
    admin_token: &'a str,
    listen_addr: Option<&'a str>,
}

pub(crate) fn spawn_runtime_broker_process(
    paths: &AppPaths,
    config: RuntimeBrokerSpawnConfig<'_>,
) -> Result<()> {
    let current_exe = env::current_exe().context("failed to locate current prodex binary")?;
    Command::new(current_exe)
        .args(runtime_broker_process_args(
            config.current_profile,
            config.upstream_base_url,
            config.include_code_review,
            config.broker_key,
            config.instance_token,
            config.admin_token,
            config.listen_addr,
        ))
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
) -> Result<RuntimeProxyEndpoint> {
    let broker_key = runtime_broker_key(upstream_base_url, include_code_review);
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
    )? {
        activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
        return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
    }

    if let Some(existing) = load_runtime_broker_registry(paths, &broker_key)?
        && !replace_runtime_broker_if_version_mismatch(paths, &broker_key, &existing)
    {
        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_token_matches(
                paths,
                &broker_key,
                &existing.instance_token,
            );
        } else if existing.upstream_base_url == upstream_base_url
            && existing.include_code_review == include_code_review
            && let Some(health) = probe_runtime_broker_health(&broker_client, &existing)?
            && health.instance_token == existing.instance_token
        {
            activate_runtime_broker_profile(&broker_client, &existing, current_profile)?;
            return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
        }
    }

    if let Some((existing_broker_key, existing)) = find_compatible_runtime_broker_registry(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
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
