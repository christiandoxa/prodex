use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, runtime_gateway_admin_auth_is_unscoped,
};
use super::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct RuntimeGatewayPrometheusRow {
    pub(super) usage: runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage,
}

pub(super) fn runtime_gateway_prometheus_text(
    rows: &BTreeMap<String, RuntimeGatewayPrometheusRow>,
) -> String {
    let totals = rows.values().fold(
        runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage::default(),
        |mut totals, row| {
            totals.requests_total = totals
                .requests_total
                .saturating_add(row.usage.requests_total);
            totals.spend_microusd = totals
                .spend_microusd
                .saturating_add(row.usage.spend_microusd);
            match row.usage.minute_epoch.cmp(&totals.minute_epoch) {
                std::cmp::Ordering::Greater => {
                    totals.minute_epoch = row.usage.minute_epoch;
                    totals.requests_this_minute = row.usage.requests_this_minute;
                    totals.tokens_this_minute = row.usage.tokens_this_minute;
                }
                std::cmp::Ordering::Equal => {
                    totals.requests_this_minute = totals
                        .requests_this_minute
                        .saturating_add(row.usage.requests_this_minute);
                    totals.tokens_this_minute = totals
                        .tokens_this_minute
                        .saturating_add(row.usage.tokens_this_minute);
                }
                std::cmp::Ordering::Less => {}
            }
            totals
        },
    );
    let mut body = String::new();
    body.push_str(
        "# HELP prodex_gateway_virtual_key_requests_total Total accepted gateway requests visible to this caller.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_requests_total counter\n");
    body.push_str(&format!(
        "prodex_gateway_virtual_key_requests_total {}\n",
        totals.requests_total
    ));
    body.push_str(
        "# HELP prodex_gateway_virtual_key_spend_microusd_total Estimated gateway spend visible to this caller in micro-USD.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_spend_microusd_total counter\n");
    body.push_str(&format!(
        "prodex_gateway_virtual_key_spend_microusd_total {}\n",
        totals.spend_microusd
    ));
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_requests Current minute accepted gateway requests visible to this caller.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_requests gauge\n");
    body.push_str(&format!(
        "prodex_gateway_virtual_key_minute_requests {}\n",
        totals.requests_this_minute
    ));
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_tokens Current minute estimated input tokens visible to this caller.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_tokens gauge\n");
    body.push_str(&format!(
        "prodex_gateway_virtual_key_minute_tokens {}\n",
        totals.tokens_this_minute
    ));
    body.push_str(&runtime_operational_prometheus_text());
    body
}

pub(super) fn runtime_gateway_prometheus_response(
    shared: &RuntimeLocalRewriteProxyShared,
    admin_auth: &RuntimeGatewayAdminAuth,
) -> tiny_http::ResponseBox {
    let usage = shared
        .gateway_usage
        .usage
        .lock()
        .map(|usage| usage.clone())
        .unwrap_or_default();
    let entries = shared
        .gateway_virtual_keys
        .lock()
        .map(|entries| entries.clone())
        .unwrap_or_default();
    let mut rows = BTreeMap::new();
    for entry in entries {
        if !admin_auth.can_access_entry(&entry) {
            continue;
        }
        let usage = usage.get(&entry.key.name).cloned().unwrap_or_default();
        rows.insert(
            entry.key.name.clone(),
            RuntimeGatewayPrometheusRow { usage },
        );
    }
    for (name, usage) in usage {
        if !runtime_gateway_admin_auth_is_unscoped(admin_auth) || !admin_auth.can_access_key(&name)
        {
            continue;
        }
        rows.entry(name)
            .or_insert_with(|| RuntimeGatewayPrometheusRow { usage });
    }

    let body = runtime_gateway_prometheus_text(&rows);

    build_runtime_proxy_response_from_parts(RuntimeHeapTrimmedBufferedResponseParts {
        status: 200,
        headers: vec![
            (
                "content-type".to_string(),
                b"text/plain; version=0.0.4; charset=utf-8".to_vec(),
            ),
            ("cache-control".to_string(), b"no-store".to_vec()),
            ("x-content-type-options".to_string(), b"nosniff".to_vec()),
        ],
        body: body.into_bytes().into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_text_aggregates_keys_without_high_cardinality_labels() {
        let key_name = "team\"a\\b\nmain";
        let mut rows = BTreeMap::new();
        rows.insert(
            key_name.to_string(),
            RuntimeGatewayPrometheusRow {
                usage: runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 7,
                    spend_microusd: 11,
                    minute_epoch: 20,
                    requests_this_minute: 2,
                    tokens_this_minute: 3,
                },
            },
        );
        rows.insert(
            "other-key".to_string(),
            RuntimeGatewayPrometheusRow {
                usage: runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 5,
                    spend_microusd: 13,
                    minute_epoch: 20,
                    requests_this_minute: 4,
                    tokens_this_minute: 6,
                },
            },
        );
        let body = runtime_gateway_prometheus_text(&rows);
        assert!(!body.contains("team\\\"a\\\\b\\nmain"));
        assert!(!body.contains("key_hash="));
        assert!(!body.contains("minute_epoch="));
        assert!(!body.contains("tenant_id="));
        assert!(!body.contains("tenant\\nx"));
        assert!(body.contains("prodex_gateway_virtual_key_requests_total 12\n"));
        assert!(body.contains("prodex_gateway_virtual_key_spend_microusd_total 24\n"));
        assert!(body.contains("prodex_gateway_virtual_key_minute_requests 6\n"));
        assert!(body.contains("prodex_gateway_virtual_key_minute_tokens 9\n"));
    }

    #[test]
    fn prometheus_text_includes_live_operational_counters() {
        record_runtime_authz_metric(
            prodex_observability::AuthzBoundaryKind::DataPlaneInference,
            prodex_observability::AuthzDecisionResult::Allowed,
        );

        let body = runtime_gateway_prometheus_text(&BTreeMap::new());

        assert!(body.contains("prodex_authz_decisions_total"));
        assert!(body.contains("authz_result=\"allowed\""));
    }
}
