// Draws a speedometer (Create's GaugeRenderer over ShaftRenderer): its body over the placeholder, the shaft through
// it turning at its speed, and on each side that shows one the head and the needle, which eases toward the speed.
// The look pack's models only: without them the gauge is the placeholder block.
fn kinetics_speedometer_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_speedometer as s};
    use horizon_world::nbt::{JavaString, Tag};
    if block != s::BLOCK {
        return;
    }
    let Some(facing) = game.native_block_state_property(state, "facing").and_then(s::dir_vec) else {
        return;
    };
    let along = game.native_block_state_property(state, "axis_along_first") == Some("true");
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let centre = [0.5; 3];
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    // The body: Create's, a hair larger than our placeholder so it covers it.
    let (wall, ..) = s::variant(facing, along);
    draw.push();
    draw.translate(centre);
    draw.scale([1.001; 3]);
    draw.translate([-0.5; 3]);
    for (turn, degrees) in s::look_turns(facing, along) {
        if degrees != 0.0 {
            draw.rotate_about(turn, degrees, centre);
        }
    }
    game.native_draw_model(draw, if wall { s::LOOK_WALL } else { s::LOOK_BASE }, 0xFF_FFFF);
    draw.pop();
    // The shaft through it.
    let axis = s::rotation_axis(facing, along);
    let (dpt, phase) = k::spin_args(pos, 0, speed, 0.0, draw.game_time);
    draw.spin(k::axis_vector(axis), centre, dpt, phase);
    draw.push();
    if let Some((turn, degrees)) = k::lay_along(axis) {
        draw.rotate_about(turn, degrees, centre);
    }
    if !game.native_draw_model(draw, k::LOOK_MODELS[0], 0xFF_FFFF) {
        game.native_draw_model(draw, k::PART_MODELS[0], 0xFF_FFFF);
    }
    draw.pop();
    draw.still();
    // The heads and needles.
    let progress = s::dial_at(pos, draw.game_time as i64, s::dial_target(speed)).progress(draw.partial);
    for face in s::HORIZONTAL {
        if !s::head_on_face(facing, along, face) {
            continue;
        }
        draw.push();
        draw.rotate_about([0.0, 1.0, 0.0], s::face_yaw(face), centre);
        draw.push();
        draw.rotate_about([1.0, 0.0, 0.0], s::dial_degrees(progress), [0.0, s::DIAL_PIVOT, s::DIAL_PIVOT]);
        game.native_draw_model(draw, s::LOOK_DIAL, 0xFF_FFFF);
        draw.pop();
        game.native_draw_model(draw, s::LOOK_HEAD, 0xFF_FFFF);
        draw.pop();
    }
}
