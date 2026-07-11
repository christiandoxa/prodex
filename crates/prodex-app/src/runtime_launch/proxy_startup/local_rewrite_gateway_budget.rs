use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub(super) fn runtime_gateway_budget_storage_key(
    tenant_id: prodex_domain::TenantId,
    virtual_key_id: prodex_domain::VirtualKeyId,
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
) -> prodex_storage::TenantStorageKey {
    let Some(budget_id) = key.budget_id.as_deref().filter(|value| !value.is_empty()) else {
        return prodex_storage::TenantStorageKey::virtual_key(tenant_id, virtual_key_id);
    };
    let mut digest = Sha256::new();
    runtime_gateway_hash_budget_scope_part(&mut digest, Some(&tenant_id.to_string()));
    runtime_gateway_hash_budget_scope_part(&mut digest, Some(budget_id));
    runtime_gateway_hash_budget_scope_part(&mut digest, key.team_id.as_deref());
    runtime_gateway_hash_budget_scope_part(&mut digest, key.project_id.as_deref());
    runtime_gateway_hash_budget_scope_part(&mut digest, key.user_id.as_deref());
    prodex_storage::TenantStorageKey::budget_group(
        tenant_id,
        virtual_key_id,
        prodex_storage::BudgetStorageScope::from_digest(digest.finalize().into()),
    )
}

fn runtime_gateway_hash_budget_scope_part(digest: &mut Sha256, value: Option<&str>) {
    match value {
        Some(value) => {
            digest.update([1]);
            digest.update((value.len() as u64).to_le_bytes());
            digest.update(value.as_bytes());
        }
        None => digest.update([0]),
    }
}

pub(super) fn runtime_gateway_budget_group_rejection(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    active_keys: &[runtime_proxy_crate::RuntimeGatewayVirtualKey],
    usage: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    estimated_cost_microusd: Option<u64>,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    let budget_id = key.budget_id.as_deref()?;
    if budget_id.is_empty() {
        return None;
    }
    let mut requests_total = 0_u64;
    let mut spend_microusd = 0_u64;
    for entry in active_keys
        .iter()
        .filter(|entry| entry.budget_id.as_deref() == Some(budget_id))
        .filter(|entry| runtime_gateway_budget_group_scope_matches(key, entry))
    {
        if let Some(usage) = usage.get(&entry.name) {
            requests_total = requests_total.saturating_add(usage.requests_total);
            spend_microusd = spend_microusd.saturating_add(usage.spend_microusd);
        }
    }
    if let Some(limit) = key.request_budget
        && requests_total >= limit
    {
        return Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded);
    }
    if let (Some(limit), Some(cost)) = (key.budget_microusd, estimated_cost_microusd)
        && spend_microusd.saturating_add(cost) > limit
    {
        return Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded);
    }
    None
}

fn runtime_gateway_budget_group_scope_matches(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    entry: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
) -> bool {
    key.tenant_id == entry.tenant_id
        && key.team_id == entry.team_id
        && key.project_id == entry.project_id
        && key.user_id == entry.user_id
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(
        name: &str,
        budget_id: Option<&str>,
        tenant_id: Option<&str>,
        request_budget: Option<u64>,
        budget_microusd: Option<u64>,
    ) -> runtime_proxy_crate::RuntimeGatewayVirtualKey {
        runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: name.to_string(),
            tenant_id: tenant_id.map(str::to_string),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: budget_id.map(str::to_string),
            token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(name),
            allowed_models: Vec::new(),
            budget_microusd,
            request_budget,
            rpm_limit: None,
            tpm_limit: None,
        }
    }

    #[test]
    fn rejects_when_budget_group_request_limit_is_exhausted() {
        let active_keys = vec![
            key("alpha", Some("shared"), None, Some(3), None),
            key("beta", Some("shared"), None, Some(3), None),
            key("gamma", Some("other"), None, Some(3), None),
        ];
        let usage = BTreeMap::from([
            (
                "alpha".to_string(),
                runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 2,
                    ..Default::default()
                },
            ),
            (
                "beta".to_string(),
                runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 1,
                    ..Default::default()
                },
            ),
            (
                "gamma".to_string(),
                runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 10,
                    ..Default::default()
                },
            ),
        ]);

        assert_eq!(
            runtime_gateway_budget_group_rejection(&active_keys[0], &active_keys, &usage, None),
            Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::RequestBudgetExceeded)
        );
    }

    #[test]
    fn rejects_when_budget_group_spend_would_exceed_limit() {
        let active_keys = vec![
            key("alpha", Some("shared"), None, None, Some(1_000)),
            key("beta", Some("shared"), None, None, Some(1_000)),
        ];
        let usage = BTreeMap::from([(
            "beta".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                spend_microusd: 800,
                ..Default::default()
            },
        )]);

        assert_eq!(
            runtime_gateway_budget_group_rejection(
                &active_keys[0],
                &active_keys,
                &usage,
                Some(250)
            ),
            Some(runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection::BudgetExceeded)
        );
    }

    #[test]
    fn budget_group_request_limit_is_scoped_by_tenant() {
        let active_keys = vec![
            key("alpha", Some("shared"), Some("tenant-a"), Some(1), None),
            key("beta", Some("shared"), Some("tenant-b"), Some(1), None),
        ];
        let usage = BTreeMap::from([(
            "alpha".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                requests_total: 1,
                ..Default::default()
            },
        )]);

        assert_eq!(
            runtime_gateway_budget_group_rejection(&active_keys[1], &active_keys, &usage, None),
            None
        );
    }

    #[test]
    fn budget_group_ids_match_exactly_without_trimming() {
        let active_keys = vec![
            key("alpha", Some("shared"), Some("tenant-a"), Some(1), None),
            key("beta", Some(" shared "), Some("tenant-a"), Some(1), None),
        ];
        let usage = BTreeMap::from([(
            "alpha".to_string(),
            runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                requests_total: 1,
                ..Default::default()
            },
        )]);

        assert_eq!(
            runtime_gateway_budget_group_rejection(&active_keys[1], &active_keys, &usage, None),
            None
        );
    }

    #[test]
    fn budget_group_storage_scope_is_stable_across_virtual_keys_and_tenant_scoped() {
        let tenant_id = prodex_domain::TenantId::new();
        let alpha = key(
            "alpha",
            Some("shared"),
            Some(&tenant_id.to_string()),
            Some(1),
            None,
        );
        let mut beta = alpha.clone();
        beta.name = "beta".to_string();
        beta.token_hash = runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token("beta");
        let alpha_scope = runtime_gateway_budget_storage_key(
            tenant_id,
            prodex_domain::VirtualKeyId::new(),
            &alpha,
        );
        let beta_scope = runtime_gateway_budget_storage_key(
            tenant_id,
            prodex_domain::VirtualKeyId::new(),
            &beta,
        );
        assert_eq!(alpha_scope.budget_scope, beta_scope.budget_scope);
        assert_ne!(alpha_scope.virtual_key_id, beta_scope.virtual_key_id);

        let other_tenant = prodex_domain::TenantId::new();
        let other_scope = runtime_gateway_budget_storage_key(
            other_tenant,
            prodex_domain::VirtualKeyId::new(),
            &beta,
        );
        assert_ne!(alpha_scope.budget_scope, other_scope.budget_scope);
    }
}
