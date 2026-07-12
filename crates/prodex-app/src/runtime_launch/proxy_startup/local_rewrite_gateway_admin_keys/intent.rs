use std::fmt;

use serde::de::{Deserialize, Deserializer, IgnoredAny, MapAccess, Visitor};
use zeroize::Zeroizing;

use super::RuntimeGatewayAdminError;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeGatewayKeyMutationIntent {
    pub(super) rotate: bool,
    supplied_token: bool,
}

impl RuntimeGatewayKeyMutationIntent {
    pub(super) fn parse(body: &[u8]) -> Result<Self, RuntimeGatewayAdminError> {
        serde_json::from_slice(body).map_err(|_| {
            RuntimeGatewayAdminError::new(400, "invalid_json", "request body is not valid JSON")
        })
    }

    pub(super) const fn rotates_secret(self) -> bool {
        self.rotate || self.supplied_token
    }
}

impl<'de> Deserialize<'de> for RuntimeGatewayKeyMutationIntent {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct IntentVisitor;

        impl<'de> Visitor<'de> for IntentVisitor {
            type Value = RuntimeGatewayKeyMutationIntent;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a gateway key update object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut intent = RuntimeGatewayKeyMutationIntent::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "rotate" => intent.rotate = map.next_value::<BooleanPresence>()?.0,
                        "token" => intent.supplied_token = map.next_value::<SecretPresence>()?.0,
                        _ => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(intent)
            }
        }

        deserializer.deserialize_map(IntentVisitor)
    }
}

struct BooleanPresence(bool);

impl<'de> Deserialize<'de> for BooleanPresence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct BooleanVisitor;

        impl<'de> Visitor<'de> for BooleanVisitor {
            type Value = BooleanPresence;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any JSON value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(BooleanPresence(value))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }

            fn visit_str<E>(self, _: &str) -> Result<Self::Value, E> {
                Ok(BooleanPresence(false))
            }
        }

        deserializer.deserialize_any(BooleanVisitor)
    }
}

struct SecretPresence(bool);

impl<'de> Deserialize<'de> for SecretPresence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SecretVisitor;

        impl<'de> Visitor<'de> for SecretVisitor {
            type Value = SecretPresence;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("any JSON value")
            }

            fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E> {
                Ok(SecretPresence(!value.trim().is_empty()))
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
                Ok(SecretPresence(!value.trim().is_empty()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
                let value = Zeroizing::new(value);
                Ok(SecretPresence(!value.trim().is_empty()))
            }

            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }

            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }

            fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_any(self)
            }

            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }

            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }

            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }

            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(SecretPresence(false))
            }
        }

        deserializer.deserialize_any(SecretVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intent_detects_rotation_without_retaining_token() {
        for (body, expected) in [
            (br#"{"rotate":true}"#.as_slice(), true),
            (br#"{"token":" secret "}"#.as_slice(), true),
            (br#"{"token":""}"#.as_slice(), false),
            (br#"{"rotate":"yes","token":null}"#.as_slice(), false),
        ] {
            assert_eq!(
                RuntimeGatewayKeyMutationIntent::parse(body)
                    .unwrap()
                    .rotates_secret(),
                expected,
            );
        }
    }

    #[test]
    fn intent_rejects_invalid_json_with_static_error() {
        let error = RuntimeGatewayKeyMutationIntent::parse(b"not-json").unwrap_err();
        assert_eq!(error.test_code(), "invalid_json");
        assert!(!format!("{error:?}").contains("not-json"));
    }
}
