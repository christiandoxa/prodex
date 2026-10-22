use super::{
    SUPER_EXPOSE_ABI_VERSION, SuperExposeDispatchDecision, ensure_rich_abi, super_expose_status,
};
use crate::MojoError;

/// Transport work selected by the compiled dispatch policy.
#[derive(Debug, PartialEq, Eq)]
pub enum SuperExposeDispatchReply {
    Continue,
    Accepted {
        status: u16,
    },
    Rejected {
        status: u16,
        code: i64,
        message: String,
    },
}

/// Audit eligibility and reply contract for one validated dispatch decision.
#[derive(Debug, PartialEq, Eq)]
pub struct SuperExposeDispatchResponsePlan {
    pub audit_required: bool,
    pub reply: SuperExposeDispatchReply,
}

unsafe extern "C" {
    fn prodex_mojo_super_expose_dispatch_response_v1(
        abi_version: i64,
        decision: i64,
        output_address: u64,
        output_capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

/// Select bounded literal response text and status without host-side decision recomputation.
pub fn super_expose_dispatch_response_plan(
    decision: SuperExposeDispatchDecision,
) -> Result<SuperExposeDispatchResponsePlan, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_u8; 64];
    let mut metadata = [-1_i64; 5];
    super_expose_status(unsafe {
        prodex_mojo_super_expose_dispatch_response_v1(
            SUPER_EXPOSE_ABI_VERSION,
            decision as i64,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            metadata.as_mut_ptr() as u64,
        )
    })?;
    let [kind, audit, raw_status, code, written] = metadata;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() || !matches!(audit, 0 | 1) {
        return Err(MojoError::InvalidOutput);
    }
    let audit_required = audit == 1;
    let status = u16::try_from(raw_status).map_err(|_| MojoError::InvalidOutput)?;
    if kind != 0 && !(100..=599).contains(&status) {
        return Err(MojoError::InvalidOutput);
    }
    let reply = match kind {
        0 if audit_required && written == 0 && status == 0 && code == 0 => {
            SuperExposeDispatchReply::Continue
        }
        1 if written == 0 && code == 0 => SuperExposeDispatchReply::Accepted { status },
        2 => SuperExposeDispatchReply::Rejected {
            status,
            code,
            message: String::from_utf8(output[..written].to_vec())
                .map_err(|_| MojoError::InvalidOutput)?,
        },
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(SuperExposeDispatchResponsePlan {
        audit_required,
        reply,
    })
}

#[cfg(test)]
#[path = "dispatch_response_tests.rs"]
mod tests;
