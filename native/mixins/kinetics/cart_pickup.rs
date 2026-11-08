// A right click on a cart a contraption rides (Create's `MinecartContraptionItem.wrenchCanBeUsedToPickUpMinecartContraptions`):
// with the wrench, sneaking or not, in survival too (`survivalContraptionPickup`), the cart and its contraption go
// into the inventory as a minecart contraption item holding the contraption's snapshot (a full inventory drops it).
// Without the wrench nobody gets in (the contraption is its passenger), as a vanilla cart with a passenger.
// shortcut: a chest cart's items drop (Create's are lost with the cart), its custom name and Create's 2 MB cap are
// left out; the snapshot is bounded by the contraption's block limit.
fn kinetics_cart_pickup(game: &mut crate::game::Game<'_>, target: Option<InteractTarget>) -> std::ops::ControlFlow<bool> {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::{JavaString, Tag};
    use std::ops::ControlFlow::{Break, Continue};
    if !game.native_authoritative() {
        return Continue(());
    }
    // The cart interact_cart picks.
    let Some((i, d)) = (match target {
        Some(t) => crate::game::Game::named_entity(&game.carts, t.id, |k| k.id).map(|i| (i, 0.0)),
        None => game.cart_hit(),
    }) else {
        return Continue(());
    };
    if target.is_none() && !game.before_block(d) {
        return Continue(());
    }
    let (id, kind) = (game.carts[i].id, game.carts[i].kind);
    let dimension = game.dim.id().to_owned();
    let Some(live) = c::LIVE.lock().unwrap().iter().find(|l| l.cart == Some(id) && l.dimension == dimension).map(|l| (l.marker, l.displays.clone()))
    else {
        return Continue(());
    };
    let item = c::contraption_item(kind.id());
    if game.held().is_none_or(|s| &*s.item != crate::game::kinetics_wrench::WRENCH) || item.is_none() {
        // `Minecart.interact`: a cart with a passenger isn't boarded.
        return if kind == CartKind::Rideable { Break(false) } else { Continue(()) };
    }
    let Some(mut data) = game.persist.raw.get(&live.0).and_then(|t| t.get("data")).cloned() else {
        return Continue(());
    };
    // `contraption.stop`: what tied it to this cart, its stall and its drills' progress go.
    if let Tag::Compound(d) = &mut data {
        d.remove(&JavaString::from("cart"));
        d.remove(&JavaString::from("stall"));
        if let Some(Tag::List(pieces)) = d.get_mut(&JavaString::from("pieces")) {
            for p in pieces {
                if let Tag::Compound(p) = p {
                    p.remove(&JavaString::from("breaker"));
                }
            }
        }
    }
    let mut stack = ItemStack::new(item.unwrap_or_default(), 1);
    stack.data_mut().set_other("minecraft:custom_data", Some(data));
    c::LIVE.lock().unwrap().retain(|l| l.marker != live.0);
    for e in live.1.into_iter().chain([live.0]) {
        game.native_remove_entity(&dimension, e);
    }
    let cart = game.carts.remove(i);
    let at = [cart.position.x, cart.position.y, cart.position.z];
    game.world.modified.insert((at[0].floor() as i32 >> 4, at[2].floor() as i32 >> 4));
    for s in cart.items.iter().flatten() {
        game.spawn_item(s.clone(), at, [0.0, 0.1, 0.0], 10);
    }
    let player = game.native_acting_id();
    if let Some(left) = game.native_give_stack(player, stack) {
        let p = game.player.as_ref().map_or(at, |p| [p.position.x, p.position.y, p.position.z]);
        game.spawn_item(left, p, [0.0; 3], 0);
    }
    game.swing_arm();
    Break(true)
}
