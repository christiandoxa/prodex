#[cfg(any(not(feature = "mojo"), test))]
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
            CompactTransportFailure => "compact_transport_failure",
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
            SelectionPlan => "selection_plan",
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
            LocalRewriteUpstreamStart => "local_rewrite_upstream_start",
            LocalRewriteUpstreamResponse => "local_rewrite_upstream_response",
            LocalRewriteRequestDetail => "local_rewrite_request_detail",
            LocalRewriteWebSearchOptionsFallback => "local_rewrite_web_search_options_fallback",
            LocalRewriteProviderModelFallback => "local_rewrite_provider_model_fallback",
            LocalRewriteProviderAuthFailure => "local_rewrite_provider_auth_failure",
            LocalRewriteGeminiBuiltinToolFallback => "local_rewrite_gemini_builtin_tool_fallback",
            LocalRewriteGeminiQuotaRotate => "local_rewrite_gemini_quota_rotate",
            LocalRewriteGeminiRateLimitRetry => "local_rewrite_gemini_rate_limit_retry",
            LocalRewriteGeminiInvalidStreamRetry => "local_rewrite_gemini_invalid_stream_retry",
            LocalRewriteGeminiInvalidStreamModelFallback => "local_rewrite_gemini_invalid_stream_model_fallback",
            LocalRewriteGeminiQuotaStatusReady => "local_rewrite_gemini_quota_status_ready",
            LocalRewriteGeminiQuotaStatusUnavailable => "local_rewrite_gemini_quota_status_unavailable",
            LocalRewriteGeminiCompactSemantic => "local_rewrite_gemini_compact_semantic",
            LocalRewriteGeminiCompactFallback => "local_rewrite_gemini_compact_fallback",
            LocalRewriteGeminiSyntheticThoughtSignature => "local_rewrite_gemini_synthetic_thought_signature",
            LocalRewriteGeminiLiveSidecarStarted => "local_rewrite_gemini_live_sidecar_started",
            LocalRewriteGeminiLiveSidecarError => "local_rewrite_gemini_live_sidecar_error",
            LocalRewriteGeminiLiveSidecarAcceptError => "local_rewrite_gemini_live_sidecar_accept_error",
            LocalRewriteGeminiLiveConnected => "local_rewrite_gemini_live_connected",
            LocalRewriteGeminiLiveError => "local_rewrite_gemini_live_error",
            LocalRewriteGeminiLiveSidecarConnected => "local_rewrite_gemini_live_sidecar_connected",
            LocalRewriteGeminiLiveSidecarSessionError => "local_rewrite_gemini_live_sidecar_session_error",
            LocalRewriteGeminiLiveFrame => "local_rewrite_gemini_live_frame",
            LocalRewriteGeminiLiveDuplexPump => "local_rewrite_gemini_live_duplex_pump",
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

#[cfg(any(not(feature = "mojo"), test))]
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

#[cfg(any(not(feature = "mojo"), test))]
runtime_doctor_marker_registry!(define_runtime_doctor_marker_enum);

pub const RUNTIME_DOCTOR_FACETS: &[&str] = &[
    "lane",
    "route",
    "profile",
    "reason",
    "transport",
    "provider",
    "family",
    "client",
    "tool_surface",
    "continuation",
    "origin",
    "warning",
    "quota_source",
    "quota_band",
    "five_hour_status",
    "weekly_status",
    "affinity",
    "context",
    "event",
    "stage",
    "state",
    "source",
    "request_shape",
    "exit",
    "mode",
    "tier",
    "decision",
    "reasons",
    "token_usage_source",
    "self_check",
    "budget_mode",
    "policy_reasons",
];

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RuntimeDoctorLogFacet {
    Lane,
    Route,
    Profile,
    Reason,
    Transport,
    Provider,
    Family,
    Client,
    ToolSurface,
    Continuation,
    Origin,
    Warning,
    QuotaSource,
    QuotaBand,
    FiveHourStatus,
    WeeklyStatus,
    Affinity,
    Context,
    Event,
    Stage,
    State,
    Source,
    RequestShape,
    Exit,
    Mode,
    Tier,
    Decision,
    Reasons,
    TokenUsageSource,
    SelfCheck,
    BudgetMode,
    PolicyReasons,
}

#[cfg(test)]
impl RuntimeDoctorLogFacet {
    pub const ALL: &'static [Self] = &[
        Self::Lane,
        Self::Route,
        Self::Profile,
        Self::Reason,
        Self::Transport,
        Self::Provider,
        Self::Family,
        Self::Client,
        Self::ToolSurface,
        Self::Continuation,
        Self::Origin,
        Self::Warning,
        Self::QuotaSource,
        Self::QuotaBand,
        Self::FiveHourStatus,
        Self::WeeklyStatus,
        Self::Affinity,
        Self::Context,
        Self::Event,
        Self::Stage,
        Self::State,
        Self::Source,
        Self::RequestShape,
        Self::Exit,
        Self::Mode,
        Self::Tier,
        Self::Decision,
        Self::Reasons,
        Self::TokenUsageSource,
        Self::SelfCheck,
        Self::BudgetMode,
        Self::PolicyReasons,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lane => "lane",
            Self::Route => "route",
            Self::Profile => "profile",
            Self::Reason => "reason",
            Self::Transport => "transport",
            Self::Provider => "provider",
            Self::Family => "family",
            Self::Client => "client",
            Self::ToolSurface => "tool_surface",
            Self::Continuation => "continuation",
            Self::Origin => "origin",
            Self::Warning => "warning",
            Self::QuotaSource => "quota_source",
            Self::QuotaBand => "quota_band",
            Self::FiveHourStatus => "five_hour_status",
            Self::WeeklyStatus => "weekly_status",
            Self::Affinity => "affinity",
            Self::Context => "context",
            Self::Event => "event",
            Self::Stage => "stage",
            Self::State => "state",
            Self::Source => "source",
            Self::RequestShape => "request_shape",
            Self::Exit => "exit",
            Self::Mode => "mode",
            Self::Tier => "tier",
            Self::Decision => "decision",
            Self::Reasons => "reasons",
            Self::TokenUsageSource => "token_usage_source",
            Self::SelfCheck => "self_check",
            Self::BudgetMode => "budget_mode",
            Self::PolicyReasons => "policy_reasons",
        }
    }

    pub fn from_name(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|facet| facet.as_str() == value)
    }
}

#[cfg(test)]
pub struct RuntimeDoctorMarkerDescriptor {
    pub marker: RuntimeDoctorMarker,
    pub name: &'static str,
}

#[cfg(test)]
pub fn runtime_doctor_marker_descriptor(value: &str) -> Option<RuntimeDoctorMarkerDescriptor> {
    RuntimeDoctorMarker::from_name(value).map(|marker| RuntimeDoctorMarkerDescriptor {
        marker,
        name: marker.as_str(),
    })
}

#[cfg(feature = "runtime-log-mojo")]
pub(crate) fn runtime_doctor_marker_is_known(value: &str) -> bool {
    prodex_mojo_core::rich::runtime_doctor_marker_known(value)
        .expect("Mojo runtime-doctor marker classifier returned invalid output")
}

#[cfg(any(not(feature = "mojo"), test))]
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
    "compact_transport_failure",
    "local_rewrite_gemini_quota_rotate",
];

#[cfg(any(not(feature = "mojo"), test))]
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
    "compact_transport_failure",
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
    "local_rewrite_gemini_invalid_stream_retry",
    "local_rewrite_gemini_invalid_stream_model_fallback",
    "local_rewrite_gemini_live_error",
    "local_rewrite_gemini_live_sidecar_error",
    "local_rewrite_gemini_live_sidecar_accept_error",
    "local_rewrite_gemini_live_sidecar_session_error",
];

#[cfg(any(not(feature = "mojo"), test))]
pub const RUNTIME_DOCTOR_PERSISTENCE_PRESSURE_MARKERS: &[&str] = &[
    "state_save_error",
    "state_save_queue_backpressure",
    "continuation_journal_save_error",
    "continuation_journal_queue_backpressure",
];

#[cfg(any(not(feature = "mojo"), test))]
pub const RUNTIME_DOCTOR_ACTIVE_PERSISTENCE_MARKERS: &[&str] = &["state_save_skipped"];

#[cfg(any(not(feature = "mojo"), test))]
pub const RUNTIME_DOCTOR_ACTIVE_QUOTA_REFRESH_MARKERS: &[&str] =
    &["profile_probe_refresh_start", "profile_probe_refresh_ok"];
