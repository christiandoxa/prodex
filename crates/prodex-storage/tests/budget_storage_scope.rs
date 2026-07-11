use prodex_domain::{TenantId, VirtualKeyId};
use prodex_storage::{BudgetStorageScope, TenantStorageKey};

#[test]
fn budget_storage_scope_is_stable_and_does_not_expose_raw_group_data() {
    let tenant_id = TenantId::new();
    let first = TenantStorageKey::budget_group(
        tenant_id,
        VirtualKeyId::new(),
        BudgetStorageScope::from_digest([9; 32]),
    );
    let second = TenantStorageKey::budget_group(
        tenant_id,
        VirtualKeyId::new(),
        BudgetStorageScope::from_digest([9; 32]),
    );
    assert_eq!(first.storage_scope(), second.storage_scope());
    assert!(first.storage_scope().starts_with("budget_group:"));
    assert!(!first.storage_scope().contains(&tenant_id.to_string()));
}
