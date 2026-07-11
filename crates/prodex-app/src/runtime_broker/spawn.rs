use super::*;

#[cfg(test)]
pub(crate) fn runtime_broker_process_args() -> Vec<OsString> {
    prodex_runtime_broker::runtime_broker_process_args()
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct RuntimeBrokerSmartContextOptions {
    pub(crate) enabled: bool,
    pub(crate) model_context_window_tokens: Option<u64>,
}

impl RuntimeBrokerSmartContextOptions {
    #[cfg(test)]
    fn disabled() -> Self {
        Self {
            enabled: false,
            model_context_window_tokens: None,
        }
    }

    fn launch_config<'a>(
        self,
        upstream_base_url: &'a str,
        include_code_review: bool,
        upstream_no_proxy: bool,
    ) -> prodex_runtime_broker::RuntimeBrokerLaunchConfig<'a> {
        prodex_runtime_broker::RuntimeBrokerLaunchConfig {
            upstream_base_url,
            include_code_review,
            upstream_no_proxy,
            smart_context_enabled: self.enabled,
        }
    }

    fn supports_cross_broker_reuse(self) -> bool {
        self.model_context_window_tokens.is_none()
    }
}

#[derive(Debug, Clone, Copy)]
struct RuntimeBrokerWaitOptions {
    smart_context: RuntimeBrokerSmartContextOptions,
    ready_timeout: Duration,
}

#[cfg(test)]
pub(crate) fn wait_for_existing_runtime_broker_recovery_or_exit(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let ready_timeout =
        Duration::from_millis(RuntimeConfig::compatibility_current().broker_ready_timeout_ms);
    wait_for_existing_runtime_broker_recovery_or_exit_with_smart_context(
        client,
        paths,
        broker_key,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        RuntimeBrokerWaitOptions {
            smart_context: RuntimeBrokerSmartContextOptions::disabled(),
            ready_timeout,
        },
    )
}

fn wait_for_existing_runtime_broker_recovery_or_exit_with_smart_context(
    client: &Client,
    paths: &AppPaths,
    broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    wait: RuntimeBrokerWaitOptions,
) -> Result<Option<RuntimeBrokerRegistry>> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    let launch_config =
        wait.smart_context
            .launch_config(upstream_base_url, include_code_review, upstream_no_proxy);
    while started_at.elapsed() < wait.ready_timeout {
        let Some(existing) = load_runtime_broker_registry(paths, broker_key)? else {
            return Ok(None);
        };

        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_instance_matches(
                paths,
                broker_key,
                &existing.instance_id,
            );
            return Ok(None);
        }

        let health = probe_runtime_broker_health(client, paths, broker_key, &existing)?;
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

        if prodex_runtime_broker::runtime_broker_registry_reuse_decision(
            &existing,
            health.as_ref(),
            launch_config,
        ) == prodex_runtime_broker::RuntimeBrokerRegistryReuseDecision::Reuse
        {
            return Ok(Some(existing));
        }

        thread::sleep(poll_interval);
    }

    Ok(None)
}

#[cfg(test)]
pub(crate) fn find_compatible_runtime_broker_registry(
    client: &Client,
    paths: &AppPaths,
    excluded_broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
) -> Result<Option<(String, RuntimeBrokerRegistry)>> {
    find_compatible_runtime_broker_registry_with_smart_context(
        client,
        paths,
        excluded_broker_key,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        RuntimeBrokerSmartContextOptions::disabled(),
    )
}

pub(crate) fn find_compatible_runtime_broker_registry_with_smart_context(
    client: &Client,
    paths: &AppPaths,
    excluded_broker_key: &str,
    upstream_base_url: &str,
    include_code_review: bool,
    upstream_no_proxy: bool,
    smart_context: RuntimeBrokerSmartContextOptions,
) -> Result<Option<(String, RuntimeBrokerRegistry)>> {
    if !smart_context.supports_cross_broker_reuse() {
        return Ok(None);
    }

    let launch_config =
        smart_context.launch_config(upstream_base_url, include_code_review, upstream_no_proxy);
    for broker_key in runtime_broker_registry_keys(paths) {
        if broker_key == excluded_broker_key {
            continue;
        }

        let Some(registry) = load_runtime_broker_registry(paths, &broker_key)? else {
            continue;
        };
        if !launch_config.matches_registry(&registry) {
            continue;
        }
        if !runtime_process_pid_alive(registry.pid) {
            remove_runtime_broker_registry_if_instance_matches(
                paths,
                &broker_key,
                &registry.instance_id,
            );
            continue;
        }
        let health = probe_runtime_broker_health(client, paths, &broker_key, &registry)?;
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
        if prodex_runtime_broker::runtime_broker_registry_reuse_decision(
            &registry,
            health.as_ref(),
            launch_config,
        ) == prodex_runtime_broker::RuntimeBrokerRegistryReuseDecision::Reuse
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
    expected_instance_id: &str,
    ready_timeout: Duration,
) -> Result<RuntimeBrokerRegistry> {
    let started_at = Instant::now();
    let poll_interval = Duration::from_millis(RUNTIME_BROKER_POLL_INTERVAL_MS);
    while started_at.elapsed() < ready_timeout {
        if let Some(registry) = load_runtime_broker_registry(paths, broker_key)?
            && registry.instance_id == expected_instance_id
            && let Some(health) = probe_runtime_broker_health(client, paths, broker_key, &registry)?
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
    let command_plan =
        prodex_runtime_broker::runtime_broker_process_command_plan(current_exe, &paths.root);
    let mut child = Command::new(command_plan.executable)
        .args(command_plan.args)
        .envs(command_plan.environment)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to spawn runtime broker process")?;
    let write_result = child
        .stdin
        .take()
        .context("runtime broker bootstrap pipe is unavailable")
        .and_then(|stdin| {
            prodex_runtime_broker::write_runtime_broker_bootstrap(stdin, config)
                .context("failed to write runtime broker bootstrap")
        });
    if let Err(err) = write_result {
        let _ = child.kill();
        let _ = child.wait();
        return Err(err);
    }
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
    smart_context_enabled: bool,
    model_context_window_tokens: Option<u64>,
) -> Result<RuntimeProxyEndpoint> {
    let runtime_config = RuntimeConfig::from_env_policy_and_cli(paths)?;
    let ready_timeout = Duration::from_millis(runtime_config.broker_ready_timeout_ms);
    let broker_key = runtime_broker_key_with_smart_context(
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        smart_context_enabled,
        model_context_window_tokens,
    );
    let smart_context = RuntimeBrokerSmartContextOptions {
        enabled: smart_context_enabled,
        model_context_window_tokens,
    };
    let launch_config = prodex_runtime_broker::RuntimeBrokerLaunchConfig {
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        smart_context_enabled: smart_context.enabled,
    };
    let ensure_lock_path = runtime_broker_ensure_lock_path(paths, &broker_key);
    let _ensure_lock = acquire_json_file_lock(&ensure_lock_path)?;
    let preferred_listen_addr = preferred_runtime_broker_listen_addr(paths, &broker_key)?;
    let broker_client = runtime_broker_client_with_config(&runtime_config)?;

    if let Some(existing) = wait_for_existing_runtime_broker_recovery_or_exit_with_smart_context(
        &broker_client,
        paths,
        &broker_key,
        upstream_base_url,
        include_code_review,
        upstream_no_proxy,
        RuntimeBrokerWaitOptions {
            smart_context,
            ready_timeout,
        },
    )? {
        activate_runtime_broker_profile(
            &broker_client,
            paths,
            &broker_key,
            &existing,
            current_profile,
        )?;
        return runtime_proxy_endpoint_from_registry(paths, &broker_key, &existing);
    }

    if let Some(existing) = load_runtime_broker_registry(paths, &broker_key)? {
        if !runtime_process_pid_alive(existing.pid) {
            remove_runtime_broker_registry_if_instance_matches(
                paths,
                &broker_key,
                &existing.instance_id,
            );
        } else {
            let health =
                probe_runtime_broker_health(&broker_client, paths, &broker_key, &existing)?;
            match replace_runtime_broker_if_version_mismatch_with_health(
                paths,
                &broker_key,
                &existing,
                health.as_ref(),
            ) {
                RuntimeBrokerVersionGuardOutcome::Compatible => {
                    if prodex_runtime_broker::runtime_broker_registry_reuse_decision(
                        &existing,
                        health.as_ref(),
                        launch_config,
                    ) == prodex_runtime_broker::RuntimeBrokerRegistryReuseDecision::Reuse
                    {
                        activate_runtime_broker_profile(
                            &broker_client,
                            paths,
                            &broker_key,
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

    if let Some((existing_broker_key, existing)) =
        find_compatible_runtime_broker_registry_with_smart_context(
            &broker_client,
            paths,
            &broker_key,
            upstream_base_url,
            include_code_review,
            upstream_no_proxy,
            smart_context,
        )?
    {
        activate_runtime_broker_profile(
            &broker_client,
            paths,
            &existing_broker_key,
            &existing,
            current_profile,
        )?;
        return runtime_proxy_endpoint_from_registry(paths, &existing_broker_key, &existing);
    }

    let instance_id = runtime_random_token("broker")?;
    let admin_token =
        prodex_runtime_broker::RuntimeBrokerSecret::new(runtime_random_token("admin")?)
            .context("failed to create runtime broker admin capability")?;
    spawn_runtime_broker_process(
        paths,
        RuntimeBrokerSpawnConfig {
            current_profile,
            upstream_base_url,
            include_code_review,
            upstream_no_proxy,
            smart_context_enabled,
            model_context_window_tokens,
            broker_key: &broker_key,
            instance_id: &instance_id,
            admin_token: &admin_token,
            listen_addr: preferred_listen_addr.as_deref(),
        },
    )?;
    let registry = wait_for_runtime_broker_ready(
        &broker_client,
        paths,
        &broker_key,
        &instance_id,
        ready_timeout,
    )?;
    activate_runtime_broker_profile(
        &broker_client,
        paths,
        &broker_key,
        &registry,
        current_profile,
    )?;
    runtime_proxy_endpoint_from_registry(paths, &broker_key, &registry)
}
