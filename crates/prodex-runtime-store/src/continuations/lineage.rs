use super::*;

pub use prodex_runtime_state::{
    RUNTIME_COMPACT_SESSION_LINEAGE_PREFIX, RUNTIME_COMPACT_TURN_STATE_LINEAGE_PREFIX,
    RUNTIME_RESPONSE_TURN_STATE_LINEAGE_PREFIX, runtime_compact_session_lineage_key,
    runtime_compact_turn_state_lineage_key, runtime_is_compact_session_lineage_key,
    runtime_is_response_turn_state_lineage_key, runtime_response_turn_state_lineage_key,
    runtime_response_turn_state_lineage_parts,
};

pub fn runtime_external_response_profile_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| {
            prodex_runtime_state::runtime_lineage_key_is_bounded(key)
                && !runtime_is_response_turn_state_lineage_key(key)
        })
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_external_session_id_bindings(
    bindings: &BTreeMap<String, ResponseProfileBinding>,
) -> BTreeMap<String, ResponseProfileBinding> {
    bindings
        .iter()
        .filter(|(key, _)| {
            prodex_runtime_state::runtime_lineage_key_is_bounded(key)
                && !runtime_is_compact_session_lineage_key(key)
        })
        .map(|(key, binding)| (key.clone(), binding.clone()))
        .collect()
}

pub fn runtime_hard_binding_owner(
    identity: &prodex_runtime_state::RuntimeHardBindingIdentity,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_profile_bindings: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> prodex_runtime_state::RuntimeHardBindingOwner {
    runtime_hard_binding_resolution(
        identity,
        response_bindings,
        turn_state_bindings,
        session_id_bindings,
        session_profile_bindings,
        profiles,
    )
    .owner()
}

pub fn runtime_hard_binding_resolution(
    identity: &prodex_runtime_state::RuntimeHardBindingIdentity,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_profile_bindings: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> prodex_runtime_state::RuntimeHardBindingResolution {
    if [
        identity.response_id.as_deref(),
        identity.turn_state.as_deref(),
        identity.session_id.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|value| !prodex_runtime_state::runtime_identity_component_is_valid(value))
    {
        return prodex_runtime_state::RuntimeHardBindingResolution::Conflict;
    }
    let mut owners = std::collections::BTreeSet::new();
    let mut unavailable = std::collections::BTreeSet::new();
    let mut binding_identities = Vec::new();
    let mut conflict = false;

    let mut inspect = |binding: Option<&ResponseProfileBinding>| {
        let Some(binding) = binding else {
            return;
        };
        if !prodex_runtime_state::runtime_identity_component_is_valid(&binding.profile_name)
            || binding.profile_name == prodex_runtime_state::RUNTIME_HARD_BINDING_CONFLICT_PROFILE
            || binding.profile_name == prodex_state::HARD_BINDING_CONFLICT_PROFILE
        {
            conflict = true;
        } else if profiles.contains_key(&binding.profile_name) {
            owners.insert(binding.profile_name.clone());
        } else {
            unavailable.insert(binding.profile_name.clone());
        }
        if let Some(binding_identity) = binding.binding_identity.as_ref() {
            if binding_identities
                .iter()
                .any(|existing| existing != binding_identity)
            {
                conflict = true;
            } else if binding_identities.is_empty() {
                binding_identities.push(binding_identity.clone());
            }
        }
    };

    inspect(
        identity
            .response_id
            .as_deref()
            .and_then(|key| response_bindings.get(key)),
    );
    inspect(
        identity
            .turn_state
            .as_deref()
            .and_then(|key| turn_state_bindings.get(key)),
    );
    if let (Some(response_id), Some(turn_state)) = (
        identity.response_id.as_deref(),
        identity.turn_state.as_deref(),
    ) {
        inspect(response_bindings.get(
            &prodex_runtime_state::runtime_response_turn_state_lineage_key(response_id, turn_state),
        ));
    }
    if let Some(turn_state) = identity.turn_state.as_deref() {
        inspect(
            turn_state_bindings
                .get(&prodex_runtime_state::runtime_compact_turn_state_lineage_key(turn_state)),
        );
    }
    if let Some(session_id) = identity.session_id.as_deref() {
        inspect(session_id_bindings.get(session_id));
        inspect(session_id_bindings.get(
            &prodex_runtime_state::runtime_compact_session_lineage_key(session_id),
        ));
        inspect(session_profile_bindings.get(session_id));
    }

    if conflict
        || owners.len() > 1
        || unavailable.len() > 1
        || (owners.len() == 1 && !unavailable.is_empty())
    {
        return prodex_runtime_state::RuntimeHardBindingResolution::Conflict;
    }
    let binding_identity = binding_identities.pop();
    if let Some(profile_name) = owners.into_iter().next() {
        return prodex_runtime_state::RuntimeHardBindingResolution::Owned {
            profile_name,
            binding_identity,
        };
    }
    if let Some(profile_name) = unavailable.into_iter().next() {
        return prodex_runtime_state::RuntimeHardBindingResolution::Unavailable {
            profile_name,
            binding_identity,
        };
    }
    prodex_runtime_state::RuntimeHardBindingResolution::Unbound
}

pub fn resolve_runtime_hard_binding_identity(
    identity: &prodex_runtime_state::RuntimeHardBindingIdentity,
    response_bindings: &BTreeMap<String, ResponseProfileBinding>,
    turn_state_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_id_bindings: &BTreeMap<String, ResponseProfileBinding>,
    session_profile_bindings: &BTreeMap<String, ResponseProfileBinding>,
    profiles: &BTreeMap<String, ProfileEntry>,
) -> prodex_runtime_state::RuntimeHardBindingOwner {
    runtime_hard_binding_owner(
        identity,
        response_bindings,
        turn_state_bindings,
        session_id_bindings,
        session_profile_bindings,
        profiles,
    )
}
