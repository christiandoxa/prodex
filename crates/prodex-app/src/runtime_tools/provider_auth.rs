use super::ChildProcessPlan;
use anyhow::{Context, Result};
use std::ffi::OsString;

pub(crate) const PRODEX_PROVIDER_CODEX_API_KEY: &str = "prodex-runtime-provider";

pub(crate) fn force_codex_api_key_auth_for_provider_runtime(child: &mut ChildProcessPlan) {
    let key = OsString::from("OPENAI_API_KEY");
    if let Some((_, value)) = child.extra_env.iter_mut().find(|(name, _)| name == &key) {
        *value = OsString::from(PRODEX_PROVIDER_CODEX_API_KEY);
    } else {
        child
            .extra_env
            .push((key, OsString::from(PRODEX_PROVIDER_CODEX_API_KEY)));
    }
}

pub(crate) fn write_provider_runtime_codex_auth(codex_home: &std::path::Path) -> Result<()> {
    prodex_shared_codex_fs::create_codex_home_if_missing(codex_home)?;
    let auth_path = codex_home.join("auth.json");
    let auth_json = serde_json::json!({
        "auth_mode": "apikey",
        "OPENAI_API_KEY": PRODEX_PROVIDER_CODEX_API_KEY,
        "tokens": null,
        "last_refresh": null,
        "agent_identity": null
    });
    let text = serde_json::to_string_pretty(&auth_json)?;
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(&auth_path), text)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", auth_path.display()))?;
    Ok(())
}
