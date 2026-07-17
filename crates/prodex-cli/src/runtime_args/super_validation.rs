use super::{SuperArgs, SuperCliAgent};

pub(super) fn validate_super_mode_compatibility(args: &SuperArgs) -> Result<(), String> {
    if args.harness.is_some() && args.provider.is_none() && args.url.is_none() {
        return Err("--harness requires --provider or --url".to_string());
    }
    if args.harness.is_some() && args.cli.is_some_and(|agent| agent != SuperCliAgent::Codex) {
        return Err("--harness is only supported with the Codex CLI bridge".to_string());
    }
    if args.presidio && matches!(args.cli, Some(SuperCliAgent::Kiro | SuperCliAgent::Agy)) {
        return Err("--presidio is unsupported for native Kiro or Antigravity".to_string());
    }
    Ok(())
}
