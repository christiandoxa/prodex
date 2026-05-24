#[path = "support/test_wait.rs"]
mod test_wait;

#[path = "auto_rotate/support.rs"]
mod support;
use support::*;

#[path = "auto_rotate/login.rs"]
mod login;
#[path = "auto_rotate/quota_doctor.rs"]
mod quota_doctor;
#[path = "auto_rotate/run.rs"]
mod run;
#[path = "auto_rotate/shared_state.rs"]
mod shared_state;
#[path = "auto_rotate/super_mode.rs"]
mod super_mode;
