use super::{MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_TENANTS, RuntimeGatewayTenantSnapshotSet};
use anyhow::Result;
use prodex_domain::TenantId;
use std::collections::BTreeMap;
use std::sync::Arc;

impl<T> RuntimeGatewayTenantSnapshotSet<T> {
    pub(in crate::runtime_launch::proxy_startup) fn bootstrap(
        snapshot: T,
        allow_fallback: bool,
    ) -> Self {
        Self {
            tenant_snapshots: BTreeMap::new(),
            fallback: allow_fallback.then(|| Arc::new(snapshot)),
        }
    }

    pub(in crate::runtime_launch::proxy_startup) fn snapshot_for(
        &self,
        tenant_id: TenantId,
    ) -> Option<Arc<T>> {
        self.tenant_snapshots
            .get(&tenant_id)
            .cloned()
            .or_else(|| self.fallback.clone())
    }

    pub(in crate::runtime_launch::proxy_startup) fn with_tenant_snapshot(
        &self,
        tenant_id: TenantId,
        snapshot: T,
    ) -> Result<Self> {
        if !self.tenant_snapshots.contains_key(&tenant_id)
            && self.tenant_snapshots.len() >= MAX_RUNTIME_GATEWAY_PROVIDER_REGISTRY_TENANTS
        {
            anyhow::bail!("provider registry tenant limit exceeded");
        }
        let mut next = self.clone();
        next.tenant_snapshots.insert(tenant_id, Arc::new(snapshot));
        Ok(next)
    }

    pub(in crate::runtime_launch::proxy_startup) fn without_tenant_snapshot(
        &self,
        tenant_id: TenantId,
    ) -> Option<Self> {
        let mut next = self.clone();
        next.tenant_snapshots.remove(&tenant_id)?;
        Some(next)
    }
}
