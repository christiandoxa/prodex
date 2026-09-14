pub const APPLICATION_DATA_PLANE_ABI_VERSION: i64 = 1;

pub const APPLICATION_CAPABILITY_RESPONSES_API: u64 = 1 << 0;
pub const APPLICATION_CAPABILITY_STREAMING: u64 = 1 << 1;
pub const APPLICATION_CAPABILITY_TOOLS: u64 = 1 << 2;
pub const APPLICATION_CAPABILITY_VISION: u64 = 1 << 3;
pub const APPLICATION_CAPABILITY_JSON_MODE: u64 = 1 << 4;
pub const APPLICATION_CAPABILITY_REMOTE_COMPACT: u64 = 1 << 5;
pub const APPLICATION_CAPABILITY_WEBSOCKET: u64 = 1 << 6;

pub const APPLICATION_MODALITY_TEXT: u64 = 1 << 0;
pub const APPLICATION_MODALITY_IMAGE: u64 = 1 << 1;
pub const APPLICATION_MODALITY_AUDIO: u64 = 1 << 2;
pub const APPLICATION_MODALITY_FILE: u64 = 1 << 4;

#[repr(i64)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRouteKind {
    Responses = 0,
    Compact = 1,
    WebSocket = 2,
    Quota = 3,
    ChatCompletions = 4,
    Embeddings = 5,
    ImagesGenerations = 6,
    ImagesEdits = 7,
    ImagesVariations = 8,
    AudioSpeech = 9,
    AudioTranscriptions = 10,
    AudioTranslations = 11,
    Batches = 12,
    Batch = 13,
    Rerank = 14,
    A2a = 15,
    Messages = 16,
    Models = 17,
    Model = 18,
    ControlPlane = 19,
    HealthLive = 20,
    HealthReady = 21,
    HealthStartup = 22,
    Unknown = 23,
}
