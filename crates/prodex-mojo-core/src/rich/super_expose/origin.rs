//! Versioned compiled MCP Origin policy over URL facts parsed by the host.

use crate::MojoError;

unsafe extern "C" {
    fn prodex_mojo_super_expose_origin_allowed_v1(
        abi_version: i64,
        host_address: u64,
        host_length: i64,
        origin_present: i64,
        origin_address: u64,
        origin_length: i64,
        scheme_address: u64,
        scheme_length: i64,
        parsed_host_address: u64,
        parsed_host_length: i64,
        parsed_flags: i64,
        output_address: u64,
    ) -> i64;
}

/// Rust supplies only RFC-compliant parsed URL fields; Mojo owns trust policy.
#[derive(Debug, Clone, Copy)]
pub struct SuperExposeParsedOriginUrl<'a> {
    pub scheme: &'a str,
    pub host: Option<&'a str>,
    pub username_empty: bool,
    pub password_absent: bool,
    pub explicit_port_absent: bool,
    pub root_path: bool,
    pub query_absent: bool,
    pub fragment_absent: bool,
}

pub fn super_expose_origin_allowed(
    host: &str,
    origin: Option<&str>,
    parsed: Option<SuperExposeParsedOriginUrl<'_>>,
) -> Result<bool, MojoError> {
    let raw = origin.unwrap_or("");
    let scheme = parsed.map_or("", |url| url.scheme);
    let parsed_host = parsed.and_then(|url| url.host).unwrap_or("");
    let mut flags = 0_i64;
    if let Some(url) = parsed {
        flags |= 1;
        flags |= i64::from(url.username_empty) << 1;
        flags |= i64::from(url.password_absent) << 2;
        flags |= i64::from(url.explicit_port_absent) << 3;
        flags |= i64::from(url.root_path) << 4;
        flags |= i64::from(url.query_absent) << 5;
        flags |= i64::from(url.fragment_absent) << 6;
    }
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_super_expose_origin_allowed_v1(
            1,
            host.as_ptr() as usize as u64,
            i64::try_from(host.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::from(origin.is_some()),
            raw.as_ptr() as usize as u64,
            i64::try_from(raw.len()).map_err(|_| MojoError::InvalidInput)?,
            scheme.as_ptr() as usize as u64,
            i64::try_from(scheme.len()).map_err(|_| MojoError::InvalidInput)?,
            parsed_host.as_ptr() as usize as u64,
            i64::try_from(parsed_host.len()).map_err(|_| MojoError::InvalidInput)?,
            flags,
            (&mut output as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => match output {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed<'a>(host: &'a str) -> SuperExposeParsedOriginUrl<'a> {
        SuperExposeParsedOriginUrl {
            scheme: "https",
            host: Some(host),
            username_empty: true,
            password_absent: true,
            explicit_port_absent: true,
            root_path: true,
            query_absent: true,
            fragment_absent: true,
        }
    }

    #[test]
    fn compiled_super_expose_origin_policy_checks_trusted_hosts_and_url_constraints() {
        assert_eq!(
            super_expose_origin_allowed("localhost", None, None),
            Ok(true)
        );
        for (host, origin, facts, expected) in [
            (
                "example.com",
                "https://EXAMPLE.COM/",
                parsed("example.com"),
                true,
            ),
            (
                "localhost",
                "https://chatgpt.com/",
                parsed("chatgpt.com"),
                true,
            ),
            (
                "localhost",
                "https://chat.openai.com/",
                parsed("chat.openai.com"),
                true,
            ),
            (
                "localhost",
                "https://chatgpt.com.attacker/",
                parsed("chatgpt.com.attacker"),
                false,
            ),
            (
                "localhost",
                "http://chatgpt.com/",
                SuperExposeParsedOriginUrl {
                    scheme: "http",
                    ..parsed("chatgpt.com")
                },
                false,
            ),
            (
                "127.0.0.1:1234",
                "http://127.0.0.1:1234",
                SuperExposeParsedOriginUrl {
                    scheme: "http",
                    host: Some("127.0.0.1"),
                    explicit_port_absent: false,
                    ..parsed("localhost")
                },
                true,
            ),
            (
                "127.0.0.1:1234",
                "http://127.0.0.1:1234/",
                SuperExposeParsedOriginUrl {
                    scheme: "http",
                    host: Some("127.0.0.1"),
                    explicit_port_absent: false,
                    ..parsed("localhost")
                },
                false,
            ),
            (
                "127.0.0.1:1234",
                "http://127.0.0.1:1235",
                SuperExposeParsedOriginUrl {
                    scheme: "http",
                    host: Some("127.0.0.1"),
                    explicit_port_absent: false,
                    ..parsed("localhost")
                },
                false,
            ),
            (
                "example.com",
                " https://example.com/",
                parsed("example.com"),
                false,
            ),
            (
                "example.com",
                "https://example.com/\u{3000}",
                parsed("example.com"),
                false,
            ),
            (
                "example.com",
                "https://example.com/",
                SuperExposeParsedOriginUrl {
                    explicit_port_absent: false,
                    ..parsed("example.com")
                },
                false,
            ),
            (
                "example.com",
                "https://example.com/",
                SuperExposeParsedOriginUrl {
                    username_empty: false,
                    ..parsed("example.com")
                },
                false,
            ),
            (
                "example.com",
                "https://example.com/",
                SuperExposeParsedOriginUrl {
                    password_absent: false,
                    ..parsed("example.com")
                },
                false,
            ),
            (
                "example.com",
                "https://example.com/path",
                SuperExposeParsedOriginUrl {
                    root_path: false,
                    ..parsed("example.com")
                },
                false,
            ),
            (
                "example.com",
                "https://example.com/?a=1",
                SuperExposeParsedOriginUrl {
                    query_absent: false,
                    ..parsed("example.com")
                },
                false,
            ),
            (
                "example.com",
                "https://example.com/#f",
                SuperExposeParsedOriginUrl {
                    fragment_absent: false,
                    ..parsed("example.com")
                },
                false,
            ),
        ] {
            assert_eq!(
                super_expose_origin_allowed(host, Some(origin), Some(facts)),
                Ok(expected),
                "{host:?}/{origin:?}"
            );
        }
        assert_eq!(
            super_expose_origin_allowed("localhost", Some("invalid"), None),
            Ok(false)
        );
    }

    #[test]
    fn compiled_super_expose_origin_abi_rejects_invalid_inputs_before_mutation() {
        let host = b"localhost";
        let origin = b"https://localhost/";
        let scheme = b"https";
        let mut output = 87_i64;
        let host_ptr = host.as_ptr() as usize as u64;
        let origin_ptr = origin.as_ptr() as usize as u64;
        let scheme_ptr = scheme.as_ptr() as usize as u64;
        let dest = (&mut output as *mut i64) as usize as u64;
        let call = |version,
                    host_address,
                    host_length,
                    present,
                    origin_address,
                    origin_length,
                    flags,
                    dest| unsafe {
            prodex_mojo_super_expose_origin_allowed_v1(
                version,
                host_address,
                host_length,
                present,
                origin_address,
                origin_length,
                scheme_ptr,
                5,
                host_ptr,
                9,
                flags,
                dest,
            )
        };
        assert_eq!(call(2, host_ptr, 9, 1, origin_ptr, 18, 127, dest), 4);
        assert_eq!(call(1, 0, 9, 1, origin_ptr, 18, 127, dest), 1);
        assert_eq!(call(1, host_ptr, -1, 1, origin_ptr, 18, 127, dest), 1);
        assert_eq!(call(1, host_ptr, 9, 2, origin_ptr, 18, 127, dest), 1);
        assert_eq!(call(1, host_ptr, 9, 1, 0, 18, 127, dest), 1);
        assert_eq!(call(1, host_ptr, 9, 1, origin_ptr, -1, 127, dest), 1);
        assert_eq!(call(1, host_ptr, 9, 1, origin_ptr, 18, 128, dest), 1);
        assert_eq!(call(1, host_ptr, 9, 1, origin_ptr, 18, 127, 0), 1);
        assert_eq!(output, 87);
    }
}
