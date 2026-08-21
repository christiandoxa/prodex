use anyhow::{Result, bail};
use std::path::Path;

pub(super) fn validate_governance_numeric_range(
    value: Option<u64>,
    minimum: u64,
    maximum: u64,
    path: &Path,
    message: &str,
) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        let Some(value) = value else {
            return Ok(());
        };
        let rule = prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value,
            minimum,
            maximum,
            related_value: 0,
        };
        let failed = prodex_mojo_core::policy::validate_numeric_rules(&[rule]).map_err(|_| {
            anyhow::anyhow!("governance policy numeric validation returned invalid output")
        })?;
        if !failed.is_empty() {
            bail!("{message} in {}", path.display());
        }
        Ok(())
    }
    #[cfg(not(feature = "mojo"))]
    {
        let Some(value) = value else {
            return Ok(());
        };
        if !(minimum..=maximum).contains(&value) {
            bail!("{message} in {}", path.display());
        }
        Ok(())
    }
}

pub(super) fn validate_governance_numeric_non_zero(
    value: Option<u64>,
    path: &Path,
    message: &str,
) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        let Some(value) = value else {
            return Ok(());
        };
        let rule = prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_NON_ZERO,
            value,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        };
        let failed = prodex_mojo_core::policy::validate_numeric_rules(&[rule]).map_err(|_| {
            anyhow::anyhow!("governance policy numeric validation returned invalid output")
        })?;
        if !failed.is_empty() {
            bail!("{message} in {}", path.display());
        }
        Ok(())
    }
    #[cfg(not(feature = "mojo"))]
    {
        let Some(value) = value else {
            return Ok(());
        };
        if value == 0 {
            bail!("{message} in {}", path.display());
        }
        Ok(())
    }
}

pub(super) fn validate_governance_session(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    #[cfg(feature = "mojo")]
    {
        validate_governance_session_mojo(governance, path)
    }
    #[cfg(not(feature = "mojo"))]
    {
        validate_governance_session_rust(governance, path)
    }
}

#[cfg(any(not(feature = "mojo"), test))]
fn validate_governance_session_rust(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    let session = &governance.session;
    if session.absolute_timeout_seconds.is_some_and(|value| {
        !(super::MIN_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS
            ..=super::MAX_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS)
            .contains(&value)
    }) {
        bail!(
            "governance.session.absolute_timeout_seconds in {} is outside the safe range",
            path.display()
        );
    }
    if session.idle_timeout_seconds.is_some_and(|value| {
        !(super::MIN_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS
            ..=super::MAX_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS)
            .contains(&value)
    }) {
        bail!(
            "governance.session.idle_timeout_seconds in {} is outside the safe range",
            path.display()
        );
    }
    if session
        .max_concurrent
        .is_some_and(|value| value == 0 || value > super::MAX_GOVERNANCE_SESSION_CONCURRENT)
    {
        bail!(
            "governance.session.max_concurrent in {} is outside the safe range",
            path.display()
        );
    }
    if let (Some(idle), Some(absolute)) = (
        session.idle_timeout_seconds,
        session.absolute_timeout_seconds,
    ) && idle > absolute
    {
        bail!(
            "governance.session.idle_timeout_seconds in {} cannot exceed absolute_timeout_seconds",
            path.display()
        );
    }
    Ok(())
}

#[cfg(feature = "mojo")]
fn validate_governance_session_mojo(
    governance: &crate::types::RuntimePolicyGovernanceSettings,
    path: &Path,
) -> Result<()> {
    let session = &governance.session;
    let mut rules = Vec::new();
    let mut failure_kinds = Vec::new();

    if let Some(value) = session.absolute_timeout_seconds {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value: u64::from(value),
            minimum: u64::from(super::MIN_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS),
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS),
            related_value: 0,
        });
        failure_kinds.push(0_u8);
    }
    if let Some(value) = session.idle_timeout_seconds {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value: u64::from(value),
            minimum: u64::from(super::MIN_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS),
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS),
            related_value: 0,
        });
        failure_kinds.push(1);
    }
    if let Some(value) = session.max_concurrent {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RANGE,
            value: u64::from(value),
            minimum: 1,
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_CONCURRENT),
            related_value: 0,
        });
        failure_kinds.push(2);
    }
    if let (Some(idle), Some(absolute)) = (
        session.idle_timeout_seconds,
        session.absolute_timeout_seconds,
    ) {
        rules.push(prodex_mojo_core::policy::NumericRule {
            kind: prodex_mojo_core::policy::POLICY_NUMERIC_RELATION_LE,
            value: u64::from(idle),
            minimum: 0,
            maximum: 0,
            related_value: u64::from(absolute),
        });
        failure_kinds.push(3);
    }

    let failed = prodex_mojo_core::policy::validate_numeric_rules(&rules).map_err(|_| {
        anyhow::anyhow!("governance session numeric validation returned invalid output")
    })?;
    if let Some(index) = failed.first() {
        match failure_kinds[*index] {
            0 => bail!(
                "governance.session.absolute_timeout_seconds in {} is outside the safe range",
                path.display()
            ),
            1 => bail!(
                "governance.session.idle_timeout_seconds in {} is outside the safe range",
                path.display()
            ),
            2 => bail!(
                "governance.session.max_concurrent in {} is outside the safe range",
                path.display()
            ),
            3 => bail!(
                "governance.session.idle_timeout_seconds in {} cannot exceed absolute_timeout_seconds",
                path.display()
            ),
            _ => unreachable!("validated governance session failure tag"),
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "mojo"))]
mod mojo_tests {
    use super::*;

    #[test]
    fn mojo_governance_session_numeric_validation_matches_rust_oracle() {
        for input in [
            "version = 1",
            "version = 1\n[governance.session]\nabsolute_timeout_seconds = 299",
            "version = 1\n[governance.session]\nabsolute_timeout_seconds = 86401",
            "version = 1\n[governance.session]\nidle_timeout_seconds = 59",
            "version = 1\n[governance.session]\nmax_concurrent = 0",
            "version = 1\n[governance.session]\nabsolute_timeout_seconds = 300\nidle_timeout_seconds = 301",
            "version = 1\n[governance.session]\nabsolute_timeout_seconds = 299\nidle_timeout_seconds = 0",
        ] {
            let policy = toml::from_str::<crate::types::RuntimePolicyFile>(input).unwrap();
            let path = Path::new("policy.toml");
            assert_eq!(
                validate_governance_session_rust(&policy.governance, path)
                    .map_err(|error| error.to_string()),
                validate_governance_session_mojo(&policy.governance, path)
                    .map_err(|error| error.to_string()),
                "{input}"
            );
        }
    }
}
