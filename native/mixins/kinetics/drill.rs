// Kinetics: the mechanical drill's rules, with no game types. This file is both a test target of the crate and the
// module the bundle adds to the game as `game::kinetics_drill`.
//
// Ported from Create's `content/kinetics/base/BlockBreakingKineticBlockEntity.java` (tick, isBreakable, getBreakSpeed,
// lazyTick), `content/kinetics/drill/DrillBlock.java` and `DirectionalKineticBlock.getStateForPlacement`,
// https://github.com/Creators-of-Create/Create, branch mc1.21.1/dev, commit 4148fd8.
//
// Copyright (c) The Create Team / The Creators of Create
// Licensed under the MIT License; the full notice is in runtime.rs.
//
// What differs from Create: no damage to entities in front of it (`DrillBlock.entityInside`), no cobblestone
// generator shortcut into a belt or hopper (`optimiseCobbleGen`), and an idle drill looks at the block in front every
// tick instead of waiting for its neighbour to change or for its lazy tick (same result, without a neighbour hook).

pub const BLOCK: &str = "kinetics:mechanical_drill";
/// What a drill on a moving contraption's `block_display` shows: the drill's own model is empty (its block entity
/// draws it), this block's is the whole drill turned to its `facing`. Never placed in the world.
pub const DISPLAY: &str = "kinetics:drill_display";
/// Create's NBT keys: the crack (0 to 9) and the ticks to its next step (-1: idle until a block is there).
pub const PROGRESS_KEY: &str = "Progress";
pub const NEXT_KEY: &str = "NextTick";

/// The models the drill is drawn with: ours (the body built facing up, the head facing south), and Create's (they
/// exist only while the pack from `tools/create_look.py` is on; the game logs the missing ones and draws ours).
pub const MODELS: [&str; 2] = ["kinetics:block/drill_body_part", "kinetics:block/drill_head_part"];
pub const LOOK_MODELS: [&str; 2] = ["create:block/mechanical_drill/block", "create:block/mechanical_drill/head"];

/// `isBreakable`: not a liquid, not air, not unbreakable (hardness -1, bedrock). Create's `NON_BREAKABLE` tag is empty.
pub fn breakable(name: &str, hardness: f32, liquid: bool) -> bool {
    let air = matches!(name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air");
    !(liquid || air || hardness < 0.0)
}

/// What a drill's block entity keeps between ticks.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Breaker {
    pub progress: i32,
    pub next: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Idle,
    /// The block went before it broke: the crack goes.
    Reset,
    /// The block was hit: the crack is at this stage (1 to 9).
    Hit(i32),
    /// The block breaks.
    Broken,
}

/// One server tick of a drill turning at `speed` (RPM): `front` is the hardness of the block in front of it, none
/// when it can't be broken (`BlockBreakingKineticBlockEntity.tick`).
pub fn step(b: &mut Breaker, speed: f32, front: Option<f32>) -> Step {
    if speed == 0.0 {
        return Step::Idle;
    }
    if b.next < 0 {
        // `destroyNextTick` once something breakable is there.
        if front.is_some() {
            b.next = 1;
        }
        return Step::Idle;
    }
    let wait = b.next;
    b.next -= 1;
    if wait > 0 {
        return Step::Idle;
    }
    let Some(hardness) = front else {
        if b.progress != 0 {
            b.progress = 0;
            return Step::Reset;
        }
        return Step::Idle;
    };
    let break_speed = speed.abs() / 100.0;
    // Java's (int) of a float saturates, as Rust's `as` does (hardness 0: at once).
    // `max(1)`: a saved progress past 10 must not make the clamp panic.
    b.progress += ((break_speed / hardness) as i32).clamp(1, (10 - b.progress).max(1));
    if b.progress >= 10 {
        b.progress = 0;
        b.next = -1;
        return Step::Broken;
    }
    b.next = (hardness / break_speed) as i32;
    Step::Hit(b.progress)
}

/// The facing of a new drill (`DirectionalKineticBlock.getStateForPlacement`): its back to a neighbour's shaft
/// pointing at it (`preferred`, that side), else its head toward the player, away from them when sneaking (who
/// also ignores the neighbours). `look` is the player's nearest looking direction.
pub fn placement_facing(look: [i32; 3], sneaking: bool, preferred: Option<[i32; 3]>) -> [i32; 3] {
    let opposite = |v: [i32; 3]| [-v[0], -v[1], -v[2]];
    match preferred {
        _ if sneaking => look,
        Some(side) => opposite(side),
        None => opposite(look),
    }
}

/// The blockstate turn of a model built facing up (Create's `directionalBlockProvider`: x 180 down, 90 sideways;
/// y the side's angle + 180), in degrees, both clockwise.
pub fn blockstate_turn(facing: [i32; 3]) -> (f32, f32) {
    match facing {
        [0, 1, 0] => (0.0, 0.0),
        [0, -1, 0] => (180.0, 0.0),
        [0, 0, -1] => (90.0, 0.0),
        [1, 0, 0] => (90.0, 90.0),
        [0, 0, 1] => (90.0, 180.0),
        _ => (90.0, 270.0),
    }
}

/// A crack's breaker id, as the client keys it: negative (Create's), one per drill.
pub fn breaker_id(pos: [i32; 3]) -> i32 {
    let h = (pos[0] as u32).wrapping_mul(73_856_093) ^ (pos[1] as u32).wrapping_mul(19_349_663) ^ (pos[2] as u32).wrapping_mul(83_492_791);
    -((h & 0x3FFF_FFFF) as i32) - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(speed: f32, hardness: f32) -> (u32, Vec<Step>) {
        let mut b = Breaker::default();
        let mut steps = Vec::new();
        for t in 1..10_000 {
            let s = step(&mut b, speed, Some(hardness));
            if s != Step::Idle {
                steps.push(s);
            }
            if s == Step::Broken {
                return (t, steps);
            }
        }
        panic!("never broke");
    }

    #[test]
    fn stone_breaks_faster_at_higher_speed() {
        // Stone: hardness 1.5. At 32 RPM a hit every 5 ticks, one stage each; at 256 RPM one every tick.
        let (slow, steps) = run(32.0, 1.5);
        assert_eq!(steps.len(), 10);
        assert_eq!(steps[0], Step::Hit(1));
        let (fast, _) = run(256.0, 1.5);
        assert!(fast < slow, "{fast} < {slow}");
        assert_eq!(slow, 1 + 9 * 5);
    }

    #[test]
    fn soft_blocks_break_at_once() {
        // Hardness 0 (a torch, grass): ten stages in the first hit.
        assert_eq!(run(16.0, 0.0), (1, vec![Step::Broken]));
    }

    #[test]
    fn standing_still_or_nothing_in_front_does_nothing() {
        let mut b = Breaker::default();
        assert_eq!(step(&mut b, 0.0, Some(1.5)), Step::Idle);
        assert_eq!(b, Breaker::default());
        // Nothing to break: it goes idle, and wakes when a block comes.
        assert_eq!(step(&mut b, 64.0, None), Step::Idle);
        assert_eq!(b.next, -1);
        assert_eq!(step(&mut b, 64.0, None), Step::Idle);
        assert_eq!(step(&mut b, 64.0, Some(1.5)), Step::Idle);
        assert_eq!(b.next, 1);
        assert_eq!(step(&mut b, 64.0, Some(1.5)), Step::Idle);
        assert!(matches!(step(&mut b, 64.0, Some(1.5)), Step::Hit(_)));
    }

    #[test]
    fn a_block_gone_mid_way_resets_the_crack() {
        let mut b = Breaker::default();
        assert_eq!(step(&mut b, 32.0, Some(50.0)), Step::Hit(1));
        b.next = 0;
        assert_eq!(step(&mut b, 32.0, None), Step::Reset);
        assert_eq!(b.progress, 0);
    }

    #[test]
    fn what_cannot_be_broken() {
        assert!(breakable("minecraft:stone", 1.5, false));
        assert!(breakable("minecraft:torch", 0.0, false));
        assert!(!breakable("minecraft:bedrock", -1.0, false));
        assert!(!breakable("minecraft:water", 100.0, true));
        assert!(!breakable("minecraft:cave_air", 0.0, false));
    }

    #[test]
    fn placement() {
        let (up, north) = ([0, 1, 0], [0, 0, -1]);
        // Looking north at a wall: the head toward the player.
        assert_eq!(placement_facing(north, false, None), [0, 0, 1]);
        assert_eq!(placement_facing(north, true, None), north);
        // A shaft below: the back on it, the head up.
        assert_eq!(placement_facing(north, false, Some([0, -1, 0])), up);
        assert_eq!(placement_facing(north, true, Some([0, -1, 0])), north);
    }

    #[test]
    fn breaker_ids_are_negative() {
        for p in [[0, 0, 0], [-1, -64, 7], [i32::MAX, i32::MIN, 3]] {
            assert!(breaker_id(p) < 0);
        }
        assert_ne!(breaker_id([0, 0, 0]), breaker_id([1, 0, 0]));
    }
}
