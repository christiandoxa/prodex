use std::path::Path;

use super::ContextBlobNoiseFinding;
use super::base64::detect_base64ish_blob_noise_supplement;
use super::binary::detect_binaryish_text_noise_supplement;
use super::lock_vendor::detect_lockfile_or_vendor_noise_supplement;
use super::minified::detect_minified_js_json_noise_supplement;
use super::paths::detect_repeated_path_flood_noise_supplement;

pub(super) fn add_context_blob_noise_supplemental_findings(
    path: Option<&Path>,
    input: &str,
    lines: &[&str],
    findings: &mut Vec<ContextBlobNoiseFinding>,
) {
    if let Some(finding) = detect_binaryish_text_noise_supplement(input) {
        super::push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_base64ish_blob_noise_supplement(lines) {
        super::push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_minified_js_json_noise_supplement(input, lines) {
        super::push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_lockfile_or_vendor_noise_supplement(path, input, lines) {
        super::push_context_blob_noise_finding(findings, finding);
    }
    if let Some(finding) = detect_repeated_path_flood_noise_supplement(input, lines) {
        super::push_context_blob_noise_finding(findings, finding);
    }
}
