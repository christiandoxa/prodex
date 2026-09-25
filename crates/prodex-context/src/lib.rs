mod critical_signal;

pub use critical_signal::{
    CriticalSignalCounts, CriticalSignalLineRange, CriticalSignalLineRangeOptions,
    CriticalSignalSelfCheck, count_critical_signals, critical_signal_available,
    critical_signal_lost_line_ranges, critical_signal_lost_line_ranges_with_options,
    critical_signal_self_check,
};
