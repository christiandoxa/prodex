use super::{
    ABI_VERSION, CLI_DEFAULT_RUN, CODEX_COMMAND_SERVER, LOGIN_POLICY, METADATA_WORDS, MojoError,
    SUPER_COMPAT_ALIAS, SUPER_LITERAL_BOUNDARY, SUPER_POSITIONED_ALIAS, boolean,
    prodex_login_menu_policy_v1, prodex_mojo_launch_args_v1, prodex_super_choice_policy_v1, status,
    views,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperProviderAlias {
    Gemini,
    DeepSeek,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginArgumentMethod {
    ChatGpt,
    DeviceCode,
    ApiKey,
    AccessToken,
    Claude,
    Antigravity,
    Status,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperPromptChoiceKey {
    Up = 0,
    Down = 1,
    PageUp = 2,
    PageDown = 3,
    Home = 4,
    End = 5,
    Enter = 6,
    Escape = 7,
    Character = 8,
    Other = 9,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperPromptTextKey {
    Character = 0,
    Backspace = 1,
    Enter = 2,
    Escape = 3,
    Other = 4,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperPromptTextAction {
    Ignore = 0,
    Append = 1,
    Backspace = 2,
    Accept = 3,
    ReturnNone = 4,
    Cancel = 5,
}

#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SuperPromptLabelKind {
    Provider = 0,
    Value = 1,
    ProviderDefault = 2,
    CustomModel = 3,
    CustomConcurrency = 4,
    DefaultConcurrency = 5,
    PresetConcurrency = 6,
    CustomConcurrencyTitle = 7,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct SuperPromptChoiceView {
    kind: i64,
    address: u64,
    length: u64,
}

const _: () = {
    assert!(std::mem::size_of::<SuperPromptChoiceView>() == 24);
    assert!(std::mem::align_of::<SuperPromptChoiceView>() == 8);
};

unsafe extern "C" {
    fn prodex_super_prompt_policy_v1(
        abi_version: i64,
        operation: i64,
        input0: i64,
        input1: i64,
        input2: i64,
        input3: i64,
        input4: i64,
        input5: i64,
        records_address: u64,
        records_count: i64,
        text_address: u64,
        text_length: i64,
        output_address: u64,
        output_capacity: i64,
        output_count_address: u64,
    ) -> i64;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoginArgumentPlan {
    pub method: LoginArgumentMethod,
    pub removed_gemini_oauth: bool,
    pub base_url_allowed: bool,
}

fn super_choice_policy(
    operation: i64,
    input0: usize,
    input1: usize,
    input2: usize,
    input3: usize,
) -> Result<[i64; 4], MojoError> {
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_super_choice_policy_v1(
            1,
            operation,
            i64::try_from(input0).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input1).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input2).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(input3).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn super_choice_visible_range(
    selected: usize,
    len: usize,
    height: usize,
) -> Result<std::ops::Range<usize>, MojoError> {
    let output = super_choice_policy(1, selected, len, height, 0)?;
    let start = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    if start > end || end > len {
        return Err(MojoError::InvalidOutput);
    }
    Ok(start..end)
}

pub fn super_choice_initial_index(selected: usize, len: usize) -> Result<usize, MojoError> {
    let output = super_choice_policy(3, selected, len, 0, 0)?;
    usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)
}

pub fn super_choice_key_plan(
    key_tag: usize,
    selected: usize,
    len: usize,
    escape_selects_last: bool,
) -> Result<(usize, i64, Option<usize>), MojoError> {
    let output = super_choice_policy(2, key_tag, selected, len, usize::from(escape_selects_last))?;
    let next_selected = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if !(0..=2).contains(&output[1]) {
        return Err(MojoError::InvalidOutput);
    }
    let selected_action = if output[2] < 0 {
        None
    } else {
        Some(usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?)
    };
    Ok((next_selected, output[1], selected_action))
}

fn super_prompt_policy(
    operation: i64,
    inputs: [i64; 6],
    records_address: u64,
    records_count: usize,
    text: Option<&str>,
    output: &mut [i64; 4],
) -> Result<(), MojoError> {
    let [input0, input1, input2, input3, input4, input5] = inputs;
    let (text_address, text_length) = if let Some(text) = text {
        (
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } else {
        (0, 0)
    };
    let status = unsafe {
        prodex_super_prompt_policy_v1(
            1,
            operation,
            input0,
            input1,
            input2,
            input3,
            input4,
            input5,
            records_address,
            i64::try_from(records_count).map_err(|_| MojoError::InvalidInput)?,
            text_address,
            text_length,
            output.as_mut_ptr() as u64,
            0,
            0,
        )
    };
    match status {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn unicode_scalar(value: Option<char>) -> i64 {
    value.map_or(0, |value| i64::from(u32::from(value)))
}

pub fn super_prompt_choice_event_plan(
    key: SuperPromptChoiceKey,
    character: Option<char>,
    control: bool,
    selected: usize,
    len: usize,
    escape_selects_last: bool,
) -> Result<(usize, i64, Option<usize>), MojoError> {
    let mut output = [-1_i64; 4];
    super_prompt_policy(
        1,
        [
            key as i64,
            unicode_scalar(character),
            i64::from(control),
            i64::try_from(selected).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(len).map_err(|_| MojoError::InvalidInput)?,
            i64::from(escape_selects_last),
        ],
        0,
        0,
        None,
        &mut output,
    )?;
    let next_selected = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if !(0..=2).contains(&output[1]) {
        return Err(MojoError::InvalidOutput);
    }
    let choice = if output[2] < 0 {
        None
    } else {
        Some(usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?)
    };
    Ok((next_selected, output[1], choice))
}

pub fn super_prompt_text_event_plan(
    key: SuperPromptTextKey,
    character: Option<char>,
    control: bool,
    current_characters: usize,
    escape_returns_none: bool,
) -> Result<(SuperPromptTextAction, Option<char>, bool), MojoError> {
    let mut output = [-1_i64; 4];
    super_prompt_policy(
        2,
        [
            key as i64,
            unicode_scalar(character),
            i64::from(control),
            i64::try_from(current_characters).map_err(|_| MojoError::InvalidInput)?,
            i64::from(escape_returns_none),
            0,
        ],
        0,
        0,
        None,
        &mut output,
    )?;
    let action = match output[0] {
        0 => SuperPromptTextAction::Ignore,
        1 => SuperPromptTextAction::Append,
        2 => SuperPromptTextAction::Backspace,
        3 => SuperPromptTextAction::Accept,
        4 => SuperPromptTextAction::ReturnNone,
        5 => SuperPromptTextAction::Cancel,
        _ => return Err(MojoError::InvalidOutput),
    };
    let character = if action == SuperPromptTextAction::Append {
        Some(
            char::from_u32(u32::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?)
                .ok_or(MojoError::InvalidOutput)?,
        )
    } else {
        None
    };
    let clear_validation = match output[2] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok((action, character, clear_validation))
}

pub fn super_prompt_text_prefix_length(value: &str) -> Result<usize, MojoError> {
    if value.len() > 65_536 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 4];
    super_prompt_policy(3, [0, 0, 0, 0, 0, 0], 0, 0, Some(value), &mut output)?;
    usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)
}

pub fn super_prompt_choice_index(
    choices: &[Option<&str>],
    current: Option<&str>,
    case_insensitive: bool,
    fallback_index: usize,
) -> Result<usize, MojoError> {
    if choices.is_empty() || choices.len() > 1_024 {
        return Err(MojoError::InvalidInput);
    }
    let records = choices
        .iter()
        .map(|choice| {
            choice.map_or(
                SuperPromptChoiceView {
                    kind: 0,
                    address: 0,
                    length: 0,
                },
                |value| SuperPromptChoiceView {
                    kind: 1,
                    address: value.as_ptr() as usize as u64,
                    length: value.len() as u64,
                },
            )
        })
        .collect::<Vec<_>>();
    let mut output = [-1_i64; 4];
    super_prompt_policy(
        4,
        [
            i64::from(case_insensitive),
            i64::try_from(fallback_index).map_err(|_| MojoError::InvalidInput)?,
            i64::from(current.is_some()),
            0,
            0,
            0,
        ],
        records.as_ptr() as usize as u64,
        records.len(),
        current,
        &mut output,
    )?;
    usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)
}

pub fn super_prompt_value_index(
    values: &[i64],
    wanted: i64,
    fallback_index: usize,
) -> Result<usize, MojoError> {
    if values.is_empty() || values.len() > 1_024 {
        return Err(MojoError::InvalidInput);
    }
    let mut output = [-1_i64; 4];
    super_prompt_policy(
        5,
        [
            wanted,
            i64::try_from(fallback_index).map_err(|_| MojoError::InvalidInput)?,
            0,
            0,
            0,
            0,
        ],
        values.as_ptr() as usize as u64,
        values.len(),
        None,
        &mut output,
    )?;
    usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)
}

pub fn super_prompt_format_label(
    kind: SuperPromptLabelKind,
    value: u16,
    text: Option<&str>,
    flag: bool,
) -> Result<String, MojoError> {
    if text.is_some_and(|text| text.len() > 65_536) {
        return Err(MojoError::InvalidInput);
    }
    let capacity = if let Some(text) = text {
        text.len().checked_add(32).ok_or(MojoError::InvalidInput)?
    } else {
        64
    };
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let (text_address, text_length) = if let Some(text) = text {
        (
            text.as_ptr() as usize as u64,
            i64::try_from(text.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    } else {
        (0, 0)
    };
    let status = unsafe {
        prodex_super_prompt_policy_v1(
            1,
            6,
            kind as i64,
            i64::from(value),
            i64::from(flag),
            0,
            0,
            0,
            0,
            0,
            text_address,
            text_length,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 | 2 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

fn login_menu_policy(operation: i64, input: [usize; 5]) -> Result<[i64; 4], MojoError> {
    let [input0, input1, input2, input3, input4] =
        input.map(|value| i64::try_from(value).map_err(|_| MojoError::InvalidInput));
    let mut output = [-1_i64; 4];
    let status = unsafe {
        prodex_login_menu_policy_v1(
            1,
            operation,
            input0?,
            input1?,
            input2?,
            input3?,
            input4?,
            output.as_mut_ptr() as u64,
        )
    };
    match status {
        0 => Ok(output),
        1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn login_menu_layout(rows: usize, entry_count: usize) -> Result<(usize, bool), MojoError> {
    let output = login_menu_policy(1, [rows, entry_count, 0, 0, 0])?;
    Ok((
        usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?,
        boolean(output[1])?,
    ))
}

pub fn login_menu_window_offset(
    selected: usize,
    current_offset: usize,
    visible_items: usize,
    entry_count: usize,
) -> Result<usize, MojoError> {
    usize::try_from(
        login_menu_policy(2, [selected, current_offset, visible_items, entry_count, 0])?[0],
    )
    .map_err(|_| MojoError::InvalidOutput)
}

pub fn login_menu_key_plan(
    key_tag: usize,
    digit: usize,
    selected: usize,
    visible_items: usize,
    entry_count: usize,
) -> Result<(usize, i64, Option<usize>), MojoError> {
    let output = login_menu_policy(3, [key_tag, digit, selected, visible_items, entry_count])?;
    let next_selected = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    if !(0..=2).contains(&output[1]) {
        return Err(MojoError::InvalidOutput);
    }
    let action_index = if output[2] < 0 {
        None
    } else {
        Some(usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?)
    };
    Ok((next_selected, output[1], action_index))
}

pub fn login_argument_plan(arguments: &[Option<&str>]) -> Result<LoginArgumentPlan, MojoError> {
    let input = views(arguments)?;
    let mut meta = [0_i64; METADATA_WORDS];
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            LOGIN_POLICY,
            0,
            input.as_ptr() as u64,
            input.len() as i64,
            0,
            0,
            0,
            meta.as_mut_ptr() as u64,
        )
    })?;
    let method = match meta[0] {
        0 => LoginArgumentMethod::ChatGpt,
        1 => LoginArgumentMethod::DeviceCode,
        2 => LoginArgumentMethod::ApiKey,
        3 => LoginArgumentMethod::AccessToken,
        4 => LoginArgumentMethod::Claude,
        5 => LoginArgumentMethod::Antigravity,
        6 => LoginArgumentMethod::Status,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(LoginArgumentPlan {
        method,
        removed_gemini_oauth: boolean(meta[1])?,
        base_url_allowed: boolean(meta[2])?,
    })
}

/// Decide whether an unrecognized top-level command should default to `run`.
pub fn default_cli_invocation_to_run(arguments: &[Option<&str>]) -> Result<bool, MojoError> {
    let input = views(arguments)?;
    let mut meta = [0_i64; METADATA_WORDS];
    // SAFETY: input and metadata live through the synchronous call. Operation
    // 13 only reads the argument views and writes metadata[0].
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            CLI_DEFAULT_RUN,
            0,
            input.as_ptr() as u64,
            input.len() as i64,
            0,
            0,
            0,
            meta.as_mut_ptr() as u64,
        )
    })?;
    boolean(meta[0])
}

fn launch_scalar_policy(
    operation: i64,
    full_access: i64,
    arguments: &[Option<&str>],
) -> Result<i64, MojoError> {
    let input = views(arguments)?;
    let mut meta = [0_i64; METADATA_WORDS];
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            operation,
            full_access,
            input.as_ptr() as u64,
            input.len() as i64,
            0,
            0,
            0,
            meta.as_mut_ptr() as u64,
        )
    })?;
    Ok(meta[0])
}

fn decode_super_provider_alias(value: i64) -> Result<Option<SuperProviderAlias>, MojoError> {
    match value {
        0 => Ok(None),
        1 => Ok(Some(SuperProviderAlias::Gemini)),
        2 => Ok(Some(SuperProviderAlias::DeepSeek)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classifies the legacy `prodex s|super gemini|deepseek` shorthand.
pub fn super_compat_provider_alias(
    arguments: &[Option<&str>],
) -> Result<Option<SuperProviderAlias>, MojoError> {
    decode_super_provider_alias(launch_scalar_policy(SUPER_COMPAT_ALIAS, 0, arguments)?)
}

/// Finds the literal `--` boundary for a top-level Super invocation.
pub fn super_literal_boundary(arguments: &[Option<&str>]) -> Result<Option<usize>, MojoError> {
    let boundary = launch_scalar_policy(SUPER_LITERAL_BOUNDARY, 0, arguments)?;
    if boundary < 0 {
        return Ok(None);
    }
    let boundary = usize::try_from(boundary).map_err(|_| MojoError::InvalidOutput)?;
    if boundary >= arguments.len() {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(boundary))
}

/// Classifies a positioned provider alias after Clap has parsed Super fields.
pub fn positioned_super_provider_alias(
    alias: Option<&str>,
    provider_present: bool,
    url_present: bool,
) -> Result<Option<SuperProviderAlias>, MojoError> {
    let flags = i64::from(provider_present) | (i64::from(url_present) << 1);
    decode_super_provider_alias(launch_scalar_policy(
        SUPER_POSITIONED_ALIAS,
        flags,
        &[alias],
    )?)
}

/// Classifies Codex command-server launches from their first argument only.
pub fn codex_command_server_subcommand(argument: Option<&str>) -> Result<bool, MojoError> {
    boolean(launch_scalar_policy(CODEX_COMMAND_SERVER, 0, &[argument])?)
}

#[cfg(test)]
mod login_policy_tests {
    use super::*;

    #[test]
    fn login_argument_policy_preserves_method_precedence() {
        let plan = login_argument_plan(&[
            Some("--device-auth"),
            Some("--with-claude"),
            Some("--with-api-key"),
            Some("--with-google"),
        ])
        .unwrap();
        assert_eq!(plan.method, LoginArgumentMethod::ApiKey);
        assert!(plan.removed_gemini_oauth);
        assert!(plan.base_url_allowed);

        let status = login_argument_plan(&[Some("status"), Some("--with-api-key")]).unwrap();
        assert_eq!(status.method, LoginArgumentMethod::Status);
        assert!(!status.removed_gemini_oauth);
        assert!(!status.base_url_allowed);

        let opaque = login_argument_plan(&[None, Some("--device-auth")]).unwrap();
        assert_eq!(opaque.method, LoginArgumentMethod::DeviceCode);
        assert!(!opaque.base_url_allowed);
    }
}

#[cfg(test)]
mod super_prompt_abi_tests {
    use super::*;

    #[test]
    fn super_prompt_abi_rejects_bad_version_utf8_and_output_capacity() {
        let mut numeric_output = [0_i64; 4];
        let bad_version = unsafe {
            prodex_super_prompt_policy_v1(
                0,
                1,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                numeric_output.as_mut_ptr() as usize as u64,
                0,
                0,
            )
        };
        assert_eq!(bad_version, 4);

        let malformed = [0xff_u8];
        let invalid_utf8 = unsafe {
            prodex_super_prompt_policy_v1(
                1,
                3,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                malformed.as_ptr() as usize as u64,
                1,
                numeric_output.as_mut_ptr() as usize as u64,
                0,
                0,
            )
        };
        assert_eq!(invalid_utf8, 1);

        let mut byte_output = [0_u8; 1];
        let mut written = -1_i64;
        let insufficient_capacity = unsafe {
            prodex_super_prompt_policy_v1(
                1,
                6,
                SuperPromptLabelKind::PresetConcurrency as i64,
                64,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                byte_output.as_mut_ptr() as usize as u64,
                1,
                (&mut written as *mut i64) as usize as u64,
            )
        };
        assert_eq!(insufficient_capacity, 3);
        assert_eq!(written, 1);
    }
}
