#![cfg(all(feature = "mojo-runtime", feature = "mojo-rich", prodex_mojo_required))]

use prodex_mojo_core::{
    MojoError,
    rich::{RuntimeProviderModelsResponsePlan, runtime_provider_models_response_plan},
};

#[test]
fn compiled_mojo_plans_provider_model_response_matrix() {
    for (method, route, catalog, present, expected) in [
        (
            "GET",
            5,
            1,
            false,
            RuntimeProviderModelsResponsePlan::ModelsList,
        ),
        (
            "get",
            6,
            1,
            true,
            RuntimeProviderModelsResponsePlan::ModelFound,
        ),
        (
            "GET",
            6,
            1,
            false,
            RuntimeProviderModelsResponsePlan::ModelMissing,
        ),
        (
            "GET",
            5,
            2,
            false,
            RuntimeProviderModelsResponsePlan::CatalogLimitExceeded,
        ),
        (
            "POST",
            5,
            1,
            false,
            RuntimeProviderModelsResponsePlan::NoResponse,
        ),
        (
            "GET",
            0,
            1,
            false,
            RuntimeProviderModelsResponsePlan::NoResponse,
        ),
        (
            "GET",
            5,
            3,
            false,
            RuntimeProviderModelsResponsePlan::CatalogRequired,
        ),
    ] {
        assert_eq!(
            runtime_provider_models_response_plan(method, route, catalog, present).unwrap(),
            expected,
            "method={method} route={route} catalog={catalog} present={present}"
        );
    }
}

#[test]
fn compiled_mojo_rejects_provider_model_response_facts_outside_abi_bounds() {
    assert_eq!(
        runtime_provider_models_response_plan("GET", 7, 1, false),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        runtime_provider_models_response_plan("GET", 5, 1, true),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        runtime_provider_models_response_plan("GET", 5, 4, false),
        Err(MojoError::InvalidInput)
    );
}
