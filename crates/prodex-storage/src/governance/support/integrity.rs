use super::codec::artifact_kind_label;
use crate::GovernanceActivationRequest;
use prodex_domain::sha256_checksum;

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
}
