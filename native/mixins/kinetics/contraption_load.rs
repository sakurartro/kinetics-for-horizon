// A level opened: entity ids start again, so the contraptions' ids kept for it are dropped; the tick finds the
// markers and makes the displays anew (contraption_tick.rs).
fn kinetics_contraption_load(dimension: &str) {
    crate::game::kinetics_contraption::forget(dimension);
}
