use super::*;
use prodex_domain::{
    AuditAction, AuditDigest, AuditEvent, AuditOutcome, AuditReasonCode, AuditReasonDetail,
    AuditResource, AuditResourceId, AuditTimestamp, compute_audit_chain_digest,
    normalize_audit_reason_detail,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GovernanceAuditIntegrityHealth {
    pub event_count: u64,
    pub chain_head_count: u64,
    pub chain_valid: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub struct GovernanceAuditExportRecord {
    pub audit_event_id: String,
    pub occurred_at_unix_ms: u64,
    pub principal_id: String,
    pub action: String,
    pub resource_kind: String,
    pub resource_id: Option<String>,
    pub outcome: String,
    pub reason_code: Option<String>,
    pub reason_detail: Option<String>,
    pub previous_digest: Option<String>,
    pub event_digest: String,
}

/// Verifies both the topology and the canonical digest of every persisted audit event.
/// Invalid persisted fields deliberately collapse to the same redacted health result.
pub fn verify_governance_audit_integrity(
    tenant_id: TenantId,
    records: &[GovernanceAuditExportRecord],
) -> GovernanceAuditIntegrityHealth {
    verify_governance_audit_integrity_with_retention_anchor(tenant_id, records, None)
}

pub fn verify_governance_audit_integrity_with_retention_anchor(
    tenant_id: TenantId,
    records: &[GovernanceAuditExportRecord],
    retention_anchor: Option<&AuditDigest>,
) -> GovernanceAuditIntegrityHealth {
    use std::collections::{HashMap, HashSet};

    let event_count = u64::try_from(records.len()).unwrap_or(u64::MAX);
    let mut valid = true;
    let mut digests = HashSet::with_capacity(records.len());
    let mut child_by_previous = HashMap::with_capacity(records.len());
    let mut root = None;

    for record in records {
        if !digests.insert(record.event_digest.clone()) {
            valid = false;
        }
        match record.previous_digest.as_ref() {
            Some(previous)
                if retention_anchor.is_some_and(|anchor| anchor.as_str() == previous) =>
            {
                if root.replace(record.event_digest.clone()).is_some() {
                    valid = false;
                }
            }
            Some(previous) => {
                if child_by_previous
                    .insert(previous.clone(), record.event_digest.clone())
                    .is_some()
                {
                    valid = false;
                }
            }
            None => {
                if root.replace(record.event_digest.clone()).is_some() {
                    valid = false;
                }
            }
        }
        valid &= audit_record_digest_is_valid(tenant_id, record);
    }

    valid &= child_by_previous
        .keys()
        .all(|previous| digests.contains(previous));
    let chain_head_count = u64::try_from(
        digests
            .iter()
            .filter(|digest| !child_by_previous.contains_key(digest.as_str()))
            .count(),
    )
    .unwrap_or(u64::MAX);

    valid &= governance_audit_chain_is_complete(
        records.len(),
        root,
        &child_by_previous,
        chain_head_count,
    );

    GovernanceAuditIntegrityHealth {
        event_count,
        chain_head_count,
        chain_valid: valid,
    }
}

fn governance_audit_chain_is_complete(
    record_count: usize,
    root: Option<String>,
    child_by_previous: &std::collections::HashMap<String, String>,
    chain_head_count: u64,
) -> bool {
    if record_count == 0 {
        return root.is_none() && chain_head_count == 0;
    }
    let Some(mut current) = root else {
        return false;
    };
    let mut visited = std::collections::HashSet::with_capacity(record_count);
    while visited.insert(current.clone()) {
        let Some(next) = child_by_previous.get(&current) else {
            break;
        };
        current = next.clone();
    }
    visited.len() == record_count && chain_head_count == 1
}

fn audit_record_digest_is_valid(tenant_id: TenantId, record: &GovernanceAuditExportRecord) -> bool {
    let parsed = (|| {
        let id = record.audit_event_id.parse::<AuditEventId>().ok()?;
        AuditTimestamp::new(record.occurred_at_unix_ms).ok()?;
        let principal_id = record.principal_id.parse::<PrincipalId>().ok()?;
        let action = AuditAction::try_new(record.action.clone()).ok()?;
        let resource_id = record
            .resource_id
            .clone()
            .map(AuditResourceId::new)
            .transpose()
            .ok()?;
        let resource = AuditResource::new_with_resource_id(
            record.resource_kind.clone(),
            resource_id,
            Some(tenant_id),
        )
        .ok()?;
        let outcome = AuditOutcome::parse(&record.outcome).ok()?;
        if let Some(reason_code) = record.reason_code.as_ref() {
            AuditReasonCode::new(reason_code.clone()).ok()?;
        }
        let reason_detail = match record.reason_detail.as_deref() {
            None => None,
            Some(reason_detail) => {
                if normalize_audit_reason_detail(reason_detail).as_deref() != Some(reason_detail) {
                    return None;
                }
                Some(AuditReasonDetail::new(reason_detail).ok()?)
            }
        };
        let previous_digest = record
            .previous_digest
            .clone()
            .map(AuditDigest::new)
            .transpose()
            .ok()?;
        let stored_digest = AuditDigest::new(record.event_digest.clone()).ok()?;
        Some((
            AuditEvent {
                id,
                occurred_at_unix_ms: record.occurred_at_unix_ms,
                tenant_id,
                principal_id,
                action,
                resource,
                outcome,
                reason_code: record.reason_code.clone(),
                reason_detail,
            },
            previous_digest,
            stored_digest,
        ))
    })();
    let Some((event, previous_digest, stored_digest)) = parsed else {
        return false;
    };
    compute_audit_chain_digest(previous_digest.as_ref(), &event) == stored_digest
}

impl fmt::Debug for GovernanceAuditExportRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GovernanceAuditExportRecord")
            .field("audit_event_id", &"<redacted>")
            .field("occurred_at_unix_ms", &"<redacted>")
            .field("principal_id", &"<redacted>")
            .field("action", &self.action)
            .field("resource_kind", &self.resource_kind)
            .field(
                "resource_id",
                &self.resource_id.as_ref().map(|_| "<redacted>"),
            )
            .field("outcome", &self.outcome)
            .field("reason_code", &self.reason_code)
            .field(
                "reason_detail",
                &self.reason_detail.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "previous_digest",
                &self.previous_digest.as_ref().map(|_| "<redacted>"),
            )
            .field("event_digest", &"<redacted>")
            .finish()
    }
}
