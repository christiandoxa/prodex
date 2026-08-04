use serde::{Deserialize, Deserializer, Serialize, de::Error as DeError};
use std::fmt;

pub const RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX: &str = "__compact_session__:";
pub const RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX: &str = "__compact_turn_state__:";
pub const RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX: &str = "__response_turn_state__:";
pub const RUNTIME_HARD_BINDING_CONFLICT_PROFILE: &str = "__prodex_hard_binding_conflict__";
pub const RUNTIME_HARD_BINDING_COMPONENT_MAX_BYTES: usize = 1_024;
pub const RUNTIME_HARD_BINDING_KEY_MAX_BYTES: usize = 4_096;
pub use prodex_provider_core::RuntimeProviderBindingIdentity;

/// Request-owned continuation identifiers used to resolve one hard affinity owner.
///
/// This deliberately carries only continuation identity. Authentication, request bodies,
/// provider credentials, and transport metadata never belong in this value.
#[derive(Clone, Default, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct RuntimeHardBindingIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turn_state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

impl std::fmt::Debug for RuntimeHardBindingIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeHardBindingIdentity")
            .field("response_id", &self.response_id.is_some())
            .field("turn_state", &self.turn_state.is_some())
            .field("session_id", &self.session_id.is_some())
            .finish()
    }
}

impl RuntimeHardBindingIdentity {
    pub fn new(
        response_id: Option<&str>,
        turn_state: Option<&str>,
        session_id: Option<&str>,
    ) -> Option<Self> {
        let values = [response_id, turn_state, session_id];
        if values.iter().any(|value| {
            value.is_some_and(|value| {
                let value = value.trim();
                !value.is_empty() && !runtime_identity_component_is_valid(value)
            })
        }) {
            return None;
        }
        let identity = Self {
            response_id: normalize_identity_part(response_id),
            turn_state: normalize_identity_part(turn_state),
            session_id: normalize_identity_part(session_id),
        };
        (!identity.is_empty()).then_some(identity)
    }

    pub fn response(response_id: &str) -> Option<Self> {
        Self::new(Some(response_id), None, None)
    }

    pub fn turn_state(turn_state: &str) -> Option<Self> {
        Self::new(None, Some(turn_state), None)
    }

    pub fn session(session_id: &str) -> Option<Self> {
        Self::new(None, None, Some(session_id))
    }

    pub fn is_empty(&self) -> bool {
        self.response_id.is_none() && self.turn_state.is_none() && self.session_id.is_none()
    }
}

fn normalize_identity_part(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn runtime_identity_component_is_valid(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= RUNTIME_HARD_BINDING_COMPONENT_MAX_BYTES
        && value.chars().all(|character| !character.is_control())
}

pub fn runtime_lineage_key_is_bounded(key: &str) -> bool {
    !key.is_empty()
        && key.len() <= RUNTIME_HARD_BINDING_KEY_MAX_BYTES
        && key.chars().all(|character| !character.is_control())
}

impl<'de> Deserialize<'de> for RuntimeHardBindingIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            #[serde(default)]
            response_id: Option<String>,
            #[serde(default)]
            turn_state: Option<String>,
            #[serde(default)]
            session_id: Option<String>,
        }

        let wire = Wire::deserialize(deserializer)?;
        if wire.response_id.is_none() && wire.turn_state.is_none() && wire.session_id.is_none() {
            return Ok(Self::default());
        }
        let identity = Self::new(
            wire.response_id.as_deref(),
            wire.turn_state.as_deref(),
            wire.session_id.as_deref(),
        );
        identity
            .or_else(|| {
                [
                    wire.response_id.as_deref(),
                    wire.turn_state.as_deref(),
                    wire.session_id.as_deref(),
                ]
                .into_iter()
                .flatten()
                .all(|value| value.trim().is_empty())
                .then(Self::default)
            })
            .ok_or_else(|| D::Error::custom("invalid runtime hard binding identity"))
    }
}

#[derive(Clone, Eq, PartialEq)]
pub enum RuntimeHardBindingOwner {
    Unbound,
    Owned(String),
    Conflict,
    Unavailable(String),
}

#[derive(Clone, Eq, PartialEq)]
pub enum RuntimeHardBindingResolution {
    Unbound,
    Owned {
        profile_name: String,
        binding_identity: Option<RuntimeProviderBindingIdentity>,
    },
    Conflict,
    Unavailable {
        profile_name: String,
        binding_identity: Option<RuntimeProviderBindingIdentity>,
    },
}

impl fmt::Debug for RuntimeHardBindingResolution {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound => formatter.write_str("RuntimeHardBindingResolution::Unbound"),
            Self::Owned { .. } => {
                formatter.write_str("RuntimeHardBindingResolution::Owned(<redacted>)")
            }
            Self::Conflict => formatter.write_str("RuntimeHardBindingResolution::Conflict"),
            Self::Unavailable { .. } => {
                formatter.write_str("RuntimeHardBindingResolution::Unavailable(<redacted>)")
            }
        }
    }
}

impl RuntimeHardBindingResolution {
    pub fn owner(&self) -> RuntimeHardBindingOwner {
        match self {
            Self::Unbound => RuntimeHardBindingOwner::Unbound,
            Self::Owned { profile_name, .. } => {
                RuntimeHardBindingOwner::Owned(profile_name.clone())
            }
            Self::Conflict => RuntimeHardBindingOwner::Conflict,
            Self::Unavailable { profile_name, .. } => {
                RuntimeHardBindingOwner::Unavailable(profile_name.clone())
            }
        }
    }

    pub fn binding_identity(&self) -> Option<&RuntimeProviderBindingIdentity> {
        match self {
            Self::Owned {
                binding_identity, ..
            }
            | Self::Unavailable {
                binding_identity, ..
            } => binding_identity.as_ref(),
            Self::Unbound | Self::Conflict => None,
        }
    }
}

impl fmt::Debug for RuntimeHardBindingOwner {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unbound => formatter.write_str("RuntimeHardBindingOwner::Unbound"),
            Self::Owned(_) => formatter.write_str("RuntimeHardBindingOwner::Owned(<redacted>)"),
            Self::Conflict => formatter.write_str("RuntimeHardBindingOwner::Conflict"),
            Self::Unavailable(_) => {
                formatter.write_str("RuntimeHardBindingOwner::Unavailable(<redacted>)")
            }
        }
    }
}

impl RuntimeHardBindingOwner {
    pub fn profile_name(&self) -> Option<&str> {
        match self {
            Self::Owned(profile_name) | Self::Unavailable(profile_name) => Some(profile_name),
            Self::Unbound | Self::Conflict => None,
        }
    }

    pub fn is_hard(&self) -> bool {
        !matches!(self, Self::Unbound)
    }
}

pub fn runtime_compact_session_lineage_key(session_id: &str) -> String {
    bounded_lineage_key(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX, [session_id])
}

pub fn runtime_compact_turn_state_lineage_key(turn_state: &str) -> String {
    bounded_lineage_key(RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX, [turn_state])
}

pub fn runtime_response_turn_state_lineage_key(response_id: &str, turn_state: &str) -> String {
    if !runtime_identity_component_is_valid(response_id)
        || !runtime_identity_component_is_valid(turn_state)
    {
        return format!("{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}__invalid__");
    }
    format!(
        "{RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX}{}:{response_id}:{turn_state}",
        response_id.len()
    )
}

pub fn runtime_is_response_turn_state_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)
}

pub fn runtime_response_turn_state_lineage_parts(key: &str) -> Option<(&str, &str)> {
    let suffix = key.strip_prefix(RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX)?;
    let (response_len, rest) = suffix.split_once(':')?;
    let response_len = response_len.parse::<usize>().ok()?;
    let response_and_sep = rest.get(..response_len.saturating_add(1))?;
    if response_and_sep.as_bytes().get(response_len).copied() != Some(b':') {
        return None;
    }
    let response_id = response_and_sep.get(..response_len)?;
    let turn_state = rest.get(response_len.saturating_add(1)..)?;
    (runtime_identity_component_is_valid(response_id)
        && runtime_identity_component_is_valid(turn_state))
    .then_some((response_id, turn_state))
}

pub fn runtime_is_compact_session_lineage_key(key: &str) -> bool {
    key.starts_with(RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX)
}

fn bounded_lineage_key<const N: usize>(prefix: &str, values: [&str; N]) -> String {
    if values
        .iter()
        .copied()
        .all(runtime_identity_component_is_valid)
    {
        let mut key = prefix.to_string();
        for value in values {
            key.push_str(value);
        }
        key
    } else {
        format!("{prefix}__invalid__")
    }
}
