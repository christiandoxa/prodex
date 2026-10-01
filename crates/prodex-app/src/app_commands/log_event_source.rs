use std::collections::BTreeMap;

pub(super) struct OperationalEventPlan {
    pub(super) source: Option<&'static str>,
    pub(super) interesting: bool,
}

pub(super) fn operational_event_plan(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> anyhow::Result<OperationalEventPlan> {
    let value = prodex_mojo_core::observability::operational_event_plan(
        event,
        fields.get("tool_surface").map(String::as_str),
        fields.get("continuation").map(String::as_str),
        fields.get("family").map(String::as_str),
        fields.get("decision").map(String::as_str),
    )
    .map_err(|error| anyhow::anyhow!("Mojo operational event plan failed: {error:?}"))?;
    let source = prodex_mojo_core::observability::operational_event_source_label(value.source)
        .map_err(|error| {
            anyhow::anyhow!("Mojo operational event source label failed: {error:?}")
        })?;
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
