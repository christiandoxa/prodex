use super::*;

#[path = "runtime_proxy_selection_and_pressure/pressure.rs"]
mod pressure;
#[path = "runtime_proxy_selection_and_pressure/selection.rs"]
mod selection;
#[path = "runtime_proxy_selection_and_pressure/rotation.rs"]
mod rotation;
#[path = "runtime_proxy_selection_and_pressure/health.rs"]
mod health;
#[path = "runtime_proxy_selection_and_pressure/doctor.rs"]
mod doctor;
#[path = "runtime_proxy_selection_and_pressure/incidents.rs"]
mod incidents;
#[path = "runtime_proxy_selection_and_pressure/persistence.rs"]
mod persistence;
#[path = "runtime_proxy_selection_and_pressure/state.rs"]
mod state;
#[path = "runtime_proxy_selection_and_pressure/admission.rs"]
mod admission;

use self::{
    admission::*, doctor::*, health::*, incidents::*, persistence::*, pressure::*, rotation::*,
    selection::*, state::*,
};
