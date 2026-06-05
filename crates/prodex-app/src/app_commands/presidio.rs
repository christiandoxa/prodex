use crate::{
    AppPaths, PresidioCommands, PresidioDoctorArgs, PresidioEnableArgs, PresidioRedactArgs,
    PresidioLanguageMode,
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
    language: Option<String>, // Deprecated: use languages instead.
    languages: Option<Vec<String>>,
    language_mode: PresidioLanguageMode,
    fail_mode: String,
}

impl Default for ProdexPresidioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            analyzer_url: DEFAULT_PRESIDIO_ANALYZER_URL.to_string(),
            anonymizer_url: DEFAULT_PRESIDIO_ANONYMIZER_URL.to_string(),
            language: Some(DEFAULT_PRESIDIO_LANGUAGE.to_string()),
            languages: None,
            language_mode: PresidioLanguageMode::Fixed,
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
    // Add language field for multi-language merge
    #[serde(default)]
    language: String,
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
    let analyzer_url = args.analyzer_url.unwrap_or_else(|| config.analyzer_url.clone());
    let anonymizer_url = args.anonymizer_url.unwrap_or_else(|| config.anonymizer_url.clone());
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let client = presidio_http_client()?;
    let analyzer = probe_presidio_health(&client, &analyzer_url);
    let anonymizer = probe_presidio_health(&client, &anonymizer_url);

    let (languages, language_mode) = resolve_languages_and_mode(&config);

    if args.json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "config_path": presidio_config_path(&paths),
                "analyzer_url": analyzer_url,
                "anonymizer_url": anonymizer_url,
                "language_mode": language_mode.as_str(),
                "languages": languages,
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
            ("Language Mode".to_string(), language_mode.as_str().to_string()),
            ("Languages".to_string(), languages.join(", ")),
        ],
    );
    Ok(())
}

fn handle_presidio_status() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let (languages, language_mode) = resolve_languages_and_mode(&config);

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
            ("Language Mode".to_string(), language_mode.as_str().to_string()),
            ("Languages".to_string(), languages.join(", ")),
            ("Fail mode".to_string(), config.fail_mode),
        ],
    );
    Ok(())
}

fn handle_presidio_enable(args: PresidioEnableArgs) -> Result<()> {
    validate_presidio_url(&args.analyzer_url, "analyzer_url")?;
    validate_presidio_url(&args.anonymizer_url, "anonymizer_url")?;

    let languages = if !args.languages.is_empty() {
        normalize_languages(args.languages)
    } else if let Some(lang) = args.language {
        normalize_languages(vec![lang])
    } else {
        normalize_languages(vec![DEFAULT_PRESIDIO_LANGUAGE.to_string()])
    };

    let language_mode = args.language_mode;

    validate_language_config(&languages, language_mode)?;

    let paths = AppPaths::discover()?;
    let config = ProdexPresidioConfig {
        enabled: true,
        analyzer_url: args.analyzer_url,
        anonymizer_url: args.anonymizer_url,
        language: None, // Deprecated, always set to None for new saves
        languages: Some(languages.clone()),
        language_mode,
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
            ("Language Mode".to_string(), language_mode.as_str().to_string()),
            ("Languages".to_string(), languages.join(", ")),
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
    let analyzer_url = args.analyzer_url.unwrap_or_else(|| config.analyzer_url.clone());
    let anonymizer_url = args.anonymizer_url.unwrap_or_else(|| config.anonymizer_url.clone());
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let text = presidio_redact_input_text(args.text, args.path)?;

    let (mut languages_to_use, mut language_mode_to_use) = resolve_languages_and_mode(&config);

    // CLI args override config
    if !args.languages.is_empty() {
        languages_to_use = normalize_languages(args.languages);
    }
    if args.language.is_some() {
        languages_to_use = normalize_languages(vec![args.language.unwrap()]);
        language_mode_to_use = PresidioLanguageMode::Fixed;
    }
    // If language_mode is provided via CLI, it overrides the config
    // Need to check if the argument was explicitly provided for `language_mode`
    // clap provides a way to check if an argument was "present" or "set_by_user"
    // However, PresidioRedactArgs does not store ValueSource, so we have to assume a default_value_t.
    // So, we'll override if it's not the default.
    if args.language_mode != PresidioLanguageMode::Fixed {
        language_mode_to_use = args.language_mode;
    }
    validate_language_config(&languages_to_use, language_mode_to_use)?;


    let all_analyzer_results = match language_mode_to_use {
        PresidioLanguageMode::Fixed => {
            presidio_analyze(&presidio_http_client()?, &analyzer_url, &text, &languages_to_use[0])?
        }
        PresidioLanguageMode::Auto => {
            let detected_lang = detect_presidio_language(&text, &languages_to_use)
                .unwrap_or_else(|| languages_to_use[0].clone());
            presidio_analyze(&presidio_http_client()?, &analyzer_url, &text, &detected_lang)?
        }
        PresidioLanguageMode::Multi => {
            let mut all_results = Vec::new();
            for lang in &languages_to_use {
                let results = presidio_analyze(&presidio_http_client()?, &analyzer_url, &text, lang)?;
                all_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            merge_presidio_analyzer_results(all_results)
        }
    };

    let anonymized = presidio_anonymize(&presidio_http_client()?, &anonymizer_url, &text, all_analyzer_results)?;

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

fn normalize_languages(langs: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = langs
        .into_iter()
        .map(|s| s.trim().to_lowercase())
        .collect();
    normalized.sort_unstable();
    normalized.dedup();
    normalized
}

fn validate_language_config(languages: &[String], mode: PresidioLanguageMode) -> Result<()> {
    if languages.is_empty() {
        bail!("Presidio languages cannot be empty");
    }
    if mode == PresidioLanguageMode::Fixed && languages.len() != 1 {
        bail!(
            "Fixed Presidio language mode requires exactly one language, found: {:?}",
            languages
        );
    }
    Ok(())
}

fn resolve_languages_and_mode(config: &ProdexPresidioConfig) -> (Vec<String>, PresidioLanguageMode) {
    let languages = config.languages.clone().unwrap_or_else(|| {
        config.language.clone().map(|l| vec![l]).unwrap_or_else(|| vec![DEFAULT_PRESIDIO_LANGUAGE.to_string()])
    });
    (normalize_languages(languages), config.language_mode)
}

// Simple heuristic for language detection
fn detect_presidio_language(text: &str, candidates: &[String]) -> Option<String> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    let id_keywords = ["yang", "dan", "di", "ke", "dari", "saya", "kami", "anda", "nomor", "nama", "alamat", "tanggal", "lahir", "dengan", "untuk"];
    let en_keywords = ["the", "and", "to", "from", "my", "name", "phone", "email", "address", "with", "for", "birth"];

    let mut id_score = 0;
    let mut en_score = 0;

    let lower_text = text.to_lowercase();

    for keyword in id_keywords.iter() {
        if lower_text.contains(keyword) {
            id_score += 1;
        }
    }
    for keyword in en_keywords.iter() {
        if lower_text.contains(keyword) {
            en_score += 1;
        }
    }

    if id_score > en_score && candidates.contains(&"id".to_string()) {
        Some("id".to_string())
    } else if en_score > id_score && candidates.contains(&"en".to_string()) {
        Some("en".to_string())
    } else {
        // Fallback to the first language in the candidates list
        candidates.first().cloned()
    }
}

fn merge_presidio_analyzer_results(mut results: Vec<PresidioAnalyzerResult>) -> Vec<PresidioAnalyzerResult> {
    results.sort_by(|a, b| {
        a.start.cmp(&b.start)
            .then_with(|| a.end.cmp(&b.end))
            .then_with(|| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal)) // Higher score first
            .then_with(|| b.entity_type.cmp(&a.entity_type)) // Consistent tie-breaking
    });

    let mut merged: Vec<PresidioAnalyzerResult> = Vec::new();
    for result in results {
        if let Some(last) = merged.last_mut() {
            // Check for exact duplicates or overlapping results.
            // If they are exactly the same in terms of start, end, and entity type, deduplicate.
            if last.start == result.start && last.end == result.end && last.entity_type == result.entity_type {
                // Keep the one with higher score, or current if scores are equal.
                if result.score > last.score {
                    *last = result;
                }
                continue;
            }

            // If there's an overlap, and current result has higher score or longer span
            let overlaps = result.start < last.end && result.end > last.start;
            if overlaps {
                if result.score > last.score || (result.score == last.score && (result.end - result.start) > (last.end - last.start)) {
                    // This logic is tricky. For overlapping, we could:
                    // 1. Keep the higher score, merging spans.
                    // 2. Keep the higher score, replacing the old.
                    // 3. Prioritize non-overlapping entities.
                    // For now, let's keep it simple: if there's an overlap and the current result
                    // has a strictly higher score, replace the last one.
                    // If scores are equal, prefer longer span.
                    // This needs more thought for a robust merging strategy for different entity types.
                    // For this task, assuming simple replacement for now.
                    // A more advanced merge would be context-aware.
                    // For now, if overlap and current is "better", replace. Otherwise, add.

                    // More robust merge logic for overlapping entities:
                    // If the new result completely subsumes the old one, replace.
                    // If the old one subsumes the new one, skip the new one.
                    // If they partially overlap, it gets complicated. For MVP, we'll try to
                    // simply replace if the new score is better, or extend the span if needed.
                    if (result.start >= last.start && result.end <= last.end) || // new is contained in old
                       (last.start >= result.start && last.end <= result.end) { // old is contained in new
                        // if contained, prefer higher score. if scores equal, keep current merged
                        if result.score > last.score {
                            *last = result;
                        }
                        continue;
                    } else if result.score > last.score {
                         // partially overlapping, and new is better score. merge spans.
                        last.start = last.start.min(result.start);
                        last.end = last.end.max(result.end);
                        last.score = result.score;
                        last.entity_type = result.entity_type; // keep new entity type
                        last.language = result.language;
                        continue;
                    }
                }
            }
        }
        merged.push(result);
    }
    merged
}
