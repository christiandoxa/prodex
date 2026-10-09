//! Required-Mojo policy for local provider dispatch and upstream request shape.

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const OP_DISPATCH: i64 = 0;
const OP_ROUTE_KIND: i64 = 1;
const OP_STANDARD_PATH: i64 = 2;
const OP_AUTH_SHAPE: i64 = 3;
const OP_ATTEMPT_INDEX: i64 = 4;
#[cfg(test)]
const MAX_ATTEMPTS: usize = 256;

unsafe extern "C" {
    fn prodex_provider_upstream_policy_v1(
        abi_version: i64,
        operation: i64,
        a: i64,
        b: i64,
        c: i64,
        d: i64,
        e: i64,
        f: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_provider_upstream_suffix_plan_v1(
        abi_version: i64,
        mode: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_provider_upstream_attempt_label_v1(
        abi_version: i64,
        candidate_count: i64,
        candidate_index: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderDispatchKind {
    OpenAi = 0,
    Anthropic = 1,
    Copilot = 2,
    DeepSeek = 3,
    Gemini = 4,
    Kiro = 5,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderUpstreamRouteKind {
    Responses = 0,
    Compact = 1,
    Standard = 3,
}

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAuthShape {
    Preserve = 0,
    Bearer = 1,
    AnthropicBetaBearer = 2,
    XApiKey = 3,
    GoogleApiKey = 4,
}

fn policy(operation: i64, values: [i64; 6]) -> Result<i64, MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_provider_upstream_policy_v1(
            ABI_VERSION,
            operation,
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_dispatch_plan(
    provider: i64,
    configured_provider: i64,
    endpoint: i64,
    streaming: bool,
) -> Result<ProviderDispatchKind, MojoError> {
    match policy(
        OP_DISPATCH,
        [
            provider,
            configured_provider,
            endpoint,
            i64::from(streaming),
            0,
            0,
        ],
    )? {
        0 => Ok(ProviderDispatchKind::OpenAi),
        1 => Ok(ProviderDispatchKind::Anthropic),
        2 => Ok(ProviderDispatchKind::Copilot),
        3 => Ok(ProviderDispatchKind::DeepSeek),
        4 => Ok(ProviderDispatchKind::Gemini),
        5 => Ok(ProviderDispatchKind::Kiro),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_route_kind(endpoint: i64) -> Result<ProviderUpstreamRouteKind, MojoError> {
    match policy(OP_ROUTE_KIND, [endpoint, 0, 0, 0, 0, 0])? {
        0 => Ok(ProviderUpstreamRouteKind::Responses),
        1 => Ok(ProviderUpstreamRouteKind::Compact),
        3 => Ok(ProviderUpstreamRouteKind::Standard),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn standard_path_uses_chat(
    wire_format: i64,
    path_is_responses: bool,
) -> Result<bool, MojoError> {
    match policy(
        OP_STANDARD_PATH,
        [wire_format, i64::from(path_is_responses), 0, 0, 0, 0],
    )? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_auth_shape(
    provider: i64,
    credential_present: bool,
    native_messages: bool,
    openai_compatible: bool,
) -> Result<ProviderAuthShape, MojoError> {
    match policy(
        OP_AUTH_SHAPE,
        [
            provider,
            i64::from(credential_present),
            i64::from(native_messages),
            i64::from(openai_compatible),
            0,
            0,
        ],
    )? {
        0 => Ok(ProviderAuthShape::Preserve),
        1 => Ok(ProviderAuthShape::Bearer),
        2 => Ok(ProviderAuthShape::AnthropicBetaBearer),
        3 => Ok(ProviderAuthShape::XApiKey),
        4 => Ok(ProviderAuthShape::GoogleApiKey),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_attempt_index(
    candidate_count: usize,
    start: usize,
    offset: usize,
) -> Result<usize, MojoError> {
    let output = policy(
        OP_ATTEMPT_INDEX,
        [
            i64::try_from(candidate_count).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(start).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(offset).map_err(|_| MojoError::InvalidInput)?,
            0,
            0,
            0,
        ],
    )?;
    usize::try_from(output)
        .ok()
        .filter(|index| *index < candidate_count)
        .ok_or(MojoError::InvalidOutput)
}

pub fn provider_attempt_label(
    candidate_count: usize,
    candidate_index: usize,
) -> Result<String, MojoError> {
    let candidate_count = i64::try_from(candidate_count).map_err(|_| MojoError::InvalidInput)?;
    let candidate_index = i64::try_from(candidate_index).map_err(|_| MojoError::InvalidInput)?;
    let mut output = [0_u8; 32];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_provider_upstream_attempt_label_v1(
            ABI_VERSION,
            candidate_count,
            candidate_index,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn provider_suffix_plan(mode: i64, value: &str) -> Result<i64, MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut output = -1_i64;
    let status = unsafe {
        prodex_provider_upstream_suffix_plan_v1(
            ABI_VERSION,
            mode,
            value.as_ptr() as usize as u64,
            length,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn self_test() -> bool {
    provider_dispatch_plan(0, 0, 0, false).is_ok_and(|value| value == ProviderDispatchKind::OpenAi)
        && provider_route_kind(1).is_ok_and(|value| value == ProviderUpstreamRouteKind::Compact)
        && standard_path_uses_chat(1, true).is_ok_and(|value| value)
        && provider_auth_shape(1, false, true, false)
            .is_ok_and(|value| value == ProviderAuthShape::XApiKey)
        && provider_attempt_index(3, 2, 1).is_ok_and(|value| value == 0)
        && provider_attempt_label(3, 1).is_ok_and(|value| value == "api-key-2")
        && provider_suffix_plan(0, "https://provider.example.com/v1").is_ok_and(|value| value == 2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_mojo_dispatch_and_route_abi_covers_all_branches() {
        for provider in 0..=5 {
            assert_eq!(
                provider_dispatch_plan(provider, provider, 0, false).unwrap() as i64,
                provider
            );
            assert_eq!(
                provider_dispatch_plan(provider, (provider + 1) % 6, 0, false),
                Err(MojoError::InvalidInput)
            );
        }
        assert_eq!(
            provider_route_kind(0).unwrap(),
            ProviderUpstreamRouteKind::Responses
        );
        assert_eq!(
            provider_route_kind(1).unwrap(),
            ProviderUpstreamRouteKind::Compact
        );
        assert_eq!(
            provider_route_kind(2).unwrap(),
            ProviderUpstreamRouteKind::Responses
        );
        assert_eq!(
            provider_route_kind(10).unwrap(),
            ProviderUpstreamRouteKind::Standard
        );
        assert_eq!(provider_route_kind(11), Err(MojoError::InvalidInput));
    }

    #[test]
    fn required_mojo_auth_and_path_abi_covers_precedence_boundaries() {
        assert_eq!(
            provider_auth_shape(0, false, false, false).unwrap(),
            ProviderAuthShape::Preserve
        );
        assert_eq!(
            provider_auth_shape(0, true, false, false).unwrap(),
            ProviderAuthShape::Bearer
        );
        assert_eq!(
            provider_auth_shape(1, false, true, false).unwrap(),
            ProviderAuthShape::XApiKey
        );
        assert_eq!(
            provider_auth_shape(1, true, true, false).unwrap(),
            ProviderAuthShape::AnthropicBetaBearer
        );
        assert_eq!(
            provider_auth_shape(3, false, true, false).unwrap(),
            ProviderAuthShape::XApiKey
        );
        assert_eq!(
            provider_auth_shape(4, false, false, false).unwrap(),
            ProviderAuthShape::GoogleApiKey
        );
        assert_eq!(
            provider_auth_shape(4, false, false, true).unwrap(),
            ProviderAuthShape::Bearer
        );
        assert_eq!(
            provider_auth_shape(5, false, false, false),
            Err(MojoError::InvalidInput)
        );
        assert!(standard_path_uses_chat(1, true).unwrap());
        assert!(!standard_path_uses_chat(1, false).unwrap());
        assert!(!standard_path_uses_chat(0, true).unwrap());
        assert_eq!(
            provider_suffix_plan(0, "https://provider.example.com/anthropic/v1").unwrap(),
            0
        );
        assert_eq!(
            provider_suffix_plan(0, "https://provider.example.com/anthropic").unwrap(),
            1
        );
        assert_eq!(
            provider_suffix_plan(0, "https://provider.example.com/beta").unwrap(),
            2
        );
        assert_eq!(
            provider_suffix_plan(0, "https://provider.example.com").unwrap(),
            3
        );
        assert_eq!(
            provider_suffix_plan(1, "https://provider.example.com/openai").unwrap(),
            0
        );
        assert_eq!(
            provider_suffix_plan(1, "https://provider.example.com/v1beta").unwrap(),
            1
        );
        assert_eq!(
            provider_suffix_plan(2, "https://provider.example.com"),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(
            provider_suffix_plan(0, &"x".repeat(4_097)),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn required_mojo_attempt_abi_rejects_bad_inputs() {
        assert_eq!(provider_attempt_index(1, 0, 0).unwrap(), 0);
        assert_eq!(provider_attempt_index(3, 2, 1).unwrap(), 0);
        assert_eq!(provider_attempt_label(1, 0).unwrap(), "api-key");
        assert_eq!(provider_attempt_label(3, 2).unwrap(), "api-key-3");
        assert_eq!(
            provider_attempt_label(MAX_ATTEMPTS, MAX_ATTEMPTS - 1).unwrap(),
            "api-key-256"
        );
        assert_eq!(
            provider_attempt_index(MAX_ATTEMPTS + 1, 0, 0),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(provider_attempt_label(3, 3), Err(MojoError::InvalidInput));
    }
}
