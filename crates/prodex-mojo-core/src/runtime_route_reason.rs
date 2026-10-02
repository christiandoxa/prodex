use crate::MojoError;

const ABI_VERSION: i64 = 1;
const UNKNOWN: i64 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RouteReasonLookup {
    pub kind: Option<u8>,
    pub stage: Option<u8>,
}

unsafe extern "C" {
    fn prodex_runtime_route_reason_lookup_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        stage_address: u64,
    ) -> i64;
    fn prodex_runtime_route_reason_stage_v1(abi_version: i64, kind: i64) -> i64;
    fn prodex_runtime_route_reason_label_v1(
        abi_version: i64,
        kind: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_runtime_route_reason_unknown_span_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_runtime_route_identifier_span_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

pub fn lookup(label: &str) -> Result<RouteReasonLookup, MojoError> {
    let mut stage = UNKNOWN;
    let kind = unsafe {
        prodex_runtime_route_reason_lookup_v1(
            ABI_VERSION,
            label.as_ptr() as usize as u64,
            signed_len(label)?,
            (&mut stage as *mut i64) as usize as u64,
        )
    };
    if kind == -2 {
        return Err(MojoError::InvalidInput);
    }
    if kind == UNKNOWN {
        return Ok(RouteReasonLookup {
            kind: None,
            stage: None,
        });
    }
    let kind = u8::try_from(kind).map_err(|_| MojoError::InvalidOutput)?;
    let stage = u8::try_from(stage).map_err(|_| MojoError::InvalidOutput)?;
    if kind > 33 || stage > 10 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(RouteReasonLookup {
        kind: Some(kind),
        stage: Some(stage),
    })
}

pub fn stage(kind: u8) -> Result<u8, MojoError> {
    let stage = unsafe { prodex_runtime_route_reason_stage_v1(ABI_VERSION, i64::from(kind)) };
    let stage = u8::try_from(stage).map_err(|_| MojoError::InvalidOutput)?;
    (stage <= 10)
        .then_some(stage)
        .ok_or(MojoError::InvalidOutput)
}

fn load_label(kind: i64) -> Result<String, MojoError> {
    let mut output = [0_u8; 64];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_runtime_route_reason_label_v1(
            ABI_VERSION,
            kind,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        -2 => return Err(MojoError::InvalidInput),
        -3 => return Err(MojoError::Capacity),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn label(kind: u8) -> Result<&'static str, MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, MojoError>> = OnceLock::new();
    let index = usize::from(kind);
    if index > 33 {
        return Err(MojoError::InvalidInput);
    }
    match LABELS.get_or_init(|| (0_i64..34).map(load_label).collect()) {
        Ok(labels) => labels
            .get(index)
            .map(String::as_str)
            .ok_or(MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn safe_identifier(value: &str) -> Result<(&str, bool), MojoError> {
    let mut span = [-1_i64; 3];
    let status = unsafe {
        prodex_runtime_route_identifier_span_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            span.as_mut_ptr() as usize as u64,
        )
    };
    if status != 0 {
        return Err(MojoError::InvalidInput);
    }
    let start = usize::try_from(span[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(span[1]).map_err(|_| MojoError::InvalidOutput)?;
    let truncated = match span[2] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    value
        .get(start..end)
        .map(|value| (value, truncated))
        .ok_or(MojoError::InvalidOutput)
}

pub fn normalize_unknown(label: &str) -> Result<Option<&str>, MojoError> {
    let mut span = [-1_i64; 2];
    let status = unsafe {
        prodex_runtime_route_reason_unknown_span_v1(
            ABI_VERSION,
            label.as_ptr() as usize as u64,
            signed_len(label)?,
            span.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(None),
        1 => {
            let start = usize::try_from(span[0]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(span[1]).map_err(|_| MojoError::InvalidOutput)?;
            label
                .get(start..end)
                .map(Some)
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidInput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_reason_kernel_smoke() {
        assert_eq!(
            lookup("quota_exhausted").unwrap(),
            RouteReasonLookup {
                kind: Some(11),
                stage: Some(6),
            }
        );
        assert_eq!(lookup(" quota_exhausted ").unwrap().kind, None);
        assert_eq!(
            normalize_unknown(" safe_reason_12 ").unwrap(),
            Some("safe_reason_12")
        );
        assert_eq!(normalize_unknown("not safe / secret").unwrap(), None);
        assert_eq!(stage(25).unwrap(), 1);
        assert_eq!(label(0).unwrap(), "auth_failure_backoff");
        assert_eq!(label(33).unwrap(), "output_limit_clamped");
        assert_eq!(label(34), Err(MojoError::InvalidInput));
        for kind in 0_u8..34 {
            let value = label(kind).unwrap();
            assert_eq!(lookup(value).unwrap().kind, Some(kind));
        }
        assert_eq!(safe_identifier("  model-x  ").unwrap(), ("model-x", false));
        let long = format!("{}é", "a".repeat(95));
        let (safe, truncated) = safe_identifier(&long).unwrap();
        assert_eq!(safe, "a".repeat(95));
        assert!(truncated);
    }
}
