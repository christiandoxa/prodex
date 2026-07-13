use super::json_body::{
    MAX_PRESIDIO_JSON_TEXT_BYTES, PresidioJsonString, collect_json_content,
    replace_json_string_values,
};
use anyhow::{Context, Result};
use prodex_domain::{
    ContentLocation, DetectorId, FindingKind, InspectionCoverage, InspectionFinding,
    MAX_INSPECTION_FINDINGS, TenantId,
};
use prodex_runtime_policy::RuntimePolicyInspectionPattern;
use std::collections::{BTreeMap, BTreeSet};

const LOCAL_DETECTOR_ID: &str = "local-bounded-v1";
const MAX_TENANT_PATTERN_MATCH_BYTES: usize = 4 * 1024;
const MAX_TENANT_PATTERNS: usize = 64;
const MAX_TENANT_PATTERNS_PER_TENANT: usize = 16;
const MAX_TENANT_PATTERN_BYTES: usize = 256;
const MAX_TENANT_PATTERN_ID_BYTES: usize = 64;
const MAX_TENANT_PATTERN_WILDCARDS: usize = 8;

#[derive(Clone, Default)]
pub(crate) struct RuntimeTenantDetectorPatterns {
    by_tenant: BTreeMap<TenantId, Vec<RuntimeTenantDetectorPattern>>,
}

#[derive(Clone)]
struct RuntimeTenantDetectorPattern {
    segments: Vec<String>,
}

impl RuntimeTenantDetectorPatterns {
    pub(crate) fn compile(entries: &[RuntimePolicyInspectionPattern]) -> Result<Self> {
        if entries.len() > MAX_TENANT_PATTERNS {
            anyhow::bail!("tenant detector pattern count exceeded safe limit");
        }
        let mut by_tenant = BTreeMap::<TenantId, Vec<RuntimeTenantDetectorPattern>>::new();
        let mut identities = BTreeSet::new();
        for entry in entries {
            let wildcard_count = entry.pattern.bytes().filter(|byte| *byte == b'*').count();
            if entry.id.is_empty()
                || entry.id.len() > MAX_TENANT_PATTERN_ID_BYTES
                || entry.pattern.is_empty()
                || entry.pattern.len() > MAX_TENANT_PATTERN_BYTES
                || wildcard_count > MAX_TENANT_PATTERN_WILDCARDS
                || entry.pattern.chars().any(char::is_control)
                || entry.pattern.starts_with('*')
                || entry.pattern.ends_with('*')
                || entry.pattern.split('*').any(str::is_empty)
                || !identities.insert((entry.tenant_id, entry.id.as_str()))
            {
                anyhow::bail!("tenant detector pattern is invalid");
            }
            let tenant_patterns = by_tenant.entry(entry.tenant_id).or_default();
            if tenant_patterns.len() >= MAX_TENANT_PATTERNS_PER_TENANT {
                anyhow::bail!("tenant detector pattern count exceeded per-tenant safe limit");
            }
            tenant_patterns.push(RuntimeTenantDetectorPattern {
                segments: entry.pattern.split('*').map(str::to_owned).collect(),
            });
        }
        Ok(Self { by_tenant })
    }

    fn for_tenant(&self, tenant_id: Option<TenantId>) -> &[RuntimeTenantDetectorPattern] {
        tenant_id
            .and_then(|tenant_id| self.by_tenant.get(&tenant_id))
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    pub(crate) fn has_for_tenant(&self, tenant_id: Option<TenantId>) -> bool {
        !self.for_tenant(tenant_id).is_empty()
    }
}

pub(crate) struct RuntimeLocalInspection {
    pub(crate) body: Vec<u8>,
    pub(crate) coverage: InspectionCoverage,
    pub(crate) findings: Vec<InspectionFinding>,
    pub(crate) changed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LocalMatch {
    start: usize,
    end: usize,
    kind: FindingKind,
}

pub(crate) fn runtime_local_inspect_and_mask(body: Vec<u8>) -> Result<RuntimeLocalInspection> {
    runtime_local_inspect_and_mask_for_tenant(body, &RuntimeTenantDetectorPatterns::default(), None)
}

pub(crate) fn runtime_local_inspect_and_mask_for_tenant(
    body: Vec<u8>,
    patterns: &RuntimeTenantDetectorPatterns,
    tenant_id: Option<TenantId>,
) -> Result<RuntimeLocalInspection> {
    let text = String::from_utf8(body).context("request body is not UTF-8")?;
    if let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&text) {
        let content = collect_json_content(&json)?;
        let mut findings = Vec::new();
        let mut masked_values = Vec::with_capacity(content.values.len());
        for value in &content.values {
            let (masked, value_findings) = inspect_and_mask_value(value, patterns, tenant_id)?;
            findings.extend(value_findings);
            if findings.len() > MAX_INSPECTION_FINDINGS {
                anyhow::bail!("local inspection finding count exceeded safe limit");
            }
            masked_values.push(masked);
        }
        if findings.is_empty() {
            return Ok(RuntimeLocalInspection {
                body: text.into_bytes(),
                coverage: content.coverage,
                findings,
                changed: false,
            });
        }
        let mut masked_values = masked_values.iter().map(String::as_str);
        replace_json_string_values(&mut json, false, &mut masked_values);
        return Ok(RuntimeLocalInspection {
            body: serde_json::to_vec(&json).context("failed to serialize masked JSON request")?,
            coverage: content.coverage,
            findings,
            changed: true,
        });
    }

    let value = PresidioJsonString {
        path: "$".to_string(),
        text,
        sensitive_kind: None,
    };
    if value.text.len() > MAX_PRESIDIO_JSON_TEXT_BYTES {
        anyhow::bail!("request content exceeds inspection limits");
    }
    let (text, findings) = inspect_and_mask_value(&value, patterns, tenant_id)?;
    Ok(RuntimeLocalInspection {
        body: text.into_bytes(),
        coverage: InspectionCoverage::Full,
        changed: !findings.is_empty(),
        findings,
    })
}

fn inspect_and_mask_value(
    value: &PresidioJsonString,
    patterns: &RuntimeTenantDetectorPatterns,
    tenant_id: Option<TenantId>,
) -> Result<(String, Vec<InspectionFinding>)> {
    let matches = local_matches(
        &value.text,
        value.sensitive_kind,
        patterns.for_tenant(tenant_id),
    )?;
    let detector_id = DetectorId::new(LOCAL_DETECTOR_ID)?;
    let findings = matches
        .iter()
        .map(|finding| {
            InspectionFinding::new(
                finding.kind,
                ContentLocation::new(&value.path, finding.start, finding.end)?,
                10_000,
                detector_id.clone(),
            )
            .map_err(anyhow::Error::from)
        })
        .collect::<Result<Vec<_>>>()?;

    let mut masked = String::with_capacity(value.text.len());
    let mut cursor = 0;
    for finding in matches {
        masked.push_str(&value.text[cursor..finding.start]);
        masked.push_str(local_placeholder(finding.kind));
        cursor = finding.end;
    }
    masked.push_str(&value.text[cursor..]);
    Ok((masked, findings))
}

fn local_matches(
    text: &str,
    sensitive_kind: Option<FindingKind>,
    tenant_patterns: &[RuntimeTenantDetectorPattern],
) -> Result<Vec<LocalMatch>> {
    if let Some(kind) = sensitive_kind.filter(|_| !text.is_empty()) {
        return Ok(vec![LocalMatch {
            start: 0,
            end: text.len(),
            kind,
        }]);
    }

    let mut matches = Vec::new();
    detect_private_keys(text, &mut matches);
    detect_labeled_credentials(text, &mut matches);
    detect_bearer_tokens(text, &mut matches);
    detect_prefixed_api_keys(text, &mut matches);
    detect_emails(text, &mut matches);
    detect_financial_identifiers(text, &mut matches);
    detect_tenant_patterns(text, tenant_patterns, &mut matches)?;
    matches.sort_by_key(|finding| (finding.start, usize::MAX - finding.end, finding.kind));

    let mut bounded = Vec::new();
    let mut covered_until = 0;
    for finding in matches {
        if finding.start < covered_until || finding.start >= finding.end {
            continue;
        }
        covered_until = finding.end;
        bounded.push(finding);
        if bounded.len() > MAX_INSPECTION_FINDINGS {
            anyhow::bail!("local inspection finding count exceeded safe limit");
        }
    }
    Ok(bounded)
}

fn detect_private_keys(text: &str, matches: &mut Vec<LocalMatch>) {
    let mut offset = 0;
    while let Some(relative) = text[offset..].find("-----BEGIN ") {
        let start = offset + relative;
        let Some(header_end_relative) = text[start..].find("PRIVATE KEY-----") else {
            break;
        };
        let body_start = start + header_end_relative + "PRIVATE KEY-----".len();
        let end = text[body_start..]
            .find("PRIVATE KEY-----")
            .map(|relative| body_start + relative + "PRIVATE KEY-----".len())
            .unwrap_or(text.len());
        matches.push(LocalMatch {
            start,
            end,
            kind: FindingKind::PrivateKey,
        });
        offset = end;
    }
}

fn detect_tenant_patterns(
    text: &str,
    patterns: &[RuntimeTenantDetectorPattern],
    matches: &mut Vec<LocalMatch>,
) -> Result<()> {
    for pattern in patterns {
        let mut search_from = 0;
        while search_from < text.len() {
            let Some(relative_start) = text[search_from..].find(&pattern.segments[0]) else {
                break;
            };
            let start = search_from + relative_start;
            let mut end = start + pattern.segments[0].len();
            let mut matched = true;
            for segment in &pattern.segments[1..] {
                let Some(relative_end) = text[end..].find(segment) else {
                    matched = false;
                    break;
                };
                end += relative_end + segment.len();
                if end - start > MAX_TENANT_PATTERN_MATCH_BYTES {
                    matched = false;
                    break;
                }
            }
            if matched {
                matches.push(LocalMatch {
                    start,
                    end,
                    kind: FindingKind::TenantSensitive,
                });
                if matches.len() > MAX_INSPECTION_FINDINGS {
                    anyhow::bail!("local inspection finding count exceeded safe limit");
                }
                search_from = end;
            } else {
                search_from = start + text[start..].chars().next().map_or(1, char::len_utf8);
            }
        }
    }
    Ok(())
}

fn detect_labeled_credentials(text: &str, matches: &mut Vec<LocalMatch>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_alphabetic() && !matches!(bytes[index], b'_' | b'-') {
            index += text[index..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let key_start = index;
        index += 1;
        while index < bytes.len()
            && (bytes[index].is_ascii_alphanumeric() || matches!(bytes[index], b'_' | b'-'))
        {
            index += 1;
        }
        let key = &text[key_start..index];
        let mut cursor = skip_ascii_whitespace(bytes, index);
        if cursor >= bytes.len()
            || !matches!(bytes[cursor], b':' | b'=')
            || !redaction::redaction_key_looks_sensitive(key)
        {
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor + 1);
        let (start, end) = delimited_value_range(bytes, cursor);
        if start < end {
            matches.push(LocalMatch {
                start,
                end,
                kind: sensitive_key_kind(key),
            });
        }
        index = end.max(index);
    }
}

fn detect_bearer_tokens(text: &str, matches: &mut Vec<LocalMatch>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index + 6 <= bytes.len() {
        if bytes[index..index + 6].eq_ignore_ascii_case(b"bearer")
            && (index == 0 || !bytes[index - 1].is_ascii_alphanumeric())
        {
            let start = skip_ascii_whitespace(bytes, index + 6);
            if start > index + 6 {
                let end = secret_token_end(bytes, start);
                if end > start {
                    matches.push(LocalMatch {
                        start,
                        end,
                        kind: FindingKind::AccessToken,
                    });
                    index = end;
                    continue;
                }
            }
        }
        index += 1;
    }
}

fn detect_prefixed_api_keys(text: &str, matches: &mut Vec<LocalMatch>) {
    const PREFIXES: &[&str] = &[
        "sk-proj-", "sk-ant-", "sk-live-", "sk_test_", "sk_live_", "sk-", "sk_",
    ];
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let Some(prefix) = PREFIXES.iter().find(|prefix| {
            bytes
                .get(index..index + prefix.len())
                .is_some_and(|value| value.eq_ignore_ascii_case(prefix.as_bytes()))
        }) else {
            index += 1;
            continue;
        };
        let end = secret_token_end(bytes, index);
        if end >= index + prefix.len() + 8 {
            matches.push(LocalMatch {
                start: index,
                end,
                kind: FindingKind::ApiKey,
            });
            index = end;
        } else {
            index += 1;
        }
    }
}

fn detect_emails(text: &str, matches: &mut Vec<LocalMatch>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !email_byte(bytes[index]) {
            index += text[index..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let start = index;
        while index < bytes.len() && email_byte(bytes[index]) {
            index += 1;
        }
        let token = &text[start..index];
        if let Some((local, domain)) = token.split_once('@')
            && !local.is_empty()
            && domain.contains('.')
            && domain.split('.').all(|part| !part.is_empty())
        {
            matches.push(LocalMatch {
                start,
                end: index,
                kind: FindingKind::EmailAddress,
            });
        }
    }
}

fn detect_financial_identifiers(text: &str, matches: &mut Vec<LocalMatch>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() {
            index += text[index..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let start = index;
        let mut digits = 0;
        while index < bytes.len()
            && (bytes[index].is_ascii_digit() || matches!(bytes[index], b' ' | b'-'))
        {
            digits += usize::from(bytes[index].is_ascii_digit());
            index += 1;
        }
        while index > start && matches!(bytes[index - 1], b' ' | b'-') {
            index -= 1;
        }
        if (13..=19).contains(&digits) {
            matches.push(LocalMatch {
                start,
                end: index,
                kind: FindingKind::FinancialAccount,
            });
        }
        index = index.max(start + 1);
    }
}

fn sensitive_key_kind(key: &str) -> FindingKind {
    let normalized = key
        .bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let normalized = std::str::from_utf8(&normalized).unwrap_or_default();
    if normalized.contains("privatekey") {
        FindingKind::PrivateKey
    } else if normalized.contains("apikey") {
        FindingKind::ApiKey
    } else if normalized.contains("token") || normalized == "authorization" {
        FindingKind::AccessToken
    } else {
        FindingKind::Password
    }
}

fn delimited_value_range(bytes: &[u8], start: usize) -> (usize, usize) {
    let Some(first) = bytes.get(start).copied() else {
        return (start, start);
    };
    if matches!(first, b'\'' | b'"') {
        let value_start = start + 1;
        let mut end = value_start;
        while end < bytes.len() && bytes[end] != first {
            end += 1;
        }
        return (value_start, end);
    }
    (start, secret_token_end(bytes, start))
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index].is_ascii_whitespace() {
        index += 1;
    }
    index
}

fn secret_token_end(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len()
        && !bytes[index].is_ascii_whitespace()
        && !matches!(
            bytes[index],
            b'"' | b'\'' | b',' | b'}' | b']' | b')' | b';' | b'&'
        )
    {
        index += 1;
    }
    index
}

fn email_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'%' | b'+' | b'-' | b'@')
}

const fn local_placeholder(_kind: FindingKind) -> &'static str {
    "<redacted>"
}

#[cfg(test)]
mod tests {
    use super::*;
    use prodex_runtime_policy::RuntimePolicyInspectionPattern;
    use uuid::Uuid;

    fn tenant(value: u128) -> TenantId {
        TenantId::from_uuid(Uuid::from_u128(value))
    }

    fn pattern(tenant_id: TenantId, id: &str, pattern: &str) -> RuntimePolicyInspectionPattern {
        RuntimePolicyInspectionPattern {
            tenant_id,
            id: id.to_string(),
            pattern: pattern.to_string(),
        }
    }

    #[test]
    fn local_inspection_masks_supported_nested_content_and_preserves_structure() {
        let private_key = concat!(
            "-----BEGIN PRIVATE ",
            "KEY-----abc-----END PRIVATE KEY-----"
        );
        let body = serde_json::to_vec(&serde_json::json!({
            "model": "gpt-5",
            "input": ["héllo user@example.com", "Bearer access-token-value"],
            "tools": [{"arguments": {
                "api_key": "tenant-api-key-value",
                "private_key": private_key,
                "note": "sk-proj-1234567890 card 4111-1111-1111-1111"
            }}]
        }))
        .unwrap();

        let inspected = runtime_local_inspect_and_mask(body).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&inspected.body).unwrap();
        let rendered = value.to_string();

        assert_eq!(value["model"], "gpt-5");
        assert!(rendered.contains("héllo <redacted>"));
        assert!(rendered.matches("<redacted>").count() >= 5);
        assert!(!rendered.contains("user@example.com"));
        assert_eq!(inspected.coverage, InspectionCoverage::Full);
        assert!(
            inspected
                .findings
                .iter()
                .any(|finding| finding.kind() == FindingKind::EmailAddress)
        );
    }

    #[test]
    fn local_inspection_rejects_deep_and_match_flood_inputs() {
        let mut deep = serde_json::json!("value");
        for _ in 0..=super::super::json_body::MAX_PRESIDIO_JSON_DEPTH {
            deep = serde_json::json!({"input": deep});
        }
        assert!(runtime_local_inspect_and_mask(serde_json::to_vec(&deep).unwrap()).is_err());

        let flood = (0..=MAX_INSPECTION_FINDINGS)
            .map(|index| format!("user{index}@example.com"))
            .collect::<Vec<_>>()
            .join(" ");
        assert!(runtime_local_inspect_and_mask(flood.into_bytes()).is_err());
    }

    #[test]
    fn malformed_private_key_is_masked_through_end_of_value() {
        let secret = concat!(
            "-----BEGIN PRIVATE ",
            "KEY-----malformed-secret-without-footer"
        );
        let body = serde_json::to_vec(&serde_json::json!({"input": secret})).unwrap();

        let inspected = runtime_local_inspect_and_mask(body).unwrap();
        let rendered = String::from_utf8(inspected.body).unwrap();

        assert!(!rendered.contains("malformed-secret-without-footer"));
        assert!(rendered.contains("<redacted>"));
    }

    #[test]
    fn tenant_patterns_are_isolated_and_support_unicode_interior_globs() {
        let first = tenant(1);
        let second = tenant(2);
        let patterns = RuntimeTenantDetectorPatterns::compile(&[
            pattern(first, "customer-code", "顧客-*秘密"),
            pattern(second, "other-code", "other-secret"),
        ])
        .unwrap();
        let body = "prefix 顧客-東京秘密 suffix other-secret"
            .as_bytes()
            .to_vec();

        let inspected =
            runtime_local_inspect_and_mask_for_tenant(body, &patterns, Some(first)).unwrap();
        let rendered = String::from_utf8(inspected.body).unwrap();

        assert_eq!(rendered, "prefix <redacted> suffix other-secret");
        assert_eq!(inspected.findings.len(), 1);
        assert_eq!(inspected.findings[0].kind(), FindingKind::TenantSensitive);
    }

    #[test]
    fn tenant_pattern_compilation_and_matches_are_bounded() {
        let tenant_id = tenant(1);
        assert!(
            RuntimeTenantDetectorPatterns::compile(&[pattern(tenant_id, "unbounded", "*secret")])
                .is_err()
        );
        let too_many = (0..=MAX_TENANT_PATTERNS_PER_TENANT)
            .map(|index| {
                pattern(
                    tenant_id,
                    &format!("id-{index}"),
                    &format!("secret-{index}"),
                )
            })
            .collect::<Vec<_>>();
        assert!(RuntimeTenantDetectorPatterns::compile(&too_many).is_err());

        let patterns =
            RuntimeTenantDetectorPatterns::compile(&[pattern(tenant_id, "flood", "tenant-secret")])
                .unwrap();
        let flood = std::iter::repeat_n("tenant-secret", MAX_INSPECTION_FINDINGS + 1)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(
            runtime_local_inspect_and_mask_for_tenant(
                flood.into_bytes(),
                &patterns,
                Some(tenant_id)
            )
            .is_err()
        );
    }
}
