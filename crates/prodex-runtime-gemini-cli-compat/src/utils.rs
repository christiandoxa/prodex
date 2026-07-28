use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::ffi::OsString;
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
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

pub(super) fn toml_string_literal(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
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
        let key = gemini_path_identity(&path);
        if seen.insert(key) {
            output.push(path);
        }
    }
    output
}

pub fn gemini_path_identity(path: &Path) -> OsString {
    #[cfg(windows)]
    {
        path.as_os_str().to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        path.as_os_str().to_os_string()
    }
}

pub fn gemini_extension_override_matches(rule: &str, cwd: &Path) -> Option<bool> {
    let mut rule = rule.trim();
    if rule.is_empty() {
        return None;
    }
    let disable = rule.starts_with('!');
    if disable {
        rule = &rule[1..];
    }
    let include_subdirs = rule.ends_with('*');
    if include_subdirs {
        rule = &rule[..rule.len().saturating_sub(1)];
    }
    let rule = normalize_enablement_path(rule);
    let cwd = normalize_enablement_path(&cwd.to_string_lossy());
    let matches = if include_subdirs {
        cwd.starts_with(&rule)
    } else {
        cwd == rule
    };
    matches.then_some(disable)
}

fn normalize_enablement_path(path: &str) -> String {
    let mut value = path.trim().replace('\\', "/");
    #[cfg(windows)]
    value.make_ascii_lowercase();
    if !value.starts_with('/') {
        value.insert(0, '/');
    }
    if !value.ends_with('/') {
        value.push('/');
    }
    value
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

    #[test]
    fn generated_string_literals_round_trip_control_characters() {
        let value = "C:\\temp\\queue\n\t\"quoted\"";
        assert_eq!(
            serde_json::from_str::<String>(&yaml_string_literal(value)).unwrap(),
            value
        );
        assert_eq!(
            toml_string_literal(value).parse::<toml::Value>().unwrap(),
            toml::Value::String(value.to_string())
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn path_dedupe_preserves_case_distinct_paths() {
        let paths = vec![PathBuf::from("/tmp/Foo"), PathBuf::from("/tmp/foo")];

        assert_eq!(dedupe_paths(paths.clone()), paths);
    }

    #[cfg(unix)]
    #[test]
    fn path_dedupe_preserves_non_utf8_paths() {
        use std::os::unix::ffi::OsStringExt;

        let paths = vec![
            PathBuf::from(OsString::from_vec(b"/tmp/\x80".to_vec())),
            PathBuf::from(OsString::from_vec(b"/tmp/\x81".to_vec())),
        ];

        assert_eq!(dedupe_paths(paths.clone()), paths);
    }

    #[cfg(windows)]
    #[test]
    fn path_identity_folds_windows_ascii_case() {
        assert_eq!(
            gemini_path_identity(Path::new(r"C:\\Work\\Foo")),
            gemini_path_identity(Path::new(r"c:\\work\\foo"))
        );
    }
}
