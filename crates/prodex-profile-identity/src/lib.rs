use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileIdentity {
    pub email: Option<String>,
    pub account_id: Option<String>,
}

impl ProfileIdentity {
    pub fn has_email(&self) -> bool {
        self.email
            .as_deref()
            .is_some_and(|email| !email.trim().is_empty())
    }

    pub fn has_account_id(&self) -> bool {
        self.account_id
            .as_deref()
            .is_some_and(|account_id| !account_id.trim().is_empty())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct StoredAuth {
    tokens: Option<StoredTokens>,
}

#[derive(Debug, Clone, Deserialize)]
struct StoredTokens {
    access_token: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenClaims {
    #[serde(default)]
    email: Option<String>,
    #[serde(rename = "https://api.openai.com/profile", default)]
    profile: Option<IdTokenProfileClaims>,
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<IdTokenAuthClaims>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenProfileClaims {
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct IdTokenAuthClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenAccountClaims {
    #[serde(rename = "https://api.openai.com/auth", default)]
    auth: Option<TokenAccountAuthClaims>,
    #[serde(rename = "https://api.openai.com/auth.chatgpt_account_id", default)]
    auth_chatgpt_account_id: Option<String>,
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

impl TokenAccountClaims {
    fn into_account_id(self) -> Option<String> {
        self.auth
            .and_then(|auth| auth.chatgpt_account_id)
            .or(self.auth_chatgpt_account_id)
            .or(self.chatgpt_account_id)
            .and_then(normalize_optional_account_id)
    }
}

#[derive(Debug, Deserialize)]
struct TokenAccountAuthClaims {
    #[serde(default)]
    chatgpt_account_id: Option<String>,
}

pub fn parse_identity_from_auth_json(raw_auth_json: &str) -> Result<ProfileIdentity> {
    let stored_auth: StoredAuth =
        serde_json::from_str(raw_auth_json).context("failed to parse auth JSON")?;
    parse_identity_from_stored_auth(&stored_auth)
}

#[allow(dead_code)]
pub fn parse_email_from_auth_json(raw_auth_json: &str) -> Result<Option<String>> {
    Ok(parse_identity_from_auth_json(raw_auth_json)?.email)
}

fn parse_identity_from_stored_auth(stored_auth: &StoredAuth) -> Result<ProfileIdentity> {
    let Some(tokens) = stored_auth.tokens.as_ref() else {
        return Ok(ProfileIdentity::default());
    };

    let id_token_identity = tokens
        .id_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(parse_identity_from_id_token)
        .transpose()?
        .unwrap_or_default();
    let stored_account_id = tokens
        .account_id
        .as_deref()
        .and_then(normalize_optional_account_id);
    let access_token_account_id = tokens
        .access_token
        .as_deref()
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .and_then(|token| parse_account_id_from_access_token(token).ok().flatten());

    Ok(ProfileIdentity {
        email: id_token_identity.email,
        account_id: id_token_identity
            .account_id
            .or(access_token_account_id)
            .or(stored_account_id),
    })
}

pub fn parse_identity_from_id_token(raw_jwt: &str) -> Result<ProfileIdentity> {
    let claims: IdTokenClaims = parse_jwt_payload(raw_jwt)?;
    Ok(ProfileIdentity {
        email: claims
            .email
            .or_else(|| claims.profile.and_then(|profile| profile.email))
            .and_then(normalize_optional_email),
        account_id: claims
            .auth
            .and_then(|auth| auth.chatgpt_account_id)
            .and_then(normalize_optional_account_id),
    })
}

#[allow(dead_code)]
pub fn parse_email_from_id_token(raw_jwt: &str) -> Result<Option<String>> {
    Ok(parse_identity_from_id_token(raw_jwt)?.email)
}

pub fn parse_account_id_from_access_token(raw_jwt: &str) -> Result<Option<String>> {
    let claims: TokenAccountClaims = parse_jwt_payload(raw_jwt)?;
    Ok(claims.into_account_id())
}

pub fn parse_jwt_payload<T>(raw_jwt: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut parts = raw_jwt.split('.');
    let (_header_b64, payload_b64, _sig_b64) = match (parts.next(), parts.next(), parts.next()) {
        (Some(header), Some(payload), Some(signature))
            if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
        {
            (header, payload, signature)
        }
        _ => bail!("invalid JWT format"),
    };

    let payload_bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
        .context("failed to decode JWT payload")?;
    serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")
}

pub fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

pub fn normalize_optional_email(email: impl AsRef<str>) -> Option<String> {
    let email = email.as_ref().trim().to_string();
    (!email.is_empty()).then_some(email)
}

pub fn normalize_optional_account_id(account_id: impl AsRef<str>) -> Option<String> {
    let account_id = account_id.as_ref().trim().to_string();
    (!account_id.is_empty()).then_some(account_id)
}

pub fn normalize_account_id(account_id: &str) -> String {
    account_id.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_identity_from_auth_json_with_id_token_account_priority() {
        let identity = parse_identity_from_auth_json(&format!(
            r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
            chatgpt_id_token("user@example.com", Some("id-token-account")),
            chatgpt_access_token_with_auth_object("access-token-account"),
        ))
        .unwrap();

        assert_eq!(identity.email.as_deref(), Some("user@example.com"));
        assert_eq!(identity.account_id.as_deref(), Some("id-token-account"));
    }

    #[test]
    fn parses_access_token_account_before_stored_account() {
        let identity = parse_identity_from_auth_json(&format!(
            r#"{{
                "tokens": {{
                    "id_token": "{}",
                    "access_token": "{}",
                    "account_id": "stored-account"
                }}
            }}"#,
            chatgpt_id_token("user@example.com", None),
            chatgpt_access_token_with_auth_object("access-token-account"),
        ))
        .unwrap();

        assert_eq!(identity.account_id.as_deref(), Some("access-token-account"));
    }

    #[test]
    fn parses_access_token_flat_account_claims() {
        let flat_claim = jwt_with_payload(serde_json::json!({
            "https://api.openai.com/auth.chatgpt_account_id": "flat-account",
        }));
        let legacy_claim = jwt_with_payload(serde_json::json!({
            "chatgpt_account_id": "legacy-account",
        }));

        assert_eq!(
            parse_account_id_from_access_token(&flat_claim)
                .unwrap()
                .as_deref(),
            Some("flat-account")
        );
        assert_eq!(
            parse_account_id_from_access_token(&legacy_claim)
                .unwrap()
                .as_deref(),
            Some("legacy-account")
        );
    }

    #[test]
    fn parses_email_from_profile_or_top_level_claim() {
        let top_level = jwt_with_payload(serde_json::json!({
            "email": " top@example.com ",
        }));
        let profile = jwt_with_payload(serde_json::json!({
            "https://api.openai.com/profile": {
                "email": " profile@example.com ",
            },
        }));

        assert_eq!(
            parse_email_from_id_token(&top_level).unwrap().as_deref(),
            Some("top@example.com")
        );
        assert_eq!(
            parse_email_from_id_token(&profile).unwrap().as_deref(),
            Some("profile@example.com")
        );
    }

    #[test]
    fn rejects_invalid_jwt_shape() {
        let err = parse_email_from_id_token("not-a-jwt").unwrap_err();

        assert!(format!("{err:#}").contains("invalid JWT format"));
    }

    #[test]
    fn normalizes_email_and_account_id() {
        assert_eq!(normalize_email(" User@Example.COM "), "user@example.com");
        assert_eq!(
            normalize_optional_email("  user@example.com  ").as_deref(),
            Some("user@example.com")
        );
        assert_eq!(normalize_optional_email("   "), None);
        assert_eq!(normalize_account_id(" acct-one "), "acct-one");
        assert_eq!(
            normalize_optional_account_id(" acct-two ").as_deref(),
            Some("acct-two")
        );
        assert_eq!(normalize_optional_account_id("   "), None);
    }

    fn chatgpt_id_token(email: &str, account_id: Option<&str>) -> String {
        let mut auth = serde_json::Map::new();
        if let Some(account_id) = account_id {
            auth.insert(
                "chatgpt_account_id".to_string(),
                serde_json::Value::String(account_id.to_string()),
            );
        }
        jwt_with_payload(serde_json::json!({
            "https://api.openai.com/profile": {
                "email": email
            },
            "https://api.openai.com/auth": auth,
        }))
    }

    fn chatgpt_access_token_with_auth_object(account_id: &str) -> String {
        jwt_with_payload(serde_json::json!({
            "https://api.openai.com/auth": {
                "chatgpt_account_id": account_id,
            },
        }))
    }

    fn jwt_with_payload(payload: serde_json::Value) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(br#"{"alg":"none","typ":"JWT"}"#);
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(payload.to_string());
        format!("{header}.{payload}.sig")
    }
}
