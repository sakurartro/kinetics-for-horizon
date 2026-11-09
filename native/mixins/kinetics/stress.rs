// Kinetics: Create's stress rules, with no game types: the bundle adds this file to the game as `game::kinetics_stress`,
// and `cargo test` runs its tests (native/Cargo.toml `[[test]] stress`, native/tests/stress.rs with the solver).
//
// API for other code (e.g. a stressometer), per kinetic block entity:
//   * `settle.rs` solves a network (`runtime::solve_groups`), calls `evaluate` and saves on every member block
//     entity the floats `NetworkStress::entries()` (keys `KEYS`: capacity, stress, size, theoretical speed). They are
//     in the update tag, so the client has them too.
//   * `read(|key| float)` turns a block entity's saved floats into `BlockNetwork { stress, theoretical_speed }`:
//     `.stress.capacity` / `.stress.stress` (su), `.stress.percent()` (what a stressometer's needle shows),
//     `.stress.overstressed()`, `.stress.size`. An overstressed network is stopped: the saved `speed` is 0 for all
//     members, `theoretical_speed` is the speed it would have.
//   * `impact(block)` / `capacity(block)`: the base su per RPM of a block (`BlockStressValues`); `DISABLE_STRESS`.
//   * `Level::of(percent)`: Create's `StressImpact` (low / medium / high / overstressed).
//
// Ported from Create's `content/kinetics/KineticNetwork.java` (calculateCapacity / calculateStress: a source gives
// capacity x |generated speed|, a member costs impact x |speed|; the network is overstressed when capacity < stress),
// `KineticBlockEntity.java` (`updateFromNetwork`, `getSpeed` = 0 while overstressed, the goggles' stress lines),
// `GeneratingKineticBlockEntity.addToGoggleTooltip`, `IRotate.StressImpact`, `CStress.java` and the
// `CStress.setImpact/setCapacity` defaults in `AllBlocks.java`,
// https://github.com/Creators-of-Create/Create, branch mc1.21.1/dev, commit 4148fd8.
//
// Copyright (c) The Create Team / The Creators of Create
// Licensed under the MIT License; the full notice is in runtime.rs.
//
// What differs from Create: no TorquePropagator / persistent network ids (the network is re-solved whole on every
// change, so there are no "unloaded" members to keep totals for); the base values are constants, not a config file.

use std::collections::HashSet;

pub type Pos = [i32; 3];

/// `kinetics.disableStress` of Create's server config.
pub const DISABLE_STRESS: bool = false;

pub const CAPACITY_KEY: &str = "network_capacity";
pub const STRESS_KEY: &str = "network_stress";
pub const SIZE_KEY: &str = "network_size";
pub const THEORETICAL_KEY: &str = "theoretical_speed";
pub const KEYS: [&str; 4] = [CAPACITY_KEY, STRESS_KEY, SIZE_KEY, THEORETICAL_KEY];

/// `BlockStressValues` defaults: (block, impact, capacity), in su at 1 RPM. Create's values for the blocks it has
/// (`AllBlocks.java`: shafts, cogwheels, gearbox, clutch, gearshift, chain drives, the casings' variants and the
/// speed controller are all `setNoImpact`, so 0 and not listed); blocks not listed have none.
const VALUES: [(&str, f32, f32); 10] = [
    ("mechanical_drill", 4.0, 0.0),
    ("mechanical_piston", 4.0, 0.0),
    ("mechanical_bearing", 4.0, 0.0),
    ("creative_motor", 0.0, 16384.0),
    ("hand_crank", 0.0, 8.0),
    ("water_wheel", 0.0, 32.0),
    ("large_water_wheel", 0.0, 128.0),
    ("cuckoo_clock", 1.0, 0.0),
    ("mysterious_cuckoo_clock", 1.0, 0.0),
    // Not Create's: the engines are not ported; the powered shaft takes the steam engine's capacity.
    ("powered_shaft", 0.0, 1024.0),
];
// The test `source`, not in Create, is as strong as the creative motor.
const SOURCE_CAPACITY: f32 = 16384.0;

fn value(name: &str) -> Option<(f32, f32)> {
    let n = name.strip_prefix("kinetics:")?;
    if n == "source" {
        return Some((0.0, SOURCE_CAPACITY));
    }
    VALUES.iter().find(|v| v.0 == n).map(|v| (v.1, v.2))
}
/// A block's stress impact at 1 RPM (`BlockStressValues.getImpact`): 0 for any block without one.
pub fn impact(name: &str) -> f32 {
    value(name).map_or(0.0, |v| v.0)
}
/// A block's stress capacity at 1 RPM (`BlockStressValues.getCapacity`).
pub fn capacity(name: &str) -> f32 {
    value(name).map_or(0.0, |v| v.1)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NetworkStress {
    pub capacity: f32,
    pub stress: f32,
    pub size: usize,
}

impl NetworkStress {
    /// `capacity < stress` (Create: `updateFromNetwork`), never with stress disabled.
    pub fn overstressed(&self) -> bool {
        !DISABLE_STRESS && self.capacity < self.stress
    }
    /// stress / capacity as the stressometer shows it (0 without capacity).
    pub fn percent(&self) -> f32 {
        if self.capacity == 0.0 { 0.0 } else { self.stress / self.capacity }
    }
    /// What a member block entity saves, with the speed it would have without the stop.
    pub fn entries(&self, theoretical_speed: f32) -> [(&'static str, f32); 4] {
        [
            (CAPACITY_KEY, self.capacity),
            (STRESS_KEY, self.stress),
            (SIZE_KEY, self.size as f32),
            (THEORETICAL_KEY, theoretical_speed),
        ]
    }
}

/// The capacity and stress of one solved network, `group` being its vertices (`runtime::solve_groups`: position
/// and half). `speed(v)` is a vertex's theoretical speed (what the solver gave, before an overstress stops
/// anything), `generated(pos)` the speed a source gives (`runtime::generated`), `impact` / `capacity` a block's base
/// values by position. Capacity: sum of capacity x |generated speed| over the sources; stress: sum of impact x
/// |speed| over the blocks (`KineticNetwork.calculateCapacity` / `calculateStress`). A two-halved block counts once.
pub fn evaluate(
    group: &[(Pos, u8)],
    speed: impl Fn((Pos, u8)) -> f32,
    generated: impl Fn(Pos) -> Option<f32>,
    impact: impl Fn(Pos) -> f32,
    capacity: impl Fn(Pos) -> f32,
) -> NetworkStress {
    let mut out = NetworkStress::default();
    let mut seen: HashSet<Pos> = HashSet::new();
    for &v in group {
        if !seen.insert(v.0) {
            continue;
        }
        out.size += 1;
        out.stress += impact(v.0) * speed(v).abs();
        if let Some(g) = generated(v.0) {
            out.capacity += capacity(v.0) * g.abs();
        }
    }
    out
}

/// A block entity's saved network facts (see `NetworkStress::entries`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlockNetwork {
    pub stress: NetworkStress,
    pub theoretical_speed: f32,
}

/// Reads them back with `float(key)` (none: the block entity has not been solved yet).
pub fn read(float: impl Fn(&str) -> Option<f32>) -> Option<BlockNetwork> {
    Some(BlockNetwork {
        stress: NetworkStress {
            capacity: float(CAPACITY_KEY)?,
            stress: float(STRESS_KEY)?,
            size: float(SIZE_KEY)? as usize,
        },
        theoretical_speed: float(THEORETICAL_KEY)?,
    })
}

/// `IRotate.StressImpact`: how loaded a network is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Low,
    Medium,
    High,
    Overstressed,
}

impl Level {
    pub fn of(percent: f32) -> Level {
        if percent > 1.0 {
            Level::Overstressed
        } else if percent > 0.75 {
            Level::High
        } else if percent > 0.5 {
            Level::Medium
        } else {
            Level::Low
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_create() {
        assert_eq!(capacity("kinetics:creative_motor"), 16384.0);
        assert_eq!(capacity("kinetics:hand_crank"), 8.0);
        assert_eq!(capacity("kinetics:water_wheel"), 32.0);
        assert_eq!(capacity("kinetics:large_water_wheel"), 128.0);
        assert_eq!(impact("kinetics:cuckoo_clock"), 1.0);
        assert_eq!(impact("kinetics:shaft"), 0.0);
        assert_eq!(impact("kinetics:gearbox"), 0.0);
        assert_eq!(capacity("kinetics:unknown"), 0.0);
    }

    #[test]
    fn capacity_and_stress_follow_speed() {
        // A motor (0) giving 16 RPM and three shafts at 3 su per RPM; a block with two halves counts once.
        let g = [([0, 0, 0], 0), ([1, 0, 0], 0), ([2, 0, 0], 0), ([2, 0, 0], 1), ([3, 0, 0], 0)];
        let ns = evaluate(&g, |_| -16.0, |p| (p[0] == 0).then_some(16.0), |p| if p[0] == 0 { 0.0 } else { 3.0 }, |_| 16384.0);
        assert_eq!(ns, NetworkStress { capacity: 16384.0 * 16.0, stress: 3.0 * 3.0 * 16.0, size: 4 });
        assert!(!ns.overstressed());
    }

    #[test]
    fn overstressed_when_impact_exceeds_capacity() {
        let g = [([0, 0, 0], 0), ([1, 0, 0], 0), ([2, 0, 0], 0)];
        let ns = evaluate(&g, |_| 16.0, |p| (p[0] == 0).then_some(16.0), |_| 5.0, |_| 8.0);
        assert_eq!((ns.capacity, ns.stress), (128.0, 5.0 * 3.0 * 16.0));
        assert!(ns.overstressed());
        assert_eq!(Level::of(ns.percent()), Level::Overstressed);
    }

    #[test]
    fn no_source_means_no_capacity() {
        let ns = evaluate(&[([0, 0, 0], 0)], |_| 0.0, |_| None, |_| 9.0, |_| 9.0);
        assert_eq!((ns.capacity, ns.stress, ns.size), (0.0, 0.0, 1));
        assert!(!ns.overstressed());
    }

    #[test]
    fn levels_and_roundtrip() {
        assert_eq!(Level::of(0.5), Level::Low);
        assert_eq!(Level::of(0.51), Level::Medium);
        assert_eq!(Level::of(0.76), Level::High);
        assert_eq!(Level::of(1.0), Level::High);
        let ns = NetworkStress { capacity: 10.0, stress: 4.0, size: 7 };
        let saved: std::collections::HashMap<_, _> = ns.entries(-32.0).into_iter().collect();
        let back = read(|k| saved.get(k).copied()).unwrap();
        assert_eq!(back, BlockNetwork { stress: ns, theoretical_speed: -32.0 });
        assert_eq!(ns.percent(), 0.4);
        assert!(read(|_| None).is_none());
    }
}
