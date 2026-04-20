use std::collections::BTreeMap;
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerLaneMetrics {
    pub active: u64,
    pub limit: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerTrafficMetrics {
    pub responses: RuntimeBrokerLaneMetrics,
    pub compact: RuntimeBrokerLaneMetrics,
    pub websocket: RuntimeBrokerLaneMetrics,
    pub standard: RuntimeBrokerLaneMetrics,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerContinuationMetrics {
    pub response_bindings: u64,
    pub turn_state_bindings: u64,
    pub session_id_bindings: u64,
    pub warm: u64,
    pub verified: u64,
    pub suspect: u64,
    pub dead: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RuntimeBrokerSnapshot {
    pub broker_key: String,
    pub listen_addr: String,
    pub pid: u32,
    pub started_at_unix_seconds: i64,
    pub current_profile: String,
    pub include_code_review: bool,
    pub persistence_role: String,
    pub prodex_version: Option<String>,
    pub executable_path: Option<String>,
    pub executable_sha256: Option<String>,
    pub active_requests: u64,
    pub active_request_limit: u64,
    pub local_overload_backoff_remaining_seconds: u64,
    pub traffic: RuntimeBrokerTrafficMetrics,
    pub profile_inflight: BTreeMap<String, u64>,
    pub retry_backoffs: u64,
    pub transport_backoffs: u64,
    pub route_circuits: u64,
    pub degraded_profiles: u64,
    pub degraded_routes: u64,
    pub continuations: RuntimeBrokerContinuationMetrics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrometheusTextOptions {
    pub include_help: bool,
}

impl Default for PrometheusTextOptions {
    fn default() -> Self {
        Self { include_help: true }
    }
}

pub fn render_runtime_broker_prometheus(snapshot: &RuntimeBrokerSnapshot) -> String {
    render_runtime_broker_prometheus_with_options(snapshot, PrometheusTextOptions::default())
}

pub fn render_runtime_broker_prometheus_with_options(
    snapshot: &RuntimeBrokerSnapshot,
    options: PrometheusTextOptions,
) -> String {
    RuntimeBrokerPrometheusRenderer::new(snapshot, options).render()
}

struct RuntimeBrokerPrometheusRenderer<'a> {
    snapshot: &'a RuntimeBrokerSnapshot,
    options: PrometheusTextOptions,
    out: String,
}

impl<'a> RuntimeBrokerPrometheusRenderer<'a> {
    fn new(snapshot: &'a RuntimeBrokerSnapshot, options: PrometheusTextOptions) -> Self {
        Self {
            snapshot,
            options,
            out: String::new(),
        }
    }

    fn render(mut self) -> String {
        self.render_info();
        self.render_lane_families();
        self.render_broker_gauge(
            "prodex_runtime_broker_active_requests",
            "Active runtime requests currently being served by the broker.",
            self.snapshot.active_requests as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_active_request_limit",
            "Maximum active runtime requests allowed by the broker.",
            self.snapshot.active_request_limit as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_local_overload_backoff_remaining_seconds",
            "Remaining backoff time for local overload shedding.",
            self.snapshot.local_overload_backoff_remaining_seconds as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_retry_backoffs",
            "Profiles currently in retry backoff.",
            self.snapshot.retry_backoffs as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_transport_backoffs",
            "Profiles currently in transport backoff.",
            self.snapshot.transport_backoffs as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_route_circuits",
            "Profiles currently protected by an open circuit per route.",
            self.snapshot.route_circuits as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_degraded_profiles",
            "Profiles with a non-zero effective health score.",
            self.snapshot.degraded_profiles as f64,
        );
        self.render_broker_gauge(
            "prodex_runtime_broker_degraded_routes",
            "Route-specific health scores that are still degraded.",
            self.snapshot.degraded_routes as f64,
        );
        render_continuation_family(
            &mut self.out,
            "prodex_runtime_broker_continuation_bindings",
            "Continuation bindings grouped by lifecycle.",
            &self.snapshot.continuations,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        render_inflight_family(
            &mut self.out,
            "prodex_runtime_broker_profile_inflight",
            "Current per-profile inflight counts.",
            &self.snapshot.profile_inflight,
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
        );
        self.out
    }

    fn render_info(&mut self) {
        self.push_help(
            "prodex_runtime_broker_info",
            "Static broker metadata and current health attributes.",
        );
        push_type(&mut self.out, "prodex_runtime_broker_info", "gauge");
        push_gauge(
            &mut self.out,
            "prodex_runtime_broker_info",
            labels(&[
                ("broker_key", self.snapshot.broker_key.as_str()),
                ("listen_addr", self.snapshot.listen_addr.as_str()),
                ("current_profile", self.snapshot.current_profile.as_str()),
                (
                    "include_code_review",
                    bool_label(self.snapshot.include_code_review),
                ),
                ("persistence_role", self.snapshot.persistence_role.as_str()),
                (
                    "prodex_version",
                    self.snapshot.prodex_version.as_deref().unwrap_or("-"),
                ),
                (
                    "executable_path",
                    self.snapshot.executable_path.as_deref().unwrap_or("-"),
                ),
                (
                    "executable_sha256",
                    self.snapshot.executable_sha256.as_deref().unwrap_or("-"),
                ),
            ]),
            1.0,
        );
    }

    fn render_lane_families(&mut self) {
        let lanes = [
            ("responses", &self.snapshot.traffic.responses),
            ("compact", &self.snapshot.traffic.compact),
            ("websocket", &self.snapshot.traffic.websocket),
            ("standard", &self.snapshot.traffic.standard),
        ];
        render_lane_family(
            &mut self.out,
            "prodex_runtime_broker_lane_active_requests",
            "Current active requests per broker lane.",
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.active as f64,
        );
        render_lane_family(
            &mut self.out,
            "prodex_runtime_broker_lane_limits",
            "Configured admission limits per broker lane.",
            self.snapshot.broker_key.as_str(),
            self.snapshot.listen_addr.as_str(),
            &lanes,
            |lane| lane.limit as f64,
        );
    }

    fn render_broker_gauge(&mut self, metric_name: &str, help: &str, value: f64) {
        self.push_help(metric_name, help);
        push_type(&mut self.out, metric_name, "gauge");
        push_gauge(
            &mut self.out,
            metric_name,
            labels(&[
                ("broker_key", self.snapshot.broker_key.as_str()),
                ("listen_addr", self.snapshot.listen_addr.as_str()),
            ]),
            value,
        );
    }

    fn push_help(&mut self, metric_name: &str, help: &str) {
        if self.options.include_help {
            push_help(&mut self.out, metric_name, help);
        }
    }
}

#[allow(dead_code)]
pub fn format_runtime_broker_snapshot_summary(snapshot: &RuntimeBrokerSnapshot) -> String {
    format!(
        "broker_key={} listen_addr={} profile={} active_requests={} limits={}/{}/{}/{} degraded_profiles={} degraded_routes={}",
        snapshot.broker_key,
        snapshot.listen_addr,
        snapshot.current_profile,
        snapshot.active_requests,
        snapshot.traffic.responses.limit,
        snapshot.traffic.compact.limit,
        snapshot.traffic.websocket.limit,
        snapshot.traffic.standard.limit,
        snapshot.degraded_profiles,
        snapshot.degraded_routes
    )
}

fn render_lane_family<F>(
    out: &mut String,
    metric_name: &str,
    help: &str,
    broker_key: &str,
    listen_addr: &str,
    lanes: &[(&str, &RuntimeBrokerLaneMetrics)],
    value: F,
) where
    F: Fn(&RuntimeBrokerLaneMetrics) -> f64,
{
    push_help(out, metric_name, help);
    push_type(out, metric_name, "gauge");
    for (lane, snapshot) in lanes {
        push_gauge(
            out,
            metric_name,
            labels(&[
                ("broker_key", broker_key),
                ("listen_addr", listen_addr),
                ("lane", lane),
            ]),
            value(snapshot),
        );
    }
}

fn render_continuation_family(
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
                ("kind", lifecycle),
            ]),
            value as f64,
        );
    }
}

fn render_inflight_family(
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

fn push_help(out: &mut String, metric_name: &str, help: &str) {
    let _ = writeln!(out, "# HELP {metric_name} {help}");
}

fn push_type(out: &mut String, metric_name: &str, metric_type: &str) {
    let _ = writeln!(out, "# TYPE {metric_name} {metric_type}");
}

fn push_gauge(out: &mut String, metric_name: &str, labels: BTreeMap<String, String>, value: f64) {
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

fn labels(items: &[(&str, &str)]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for (key, value) in items {
        map.insert((*key).to_string(), (*value).to_string());
    }
    map
}

fn bool_label(value: bool) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_snapshot() -> RuntimeBrokerSnapshot {
        let mut profile_inflight = BTreeMap::new();
        profile_inflight.insert("main".to_string(), 3);
        profile_inflight.insert("second".to_string(), 1);

        RuntimeBrokerSnapshot {
            broker_key: "broker-123".to_string(),
            listen_addr: "127.0.0.1:8080".to_string(),
            pid: 4242,
            started_at_unix_seconds: 1_715_000_000,
            current_profile: "main".to_string(),
            include_code_review: false,
            persistence_role: "owner".to_string(),
            prodex_version: Some("0.29.0".to_string()),
            executable_path: Some("/tmp/prodex".to_string()),
            executable_sha256: Some("abcd1234".to_string()),
            active_requests: 5,
            active_request_limit: 12,
            local_overload_backoff_remaining_seconds: 0,
            traffic: RuntimeBrokerTrafficMetrics {
                responses: RuntimeBrokerLaneMetrics {
                    active: 3,
                    limit: 9,
                },
                compact: RuntimeBrokerLaneMetrics {
                    active: 1,
                    limit: 3,
                },
                websocket: RuntimeBrokerLaneMetrics {
                    active: 0,
                    limit: 4,
                },
                standard: RuntimeBrokerLaneMetrics {
                    active: 1,
                    limit: 2,
                },
            },
            profile_inflight,
            retry_backoffs: 2,
            transport_backoffs: 1,
            route_circuits: 4,
            degraded_profiles: 1,
            degraded_routes: 2,
            continuations: RuntimeBrokerContinuationMetrics {
                response_bindings: 7,
                turn_state_bindings: 2,
                session_id_bindings: 1,
                warm: 1,
                verified: 3,
                suspect: 2,
                dead: 1,
            },
        }
    }

    #[test]
    fn renders_prometheus_text_with_help_and_labels() {
        let rendered = render_runtime_broker_prometheus(&sample_snapshot());
        assert!(rendered.contains("# HELP prodex_runtime_broker_info"));
        assert!(rendered.contains("# TYPE prodex_runtime_broker_info gauge"));
        assert!(rendered.contains("prodex_runtime_broker_active_requests"));
        assert!(rendered.contains("broker_key=\"broker-123\""));
        assert!(rendered.contains("listen_addr=\"127.0.0.1:8080\""));
        assert!(rendered.contains("current_profile=\"main\""));
        assert!(rendered.contains("prodex_version=\"0.29.0\""));
        assert!(rendered.contains("executable_sha256=\"abcd1234\""));
        assert!(rendered.contains("lane=\"responses\""));
        assert!(rendered.contains("profile=\"main\""));
    }

    #[test]
    fn escapes_label_values_and_keeps_metric_order_stable() {
        let mut snapshot = sample_snapshot();
        snapshot.broker_key = "broker\\\"1\n".to_string();
        snapshot.listen_addr = "127.0.0.1:8080".to_string();
        let rendered = render_runtime_broker_prometheus_with_options(
            &snapshot,
            PrometheusTextOptions {
                include_help: false,
            },
        );

        assert!(rendered.contains("broker_key=\"broker\\\\\\\"1\\n\""));
        let first = rendered
            .lines()
            .find(|line| line.starts_with("prodex_runtime_broker_info"))
            .unwrap();
        let second = rendered
            .lines()
            .find(|line| line.starts_with("prodex_runtime_broker_active_requests"))
            .unwrap();
        assert!(first.starts_with("prodex_runtime_broker_info"));
        assert!(second.starts_with("prodex_runtime_broker_active_requests"));
    }

    #[test]
    fn summary_is_concise_and_machine_readable() {
        let summary = format_runtime_broker_snapshot_summary(&sample_snapshot());
        assert!(summary.contains("broker_key=broker-123"));
        assert!(summary.contains("listen_addr=127.0.0.1:8080"));
        assert!(summary.contains("limits=9/3/4/2"));
        assert!(summary.contains("degraded_profiles=1"));
        assert!(summary.contains("degraded_routes=2"));
    }
}
