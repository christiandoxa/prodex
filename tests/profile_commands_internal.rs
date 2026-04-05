mod prodex_impl {
    #![allow(dead_code, unused_imports)]

    include!("../src/main.rs");

    mod profile_commands_internal_tests {
        #![allow(dead_code, unused_imports)]

        use super::*;

        include!("../src/profile_commands.rs");
        include!("support/profile_commands_body.rs");
    }
}
