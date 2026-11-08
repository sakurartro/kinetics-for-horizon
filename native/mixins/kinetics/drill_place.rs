// A drill is placed with its back to a shaft beside it, else its head toward the player (away when sneaking).
fn kinetics_drill_place(
    level: &GameLevel<'_>,
    state: BlockStateId,
    context: &PlacementContext,
    answer: &mut Option<Option<BlockStateId>>,
) {
    use crate::game::{kinetics as k, kinetics_drill as d, kinetics_motor as m};
    if !matches!(level.native_state_block(state), Some(d::BLOCK | crate::game::kinetics_contraption::PISTON | crate::game::kinetics_contraption::BEARING)) {
        return;
    }
    // A neighbour has a shaft toward the drill when it turns on the axis that joins them (as for the motor).
    let mut shaft = [false; 6];
    for (i, dir) in m::DIRS.iter().enumerate() {
        let at = [context.pos.x + dir[0], context.pos.y + dir[1], context.pos.z + dir[2]];
        let neighbour = level.world.block(at[0], at[1], at[2]);
        if level.native_state_block(neighbour).and_then(k::kind_of).is_none() {
            continue;
        }
        let axis = ["x", "y", "z"][m::axis(*dir)];
        shaft[i] = level.native_state_with_property(neighbour, "axis", axis) == Some(neighbour);
    }
    let look = m::nearest_looking(context.yaw, context.pitch);
    let facing = d::placement_facing(look, context.sneaking, m::preferred(shaft));
    if let Some(placed) = level.native_state_with_property(state, "facing", m::dir_name(facing)) {
        *answer = Some(Some(placed));
    }
}
