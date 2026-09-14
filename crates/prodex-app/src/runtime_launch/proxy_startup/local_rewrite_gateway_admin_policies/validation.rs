use super::*;

fn governance_artifact_is_valid(
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
    resource: RuntimeGovernanceResource,
    expected_revision_id: Option<&str>,
    artifact: &[u8],
) -> bool {
    match resource {
        RuntimeGovernanceResource::Policy => crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                artifact,
                shared.runtime_shared.runtime_config.governance.mode,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id.is_none_or(|expected| {
                    snapshot.application.policy.revision().to_string() == expected
                })
            }),
        RuntimeGovernanceResource::ClassificationRules => super::local_rewrite_classification_rules::compile_runtime_classification_rules_artifact(
                tenant_id,
                artifact,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id.is_none_or(|expected| {
                    snapshot.classification_rules().revision().as_str() == expected
                })
            }),
        RuntimeGovernanceResource::ProviderRegistry => super::local_rewrite_provider_registry::compile_runtime_gateway_provider_registry_artifact_for_deployment(
                artifact,
                &shared.provider,
                shared.provider_credential.as_ref(),
                shared.runtime_shared.runtime_config.governance.mode,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id
                    .is_none_or(|expected| snapshot.revision().to_string() == expected)
            }),
        RuntimeGovernanceResource::RoutingScores => super::local_rewrite_provider_registry::compile_runtime_gateway_routing_scores_artifact(
                artifact,
            )
            .is_ok_and(|snapshot| {
                expected_revision_id
                    .is_none_or(|expected| snapshot.revision.to_string() == expected)
            }),
    }
}

pub(super) fn governance_artifact_validation_is_valid(
    shared: &RuntimeLocalRewriteProxyShared,
    resource: RuntimeGovernanceResource,
    input: &GovernanceArtifactValidationInput<'_>,
) -> bool {
    input.kind == resource.kind()
        && runtime_governance_artifact_authenticity_is_valid(shared, input)
        && governance_artifact_is_valid(
            shared,
            input.tenant_id,
            resource,
            Some(input.revision_id),
            input.compiled_artifact,
        )
}

pub(super) fn validate_response(
    captured: &RuntimeProxyRequest,
    shared: &RuntimeLocalRewriteProxyShared,
    tenant_id: prodex_domain::TenantId,
    resource: RuntimeGovernanceResource,
) -> tiny_http::ResponseBox {
    let body = match runtime_gateway_admin_json_body(captured) {
        Ok(body) => body,
        Err(response) => return response,
    };
    let Some(artifact) = body.get("artifact") else {
        return invalid_request();
    };
    let Ok(bytes) = serde_json::to_vec(artifact) else {
        return invalid_request();
    };
    if bytes.is_empty() || bytes.len() > prodex_storage::MAX_COMPILED_GOVERNANCE_ARTIFACT_BYTES {
        return invalid_request();
    }
    if !governance_artifact_is_valid(shared, tenant_id, resource, None, &bytes) {
        return invalid_request();
    }
    let signing = match body.get("revision_id") {
        Some(value) => {
            let Some(revision_id) = value.as_str() else {
                return invalid_request();
            };
            let input = GovernanceArtifactValidationInput {
                tenant_id,
                kind: resource.kind(),
                revision_id,
                compiled_artifact: &bytes,
                authenticity: None,
            };
            if prodex_storage::governance_support::validate_governance_revision_id(
                resource.kind(),
                revision_id,
            )
            .is_err()
                || !governance_artifact_is_valid(
                    shared,
                    tenant_id,
                    resource,
                    Some(revision_id),
                    &bytes,
                )
            {
                return invalid_request();
            }
            Some(serde_json::json!({
                "algorithm": "ed25519",
                "key_selection": "governance.artifact_verifiers.key_id",
                "payload_base64": governance_artifact_signature_payload_base64(&input),
            }))
        }
        None => None,
    };
    runtime_gateway_admin_json_response(
        200,
        serde_json::json!({
            "object": format!("governance.{}_validation", resource.label()),
            "valid": true,
            "fingerprint": artifact_fingerprint(&bytes),
            "signing": signing,
        }),
    )
}
