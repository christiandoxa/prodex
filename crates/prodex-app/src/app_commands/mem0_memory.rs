use super::runtime_launch;
use crate::{
    AppPaths, RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES, RuntimeLaunchRequest,
    RuntimeRotationProxy, print_launch_status, read_blocking_response_body_with_limit,
    read_blocking_response_text_with_limit,
};
use anyhow::{Context, Result, bail};
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

const MEM0_REPO_URL: &str = "https://github.com/mem0ai/mem0.git";
const MEM0_API_URL: &str = "http://127.0.0.1:8888";
const MEM0_HOST_GATEWAY_NAME: &str = "host.docker.internal";
const MEM0_EMBEDDER_MODEL: &str = "text-embedding-3-small";
const MEM0_DEFAULT_LLM_MODEL: &str = "gpt-4.1-nano-2025-04-14";
const MEM0_AUTH_ENV_NAME: &str = concat!("JWT", "_SECRET");
const MEM0_AUTH_RANDOM_PREFIX: &str = "prodex-mem0-jwt";
const MEM0_MODEL_CANDIDATES: &[&str] = &[
    "gpt-5.4-nano",
    "gpt-5-nano",
    "gpt-4.1-nano-2025-04-14",
    "gpt-4.1-nano",
    "gpt-5.4-mini",
    "gpt-5-mini",
    "gpt-4.1-mini",
    "gpt-4o-mini",
    "prodex-fast",
];

pub(crate) struct ManagedMem0Memory {
    pub(crate) api_url: String,
    pub(crate) api_key: String,
    pub(crate) _llm_model: String,
    pub(crate) _embedder_model: String,
    pub(crate) _gateway_proxy: RuntimeRotationProxy,
}

pub(crate) fn start_managed_mem0_memory(
    paths: &AppPaths,
    request: &RuntimeLaunchRequest<'_>,
) -> Result<ManagedMem0Memory> {
    print_launch_status("starting managed Mem0 memory via Docker...");
    let gateway_token = random_url_safe_token("prodex-mem0")?;
    print_launch_status("starting session-local Prodex gateway for Mem0...");
    let gateway_proxy = runtime_launch::start_mem0_memory_gateway_for_runtime_request(
        paths,
        request,
        &gateway_token,
    )
    .context("failed to start Prodex gateway for managed Mem0 memory")?;
    let gateway_base_url = format!(
        "http://{}:{}",
        MEM0_HOST_GATEWAY_NAME,
        gateway_proxy.listen_addr.port()
    );
    print_launch_status(&format!(
        "Mem0 gateway ready at http://127.0.0.1:{}.",
        gateway_proxy.listen_addr.port()
    ));
    let llm_model = select_mem0_memory_model(&gateway_proxy, &gateway_token)
        .unwrap_or_else(|| MEM0_DEFAULT_LLM_MODEL.to_string());
    print_launch_status(&format!(
        "Mem0 model={} embedder={}.",
        llm_model, MEM0_EMBEDDER_MODEL
    ));
    let root = paths.root.join("mem0");
    let checkout = root.join("mem0");
    let server_dir = checkout.join("server");
    if mem0_checkout_has_server_compose(&checkout)? {
        print_launch_status(&format!(
            "using existing Mem0 server checkout at {}.",
            server_dir.display()
        ));
    } else {
        print_launch_status(&format!(
            "cloning Mem0 OSS server into {}...",
            checkout.display()
        ));
    }
    ensure_mem0_checkout(&checkout)?;
    let secrets = load_or_create_mem0_secrets(&server_dir)?;
    print_launch_status("writing Mem0 local .env and Docker override...");
    write_mem0_env(
        &server_dir,
        &gateway_token,
        &gateway_base_url,
        &secrets,
        &llm_model,
        MEM0_EMBEDDER_MODEL,
    )?;
    write_mem0_compose_override(&server_dir)?;
    print_launch_status(
        "starting Mem0 Docker Compose stack. First launch can build images and take several minutes...",
    );
    start_mem0_compose_stack(&server_dir)?;
    print_launch_status(&format!("waiting for Mem0 API at {MEM0_API_URL}..."));
    wait_for_mem0_api(MEM0_API_URL, &secrets.admin_api_key)?;
    print_launch_status("configuring Mem0 server to use Prodex gateway...");
    configure_mem0_server(
        MEM0_API_URL,
        &secrets.admin_api_key,
        &gateway_token,
        &gateway_base_url,
        &secrets,
        &llm_model,
        MEM0_EMBEDDER_MODEL,
    )?;
    print_launch_status(&format!("managed Mem0 memory is ready at {MEM0_API_URL}."));
    Ok(ManagedMem0Memory {
        api_url: MEM0_API_URL.to_string(),
        api_key: secrets.admin_api_key,
        _llm_model: llm_model,
        _embedder_model: MEM0_EMBEDDER_MODEL.to_string(),
        _gateway_proxy: gateway_proxy,
    })
}

fn configure_mem0_server(
    api_url: &str,
    admin_api_key: &str,
    gateway_token: &str,
    gateway_base_url: &str,
    secrets: &Mem0Secrets,
    llm_model: &str,
    embedder_model: &str,
) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("failed to build Mem0 configure HTTP client")?;
    let response = client
        .post(format!("{api_url}/configure"))
        .header("X-API-Key", admin_api_key)
        .json(&json!({
            "version": "v1.1",
            "vector_store": {
                "provider": "pgvector",
                "config": {
                    "host": "postgres",
                    "port": 5432,
                    "dbname": "postgres",
                    "user": "postgres",
                    "password": secrets.postgres_password,
                    "collection_name": "memories",
                },
            },
            "llm": {
                "provider": "openai",
                "config": {
                    "api_key": gateway_token,
                    "openai_base_url": format!("{gateway_base_url}/v1"),
                    "temperature": 0.2,
                    "model": llm_model,
                },
            },
            "embedder": {
                "provider": "openai",
                "config": {
                    "api_key": gateway_token,
                    "openai_base_url": format!("{gateway_base_url}/v1"),
                    "model": embedder_model,
                },
            },
            "history_db_path": "/app/history/history.db",
        }))
        .send()
        .context("failed to configure managed Mem0 server")?;
    let status = response.status();
    let body = read_blocking_response_text_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read managed Mem0 configure response",
    )
    .unwrap_or_default();
    if !status.is_success() {
        bail!("managed Mem0 /configure returned {status}: {body}");
    }
    Ok(())
}

fn ensure_mem0_checkout(checkout: &Path) -> Result<()> {
    if mem0_checkout_has_server_compose(checkout)? {
        return Ok(());
    }
    if mem0_path_exists(checkout)? {
        bail!(
            "{} exists but does not look like a Mem0 checkout with server/docker-compose.yaml",
            checkout.display()
        );
    }
    ensure_mem0_checkout_parent(checkout)?;
    let status = Command::new("git")
        .args(["clone", "--depth=1", MEM0_REPO_URL])
        .arg(checkout)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to execute git clone for managed Mem0")?;
    if !status.success() {
        bail!("failed to clone Mem0 server from {MEM0_REPO_URL}");
    }
    Ok(())
}

fn ensure_mem0_checkout_parent(checkout: &Path) -> Result<()> {
    let Some(parent) = checkout.parent() else {
        return Ok(());
    };
    if mem0_path_exists(parent)? {
        if !mem0_path_is_regular_dir(parent)? {
            bail!(
                "Mem0 checkout parent {} is not a directory",
                parent.display()
            );
        }
        return Ok(());
    }
    fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
    if !mem0_path_is_regular_dir(parent)? {
        bail!(
            "Mem0 checkout parent {} is not a directory",
            parent.display()
        );
    }
    Ok(())
}

fn mem0_checkout_has_server_compose(checkout: &Path) -> Result<bool> {
    let server = checkout.join("server");
    Ok(mem0_path_is_regular_dir(checkout)?
        && mem0_path_is_regular_dir(&server)?
        && mem0_path_is_regular_file(&server.join("docker-compose.yaml"))?)
}

fn mem0_path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn mem0_path_is_regular_dir(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_dir()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn mem0_path_is_regular_file(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(!metadata.file_type().is_symlink() && metadata.is_file()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Mem0Secrets {
    postgres_password: String,
    admin_api_key: String,
    jwt_secret: String,
}

fn load_or_create_mem0_secrets(server_dir: &Path) -> Result<Mem0Secrets> {
    let env_path = server_dir.join(".env");
    let existing = read_env_assignments(&env_path)?;
    Ok(Mem0Secrets {
        postgres_password: env_or_random_value(&existing, "POSTGRES_PASSWORD", "prodex-pg")?,
        admin_api_key: env_or_random_value(&existing, "ADMIN_API_KEY", "prodex-mem0-admin")?,
        jwt_secret: env_or_random_value(&existing, MEM0_AUTH_ENV_NAME, MEM0_AUTH_RANDOM_PREFIX)?,
    })
}

fn env_or_random_value(
    values: &BTreeMap<String, String>,
    key: &str,
    prefix: &str,
) -> Result<String> {
    if let Some(value) = values
        .get(key)
        .cloned()
        .filter(|value| !value.trim().is_empty())
    {
        return Ok(value);
    }
    random_url_safe_token(prefix)
}

fn write_mem0_env(
    server_dir: &Path,
    gateway_token: &str,
    gateway_base_url: &str,
    secrets: &Mem0Secrets,
    llm_model: &str,
    embedder_model: &str,
) -> Result<()> {
    let env_path = server_dir.join(".env");
    let contents = render_mem0_env(
        gateway_token,
        gateway_base_url,
        secrets,
        llm_model,
        embedder_model,
    );
    write_secret_file(&env_path, &contents)?;
    Ok(())
}

fn write_secret_file(path: &Path, contents: &str) -> Result<()> {
    secret_store::SecretManager::new(secret_store::FileSecretBackend::new())
        .write_text(&secret_store::SecretLocation::file(path), contents)
        .map_err(anyhow::Error::new)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn render_mem0_env(
    gateway_token: &str,
    gateway_base_url: &str,
    secrets: &Mem0Secrets,
    llm_model: &str,
    embedder_model: &str,
) -> String {
    format!(
        "\
OPENAI_API_KEY={gateway_token}
OPENAI_BASE_URL={gateway_base_url}/v1
POSTGRES_HOST=postgres
POSTGRES_PORT=5432
POSTGRES_DB=postgres
POSTGRES_USER=postgres
POSTGRES_PASSWORD={postgres_password}
POSTGRES_COLLECTION_NAME=memories
ADMIN_API_KEY={admin_api_key}
JWT_SECRET={jwt_secret}
AUTH_DISABLED=false
DASHBOARD_URL=http://localhost:3000
APP_DB_NAME=mem0_app
MEM0_DEFAULT_LLM_MODEL={llm_model}
MEM0_DEFAULT_EMBEDDER_MODEL={embedder_model}
MEM0_TELEMETRY=false
REQUEST_LOG_RETENTION_DAYS=30
",
        postgres_password = secrets.postgres_password,
        admin_api_key = secrets.admin_api_key,
        jwt_secret = secrets.jwt_secret,
    )
}

fn write_mem0_compose_override(server_dir: &Path) -> Result<()> {
    let override_path = server_dir.join("docker-compose.override.yml");
    let contents = "\
services:
  mem0:
    extra_hosts:
      - \"host.docker.internal:host-gateway\"
";
    fs::write(&override_path, contents)
        .with_context(|| format!("failed to write {}", override_path.display()))?;
    Ok(())
}

fn start_mem0_compose_stack(server_dir: &Path) -> Result<()> {
    let compose = DockerComposeCommand::detect()?;
    compose.run(server_dir, &["up", "-d", "postgres"])?;
    compose.run(server_dir, &["up", "-d", "--force-recreate", "mem0"])?;
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DockerComposeCommand {
    program: &'static str,
    prefix_args: Vec<&'static str>,
}

impl DockerComposeCommand {
    fn detect() -> Result<Self> {
        let docker = Self {
            program: "docker",
            prefix_args: vec!["compose"],
        };
        if docker.version_ok() {
            return Ok(docker);
        }
        let docker_compose = Self {
            program: "docker-compose",
            prefix_args: Vec::new(),
        };
        if docker_compose.version_ok() {
            return Ok(docker_compose);
        }
        bail!("managed Mem0 memory requires Docker Compose (`docker compose` or `docker-compose`)");
    }

    fn version_ok(&self) -> bool {
        Command::new(self.program)
            .args(&self.prefix_args)
            .arg("version")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }

    fn run(&self, server_dir: &Path, args: &[&str]) -> Result<()> {
        print_launch_status(&format!(
            "running `{}` in {} ...",
            std::iter::once(self.display())
                .chain(args.iter().map(|arg| (*arg).to_string()))
                .collect::<Vec<_>>()
                .join(" "),
            server_dir.display()
        ));
        let status = Command::new(self.program)
            .args(&self.prefix_args)
            .args(args)
            .current_dir(server_dir)
            .env("COMPOSE_PROJECT_NAME", "prodex-mem0")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("failed to execute {} for managed Mem0", self.display()))?;
        if !status.success() {
            bail!("{} {} failed with {status}", self.display(), args.join(" "));
        }
        Ok(())
    }

    fn display(&self) -> String {
        std::iter::once(self.program)
            .chain(self.prefix_args.iter().copied())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn wait_for_mem0_api(api_url: &str, api_key: &str) -> Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .context("failed to build Mem0 readiness HTTP client")?;
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        let ready = client
            .get(format!("{api_url}/configure/providers"))
            .header("X-API-Key", api_key)
            .send()
            .is_ok_and(|response| response.status().is_success());
        if ready {
            return Ok(());
        }
        thread::sleep(Duration::from_secs(2));
    }
    bail!("managed Mem0 server did not become ready at {api_url}");
}

fn select_mem0_memory_model(proxy: &RuntimeRotationProxy, gateway_token: &str) -> Option<String> {
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .ok()?;
    let response = client
        .get(format!("http://{}/v1/models", proxy.listen_addr))
        .bearer_auth(gateway_token)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = read_blocking_response_body_with_limit(
        response,
        RUNTIME_PROXY_BUFFERED_RESPONSE_MAX_BYTES,
        "failed to read managed Mem0 model response",
    )
    .ok()?;
    let value: Value = serde_json::from_slice(&body).ok()?;
    let models = value
        .get("data")
        .and_then(Value::as_array)?
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .collect::<Vec<_>>();
    MEM0_MODEL_CANDIDATES
        .iter()
        .find(|candidate| models.iter().any(|model| model == *candidate))
        .map(|model| (*model).to_string())
}

fn read_env_assignments(path: &Path) -> Result<BTreeMap<String, String>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(err) => return Err(err).with_context(|| format!("failed to read {}", path.display())),
    };
    let mut values = BTreeMap::new();
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), unquote_env_value(value.trim()));
    }
    Ok(values)
}

fn unquote_env_value(value: &str) -> String {
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn random_url_safe_token(prefix: &str) -> Result<String> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes).context("failed to generate managed Mem0 secret")?;
    let token = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
    Ok(format!("{prefix}-{token}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn render_mem0_env_routes_openai_to_gateway() {
        let secrets = Mem0Secrets {
            postgres_password: "pg".to_string(),
            admin_api_key: "admin".to_string(),
            jwt_secret: "jwt".to_string(),
        };
        let env = render_mem0_env(
            "gateway-token",
            "http://host.docker.internal:1234",
            &secrets,
            "gpt-4.1-nano-2025-04-14",
            "text-embedding-3-small",
        );
        assert!(env.contains("OPENAI_API_KEY=gateway-token\n"));
        assert!(env.contains("OPENAI_BASE_URL=http://host.docker.internal:1234/v1\n"));
        assert!(env.contains("AUTH_DISABLED=false\n"));
        assert!(env.contains("MEM0_TELEMETRY=false\n"));
    }

    #[test]
    fn env_reader_preserves_existing_secret_values() {
        let root = temp_mem0_dir("env-reader");
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(".env"),
            "POSTGRES_PASSWORD='pg secret'\nADMIN_API_KEY=admin\nJWT_SECRET=\"jwt secret\"\n",
        )
        .unwrap();
        let secrets = load_or_create_mem0_secrets(&root).unwrap();
        assert_eq!(secrets.postgres_password, "pg secret");
        assert_eq!(secrets.admin_api_key, "admin");
        assert_eq!(secrets.jwt_secret, "jwt secret");
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_mem0_checkout_rejects_symlink_checkout() {
        let root = temp_mem0_dir("checkout-symlink");
        let checkout = root.join("mem0");
        let outside = root.join("outside");
        fs::create_dir_all(outside.join("server")).unwrap();
        fs::write(outside.join("server/docker-compose.yaml"), "services: {}\n").unwrap();
        std::os::unix::fs::symlink(&outside, &checkout).unwrap();

        let err = ensure_mem0_checkout(&checkout).expect_err("symlink checkout should reject");

        assert!(
            err.to_string()
                .contains("does not look like a Mem0 checkout"),
            "unexpected error: {err:#}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn ensure_mem0_checkout_rejects_symlink_parent() {
        let root = temp_mem0_dir("checkout-parent-symlink");
        let outside = root.join("outside-parent");
        let parent = root.join("mem0-root");
        let checkout = parent.join("mem0");
        fs::create_dir_all(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, &parent).unwrap();

        let err = ensure_mem0_checkout(&checkout).expect_err("symlink parent should reject");

        assert!(
            err.to_string().contains("is not a directory"),
            "unexpected error: {err:#}"
        );
        assert!(!outside.join("mem0").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn write_mem0_env_restricts_existing_env_file() {
        use std::os::unix::fs::PermissionsExt;

        let root = temp_mem0_dir("env-mode");
        fs::create_dir_all(&root).unwrap();
        let env_path = root.join(".env");
        fs::write(&env_path, "OLD_SECRET=leaky\n").unwrap();
        fs::set_permissions(&env_path, fs::Permissions::from_mode(0o644)).unwrap();

        let secrets = Mem0Secrets {
            postgres_password: "pg".to_string(),
            admin_api_key: "admin".to_string(),
            jwt_secret: "jwt".to_string(),
        };
        write_mem0_env(
            &root,
            "gateway-token",
            "http://host.docker.internal:1234",
            &secrets,
            "llm",
            "embedder",
        )
        .unwrap();

        assert_eq!(
            fs::metadata(&env_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn write_mem0_env_replaces_symlink_without_touching_target() {
        let root = temp_mem0_dir("env-symlink");
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.env");
        let env_path = root.join(".env");
        fs::write(&target, "DO_NOT_TOUCH=1\n").unwrap();
        std::os::unix::fs::symlink(&target, &env_path).unwrap();
        let secrets = Mem0Secrets {
            postgres_password: "pg".to_string(),
            admin_api_key: "admin".to_string(),
            jwt_secret: "jwt".to_string(),
        };

        write_mem0_env(
            &root,
            "gateway-token",
            "http://host",
            &secrets,
            "llm",
            "embedder",
        )
        .unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "DO_NOT_TOUCH=1\n");
        assert!(
            !fs::symlink_metadata(&env_path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(
            fs::read_to_string(&env_path)
                .unwrap()
                .contains("OPENAI_API_KEY=gateway-token\n")
        );
        let _ = fs::remove_dir_all(root);
    }

    fn temp_mem0_dir(name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!("prodex-mem0-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        path
    }
}
