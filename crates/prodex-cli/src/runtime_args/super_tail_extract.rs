use std::ffi::OsString;

use super::{
    SuperArgs, SuperCliAgent, parse_runtime_base_url, parse_super_external_provider,
    parse_super_local_url,
};

struct ScannedValue<'a> {
    value: Option<&'a str>,
    consumed_count: usize,
}

enum SuperOverride {
    Provider(super::SuperExternalProvider),
    Harness(prodex_provider_core::HarnessMode),
    ApiKey(String),
    LocalModel(String),
    Profile(String),
    AutoRotate(bool),
    AutoRedeem,
    SkipQuotaCheck,
    DryRun,
    NoProxy,
    Presidio(bool),
    BaseUrl(String),
    Url(String),
    LocalContextWindow(usize),
    LocalAutoCompactTokenLimit(usize),
    Cli(SuperCliAgent),
}

enum ScanOutcome {
    Apply {
        value: SuperOverride,
        consumed_count: usize,
    },
    Forward {
        consumed_count: usize,
    },
    Unknown,
}

pub(super) fn extract_provider_overrides_from_codex_args(
    args: &mut SuperArgs,
) -> Result<(), String> {
    let codex_args = std::mem::take(&mut args.codex_args);
    let mut remaining = Vec::with_capacity(codex_args.len());
    let mut index = 0;
    while index < codex_args.len() {
        match scan_override(&codex_args, index) {
            Ok(ScanOutcome::Apply {
                value,
                consumed_count,
            }) => {
                apply_override(args, value);
                index += consumed_count;
            }
            Ok(ScanOutcome::Forward { consumed_count }) => {
                remaining.extend(codex_args[index..index + consumed_count].iter().cloned());
                index += consumed_count;
            }
            Ok(ScanOutcome::Unknown) => {
                remaining.push(codex_args[index].clone());
                index += 1;
            }
            Err(err) => {
                remaining.extend(codex_args[index..].iter().cloned());
                args.codex_args = remaining;
                return Err(err);
            }
        }
    }
    args.codex_args = remaining;
    Ok(())
}

fn scan_override(args: &[OsString], index: usize) -> Result<ScanOutcome, String> {
    let Some(argument) = args[index].to_str() else {
        return Ok(ScanOutcome::Unknown);
    };
    if let Some(scanned) = scan_value(args, index, &["--provider"]) {
        return Ok(parse_optional(
            scanned,
            parse_super_external_provider,
            SuperOverride::Provider,
        ));
    }
    if let Some(scanned) = scan_value(args, index, &["--harness"]) {
        let value = scanned
            .value
            .ok_or_else(|| "--harness requires auto, native, or minimal".to_string())?
            .parse()
            .map_err(|err: prodex_provider_core::ParseHarnessModeError| err.to_string())?;
        return Ok(apply(scanned.consumed_count, SuperOverride::Harness(value)));
    }
    if let Some(scanned) = scan_value(args, index, &["--api-key"]) {
        return Ok(parse_string(scanned, SuperOverride::ApiKey));
    }
    if let Some(scanned) = scan_value(args, index, &["--model", "--local-model"]) {
        return Ok(parse_string(scanned, SuperOverride::LocalModel));
    }
    if let Some(scanned) = scan_value(args, index, &["--profile"]) {
        return Ok(parse_string(scanned, SuperOverride::Profile));
    }
    let boolean = match argument {
        "--no-auto-rotate" => Some(SuperOverride::AutoRotate(false)),
        "--auto-rotate" => Some(SuperOverride::AutoRotate(true)),
        "--auto-redeem" => Some(SuperOverride::AutoRedeem),
        "--skip-quota-check" => Some(SuperOverride::SkipQuotaCheck),
        "--dry-run" => Some(SuperOverride::DryRun),
        "--no-proxy" => Some(SuperOverride::NoProxy),
        "--presidio" => Some(SuperOverride::Presidio(true)),
        "--no-presidio" => Some(SuperOverride::Presidio(false)),
        _ => None,
    };
    if let Some(value) = boolean {
        return Ok(apply(1, value));
    }
    if let Some(scanned) = scan_value(args, index, &["--base-url"]) {
        return scanned
            .value
            .map_or(Ok(forward(scanned.consumed_count)), |value| {
                parse_runtime_base_url(value)
                    .map(SuperOverride::BaseUrl)
                    .map(|value| apply(scanned.consumed_count, value))
            });
    }
    if let Some(scanned) = scan_value(args, index, &["--url"]) {
        return scanned
            .value
            .map_or(Ok(forward(scanned.consumed_count)), |value| {
                parse_super_local_url(value)
                    .map(SuperOverride::Url)
                    .map(|value| apply(scanned.consumed_count, value))
            });
    }
    if let Some(scanned) = scan_value(args, index, &["--context-window", "--local-context-window"])
    {
        return Ok(parse_optional(
            scanned,
            str::parse::<usize>,
            SuperOverride::LocalContextWindow,
        ));
    }
    if let Some(scanned) = scan_value(
        args,
        index,
        &[
            "--auto-compact-token-limit",
            "--local-auto-compact-token-limit",
        ],
    ) {
        return Ok(parse_optional(
            scanned,
            str::parse::<usize>,
            SuperOverride::LocalAutoCompactTokenLimit,
        ));
    }
    if let Some(scanned) = scan_value(args, index, &["--cli"]) {
        return Ok(parse_optional(
            scanned,
            |value| parse_super_cli_agent(value).ok_or(()),
            SuperOverride::Cli,
        ));
    }
    Ok(ScanOutcome::Unknown)
}

fn scan_value<'a>(args: &'a [OsString], index: usize, names: &[&str]) -> Option<ScannedValue<'a>> {
    let argument = args[index].to_str()?;
    if names.contains(&argument) {
        let value = args.get(index + 1).and_then(|value| value.to_str());
        return Some(ScannedValue {
            value,
            consumed_count: usize::from(value.is_some()) + 1,
        });
    }
    let (name, value) = argument.split_once('=')?;
    names.contains(&name).then_some(ScannedValue {
        value: Some(value),
        consumed_count: 1,
    })
}

fn parse_optional<T, E>(
    scanned: ScannedValue<'_>,
    parse: impl FnOnce(&str) -> Result<T, E>,
    wrap: impl FnOnce(T) -> SuperOverride,
) -> ScanOutcome {
    scanned
        .value
        .and_then(|value| parse(value).ok())
        .map_or_else(
            || forward(scanned.consumed_count),
            |value| apply(scanned.consumed_count, wrap(value)),
        )
}

fn parse_string(
    scanned: ScannedValue<'_>,
    wrap: impl FnOnce(String) -> SuperOverride,
) -> ScanOutcome {
    scanned.value.map_or_else(
        || forward(scanned.consumed_count),
        |value| apply(scanned.consumed_count, wrap(value.to_string())),
    )
}

fn apply(consumed_count: usize, value: SuperOverride) -> ScanOutcome {
    ScanOutcome::Apply {
        value,
        consumed_count,
    }
}

fn forward(consumed_count: usize) -> ScanOutcome {
    ScanOutcome::Forward { consumed_count }
}

fn apply_override(args: &mut SuperArgs, value: SuperOverride) {
    match value {
        SuperOverride::Provider(value) => args.provider = Some(value),
        SuperOverride::Harness(value) => args.harness = Some(value),
        SuperOverride::ApiKey(value) => args.api_key = Some(value),
        SuperOverride::LocalModel(value) => args.local_model = Some(value),
        SuperOverride::Profile(value) if args.profile.is_none() => args.profile = Some(value),
        SuperOverride::Profile(_) => {}
        SuperOverride::AutoRotate(true) => {
            args.auto_rotate = true;
            args.no_auto_rotate = false;
        }
        SuperOverride::AutoRotate(false) => {
            args.no_auto_rotate = true;
            args.auto_rotate = false;
        }
        SuperOverride::AutoRedeem => args.auto_redeem = true,
        SuperOverride::SkipQuotaCheck => args.skip_quota_check = true,
        SuperOverride::DryRun => args.dry_run = true,
        SuperOverride::NoProxy => args.no_proxy = true,
        SuperOverride::Presidio(true) => {
            args.presidio = true;
            args.no_presidio = false;
        }
        SuperOverride::Presidio(false) => {
            args.no_presidio = true;
            args.presidio = false;
        }
        SuperOverride::BaseUrl(value) => args.base_url = Some(value),
        SuperOverride::Url(value) => args.url = Some(value),
        SuperOverride::LocalContextWindow(value) => args.local_context_window = Some(value),
        SuperOverride::LocalAutoCompactTokenLimit(value) => {
            args.local_auto_compact_token_limit = Some(value);
        }
        SuperOverride::Cli(value) => args.cli = Some(value),
    }
}

fn parse_super_cli_agent(value: &str) -> Option<SuperCliAgent> {
    match value {
        "codex" => Some(SuperCliAgent::Codex),
        "gemini" => Some(SuperCliAgent::Gemini),
        "kiro" => Some(SuperCliAgent::Kiro),
        "agy" => Some(SuperCliAgent::Agy),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_scanner_reports_consumption_without_mutating_arguments() {
        let args = vec![
            OsString::from("--model"),
            OsString::from("split"),
            OsString::from("--local-model=equals"),
        ];
        let before = args.clone();

        let split = scan_value(&args, 0, &["--model", "--local-model"]).unwrap();
        assert_eq!(split.value, Some("split"));
        assert_eq!(split.consumed_count, 2);
        let equals = scan_value(&args, 2, &["--model", "--local-model"]).unwrap();
        assert_eq!(equals.value, Some("equals"));
        assert_eq!(equals.consumed_count, 1);
        assert_eq!(args, before);
    }
}
