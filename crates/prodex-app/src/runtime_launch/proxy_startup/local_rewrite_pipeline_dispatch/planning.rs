use prodex_provider_core::ProviderId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeLocalRewriteCandidateAttempt {
    Stop,
    Skip,
    Primary,
    Fallback,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeLocalRewriteAttemptResult {
    Success,
    Retry,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeLocalRewriteBindingDecision {
    Valid,
    MissingIdentity,
    ProviderMismatch,
    IdentityMismatch,
    SelectedIdentityRequired,
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_local_rewrite_candidate_count(
    hard_continuation: bool,
    fallback_count: usize,
) -> usize {
    prodex_mojo_core::rich::plan_application_candidate(hard_continuation, fallback_count, 0, true)
        .expect("Mojo application candidate plan returned invalid output")
        .candidate_count
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_local_rewrite_candidate_count(
    hard_continuation: bool,
    fallback_count: usize,
) -> usize {
    if hard_continuation {
        1
    } else {
        fallback_count + 1
    }
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_local_rewrite_candidate_attempt(
    hard_continuation: bool,
    fallback_count: usize,
    attempt_index: usize,
    primary_available: bool,
) -> RuntimeLocalRewriteCandidateAttempt {
    use prodex_mojo_core::rich::ApplicationCandidateAttempt as MojoAttempt;
    match prodex_mojo_core::rich::plan_application_candidate(
        hard_continuation,
        fallback_count,
        attempt_index,
        primary_available,
    )
    .expect("Mojo application candidate plan returned invalid output")
    .attempt
    {
        MojoAttempt::Stop => RuntimeLocalRewriteCandidateAttempt::Stop,
        MojoAttempt::Skip => RuntimeLocalRewriteCandidateAttempt::Skip,
        MojoAttempt::Primary => RuntimeLocalRewriteCandidateAttempt::Primary,
        MojoAttempt::Fallback => RuntimeLocalRewriteCandidateAttempt::Fallback,
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_local_rewrite_candidate_attempt(
    hard_continuation: bool,
    fallback_count: usize,
    attempt_index: usize,
    primary_available: bool,
) -> RuntimeLocalRewriteCandidateAttempt {
    let candidate_count = runtime_local_rewrite_candidate_count(hard_continuation, fallback_count);
    if attempt_index >= candidate_count {
        RuntimeLocalRewriteCandidateAttempt::Stop
    } else if attempt_index == 0 && !primary_available {
        RuntimeLocalRewriteCandidateAttempt::Skip
    } else if attempt_index == 0 {
        RuntimeLocalRewriteCandidateAttempt::Primary
    } else {
        RuntimeLocalRewriteCandidateAttempt::Fallback
    }
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_local_rewrite_attempt_result(
    transport_succeeded: bool,
    retryable_response: bool,
    retry_allowed: bool,
) -> RuntimeLocalRewriteAttemptResult {
    use prodex_mojo_core::rich::ApplicationAttemptResult as MojoResult;
    match prodex_mojo_core::rich::plan_application_attempt_result(
        transport_succeeded,
        retryable_response,
        retry_allowed,
    )
    .expect("Mojo application attempt plan returned invalid output")
    {
        MojoResult::Success => RuntimeLocalRewriteAttemptResult::Success,
        MojoResult::Retry => RuntimeLocalRewriteAttemptResult::Retry,
        MojoResult::Stop => RuntimeLocalRewriteAttemptResult::Stop,
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_local_rewrite_attempt_result(
    transport_succeeded: bool,
    retryable_response: bool,
    retry_allowed: bool,
) -> RuntimeLocalRewriteAttemptResult {
    if retry_allowed && (!transport_succeeded || retryable_response) {
        RuntimeLocalRewriteAttemptResult::Retry
    } else if transport_succeeded {
        RuntimeLocalRewriteAttemptResult::Success
    } else {
        RuntimeLocalRewriteAttemptResult::Stop
    }
}

#[cfg(feature = "mojo-core")]
pub(super) fn runtime_local_rewrite_binding_decision(
    continuation_bound: bool,
    bound_identity_present: bool,
    provider_matches: bool,
    selected_identity_present: bool,
    identity_matches: bool,
    selected_provider: ProviderId,
) -> RuntimeLocalRewriteBindingDecision {
    use prodex_mojo_core::rich::ApplicationBindingDecision as MojoDecision;
    let selected_identity_optional =
        matches!(selected_provider, ProviderId::OpenAi | ProviderId::Copilot);
    match prodex_mojo_core::rich::plan_application_binding(
        prodex_mojo_core::rich::ApplicationBindingInput {
            continuation_bound,
            bound_identity_present,
            provider_matches,
            selected_identity_present,
            identity_matches,
            selected_identity_optional,
        },
    )
    .expect("Mojo application binding plan returned invalid output")
    {
        MojoDecision::Valid => RuntimeLocalRewriteBindingDecision::Valid,
        MojoDecision::MissingIdentity => RuntimeLocalRewriteBindingDecision::MissingIdentity,
        MojoDecision::ProviderMismatch => RuntimeLocalRewriteBindingDecision::ProviderMismatch,
        MojoDecision::IdentityMismatch => RuntimeLocalRewriteBindingDecision::IdentityMismatch,
        MojoDecision::SelectedIdentityRequired => {
            RuntimeLocalRewriteBindingDecision::SelectedIdentityRequired
        }
    }
}

#[cfg(not(feature = "mojo-core"))]
pub(super) fn runtime_local_rewrite_binding_decision(
    continuation_bound: bool,
    bound_identity_present: bool,
    provider_matches: bool,
    selected_identity_present: bool,
    identity_matches: bool,
    selected_provider: ProviderId,
) -> RuntimeLocalRewriteBindingDecision {
    if !continuation_bound {
        RuntimeLocalRewriteBindingDecision::Valid
    } else if !bound_identity_present {
        RuntimeLocalRewriteBindingDecision::MissingIdentity
    } else if !provider_matches {
        RuntimeLocalRewriteBindingDecision::ProviderMismatch
    } else if selected_identity_present && !identity_matches {
        RuntimeLocalRewriteBindingDecision::IdentityMismatch
    } else if !selected_identity_present
        && !matches!(selected_provider, ProviderId::OpenAi | ProviderId::Copilot)
    {
        RuntimeLocalRewriteBindingDecision::SelectedIdentityRequired
    } else {
        RuntimeLocalRewriteBindingDecision::Valid
    }
}
