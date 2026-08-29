use super::{
    RuntimePolicyProxyPreset, RuntimePolicyProxyPresetSelection, RuntimePolicyProxySettings,
};

impl RuntimePolicyProxyPreset {
    pub(super) fn settings(self) -> RuntimePolicyProxySettings {
        #[cfg(feature = "mojo")]
        {
            let preset = RuntimePolicyProxyPresetSelection::selected(self);
            let preset_id = match self {
                Self::Low => 0,
                Self::Default => 1,
                Self::ManyTerminals => 2,
                Self::Aggressive => 3,
            };
            let values = prodex_mojo_core::runtime::runtime_tuning_proxy_preset_defaults(preset_id)
                .expect("valid runtime-proxy preset must have Mojo defaults");
            RuntimePolicyProxySettings {
                preset,
                worker_count: values.worker_count,
                long_lived_worker_count: values.long_lived_worker_count,
                probe_refresh_worker_count: values.probe_refresh_worker_count,
                async_worker_count: values.async_worker_count,
                long_lived_queue_capacity: values.long_lived_queue_capacity,
                active_request_limit: values.active_request_limit,
                profile_inflight_soft_limit: values.profile_inflight_soft_limit,
                profile_inflight_hard_limit: values.profile_inflight_hard_limit,
                responses_active_limit: values.responses_active_limit,
                compact_active_limit: values.compact_active_limit,
                websocket_active_limit: values.websocket_active_limit,
                standard_active_limit: values.standard_active_limit,
                websocket_connect_worker_count: values.websocket_connect_worker_count,
                websocket_connect_queue_capacity: values.websocket_connect_queue_capacity,
                websocket_connect_overflow_capacity: values.websocket_connect_overflow_capacity,
                websocket_dns_worker_count: values.websocket_dns_worker_count,
                websocket_dns_queue_capacity: values.websocket_dns_queue_capacity,
                websocket_dns_overflow_capacity: values.websocket_dns_overflow_capacity,
                startup_sync_probe_warm_limit: values.startup_sync_probe_warm_limit,
                ..RuntimePolicyProxySettings::default()
            }
        }
        #[cfg(not(feature = "mojo"))]
        {
            self.settings_rust()
        }
    }

    #[cfg(not(feature = "mojo"))]
    fn settings_rust(self) -> RuntimePolicyProxySettings {
        let preset = RuntimePolicyProxyPresetSelection::selected(self);
        // Presets only tune local concurrency/admission knobs. Transport timeouts stay unset
        // to preserve upstream Codex stream and reconnect behavior.
        match self {
            Self::Low => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(4),
                long_lived_worker_count: Some(8),
                probe_refresh_worker_count: Some(2),
                async_worker_count: Some(2),
                long_lived_queue_capacity: Some(128),
                active_request_limit: Some(48),
                profile_inflight_soft_limit: Some(2),
                profile_inflight_hard_limit: Some(4),
                responses_active_limit: Some(36),
                compact_active_limit: Some(3),
                websocket_active_limit: Some(8),
                standard_active_limit: Some(2),
                websocket_connect_worker_count: Some(4),
                websocket_connect_queue_capacity: Some(32),
                websocket_connect_overflow_capacity: Some(64),
                websocket_dns_worker_count: Some(2),
                websocket_dns_queue_capacity: Some(16),
                websocket_dns_overflow_capacity: Some(32),
                startup_sync_probe_warm_limit: Some(1),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Default => RuntimePolicyProxySettings {
                preset,
                ..RuntimePolicyProxySettings::default()
            },
            Self::ManyTerminals => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(12),
                long_lived_worker_count: Some(32),
                probe_refresh_worker_count: Some(4),
                async_worker_count: Some(4),
                long_lived_queue_capacity: Some(512),
                active_request_limit: Some(160),
                profile_inflight_soft_limit: Some(4),
                profile_inflight_hard_limit: Some(8),
                responses_active_limit: Some(120),
                compact_active_limit: Some(8),
                websocket_active_limit: Some(32),
                standard_active_limit: Some(8),
                websocket_connect_worker_count: Some(12),
                websocket_connect_queue_capacity: Some(96),
                websocket_connect_overflow_capacity: Some(384),
                websocket_dns_worker_count: Some(6),
                websocket_dns_queue_capacity: Some(48),
                websocket_dns_overflow_capacity: Some(96),
                startup_sync_probe_warm_limit: Some(2),
                ..RuntimePolicyProxySettings::default()
            },
            Self::Aggressive => RuntimePolicyProxySettings {
                preset,
                worker_count: Some(24),
                long_lived_worker_count: Some(96),
                probe_refresh_worker_count: Some(8),
                async_worker_count: Some(8),
                long_lived_queue_capacity: Some(1024),
                active_request_limit: Some(384),
                profile_inflight_soft_limit: Some(8),
                profile_inflight_hard_limit: Some(16),
                responses_active_limit: Some(288),
                compact_active_limit: Some(16),
                websocket_active_limit: Some(96),
                standard_active_limit: Some(16),
                websocket_connect_worker_count: Some(16),
                websocket_connect_queue_capacity: Some(128),
                websocket_connect_overflow_capacity: Some(512),
                websocket_dns_worker_count: Some(8),
                websocket_dns_queue_capacity: Some(64),
                websocket_dns_overflow_capacity: Some(128),
                startup_sync_probe_warm_limit: Some(3),
                ..RuntimePolicyProxySettings::default()
            },
        }
    }
}
