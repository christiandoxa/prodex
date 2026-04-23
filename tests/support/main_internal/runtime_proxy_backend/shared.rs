use super::*;

pub(crate) fn runtime_proxy_backend_profile_name_for_account_id(account_id: &str) -> Option<&'static str> {
    match account_id {
        "main-account" => Some("main"),
        "second-account" => Some("second"),
        "third-account" => Some("third"),
        "fourth-account" => Some("fourth"),
        "fifth-account" => Some("fifth"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_backend_account_id_for_profile_name(profile_name: &str) -> Option<&'static str> {
    match profile_name {
        "main" => Some("main-account"),
        "second" => Some("second-account"),
        "third" => Some("third-account"),
        "fourth" => Some("fourth-account"),
        "fifth" => Some("fifth-account"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_backend_initial_response_id_for_account(account_id: &str) -> Option<&'static str> {
    match account_id {
        "second-account" => Some("resp-second"),
        "third-account" => Some("resp-third"),
        "fourth-account" => Some("resp-fourth"),
        "fifth-account" => Some("resp-fifth"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_backend_response_owner_account_id(response_id: &str) -> Option<&'static str> {
    if response_id == "resp-second" || response_id.starts_with("resp-second-next") {
        Some("second-account")
    } else if response_id == "resp-third" || response_id.starts_with("resp-third-next") {
        Some("third-account")
    } else if response_id == "resp-fourth" || response_id.starts_with("resp-fourth-next") {
        Some("fourth-account")
    } else if response_id == "resp-fifth" || response_id.starts_with("resp-fifth-next") {
        Some("fifth-account")
    } else {
        None
    }
}

pub(crate) fn runtime_proxy_backend_profile_name_for_response_id(response_id: &str) -> Option<&'static str> {
    runtime_proxy_backend_response_owner_account_id(response_id)
        .and_then(runtime_proxy_backend_profile_name_for_account_id)
}

pub(crate) fn runtime_proxy_backend_initial_response_id_for_profile_name(
    profile_name: &str,
) -> Option<&'static str> {
    runtime_proxy_backend_account_id_for_profile_name(profile_name)
        .and_then(runtime_proxy_backend_initial_response_id_for_account)
}

pub(crate) fn runtime_proxy_backend_next_response_id(previous_response_id: Option<&str>) -> Option<String> {
    match previous_response_id {
        Some("resp-second") | Some("resp-third") => {
            Some(format!("{}-next", previous_response_id.unwrap()))
        }
        Some(previous_response_id)
            if previous_response_id.starts_with("resp-second-next")
                || previous_response_id.starts_with("resp-third-next") =>
        {
            Some(format!("{previous_response_id}-next"))
        }
        _ => None,
    }
}

pub(crate) fn runtime_proxy_backend_is_owned_continuation(
    account_id: &str,
    previous_response_id: Option<&str>,
) -> bool {
    previous_response_id.is_some_and(|previous_response_id| {
        runtime_proxy_backend_response_owner_account_id(previous_response_id) == Some(account_id)
            && runtime_proxy_backend_next_response_id(Some(previous_response_id)).is_some()
    })
}

pub(crate) fn runtime_proxy_backend_profile_name_for_compact_turn_state(
    turn_state: &str,
) -> Option<&'static str> {
    match turn_state {
        "compact-turn-main" => Some("main"),
        "compact-turn-second" => Some("second"),
        "compact-turn-third" => Some("third"),
        _ => None,
    }
}

pub(crate) fn runtime_proxy_response_ids_from_http_body(body: &str) -> Vec<String> {
    let response_ids = extract_runtime_response_ids_from_body_bytes(body.as_bytes());
    if !response_ids.is_empty() {
        return response_ids;
    }

    body.lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .flat_map(extract_runtime_response_ids_from_payload)
        .collect()
}

pub(crate) fn wait_for_backend_usage_accounts(
    backend: &RuntimeProxyBackend,
    expected_accounts: &[&str],
) -> Vec<String> {
    let mut expected_accounts = expected_accounts
        .iter()
        .map(|account| (*account).to_string())
        .collect::<Vec<_>>();
    expected_accounts.sort();

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let mut usage_accounts = backend.usage_accounts();
        usage_accounts.sort();
        if usage_accounts == expected_accounts || Instant::now() >= deadline {
            return usage_accounts;
        }
        thread::sleep(Duration::from_millis(10));
    }
}

pub(crate) fn sorted_backend_usage_accounts(backend: &RuntimeProxyBackend) -> Vec<String> {
    let mut usage_accounts = backend.usage_accounts();
    usage_accounts.sort();
    usage_accounts
}

pub(crate) fn closed_loopback_backend_base_url() -> String {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("closed loopback helper should bind a port");
    let addr = listener
        .local_addr()
        .expect("closed loopback helper should read local address");
    drop(listener);
    format!("http://{addr}/backend-api")
}

pub(crate) fn unresponsive_loopback_backend_listener() -> (TcpListener, String) {
    let listener =
        TcpListener::bind("127.0.0.1:0").expect("unresponsive loopback helper should bind");
    let addr = listener
        .local_addr()
        .expect("unresponsive loopback helper should read local address");
    (listener, format!("http://{addr}/backend-api"))
}
