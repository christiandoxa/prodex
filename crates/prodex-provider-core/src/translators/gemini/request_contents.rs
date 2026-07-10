#[path = "request_contents/items.rs"]
mod items;
#[path = "request_contents/system_instruction.rs"]
mod system_instruction;
#[path = "request_contents/text.rs"]
mod text;

pub(crate) use self::items::gemini_contains_local_media_path;
pub(crate) use self::items::gemini_contents_from_request;
pub(super) use self::system_instruction::gemini_system_instruction_from_request;
pub(crate) use self::text::{
    gemini_contextual_user_instruction_text, gemini_is_contextual_user_fragment,
};
