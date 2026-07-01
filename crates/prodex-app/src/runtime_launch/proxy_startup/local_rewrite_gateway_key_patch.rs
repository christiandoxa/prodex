use super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::local_rewrite_gateway_scim::runtime_gateway_scim_optional_scope_value;
use super::local_rewrite_gateway_store_types::RuntimeGatewayStoredVirtualKey;

pub(super) fn runtime_gateway_apply_virtual_key_patch(
    record: &mut RuntimeGatewayStoredVirtualKey,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(value) = body.get("tenant_id") {
        record.tenant_id = runtime_gateway_scim_optional_scope_value(value, "tenant_id")?;
    } else if !partial {
        record.tenant_id = None;
    }
    for (field, target) in [
        ("team_id", &mut record.team_id),
        ("project_id", &mut record.project_id),
        ("user_id", &mut record.user_id),
        ("budget_id", &mut record.budget_id),
    ] {
        if let Some(value) = body.get(field) {
            *target = runtime_gateway_scim_optional_scope_value(value, field)?;
        } else if !partial {
            *target = None;
        }
    }

    if let Some(models) = body.get("allowed_models") {
        let Some(values) = models.as_array() else {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "invalid_allowed_models",
                "allowed_models must be an array of strings",
            ));
        };
        record.allowed_models = values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .filter(|value| !value.chars().any(char::is_whitespace))
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                RuntimeGatewayAdminError::new(
                    400,
                    "invalid_allowed_models",
                    "allowed_models must be an array of strings",
                )
            })?
            .into_iter()
            .map(str::to_string)
            .collect();
    } else if !partial {
        record.allowed_models = Vec::new();
    }

    if let Some(value) = body.get("budget_microusd") {
        record.budget_microusd = runtime_gateway_optional_u64(value, "budget_microusd")?;
    } else if let Some(value) = body.get("budget_usd") {
        record.budget_microusd = runtime_gateway_optional_f64_microusd(value, "budget_usd")?;
    } else if !partial {
        record.budget_microusd = None;
    }
    for (field, target) in [
        ("request_budget", &mut record.request_budget),
        ("rpm_limit", &mut record.rpm_limit),
        ("tpm_limit", &mut record.tpm_limit),
    ] {
        if let Some(value) = body.get(field) {
            *target = runtime_gateway_optional_u64(value, field)?;
        } else if !partial {
            *target = None;
        }
    }
    if let Some(value) = body.get("disabled") {
        let Some(disabled) = value.as_bool() else {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "invalid_disabled",
                "disabled must be a boolean",
            ));
        };
        record.disabled = Some(disabled);
    } else if !partial {
        record.disabled = Some(false);
    }
    Ok(())
}

fn runtime_gateway_optional_u64(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<u64>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value.as_u64().map(Some).ok_or_else(|| {
        RuntimeGatewayAdminError::new(
            400,
            "invalid_numeric_limit",
            format!("{field} must be a u64 or null"),
        )
    })
}

fn runtime_gateway_optional_f64_microusd(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<u64>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| Some((value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64))
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_numeric_limit",
                format!("{field} must be a non-negative number or null"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored_key() -> RuntimeGatewayStoredVirtualKey {
        RuntimeGatewayStoredVirtualKey {
            name: "team-a".to_string(),
            token_hash_base64: "hash".to_string(),
            tenant_id: Some("tenant-a".to_string()),
            team_id: None,
            project_id: None,
            user_id: None,
            budget_id: None,
            allowed_models: Vec::new(),
            budget_microusd: None,
            request_budget: None,
            rpm_limit: None,
            tpm_limit: None,
            disabled: Some(false),
            created_at_epoch: 1,
            updated_at_epoch: 1,
        }
    }

    #[test]
    fn virtual_key_scope_patch_rejects_whitespace_instead_of_trimming() {
        let mut record = stored_key();

        let err = runtime_gateway_apply_virtual_key_patch(
            &mut record,
            &serde_json::json!({"tenant_id": " tenant-b "}),
            true,
        )
        .unwrap_err();

        assert_eq!(err.test_status(), 400);
        assert_eq!(err.test_code(), "invalid_scim_field");
        assert_eq!(record.tenant_id.as_deref(), Some("tenant-a"));
    }

    #[test]
    fn virtual_key_scope_patch_accepts_null_unset() {
        let mut record = stored_key();

        let result = runtime_gateway_apply_virtual_key_patch(
            &mut record,
            &serde_json::json!({"tenant_id": null}),
            true,
        );

        assert!(result.is_ok());
        assert_eq!(record.tenant_id, None);
    }

    #[test]
    fn virtual_key_allowed_models_reject_whitespace_instead_of_trimming() {
        let mut record = stored_key();

        let err = runtime_gateway_apply_virtual_key_patch(
            &mut record,
            &serde_json::json!({"allowed_models": [" gpt-5 "]}),
            true,
        )
        .unwrap_err();

        assert_eq!(err.test_status(), 400);
        assert_eq!(err.test_code(), "invalid_allowed_models");
        assert!(record.allowed_models.is_empty());
    }
}
