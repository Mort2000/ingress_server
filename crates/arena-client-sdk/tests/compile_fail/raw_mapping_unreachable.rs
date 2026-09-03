// error-code: E0599
// P4-CAPABILITY (M8 gap fill, D77): the participant-facing SDK types
// expose NO raw-mapping escape — neither the observation view nor the
// map view hands out a pointer or the raw region bytes. The only raw
// mapping code lives inside the SDK's own shm module (operator-side,
// audited); participants get safe views.
use arena_client_sdk::{MapView, ObservationView};

fn view_escape(view: &ObservationView<'_>) -> *const u8 {
    view.raw_mapping().as_ptr()
}

fn map_escape(view: &MapView<'_>) -> *const u8 {
    view.raw_mapping().as_ptr()
}

fn main() {
    let _ = (view_escape, map_escape);
}
