use crate::*;
mod common;
mod selection_cases;
mod stream_cases;
pub use prodex_bench_support::{
    RUNTIME_PROXY_COMPACT_SESSION_SELECTION_BENCH_CASE, RUNTIME_PROXY_HOT_PATH_BENCH_CASE_SPECS,
    RUNTIME_PROXY_LINEAGE_CLEANUP_BENCH_CASE, RUNTIME_PROXY_MIXED_POOL_BENCH_CASE,
    RUNTIME_PROXY_PREVIOUS_RESPONSE_BENCH_CASE, RUNTIME_PROXY_QUOTA_FALLBACK_BENCH_CASE,
    RUNTIME_PROXY_RUNTIME_MEM_SUPER_SLIM_BENCH_CASE,
    RUNTIME_PROXY_SMART_CONTEXT_REWRITE_BENCH_CASE, RUNTIME_PROXY_SSE_INSPECT_BENCH_CASE,
    RUNTIME_PROXY_WEBSOCKET_STALE_REUSE_BENCH_CASE, RuntimeProxyHotPathBenchCaseSpec,
    RuntimeProxyHotPathBenchCaseSuite, RuntimeProxyHotPathBenchCheckConfig,
    RuntimeProxyHotPathBenchCheckResult, RuntimeProxyHotPathBenchScenarioSizes,
    RuntimeProxyHotPathBenchThreshold, run_runtime_proxy_hot_path_case,
    run_runtime_proxy_hot_path_case_suite,
};
pub use selection_cases::{
    RuntimeProxyCompactSessionSelectionBenchCase, RuntimeProxyMixedPoolSelectionBenchCase,
    RuntimeProxyPreviousResponseBenchCase, RuntimeProxyQuotaFallbackBenchCase,
    RuntimeProxyWebsocketStaleReuseBenchCase,
};
pub use stream_cases::{
    RuntimeProxyLineageCleanupBenchCase, RuntimeProxyRuntimeMemSuperSlimBenchCase,
    RuntimeProxySmartContextRewriteBenchCase, RuntimeProxySseInspectBenchCase,
};

use self::common::*;

#[doc(hidden)]
pub fn run_runtime_proxy_hot_path_bench_check(
    config: RuntimeProxyHotPathBenchCheckConfig,
) -> Vec<RuntimeProxyHotPathBenchCheckResult> {
    let sizes = RuntimeProxyHotPathBenchScenarioSizes::default();
    let quota_fallback = RuntimeProxyQuotaFallbackBenchCase::new(sizes.quota_fallback);
    let previous_response = RuntimeProxyPreviousResponseBenchCase::new(sizes.previous_response);
    let mixed_pool_selection =
        RuntimeProxyMixedPoolSelectionBenchCase::new(sizes.mixed_pool_selection);
    let compact_session_selection =
        RuntimeProxyCompactSessionSelectionBenchCase::new(sizes.compact_session_selection);
    let websocket_stale_reuse =
        RuntimeProxyWebsocketStaleReuseBenchCase::new(sizes.websocket_stale_reuse);
    let sse_inspect = RuntimeProxySseInspectBenchCase::new(sizes.sse_inspect);
    let lineage_cleanup = RuntimeProxyLineageCleanupBenchCase::new(sizes.lineage_cleanup);
    let smart_context_rewrite =
        RuntimeProxySmartContextRewriteBenchCase::new(sizes.smart_context_rewrite);
    let runtime_mem_super_slim =
        RuntimeProxyRuntimeMemSuperSlimBenchCase::new(sizes.runtime_mem_super_slim);

    run_runtime_proxy_hot_path_case_suite(
        config,
        RuntimeProxyHotPathBenchCaseSuite {
            quota_fallback: || quota_fallback.has_route_eligible_quota_fallback(),
            previous_response: || previous_response.next_previous_response_candidate(),
            mixed_pool_selection: || mixed_pool_selection.select_fresh_response_candidate(),
            compact_session_selection: || {
                compact_session_selection.select_compact_session_candidate()
            },
            websocket_stale_reuse: || websocket_stale_reuse.evaluate_stale_reuse_affinity(),
            sse_inspect: || sse_inspect.inspect(),
            lineage_cleanup: || lineage_cleanup.clear_dead_response_bindings(),
            smart_context_rewrite: || smart_context_rewrite.rewrite_large_tool_output(),
            runtime_mem_super_slim: || runtime_mem_super_slim.shadow_token_heavy_events(),
        },
    )
}
