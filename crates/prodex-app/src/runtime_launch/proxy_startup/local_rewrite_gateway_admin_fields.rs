pub(super) fn runtime_gateway_admin_key_patch_fields(body: &serde_json::Value) -> Vec<String> {
    [
        "tenant_id",
        "token",
        "rotate",
        "allowed_models",
        "budget_microusd",
        "budget_usd",
        "request_budget",
        "rpm_limit",
        "tpm_limit",
        "disabled",
    ]
    .into_iter()
    .filter(|field| body.get(field).is_some())
    .map(str::to_string)
    .collect()
}

pub(super) fn runtime_gateway_admin_request_dimension(
    body: &serde_json::Value,
    field: &str,
    admin_scope: Option<&str>,
) -> Option<String> {
    runtime_gateway_admin_optional_dimension_from_body(body, field, admin_scope)
}

pub(super) fn runtime_gateway_admin_optional_dimension_from_body(
    body: &serde_json::Value,
    field: &str,
    current: Option<&str>,
) -> Option<String> {
    body.get(field)
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| current.map(str::to_string))
}

pub(super) fn runtime_gateway_admin_body_tenant_id(
    body: &serde_json::Value,
) -> Option<Option<String>> {
    body.get("tenant_id").map(|value| {
        value
            .as_str()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub(super) fn runtime_gateway_validate_virtual_key_name(name: &str) -> Result<(), &'static str> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err("gateway virtual key name must use 1-128 ASCII letters, numbers, '.', '-', or '_'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_dimension_trims_strings_and_preserves_current_when_absent() {
        let body = serde_json::json!({"team_id": " platform "});
        assert_eq!(
            runtime_gateway_admin_optional_dimension_from_body(&body, "team_id", None).as_deref(),
            Some("platform")
        );
        assert_eq!(
            runtime_gateway_admin_optional_dimension_from_body(&body, "project_id", Some("old"))
                .as_deref(),
            Some("old")
        );
    }

    #[test]
    fn virtual_key_name_accepts_only_stable_ascii_names() {
        assert!(runtime_gateway_validate_virtual_key_name("team.alpha-1").is_ok());
        assert!(runtime_gateway_validate_virtual_key_name("").is_err());
        assert!(runtime_gateway_validate_virtual_key_name("bad space").is_err());
    }
}
