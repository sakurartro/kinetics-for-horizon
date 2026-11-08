// A minecart contraption item used on a rail (Create's `MinecartContraptionItem.useOn`): its cart is put there as
// the cart's own item puts one (one item used, none in creative), and its contraption rides it again, turned to face
// the way the player looks (`createAtYaw`); contraption_tick.rs takes it from there.
fn kinetics_cart_place(game: &mut crate::game::Game<'_>, stack: &ItemStack, pos: [i32; 3], face: [i32; 3], claimed: &mut bool) {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::Tag;
    let Some(cart) = c::contraption_cart(&stack.item) else { return };
    if *claimed || !game.native_authoritative() {
        return;
    }
    let data = stack.data().and_then(|d| d.other("minecraft:custom_data")).cloned();
    let hit = crate::game::Hit { pos, face, point: pos.map(f64::from) };
    // False off a rail: nothing happens, as `useOn` FAILs.
    if !game.place_minecart(&hit, &ItemStack::new(cart, 1)) {
        return;
    }
    *claimed = true;
    let Some(Tag::Compound(mut data)) = data else { return };
    let Some(k) = game.carts.last() else { return };
    let (id, at) = (k.id, [k.position.x, k.position.y, k.position.z]);
    let yaw = c::facing_yaw(game.player.as_ref().map_or(0.0, |p| p.yaw));
    data.insert("cart".into(), Tag::Int(id as i32));
    data.insert("yaw".into(), Tag::Double(yaw.to_bits()));
    let mut nbt = Compound::new();
    nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::MARKER_TAG.into())]));
    nbt.insert("data".into(), Tag::Compound(data));
    let dimension = game.dim.id().to_owned();
    // A marker a mod kept out: the item comes back rather than the contraption being lost.
    if game.native_summon(&dimension, "minecraft:marker", at, nbt).is_none() && !game.creative {
        let mut back = stack.clone();
        back.count = 1;
        let player = game.native_acting_id();
        let _ = game.native_give_stack(player, back);
    }
}
