// Super glue (BUR.md phase 4, Create's `SuperGlueItem`, `SuperGlueSelectionHandler`): a click on a block remembers it
// as the player's first corner, a click on a second one, less than 24 blocks away, glues the box between them (a
// `kinetics.glue` marker, contraption.rs) and wears the glue by one (not in creative). Sneaking with it forgets the
// first corner and unglues every box holding the clicked block.
// shortcut: no frame drawn while the glue is in hand, the box is told in chat; add it with `native_draw_cube` from a
// hook that draws in the world. Glue held in the off hand glues without wearing (Horizon wears the main hand only).
fn kinetics_glue_use(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    claimed: &mut bool,
) {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::Tag;
    if *claimed || &*stack.item != c::GLUE_ITEM || !game.native_authoritative() || game.block_action_restricted(pos) {
        return;
    }
    *claimed = true;
    let dimension = game.dim.id().to_owned();
    let player = game.native_acting_id();
    // The acting player's own sneak (a LAN guest's too: their state is swapped in).
    if game.player.as_ref().is_some_and(|p| p.shift_key_down) {
        c::PENDING.lock().unwrap().retain(|p| p.0 != player);
        let mut gone = 0;
        let hit: Vec<u32> = game
            .others
            .iter()
            .filter(|o| o.kind == "minecraft:marker")
            .filter(|o| {
                let raw = game.persist.raw.get(&o.id);
                let glue = matches!(raw.and_then(|t| t.get("Tags")), Some(Tag::List(l)) if l.iter().any(|t| matches!(t, Tag::String(s) if s.to_string_lossy() == c::GLUE_TAG)));
                let corner = |key: &str| match raw.and_then(|t| t.get("data")).and_then(|d| d.get(key)) {
                    Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
                    _ => None,
                };
                glue && matches!((corner("min"), corner("max")), (Some(a), Some(b)) if c::Glue::new(a, b).contains(pos))
            })
            .map(|o| o.id)
            .collect();
        for id in hit {
            game.native_remove_entity(&dimension, id);
            gone += 1;
        }
        game.native_feedback(Text::literal(format!("{gone} glued boxes removed")));
        return;
    }
    let first = {
        let mut pending = c::PENDING.lock().unwrap();
        let i = pending.iter().position(|p| p.0 == player);
        match i.map(|i| pending[i].clone()) {
            Some((_, d, a)) if d == dimension && !c::glue_in_reach(a, pos) => Err(a),
            Some((_, d, a)) if d == dimension => {
                pending.retain(|p| p.0 != player);
                Ok(Some(a))
            }
            _ => {
                pending.retain(|p| p.0 != player);
                pending.push((player, dimension.clone(), pos));
                Ok(None)
            }
        }
    };
    let a = match first {
        Ok(Some(a)) => a,
        Ok(None) => {
            game.native_feedback(Text::literal(format!("glue: first corner {pos:?}")));
            return;
        }
        // Create keeps the first corner and refuses the second.
        Err(a) => {
            game.native_feedback(Text::literal(format!("glue: {pos:?} is too far from {a:?}")));
            return;
        }
    };
    let g = c::Glue::new(a, pos);
    let mut d = Compound::new();
    d.insert("min".into(), Tag::IntArray(g.min.to_vec()));
    d.insert("max".into(), Tag::IntArray(g.max.to_vec()));
    let mut nbt = Compound::new();
    nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::GLUE_TAG.into())]));
    nbt.insert("data".into(), Tag::Compound(d));
    if game.native_summon(&dimension, "minecraft:marker", g.min.map(f64::from), nbt).is_some() {
        if game.native_held().is_some_and(|h| h.0 == c::GLUE_ITEM) {
            game.damage_held(1);
        }
        game.native_feedback(Text::literal(format!("glued {:?} to {:?}", g.min, g.max)));
    }
}
