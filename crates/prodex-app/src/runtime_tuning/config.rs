use super::*;

#[path = "config/gateway_helpers.rs"]
mod gateway_helpers;
use gateway_helpers::*;

struct RuntimeGatewayConfigInput<'a> {
    service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
    provider_override: Option<SuperExternalProvider>,
    base_url_override: Option<&'a str>,
}

impl<'a> RuntimeGatewayConfigInput<'a> {
    fn new(
        service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
        args: &'a GatewayArgs,
    ) -> Self {
        Self {
            service_mode,
            provider_override: args.provider,
            base_url_override: args.base_url.as_deref(),
        }
    }
}

impl RuntimeConfig {
    pub(crate) fn from_env_policy_and_cli(paths: &AppPaths) -> Result<Self, ConfigErrors> {
        let environment = RuntimeConfigEnvironment::read_process();
        Self::from_environment(paths, environment)
    }

    pub(crate) fn from_gateway_env_policy_and_cli(
        paths: &AppPaths,
        service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
        args: &GatewayArgs,
    ) -> Result<Self, ConfigErrors> {
        let data_plane = service_mode == prodex_runtime_policy::RuntimePolicyServiceMode::Gateway;
        let environment = RuntimeConfigEnvironment::read_gateway_process(data_plane);
        Self::from_environment_with_gateway(
            paths,
            environment,
            Some(RuntimeGatewayConfigInput::new(service_mode, args)),
        )
    }

    pub(super) fn from_environment(
        paths: &AppPaths,
        environment: RuntimeConfigEnvironment,
    ) -> Result<Self, ConfigErrors> {
        Self::from_environment_with_gateway(paths, environment, None)
    }

    #[cfg(test)]
    pub(super) fn from_gateway_environment(
        paths: &AppPaths,
        environment: RuntimeConfigEnvironment,
        service_mode: prodex_runtime_policy::RuntimePolicyServiceMode,
        args: &GatewayArgs,
    ) -> Result<Self, ConfigErrors> {
        Self::from_environment_with_gateway(
            paths,
            environment,
            Some(RuntimeGatewayConfigInput::new(service_mode, args)),
        )
    }

    fn from_environment_with_gateway(
        paths: &AppPaths,
        environment: RuntimeConfigEnvironment,
        gateway_input: Option<RuntimeGatewayConfigInput<'_>>,
    ) -> Result<Self, ConfigErrors> {
        let mut parser = RuntimeConfigParser::new(environment);
        let loaded_policy = match prodex_runtime_policy::load_runtime_policy_cached(&paths.root) {
            Ok(policy) => policy,
            Err(_) => {
                parser.errors.push(ConfigError {
                    key: "runtime.policy",
                    message: "could not be loaded".to_string(),
                });
                None
            }
        };
        let runtime_policy = loaded_policy.as_ref().map(|policy| policy.runtime.clone());
        let mut proxy_policy = loaded_policy
            .as_ref()
            .map(|policy| policy.runtime_proxy.clone())
            .unwrap_or_default();
        let preset_key = prodex_runtime_policy::PRODEX_RUNTIME_PROXY_PRESET_ENV;
        let preset = parser.environment.get(preset_key).and_then(|value| {
            let parsed = value
                .to_str()
                .and_then(prodex_runtime_policy::RuntimePolicyProxyPreset::parse);
            if parsed.is_none() {
                parser.compatibility_defaults.push(preset_key);
            }
            parsed
        });
        proxy_policy = proxy_policy.with_effective_preset(preset);
        let mut config = Self::parse(&mut parser, runtime_policy.as_ref(), &proxy_policy);
        config.governance_policy = loaded_policy
            .as_ref()
            .map(|policy| policy.governance.clone())
            .unwrap_or_default();
        config.governance = loaded_policy
            .as_ref()
            .map(|policy| crate::runtime_governance::runtime_governance_config(&policy.governance))
            .unwrap_or_else(prodex_config::GovernanceConfig::personal_compatible);
        config.governance_snapshot =
            crate::runtime_governance::build_runtime_governance_snapshot(&config.governance_policy)
                .ok();
        if config.governance_snapshot.is_none() {
            parser.errors.push(ConfigError {
                key: "runtime.policy.governance",
                message: "contains an invalid immutable governance snapshot".to_string(),
            });
        }
        if let Some(input) = gateway_input {
            config.gateway.launch = Self::parse_gateway_launch(
                &mut parser,
                loaded_policy.as_ref(),
                input,
                &config.gemini,
            );
        }
        if parser.errors.is_empty() {
            let config = RuntimeConfig {
                compatibility_defaults: parser.compatibility_defaults,
                ..config
            };
            RUNTIME_RESPONSES_QUOTA_CRITICAL_FLOOR_PERCENT.store(
                config.responses_quota_critical_floor_percent,
                Ordering::Relaxed,
            );
            Ok(config)
        } else {
            Err(ConfigErrors(parser.errors))
        }
    }

    fn parse_gateway_launch(
        parser: &mut RuntimeConfigParser,
        policy: Option<&prodex_runtime_policy::RuntimePolicyConfig>,
        input: RuntimeGatewayConfigInput<'_>,
        gemini: &RuntimeGeminiConfig,
    ) -> RuntimeGatewayLaunchEnvironment {
        if input.service_mode == prodex_runtime_policy::RuntimePolicyServiceMode::ControlPlane {
            return RuntimeGatewayLaunchEnvironment::ControlPlane;
        }
        let gateway = policy.map(|policy| &policy.gateway);
        let provider = input.provider_override.or_else(|| {
            gateway
                .and_then(|gateway| gateway.provider.as_deref())
                .and_then(runtime_gateway_policy_provider)
        });
        let upstream_base_url = runtime_gateway_upstream_base_url(
            parser,
            input.base_url_override,
            gateway.and_then(|gateway| gateway.base_url.as_deref()),
            provider,
        );
        let deepseek = (provider == Some(SuperExternalProvider::DeepSeek)).then(|| {
            let strict_environment = parser
                .environment
                .get("PRODEX_DEEPSEEK_STRICT_TOOLS")
                .cloned();
            let beta_environment = parser
                .environment
                .get("PRODEX_DEEPSEEK_BETA_BASE_URL")
                .cloned();
            let search_environment = parser
                .environment
                .get("PRODEX_DEEPSEEK_WEB_SEARCH_MODE")
                .cloned();
            let strict_tools = runtime_gateway_capture(
                parser,
                "PRODEX_DEEPSEEK_STRICT_TOOLS",
                crate::runtime_deepseek_gateway_strict_tools(
                    Path::new(""),
                    strict_environment.as_deref(),
                ),
                false,
            );
            let beta_base_url = runtime_gateway_capture(
                parser,
                "PRODEX_DEEPSEEK_BETA_BASE_URL",
                crate::runtime_deepseek_gateway_beta_base_url(
                    Path::new(""),
                    beta_environment.as_deref(),
                ),
                "https://api.deepseek.com/beta".to_string(),
            );
            let web_search_mode = runtime_gateway_capture(
                parser,
                "PRODEX_DEEPSEEK_WEB_SEARCH_MODE",
                crate::runtime_deepseek_gateway_web_search_mode(
                    Path::new(""),
                    search_environment.as_deref(),
                ),
                RuntimeDeepSeekWebSearchMode::default(),
            );
            RuntimeGatewayDeepSeekConfig {
                strict_tools,
                beta_base_url,
                web_search_mode,
            }
        });
        let gemini_model_resolution = (provider == Some(SuperExternalProvider::Gemini))
            .then(|| RuntimeGeminiModelResolution::from_runtime_config(gemini));
        let production = policy.is_some_and(|policy| policy.secrets.production);
        let present_secret_env = if production {
            BTreeSet::new()
        } else {
            parser.environment.present_gateway_secret_env()
        };
        RuntimeGatewayLaunchEnvironment::DataPlane {
            upstream_base_url,
            deepseek,
            gemini_model_resolution,
            present_secret_env,
        }
    }

    #[cfg(any(test, feature = "bench-support"))]
    pub(crate) fn compatibility_current() -> Self {
        let paths = AppPaths::discover()
            .unwrap_or_else(|error| panic!("failed to discover runtime configuration: {error}"));
        Self::from_env_policy_and_cli(&paths).unwrap_or_else(|errors| panic!("{errors}"))
    }

    fn parse(
        parser: &mut RuntimeConfigParser,
        runtime_policy: Option<&prodex_runtime_policy::RuntimePolicyRuntimeSettings>,
        policy: &prodex_runtime_policy::RuntimePolicyProxySettings,
    ) -> Self {
        let parallelism = thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(4);
        let worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROXY_WORKER_COUNT",
                policy.worker_count,
                runtime_proxy_worker_count_default(parallelism),
            )
            .clamp(1, 64);
        let long_lived_worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROXY_LONG_LIVED_WORKER_COUNT",
                policy.long_lived_worker_count,
                runtime_proxy_long_lived_worker_count_default(parallelism),
            )
            .clamp(1, 256);
        let async_worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROXY_ASYNC_WORKER_COUNT",
                policy.async_worker_count,
                runtime_proxy_async_worker_count_default(parallelism),
            )
            .clamp(2, 8);
        let probe_refresh_worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROBE_REFRESH_WORKER_COUNT",
                policy.probe_refresh_worker_count,
                runtime_probe_refresh_worker_count_default(parallelism),
            )
            .clamp(1, 8);
        let long_lived_queue_capacity = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_CAPACITY",
                policy.long_lived_queue_capacity,
                runtime_proxy_long_lived_queue_capacity_default(long_lived_worker_count),
            )
            .max(1);
        let active_request_limit = parser
            .positive_usize(
                "PRODEX_RUNTIME_PROXY_ACTIVE_REQUEST_LIMIT",
                policy.active_request_limit,
                runtime_proxy_active_request_limit_default(worker_count, long_lived_worker_count),
            )
            .max(1);
        let lane_limits = Self::parse_lane_limits(
            parser,
            policy,
            active_request_limit,
            worker_count,
            long_lived_worker_count,
        );
        let websocket = Self::parse_websocket_tuning(parser, policy, parallelism);
        let (precommit, pressure_precommit, continuation_precommit) = (
            runtime_proxy_precommit_budget(false, false),
            runtime_proxy_precommit_budget(false, true),
            runtime_proxy_precommit_budget(true, false),
        );
        let tuning = RuntimeTuningSnapshotInput {
            worker_count,
            long_lived_worker_count,
            async_worker_count,
            probe_refresh_worker_count,
            long_lived_queue_capacity,
            active_request_limit,
            lane_limits: RuntimeTuningLaneLimits {
                responses: lane_limits.responses,
                compact: lane_limits.compact,
                websocket: lane_limits.websocket,
                standard: lane_limits.standard,
            },
            precommit: RuntimeTuningPrecommitBudget {
                attempt_limit: precommit.0,
                budget: precommit.1,
            },
            pressure_precommit: RuntimeTuningPrecommitBudget {
                attempt_limit: pressure_precommit.0,
                budget: pressure_precommit.1,
            },
            continuation_precommit: RuntimeTuningPrecommitBudget {
                attempt_limit: continuation_precommit.0,
                budget: continuation_precommit.1,
            },
            admission_wait_budget_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS",
                policy.admission_wait_budget_ms,
                RUNTIME_PROXY_ADMISSION_WAIT_BUDGET_MS,
            ),
            pressure_admission_wait_budget_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS",
                policy.pressure_admission_wait_budget_ms,
                RUNTIME_PROXY_PRESSURE_ADMISSION_WAIT_BUDGET_MS,
            ),
            long_lived_queue_wait_budget_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
                policy.long_lived_queue_wait_budget_ms,
                RUNTIME_PROXY_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
            ),
            pressure_long_lived_queue_wait_budget_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS",
                policy.pressure_long_lived_queue_wait_budget_ms,
                RUNTIME_PROXY_PRESSURE_LONG_LIVED_QUEUE_WAIT_BUDGET_MS,
            ),
            http_connect_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS",
                policy.http_connect_timeout_ms,
                RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS,
            ),
            stream_idle_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS",
                policy.stream_idle_timeout_ms,
                RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS,
            ),
            sse_lookahead_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS",
                policy.sse_lookahead_timeout_ms,
                RUNTIME_PROXY_SSE_LOOKAHEAD_TIMEOUT_MS,
            ),
            websocket_connect_timeout_ms: websocket.connect_timeout_ms,
            websocket_happy_eyeballs_delay_ms: websocket.happy_eyeballs_delay_ms,
            websocket_precommit_progress_timeout_ms: websocket.precommit_progress_timeout_ms,
            websocket_connect_worker_count: websocket.connect_worker_count,
            websocket_connect_queue_capacity: websocket.connect_queue_capacity,
            websocket_connect_overflow_capacity: websocket.connect_overflow_capacity,
            websocket_dns_worker_count: websocket.dns_worker_count,
            websocket_dns_queue_capacity: websocket.dns_queue_capacity,
            websocket_dns_overflow_capacity: websocket.dns_overflow_capacity,
            websocket_previous_response_reuse_stale_ms: parser.compatibility_u64(
                "PRODEX_RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS",
                policy
                    .websocket_previous_response_reuse_stale_ms
                    .unwrap_or(RUNTIME_PROXY_WEBSOCKET_PREVIOUS_RESPONSE_REUSE_STALE_MS),
                false,
                true,
                u64::MAX,
            ),
            profile_inflight_soft_limit: parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_SOFT_LIMIT",
                policy.profile_inflight_soft_limit,
                RUNTIME_PROFILE_INFLIGHT_SOFT_LIMIT,
            ),
            profile_inflight_hard_limit: parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_PROFILE_INFLIGHT_HARD_LIMIT",
                policy.profile_inflight_hard_limit,
                RUNTIME_PROFILE_INFLIGHT_HARD_LIMIT,
            ),
        }
        .into_snapshot();
        Self::parse_remaining(parser, runtime_policy, policy, tuning)
    }

    fn parse_lane_limits(
        parser: &mut RuntimeConfigParser,
        policy: &prodex_runtime_policy::RuntimePolicyProxySettings,
        global_limit: usize,
        worker_count: usize,
        long_lived_worker_count: usize,
    ) -> RuntimeProxyLaneLimits {
        let overrides = RuntimeProxyLaneLimitOverrides {
            responses: nonzero(parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT",
                policy.responses_active_limit,
                1,
            )),
            compact: nonzero(parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT",
                policy.compact_active_limit,
                1,
            )),
            websocket: nonzero(parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT",
                policy.websocket_active_limit,
                1,
            )),
            standard: nonzero(parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT",
                policy.standard_active_limit,
                1,
            )),
        };
        let configured = runtime_proxy_lane_limits_from_overrides(
            global_limit,
            worker_count,
            long_lived_worker_count,
            RuntimeProxyLaneLimitOverrides {
                responses: if parser
                    .environment
                    .get("PRODEX_RUNTIME_PROXY_RESPONSES_ACTIVE_LIMIT")
                    .is_some()
                    || policy.responses_active_limit.is_some()
                {
                    overrides.responses
                } else {
                    None
                },
                compact: if parser
                    .environment
                    .get("PRODEX_RUNTIME_PROXY_COMPACT_ACTIVE_LIMIT")
                    .is_some()
                    || policy.compact_active_limit.is_some()
                {
                    overrides.compact
                } else {
                    None
                },
                websocket: if parser
                    .environment
                    .get("PRODEX_RUNTIME_PROXY_WEBSOCKET_ACTIVE_LIMIT")
                    .is_some()
                    || policy.websocket_active_limit.is_some()
                {
                    overrides.websocket
                } else {
                    None
                },
                standard: if parser
                    .environment
                    .get("PRODEX_RUNTIME_PROXY_STANDARD_ACTIVE_LIMIT")
                    .is_some()
                    || policy.standard_active_limit.is_some()
                {
                    overrides.standard
                } else {
                    None
                },
            },
        );
        RuntimeProxyLaneLimits {
            responses: configured.responses,
            compact: configured.compact,
            websocket: configured.websocket,
            standard: configured.standard,
        }
    }

    fn parse_websocket_tuning(
        parser: &mut RuntimeConfigParser,
        policy: &prodex_runtime_policy::RuntimePolicyProxySettings,
        parallelism: usize,
    ) -> ParsedWebsocketTuning {
        let connect_worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_WEBSOCKET_CONNECT_WORKER_COUNT",
                policy.websocket_connect_worker_count,
                runtime_websocket_tcp_connect_worker_count_default(parallelism),
            )
            .max(1);
        let connect_queue_capacity = parser
            .positive_usize(
                "PRODEX_RUNTIME_WEBSOCKET_CONNECT_QUEUE_CAPACITY",
                policy.websocket_connect_queue_capacity,
                runtime_websocket_tcp_connect_queue_capacity_default(connect_worker_count),
            )
            .max(connect_worker_count)
            .max(1);
        let dns_worker_count = parser
            .positive_usize(
                "PRODEX_RUNTIME_WEBSOCKET_DNS_WORKER_COUNT",
                policy.websocket_dns_worker_count,
                runtime_websocket_dns_resolve_worker_count_default(parallelism),
            )
            .max(1);
        let dns_queue_capacity = parser
            .positive_usize(
                "PRODEX_RUNTIME_WEBSOCKET_DNS_QUEUE_CAPACITY",
                policy.websocket_dns_queue_capacity,
                runtime_websocket_dns_resolve_queue_capacity_default(dns_worker_count),
            )
            .max(dns_worker_count)
            .max(1);
        ParsedWebsocketTuning {
            connect_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS",
                policy.websocket_connect_timeout_ms,
                RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS,
            ),
            happy_eyeballs_delay_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS",
                policy.websocket_happy_eyeballs_delay_ms,
                RUNTIME_PROXY_WEBSOCKET_HAPPY_EYEBALLS_DELAY_MS,
            ),
            precommit_progress_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS",
                policy.websocket_precommit_progress_timeout_ms,
                RUNTIME_PROXY_WEBSOCKET_PRECOMMIT_PROGRESS_TIMEOUT_MS,
            ),
            connect_worker_count,
            connect_queue_capacity,
            connect_overflow_capacity: parser.usize_allow_zero(
                "PRODEX_RUNTIME_WEBSOCKET_CONNECT_OVERFLOW_CAPACITY",
                policy.websocket_connect_overflow_capacity,
                runtime_websocket_tcp_connect_overflow_capacity_default(
                    connect_worker_count,
                    connect_queue_capacity,
                ),
            ),
            dns_worker_count,
            dns_queue_capacity,
            dns_overflow_capacity: parser.usize_allow_zero(
                "PRODEX_RUNTIME_WEBSOCKET_DNS_OVERFLOW_CAPACITY",
                policy.websocket_dns_overflow_capacity,
                runtime_websocket_dns_resolve_overflow_capacity_default(
                    dns_worker_count,
                    dns_queue_capacity,
                ),
            ),
        }
    }

    fn parse_remaining(
        parser: &mut RuntimeConfigParser,
        runtime_policy: Option<&prodex_runtime_policy::RuntimePolicyRuntimeSettings>,
        policy: &prodex_runtime_policy::RuntimePolicyProxySettings,
        tuning: RuntimeTuningSnapshot,
    ) -> Self {
        let log_format = parser
            .environment
            .get("PRODEX_RUNTIME_LOG_FORMAT")
            .and_then(|value| value.to_str())
            .and_then(RuntimeLogFormat::parse)
            .or_else(|| runtime_policy.and_then(|runtime| runtime.log_format))
            .unwrap_or(RuntimeLogFormat::Text);
        if parser
            .environment
            .get("PRODEX_RUNTIME_LOG_FORMAT")
            .is_some()
            && parser
                .environment
                .get("PRODEX_RUNTIME_LOG_FORMAT")
                .and_then(|value| value.to_str())
                .and_then(RuntimeLogFormat::parse)
                .is_none()
        {
            parser
                .compatibility_defaults
                .push("PRODEX_RUNTIME_LOG_FORMAT");
        }
        let log_dir = parser
            .environment
            .get("PRODEX_RUNTIME_LOG_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| runtime_policy.and_then(|runtime| runtime.log_dir.clone()))
            .unwrap_or_else(env::temp_dir);
        let https_proxy = parser.compatibility_proxy(&[
            "HTTPS_PROXY",
            "https_proxy",
            "ALL_PROXY",
            "all_proxy",
            "PROXY",
            "proxy",
        ]);
        let http_proxy = parser.compatibility_proxy(&[
            "HTTP_PROXY",
            "http_proxy",
            "ALL_PROXY",
            "all_proxy",
            "PROXY",
            "proxy",
        ]);
        let no_proxy = ["NO_PROXY", "no_proxy"]
            .into_iter()
            .filter_map(|key| parser.environment.get(key))
            .map(|value| value.to_string_lossy().into_owned())
            .collect();
        let gateway = RuntimeGatewayConfig {
            replica_count: parser.positive_u16("PRODEX_GATEWAY_REPLICA_COUNT", 1),
            require_multi_replica_accounting_checks: parser
                .strict_bool("PRODEX_REQUIRE_MULTI_REPLICA_ACCOUNTING_CHECKS", false),
            launch: RuntimeGatewayLaunchEnvironment::default(),
        };
        let gemini = Self::parse_gemini(parser);
        Self {
            tuning,
            compact_request_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS",
                policy.compact_request_timeout_ms,
                RUNTIME_PROXY_COMPACT_REQUEST_TIMEOUT_MS,
            ),
            prefetch_backpressure_retry_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS",
                policy.prefetch_backpressure_retry_ms,
                RUNTIME_PROXY_PREFETCH_BACKPRESSURE_RETRY_MS,
            ),
            prefetch_backpressure_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS",
                policy.prefetch_backpressure_timeout_ms,
                RUNTIME_PROXY_PREFETCH_BACKPRESSURE_TIMEOUT_MS,
            ),
            prefetch_max_buffered_bytes: parser.positive_usize(
                "PRODEX_RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES",
                policy.prefetch_max_buffered_bytes,
                RUNTIME_PROXY_PREFETCH_MAX_BUFFERED_BYTES,
            ),
            sync_probe_pressure_pause_ms: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS",
                policy.sync_probe_pressure_pause_ms,
                RUNTIME_PROXY_SYNC_PROBE_PRESSURE_PAUSE_MS,
            ),
            responses_quota_critical_floor_percent: parser
                .positive_i64(
                    "PRODEX_RUNTIME_PROXY_RESPONSES_CRITICAL_FLOOR_PERCENT",
                    policy.responses_critical_floor_percent,
                    2,
                )
                .clamp(1, 10),
            startup_sync_probe_warm_limit: parser
                .positive_usize(
                    "PRODEX_RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT",
                    policy.startup_sync_probe_warm_limit,
                    RUNTIME_STARTUP_SYNC_PROBE_WARM_LIMIT,
                )
                .min(RUNTIME_STARTUP_PROBE_WARM_LIMIT),
            broker_ready_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS",
                policy.broker_ready_timeout_ms,
                RUNTIME_BROKER_READY_TIMEOUT_MS,
            ),
            broker_health_connect_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS",
                policy.broker_health_connect_timeout_ms,
                RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS,
            ),
            broker_health_read_timeout_ms: parser.positive_u64(
                "PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS",
                policy.broker_health_read_timeout_ms,
                RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS,
            ),
            max_request_body_bytes: parser.positive_u64(
                "PRODEX_RUNTIME_PROXY_MAX_REQUEST_BODY_BYTES",
                None,
                RUNTIME_PROXY_DEFAULT_MAX_REQUEST_BODY_BYTES,
            ),
            debug_anthropic_compat: parser
                .environment
                .get("PRODEX_DEBUG_ANTHROPIC_COMPAT")
                .is_some(),
            smart_context_shadow: parser.compatibility_flag("PRODEX_SMART_CONTEXT_SHADOW"),
            smart_context_canary_percent: parser.compatibility_u64(
                "PRODEX_SMART_CONTEXT_CANARY_PERCENT",
                100,
                true,
                true,
                100,
            ) as u8,
            fault_upstream_connect_error_once: parser.usize_allow_zero(
                "PRODEX_RUNTIME_FAULT_UPSTREAM_CONNECT_ERROR_ONCE",
                None,
                0,
            ),
            fault_stream_read_error_once: parser.usize_allow_zero(
                "PRODEX_RUNTIME_FAULT_STREAM_READ_ERROR_ONCE",
                None,
                0,
            ),
            fault_smart_context_panic_once: parser.usize_allow_zero(
                "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_PANIC_ONCE",
                None,
                0,
            ),
            fault_smart_context_unwind_once: parser.usize_allow_zero(
                "PRODEX_RUNTIME_FAULT_SMART_CONTEXT_UNWIND_ONCE",
                None,
                0,
            ),
            log_dir,
            log_format,
            websocket_environment: RuntimeWebsocketEnvironment {
                https_proxy,
                http_proxy,
                no_proxy,
            },
            oidc: RuntimeOidcTimingConfig {
                prefetch_timeout: Duration::from_millis(parser.bounded_u64(
                    "PRODEX_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS",
                    DEFAULT_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS,
                    false,
                    MAX_RUNTIME_GATEWAY_OIDC_PREFETCH_TIMEOUT_MS,
                )),
                http_cache_ttl: Duration::from_secs(parser.bounded_u64(
                    "PRODEX_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS",
                    DEFAULT_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS,
                    true,
                    MAX_RUNTIME_GATEWAY_OIDC_HTTP_CACHE_TTL_SECONDS,
                )),
                refresh_failure_backoff: Duration::from_millis(parser.bounded_u64(
                    "PRODEX_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS",
                    DEFAULT_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS,
                    false,
                    MAX_RUNTIME_GATEWAY_OIDC_REFRESH_FAILURE_BACKOFF_MS,
                )),
                last_known_good_window: Duration::from_secs(parser.bounded_u64(
                    "PRODEX_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS",
                    DEFAULT_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS,
                    true,
                    MAX_RUNTIME_GATEWAY_OIDC_LAST_KNOWN_GOOD_SECONDS,
                )),
            },
            gateway,
            governance: prodex_config::GovernanceConfig::personal_compatible(),
            governance_policy: prodex_runtime_policy::RuntimePolicyGovernanceSettings::default(),
            governance_snapshot: None,
            gemini,
            compatibility_defaults: Vec::new(),
        }
    }

    fn parse_gemini(parser: &mut RuntimeConfigParser) -> RuntimeGeminiConfig {
        let home_dir = parser.environment.nonempty_path("HOME");
        let config_dir = parser
            .environment
            .nonempty_path("GEMINI_CLI_HOME")
            .map(|path| path.join(".gemini"))
            .or_else(|| home_dir.as_ref().map(|home| home.join(".gemini")));
        let system_settings_path = parser.environment.path("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
        let system_defaults_path = parser.environment.path("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
        let split_paths = |key| {
            parser
                .environment
                .get(key)
                .map(env::split_paths)
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        };
        let extension_dirs = split_paths("PRODEX_GEMINI_EXTENSION_DIRS");
        let import_paths = [
            "PRODEX_GEMINI_SESSION_FILE",
            "PRODEX_GEMINI_CHECKPOINT_FILE",
            "PRODEX_GEMINI_IMPORT_FILE",
        ]
        .into_iter()
        .flat_map(split_paths)
        .collect();
        let extension_memory_paths = split_paths("PRODEX_GEMINI_EXTENSION_MEMORY");
        let export_checkpoint_path = [
            "PRODEX_GEMINI_EXPORT_FILE",
            "PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE",
        ]
        .into_iter()
        .filter_map(|key| parser.environment.get(key))
        .find(|path| !path.is_empty())
        .map(PathBuf::from);
        let tool_output_dir = parser
            .environment
            .get("PRODEX_GEMINI_TOOL_OUTPUT_DIR")
            .filter(|path| !path.is_empty())
            .map(PathBuf::from);
        let extension_selection =
            match parser
                .compatibility_text("PRODEX_GEMINI_EXTENSIONS")
                .map(|value| {
                    value
                        .split([',', ';', ' ', '\n', '\t'])
                        .filter_map(|item| {
                            let item = item.trim().to_ascii_lowercase();
                            (!item.is_empty()).then_some(item)
                        })
                        .collect::<BTreeSet<_>>()
                }) {
                None => RuntimeGeminiExtensionSelection::All,
                Some(names) if names.is_empty() => RuntimeGeminiExtensionSelection::All,
                Some(names) if names.len() == 1 && names.contains("none") => {
                    RuntimeGeminiExtensionSelection::None
                }
                Some(names) => RuntimeGeminiExtensionSelection::Names(names),
            };
        let memory_files_disabled =
            parser.compatibility_optional_bool("PRODEX_GEMINI_DISABLE_MEMORY") == Some(true)
                || parser.compatibility_optional_bool("PRODEX_GEMINI_DISABLE_CONTEXT_FILES")
                    == Some(true);
        let memory_files_default = parser
            .compatibility_optional_bool("PRODEX_GEMINI_LOAD_MEMORY")
            .or_else(|| parser.compatibility_optional_bool("PRODEX_GEMINI_MEMORY"))
            .unwrap_or(true);
        let live_url = parser
            .compatibility_text("PRODEX_GEMINI_LIVE_URL")
            .filter(|value| !value.trim().is_empty());
        let live_model = parser.compatibility_text("PRODEX_GEMINI_LIVE_MODEL");
        let sticky_fresh_oauth = parser
            .compatibility_text("PRODEX_GEMINI_STICKY_FRESH_OAUTH")
            .is_none_or(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "0" | "false" | "off" | "no"
                )
            });
        RuntimeGeminiConfig {
            home_dir,
            config_dir,
            system_settings_path,
            system_defaults_path,
            extension_dirs,
            extension_selection,
            export_checkpoint_path,
            import_paths,
            tool_output_mask_threshold: parser.compatibility_u64(
                "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD",
                RuntimeGeminiConfig::DEFAULT_TOOL_OUTPUT_MASK_THRESHOLD as u64,
                false,
                true,
                usize::MAX as u64,
            ) as usize,
            tool_output_dir,
            memory_files_disabled,
            memory_files_default,
            extension_memory_paths,
            live_url,
            live_model,
            sticky_fresh_oauth,
        }
    }
}
