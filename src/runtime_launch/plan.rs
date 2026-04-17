use super::*;

#[derive(Debug, Clone)]
pub(crate) struct ChildProcessPlan {
    pub(crate) binary: OsString,
    pub(crate) args: Vec<OsString>,
    pub(crate) codex_home: PathBuf,
    pub(crate) extra_env: Vec<(OsString, OsString)>,
    pub(crate) removed_env: Vec<OsString>,
}

impl ChildProcessPlan {
    pub(crate) fn new(binary: OsString, codex_home: PathBuf) -> Self {
        Self {
            binary,
            args: Vec::new(),
            codex_home,
            extra_env: Vec::new(),
            removed_env: Vec::new(),
        }
    }

    pub(crate) fn with_args(mut self, args: Vec<OsString>) -> Self {
        self.args = args;
        self
    }

    pub(crate) fn with_extra_env<I, K>(mut self, extra_env: I) -> Self
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

    pub(crate) fn with_removed_env<I, K>(mut self, removed_env: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<OsString>,
    {
        self.removed_env = removed_env.into_iter().map(Into::into).collect();
        self
    }
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeLaunchPlan {
    pub(crate) child: ChildProcessPlan,
    pub(crate) cleanup_paths: Vec<PathBuf>,
}

impl RuntimeLaunchPlan {
    pub(crate) fn new(child: ChildProcessPlan) -> Self {
        Self {
            child,
            cleanup_paths: Vec::new(),
        }
    }

    pub(crate) fn with_cleanup_path(mut self, path: PathBuf) -> Self {
        self.cleanup_paths.push(path);
        self
    }
}

pub(crate) fn cleanup_runtime_launch_plan(plan: &RuntimeLaunchPlan) {
    for path in &plan.cleanup_paths {
        let _ = fs::remove_dir_all(path);
    }
}
