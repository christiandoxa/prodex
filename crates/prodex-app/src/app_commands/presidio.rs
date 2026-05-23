use crate::{
    AppPaths, PresidioCommands, PresidioDoctorArgs, PresidioEnableArgs, PresidioRedactArgs,
};
use anyhow::{Context, Result, bail};
use reqwest::blocking::Client;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::Duration;
use terminal_ui::print_panel;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const DEFAULT_PRESIDIO_LANGUAGE: &str = "en";
const PRESIDIO_HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct ProdexPresidioConfig {
    enabled: bool,
    analyzer_url: String,
    anonymizer_url: String,
    language: String,
    fail_mode: String,
}

impl Default for ProdexPresidioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            analyzer_url: DEFAULT_PRESIDIO_ANALYZER_URL.to_string(),
            anonymizer_url: DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string(),
            language: DEFAULT_PRESIDIO_LANGUAGE.to_string(),
            fail_mode: "open".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
struct PresidioAnalyzerResult {
    start: usize,
    end: usize,
    score: f64,
    entity_type: String,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct PresidioAnonymizeResponse {
    text: String,
    #[serde(default)]
    items: Vec<serde_json::Value>,
}

#[derive(Debug)]
struct PresidioHealth {
    ok: bool,
    message: String,
}

pub(crate) fn handle_presidio(command: PresidioCommands) -> Result<()> {
    match command {
        PresidioCommands::Doctor(args) => handle_presidio_doctor(args),
        PresidioCommands::Status => handle_presidio_status(),
        PresidioCommands::Redact(args) => handle_presidio_redact(args),
        PresidioCommands::Enable(args) => handle_presidio_enable(args),
        PresidioCommands::Disable => handle_presidio_disable(),
    }
}

fn handle_presidio_doctor(args: PresidioDoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let analyzer_url = args.analyzer_url.unwrap_or(config.analyzer_url);
    let anonymizer_url = args.anonymizer_url.unwrap_or(config.anonymizer_url);
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let client = presidio_http_client()?;
    let analyzer = probe_presidio_health(&client, &analyzer_url);
    let anonymizer = probe_presidio_health(&client, &anonymizer_url);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "config_path": presidio_config_path(&paths),
                "analyzer_url": analyzer_url,
                "anonymizer_url": anonymizer_url,
                "analyzer": {
                    "ok": analyzer.ok,
                    "message": analyzer.message,
                },
                "anonymizer": {
                    "ok": anonymizer.ok,
                    "message": anonymizer.message,
                },
            }))?
        );
        return Ok(());
    }

    print_panel(
        "Presidio",
        &[
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Analyzer".to_string(), analyzer_url),
            (
                "Analyzer health".to_string(),
                presidio_health_label(&analyzer),
            ),
            ("Anonymizer".to_string(), anonymizer_url),
            (
                "Anonymizer health".to_string(),
                presidio_health_label(&anonymizer),
            ),
        ],
    );
    Ok(())
}

fn handle_presidio_status() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    print_panel(
        "Presidio",
        &[
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), config.enabled.to_string()),
            ("Analyzer".to_string(), config.analyzer_url),
            ("Anonymizer".to_string(), config.anonymizer_url),
            ("Language".to_string(), config.language),
            ("Fail mode".to_string(), config.fail_mode),
        ],
    );
    Ok(())
}

fn handle_presidio_enable(args: PresidioEnableArgs) -> Result<()> {
    validate_presidio_url(&args.analyzer_url, "analyzer_url")?;
    validate_presidio_url(&args.anonymizer_url, "anonymizer_url")?;
    let paths = AppPaths::discover()?;
    let config = ProdexPresidioConfig {
        enabled: true,
        analyzer_url: args.analyzer_url,
        anonymizer_url: args.anonymizer_url,
        language: args.language,
        fail_mode: args.fail_mode.as_str().to_string(),
    };
    save_presidio_config(&paths, &config)?;
    print_panel(
        "Presidio",
        &[
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), "true".to_string()),
            ("Analyzer".to_string(), config.analyzer_url),
            ("Anonymizer".to_string(), config.anonymizer_url),
            ("Language".to_string(), config.language),
            ("Fail mode".to_string(), config.fail_mode),
        ],
    );
    Ok(())
}

fn handle_presidio_disable() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut config = load_presidio_config(&paths)?.unwrap_or_default();
    config.enabled = false;
    save_presidio_config(&paths, &config)?;
    print_panel(
        "Presidio",
        &[
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), "false".to_string()),
        ],
    );
    Ok(())
}

fn handle_presidio_redact(args: PresidioRedactArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let analyzer_url = args.analyzer_url.unwrap_or(config.analyzer_url);
    let anonymizer_url = args.anonymizer_url.unwrap_or(config.anonymizer_url);
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let text = presidio_redact_input_text(args.text, args.path)?;
    let client = presidio_http_client()?;
    let analyzer_results = presidio_analyze(&client, &analyzer_url, &text, &args.language)?;
    let anonymized = presidio_anonymize(&client, &anonymizer_url, &text, analyzer_results)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&anonymized)?);
    } else {
        println!("{}", anonymized.text);
    }
    Ok(())
}

fn presidio_redact_input_text(text: Option<String>, path: Option<PathBuf>) -> Result<String> {
    if let Some(text) = text {
        return Ok(text);
    }
    if let Some(path) = path {
        return fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()));
    }
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .context("failed to read stdin")?;
    Ok(input)
}

fn presidio_analyze(
    client: &Client,
    analyzer_url: &str,
    text: &str,
    language: &str,
) -> Result<Vec<PresidioAnalyzerResult>> {
    let response = client
        .post(presidio_endpoint(analyzer_url, "analyze"))
        .json(&serde_json::json!({
            "text": text,
            "language": language,
        }))
        .send()
        .context("failed to call Presidio Analyzer")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        bail!("Presidio Analyzer returned {status}: {}", body.trim());
    }
    response
        .json::<Vec<PresidioAnalyzerResult>>()
        .context("failed to parse Presidio Analyzer response")
}

fn presidio_anonymize(
    client: &Client,
    anonymizer_url: &str,
    text: &str,
    analyzer_results: Vec<PresidioAnalyzerResult>,
) -> Result<PresidioAnonymizeResponse> {
    let response = client
        .post(presidio_endpoint(anonymizer_url, "anonymize"))
        .json(&serde_json::json!({
            "text": text,
            "analyzer_results": analyzer_results,
        }))
        .send()
        .context("failed to call Presidio Anonymizer")?;
    let status = response.status();
    if !status.is_success() {
        let body = response.text().unwrap_or_default();
        bail!("Presidio Anonymizer returned {status}: {}", body.trim());
    }
    response
        .json::<PresidioAnonymizeResponse>()
        .context("failed to parse Presidio Anonymizer response")
}

fn probe_presidio_health(client: &Client, base_url: &str) -> PresidioHealth {
    match client.get(presidio_endpoint(base_url, "health")).send() {
        Ok(response) => {
            let status = response.status();
            let message = response.text().unwrap_or_default();
            PresidioHealth {
                ok: status.is_success(),
                message: if message.trim().is_empty() {
                    status.to_string()
                } else {
                    format!("{status} {}", message.trim())
                },
            }
        }
        Err(err) => PresidioHealth {
            ok: false,
            message: err.to_string(),
        },
    }
}

fn presidio_health_label(health: &PresidioHealth) -> String {
    if health.ok {
        format!("ok ({})", health.message)
    } else {
        format!("failed ({})", health.message)
    }
}

fn presidio_http_client() -> Result<Client> {
    Client::builder()
        .timeout(PRESIDIO_HTTP_TIMEOUT)
        .no_proxy()
        .build()
        .context("failed to build Presidio HTTP client")
}

fn load_presidio_config(paths: &AppPaths) -> Result<Option<ProdexPresidioConfig>> {
    let path = presidio_config_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let raw =
        fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
    let config = toml::from_str::<ProdexPresidioConfig>(&raw)
        .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(Some(config))
}

fn save_presidio_config(paths: &AppPaths, config: &ProdexPresidioConfig) -> Result<()> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let path = presidio_config_path(paths);
    let raw = toml::to_string_pretty(config).context("failed to render Presidio config")?;
    fs::write(&path, raw).with_context(|| format!("failed to write {}", path.display()))
}

fn presidio_config_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(PRODEX_PRESIDIO_FILE_NAME)
}

fn validate_presidio_url(url: &str, field: &str) -> Result<()> {
    let parsed = reqwest::Url::parse(url).with_context(|| format!("invalid {field}: {url}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        bail!("invalid {field}: scheme must be http or https");
    }
    if parsed.host_str().is_none() {
        bail!("invalid {field}: host is required");
    }
    Ok(())
}

fn presidio_endpoint(base_url: &str, path: &str) -> String {
    format!("{}/{}", base_url.trim_end_matches('/'), path)
}
