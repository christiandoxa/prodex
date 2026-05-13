macro_rules! runtime_doctor_marker_registry {
    ($macro:ident) => {
        $macro! {
            ChainRetriedOwner => "chain_retried_owner",
            ChainDeadUpstreamConfirmed => "chain_dead_upstream_confirmed",
            StaleContinuation => "stale_continuation",
            RuntimeProxyQueueOverloaded => "runtime_proxy_queue_overloaded",
            RuntimeProxyActiveLimitReached => "runtime_proxy_active_limit_reached",
            RuntimeProxyLaneLimitReached => "runtime_proxy_lane_limit_reached",
            RuntimeProxyOverloadBackoff => "runtime_proxy_overload_backoff",
            RuntimeProxyAdmissionWaitStarted => "runtime_proxy_admission_wait_started",
            RuntimeProxyAdmissionWaitExhausted => "runtime_proxy_admission_wait_exhausted",
            RuntimeProxyAdmissionRecovered => "runtime_proxy_admission_recovered",
            RuntimeProxyQueueWaitStarted => "runtime_proxy_queue_wait_started",
            RuntimeProxyQueueWaitExhausted => "runtime_proxy_queue_wait_exhausted",
            RuntimeProxyQueueRecovered => "runtime_proxy_queue_recovered",
            ProfileInflightSaturated => "profile_inflight_saturated",
            ProfileInflight => "profile_inflight",
            UpstreamConnectTimeout => "upstream_connect_timeout",
            UpstreamConnectDnsError => "upstream_connect_dns_error",
            UpstreamTlsHandshakeError => "upstream_tls_handshake_error",
            UpstreamConnectError => "upstream_connect_error",
            UpstreamConnectHttp => "upstream_connect_http",
            UpstreamCloseBeforeCompleted => "upstream_close_before_completed",
            UpstreamConnectionClosed => "upstream_connection_closed",
            UpstreamOverloadPassthrough => "upstream_overload_passthrough",
            UpstreamOverloaded => "upstream_overloaded",
            UpstreamReadError => "upstream_read_error",
            UpstreamSendError => "upstream_send_error",
            UpstreamStreamError => "upstream_stream_error",
            PrecommitBudgetExhausted => "precommit_budget_exhausted",
            ProfileRetryBackoff => "profile_retry_backoff",
            ProfileTransportBackoff => "profile_transport_backoff",
            ProfileTransportFailure => "profile_transport_failure",
            ProfileCircuitOpen => "profile_circuit_open",
            ProfileCircuitHalfOpenProbe => "profile_circuit_half_open_probe",
            ProfileHealth => "profile_health",
            ProfileLatency => "profile_latency",
            ProfileBadPairing => "profile_bad_pairing",
            ProfileQuotaQuarantine => "profile_quota_quarantine",
            ProfileAuthBackoff => "profile_auth_backoff",
            ProfileAuthBackoffCleared => "profile_auth_backoff_cleared",
            ProfileAuthProactiveSync => "profile_auth_proactive_sync",
            ProfileAuthProactiveSyncFailed => "profile_auth_proactive_sync_failed",
            PreviousResponseNotFound => "previous_response_not_found",
            PreviousResponseNegativeCache => "previous_response_negative_cache",
            PreviousResponseFreshFallback => "previous_response_fresh_fallback",
            PreviousResponseFreshFallbackBlocked => "previous_response_fresh_fallback_blocked",
            PreviousResponseBindingCleared => "previous_response_binding_cleared",
            PreviousResponseOwner => "previous_response_owner",
            PreviousResponseReleaseAffinity => "previous_response_release_affinity",
            PreviousResponseReleaseDeferred => "previous_response_release_deferred",
            PreviousResponseTurnStateRehydrated => "previous_response_turn_state_rehydrated",
            CompactCommittedOwner => "compact_committed_owner",
            CompactFollowupOwner => "compact_followup_owner",
            CompactFreshFallbackBlocked => "compact_fresh_fallback_blocked",
            CompactPressureShed => "compact_pressure_shed",
            CompactLineageReleased => "compact_lineage_released",
            CompactCommitted => "compact_committed",
            CompactPrecommitBudgetExhausted => "compact_precommit_budget_exhausted",
            CompactCandidateExhausted => "compact_candidate_exhausted",
            CompactRetryableFailure => "compact_retryable_failure",
            CompactOverloadConservativeRetry => "compact_overload_conservative_retry",
            CompactQuotaUnclassified => "compact_quota_unclassified",
            CompactPreSendAllowQuotaExhausted => "compact_pre_send_allow_quota_exhausted",
            CompactFinalFailure => "compact_final_failure",
            CompactExitCommitted => "compact_exit_committed",
            CompactExitCommittedOwner => "compact_exit_committed_owner",
            CompactExitFollowupOwner => "compact_exit_followup_owner",
            CompactExitFreshFallbackBlocked => "compact_exit_fresh_fallback_blocked",
            CompactExitPressureShed => "compact_exit_pressure_shed",
            CompactExitLineageReleased => "compact_exit_lineage_released",
            CompactExitPrecommitBudgetExhausted => "compact_exit_precommit_budget_exhausted",
            CompactExitCandidateExhausted => "compact_exit_candidate_exhausted",
            CompactExitRetryableFailure => "compact_exit_retryable_failure",
            CompactExitOverloadConservativeRetry => "compact_exit_overload_conservative_retry",
            CompactExitQuotaUnclassified => "compact_exit_quota_unclassified",
            SelectionKeepAffinity => "selection_keep_affinity",
            SelectionKeepCurrent => "selection_keep_current",
            SelectionPick => "selection_pick",
            SelectionSkipCurrent => "selection_skip_current",
            SelectionSkipAffinity => "selection_skip_affinity",
            LocalSelectionBlocked => "local_selection_blocked",
            ResponsesPreSendSkip => "responses_pre_send_skip",
            WebsocketPreSendSkip => "websocket_pre_send_skip",
            QuotaReleaseProfileAffinity => "quota_release_profile_affinity",
            QuotaReleaseAffinity => "quota_release_affinity",
            QuotaBlocked => "quota_blocked",
            QuotaCriticalFloorBeforeSend => "quota_critical_floor_before_send",
            UpstreamUsageLimitPassthrough => "upstream_usage_limit_passthrough",
            CompatRequestSurface => "compat_request_surface",
            CompatWarning => "compat_warning",
            SmartContextAutopilot => "smart_context_autopilot",
            RuntimeProxySyncProbePressurePause => "runtime_proxy_sync_probe_pressure_pause",
            WebsocketReuseSkipQuotaExhausted => "websocket_reuse_skip_quota_exhausted",
            WebsocketReuseWatchdog => "websocket_reuse_watchdog",
            WebsocketReuseWatchdogTimeout => "websocket_reuse_watchdog_timeout",
            WebsocketReuseLockedAffinityOwnerFreshRetry => "websocket_reuse_locked_affinity_owner_fresh_retry",
            WebsocketReuseNonreplayableFreshRetry => "websocket_reuse_nonreplayable_fresh_retry",
            WebsocketReuseOwnerFreshRetry => "websocket_reuse_owner_fresh_retry",
            WebsocketReusePreviousResponseBlocked => "websocket_reuse_previous_response_blocked",
            WebsocketReuseStalePreviousResponseBlocked => "websocket_reuse_stale_previous_response_blocked",
            WebsocketPrecommitFrameTimeout => "websocket_precommit_frame_timeout",
            WebsocketPrecommitHoldTimeout => "websocket_precommit_hold_timeout",
            WebsocketDnsResolveTimeout => "websocket_dns_resolve_timeout",
            WebsocketDnsOverflowEnqueue => "websocket_dns_overflow_enqueue",
            WebsocketDnsOverflowDispatch => "websocket_dns_overflow_dispatch",
            WebsocketDnsOverflowReject => "websocket_dns_overflow_reject",
            WebsocketConnectLocalPressure => "websocket_connect_local_pressure",
            WebsocketConnectOverflowEnqueue => "websocket_connect_overflow_enqueue",
            WebsocketConnectOverflowDispatch => "websocket_connect_overflow_dispatch",
            WebsocketConnectOverflowReject => "websocket_connect_overflow_reject",
            WebsocketConnectOverflowRejected => "websocket_connect_overflow_rejected",
            WebsocketProxyConnectStart => "websocket_proxy_connect_start",
            WebsocketProxyTunnelOk => "websocket_proxy_tunnel_ok",
            WebsocketProxyTunnelFailure => "websocket_proxy_tunnel_failure",
            ProfileAuthRecovered => "profile_auth_recovered",
            ProfileAuthRecoveryFailed => "profile_auth_recovery_failed",
            StreamReadError => "stream_read_error",
            TokenUsage => "token_usage",
            LocalWriterError => "local_writer_error",
            FirstUpstreamChunk => "first_upstream_chunk",
            FirstLocalChunk => "first_local_chunk",
            StateSaveOk => "state_save_ok",
            StateSaveSkipped => "state_save_skipped",
            StateSaveError => "state_save_error",
            StateSaveQueued => "state_save_queued",
            StateSaveQueueBackpressure => "state_save_queue_backpressure",
            ContinuationJournalSaveOk => "continuation_journal_save_ok",
            ContinuationJournalSaveError => "continuation_journal_save_error",
            ContinuationJournalSaveQueued => "continuation_journal_save_queued",
            ContinuationJournalQueueBackpressure => "continuation_journal_queue_backpressure",
            RuntimeProxyRestoreCounts => "runtime_proxy_restore_counts",
            RuntimeProxyStartupAudit => "runtime_proxy_startup_audit",
            RuntimeProxyUpstreamProxyMode => "runtime_proxy_upstream_proxy_mode",
            ProfileProbeRefreshQueued => "profile_probe_refresh_queued",
            ProfileProbeRefreshStart => "profile_probe_refresh_start",
            ProfileProbeRefreshOk => "profile_probe_refresh_ok",
            ProfileProbeRefreshError => "profile_probe_refresh_error",
            ProfileProbeRefreshBackpressure => "profile_probe_refresh_backpressure",
            ProfileProbeRefreshPanic => "profile_probe_refresh_panic",
            SelectionSkipSyncProbe => "selection_skip_sync_probe",
            QuotaBlockedAffinityReleased => "quota_blocked_affinity_released",
        }
    };
}

macro_rules! define_runtime_doctor_marker_enum {
    ($($variant:ident => $name:literal,)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum RuntimeDoctorMarker {
            $($variant,)+
        }

        impl RuntimeDoctorMarker {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub fn from_name(value: &str) -> Option<Self> {
                match value {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl AsRef<str> for RuntimeDoctorMarker {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        pub const RUNTIME_DOCTOR_MARKERS: &[&str] = &[$($name,)+];
    };
}

runtime_doctor_marker_registry!(define_runtime_doctor_marker_enum);

macro_rules! runtime_doctor_facet_registry {
    ($macro:ident) => {
        $macro! {
            Lane => "lane",
            Route => "route",
            Profile => "profile",
            Reason => "reason",
            Transport => "transport",
            Family => "family",
            Client => "client",
            ToolSurface => "tool_surface",
            Continuation => "continuation",
            Origin => "origin",
            Warning => "warning",
            QuotaSource => "quota_source",
            QuotaBand => "quota_band",
            FiveHourStatus => "five_hour_status",
            WeeklyStatus => "weekly_status",
            Affinity => "affinity",
            Context => "context",
            Event => "event",
            Stage => "stage",
            State => "state",
            Source => "source",
            RequestShape => "request_shape",
            Exit => "exit",
            Mode => "mode",
            Tier => "tier",
            Decision => "decision",
            Reasons => "reasons",
            TokenUsageSource => "token_usage_source",
            SelfCheck => "self_check",
            BudgetMode => "budget_mode",
            PolicyReasons => "policy_reasons",
        }
    };
}

macro_rules! define_runtime_doctor_facet_enum {
    ($($variant:ident => $name:literal,)+) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum RuntimeDoctorLogFacet {
            $($variant,)+
        }

        impl RuntimeDoctorLogFacet {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)+
                }
            }

            pub fn from_name(value: &str) -> Option<Self> {
                match value {
                    $($name => Some(Self::$variant),)+
                    _ => None,
                }
            }
        }

        impl AsRef<str> for RuntimeDoctorLogFacet {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        pub const RUNTIME_DOCTOR_FACETS: &[&str] = &[$($name,)+];
    };
}

runtime_doctor_facet_registry!(define_runtime_doctor_facet_enum);

pub struct RuntimeDoctorMarkerDescriptor {
    pub marker: RuntimeDoctorMarker,
    pub name: &'static str,
}

impl RuntimeDoctorMarkerDescriptor {
    pub const fn new(marker: RuntimeDoctorMarker) -> Self {
        Self {
            marker,
            name: marker.as_str(),
        }
    }
}

pub fn runtime_doctor_marker_descriptor(value: &str) -> Option<RuntimeDoctorMarkerDescriptor> {
    RuntimeDoctorMarker::from_name(value).map(RuntimeDoctorMarkerDescriptor::new)
}

pub const RUNTIME_DOCTOR_COUNT_FIELD_ROWS: &[(&str, &str)] = &[
    ("Queue overload", "runtime_proxy_queue_overloaded"),
    ("Active limit", "runtime_proxy_active_limit_reached"),
    ("Lane limit", "runtime_proxy_lane_limit_reached"),
    ("In-flight saturated", "profile_inflight_saturated"),
    ("Overload backoff", "runtime_proxy_overload_backoff"),
    (
        "Admission wait exhausted",
        "runtime_proxy_admission_wait_exhausted",
    ),
    ("Queue wait exhausted", "runtime_proxy_queue_wait_exhausted"),
    ("Pre-commit budget", "precommit_budget_exhausted"),
    ("Responses pre-send skips", "responses_pre_send_skip"),
    ("Websocket pre-send skips", "websocket_pre_send_skip"),
    (
        "Quota critical floor pre-send",
        "quota_critical_floor_before_send",
    ),
    (
        "Upstream usage-limit passthrough",
        "upstream_usage_limit_passthrough",
    ),
    ("Retry backoff", "profile_retry_backoff"),
    ("Transport backoff", "profile_transport_backoff"),
    ("Route circuits", "profile_circuit_open"),
    ("Health penalties", "profile_health"),
    ("Latency penalties", "profile_latency"),
    ("Bad pairing", "profile_bad_pairing"),
    ("Chain owner retries", "chain_retried_owner"),
    ("Chain dead upstream", "chain_dead_upstream_confirmed"),
    ("Stale continuations", "stale_continuation"),
    ("Prev not found", "previous_response_not_found"),
    ("Prev negative cache", "previous_response_negative_cache"),
    ("Legacy prev recovery", "previous_response_fresh_fallback"),
    (
        "Prev fail-closed",
        "previous_response_fresh_fallback_blocked",
    ),
    ("Compact guard", "compact_fresh_fallback_blocked"),
    ("Compact shed", "compact_pressure_shed"),
    ("Compact committed", "compact_committed"),
    ("Compact budget", "compact_precommit_budget_exhausted"),
    ("Compact exhausted", "compact_candidate_exhausted"),
    ("Compact retry", "compact_retryable_failure"),
    ("Compact owner retry", "compact_overload_conservative_retry"),
    ("Compact quota misc", "compact_quota_unclassified"),
    ("Compact final", "compact_final_failure"),
    ("Selection picks", "selection_pick"),
    ("Selection skips", "selection_skip_current"),
    ("Sync-probe skips", "selection_skip_sync_probe"),
    ("WS reuse watchdog", "websocket_reuse_watchdog"),
    (
        "WS first-frame timeouts",
        "websocket_precommit_frame_timeout",
    ),
    (
        "WS precommit hold timeouts",
        "websocket_precommit_hold_timeout",
    ),
    ("WS DNS timeouts", "websocket_dns_resolve_timeout"),
    ("WS DNS overflow enqueue", "websocket_dns_overflow_enqueue"),
    (
        "WS DNS overflow dispatch",
        "websocket_dns_overflow_dispatch",
    ),
    ("WS DNS overflow reject", "websocket_dns_overflow_reject"),
    (
        "WS connect local pressure",
        "websocket_connect_local_pressure",
    ),
    (
        "WS connect overflow enqueue",
        "websocket_connect_overflow_enqueue",
    ),
    (
        "WS connect overflow dispatch",
        "websocket_connect_overflow_dispatch",
    ),
    (
        "WS connect overflow reject",
        "websocket_connect_overflow_reject",
    ),
    (
        "WS connect overflow rejected",
        "websocket_connect_overflow_rejected",
    ),
    ("WS proxy tunnel ok", "websocket_proxy_tunnel_ok"),
    ("WS proxy tunnel failure", "websocket_proxy_tunnel_failure"),
    ("Auth recovered", "profile_auth_recovered"),
    ("Auth recovery failed", "profile_auth_recovery_failed"),
    ("Stream read errors", "stream_read_error"),
    ("Writer errors", "local_writer_error"),
    ("State save errors", "state_save_error"),
    ("State save pressure", "state_save_queue_backpressure"),
    ("Cont journal err", "continuation_journal_save_error"),
    (
        "Cont journal pressure",
        "continuation_journal_queue_backpressure",
    ),
    ("State save ok", "state_save_ok"),
    ("Cont journal ok", "continuation_journal_save_ok"),
    ("State save skipped", "state_save_skipped"),
    ("Startup audit", "runtime_proxy_startup_audit"),
    ("Admission recovered", "runtime_proxy_admission_recovered"),
    ("Queue recovered", "runtime_proxy_queue_recovered"),
    ("Probe refresh", "profile_probe_refresh_start"),
    ("Probe refresh errors", "profile_probe_refresh_error"),
    (
        "Probe refresh pressure",
        "profile_probe_refresh_backpressure",
    ),
    ("Compat samples", "compat_request_surface"),
    ("Smart context", "smart_context_autopilot"),
];

pub const RUNTIME_DOCTOR_SELECTION_PRESSURE_MARKERS: &[&str] = &[
    "selection_keep_affinity",
    "selection_keep_current",
    "selection_pick",
    "selection_skip_current",
    "selection_skip_affinity",
    "selection_skip_sync_probe",
    "local_selection_blocked",
    "precommit_budget_exhausted",
    "compact_precommit_budget_exhausted",
    "compact_candidate_exhausted",
];

pub const RUNTIME_DOCTOR_TRANSPORT_PRESSURE_MARKERS: &[&str] = &[
    "stream_read_error",
    "upstream_connect_timeout",
    "upstream_connect_dns_error",
    "upstream_tls_handshake_error",
    "upstream_connect_error",
    "upstream_connect_http",
    "upstream_close_before_completed",
    "upstream_connection_closed",
    "upstream_read_error",
    "upstream_send_error",
    "upstream_stream_error",
    "profile_transport_failure",
    "profile_transport_backoff",
    "profile_circuit_open",
    "profile_circuit_half_open_probe",
    "websocket_precommit_frame_timeout",
    "websocket_precommit_hold_timeout",
    "websocket_dns_resolve_timeout",
    "websocket_dns_overflow_enqueue",
    "websocket_dns_overflow_dispatch",
    "websocket_dns_overflow_reject",
    "websocket_connect_local_pressure",
    "websocket_connect_overflow_enqueue",
    "websocket_connect_overflow_dispatch",
    "websocket_connect_overflow_reject",
    "websocket_connect_overflow_rejected",
    "websocket_proxy_tunnel_failure",
    "local_writer_error",
];

pub const RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS: &[&str] = &[
    "state_save_error",
    "state_save_queue_backpressure",
    "continuation_journal_save_error",
    "continuation_journal_queue_backpressure",
];

pub const RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS: &[&str] = &["state_save_skipped"];

pub const RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS: &[&str] =
    &["profile_probe_refresh_start", "profile_probe_refresh_ok"];
