#[path = "artifact_store/content.rs"]
mod content;
#[path = "artifact_store/persistence.rs"]
mod persistence;
#[path = "artifact_store/projection.rs"]
mod projection;
#[path = "artifact_store/types.rs"]
mod types;

pub(in crate::runtime_state_shared) use self::types::RuntimeSmartContextArtifactProjectionPrewarm;
pub(crate) use self::types::{
    RuntimeSmartContextArtifactRepoMap, RuntimeSmartContextArtifactRepoMapEntry,
    RuntimeSmartContextArtifactRepoMapEntryKind, RuntimeSmartContextArtifactStore,
    RuntimeSmartContextStaticFingerprintMetadata,
};
