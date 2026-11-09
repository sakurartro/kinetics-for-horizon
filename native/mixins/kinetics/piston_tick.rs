// Every tick of a mechanical piston's block entity (BUR.md phase 4, Create's `MechanicalPistonBlockEntity`): turned
// one way it takes what is glued to the block in front of it out along the line of poles behind it; turned the
// other, back. The contraption machinery (contraption_tick.rs) does the moving; this only asks, once, when the
// structure is at the other end: a refused ask (nothing glued there yet) is asked again next tick. A structure
// still on its way (or stuck) is turned round where it is.
// shortcut: no head or pole drawn growing, the poles only count the reach; add them with the piston's submit.
fn kinetics_piston_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::{kinetics as k, kinetics_contraption as c, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if &*entity.id != c::PISTON || !game.native_authoritative() {
        return;
    }
    let Some(f) = game.native_block_property(pos, "facing").as_deref().and_then(m::dir_vec) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let v = c::piston_speed(speed, f);
    if v == 0.0 {
        return;
    }
    let extended = matches!(entity.nbt.get(&JavaString::from(c::EXTENDED_KEY)), Some(Tag::Int(1)));
    // Its structure while it moves: it goes to the end the piston turns toward, at the piston's speed.
    let moving = game.others.iter().map(|o| o.id).find(|id| {
        matches!(game.persist.raw.get(id).and_then(|t| t.get("data")).and_then(|d| d.get("piston")), Some(Tag::IntArray(p)) if p[..] == pos[..])
    });
    if let Some(id) = moving {
        if let Some(Tag::Compound(d)) = game.persist.raw.get_mut(&id).and_then(|t| t.get_mut("data")) {
            if let Some(end) = d.get(&JavaString::from(if v > 0.0 { "out" } else { "in" })).cloned() {
                d.insert("target".into(), end);
            }
            d.insert("speed".into(), Tag::Double(v.abs().to_bits()));
        }
        entity.nbt.insert(JavaString::from(c::EXTENDED_KEY), Tag::Int(i32::from(v > 0.0)));
        return;
    }
    if extended == (v > 0.0) {
        return;
    }
    // Back along the poles it went out on; out along the poles behind it, which are its reach.
    let saved = match entity.nbt.get(&JavaString::from(c::REACH_KEY)) {
        Some(Tag::Int(r)) if extended => Some(*r),
        _ => None,
    };
    let mut reach = 0;
    while saved.is_none() && reach < c::MAX_POLES {
        let at = [pos[0] - f[0] * (reach + 1), pos[1] - f[1] * (reach + 1), pos[2] - f[2] * (reach + 1)];
        if game.res.facts().get(game.world.block(at[0], at[1], at[2])).name.as_ref() != c::POLE {
            break;
        }
        reach += 1;
    }
    let reach = saved.unwrap_or(reach);
    if reach == 0 {
        return;
    }
    entity.nbt.insert(JavaString::from(c::REACH_KEY), Tag::Int(reach));
    // Extending, the structure starts at the block in front; retracting, `reach` blocks past it. Forward is +facing.
    let (start, dir) = if extended { (reach + 1, -1) } else { (1, 1) };
    c::REQUESTS.lock().unwrap().push(c::Request {
        dimension: game.dim.id().to_owned(),
        anchor: [0, 1, 2].map(|i| pos[i] + f[i] * start),
        by: [0, 1, 2].map(|i| f[i] * reach * dir),
        speed: v.abs(),
        extra: None,
        feedback: false,
        owner: Some((pos, c::EXTENDED_KEY, i32::from(!extended))),
        spin: None,
        cart: None,
    });
}
