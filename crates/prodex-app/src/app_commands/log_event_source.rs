use std::collections::BTreeMap;

pub(super) struct OperationalEventPlan {
    pub(super) source: &'static str,
    pub(super) load_fields: Option<BTreeMap<String, String>>,
}

pub(super) fn operational_event_plan(
    event: &str,
    fields: &BTreeMap<String, String>,
) -> anyhow::Result<Option<OperationalEventPlan>> {
    use prodex_mojo_core::observability::OperationalLoadField;
    let values: Vec<_> = fields
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect();
    let Some(plan) = prodex_mojo_core::observability::operational_stream_plan(event, &values)
        .map_err(|error| anyhow::anyhow!("Mojo operational stream plan failed: {error:?}"))?
    else {
        return Ok(None);
    };
    let load_fields = plan.load_fields.map(|selected| {
        selected
            .into_iter()
            .map(|field| match field {
                OperationalLoadField::Plain(index) => {
                    let (key, value) = values[index];
                    (key.to_string(), value.to_string())
                }
                OperationalLoadField::Endpoint(index) => {
                    let (key, value) = values[index];
                    (key.to_string(), super::safe_endpoint(value))
                }
            })
            .collect()
    });
    Ok(Some(OperationalEventPlan {
        source: plan.source,
        load_fields,
    }))
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
        let request = operational_event_plan("request_captured", &BTreeMap::new())
            .unwrap()
            .unwrap();
        assert_eq!(request.source, "request");
        assert_eq!(request.load_fields, None);

        let mcp = operational_event_plan(
            "compat_request_surface",
            &fields(&[("tool_surface", "mcp,functions")]),
        )
        .unwrap()
        .unwrap();
        assert_eq!(mcp.source, "mcp");
        assert_eq!(mcp.load_fields, None);

        let quiet = operational_event_plan(
            "compat_request_surface",
            &fields(&[
                ("tool_surface", "none"),
                ("continuation", "none"),
                ("family", "codex"),
            ]),
        )
        .unwrap();
        assert!(quiet.is_none());

        let fallback = operational_event_plan(
            "smart_context_prepare_fallback",
            &fields(&[("decision", "pass_through")]),
        )
        .unwrap();
        assert!(fallback.is_none());
    }
}
