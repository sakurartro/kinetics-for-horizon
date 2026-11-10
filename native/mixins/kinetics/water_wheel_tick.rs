// Every tick of a water wheel's block entity: now and then it looks at the water around it
// (`WaterWheelBlockEntity.determineAndApplyFlowScore`); a new score is saved and the network solved again.
fn kinetics_water_wheel_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::{kinetics as k, kinetics_water_wheel as w};
    use horizon_world::nbt::{JavaString, Tag};
    let name: &str = &entity.id;
    if !w::is_wheel(name) || !game.native_authoritative() {
        return;
    }
    let dimension = game.dim.id();
    if !w::due(dimension, pos) {
        return;
    }
    let Some(axis) = game
        .native_block_property(pos, if name == w::LARGE { "axis" } else { "facing" })
        .as_deref()
        .and_then(w::axis_index)
    else {
        return;
    };
    let current = |p: [i32; 3]| {
        w::bubble_push(&game.native_block(p), game.native_block_property(p, "drag")).or_else(|| game.native_fluid_flow(dimension, p))
    };
    // An unloaded chunk beside the wheel: keep the last score, and so the last speed.
    let Some(score) = w::flow_score(&current, pos, axis, w::size(name)) else { return };
    let key = JavaString::from(w::FLOW_KEY);
    let old = match entity.nbt.get(&key) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    if score != old {
        entity.nbt.insert(key, Tag::Int(score));
        k::mark(dimension, pos);
    }
}
