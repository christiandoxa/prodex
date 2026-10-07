use super::*;

pub(super) fn runtime_kiro_messages_streaming_upstream_result(
    context: RuntimeKiroRequestContext<'_>,
) -> Result<RuntimeLocalRewriteUpstreamResult> {
    let profile_name = context.auth.profile_name.clone();
    let buffered = runtime_kiro_buffered_upstream_result(context)?;
    let RuntimeLocalRewriteUpstreamResponse::Buffered(parts) = buffered.response else {
        anyhow::bail!("Kiro Messages compatibility expected a buffered ACP result");
    };
    let message: Value = serde_json::from_slice(&parts.body)
        .context("failed to parse Kiro Anthropic Messages response JSON")?;
    let body = prodex_provider_core::kiro_provider_core_anthropic_sse_body(&message)
        .map_err(|error| anyhow::anyhow!("Mojo Kiro Anthropic SSE rendering failed: {error:?}"))?;
    Ok(RuntimeLocalRewriteUpstreamResult {
        response: RuntimeLocalRewriteUpstreamResponse::Streaming(
            RuntimeLocalRewriteStreamingResponse {
                status: 200,
                headers: vec![(
                    "content-type".to_string(),
                    "text/event-stream; charset=utf-8".to_string(),
                )],
                body: Box::new(Cursor::new(body)),
                profile_name,
                accepted_binding_recorder: None,
                accepted_binding: None,
            },
        ),
        gemini_context: None,
        copilot_context: None,
    })
}
