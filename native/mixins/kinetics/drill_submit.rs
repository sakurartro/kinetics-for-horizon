// Draws a drill: its body turned to its facing, and its head turning at its speed. Create's models when the look
// pack is on, else ours.
fn kinetics_drill_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_drill as d, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if block != d::BLOCK {
        return;
    }
    let Some(facing) = game.native_block_state_property(state, "facing").and_then(m::dir_vec) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let centre = [0.5; 3];
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|o| game.native_light([pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]])));
    // The body, built facing up, as its blockstate would turn it: the later turn applies first, x then y.
    let (x, y) = d::blockstate_turn(facing);
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], -y, centre);
    draw.rotate_about([1.0, 0.0, 0.0], -x, centre);
    if !game.native_draw_model(draw, d::LOOK_MODELS[0], 0xFF_FFFF) {
        game.native_draw_model(draw, d::MODELS[0], 0xFF_FFFF);
    }
    draw.pop();
    // The head, built facing south: laid to the facing, then turned about its axis.
    let (dpt, phase) = k::spin_args(pos, 0, speed, 0.0, draw.game_time);
    draw.spin(k::axis_vector(m::axis(facing)), centre, dpt, phase);
    draw.push();
    if let Some((turn, degrees)) = m::turn_from_south(facing) {
        draw.rotate_about(turn, degrees, centre);
    }
    if !game.native_draw_model(draw, d::LOOK_MODELS[1], 0xFF_FFFF) {
        game.native_draw_model(draw, d::MODELS[1], 0xFF_FFFF);
    }
    draw.pop();
    draw.still();
    draw.keep();
}
