//! Typed, fail-closed aggregation and ordering for runtime-doctor bindings.
//! The host supplies binding snapshots; Mojo exclusively owns the numeric
//! reducer, missing-sample cap and descending-count ordering.

use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address};
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const MAX_ROWS: usize = 1_000_000;
const MAX_PROFILES: usize = 1_000_000;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_binding_source_v1(
        abi_version: i64,
        rows: u64,
        row_count: i64,
        profile_count: i64,
        profile_counts: u64,
        order: u64,
        scratch: u64,
        samples: u64,
        summary: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorBindingRow {
    pub profile_index: usize,
    /// 0 response, 1 session, 2 turn-state, 3 session-id.
    pub kind: i64,
    pub bound_at: i64,
    pub known_profile: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDoctorBindingProfilePlan {
    /// Index in the lexical profile-name table supplied by the Rust host.
    pub name_index: usize,
    pub counts: [usize; 4],
    pub total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDoctorBindingSourcePlan {
    pub by_kind: [usize; 4],
    pub total: usize,
    pub oldest_bound_at: Option<i64>,
    pub newest_bound_at: Option<i64>,
    pub missing_profile_bindings: usize,
    /// Source-record indices in the first-eight missing-binding order.
    pub missing_sample_indices: Vec<usize>,
    pub profiles: Vec<RuntimeDoctorBindingProfilePlan>,
}

fn nonnegative(value: i64) -> Result<usize, MojoError> {
    usize::try_from(value).map_err(|_| MojoError::InvalidOutput)
}

/// Aggregate one runtime-doctor binding source through compiled Mojo.
/// Profiles are indexed in ascending BTreeMap name order; Mojo uses these
/// indices to break equal-total ties without comparing or copying strings.
pub fn runtime_doctor_binding_source_plan(
    rows: &[RuntimeDoctorBindingRow],
    profile_count: usize,
) -> Result<RuntimeDoctorBindingSourcePlan, MojoError> {
    ensure_rich_abi()?;
    if rows.len() > MAX_ROWS || profile_count > MAX_PROFILES {
        return Err(MojoError::Capacity);
    }
    let mut input = Vec::<i64>::with_capacity(rows.len() * 4);
    for row in rows {
        input.push(i64::try_from(row.profile_index).map_err(|_| MojoError::InvalidInput)?);
        input.push(row.kind);
        input.push(row.bound_at);
        input.push(i64::from(row.known_profile));
    }
    let mut profile_counts = vec![0_i64; profile_count * 5];
    let mut order = vec![-1_i64; profile_count];
    let mut scratch = vec![-1_i64; profile_count];
    let mut samples = [-1_i64; 8];
    let mut summary = [-1_i64; 10];
    let status = unsafe {
        prodex_mojo_runtime_doctor_binding_source_v1(
            ABI_VERSION,
            mojo_pointer_address(input.as_ptr()),
            i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(profile_count).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(profile_counts.as_mut_ptr()),
            mojo_mut_pointer_address(order.as_mut_ptr()),
            mojo_mut_pointer_address(scratch.as_mut_ptr()),
            mojo_mut_pointer_address(samples.as_mut_ptr()),
            mojo_mut_pointer_address(summary.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }

    let [
        has_entries,
        oldest,
        newest,
        missing,
        sample_count,
        c0,
        c1,
        c2,
        c3,
        total,
    ] = summary;
    if !matches!(has_entries, 0 | 1) || (has_entries == 1) != !rows.is_empty() {
        return Err(MojoError::InvalidOutput);
    }
    let total = nonnegative(total)?;
    if total != rows.len() {
        return Err(MojoError::InvalidOutput);
    }
    let sample_count = nonnegative(sample_count)?;
    if sample_count > samples.len() || sample_count > rows.len() {
        return Err(MojoError::InvalidOutput);
    }
    let missing_profile_bindings = nonnegative(missing)?;
    if sample_count != missing_profile_bindings.min(samples.len()) {
        return Err(MojoError::InvalidOutput);
    }
    let missing_sample_indices = samples[..sample_count]
        .iter()
        .map(|&index| {
            let index = nonnegative(index)?;
            if index >= rows.len() || rows[index].known_profile {
                return Err(MojoError::InvalidOutput);
            }
            Ok(index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if missing_sample_indices
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(MojoError::InvalidOutput);
    }

    let mut seen = vec![false; profile_count];
    let mut profiles = Vec::with_capacity(profile_count);
    for entry in order {
        let index = nonnegative(entry)?;
        if index >= profile_count || std::mem::replace(&mut seen[index], true) {
            return Err(MojoError::InvalidOutput);
        }
        let base = index * 5;
        let mut counts = [0usize; 5];
        for (offset, count) in counts.iter_mut().enumerate() {
            *count = nonnegative(profile_counts[base + offset])?;
        }
        if counts[..4].iter().copied().sum::<usize>() != counts[4] {
            return Err(MojoError::InvalidOutput);
        }
        profiles.push(RuntimeDoctorBindingProfilePlan {
            name_index: index,
            counts: [counts[0], counts[1], counts[2], counts[3]],
            total: counts[4],
        });
    }
    Ok(RuntimeDoctorBindingSourcePlan {
        by_kind: [
            nonnegative(c0)?,
            nonnegative(c1)?,
            nonnegative(c2)?,
            nonnegative(c3)?,
        ],
        total,
        oldest_bound_at: (has_entries == 1).then_some(oldest),
        newest_bound_at: (has_entries == 1).then_some(newest),
        missing_profile_bindings,
        missing_sample_indices,
        profiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_binding_reducer_owns_counts_bounds_samples_and_tied_order() {
        let rows = [
            RuntimeDoctorBindingRow {
                profile_index: 0,
                kind: 0,
                bound_at: 5,
                known_profile: true,
            },
            RuntimeDoctorBindingRow {
                profile_index: 1,
                kind: 2,
                bound_at: -7,
                known_profile: false,
            },
            RuntimeDoctorBindingRow {
                profile_index: 1,
                kind: 0,
                bound_at: 9,
                known_profile: false,
            },
            RuntimeDoctorBindingRow {
                profile_index: 0,
                kind: 1,
                bound_at: 8,
                known_profile: true,
            },
        ];
        let plan = runtime_doctor_binding_source_plan(&rows, 2).unwrap();
        assert_eq!(plan.by_kind, [2, 1, 1, 0]);
        assert_eq!(plan.total, 4);
        assert_eq!(
            (plan.oldest_bound_at, plan.newest_bound_at),
            (Some(-7), Some(9))
        );
        assert_eq!(plan.missing_profile_bindings, 2);
        assert_eq!(plan.missing_sample_indices, [1, 2]);
        assert_eq!(
            plan.profiles,
            vec![
                RuntimeDoctorBindingProfilePlan {
                    name_index: 0,
                    counts: [1, 1, 0, 0],
                    total: 2
                },
                RuntimeDoctorBindingProfilePlan {
                    name_index: 1,
                    counts: [1, 0, 1, 0],
                    total: 2
                },
            ]
        );
        let empty = runtime_doctor_binding_source_plan(&[], 0).unwrap();
        assert_eq!((empty.oldest_bound_at, empty.newest_bound_at), (None, None));
        assert!(empty.profiles.is_empty());
    }

    #[test]
    fn mojo_binding_reducer_caps_missing_samples_and_reorders_by_count() {
        let rows = (0..12)
            .map(|i| RuntimeDoctorBindingRow {
                profile_index: if i < 9 { 2 } else { 0 },
                kind: 3,
                bound_at: i64::MIN + i,
                known_profile: false,
            })
            .collect::<Vec<_>>();
        let plan = runtime_doctor_binding_source_plan(&rows, 3).unwrap();
        assert_eq!(plan.missing_profile_bindings, 12);
        assert_eq!(plan.missing_sample_indices, (0..8).collect::<Vec<_>>());
        assert_eq!(
            plan.profiles
                .iter()
                .map(|row| row.name_index)
                .collect::<Vec<_>>(),
            [2, 0, 1]
        );
        assert_eq!(plan.oldest_bound_at, Some(i64::MIN));
        assert_eq!(plan.newest_bound_at, Some(i64::MIN + 11));
    }

    #[test]
    fn mojo_binding_reducer_rejects_invalid_row_and_abi() {
        let invalid = [RuntimeDoctorBindingRow {
            profile_index: 1,
            kind: 0,
            bound_at: 0,
            known_profile: true,
        }];
        assert_eq!(
            runtime_doctor_binding_source_plan(&invalid, 1),
            Err(MojoError::InvalidInput)
        );
        let invalid = [RuntimeDoctorBindingRow {
            profile_index: 0,
            kind: 4,
            bound_at: 0,
            known_profile: true,
        }];
        assert_eq!(
            runtime_doctor_binding_source_plan(&invalid, 1),
            Err(MojoError::InvalidInput)
        );
        let mut output = [-1_i64; 10];
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_binding_source_v1(
                    2,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr())
                ),
                4
            );
        }
        assert_eq!(output, [-1; 10]);
    }
}
