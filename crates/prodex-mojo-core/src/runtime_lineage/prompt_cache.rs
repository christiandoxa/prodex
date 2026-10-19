//! Prompt-cache keys and owner transitions; map access remains in the host.
use crate::MojoError;

#[derive(Clone, Copy, Debug)]
pub enum PromptCacheEvent {
    Remember,
    Hit(u64),
}

impl PromptCacheEvent {
    fn abi(self) -> (i64, u64) {
        match self {
            Self::Remember => (0, 0),
            Self::Hit(tokens) => (1, tokens),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum PromptCacheKey<'a> {
    NoKey,
    NoCachedTokens,
    Value(&'a str),
}

#[derive(Clone, Copy, Debug)]
pub struct PromptCacheBindingFacts {
    pub same_owner: bool,
    pub bound_at: i64,
    pub cached_input_tokens: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptCacheOwnerUpdate {
    Inserted,
    Changed,
    Unchanged,
    Preserved,
}

#[derive(Debug, PartialEq, Eq)]
pub struct PromptCacheUpdatePlan {
    pub owner: PromptCacheOwnerUpdate,
    pub bound_at: i64,
    pub cached_input_tokens: u64,
}

unsafe extern "C" {
    fn prodex_prompt_cache_key_v1(
        version: i64,
        hit: i64,
        tokens: u64,
        key_address: u64,
        key_length: i64,
        output_address: u64,
        capacity: i64,
    ) -> i64;
    fn prodex_prompt_cache_update_v1(
        version: i64,
        hit: i64,
        present: i64,
        same_owner: i64,
        previous_bound_at: i64,
        previous_tokens: u64,
        tokens: u64,
        now: i64,
        output_address: u64,
        capacity: i64,
    ) -> i64;
}

fn check_status(status: i64) -> Result<(), MojoError> {
    match status {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Normalizes a key after applying zero-token observation precedence.
pub fn prepare_prompt_cache_key(
    key: Option<&str>,
    event: PromptCacheEvent,
) -> Result<PromptCacheKey<'_>, MojoError> {
    let key = key.unwrap_or_default();
    let length = i64::try_from(key.len()).map_err(|_| MojoError::InvalidInput)?;
    let (hit, tokens) = event.abi();
    let mut output = [u64::MAX; 3];
    check_status(unsafe {
        prodex_prompt_cache_key_v1(
            1,
            hit,
            tokens,
            key.as_ptr() as u64,
            length,
            output.as_mut_ptr() as u64,
            3,
        )
    })?;
    match output {
        [0, 0, 0] if !matches!(event, PromptCacheEvent::Hit(0)) => Ok(PromptCacheKey::NoKey),
        [1, 0, 0] if matches!(event, PromptCacheEvent::Hit(0)) => {
            Ok(PromptCacheKey::NoCachedTokens)
        }
        [2, start, end] if !matches!(event, PromptCacheEvent::Hit(0)) => {
            let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
            key.get(start..end)
                .filter(|value| !value.is_empty())
                .map(PromptCacheKey::Value)
                .ok_or(MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans an update from non-secret owner-equality, timestamp and token facts.
pub fn plan_prompt_cache_update(
    event: PromptCacheEvent,
    existing: Option<PromptCacheBindingFacts>,
    now: i64,
) -> Result<PromptCacheUpdatePlan, MojoError> {
    let (hit, tokens) = event.abi();
    let mut output = [u64::MAX; 3];
    check_status(unsafe {
        prodex_prompt_cache_update_v1(
            1,
            hit,
            i64::from(existing.is_some()),
            i64::from(existing.is_some_and(|binding| binding.same_owner)),
            existing.map_or(0, |binding| binding.bound_at),
            existing.map_or(0, |binding| binding.cached_input_tokens),
            tokens,
            now,
            output.as_mut_ptr() as u64,
            3,
        )
    })?;
    let owner = match output[0] {
        0 if existing.is_none() => PromptCacheOwnerUpdate::Inserted,
        1 if existing.is_some() => PromptCacheOwnerUpdate::Changed,
        2 if existing.is_some() => PromptCacheOwnerUpdate::Unchanged,
        3 if existing.is_some() => PromptCacheOwnerUpdate::Preserved,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(PromptCacheUpdatePlan {
        owner,
        bound_at: output[1] as i64,
        cached_input_tokens: output[2],
    })
}

#[cfg(test)]
#[path = "prompt_cache_tests.rs"]
mod tests;
