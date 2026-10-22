use crate::{
    MojoError,
    json::{signed, status},
};

/// Exact terminal foreground selected by the compiled dry-run value policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeLaunchTuiColor {
    Reset,
    Red,
    Green,
    Cyan,
}

/// Whole-line theme or checked field slices, including the original colon.
#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeLaunchTuiLine<'a> {
    Primary,
    Title,
    Field {
        label: &'a str,
        value: &'a str,
        color: RuntimeLaunchTuiColor,
    },
}

#[repr(i64)]
enum TuiOperation {
    Line = 0,
    Height = 1,
}

unsafe extern "C" {
    fn prodex_runtime_launch_tui_plan_v1(
        abi: i64,
        operation: i64,
        text: u64,
        text_length: i64,
        output: u64,
        output_capacity: i64,
    ) -> i64;
}

fn tui_plan(text: &str, operation: TuiOperation) -> Result<[i64; 4], MojoError> {
    let mut output = [-1_i64; 4];
    // SAFETY: the immutable UTF-8 text and distinct four-word output outlive
    // the synchronous call; Mojo validates mode, bounds, alignment and capacity.
    status(unsafe {
        prodex_runtime_launch_tui_plan_v1(
            1,
            operation as i64,
            text.as_ptr() as u64,
            signed(text.len())?,
            output.as_mut_ptr() as u64,
            signed(output.len())?,
        )
    })?;
    Ok(output)
}

/// Select title/value precedence and borrow the validated field boundaries.
pub fn runtime_launch_tui_line_plan(text: &str) -> Result<RuntimeLaunchTuiLine<'_>, MojoError> {
    match tui_plan(text, TuiOperation::Line)? {
        [0, 0, 0, 0] => Ok(RuntimeLaunchTuiLine::Primary),
        [1, 0, 0, 0] => Ok(RuntimeLaunchTuiLine::Title),
        [2, label_end, value_start, color] => {
            let label_end = usize::try_from(label_end).map_err(|_| MojoError::InvalidOutput)?;
            let value_start = usize::try_from(value_start).map_err(|_| MojoError::InvalidOutput)?;
            if label_end == 0 || label_end > value_start {
                return Err(MojoError::InvalidOutput);
            }
            let color = match color {
                0 => RuntimeLaunchTuiColor::Reset,
                1 => RuntimeLaunchTuiColor::Red,
                2 => RuntimeLaunchTuiColor::Green,
                3 => RuntimeLaunchTuiColor::Cyan,
                _ => return Err(MojoError::InvalidOutput),
            };
            Ok(RuntimeLaunchTuiLine::Field {
                label: text.get(..label_end).ok_or(MojoError::InvalidOutput)?,
                value: text.get(value_start..).ok_or(MojoError::InvalidOutput)?,
                color,
            })
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Return the native bounded inline viewport, preserving Rust line semantics.
pub fn runtime_launch_tui_height(text: &str) -> Result<u16, MojoError> {
    match tui_plan(text, TuiOperation::Height)? {
        [height @ 8..=32, 0, 0, 0] => u16::try_from(height).map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
#[path = "report_tui_tests.rs"]
mod tests;
