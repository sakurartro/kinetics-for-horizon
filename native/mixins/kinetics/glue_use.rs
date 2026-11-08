// Super glue (BUR.md phase 4, Create's `SuperGlueItem`): a click on a block remembers it as one corner, a click on a
// second one glues the box between them (a `kinetics.glue` marker, contraption.rs). Sneaking with it unglues every
// box holding the clicked block.
// shortcut: no frame drawn while the glue is in hand, the box is told in chat; add it with `native_draw_cube` from a
// hook that draws in the world.
fn kinetics_glue_use(
    game: &mut crate::game::Game<'_>,
    stack: &ItemStack,
    pos: [i32; 3],
    claimed: &mut bool,
) {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::Tag;
    if *claimed || &*stack.item != c::GLUE_ITEM || !game.native_authoritative() {
        return;
    }
    *claimed = true;
    let dimension = game.dim.id().to_owned();
    if game.keys.contains(&crate::game::KeyCode::ShiftLeft) {
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
        match pending.take() {
            Some((d, a)) if d == dimension => Some(a),
            _ => {
                *pending = Some((dimension.clone(), pos));
                None
            }
        }
    };
    let Some(a) = first else {
        game.native_feedback(Text::literal(format!("glue: first corner {pos:?}")));
        return;
    };
    let g = c::Glue::new(a, pos);
    let mut d = Compound::new();
    d.insert("min".into(), Tag::IntArray(g.min.to_vec()));
    d.insert("max".into(), Tag::IntArray(g.max.to_vec()));
    let mut nbt = Compound::new();
    nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::GLUE_TAG.into())]));
    nbt.insert("data".into(), Tag::Compound(d));
    if game.native_summon(&dimension, "minecraft:marker", g.min.map(f64::from), nbt).is_some() {
        game.native_feedback(Text::literal(format!("glued {:?} to {:?}", g.min, g.max)));
    }
}
