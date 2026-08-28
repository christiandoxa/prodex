use super::{
    RUNTIME_CANDIDATE_AVAILABILITY_UNKNOWN, RUNTIME_CANDIDATE_DECISION_FIELD_COUNT,
    RUNTIME_CANDIDATE_PLAN_FIELD_COUNT, RUNTIME_CANDIDATE_PLAN_MAX_COUNT,
    RUNTIME_CANDIDATE_SKIP_EXCLUDED, RuntimeCandidateDecision,
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
    if count > RUNTIME_CANDIDATE_PLAN_MAX_COUNT
        || excluded.len() != count
        || excluded.iter().any(|flag| !matches!(flag, 0 | 1))
    {
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
    if status != 0
        || ready_count < 0
        || fallback_count < 0
        || ready_count as usize > candidate_count
        || fallback_count as usize > candidate_count
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
    if seen_fallback
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
            Some(RuntimeCandidateDecision {
                eligible,
                availability: tags[base + 1],
                quota_guard_reason: tags[base + 2],
                ready_skip_reason: tags[base + 3],
                fallback_skip_reason: tags[base + 4],
            })
        })
        .collect()
}

fn plan_indices(values: &[i64], count: i64, candidate_count: usize) -> Option<Vec<usize>> {
    values
        .get(..usize::try_from(count).ok()?)?
        .iter()
        .map(|value| {
            usize::try_from(*value)
                .ok()
                .filter(|index| *index < candidate_count)
        })
        .collect()
}
