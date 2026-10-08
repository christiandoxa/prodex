//! Mojo-owned provider login validation and lifecycle decisions.

use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum ProfileLoginPolicyOperation {
    ValidateProvider = 0,
    ValidateTarget = 1,
    Execution = 2,
    AuthCommit = 3,
    AutoRoute = 4,
    MethodRoute = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderLoginValidation {
    Allowed,
    CodexProviderUnsupported,
    ClaudeProviderUnsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginTargetValidation {
    Valid,
    Missing,
    Changed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginExecution {
    DirectProfileHome,
    TemporaryLoginHome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMethodRoute {
    ExternalClaude,
    DirectApiKey,
    CodexChild,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginMethodPlan {
    pub route: LoginMethodRoute,
    pub allows_base_url: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoLoginRoute {
    Status,
    Anthropic,
    ApiKey,
    Identity,
    AuthLabelRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthCommitPlan {
    pub is_api_key: bool,
    pub clear_email: bool,
    pub write_base_url: bool,
}

unsafe extern "C" {
    fn prodex_profile_login_policy_v1(
        abi_version: i64,
        operation: i64,
        label_address: u64,
        label_length: i64,
        method: i64,
        input0: i64,
        input1: i64,
        output_address: u64,
    ) -> i64;
}

fn call(
    operation: ProfileLoginPolicyOperation,
    label: Option<&str>,
    method: i64,
    input0: i64,
    input1: i64,
) -> Result<[i64; 4], MojoError> {
    let (label_address, label_length) = match label {
        Some(label) => (
            label.as_ptr() as usize as u64,
            i64::try_from(label.len()).map_err(|_| MojoError::InvalidInput)?,
        ),
        None => (0, 0),
    };
    let mut output = [0_i64; 4];
    let status = unsafe {
        prodex_profile_login_policy_v1(
            ABI_VERSION,
            operation as i64,
            label_address,
            label_length,
            method,
            input0,
            input1,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn boolean(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Validates whether a provider can receive the selected login method.
pub fn validate_provider_login(
    provider_label: &str,
    login_method: i64,
) -> Result<ProviderLoginValidation, MojoError> {
    match call(
        ProfileLoginPolicyOperation::ValidateProvider,
        Some(provider_label),
        login_method,
        0,
        0,
    )?[0]
    {
        0 => Ok(ProviderLoginValidation::Allowed),
        1 => Ok(ProviderLoginValidation::CodexProviderUnsupported),
        2 => Ok(ProviderLoginValidation::ClaudeProviderUnsupported),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Decides whether the profile snapshot still owns the login target.
pub fn validate_login_target(
    profile_present: bool,
    profile_matches_snapshot: bool,
) -> Result<LoginTargetValidation, MojoError> {
    let output = call(
        ProfileLoginPolicyOperation::ValidateTarget,
        None,
        0,
        i64::from(profile_present),
        i64::from(profile_matches_snapshot),
    )?;
    match output[0] {
        0 => Ok(LoginTargetValidation::Valid),
        1 => Ok(LoginTargetValidation::Missing),
        2 => Ok(LoginTargetValidation::Changed),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Chooses the filesystem side-effect shape for a named login.
pub fn login_execution(login_method: i64) -> Result<LoginExecution, MojoError> {
    match call(
        ProfileLoginPolicyOperation::Execution,
        None,
        login_method,
        0,
        0,
    )?[0]
    {
        0 => Ok(LoginExecution::DirectProfileHome),
        1 => Ok(LoginExecution::TemporaryLoginHome),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans the non-secret state changes after an auth file is read.
pub fn auth_commit_plan(
    auth_label: &str,
    base_url_specified: bool,
) -> Result<AuthCommitPlan, MojoError> {
    let output = call(
        ProfileLoginPolicyOperation::AuthCommit,
        Some(auth_label),
        0,
        i64::from(base_url_specified),
        0,
    )?;
    Ok(AuthCommitPlan {
        is_api_key: boolean(output[0])?,
        clear_email: boolean(output[1])?,
        write_base_url: boolean(output[2])?,
    })
}

/// Chooses the auto-login state transition without moving auth contents into Mojo.
pub fn auto_login_route(
    login_method: i64,
    auth_label: Option<&str>,
) -> Result<AutoLoginRoute, MojoError> {
    match call(
        ProfileLoginPolicyOperation::AutoRoute,
        auth_label,
        login_method,
        i64::from(auth_label.is_some()),
        0,
    )?[0]
    {
        0 => Ok(AutoLoginRoute::Status),
        1 => Ok(AutoLoginRoute::Anthropic),
        2 => Ok(AutoLoginRoute::ApiKey),
        3 => Ok(AutoLoginRoute::Identity),
        4 => Ok(AutoLoginRoute::AuthLabelRequired),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Chooses the credential/process boundary without moving credential bytes into Mojo.
pub fn login_method_plan(
    login_method: i64,
    api_key_present: bool,
) -> Result<LoginMethodPlan, MojoError> {
    let output = call(
        ProfileLoginPolicyOperation::MethodRoute,
        None,
        login_method,
        i64::from(api_key_present),
        0,
    )?;
    let route = match output[0] {
        0 => LoginMethodRoute::ExternalClaude,
        1 => LoginMethodRoute::DirectApiKey,
        2 => LoginMethodRoute::CodexChild,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(LoginMethodPlan {
        route,
        allows_base_url: boolean(output[1])?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_login_matrix_is_mojo_owned() {
        assert_eq!(
            validate_provider_login("openai", 0).unwrap(),
            ProviderLoginValidation::Allowed
        );
        assert_eq!(
            validate_provider_login("gemini", 4).unwrap(),
            ProviderLoginValidation::ClaudeProviderUnsupported
        );
        assert_eq!(
            validate_provider_login("anthropic", 0).unwrap(),
            ProviderLoginValidation::CodexProviderUnsupported
        );
        assert_eq!(
            validate_provider_login("kiro", 4).unwrap(),
            ProviderLoginValidation::Allowed
        );
    }

    #[test]
    fn lifecycle_policy_keeps_status_and_api_key_boundaries() {
        assert_eq!(
            login_execution(6).unwrap(),
            LoginExecution::DirectProfileHome
        );
        assert_eq!(
            login_execution(0).unwrap(),
            LoginExecution::TemporaryLoginHome
        );
        assert_eq!(
            auth_commit_plan("api-key", true).unwrap(),
            AuthCommitPlan {
                is_api_key: true,
                clear_email: true,
                write_base_url: true,
            }
        );
        assert_eq!(
            auto_login_route(4, None).unwrap(),
            AutoLoginRoute::Anthropic
        );
        assert_eq!(
            auto_login_route(0, None).unwrap(),
            AutoLoginRoute::AuthLabelRequired
        );
        assert_eq!(
            login_method_plan(2, true).unwrap(),
            LoginMethodPlan {
                route: LoginMethodRoute::DirectApiKey,
                allows_base_url: true,
            }
        );
    }
}
