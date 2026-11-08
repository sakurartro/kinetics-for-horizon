// Every tick of a mechanical bearing (BUR.md phase 6, Create's `MechanicalBearingBlockEntity`): once it has speed it
// takes what is glued to the block in front of it and turns it about its axis (contraption_tick.rs does the turning,
// and puts it down when the bearing stops). A refused ask (nothing glued there yet) is asked again next tick.
fn kinetics_bearing_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::{kinetics as k, kinetics_contraption as c, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if &*entity.id != c::BEARING || !game.native_authoritative() {
        return;
    }
    let Some(f) = game.native_block_property(pos, "facing").as_deref().and_then(m::dir_vec) else {
        return;
    };
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    if speed == 0.0 || matches!(entity.nbt.get(&JavaString::from(c::RUNNING_KEY)), Some(Tag::Int(1))) {
        return;
    }
    c::REQUESTS.lock().unwrap().push(c::Request {
        dimension: game.dim.id().to_owned(),
        anchor: [0, 1, 2].map(|i| pos[i] + f[i]),
        by: [0; 3],
        speed: 0.0,
        extra: None,
        feedback: false,
        owner: Some((pos, c::RUNNING_KEY, 1)),
        spin: Some(f),
        cart: None,
    });
}
