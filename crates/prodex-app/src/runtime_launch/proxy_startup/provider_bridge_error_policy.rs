use super::{RuntimeProviderBridgeKind, RuntimeProviderErrorClass};
use prodex_provider_core::{ProviderErrorClass, ProviderErrorClassification, provider_translator};

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_error_class(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> RuntimeProviderErrorClass {
    runtime_provider_error_classification(kind, status, body).0
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_error_cooldown_ms(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> u64 {
    runtime_provider_error_classification(kind, status, body)
        .1
        .cooldown_ms
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_should_retry_with_next_model(
    class: RuntimeProviderErrorClass,
) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
            | RuntimeProviderErrorClass::NotFound
    )
}

pub(in crate::runtime_launch::proxy_startup) fn runtime_provider_should_rotate_auth_after_response(
    class: RuntimeProviderErrorClass,
) -> bool {
    matches!(
        class,
        RuntimeProviderErrorClass::Auth
            | RuntimeProviderErrorClass::Quota
            | RuntimeProviderErrorClass::RateLimit
            | RuntimeProviderErrorClass::Transient
    )
}

fn runtime_provider_error_classification(
    kind: RuntimeProviderBridgeKind,
    status: u16,
    body: &[u8],
) -> (RuntimeProviderErrorClass, ProviderErrorClassification) {
    let translator = provider_translator(kind.provider_id());
    let best =
        prodex_provider_core::classify_provider_error_body(status, body, |status, code, text| {
            translator.classify_error(status, code, text)
        });
    (
        runtime_provider_error_class_from_core(best.class, status),
        best,
    )
}

fn runtime_provider_error_class_from_core(
    class: ProviderErrorClass,
    status: u16,
) -> RuntimeProviderErrorClass {
    match class {
        ProviderErrorClass::Auth => RuntimeProviderErrorClass::Auth,
        ProviderErrorClass::Quota => RuntimeProviderErrorClass::Quota,
        ProviderErrorClass::RateLimit => RuntimeProviderErrorClass::RateLimit,
        ProviderErrorClass::Transient => RuntimeProviderErrorClass::Transient,
        ProviderErrorClass::NotFound => RuntimeProviderErrorClass::NotFound,
        ProviderErrorClass::Other => {
            if status >= 500 {
                RuntimeProviderErrorClass::Transient
            } else {
                RuntimeProviderErrorClass::Fatal
            }
        }
    }
}
