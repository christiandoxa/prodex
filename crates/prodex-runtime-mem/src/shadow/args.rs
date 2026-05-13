use super::*;
use std::ffi::OsString;

pub fn runtime_mem_extract_mode_with_detail(
    args: &[OsString],
) -> (Option<RuntimeMemTranscriptMode>, Vec<OsString>) {
    let Some(first) = args.first().and_then(|arg| arg.to_str()) else {
        return (None, args.to_vec());
    };
    if first == CLAUDE_MEM_SUPER_SLIM_PREFIX {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[1..].to_vec(),
        );
    }
    if first == CLAUDE_MEM_FULL_PREFIX {
        return (Some(RuntimeMemTranscriptMode::Full), args[1..].to_vec());
    }
    if first != CLAUDE_MEM_PREFIX {
        return (None, args.to_vec());
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_SUPER_SLIM_FLAG)
    {
        return (
            Some(RuntimeMemTranscriptMode::SuperSlim),
            args[2..].to_vec(),
        );
    }
    if args
        .get(1)
        .and_then(|arg| arg.to_str())
        .is_some_and(|arg| arg == CLAUDE_MEM_FULL_FLAG)
    {
        return (Some(RuntimeMemTranscriptMode::Full), args[2..].to_vec());
    }
    (Some(RuntimeMemTranscriptMode::Slim), args[1..].to_vec())
}
