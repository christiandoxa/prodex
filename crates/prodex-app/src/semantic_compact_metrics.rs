use std::fmt::Write;
use std::sync::atomic::{AtomicU64, Ordering};

const PROVIDERS: [&str; 2] = ["gemini", "kiro"];
const MODES: [&str; 2] = ["semantic", "local-fallback"];
const REASONS: [&str; 6] = [
    "timeout",
    "unsupported",
    "unavailable",
    "invalid-response",
    "provider-error",
    "local-policy",
];
static TOTAL: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
static FALLBACK_TOTAL: [AtomicU64; 12] = [const { AtomicU64::new(0) }; 12];

pub(crate) fn record_semantic_compact(provider: &str, mode: &str, reason: Option<&str>) {
    let Some(provider_index) = PROVIDERS.iter().position(|value| *value == provider) else {
        return;
    };
    let Some(mode_index) = MODES.iter().position(|value| *value == mode) else {
        return;
    };
    TOTAL[provider_index * MODES.len() + mode_index].fetch_add(1, Ordering::Relaxed);
    if mode == "local-fallback" {
        let bounded_reason =
            reason
                .filter(|reason| REASONS.contains(reason))
                .unwrap_or(if reason.is_some() {
                    "provider-error"
                } else {
                    "local-policy"
                });
        let reason_index = REASONS
            .iter()
            .position(|reason| *reason == bounded_reason)
            .expect("bounded compact reason must be registered");
        FALLBACK_TOTAL[provider_index * REASONS.len() + reason_index]
            .fetch_add(1, Ordering::Relaxed);
    }
}

pub(crate) fn render_semantic_compact_metrics() -> String {
    let mut output = String::from(
        "# HELP prodex_semantic_compact_total Semantic compact results by provider and mode.\n\
# TYPE prodex_semantic_compact_total counter\n",
    );
    for (provider_index, provider) in PROVIDERS.iter().enumerate() {
        for (mode_index, mode) in MODES.iter().enumerate() {
            let value = TOTAL[provider_index * MODES.len() + mode_index].load(Ordering::Relaxed);
            let _ = writeln!(
                output,
                "prodex_semantic_compact_total{{provider=\"{provider}\",mode=\"{mode}\"}} {value}"
            );
        }
    }
    output.push_str(
        "# HELP prodex_semantic_compact_fallback_total Semantic compact fallbacks by provider and bounded reason.\n\
# TYPE prodex_semantic_compact_fallback_total counter\n",
    );
    for (provider_index, provider) in PROVIDERS.iter().enumerate() {
        for (reason_index, reason) in REASONS.iter().enumerate() {
            let value = FALLBACK_TOTAL[provider_index * REASONS.len() + reason_index]
                .load(Ordering::Relaxed);
            let _ = writeln!(
                output,
                "prodex_semantic_compact_fallback_total{{provider=\"{provider}\",reason=\"{reason}\"}} {value}"
            );
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_bounded_and_raw_errors_are_ignored() {
        record_semantic_compact("gemini", "local-fallback", Some("timeout"));
        record_semantic_compact("gemini", "local-fallback", Some("secret raw error"));
        record_semantic_compact("kiro", "semantic", None);
        let rendered = render_semantic_compact_metrics();
        assert!(rendered.contains("prodex_semantic_compact_total"));
        assert!(rendered.contains("reason=\"timeout\""));
        assert!(rendered.contains("reason=\"provider-error\""));
        assert!(!rendered.contains("provider=\"kiro\",mode=\"semantic\"} 0\n"));
        assert!(!rendered.contains("secret raw error"));
    }
}
