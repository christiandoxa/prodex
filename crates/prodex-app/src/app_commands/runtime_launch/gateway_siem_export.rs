use super::gateway_secret_config::GatewaySecretResolver;
use anyhow::{Context, Result, bail};
use aws_lc_rs::hmac;
use prodex_domain::SecretPurpose;
use prodex_storage::{
    BoundedGovernanceAuditExporter, GovernanceAuditExporter, GovernanceRepositoryError,
    SiemExportBatch, SiemExportEvent, SiemExportTransport, SiemExporterCapabilities,
    SiemOutboxRetryPolicy,
};
use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderValue};
use sha2::{Digest, Sha256};
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

pub(crate) struct RuntimeSiemWorkerConfig {
    exporter: Mutex<BoundedGovernanceAuditExporter<RuntimeSiemHttpsTransport>>,
    retry_policy: SiemOutboxRetryPolicy,
    batch_limit: u16,
    maximum_lag_ms: u64,
}

impl fmt::Debug for RuntimeSiemWorkerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeSiemWorkerConfig")
            .field("exporter", &"<redacted>")
            .field("retry_policy", &self.retry_policy)
            .field("batch_limit", &self.batch_limit)
            .field("maximum_lag_ms", &self.maximum_lag_ms)
            .finish()
    }
}

impl RuntimeSiemWorkerConfig {
    pub(crate) fn from_policy(
        policy: &prodex_runtime_policy::RuntimePolicyGatewayObservabilitySettings,
        resolver: &GatewaySecretResolver,
    ) -> Result<Option<Self>> {
        let Some((capabilities, transport)) =
            RuntimeSiemHttpsTransport::from_policy(policy, resolver)?
        else {
            return Ok(None);
        };
        let retry_policy = SiemOutboxRetryPolicy::bounded(
            policy.siem_max_attempts.unwrap_or(5),
            policy.siem_retry_base_ms.unwrap_or(1_000),
            policy.siem_retry_max_ms.unwrap_or(30_000),
        )
        .map_err(|_| anyhow::anyhow!("SIEM retry policy is invalid"))?;
        let batch_limit = capabilities.max_batch_events;
        let maximum_lag_ms = policy.siem_max_lag_ms.unwrap_or(60_000);
        Ok(Some(Self {
            exporter: Mutex::new(BoundedGovernanceAuditExporter::new(capabilities, transport)),
            retry_policy,
            batch_limit,
            maximum_lag_ms,
        }))
    }

    pub(crate) fn run_once(
        &self,
        repository: &prodex_storage_sqlite_runtime::GovernanceSqliteRepository,
        now_unix_ms: u64,
    ) -> Result<
        prodex_storage_sqlite_runtime::SiemOutboxWorkerReport,
        prodex_storage_sqlite_runtime::GovernanceRepositoryError,
    > {
        let mut exporter = self
            .exporter
            .lock()
            .map_err(|_| prodex_storage_sqlite_runtime::GovernanceRepositoryError::Database)?;
        repository.run_siem_outbox_exporter_batch(
            now_unix_ms,
            self.batch_limit,
            self.retry_policy,
            &mut *exporter,
        )
    }

    pub(crate) fn run_once_postgres(
        &self,
        repository: &prodex_storage_postgres_runtime::PostgresRepository,
        runtime: &tokio::runtime::Handle,
        tenant_ids: &[prodex_domain::TenantId],
        now_unix_ms: u64,
    ) -> Result<(), GovernanceRepositoryError> {
        const CLAIM_LEASE_MS: u64 = 60_000;

        let mut exporter = self
            .exporter
            .lock()
            .map_err(|_| GovernanceRepositoryError::Database)?;
        let started_at = Instant::now();
        for tenant_id in tenant_ids {
            for _ in 0..self.batch_limit {
                let claim_now = now_unix_ms.saturating_add(
                    started_at
                        .elapsed()
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
                let Some(claim) = runtime
                    .block_on(repository.governance_claim_siem_outbox_batch(
                        *tenant_id,
                        claim_now,
                        1,
                        CLAIM_LEASE_MS,
                    ))?
                    .into_iter()
                    .next()
                else {
                    break;
                };
                let delivered = SiemExportBatch::bounded(
                    vec![SiemExportEvent {
                        event_id: claim.event_id,
                        event_envelope: claim.event_envelope.clone(),
                    }],
                    exporter.capabilities(),
                )
                .ok()
                .and_then(|batch| exporter.export_batch(&batch).ok())
                .is_some_and(|receipt| receipt.accepted_events == 1);
                let finalize_now = now_unix_ms.saturating_add(
                    started_at
                        .elapsed()
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX),
                );
                runtime.block_on(repository.governance_finalize_siem_outbox_claim(
                    &claim,
                    delivered,
                    finalize_now,
                    self.retry_policy,
                ))?;
            }
        }
        Ok(())
    }

    pub(crate) fn plan_health(
        &self,
        health: prodex_storage_sqlite_runtime::GovernanceOutboxHealth,
        now_unix_ms: u64,
    ) -> Result<
        prodex_observability::SiemOutboxHealthMetricPlan,
        prodex_domain::TelemetryAttributeError,
    > {
        prodex_observability::plan_siem_outbox_health_metric(
            health.pending,
            health.dead_lettered,
            health.oldest_pending_at_unix_ms,
            now_unix_ms,
            self.maximum_lag_ms,
        )
    }
}

pub(crate) struct RuntimeSiemHttpsTransport {
    endpoint: reqwest::Url,
    client: Client,
    bearer_token: Zeroizing<String>,
    signing_key: Option<Zeroizing<Vec<u8>>>,
}

impl fmt::Debug for RuntimeSiemHttpsTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeSiemHttpsTransport")
            .field("endpoint", &"<redacted>")
            .field("bearer_token", &"<redacted>")
            .field(
                "signing_key",
                &self.signing_key.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl RuntimeSiemHttpsTransport {
    pub(crate) fn from_policy(
        policy: &prodex_runtime_policy::RuntimePolicyGatewayObservabilitySettings,
        resolver: &GatewaySecretResolver,
    ) -> Result<Option<(SiemExporterCapabilities, Self)>> {
        let Some(endpoint) = policy.siem_endpoint.as_deref() else {
            return Ok(None);
        };
        let endpoint = strict_https_endpoint(endpoint)?;
        let credential_ref = policy
            .siem_bearer_token_ref
            .clone()
            .context("SIEM HTTPS transport requires a bearer SecretRef")?;
        let bearer_token = resolve_required(
            resolver,
            "gateway.observability.siem_bearer_token_ref",
            &credential_ref,
        )?;
        let mtls_identity_ref = policy.siem_mtls_identity_ref.clone();
        let identity = mtls_identity_ref
            .as_ref()
            .map(|reference| {
                resolve_required(
                    resolver,
                    "gateway.observability.siem_mtls_identity_ref",
                    reference,
                )
                .and_then(|pem| {
                    reqwest::Identity::from_pem(pem.as_bytes())
                        .context("SIEM mTLS identity must be a PEM certificate and private key")
                })
            })
            .transpose()?;
        let signing_key_ref = policy.siem_signing_key_ref.clone();
        let signing_key = signing_key_ref
            .as_ref()
            .map(|reference| {
                resolve_required(
                    resolver,
                    "gateway.observability.siem_signing_key_ref",
                    reference,
                )
                .map(|value| Zeroizing::new(value.as_bytes().to_vec()))
            })
            .transpose()?;
        let mut builder = Client::builder()
            .https_only(true)
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15));
        if let Some(identity) = identity {
            builder = builder.identity(identity);
        }
        let client = builder
            .build()
            .context("failed to build SIEM HTTPS client")?;
        let capabilities = SiemExporterCapabilities::bounded(
            credential_ref,
            mtls_identity_ref,
            signing_key_ref,
            policy.siem_max_batch_events.unwrap_or(64),
            policy.siem_max_batch_bytes.unwrap_or(256 * 1024),
        )
        .map_err(|_| anyhow::anyhow!("SIEM exporter capability bounds are invalid"))?;
        Ok(Some((
            capabilities,
            Self {
                endpoint,
                client,
                bearer_token,
                signing_key,
            },
        )))
    }
}

impl SiemExportTransport for RuntimeSiemHttpsTransport {
    type Error = RuntimeSiemTransportError;

    fn send(
        &mut self,
        _capabilities: &SiemExporterCapabilities,
        batch: &SiemExportBatch,
    ) -> Result<(), Self::Error> {
        let body = serde_json::to_vec(&serde_json::json!({
            "events": batch.events().iter().map(|event| serde_json::json!({
                "event_id": event.event_id.to_string(),
                "event": event.event_envelope,
            })).collect::<Vec<_>>()
        }))
        .map_err(|_| RuntimeSiemTransportError::Encode)?;
        let mut request = self
            .client
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "application/json")
            .header(AUTHORIZATION, bearer_header(self.bearer_token.as_str())?)
            .header("idempotency-key", idempotency_key(batch)?);
        if let Some(signing_key) = self.signing_key.as_deref() {
            request = request.header("x-prodex-signature", hmac_signature(signing_key, &body)?);
        }
        let response = request
            .body(body)
            .send()
            .map_err(|_| RuntimeSiemTransportError::Transport)?;
        if !response.status().is_success() {
            return Err(RuntimeSiemTransportError::Rejected);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimeSiemTransportError {
    Encode,
    Header,
    Transport,
    Rejected,
}

fn strict_https_endpoint(value: &str) -> Result<reqwest::Url> {
    if value.len() > 2_048 {
        bail!("SIEM endpoint exceeds the safe URL length");
    }
    let endpoint = reqwest::Url::parse(value).context("SIEM endpoint is invalid")?;
    if endpoint.scheme() != "https"
        || endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
        || endpoint.path().len() > 1_024
    {
        bail!("SIEM transport requires an HTTPS endpoint without credentials, query, or fragment");
    }
    Ok(endpoint)
}

fn resolve_required(
    resolver: &GatewaySecretResolver,
    context: &str,
    reference: &prodex_domain::SecretRef,
) -> Result<Zeroizing<String>> {
    resolver
        .resolve_static(
            context,
            Some(reference),
            None,
            None,
            SecretPurpose::TelemetryExportCredential,
        )?
        .context("required projected SIEM secret is unavailable")
        .map(Zeroizing::new)
}

fn bearer_header(token: &str) -> Result<HeaderValue, RuntimeSiemTransportError> {
    HeaderValue::from_str(&format!("Bearer {token}")).map_err(|_| RuntimeSiemTransportError::Header)
}

fn idempotency_key(batch: &SiemExportBatch) -> Result<HeaderValue, RuntimeSiemTransportError> {
    let mut digest = Sha256::new();
    for event in batch.events() {
        digest.update(event.event_id.to_string().as_bytes());
        digest.update([0]);
    }
    HeaderValue::from_str(&hex(&digest.finalize())).map_err(|_| RuntimeSiemTransportError::Header)
}

fn hmac_signature(
    signing_key: &[u8],
    body: &[u8],
) -> Result<HeaderValue, RuntimeSiemTransportError> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, signing_key);
    let value = format!("hmac-sha256={}", hex(hmac::sign(&key, body).as_ref()));
    HeaderValue::from_str(&value).map_err(|_| RuntimeSiemTransportError::Header)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transport_requires_strict_https_and_uses_library_hmac() {
        assert!(strict_https_endpoint("https://siem.example.com/events").is_ok());
        for invalid in [
            "http://siem.example.com/events",
            "https://user@siem.example.com/events",
            "https://siem.example.com/events?tenant=secret",
        ] {
            assert!(strict_https_endpoint(invalid).is_err());
        }
        let signature = hmac_signature(b"synthetic-key", b"synthetic-body").unwrap();
        let rendered = signature.to_str().unwrap();
        assert!(rendered.starts_with("hmac-sha256="));
        assert!(!rendered.contains("synthetic-key"));
        assert!(!rendered.contains("synthetic-body"));

        let transport = RuntimeSiemHttpsTransport {
            endpoint: strict_https_endpoint("https://siem.example.com/events").unwrap(),
            client: Client::builder().build().unwrap(),
            bearer_token: Zeroizing::new("synthetic-bearer".to_string()),
            signing_key: Some(Zeroizing::new(b"synthetic-signing-key".to_vec())),
        };
        let debug = format!("{transport:?}");
        assert!(!debug.contains("synthetic-bearer"));
        assert!(!debug.contains("synthetic-signing-key"));
    }
}
