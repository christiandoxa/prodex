use prodex_application::{
    ApplicationGatewayScimUserMutation, ApplicationGatewayScimUserPatch,
    ApplicationGatewayVirtualKeyMutation, ApplicationGatewayVirtualKeyPatch, ApplicationPatchValue,
    ApplicationScimUserUpdateMode, ApplicationSecretFingerprint,
};
use prodex_domain::{PrincipalId, VirtualKeyId};

use super::RuntimeGatewayAdminError;

const SCIM_PRODEX_SCHEMA: &str = "urn:prodex:params:scim:schemas:gateway:2.0:User";

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_create_mutation(
    id: VirtualKeyId,
    body: &serde_json::Value,
    secret_fingerprint: ApplicationSecretFingerprint,
) -> Result<ApplicationGatewayVirtualKeyMutation, RuntimeGatewayAdminError> {
    let name = body
        .get("name")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(invalid_key_name)?
        .to_string();
    Ok(ApplicationGatewayVirtualKeyMutation::Create {
        id,
        name,
        patch: runtime_gateway_virtual_key_patch(body)?,
        secret_fingerprint,
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_update_mutation(
    id: VirtualKeyId,
    body: &serde_json::Value,
    secret_fingerprint: Option<ApplicationSecretFingerprint>,
) -> Result<ApplicationGatewayVirtualKeyMutation, RuntimeGatewayAdminError> {
    let patch = runtime_gateway_virtual_key_patch(body)?;
    Ok(match secret_fingerprint {
        Some(secret_fingerprint) => ApplicationGatewayVirtualKeyMutation::RotateSecret {
            id,
            patch,
            secret_fingerprint,
        },
        None => ApplicationGatewayVirtualKeyMutation::Update { id, patch },
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_delete_mutation(
    id: VirtualKeyId,
) -> ApplicationGatewayVirtualKeyMutation {
    ApplicationGatewayVirtualKeyMutation::Delete { id }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_virtual_key_patch(
    body: &serde_json::Value,
) -> Result<ApplicationGatewayVirtualKeyPatch, RuntimeGatewayAdminError> {
    Ok(ApplicationGatewayVirtualKeyPatch {
        tenant_id: optional_string_patch(body.get("tenant_id"))?,
        team_id: optional_string_patch(body.get("team_id"))?,
        project_id: optional_string_patch(body.get("project_id"))?,
        user_id: optional_string_patch(body.get("user_id"))?,
        budget_id: optional_string_patch(body.get("budget_id"))?,
        allowed_models: string_array_patch(body.get("allowed_models"), invalid_models)?,
        budget_microusd: budget_patch(body)?,
        request_budget: optional_u64_patch(body.get("request_budget"))?,
        rpm_limit: optional_u64_patch(body.get("rpm_limit"))?,
        tpm_limit: optional_u64_patch(body.get("tpm_limit"))?,
        disabled: optional_key_bool_patch(body.get("disabled"))?,
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_scim_create_mutation(
    id: PrincipalId,
    body: &serde_json::Value,
) -> Result<ApplicationGatewayScimUserMutation, RuntimeGatewayAdminError> {
    Ok(ApplicationGatewayScimUserMutation::Create {
        id,
        patch: runtime_gateway_scim_patch(body)?,
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_scim_update_mutation(
    id: PrincipalId,
    mode: ApplicationScimUserUpdateMode,
    body: &serde_json::Value,
) -> Result<ApplicationGatewayScimUserMutation, RuntimeGatewayAdminError> {
    Ok(ApplicationGatewayScimUserMutation::Update {
        id,
        mode,
        patch: runtime_gateway_scim_patch(body)?,
    })
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_scim_delete_mutation(
    id: PrincipalId,
) -> ApplicationGatewayScimUserMutation {
    ApplicationGatewayScimUserMutation::Delete { id }
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_gateway_scim_patch(
    body: &serde_json::Value,
) -> Result<ApplicationGatewayScimUserPatch, RuntimeGatewayAdminError> {
    if let Some(operations) = body.get("Operations").or_else(|| body.get("operations")) {
        let operations = operations.as_array().ok_or_else(invalid_scim_operation)?;
        let mut patch = ApplicationGatewayScimUserPatch::default();
        for operation in operations {
            apply_scim_operation(&mut patch, operation)?;
        }
        return Ok(patch);
    }
    scim_fields_patch(body)
}

fn scim_fields_patch(
    body: &serde_json::Value,
) -> Result<ApplicationGatewayScimUserPatch, RuntimeGatewayAdminError> {
    Ok(ApplicationGatewayScimUserPatch {
        tenant_id: optional_string_patch(prodex_field(body, &["tenant_id", "tenantId"]))?,
        user_name: optional_string_patch(field(body, &["userName", "user_name"]))?,
        external_id: optional_string_patch(field(body, &["externalId", "external_id"]))?,
        display_name: optional_string_patch(field(body, &["displayName", "display_name"]))?,
        active: optional_bool_patch(body.get("active"))?,
        role: optional_string_patch(prodex_field(body, &["role"]))?,
        team_id: optional_string_patch(prodex_field(body, &["team_id", "teamId"]))?,
        project_id: optional_string_patch(prodex_field(body, &["project_id", "projectId"]))?,
        user_id: optional_string_patch(prodex_field(body, &["user_id", "userId"]))?,
        budget_id: optional_string_patch(prodex_field(body, &["budget_id", "budgetId"]))?,
        allowed_key_prefixes: string_array_patch(
            prodex_field(body, &["allowed_key_prefixes", "allowedKeyPrefixes"]),
            invalid_scim_field,
        )?,
    })
}

fn apply_scim_operation(
    patch: &mut ApplicationGatewayScimUserPatch,
    operation: &serde_json::Value,
) -> Result<(), RuntimeGatewayAdminError> {
    let op = operation
        .get("op")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("replace")
        .to_ascii_lowercase();
    if !matches!(op.as_str(), "add" | "replace" | "remove") {
        return Err(invalid_scim_operation());
    }
    let value = operation.get("value").unwrap_or(&serde_json::Value::Null);
    let Some(path) = operation.get("path").and_then(serde_json::Value::as_str) else {
        let value = value.as_object().ok_or_else(invalid_scim_operation)?;
        let fields = scim_fields_patch(&serde_json::Value::Object(value.clone()))?;
        merge_scim_patch(patch, fields);
        return Ok(());
    };
    let remove = op == "remove";
    match path.to_ascii_lowercase().as_str() {
        "username" => patch.user_name = scim_string_operation(value, remove)?,
        "externalid" => patch.external_id = scim_string_operation(value, remove)?,
        "displayname" => patch.display_name = scim_string_operation(value, remove)?,
        "active" => patch.active = scim_bool_operation(value, remove)?,
        "role" | "urn:prodex:params:scim:schemas:gateway:2.0:user.role" => {
            patch.role = scim_string_operation(value, remove)?;
        }
        "tenant_id" | "tenantid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.tenant_id" => {
            patch.tenant_id = scim_string_operation(value, remove)?;
        }
        "team_id" | "teamid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.team_id" => {
            patch.team_id = scim_string_operation(value, remove)?;
        }
        "project_id"
        | "projectid"
        | "urn:prodex:params:scim:schemas:gateway:2.0:user.project_id" => {
            patch.project_id = scim_string_operation(value, remove)?;
        }
        "user_id" | "userid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.user_id" => {
            patch.user_id = scim_string_operation(value, remove)?;
        }
        "budget_id" | "budgetid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.budget_id" => {
            patch.budget_id = scim_string_operation(value, remove)?;
        }
        "allowed_key_prefixes"
        | "allowedkeyprefixes"
        | "urn:prodex:params:scim:schemas:gateway:2.0:user.allowed_key_prefixes" => {
            patch.allowed_key_prefixes = if remove {
                ApplicationPatchValue::Clear
            } else {
                string_array_patch(Some(value), invalid_scim_field)?
            };
        }
        _ => return Err(unsupported_scim_path()),
    }
    Ok(())
}

fn merge_scim_patch(
    target: &mut ApplicationGatewayScimUserPatch,
    source: ApplicationGatewayScimUserPatch,
) {
    macro_rules! merge {
        ($field:ident) => {
            if source.$field != ApplicationPatchValue::Unchanged {
                target.$field = source.$field;
            }
        };
    }
    merge!(tenant_id);
    merge!(user_name);
    merge!(external_id);
    merge!(display_name);
    merge!(active);
    merge!(role);
    merge!(team_id);
    merge!(project_id);
    merge!(user_id);
    merge!(budget_id);
    merge!(allowed_key_prefixes);
}

fn optional_string_patch(
    value: Option<&serde_json::Value>,
) -> Result<ApplicationPatchValue<String>, RuntimeGatewayAdminError> {
    match value {
        None => Ok(ApplicationPatchValue::Unchanged),
        Some(serde_json::Value::Null) => Ok(ApplicationPatchValue::Clear),
        Some(serde_json::Value::String(value)) => Ok(ApplicationPatchValue::Set(value.clone())),
        Some(_) => Err(invalid_scim_field()),
    }
}

fn optional_bool_patch(
    value: Option<&serde_json::Value>,
) -> Result<ApplicationPatchValue<bool>, RuntimeGatewayAdminError> {
    match value {
        None => Ok(ApplicationPatchValue::Unchanged),
        Some(serde_json::Value::Null) => Ok(ApplicationPatchValue::Clear),
        Some(serde_json::Value::Bool(value)) => Ok(ApplicationPatchValue::Set(*value)),
        Some(_) => Err(invalid_scim_field()),
    }
}

fn optional_key_bool_patch(
    value: Option<&serde_json::Value>,
) -> Result<ApplicationPatchValue<bool>, RuntimeGatewayAdminError> {
    match value {
        None => Ok(ApplicationPatchValue::Unchanged),
        Some(serde_json::Value::Null) => Ok(ApplicationPatchValue::Clear),
        Some(serde_json::Value::Bool(value)) => Ok(ApplicationPatchValue::Set(*value)),
        Some(_) => Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_disabled",
            "disabled must be a boolean",
        )),
    }
}

fn optional_u64_patch(
    value: Option<&serde_json::Value>,
) -> Result<ApplicationPatchValue<u64>, RuntimeGatewayAdminError> {
    match value {
        None => Ok(ApplicationPatchValue::Unchanged),
        Some(serde_json::Value::Null) => Ok(ApplicationPatchValue::Clear),
        Some(value) => value
            .as_u64()
            .map(ApplicationPatchValue::Set)
            .ok_or_else(invalid_numeric),
    }
}

fn string_array_patch(
    value: Option<&serde_json::Value>,
    error: fn() -> RuntimeGatewayAdminError,
) -> Result<ApplicationPatchValue<Vec<String>>, RuntimeGatewayAdminError> {
    match value {
        None => Ok(ApplicationPatchValue::Unchanged),
        Some(serde_json::Value::Null) => Ok(ApplicationPatchValue::Clear),
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .map(|value| value.as_str().map(str::to_string).ok_or_else(error))
            .collect::<Result<Vec<_>, _>>()
            .map(ApplicationPatchValue::Set),
        Some(_) => Err(error()),
    }
}

fn budget_patch(
    body: &serde_json::Value,
) -> Result<ApplicationPatchValue<u64>, RuntimeGatewayAdminError> {
    if let Some(value) = body.get("budget_microusd") {
        return optional_u64_patch(Some(value));
    }
    let Some(value) = body.get("budget_usd") else {
        return Ok(ApplicationPatchValue::Unchanged);
    };
    if value.is_null() {
        return Ok(ApplicationPatchValue::Clear);
    }
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| {
            ApplicationPatchValue::Set(
                (value * 1_000_000.0).round().clamp(1.0, u64::MAX as f64) as u64
            )
        })
        .ok_or_else(invalid_numeric)
}

fn scim_string_operation(
    value: &serde_json::Value,
    remove: bool,
) -> Result<ApplicationPatchValue<String>, RuntimeGatewayAdminError> {
    if remove {
        Ok(ApplicationPatchValue::Clear)
    } else {
        optional_string_patch(Some(value))
    }
}

fn scim_bool_operation(
    value: &serde_json::Value,
    remove: bool,
) -> Result<ApplicationPatchValue<bool>, RuntimeGatewayAdminError> {
    if remove {
        Ok(ApplicationPatchValue::Clear)
    } else {
        optional_bool_patch(Some(value))
    }
}

fn field<'a>(body: &'a serde_json::Value, names: &[&str]) -> Option<&'a serde_json::Value> {
    names.iter().find_map(|name| body.get(*name))
}

fn prodex_field<'a>(body: &'a serde_json::Value, names: &[&str]) -> Option<&'a serde_json::Value> {
    field(body, names).or_else(|| {
        let extension = body.get(SCIM_PRODEX_SCHEMA)?;
        field(extension, names)
    })
}

fn invalid_key_name() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_gateway_key_name",
        "gateway virtual key name is required",
    )
}

fn invalid_models() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_allowed_models",
        "allowed_models must be an array of strings",
    )
}

fn invalid_numeric() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(
        400,
        "invalid_numeric_limit",
        "gateway numeric limit must be a u64 or null",
    )
}

fn invalid_scim_field() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(400, "invalid_scim_field", "SCIM field is invalid")
}

fn invalid_scim_operation() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(400, "invalid_scim_operation", "SCIM operation is invalid")
}

fn unsupported_scim_path() -> RuntimeGatewayAdminError {
    RuntimeGatewayAdminError::new(400, "unsupported_scim_path", "unsupported SCIM path")
}
