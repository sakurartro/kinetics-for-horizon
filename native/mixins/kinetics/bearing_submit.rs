// Draws a mechanical bearing's parts (Create's BearingRenderer): half a shaft out of its back turning at its speed,
// and its top turned to its facing. The look pack's models only: without them the bearing is the placeholder block.
// shortcut: the top stands still, as on an idle bearing; while a structure turns on it Create turns the top with the
// structure, whose angle the client doesn't have; send it in the update tag to draw that.
fn kinetics_bearing_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_contraption as c, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if block != c::BEARING {
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
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    // The shaft, built pointing south: laid to the back, then turned about its axis.
    let (dpt, phase) = k::spin_args(pos, 0, speed, 0.0, draw.game_time);
    draw.spin(k::axis_vector(m::axis(facing)), centre, dpt, phase);
    draw.push();
    if let Some((turn, degrees)) = m::turn_from_south(facing.map(|v| -v)) {
        draw.rotate_about(turn, degrees, centre);
    }
    game.native_draw_model(draw, crate::game::kinetics_casing::HALF_SHAFT.1, 0xFF_FFFF);
    draw.pop();
    draw.still();
    // The top, built facing up: Create's turns, about y by the back's horizontal angle, then about x.
    let (yaw, pitch) = match facing {
        [0, 1, 0] => (0.0, 0.0),
        [0, -1, 0] => (0.0, -180.0),
        [0, 0, -1] => (0.0, -90.0),
        [0, 0, 1] => (180.0, -90.0),
        [1, 0, 0] => (-90.0, -90.0),
        _ => (90.0, -90.0),
    };
    draw.push();
    draw.rotate_about([0.0, 1.0, 0.0], yaw, centre);
    draw.rotate_about([1.0, 0.0, 0.0], pitch, centre);
    // Registered in models.rs.
    game.native_draw_model(draw, "create:block/bearing/top", 0xFF_FFFF);
    draw.pop();
    draw.keep();
}
