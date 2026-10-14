//! Runtime-doctor marker context and field aggregation ABI.
//!
//! Mojo groups the bounded parsed-event batch and returns representative input
//! indices. Rust retains the owned strings and projects the validated rows into
//! the existing diagnostic maps.

use super::{
    RichStringView, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view,
};
use crate::MojoError;

/// Version of the runtime-doctor marker aggregation ABI.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_ABI_VERSION: i64 = 1;
/// Maximum number of parsed marker events passed to one Mojo call.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH: usize = 256;
/// Number of facet values supplied for each parsed marker event.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT: usize = 32;
const VALUE_WIDTH: usize = 6 + RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT;
const MAX_ROWS_PER_EVENT: usize = 1 + 3 + 1 + 2 + RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT;
const MAX_OUTPUT_ROWS: usize = RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH * MAX_ROWS_PER_EVENT;

/// Output kind for one marker's total count.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL: i64 = 0;
/// Output kind for a marker's non-sentinel route value.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE: i64 = 1;
/// Output kind for a marker's non-sentinel lane value.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE: i64 = 2;
/// Output kind for a marker's non-sentinel profile value.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_PROFILE: i64 = 3;
/// Output kind for `chain_retried_owner` reasons.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON: i64 = 4;
/// Output kind for `chain_dead_upstream_confirmed` reasons.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON: i64 = 5;
/// Output kind for `stale_continuation` reasons.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON: i64 = 6;
/// Output kind for `previous_response_not_found` routes.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE: i64 = 7;
/// Output kind for `previous_response_not_found` transports.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT: i64 = 8;
/// Output kind for `previous_response_fresh_fallback_blocked` request shapes.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_REQUEST_SHAPE: i64 = 9;
/// First output kind assigned to the ordered `RUNTIME_DOCTOR_FACETS` values.
pub const RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE: i64 = 10;
const RUNTIME_DOCTOR_MARKER_CONTEXT_SUMMARY_ORDER_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_MARKER_CONTEXT_SUMMARY_ORDER_MAX_COUNT: usize = 65_537;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_marker_context_summary_order_v1(
        abi_version: i64,
        marker_views: u64,
        totals: u64,
        included: u64,
        input_count: i64,
        permutation: u64,
        scratch: u64,
        output_count: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_marker_aggregation_v1(
        abi_version: i64,
        markers: u64,
        values: u64,
        presence: u64,
        weights: u64,
        input_count: i64,
        output_representatives: u64,
        output_kinds: u64,
        output_value_slots: u64,
        output_counts: u64,
        output_capacity: i64,
        output_count: u64,
        latest_stale_reason_index: u64,
    ) -> i64;
}

/// One runtime-doctor marker context row projected into the Mojo ordering ABI.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorMarkerContextSummaryOrderInput<'a> {
    pub marker: &'a str,
    pub total: usize,
    pub has_context: bool,
}

/// Filter context-bearing rows and order them through the versioned Mojo reducer.
pub fn runtime_doctor_marker_context_summary_order(
    input: &[RuntimeDoctorMarkerContextSummaryOrderInput<'_>],
) -> Result<Vec<usize>, MojoError> {
    ensure_rich_abi()?;
    if input.len() > RUNTIME_DOCTOR_MARKER_CONTEXT_SUMMARY_ORDER_MAX_COUNT {
        return Err(MojoError::Capacity);
    }
    let markers = input.iter().map(|row| view(row.marker)).collect::<Vec<_>>();
    let totals = input
        .iter()
        .map(|row| i64::try_from(row.total).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let included = input
        .iter()
        .map(|row| i64::from(row.has_context))
        .collect::<Vec<_>>();
    let mut permutation = vec![-1_i64; input.len().max(1)];
    let mut scratch = vec![-1_i64; input.len().max(1)];
    let mut output_count = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_context_summary_order_v1(
            RUNTIME_DOCTOR_MARKER_CONTEXT_SUMMARY_ORDER_ABI_VERSION,
            mojo_pointer_address(markers.as_ptr()),
            mojo_pointer_address(totals.as_ptr()),
            mojo_pointer_address(included.as_ptr()),
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(permutation.as_mut_ptr()),
            mojo_mut_pointer_address(scratch.as_mut_ptr()),
            mojo_mut_pointer_address(&mut output_count),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > input.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut seen = vec![false; input.len()];
    permutation
        .get(..output_count)
        .ok_or(MojoError::InvalidOutput)?
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).map_err(|_| MojoError::InvalidOutput)?;
            let seen = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *seen || !input[index].has_context {
                return Err(MojoError::InvalidOutput);
            }
            *seen = true;
            Ok(index)
        })
        .collect()
}

/// Parsed fields supplied to the bounded marker aggregation kernel.
#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorMarkerAggregationInput<'a, 'b> {
    pub marker: &'a str,
    pub route: Option<&'a str>,
    pub lane: Option<&'a str>,
    pub profile: Option<&'a str>,
    pub reason: Option<&'a str>,
    pub transport: Option<&'a str>,
    pub request_shape: Option<&'a str>,
    pub facets: &'b [Option<&'a str>],
}

/// One Mojo-grouped aggregation row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorMarkerAggregationRow {
    pub representative_index: usize,
    pub kind: i64,
    pub value_slot: Option<usize>,
    pub count: usize,
}

/// Bounded marker aggregation output, including stale-reason chronology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDoctorMarkerAggregation {
    pub rows: Vec<RuntimeDoctorMarkerAggregationRow>,
    pub latest_stale_reason_index: Option<usize>,
}

/// Group marker context, marker-specific reasons, continuation fields, and
/// ordered facet values through the versioned Mojo reducer.
pub fn runtime_doctor_marker_aggregation<'a, 'b>(
    input: &[RuntimeDoctorMarkerAggregationInput<'a, 'b>],
) -> Result<RuntimeDoctorMarkerAggregation, MojoError> {
    ensure_rich_abi()?;
    if input.len() > RUNTIME_DOCTOR_MARKER_AGGREGATION_MAX_BATCH {
        return Err(MojoError::Capacity);
    }
    if input.is_empty() {
        return Ok(RuntimeDoctorMarkerAggregation {
            rows: Vec::new(),
            latest_stale_reason_index: None,
        });
    }

    let MarkerAggregationInputBuffers {
        markers,
        values,
        presence,
    } = MarkerAggregationInputBuffers::encode(input)?;
    let weights = vec![1_i64; input.len()];

    let input_count = i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?;
    let output_capacity = input
        .len()
        .checked_mul(MAX_ROWS_PER_EVENT)
        .ok_or(MojoError::InvalidInput)?;
    if output_capacity > MAX_OUTPUT_ROWS {
        return Err(MojoError::Capacity);
    }
    let mut representatives = vec![0_i64; output_capacity];
    let mut kinds = vec![0_i64; output_capacity];
    let mut value_slots = vec![-1_i64; output_capacity];
    let mut counts = vec![0_i64; output_capacity];
    let mut output_count = 0_i64;
    let mut latest_stale_reason_index = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_marker_aggregation_v1(
            RUNTIME_DOCTOR_MARKER_AGGREGATION_ABI_VERSION,
            mojo_pointer_address(markers.as_ptr()),
            mojo_pointer_address(values.as_ptr()),
            mojo_pointer_address(presence.as_ptr()),
            mojo_pointer_address(weights.as_ptr()),
            input_count,
            mojo_mut_pointer_address(representatives.as_mut_ptr()),
            mojo_mut_pointer_address(kinds.as_mut_ptr()),
            mojo_mut_pointer_address(value_slots.as_mut_ptr()),
            mojo_mut_pointer_address(counts.as_mut_ptr()),
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut output_count),
            mojo_mut_pointer_address(&mut latest_stale_reason_index),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 | 5 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > output_capacity {
        return Err(MojoError::InvalidOutput);
    }
    let latest_stale_reason_index = match latest_stale_reason_index {
        -1 => None,
        value if value >= 0 => Some(usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?),
        _ => return Err(MojoError::InvalidOutput),
    };
    if latest_stale_reason_index.is_some_and(|index| index >= input.len()) {
        return Err(MojoError::InvalidOutput);
    }

    let mut rows = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let representative_index =
            usize::try_from(representatives[index]).map_err(|_| MojoError::InvalidOutput)?;
        if representative_index >= input.len() {
            return Err(MojoError::InvalidOutput);
        }
        let kind = kinds[index];
        let value_slot = validate_value_slot(kind, value_slots[index])?;
        let count = usize::try_from(counts[index]).map_err(|_| MojoError::InvalidOutput)?;
        if count == 0 {
            return Err(MojoError::InvalidOutput);
        }
        rows.push(RuntimeDoctorMarkerAggregationRow {
            representative_index,
            kind,
            value_slot,
            count,
        });
    }
    Ok(RuntimeDoctorMarkerAggregation {
        rows,
        latest_stale_reason_index,
    })
}

/// Rust owns string borrows and contiguous wire buffers; Mojo still owns all
/// grouping, sentinel handling, counts and per-marker decisions.
struct MarkerAggregationInputBuffers {
    markers: Vec<RichStringView>,
    values: Vec<RichStringView>,
    presence: Vec<i64>,
}

impl MarkerAggregationInputBuffers {
    fn encode(input: &[RuntimeDoctorMarkerAggregationInput<'_, '_>]) -> Result<Self, MojoError> {
        let mut markers = Vec::with_capacity(input.len());
        let mut values = Vec::with_capacity(
            input
                .len()
                .checked_mul(VALUE_WIDTH)
                .ok_or(MojoError::InvalidInput)?,
        );
        let mut presence = Vec::with_capacity(input.len());
        for event in input {
            if event.facets.len() != RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT {
                return Err(MojoError::InvalidInput);
            }
            markers.push(view(event.marker));
            let fixed = [
                event.route,
                event.lane,
                event.profile,
                event.reason,
                event.transport,
                event.request_shape,
            ];
            let mut mask = 0_i64;
            for (slot, value) in fixed.into_iter().enumerate() {
                append_value(&mut values, &mut mask, slot, value)?;
            }
            for (facet_index, value) in event.facets.iter().copied().enumerate() {
                append_value(&mut values, &mut mask, 6 + facet_index, value)?;
            }
            presence.push(mask);
        }
        Ok(Self {
            markers,
            values,
            presence,
        })
    }
}

fn append_value(
    values: &mut Vec<RichStringView>,
    presence: &mut i64,
    slot: usize,
    value: Option<&str>,
) -> Result<(), MojoError> {
    let bit = 1_i64
        .checked_shl(u32::try_from(slot).map_err(|_| MojoError::InvalidInput)?)
        .ok_or(MojoError::InvalidInput)?;
    if let Some(value) = value {
        *presence |= bit;
        values.push(view(value));
    } else {
        values.push(RichStringView::default());
    }
    Ok(())
}

fn validate_value_slot(kind: i64, value_slot: i64) -> Result<Option<usize>, MojoError> {
    let expected = match kind {
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_MARKER_TOTAL => None,
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_ROUTE => Some(0),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_LANE => Some(1),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CONTEXT_PROFILE => Some(2),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_RETRIED_REASON
        | RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_CHAIN_DEAD_REASON
        | RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_STALE_REASON => Some(3),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_ROUTE => Some(0),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_TRANSPORT => Some(4),
        RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_PREVIOUS_REQUEST_SHAPE => Some(5),
        kind if (RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE
            ..RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE
                + RUNTIME_DOCTOR_MARKER_AGGREGATION_FACET_COUNT as i64)
            .contains(&kind) =>
        {
            Some(
                usize::try_from(kind - RUNTIME_DOCTOR_MARKER_AGGREGATION_KIND_FACET_BASE)
                    .map_err(|_| MojoError::InvalidOutput)?
                    + 6,
            )
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    match (expected, value_slot) {
        (None, -1) => Ok(None),
        (Some(expected), actual) if actual == expected as i64 => Ok(Some(expected)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeDoctorMarkerContextSummaryOrderInput, runtime_doctor_marker_context_summary_order,
    };

    #[test]
    fn mojo_marker_context_order_filters_and_breaks_ties_by_marker_bytes() {
        let input = [
            RuntimeDoctorMarkerContextSummaryOrderInput {
                marker: "zeta",
                total: 3,
                has_context: true,
            },
            RuntimeDoctorMarkerContextSummaryOrderInput {
                marker: "alpha",
                total: 3,
                has_context: true,
            },
            RuntimeDoctorMarkerContextSummaryOrderInput {
                marker: "ignored",
                total: 99,
                has_context: false,
            },
            RuntimeDoctorMarkerContextSummaryOrderInput {
                marker: "beta",
                total: 4,
                has_context: true,
            },
            RuntimeDoctorMarkerContextSummaryOrderInput {
                marker: "é",
                total: 3,
                has_context: true,
            },
        ];

        assert_eq!(
            runtime_doctor_marker_context_summary_order(&input).unwrap(),
            [3, 1, 0, 4],
        );
    }
}
