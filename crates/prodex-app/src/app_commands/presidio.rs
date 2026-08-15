use crate::{
    AppPaths, PresidioCommands, PresidioDoctorArgs, PresidioEnableArgs, PresidioLanguageMode,
    PresidioRedactArgs, print_launch_status,
};
use anyhow::{Context, Result, bail};
use prodex_presidio::{
    PresidioAnalyzerResult, PresidioBlockingClient, PresidioHealth,
    ProdexPresidioRuntimeFileConfig, validate_presidio_file_config, validate_presidio_url,
};
use ratatui::layout::{Constraint, Direction, Layout};
#[cfg(test)]
use ratatui::style::Color;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use terminal_ui::print_panel;
use terminal_ui::{
    tui_border_style, tui_connected_header_block, tui_error_style, tui_primary_style,
    tui_secondary_style, tui_success_style, tui_title_style,
};

mod docker;
mod startup;

const PRODEX_PRESIDIO_FILE_NAME: &str = "presidio.toml";
const DEFAULT_PRESIDIO_ANALYZER_URL: &str = "http://localhost:5002";
const DEFAULT_PRESIDIO_ANONYMIZER_URL: &str = "http://localhost:5001";
const DEFAULT_PRESIDIO_LANGUAGE: &str = "en";
const PRESIDIO_AUTO_START_ENV: &str = "PRODEX_PRESIDIO_AUTO_START";

#[derive(Debug, Clone)]
struct PresidioPanel {
    title: String,
    fields: Vec<(String, String)>,
}

type ProdexPresidioConfig = ProdexPresidioRuntimeFileConfig;

pub(crate) fn handle_presidio(command: PresidioCommands) -> Result<()> {
    match command {
        PresidioCommands::Doctor(args) => handle_presidio_doctor(args),
        PresidioCommands::Status => handle_presidio_status(),
        PresidioCommands::Redact(args) => handle_presidio_redact(args),
        PresidioCommands::Enable(args) => handle_presidio_enable(args),
        PresidioCommands::Disable => handle_presidio_disable(),
    }
}

pub(crate) fn stored_presidio_preference() -> Result<Option<bool>> {
    let paths = AppPaths::discover()?;
    Ok(load_presidio_config(&paths)?.map(|config| config.enabled))
}

pub(crate) fn ensure_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, false)
}

pub(crate) fn ensure_required_presidio_services_for_super_launch(paths: &AppPaths) -> Result<()> {
    ensure_presidio_services_for_super_launch_inner(paths, true)
}

fn ensure_presidio_services_for_super_launch_inner(paths: &AppPaths, required: bool) -> Result<()> {
    let config = load_presidio_config(paths)?.unwrap_or_default();
    if required && !config.fail_mode.eq_ignore_ascii_case("closed") {
        anyhow::bail!(
            "--require-tool presidio requires fail_mode = \"closed\" in {}",
            presidio_config_path(paths).display()
        );
    }
    let analyzer_url = &config.analyzer_url;
    let anonymizer_url = &config.anonymizer_url;
    print_launch_status(&format!(
        "Presidio redaction enabled; checking Analyzer={} Anonymizer={} ...",
        analyzer_url, anonymizer_url
    ));
    let client = PresidioBlockingClient::from_config(&config)?;
    let analyzer = client.probe_health(analyzer_url);
    let anonymizer = client.probe_health(anonymizer_url);
    if analyzer.ok && anonymizer.ok {
        print_launch_status("Presidio services are ready.");
        return Ok(());
    }

    if let Some((message, reason)) = startup::presidio_startup_blocker(analyzer_url, anonymizer_url)
    {
        if required {
            return required_presidio_services_error(&analyzer, &anonymizer, reason);
        }
        print_launch_status(message);
        return Ok(());
    }

    let changes = match startup::start_presidio_containers(&analyzer, &anonymizer) {
        Ok(changes) => changes,
        Err(error) => {
            return startup::presidio_startup_failure(required, &config.fail_mode, error);
        }
    };

    print_launch_status("waiting for Presidio services to become ready...");
    if startup::wait_for_presidio_services(&client, analyzer_url, anonymizer_url) {
        print_launch_status("Presidio services are ready.");
        return Ok(());
    }

    let cleanup_error = startup::rollback_presidio_containers(&changes).err();
    if required {
        let reason = cleanup_error
            .map(|error| {
                format!("services did not become healthy before launch; cleanup failed: {error}")
            })
            .unwrap_or_else(|| "services did not become healthy before launch".to_string());
        return required_presidio_services_error(&analyzer, &anonymizer, &reason);
    }
    if let Some(error) = cleanup_error {
        print_launch_status(&format!(
            "Presidio startup cleanup failed; continuing with runtime fail_mode={}: {error}",
            config.fail_mode
        ));
    }
    print_launch_status(&format!(
        "Presidio services did not become healthy before launch; continuing with runtime fail_mode={}",
        config.fail_mode
    ));
    Ok(())
}

fn required_presidio_services_error(
    analyzer: &PresidioHealth,
    anonymizer: &PresidioHealth,
    reason: &str,
) -> Result<()> {
    bail!(
        "required Presidio services are not ready: Analyzer {}; Anonymizer {}; {reason}",
        presidio_health_label(analyzer),
        presidio_health_label(anonymizer),
    )
}

fn handle_presidio_doctor(args: PresidioDoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let analyzer_url = args
        .analyzer_url
        .unwrap_or_else(|| config.analyzer_url.clone());
    let anonymizer_url = args
        .anonymizer_url
        .unwrap_or_else(|| config.anonymizer_url.clone());
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let client = PresidioBlockingClient::from_config(&config)?;
    let analyzer = client.probe_health(&analyzer_url);
    let anonymizer = client.probe_health(&anonymizer_url);

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

    print_presidio_panel(
        "Presidio",
        vec![
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
            (
                "Language Mode".to_string(),
                language_mode.as_str().to_string(),
            ),
            ("Languages".to_string(), languages.join(", ")),
        ],
    )?;
    Ok(())
}

fn handle_presidio_status() -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let (languages, language_mode) = resolve_languages_and_mode(&config);

    print_presidio_panel(
        "Presidio",
        vec![
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), config.enabled.to_string()),
            ("Analyzer".to_string(), config.analyzer_url),
            ("Anonymizer".to_string(), config.anonymizer_url),
            (
                "Language Mode".to_string(),
                language_mode.as_str().to_string(),
            ),
            ("Languages".to_string(), languages.join(", ")),
            ("Fail mode".to_string(), config.fail_mode),
        ],
    )?;
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
    let mut config = load_presidio_config(&paths)?.unwrap_or_default();
    config.enabled = true;
    config.analyzer_url = args.analyzer_url;
    config.anonymizer_url = args.anonymizer_url;
    config.language = None;
    config.languages = Some(languages.clone());
    config.language_mode = language_mode;
    config.fail_mode = args.fail_mode.as_str().to_string();
    validate_presidio_file_config(&config)?;
    save_presidio_config(&paths, &config)?;
    print_presidio_panel(
        "Presidio",
        vec![
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), "true".to_string()),
            ("Analyzer".to_string(), config.analyzer_url),
            ("Anonymizer".to_string(), config.anonymizer_url),
            (
                "Language Mode".to_string(),
                language_mode.as_str().to_string(),
            ),
            ("Languages".to_string(), languages.join(", ")),
            ("Fail mode".to_string(), config.fail_mode),
        ],
    )?;
    Ok(())
}

fn handle_presidio_disable() -> Result<()> {
    let paths = AppPaths::discover()?;
    let mut config = load_presidio_config(&paths)?.unwrap_or_default();
    config.enabled = false;
    save_presidio_config(&paths, &config)?;
    print_presidio_panel(
        "Presidio",
        vec![
            (
                "Config".to_string(),
                presidio_config_path(&paths).display().to_string(),
            ),
            ("Enabled".to_string(), "false".to_string()),
        ],
    )?;
    Ok(())
}

fn print_presidio_panel(title: &str, fields: Vec<(String, String)>) -> Result<()> {
    let panel = PresidioPanel {
        title: title.to_string(),
        fields,
    };
    let height = presidio_tui_height(&panel);
    let Some(mut terminal) = crate::try_inline_stdout_terminal(height) else {
        print_panel(&panel.title, &panel.fields)?;
        return Ok(());
    };
    terminal.draw(|frame| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(frame.area());
        let header = Paragraph::new(Line::from(vec![
            Span::styled("Prodex Presidio", tui_title_style()),
            Span::raw("  "),
            Span::styled(&panel.title, tui_secondary_style()),
        ]))
        .block(tui_connected_header_block(tui_border_style()));
        frame.render_widget(header, chunks[0]);

        let body = Paragraph::new(presidio_tui_text(&panel))
            .block(
                Block::default()
                    .borders(Borders::LEFT | Borders::RIGHT | Borders::BOTTOM)
                    .border_style(tui_border_style()),
            )
            .wrap(Wrap { trim: false });
        frame.render_widget(body, chunks[1]);
    })?;
    let _ = terminal.show_cursor();
    Ok(())
}

fn presidio_tui_height(panel: &PresidioPanel) -> u16 {
    let rows = 4usize.saturating_add(panel.fields.len());
    rows.clamp(6, 24) as u16
}

fn presidio_tui_text(panel: &PresidioPanel) -> Text<'static> {
    let mut lines = Vec::with_capacity(panel.fields.len() + 1);
    lines.push(Line::from(vec![Span::styled(
        panel.title.clone(),
        tui_primary_style().add_modifier(Modifier::BOLD),
    )]));
    for (label, value) in &panel.fields {
        lines.push(Line::from(vec![
            Span::styled(format!("{label:>18} "), tui_secondary_style()),
            Span::styled(value.clone(), presidio_value_style(label, value)),
        ]));
    }
    Text::from(lines)
}

fn presidio_value_style(label: &str, value: &str) -> Style {
    let lower_label = label.to_ascii_lowercase();
    let lower_value = value.to_ascii_lowercase();
    if (lower_label.contains("health") && lower_value.starts_with("ok"))
        || (lower_label == "enabled" && lower_value == "true")
    {
        tui_success_style()
    } else if lower_label.contains("health") && lower_value.starts_with("failed")
        || lower_label == "enabled" && lower_value == "false"
    {
        tui_error_style()
    } else {
        tui_primary_style()
    }
}

fn handle_presidio_redact(args: PresidioRedactArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let config = load_presidio_config(&paths)?.unwrap_or_default();
    let analyzer_url = args
        .analyzer_url
        .unwrap_or_else(|| config.analyzer_url.clone());
    let anonymizer_url = args
        .anonymizer_url
        .unwrap_or_else(|| config.anonymizer_url.clone());
    validate_presidio_url(&analyzer_url, "analyzer_url")?;
    validate_presidio_url(&anonymizer_url, "anonymizer_url")?;

    let text = presidio_redact_input_text(args.text, args.path)?;

    let (mut languages_to_use, mut language_mode_to_use) = resolve_languages_and_mode(&config);

    // CLI args override config
    if !args.languages.is_empty() {
        languages_to_use = normalize_languages(args.languages);
    }
    if let Some(language) = args.language {
        languages_to_use = normalize_languages(vec![language]);
        language_mode_to_use = PresidioLanguageMode::Fixed;
    }
    if let Some(language_mode) = args.language_mode {
        language_mode_to_use = language_mode;
    }
    validate_language_config(&languages_to_use, language_mode_to_use)?;
    let client = PresidioBlockingClient::from_config(&config)?;

    let all_analyzer_results = match language_mode_to_use {
        PresidioLanguageMode::Fixed => {
            client.analyze(&analyzer_url, &text, &languages_to_use[0])?
        }
        PresidioLanguageMode::Auto => {
            let detected_lang = detect_presidio_language(&text, &languages_to_use)
                .unwrap_or_else(|| languages_to_use[0].clone());
            client.analyze(&analyzer_url, &text, &detected_lang)?
        }
        PresidioLanguageMode::Multi => {
            let mut all_results = Vec::new();
            for lang in &languages_to_use {
                let results = client.analyze(&analyzer_url, &text, lang)?;
                all_results.extend(results.into_iter().map(|mut r| {
                    r.language = lang.clone();
                    r
                }));
            }
            merge_presidio_analyzer_results(all_results)
        }
    };

    let anonymized = client.anonymize(&anonymizer_url, &text, all_analyzer_results)?;

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

fn presidio_auto_start_disabled() -> bool {
    env::var(PRESIDIO_AUTO_START_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn presidio_health_label(health: &PresidioHealth) -> String {
    if health.ok {
        format!("ok ({})", health.message)
    } else {
        format!("failed ({})", health.message)
    }
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
    validate_presidio_file_config(&config)?;
    Ok(Some(config))
}

fn save_presidio_config(paths: &AppPaths, config: &ProdexPresidioConfig) -> Result<()> {
    validate_presidio_file_config(config)?;
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("failed to create {}", paths.root.display()))?;
    let path = presidio_config_path(paths);
    let raw = toml::to_string_pretty(config).context("failed to render Presidio config")?;
    secret_store::write_private_file_atomic(&path, raw.as_bytes())
        .with_context(|| format!("failed to write {}", path.display()))
}

fn presidio_config_path(paths: &AppPaths) -> PathBuf {
    paths.root.join(PRODEX_PRESIDIO_FILE_NAME)
}

fn normalize_languages(langs: Vec<String>) -> Vec<String> {
    let mut normalized: Vec<String> = langs.into_iter().map(|s| s.trim().to_lowercase()).collect();
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

fn resolve_languages_and_mode(
    config: &ProdexPresidioConfig,
) -> (Vec<String>, PresidioLanguageMode) {
    let languages = config.languages.clone().unwrap_or_else(|| {
        config
            .language
            .clone()
            .map(|l| vec![l])
            .unwrap_or_else(|| vec![DEFAULT_PRESIDIO_LANGUAGE.to_string()])
    });
    (normalize_languages(languages), config.language_mode)
}

// Simple heuristic for language detection
fn detect_presidio_language(text: &str, candidates: &[String]) -> Option<String> {
    if candidates.len() == 1 {
        return Some(candidates[0].clone());
    }

    let id_keywords = [
        "yang", "dan", "di", "ke", "dari", "saya", "kami", "anda", "nomor", "nama", "alamat",
        "tanggal", "lahir", "dengan", "untuk",
    ];
    let en_keywords = [
        "the", "and", "to", "from", "my", "name", "phone", "email", "address", "with", "for",
        "birth",
    ];

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

fn merge_presidio_analyzer_results(
    mut results: Vec<PresidioAnalyzerResult>,
) -> Vec<PresidioAnalyzerResult> {
    results.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then_with(|| a.end.cmp(&b.end))
            .then_with(|| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            }) // Higher score first
            .then_with(|| b.entity_type.cmp(&a.entity_type)) // Consistent tie-breaking
    });

    let mut merged: Vec<PresidioAnalyzerResult> = Vec::new();
    for result in results {
        if let Some(result) = merge_presidio_analyzer_result(&mut merged, result) {
            merged.push(result);
        }
    }
    merged
}

fn merge_presidio_analyzer_result(
    merged: &mut [PresidioAnalyzerResult],
    result: PresidioAnalyzerResult,
) -> Option<PresidioAnalyzerResult> {
    let Some(last) = merged.last_mut() else {
        return Some(result);
    };
    if last.start == result.start
        && last.end == result.end
        && last.entity_type == result.entity_type
    {
        replace_presidio_analyzer_result_if_stronger(last, result);
        return None;
    }
    let overlaps = result.start < last.end && result.end > last.start;
    let stronger_or_longer = result.score > last.score
        || (result.score == last.score && (result.end - result.start) > (last.end - last.start));
    if !overlaps || !stronger_or_longer {
        return Some(result);
    }
    let contained = (result.start >= last.start && result.end <= last.end)
        || (last.start >= result.start && last.end <= result.end);
    if contained {
        replace_presidio_analyzer_result_if_stronger(last, result);
        return None;
    }
    if result.score > last.score {
        last.start = last.start.min(result.start);
        last.end = last.end.max(result.end);
        last.score = result.score;
        last.entity_type = result.entity_type;
        last.language = result.language;
        return None;
    }
    Some(result)
}

fn replace_presidio_analyzer_result_if_stronger(
    current: &mut PresidioAnalyzerResult,
    candidate: PresidioAnalyzerResult,
) {
    if candidate.score > current.score {
        *current = candidate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn presidio_tui_text_contains_fields() {
        let panel = PresidioPanel {
            title: "Presidio".to_string(),
            fields: vec![
                ("Enabled".to_string(), "true".to_string()),
                ("Analyzer health".to_string(), "ok (ready)".to_string()),
            ],
        };

        let text = presidio_tui_text(&panel);
        let rendered = text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");

        assert!(rendered.contains("Presidio"));
        assert!(rendered.contains("Enabled"));
        assert!(rendered.contains("ok (ready)"));
        assert_eq!(presidio_tui_height(&panel), 6);
    }

    #[test]
    fn presidio_value_style_highlights_status() {
        assert_eq!(
            presidio_value_style("Analyzer health", "ok (ready)").fg,
            Some(Color::Green)
        );
        assert_eq!(
            presidio_value_style("Enabled", "false").fg,
            Some(Color::Red)
        );
    }

    #[test]
    fn stored_presidio_url_secret_is_rejected_before_status_rendering() {
        let root = std::env::temp_dir().join(format!(
            "prodex-presidio-status-url-boundary-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(PRODEX_PRESIDIO_FILE_NAME),
            "enabled = true\n\
             analyzer_url = 'https://user:presidio-status-secret-sentinel@example.test'\n\
             anonymizer_url = 'https://example.test'\n\
             language = 'en'\n\
             language_mode = 'fixed'\n\
             fail_mode = 'open'\n",
        )
        .unwrap();
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
        };

        let error = match load_presidio_config(&paths) {
            Ok(_) => panic!("credential-bearing Presidio URL should fail before rendering"),
            Err(error) => error.to_string(),
        };

        assert!(
            error.contains("no credentials, query, or fragment"),
            "{error}"
        );
        assert!(!error.contains("secret-sentinel"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn stored_presidio_config_preserves_runtime_safety_fields() {
        let _lock = crate::TestEnvVarGuard::lock();
        let root = std::env::temp_dir().join(format!(
            "prodex-presidio-preference-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let paths = AppPaths::discover().unwrap();
        let config = ProdexPresidioConfig {
            enabled: true,
            trusted_hosts: vec!["presidio.example.com".to_string()],
            timeout_ms: 12_345,
            max_response_bytes: 2 * 1024 * 1024,
            max_concurrency: 17,
            ..Default::default()
        };
        save_presidio_config(&paths, &config).unwrap();

        let mut config = load_presidio_config(&paths).unwrap().unwrap();
        assert!(config.enabled);
        assert_eq!(stored_presidio_preference().unwrap(), Some(true));
        config.enabled = false;
        save_presidio_config(&paths, &config).unwrap();
        let saved = load_presidio_config(&paths).unwrap().unwrap();

        assert!(!saved.enabled);
        assert_eq!(stored_presidio_preference().unwrap(), Some(false));
        assert_eq!(saved.trusted_hosts, vec!["presidio.example.com"]);
        assert_eq!(saved.timeout_ms, 12_345);
        assert_eq!(saved.max_response_bytes, 2 * 1024 * 1024);
        assert_eq!(saved.max_concurrency, 17);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn required_presidio_readiness_fails_closed_when_auto_start_is_disabled() {
        let _lock = crate::TestEnvVarGuard::lock();
        let root = std::env::temp_dir().join(format!(
            "prodex-presidio-required-readiness-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let _home = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
        let _auto_start = crate::TestEnvVarGuard::set(PRESIDIO_AUTO_START_ENV, "0");
        let paths = AppPaths::discover().unwrap();
        save_presidio_config(
            &paths,
            &ProdexPresidioConfig {
                enabled: true,
                analyzer_url: "http://127.0.0.1:1".to_string(),
                anonymizer_url: "http://127.0.0.1:1".to_string(),
                fail_mode: "closed".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

        let error = ensure_required_presidio_services_for_super_launch(&paths)
            .expect_err("required Presidio must fail when services are unavailable");
        assert!(error.to_string().contains("required Presidio services"));
        assert!(error.to_string().contains("automatic startup is disabled"));

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn required_presidio_rejects_fail_open() {
        let root = std::env::temp_dir().join(format!(
            "prodex-presidio-required-fail-mode-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        ));
        fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let paths = AppPaths {
            root: root.clone(),
            state_file: root.join("state.json"),
            managed_profiles_root: root.join("profiles"),
            shared_codex_root: root.join("shared"),
            legacy_shared_codex_root: root.join("legacy-shared"),
        };
        save_presidio_config(
            &paths,
            &ProdexPresidioConfig {
                enabled: true,
                fail_mode: "open".to_string(),
                ..Default::default()
            },
        )
        .unwrap();

        let error = ensure_required_presidio_services_for_super_launch(&paths)
            .expect_err("required Presidio must reject fail-open mode");
        assert!(
            error.to_string().contains("fail_mode = \"closed\""),
            "{error}"
        );

        fs::remove_dir_all(root).unwrap();
    }
}
