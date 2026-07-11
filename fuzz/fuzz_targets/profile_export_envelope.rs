#![no_main]

use libfuzzer_sys::fuzz_target;
use prodex_profile_export::{ProfileExportPayload, parse_profile_export_envelope};

const FUZZ_INPUT_MAX_BYTES: usize = 1024 * 1024;

fuzz_target!(|input: &[u8]| {
    if input.len() > FUZZ_INPUT_MAX_BYTES {
        return;
    }
    let _ = parse_profile_export_envelope::<ProfileExportPayload<serde_json::Value>>(input);
});
