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

#[derive(serde::Deserialize)]
struct OptionalExecAliasPlan {
    tool: String,
    program: OptionalExecProgramPlan,
    prefix: Vec<String>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum OptionalExecProgramPlan {
    ToolPath { missing_path_message: String },
    Prodex,
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
        // Keep OS program paths outside the alias-planning text ABI.
        let Some(alias) = requested_program.strip_prefix(OPTIONAL_ALIAS_PREFIX) else {
            return Ok(ResolvedExecProgram {
                program: PathBuf::from(requested_program),
                args,
                optional_tool: None,
            });
        };

        let body = prodex_mojo_core::rich::super_expose_optional_alias_plan(alias)
            .map_err(|error| format!("optional-tool Mojo alias planning failed: {error:?}"))?;
        let plan: Option<OptionalExecAliasPlan> = serde_json::from_slice(&body)
            .map_err(|error| format!("optional-tool Mojo alias plan was invalid: {error}"))?;
        let plan = plan.ok_or_else(|| format!("unknown optional-tool exec alias: {alias}"))?;
        let id = OptionalToolId::from_str(&plan.tool)
            .map_err(|_| "optional-tool Mojo alias plan returned an unknown tool".to_string())?;
        let tool = self
            .tool(id)
            .ok_or_else(|| format!("optional tool {id} is not known"))?;
        if !tool.usable {
            return Err(format!(
                "optional tool {id} is unavailable: {}",
                tool.detail
            ));
        }

        let program = match plan.program {
            OptionalExecProgramPlan::ToolPath {
                missing_path_message,
            } => tool.path.clone().ok_or(missing_path_message)?,
            OptionalExecProgramPlan::Prodex => self.prodex_program.clone(),
        };
        let mut prefix = plan
            .prefix
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
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
                OptionalToolId::Ponytail => {
                    set_path_env(command, "PRODEX_EXPOSE_PONYTAIL_ROOT", tool.path.as_deref())
                }
                OptionalToolId::Presidio => {
                    command.env("PRODEX_EXPOSE_PRESIDIO_READY", "1");
                }
            }
        }
    }

    fn presentation<T: serde::de::DeserializeOwned>(
        &self,
        operation: prodex_mojo_core::json::SuperExposeOptionalToolOperation,
    ) -> T {
        // Rust supplies the existing enum Debug label as boundary data.
        let facts = Value::Array(
            self.tools
                .iter()
                .map(|tool| {
                    json!({
                        "id": tool.id.as_str(),
                        "kind": format!("{:?}", tool.kind),
                        "available": tool.usable,
                        "version": tool.version.as_deref(),
                    })
                })
                .collect(),
        );
        let source = crate::json_boundary::app_json_tree(&facts)
            .expect("non-secret optional-tool observations fit the JSON ABI");
        let body = prodex_mojo_core::json::super_expose_optional_tool_presentation(
            &source.nodes,
            std::str::from_utf8(&source.raw).expect("Serde emits UTF-8 JSON"),
            operation,
        )
        .expect("compiled Mojo optional-tool presentation failed");
        serde_json::from_slice(&body)
            .expect("compiled Mojo optional-tool presentation returned an invalid value")
    }

    pub(super) fn instructions(&self) -> String {
        self.presentation(prodex_mojo_core::json::SuperExposeOptionalToolOperation::Instructions)
    }

    pub(super) fn public_manifest(&self) -> Value {
        self.presentation(prodex_mojo_core::json::SuperExposeOptionalToolOperation::Manifest)
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
            OptionalToolId::Ponytail,
            Some(PathBuf::from("/validated/ponytail")),
            PathBuf::from("/prodex"),
        );
        let resolved = context
            .resolve_exec(
                "optional:ponytail",
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
                "ponytail",
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
        let _lock = crate::test_support::acquire_test_env_lock();
        let root = crate::test_support::test_temp_root().join(format!(
            "prodex-expose-discovery-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(&root).unwrap();
        // This is the absent-tool startup contract, not a benchmark of the
        // runner's Node/npm cache or a user's local redaction services. Those
        // external health probes have separate bounded-process coverage.
        std::fs::write(root.join("presidio.toml"),
            "analyzer_url = 'http://127.0.0.1:0'\nanonymizer_url = 'http://127.0.0.1:0'\ntimeout_ms = 100\n",
        ).unwrap();
        {
            let _home = crate::TestEnvVarGuard::set_home(&root);
            let _path = crate::TestEnvVarGuard::set("PATH", root.to_str().unwrap());
            let _prodex = crate::TestEnvVarGuard::set("PRODEX_HOME", root.to_str().unwrap());
            let _data = crate::TestEnvVarGuard::set("XDG_DATA_HOME", root.to_str().unwrap());
            let _optimizers = crate::TestEnvVarGuard::set(
                prodex_optional_tools::PRODEX_OPTIMIZERS_HOME_ENV,
                root.to_str().unwrap(),
            );
            let _no_proxy = crate::TestEnvVarGuard::set("NO_PROXY", "127.0.0.1");
            let _no_proxy_lower = crate::TestEnvVarGuard::set("no_proxy", "127.0.0.1");
            let started = std::time::Instant::now();
            let context = ExposeOptionalTools::discover();
            assert_eq!(context.tools.len(), OptionalToolId::ALL.len());
            assert_eq!(
                context.available_count(),
                0,
                "host tools must not enter the isolated fixture"
            );
            assert!(
                started.elapsed() < std::time::Duration::from_secs(8),
                "optional-tool discovery exceeded startup budget: {:?}",
                started.elapsed()
            );
        }
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn unavailable_alias_fails_closed() {
        let context = ExposeOptionalTools::empty_for_tests();
        let error = context.resolve_exec("optional:rtk", vec![]).unwrap_err();
        assert!(error.contains("optional tool rtk is unavailable"));
    }
}

#[cfg(test)]
#[path = "optional_tools_presentation_tests.rs"]
mod presentation_tests;

#[cfg(test)]
#[path = "optional_alias_tests.rs"]
mod alias_tests;
