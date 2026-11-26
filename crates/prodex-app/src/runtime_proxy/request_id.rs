use crate::runtime_state_shared::RuntimeRotationProxyShared;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::Path;
use std::sync::atomic::Ordering;
use std::thread;

const RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS: u64 = 32;
const RUNTIME_REQUEST_SEQUENCE_LOCAL_START: u64 = 1;

fn runtime_proxy_request_id_from_entropy(request_id_entropy: u128, local_sequence: u64) -> u64 {
    let local_mask = (1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1;
    let mut hasher = DefaultHasher::new();
    request_id_entropy.hash(&mut hasher);
    local_sequence.hash(&mut hasher);
    let mut request_prefix = hasher.finish() >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS;
    if request_prefix == 0 {
        request_prefix = 1;
    }
    (request_prefix << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) | (local_sequence & local_mask)
}

pub(crate) fn runtime_proxy_request_sequence_seed(log_path: &Path) -> u64 {
    let request_id_entropy = prodex_domain::RequestId::new().as_uuid().as_u128();
    let pid = std::process::id();
    let thread_id = thread::current().id();
    let mut hasher = DefaultHasher::new();
    request_id_entropy.hash(&mut hasher);
    pid.hash(&mut hasher);
    thread_id.hash(&mut hasher);
    log_path.hash(&mut hasher);
    let mut instance_prefix = hasher.finish() >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS;
    if instance_prefix == 0 {
        instance_prefix = 1;
    }
    (instance_prefix << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) | RUNTIME_REQUEST_SEQUENCE_LOCAL_START
}

pub(crate) fn runtime_proxy_next_request_id(shared: &RuntimeRotationProxyShared) -> u64 {
    let local_sequence = shared.request_sequence.fetch_add(1, Ordering::Relaxed);
    runtime_proxy_request_id_from_entropy(
        prodex_domain::RequestId::new().as_uuid().as_u128(),
        local_sequence,
    )
}

#[cfg(test)]
mod runtime_request_id_tests {
    use super::{
        RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS, RUNTIME_REQUEST_SEQUENCE_LOCAL_START,
        runtime_proxy_request_id_from_entropy, runtime_proxy_request_sequence_seed,
    };
    use std::path::Path;

    #[test]
    fn runtime_request_sequence_seed_reserves_instance_high_bits() {
        let seed =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-a.log"));

        assert_eq!(
            seed & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            RUNTIME_REQUEST_SEQUENCE_LOCAL_START
        );
        assert_ne!(seed >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS, 0);
    }

    #[test]
    fn runtime_request_sequence_seed_includes_runtime_identity_material() {
        let first =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-a.log"));
        let second =
            runtime_proxy_request_sequence_seed(Path::new("/tmp/prodex-runtime-instance-b.log"));

        assert_ne!(
            first >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS,
            second >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS
        );
    }

    #[test]
    fn runtime_request_id_uses_entropy_for_high_bits() {
        let local_sequence = 42;
        let first = runtime_proxy_request_id_from_entropy(1, local_sequence);
        let second = runtime_proxy_request_id_from_entropy(2, local_sequence);

        assert_eq!(
            first & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            local_sequence
        );
        assert_eq!(
            second & ((1_u64 << RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS) - 1),
            local_sequence
        );
        assert_ne!(
            first >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS,
            second >> RUNTIME_REQUEST_SEQUENCE_LOCAL_BITS
        );
    }
}
