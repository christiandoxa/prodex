use super::*;

pub const APPLICATION_SCOPE_ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationScopeOperation {
    DimensionsUnrestricted = 0,
    TenantMatches = 1,
    DimensionsMatch = 2,
    AllMatch = 3,
    ResourceNameMatches = 4,
    ResourcePrefixesAllowed = 5,
}

pub struct ApplicationScopeInput<'a> {
    pub operation: ApplicationScopeOperation,
    pub scope_values: [Option<&'a str>; 5],
    pub values: [Option<&'a str>; 5],
    pub scope_prefixes: &'a [&'a str],
    pub candidates: &'a [&'a str],
}

unsafe extern "C" {
    fn prodex_mojo_rich_application_governance_scope_v1(
        abi_version: i64,
        operation: i64,
        scope_values: u64,
        scope_present_mask: i64,
        values: u64,
        value_present_mask: i64,
        scope_prefixes: u64,
        scope_prefix_count: i64,
        candidates: u64,
        candidate_count: i64,
        result: u64,
    ) -> i64;
}

pub fn application_governance_scope(input: ApplicationScopeInput<'_>) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let scope_values = input.scope_values.map(|value| view(value.unwrap_or("")));
    let values = input.values.map(|value| view(value.unwrap_or("")));
    let scope_prefixes = input
        .scope_prefixes
        .iter()
        .map(|value| view(value))
        .collect::<Vec<_>>();
    let candidates = input
        .candidates
        .iter()
        .map(|value| view(value))
        .collect::<Vec<_>>();
    let mut result = -1_i64;
    let status = unsafe {
        prodex_mojo_rich_application_governance_scope_v1(
            APPLICATION_SCOPE_ABI_VERSION,
            input.operation as i64,
            mojo_pointer_address(scope_values.as_ptr()),
            presence_mask(input.scope_values),
            mojo_pointer_address(values.as_ptr()),
            presence_mask(input.values),
            mojo_pointer_address(scope_prefixes.as_ptr()),
            i64::try_from(scope_prefixes.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(candidates.as_ptr()),
            i64::try_from(candidates.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 11, 0, 0, 0));
    }
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn presence_mask(values: [Option<&str>; 5]) -> i64 {
    values.iter().enumerate().fold(0, |mask, (index, value)| {
        mask | if value.is_some() { 1 << index } else { 0 }
    })
}
