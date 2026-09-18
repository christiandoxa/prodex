use serde::Serialize;
use std::path::Path;

use crate::{command_lines, count_text_lines, normalize_command_output};

#[cfg(not(feature = "mojo"))]
mod base64;
mod basic;
#[cfg(not(feature = "mojo"))]
mod binary;
mod lock_vendor;
#[cfg(not(feature = "mojo"))]
mod minified;
mod paths;
#[cfg(not(feature = "mojo"))]
mod supplement;

use self::basic::{is_lockfile_or_vendor_path, repeated_path_flood};
#[cfg(not(feature = "mojo"))]
use self::basic::{looks_like_base64_blob, looks_like_minified_js_json};
#[cfg(feature = "mojo")]
use self::lock_vendor::context_lockfile_or_vendor_path_finding;
pub(crate) use self::paths::context_noise_strip_path_location_suffix_supplement;
#[cfg(not(feature = "mojo"))]
use self::supplement::add_context_blob_noise_supplemental_findings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextBlobNoiseKind {
    Base64Blob,
    MinifiedJsJson,
    LockfileOrVendor,
    BinaryText,
    RepeatedPathFlood,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseFinding {
    pub kind: ContextBlobNoiseKind,
    pub line: Option<usize>,
    pub bytes: usize,
    pub score: usize,
    pub detail: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ContextBlobNoiseReport {
    pub bytes: usize,
    pub lines: usize,
    pub findings: Vec<ContextBlobNoiseFinding>,
}

impl ContextBlobNoiseReport {
    pub fn is_noise(&self) -> bool {
        !self.findings.is_empty()
    }

    pub fn has_kind(&self, kind: ContextBlobNoiseKind) -> bool {
        self.findings.iter().any(|finding| finding.kind == kind)
    }
}

pub fn detect_context_blob_noise(input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(None, input)
}

pub fn detect_context_blob_noise_for_path(path: &Path, input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner(Some(path), input)
}

pub fn is_context_blob_noise(input: &str) -> bool {
    detect_context_blob_noise(input).is_noise()
}

#[cfg(feature = "mojo")]
fn detect_context_blob_noise_inner(path: Option<&Path>, input: &str) -> ContextBlobNoiseReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let analysis = prodex_mojo_core::context::analyze_blob_noise(input, &normalized, lines.len())
        .expect("Mojo context blob-noise analysis returned invalid output");
    let mut findings = Vec::new();

    if path.is_some_and(is_lockfile_or_vendor_path) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: input.len().min(usize::MAX / 2),
            detail: "path looks like generated dependency/vendor content".to_string(),
        });
    }
    if analysis.primary_binary {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::BinaryText,
            line: None,
            bytes: input.len(),
            score: input.len(),
            detail: "text contains binary replacement or NUL characters".to_string(),
        });
    }
    for (index, (line, flags)) in lines.iter().zip(&analysis.line_flags).enumerate() {
        let trimmed = line.trim();
        if flags & prodex_mojo_core::context::CONTEXT_BLOB_PRIMARY_BASE64 != 0 {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::Base64Blob,
                line: Some(index + 1),
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long high-entropy base64-like line".to_string(),
            });
        }
        if flags & prodex_mojo_core::context::CONTEXT_BLOB_PRIMARY_MINIFIED != 0 {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::MinifiedJsJson,
                line: Some(index + 1),
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long minified JSON/JavaScript-like line".to_string(),
            });
        }
    }
    if let Some((line, count)) = repeated_path_flood(&lines) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::RepeatedPathFlood,
            line: Some(line),
            bytes: input.len(),
            score: count,
            detail: format!("{count} path-like lines detected"),
        });
    }

    if let Some(binary) = analysis.binary {
        push_context_blob_noise_finding(
            &mut findings,
            ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::BinaryText,
                line: Some(binary.first_line),
                bytes: input.len(),
                score: binary.score,
                detail: format!(
                    "binaryish_controls={}, nul_chars={}, replacement_chars={}",
                    binary.suspicious, binary.nul, binary.replacement
                ),
            },
        );
    }
    if let Some(base64) = analysis.base64 {
        push_context_blob_noise_finding(
            &mut findings,
            ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::Base64Blob,
                line: Some(base64.line),
                bytes: base64.bytes,
                score: base64.score,
                detail: format!("base64ish_bytes={}", base64.bytes),
            },
        );
    }
    if let Some(minified) = analysis.minified {
        let detail = if minified.kind == 1 {
            "minified_json"
        } else {
            "minified_javascript"
        };
        push_context_blob_noise_finding(
            &mut findings,
            ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::MinifiedJsJson,
                line: Some(1),
                bytes: input.trim().len(),
                score: minified.score,
                detail: format!("{detail}, max_line_bytes={}", minified.max_line_bytes),
            },
        );
    }
    if let Some(path_finding) = context_lockfile_or_vendor_path_finding(path, input.len()) {
        push_context_blob_noise_finding(&mut findings, path_finding);
    }
    if let Some(lock) = analysis.lock {
        let detail = match lock.kind {
            1 => format!("cargo_lock_packages={}", lock.value),
            2 => "npm_lockfile_json".to_string(),
            3 => "package_manager_lockfile".to_string(),
            4 => format!("vendor_path_lines={}", lock.value),
            _ => unreachable!("validated by Mojo adapter"),
        };
        push_context_blob_noise_finding(
            &mut findings,
            ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::LockfileOrVendor,
                line: Some(lock.line),
                bytes: input.len(),
                score: lock.score,
                detail,
            },
        );
    }
    if let Some(finding) = paths::detect_repeated_path_flood_noise_supplement(input, &lines) {
        push_context_blob_noise_finding(&mut findings, finding);
    }

    ContextBlobNoiseReport {
        bytes: input.len(),
        lines: count_text_lines(&normalized),
        findings,
    }
}

#[cfg(not(feature = "mojo"))]
fn detect_context_blob_noise_inner(path: Option<&Path>, input: &str) -> ContextBlobNoiseReport {
    detect_context_blob_noise_inner_rust(path, input)
}

#[cfg(not(feature = "mojo"))]
fn detect_context_blob_noise_inner_rust(
    path: Option<&Path>,
    input: &str,
) -> ContextBlobNoiseReport {
    let normalized = normalize_command_output(input);
    let lines = command_lines(&normalized);
    let mut findings = Vec::new();

    if path.is_some_and(is_lockfile_or_vendor_path) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::LockfileOrVendor,
            line: None,
            bytes: input.len(),
            score: input.len().min(usize::MAX / 2),
            detail: "path looks like generated dependency/vendor content".to_string(),
        });
    }
    if input.chars().any(|ch| ch == '\0' || ch == '\u{fffd}') {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::BinaryText,
            line: None,
            bytes: input.len(),
            score: input.len(),
            detail: "text contains binary replacement or NUL characters".to_string(),
        });
    }
    for (index, line) in lines.iter().enumerate() {
        let line_number = Some(index + 1);
        let trimmed = line.trim();
        if trimmed.len() >= 512 && looks_like_base64_blob(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::Base64Blob,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long high-entropy base64-like line".to_string(),
            });
        }
        if trimmed.len() >= 800 && looks_like_minified_js_json(trimmed) {
            findings.push(ContextBlobNoiseFinding {
                kind: ContextBlobNoiseKind::MinifiedJsJson,
                line: line_number,
                bytes: trimmed.len(),
                score: trimmed.len(),
                detail: "long minified JSON/JavaScript-like line".to_string(),
            });
        }
    }
    if let Some((line, count)) = repeated_path_flood(&lines) {
        findings.push(ContextBlobNoiseFinding {
            kind: ContextBlobNoiseKind::RepeatedPathFlood,
            line: Some(line),
            bytes: input.len(),
            score: count,
            detail: format!("{count} path-like lines detected"),
        });
    }
    add_context_blob_noise_supplemental_findings(path, input, &lines, &mut findings);
    ContextBlobNoiseReport {
        bytes: input.len(),
        lines: count_text_lines(&normalized),
        findings,
    }
}

pub(super) fn push_context_blob_noise_finding(
    findings: &mut Vec<ContextBlobNoiseFinding>,
    finding: ContextBlobNoiseFinding,
) {
    if !findings
        .iter()
        .any(|existing| existing.kind == finding.kind)
    {
        findings.push(finding);
    }
}
