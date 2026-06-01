use super::code_assist::{
    GeminiCodeAssistSetupMode, fetch_gemini_code_assist_plan, gemini_code_assist_endpoint,
    gemini_validation_error_message, gemini_validation_from_body,
    resolve_gemini_code_assist_project_with_endpoint,
};
use super::{
    GeminiOAuthSecret, force_refresh_gemini_oauth_secret, gemini_oauth_project_from_env,
    normalize_gemini_project_id, refresh_gemini_oauth_secret_if_needed, write_gemini_oauth_secret,
};
use anyhow::{Context, Result, bail};
use prodex_quota::GeminiQuotaInfo;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

pub(crate) fn fetch_gemini_quota(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json(codex_home, provider_project_id)?;
    gemini_quota_info_from_value(codex_home, value)
}

pub(crate) fn fetch_gemini_quota_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<GeminiQuotaInfo> {
    let value = fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        code_assist_endpoint,
    )?;
    gemini_quota_info_from_value(codex_home, value)
}

fn gemini_quota_info_from_value(codex_home: &Path, value: Value) -> Result<GeminiQuotaInfo> {
    serde_json::from_value(value).with_context(|| {
        format!(
            "invalid JSON returned by Gemini quota backend for {}",
            codex_home.display()
        )
    })
}

pub(crate) fn fetch_gemini_quota_json(
    codex_home: &Path,
    provider_project_id: Option<&str>,
) -> Result<Value> {
    let code_assist_endpoint = gemini_code_assist_endpoint();
    fetch_gemini_quota_json_with_code_assist_endpoint(
        codex_home,
        provider_project_id,
        &code_assist_endpoint,
    )
}

fn fetch_gemini_quota_json_with_code_assist_endpoint(
    codex_home: &Path,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<Value> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Gemini quota HTTP client")?;
    let mut secret = refresh_gemini_oauth_secret_if_needed(codex_home)?;
    let mut project_id = resolve_gemini_quota_project_id(
        &client,
        codex_home,
        &mut secret,
        provider_project_id,
        code_assist_endpoint,
    )?;
    let mut value =
        match retrieve_gemini_user_quota(&client, &secret, &project_id, code_assist_endpoint) {
            Ok(value) => value,
            Err(err) if gemini_error_is_http_401(&err) => {
                secret = force_refresh_gemini_oauth_secret(codex_home)
                    .context("Gemini quota auth failed and OAuth token refresh failed")?;
                project_id = resolve_gemini_quota_project_id(
                    &client,
                    codex_home,
                    &mut secret,
                    provider_project_id,
                    code_assist_endpoint,
                )?;
                retrieve_gemini_user_quota(&client, &secret, &project_id, code_assist_endpoint)?
            }
            Err(err) => return Err(err),
        };
    let plan = fetch_gemini_code_assist_plan(&client, &secret, &project_id, code_assist_endpoint)
        .ok()
        .flatten();
    if let Some(object) = value.as_object_mut() {
        object.insert("email".to_string(), Value::String(secret.email.clone()));
        if let Some(plan) = plan {
            object.insert("plan".to_string(), Value::String(plan));
        }
        object.insert("project_id".to_string(), Value::String(project_id));
    }
    Ok(value)
}

fn resolve_gemini_quota_project_id(
    client: &Client,
    codex_home: &Path,
    secret: &mut GeminiOAuthSecret,
    provider_project_id: Option<&str>,
    code_assist_endpoint: &str,
) -> Result<String> {
    if let Some(project_id) = normalize_gemini_project_id(provider_project_id) {
        return Ok(project_id);
    }
    if let Some(project_id) = normalize_gemini_project_id(secret.project_id.as_deref()) {
        return Ok(project_id);
    }
    if let Some(project_id) = gemini_oauth_project_from_env() {
        return Ok(project_id);
    }
    if let Some(project_id) = resolve_gemini_code_assist_project_with_endpoint(
        client,
        secret,
        code_assist_endpoint,
        GeminiCodeAssistSetupMode::NonInteractive,
    )? {
        secret.project_id = Some(project_id.clone());
        write_gemini_oauth_secret(codex_home, secret)?;
        return Ok(project_id);
    }
    bail!(
        "Gemini quota requires a Code Assist project; run `prodex login --with-google` again or set GOOGLE_CLOUD_PROJECT"
    )
}

fn retrieve_gemini_user_quota(
    client: &Client,
    secret: &GeminiOAuthSecret,
    project_id: &str,
    code_assist_endpoint: &str,
) -> Result<Value> {
    let response = client
        .post(format!("{code_assist_endpoint}:retrieveUserQuota"))
        .bearer_auth(&secret.access_token)
        .json(&json!({
            "project": project_id,
        }))
        .send()
        .context("failed to fetch Gemini quota")?;
    let status = response.status();
    let body = response
        .text()
        .context("failed to read Gemini quota response")?;
    if !status.is_success() {
        if let Some(validation) = gemini_validation_from_body(&body) {
            bail!("{}", gemini_validation_error_message(&validation));
        }
        bail!(
            "Gemini quota request failed (HTTP {}): {body}",
            status.as_u16()
        );
    }
    serde_json::from_str(&body).context("failed to parse Gemini quota response")
}

fn gemini_error_is_http_401(err: &anyhow::Error) -> bool {
    err.chain()
        .any(|cause| cause.to_string().contains("HTTP 401"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tiny_http::{Response as TinyResponse, Server as TinyServer};

    #[test]
    fn fetch_gemini_quota_uses_code_assist_retrieve_user_quota() {
        let server = TinyServer::http("127.0.0.1:0").expect("quota test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let _endpoint_guard =
            crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", endpoint.as_str());
        let root = temp_codex_home("quota");
        let secret = GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(super::super::now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: Some("gemini-project".to_string()),
        };
        write_gemini_oauth_secret(&root, &secret).expect("Gemini OAuth secret should write");

        let handle = thread::spawn(move || {
            let mut request = server.recv().expect("quota request should arrive");
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1internal:retrieveUserQuota");
            assert!(
                request.headers().iter().any(|header| {
                    header.field.equiv("authorization")
                        && header.value.as_str() == "Bearer token-123"
                }),
                "quota request should use Gemini OAuth bearer token"
            );
            let mut body = String::new();
            request
                .as_reader()
                .read_to_string(&mut body)
                .expect("quota request body should read");
            assert!(body.contains("\"project\":\"gemini-project\""));
            assert!(!body.contains("userAgent"));
            request
                .respond(TinyResponse::from_string(
                    r#"{"buckets":[{"modelId":"models/gemini-2.5-pro","remainingAmount":"50","remainingFraction":0.5,"resetTime":"2026-05-09T00:00:00Z"}]}"#,
                ))
                .expect("quota response should send");

            let mut plan = server.recv().expect("plan request should arrive");
            assert_eq!(plan.method().as_str(), "POST");
            assert_eq!(plan.url(), "/v1internal:loadCodeAssist");
            let mut plan_body = String::new();
            plan.as_reader()
                .read_to_string(&mut plan_body)
                .expect("plan request body should read");
            assert!(plan_body.contains("\"mode\":\"HEALTH_CHECK\""));
            assert!(plan_body.contains("\"cloudaicompanionProject\":\"gemini-project\""));
            plan.respond(TinyResponse::from_string(
                r#"{"paidTier":{"id":"g1-pro-tier","name":"Gemini Code Assist in Google One AI Pro"},"currentTier":{"id":"standard-tier"}}"#,
            ))
            .expect("plan response should send");
        });

        let quota = fetch_gemini_quota(&root, None).expect("Gemini quota should fetch");
        handle.join().expect("quota test server should finish");

        assert_eq!(quota.email.as_deref(), Some("gemini-user@example.com"));
        assert_eq!(quota.plan.as_deref(), Some("pro"));
        assert_eq!(quota.project_id.as_deref(), Some("gemini-project"));
        assert_eq!(quota.buckets.len(), 1);
        assert_eq!(
            quota.buckets[0].model_id.as_deref(),
            Some("models/gemini-2.5-pro")
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_gemini_quota_surfaces_validation_required_url() {
        let server = TinyServer::http("127.0.0.1:0").expect("quota test server should bind");
        let listen_addr = server.server_addr().to_ip().unwrap();
        let endpoint = format!("http://{listen_addr}/v1internal");
        let _endpoint_guard =
            crate::TestEnvVarGuard::set("PRODEX_GEMINI_CODE_ASSIST_ENDPOINT", endpoint.as_str());
        let root = temp_codex_home("quota-validation");
        let secret = test_gemini_secret(Some("gemini-project"));
        write_gemini_oauth_secret(&root, &secret).expect("Gemini OAuth secret should write");

        let handle = thread::spawn(move || {
            let request = server.recv().expect("quota request should arrive");
            assert_eq!(request.method().as_str(), "POST");
            assert_eq!(request.url(), "/v1internal:retrieveUserQuota");
            request
                .respond(
                    TinyResponse::from_string(
                        r#"{"error":{"code":403,"message":"Verify your account to continue.","status":"PERMISSION_DENIED","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"VALIDATION_REQUIRED","domain":"cloudcode-pa.googleapis.com","metadata":{"validation_error_message":"Verify your account to continue.","validation_url":"https://accounts.google.com/verify"}},{"@type":"type.googleapis.com/google.rpc.Help","links":[{"description":"Verify your account","url":"https://accounts.google.com/verify"},{"description":"Learn more","url":"https://support.google.com/accounts?p=al_alert"}]}]}}"#,
                    )
                    .with_status_code(403),
                )
                .expect("validation response should send");
        });

        let err = fetch_gemini_quota(&root, None).expect_err("quota should require validation");
        handle
            .join()
            .expect("quota validation test server should finish");
        let message = format!("{err:#}");
        assert!(message.contains("Verify your account"));
        assert!(message.contains("https://accounts.google.com/verify"));
        assert!(message.contains("https://support.google.com/accounts?p=al_alert"));

        let _ = std::fs::remove_dir_all(root);
    }

    fn test_gemini_secret(project_id: Option<&str>) -> GeminiOAuthSecret {
        GeminiOAuthSecret {
            auth_mode: "gemini_oauth".to_string(),
            access_token: "token-123".to_string(),
            refresh_token: Some("refresh-123".to_string()),
            token_type: Some("Bearer".to_string()),
            scope: None,
            expiry_date: Some(super::super::now_ms() + 3_600_000),
            email: "gemini-user@example.com".to_string(),
            project_id: project_id.map(str::to_string),
        }
    }

    fn temp_codex_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gemini-auth-{name}-{stamp}"))
    }
}
