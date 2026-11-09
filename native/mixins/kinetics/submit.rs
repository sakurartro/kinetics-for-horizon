// Draws a shaft or cogwheel turned to its axis and by its speed.
// shortcut: no draw.keep() for parts standing still: kept geometry follows the block's own light, not
// the neighbours' we use; add it back if frames drop with many parts.
fn kinetics_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::kinetics as k;
    use horizon_world::nbt::{JavaString, Tag};
    let Some(kind) = k::kind_of(block) else { return };
    // The water wheels are drawn by water_wheel_submit.rs.
    if crate::game::kinetics_water_wheel::is_wheel(block) {
        return;
    }
    // A clutch or gearshift: two shaft halves, each at its own speed (SplitShaftRenderer).
    if k::is_split(kind) {
        use crate::game::kinetics_clutch as c;
        let Some(axis) = game.native_block_state_property(state, "axis").and_then(k::axis_of) else {
            return;
        };
        let ticks = draw.game_time as f64 + f64::from(draw.partial);
        let centre = [0.5; 3];
        let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
        draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
        // Create's body over our placeholder, a hair larger so it covers it; turned as Create's block state turns it.
        if let Some(model) = c::static_look(block, game.native_block_state_property(state, "powered") == Some("true")) {
            draw.push();
            draw.translate(centre);
            draw.scale([1.001; 3]);
            draw.translate([-0.5; 3]);
            let (x, y) = k::axis_turn(axis);
            draw.rotate_about([0.0, 1.0, 0.0], -y, centre);
            draw.rotate_about([1.0, 0.0, 0.0], -x, centre);
            game.native_draw_model(draw, model, 0xFF_FFFF);
            draw.pop();
        }
        for half in 0..2u8 {
            let key = JavaString::from(if half == 0 { k::SPEED_KEY } else { c::SPEED_POS_KEY });
            let speed = match entity.nbt.get(&key) {
                Some(Tag::Float(bits)) => f32::from_bits(*bits),
                _ => 0.0,
            };
            let angle = k::angle(speed, ticks);
            // Create's model first (only with the look pack on), ours the fallback.
            for (look, model) in [(true, c::LOOK_HALF_MODEL), (false, c::HALF_MODEL)] {
                draw.push();
                for (turn, degrees) in c::half_turns(axis, half == 1, look, angle) {
                    draw.rotate_about(turn, degrees, centre);
                }
                let drawn = game.native_draw_model(draw, model, 0xFF_FFFF);
                draw.pop();
                if drawn {
                    break;
                }
            }
        }
        return;
    }
    // The cased parts are drawn by casing_submit.rs.
    if crate::game::kinetics_casing::is_encased(block) {
        return;
    }
    let Some(model) = k::part_model(kind) else { return };
    let Some(axis) = game.native_block_state_property(state, "axis").and_then(k::axis_of) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let ticks = draw.game_time as f64 + f64::from(draw.partial);
    let angle = k::angle(speed, ticks) + k::tooth_offset(kind, axis, pos);
    let centre = [0.5; 3];
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    draw.push();
    draw.rotate_about(k::axis_vector(axis), angle, centre);
    if let Some((turn, degrees)) = k::lay_along(axis) {
        draw.rotate_about(turn, degrees, centre);
    }
    let look = k::look_model(kind).is_some_and(|m| game.native_draw_model(draw, m, 0xFF_FFFF));
    if !look {
        game.native_draw_model(draw, model, 0xFF_FFFF);
    }
    draw.pop();
    // A speed controller under a large cogwheel on a horizontal axis: Create's bracket around the cogwheel's foot
    // (SpeedControllerRenderer), in the block above, half a turn about y along x, a quarter along z.
    let above = [pos[0], pos[1] + 1, pos[2]];
    if matches!(kind, k::Kind::Controller { .. })
        && game.native_block(above) == "kinetics:large_cogwheel"
        && game.native_block_property(above, "axis").as_deref().is_some_and(|a| a != "y")
    {
        draw.light = k::part_light([game.native_light(above)]);
        draw.push();
        draw.translate([0.0, 1.0, 0.0]);
        draw.rotate_about([0.0, 1.0, 0.0], if axis == 0 { 180.0 } else { 90.0 }, centre);
        // Registered in models.rs.
        game.native_draw_model(draw, "create:block/rotation_speed_controller/bracket", 0xFF_FFFF);
        draw.pop();
    }
}
