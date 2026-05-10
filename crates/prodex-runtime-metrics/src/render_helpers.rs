use std::collections::BTreeMap;
use std::fmt::Write as _;

use super::*;

pub(super) fn render_lane_family<F>(
    out: &mut String,
    descriptor: LaneFamilyDescriptor<'_>,
    broker_key: &str,
    listen_addr: &str,
    lanes: &[(&str, &RuntimeBrokerLaneMetrics)],
    value: F,
) where
    F: Fn(&RuntimeBrokerLaneMetrics) -> f64,
{
    push_help(out, descriptor.metric_name, descriptor.help);
    push_type(out, descriptor.metric_name, descriptor.metric_type);
    for (lane, snapshot) in lanes {
        push_gauge(
            out,
            descriptor.metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("lane", lane),
            ]),
            value(snapshot),
        );
    }
}

#[derive(Clone, Copy)]
pub(super) struct LaneFamilyDescriptor<'a> {
    pub(super) metric_name: &'a str,
    pub(super) help: &'a str,
    pub(super) metric_type: &'a str,
}

pub(super) fn render_continuation_binding_counts_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    continuations: &RuntimeBrokerContinuationMetrics,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (kind_label, value) in [
        ("response", continuations.response_bindings),
        ("turn_state", continuations.turn_state_bindings),
        ("session_id", continuations.session_id_bindings),
    ] {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("binding_kind", kind_label),
            ]),
            value as f64,
        );
    }
}

pub(super) fn render_continuation_lifecycle_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    continuations: &RuntimeBrokerContinuationMetrics,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (lifecycle, value) in [
        ("warm", continuations.warm),
        ("verified", continuations.verified),
        ("suspect", continuations.suspect),
        ("dead", continuations.dead),
    ] {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("lifecycle", lifecycle),
            ]),
            value as f64,
        );
    }
}

pub(super) fn render_continuation_signal_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    metrics: &RuntimeBrokerContinuationSignalMetrics,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (kind_label, value) in [
        ("response", metrics.response),
        ("turn_state", metrics.turn_state),
        ("session_id", metrics.session_id),
    ] {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("binding_kind", kind_label),
            ]),
            value as f64,
        );
    }
}

pub(super) fn render_route_continuity_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    metrics: &RuntimeBrokerRouteContinuityMetrics,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (route, value) in [
        ("responses", metrics.responses),
        ("compact", metrics.compact),
        ("websocket", metrics.websocket),
        ("standard", metrics.standard),
    ] {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("route", route),
            ]),
            value as f64,
        );
    }
}

pub(super) fn render_continuity_failure_reason_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    metrics: &RuntimeBrokerContinuityFailureReasonMetrics,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "counter");
    for (event, reasons) in [
        ("chain_retried_owner", &metrics.chain_retried_owner),
        (
            "chain_dead_upstream_confirmed",
            &metrics.chain_dead_upstream_confirmed,
        ),
        ("stale_continuation", &metrics.stale_continuation),
    ] {
        for (reason, value) in reasons {
            push_gauge(
                out,
                metric_name,
                labels(&[
                    ("broker_key", broker_key),
                    ("listen_addr", listen_addr),
                    ("event", event),
                    ("reason", reason.as_str()),
                ]),
                *value as f64,
            );
        }
    }
}

pub(super) fn render_inflight_family(
    out: &mut String,
    metric_name: &str,
    help: &str,
    inflight: &BTreeMap<String, u64>,
    broker_key: &str,
    listen_addr: &str,
) {
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (profile, count) in inflight {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("profile", profile.as_str()),
            ]),
            *count as f64,
        );
    }
}

pub(super) fn push_help(out: &mut String, metric_name: &str, help: &str) {
    let _ = writeln!(out, "# HELP {metric_name} {help}");
}

pub(super) fn push_type(out: &mut String, metric_name: &str, metric_type: &str) {
    let _ = writeln!(out, "# TYPE {metric_name} {metric_type}");
}

pub(super) fn push_gauge(
    out: &mut String,
    metric_name: &str,
    labels: BTreeMap<String, String>,
    value: f64,
) {
    let _ = write!(out, "{metric_name}");
    if !labels.is_empty() {
        let _ = write!(out, "{{");
        let mut first = true;
        for (key, value) in labels {
            if !first {
                let _ = write!(out, ",");
            }
            first = false;
            let _ = write!(
                out,
                "{}=\"{}\"",
                escape_prometheus_label_name(&key),
                escape_prometheus_label_value(&value)
            );
        }
        let _ = write!(out, "}}");
    }
    let _ = writeln!(out, " {}", format_float(value));
}

pub(super) fn labels(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (key, value) in items {
        map.insert((*key).to_string(), (*value).to_string());
    }
    map
}

pub(super) fn bool_label(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn format_float(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

fn escape_prometheus_label_name(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '_' | ':' => ch.to_string(),
            _ => "_".to_string(),
        })
        .collect()
}

fn escape_prometheus_label_value(value: &str) -> String {
    let mut out = String::new();
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '"' => out.push_str("\\\""),
            _ => out.push(ch),
        }
    }
    out
}
