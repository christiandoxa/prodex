use super::*;

#[test]
fn test_env_var_guard_restores_previous_value_and_supports_nested_reentry() {
    let key = "PRODEX_TEST_ENV_GUARD_REENTRY";
    let previous = env::var_os(key);

    {
        let _outer = TestEnvVarGuard::set(key, "outer");
        assert_eq!(env::var(key).ok().as_deref(), Some("outer"));

        {
            let _inner = TestEnvVarGuard::set(key, "inner");
            assert_eq!(env::var(key).ok().as_deref(), Some("inner"));
        }

        assert_eq!(env::var(key).ok().as_deref(), Some("outer"));
    }

    assert_eq!(env::var_os(key), previous);
}
