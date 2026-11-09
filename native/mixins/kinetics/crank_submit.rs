// Draws the hand crank's shaft and handle, laid along its facing and turned by the speed its network gives it.
fn kinetics_crank_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_crank as c};
    use horizon_world::nbt::{JavaString, Tag};
    if block != c::BLOCK {
        return;
    }
    let Some(facing) = game.native_block_state_property(state, "facing") else { return };
    let Some(axis) = c::axis_of(facing) else { return };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let ticks = draw.game_time as f64 + f64::from(draw.partial);
    let centre = [0.5; 3];
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    draw.push();
    draw.rotate_about(k::axis_vector(axis), c::spin(pos, ticks, speed), centre);
    if let Some((turn, degrees)) = c::lay_along(facing) {
        draw.rotate_about(turn, degrees, centre);
    }
    let look = game.native_draw_model(draw, c::LOOK_MODELS[0], 0xFF_FFFF);
    if look {
        // Create's handle has the wall on its +z side: turned up first, to ours.
        draw.push();
        draw.rotate_about([1.0, 0.0, 0.0], 90.0, centre);
        game.native_draw_model(draw, c::LOOK_MODELS[1], 0xFF_FFFF);
        draw.pop();
    }
    if !look {
        game.native_draw_model(draw, c::PART_MODEL, 0xFF_FFFF);
    }
    draw.pop();
}
