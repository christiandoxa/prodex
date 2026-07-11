use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DurableStoreKind {
    Postgres,
    Sqlite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CacheStoreKind {
    Redis,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MigrationRuntimePolicy {
    ExternalMigratorOnly,
    RequestPathForbidden,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct StorageTopology {
    pub durable_store: DurableStoreKind,
    pub cache_store: Option<CacheStoreKind>,
    pub migration_policy: MigrationRuntimePolicy,
}

impl fmt::Debug for StorageTopology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StorageTopology")
            .field("durable_store", &"<redacted>")
            .field("cache_store", &"<redacted>")
            .field("migration_policy", &"<redacted>")
            .finish()
    }
}

impl StorageTopology {
    pub fn validate_for_gateway(self) -> Result<(), StorageTopologyError> {
        if self.migration_policy != MigrationRuntimePolicy::RequestPathForbidden {
            return Err(StorageTopologyError::MigrationMayRunOnRequestPath);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StorageTopologyError {
    MigrationMayRunOnRequestPath,
}

impl fmt::Display for StorageTopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MigrationMayRunOnRequestPath => write!(
                f,
                "migrations must not run from request-serving storage paths"
            ),
        }
    }
}

impl Error for StorageTopologyError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoragePlanErrorStatus {
    BadRequest,
    InvalidConfiguration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoragePlanErrorResponsePlan {
    pub status: StoragePlanErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_storage_topology_error_response(
    _error: &StorageTopologyError,
) -> StoragePlanErrorResponsePlan {
    StoragePlanErrorResponsePlan {
        status: StoragePlanErrorStatus::InvalidConfiguration,
        code: "storage_topology_invalid",
        message: "storage topology configuration is invalid",
    }
}
