use super::local_rewrite::{
    RUNTIME_GATEWAY_SCIM_PRODEX_SCHEMA, RUNTIME_GATEWAY_SCIM_USER_SCHEMA,
    RuntimeLocalRewriteProxyShared,
};
use super::local_rewrite_gateway_store_types::RuntimeGatewayScimUser;

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
            "version": runtime_gateway_scim_user_etag(user),
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

pub(super) fn runtime_gateway_scim_user_etag(user: &RuntimeGatewayScimUser) -> String {
    format!("\"gateway-scim-user-{}\"", user.updated_at_epoch)
}
