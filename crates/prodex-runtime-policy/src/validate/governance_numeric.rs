use crate::types::{RuntimeGovernancePolicyObligation, RuntimePolicyGovernanceSettings};
use crate::validate_helpers::{NumericRule, failed_numeric_rules};
use anyhow::{Result, bail};
use std::path::Path;

#[derive(Clone, Copy)]
pub(super) enum SessionFailure {
    AbsoluteTimeout,
    IdleTimeout,
    ConcurrentLimit,
    TimeoutOrdering,
}

pub(super) fn validate_governance_session(
    failure: Option<SessionFailure>,
    path: &Path,
) -> Result<()> {
    if let Some(failure) = failure {
        match failure {
            SessionFailure::AbsoluteTimeout => bail!(
                "governance.session.absolute_timeout_seconds in {} is outside the safe range",
                path.display()
            ),
            SessionFailure::IdleTimeout => bail!(
                "governance.session.idle_timeout_seconds in {} is outside the safe range",
                path.display()
            ),
            SessionFailure::ConcurrentLimit => bail!(
                "governance.session.max_concurrent in {} is outside the safe range",
                path.display()
            ),
            SessionFailure::TimeoutOrdering => bail!(
                "governance.session.idle_timeout_seconds in {} cannot exceed absolute_timeout_seconds",
                path.display()
            ),
        }
    }
    Ok(())
}

pub(super) struct GovernanceRuleNumericFailures {
    pub(super) condition: bool,
    pub(super) obligations: Vec<bool>,
}

pub(super) struct GovernanceNumericFailures {
    pub(super) session: Option<SessionFailure>,
    pub(super) policy_rules: Vec<GovernanceRuleNumericFailures>,
}

#[derive(Clone, Copy)]
enum FailureLocation {
    Session(SessionFailure),
    Condition(usize),
    Obligation { rule: usize, obligation: usize },
}

pub(super) fn governance_numeric_failures(
    governance: &RuntimePolicyGovernanceSettings,
) -> Result<GovernanceNumericFailures> {
    let mut policy_rules = governance
        .policy_rules
        .iter()
        .map(|rule| GovernanceRuleNumericFailures {
            condition: false,
            obligations: vec![false; rule.obligations.len().min(super::MAX_POLICY_OBLIGATIONS)],
        })
        .collect::<Vec<_>>();
    let mut rules = Vec::new();
    let mut locations = Vec::new();

    for (rule_index, rule) in governance.policy_rules.iter().enumerate() {
        if let Some(value) = rule.condition.minimum_authentication_strength {
            rules.push(NumericRule::Range {
                value: u64::from(value),
                minimum: 1,
                maximum: 3,
            });
            locations.push(FailureLocation::Condition(rule_index));
        }
        for (obligation_index, obligation) in rule
            .obligations
            .iter()
            .take(super::MAX_POLICY_OBLIGATIONS)
            .enumerate()
        {
            let numeric_rule = match obligation {
                RuntimeGovernancePolicyObligation::MinimumAuthenticationStrength { value } => {
                    Some(NumericRule::Range {
                        value: u64::from(*value),
                        minimum: 1,
                        maximum: 3,
                    })
                }
                RuntimeGovernancePolicyObligation::MaxInputTokens { value }
                | RuntimeGovernancePolicyObligation::MaxOutputTokens { value }
                | RuntimeGovernancePolicyObligation::MaxContextTokens { value }
                | RuntimeGovernancePolicyObligation::SessionIdleTimeoutSeconds { value }
                | RuntimeGovernancePolicyObligation::SessionAbsoluteTimeoutSeconds { value } => {
                    Some(NumericRule::NonZero(u64::from(*value)))
                }
                _ => None,
            };
            if let Some(numeric_rule) = numeric_rule {
                rules.push(numeric_rule);
                locations.push(FailureLocation::Obligation {
                    rule: rule_index,
                    obligation: obligation_index,
                });
            }
        }
    }

    let session = &governance.session;
    if let Some(value) = session.absolute_timeout_seconds {
        rules.push(NumericRule::Range {
            value: u64::from(value),
            minimum: u64::from(super::MIN_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS),
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_ABSOLUTE_TIMEOUT_SECONDS),
        });
        locations.push(FailureLocation::Session(SessionFailure::AbsoluteTimeout));
    }
    if let Some(value) = session.idle_timeout_seconds {
        rules.push(NumericRule::Range {
            value: u64::from(value),
            minimum: u64::from(super::MIN_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS),
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_IDLE_TIMEOUT_SECONDS),
        });
        locations.push(FailureLocation::Session(SessionFailure::IdleTimeout));
    }
    if let Some(value) = session.max_concurrent {
        rules.push(NumericRule::Range {
            value: u64::from(value),
            minimum: 1,
            maximum: u64::from(super::MAX_GOVERNANCE_SESSION_CONCURRENT),
        });
        locations.push(FailureLocation::Session(SessionFailure::ConcurrentLimit));
    }
    if let (Some(idle), Some(absolute)) = (
        session.idle_timeout_seconds,
        session.absolute_timeout_seconds,
    ) {
        rules.push(NumericRule::LessOrEqual {
            value: u64::from(idle),
            maximum: u64::from(absolute),
        });
        locations.push(FailureLocation::Session(SessionFailure::TimeoutOrdering));
    }

    let mut session = None;
    for index in failed_numeric_rules(&rules)? {
        match locations[index] {
            FailureLocation::Session(failure) => {
                session.get_or_insert(failure);
            }
            FailureLocation::Condition(rule) => policy_rules[rule].condition = true,
            FailureLocation::Obligation { rule, obligation } => {
                policy_rules[rule].obligations[obligation] = true;
            }
        }
    }
    Ok(GovernanceNumericFailures {
        session,
        policy_rules,
    })
}
