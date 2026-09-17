use std::sync::OnceLock;

const METRIC_NAME_SLOTS: usize = 228;

static METRIC_NAMES: [OnceLock<String>; METRIC_NAME_SLOTS] =
    [const { OnceLock::new() }; METRIC_NAME_SLOTS];

pub(crate) fn metric_name(plan: usize, slot: usize) -> &'static str {
    let index = plan
        .checked_mul(3)
        .and_then(|value| value.checked_add(slot))
        .filter(|index| *index < METRIC_NAME_SLOTS)
        .expect("observability metric-name index is bounded");
    METRIC_NAMES[index]
        .get_or_init(|| {
            prodex_mojo_core::observability::metric_name(plan as i64, slot as i64)
                .expect("Mojo observability metric-name planner returned invalid output")
        })
        .as_str()
}

const LABEL_KEY_SLOTS: usize = 149;

static LABEL_KEYS: [OnceLock<String>; LABEL_KEY_SLOTS] =
    [const { OnceLock::new() }; LABEL_KEY_SLOTS];

pub(crate) fn label_key(key: usize) -> &'static str {
    LABEL_KEYS
        .get(key)
        .expect("observability label-key index is bounded")
        .get_or_init(|| {
            prodex_mojo_core::observability::label_key(key as i64)
                .expect("Mojo observability label-key planner returned invalid output")
        })
        .as_str()
}
