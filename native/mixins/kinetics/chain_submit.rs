// Draws a chain drive or gearshift: Create's static model over our placeholder (the blockstate model) while the look
// pack is on, and the shaft turning inside it.
fn kinetics_chain_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::kinetics as k;
    use crate::game::kinetics_chain as c;
    use horizon_world::nbt::{JavaString, Tag};
    if !c::is_chain(block) {
        return;
    }
    let prop = |key: &str| game.native_block_state_property(state, key);
    let (Some(axis), Some(part)) = (prop("axis").and_then(k::axis_of), prop("part").and_then(c::Part::parse)) else {
        return;
    };
    let shape = c::Shape { axis, first: prop(c::FIRST) == Some("true"), part };
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    let centre = [0.5; 3];
    if let Some((model, x, y)) = c::static_look(block, shape, prop("powered") == Some("true")) {
        draw.push();
        // A hair larger than the block, so it covers our placeholder under it; the block state's (clockwise) turns,
        // x first: the later turn applies first.
        draw.translate(centre);
        draw.scale([1.001; 3]);
        draw.translate([-0.5; 3]);
        draw.rotate_about([0.0, 1.0, 0.0], -(y as f32), centre);
        draw.rotate_about([1.0, 0.0, 0.0], -(x as f32), centre);
        game.native_draw_model(draw, &model, 0xFF_FFFF);
        draw.pop();
    }
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let angle = k::angle(speed, draw.game_time as f64 + f64::from(draw.partial));
    draw.push();
    draw.rotate_about(k::axis_vector(axis), angle, centre);
    if let Some((turn, degrees)) = k::lay_along(axis) {
        draw.rotate_about(turn, degrees, centre);
    }
    let look = game.native_draw_model(draw, k::LOOK_MODELS[0], 0xFF_FFFF);
    if !look {
        game.native_draw_model(draw, k::PART_MODELS[0], 0xFF_FFFF);
    }
    draw.pop();
}
