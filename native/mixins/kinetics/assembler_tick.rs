// Every tick of a cart assembler (BUR.md phase 5, Create's `CartAssemblerBlockEntity.tryAssemble`): every 8 ticks at
// most it acts on a minecart on it. Powered, it mounts what is glued above it on the cart (a contraption riding it,
// contraption_tick.rs) and sends a moving cart off at full speed; without a signal it brakes the cart (a contraption
// on it is put down by contraption_tick.rs).
fn kinetics_assembler_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::kinetics_contraption as c;
    use crate::game::minecarts::CartKind;
    use horizon_core::geometry::Vec3;
    use horizon_world::nbt::{JavaString, Tag};
    if &*entity.id != c::ASSEMBLER || !game.native_authoritative() {
        return;
    }
    // Create's assembler is straight only (`RAIL_SHAPE`: north_south, east_west). shortcut: the powered rail template's
    // shape can't be narrowed, so a slope the rail logic gives it is undone here at its next tick.
    let shape = game.native_block_property(pos, "shape").unwrap_or_default();
    if let Some(axis) = shape.strip_prefix("ascending_") {
        let straight = if matches!(axis, "east" | "west") { "east_west" } else { "north_south" };
        game.native_set_block_property_with(pos, "shape", straight, 3);
    }
    let since = match entity.nbt.get(&JavaString::from(c::SINCE_KEY)) {
        Some(Tag::Int(v)) => *v,
        _ => c::ASSEMBLER_COOLDOWN,
    };
    if since < c::ASSEMBLER_COOLDOWN {
        entity.nbt.insert(JavaString::from(c::SINCE_KEY), Tag::Int(since + 1));
        return;
    }
    // `canAssembleTo`: a cart that can be ridden, a furnace or a chest minecart, on this rail.
    let Some(i) = game.carts.iter().position(|k| {
        matches!(k.kind, CartKind::Rideable | CartKind::Furnace | CartKind::Chest)
            && [k.position.x.floor() as i32, k.position.y.floor() as i32, k.position.z.floor() as i32] == pos
    }) else {
        return;
    };
    entity.nbt.insert(JavaString::from(c::SINCE_KEY), Tag::Int(0));
    let (id, p, v) = (game.carts[i].id, game.carts[i].position, game.carts[i].velocity);
    if game.native_block_property(pos, "powered").as_deref() != Some("true") {
        let b = c::assembler_brake(pos, [p.x, p.y, p.z]);
        game.carts[i].velocity = Vec3::new(b[0], b[1], b[2]);
        return;
    }
    let shape = game.native_block_property(pos, "shape").unwrap_or_default();
    let backwards = game.native_block_property(pos, "backwards").as_deref() == Some("true");
    // A cart someone rides, or one that already carries a contraption, isn't assembled to (`getPassengers`).
    let ridden = game.riding == Some(id) || game.mobs.iter().any(|m| !m.removed && m.extra.mount == Some(id));
    let carrying = c::LIVE.lock().unwrap().iter().any(|l| l.cart == Some(id));
    if !ridden && !carrying {
        c::REQUESTS.lock().unwrap().push(c::Request {
            dimension: game.dim.id().to_owned(),
            anchor: [pos[0], pos[1] + 1, pos[2]],
            by: [0; 3],
            speed: 0.0,
            extra: None,
            feedback: false,
            owner: None,
            spin: None,
            cart: Some((id, c::assembler_yaw(&shape, backwards))),
        });
    }
    let x_axis = shape != "north_south";
    let solid = |d: i32| {
        let q = if x_axis { [pos[0] + d, pos[1], pos[2]] } else { [pos[0], pos[1], pos[2] + d] };
        game.res.facts().get(game.world.block(q[0], q[1], q[2])).full_collision
    };
    if let Some(n) = c::assembler_push(x_axis, [v.x, v.y, v.z], [solid(-1), solid(1)]) {
        game.carts[i].velocity = Vec3::new(n[0], n[1], n[2]);
    }
}
