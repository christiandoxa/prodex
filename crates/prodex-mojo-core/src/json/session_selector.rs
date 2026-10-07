use super::{StringView, status};
use crate::MojoError;

unsafe extern "C" {
    fn prodex_session_selector_is_full_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_session_selector_matches_v1(
        abi_version: i64,
        id_address: u64,
        id_length: i64,
        selector_address: u64,
        selector_length: i64,
        exact: i64,
        output_address: u64,
    ) -> i64;
}

const SESSION_SELECTOR_ABI_VERSION: i64 = 1;

fn view(value: &str) -> StringView {
    value.into()
}

pub fn session_selector_is_full(value: &str) -> Result<bool, MojoError> {
    let mut output = [-1_i64; 1];
    let value = view(value);
    status(unsafe {
        prodex_session_selector_is_full_v1(
            SESSION_SELECTOR_ABI_VERSION,
            value.address,
            i64::try_from(value.length).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn session_selector_matches(id: &str, selector: &str, exact: bool) -> Result<bool, MojoError> {
    let mut output = [-1_i64; 1];
    let id = view(id);
    let selector = view(selector);
    status(unsafe {
        prodex_session_selector_matches_v1(
            SESSION_SELECTOR_ABI_VERSION,
            id.address,
            i64::try_from(id.length).map_err(|_| MojoError::InvalidInput)?,
            selector.address,
            i64::try_from(selector.length).map_err(|_| MojoError::InvalidInput)?,
            i64::from(exact),
            output.as_mut_ptr() as u64,
        )
    })?;
    match output[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}
