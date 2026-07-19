use super::RuntimeGatewayGovernanceSessionStore;
use crate::RuntimeProxyRequest;
use prodex_domain::{Channel, Principal, TenantContext};
use prodex_storage::{GovernanceRepositoryError, GovernanceWriteOutcome};

impl RuntimeGatewayGovernanceSessionStore {
    pub(in crate::runtime_launch::proxy_startup) fn revoke_current(
        &self,
        request: &RuntimeProxyRequest,
        tenant: TenantContext,
        actor: &Principal,
        now_seconds: u64,
    ) -> Result<GovernanceWriteOutcome, GovernanceRepositoryError> {
        let snapshot = self.snapshot(request, tenant, actor, Channel::Api, now_seconds);
        if !snapshot.store_ready {
            return Err(GovernanceRepositoryError::Database);
        }
        if !snapshot.existing || !snapshot.binding_valid || snapshot.policy.revoked {
            return Err(GovernanceRepositoryError::NotFound);
        }
        let session_id_hash = snapshot
            .session_id_hash()
            .ok_or(GovernanceRepositoryError::InvalidInput)?;
        self.revoke_by_hash(tenant, &session_id_hash, actor, "session.self_revoke")
    }
}

#[cfg(test)]
mod tests {
    use super::super::RuntimeGatewayGovernanceSessionBankCommand;
    use super::RuntimeGatewayGovernanceSessionStore;
    use crate::RuntimeProxyRequest;
    use prodex_domain::{
        Channel, CredentialScope, DataClassification, PolicyRevisionId, Principal, PrincipalId,
        PrincipalKind, Role, TenantContext, TenantId,
    };
    use prodex_provider_core::ProviderId;
    use prodex_storage::{GovernanceRepositoryError, GovernanceWriteOutcome};
    use std::sync::mpsc::sync_channel;
    use std::thread;

    fn principal(tenant_id: TenantId) -> Principal {
        Principal::new(
            PrincipalId::new(),
            Some(tenant_id),
            PrincipalKind::VirtualKey,
            Role::Operator,
            CredentialScope::DataPlane,
        )
    }

    fn request() -> RuntimeProxyRequest {
        RuntimeProxyRequest {
            method: "POST".to_string(),
            path_and_query: "/v1/responses".to_string(),
            headers: vec![("session_id".to_string(), "opaque-session".to_string())],
            body: Vec::new(),
        }
    }

    #[test]
    fn current_session_owner_can_revoke_without_supplying_a_hash() {
        let store = RuntimeGatewayGovernanceSessionStore::default();
        let tenant = TenantContext {
            tenant_id: TenantId::new(),
        };
        let owner = principal(tenant.tenant_id);
        let initial = store.snapshot(&request(), tenant, &owner, Channel::Api, 100);
        store
            .remember(
                initial,
                tenant,
                &owner,
                Channel::Api,
                100,
                DataClassification::Internal,
                PolicyRevisionId::new(),
                1,
                1,
                ProviderId::OpenAi,
                prodex_config::GovernanceSessionConfig::default(),
            )
            .unwrap();
        assert!(matches!(
            store.revoke_current(&request(), tenant, &principal(tenant.tenant_id), 101),
            Err(GovernanceRepositoryError::NotFound)
        ));

        let (sender, receiver) = sync_channel(1);
        store.0.state.lock().unwrap().bank = Some(sender);
        let worker_owner = owner.clone();
        let worker = thread::spawn(move || {
            let RuntimeGatewayGovernanceSessionBankCommand::Revoke {
                actor,
                reason_code,
                acknowledge,
                ..
            } = receiver.recv().unwrap()
            else {
                panic!("expected session revocation");
            };
            assert_eq!(actor.id, worker_owner.id);
            assert_eq!(reason_code, "session.self_revoke");
            acknowledge
                .send(Ok(GovernanceWriteOutcome::Applied))
                .unwrap();
        });

        assert_eq!(
            store.revoke_current(&request(), tenant, &owner, 101),
            Ok(GovernanceWriteOutcome::Applied)
        );
        assert!(
            store
                .snapshot(&request(), tenant, &owner, Channel::Api, 102)
                .policy
                .revoked
        );
        worker.join().unwrap();
    }
}
