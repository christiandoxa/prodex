use crate::MojoError;
use std::ops::Range;
use std::time::{SystemTime, UNIX_EPOCH};

const ABI_VERSION: i64 = 1;
const PAIR_SET_COOKIE: i64 = 0;
const PAIR_CALLER_NAME: i64 = 1;
const MAX_EVICTION_TIMESTAMPS: usize = 129 * 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CookiePairPlan {
    pub name: Range<usize>,
    pub value: Range<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CookieAttributePlan {
    Ignore,
    Secure,
    Path(Range<usize>),
    MaxAge(i64),
    Expires(Range<usize>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CookieDefaultPathPlan {
    Root,
    Prefix(usize),
}

unsafe extern "C" {
    fn prodex_runtime_cookie_pair_plan_v1(
        abi_version: i64,
        mode: i64,
        input_address: u64,
        input_length: i64,
        max_name_bytes: i64,
        max_value_bytes: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_cookie_attribute_plan_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        max_path_bytes: i64,
        max_age_seen: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_cookie_default_path_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_cookie_path_matches_v1(
        abi_version: i64,
        request_address: u64,
        request_length: i64,
        cookie_address: u64,
        cookie_length: i64,
    ) -> i64;
    fn prodex_runtime_cookie_scheme_secure_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
    ) -> i64;
    fn prodex_runtime_cookie_host_normalize_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_cookie_oldest_timestamp_index_v1(
        abi_version: i64,
        timestamps_address: u64,
        count: i64,
        output_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn text_args(value: &str) -> Result<(u64, i64), MojoError> {
    Ok((
        value.as_ptr() as usize as u64,
        i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
    ))
}

fn range(input: &str, start: i64, end: i64) -> Result<Range<usize>, MojoError> {
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if start > end
        || end > input.len()
        || !input.is_char_boundary(start)
        || !input.is_char_boundary(end)
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(start..end)
}

fn pair_output(
    mode: i64,
    input: &str,
    max_name_bytes: usize,
    max_value_bytes: usize,
) -> Result<Option<[i64; 5]>, MojoError> {
    let (address, length) = text_args(input)?;
    let mut output = [0_i64; 5];
    status(unsafe {
        prodex_runtime_cookie_pair_plan_v1(
            ABI_VERSION,
            mode,
            address,
            length,
            i64::try_from(max_name_bytes).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_value_bytes).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(None),
        1 => Ok(Some(output)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn set_cookie_pair(
    input: &str,
    max_name_bytes: usize,
    max_value_bytes: usize,
) -> Result<Option<CookiePairPlan>, MojoError> {
    let Some(output) = pair_output(PAIR_SET_COOKIE, input, max_name_bytes, max_value_bytes)? else {
        return Ok(None);
    };
    Ok(Some(CookiePairPlan {
        name: range(input, output[1], output[2])?,
        value: range(input, output[3], output[4])?,
    }))
}

pub fn caller_cookie_name(
    input: &str,
    max_name_bytes: usize,
) -> Result<Option<Range<usize>>, MojoError> {
    let Some(output) = pair_output(PAIR_CALLER_NAME, input, max_name_bytes, 0)? else {
        return Ok(None);
    };
    Ok(Some(range(input, output[1], output[2])?))
}

pub fn attribute_plan(
    input: &str,
    max_path_bytes: usize,
    max_age_seen: bool,
) -> Result<CookieAttributePlan, MojoError> {
    let (address, length) = text_args(input)?;
    let mut output = [0_i64; 4];
    status(unsafe {
        prodex_runtime_cookie_attribute_plan_v1(
            ABI_VERSION,
            address,
            length,
            i64::try_from(max_path_bytes).map_err(|_| MojoError::InvalidInput)?,
            i64::from(max_age_seen),
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(CookieAttributePlan::Ignore),
        1 => Ok(CookieAttributePlan::Secure),
        2 => Ok(CookieAttributePlan::Path(range(
            input, output[1], output[2],
        )?)),
        3 => Ok(CookieAttributePlan::MaxAge(output[3])),
        4 => Ok(CookieAttributePlan::Expires(range(
            input, output[1], output[2],
        )?)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn default_path_plan(path: &str) -> Result<CookieDefaultPathPlan, MojoError> {
    let (address, length) = text_args(path)?;
    let mut output = [0_i64; 2];
    status(unsafe {
        prodex_runtime_cookie_default_path_v1(
            ABI_VERSION,
            address,
            length,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    match output[0] {
        0 => Ok(CookieDefaultPathPlan::Root),
        1 => {
            let end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
            if end > path.len() || !path.is_char_boundary(end) {
                return Err(MojoError::InvalidOutput);
            }
            Ok(CookieDefaultPathPlan::Prefix(end))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_status(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        -2 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn path_matches(request_path: &str, cookie_path: &str) -> Result<bool, MojoError> {
    let (request_address, request_length) = text_args(request_path)?;
    let (cookie_address, cookie_length) = text_args(cookie_path)?;
    bool_status(unsafe {
        prodex_runtime_cookie_path_matches_v1(
            ABI_VERSION,
            request_address,
            request_length,
            cookie_address,
            cookie_length,
        )
    })
}

pub fn scheme_is_secure(scheme: &str) -> Result<bool, MojoError> {
    let (address, length) = text_args(scheme)?;
    bool_status(unsafe { prodex_runtime_cookie_scheme_secure_v1(ABI_VERSION, address, length) })
}

pub fn normalize_host(host: &str) -> Result<Option<String>, MojoError> {
    let (address, length) = text_args(host)?;
    let mut output = vec![0_u8; host.len()];
    let mut written = -1_i64;
    status(unsafe {
        prodex_runtime_cookie_host_normalize_v1(
            ABI_VERSION,
            address,
            length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    if written == 0 {
        return Ok(None);
    }
    Ok(Some(
        String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)?,
    ))
}

pub fn oldest_timestamp_index(
    timestamps: &[SystemTime],
) -> Result<Option<usize>, MojoError> {
    if timestamps.len() > MAX_EVICTION_TIMESTAMPS {
        return Err(MojoError::InvalidInput);
    }

    let mut input = Vec::with_capacity(timestamps.len() * 3);
    for timestamp in timestamps {
        let (after_epoch, duration) = match timestamp.duration_since(UNIX_EPOCH) {
            Ok(duration) => (1, duration),
            Err(error) => (0, error.duration()),
        };
        input.extend([
            after_epoch,
            duration.as_secs(),
            u64::from(duration.subsec_nanos()),
        ]);
    }

    let mut output = -2_i64;
    status(unsafe {
        prodex_runtime_cookie_oldest_timestamp_index_v1(
            ABI_VERSION,
            input.as_ptr() as usize as u64,
            i64::try_from(timestamps.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut output as *mut i64) as usize as u64,
        )
    })?;
    match output {
        -1 => Ok(None),
        index if index >= 0 => {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            if index < timestamps.len() {
                Ok(Some(index))
            } else {
                Err(MojoError::InvalidOutput)
            }
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_cookie_text_policy_preserves_parser_contracts() {
        let pair = set_cookie_pair("  sid = token  ", 256, 8192)
            .unwrap()
            .unwrap();
        assert_eq!(&"  sid = token  "[pair.name], "sid");
        assert_eq!(&"  sid = token  "[pair.value], "token");
        assert!(set_cookie_pair("bad name=x", 256, 8192).unwrap().is_none());
        assert_eq!(
            caller_cookie_name(" sid = any value ", 256)
                .unwrap()
                .map(|range| &" sid = any value "[range]),
            Some("sid")
        );

        assert_eq!(
            attribute_plan(" SeCuRe ", 4096, false).unwrap(),
            CookieAttributePlan::Secure
        );
        assert_eq!(
            attribute_plan(" Path = /api ", 4096, false).unwrap(),
            CookieAttributePlan::Path(8..12)
        );
        assert_eq!(
            attribute_plan(" Max-Age = -1 ", 4096, false).unwrap(),
            CookieAttributePlan::MaxAge(-1)
        );
        assert_eq!(
            attribute_plan(" Max-Age = +1 ", 4096, false).unwrap(),
            CookieAttributePlan::Ignore
        );
        assert!(matches!(
            attribute_plan(" Expires = Tue, 01 Jan 2030 00:00:00 GMT ", 4096, false).unwrap(),
            CookieAttributePlan::Expires(_)
        ));
        assert_eq!(
            attribute_plan(" Expires = Tue, 01 Jan 2030 00:00:00 GMT ", 4096, true).unwrap(),
            CookieAttributePlan::Ignore
        );

        assert_eq!(
            default_path_plan("/backend-api/responses").unwrap(),
            CookieDefaultPathPlan::Prefix(12)
        );
        assert_eq!(default_path_plan("/").unwrap(), CookieDefaultPathPlan::Root);
        assert!(path_matches("/api/responses", "/api").unwrap());
        assert!(!path_matches("/apix", "/api").unwrap());
        assert!(scheme_is_secure("https").unwrap());
        assert!(scheme_is_secure("wss").unwrap());
        assert!(!scheme_is_secure("HTTPS").unwrap());
        assert_eq!(
            normalize_host("  .CHATGPT.COM. ").unwrap().as_deref(),
            Some("chatgpt.com")
        );
    }
}
