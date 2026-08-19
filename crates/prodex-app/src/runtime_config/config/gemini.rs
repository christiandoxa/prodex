use super::super::{RuntimeConfigParser, RuntimeGeminiConfig, RuntimeGeminiExtensionSelection};
use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

pub(super) fn parse_gemini(parser: &mut RuntimeConfigParser) -> RuntimeGeminiConfig {
    let home_dir = parser.environment.nonempty_path("HOME");
    let config_dir = parser
        .environment
        .nonempty_path("GEMINI_CLI_HOME")
        .map(|path| path.join(".gemini"))
        .or_else(|| home_dir.as_ref().map(|home| home.join(".gemini")));
    let system_settings_path = parser.environment.path("GEMINI_CLI_SYSTEM_SETTINGS_PATH");
    let system_defaults_path = parser.environment.path("GEMINI_CLI_SYSTEM_DEFAULTS_PATH");
    let split_paths = |key| {
        parser
            .environment
            .get(key)
            .map(env::split_paths)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
    };
    let extension_dirs = split_paths("PRODEX_GEMINI_EXTENSION_DIRS");
    let import_paths = [
        "PRODEX_GEMINI_SESSION_FILE",
        "PRODEX_GEMINI_CHECKPOINT_FILE",
        "PRODEX_GEMINI_IMPORT_FILE",
    ]
    .into_iter()
    .flat_map(split_paths)
    .collect();
    let extension_memory_paths = split_paths("PRODEX_GEMINI_EXTENSION_MEMORY");
    let export_checkpoint_path = [
        "PRODEX_GEMINI_EXPORT_FILE",
        "PRODEX_GEMINI_CHECKPOINT_EXPORT_FILE",
    ]
    .into_iter()
    .filter_map(|key| parser.environment.get(key))
    .find(|path| !path.is_empty())
    .map(PathBuf::from);
    let tool_output_dir = parser
        .environment
        .get("PRODEX_GEMINI_TOOL_OUTPUT_DIR")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let extension_selection =
        match parser
            .compatibility_text("PRODEX_GEMINI_EXTENSIONS")
            .map(|value| {
                value
                    .split([',', ';', ' ', '\n', '\t'])
                    .filter_map(|item| {
                        let item = item.trim().to_ascii_lowercase();
                        (!item.is_empty()).then_some(item)
                    })
                    .collect::<BTreeSet<_>>()
            }) {
            None => RuntimeGeminiExtensionSelection::All,
            Some(names) if names.is_empty() => RuntimeGeminiExtensionSelection::All,
            Some(names) if names.len() == 1 && names.contains("none") => {
                RuntimeGeminiExtensionSelection::None
            }
            Some(names) => RuntimeGeminiExtensionSelection::Names(names),
        };
    let memory_files_disabled = parser.compatibility_optional_bool("PRODEX_GEMINI_DISABLE_MEMORY")
        == Some(true)
        || parser.compatibility_optional_bool("PRODEX_GEMINI_DISABLE_CONTEXT_FILES") == Some(true);
    let memory_files_default = parser
        .compatibility_optional_bool("PRODEX_GEMINI_LOAD_MEMORY")
        .or_else(|| parser.compatibility_optional_bool("PRODEX_GEMINI_MEMORY"))
        .unwrap_or(true);
    let live_url = parser
        .compatibility_text("PRODEX_GEMINI_LIVE_URL")
        .filter(|value| !value.trim().is_empty());
    let live_model = parser.compatibility_text("PRODEX_GEMINI_LIVE_MODEL");
    let sticky_fresh_oauth = parser
        .compatibility_text("PRODEX_GEMINI_STICKY_FRESH_OAUTH")
        .is_none_or(|value| {
            !matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "off" | "no"
            )
        });
    RuntimeGeminiConfig {
        home_dir,
        config_dir,
        system_settings_path,
        system_defaults_path,
        extension_dirs,
        extension_selection,
        export_checkpoint_path,
        import_paths,
        tool_output_mask_threshold: parser.compatibility_u64(
            "PRODEX_GEMINI_TOOL_OUTPUT_MASK_THRESHOLD",
            RuntimeGeminiConfig::DEFAULT_TOOL_OUTPUT_MASK_THRESHOLD as u64,
            false,
            true,
            usize::MAX as u64,
        ) as usize,
        tool_output_dir,
        memory_files_disabled,
        memory_files_default,
        extension_memory_paths,
        live_url,
        live_model,
        sticky_fresh_oauth,
    }
}
