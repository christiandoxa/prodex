pub const RUNTIME_DOCTOR_FACETS: &[&str] = &[
    "lane",
    "route",
    "profile",
    "reason",
    "transport",
    "provider",
    "family",
    "client",
    "tool_surface",
    "continuation",
    "origin",
    "warning",
    "quota_source",
    "quota_band",
    "five_hour_status",
    "weekly_status",
    "affinity",
    "context",
    "event",
    "stage",
    "state",
    "source",
    "request_shape",
    "exit",
    "mode",
    "tier",
    "decision",
    "reasons",
    "token_usage_source",
    "self_check",
    "budget_mode",
    "policy_reasons",
];

#[cfg(feature = "runtime-log-mojo")]
pub(crate) fn runtime_doctor_marker_is_known(value: &str) -> bool {
    prodex_mojo_core::rich::runtime_doctor_marker_known(value)
        .expect("Mojo runtime-doctor marker classifier returned invalid output")
}
