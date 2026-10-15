# Runtime usage-limit recovery candidate selection in Mojo

The compiled `prodex_launch_usage_limit_recovery_candidate_v1` policy now
selects the first unattempted eligible replacement and the bounded failed-profile
fallback. Rust keeps profile, authentication, quota, process, and relaunch
effects; required-Mojo ABI and app caller regressions cover ordering, attempted
profiles, and fallback precedence.
