// The game's minecarts know vanilla's four rails by name. A cart assembler is a plain rail to them, as Create's
// (a `BaseRailBlock`); what it does to a cart on it is assembler_tick.rs's.
fn kinetics_cart_rail(game: &crate::game::Game<'_>, x: i32, y: i32, z: i32) -> std::ops::ControlFlow<Option<Rail>> {
    let state = game.world.block(x, y, z);
    if &*game.res.facts().get(state).name != crate::game::kinetics_contraption::ASSEMBLER {
        return std::ops::ControlFlow::Continue(());
    }
    let shape = game.pack.assets.registry.state(state).and_then(|s| s.properties.get("shape").cloned()).unwrap_or_default();
    std::ops::ControlFlow::Break(Some(Rail { name: Arc::from("minecraft:rail"), shape: Shape::parse(&shape), powered: false }))
}
