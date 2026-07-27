use super::codec::artifact_kind_label;
use crate::{GovernanceActivationRequest, GovernanceArtifactKind};
use prodex_domain::sha256_checksum;

const ARTIFACT_SIGNATURE_DOMAIN: &[u8] = b"prodex-governance-artifact-signature-v1\0";

pub fn activation_etag(
    request: &GovernanceActivationRequest,
    previous_etag: Option<&str>,
) -> String {
    let material = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        request.tenant_id,
        artifact_kind_label(request.kind),
        request.revision_id,
        request.action.as_str(),
        previous_etag.unwrap_or(""),
        request.idempotency_key.as_str(),
    );
    artifact_checksum(material.as_bytes())
}

pub fn artifact_checksum(artifact: &[u8]) -> String {
    sha256_checksum(artifact)
}

/// Builds the stable, domain-separated payload covered by a detached governance signature.
///
/// Every variable field is length-prefixed so a signature cannot be replayed for a different
/// tenant, artifact kind, revision, or compiled payload by rearranging field boundaries.
pub fn artifact_signature_message(
    tenant_id: prodex_domain::TenantId,
    kind: GovernanceArtifactKind,
    revision_id: &str,
    artifact: &[u8],
) -> Vec<u8> {
    let tenant_id = tenant_id.to_string();
    let kind = artifact_kind_label(kind);
    let checksum = artifact_checksum(artifact);
    let mut message = Vec::with_capacity(
        ARTIFACT_SIGNATURE_DOMAIN.len()
            + tenant_id.len()
            + kind.len()
            + revision_id.len()
            + checksum.len()
            + 32,
    );
    message.extend_from_slice(ARTIFACT_SIGNATURE_DOMAIN);
    for value in [
        tenant_id.as_bytes(),
        kind.as_bytes(),
        revision_id.as_bytes(),
        checksum.as_bytes(),
    ] {
        message.extend_from_slice(&(value.len() as u64).to_be_bytes());
        message.extend_from_slice(value);
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_checksum_is_stable_sha256() {
        assert_eq!(
            artifact_checksum(b"prodex"),
            "sha256:f2c03f58d5b3a3327b8e360923c44396fa3c376d207495c18a785e59e82aad64"
        );
    }

    #[test]
    fn artifact_signature_message_binds_every_identity_dimension() {
        let tenant = "00000000-0000-4000-8000-000000000001"
            .parse::<prodex_domain::TenantId>()
            .unwrap();
        let baseline = artifact_signature_message(
            tenant,
            GovernanceArtifactKind::Policy,
            "policy-1",
            br#"{"revision":"policy-1"}"#,
        );

        assert_ne!(
            baseline,
            artifact_signature_message(
                tenant,
                GovernanceArtifactKind::RoutingScores,
                "policy-1",
                br#"{"revision":"policy-1"}"#,
            )
        );
        assert_ne!(
            baseline,
            artifact_signature_message(
                tenant,
                GovernanceArtifactKind::Policy,
                "policy-2",
                br#"{"revision":"policy-1"}"#,
            )
        );
        assert_ne!(
            baseline,
            artifact_signature_message(
                tenant,
                GovernanceArtifactKind::Policy,
                "policy-1",
                br#"{"revision":"policy-2"}"#,
            )
        );
    }
}
