fn main() {
    let matrix = prodex_provider_core::provider_adapter_contract_matrix();
    println!(
        "{}",
        serde_json::to_string_pretty(&matrix).expect("provider contract matrix should serialize")
    );
}
