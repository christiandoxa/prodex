use super::{AUDIT_RECORD_VERSION, AuditEventPayload, AuditLogEventRecord};
use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};

pub(super) fn checksum_json<T: Serialize>(value: &T) -> Result<String> {
    let encoded = serde_json::to_vec(value).context("failed to serialize checksummed record")?;
    let digest = Sha256::digest(encoded);
    let mut checksum = String::with_capacity(7 + digest.len() * 2);
    checksum.push_str("sha256:");
    for byte in digest {
        checksum.push(char::from(b"0123456789abcdef"[(byte >> 4) as usize]));
        checksum.push(char::from(b"0123456789abcdef"[(byte & 0x0f) as usize]));
    }
    Ok(checksum)
}

pub(super) fn constant_time_text_eq(left: &str, right: &str) -> bool {
    left.len() == right.len()
        && left
            .as_bytes()
            .iter()
            .zip(right.as_bytes())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0
}

pub(super) fn validate_audit_event(event: &AuditLogEventRecord) -> Result<()> {
    if event.schema_version == 0 && event.checksum.is_none() {
        return Ok(());
    }
    if event.schema_version != AUDIT_RECORD_VERSION {
        bail!("unsupported audit log record version");
    }
    let checksum = event
        .checksum
        .as_deref()
        .context("missing audit log checksum")?;
    let payload = AuditEventPayload {
        recorded_at: &event.recorded_at,
        recorded_at_epoch: event.recorded_at_epoch,
        pid: event.pid,
        component: &event.component,
        action: &event.action,
        outcome: &event.outcome,
        details: &event.details,
    };
    let expected = checksum_json(&payload).context("failed to checksum audit event")?;
    if !constant_time_text_eq(&expected, checksum) {
        bail!("audit log checksum mismatch");
    }
    Ok(())
}
