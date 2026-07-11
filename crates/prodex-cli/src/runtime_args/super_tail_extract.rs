use super::{SuperArgs, SuperCliAgent, parse_super_external_provider, parse_super_local_url};

pub(super) fn extract_provider_overrides_from_codex_args(args: &mut SuperArgs) {
    let mut i = 0;
    while i < args.codex_args.len() {
        let Some(arg) = args.codex_args[i].to_str() else {
            i += 1;
            continue;
        };
        let (consumed, skip) = match arg {
            "--provider" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => match parse_super_external_provider(val) {
                    Ok(provider) => {
                        args.provider = Some(provider);
                        (2, true)
                    }
                    Err(_) => (2, false),
                },
                None => (1, false),
            },
            a if a.starts_with("--provider=") => {
                if let Some(val) = a.strip_prefix("--provider=")
                    && let Ok(provider) = parse_super_external_provider(val)
                {
                    args.provider = Some(provider);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            "--api-key" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => {
                    args.api_key = Some(val.to_string());
                    (2, true)
                }
                None => (1, false),
            },
            a if a.starts_with("--api-key=") => {
                args.api_key = a.strip_prefix("--api-key=").map(|v| v.to_string());
                (1, true)
            }
            "--model" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => {
                    args.local_model = Some(val.to_string());
                    (2, true)
                }
                None => (1, false),
            },
            a if a.starts_with("--model=") => {
                args.local_model = a.strip_prefix("--model=").map(|v| v.to_string());
                (1, true)
            }
            "--local-model" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => {
                    args.local_model = Some(val.to_string());
                    (2, true)
                }
                None => (1, false),
            },
            a if a.starts_with("--local-model=") => {
                args.local_model = a.strip_prefix("--local-model=").map(|v| v.to_string());
                (1, true)
            }
            "--profile" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => {
                    if args.profile.is_none() {
                        args.profile = Some(val.to_string());
                    }
                    (2, true)
                }
                None => (1, false),
            },
            a if a.starts_with("--profile=") => {
                if args.profile.is_none() {
                    args.profile = a.strip_prefix("--profile=").map(|v| v.to_string());
                }
                (1, true)
            }
            "--no-auto-rotate" => {
                args.no_auto_rotate = true;
                args.auto_rotate = false;
                (1, true)
            }
            "--auto-rotate" => {
                args.auto_rotate = true;
                args.no_auto_rotate = false;
                (1, true)
            }
            "--auto-redeem" => {
                args.auto_redeem = true;
                (1, true)
            }
            "--skip-quota-check" => {
                args.skip_quota_check = true;
                (1, true)
            }
            "--dry-run" => {
                args.dry_run = true;
                (1, true)
            }
            "--no-proxy" => {
                args.no_proxy = true;
                (1, true)
            }
            "--presidio" => {
                args.presidio = true;
                args.no_presidio = false;
                (1, true)
            }
            "--no-presidio" => {
                args.no_presidio = true;
                args.presidio = false;
                (1, true)
            }
            "--base-url" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => {
                    args.base_url = Some(val.to_string());
                    (2, true)
                }
                None => (1, false),
            },
            a if a.starts_with("--base-url=") => {
                args.base_url = a.strip_prefix("--base-url=").map(|v| v.to_string());
                (1, true)
            }
            "--url" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => match parse_super_local_url(val) {
                    Ok(url) => {
                        args.url = Some(url);
                        (2, true)
                    }
                    Err(_) => (2, false),
                },
                None => (1, false),
            },
            a if a.starts_with("--url=") => {
                if let Some(val) = a.strip_prefix("--url=")
                    && let Ok(url) = parse_super_local_url(val)
                {
                    args.url = Some(url);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            "--context-window" | "--local-context-window" => {
                match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                    Some(val) => match val.parse::<usize>() {
                        Ok(tokens) => {
                            args.local_context_window = Some(tokens);
                            (2, true)
                        }
                        Err(_) => (2, false),
                    },
                    None => (1, false),
                }
            }
            a if a.starts_with("--context-window=") => {
                if let Some(tokens) = a
                    .strip_prefix("--context-window=")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    args.local_context_window = Some(tokens);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            a if a.starts_with("--local-context-window=") => {
                if let Some(tokens) = a
                    .strip_prefix("--local-context-window=")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    args.local_context_window = Some(tokens);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            "--auto-compact-token-limit" | "--local-auto-compact-token-limit" => {
                match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                    Some(val) => match val.parse::<usize>() {
                        Ok(tokens) => {
                            args.local_auto_compact_token_limit = Some(tokens);
                            (2, true)
                        }
                        Err(_) => (2, false),
                    },
                    None => (1, false),
                }
            }
            a if a.starts_with("--auto-compact-token-limit=") => {
                if let Some(tokens) = a
                    .strip_prefix("--auto-compact-token-limit=")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    args.local_auto_compact_token_limit = Some(tokens);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            a if a.starts_with("--local-auto-compact-token-limit=") => {
                if let Some(tokens) = a
                    .strip_prefix("--local-auto-compact-token-limit=")
                    .and_then(|v| v.parse::<usize>().ok())
                {
                    args.local_auto_compact_token_limit = Some(tokens);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            "--cli" => match args.codex_args.get(i + 1).and_then(|v| v.to_str()) {
                Some(val) => match parse_super_cli_agent(val) {
                    Some(agent) => {
                        args.cli = Some(agent);
                        (2, true)
                    }
                    None => (2, false),
                },
                None => (1, false),
            },
            a if a.starts_with("--cli=") => {
                if let Some(val) = a.strip_prefix("--cli=")
                    && let Some(agent) = parse_super_cli_agent(val)
                {
                    args.cli = Some(agent);
                    (1, true)
                } else {
                    (1, false)
                }
            }
            _ => {
                i += 1;
                continue;
            }
        };
        if skip {
            let drain_end = (i + consumed).min(args.codex_args.len());
            args.codex_args.drain(i..drain_end);
        } else {
            i += consumed;
        }
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
