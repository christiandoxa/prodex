use crate::runtime_launch::proxy_startup::local_rewrite::{
    RuntimeGatewayAdminRole, RuntimeGatewayAdminToken,
};
use crate::{AppPaths, RuntimeRotationProxy};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use reqwest::{IntoUrl, blocking::RequestBuilder};
use std::fs;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tiny_http::{Header as TinyHeader, Response as TinyResponse, Server as TinyServer};

pub(super) trait IdempotentClient {
    fn idempotent_post(&self, url: impl IntoUrl) -> RequestBuilder;
    fn idempotent_patch(&self, url: impl IntoUrl) -> RequestBuilder;
    fn idempotent_delete(&self, url: impl IntoUrl) -> RequestBuilder;
}

impl IdempotentClient for reqwest::blocking::Client {
    fn idempotent_post(&self, url: impl IntoUrl) -> RequestBuilder {
        idempotent(self.post(url))
    }

    fn idempotent_patch(&self, url: impl IntoUrl) -> RequestBuilder {
        idempotent(self.patch(url))
    }

    fn idempotent_delete(&self, url: impl IntoUrl) -> RequestBuilder {
        idempotent(self.delete(url))
    }
}

fn idempotent(request: RequestBuilder) -> RequestBuilder {
    static NEXT_KEY: AtomicUsize = AtomicUsize::new(1);
    request.header(
        "Idempotency-Key",
        format!("test-{}", NEXT_KEY.fetch_add(1, Ordering::Relaxed)),
    )
}

pub(super) fn wait_for_oidc_cache(proxy: &RuntimeRotationProxy, minimum_entries: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        let entries = proxy
            .gateway_side_effect_snapshot
            .as_ref()
            .map(|snapshot| snapshot().oidc_cache_entries)
            .unwrap_or_default();
        if entries >= minimum_entries {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "OIDC background cache did not reach {minimum_entries} entries"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(super) fn runtime_gateway_test_admin_token(token: &str) -> RuntimeGatewayAdminToken {
    RuntimeGatewayAdminToken {
        name: "test-admin".to_string(),
        token_hash: runtime_proxy_crate::LocalBridgeBearerTokenHash::from_token(token),
        role: RuntimeGatewayAdminRole::Admin,
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_key_prefixes: Vec::new(),
    }
}

pub(super) struct TestUpstream {
    pub(super) addr: SocketAddr,
    pub(super) body_rx: mpsc::Receiver<Vec<u8>>,
    pub(super) headers_rx: mpsc::Receiver<Vec<(String, String)>>,
    pub(super) path_rx: mpsc::Receiver<String>,
    _thread: thread::JoinHandle<()>,
}

impl TestUpstream {
    pub(super) fn start() -> Self {
        Self::start_n(1)
    }

    pub(super) fn start_with_response_body(response_body: &'static str) -> Self {
        Self::start_n_with_response(1, "application/json", response_body)
    }

    pub(super) fn start_n_with_response_body(
        request_count: usize,
        response_body: &'static str,
    ) -> Self {
        Self::start_n_with_response(request_count, "application/json", response_body)
    }

    pub(super) fn start_with_response(
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        Self::start_n_with_response(1, content_type, response_body)
    }

    pub(super) fn start_n(request_count: usize) -> Self {
        Self::start_n_with_response(
            request_count,
            "application/json",
            r#"{"id":"resp_test","usage":{"input_tokens":7,"output_tokens":11,"total_tokens":18}}"#,
        )
    }

    pub(super) fn start_with_statuses(
        statuses: &[u16],
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        Self::start_with_responses(statuses.to_vec(), content_type, response_body)
    }

    fn start_n_with_response(
        request_count: usize,
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        Self::start_with_responses(vec![200; request_count], content_type, response_body)
    }

    fn start_with_responses(
        statuses: Vec<u16>,
        content_type: &'static str,
        response_body: &'static str,
    ) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test upstream should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test upstream should expose TCP addr");
        let (body_tx, body_rx) = mpsc::channel();
        let (headers_tx, headers_rx) = mpsc::channel();
        let (path_tx, path_rx) = mpsc::channel();
        let thread = thread::spawn(move || {
            for status in statuses {
                let mut request = server.recv().expect("test upstream should receive request");
                let path = request.url().to_string();
                let headers = request
                    .headers()
                    .iter()
                    .map(|header| {
                        (
                            header.field.to_string().to_ascii_lowercase(),
                            header.value.as_str().to_string(),
                        )
                    })
                    .collect::<Vec<_>>();
                let mut body = Vec::new();
                request
                    .as_reader()
                    .read_to_end(&mut body)
                    .expect("test upstream should read request body");
                let _ = path_tx.send(path);
                let _ = headers_tx.send(headers);
                let _ = body_tx.send(body);
                let mut response =
                    TinyResponse::from_string(response_body).with_status_code(status);
                response.add_header(TinyHeader::from_bytes("content-type", content_type).unwrap());
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            body_rx,
            headers_rx,
            path_rx,
            _thread: thread,
        }
    }
}

pub(super) struct TestJwksServer {
    pub(super) addr: SocketAddr,
    request_count: Arc<AtomicUsize>,
    _thread: thread::JoinHandle<()>,
}

impl TestJwksServer {
    pub(super) fn start() -> Self {
        Self::start_with_success_count(16)
    }

    pub(super) fn start_with_success_count(success_count: usize) -> Self {
        Self::start_with_success_count_and_cache_control(success_count, None)
    }

    pub(super) fn start_with_delay(delay: Duration) -> Self {
        Self::start_with_options(0, None, delay)
    }

    pub(super) fn start_with_success_count_and_cache_control(
        success_count: usize,
        cache_control: Option<&'static str>,
    ) -> Self {
        Self::start_with_options(success_count, cache_control, Duration::ZERO)
    }

    fn start_with_options(
        success_count: usize,
        cache_control: Option<&'static str>,
        response_delay: Duration,
    ) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test JWKS server should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test JWKS server should expose TCP addr");
        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_for_thread = Arc::clone(&request_count);
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(request) = server.recv() else {
                    break;
                };
                let current = request_count_for_thread.fetch_add(1, Ordering::Relaxed);
                if !response_delay.is_zero() {
                    std::thread::sleep(response_delay);
                }
                let mut response = if current < success_count {
                    TinyResponse::from_string(gateway_oidc_test_jwks()).with_status_code(200)
                } else {
                    TinyResponse::from_string("temporary JWKS failure").with_status_code(503)
                };
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                if let Some(cache_control) = cache_control {
                    response.add_header(
                        TinyHeader::from_bytes("cache-control", cache_control).unwrap(),
                    );
                }
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            request_count,
            _thread: thread,
        }
    }

    pub(super) fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }
}

pub(super) struct TestOidcDiscoveryServer {
    pub(super) addr: SocketAddr,
    request_count: Arc<AtomicUsize>,
    _thread: thread::JoinHandle<()>,
}

impl TestOidcDiscoveryServer {
    pub(super) fn start() -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test OIDC server should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test OIDC server should expose TCP addr");
        let request_count = Arc::new(AtomicUsize::new(0));
        let request_count_for_thread = Arc::clone(&request_count);
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(request) = server.recv() else {
                    break;
                };
                request_count_for_thread.fetch_add(1, Ordering::Relaxed);
                let path = request.url().to_string();
                let body = if path == "/.well-known/openid-configuration" {
                    format!(r#"{{"issuer":"http://{addr}","jwks_uri":"http://{addr}/jwks.json"}}"#)
                } else {
                    gateway_oidc_test_jwks().to_string()
                };
                let mut response = TinyResponse::from_string(body).with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            request_count,
            _thread: thread,
        }
    }

    pub(super) fn request_count(&self) -> usize {
        self.request_count.load(Ordering::Relaxed)
    }
}

fn gateway_oidc_test_jwks() -> &'static str {
    r#"{"keys":[{"kty":"RSA","n":"yRE6rHuNR0QbHO3H3Kt2pOKGVhQqGZXInOduQNxXzuKlvQTLUTv4l4sggh5_CYYi_cvI-SXVT9kPWSKXxJXBXd_4LkvcPuUakBoAkfh-eiFVMh2VrUyWyj3MFl0HTVF9KwRXLAcwkREiS3npThHRyIxuy0ZMeZfxVL5arMhw1SRELB8HoGfG_AtH89BIE9jDBHZ9dLelK9a184zAf8LwoPLxvJb3Il5nncqPcSfKDDodMFBIMc4lQzDKL5gvmiXLXB1AGLm8KBjfE8s3L5xqi-yUod-j8MtvIj812dkS4QMiRVN_by2h3ZY8LYVGrqZXZTcgn2ujn8uKjXLZVD5TdQ","e":"AQAB","kid":"rsa01","alg":"RS256","use":"sig"}]}"#
}

pub(super) fn gateway_oidc_test_token(
    issuer: &str,
    audience: &str,
    email: &str,
    role: &str,
    prefixes: &[&str],
) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    gateway_oidc_test_token_with_exp(issuer, audience, email, role, prefixes, exp)
}

pub(super) fn gateway_oidc_test_token_without_scope_claims(
    issuer: &str,
    audience: &str,
    email: &str,
) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": email,
        "email": email,
        "exp": exp,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some("rsa01".to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(GATEWAY_OIDC_TEST_PRIVATE_KEY.as_bytes())
            .expect("test RSA key should parse"),
    )
    .expect("test OIDC token should sign")
}

pub(super) fn gateway_oidc_test_token_with_exp(
    issuer: &str,
    audience: &str,
    email: &str,
    role: &str,
    prefixes: &[&str],
    exp: u64,
) -> String {
    gateway_oidc_test_token_with_exp_and_kid(issuer, audience, email, role, prefixes, exp, "rsa01")
}

pub(super) fn gateway_oidc_test_token_with_exp_and_kid(
    issuer: &str,
    audience: &str,
    email: &str,
    role: &str,
    prefixes: &[&str],
    exp: u64,
    kid: &str,
) -> String {
    let claims = serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": email,
        "email": email,
        "prodex_role": role,
        "prodex_key_prefixes": prefixes,
        "exp": exp,
    });
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(kid.to_string());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(GATEWAY_OIDC_TEST_PRIVATE_KEY.as_bytes())
            .expect("test RSA key should parse"),
    )
    .expect("test OIDC token should sign")
}

pub(super) fn gateway_oidc_test_hs256_token(issuer: &str, audience: &str) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600;
    let claims = serde_json::json!({
        "iss": issuer,
        "aud": audience,
        "sub": "alice@example.com",
        "email": "alice@example.com",
        "prodex_role": "admin",
        "exp": exp,
    });
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(b"not-an-oidc-asymmetric-key"),
    )
    .expect("test HS256 token should sign")
}

const GATEWAY_OIDC_TEST_PRIVATE_KEY: &str = concat!(
    "-----BEGIN ",
    "PRIVATE KEY-----\n",
    r#"MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDJETqse41HRBsc
7cfcq3ak4oZWFCoZlcic525A3FfO4qW9BMtRO/iXiyCCHn8JhiL9y8j5JdVP2Q9Z
IpfElcFd3/guS9w+5RqQGgCR+H56IVUyHZWtTJbKPcwWXQdNUX0rBFcsBzCRESJL
eelOEdHIjG7LRkx5l/FUvlqsyHDVJEQsHwegZ8b8C0fz0EgT2MMEdn10t6Ur1rXz
jMB/wvCg8vG8lvciXmedyo9xJ8oMOh0wUEgxziVDMMovmC+aJctcHUAYubwoGN8T
yzcvnGqL7JSh36Pwy28iPzXZ2RLhAyJFU39vLaHdljwthUaupldlNyCfa6Ofy4qN
ctlUPlN1AgMBAAECggEAdESTQjQ70O8QIp1ZSkCYXeZjuhj081CK7jhhp/4ChK7J
GlFQZMwiBze7d6K84TwAtfQGZhQ7km25E1kOm+3hIDCoKdVSKch/oL54f/BK6sKl
qlIzQEAenho4DuKCm3I4yAw9gEc0DV70DuMTR0LEpYyXcNJY3KNBOTjN5EYQAR9s
2MeurpgK2MdJlIuZaIbzSGd+diiz2E6vkmcufJLtmYUT/k/ddWvEtz+1DnO6bRHh
xuuDMeJA/lGB/EYloSLtdyCF6sII6C6slJJtgfb0bPy7l8VtL5iDyz46IKyzdyzW
tKAn394dm7MYR1RlUBEfqFUyNK7C+pVMVoTwCC2V4QKBgQD64syfiQ2oeUlLYDm4
CcKSP3RnES02bcTyEDFSuGyyS1jldI4A8GXHJ/lG5EYgiYa1RUivge4lJrlNfjyf
dV230xgKms7+JiXqag1FI+3mqjAgg4mYiNjaao8N8O3/PD59wMPeWYImsWXNyeHS
55rUKiHERtCcvdzKl4u35ZtTqQKBgQDNKnX2bVqOJ4WSqCgHRhOm386ugPHfy+8j
m6cicmUR46ND6ggBB03bCnEG9OtGisxTo/TuYVRu3WP4KjoJs2LD5fwdwJqpgtHl
yVsk45Y1Hfo+7M6lAuR8rzCi6kHHNb0HyBmZjysHWZsn79ZM+sQnLpgaYgQGRbKV
DZWlbw7g7QKBgQCl1u+98UGXAP1jFutwbPsx40IVszP4y5ypCe0gqgon3UiY/G+1
zTLp79GGe/SjI2VpQ7AlW7TI2A0bXXvDSDi3/5Dfya9ULnFXv9yfvH1QwWToySpW
Kvd1gYSoiX84/WCtjZOr0e0HmLIb0vw0hqZA4szJSqoxQgvF22EfIWaIaQKBgQCf
34+OmMYw8fEvSCPxDxVvOwW2i7pvV14hFEDYIeZKW2W1HWBhVMzBfFB5SE8yaCQy
pRfOzj9aKOCm2FjjiErVNpkQoi6jGtLvScnhZAt/lr2TXTrl8OwVkPrIaN0bG/AS
aUYxmBPCpXu3UjhfQiWqFq/mFyzlqlgvuCc9g95HPQKBgAscKP8mLxdKwOgX8yFW
GcZ0izY/30012ajdHY+/QK5lsMoxTnn0skdS+spLxaS5ZEO4qvPVb8RAoCkWMMal
2pOhmquJQVDPDLuZHdrIiKiDM20dy9sMfHygWcZjQ4WSxf/J7T9canLZIXFhHAZT
3wc9h4G8BBCtWN2TN/LsGZdB
"#,
    "-----END ",
    "PRIVATE KEY-----",
);

pub(super) fn temp_root(name: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("prodex-{name}-{nonce}"));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    #[cfg(unix)]
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    root
}

pub(super) fn write_private_test_secret(
    path: impl AsRef<std::path::Path>,
    text: impl Into<String>,
) -> Result<(), secret_store::SecretError> {
    let path = path.as_ref();
    #[cfg(unix)]
    if let Some(parent) = path.parent() {
        fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
            .expect("test secret parent should be private");
    }
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), text)
}

pub(super) fn app_paths_for_root(root: std::path::PathBuf) -> AppPaths {
    AppPaths {
        state_file: root.join("state.json"),
        managed_profiles_root: root.join("profiles"),
        shared_codex_root: root.join("shared-codex"),
        legacy_shared_codex_root: root.join("shared"),
        root,
    }
}

pub(super) fn wait_for_usage_file(path: &std::path::Path) -> serde_json::Value {
    wait_for_json_file(path)
}

pub(super) fn wait_for_sqlite_usage_total(path: &std::path::Path, key_name: &str, expected: u64) {
    for _ in 0..50 {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let total = conn
                .query_row(
                    "SELECT requests_total FROM prodex_gateway_virtual_key_usage WHERE key_name = ?1",
                    [key_name],
                    |row| row.get::<_, i64>(0),
                )
                .ok()
                .and_then(|value| u64::try_from(value).ok());
            if total == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite usage total for {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_ledger_file_key_response_status(
    path: &std::path::Path,
    key_name: &str,
    expected: u16,
) {
    for _ in 0..50 {
        if let Ok(bytes) = fs::read(path) {
            for line in String::from_utf8_lossy(&bytes).lines() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(line)
                    && value["key_name"] == key_name
                    && value["response_status"] == expected
                {
                    return;
                }
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "ledger response status for key {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_sqlite_ledger_key_response_status(
    path: &std::path::Path,
    key_name: &str,
    expected: u16,
) {
    for _ in 0..50 {
        if let Ok(conn) = rusqlite::Connection::open(path) {
            let status = conn
                .query_row(
                    "SELECT response_status FROM prodex_gateway_billing_ledger WHERE key_name = ?1",
                    [key_name],
                    |row| row.get::<_, Option<i64>>(0),
                )
                .ok()
                .flatten()
                .and_then(|value| u16::try_from(value).ok());
            if status == Some(expected) {
                return;
            }
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!(
        "sqlite ledger response status for key {key_name} did not reach {expected} at {}",
        path.display()
    );
}

pub(super) fn wait_for_json_file(path: &std::path::Path) -> serde_json::Value {
    for _ in 0..50 {
        if let Ok(bytes) = fs::read(path)
            && let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes)
        {
            return value;
        }
        thread::sleep(Duration::from_millis(20));
    }
    panic!("usage file was not written at {}", path.display());
}

pub(super) struct TestGuardrailWebhook {
    pub(super) addr: SocketAddr,
    _thread: thread::JoinHandle<()>,
}

impl TestGuardrailWebhook {
    pub(super) fn start_deny(reason: &'static str) -> Self {
        let server = TinyServer::http("127.0.0.1:0").expect("test webhook should bind");
        let addr = server
            .server_addr()
            .to_ip()
            .expect("test webhook should expose TCP addr");
        let thread = thread::spawn(move || {
            for _ in 0..16 {
                let Ok(mut request) = server.recv() else {
                    break;
                };
                let mut body = Vec::new();
                let _ = request.as_reader().read_to_end(&mut body);
                let mut response = TinyResponse::from_string(format!(
                    r#"{{"allow":false,"reason":"{reason}","message":"do-not-log-webhook-message"}}"#
                ))
                .with_status_code(200);
                response.add_header(
                    TinyHeader::from_bytes("content-type", "application/json").unwrap(),
                );
                let _ = request.respond(response);
            }
        });
        Self {
            addr,
            _thread: thread,
        }
    }
}
