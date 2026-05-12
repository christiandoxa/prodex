use serde_json::Value;

use crate::{
    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD, RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER,
};

use super::runtime_mem_super_slim_v2_emitted_artifact_ref;

#[derive(Debug, Default)]
pub(crate) struct RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    previous_emitted_ref: Option<String>,
}

impl RuntimeMemSuperSlimV2ArtifactRefDedupeState {
    pub(crate) fn dedupe_consecutive_event_ref(&mut self, mut event: Value) -> Value {
        let Some(artifact_ref) = runtime_mem_super_slim_v2_emitted_artifact_ref(&event) else {
            self.previous_emitted_ref = None;
            return event;
        };

        if self.previous_emitted_ref.as_deref() == Some(artifact_ref.as_str()) {
            if let Some(object) = event.as_object_mut() {
                object.remove("r");
                object.insert(
                    RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_FIELD.to_string(),
                    Value::String(RUNTIME_MEM_SUPER_SLIM_V2_PREVIOUS_REF_MARKER.to_string()),
                );
            }
        } else {
            self.previous_emitted_ref = Some(artifact_ref);
        }

        event
    }
}
