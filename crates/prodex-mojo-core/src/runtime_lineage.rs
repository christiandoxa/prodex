use crate::MojoError;

const ABI_VERSION: i64 = 1;
const KEY_MAX_BYTES: usize = 4_096;
const CLASS_COMPONENT: i64 = 0;
const CLASS_KEY: i64 = 1;
const CLASS_RESPONSE_TURN_KEY: i64 = 2;
const CLASS_COMPACT_SESSION_KEY: i64 = 3;
const BUILD_COMPACT_SESSION: i64 = 0;
const BUILD_COMPACT_TURN_STATE: i64 = 1;
const BUILD_RESPONSE_TURN_STATE: i64 = 2;

unsafe extern "C" {
    fn prodex_runtime_lineage_classify_v1(
        abi_version: i64,
        kind: i64,
        address: u64,
        length: i64,
    ) -> i64;
    fn prodex_runtime_lineage_build_v1(
        abi_version: i64,
        kind: i64,
        first_address: u64,
        first_length: i64,
        second_address: u64,
        second_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_lineage_parts_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn classify(kind: i64, value: &str) -> Result<bool, MojoError> {
    match unsafe {
        prodex_runtime_lineage_classify_v1(
            ABI_VERSION,
            kind,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    } {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn component_valid(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_COMPONENT, value)
}

pub fn key_valid(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_KEY, value)
}

pub fn is_response_turn_key(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_RESPONSE_TURN_KEY, value)
}

pub fn is_compact_session_key(value: &str) -> Result<bool, MojoError> {
    classify(CLASS_COMPACT_SESSION_KEY, value)
}

fn build(kind: i64, first: &str, second: Option<&str>) -> Result<String, MojoError> {
    let second = second.unwrap_or_default();
    let mut output = vec![0_u8; KEY_MAX_BYTES];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_runtime_lineage_build_v1(
            ABI_VERSION,
            kind,
            first.as_ptr() as usize as u64,
            signed_len(first)?,
            second.as_ptr() as usize as u64,
            signed_len(second)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    if status != 0 {
        return Err(if status == 3 {
            MojoError::Capacity
        } else {
            MojoError::InvalidInput
        });
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    String::from_utf8(
        output
            .get(..written)
            .ok_or(MojoError::InvalidOutput)?
            .to_vec(),
    )
    .map_err(|_| MojoError::InvalidOutput)
}

pub fn compact_session_key(value: &str) -> Result<String, MojoError> {
    build(BUILD_COMPACT_SESSION, value, None)
}

pub fn compact_turn_state_key(value: &str) -> Result<String, MojoError> {
    build(BUILD_COMPACT_TURN_STATE, value, None)
}

pub fn response_turn_state_key(response_id: &str, turn_state: &str) -> Result<String, MojoError> {
    build(BUILD_RESPONSE_TURN_STATE, response_id, Some(turn_state))
}

pub fn response_turn_state_parts(
    key: &str,
) -> Result<Option<(usize, usize, usize, usize)>, MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_runtime_lineage_parts_v1(
            ABI_VERSION,
            key.as_ptr() as usize as u64,
            signed_len(key)?,
            output.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    if output[0] < 0 {
        return Ok(None);
    }
    Ok(Some((
        usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?,
        usize::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?,
    )))
}
