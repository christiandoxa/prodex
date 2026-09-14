pub const POLICY_NUMERIC_NON_ZERO: i64 = 0;
pub const POLICY_NUMERIC_RANGE: i64 = 1;
pub const POLICY_NUMERIC_RELATION_LE: i64 = 2;

mod gateway_admin;
pub use gateway_admin::{
    GatewayAdminRetentionPlan, audit_event_is_expired, audit_hold_is_active,
    audit_retention_cutoff, audit_time_range_contains, compare_audit_positions,
    gateway_admin_purge_protected_count, gateway_admin_retention_cutoff, plan_gateway_admin_limit,
    plan_gateway_admin_retention,
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

unsafe extern "C" {
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
}

fn policy_string_view(value: Option<&str>) -> PolicyStringView {
    value.map_or(PolicyStringView { ptr: 0, len: 0 }, |value| {
        PolicyStringView {
            ptr: value.as_ptr() as u64,
            len: value.len() as u64,
        }
    })
}

fn governance_predicates(
    mode: i64,
    values: &[(Option<i64>, Option<i64>)],
    selectors: &[(Option<&str>, Option<&str>, bool)],
) -> Result<bool, crate::MojoError> {
    let values = values
        .iter()
        .map(|(left, right)| GovernanceOptionalValuePair {
            left: left.unwrap_or_default(),
            left_present: i64::from(left.is_some()),
            right: right.unwrap_or_default(),
            right_present: i64::from(right.is_some()),
        })
        .collect::<Vec<_>>();
    let selectors = selectors
        .iter()
        .map(|(left, right, wildcard)| GovernanceOptionalSelectorPair {
            left: policy_string_view(*left),
            left_present: i64::from(left.is_some()),
            right: policy_string_view(*right),
            right_present: i64::from(right.is_some()),
            wildcard: i64::from(*wildcard),
        })
        .collect::<Vec<_>>();
    let mut output = -1_i64;
    let status = unsafe {
        prodex_mojo_governance_predicates_v1(
            6,
            mode,
            values.as_ptr() as u64,
            i64::try_from(values.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            selectors.as_ptr() as u64,
            i64::try_from(selectors.len()).map_err(|_| crate::MojoError::InvalidInput)?,
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

pub fn governance_selector_matches(selector: &str, value: &str) -> Result<bool, crate::MojoError> {
    governance_predicates(0, &[], &[(Some(selector), Some(value), true)])
}

pub fn governance_policy_conditions_overlap(
    values: &[(Option<i64>, Option<i64>)],
    exact_selectors: &[(Option<&str>, Option<&str>)],
    wildcard_selectors: &[(Option<&str>, Option<&str>)],
) -> Result<bool, crate::MojoError> {
    let selectors = exact_selectors
        .iter()
        .map(|(left, right)| (*left, *right, false))
        .chain(
            wildcard_selectors
                .iter()
                .map(|(left, right)| (*left, *right, true)),
        )
        .collect::<Vec<_>>();
    governance_predicates(1, values, &selectors)
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
