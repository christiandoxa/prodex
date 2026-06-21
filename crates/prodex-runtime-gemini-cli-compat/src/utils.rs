use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::path::{Path, PathBuf};

pub(super) fn safe_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_separator = false;
    for ch in value.chars() {
        let ch = ch.to_ascii_lowercase();
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            previous_separator = false;
        } else if !previous_separator {
            slug.push('-');
            previous_separator = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "item".to_string()
    } else {
        slug
    }
}

pub(super) fn safe_key(value: &str) -> String {
    safe_slug(value).replace('-', "_")
}

pub(super) fn unique_slug(slug: &str, seen: &mut BTreeSet<String>) -> String {
    let base = safe_slug(slug);
    if seen.insert(base.clone()) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if seen.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

pub(super) fn yaml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn toml_string_literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn toml_multiline_string_literal(value: &str) -> String {
    format!(
        "\"\"\"\n{}\n\"\"\"",
        value.replace("\"\"\"", "\\\"\\\"\\\"")
    )
}

pub(super) fn first_nonempty_line(text: &str) -> Option<String> {
    strip_front_matter(text)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.trim_matches('"').to_string())
}

pub(super) fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

pub(super) fn translate_gemini_prompt_placeholders(input: &str) -> String {
    let mut output = String::new();
    let mut rest = input;
    while let Some(start) = rest.find("{{") {
        output.push_str(&rest[..start]);
        let after_start = &rest[start + 2..];
        let Some(end) = after_start.find("}}") else {
            output.push_str(&rest[start..]);
            return output;
        };
        let key = after_start[..end].trim();
        match key {
            "args" | "arguments" => output.push_str("$ARGUMENTS"),
            _ if key.starts_with("args.") => {
                output.push('$');
                output.push_str(&safe_placeholder_name(&key[5..]));
            }
            _ => {
                output.push_str("{{");
                output.push_str(key);
                output.push_str("}}");
            }
        }
        rest = &after_start[end + 2..];
    }
    output.push_str(rest);
    output
}

fn safe_placeholder_name(value: &str) -> String {
    let name = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect::<String>();
    if name.is_empty() {
        "ARGUMENTS".to_string()
    } else {
        name
    }
}

pub(super) fn strip_front_matter(text: &str) -> String {
    let mut lines = text.lines();
    if lines.next() != Some("---") {
        return text.to_string();
    }
    for line in lines.by_ref() {
        if line == "---" {
            return lines.collect::<Vec<_>>().join("\n");
        }
    }
    text.to_string()
}

pub(super) fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for path in paths {
        let key = path.to_string_lossy().to_ascii_lowercase();
        if seen.insert(key) {
            output.push(path);
        }
    }
    output
}

pub(super) fn gemini_env_bool(key: &str) -> Option<bool> {
    env::var(key)
        .ok()
        .and_then(|value| match value.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Some(true),
            "0" | "false" | "no" | "off" => Some(false),
            _ => None,
        })
}

pub(super) struct GeminiCompatVars {
    extension_path: String,
    workspace_path: String,
    separator: String,
    env: BTreeMap<String, String>,
}

impl GeminiCompatVars {
    pub(super) fn new(extension_path: &Path, cwd: Option<&Path>) -> Self {
        Self {
            extension_path: extension_path.display().to_string(),
            workspace_path: cwd
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            separator: std::path::MAIN_SEPARATOR.to_string(),
            env: BTreeMap::new(),
        }
    }

    pub(super) fn with_env(mut self, env: &BTreeMap<String, String>) -> Self {
        self.env = env.clone();
        self
    }

    pub(super) fn expand(&self, value: &str) -> String {
        let mut value = value
            .replace("${extensionPath}", &self.extension_path)
            .replace("${extension_path}", &self.extension_path)
            .replace("${workspacePath}", &self.workspace_path)
            .replace("${workspaceRoot}", &self.workspace_path)
            .replace("${workspace_path}", &self.workspace_path)
            .replace("${cwd}", &self.workspace_path)
            .replace("${/}", &self.separator)
            .replace("${pathSeparator}", &self.separator);
        for (key, replacement) in &self.env {
            value = value.replace(&format!("${{{key}}}"), replacement);
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_helpers_normalize_values_and_dedupe() {
        let mut seen = BTreeSet::new();
        assert_eq!(safe_slug(" Gemini CLI "), "gemini-cli");
        assert_eq!(safe_key("Gemini CLI"), "gemini_cli");
        assert_eq!(unique_slug("Gemini CLI", &mut seen), "gemini-cli");
        assert_eq!(unique_slug("gemini-cli", &mut seen), "gemini-cli-2");
    }

    #[test]
    fn prompt_placeholder_translation_preserves_unknown_placeholders() {
        assert_eq!(
            translate_gemini_prompt_placeholders("Use {{args.path}} {{arguments}} {{user}}"),
            "Use $PATH $ARGUMENTS {{user}}"
        );
    }
}
