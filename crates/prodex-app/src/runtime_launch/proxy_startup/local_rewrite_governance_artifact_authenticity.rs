use aws_lc_rs::signature::{ED25519, UnparsedPublicKey};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use prodex_runtime_policy::RuntimePolicyGovernanceSettings;
use prodex_storage::{
    GovernanceArtifactAuthenticity, GovernanceArtifactValidationInput,
    governance_support::artifact_signature_message,
};

use super::local_rewrite::RuntimeLocalRewriteProxyShared;

fn decode_base64(value: &str) -> Option<Vec<u8>> {
    [STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD]
        .into_iter()
        .find_map(|engine| engine.decode(value).ok())
}

pub(super) fn governance_artifact_authenticity_is_valid(
    governance: &RuntimePolicyGovernanceSettings,
    input: &GovernanceArtifactValidationInput<'_>,
) -> bool {
    let Some(authenticity) = input.authenticity else {
        return !matches!(
            governance.mode,
            prodex_runtime_policy::RuntimeGovernanceMode::EnterpriseEnforce
                | prodex_runtime_policy::RuntimeGovernanceMode::BankEnforce
        );
    };
    if !authenticity.is_well_formed() {
        return false;
    }
    let Some(verifier) = governance
        .artifact_verifiers
        .iter()
        .find(|candidate| candidate.key_id == authenticity.key_id)
    else {
        return false;
    };
    let Some(public_key) = decode_base64(&verifier.ed25519_public_key_base64) else {
        return false;
    };
    let Some(signature) = decode_base64(&authenticity.signature_base64) else {
        return false;
    };
    if public_key.len() != 32 || signature.len() != 64 {
        return false;
    }
    let message = artifact_signature_message(
        input.tenant_id,
        input.kind,
        input.revision_id,
        input.compiled_artifact,
    );
    UnparsedPublicKey::new(&ED25519, public_key)
        .verify(&message, &signature)
        .is_ok()
}

pub(super) fn runtime_governance_artifact_authenticity_is_valid(
    shared: &RuntimeLocalRewriteProxyShared,
    input: &GovernanceArtifactValidationInput<'_>,
) -> bool {
    governance_artifact_authenticity_is_valid(
        &shared.runtime_shared.runtime_config.governance_policy,
        input,
    )
}

pub(super) fn governance_artifact_signature_payload_base64(
    input: &GovernanceArtifactValidationInput<'_>,
) -> String {
    STANDARD.encode(artifact_signature_message(
        input.tenant_id,
        input.kind,
        input.revision_id,
        input.compiled_artifact,
    ))
}

pub(super) fn parse_governance_artifact_authenticity(
    body: &serde_json::Value,
) -> Result<Option<GovernanceArtifactAuthenticity>, ()> {
    let Some(value) = body.get("authenticity") else {
        return Ok(None);
    };
    let object = value.as_object().ok_or(())?;
    if object.len() != 2 || !object.contains_key("key_id") || !object.contains_key("signature") {
        return Err(());
    }
    let authenticity = GovernanceArtifactAuthenticity {
        key_id: object
            .get("key_id")
            .and_then(serde_json::Value::as_str)
            .ok_or(())?
            .to_string(),
        signature_base64: object
            .get("signature")
            .and_then(serde_json::Value::as_str)
            .ok_or(())?
            .to_string(),
    };
    authenticity
        .is_well_formed()
        .then_some(Some(authenticity))
        .ok_or(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::signature::{Ed25519KeyPair, KeyPair};
    use prodex_runtime_policy::{RuntimeGovernanceMode, RuntimePolicyGovernanceArtifactVerifier};
    use prodex_storage::GovernanceArtifactKind;

    fn signed_input<'a>(
        tenant_id: prodex_domain::TenantId,
        artifact: &'a [u8],
        authenticity: &'a GovernanceArtifactAuthenticity,
    ) -> GovernanceArtifactValidationInput<'a> {
        GovernanceArtifactValidationInput {
            tenant_id,
            kind: GovernanceArtifactKind::Policy,
            revision_id: "policy-1",
            compiled_artifact: artifact,
            authenticity: Some(authenticity),
        }
    }

    #[test]
    fn detached_signature_binds_artifact_and_identity() {
        let pair = Ed25519KeyPair::from_seed_unchecked(&[7_u8; 32]).unwrap();
        let tenant_id = prodex_domain::TenantId::from_uuid(
            uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap(),
        );
        let artifact = br#"{"revision":"policy-1"}"#;
        let message = artifact_signature_message(
            tenant_id,
            GovernanceArtifactKind::Policy,
            "policy-1",
            artifact,
        );
        let authenticity = GovernanceArtifactAuthenticity {
            key_id: "release-2026-01".to_string(),
            signature_base64: STANDARD.encode(pair.sign(&message).as_ref()),
        };
        let governance = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::EnterpriseEnforce,
            artifact_verifiers: vec![RuntimePolicyGovernanceArtifactVerifier {
                key_id: authenticity.key_id.clone(),
                ed25519_public_key_base64: STANDARD.encode(pair.public_key().as_ref()),
            }],
            ..RuntimePolicyGovernanceSettings::default()
        };

        assert!(governance_artifact_authenticity_is_valid(
            &governance,
            &signed_input(tenant_id, artifact, &authenticity),
        ));
        assert!(!governance_artifact_authenticity_is_valid(
            &governance,
            &signed_input(tenant_id, br#"{"revision":"tampered"}"#, &authenticity),
        ));
    }

    #[test]
    fn unsigned_artifacts_are_legacy_only() {
        let tenant_id = prodex_domain::TenantId::new();
        let input = GovernanceArtifactValidationInput {
            tenant_id,
            kind: GovernanceArtifactKind::Policy,
            revision_id: "policy-1",
            compiled_artifact: b"{}",
            authenticity: None,
        };
        let governance = RuntimePolicyGovernanceSettings::default();
        assert!(governance_artifact_authenticity_is_valid(
            &governance,
            &input
        ));
        let governance = RuntimePolicyGovernanceSettings {
            mode: RuntimeGovernanceMode::EnterpriseEnforce,
            ..RuntimePolicyGovernanceSettings::default()
        };
        assert!(!governance_artifact_authenticity_is_valid(
            &governance,
            &input
        ));
    }

    #[test]
    fn authenticity_json_is_strict_and_redaction_safe() {
        let parsed = parse_governance_artifact_authenticity(&serde_json::json!({
            "authenticity": {"key_id": "release-1", "signature": "AQID"}
        }))
        .unwrap()
        .unwrap();
        assert_eq!(parsed.key_id, "release-1");
        assert!(!format!("{parsed:?}").contains("AQID"));
        assert!(
            parse_governance_artifact_authenticity(&serde_json::json!({
                "authenticity": {"key_id": "release-1", "signature": "AQID", "extra": true}
            }))
            .is_err()
        );
    }
}
