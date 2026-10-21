use crate::MojoError;
#[path = "optional_tools_policy/agents_block_render.rs"]
mod agents_block_render;
pub use agents_block_render::{AgentsBlockOperation, AgentsBlockRender, agents_block_render};

#[path = "optional_tools_policy/agents_reference.rs"]
mod agents_reference;
pub use agents_reference::{AgentsReferenceUpdate, agents_reference_update};

#[path = "optional_tools_policy/marked_blocks.rs"]
mod marked_blocks;
pub use marked_blocks::{MarkedBlockStripError, strip_marked_instruction_blocks};

#[path = "optional_tools_policy/legacy_paragraphs.rs"]
mod legacy_paragraphs;
pub use legacy_paragraphs::optional_tool_retained_legacy_paragraph_indices;
#[path = "optional_tools_policy/probe_version.rs"]
mod probe_version;
pub use probe_version::{OptionalToolProbeToken, optional_tool_probe_token};

const ABI_VERSION: i64 = 1;
const AWARENESS_TEXT_COUNT: usize = 4;

pub const CAP_CODEX: u64 = 1;
pub const CAP_CLAUDE: u64 = 2;
pub const CAP_SHELL_COMPRESSION: u64 = 4;
pub const CAP_STRUCTURAL_NAVIGATION: u64 = 8;
pub const CAP_BROWSER_AUTOMATION: u64 = 16;
pub const CAP_SIMPLICITY_REVIEW: u64 = 32;
pub const CAP_REDACTION: u64 = 64;

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolPolicyId {
    Caveman = 0,
    Rtk = 1,
    CodebaseMemoryMcp = 2,
    PlaywrightMcp = 3,
    Ponytail = 4,
    Presidio = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolPolicyKind {
    Command,
    CodexPlugin,
    McpServer,
    Service,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptionalToolDescriptorPolicy {
    pub kind: OptionalToolPolicyKind,
    pub capability_mask: u64,
    pub super_default: bool,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct OptionalToolStringView {
    ptr: u64,
    len: u64,
}

unsafe extern "C" {
    fn prodex_optional_tool_required_mcp_contract_v1(
        abi_version: i64,
        enabled_tag: i64,
        actual_command_address: u64,
        actual_command_length: i64,
        expected_command_address: u64,
        expected_command_length: i64,
        actual_args_address: u64,
        actual_args_count: i64,
        expected_args_address: u64,
        expected_args_count: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_optional_tool_config_shape_v1(
        abi_version: i64,
        selected_mask: i64,
        shape_address: u64,
        shape_count: i64,
        issue_address: u64,
    ) -> i64;
    fn prodex_optional_tool_class_v1(abi_version: i64, address: u64, length: i64) -> i64;
    fn prodex_optional_tool_descriptor_policy_v1(abi_version: i64, tool_id: i64) -> i64;
    fn prodex_optional_tool_overlay_rollout_name_v1(
        abi_version: i64,
        name_address: u64,
        name_length: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_optional_tool_manifest_tree_supported_v1(
        abi_version: i64,
        value_address: u64,
        value_length: i64,
        vetted_address: u64,
        vetted_length: i64,
        legacy_address: u64,
        legacy_length: i64,
    ) -> i64;
    fn prodex_optional_tool_hex_encode_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        output_address: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_optional_tool_awareness_v1(
        abi_version: i64,
        availability_mask: i64,
        presidio_enabled: i64,
        text_address: u64,
        text_count: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

/// TOML type facts are acquired by Rust, but the selected-section shape
/// policy, precedence, and error identity belong exclusively to Mojo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptionalToolConfigShapeIssue {
    Features,
    Marketplaces,
    Plugins,
    PonytailMarketplace,
    PonytailPlugin,
    McpServers,
    CodebaseMemoryMcp,
    Playwright,
}

/// Fields: features, marketplaces, plugins, marketplaces.ponytail,
/// plugins.ponytail@ponytail, mcp_servers, codebase-memory-mcp, playwright.
/// Shapes: 0 absent, 1 table, 2 present non-table.
/// Mask: bit 0 Ponytail, 1 Codebase Memory MCP, 2 Playwright MCP.
pub fn optional_tool_config_shape_preflight(
    selected_mask: u8,
    shapes: [i64; 8],
) -> Result<Option<OptionalToolConfigShapeIssue>, MojoError> {
    let mut issue = -1_i64;
    let status = unsafe {
        prodex_optional_tool_config_shape_v1(
            ABI_VERSION,
            i64::from(selected_mask),
            shapes.as_ptr() as usize as u64,
            shapes.len() as i64,
            (&mut issue as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    Ok(match issue {
        0 => None,
        1 => Some(OptionalToolConfigShapeIssue::Features),
        2 => Some(OptionalToolConfigShapeIssue::Marketplaces),
        3 => Some(OptionalToolConfigShapeIssue::Plugins),
        4 => Some(OptionalToolConfigShapeIssue::PonytailMarketplace),
        5 => Some(OptionalToolConfigShapeIssue::PonytailPlugin),
        6 => Some(OptionalToolConfigShapeIssue::McpServers),
        7 => Some(OptionalToolConfigShapeIssue::CodebaseMemoryMcp),
        8 => Some(OptionalToolConfigShapeIssue::Playwright),
        _ => return Err(MojoError::InvalidOutput),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredMcpEnabled {
    Absent,
    True,
    False,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredMcpConfigIssue {
    Disabled,
    EnabledNotBoolean,
    CommandOrArgsMismatch,
}

/// Validate the exact inherited command, args vector and enabled flag through
/// compiled Mojo. None indicates an absent or non-string command/args field;
/// the host has no alternative command-matching implementation.
pub fn optional_tool_required_mcp_contract(
    enabled: RequiredMcpEnabled,
    actual_command: Option<&str>,
    actual_args: Option<&[&str]>,
    expected_command: &str,
    expected_args: &[String],
) -> Result<Option<RequiredMcpConfigIssue>, MojoError> {
    let actual_views = actual_args
        .unwrap_or(&[])
        .iter()
        .map(|value| OptionalToolStringView {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let expected_views = expected_args
        .iter()
        .map(|value| OptionalToolStringView {
            ptr: value.as_ptr() as usize as u64,
            len: value.len() as u64,
        })
        .collect::<Vec<_>>();
    let tag = match enabled {
        RequiredMcpEnabled::Absent => 0,
        RequiredMcpEnabled::True => 1,
        RequiredMcpEnabled::False => 2,
        RequiredMcpEnabled::Invalid => 3,
    };
    let mut result = -1_i64;
    let status = unsafe {
        prodex_optional_tool_required_mcp_contract_v1(
            ABI_VERSION,
            tag,
            actual_command.map_or(0, |value| value.as_ptr() as usize as u64),
            actual_command.map_or(Ok(-1), signed_len)?,
            expected_command.as_ptr() as usize as u64,
            signed_len(expected_command)?,
            actual_views.as_ptr() as usize as u64,
            actual_args.map_or(Ok(-1), |items| {
                i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)
            })?,
            expected_views.as_ptr() as usize as u64,
            i64::try_from(expected_views.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match result {
        0 => Ok(None),
        1 => Ok(Some(RequiredMcpConfigIssue::Disabled)),
        2 => Ok(Some(RequiredMcpConfigIssue::EnabledNotBoolean)),
        3 => Ok(Some(RequiredMcpConfigIssue::CommandOrArgsMismatch)),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn signed_len(value: &str) -> Result<i64, MojoError> {
    i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)
}

pub fn optional_tool_class(value: &str) -> Result<Option<OptionalToolPolicyId>, MojoError> {
    let result = unsafe {
        prodex_optional_tool_class_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
        )
    };
    match result {
        -1 => Ok(None),
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(Some(OptionalToolPolicyId::Caveman)),
        1 => Ok(Some(OptionalToolPolicyId::Rtk)),
        2 => Ok(Some(OptionalToolPolicyId::CodebaseMemoryMcp)),
        3 => Ok(Some(OptionalToolPolicyId::PlaywrightMcp)),
        4 => Ok(Some(OptionalToolPolicyId::Ponytail)),
        5 => Ok(Some(OptionalToolPolicyId::Presidio)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn optional_tool_descriptor_policy(
    id: OptionalToolPolicyId,
) -> Result<OptionalToolDescriptorPolicy, MojoError> {
    let result = unsafe { prodex_optional_tool_descriptor_policy_v1(ABI_VERSION, id as i64) };
    if result == -2 {
        return Err(MojoError::InvalidInput);
    }
    if result < 0 {
        return Err(MojoError::InvalidOutput);
    }
    let kind = match result & 0xff {
        0 => OptionalToolPolicyKind::Command,
        1 => OptionalToolPolicyKind::CodexPlugin,
        2 => OptionalToolPolicyKind::McpServer,
        3 => OptionalToolPolicyKind::Service,
        _ => return Err(MojoError::InvalidOutput),
    };
    let capability_mask =
        u64::try_from((result >> 8) & 0xff).map_err(|_| MojoError::InvalidOutput)?;
    let super_default = ((result >> 16) & 1) == 1;
    Ok(OptionalToolDescriptorPolicy {
        kind,
        capability_mask,
        super_default,
    })
}

/// Compiled exact Codex SQLite state-name policy used by overlay sharing
/// and localization. Host filesystem side effects remain outside Mojo.
pub fn optional_tool_overlay_rollout_name(name: &str) -> Result<bool, MojoError> {
    if name.len() > 1_048_576 {
        return Err(MojoError::InvalidInput);
    }
    let mut result = -1_i64;
    let status = unsafe {
        prodex_optional_tool_overlay_rollout_name_v1(
            ABI_VERSION,
            name.as_ptr() as usize as u64,
            i64::try_from(name.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut result as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn optional_tool_manifest_tree_supported(
    value: &str,
    vetted: &str,
    legacy: &str,
) -> Result<bool, MojoError> {
    let result = unsafe {
        prodex_optional_tool_manifest_tree_supported_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            signed_len(value)?,
            vetted.as_ptr() as usize as u64,
            signed_len(vetted)?,
            legacy.as_ptr() as usize as u64,
            signed_len(legacy)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn optional_tool_hex_encode(input: &[u8], output: &mut [u8]) -> Result<usize, MojoError> {
    let result = unsafe {
        prodex_optional_tool_hex_encode_v1(
            ABI_VERSION,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match result {
        -2 => Err(MojoError::InvalidInput),
        -3 => Err(MojoError::Capacity),
        written if written >= 0 => {
            let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
            if written == input.len().checked_mul(2).ok_or(MojoError::InvalidOutput)?
                && written <= output.len()
            {
                Ok(written)
            } else {
                Err(MojoError::InvalidOutput)
            }
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Render the Super optional-tool awareness document through the compiled Mojo owner.
pub fn render_optional_tool_awareness(
    paths: [Option<&str>; AWARENESS_TEXT_COUNT],
    presidio_enabled: bool,
) -> Result<String, MojoError> {
    let availability_mask = paths.iter().enumerate().fold(0_i64, |mask, (index, path)| {
        mask | (i64::from(path.is_some()) << index)
    });
    let views = paths
        .into_iter()
        .map(|value| {
            value.map_or(Ok(OptionalToolStringView { ptr: 0, len: 0 }), |value| {
                Ok(OptionalToolStringView {
                    ptr: value.as_ptr() as usize as u64,
                    len: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
                })
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let path_bytes = views.iter().try_fold(0_usize, |total, view| {
        total
            .checked_add(usize::try_from(view.len).map_err(|_| MojoError::InvalidInput)?)
            .ok_or(MojoError::InvalidInput)
    })?;
    let capacity = path_bytes
        .checked_add(4096)
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_optional_tool_awareness_v1(
            ABI_VERSION,
            availability_mask,
            i64::from(presidio_enabled),
            views.as_ptr() as usize as u64,
            AWARENESS_TEXT_COUNT as i64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_required_mcp_contract_preserves_enabled_precedence_and_exact_command_args() {
        use RequiredMcpConfigIssue::*;
        use RequiredMcpEnabled::*;
        let expected_args = vec!["--headless".to_owned(), "--isolated".to_owned()];
        let valid_args = ["--headless", "--isolated"];
        for (enabled, command, args, expected) in [
            (Absent, Some("/bin/npx"), Some(valid_args.as_slice()), None),
            (True, Some("/bin/npx"), Some(valid_args.as_slice()), None),
            (
                False,
                Some("/bin/npx"),
                Some(valid_args.as_slice()),
                Some(Disabled),
            ),
            (
                Invalid,
                Some("/bin/npx"),
                Some(valid_args.as_slice()),
                Some(EnabledNotBoolean),
            ),
            (
                True,
                None,
                Some(valid_args.as_slice()),
                Some(CommandOrArgsMismatch),
            ),
            (
                True,
                Some("/bin/npx "),
                Some(valid_args.as_slice()),
                Some(CommandOrArgsMismatch),
            ),
            (True, Some("/bin/npx"), None, Some(CommandOrArgsMismatch)),
            (
                True,
                Some("/bin/npx"),
                Some(["--headless"].as_slice()),
                Some(CommandOrArgsMismatch),
            ),
            (
                True,
                Some("/bin/npx"),
                Some(["--isolated", "--headless"].as_slice()),
                Some(CommandOrArgsMismatch),
            ),
        ] {
            assert_eq!(
                optional_tool_required_mcp_contract(
                    enabled,
                    command,
                    args,
                    "/bin/npx",
                    &expected_args,
                ),
                Ok(expected),
                "{enabled:?}/{command:?}/{args:?}",
            );
        }
        assert_eq!(
            optional_tool_required_mcp_contract(
                True,
                Some("/bin/東京🔥"),
                Some(&["🧭"]),
                "/bin/東京🔥",
                &["🧭".to_string()],
            ),
            Ok(None),
        );
    }

    #[test]
    fn compiled_required_mcp_contract_abi_rejects_invalid_inputs_before_writes() {
        let command = b"npx";
        let ptr = command.as_ptr() as usize as u64;
        let mut output = 77_i64;
        let out = (&mut output as *mut i64) as usize as u64;
        let call = |version, enabled, cmd, cmd_len, exp, exp_len, arg_count, dest| unsafe {
            prodex_optional_tool_required_mcp_contract_v1(
                version, enabled, cmd, cmd_len, exp, exp_len, 0, arg_count, 0, 0, dest,
            )
        };
        assert_eq!(call(2, 1, ptr, 3, ptr, 3, 0, out), 4);
        assert_eq!(call(1, 4, ptr, 3, ptr, 3, 0, out), 1);
        assert_eq!(call(1, 1, 0, 3, ptr, 3, 0, out), 1);
        assert_eq!(call(1, 1, ptr, -2, ptr, 3, 0, out), 1);
        assert_eq!(call(1, 1, ptr, 3, 0, 3, 0, out), 1);
        assert_eq!(call(1, 1, ptr, 3, ptr, 3, 1, out), 1);
        assert_eq!(call(1, 1, ptr, 3, ptr, 3, 0, 0), 1);
        let invalid = [0xff_u8];
        assert_eq!(
            call(1, 1, invalid.as_ptr() as usize as u64, 1, ptr, 3, 0, out),
            1
        );
        assert_eq!(output, 77);
    }

    #[test]
    fn compiled_optional_tool_config_shape_scopes_selection_and_error_precedence() {
        use OptionalToolConfigShapeIssue::*;
        let empty = [0_i64; 8];
        assert_eq!(optional_tool_config_shape_preflight(0, empty), Ok(None));
        assert_eq!(optional_tool_config_shape_preflight(7, empty), Ok(None));
        for (index, issue) in [
            (0, Features),
            (1, Marketplaces),
            (2, Plugins),
            (3, PonytailMarketplace),
            (4, PonytailPlugin),
        ] {
            let mut shapes = empty;
            shapes[index] = 2;
            assert_eq!(
                optional_tool_config_shape_preflight(1, shapes),
                Ok(Some(issue)),
                "Ponytail field index={index}",
            );
            assert_eq!(optional_tool_config_shape_preflight(6, shapes), Ok(None));
        }
        let mut all_invalid = [2_i64; 8];
        assert_eq!(
            optional_tool_config_shape_preflight(7, all_invalid),
            Ok(Some(Features))
        );
        all_invalid[0] = 1;
        assert_eq!(
            optional_tool_config_shape_preflight(7, all_invalid),
            Ok(Some(Marketplaces))
        );
        let mut mcp = empty;
        mcp[5] = 2;
        assert_eq!(optional_tool_config_shape_preflight(0, mcp), Ok(None));
        assert_eq!(optional_tool_config_shape_preflight(1, mcp), Ok(None));
        assert_eq!(
            optional_tool_config_shape_preflight(6, mcp),
            Ok(Some(McpServers))
        );
        mcp[5] = 1;
        mcp[6] = 2;
        mcp[7] = 2;
        assert_eq!(
            optional_tool_config_shape_preflight(2, mcp),
            Ok(Some(CodebaseMemoryMcp))
        );
        assert_eq!(
            optional_tool_config_shape_preflight(4, mcp),
            Ok(Some(Playwright))
        );
        assert_eq!(
            optional_tool_config_shape_preflight(6, mcp),
            Ok(Some(CodebaseMemoryMcp))
        );
        mcp[5] = 0;
        assert_eq!(optional_tool_config_shape_preflight(6, mcp), Ok(None));
    }

    #[test]
    fn compiled_optional_tool_config_shape_abi_rejects_bad_flags_and_shapes_before_writes() {
        let shapes = [0_i64; 8];
        let mut invalid = shapes;
        invalid[7] = 3;
        let src = shapes.as_ptr() as usize as u64;
        let mut issue = 91_i64;
        let out = (&mut issue as *mut i64) as usize as u64;
        let call = |version, mask, ptr, count, dst| unsafe {
            prodex_optional_tool_config_shape_v1(version, mask, ptr, count, dst)
        };
        assert_eq!(call(2, 7, src, 8, out), 4);
        assert_eq!(call(1, -1, src, 8, out), 1);
        assert_eq!(call(1, 8, src, 8, out), 1);
        assert_eq!(call(1, 7, 0, 8, out), 1);
        assert_eq!(call(1, 7, src, 7, out), 1);
        assert_eq!(call(1, 7, src, 8, 0), 1);
        assert_eq!(call(1, 0, invalid.as_ptr() as usize as u64, 8, out), 1);
        assert_eq!(issue, 91);
    }

    #[test]
    fn optional_tool_hex_encode_enforces_capacity_and_does_not_overwrite_canaries() {
        let mut guard = [0xa5_u8; 10];
        let input = [0_u8, 1, 0xfe, 0xff];
        assert_eq!(
            optional_tool_hex_encode(&input, &mut guard[1..8]),
            Err(MojoError::Capacity)
        );
        assert_eq!(guard[0], 0xa5);
        assert_eq!(guard[8], 0xa5);
        assert_eq!(guard[9], 0xa5);
        assert_eq!(optional_tool_hex_encode(&input, &mut guard[1..9]), Ok(8));
        assert_eq!(&guard[1..9], b"0001feff");
        assert_eq!(guard[0], 0xa5);
        assert_eq!(guard[9], 0xa5);
        let mut empty: [u8; 0] = [];
        assert_eq!(optional_tool_hex_encode(&[], &mut empty), Ok(0));
    }

    #[test]
    fn compiled_overlay_rollout_state_name_preserves_exact_sqlite_suffixes() {
        for (name, expected) in [
            ("state_1.sqlite", true),
            ("state_1.sqlite-shm", true),
            ("state_1.sqlite-wal", true),
            ("state_.sqlite", true),
            ("state_雪.sqlite-wal", true),
            ("state_1.Sqlite", false),
            ("STATE_1.sqlite", false),
            ("state_1.sqlite-wal.backup", false),
            ("state_1.sqlite-journal", false),
            ("state_1.db", false),
            ("different.sqlite", false),
            ("", false),
        ] {
            assert_eq!(
                optional_tool_overlay_rollout_name(name),
                Ok(expected),
                "{name:?}"
            );
        }
        assert_eq!(
            optional_tool_overlay_rollout_name(&"x".repeat(1_048_577)),
            Err(MojoError::InvalidInput)
        );
    }

    #[test]
    fn optional_tools_policy_preserves_aliases_descriptors_and_defaults() {
        assert_eq!(
            optional_tool_class(" CBM ").unwrap(),
            Some(OptionalToolPolicyId::CodebaseMemoryMcp)
        );
        assert_eq!(
            optional_tool_class("PLAYWRIGHT").unwrap(),
            Some(OptionalToolPolicyId::PlaywrightMcp)
        );
        assert_eq!(optional_tool_class("unknown").unwrap(), None);

        let caveman = optional_tool_descriptor_policy(OptionalToolPolicyId::Caveman).unwrap();
        assert_eq!(caveman.kind, OptionalToolPolicyKind::CodexPlugin);
        assert_eq!(caveman.capability_mask, CAP_CODEX | CAP_CLAUDE);
        assert!(caveman.super_default);

        let presidio = optional_tool_descriptor_policy(OptionalToolPolicyId::Presidio).unwrap();
        assert_eq!(presidio.kind, OptionalToolPolicyKind::Service);
        assert_eq!(presidio.capability_mask, CAP_REDACTION);
        assert!(!presidio.super_default);

        assert!(optional_tool_manifest_tree_supported("current", "current", "legacy").unwrap());
        assert!(optional_tool_manifest_tree_supported("legacy", "current", "legacy").unwrap());
        assert!(!optional_tool_manifest_tree_supported("CURRENT", "current", "legacy").unwrap());
    }

    #[test]
    fn optional_tool_awareness_render_preserves_paths_and_presence_labels() {
        let rendered = render_optional_tool_awareness(
            [
                Some("/bin/rtk"),
                None,
                Some("/bin/npx"),
                Some("/tmp/ponytail"),
            ],
            true,
        )
        .unwrap();
        assert!(rendered.contains("- rtk: yes (/bin/rtk)"));
        assert!(rendered.contains("- codebase-memory-mcp: no"));
        assert!(rendered.contains("- playwright-mcp: yes (/bin/npx)"));
        assert!(rendered.contains("- ponytail plugin: yes (/tmp/ponytail)"));
        assert!(rendered.contains("- presidio: enabled"));
        assert!(!rendered.contains("prodex-memory"));
    }
}
