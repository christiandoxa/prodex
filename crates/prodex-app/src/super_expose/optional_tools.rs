use prodex_optional_tools::{
    OptionalToolId, OptionalToolSet, ToolHealth, ToolHealthStatus, ToolKind,
    optional_tool_descriptor, resolve_optional_tools_for_launch,
};
use serde_json::{Value, json};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

const OPTIONAL_ALIAS_PREFIX: &str = "optional:";

#[derive(Debug, Clone)]
struct ExposeOptionalTool {
    id: OptionalToolId,
    kind: ToolKind,
    usable: bool,
    path: Option<PathBuf>,
    version: Option<String>,
    detail: String,
}

#[derive(Debug, Clone)]
pub(super) struct ExposeOptionalTools {
    tools: Vec<ExposeOptionalTool>,
    prodex_program: PathBuf,
}

#[derive(Debug)]
pub(super) struct ResolvedExecProgram {
    pub(super) program: PathBuf,
    pub(super) args: Vec<OsString>,
    pub(super) optional_tool: Option<OptionalToolId>,
}

impl ExposeOptionalTools {
    pub(super) fn discover() -> Self {
        let prodex_program = std::env::current_exe()
            .ok()
            .and_then(|path| path.canonicalize().ok())
            .unwrap_or_else(|| PathBuf::from("prodex"));
        let selected = OptionalToolId::ALL
            .into_iter()
            .filter(|id| *id != OptionalToolId::Presidio)
            .collect::<OptionalToolSet>();
        let required = OptionalToolSet::default();
        let plan = resolve_optional_tools_for_launch(&selected, &required);

        let tools = OptionalToolId::ALL
            .into_iter()
            .map(|id| {
                let descriptor = optional_tool_descriptor(id);
                if id == OptionalToolId::Presidio {
                    let (usable, detail) = crate::app_commands::presidio_expose_health();
                    return ExposeOptionalTool {
                        id,
                        kind: descriptor.kind,
                        usable,
                        path: None,
                        version: None,
                        detail,
                    };
                }
                if let Some(activation) = plan
                    .activations
                    .iter()
                    .find(|activation| activation.tool.descriptor.id == id)
                {
                    return ExposeOptionalTool {
                        id,
                        kind: descriptor.kind,
                        usable: true,
                        path: activation.tool.path.clone(),
                        version: activation.tool.version.clone(),
                        detail: "installed and validated for Super launch".to_string(),
                    };
                }
                if let Some(health) = plan.unavailable.iter().find(|health| health.id == id) {
                    return Self::tool_from_health(id, descriptor.kind, health.clone());
                }
                ExposeOptionalTool {
                    id,
                    kind: descriptor.kind,
                    usable: false,
                    path: None,
                    version: None,
                    detail: "optional tool was not resolved for Super launch".to_string(),
                }
            })
            .collect();

        Self {
            tools,
            prodex_program,
        }
    }

    fn tool_from_health(
        id: OptionalToolId,
        kind: ToolKind,
        health: ToolHealth,
    ) -> ExposeOptionalTool {
        ExposeOptionalTool {
            id,
            kind,
            usable: health.status == ToolHealthStatus::Installed && health.can_activate,
            path: health.path,
            version: health.version,
            detail: health.detail,
        }
    }

    #[cfg(test)]
    pub(super) fn empty_for_tests() -> Self {
        Self {
            tools: OptionalToolId::ALL
                .into_iter()
                .map(|id| ExposeOptionalTool {
                    id,
                    kind: optional_tool_descriptor(id).kind,
                    usable: false,
                    path: None,
                    version: None,
                    detail: "unavailable in test fixture".to_string(),
                })
                .collect(),
            prodex_program: PathBuf::from("prodex"),
        }
    }

    #[cfg(test)]
    pub(super) fn with_tool_for_tests(
        id: OptionalToolId,
        path: Option<PathBuf>,
        prodex_program: PathBuf,
    ) -> Self {
        let mut value = Self::empty_for_tests();
        value.prodex_program = prodex_program;
        if let Some(tool) = value.tools.iter_mut().find(|tool| tool.id == id) {
            tool.usable = true;
            tool.path = path;
            tool.version = Some("test".to_string());
            tool.detail = "installed and validated".to_string();
        }
        value
    }

    fn tool(&self, id: OptionalToolId) -> Option<&ExposeOptionalTool> {
        self.tools.iter().find(|tool| tool.id == id)
    }

    pub(super) fn available_ids(&self) -> Vec<&'static str> {
        self.tools
            .iter()
            .filter(|tool| tool.usable)
            .map(|tool| tool.id.as_str())
            .collect()
    }

    pub(super) fn available_count(&self) -> usize {
        self.tools.iter().filter(|tool| tool.usable).count()
    }

    pub(super) fn resolve_exec(
        &self,
        requested_program: &str,
        args: Vec<OsString>,
    ) -> Result<ResolvedExecProgram, String> {
        let Some(alias) = requested_program.strip_prefix(OPTIONAL_ALIAS_PREFIX) else {
            return Ok(ResolvedExecProgram {
                program: PathBuf::from(requested_program),
                args,
                optional_tool: None,
            });
        };

        let (id, integrated_playwright) = if alias == "playwright" {
            (OptionalToolId::PlaywrightMcp, true)
        } else {
            (
                OptionalToolId::from_str(alias)
                    .map_err(|_| format!("unknown optional-tool exec alias: {alias}"))?,
                false,
            )
        };
        let tool = self
            .tool(id)
            .ok_or_else(|| format!("optional tool {id} is not known"))?;
        if !tool.usable {
            return Err(format!(
                "optional tool {id} is unavailable: {}",
                tool.detail
            ));
        }

        let (program, mut prefix) = match id {
            OptionalToolId::Rtk | OptionalToolId::CodebaseMemoryMcp => (
                tool.path.clone().ok_or_else(|| {
                    format!("validated optional tool {id} has no executable path")
                })?,
                Vec::new(),
            ),
            OptionalToolId::PlaywrightMcp if !integrated_playwright => (
                tool.path.clone().ok_or_else(|| {
                    "validated Playwright MCP has no npx executable path".to_string()
                })?,
                vec![
                    OsString::from("--no-install"),
                    OsString::from("@playwright/mcp"),
                ],
            ),
            OptionalToolId::Caveman | OptionalToolId::Ponytail | OptionalToolId::PlaywrightMcp => (
                self.prodex_program.clone(),
                vec![
                    OsString::from("super"),
                    OsString::from("--no-sub-agent"),
                    OsString::from("--no-presidio"),
                    OsString::from("--tool"),
                    OsString::from(match id {
                        OptionalToolId::Caveman => "caveman",
                        OptionalToolId::Ponytail => "ponytail",
                        OptionalToolId::PlaywrightMcp => "playwright",
                        _ => unreachable!(),
                    }),
                ],
            ),
            OptionalToolId::Presidio => (
                self.prodex_program.clone(),
                vec![
                    OsString::from("super"),
                    OsString::from("--no-sub-agent"),
                    OsString::from("--presidio"),
                ],
            ),
        };
        prefix.extend(args);
        Ok(ResolvedExecProgram {
            program,
            args: prefix,
            optional_tool: Some(id),
        })
    }

    pub(super) fn apply_environment(&self, command: &mut Command, requested_path: Option<&OsStr>) {
        let mut dirs = Vec::<PathBuf>::new();
        for id in [
            OptionalToolId::Rtk,
            OptionalToolId::CodebaseMemoryMcp,
            OptionalToolId::PlaywrightMcp,
        ] {
            let Some(path) = self
                .tool(id)
                .filter(|tool| tool.usable)
                .and_then(|tool| tool.path.as_deref())
            else {
                continue;
            };
            if let Some(parent) = path.parent()
                && !dirs.iter().any(|existing| existing == parent)
            {
                dirs.push(parent.to_path_buf());
            }
        }
        let inherited_path = std::env::var_os("PATH");
        if let Some(path) = requested_path.or(inherited_path.as_deref()) {
            for dir in std::env::split_paths(path) {
                if !dirs.iter().any(|existing| existing == &dir) {
                    dirs.push(dir);
                }
            }
        }
        if let Ok(path) = std::env::join_paths(dirs) {
            command.env("PATH", path);
        }

        command.env(
            "PRODEX_EXPOSE_OPTIONAL_TOOLS",
            self.available_ids().join(","),
        );
        for tool in self.tools.iter().filter(|tool| tool.usable) {
            match tool.id {
                OptionalToolId::Rtk => {
                    set_path_env(command, "PRODEX_EXPOSE_RTK_BIN", tool.path.as_deref())
                }
                OptionalToolId::CodebaseMemoryMcp => set_path_env(
                    command,
                    "PRODEX_EXPOSE_CODEBASE_MEMORY_BIN",
                    tool.path.as_deref(),
                ),
                OptionalToolId::PlaywrightMcp => set_path_env(
                    command,
                    "PRODEX_EXPOSE_PLAYWRIGHT_NPX",
                    tool.path.as_deref(),
                ),
                OptionalToolId::Caveman => {
                    set_path_env(command, "PRODEX_EXPOSE_CAVEMAN_ROOT", tool.path.as_deref())
                }
                OptionalToolId::Ponytail => {
                    set_path_env(command, "PRODEX_EXPOSE_PONYTAIL_ROOT", tool.path.as_deref())
                }
                OptionalToolId::Presidio => {
                    command.env("PRODEX_EXPOSE_PRESIDIO_READY", "1");
                }
            }
        }
    }

    pub(super) fn tool_description_suffix(&self) -> String {
        let aliases = self
            .tools
            .iter()
            .filter(|tool| tool.usable)
            .flat_map(|tool| {
                let mut values = vec![format!("{OPTIONAL_ALIAS_PREFIX}{}", tool.id.as_str())];
                if tool.id == OptionalToolId::PlaywrightMcp {
                    values.push("optional:playwright".to_string());
                }
                values
            })
            .collect::<Vec<_>>();
        if aliases.is_empty() {
            " No validated Prodex optional-tool aliases are active for this endpoint.".to_string()
        } else {
            format!(
                " Validated optional-tool aliases available at endpoint start: {}. Prefer these aliases over guessing PATH locations.",
                aliases.join(", ")
            )
        }
    }

    pub(super) fn instructions(&self) -> String {
        let mut lines = vec![
            "Optional tools are validated once when this expose endpoint starts. Use only entries marked available; unavailable or incompatible tools are not active.".to_string(),
            "Use program optional:<name> for validated aliases. Normal program names and paths keep their existing behavior.".to_string(),
        ];
        for tool in &self.tools {
            if !tool.usable {
                lines.push(format!("- {}: unavailable.", tool.id));
                continue;
            }
            let version = tool
                .version
                .as_deref()
                .map(|version| format!(" ({version})"))
                .unwrap_or_default();
            let invocation = match tool.id {
                OptionalToolId::Rtk => "program optional:rtk; use it for noisy shell output",
                OptionalToolId::CodebaseMemoryMcp => {
                    "program optional:codebase-memory-mcp; one-shot structural navigation is available with args cli --json <tool> ..."
                }
                OptionalToolId::PlaywrightMcp => {
                    "program optional:playwright-mcp starts the validated MCP package via npx; program optional:playwright launches noninteractive Prodex Super with Playwright enabled"
                }
                OptionalToolId::Caveman => {
                    "program optional:caveman launches noninteractive Prodex Super with the validated Caveman plugin enabled"
                }
                OptionalToolId::Ponytail => {
                    "program optional:ponytail launches noninteractive Prodex Super with the validated Ponytail plugin enabled"
                }
                OptionalToolId::Presidio => {
                    "program optional:presidio launches noninteractive Prodex Super with Presidio enabled; this alias exists only while Analyzer and Anonymizer are healthy"
                }
            };
            lines.push(format!(
                "- {}{} [{:?}]: {}.",
                tool.id, version, tool.kind, invocation
            ));
        }
        lines.join("\n")
    }

    pub(super) fn public_manifest(&self) -> Value {
        Value::Array(
            self.tools
                .iter()
                .map(|tool| {
                    json!({
                        "id": tool.id.as_str(),
                        "kind": format!("{:?}", tool.kind),
                        "available": tool.usable,
                        "version": tool.version.as_deref(),
                        "alias": format!("{OPTIONAL_ALIAS_PREFIX}{}", tool.id.as_str()),
                    })
                })
                .collect(),
        )
    }
}

fn set_path_env(command: &mut Command, key: &str, value: Option<&Path>) {
    if let Some(value) = value {
        command.env(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_command_alias_resolves_to_validated_path() {
        let context = ExposeOptionalTools::with_tool_for_tests(
            OptionalToolId::Rtk,
            Some(PathBuf::from("/validated/rtk")),
            PathBuf::from("/prodex"),
        );
        let resolved = context
            .resolve_exec("optional:rtk", vec![OsString::from("gain")])
            .unwrap();
        assert_eq!(resolved.program, PathBuf::from("/validated/rtk"));
        assert_eq!(resolved.args, vec![OsString::from("gain")]);
        assert_eq!(resolved.optional_tool, Some(OptionalToolId::Rtk));
    }

    #[test]
    fn plugin_alias_uses_prodex_super_activation() {
        let context = ExposeOptionalTools::with_tool_for_tests(
            OptionalToolId::Caveman,
            Some(PathBuf::from("/validated/caveman")),
            PathBuf::from("/prodex"),
        );
        let resolved = context
            .resolve_exec(
                "optional:caveman",
                vec![OsString::from("exec"), OsString::from("task")],
            )
            .unwrap();
        assert_eq!(resolved.program, PathBuf::from("/prodex"));
        assert_eq!(
            resolved.args,
            vec![
                "super",
                "--no-sub-agent",
                "--no-presidio",
                "--tool",
                "caveman",
                "exec",
                "task",
            ]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn discovery_completes_within_expose_startup_budget() {
        let started = std::time::Instant::now();
        let context = ExposeOptionalTools::discover();
        assert_eq!(context.tools.len(), OptionalToolId::ALL.len());
        assert!(
            started.elapsed() < std::time::Duration::from_secs(8),
            "optional-tool discovery exceeded startup budget: {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn unavailable_alias_fails_closed() {
        let context = ExposeOptionalTools::empty_for_tests();
        let error = context.resolve_exec("optional:rtk", vec![]).unwrap_err();
        assert!(error.contains("optional tool rtk is unavailable"));
    }
}
