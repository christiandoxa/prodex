use super::super::deepseek_rewrite::RuntimeDeepSeekConversationStore;
use super::super::local_rewrite_copilot::RuntimeCopilotOAuthPool;
use super::super::local_rewrite_gemini::RuntimeGeminiOAuthPool;
use super::super::local_rewrite_options::RuntimeLocalRewriteProviderOptions;
use super::RuntimeLocalRewriteModelMemory;
use crate::runtime_state_shared::RuntimeRotationProxyShared;
use std::ops::Deref;
use std::sync::Arc;

pub(in super::super) struct RuntimeLocalRewriteProcessServices {
    pub(in super::super) runtime_shared: RuntimeRotationProxyShared,
    pub(in super::super) mount_path: String,
    pub(in super::super) deepseek_conversations: RuntimeDeepSeekConversationStore,
    pub(in super::super) gemini_conversations: RuntimeDeepSeekConversationStore,
    pub(in super::super) gemini_oauth_pool: Option<RuntimeGeminiOAuthPool>,
    pub(in super::super) copilot_oauth_pool: Option<RuntimeCopilotOAuthPool>,
    pub(in super::super) model_memory: RuntimeLocalRewriteModelMemory,
    pub(in super::super) api_key_cursor: Arc<std::sync::atomic::AtomicUsize>,
    pub(in super::super) provider_sse_prefetch_slots: Arc<tokio::sync::Semaphore>,
    pub(in super::super) allow_local_file_access: bool,
}

#[derive(Clone)]
pub(in super::super) struct RuntimeLocalRewriteRequestContext {
    pub(in super::super) process: Arc<RuntimeLocalRewriteProcessServices>,
    pub(in super::super) upstream_base_url: String,
    pub(in super::super) provider: Arc<RuntimeLocalRewriteProviderOptions>,
}

pub(in super::super) type RuntimeLocalRewriteProxyShared = RuntimeLocalRewriteRequestContext;

impl Deref for RuntimeLocalRewriteRequestContext {
    type Target = RuntimeLocalRewriteProcessServices;

    fn deref(&self) -> &Self::Target {
        self.process.as_ref()
    }
}
