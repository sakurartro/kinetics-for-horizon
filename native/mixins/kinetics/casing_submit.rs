// Draws what the casing blocks' block entities carry: Create's static model over our placeholder (the blockstate
// model) while the look pack is on, and the part turning inside a cased shaft or cogwheel, with the shafts out of
// the ends of a cogwheel that has them. The bare shafts and cogwheels are drawn by submit.rs.
fn kinetics_casing_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::kinetics as k;
    use crate::game::kinetics_casing as c;
    use horizon_world::nbt::{JavaString, Tag};
    if !c::has_block_entity(block) {
        return;
    }
    let prop = |key: &str| game.native_block_state_property(state, key);
    let axis = prop("axis").and_then(k::axis_of);
    let cog = c::is_encased_cog(block);
    let (bottom, top) = (prop(c::BOTTOM_SHAFT) == Some("true"), prop(c::TOP_SHAFT) == Some("true"));
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    let centre = [0.5; 3];
    // A part built along y laid along the axis.
    let lay_along = |draw: &mut BlockEntityDraw| {
        if let Some((lay, by)) = axis.and_then(k::lay_along) {
            draw.rotate_about(lay, by, centre);
        }
    };
    if let Some(model) = c::static_look(block, bottom, top) {
        draw.push();
        // A hair larger than the block, so it covers our placeholder under it; turned as Create's block state turns it.
        draw.translate(centre);
        draw.scale([1.001; 3]);
        draw.translate([-0.5; 3]);
        let (x, y) = k::axis_turn(axis.unwrap_or(1));
        draw.rotate_about([0.0, 1.0, 0.0], -y, centre);
        draw.rotate_about([1.0, 0.0, 0.0], -x, centre);
        game.native_draw_model(draw, model, 0xFF_FFFF);
        draw.pop();
    }
    let (Some(axis), Some((ours, look))) = (axis, c::spinner(block)) else {
        return;
    };
    let Some(kind) = k::kind_of(block) else { return };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    // Create's model if its pack is on, ours otherwise.
    let part = |draw: &mut BlockEntityDraw, (ours, look): (&str, &str)| {
        if !game.native_draw_model(draw, look, 0xFF_FFFF) {
            game.native_draw_model(draw, ours, 0xFF_FFFF);
        }
    };
    let (dpt, phase) = k::spin_args(pos, 0, speed, k::tooth_offset(kind, axis, pos), draw.game_time);
    draw.spin(k::axis_vector(axis), centre, dpt, phase);
    draw.push();
    lay_along(draw);
    part(draw, (ours, look));
    draw.pop();
    if cog {
        // The half shafts are built along z; -90 degrees about x lays one along +y, 90 along -y.
        let (dpt, phase) = k::spin_args(pos, 1, speed, 0.0, draw.game_time);
        draw.spin(k::axis_vector(axis), centre, dpt, phase);
        for (on, degrees) in [(top, -90.0), (bottom, 90.0)] {
            if on {
                draw.push();
                lay_along(draw);
                draw.rotate_about([1.0, 0.0, 0.0], degrees, centre);
                part(draw, c::HALF_SHAFT);
                draw.pop();
            }
        }
    }
    draw.still();
    draw.keep();
}
