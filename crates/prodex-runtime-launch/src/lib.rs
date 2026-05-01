use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ChildProcessPlan {
    pub binary: OsString,
    pub args: Vec<OsString>,
    pub codex_home: PathBuf,
    pub extra_env: Vec<(OsString, OsString)>,
    pub removed_env: Vec<OsString>,
}

impl ChildProcessPlan {
    pub fn new(binary: OsString, codex_home: PathBuf) -> Self {
        Self {
            binary,
            args: Vec::new(),
            codex_home,
            extra_env: Vec::new(),
            removed_env: Vec::new(),
        }
    }

    pub fn with_args(mut self, args: Vec<OsString>) -> Self {
        self.args = args;
        self
    }

    pub fn with_extra_env<I, K>(mut self, extra_env: I) -> Self
    where
        I: IntoIterator<Item = (K, OsString)>,
        K: Into<OsString>,
    {
        self.extra_env = extra_env
            .into_iter()
            .map(|(key, value)| (key.into(), value))
            .collect();
        self
    }

    pub fn with_removed_env<I, K>(mut self, removed_env: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<OsString>,
    {
        self.removed_env = removed_env.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeLaunchPlan {
    pub child: ChildProcessPlan,
    pub cleanup_paths: Vec<PathBuf>,
}

impl RuntimeLaunchPlan {
    pub fn new(child: ChildProcessPlan) -> Self {
        Self {
            child,
            cleanup_paths: Vec::new(),
        }
    }

    pub fn with_cleanup_path(mut self, path: PathBuf) -> Self {
        self.cleanup_paths.push(path);
        self
    }
}

pub fn cleanup_runtime_launch_plan(plan: &RuntimeLaunchPlan) {
    for path in &plan.cleanup_paths {
        let _ = fs::remove_dir_all(path);
    }
}
