use sha2::{Digest, Sha256};

use super::{AuditDigest, AuditEvent};

const AUDIT_CHAIN_DOMAIN: &[u8] = b"prodex:audit-chain:v1";

/// Computes the canonical digest for one append-only audit-chain link.
pub fn compute_audit_chain_digest(
    previous_digest: Option<&AuditDigest>,
    event: &AuditEvent,
) -> AuditDigest {
    let mut hasher = Sha256::new();
    hasher.update(AUDIT_CHAIN_DOMAIN);
    frame_optional(&mut hasher, previous_digest.map(AuditDigest::as_str));
    frame(&mut hasher, event.id.as_uuid().as_bytes());
    frame(&mut hasher, &event.occurred_at_unix_ms.to_be_bytes());
    frame(&mut hasher, event.tenant_id.as_uuid().as_bytes());
    frame(&mut hasher, event.principal_id.as_uuid().as_bytes());
    frame(&mut hasher, event.action.as_str().as_bytes());
    frame(&mut hasher, event.resource.kind.as_bytes());
    frame_optional(&mut hasher, event.resource.id.as_deref());
    match event.resource.tenant_id {
        Some(tenant_id) => {
            hasher.update([1]);
            frame(&mut hasher, tenant_id.as_uuid().as_bytes());
        }
        None => hasher.update([0]),
    }
    frame(&mut hasher, event.outcome.as_str().as_bytes());
    frame_optional(&mut hasher, event.reason_code.as_deref());

    AuditDigest::new(format!("sha256:{}", hex_lower(&hasher.finalize())))
        .expect("canonical SHA-256 audit digest is valid")
}

/// Computes the canonical lowercase SHA-256 checksum used by persisted artifacts.
pub fn sha256_checksum(value: &[u8]) -> String {
    format!("sha256:{}", hex_lower(&Sha256::digest(value)))
}

fn frame(hasher: &mut Sha256, value: &[u8]) {
    hasher.update(
        u64::try_from(value.len())
            .expect("audit field length fits u64")
            .to_be_bytes(),
    );
    hasher.update(value);
}

fn frame_optional(hasher: &mut Sha256, value: Option<impl AsRef<[u8]>>) {
    match value {
        Some(value) => {
            hasher.update([1]);
            frame(hasher, value.as_ref());
        }
        None => hasher.update([0]),
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}
