//! Measured diagnostics for compiled managed-plugin manifest contracts.
use super::{ABI_VERSION, OptionalToolPolicyId, OptionalToolStableVersion, OptionalToolStringView};
use crate::{MojoError, text_output::render_measured_text};

const MAX_TEXT_BYTES: usize = 1_048_576;

#[repr(i64)]
enum ManifestPhase {
    Header,
    Version,
    Identity,
}

/// Host-acquired manifest fields after the compiled stable-version gate.
#[derive(Clone, Copy)]
pub struct OptionalToolManifestIdentity<'a> {
    pub version: &'a str,
    pub directory_version: Option<&'a str>,
    pub source: &'a str,
    pub expected_source: &'a str,
    pub commit: &'a str,
    pub tree_sha256: &'a str,
}

unsafe extern "C" {
    fn prodex_optional_tool_install_policy_v1(
        abi_version: i64,
        tool: i64,
        phase: i64,
        tag: i64,
        fields_address: u64,
        field_count: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        written_address: u64,
    ) -> i64;
}

fn diagnostic(
    tool: OptionalToolPolicyId,
    phase: ManifestPhase,
    tag: i64,
    fields: [&str; 6],
) -> Result<Option<String>, MojoError> {
    if fields.iter().any(|field| field.len() > MAX_TEXT_BYTES) {
        return Err(MojoError::InvalidInput);
    }
    let views = fields.map(|field| OptionalToolStringView {
        ptr: field.as_ptr() as u64,
        len: field.len() as u64,
    });
    let phase = phase as i64;
    let message = render_measured_text(|measuring, output, capacity, written| unsafe {
        prodex_optional_tool_install_policy_v1(
            ABI_VERSION,
            tool as i64,
            phase,
            tag,
            views.as_ptr() as u64,
            views.len() as i64,
            measuring,
            output,
            capacity,
            written,
        )
    })?;
    Ok((!message.is_empty()).then_some(message))
}

/// Check schema and ID before evaluating versions or opening further files.
/// Only Caveman and Ponytail are managed plugin manifest kinds.
pub fn optional_tool_manifest_header_error(
    tool: OptionalToolPolicyId,
    schema_version: u32,
    id: &str,
) -> Result<Option<String>, MojoError> {
    diagnostic(
        tool,
        ManifestPhase::Header,
        i64::from(schema_version),
        [id, "", "", "", "", ""],
    )
}

/// Render the compiled stable-version classification without redoing its decision.
pub fn optional_tool_manifest_version_error(
    tool: OptionalToolPolicyId,
    version: OptionalToolStableVersion,
    text: &str,
    minimum: &str,
    latest: &str,
) -> Result<Option<String>, MojoError> {
    let tag = match version {
        OptionalToolStableVersion::Malformed => 0,
        OptionalToolStableVersion::Incompatible => 1,
        OptionalToolStableVersion::Supported => 2,
    };
    diagnostic(
        tool,
        ManifestPhase::Version,
        tag,
        [text, minimum, latest, "", "", ""],
    )
}

/// Check directory identity, source and digest syntax before plugin-file reads.
pub fn optional_tool_manifest_identity_error(
    tool: OptionalToolPolicyId,
    input: OptionalToolManifestIdentity<'_>,
) -> Result<Option<String>, MojoError> {
    diagnostic(
        tool,
        ManifestPhase::Identity,
        i64::from(input.directory_version.is_some()),
        [
            input.version,
            input.directory_version.unwrap_or_default(),
            input.source,
            input.expected_source,
            input.commit,
            input.tree_sha256,
        ],
    )
}

#[cfg(test)]
#[path = "install_policy_tests.rs"]
mod tests;
