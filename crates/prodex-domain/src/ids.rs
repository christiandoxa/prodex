use std::error::Error;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum IdParseErrorStatus {
    InvalidRequest,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct IdParseErrorResponsePlan {
    pub status: IdParseErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

#[derive(Clone, PartialEq, Eq)]
pub struct IdParseError {
    kind: &'static str,
}

impl IdParseError {
    fn new(kind: &'static str, _value: &str) -> Self {
        Self { kind }
    }

    pub fn kind(&self) -> &'static str {
        self.kind
    }
}

impl fmt::Debug for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IdParseError")
            .field("kind", &"<redacted>")
            .finish()
    }
}

impl fmt::Display for IdParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("identifier is invalid")
    }
}

impl Error for IdParseError {}

pub fn plan_id_parse_error_response(error: &IdParseError) -> IdParseErrorResponsePlan {
    let code = match error.kind() {
        "tenant_id" => "tenant_id_invalid",
        "principal_id" => "principal_id_invalid",
        "request_id" => "request_id_invalid",
        "call_id" => "call_id_invalid",
        "reservation_id" => "reservation_id_invalid",
        "virtual_key_id" => "virtual_key_id_invalid",
        "role_binding_id" => "role_binding_id_invalid",
        "provider_credential_id" => "provider_credential_id_invalid",
        "policy_revision_id" => "policy_revision_id_invalid",
        "audit_event_id" => "audit_event_id_invalid",
        _ => "identifier_invalid",
    };

    IdParseErrorResponsePlan {
        status: IdParseErrorStatus::InvalidRequest,
        code,
        message: "identifier is invalid",
    }
}

macro_rules! domain_id {
    ($name:ident, $kind:literal) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(Uuid);

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($name))
                    .field(&"<redacted>")
                    .finish()
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::now_v7())
            }

            pub fn from_uuid(uuid: Uuid) -> Self {
                Self(uuid)
            }

            pub fn as_uuid(self) -> Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl FromStr for $name {
            type Err = IdParseError;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value)
                    .map(Self)
                    .map_err(|_| IdParseError::new($kind, value))
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::from_str(&value).map_err(serde::de::Error::custom)
            }
        }
    };
}

domain_id!(TenantId, "tenant_id");
domain_id!(PrincipalId, "principal_id");
domain_id!(RequestId, "request_id");
domain_id!(CallId, "call_id");
domain_id!(ReservationId, "reservation_id");
domain_id!(VirtualKeyId, "virtual_key_id");
domain_id!(RoleBindingId, "role_binding_id");
domain_id!(ProviderCredentialId, "provider_credential_id");
domain_id!(PolicyRevisionId, "policy_revision_id");
domain_id!(AuditEventId, "audit_event_id");
