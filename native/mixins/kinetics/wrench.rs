// The wrench: a right click on a kinetic block turns its axis about the clicked face, sneaking picks it up.
// A turned block is a block change like any other: its network is solved again by `kinetics_block_changed`.
// shortcut: no break event for other mods (Create posts one), no turn/remove sounds, and only our own blocks
// are wrenchable (Create's wrench_pickup tag of vanilla redstone parts is left out).
fn kinetics_wrench(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    face: [i32; 3],
    claimed: &mut bool,
) {
    use crate::game::{kinetics as k, kinetics_wrench as w};
    if *claimed || &*stack.item != w::WRENCH || game.block_action_restricted(pos) {
        return;
    }
    let block = game.native_block(pos);
    let axis = game.native_block_property(pos, "axis");
    let sneaking = game.keys.contains(&crate::game::KeyCode::ShiftLeft);
    // A large water wheel can't be turned (its body is built for its axis), only picked up.
    if block == crate::game::kinetics_water_wheel::LARGE && !sneaking {
        return;
    }
    // A cart assembler's arrow turns a quarter clockwise (Create's `CartAssemblerBlock.onWrenched`).
    if block == crate::game::kinetics_contraption::ASSEMBLER && !sneaking {
        let shape = game.native_block_property(pos, "shape").unwrap_or_default();
        if shape == "east_west" || shape == "north_south" {
            let backwards = game.native_block_property(pos, "backwards").as_deref() == Some("true");
            let (s, b) = crate::game::kinetics_contraption::assembler_turned(&shape, backwards);
            game.native_set_block_property_with(pos, "backwards", if b { "true" } else { "false" }, 3);
            game.native_set_block_property_with(pos, "shape", s, 3);
        }
        *claimed = true;
        return;
    }
    match w::action(k::kind_of(&block).is_some(), sneaking, axis.as_deref().and_then(k::axis_of), face) {
        w::Action::Nothing => return,
        w::Action::Rotate(value) => {
            // A controller's axis is horizontal only.
            if value != "y" || block != crate::game::kinetics_speed_controller::BLOCK {
                game.native_set_block_property_with(pos, "axis", value, 3);
            }
        }
        w::Action::Remove => {
            // The block's item goes to the player's inventory (nothing in creative); a full one drops it.
            let drop = !game.creative && game.native_give_items(&block, 1) == 0;
            let at = horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]);
            game.with_level(|b, level, ticks| b.destroy_block(level, ticks, at, drop));
            let _ = game.process_level_events();
        }
    }
    *claimed = true;
}
