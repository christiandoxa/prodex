use criterion::{Criterion, black_box, criterion_group, criterion_main};
use prodex::bench_support::{
    RuntimeProxyLineageCleanupBenchCase, RuntimeProxyPreviousResponseBenchCase,
    RuntimeProxyQuotaFallbackBenchCase, RuntimeProxySseInspectBenchCase,
};

fn runtime_proxy_hot_paths(c: &mut Criterion) {
    let quota_fallback = RuntimeProxyQuotaFallbackBenchCase::new(64);
    c.bench_function("runtime_route_eligible_quota_fallback_scan", |b| {
        b.iter(|| black_box(quota_fallback.has_route_eligible_quota_fallback()))
    });

    let previous_response = RuntimeProxyPreviousResponseBenchCase::new(64);
    c.bench_function("runtime_previous_response_candidate_selection", |b| {
        b.iter(|| black_box(previous_response.next_previous_response_candidate()))
    });

    let sse_inspect = RuntimeProxySseInspectBenchCase::new(64);
    c.bench_function("runtime_sse_lookahead_inspection", |b| {
        b.iter(|| black_box(sse_inspect.inspect()))
    });

    let lineage_cleanup = RuntimeProxyLineageCleanupBenchCase::new(128);
    c.bench_function("runtime_dead_lineage_cleanup", |b| {
        b.iter(|| black_box(lineage_cleanup.clear_dead_response_bindings()))
    });
}

criterion_group!(benches, runtime_proxy_hot_paths);
criterion_main!(benches);
