use super::{
    RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN, RUNTIME_CANDIDATE_DECISION_FIELD_COUNT,
    RUNTIME_CANDIDATE_PLAN_FIELD_COUNT, RUNTIME_CANDIDATE_SKIP_EXCLUDED, RuntimeCandidateDecision,
};

pub(super) fn input_count(
    fields: &[i64],
    excluded: &[i64],
    route_kind: i64,
) -> Result<usize, crate::MojoError> {
    if !fields
        .len()
        .is_multiple_of(RUNTIME_CANDIDATE_PLAN_FIELD_COUNT)
        || !(0..=3).contains(&route_kind)
    {
        return Err(crate::MojoError::InvalidInput);
    }
    let count = fields.len() / RUNTIME_CANDIDATE_PLAN_FIELD_COUNT;
    if excluded.len() != count || excluded.iter().any(|flag| !matches!(flag, 0 | 1)) {
        return Err(crate::MojoError::InvalidInput);
    }
    Ok(count)
}

pub(super) fn output(
    status: i64,
    ready_count: i64,
    fallback_count: i64,
    ready_values: &[i64],
    fallback_values: &[i64],
    decision_tags: &[i64],
    candidate_count: usize,
) -> Result<PlanOutput, crate::MojoError> {
    let ready_count = usize::try_from(ready_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    let fallback_count =
        usize::try_from(fallback_count).map_err(|_| crate::MojoError::InvalidOutput)?;
    let decision_tag_count = candidate_count
        .checked_mul(RUNTIME_CANDIDATE_DECISION_FIELD_COUNT)
        .ok_or(crate::MojoError::InvalidOutput)?;
    if status != 0
        || ready_count > candidate_count
        || fallback_count > candidate_count
        || ready_values.len() != candidate_count
        || fallback_values.len() != candidate_count
        || decision_tags.len() != decision_tag_count
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    let ready_indices = plan_indices(ready_values, ready_count, candidate_count)
        .ok_or(crate::MojoError::InvalidOutput)?;
    let fallback_indices = plan_indices(fallback_values, fallback_count, candidate_count)
        .ok_or(crate::MojoError::InvalidOutput)?;
    let decisions =
        decisions(decision_tags, candidate_count).ok_or(crate::MojoError::InvalidOutput)?;
    if fallback_indices.len()
        != decisions
            .iter()
            .filter(|decision| decision.eligible)
            .count()
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok((ready_indices, fallback_indices, decisions))
}

type PlanOutput = (Vec<usize>, Vec<usize>, Vec<RuntimeCandidateDecision>);

pub(super) fn validate(
    fields: &[i64],
    ready_indices: &[usize],
    fallback_indices: &[usize],
    decisions: &[RuntimeCandidateDecision],
) -> Result<(), crate::MojoError> {
    let mut seen_ready = vec![false; decisions.len()];
    for index in ready_indices {
        if seen_ready[*index]
            || !decisions[*index].eligible
            || fields[*index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT] != 0
        {
            return Err(crate::MojoError::InvalidOutput);
        }
        seen_ready[*index] = true;
    }
    let mut seen_fallback = vec![false; decisions.len()];
    for index in fallback_indices {
        if seen_fallback[*index] || !decisions[*index].eligible {
            return Err(crate::MojoError::InvalidOutput);
        }
        seen_fallback[*index] = true;
    }
    if seen_ready.iter().enumerate().any(|(index, seen)| {
        *seen
            != (decisions[index].eligible
                && fields[index * RUNTIME_CANDIDATE_PLAN_FIELD_COUNT] == 0)
    }) || seen_fallback
        .iter()
        .zip(decisions)
        .any(|(seen, decision)| *seen != decision.eligible)
    {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(())
}

fn decisions(tags: &[i64], candidate_count: usize) -> Option<Vec<RuntimeCandidateDecision>> {
    (0..candidate_count)
        .map(|index| {
            let base = index * RUNTIME_CANDIDATE_DECISION_FIELD_COUNT;
            let eligible = match tags[base] {
                0 => false,
                1 => true,
                _ => return None,
            };
            (0..=RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN)
                .contains(&tags[base + 1])
                .then_some(())?;
            (0..=RUNTIME_CANDIDATE_SKIP_EXCLUDED)
                .contains(&tags[base + 2])
                .then_some(())?;
            (0..=RUNTIME_CANDIDATE_SKIP_EXCLUDED)
                .contains(&tags[base + 3])
                .then_some(())?;
            (0..=RUNTIME_CANDIDATE_SKIP_EXCLUDED)
                .contains(&tags[base + 4])
                .then_some(())?;
            let inflight_soft_limited = match tags[base + 5] {
                0 => false,
                1 => true,
                _ => return None,
            };
            Some(RuntimeCandidateDecision {
                eligible,
                availability: tags[base + 1],
                quota_guard_reason: tags[base + 2],
                ready_skip_reason: tags[base + 3],
                fallback_skip_reason: tags[base + 4],
                inflight_soft_limited,
            })
        })
        .collect()
}

fn plan_indices(values: &[i64], count: usize, candidate_count: usize) -> Option<Vec<usize>> {
    values
        .get(..count)?
        .iter()
        .map(|value| {
            usize::try_from(*value)
                .ok()
                .filter(|index| *index < candidate_count)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_rejects_invalid_inflight_flags_and_buffer_shapes() {
        let mut tags = [0_i64; RUNTIME_CANDIDATE_DECISION_FIELD_COUNT];
        tags[5] = 2;
        assert!(matches!(
            output(0, 0, 0, &[0], &[0], &tags, 1),
            Err(crate::MojoError::InvalidOutput)
        ));

        tags[5] = 1;
        assert!(matches!(
            output(0, 0, 0, &[], &[0], &tags, 1),
            Err(crate::MojoError::InvalidOutput)
        ));
        assert!(matches!(
            output(0, 0, 0, &[0], &[0], &tags[..5], 1),
            Err(crate::MojoError::InvalidOutput)
        ));

        tags[5] = 0;
        assert!(matches!(
            output(0, 0, 0, &[], &[0], &tags, 1),
            Err(crate::MojoError::InvalidOutput)
        ));
    }
}
