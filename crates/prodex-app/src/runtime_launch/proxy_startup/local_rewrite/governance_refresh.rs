use super::{RuntimeGovernanceAuthority, runtime_gateway_load_governance_snapshot};

pub(super) fn runtime_gateway_refresh_policy_snapshot(
    authority: &RuntimeGovernanceAuthority,
    sqlite_repository: Option<&prodex_storage_sqlite_runtime::GovernanceSqliteRepository>,
    tenant_id: prodex_domain::TenantId,
    governance_policy: &prodex_runtime_policy::RuntimePolicyGovernanceSettings,
    deployment_mode: prodex_config::GovernanceMode,
    snapshots: &mut crate::runtime_governance::RuntimeGovernanceAuthoritySnapshotSet,
) -> (usize, usize) {
    let stored = runtime_gateway_load_governance_snapshot(
        authority,
        sqlite_repository,
        tenant_id,
        prodex_storage::GovernanceArtifactKind::Policy,
        |input| {
            super::super::local_rewrite_governance_artifact_authenticity::governance_artifact_authenticity_is_valid(
                governance_policy,
                input,
            ) && crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                input.compiled_artifact,
                deployment_mode,
            )
            .is_ok_and(|snapshot| {
                snapshot.application.policy.revision().to_string() == input.revision_id
            })
        },
    );
    match stored {
        Ok(stored) => {
            let snapshot =
                crate::runtime_governance::compile_runtime_governance_artifact_for_deployment(
                    &stored.compiled_artifact,
                    deployment_mode,
                );
            if let Ok(snapshot) = snapshot
                && snapshot.application.policy.revision().to_string() == stored.revision_id
                && let Ok(updated) = snapshots.with_tenant_snapshot(tenant_id, snapshot)
            {
                *snapshots = updated;
                (1, 0)
            } else {
                (0, 0)
            }
        }
        Err(error)
            if deployment_mode.is_enforcing()
                && error.downcast_ref::<prodex_storage::GovernanceRepositoryError>()
                    == Some(&prodex_storage::GovernanceRepositoryError::SnapshotUnavailable) =>
        {
            if let Some(updated) = snapshots.without_tenant_snapshot(tenant_id) {
                *snapshots = updated;
                (0, 1)
            } else {
                (0, 0)
            }
        }
        Err(_) => (0, 0),
    }
}
