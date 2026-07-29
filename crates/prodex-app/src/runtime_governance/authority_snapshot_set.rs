use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;

use super::{
    MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS, RuntimeGovernanceAuthoritySnapshot,
    RuntimeGovernanceAuthoritySnapshotSet,
};

impl RuntimeGovernanceAuthoritySnapshotSet {
    pub(crate) fn bootstrap(
        snapshot: RuntimeGovernanceAuthoritySnapshot,
        allow_fallback: bool,
    ) -> Self {
        Self {
            tenant_snapshots: BTreeMap::new(),
            fallback: allow_fallback.then(|| Arc::new(snapshot)),
        }
    }

    pub(crate) fn snapshot_for(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Option<Arc<RuntimeGovernanceAuthoritySnapshot>> {
        self.tenant_snapshots
            .get(&tenant_id)
            .cloned()
            .or_else(|| self.fallback.clone())
    }

    pub(crate) fn with_tenant_snapshot(
        &self,
        tenant_id: prodex_domain::TenantId,
        snapshot: RuntimeGovernanceAuthoritySnapshot,
    ) -> Result<Self> {
        if !self.tenant_snapshots.contains_key(&tenant_id)
            && self.tenant_snapshots.len() >= MAX_RUNTIME_GOVERNANCE_AUTHORITY_TENANTS
        {
            anyhow::bail!("governance authority tenant limit exceeded");
        }
        let mut next = self.clone();
        next.tenant_snapshots.insert(tenant_id, Arc::new(snapshot));
        Ok(next)
    }

    pub(crate) fn without_tenant_snapshot(
        &self,
        tenant_id: prodex_domain::TenantId,
    ) -> Option<Self> {
        self.tenant_snapshots.contains_key(&tenant_id).then(|| {
            let mut next = self.clone();
            next.tenant_snapshots.remove(&tenant_id);
            next
        })
    }

    pub(crate) fn policies_are_servable(
        &self,
        tenant_ids: &[prodex_domain::TenantId],
        now_unix_ms: u64,
    ) -> bool {
        !tenant_ids.is_empty()
            && tenant_ids.iter().all(|tenant_id| {
                self.snapshot_for(*tenant_id)
                    .is_some_and(|snapshot| snapshot.application.policy.is_valid_at(now_unix_ms))
            })
    }
}
