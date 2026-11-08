// Every tick of a cart assembler (BUR.md phase 5, Create's `CartAssemblerBlockEntity`): the rail it is. When its
// signal comes on it takes what is glued to the block above it and sets it going along the rails
// (contraption_tick.rs rides it, and puts it down on an assembler with no signal or where the rails end).
fn kinetics_assembler_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::{JavaString, Tag};
    if &*entity.id != c::ASSEMBLER || !game.native_authoritative() {
        return;
    }
    let powered = game.native_block_property(pos, "powered").as_deref() == Some("true");
    let was = matches!(entity.nbt.get(&JavaString::from(c::WAS_POWERED_KEY)), Some(Tag::Int(1)));
    if powered == was {
        return;
    }
    entity.nbt.insert(JavaString::from(c::WAS_POWERED_KEY), Tag::Int(i32::from(powered)));
    if !powered {
        return;
    }
    let shape = game.native_block_property(pos, "shape").unwrap_or_default();
    let Some(heading) = c::cart_start(pos, &shape, |p| c::is_rail(&game.res.facts().get(game.world.block(p[0], p[1], p[2])).name)) else {
        return;
    };
    c::REQUESTS.lock().unwrap().push(c::Request {
        dimension: game.dim.id().to_owned(),
        anchor: [pos[0], pos[1] + 1, pos[2]],
        by: [0; 3],
        speed: c::CART_START,
        extra: None,
        feedback: false,
        owner: None,
        spin: None,
        cart: Some(heading),
    });
}
