use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MODE_ROUTE_LABEL: i64 = 11;
const MODE_ROUTE_FROM_LABEL: i64 = 12;
const MODE_ROUTE_COUPLED: i64 = 13;
const MODE_ROUTE_BUILD_KEY: i64 = 14;
const MODE_ROUTE_KEY_PARTS: i64 = 15;
const MODE_ROUTE_PROFILE_SUFFIX: i64 = 16;
const MODE_ROUTE_CIRCUIT_HEALTH_KEY: i64 = 17;

pub const ROUTE_KEY_HEALTH: u8 = 0;
pub const ROUTE_KEY_BAD_PAIRING: u8 = 1;
pub const ROUTE_KEY_SUCCESS_STREAK: u8 = 2;
pub const ROUTE_KEY_PERFORMANCE: u8 = 3;
pub const ROUTE_KEY_CIRCUIT: u8 = 4;
pub const ROUTE_KEY_CIRCUIT_REOPEN: u8 = 5;
pub const ROUTE_KEY_TRANSPORT_BACKOFF: u8 = 6;

unsafe extern "C" {
    fn prodex_runtime_state_route_policy_v1(
        abi_version: i64,
        mode: i64,
        route_kind: i64,
        key_kind: i64,
        key_address: u64,
        key_length: i64,
        prefix_address: u64,
        prefix_length: i64,
        profile_address: u64,
        profile_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        spans_address: u64,
    ) -> i64;
}

fn route_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

struct RouteCall<'a> {
    mode: i64,
    route_kind: u8,
    key_kind: u8,
    key: &'a str,
    prefix: &'a str,
    profile: &'a str,
    output: Option<&'a mut [u8]>,
    written: Option<&'a mut i64>,
    spans: Option<&'a mut [i64]>,
}

fn route_call(mut input: RouteCall<'_>) -> Result<(), MojoError> {
    let (output_address, output_capacity) = match input.output.as_mut() {
        Some(buffer) => (
            buffer.as_mut_ptr() as usize as u64,
            i64::try_from(buffer.len()).map_err(|_| MojoError::InvalidInput)?,
        ),
        None => (0, 0),
    };
    let written_address = input
        .written
        .map_or(0, |value| value as *mut i64 as usize as u64);
    let spans_address = input
        .spans
        .as_mut()
        .map_or(0, |values| values.as_mut_ptr() as usize as u64);
    let status = unsafe {
        prodex_runtime_state_route_policy_v1(
            ABI_VERSION,
            input.mode,
            i64::from(input.route_kind),
            i64::from(input.key_kind),
            input.key.as_ptr() as usize as u64,
            i64::try_from(input.key.len()).map_err(|_| MojoError::InvalidInput)?,
            input.prefix.as_ptr() as usize as u64,
            i64::try_from(input.prefix.len()).map_err(|_| MojoError::InvalidInput)?,
            input.profile.as_ptr() as usize as u64,
            i64::try_from(input.profile.len()).map_err(|_| MojoError::InvalidInput)?,
            output_address,
            output_capacity,
            written_address,
            spans_address,
        )
    };
    route_status(status)
}

fn load_route_kind_label(kind: u8) -> Result<String, MojoError> {
    let mut output = [0_u8; 16];
    let mut written = 0_i64;
    route_call(RouteCall {
        mode: MODE_ROUTE_LABEL,
        route_kind: kind,
        key_kind: 0,
        key: "",
        prefix: "",
        profile: "",
        output: Some(&mut output),
        written: Some(&mut written),
        spans: None,
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn route_kind_label(kind: u8) -> Result<&'static str, MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, MojoError>> = OnceLock::new();
    if kind > 3 {
        return Err(MojoError::InvalidInput);
    }
    match LABELS.get_or_init(|| (0_u8..=3).map(load_route_kind_label).collect()) {
        Ok(labels) => labels
            .get(usize::from(kind))
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn route_kind_from_label(label: &str) -> Result<Option<u8>, MojoError> {
    let mut spans = [-1_i64; 4];
    route_call(RouteCall {
        mode: MODE_ROUTE_FROM_LABEL,
        route_kind: 0,
        key_kind: 0,
        key: label,
        prefix: "",
        profile: "",
        output: None,
        written: None,
        spans: Some(&mut spans),
    })?;
    match spans[0] {
        -1 => Ok(None),
        0..=3 => Ok(Some(
            u8::try_from(spans[0]).map_err(|_| MojoError::InvalidOutput)?,
        )),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn route_coupled_kind(kind: u8) -> Result<u8, MojoError> {
    let mut spans = [0_i64; 4];
    route_call(RouteCall {
        mode: MODE_ROUTE_COUPLED,
        route_kind: kind,
        key_kind: 0,
        key: "",
        prefix: "",
        profile: "",
        output: None,
        written: None,
        spans: Some(&mut spans),
    })?;
    match spans[0] {
        0..=3 => u8::try_from(spans[0]).map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn route_key(key_kind: u8, route_kind: u8, profile: &str) -> Result<String, MojoError> {
    let capacity = profile
        .len()
        .checked_add(64)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    route_call(RouteCall {
        mode: MODE_ROUTE_BUILD_KEY,
        route_kind,
        key_kind,
        key: "",
        prefix: "",
        profile,
        output: Some(&mut output),
        written: Some(&mut written),
        spans: None,
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

pub fn route_key_parts(
    key: &str,
    prefix: &str,
) -> Result<Option<(usize, usize, usize, usize)>, MojoError> {
    let mut spans = [-1_i64; 4];
    route_call(RouteCall {
        mode: MODE_ROUTE_KEY_PARTS,
        route_kind: 0,
        key_kind: 0,
        key,
        prefix,
        profile: "",
        output: None,
        written: None,
        spans: Some(&mut spans),
    })?;
    if spans.iter().all(|value| *value == -1) {
        return Ok(None);
    }
    if spans.iter().any(|value| *value < 0) {
        return Err(MojoError::InvalidOutput);
    }
    let spans = spans
        .into_iter()
        .map(|value| usize::try_from(value).map_err(|_| MojoError::InvalidOutput))
        .collect::<Result<Vec<_>, _>>()?;
    if spans[0] > spans[1] || spans[1] > spans[3] || spans[2] > spans[3] || spans[3] > key.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((spans[0], spans[1], spans[2], spans[3])))
}

pub fn route_profile_suffix_span(key: &str) -> Result<(usize, usize), MojoError> {
    let mut spans = [-1_i64; 4];
    route_call(RouteCall {
        mode: MODE_ROUTE_PROFILE_SUFFIX,
        route_kind: 0,
        key_kind: 0,
        key,
        prefix: "",
        profile: "",
        output: None,
        written: None,
        spans: Some(&mut spans),
    })?;
    let start = usize::try_from(spans[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(spans[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > key.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok((start, end))
}

pub fn route_circuit_health_key(key: &str) -> Result<String, MojoError> {
    let capacity = key.len().checked_add(1).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    route_call(RouteCall {
        mode: MODE_ROUTE_CIRCUIT_HEALTH_KEY,
        route_kind: 0,
        key_kind: 0,
        key,
        prefix: "",
        profile: "",
        output: Some(&mut output),
        written: Some(&mut written),
        spans: None,
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_state_route_kernel_preserves_key_contract() {
        assert_eq!(route_kind_label(0).unwrap(), "responses");
        assert_eq!(route_kind_label(3).unwrap(), "standard");
        assert_eq!(route_kind_from_label("websocket").unwrap(), Some(2));
        assert_eq!(route_kind_from_label("unknown").unwrap(), None);
        assert_eq!(route_coupled_kind(0).unwrap(), 2);
        assert_eq!(route_coupled_kind(1).unwrap(), 3);

        assert_eq!(
            route_key(ROUTE_KEY_HEALTH, 0, "alpha").unwrap(),
            "__route_health__:responses:alpha"
        );
        assert_eq!(
            route_key(ROUTE_KEY_CIRCUIT_REOPEN, 2, "").unwrap(),
            "__route_circuit_reopen__:websocket:"
        );
        assert_eq!(
            route_key_parts("__route_health__:compact:alpha", "__route_health__:").unwrap(),
            Some((17, 24, 25, 30))
        );
        assert_eq!(
            route_key_parts("prefix:route:profile", "prefix:").unwrap(),
            Some((7, 12, 13, 20))
        );
        assert_eq!(route_key_parts("malformed", "prefix:").unwrap(), None);
        assert_eq!(
            route_profile_suffix_span("__route_circuit__:responses:alpha").unwrap(),
            (28, 33)
        );
        assert_eq!(
            route_circuit_health_key("__route_circuit__:responses:alpha").unwrap(),
            "__route_health__:responses:alpha"
        );
        assert_eq!(
            route_circuit_health_key("prefix__route_circuit__middle__route_circuit__").unwrap(),
            "prefix__route_health__middle__route_circuit__"
        );
        assert_eq!(route_circuit_health_key("other:key").unwrap(), "other:key");
        assert_eq!(route_kind_label(4), Err(MojoError::InvalidInput));
        assert_eq!(route_coupled_kind(4), Err(MojoError::InvalidInput));
        assert_eq!(
            route_key(ROUTE_KEY_TRANSPORT_BACKOFF + 1, 0, "alpha"),
            Err(MojoError::InvalidInput)
        );
        assert_eq!(route_key_parts("", "").unwrap(), None);
        assert_eq!(route_profile_suffix_span("").unwrap(), (0, 0));
    }
}
