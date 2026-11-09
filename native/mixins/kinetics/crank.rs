// Kinetics: the hand crank's rules, with no game types. This file is both a test target of the crate and the
// module the bundle adds to the game as `game::kinetics_crank`.
//
// Ported from Create's `content/kinetics/crank/HandCrankBlockEntity.java` (turn, getGeneratedSpeed, tick) and
// `HandCrankBlock.java` (getRotationSpeed, useItemOn),
// https://github.com/Creators-of-Create/Create, branch mc1.21.1/dev, commit 4148fd8.
//
// Copyright (c) The Create Team / The Creators of Create
// Licensed under the MIT License:
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.
//
// shortcut: a crank's `inUse` and direction live in memory only (Create saves them), so a world reloaded
// mid-turn finds the crank at rest; nothing to do unless that is ever seen.

use std::collections::HashMap;
use std::sync::Mutex;

pub type Pos = [i32; 3];

pub const BLOCK: &str = "kinetics:hand_crank";
/// `HandCrankBlock.getRotationSpeed`, in RPM.
pub const SPEED: i32 = 32;
/// Ticks a turn lasts after the last right click (`inUse = 10`).
pub const IN_USE: i32 = 10;
/// Food exhaustion of one turn: `getRotationSpeed() * crankHungerMultiplier` (0.01).
pub const EXHAUSTION: f32 = 0.32;
/// The ratchet sounds every this many ticks while turned (`tickAudio`).
pub const SOUND_EVERY: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Turn {
    in_use: i32,
    backwards: bool,
    age: u32,
}

// Cranks being turned, by dimension (`minecraft:overworld`); every call runs on the server thread.
type Turns = HashMap<String, HashMap<Pos, Turn>>;
static TURNS: Mutex<Option<Turns>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Turns) -> R) -> R {
    f(TURNS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new))
}

/// A player turns the crank at `pos` (`turn(back)`): `back` is a sneaking player. Whether its network must be
/// solved again (it was at rest or changed direction).
pub fn turn(dimension: &str, pos: Pos, back: bool) -> bool {
    with(|all| {
        let cranks = all.entry(dimension.to_string()).or_default();
        let update = cranks.get(&pos).is_none_or(|t| t.in_use == 0 || t.backwards != back);
        let age = cranks.get(&pos).map_or(0, |t| t.age);
        cranks.insert(pos, Turn { in_use: IN_USE, backwards: back, age });
        update
    })
}

/// The crank's speed before the facing is applied: 32 turned, -32 turned by a sneaking player, 0 at rest.
pub fn speed(dimension: &str, pos: Pos) -> i32 {
    with(|all| match all.get(dimension).and_then(|c| c.get(&pos)) {
        Some(t) if t.in_use > 0 => {
            if t.backwards {
                -SPEED
            } else {
                SPEED
            }
        }
        _ => 0,
    })
}

/// One tick of `dimension`'s cranks: those that ran out (to solve again) and those due their sound with its pitch.
pub fn tick(dimension: &str) -> (Vec<Pos>, Vec<(Pos, f32)>) {
    with(|all| {
        let (mut stopped, mut sounds) = (Vec::new(), Vec::new());
        if let Some(cranks) = all.get_mut(dimension) {
            cranks.retain(|&p, t| {
                t.in_use -= 1;
                t.age += 1;
                if t.in_use <= 0 {
                    stopped.push(p);
                    return false;
                }
                if t.age % SOUND_EVERY == 0 {
                    // `AllSoundEvents.CRANKING`: .65 + (10 - inUse) / 10.
                    sounds.push((p, 0.65 + (IN_USE - t.in_use) as f32 / 10.0));
                }
                true
            });
        }
        (stopped, sounds)
    })
}

/// The models the crank is drawn with: ours, and Create's (they exist only while the pack from
/// `tools/create_look.py` is on; the game logs the missing ones and draws ours).
pub const PART_MODEL: &str = "kinetics:block/crank_part";
/// Create's base (`HAND_CRANK_BASE`, its 5 pixel axle) is built pointing up like ours; its handle has the wall on +z.
pub const LOOK_MODELS: [&str; 2] = ["create:block/hand_crank/block", "create:block/hand_crank/handle"];

/// The axis (0 x, 1 y, 2 z) of a `facing`.
pub fn axis_of(facing: &str) -> Option<usize> {
    match facing {
        "east" | "west" => Some(0),
        "up" | "down" => Some(1),
        "north" | "south" => Some(2),
        _ => None,
    }
}

/// The turn that lays our part model (built pointing up, its shaft in the wall at the bottom) along `facing`.
pub fn lay_along(facing: &str) -> Option<([f32; 3], f32)> {
    match facing {
        "up" => None,
        "down" => Some(([1.0, 0.0, 0.0], 180.0)),
        "north" => Some(([1.0, 0.0, 0.0], -90.0)),
        "south" => Some(([1.0, 0.0, 0.0], 90.0)),
        "east" => Some(([0.0, 0.0, 1.0], -90.0)),
        _ => Some(([0.0, 0.0, 1.0], 90.0)),
    }
}

// The handle's angle as the window draws it (client side), by position.
#[derive(Clone, Copy, Debug, Default)]
struct Spin {
    tick: i64,
    angle: f32,
    vel: f32,
}
static SPINS: Mutex<Option<HashMap<Pos, Spin>>> = Mutex::new(None);

/// The angle in degrees of the handle at `pos` at `ticks` (game time plus the partial tick) while the network
/// gives it `rpm`: its speed chases the network's by a quarter each tick (`chasingAngularVelocity`), so it winds
/// up and down instead of jumping.
pub fn spin(pos: Pos, ticks: f64, rpm: f32) -> f32 {
    let target = rpm * 360.0 / 1200.0;
    let whole = ticks.floor() as i64;
    let mut all = SPINS.lock().unwrap_or_else(|e| e.into_inner());
    let s = all
        .get_or_insert_with(HashMap::new)
        .entry(pos)
        .or_insert(Spin { tick: whole, angle: 0.0, vel: target });
    if whole < s.tick || whole - s.tick > 40 {
        // Time jumped (another world, a long wait): no winding.
        *s = Spin { tick: whole, angle: s.angle, vel: target };
    }
    while s.tick < whole {
        s.vel += (target - s.vel) / 4.0;
        s.angle = (s.angle + s.vel) % 360.0;
        s.tick += 1;
    }
    (s.angle + (ticks - whole as f64) as f32 * s.vel) % 360.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_handle_winds_up_to_the_network_speed_and_down_again() {
        let p = [7, 7, 7];
        // 32 rpm is 9.6 degrees a tick; a fresh handle starts at rest.
        assert_eq!(spin(p, 100.0, 0.0), 0.0);
        let mut last = 0.0;
        for t in 101..160 {
            last = spin(p, t as f64, 32.0);
        }
        let step = (spin(p, 160.0, 32.0) - last).rem_euclid(360.0);
        assert!((step - 9.6).abs() < 0.01, "{step}");
        // Let go: it keeps turning a while, slower each tick.
        let a = spin(p, 161.0, 0.0);
        let b = spin(p, 162.0, 0.0);
        let c = spin(p, 163.0, 0.0);
        assert!((b - a).rem_euclid(360.0) > (c - b).rem_euclid(360.0));
    }

    #[test]
    fn a_facing_has_an_axis_and_a_turn_that_points_the_model_along_it() {
        assert_eq!(axis_of("west"), Some(0));
        assert_eq!(axis_of("down"), Some(1));
        assert_eq!(axis_of("north"), Some(2));
        assert_eq!(axis_of("sideways"), None);
        assert_eq!(lay_along("up"), None);
        // North points the model's up (+y) to -z: -90 degrees about x.
        assert_eq!(lay_along("north"), Some(([1.0, 0.0, 0.0], -90.0)));
    }

    #[test]
    fn a_turn_lasts_ten_ticks_and_then_stops() {
        let (d, p) = ("test:turn", [1, 2, 3]);
        assert_eq!(speed(d, p), 0);
        assert!(turn(d, p, false));
        assert_eq!(speed(d, p), 32);
        for _ in 0..9 {
            assert!(tick(d).0.is_empty());
        }
        assert_eq!(speed(d, p), 32);
        assert_eq!(tick(d).0, vec![p]);
        assert_eq!(speed(d, p), 0);
    }

    #[test]
    fn holding_the_button_keeps_it_turning_and_only_a_change_resolves() {
        let (d, p) = ("test:hold", [0, 0, 0]);
        assert!(turn(d, p, false));
        for _ in 0..4 {
            tick(d);
        }
        // Held: another click renews the time, no new solve.
        assert!(!turn(d, p, false));
        for _ in 0..9 {
            tick(d);
        }
        assert_eq!(speed(d, p), 32);
        // Sneaking turns it the other way: solved again.
        assert!(turn(d, p, true));
        assert_eq!(speed(d, p), -32);
    }

    #[test]
    fn dimensions_are_apart() {
        turn("test:a", [0, 0, 0], false);
        assert_eq!(speed("test:b", [0, 0, 0]), 0);
        assert!(tick("test:b").0.is_empty());
    }

    #[test]
    fn the_ratchet_sounds_every_ten_ticks_of_a_turn() {
        let (d, p) = ("test:sound", [9, 9, 9]);
        let mut heard = Vec::new();
        for t in 1..=25 {
            turn(d, p, false);
            if tick(d).1.iter().any(|s| s.0 == p) {
                heard.push(t);
            }
        }
        assert_eq!(heard, vec![10, 20]);
    }
}
