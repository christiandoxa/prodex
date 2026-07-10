use serde_json::Value;

mod rtk;
mod rtk_noisy;

pub(crate) use self::rtk::{
    chat_compatible_rtk_wrapped_tool_arguments, rtk_prefixed_noisy_shell_command,
};

pub fn wrap_json_string_arg_with(
    arguments: &str,
    keys: &[&str],
    rewriter: impl Fn(&str) -> Option<String>,
) -> String {
    let Ok(mut value) = serde_json::from_str::<Value>(arguments) else {
        return arguments.to_string();
    };
    let Some(object) = value.as_object_mut() else {
        return arguments.to_string();
    };
    for key in keys {
        let Some(wrapped) = object.get(*key).and_then(Value::as_str).and_then(&rewriter) else {
            continue;
        };
        object.insert((*key).to_string(), Value::String(wrapped));
        return serde_json::to_string(&value).unwrap_or_else(|_| arguments.to_string());
    }
    arguments.to_string()
}

pub fn prefix_command_with_rtk(command: &str) -> Option<String> {
    if command.trim().is_empty() || command.trim_start().starts_with("rtk ") {
        None
    } else {
        Some(format!("rtk {command}"))
    }
}
