use anyhow::{Context, Result, bail};
use base64::Engine;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

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

pub fn find_matching_profile_identity(
    discovered: &[(String, ProfileIdentity)],
    target: &ProfileIdentity,
) -> Option<String> {
    let target_account_id = target.account_id.as_deref().map(normalize_account_id);
    let target_email = target.email.as_deref().map(normalize_email);

    if let Some(target_account_id) = target_account_id.as_deref() {
        for (name, identity) in discovered {
            if identity
                .account_id
                .as_deref()
                .is_some_and(|account_id| normalize_account_id(account_id) == target_account_id)
                && profile_identity_email_matches_target(identity, target_email.as_deref())
            {
                return Some(name.clone());
            }
        }
    }

    let target_email = target_email.as_deref()?;
    let mut legacy_email_matches = discovered
        .iter()
        .filter(|(_, identity)| {
            identity.account_id.is_none()
                && identity
                    .email
                    .as_deref()
                    .is_some_and(|email| normalize_email(email) == target_email)
        })
        .map(|(name, _)| name.clone());
    let match_name = legacy_email_matches.next()?;
    legacy_email_matches.next().is_none().then_some(match_name)
}

fn profile_identity_email_matches_target(
    identity: &ProfileIdentity,
    target_email: Option<&str>,
) -> bool {
    let Some(target_email) = target_email else {
        return true;
    };

    identity
        .email
        .as_deref()
        .is_some_and(|email| normalize_email(email) == target_email)
}

#[derive(Deserialize)]
struct StoredAuth {
    tokens: Option<StoredTokens>,
}

impl fmt::Debug for StoredAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredAuth")
            .field("tokens", &self.tokens)
            .finish()
    }
}

impl Zeroize for StoredAuth {
    fn zeroize(&mut self) {
        self.tokens.zeroize();
    }
}

impl Drop for StoredAuth {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for StoredAuth {}

#[derive(Deserialize)]
struct StoredTokens {
    access_token: Option<String>,
    account_id: Option<String>,
    id_token: Option<String>,
}

impl fmt::Debug for StoredTokens {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StoredTokens")
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "account_id",
                &self.account_id.as_ref().map(|_| "<redacted>"),
            )
            .field("id_token", &self.id_token.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl Zeroize for StoredTokens {
    fn zeroize(&mut self) {
        self.access_token.zeroize();
        self.account_id.zeroize();
        self.id_token.zeroize();
    }
}

impl Drop for StoredTokens {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl ZeroizeOnDrop for StoredTokens {}

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

    let payload_bytes = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload_b64)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(payload_b64))
            .context("failed to decode JWT payload")?,
    );
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

pub fn profile_name_from_email(email: &str) -> String {
    let normalized = normalize_email(email);
    let mut profile_name = String::new();

    for ch in normalized.chars() {
        match ch {
            'a'..='z' | '0'..='9' | '.' | '_' | '-' => profile_name.push(ch),
            '@' => profile_name.push('_'),
            _ => profile_name.push('-'),
        }
    }

    let profile_name = profile_name
        .trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string();
    if profile_name.is_empty() || profile_name == "." || profile_name == ".." {
        "profile".to_string()
    } else {
        profile_name
    }
}

pub fn profile_name_looks_email_derived_for_other_email(profile_name: &str, email: &str) -> bool {
    let target_base = profile_name_from_email(email);
    let candidate_base = strip_unique_profile_suffix(profile_name);

    candidate_base != target_base && profile_name_base_looks_email_derived(candidate_base)
}

fn strip_unique_profile_suffix(profile_name: &str) -> &str {
    let Some((base, suffix)) = profile_name.rsplit_once('-') else {
        return profile_name;
    };

    if !suffix.is_empty()
        && suffix.chars().all(|ch| ch.is_ascii_digit())
        && profile_name_base_looks_email_derived(base)
    {
        base
    } else {
        profile_name
    }
}

fn profile_name_base_looks_email_derived(profile_name: &str) -> bool {
    let Some((local, domain)) = profile_name.rsplit_once('_') else {
        return false;
    };

    !local.is_empty()
        && domain.contains('.')
        && domain
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-'))
        && domain
            .split('.')
            .all(|label| !label.is_empty() && !label.starts_with('-') && !label.ends_with('-'))
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("profile name cannot be empty");
    }

    if name.contains(std::path::MAIN_SEPARATOR) {
        bail!("profile name cannot contain path separators");
    }

    if name == "." || name == ".." {
        bail!("profile name cannot be '.' or '..'");
    }

    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        bail!("profile name may only contain letters, numbers, '.', '_' or '-'");
    }

    Ok(())
}

pub fn validate_add_profile_options(
    codex_home_provided: bool,
    copy_from_provided: bool,
    copy_current: bool,
) -> Result<()> {
    if codex_home_provided && (copy_from_provided || copy_current) {
        bail!("--codex-home cannot be combined with --copy-from or --copy-current");
    }

    if copy_from_provided && copy_current {
        bail!("use either --copy-from or --copy-current");
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddProfileSourceKind {
    ExternalHome,
    CopyFrom,
    CopyCurrent,
    EmptyManaged,
}

impl AddProfileSourceKind {
    pub fn managed(self) -> bool {
        !matches!(self, Self::ExternalHome)
    }
}

pub fn resolve_add_profile_source_kind(
    codex_home_provided: bool,
    copy_from_provided: bool,
    copy_current: bool,
) -> Result<AddProfileSourceKind> {
    validate_add_profile_options(codex_home_provided, copy_from_provided, copy_current)?;
    Ok(if codex_home_provided {
        AddProfileSourceKind::ExternalHome
    } else if copy_current {
        AddProfileSourceKind::CopyCurrent
    } else if copy_from_provided {
        AddProfileSourceKind::CopyFrom
    } else {
        AddProfileSourceKind::EmptyManaged
    })
}

pub fn should_activate_profile(active_profile_exists: bool, activate_requested: bool) -> bool {
    !active_profile_exists || activate_requested
}

pub fn unique_profile_name_for_email(
    email: &str,
    is_available: impl FnMut(&str) -> bool,
) -> String {
    unique_profile_name_from_base(&profile_name_from_email(email), "profile", is_available)
}

pub fn copilot_profile_name_base(login: &str) -> String {
    profile_name_from_email(&format!("copilot-{login}"))
}

pub fn unique_copilot_profile_name(login: &str, is_available: impl FnMut(&str) -> bool) -> String {
    unique_profile_name_from_base(&copilot_profile_name_base(login), "copilot", is_available)
}

pub fn unique_profile_name_from_base(
    base_name: &str,
    fallback_name: &str,
    mut is_available: impl FnMut(&str) -> bool,
) -> String {
    let base_name = if base_name.trim().is_empty() {
        fallback_name.to_string()
    } else {
        base_name.to_string()
    };
    if is_available(&base_name) {
        return base_name;
    }

    for suffix in 2.. {
        let candidate = format!("{base_name}-{suffix}");
        if is_available(&candidate) {
            return candidate;
        }
    }

    unreachable!("integer suffix space should not be exhausted")
}

pub fn resolve_remove_profile_targets<'a>(
    profiles: impl IntoIterator<Item = (&'a str, bool)>,
    remove_all: bool,
    requested_name: Option<&str>,
    delete_home: bool,
) -> Result<Vec<String>> {
    let profiles = profiles
        .into_iter()
        .map(|(name, managed)| (name.to_string(), managed))
        .collect::<Vec<_>>();

    let target_names = if remove_all {
        profiles
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>()
    } else {
        let Some(name) = requested_name else {
            bail!("provide a profile name or pass --all");
        };
        if !profiles
            .iter()
            .any(|(profile_name, _)| profile_name == name)
        {
            bail!("profile '{}' does not exist", name);
        }
        vec![name.to_string()]
    };

    if remove_all && delete_home {
        let external_profiles = profiles
            .iter()
            .filter(|(name, managed)| !managed && target_names.contains(name))
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();
        if !external_profiles.is_empty() {
            bail!(
                "--delete-home with --all refuses to delete external profiles: {}",
                external_profiles.join(", ")
            );
        }
    }

    Ok(target_names)
}

pub fn should_delete_profile_home(
    managed_profile: bool,
    delete_home: bool,
    codex_home_label: impl std::fmt::Display,
) -> Result<bool> {
    let should_delete_home = managed_profile || delete_home;
    if !should_delete_home {
        return Ok(false);
    }

    if !managed_profile && delete_home {
        bail!("refusing to delete external path {}", codex_home_label);
    }

    Ok(true)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedProfileStatePlan {
    pub removed_names: BTreeSet<String>,
    pub active_profile: Option<String>,
}

pub fn plan_removed_profile_state<'a>(
    remaining_profile_names: impl IntoIterator<Item = &'a str>,
    current_active_profile: Option<&str>,
    removed_names: impl IntoIterator<Item = &'a str>,
) -> RemovedProfileStatePlan {
    let removed_names = removed_names
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    let active_profile = if current_active_profile
        .is_some_and(|profile_name| removed_names.contains(profile_name))
    {
        remaining_profile_names
            .into_iter()
            .next()
            .map(ToOwned::to_owned)
    } else {
        current_active_profile.map(ToOwned::to_owned)
    };

    RemovedProfileStatePlan {
        removed_names,
        active_profile,
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
