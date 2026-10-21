//! Optional-tool semantic version candidates with Mojo-owned token scanning.
use super::OptionalToolId;
use anyhow::{Context, Result};
use prodex_mojo_core::optional_tools_policy::optional_tool_probe_token;
use semver::Version;

fn parsed_probe_semver(value: &str) -> Result<Option<Version>> {
    let mut cursor = 0usize;
    while let Some(token) = optional_tool_probe_token(value, cursor).map_err(|error| {
        anyhow::anyhow!("Mojo optional tool version token planner failed: {error:?}")
    })? {
        if let Ok(version) = Version::parse(&value[token.start..token.end]) {
            return Ok(Some(version));
        }
        cursor = token.next_cursor;
    }
    Ok(None)
}

pub(super) fn validate_minimum_probe_version(
    id: OptionalToolId,
    version_line: &str,
) -> Result<Version> {
    let found = parsed_probe_semver(version_line)?.with_context(|| {
        format!("{id} did not report a recognizable semantic version: {version_line}")
    })?;
    let minimum = Version::parse(crate::optional_tool_minimum_supported_version(id))
        .with_context(|| format!("invalid Prodex minimum version policy for {id}"))?;
    anyhow::ensure!(
        found >= minimum,
        "{id} {found} is too old; Prodex requires {} or newer. Update {id} to the latest stable release (release-qualified reference: {})",
        crate::optional_tool_minimum_supported_version(id),
        crate::optional_tool_recommended_version(id),
    );
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiled_probe_scanner_matches_semver_candidate_priority() {
        assert_eq!(
            parsed_probe_semver("rtk v0.50.0").unwrap(),
            Some(Version::new(0, 50, 0))
        );
        assert_eq!(
            parsed_probe_semver("rtk (vv0.50.0) version").unwrap(),
            Some(Version::new(0, 50, 0))
        );
        assert_eq!(
            parsed_probe_semver("  \u{2003} tool\u{00a0}[v1.2.3-rc.1] ")
                .unwrap()
                .unwrap()
                .to_string(),
            "1.2.3-rc.1"
        );
        assert_eq!(
            parsed_probe_semver("0.0.1 999.0.0").unwrap(),
            Some(Version::new(0, 0, 1))
        );
        assert_eq!(parsed_probe_semver("V1.2.3 and invalid").unwrap(), None);
        assert_eq!(parsed_probe_semver("not a version").unwrap(), None);
        assert_eq!(parsed_probe_semver("").unwrap(), None);
        assert_eq!(parsed_probe_semver("\u{2002} ").unwrap(), None);
    }

    #[test]
    fn compiled_probe_scanner_preserves_version_gates() {
        assert_eq!(
            validate_minimum_probe_version(OptionalToolId::Rtk, "rtk 999.0.0").unwrap(),
            Version::new(999, 0, 0),
        );
        assert!(
            validate_minimum_probe_version(OptionalToolId::Rtk, "rtk 0.0.1")
                .unwrap_err()
                .to_string()
                .contains("too old")
        );
        assert!(
            validate_minimum_probe_version(OptionalToolId::Rtk, "rtk unavailable")
                .unwrap_err()
                .to_string()
                .contains("recognizable semantic version")
        );
    }

    #[test]
    fn compiled_probe_scanner_rejects_unbounded_output() {
        assert!(parsed_probe_semver(&"x".repeat(1_048_577)).is_err());
    }
}
