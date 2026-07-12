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
    if class == ProviderErrorClass::Other && status >= 500 {
        ProviderErrorClass::Transient
    } else {
        class
    }
}
