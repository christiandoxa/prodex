use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WebsocketProxyPatternPlan {
    pub host_start: usize,
    pub host_end: usize,
    pub port: Option<u16>,
}

unsafe extern "C" {
    fn prodex_websocket_proxy_default_port_v1(
        abi_version: i64,
        scheme_address: u64,
        scheme_length: i64,
        scheme_present: i64,
    ) -> i64;
    fn prodex_websocket_proxy_url_candidate_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
    fn prodex_websocket_proxy_pattern_plan_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_websocket_proxy_pattern_matches_v1(
        abi_version: i64,
        pattern_address: u64,
        pattern_length: i64,
        host_address: u64,
        host_length: i64,
        port: i64,
    ) -> i64;
    fn prodex_websocket_proxy_value_matches_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        host_address: u64,
        host_length: i64,
        port: i64,
    ) -> i64;
    fn prodex_websocket_proxy_normalize_host_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_websocket_proxy_authority_v1(
        abi_version: i64,
        host_address: u64,
        host_length: i64,
        port: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

fn status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        2 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn bool_result(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn decode_text(output: &[u8], written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn default_port(scheme: Option<&str>) -> Result<u16, MojoError> {
    let value = scheme.unwrap_or_default();
    let result = unsafe {
        prodex_websocket_proxy_default_port_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            i64::from(scheme.is_some()),
        )
    };
    match result {
        80 | 443 => u16::try_from(result).map_err(|_| MojoError::InvalidOutput),
        -1 => Err(MojoError::InvalidInput),
        -4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn proxy_url_candidate(value: &str) -> Result<Option<String>, MojoError> {
    let capacity = value
        .len()
        .checked_add(7)
        .ok_or(MojoError::InvalidInput)?
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let mut present = -1_i64;
    status(unsafe {
        prodex_websocket_proxy_url_candidate_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    })?;
    match present {
        0 => Ok(None),
        1 => decode_text(&output, written).map(Some),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn pattern_host_port(pattern: &str) -> Result<WebsocketProxyPatternPlan, MojoError> {
    let mut output = [-1_i64; 4];
    status(unsafe {
        prodex_websocket_proxy_pattern_plan_v1(
            ABI_VERSION,
            pattern.as_ptr() as usize as u64,
            signed_len(pattern)?,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    let host_start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let host_end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if host_start > host_end || host_end > pattern.len() {
        return Err(MojoError::InvalidOutput);
    }
    let port = match output[2] {
        0 => None,
        1 => Some(u16::try_from(output[3]).map_err(|_| MojoError::InvalidOutput)?),
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(WebsocketProxyPatternPlan {
        host_start,
        host_end,
        port,
    })
}

pub fn pattern_matches(pattern: &str, host: &str, port: u16) -> Result<bool, MojoError> {
    bool_result(unsafe {
        prodex_websocket_proxy_pattern_matches_v1(
            ABI_VERSION,
            pattern.as_ptr() as usize as u64,
            signed_len(pattern)?,
            host.as_ptr() as usize as u64,
            signed_len(host)?,
            i64::from(port),
        )
    })
}

pub fn value_matches(value: &str, host: &str, port: u16) -> Result<bool, MojoError> {
    bool_result(unsafe {
        prodex_websocket_proxy_value_matches_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            host.as_ptr() as usize as u64,
            signed_len(host)?,
            i64::from(port),
        )
    })
}

pub fn normalize_host(host: &str) -> Result<String, MojoError> {
    let mut output = vec![0_u8; host.len().max(1)];
    let mut written = -1_i64;
    status(unsafe {
        prodex_websocket_proxy_normalize_host_v1(
            ABI_VERSION,
            host.as_ptr() as usize as u64,
            signed_len(host)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    decode_text(&output, written)
}

pub fn authority(host: &str, port: u16) -> Result<String, MojoError> {
    let capacity = host
        .len()
        .checked_add(8)
        .ok_or(MojoError::InvalidInput)?
        .max(8);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    status(unsafe {
        prodex_websocket_proxy_authority_v1(
            ABI_VERSION,
            host.as_ptr() as usize as u64,
            signed_len(host)?,
            i64::from(port),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    decode_text(&output, written)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_proxy_policy_preserves_string_and_port_contracts() {
        assert_eq!(default_port(Some("wss")).unwrap(), 443);
        assert_eq!(default_port(Some("https")).unwrap(), 443);
        assert_eq!(default_port(Some("WSS")).unwrap(), 80);
        assert_eq!(default_port(None).unwrap(), 80);

        assert_eq!(
            proxy_url_candidate(" \u{2003}127.0.0.1:1080\u{00a0} ")
                .unwrap()
                .as_deref(),
            Some("http://127.0.0.1:1080")
        );
        assert_eq!(
            proxy_url_candidate(" socks5://proxy.test:1080 ")
                .unwrap()
                .as_deref(),
            Some("socks5://proxy.test:1080")
        );
        assert_eq!(proxy_url_candidate("  ").unwrap(), None);

        assert_eq!(
            pattern_host_port("[::1]:8080").unwrap(),
            WebsocketProxyPatternPlan {
                host_start: 1,
                host_end: 4,
                port: Some(8080),
            }
        );
        assert_eq!(pattern_host_port("host:+443").unwrap().port, Some(443));
        assert_eq!(pattern_host_port("host:65536").unwrap().port, None);

        assert!(pattern_matches(".openai.com", "api.openai.com", 443).unwrap());
        assert!(pattern_matches("[::1]:8080", "::1", 8080).unwrap());
        assert!(!pattern_matches("[::1]:8080", "::1", 443).unwrap());
        assert!(value_matches(".example.com, api.openai.com:443", "api.openai.com", 443).unwrap());

        assert_eq!(normalize_host("[[::1]]").unwrap(), "::1");
        assert_eq!(
            authority("api.openai.com", 443).unwrap(),
            "api.openai.com:443"
        );
        assert_eq!(authority("::1", 8080).unwrap(), "[::1]:8080");
    }
}
