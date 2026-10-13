use super::{StatusResourceCounters, StatusResourceSnapshot};
use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::ProdexProcessInfo;

pub(super) fn collect_status_resource_counters(
    processes: &[ProdexProcessInfo],
) -> StatusResourceCounters {
    let mut pids = processes
        .iter()
        .map(|process| process.pid)
        .collect::<BTreeSet<_>>();
    pids.insert(std::process::id());
    let Some(system_cpu_ticks) = fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|text| prodex_mojo_core::status_resource::parse_system_cpu_ticks(&text))
    else {
        return StatusResourceCounters {
            process_count: pids.len(),
            runtime_process_count: processes.iter().filter(|process| process.runtime).count(),
            ..StatusResourceCounters::default()
        };
    };

    let mut counters = StatusResourceCounters {
        available: true,
        process_count: pids.len(),
        runtime_process_count: processes.iter().filter(|process| process.runtime).count(),
        system_cpu_ticks,
        memory_total_bytes: fs::read_to_string("/proc/meminfo")
            .ok()
            .and_then(|text| prodex_mojo_core::status_resource::parse_kib_field(&text, "MemTotal"))
            .unwrap_or_default(),
        ..StatusResourceCounters::default()
    };
    let mut socket_inodes = HashSet::new();

    for pid in pids {
        let dir = PathBuf::from(format!("/proc/{pid}"));
        counters.process_cpu_ticks = counters.process_cpu_ticks.saturating_add(
            fs::read_to_string(dir.join("stat"))
                .ok()
                .and_then(|text| prodex_mojo_core::status_resource::parse_process_cpu_ticks(&text))
                .unwrap_or_default(),
        );
        counters.resident_bytes = counters.resident_bytes.saturating_add(
            fs::read_to_string(dir.join("status"))
                .ok()
                .and_then(|text| prodex_mojo_core::status_resource::parse_kib_field(&text, "VmRSS"))
                .unwrap_or_default(),
        );
        if let Ok(io) = fs::read_to_string(dir.join("io")) {
            counters.disk_read_bytes = counters.disk_read_bytes.saturating_add(
                prodex_mojo_core::status_resource::parse_u64_field(&io, "read_bytes")
                    .unwrap_or_default(),
            );
            counters.disk_write_bytes = counters.disk_write_bytes.saturating_add(
                prodex_mojo_core::status_resource::parse_u64_field(&io, "write_bytes")
                    .unwrap_or_default(),
            );
        }
        collect_socket_inodes(&dir.join("fd"), &mut socket_inodes);
    }

    counters.socket_count = socket_inodes.len();
    for path in [
        "/proc/net/tcp",
        "/proc/net/tcp6",
        "/proc/net/udp",
        "/proc/net/udp6",
    ] {
        let Ok(table) = fs::read_to_string(path) else {
            continue;
        };
        let (rx, tx) =
            prodex_mojo_core::status_resource::parse_network_queues(&table, &socket_inodes);
        counters.network_rx_queue_bytes = counters.network_rx_queue_bytes.saturating_add(rx);
        counters.network_tx_queue_bytes = counters.network_tx_queue_bytes.saturating_add(tx);
    }
    counters
}

pub(super) fn status_resource_snapshot(
    previous: Option<(StatusResourceCounters, Duration)>,
    current: StatusResourceCounters,
) -> StatusResourceSnapshot {
    let previous = previous.map(|(previous, elapsed)| {
        (
            prodex_mojo_core::info_render::InfoStatusResourceCounters {
                available: previous.available,
                process_cpu_ticks: previous.process_cpu_ticks,
                system_cpu_ticks: previous.system_cpu_ticks,
                disk_read_bytes: previous.disk_read_bytes,
                disk_write_bytes: previous.disk_write_bytes,
            },
            elapsed.as_secs_f64(),
        )
    });
    let metrics = prodex_mojo_core::info_render::status_resource_metrics(
        previous,
        prodex_mojo_core::info_render::InfoStatusResourceCounters {
            available: current.available,
            process_cpu_ticks: current.process_cpu_ticks,
            system_cpu_ticks: current.system_cpu_ticks,
            disk_read_bytes: current.disk_read_bytes,
            disk_write_bytes: current.disk_write_bytes,
        },
    )
    .expect("Mojo status resource snapshot returned invalid output");

    StatusResourceSnapshot {
        available: current.available,
        process_count: current.process_count,
        runtime_process_count: current.runtime_process_count,
        cpu_percent: metrics.cpu_percent,
        resident_bytes: current.resident_bytes,
        memory_total_bytes: current.memory_total_bytes,
        disk_read_bytes: current.disk_read_bytes,
        disk_write_bytes: current.disk_write_bytes,
        disk_read_bytes_per_second: metrics.disk_read_bytes_per_second,
        disk_write_bytes_per_second: metrics.disk_write_bytes_per_second,
        socket_count: current.socket_count,
        network_rx_queue_bytes: current.network_rx_queue_bytes,
        network_tx_queue_bytes: current.network_tx_queue_bytes,
    }
}

fn collect_socket_inodes(fd_dir: &Path, inodes: &mut HashSet<u64>) {
    let Ok(entries) = fs::read_dir(fd_dir) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(target) = fs::read_link(entry.path()) else {
            continue;
        };
        let Some(target) = target.to_str() else {
            continue;
        };
        if let Some(inode) = prodex_mojo_core::status_resource::socket_inode_target(target) {
            inodes.insert(inode);
        }
    }
}
