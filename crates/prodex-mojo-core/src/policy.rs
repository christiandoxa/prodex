pub const POLICY_NUMERIC_NON_ZERO: i64 = 0;
pub const POLICY_NUMERIC_RANGE: i64 = 1;
pub const POLICY_NUMERIC_RELATION_LE: i64 = 2;

mod gateway_admin;
mod governance;
pub use gateway_admin::{
    GatewayAdminRetentionPlan, audit_event_is_expired, audit_hold_is_active,
    audit_retention_cutoff, audit_time_range_contains, compare_audit_positions,
    gateway_admin_purge_protected_count, gateway_admin_retention_cutoff, plan_gateway_admin_limit,
    plan_gateway_admin_retention,
};
pub use governance::{
    governance_policy_conditions_overlap, governance_policy_decision_plan,
    governance_policy_effect, governance_policy_rule_matches, governance_policy_shape,
    governance_required_attributes_present, governance_selector_matches,
};

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyTextKind {
    ObservabilitySchema = 1,
    StateBackend = 2,
    AdminRole = 3,
    WebhookPhase = 4,
    HttpEndpoint = 5,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct PolicyStringView {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernanceOptionalValuePair {
    left: i64,
    left_present: i64,
    right: i64,
    right_present: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernanceOptionalSelectorPair {
    left: PolicyStringView,
    left_present: i64,
    right: PolicyStringView,
    right_present: i64,
    wildcard: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernanceMatchValue {
    condition: u64,
    condition_present: i64,
    input: u64,
    input_present: i64,
    comparison: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernanceMatchSelector {
    selector: PolicyStringView,
    selector_present: i64,
    values: u64,
    value_count: i64,
    wildcard: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernancePolicyRuleShape {
    id: PolicyStringView,
    effect: i64,
    obligation_count: i64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernancePolicyDecisionRule {
    matched: i64,
    effect: i64,
}

unsafe extern "C" {
    fn prodex_mojo_governance_finding_classification_v1(
        abi_version: i64,
        mode: i64,
        values: u64,
        value_count: i64,
        classification: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_migration_step_order_v1(
        abi_version: i64,
        steps: u64,
        step_count: i64,
        output: u64,
    ) -> i64;
    fn prodex_runtime_policy_validate_text(
        abi_version: i64,
        value: u64,
        kind: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_predicates_v1(
        abi_version: i64,
        mode: i64,
        values: u64,
        value_count: i64,
        selectors: u64,
        selector_count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_rule_matches_v1(
        abi_version: i64,
        values: u64,
        value_count: i64,
        selectors: u64,
        selector_count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_required_attributes_v1(
        abi_version: i64,
        required_masks: u64,
        rule_count: i64,
        available_mask: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_effect_v1(
        abi_version: i64,
        effects: u64,
        effect_count: i64,
        default_effect: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_policy_shape_v1(
        abi_version: i64,
        valid_until_unix_ms: u64,
        rules: u64,
        rule_count: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_policy_decision_v1(
        abi_version: i64,
        rules: u64,
        rule_count: i64,
        default_effect: i64,
        matched: u64,
        effect: u64,
    ) -> i64;
}

fn governance_finding_classification(
    mode: i64,
    values: &[i64],
    value_count: usize,
    classification: u8,
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_finding_classification_v1(
            6,
            mode,
            values.as_ptr() as u64,
            i64::try_from(value_count).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(classification),
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    Ok(output)
}

pub fn governance_finding_minimum_classification(kind: u8) -> Result<u8, crate::MojoError> {
    let value = governance_finding_classification(0, &[i64::from(kind)], 1, 0)?;
    let value = u8::try_from(value).map_err(|_| crate::MojoError::InvalidOutput)?;
    (value <= 3)
        .then_some(value)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn governance_findings_exceed_classification(
    kinds: &[u8],
    classification: u8,
) -> Result<bool, crate::MojoError> {
    let values = kinds
        .iter()
        .map(|kind| i64::from(*kind))
        .collect::<Vec<_>>();
    match governance_finding_classification(1, &values, values.len(), classification)? {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_classification_rules_valid(rules: &[(u8, u8)]) -> Result<i64, crate::MojoError> {
    let values = rules
        .iter()
        .flat_map(|(kind, classification)| [i64::from(*kind), i64::from(*classification)])
        .collect::<Vec<_>>();
    let output = governance_finding_classification(2, &values, rules.len(), 0)?;
    (0..=2)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

pub fn migration_step_order(steps: &[i64]) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_migration_step_order_v1(
            1,
            steps.as_ptr() as u64,
            i64::try_from(steps.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (0..=3)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn policy_string_view(value: Option<&str>) -> PolicyStringView {
    value.map_or(PolicyStringView { ptr: 0, len: 0 }, |value| {
        PolicyStringView {
            ptr: value.as_ptr() as u64,
            len: value.len() as u64,
        }
    })
}

pub fn validate_text(value: &str, kind: PolicyTextKind) -> Result<bool, crate::MojoError> {
    let value = PolicyStringView {
        ptr: value.as_ptr() as u64,
        len: value.len() as u64,
    };
    let mut output = -1_i64;
    let status = unsafe {
        prodex_runtime_policy_validate_text(
            6,
            (&value as *const PolicyStringView) as u64,
            kind as i64,
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub const ACCOUNTING_USAGE_ADD: i64 = 0;
pub const ACCOUNTING_USAGE_SATURATING_SUB: i64 = 1;
pub const ACCOUNTING_USAGE_EXCEEDS: i64 = 2;
pub const ACCOUNTING_SNAPSHOT_AVAILABLE: i64 = 3;
pub const ACCOUNTING_RESERVE: i64 = 4;
pub const ACCOUNTING_COMMIT: i64 = 5;
pub const ACCOUNTING_RECORD: i64 = 6;
pub const ACCOUNTING_IS_EXPIRED: i64 = 7;
pub const ACCOUNTING_RELEASE: i64 = 8;
pub const ACCOUNTING_RECONCILE: i64 = 9;
pub const GOVERNANCE_MATCH_EXACT: i64 = 0;
pub const GOVERNANCE_MATCH_MINIMUM: i64 = 1;
pub const GOVERNANCE_MATCH_MAXIMUM: i64 = 2;
pub const GOVERNANCE_MATCH_CONTAINS: i64 = 3;
pub const GOVERNANCE_OBLIGATION_OTHER: i64 = 0;
pub const GOVERNANCE_OBLIGATION_ALLOW_PROVIDER: i64 = 1;
pub const GOVERNANCE_OBLIGATION_DENY_PROVIDER: i64 = 2;
pub const GOVERNANCE_OBLIGATION_REQUIRE_REGION: i64 = 3;
pub const GOVERNANCE_OBLIGATION_PROHIBIT_RETENTION: i64 = 4;
pub const GOVERNANCE_OBLIGATION_RETENTION_SECONDS: i64 = 5;
pub const GOVERNANCE_OBLIGATION_DISABLE_TOOLS: i64 = 6;
pub const GOVERNANCE_OBLIGATION_ALLOW_TOOL: i64 = 7;
pub const GOVERNANCE_OBLIGATION_MAX_INPUT: i64 = 8;
pub const GOVERNANCE_OBLIGATION_MAX_OUTPUT: i64 = 9;
pub const GOVERNANCE_OBLIGATION_MAX_CONTEXT: i64 = 10;
pub const GOVERNANCE_OBLIGATION_SESSION_IDLE: i64 = 11;
pub const GOVERNANCE_OBLIGATION_SESSION_ABSOLUTE: i64 = 12;
pub const GOVERNANCE_OBLIGATION_MIN_AUTHENTICATION: i64 = 13;
pub const GOVERNANCE_APPROVAL_INVALID: i64 = 0;
pub const GOVERNANCE_APPROVAL_VOTE_PENDING: i64 = 1;
pub const GOVERNANCE_APPROVAL_VOTE_APPROVED: i64 = 2;
pub const GOVERNANCE_APPROVAL_REJECT: i64 = 3;
pub const GOVERNANCE_APPROVAL_CANCEL: i64 = 4;
pub const GOVERNANCE_APPROVAL_ACTIVATE: i64 = 5;
pub const GOVERNANCE_APPROVAL_SUPERSEDE: i64 = 6;
pub const GOVERNANCE_APPROVAL_ROLLBACK: i64 = 7;

pub struct GovernanceClassificationInput<'a> {
    pub base_classification: u8,
    pub coverage: u8,
    pub unsupported_coverage_floor: u8,
    pub session_floor: u8,
    pub route_floor: u8,
    pub risk_floor: u8,
    pub trusted_label: Option<u8>,
    pub untrusted_label: Option<u8>,
    pub prior_classification: Option<u8>,
    pub findings: &'a [u8],
    pub rules: &'a [(u8, u8)],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GovernanceClassificationResult {
    pub classification: u8,
    pub reason_bits: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitPlan {
    pub capacity_allows: bool,
    pub reset_unix_ms: u64,
    pub remaining: u64,
    pub retry_after_seconds: u64,
    pub remaining_after_admission: u64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct GovernanceObligationPredicateInput {
    kind: i64,
    value: u64,
    selector: PolicyStringView,
    selector_present: i64,
}

unsafe extern "C" {
    fn prodex_mojo_rate_limit_plan_v1(
        abi_version: i64,
        max_requests: u64,
        window_seconds: u64,
        used_requests: u64,
        current_reset_unix_ms: u64,
        requested_requests: u64,
        now_unix_ms: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_classification_v1(
        abi_version: i64,
        base_classification: i64,
        coverage: i64,
        unsupported_coverage_floor: i64,
        session_floor: i64,
        route_floor: i64,
        risk_floor: i64,
        trusted_label: i64,
        trusted_present: i64,
        untrusted_label: i64,
        untrusted_present: i64,
        prior_classification: i64,
        prior_present: i64,
        findings: u64,
        finding_count: i64,
        rules: u64,
        rule_count: i64,
        classification: u64,
        reason_bits: u64,
    ) -> i64;
    fn prodex_mojo_governance_approval_transition_v1(
        abi_version: i64,
        state: i64,
        action: i64,
        vote_count: u64,
        required_quorum: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_governance_obligation_predicate_v1(
        abi_version: i64,
        mode: i64,
        left: u64,
        right: u64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_policy_refresh_decision_v1(
        abi_version: i64,
        refresh_after_unix_ms: u64,
        stale_after_unix_ms: u64,
        expires_after_unix_ms: u64,
        now_unix_ms: u64,
        active_invalidated: i64,
        last_known_good_present: i64,
        last_known_good_invalidated: i64,
        output: u64,
    ) -> i64;
}

pub fn rate_limit_plan(
    max_requests: u64,
    window_seconds: u64,
    used_requests: u64,
    current_reset_unix_ms: u64,
    requested_requests: u64,
    now_unix_ms: u64,
) -> Result<RateLimitPlan, crate::MojoError> {
    let mut output = [0_u64; 5];
    let status = unsafe {
        prodex_mojo_rate_limit_plan_v1(
            1,
            max_requests,
            window_seconds,
            used_requests,
            current_reset_unix_ms,
            requested_requests,
            now_unix_ms,
            output.as_mut_ptr() as u64,
        )
    };
    if status != 0 || output[0] > 2 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    Ok(RateLimitPlan {
        capacity_allows: output[0] == 2,
        reset_unix_ms: output[1],
        remaining: output[2],
        retry_after_seconds: output[3],
        remaining_after_admission: output[4],
    })
}

pub fn governance_classification(
    input: GovernanceClassificationInput<'_>,
) -> Result<GovernanceClassificationResult, crate::MojoError> {
    let option = |value: Option<u8>| value.map_or((0, 0), |value| (i64::from(value), 1));
    let (trusted, trusted_present) = option(input.trusted_label);
    let (untrusted, untrusted_present) = option(input.untrusted_label);
    let (prior, prior_present) = option(input.prior_classification);
    let findings = input
        .findings
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let rules = input
        .rules
        .iter()
        .flat_map(|(kind, classification)| [i64::from(*kind), i64::from(*classification)])
        .collect::<Vec<_>>();
    let mut classification = -1_i64;
    let mut reason_bits = 0_i64;
    let status = unsafe {
        prodex_mojo_governance_classification_v1(
            6,
            i64::from(input.base_classification),
            i64::from(input.coverage),
            i64::from(input.unsupported_coverage_floor),
            i64::from(input.session_floor),
            i64::from(input.route_floor),
            i64::from(input.risk_floor),
            trusted,
            trusted_present,
            untrusted,
            untrusted_present,
            prior,
            prior_present,
            findings.as_ptr() as u64,
            i64::try_from(findings.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            rules.as_ptr() as u64,
            i64::try_from(input.rules.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut classification as *mut i64) as u64,
            (&mut reason_bits as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    let classification =
        u8::try_from(classification).map_err(|_| crate::MojoError::InvalidOutput)?;
    let reason_bits = u8::try_from(reason_bits).map_err(|_| crate::MojoError::InvalidOutput)?;
    if classification > 3 || reason_bits & !127 != 0 {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(GovernanceClassificationResult {
        classification,
        reason_bits,
    })
}

pub fn governance_approval_transition(
    state: i64,
    action: i64,
    vote_count: usize,
    required_quorum: u8,
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_approval_transition_v1(
            6,
            state,
            action,
            u64::try_from(vote_count).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::from(required_quorum),
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (GOVERNANCE_APPROVAL_INVALID..=GOVERNANCE_APPROVAL_ROLLBACK)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

fn governance_obligation_predicate(
    mode: i64,
    left: (i64, u64, Option<&str>),
    right: Option<(i64, u64, Option<&str>)>,
) -> Result<bool, crate::MojoError> {
    let input = |(kind, value, selector)| GovernanceObligationPredicateInput {
        kind,
        value,
        selector: policy_string_view(selector),
        selector_present: i64::from(selector.is_some()),
    };
    let left = input(left);
    let right = right.map(input);
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_obligation_predicate_v1(
            6,
            mode,
            (&left as *const GovernanceObligationPredicateInput) as u64,
            right.as_ref().map_or(0, |right| {
                (right as *const GovernanceObligationPredicateInput) as u64
            }),
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match output {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn governance_obligation_bound_is_invalid(
    input: (i64, u64, Option<&str>),
) -> Result<bool, crate::MojoError> {
    governance_obligation_predicate(0, input, None)
}

pub fn governance_obligations_conflict(
    left: (i64, u64, Option<&str>),
    right: (i64, u64, Option<&str>),
) -> Result<bool, crate::MojoError> {
    governance_obligation_predicate(1, left, Some(right))
}

pub fn policy_refresh_decision(
    window: [u64; 3],
    now_unix_ms: u64,
    active_invalidated: bool,
    last_known_good_present: bool,
    last_known_good_invalidated: bool,
) -> Result<i64, crate::MojoError> {
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_policy_refresh_decision_v1(
            1,
            window[0],
            window[1],
            window[2],
            now_unix_ms,
            i64::from(active_invalidated),
            i64::from(last_known_good_present),
            i64::from(last_known_good_invalidated),
            (&mut output as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    (0..=4)
        .contains(&output)
        .then_some(output)
        .ok_or(crate::MojoError::InvalidOutput)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccountingOperation {
    pub result_code: i64,
    pub values: [u64; 4],
}

unsafe extern "C" {
    fn prodex_domain_accounting_arithmetic_v1(
        abi_version: i64,
        operation: i64,
        values: u64,
        value_count: i64,
        output: u64,
        result: u64,
    ) -> i64;
}

pub fn accounting_operation(
    operation: i64,
    values: &[u64],
) -> Result<AccountingOperation, crate::MojoError> {
    if values.is_empty() || values.len() > 8 {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut output = [0_u64; 4];
    let mut result_code = -1_i64;
    let status = unsafe {
        prodex_domain_accounting_arithmetic_v1(
            6,
            operation,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
            (&mut result_code as *mut i64) as u64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 | 2 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    let valid_result = match operation {
        ACCOUNTING_USAGE_ADD | ACCOUNTING_SNAPSHOT_AVAILABLE => matches!(result_code, 0 | 1),
        ACCOUNTING_USAGE_SATURATING_SUB | ACCOUNTING_USAGE_EXCEEDS | ACCOUNTING_IS_EXPIRED => {
            result_code == 0
        }
        ACCOUNTING_RESERVE | ACCOUNTING_COMMIT => (0..=4).contains(&result_code),
        ACCOUNTING_RECORD => (0..=3).contains(&result_code),
        ACCOUNTING_RELEASE | ACCOUNTING_RECONCILE => (0..=2).contains(&result_code),
        _ => false,
    };
    if !valid_result {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(AccountingOperation {
        result_code,
        values: output,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumericRule {
    pub kind: i64,
    pub value: u64,
    pub minimum: u64,
    pub maximum: u64,
    pub related_value: u64,
}

unsafe extern "C" {
    fn prodex_runtime_policy_validate_numeric(
        values: *const u64,
        kinds: *const i64,
        minimums: *const u64,
        maximums: *const u64,
        related_values: *const u64,
        failed_rules: *mut i64,
        count: i64,
    ) -> i64;
}

pub fn validate_numeric_rules(rules: &[NumericRule]) -> Result<Vec<usize>, crate::MojoError> {
    if rules.is_empty() {
        return Ok(Vec::new());
    }
    if rules.iter().any(|rule| {
        !matches!(
            rule.kind,
            POLICY_NUMERIC_NON_ZERO | POLICY_NUMERIC_RANGE | POLICY_NUMERIC_RELATION_LE
        ) || (rule.kind == POLICY_NUMERIC_RANGE && rule.minimum > rule.maximum)
    }) {
        return Err(crate::MojoError::InvalidInput);
    }

    let values = rules.iter().map(|rule| rule.value).collect::<Vec<_>>();
    let kinds = rules.iter().map(|rule| rule.kind).collect::<Vec<_>>();
    let minimums = rules.iter().map(|rule| rule.minimum).collect::<Vec<_>>();
    let maximums = rules.iter().map(|rule| rule.maximum).collect::<Vec<_>>();
    let related_values = rules
        .iter()
        .map(|rule| rule.related_value)
        .collect::<Vec<_>>();
    let mut failed_rules = vec![0_i64; rules.len()];
    let status = unsafe {
        prodex_runtime_policy_validate_numeric(
            values.as_ptr(),
            kinds.as_ptr(),
            minimums.as_ptr(),
            maximums.as_ptr(),
            related_values.as_ptr(),
            failed_rules.as_mut_ptr(),
            i64::try_from(rules.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || failed_rules.iter().any(|failed| !matches!(failed, 0 | 1)) {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok(failed_rules
        .into_iter()
        .enumerate()
        .filter_map(|(index, failed)| (failed == 1).then_some(index))
        .collect())
}

pub fn self_test() -> bool {
    validate_numeric_rules(&[
        NumericRule {
            kind: POLICY_NUMERIC_NON_ZERO,
            value: 1,
            minimum: 0,
            maximum: u64::MAX,
            related_value: 0,
        },
        NumericRule {
            kind: POLICY_NUMERIC_RANGE,
            value: 10,
            minimum: 1,
            maximum: 10,
            related_value: 0,
        },
        NumericRule {
            kind: POLICY_NUMERIC_RELATION_LE,
            value: 2,
            minimum: 0,
            maximum: 0,
            related_value: 3,
        },
    ])
    .is_ok_and(|failed| failed.is_empty())
        && governance_policy_shape(1, &[("rule-a", 0, 0)]).is_ok_and(|result| result == 0)
        && governance_policy_shape(0, &[]).is_ok_and(|result| result == 1)
        && governance_policy_shape(1, &[("rule-a", 0, 0), ("rule-a", 0, 0)])
            .is_ok_and(|result| result == 2)
        && governance_policy_decision_plan(&[(true, 0), (false, 2), (true, 1)], 2)
            .is_ok_and(|result| result == (1, vec![true, false, true]))
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn numeric_validation_self_test_passes() {
    assert!(self_test());
}

#[cfg(all(test, feature = "mojo-runtime"))]
#[test]
fn numeric_validation_preserves_failures_across_large_batches() {
    let valid = NumericRule {
        kind: POLICY_NUMERIC_NON_ZERO,
        value: 1,
        minimum: 0,
        maximum: u64::MAX,
        related_value: 0,
    };
    let mut rules = vec![valid; 130];
    rules[0].value = 0;
    rules[63] = NumericRule {
        kind: POLICY_NUMERIC_RANGE,
        value: 0,
        minimum: 1,
        maximum: 3,
        related_value: 0,
    };
    rules[64] = NumericRule {
        kind: POLICY_NUMERIC_RANGE,
        value: 4,
        minimum: 1,
        maximum: 3,
        related_value: 0,
    };
    rules[129] = NumericRule {
        kind: POLICY_NUMERIC_RELATION_LE,
        value: 2,
        minimum: 0,
        maximum: 0,
        related_value: 1,
    };

    assert_eq!(
        validate_numeric_rules(&rules).expect("fixed policy rules should be ABI-valid"),
        vec![0, 63, 64, 129]
    );
}
