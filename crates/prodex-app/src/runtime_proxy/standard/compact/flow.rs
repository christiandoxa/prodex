pub(in super::super) enum RuntimeCompactFailureFlow {
    Retry,
    Return(tiny_http::ResponseBox),
}
