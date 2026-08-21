pub const POLICY_NUMERIC_NON_ZERO: i64 = 0;
pub const POLICY_NUMERIC_RANGE: i64 = 1;
pub const POLICY_NUMERIC_RELATION_LE: i64 = 2;

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
        failed_rules: *mut u64,
        count: i64,
    ) -> i64;
}

pub fn validate_numeric_rules(rules: &[NumericRule]) -> Result<Vec<usize>, crate::MojoError> {
    if rules.len() > 64
        || rules.iter().any(|rule| {
            !matches!(
                rule.kind,
                POLICY_NUMERIC_NON_ZERO | POLICY_NUMERIC_RANGE | POLICY_NUMERIC_RELATION_LE
            ) || (rule.kind == POLICY_NUMERIC_RANGE && rule.minimum > rule.maximum)
        })
    {
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
    let mut failed_rules = 0_u64;
    let status = unsafe {
        prodex_runtime_policy_validate_numeric(
            values.as_ptr(),
            kinds.as_ptr(),
            minimums.as_ptr(),
            maximums.as_ptr(),
            related_values.as_ptr(),
            &mut failed_rules,
            i64::try_from(rules.len()).map_err(|_| crate::MojoError::InvalidInput)?,
        )
    };
    if status != 0 || (rules.len() < 64 && failed_rules >> rules.len() != 0) {
        return Err(crate::MojoError::InvalidOutput);
    }

    Ok((0..rules.len())
        .filter(|index| failed_rules & (1_u64 << index) != 0)
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
fn numeric_validation_matches_rust_oracle_for_generated_batches() {
    let mut state = 0x706f6c696379_u64;
    for case in 0..2_000 {
        let count = (next_random(&mut state) % 65) as usize;
        let mut rules = Vec::with_capacity(count);
        for _ in 0..count {
            let kind = (next_random(&mut state) % 3) as i64;
            let minimum = next_random(&mut state) % 100;
            let maximum = minimum + next_random(&mut state) % 100;
            rules.push(NumericRule {
                kind,
                value: next_random(&mut state) % (maximum + 2),
                minimum,
                maximum,
                related_value: next_random(&mut state) % 100,
            });
        }
        let expected = rules
            .iter()
            .enumerate()
            .filter_map(|(index, rule)| {
                let failed = match rule.kind {
                    POLICY_NUMERIC_NON_ZERO => rule.value == 0,
                    POLICY_NUMERIC_RANGE => rule.value < rule.minimum || rule.value > rule.maximum,
                    POLICY_NUMERIC_RELATION_LE => rule.value > rule.related_value,
                    _ => false,
                };
                failed.then_some(index)
            })
            .collect::<Vec<_>>();
        let actual = validate_numeric_rules(&rules)
            .expect("generated policy numeric rules should be ABI-valid");
        assert_eq!(actual, expected, "policy numeric case {case}");
    }
}

#[cfg(all(test, feature = "mojo-runtime"))]
fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
