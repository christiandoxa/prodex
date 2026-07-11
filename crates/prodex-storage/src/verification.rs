use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MultiReplicaAccountingCheck {
    NoLostUpdate,
    NoDuplicateCharge,
    NoDroppedLedgerEvent,
    NoRequestIdCollision,
    NoLimitOvershoot,
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingConcurrencySpec {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub checks: Vec<MultiReplicaAccountingCheck>,
}

impl fmt::Debug for MultiReplicaAccountingConcurrencySpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingConcurrencySpec")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("checks", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingEvidence {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub passed_checks: Vec<MultiReplicaAccountingCheck>,
    pub documented_limit_overshoot_tolerance: bool,
}

impl fmt::Debug for MultiReplicaAccountingEvidence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingEvidence")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("passed_checks", &"<redacted>")
            .field(
                "documented_limit_overshoot_tolerance",
                &self.documented_limit_overshoot_tolerance,
            )
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct MultiReplicaAccountingVerificationPlan {
    pub topology: StorageTopology,
    pub gateway_replica_count: u16,
    pub verified_checks: Vec<MultiReplicaAccountingCheck>,
}

impl fmt::Debug for MultiReplicaAccountingVerificationPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MultiReplicaAccountingVerificationPlan")
            .field("topology", &"<redacted>")
            .field("gateway_replica_count", &"<redacted>")
            .field("verified_checks", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum MultiReplicaAccountingConcurrencySpecError {
    RequiresAtLeastTwoGatewayReplicas {
        actual: u16,
    },
    RequiresPostgresDurableStore,
    RequiresRedisCoordination,
    EvidenceTopologyMismatch {
        expected: StorageTopology,
        actual: StorageTopology,
    },
    EvidenceReplicaCountMismatch {
        expected: u16,
        actual: u16,
    },
    MissingEvidenceCheck {
        check: MultiReplicaAccountingCheck,
    },
    MissingLimitOvershootTolerance,
}

impl fmt::Debug for MultiReplicaAccountingConcurrencySpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequiresAtLeastTwoGatewayReplicas { .. } => f
                .debug_struct("RequiresAtLeastTwoGatewayReplicas")
                .field("actual", &"<redacted>")
                .finish(),
            Self::RequiresPostgresDurableStore => f.write_str("RequiresPostgresDurableStore"),
            Self::RequiresRedisCoordination => f.write_str("RequiresRedisCoordination"),
            Self::EvidenceTopologyMismatch { .. } => f
                .debug_struct("EvidenceTopologyMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::EvidenceReplicaCountMismatch { .. } => f
                .debug_struct("EvidenceReplicaCountMismatch")
                .field("expected", &"<redacted>")
                .field("actual", &"<redacted>")
                .finish(),
            Self::MissingEvidenceCheck { .. } => f
                .debug_struct("MissingEvidenceCheck")
                .field("check", &"<redacted>")
                .finish(),
            Self::MissingLimitOvershootTolerance => f.write_str("MissingLimitOvershootTolerance"),
        }
    }
}

impl fmt::Display for MultiReplicaAccountingConcurrencySpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "multi-replica accounting configuration is invalid")
    }
}

impl Error for MultiReplicaAccountingConcurrencySpecError {}

pub fn plan_multi_replica_accounting_error_response(
    _error: &MultiReplicaAccountingConcurrencySpecError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::InvalidConfiguration,
        code: "multi_replica_accounting_invalid",
        message: "multi-replica accounting configuration is invalid",
    }
}

pub fn plan_multi_replica_accounting_concurrency_spec(
    topology: StorageTopology,
    gateway_replica_count: u16,
) -> Result<MultiReplicaAccountingConcurrencySpec, MultiReplicaAccountingConcurrencySpecError> {
    if gateway_replica_count < 2 {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::RequiresAtLeastTwoGatewayReplicas {
                actual: gateway_replica_count,
            },
        );
    }
    if topology.durable_store != DurableStoreKind::Postgres {
        return Err(MultiReplicaAccountingConcurrencySpecError::RequiresPostgresDurableStore);
    }
    if topology.cache_store != Some(CacheStoreKind::Redis) {
        return Err(MultiReplicaAccountingConcurrencySpecError::RequiresRedisCoordination);
    }
    Ok(MultiReplicaAccountingConcurrencySpec {
        topology,
        gateway_replica_count,
        checks: vec![
            MultiReplicaAccountingCheck::NoLostUpdate,
            MultiReplicaAccountingCheck::NoDuplicateCharge,
            MultiReplicaAccountingCheck::NoDroppedLedgerEvent,
            MultiReplicaAccountingCheck::NoRequestIdCollision,
            MultiReplicaAccountingCheck::NoLimitOvershoot,
        ],
    })
}

pub fn plan_multi_replica_accounting_verification(
    spec: &MultiReplicaAccountingConcurrencySpec,
    evidence: MultiReplicaAccountingEvidence,
) -> Result<MultiReplicaAccountingVerificationPlan, MultiReplicaAccountingConcurrencySpecError> {
    if evidence.topology != spec.topology {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceTopologyMismatch {
                expected: spec.topology,
                actual: evidence.topology,
            },
        );
    }
    if evidence.gateway_replica_count != spec.gateway_replica_count {
        return Err(
            MultiReplicaAccountingConcurrencySpecError::EvidenceReplicaCountMismatch {
                expected: spec.gateway_replica_count,
                actual: evidence.gateway_replica_count,
            },
        );
    }
    for check in &spec.checks {
        if !evidence.passed_checks.contains(check) {
            return Err(
                MultiReplicaAccountingConcurrencySpecError::MissingEvidenceCheck { check: *check },
            );
        }
    }
    if spec
        .checks
        .contains(&MultiReplicaAccountingCheck::NoLimitOvershoot)
        && !evidence.documented_limit_overshoot_tolerance
    {
        return Err(MultiReplicaAccountingConcurrencySpecError::MissingLimitOvershootTolerance);
    }

    Ok(MultiReplicaAccountingVerificationPlan {
        topology: evidence.topology,
        gateway_replica_count: evidence.gateway_replica_count,
        verified_checks: spec.checks.clone(),
    })
}
