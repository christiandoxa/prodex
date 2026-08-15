use anyhow::{Context, Result, bail};
use std::process::Command;
use std::time::Duration;

pub(super) const PRESIDIO_ANALYZER_CONTAINER: &str = "presidio-analyzer";
pub(super) const PRESIDIO_ANONYMIZER_CONTAINER: &str = "presidio-anonymizer";
pub(super) const PRESIDIO_ANALYZER_IMAGE: &str = "ghcr.io/data-privacy-stack/presidio-analyzer:2.2.364@sha256:ae8f6f111ac2f04e3fec552f7f80edd0dcbfa2dd69ee1b9e030475be31669885";
pub(super) const PRESIDIO_ANONYMIZER_IMAGE: &str = "ghcr.io/data-privacy-stack/presidio-anonymizer:2.2.364@sha256:e567013893ebc80994e3799f6f55c86aa1f0b0fadb779571ab346f0ec45365c1";
const PRESIDIO_MANAGED_LABEL: &str = "com.prodex.presidio.managed";
const PRESIDIO_SERVICE_LABEL: &str = "com.prodex.presidio.service";
const PRESIDIO_DOCKER_TIMEOUT: Duration = Duration::from_secs(300);
const PRESIDIO_DOCKER_OUTPUT_MAX_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PresidioContainerChange {
    Started,
    Created,
}

pub(super) fn docker_available() -> bool {
    let mut command = Command::new("docker");
    command.arg("version");
    crate::command_probe_output(&mut command, "Docker version probe")
        .is_ok_and(|output| output.status.success())
}

pub(super) fn ensure_presidio_container(
    name: &str,
    image: &str,
    host_port: &str,
) -> Result<Option<PresidioContainerChange>> {
    if let Some(container) = inspect_presidio_container(name)? {
        validate_presidio_container(&container, name, image, host_port)?;
        if container
            .pointer("/State/Running")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            return Ok(None);
        }
        let mut command = Command::new("docker");
        command.args(["start", name]);
        let output = crate::command_output_with_timeout(
            &mut command,
            PRESIDIO_DOCKER_TIMEOUT,
            PRESIDIO_DOCKER_OUTPUT_MAX_BYTES,
            "Presidio docker start",
        )
        .with_context(|| format!("failed to start Presidio container {name}"))?;
        if !output.status.success() {
            bail!("docker start {name} failed with {}", output.status);
        }
        return Ok(Some(PresidioContainerChange::Started));
    }

    let published_port = format!("127.0.0.1:{host_port}:3000");
    let managed_label = format!("{PRESIDIO_MANAGED_LABEL}=true");
    let service_label = format!("{PRESIDIO_SERVICE_LABEL}={name}");
    let mut command = Command::new("docker");
    command.args([
        "run",
        "-d",
        "--name",
        name,
        "--label",
        &managed_label,
        "--label",
        &service_label,
        "-p",
        &published_port,
        image,
    ]);
    let output = crate::command_output_with_timeout(
        &mut command,
        PRESIDIO_DOCKER_TIMEOUT,
        PRESIDIO_DOCKER_OUTPUT_MAX_BYTES,
        "Presidio docker run",
    )
    .with_context(|| format!("failed to run Presidio container {name}"))?;
    if !output.status.success() {
        bail!("docker run {name} failed with {}", output.status);
    }
    Ok(Some(PresidioContainerChange::Created))
}

pub(super) fn cleanup_presidio_container(
    name: &str,
    change: PresidioContainerChange,
) -> Result<()> {
    let args = cleanup_presidio_container_args(name, change);
    let action = match change {
        PresidioContainerChange::Started => "stop",
        PresidioContainerChange::Created => "remove",
    };
    let mut command = Command::new("docker");
    command.args(&args);
    let output = crate::command_output_with_timeout(
        &mut command,
        PRESIDIO_DOCKER_TIMEOUT,
        PRESIDIO_DOCKER_OUTPUT_MAX_BYTES,
        &format!("Presidio docker {action}"),
    )
    .with_context(|| format!("failed to {action} Presidio container {name}"))?;
    if !output.status.success() {
        bail!("docker {action} {name} failed with {}", output.status);
    }
    Ok(())
}

fn cleanup_presidio_container_args(name: &str, change: PresidioContainerChange) -> Vec<&str> {
    match change {
        PresidioContainerChange::Started => vec!["stop", name],
        PresidioContainerChange::Created => vec!["rm", "--force", name],
    }
}

fn inspect_presidio_container(name: &str) -> Result<Option<serde_json::Value>> {
    let mut command = Command::new("docker");
    command.args(["container", "inspect", "--format", "{{json .}}", name]);
    let output = crate::command_probe_output(&mut command, "Presidio docker inspect")
        .with_context(|| format!("failed to inspect Presidio container {name}"))?;
    if output.status.success() {
        return serde_json::from_slice(&output.stdout)
            .map(Some)
            .with_context(|| {
                format!("failed to parse Docker inspection for Presidio container {name}")
            });
    }
    let error = String::from_utf8_lossy(&output.stderr);
    if error.contains("No such object") || error.contains("No such container") {
        return Ok(None);
    }
    bail!("docker inspect {name} failed with {}", output.status);
}

fn validate_presidio_container(
    container: &serde_json::Value,
    name: &str,
    image: &str,
    host_port: &str,
) -> Result<()> {
    let labels = container.pointer("/Config/Labels");
    let managed = labels
        .and_then(|labels| labels.get(PRESIDIO_MANAGED_LABEL))
        .and_then(serde_json::Value::as_str)
        == Some("true");
    let service = labels
        .and_then(|labels| labels.get(PRESIDIO_SERVICE_LABEL))
        .and_then(serde_json::Value::as_str)
        == Some(name);
    let image_matches = container
        .pointer("/Config/Image")
        .and_then(serde_json::Value::as_str)
        == Some(image);
    let bindings = container.pointer("/HostConfig/PortBindings");
    let port_matches = bindings
        .and_then(|bindings| bindings.as_object())
        .filter(|bindings| bindings.len() == 1)
        .and_then(|bindings| bindings.get("3000/tcp"))
        .and_then(serde_json::Value::as_array)
        .filter(|bindings| bindings.len() == 1)
        .and_then(|bindings| bindings.first())
        .is_some_and(|binding| {
            binding.get("HostPort").and_then(serde_json::Value::as_str) == Some(host_port)
                && binding.get("HostIp").and_then(serde_json::Value::as_str) == Some("127.0.0.1")
        });
    if !managed || !service || !image_matches || !port_matches {
        bail!(
            "refusing existing Presidio container {name}: Prodex ownership, image, or port configuration does not match"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_changes_distinguish_restore_from_remove() {
        assert_eq!(
            cleanup_presidio_container_args("presidio-analyzer", PresidioContainerChange::Started),
            ["stop", "presidio-analyzer"],
        );
        assert_eq!(
            cleanup_presidio_container_args("presidio-analyzer", PresidioContainerChange::Created),
            ["rm", "--force", "presidio-analyzer"],
        );
    }

    #[test]
    fn managed_presidio_container_contract_rejects_unowned_or_mismatched_containers() {
        let container = serde_json::json!({
            "Config": {
                "Image": PRESIDIO_ANALYZER_IMAGE,
                "Labels": {
                    "com.prodex.presidio.managed": "true",
                    "com.prodex.presidio.service": PRESIDIO_ANALYZER_CONTAINER
                }
            },
            "HostConfig": {
                "PortBindings": {
                    "3000/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "5002" }]
                }
            }
        });
        validate_presidio_container(
            &container,
            PRESIDIO_ANALYZER_CONTAINER,
            PRESIDIO_ANALYZER_IMAGE,
            "5002",
        )
        .unwrap();
        for image in [PRESIDIO_ANALYZER_IMAGE, PRESIDIO_ANONYMIZER_IMAGE] {
            assert!(image.contains("@sha256:"));
            assert!(!image.contains(":latest"));
        }

        for (path, value) in [
            (
                "/Config/Labels/com.prodex.presidio.managed",
                serde_json::json!("false"),
            ),
            ("/Config/Image", serde_json::json!("example.com/not-prodex")),
            (
                "/HostConfig/PortBindings/3000~1tcp/0/HostPort",
                serde_json::json!("5003"),
            ),
            (
                "/HostConfig/PortBindings/3000~1tcp/0/HostIp",
                serde_json::json!("0.0.0.0"),
            ),
        ] {
            let mut invalid = container.clone();
            let (parent, key) = path.rsplit_once('/').unwrap();
            invalid.pointer_mut(parent).unwrap()[key] = value;
            assert!(
                validate_presidio_container(
                    &invalid,
                    PRESIDIO_ANALYZER_CONTAINER,
                    PRESIDIO_ANALYZER_IMAGE,
                    "5002",
                )
                .is_err(),
                "container mutation {path} must be rejected"
            );
        }
    }
}
