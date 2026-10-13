#![cfg(feature = "mojo-rich")]
#![allow(unsafe_code)]

use prodex_mojo_core::rich::{
    RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_BAD_PAIRING, RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT,
    RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH, RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY,
    RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct View {
    ptr: u64,
    len: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct DegradedRouteRecord {
    kind: i64,
    source_index: i64,
    profile_start: i64,
    profile_len: i64,
    route_start: i64,
    route_len: i64,
    value: i64,
}

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_degraded_routes_v1(
        abi: i64,
        now: i64,
        health_decay_seconds: i64,
        bad_pairing_decay_seconds: i64,
        keys: u64,
        values: u64,
        updated: u64,
        group_starts: u64,
        total_count: i64,
        output: u64,
        output_capacity: i64,
        output_count: u64,
    ) -> i64;
}

fn render_records(keys: &[&str], records: &[DegradedRouteRecord], now: i64) -> Vec<String> {
    records
        .iter()
        .map(|record| {
            let key = keys[record.source_index as usize];
            let profile = &key[record.profile_start as usize
                ..(record.profile_start + record.profile_len) as usize];
            let route = if record.route_start < 0 {
                if record.kind == RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY {
                    "retry"
                } else {
                    "transport"
                }
            } else {
                &key[record.route_start as usize..(record.route_start + record.route_len) as usize]
            };
            match record.kind {
                RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT => format!(
                    "{profile}/{route} circuit={} until={}",
                    if record.value > now {
                        "open"
                    } else {
                        "half-open"
                    },
                    record.value
                ),
                RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT => {
                    format!("{profile}/{route} transport_backoff until={}", record.value)
                }
                RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY => {
                    format!("{profile}/{route} retry_backoff until={}", record.value)
                }
                RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_BAD_PAIRING => {
                    format!("{profile}/{route} bad_pairing={}", record.value)
                }
                RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH => {
                    format!("{profile}/{route} health={}", record.value)
                }
                _ => panic!("unexpected record kind"),
            }
        })
        .collect()
}

#[test]
fn degraded_route_abi_classifies_decays_sorts_deduplicates_and_truncates() {
    let keys = [
        "zeta",
        "alpha",
        "__route_transport_backoff__:websocket:alpha",
        "beta",
        "__route_circuit__:responses:alpha",
        "__route_circuit__:compact:beta",
        "__route_health__:responses:alpha",
        "__route_bad_pairing__:compact:beta",
        "__route_health__:standard:ignored",
        "__route_health__:responses:alpha",
        "__route_bad_pairing__:chat:beta",
        "__route_bad_pairing__:chat:delta",
        "__route_bad_pairing__:chat:epsilon",
        "__route_bad_pairing__:chat:zeta",
    ];
    let views = keys
        .iter()
        .map(|key| View {
            ptr: key.as_ptr() as u64,
            len: key.len() as u64,
        })
        .collect::<Vec<_>>();
    let values = [
        1005_i64, 1007, 1010, 1005, 1001, 999, 5, 4, 1, 5, 1, 1, 1, 1,
    ];
    let updated = [
        0_i64, 0, 0, 0, 0, 0, 980, 995, 900, 980, 1000, 1000, 1000, 1000,
    ];
    let starts = [0_i64, 2, 4, 6, 14];
    let mut output = [DegradedRouteRecord::default(); 8];
    let mut output_count = -1_i64;
    let code = unsafe {
        prodex_mojo_runtime_doctor_degraded_routes_v1(
            1,
            1000,
            10,
            5,
            views.as_ptr() as u64,
            values.as_ptr() as u64,
            updated.as_ptr() as u64,
            starts.as_ptr() as u64,
            keys.len() as i64,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            &mut output_count as *mut i64 as u64,
        )
    };
    assert_eq!(code, 0);
    assert_eq!(output_count, 8);
    assert_eq!(
        render_records(&keys, &output, 1000),
        [
            "alpha/responses circuit=open until=1001",
            "alpha/responses health=3",
            "alpha/retry retry_backoff until=1007",
            "alpha/websocket transport_backoff until=1010",
            "beta/chat bad_pairing=1",
            "beta/compact bad_pairing=3",
            "beta/compact circuit=half-open until=999",
            "beta/transport transport_backoff until=1005",
        ]
    );
}

#[test]
fn degraded_route_abi_rejects_invalid_abi_capacity_ranges_and_score_values() {
    let key = "__route_health__:responses:alpha";
    let view = View {
        ptr: key.as_ptr() as u64,
        len: key.len() as u64,
    };
    let values = [1_i64];
    let updated = [0_i64];
    let starts = [0_i64, 0, 0, 0, 1];
    let mut output = [DegradedRouteRecord::default(); 8];
    let mut output_count = -1_i64;
    let mut call = |abi, capacity, starts: &[i64], values: &[i64]| unsafe {
        prodex_mojo_runtime_doctor_degraded_routes_v1(
            abi,
            10,
            1,
            1,
            &view as *const View as u64,
            values.as_ptr() as u64,
            updated.as_ptr() as u64,
            starts.as_ptr() as u64,
            1,
            output.as_mut_ptr() as u64,
            capacity,
            &mut output_count as *mut i64 as u64,
        )
    };
    assert_eq!(call(2, 8, &starts, &values), 4);
    assert_eq!(call(1, 7, &starts, &values), 1);
    assert_eq!(call(1, 8, &[1, 0, 0, 0, 1], &values), 2);
    assert_eq!(call(1, 8, &starts, &[-1]), 2);
}
