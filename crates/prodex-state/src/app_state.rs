use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::collections::BTreeMap;

use crate::{ProfileEntry, ResponseProfileBinding};

pub const APP_STATE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AppState {
    pub active_profile: Option<String>,
    pub profiles: BTreeMap<String, ProfileEntry>,
    pub last_run_selected_at: BTreeMap<String, i64>,
    pub response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    pub session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
}

#[derive(Serialize)]
struct AppStateEnvelopeRef<'a> {
    schema_version: u32,
    active_profile: &'a Option<String>,
    profiles: &'a BTreeMap<String, ProfileEntry>,
    last_run_selected_at: &'a BTreeMap<String, i64>,
    response_profile_bindings: &'a BTreeMap<String, ResponseProfileBinding>,
    session_profile_bindings: &'a BTreeMap<String, ResponseProfileBinding>,
}

impl Serialize for AppState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        AppStateEnvelopeRef {
            schema_version: APP_STATE_SCHEMA_VERSION,
            active_profile: &self.active_profile,
            profiles: &self.profiles,
            last_run_selected_at: &self.last_run_selected_at,
            response_profile_bindings: &self.response_profile_bindings,
            session_profile_bindings: &self.session_profile_bindings,
        }
        .serialize(serializer)
    }
}

#[derive(Deserialize)]
struct AppStateEnvelope {
    #[serde(default)]
    schema_version: u32,
    active_profile: Option<String>,
    #[serde(default)]
    profiles: BTreeMap<String, ProfileEntry>,
    #[serde(default)]
    last_run_selected_at: BTreeMap<String, i64>,
    #[serde(default)]
    response_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
    #[serde(default)]
    session_profile_bindings: BTreeMap<String, ResponseProfileBinding>,
}

impl<'de> Deserialize<'de> for AppState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let envelope = AppStateEnvelope::deserialize(deserializer)?;
        if !matches!(envelope.schema_version, 0 | APP_STATE_SCHEMA_VERSION) {
            return Err(de::Error::custom(format!(
                "unsupported prodex state schema version {}",
                envelope.schema_version
            )));
        }
        Ok(Self {
            active_profile: envelope.active_profile,
            profiles: envelope.profiles,
            last_run_selected_at: envelope.last_run_selected_at,
            response_profile_bindings: envelope.response_profile_bindings,
            session_profile_bindings: envelope.session_profile_bindings,
        })
    }
}
