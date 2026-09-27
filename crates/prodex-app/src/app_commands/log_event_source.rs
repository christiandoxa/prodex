use std::collections::BTreeMap;

pub(super) struct OperationalEventPlan {
    pub(super) source: Option<&'static str>,
    pub(super) interesting: bool,
}

pub(super) fn operational_event_plan(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> anyhow::Result<OperationalEventPlan> {
    const SOURCES: [Option<&str>; 20] = [
        None,
        Some("request"),
        Some("mcp"),
        Some("agent"),
        Some("route"),
        Some("quota"),
        Some("retry"),
        Some("backoff"),
        Some("health"),
        Some("error"),
        Some("model"),
        Some("upstream"),
        Some("stream"),
        Some("response"),
        Some("terminal"),
        Some("tool"),
        Some("load"),
        Some("smart"),
        Some("compact"),
        Some("event"),
    ];
    let value = prodex_mojo_core::observability::operational_event_plan(
        event,
        fields.get("tool_surface").map(String::as_str),
        fields.get("continuation").map(String::as_str),
        fields.get("family").map(String::as_str),
        fields.get("decision").map(String::as_str),
    )
    .map_err(|error| anyhow::anyhow!("Mojo operational event plan failed: {error:?}"))?;
    let source = usize::try_from(value.source)
        .ok()
        .and_then(|index| SOURCES.get(index))
        .copied()
        .flatten();
    Ok(OperationalEventPlan {
        source,
        interesting: value.interesting,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(values: &[(&str, &str)]) -> BTreeMap<String, String> {
        values
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect()
    }

    #[test]
    fn operational_event_plan_keeps_representative_mojo_contracts() {
        let request = operational_event_plan("request_captured", &BTreeMap::new()).unwrap();
        assert_eq!(request.source, Some("request"));
        assert!(request.interesting);

        let mcp = operational_event_plan(
            "compat_request_surface",
            &fields(&[("tool_surface", "mcp,functions")]),
        )
        .unwrap();
        assert_eq!(mcp.source, Some("mcp"));
        assert!(mcp.interesting);

        let quiet = operational_event_plan(
            "compat_request_surface",
            &fields(&[
                ("tool_surface", "none"),
                ("continuation", "none"),
                ("family", "codex"),
            ]),
        )
        .unwrap();
        assert_eq!(quiet.source, None);
        assert!(!quiet.interesting);

        let fallback = operational_event_plan(
            "smart_context_prepare_fallback",
            &fields(&[("decision", "pass_through")]),
        )
        .unwrap();
        assert_eq!(fallback.source, Some("event"));
        assert!(!fallback.interesting);
    }
}
