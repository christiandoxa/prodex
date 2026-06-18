use std::collections::BTreeMap;

pub(super) fn runtime_gateway_budget_group_rejection(
    key: &runtime_proxy_crate::RuntimeGatewayVirtualKey,
    active_keys: &[runtime_proxy_crate::RuntimeGatewayVirtualKey],
    usage: &BTreeMap<String, runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage>,
    estimated_cost_microusd: Option<u64>,
) -> Option<runtime_proxy_crate::RuntimeGatewayVirtualKeyRejection> {
    let budget_id = key.budget_id.as_deref()?.trim();
    if budget_id.is_empty() {
        return None;
    }
    let mut requests_total = 0_u64;
    let mut spend_microusd = 0_u64;
    for entry in active_keys
        .iter()
        .filter(|entry| entry.budget_id.as_deref().map(str::trim) == Some(budget_id))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn key(
        name: &str,
        budget_id: Option<&str>,
        request_budget: Option<u64>,
        budget_microusd: Option<u64>,
    ) -> runtime_proxy_crate::RuntimeGatewayVirtualKey {
        runtime_proxy_crate::RuntimeGatewayVirtualKey {
            name: name.to_string(),
            tenant_id: None,
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
            key("alpha", Some("shared"), Some(3), None),
            key("beta", Some("shared"), Some(3), None),
            key("gamma", Some("other"), Some(3), None),
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
            key("alpha", Some("shared"), None, Some(1_000)),
            key("beta", Some("shared"), None, Some(1_000)),
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
}
