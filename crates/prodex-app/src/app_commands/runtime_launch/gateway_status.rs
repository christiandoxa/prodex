use terminal_ui::print_panel;

pub(super) fn print_gateway_status(
    listen_addr: std::net::SocketAddr,
    provider_name: &str,
    auth_required: bool,
) {
    let fields = vec![
        ("URL".to_string(), format!("http://{listen_addr}")),
        ("Provider".to_string(), provider_name.to_string()),
        ("Auth required".to_string(), auth_required.to_string()),
        (
            "Endpoints".to_string(),
            "/v1/responses, /v1/chat/completions, /v1/embeddings, /v1/images/*, /v1/audio/*, /v1/batches, /v1/rerank, /v1/a2a, /v1/messages".to_string(),
        ),
        ("Models".to_string(), "/v1/models".to_string()),
    ];

    print_panel("Gateway", &fields);
}
