#![cfg(all(feature = "mojo-runtime", prodex_mojo_required))]

use prodex_mojo_core::runtime_proxy_body_limit::runtime_proxy_body_size_exceeds_limit;

#[test]
fn runtime_proxy_body_limit_accepts_exact_limit_and_rejects_limit_plus_one() {
    assert!(!runtime_proxy_body_size_exceeds_limit(64, 63).unwrap());
    assert!(!runtime_proxy_body_size_exceeds_limit(64, 64).unwrap());
    assert!(runtime_proxy_body_size_exceeds_limit(64, 65).unwrap());
    assert!(!runtime_proxy_body_size_exceeds_limit(u64::MAX, u64::MAX).unwrap());
}
