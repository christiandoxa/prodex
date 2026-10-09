use super::{
    ABI_VERSION, CLI_DEFAULT_RUN, LOGIN_POLICY, METADATA_WORDS, MojoError, boolean,
    prodex_login_menu_policy_v1, prodex_mojo_launch_args_v1, prodex_super_choice_policy_v1, status,
    views,
};

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
