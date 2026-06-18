use super::local_rewrite::{
    RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA, RUNTIME_GATEWAY_SCIM_USER_SCHEMA,
    RuntimeLocalRewriteProxyShared, runtime_gateway_generate_virtual_key_token,
};
use super::local_rewrite_gateway_admin_response::RuntimeGatewayAdminError;
use super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;
use super::*;

pub(super) fn runtime_gateway_scim_user_json(
    user: &RuntimeGatewayScimUser,
    shared: &RuntimeLocalRewriteProxyShared,
) -> serde_json::Value {
    let mount_path = shared.mount_path.trim_end_matches('/');
    let location = format!("{mount_path}/prodex/gateway/scim/v2/Users/{}", user.id);
    let mut payload = serde_json::json!({
        "schemas": [RUNTIME_GATEWAY_SCIM_USER_SCHEMA, RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA],
        "id": user.id,
        "userName": user.user_name,
        "tenant_id": user.tenant_id,
        "team_id": user.team_id,
        "project_id": user.project_id,
        "user_id": user.user_id,
        "budget_id": user.budget_id,
        "externalId": user.external_id,
        "displayName": user.display_name,
        "active": user.active,
        "meta": {
            "resourceType": "User",
            "location": location,
            "created": user.created_at_epoch,
            "lastModified": user.updated_at_epoch,
        }
    });
    payload[RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA] = serde_json::json!({
        "tenant_id": user.tenant_id,
        "team_id": user.team_id,
        "project_id": user.project_id,
        "user_id": user.user_id,
        "budget_id": user.budget_id,
        "role": user.role,
        "allowed_key_prefixes": user.allowed_key_prefixes,
    });
    payload
}

pub(super) fn runtime_gateway_generate_scim_user_id() -> Result<String> {
    let token = runtime_gateway_generate_virtual_key_token()?;
    Ok(format!("user_{}", token.trim_start_matches("pk-")))
}

pub(super) fn runtime_gateway_apply_scim_user_patch(
    user: &mut RuntimeGatewayScimUser,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(operations) = body
        .get("Operations")
        .and_then(serde_json::Value::as_array)
        .or_else(|| body.get("operations").and_then(serde_json::Value::as_array))
    {
        for operation in operations {
            runtime_gateway_apply_scim_operation(user, operation)?;
        }
    } else {
        runtime_gateway_apply_scim_user_fields(user, body, partial)?;
    }
    runtime_gateway_validate_scim_user(user)
}

fn runtime_gateway_apply_scim_operation(
    user: &mut RuntimeGatewayScimUser,
    operation: &serde_json::Value,
) -> Result<(), RuntimeGatewayAdminError> {
    let path = operation
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(|value| value.to_ascii_lowercase());
    let value = operation.get("value").unwrap_or(&serde_json::Value::Null);
    let op = operation
        .get("op")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("replace")
        .to_ascii_lowercase();
    if !matches!(op.as_str(), "add" | "replace" | "remove") {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_operation",
            "SCIM operation op must be add, replace, or remove",
        ));
    }
    let Some(path) = path else {
        if value.is_object() {
            return runtime_gateway_apply_scim_user_fields(user, value, true);
        }
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_operation",
            "SCIM operation without path must provide an object value",
        ));
    };
    match path.as_str() {
        "username" | "userName" => {
            user.user_name = runtime_gateway_scim_string_value(value, "userName")?;
        }
        "externalid" => {
            user.external_id = runtime_gateway_scim_optional_string_value(value, "externalId")?;
        }
        "displayname" => {
            user.display_name = runtime_gateway_scim_optional_string_value(value, "displayName")?;
        }
        "active" => {
            user.active = runtime_gateway_scim_bool_value(value, "active")?;
        }
        "role" | "urn:prodex:params:scim:schemas:gateway:2.0:user.role" => {
            user.role = runtime_gateway_scim_optional_role_value(value)?;
        }
        "tenant_id" | "tenantid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.tenant_id" => {
            user.tenant_id = runtime_gateway_scim_optional_string_value(value, "tenant_id")?;
        }
        "team_id" | "teamid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.team_id" => {
            user.team_id = runtime_gateway_scim_optional_string_value(value, "team_id")?;
        }
        "project_id"
        | "projectid"
        | "urn:prodex:params:scim:schemas:gateway:2.0:user.project_id" => {
            user.project_id = runtime_gateway_scim_optional_string_value(value, "project_id")?;
        }
        "user_id" | "userid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.user_id" => {
            user.user_id = runtime_gateway_scim_optional_string_value(value, "user_id")?;
        }
        "budget_id" | "budgetid" | "urn:prodex:params:scim:schemas:gateway:2.0:user.budget_id" => {
            user.budget_id = runtime_gateway_scim_optional_string_value(value, "budget_id")?;
        }
        "allowed_key_prefixes"
        | "allowedkeyprefixes"
        | "urn:prodex:params:scim:schemas:gateway:2.0:user.allowed_key_prefixes" => {
            user.allowed_key_prefixes =
                runtime_gateway_scim_key_prefixes_value(value, "allowed_key_prefixes")?;
        }
        _ => {
            return Err(RuntimeGatewayAdminError::new(
                400,
                "unsupported_scim_path",
                format!("unsupported SCIM path {path}"),
            ));
        }
    }
    Ok(())
}

fn runtime_gateway_apply_scim_user_fields(
    user: &mut RuntimeGatewayScimUser,
    body: &serde_json::Value,
    partial: bool,
) -> Result<(), RuntimeGatewayAdminError> {
    if let Some(value) = body.get("userName").or_else(|| body.get("user_name")) {
        user.user_name = runtime_gateway_scim_string_value(value, "userName")?;
    } else if !partial && user.user_name.trim().is_empty() {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_user_name",
            "SCIM userName is required",
        ));
    }
    if let Some(value) = body.get("externalId").or_else(|| body.get("external_id")) {
        user.external_id = runtime_gateway_scim_optional_string_value(value, "externalId")?;
    } else if !partial {
        user.external_id = None;
    }
    if let Some(value) = body.get("displayName").or_else(|| body.get("display_name")) {
        user.display_name = runtime_gateway_scim_optional_string_value(value, "displayName")?;
    } else if !partial {
        user.display_name = None;
    }
    if let Some(value) = body.get("active") {
        user.active = runtime_gateway_scim_bool_value(value, "active")?;
    } else if !partial {
        user.active = true;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "role") {
        user.role = runtime_gateway_scim_optional_role_value(value)?;
    } else if !partial {
        user.role = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "tenant_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "tenantId"))
    {
        user.tenant_id = runtime_gateway_scim_optional_string_value(value, "tenant_id")?;
    } else if !partial {
        user.tenant_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "team_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "teamId"))
    {
        user.team_id = runtime_gateway_scim_optional_string_value(value, "team_id")?;
    } else if !partial {
        user.team_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "project_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "projectId"))
    {
        user.project_id = runtime_gateway_scim_optional_string_value(value, "project_id")?;
    } else if !partial {
        user.project_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "user_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "userId"))
    {
        user.user_id = runtime_gateway_scim_optional_string_value(value, "user_id")?;
    } else if !partial {
        user.user_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "budget_id")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "budgetId"))
    {
        user.budget_id = runtime_gateway_scim_optional_string_value(value, "budget_id")?;
    } else if !partial {
        user.budget_id = None;
    }
    if let Some(value) = runtime_gateway_scim_prodex_field(body, "allowed_key_prefixes")
        .or_else(|| runtime_gateway_scim_prodex_field(body, "allowedKeyPrefixes"))
    {
        user.allowed_key_prefixes =
            runtime_gateway_scim_key_prefixes_value(value, "allowed_key_prefixes")?;
    } else if !partial {
        user.allowed_key_prefixes = Vec::new();
    }
    Ok(())
}

fn runtime_gateway_scim_prodex_field<'a>(
    body: &'a serde_json::Value,
    field: &str,
) -> Option<&'a serde_json::Value> {
    body.get(field)
        .or_else(|| body.get(RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA)?.get(field))
}

pub(super) fn runtime_gateway_scim_string_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<String, RuntimeGatewayAdminError> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be a non-empty string"),
            )
        })
}

fn runtime_gateway_scim_optional_string_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Option<String>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be a string or null"),
            )
        })
}

fn runtime_gateway_scim_bool_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<bool, RuntimeGatewayAdminError> {
    value.as_bool().ok_or_else(|| {
        RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_field",
            format!("{field} must be a boolean"),
        )
    })
}

fn runtime_gateway_scim_optional_role_value(
    value: &serde_json::Value,
) -> Result<Option<String>, RuntimeGatewayAdminError> {
    if value.is_null() {
        return Ok(None);
    }
    let role = runtime_gateway_scim_string_value(value, "role")?;
    if RuntimeGatewayAdminRole::parse(&role).is_none() {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_role",
            "role must be admin or viewer",
        ));
    }
    Ok(Some(role.to_ascii_lowercase()))
}

fn runtime_gateway_scim_key_prefixes_value(
    value: &serde_json::Value,
    field: &'static str,
) -> Result<Vec<String>, RuntimeGatewayAdminError> {
    let Some(values) = value.as_array() else {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_field",
            format!("{field} must be an array of non-empty strings"),
        ));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| {
            RuntimeGatewayAdminError::new(
                400,
                "invalid_scim_field",
                format!("{field} must be an array of non-empty strings"),
            )
        })
}

fn runtime_gateway_validate_scim_user(
    user: &RuntimeGatewayScimUser,
) -> Result<(), RuntimeGatewayAdminError> {
    let user_name = user.user_name.trim();
    if user_name.is_empty() || user_name.len() > 320 {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_user_name",
            "SCIM userName must be 1-320 characters",
        ));
    }
    if let Some(role) = user.role.as_deref()
        && RuntimeGatewayAdminRole::parse(role).is_none()
    {
        return Err(RuntimeGatewayAdminError::new(
            400,
            "invalid_scim_role",
            "role must be admin or viewer",
        ));
    }
    Ok(())
}
