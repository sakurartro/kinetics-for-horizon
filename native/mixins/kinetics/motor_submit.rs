// Draws a creative motor: its body turned to its facing, and half a shaft turning at its speed. Create's models when
// the look pack is on, else ours.
fn kinetics_motor_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if block != m::BLOCK {
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
    let south = m::turn_from_south(facing);
    let vertical = facing[1] != 0;
    // The body: Create's, standing for a motor facing up or down, lying otherwise.
    let look = if vertical { m::LOOK_MODELS[1] } else { m::LOOK_MODELS[0] };
    let look_turn = if vertical { m::turn_from_up(facing) } else { south };
    draw.push();
    if let Some((turn, degrees)) = look_turn {
        draw.rotate_about(turn, degrees, centre);
    }
    let drawn = game.native_draw_model(draw, look, 0xFF_FFFF);
    draw.pop();
    if !drawn {
        draw.push();
        if let Some((turn, degrees)) = south {
            draw.rotate_about(turn, degrees, centre);
        }
        game.native_draw_model(draw, m::MODELS[0], 0xFF_FFFF);
        draw.pop();
    }
    // The shaft: laid to the facing, then turned about its axis.
    let (dpt, phase) = k::spin_args(pos, 0, speed, 0.0, draw.game_time);
    draw.spin(k::axis_vector(m::axis(facing)), centre, dpt, phase);
    draw.push();
    if let Some((turn, degrees)) = south {
        draw.rotate_about(turn, degrees, centre);
    }
    if !game.native_draw_model(draw, m::LOOK_MODELS[2], 0xFF_FFFF) {
        game.native_draw_model(draw, m::MODELS[1], 0xFF_FFFF);
    }
    draw.pop();
    draw.still();
    draw.keep();
}
