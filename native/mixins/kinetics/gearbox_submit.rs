// Draws what turns inside a gearbox: a half shaft out of each of its four faces, each at the speed its face
// has (Create's GearboxRenderer). The box itself is the blockstate's model.
fn kinetics_gearbox_submit(
    game: &crate::game::Game<'_>,
    pos: [i32; 3],
    block: &str,
    state: BlockStateId,
    entity: &ModBlockEntity,
    draw: &mut BlockEntityDraw,
) {
    use crate::game::kinetics as k;
    use crate::game::kinetics_gearbox as g;
    use horizon_world::nbt::{JavaString, Tag};
    if block != g::GEARBOX {
        return;
    }
    let Some(box_axis) = game.native_block_state_property(state, "axis").and_then(k::axis_of) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let centre = [0.5; 3];
    let around = [[0, 0, 0], [1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
    draw.light = k::part_light(around.map(|d| game.native_light([pos[0] + d[0], pos[1] + d[1], pos[2] + d[2]])));
    // Create's box over our placeholder, a hair larger so it covers it; turned as Create's block state turns it.
    draw.push();
    draw.translate(centre);
    draw.scale([1.001; 3]);
    draw.translate([-0.5; 3]);
    let (x, y) = k::axis_turn(box_axis);
    draw.rotate_about([0.0, 1.0, 0.0], -y, centre);
    draw.rotate_about([1.0, 0.0, 0.0], -x, centre);
    game.native_draw_model(draw, g::STATIC_LOOK, 0xFF_FFFF);
    draw.pop();
    for (face, (axis, positive)) in g::faces(box_axis).enumerate() {
        let (dpt, phase) = k::spin_args(pos, face as u8, speed * k::gearbox_sign(box_axis, axis, positive), 0.0, draw.game_time);
        draw.spin(k::axis_vector(axis), centre, dpt, phase);
        draw.push();
        if let Some((turn, degrees)) = g::lay_face(axis, positive) {
            draw.rotate_about(turn, degrees, centre);
        }
        if !game.native_draw_model(draw, g::LOOK_MODELS[0], 0xFF_FFFF) {
            game.native_draw_model(draw, g::PART_MODELS[0], 0xFF_FFFF);
        }
        draw.pop();
        draw.still();
    }
    draw.keep();
}
