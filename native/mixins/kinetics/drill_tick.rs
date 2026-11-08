// Every tick of a drill's block entity: it hits the block in front of it and breaks it when the crack is full
// (`BlockBreakingKineticBlockEntity.tick`, drill.rs `step`). The break is the game's `Level.destroyBlock(pos, true)`
// (Create's `BlockHelper.destroyBlock`): the block's drops without a tool, its fluid left in its place.
fn kinetics_drill_tick(game: &mut crate::game::Game<'_>, pos: [i32; 3], entity: &mut ModBlockEntity) {
    use crate::game::{kinetics as k, kinetics_drill as d, kinetics_motor as m};
    use horizon_world::nbt::{JavaString, Tag};
    if &*entity.id != d::BLOCK || !game.native_authoritative() {
        return;
    }
    let Some(facing) = game.native_block_property(pos, "facing").as_deref().and_then(m::dir_vec) else {
        return;
    };
    let front = [pos[0] + facing[0], pos[1] + facing[1], pos[2] + facing[2]];
    let speed = match entity.nbt.get(&JavaString::from(k::SPEED_KEY)) {
        Some(Tag::Float(bits)) => f32::from_bits(*bits),
        _ => 0.0,
    };
    let int = |key: &str| match entity.nbt.get(&JavaString::from(key)) {
        Some(Tag::Int(v)) => *v,
        _ => 0,
    };
    let before = d::Breaker { progress: int(d::PROGRESS_KEY), next: int(d::NEXT_KEY) };
    let state = game.world.block(front[0], front[1], front[2]);
    let facts = game.res.facts().get(state);
    let hardness = d::breakable(&facts.name, facts.hardness, facts.has_class("LiquidBlock")).then_some(facts.hardness);
    let hit = facts.sounds.hit.clone();
    let mut b = before;
    let step = d::step(&mut b, speed, hardness);
    if b != before {
        entity.nbt.insert(JavaString::from(d::PROGRESS_KEY), Tag::Int(b.progress));
        entity.nbt.insert(JavaString::from(d::NEXT_KEY), Tag::Int(b.next));
    }
    let crack = match step {
        d::Step::Idle => return,
        d::Step::Reset | d::Step::Broken => -1,
        d::Step::Hit(stage) => stage,
    };
    if step != d::Step::Reset {
        let at = pos.map(|c| f64::from(c) + 0.5);
        // SoundSource 4 is `block`.
        game.with_level(|_, level, _| level.native_play_sound(&hit, at, 4, 0.25, 1.0));
    }
    // ServerLevel.destroyBlockProgress: the window's own crack, and every player's nearby.
    let id = d::breaker_id(pos);
    game.destroy_block_progress(id, front, crack);
    if let Some(host) = game.lan.as_ref() {
        host.hub.broadcast_near(
            front.map(f64::from),
            64.0,
            &horizon_protocol::packets::play::Clientbound::BlockDestruction(horizon_protocol::packets::play::BlockDestruction {
                id,
                pos: front,
                progress: if (0..10).contains(&crack) { crack as u8 } else { 255 },
            }),
            None,
        );
    }
    if step == d::Step::Broken {
        // An error here is the level's events after the break, which the game itself only reports.
        let _ = game.destroy_block_dropping(front);
    }
}
