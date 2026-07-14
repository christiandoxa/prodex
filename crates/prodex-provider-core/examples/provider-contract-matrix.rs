fn main() {
    let matrix = prodex_provider_core::provider_contract_catalog(
        prodex_provider_core::EffectiveHarnessMode::Native,
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&matrix).expect("provider contract matrix should serialize")
    );
}
