use std::path::PathBuf;

use prodex_storage_postgres_runtime::{PostgresRuntimeConfig, PostgresTlsConfig, connect_blocking};

#[test]
fn blocking_and_pooled_connections_verify_postgres_tls() {
    let Ok(url) = std::env::var("PRODEX_TEST_POSTGRES_TLS_URL") else {
        eprintln!("skipping: PRODEX_TEST_POSTGRES_TLS_URL is not set");
        return;
    };
    let ca_path = PathBuf::from(
        std::env::var("PRODEX_TEST_POSTGRES_TLS_CA")
            .expect("PRODEX_TEST_POSTGRES_TLS_CA must accompany the TLS URL"),
    );
    let tls = PostgresTlsConfig::verify_full(Some(ca_path));

    let mut blocking = connect_blocking(&url, &tls).expect("blocking TLS connection should open");
    let blocking_tls: bool = blocking
        .query_one(
            "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
            &[],
        )
        .expect("blocking TLS state should query")
        .get(0);
    assert!(blocking_tls);

    {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let config = PostgresRuntimeConfig::new(url.clone(), 1).unwrap();
            let pool = config
                .create_pool_with_tls_config(&tls)
                .expect("TLS pool should build");
            let pooled = pool.get().await.expect("pooled TLS connection should open");
            let pooled_tls: bool = pooled
                .query_one(
                    "SELECT ssl FROM pg_stat_ssl WHERE pid = pg_backend_pid()",
                    &[],
                )
                .await
                .expect("pooled TLS state should query")
                .get(0);
            assert!(pooled_tls);
        });
    }

    let mismatch_url = url.replacen("localhost", "127.0.0.1", 1);
    assert!(
        connect_blocking(&mismatch_url, &tls).is_err(),
        "certificate hostname mismatch must fail closed"
    );
}
