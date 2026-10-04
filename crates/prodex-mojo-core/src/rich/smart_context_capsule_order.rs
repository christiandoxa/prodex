use super::{
    MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, status_error,
};

const SMART_CONTEXT_CAPSULE_ORDER_ABI_VERSION: i64 = 1;
const SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT: usize = 65_537;

/// Borrowed capsule keys used to request the production Mojo ordering.
#[derive(Debug, Clone, Copy)]
pub struct SmartContextCapsuleOrderInput<'a> {
    pub id: &'a str,
    pub relevance: f32,
    pub token_cost: usize,
    pub required: bool,
}

unsafe extern "C" {
    fn prodex_mojo_smart_context_capsule_order_v1(
        abi_version: i64,
        id_addresses: u64,
        id_lengths: u64,
        relevances: u64,
        token_costs: u64,
        required: u64,
        permutation: u64,
        scratch: u64,
        count: i64,
    ) -> i64;
}

/// Return the stable capsule-order permutation without moving host-owned IDs.
pub fn order_smart_context_capsules(
    inputs: &[SmartContextCapsuleOrderInput<'_>],
) -> Result<Vec<usize>, MojoError> {
    ensure_rich_abi()?;
    if inputs.len() > SMART_CONTEXT_CAPSULE_ORDER_MAX_COUNT {
        return Err(MojoError::InvalidInput);
    }

    let mut id_addresses = Vec::with_capacity(inputs.len());
    let mut id_lengths = Vec::with_capacity(inputs.len());
    let mut relevances = Vec::with_capacity(inputs.len());
    let mut token_costs = Vec::with_capacity(inputs.len());
    let mut required = Vec::with_capacity(inputs.len());
    for input in inputs {
        id_addresses.push(mojo_pointer_address(input.id.as_ptr()));
        id_lengths.push(i64::try_from(input.id.len()).map_err(|_| MojoError::InvalidInput)?);
        relevances.push(input.relevance);
        token_costs.push(u64::try_from(input.token_cost).map_err(|_| MojoError::InvalidInput)?);
        required.push(i64::from(input.required));
    }

    let mut permutation = vec![-1_i64; inputs.len().max(1)];
    let mut scratch = vec![0_i64; inputs.len().max(1)];
    let status = unsafe {
        prodex_mojo_smart_context_capsule_order_v1(
            SMART_CONTEXT_CAPSULE_ORDER_ABI_VERSION,
            mojo_pointer_address(id_addresses.as_ptr()),
            mojo_pointer_address(id_lengths.as_ptr()),
            mojo_pointer_address(relevances.as_ptr()),
            mojo_pointer_address(token_costs.as_ptr()),
            mojo_pointer_address(required.as_ptr()),
            mojo_mut_pointer_address(permutation.as_mut_ptr()),
            mojo_mut_pointer_address(scratch.as_mut_ptr()),
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(status_error(status, 7, 1, -1, 0));
    }

    let mut seen = vec![false; inputs.len()];
    permutation
        .get(..inputs.len())
        .ok_or(MojoError::InvalidOutput)?
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).map_err(|_| MojoError::InvalidOutput)?;
            let was_seen = seen.get_mut(index).ok_or(MojoError::InvalidOutput)?;
            if *was_seen {
                return Err(MojoError::InvalidOutput);
            }
            *was_seen = true;
            Ok(index)
        })
        .collect()
}
