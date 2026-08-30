use super::*;

pub const APPLICATION_METADATA_ABI_VERSION: i64 = 1;
pub const APPLICATION_METADATA_MAX_HEADERS: usize = 64;

/// Presence-only metadata derived from bounded header-name views.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApplicationRequestMetadataPlan {
    pub observed_header_count: usize,
    pub headers_truncated: bool,
    pub trace_context_present: bool,
    pub credential_present: bool,
    pub affinity_present: bool,
    pub codex_metadata_present: bool,
    pub user_agent_present: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct ApplicationRequestMetadataResult {
    abi_version: i64,
    observed_header_count: i64,
    headers_truncated: i64,
    trace_context_present: i64,
    credential_present: i64,
    affinity_present: i64,
    codex_metadata_present: i64,
    user_agent_present: i64,
}

const _: () = assert!(std::mem::size_of::<ApplicationRequestMetadataResult>() == 64);

unsafe extern "C" {
    fn prodex_mojo_rich_application_request_metadata_v1(
        abi_version: i64,
        header_names: u64,
        header_count: i64,
        total_header_count: i64,
        result: u64,
    ) -> i64;
}

/// Normalize header-name DTOs without crossing header values or credentials.
pub fn normalize_application_request_metadata(
    header_names: &[&str],
    total_header_count: usize,
) -> Result<ApplicationRequestMetadataPlan, MojoError> {
    ensure_rich_abi()?;
    if header_names.len() > APPLICATION_METADATA_MAX_HEADERS
        || total_header_count < header_names.len()
    {
        return Err(MojoError::InvalidInput);
    }
    let header_names = header_names
        .iter()
        .map(|name| view(name))
        .collect::<Vec<_>>();
    let mut result = ApplicationRequestMetadataResult::default();
    let status = unsafe {
        prodex_mojo_rich_application_request_metadata_v1(
            APPLICATION_METADATA_ABI_VERSION,
            mojo_pointer_address(header_names.as_ptr()),
            i64::try_from(header_names.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(total_header_count).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 10, 0, 0, 0));
    }
    if result.abi_version != APPLICATION_METADATA_ABI_VERSION
        || result.observed_header_count < 0
        || result.observed_header_count as usize > APPLICATION_METADATA_MAX_HEADERS
        || result.observed_header_count as usize != total_header_count.min(header_names.len())
        || result.headers_truncated != i64::from(total_header_count > header_names.len())
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(ApplicationRequestMetadataPlan {
        observed_header_count: result.observed_header_count as usize,
        headers_truncated: decode_flag(result.headers_truncated)?,
        trace_context_present: decode_flag(result.trace_context_present)?,
        credential_present: decode_flag(result.credential_present)?,
        affinity_present: decode_flag(result.affinity_present)?,
        codex_metadata_present: decode_flag(result.codex_metadata_present)?,
        user_agent_present: decode_flag(result.user_agent_present)?,
    })
}

fn decode_flag(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
