use super::*;

pub(super) struct RuntimeLaunchRequest<'a> {
    pub(super) profile: Option<&'a str>,
    pub(super) allow_auto_rotate: bool,
    pub(super) skip_quota_check: bool,
    pub(super) base_url: Option<&'a str>,
    pub(super) upstream_no_proxy: bool,
    pub(super) include_code_review: bool,
    pub(super) force_runtime_proxy: bool,
    pub(super) model_provider_override: Option<&'a str>,
}

pub(super) struct PreparedRuntimeLaunch {
    pub(super) paths: AppPaths,
    pub(super) codex_home: PathBuf,
    pub(super) managed: bool,
    pub(super) runtime_proxy: Option<RuntimeProxyEndpoint>,
}
