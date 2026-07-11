use prodex_domain::BudgetLimit;

#[test]
fn request_budget_limit_defaults_unlimited_and_accepts_explicit_cap() {
    let limit = BudgetLimit::new(100, 10_000);
    assert_eq!(limit.max_requests, u64::MAX);
    assert_eq!(limit.with_max_requests(7).max_requests, 7);
}
