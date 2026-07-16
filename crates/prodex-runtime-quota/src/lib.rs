mod cache;
mod pressure;
mod route;
pub mod selection;
mod snapshot;
mod source;
mod summary;
mod window;

pub use cache::{runtime_profile_probe_cache_freshness, runtime_profile_usage_cache_is_fresh};
pub use pressure::{
    RuntimeQuotaPressureSortKey, runtime_quota_pressure_band_for_route,
    runtime_quota_pressure_band_from_proxy, runtime_quota_pressure_band_rank,
    runtime_quota_pressure_band_reason, runtime_quota_pressure_band_to_proxy,
    runtime_quota_pressure_sort_key_for_route,
    runtime_quota_pressure_sort_key_for_route_from_summary, runtime_quota_sort_key_from_proxy,
    runtime_response_quota_pressure_sort_key_to_proxy,
};
pub use route::runtime_route_kind_to_proxy;
pub use selection::*;
pub use snapshot::{
    RuntimeProfileUsageSnapshot, runtime_profile_usage_snapshot_from_usage,
    runtime_profile_usage_snapshot_hold_active, runtime_profile_usage_snapshot_hold_expired,
    runtime_quota_summary_from_usage_snapshot, runtime_quota_summary_from_usage_snapshot_at,
    runtime_snapshot_blocks_same_request_cold_start_probe, runtime_usage_snapshot_from_proxy,
    runtime_usage_snapshot_is_usable, runtime_usage_snapshot_to_proxy,
    usage_from_runtime_usage_snapshot,
};
pub use source::{
    runtime_quota_source_from_proxy, runtime_quota_source_label,
    runtime_quota_source_option_to_proxy, runtime_quota_source_to_proxy,
};
pub use summary::{
    runtime_precommit_quota_block_reason, runtime_precommit_quota_gate_final_decision,
    runtime_precommit_quota_gate_initial_decision, runtime_quota_precommit_guard_reason,
    runtime_quota_soft_affinity_rejection_reason, runtime_quota_summary_allows_soft_affinity,
    runtime_quota_summary_blocking_reset_at, runtime_quota_summary_for_route,
    runtime_quota_summary_from_cached_sources, runtime_quota_summary_from_proxy,
    runtime_quota_summary_log_fields, runtime_quota_summary_requires_live_source_after_probe,
    runtime_quota_summary_requires_precommit_live_probe, runtime_quota_summary_to_proxy,
    runtime_selection_quota_summary_from_proxy, runtime_selection_quota_summary_to_proxy,
};
pub use window::{
    runtime_quota_window_observation, runtime_quota_window_observation_at,
    runtime_quota_window_status_from_proxy, runtime_quota_window_status_reason,
    runtime_quota_window_status_to_proxy, runtime_quota_window_summary,
    runtime_quota_window_summary_from_proxy, runtime_quota_window_summary_from_usage_snapshot_at,
    runtime_quota_window_summary_to_proxy, runtime_quota_window_usable_for_auto_rotate,
};

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
