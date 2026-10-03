use anyhow::{Context, Result, bail};
use base64::Engine;
use prodex_mojo_core::profile_identity as mojo_profile_identity;
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
    let records = discovered
        .iter()
        .map(
            |(_, identity)| mojo_profile_identity::ProfileIdentityRecord {
                email: identity.email.as_deref(),
                account_id: identity.account_id.as_deref(),
            },
        )
        .collect::<Vec<_>>();
    let index = mojo_profile_identity::find_matching_profile_identity(
        &records,
        target.account_id.as_deref(),
        target.email.as_deref(),
    )
    .expect("Mojo profile identity matcher returned invalid output")?;
    discovered.get(index).map(|(name, _)| name.clone())
}

fn first_present_identity_value<const N: usize>(values: [Option<String>; N]) -> Option<String> {
    let present = values.each_ref().map(Option::is_some);
    let index = mojo_profile_identity::first_present_identity_source(&present)
        .expect("Mojo profile identity source planner returned invalid output")?;
    values.into_iter().nth(index).flatten()
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
        first_present_identity_value([
            self.auth.and_then(|auth| auth.chatgpt_account_id),
            self.auth_chatgpt_account_id,
            self.chatgpt_account_id,
        ])
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

    let ProfileIdentity {
        email,
        account_id: id_token_account_id,
    } = id_token_identity;
    Ok(ProfileIdentity {
        email,
        account_id: first_present_identity_value([
            id_token_account_id,
            access_token_account_id,
            stored_account_id,
        ]),
    })
}

pub fn parse_identity_from_id_token(raw_jwt: &str) -> Result<ProfileIdentity> {
    let claims: IdTokenClaims = parse_jwt_payload(raw_jwt)?;
    let IdTokenClaims {
        email,
        profile,
        auth,
    } = claims;
    Ok(ProfileIdentity {
        email: first_present_identity_value([email, profile.and_then(|profile| profile.email)])
            .and_then(normalize_optional_email),
        account_id: auth
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
    let (_header_b64, payload_b64, _sig_b64) =
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(header), Some(payload), Some(signature), None)
                if !header.is_empty() && !payload.is_empty() && !signature.is_empty() =>
            {
                (header, payload, signature)
            }
            _ => bail!("invalid JWT format"),
        };

    let payload_bytes = Zeroizing::new(
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(payload_b64)
            .context("failed to decode JWT payload")?,
    );
    if base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&*payload_bytes) != payload_b64 {
        bail!("non-canonical JWT payload encoding");
    }
    serde_json::from_slice(&payload_bytes).context("failed to parse JWT payload JSON")
}

pub fn normalize_email(email: &str) -> String {
    mojo_profile_identity::normalize_email(email)
        .expect("Mojo profile email normalization returned invalid output")
}

pub fn normalize_optional_email(email: impl AsRef<str>) -> Option<String> {
    let normalized = mojo_profile_identity::normalize_email(email.as_ref())
        .expect("Mojo optional email normalization returned invalid output");
    (!normalized.is_empty()).then_some(normalized)
}

pub fn normalize_optional_account_id(account_id: impl AsRef<str>) -> Option<String> {
    let normalized = mojo_profile_identity::normalize_account_id(account_id.as_ref())
        .expect("Mojo optional account normalization returned invalid output");
    (!normalized.is_empty()).then_some(normalized)
}

pub fn normalize_account_id(account_id: &str) -> String {
    mojo_profile_identity::normalize_account_id(account_id)
        .expect("Mojo profile account normalization returned invalid output")
}

pub fn canonical_profile_identity_key(
    account_id: Option<&str>,
    email: Option<&str>,
) -> Option<String> {
    mojo_profile_identity::canonical_profile_identity_key(account_id, email)
        .expect("Mojo canonical profile identity key returned invalid output")
}

pub fn profile_name_from_email(email: &str) -> String {
    mojo_profile_identity::profile_name_from_email(email)
        .expect("Mojo profile name normalization returned invalid output")
}

pub fn profile_name_looks_email_derived_for_other_email(profile_name: &str, email: &str) -> bool {
    mojo_profile_identity::profile_name_looks_email_derived_for_other_email(profile_name, email)
        .expect("Mojo profile name derivation check returned invalid output")
}

pub fn validate_profile_name(name: &str) -> Result<()> {
    use mojo_profile_identity::ProfileNameValidation;
    match mojo_profile_identity::validate_profile_name(name)
        .expect("Mojo profile name validation returned invalid output")
    {
        ProfileNameValidation::Valid => Ok(()),
        ProfileNameValidation::Empty => bail!("profile name cannot be empty"),
        ProfileNameValidation::PathSeparator => {
            bail!("profile name cannot contain path separators")
        }
        ProfileNameValidation::DotPath => bail!("profile name cannot be '.' or '..'"),
        ProfileNameValidation::InvalidCharacter => {
            bail!("profile name may only contain letters, numbers, '.', '_' or '-'")
        }
    }
}

pub fn validate_add_profile_options(
    codex_home_provided: bool,
    copy_from_provided: bool,
    copy_current: bool,
) -> Result<()> {
    resolve_add_profile_source_kind(codex_home_provided, copy_from_provided, copy_current)
        .map(|_| ())
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
    use mojo_profile_identity::AddProfileSourcePlan;
    match mojo_profile_identity::add_profile_source_plan(
        codex_home_provided,
        copy_from_provided,
        copy_current,
    )
    .expect("Mojo add-profile source planner returned invalid output")
    {
        AddProfileSourcePlan::ExternalHome => Ok(AddProfileSourceKind::ExternalHome),
        AddProfileSourcePlan::CopyFrom => Ok(AddProfileSourceKind::CopyFrom),
        AddProfileSourcePlan::CopyCurrent => Ok(AddProfileSourceKind::CopyCurrent),
        AddProfileSourcePlan::EmptyManaged => Ok(AddProfileSourceKind::EmptyManaged),
        AddProfileSourcePlan::ExternalHomeConflict => {
            bail!("--codex-home cannot be combined with --copy-from or --copy-current")
        }
        AddProfileSourcePlan::CopyConflict => bail!("use either --copy-from or --copy-current"),
    }
}

pub fn should_activate_profile(active_profile_exists: bool, activate_requested: bool) -> bool {
    mojo_profile_identity::should_activate_profile(active_profile_exists, activate_requested)
        .expect("Mojo profile activation planner returned invalid output")
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
    let mut attempt = 0_u64;
    loop {
        let candidate =
            mojo_profile_identity::profile_name_candidate(base_name, fallback_name, attempt)
                .expect("Mojo profile-name candidate planner returned invalid output");
        if is_available(&candidate) {
            return candidate;
        }
        attempt = attempt
            .checked_add(1)
            .expect("profile name candidate space exhausted");
    }
}

pub fn resolve_remove_profile_targets<'a>(
    profiles: impl IntoIterator<Item = (&'a str, bool)>,
    remove_all: bool,
    requested_name: Option<&str>,
    delete_home: bool,
) -> Result<Vec<String>> {
    use mojo_profile_identity::RemoveProfileTargetsPlan;

    let profiles = profiles
        .into_iter()
        .map(|(name, managed)| (name.to_string(), managed))
        .collect::<Vec<_>>();
    let records = profiles
        .iter()
        .map(
            |(name, managed)| mojo_profile_identity::ProfileRemovalRecord {
                name,
                managed: *managed,
            },
        )
        .collect::<Vec<_>>();
    match mojo_profile_identity::remove_profile_targets_plan(
        &records,
        remove_all,
        requested_name,
        delete_home,
    )
    .expect("Mojo remove-profile target planner returned invalid output")
    {
        RemoveProfileTargetsPlan::All => Ok(profiles.into_iter().map(|(name, _)| name).collect()),
        RemoveProfileTargetsPlan::One(index) => Ok(vec![profiles[index].0.clone()]),
        RemoveProfileTargetsPlan::MissingRequested => bail!("provide a profile name or pass --all"),
        RemoveProfileTargetsPlan::NotFound => {
            bail!(
                "profile '{}' does not exist",
                requested_name.unwrap_or_default()
            )
        }
        RemoveProfileTargetsPlan::ExternalBulk(external_profiles) => bail!(
            "--delete-home with --all refuses to delete external profiles: {external_profiles}"
        ),
    }
}

pub fn should_delete_profile_home(
    managed_profile: bool,
    delete_home: bool,
    codex_home_label: impl std::fmt::Display,
) -> Result<bool> {
    use mojo_profile_identity::ProfileHomeDeletePlan;
    match mojo_profile_identity::profile_home_delete_plan(managed_profile, delete_home)
        .expect("Mojo profile-home delete planner returned invalid output")
    {
        ProfileHomeDeletePlan::Keep => Ok(false),
        ProfileHomeDeletePlan::Delete => Ok(true),
        ProfileHomeDeletePlan::RejectExternal => {
            bail!("refusing to delete external path {}", codex_home_label)
        }
    }
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
    use mojo_profile_identity::RemovedActiveProfileChoice;

    let removed_names = removed_names
        .into_iter()
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();
    let current_removed =
        current_active_profile.is_some_and(|profile_name| removed_names.contains(profile_name));
    let first_remaining = remaining_profile_names.into_iter().next();
    let active_profile = match mojo_profile_identity::removed_active_profile_choice(
        current_active_profile.is_some(),
        current_removed,
        first_remaining.is_some(),
    )
    .expect("Mojo removed-profile active selection returned invalid output")
    {
        RemovedActiveProfileChoice::None => None,
        RemovedActiveProfileChoice::Current => current_active_profile.map(ToOwned::to_owned),
        RemovedActiveProfileChoice::FirstRemaining => first_remaining.map(ToOwned::to_owned),
    };

    RemovedProfileStatePlan {
        removed_names,
        active_profile,
    }
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
