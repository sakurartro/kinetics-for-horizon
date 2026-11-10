// Draws a cuckoo clock: the body, the half shaft turning with its speed, the two hands, the doors and the figure
// (`CuckooClockRenderer`). Create's models when the look pack is on, else ours.
fn kinetics_clock_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::{kinetics as k, kinetics_casing, kinetics_clock as c, kinetics_crank};
    use horizon_world::nbt::{JavaString, Tag};
    if !c::is_clock(block) {
        return;
    }
    let Some(facing) = game.native_block_state_property(state, "facing") else { return };
    let (Some(north), Some(west), Some(axis)) = (c::north_to(facing), c::west_to(facing), kinetics_crank::axis_of(facing)) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let anim = match entity.nbt.get(&JavaString::from(c::ANIM_KEY)) {
        Some(Tag::Int(v)) => *v,
        _ => c::NONE,
    };
    let start = match entity.nbt.get(&JavaString::from(c::START_KEY)) {
        Some(Tag::Long(v)) => *v,
        _ => 0,
    };
    let ticks = draw.game_time as f64 + f64::from(draw.partial);
    let (centre, up) = ([0.5; 3], [0.0, 1.0, 0.0]);
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    // Create's model if its pack is on, ours otherwise.
    let part = |draw: &mut BlockEntityDraw, look: &str, ours: &str| {
        if !game.native_draw_model(draw, look, 0xFF_FFFF) {
            game.native_draw_model(draw, ours, 0xFF_FFFF);
        }
    };
    let model = |draw: &mut BlockEntityDraw, i: usize| part(draw, c::LOOK_MODELS[i], c::PART_MODELS[i]);
    // The body (built facing north) and the shaft out of its back, laid along the facing and turned by the speed.
    draw.push();
    draw.rotate_about(up, north, centre);
    model(draw, 0);
    draw.pop();
    let (dpt, phase) = k::spin_args(pos, 0, speed, 0.0, draw.game_time);
    draw.spin(k::axis_vector(axis), centre, dpt, phase);
    draw.push();
    if let Some((turn, degrees)) = kinetics_crank::lay_along(facing) {
        draw.rotate_about(turn, degrees, centre);
    }
    draw.push();
    // The half shaft's models are built along z, as the crank's: turned down, to the wall.
    draw.rotate_about([1.0, 0.0, 0.0], 90.0, centre);
    part(draw, kinetics_casing::HALF_SHAFT.1, kinetics_casing::HALF_SHAFT.0);
    draw.pop();
    draw.pop();
    draw.still();
    // The hands, built on the west side: turned to the facing, then about their pivot.
    let natural = c::natural(game.native_dimension());
    let (hours, minutes) = c::time(game.native_day_time(), natural);
    let (hour, minute) = c::hands(pos, ticks, speed != 0.0, hours, minutes);
    for (i, angle) in [(1, hour), (2, minute)] {
        draw.push();
        draw.rotate_about(up, west, centre);
        draw.rotate_about([1.0, 0.0, 0.0], angle, [2.0 / 16.0, 6.0 / 16.0, 8.0 / 16.0]);
        model(draw, i);
        draw.pop();
    }
    // The doors, and the figure the animation brings out between them.
    let value = (ticks - start as f64) as f32;
    let angle = if anim == c::NONE { 0.0 } else { c::door_angle(anim, value) };
    for (i, pivot, sign) in [(3, 6.0 / 16.0, -1.0), (4, 10.0 / 16.0, 1.0)] {
        draw.push();
        draw.rotate_about(up, west, centre);
        draw.rotate_about(up, angle * sign, [2.0 / 16.0, 0.0, pivot]);
        model(draw, i);
        draw.pop();
    }
    if anim != c::NONE {
        draw.push();
        draw.rotate_about(up, west, centre);
        draw.translate([c::figure_offset(angle), 0.0, 0.0]);
        model(draw, if anim == c::PIG { 5 } else { 6 });
        draw.pop();
    }
}
