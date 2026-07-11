use super::local_rewrite::RuntimeLocalRewriteProxyShared;
use super::local_rewrite_gateway_admin_auth::{
    RuntimeGatewayAdminAuth, runtime_gateway_admin_auth_is_unscoped,
};
use super::*;
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct RuntimeGatewayPrometheusRow {
    pub(super) source: String,
    pub(super) disabled: bool,
    pub(super) tenant_id: Option<String>,
    pub(super) team_id: Option<String>,
    pub(super) project_id: Option<String>,
    pub(super) user_id: Option<String>,
    pub(super) budget_id: Option<String>,
    pub(super) usage: runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage,
}

pub(super) fn runtime_gateway_prometheus_text(
    rows: &BTreeMap<String, RuntimeGatewayPrometheusRow>,
) -> String {
    let mut body = String::new();
    body.push_str(
        "# HELP prodex_gateway_virtual_key_requests_total Total accepted gateway requests by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_requests_total counter\n");
    for (name, row) in rows {
        let labels = runtime_gateway_prometheus_key_labels(name, row);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_requests_total{{{labels}}} {}\n",
            row.usage.requests_total
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_spend_microusd_total Estimated gateway spend by virtual key in micro-USD.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_spend_microusd_total counter\n");
    for (name, row) in rows {
        let labels = runtime_gateway_prometheus_key_labels(name, row);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_spend_microusd_total{{{labels}}} {}\n",
            row.usage.spend_microusd
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_requests Current minute accepted gateway requests by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_requests gauge\n");
    for (name, row) in rows {
        let labels = runtime_gateway_prometheus_key_labels(name, row);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_minute_requests{{{labels},minute_epoch=\"{}\"}} {}\n",
            row.usage.minute_epoch, row.usage.requests_this_minute
        ));
    }
    body.push_str(
        "# HELP prodex_gateway_virtual_key_minute_tokens Current minute estimated input tokens by virtual key.\n",
    );
    body.push_str("# TYPE prodex_gateway_virtual_key_minute_tokens gauge\n");
    for (name, row) in rows {
        let labels = runtime_gateway_prometheus_key_labels(name, row);
        body.push_str(&format!(
            "prodex_gateway_virtual_key_minute_tokens{{{labels},minute_epoch=\"{}\"}} {}\n",
            row.usage.minute_epoch, row.usage.tokens_this_minute
        ));
    }
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
            RuntimeGatewayPrometheusRow {
                source: entry.source.as_str().to_string(),
                disabled: entry.disabled,
                tenant_id: entry.tenant_id.clone(),
                team_id: entry.key.team_id.clone(),
                project_id: entry.key.project_id.clone(),
                user_id: entry.key.user_id.clone(),
                budget_id: entry.key.budget_id.clone(),
                usage,
            },
        );
    }
    for (name, usage) in usage {
        if !runtime_gateway_admin_auth_is_unscoped(admin_auth) || !admin_auth.can_access_key(&name)
        {
            continue;
        }
        rows.entry(name)
            .or_insert_with(|| RuntimeGatewayPrometheusRow {
                source: "unknown".to_string(),
                disabled: false,
                tenant_id: None,
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                usage,
            });
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

fn runtime_gateway_prometheus_key_labels(name: &str, row: &RuntimeGatewayPrometheusRow) -> String {
    format!(
        "key_hash=\"{}\",source=\"{}\",disabled=\"{}\",tenant_scoped=\"{}\",team_scoped=\"{}\",project_scoped=\"{}\",user_scoped=\"{}\",budget_scoped=\"{}\"",
        runtime_gateway_prometheus_label_escape(&runtime_gateway_prometheus_stable_hash(name)),
        runtime_gateway_prometheus_label_escape(&row.source),
        row.disabled,
        row.tenant_id.is_some(),
        row.team_id.is_some(),
        row.project_id.is_some(),
        row.user_id.is_some(),
        row.budget_id.is_some()
    )
}

fn runtime_gateway_prometheus_stable_hash(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

fn runtime_gateway_prometheus_label_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prometheus_text_omits_raw_high_cardinality_labels() {
        assert_eq!(
            runtime_gateway_prometheus_stable_hash("team-a"),
            "de90a3717d1252d6"
        );
        let key_name = "team\"a\\b\nmain";
        let mut rows = BTreeMap::new();
        rows.insert(
            key_name.to_string(),
            RuntimeGatewayPrometheusRow {
                source: "policy".to_string(),
                disabled: false,
                tenant_id: Some("tenant\nx".to_string()),
                team_id: None,
                project_id: None,
                user_id: None,
                budget_id: None,
                usage: runtime_proxy_crate::RuntimeGatewayVirtualKeyUsage {
                    requests_total: 7,
                    ..Default::default()
                },
            },
        );
        let body = runtime_gateway_prometheus_text(&rows);
        assert!(!body.contains("team\\\"a\\\\b\\nmain"));
        assert!(body.contains(&format!(
            "key_hash=\"{}\"",
            runtime_gateway_prometheus_stable_hash(key_name)
        )));
        assert!(!body.contains("tenant_id="));
        assert!(!body.contains("tenant\\nx"));
        assert!(body.contains("tenant_scoped=\"true\""));
        assert!(body.contains("user_scoped=\"false\""));
        assert!(body.contains(" 7\n"));
    }
}
