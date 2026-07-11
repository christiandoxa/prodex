#![no_main]

use libfuzzer_sys::fuzz_target;
use prodex_runtime_policy::{RuntimePolicyFile, validate_runtime_policy_file};
use std::path::Path;

const MAX_POLICY_INPUT_BYTES: usize = 1024 * 1024;

fuzz_target!(|input: &[u8]| {
    if input.len() > MAX_POLICY_INPUT_BYTES {
        return;
    }
    let Ok(text) = std::str::from_utf8(input) else {
        return;
    };
    let Ok(policy) = toml::from_str::<RuntimePolicyFile>(text) else {
        return;
    };
    let _ = validate_runtime_policy_file(&policy, Path::new("policy.toml"));
});
