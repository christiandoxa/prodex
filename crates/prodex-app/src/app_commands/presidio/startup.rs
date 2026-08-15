use super::docker::{
    PRESIDIO_ANALYZER_CONTAINER, PRESIDIO_ANALYZER_IMAGE, PRESIDIO_ANONYMIZER_CONTAINER,
    PRESIDIO_ANONYMIZER_IMAGE, PresidioContainerChange, cleanup_presidio_container,
    docker_available, ensure_presidio_container,
};
use super::{
    DEFAULT_PRESIDIO_ANALYZER_URL, DEFAULT_PRESIDIO_ANONYMIZER_URL, PresidioHealth,
    presidio_auto_start_disabled, print_launch_status,
};
use anyhow::{Result, bail};
use prodex_presidio::PresidioBlockingClient;
use std::thread;
use std::time::{Duration, Instant};

pub(super) fn presidio_startup_blocker(
    analyzer_url: &str,
    anonymizer_url: &str,
) -> Option<(&'static str, &'static str)> {
    if presidio_auto_start_disabled() {
        return Some((
            "Presidio auto-start disabled by PRODEX_PRESIDIO_AUTO_START=0; continuing with configured endpoints.",
            "automatic startup is disabled",
        ));
    }
    if analyzer_url != DEFAULT_PRESIDIO_ANALYZER_URL
        || anonymizer_url != DEFAULT_PRESIDIO_ANONYMIZER_URL
    {
        return Some((
            "Presidio uses custom endpoints; not starting Docker containers automatically.",
            "custom endpoints are not started automatically",
        ));
    }
    (!docker_available()).then_some((
        "Docker is unavailable, so Presidio containers were not started.",
        "Docker is unavailable",
    ))
}

pub(super) fn presidio_startup_failure(
    required: bool,
    fail_mode: &str,
    error: anyhow::Error,
) -> Result<()> {
    if required {
        return Err(error);
    }
    print_launch_status(&format!(
        "Presidio services could not start; continuing with runtime fail_mode={fail_mode}: {error}"
    ));
    Ok(())
}

pub(super) fn start_presidio_containers(
    analyzer: &PresidioHealth,
    anonymizer: &PresidioHealth,
) -> Result<Vec<(&'static str, PresidioContainerChange)>> {
    start_presidio_containers_with(
        analyzer,
        anonymizer,
        ensure_presidio_container,
        cleanup_presidio_container,
    )
}

fn start_presidio_containers_with(
    analyzer: &PresidioHealth,
    anonymizer: &PresidioHealth,
    mut ensure: impl FnMut(&str, &str, &str) -> Result<Option<PresidioContainerChange>>,
    mut cleanup: impl FnMut(&str, PresidioContainerChange) -> Result<()>,
) -> Result<Vec<(&'static str, PresidioContainerChange)>> {
    let mut changes = Vec::new();
    if !analyzer.ok {
        print_launch_status("starting Presidio Analyzer Docker container...");
        match ensure(PRESIDIO_ANALYZER_CONTAINER, PRESIDIO_ANALYZER_IMAGE, "5002") {
            Ok(Some(change)) => changes.push((PRESIDIO_ANALYZER_CONTAINER, change)),
            Ok(None) => {}
            Err(error) => return Err(error),
        }
    }
    if !anonymizer.ok {
        print_launch_status("starting Presidio Anonymizer Docker container...");
        match ensure(
            PRESIDIO_ANONYMIZER_CONTAINER,
            PRESIDIO_ANONYMIZER_IMAGE,
            "5001",
        ) {
            Ok(Some(change)) => changes.push((PRESIDIO_ANONYMIZER_CONTAINER, change)),
            Ok(None) => {}
            Err(error) => {
                return Err(presidio_startup_error_with_cleanup(
                    error,
                    &changes,
                    &mut cleanup,
                ));
            }
        }
    }
    Ok(changes)
}

fn presidio_startup_error_with_cleanup(
    error: anyhow::Error,
    changes: &[(&'static str, PresidioContainerChange)],
    cleanup: &mut impl FnMut(&str, PresidioContainerChange) -> Result<()>,
) -> anyhow::Error {
    match rollback_presidio_containers_with(changes, cleanup) {
        Ok(()) => error,
        Err(cleanup_error) => error.context(format!(
            "Presidio startup failed and cleanup failed: {cleanup_error}"
        )),
    }
}

pub(super) fn rollback_presidio_containers(
    changes: &[(&'static str, PresidioContainerChange)],
) -> Result<()> {
    rollback_presidio_containers_with(changes, cleanup_presidio_container)
}

fn rollback_presidio_containers_with(
    changes: &[(&'static str, PresidioContainerChange)],
    mut cleanup: impl FnMut(&str, PresidioContainerChange) -> Result<()>,
) -> Result<()> {
    let mut errors = Vec::new();
    for &(name, change) in changes.iter().rev() {
        if let Err(error) = cleanup(name, change) {
            errors.push(format!("{name}: {error}"));
        }
    }
    if errors.is_empty() {
        return Ok(());
    }
    bail!(
        "failed to clean up Presidio containers: {}",
        errors.join("; ")
    )
}

pub(super) fn wait_for_presidio_services(
    client: &PresidioBlockingClient,
    analyzer_url: &str,
    anonymizer_url: &str,
) -> bool {
    let deadline = Instant::now() + Duration::from_secs(90);
    while Instant::now() < deadline {
        let analyzer = client.probe_health(analyzer_url);
        let anonymizer = client.probe_health(anonymizer_url);
        if analyzer.ok && anonymizer.ok {
            return true;
        }
        thread::sleep(Duration::from_secs(2));
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presidio_startup_errors_fail_open_only_when_optional() {
        assert!(
            presidio_startup_failure(false, "open", anyhow::anyhow!("synthetic startup failure"))
                .is_ok()
        );

        let error =
            presidio_startup_failure(true, "closed", anyhow::anyhow!("synthetic startup failure"))
                .unwrap_err();
        assert!(error.to_string().contains("synthetic startup failure"));
    }

    #[test]
    fn partial_presidio_startup_rolls_back_started_container() {
        let analyzer = PresidioHealth {
            ok: false,
            message: "unavailable".to_string(),
        };
        let anonymizer = PresidioHealth {
            ok: false,
            message: "unavailable".to_string(),
        };
        let mut ensure_calls = 0;
        let mut cleaned = Vec::new();
        let error = start_presidio_containers_with(
            &analyzer,
            &anonymizer,
            |_name, _image, _port| {
                ensure_calls += 1;
                if ensure_calls == 1 {
                    Ok(Some(PresidioContainerChange::Created))
                } else {
                    Err(anyhow::anyhow!("synthetic anonymizer startup failure"))
                }
            },
            |name, change| {
                cleaned.push((name.to_string(), change));
                Ok(())
            },
        )
        .unwrap_err();

        assert_eq!(ensure_calls, 2);
        assert_eq!(
            cleaned,
            vec![(
                PRESIDIO_ANALYZER_CONTAINER.to_string(),
                PresidioContainerChange::Created,
            )]
        );
        assert!(
            error
                .to_string()
                .contains("synthetic anonymizer startup failure")
        );
    }
}
