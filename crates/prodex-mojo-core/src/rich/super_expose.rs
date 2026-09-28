use super::{ensure_rich_abi, mojo_mut_pointer_address};
use crate::MojoError;

const SUPER_EXPOSE_ABI_VERSION: i64 = 1;
const SUPER_EXPOSE_MAX_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeMethod {
    Unknown,
    ServerDiscover,
    Initialize,
    Ping,
    ToolsList,
    ToolsCall,
    Notification,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeTool {
    Unknown,
    Start,
    Status,
    Result,
    Cancel,
    List,
    Exec,
    Events,
    SessionPromptWrite,
    SessionPreempt,
    SessionOutputRead,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SuperExposeRoute {
    pub method: SuperExposeMethod,
    pub tool: SuperExposeTool,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_route_v1(
        abi_version: i64,
        method_address: u64,
        method_length: i64,
        tool_address: u64,
        tool_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tool_allowed_v1(
        abi_version: i64,
        mode: i64,
        tool_address: u64,
        tool_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tunnel_id_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tunnel_client_version_valid_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        output_address: u64,
    ) -> i64;
}

pub fn super_expose_route(method: &str, tool: Option<&str>) -> Result<SuperExposeRoute, MojoError> {
    ensure_rich_abi()?;
    let tool = tool.unwrap_or_default();
    if method.len() > SUPER_EXPOSE_MAX_NAME_BYTES || tool.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [0_i64; 2];
    let status = unsafe {
        prodex_mojo_super_expose_route_v1(
            SUPER_EXPOSE_ABI_VERSION,
            method.as_ptr() as u64,
            i64::try_from(method.len()).map_err(|_| MojoError::InvalidInput)?,
            tool.as_ptr() as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    Ok(SuperExposeRoute {
        method: match output[0] {
            6 => SuperExposeMethod::ServerDiscover,
            1 => SuperExposeMethod::Initialize,
            2 => SuperExposeMethod::Ping,
            3 => SuperExposeMethod::ToolsList,
            4 => SuperExposeMethod::ToolsCall,
            5 => SuperExposeMethod::Notification,
            0 => SuperExposeMethod::Unknown,
            _ => return Err(MojoError::InvalidOutput),
        },
        tool: match output[1] {
            1 => SuperExposeTool::Start,
            2 => SuperExposeTool::Status,
            3 => SuperExposeTool::Result,
            4 => SuperExposeTool::Cancel,
            5 => SuperExposeTool::List,
            6 => SuperExposeTool::Exec,
            7 => SuperExposeTool::Events,
            8 => SuperExposeTool::SessionPromptWrite,
            9 => SuperExposeTool::SessionPreempt,
            10 => SuperExposeTool::SessionOutputRead,
            0 => SuperExposeTool::Unknown,
            _ => return Err(MojoError::InvalidOutput),
        },
    })
}

pub fn super_expose_tool_allowed(exec_only: bool, tool: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if tool.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tool_allowed_v1(
            SUPER_EXPOSE_ABI_VERSION,
            i64::from(exec_only),
            tool.as_ptr() as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_tunnel_id_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > SUPER_EXPOSE_MAX_NAME_BYTES {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_id_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_tunnel_client_version_output_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    if value.len() > 16_384 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = 0_i64;
    let status = unsafe {
        prodex_mojo_super_expose_tunnel_client_version_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tunnel_client_version_policy_accepts_official_format_across_versions() {
        for value in [
            "0.0.13+4b5267f823be0b046bb883aacb51603cfde3a0ea (git sha: 4b5267f823be0b046bb883aacb51603cfde3a0ea)",
            "0.0.15+a390c168ff1b2d14e73a95991c186c6aba3ff5a0 (git sha: a390c168ff1b2d14e73a95991c186c6aba3ff5a0)",
            "0.0.16+1111111111111111111111111111111111111111 (git sha: 1111111111111111111111111111111111111111)",
        ] {
            assert!(
                super_expose_tunnel_client_version_output_valid(value).unwrap(),
                "{value}"
            );
        }
    }

    #[test]
    fn tunnel_client_version_policy_rejects_malformed_official_format() {
        for value in [
            "0.0.15+1111111111111111111111111111111111111111 (git sha: 2222222222222222222222222222222222222222)",
            "0.0.15+not-a-git-sha (git sha: not-a-git-sha)",
            "0.0.15",
        ] {
            assert!(
                !super_expose_tunnel_client_version_output_valid(value).unwrap(),
                "{value}"
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperExposeProtocolDecision {
    Ok,
    UnsupportedVersion,
    VersionMismatch,
    MetadataRequired,
    MethodMismatch,
    NameMismatch,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_protocol_version_supported_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_protocol_metadata_v1(
        abi_version: i64,
        method_address: u64,
        method_length: i64,
        header_present: i64,
        header_address: u64,
        header_length: i64,
        body_present: i64,
        body_address: u64,
        body_length: i64,
        method_header_present: i64,
        method_header_address: u64,
        method_header_length: i64,
        name_header_present: i64,
        name_header_address: u64,
        name_header_length: i64,
        body_name_present: i64,
        body_name_address: u64,
        body_name_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_media_header_v1(
        abi_version: i64,
        kind: i64,
        present: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_json_nesting_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        limit: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_tool_argument_allowed_v1(
        abi_version: i64,
        tool_address: u64,
        tool_length: i64,
        key_address: u64,
        key_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_run_id_valid_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_mojo_super_expose_string_valid_v1(
        abi_version: i64,
        address: u64,
        length: i64,
        max_bytes: i64,
        output_address: u64,
    ) -> i64;
}

fn super_expose_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn optional_string_parts(value: Option<&str>) -> Result<(i64, u64, i64), MojoError> {
    match value {
        Some(value) => Ok((
            1,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        )),
        None => Ok((0, 0, 0)),
    }
}

fn bool_output(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_expose_protocol_version_supported(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_protocol_version_supported_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_protocol_metadata(
    method: &str,
    header_version: Option<&str>,
    body_version: Option<&str>,
    method_header: Option<&str>,
    name_header: Option<&str>,
    body_name: Option<&str>,
) -> Result<SuperExposeProtocolDecision, MojoError> {
    ensure_rich_abi()?;
    let header = optional_string_parts(header_version)?;
    let body = optional_string_parts(body_version)?;
    let method_header = optional_string_parts(method_header)?;
    let name_header = optional_string_parts(name_header)?;
    let body_name = optional_string_parts(body_name)?;
    let mut output = -1_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_protocol_metadata_v1(
            SUPER_EXPOSE_ABI_VERSION,
            method.as_ptr() as usize as u64,
            i64::try_from(method.len()).map_err(|_| MojoError::InvalidInput)?,
            header.0,
            header.1,
            header.2,
            body.0,
            body.1,
            body.2,
            method_header.0,
            method_header.1,
            method_header.2,
            name_header.0,
            name_header.1,
            name_header.2,
            body_name.0,
            body_name.1,
            body_name.2,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    match output {
        0 => Ok(SuperExposeProtocolDecision::Ok),
        1 => Ok(SuperExposeProtocolDecision::UnsupportedVersion),
        2 => Ok(SuperExposeProtocolDecision::VersionMismatch),
        3 => Ok(SuperExposeProtocolDecision::MetadataRequired),
        4 => Ok(SuperExposeProtocolDecision::MethodMismatch),
        5 => Ok(SuperExposeProtocolDecision::NameMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn media_header_allowed(kind: i64, value: Option<&str>) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let value = optional_string_parts(value)?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_media_header_v1(
            SUPER_EXPOSE_ABI_VERSION,
            kind,
            value.0,
            value.1,
            value.2,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_content_type_allowed(value: Option<&str>) -> Result<bool, MojoError> {
    media_header_allowed(0, value)
}

pub fn super_expose_accept_allowed(value: Option<&str>) -> Result<bool, MojoError> {
    media_header_allowed(1, value)
}

pub fn super_expose_json_nesting_within_limit(
    body: &[u8],
    limit: usize,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_json_nesting_v1(
            SUPER_EXPOSE_ABI_VERSION,
            body.as_ptr() as usize as u64,
            i64::try_from(body.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(limit).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_tool_argument_allowed(tool: &str, key: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_tool_argument_allowed_v1(
            SUPER_EXPOSE_ABI_VERSION,
            tool.as_ptr() as usize as u64,
            i64::try_from(tool.len()).map_err(|_| MojoError::InvalidInput)?,
            key.as_ptr() as usize as u64,
            i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_run_id_valid(value: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_run_id_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

pub fn super_expose_string_valid(value: &str, max_bytes: usize) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut output = 0_i64;
    super_expose_status(unsafe {
        prodex_mojo_super_expose_string_valid_v1(
            SUPER_EXPOSE_ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_bytes).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output),
        )
    })?;
    bool_output(output)
}

#[cfg(test)]
mod protocol_policy_tests {
    use super::*;

    #[test]
    fn protocol_version_and_metadata_policy_preserve_precedence() {
        assert!(super_expose_protocol_version_supported("2026-07-28").unwrap());
        assert!(super_expose_protocol_version_supported("2024-11-05").unwrap());
        assert!(!super_expose_protocol_version_supported(&"x".repeat(512)).unwrap());

        assert_eq!(
            super_expose_protocol_metadata(
                "ping",
                Some("2025-11-25"),
                Some("2025-11-25"),
                None,
                None,
                None,
            )
            .unwrap(),
            SuperExposeProtocolDecision::Ok,
        );
        assert_eq!(
            super_expose_protocol_metadata(
                "ping",
                Some("2099-01-01"),
                Some("2026-07-28"),
                Some("ping"),
                None,
                None,
            )
            .unwrap(),
            SuperExposeProtocolDecision::UnsupportedVersion,
        );
        assert_eq!(
            super_expose_protocol_metadata(
                "ping",
                Some("2025-11-25"),
                Some("2026-07-28"),
                Some("ping"),
                None,
                None,
            )
            .unwrap(),
            SuperExposeProtocolDecision::VersionMismatch,
        );
        assert_eq!(
            super_expose_protocol_metadata(
                "ping",
                Some("2026-07-28"),
                None,
                Some("ping"),
                None,
                None,
            )
            .unwrap(),
            SuperExposeProtocolDecision::MetadataRequired,
        );
        assert_eq!(
            super_expose_protocol_metadata(
                "ping",
                Some("2026-07-28"),
                Some("2026-07-28"),
                Some("tools/list"),
                None,
                None,
            )
            .unwrap(),
            SuperExposeProtocolDecision::MethodMismatch,
        );
        assert_eq!(
            super_expose_protocol_metadata(
                "tools/call",
                Some("2026-07-28"),
                Some("2026-07-28"),
                Some("tools/call"),
                Some("prodex_super_status"),
                Some("prodex_super_result"),
            )
            .unwrap(),
            SuperExposeProtocolDecision::NameMismatch,
        );
    }

    #[test]
    fn media_headers_and_json_nesting_match_rust_contract() {
        assert!(
            super_expose_content_type_allowed(Some(
                "\u{2003}APPLICATION/JSON\u{3000}; charset=utf-8"
            ))
            .unwrap()
        );
        assert!(!super_expose_content_type_allowed(None).unwrap());
        assert!(
            super_expose_accept_allowed(Some("text/plain;q=0.2, \u{2003}application/json ; q=1"))
                .unwrap()
        );
        assert!(!super_expose_accept_allowed(Some("text/plain")).unwrap());

        let body = br#"{"text":"[{}]","items":[{}]}"#;
        assert!(super_expose_json_nesting_within_limit(body, 3).unwrap());
        assert!(!super_expose_json_nesting_within_limit(body, 2).unwrap());
        assert!(!super_expose_json_nesting_within_limit(b"}", 64).unwrap());
        assert!(!super_expose_json_nesting_within_limit(b"{\"x\":\"abc\\", 64).unwrap());
    }

    #[test]
    fn tool_argument_and_identifier_policy_match_public_contract() {
        assert!(super_expose_tool_argument_allowed("prodex_super_events", "run_id").unwrap());
        assert!(super_expose_tool_argument_allowed("prodex_super_events", "after_seq").unwrap());
        assert!(!super_expose_tool_argument_allowed("prodex_super_events", "extra").unwrap());
        assert!(
            !super_expose_tool_argument_allowed("prodex_super_list", &"x".repeat(1024),).unwrap()
        );
        assert!(super_expose_tool_argument_allowed("unknown-tool", "anything").unwrap());

        assert!(super_expose_run_id_valid("spr_").unwrap());
        assert!(super_expose_run_id_valid("spr_ABC-123").unwrap());
        assert!(!super_expose_run_id_valid("spr_bad:1").unwrap());

        assert!(super_expose_string_valid("model α", 256).unwrap());
        assert!(!super_expose_string_valid("bad\nmodel", 256).unwrap());
        assert!(!super_expose_string_valid("bad\u{0085}model", 256).unwrap());
        assert!(!super_expose_string_valid(&"x".repeat(257), 256).unwrap());
    }
}
