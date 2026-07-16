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
        .and_then(|text| parse_system_cpu_ticks(&text))
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
            .and_then(|text| parse_kib_field(&text, "MemTotal"))
            .unwrap_or_default(),
        ..StatusResourceCounters::default()
    };
    let mut socket_inodes = HashSet::new();

    for pid in pids {
        let dir = PathBuf::from(format!("/proc/{pid}"));
        counters.process_cpu_ticks = counters.process_cpu_ticks.saturating_add(
            fs::read_to_string(dir.join("stat"))
                .ok()
                .and_then(|text| parse_process_cpu_ticks(&text))
                .unwrap_or_default(),
        );
        counters.resident_bytes = counters.resident_bytes.saturating_add(
            fs::read_to_string(dir.join("status"))
                .ok()
                .and_then(|text| parse_kib_field(&text, "VmRSS"))
                .unwrap_or_default(),
        );
        if let Ok(io) = fs::read_to_string(dir.join("io")) {
            counters.disk_read_bytes = counters
                .disk_read_bytes
                .saturating_add(parse_u64_field(&io, "read_bytes").unwrap_or_default());
            counters.disk_write_bytes = counters
                .disk_write_bytes
                .saturating_add(parse_u64_field(&io, "write_bytes").unwrap_or_default());
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
        let (rx, tx) = parse_network_queues(&table, &socket_inodes);
        counters.network_rx_queue_bytes = counters.network_rx_queue_bytes.saturating_add(rx);
        counters.network_tx_queue_bytes = counters.network_tx_queue_bytes.saturating_add(tx);
    }
    counters
}

pub(super) fn status_resource_snapshot(
    previous: Option<(StatusResourceCounters, Duration)>,
    current: StatusResourceCounters,
) -> StatusResourceSnapshot {
    let (cpu_percent, disk_read_bytes_per_second, disk_write_bytes_per_second) = previous
        .filter(|(previous, _)| previous.available && current.available)
        .map(|(previous, elapsed)| {
            let system_delta = current
                .system_cpu_ticks
                .saturating_sub(previous.system_cpu_ticks);
            let process_delta = current
                .process_cpu_ticks
                .saturating_sub(previous.process_cpu_ticks);
            let cpu = (system_delta > 0)
                .then_some((process_delta as f64 / system_delta as f64 * 100.0).clamp(0.0, 100.0));
            let seconds = elapsed.as_secs_f64().max(0.001);
            (
                cpu,
                (current
                    .disk_read_bytes
                    .saturating_sub(previous.disk_read_bytes) as f64
                    / seconds) as u64,
                (current
                    .disk_write_bytes
                    .saturating_sub(previous.disk_write_bytes) as f64
                    / seconds) as u64,
            )
        })
        .unwrap_or((None, 0, 0));

    StatusResourceSnapshot {
        available: current.available,
        process_count: current.process_count,
        runtime_process_count: current.runtime_process_count,
        cpu_percent,
        resident_bytes: current.resident_bytes,
        memory_total_bytes: current.memory_total_bytes,
        disk_read_bytes: current.disk_read_bytes,
        disk_write_bytes: current.disk_write_bytes,
        disk_read_bytes_per_second,
        disk_write_bytes_per_second,
        socket_count: current.socket_count,
        network_rx_queue_bytes: current.network_rx_queue_bytes,
        network_tx_queue_bytes: current.network_tx_queue_bytes,
    }
}

pub(super) fn parse_process_cpu_ticks(text: &str) -> Option<u64> {
    let fields = text
        .rsplit_once(')')?
        .1
        .split_whitespace()
        .collect::<Vec<_>>();
    Some(
        fields
            .get(11)?
            .parse::<u64>()
            .ok()?
            .saturating_add(fields.get(12)?.parse::<u64>().ok()?),
    )
}

pub(super) fn parse_system_cpu_ticks(text: &str) -> Option<u64> {
    let mut fields = text.lines().next()?.split_whitespace();
    if fields.next()? != "cpu" {
        return None;
    }
    let mut total = 0_u64;
    for value in fields.take(8) {
        total = total.saturating_add(value.parse::<u64>().ok()?);
    }
    Some(total)
}

pub(super) fn parse_kib_field(text: &str, key: &str) -> Option<u64> {
    let value = text.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        (candidate == key).then_some(value)
    })?;
    value
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()
        .map(|kib| kib.saturating_mul(1024))
}

pub(super) fn parse_u64_field(text: &str, key: &str) -> Option<u64> {
    text.lines().find_map(|line| {
        let (candidate, value) = line.split_once(':')?;
        (candidate == key).then(|| value.trim().parse::<u64>().ok())?
    })
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
        if let Some(inode) = target
            .strip_prefix("socket:[")
            .and_then(|value| value.strip_suffix(']'))
            .and_then(|value| value.parse::<u64>().ok())
        {
            inodes.insert(inode);
        }
    }
}

pub(super) fn parse_network_queues(text: &str, inodes: &HashSet<u64>) -> (u64, u64) {
    let mut rx = 0_u64;
    let mut tx = 0_u64;
    for line in text.lines().skip(1) {
        let fields = line.split_whitespace().collect::<Vec<_>>();
        let Some(inode) = fields.get(9).and_then(|value| value.parse::<u64>().ok()) else {
            continue;
        };
        if !inodes.contains(&inode) {
            continue;
        }
        let Some((tx_hex, rx_hex)) = fields.get(4).and_then(|value| value.split_once(':')) else {
            continue;
        };
        tx = tx.saturating_add(u64::from_str_radix(tx_hex, 16).unwrap_or_default());
        rx = rx.saturating_add(u64::from_str_radix(rx_hex, 16).unwrap_or_default());
    }
    (rx, tx)
}
