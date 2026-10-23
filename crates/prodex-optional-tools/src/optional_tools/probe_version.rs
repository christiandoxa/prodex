//! Optional-tool version compatibility with compiled Mojo as the semantic owner.
use super::OptionalToolId;
use anyhow::{Result, bail};
use prodex_mojo_core::optional_tools_policy::{
    OptionalToolProbeVersion, optional_tool_probe_version,
};

pub(super) fn validate_minimum_probe_version(
    id: OptionalToolId,
    version_line: &str,
) -> Result<String> {
    match optional_tool_probe_version(
        version_line,
        crate::optional_tool_minimum_supported_version(id),
    )
    .map_err(|error| anyhow::anyhow!("optional tool version policy failed: {error:?}"))?
    {
        OptionalToolProbeVersion::Missing => {
            bail!("{id} did not report a recognizable semantic version: {version_line}")
        }
        OptionalToolProbeVersion::TooOld(found) => bail!(
            "{id} {found} is too old; Prodex requires {} or newer. Update {id} to the latest stable release (release-qualified reference: {})",
            crate::optional_tool_minimum_supported_version(id),
            crate::optional_tool_recommended_version(id),
        ),
        OptionalToolProbeVersion::Supported(found) => Ok(found.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    fn parsed_probe_semver(value: &str) -> Result<Option<Version>> {
        Ok(
            match optional_tool_probe_version(value, "0.0.0")
                .map_err(|error| anyhow::anyhow!("version policy failed: {error:?}"))?
            {
                OptionalToolProbeVersion::Missing => None,
                OptionalToolProbeVersion::TooOld(value)
                | OptionalToolProbeVersion::Supported(value) => {
                    Some(Version::parse(value).unwrap())
                }
            },
        )
    }

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
            "999.0.0",
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

    #[test]
    fn compiled_probe_scanner_matches_semver_total_order_and_invalid_candidate_skips() {
        for candidate in [
            "0.0.0-alpha",
            "1.2.3",
            "1.2.4-rc.1",
            "1.2.3-alpha.1",
            "1.2.3+build.01",
            "1.2.3+build.2",
            "1.2.3+build.002",
            "1.2.3+000",
            "1.2.3+0",
            "1.2.3+00",
            "18446744073709551615.0.0",
            "01.2.3",
            "1.2.3+",
            "1.2.3-alpha.01",
            "1.2.3.4",
            "18446744073709551616.0.0",
        ] {
            let selected = if Version::parse(candidate).is_ok() {
                candidate
            } else {
                "9999.0.0"
            };
            let reference = Version::parse(selected).unwrap();
            let text = format!("version [{candidate}] 9999.0.0");
            for minimum in [
                "0.0.0",
                "1.2.3",
                "1.2.3+build.01",
                "1.2.3+build.2",
                "1.2.3+00",
            ] {
                let expected = if reference >= Version::parse(minimum).unwrap() {
                    OptionalToolProbeVersion::Supported(selected)
                } else {
                    OptionalToolProbeVersion::TooOld(selected)
                };
                assert_eq!(
                    optional_tool_probe_version(&text, minimum),
                    Ok(expected),
                    "{text} >= {minimum}"
                );
            }
        }
    }
}
