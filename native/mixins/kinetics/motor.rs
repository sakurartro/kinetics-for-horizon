// Kinetics: the creative motor's rules, with no game types: the bundle adds this file to the game as
// `game::kinetics_motor`, and `cargo test` runs its tests (native/Cargo.toml `[[test]] motor`).
//
// Ported from Create's `content/kinetics/motor/CreativeMotorBlockEntity.java` (DEFAULT_SPEED 16, MAX_SPEED 256, the
// value between -256 and 256, `convertToDirection`), `CreativeMotorBlock.java` and `DirectionalKineticBlock.java`
// (`getPreferredFacing`, `getStateForPlacement`), and `KineticScrollValueBehaviour.java` (a negative value turns
// clockwise, the label shows the absolute value),
// https://github.com/Creators-of-Create/Create, branch mc1.21.1/dev, commit 4148fd8.
//
// Copyright (c) The Create Team / The Creators of Create
// Licensed under the MIT License; the full notice is in runtime.rs.
//
// What differs from Create: its 1.21.1 motor sets the speed on a value-settings board (hold the right button on the
// value box, drag to a value); here the wheel over a side of the motor changes it (1 per step, 16 with Shift) and the
// right button with both hands empty reverses it. The value box is a line under the crosshair.

use std::sync::Mutex;

pub type Pos = [i32; 3];

pub const BLOCK: &str = "kinetics:creative_motor";
/// The mod message the client sends when the wheel turns over a motor: x, y, z (i32) and the change (i16), big endian.
pub const CHANNEL: &str = "kinetics:motor";
pub const DEFAULT_RPM: i32 = 16;
pub const MAX_RPM: i32 = 256;
/// The change of one wheel step, and with Shift held.
pub const STEP: i32 = 1;
pub const BIG_STEP: i32 = 16;
/// How far from the motor the server still takes the wheel from a player it can place (blocks).
pub const REACH: f64 = 8.0;

/// The standalone models the motor's block entity draws: ours (body built facing south, half a shaft from the middle
/// to the south end), and Create's, which exist only while the pack from `tools/create_look.py` is on.
pub const MODELS: [&str; 2] = ["kinetics:block/motor_part", "kinetics:block/shaft_half_part"];
pub const LOOK_MODELS: [&str; 3] = [
    "create:block/creative_motor/block",
    "create:block/creative_motor/block_vertical",
    "create:block/shaft_half",
];

pub fn clamp(rpm: i32) -> i32 {
    rpm.clamp(-MAX_RPM, MAX_RPM)
}

/// The setting after `steps` wheel steps (positive: up), `big` for Shift.
pub fn scrolled(rpm: i32, steps: i32, big: bool) -> i32 {
    past_zero(clamp(rpm.saturating_add(steps.saturating_mul(if big { BIG_STEP } else { STEP }))), steps)
}

/// Create's setting is never 0 (`KineticScrollValueBehaviour.setValueSettings`: at least 1): a step onto 0 goes on to
/// 1 the same way.
fn past_zero(rpm: i32, step: i32) -> i32 {
    if rpm == 0 { step.signum() } else { rpm }
}

/// The speed the motor gives along the positive end of its axis: Create's `convertToDirection`, which turns the
/// setting around for a motor facing the negative end.
pub fn output(rpm: i32, toward: i32) -> f32 {
    (clamp(rpm) * toward.signum()) as f32
}

/// The setting as the value box shows it: its size, and the way (negative: clockwise).
pub fn label(rpm: i32) -> String {
    format!("{} RPM {}", rpm.abs(), if rpm < 0 { "\u{27f3}" } else { "\u{27f2}" })
}

/// What a player asked of a motor's setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    Add(i32),
    Reverse,
}

pub fn apply(rpm: i32, op: Op) -> i32 {
    match op {
        Op::Add(change) => past_zero(clamp(rpm.saturating_add(change)), change),
        Op::Reverse => -clamp(rpm),
    }
}

pub fn encode(pos: Pos, change: i32) -> [u8; 14] {
    let mut out = [0u8; 14];
    for i in 0..3 {
        out[i * 4..i * 4 + 4].copy_from_slice(&pos[i].to_be_bytes());
    }
    out[12..].copy_from_slice(&(change.clamp(-512, 512) as i16).to_be_bytes());
    out
}

/// A client's bytes are its own input: anything but exactly the right length is dropped.
pub fn decode(data: &[u8]) -> Option<(Pos, i32)> {
    if data.len() != 14 {
        return None;
    }
    let word = |i: usize| i32::from_be_bytes([data[i], data[i + 1], data[i + 2], data[i + 3]]);
    let change = i32::from(i16::from_be_bytes([data[12], data[13]]));
    Some(([word(0), word(4), word(8)], change.clamp(-512, 512)))
}

pub fn in_reach(player: [f64; 3], pos: Pos) -> bool {
    (0..3).map(|a| (player[a] - f64::from(pos[a]) - 0.5).powi(2)).sum::<f64>() <= REACH * REACH
}

// Changes asked for (the wheel's message, the right button) and applied by the server at the end of a level's tick:
// the sender's level isn't known when the message arrives, so a change waits until a level has the motor.
struct Pending {
    pos: Pos,
    op: Op,
    ttl: u8,
}
static PENDING: Mutex<Vec<Pending>> = Mutex::new(Vec::new());
const PENDING_CAP: usize = 256;
/// Level passes a change waits for its motor: a few ticks of the three levels.
const PENDING_TTL: u8 = 12;

pub fn push(pos: Pos, op: Op) {
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    // One reversal per motor: the click fires for both hands, and for a client's prediction beside the server.
    if op == Op::Reverse && pending.iter().any(|p| p.pos == pos && p.op == op) {
        return;
    }
    if pending.len() < PENDING_CAP {
        pending.push(Pending { pos, op, ttl: PENDING_TTL });
    }
}

/// The changes for motors `is_motor` finds in the level being ticked; the others wait, or are dropped once they
/// have waited out their passes.
pub fn take(is_motor: impl Fn(Pos) -> bool) -> Vec<(Pos, Op)> {
    let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    pending.retain_mut(|p| {
        if is_motor(p.pos) {
            out.push((p.pos, p.op));
            return false;
        }
        p.ttl -= 1;
        p.ttl > 0
    });
    out
}

/// The unit vector of a `facing` value.
pub fn dir_vec(name: &str) -> Option<[i32; 3]> {
    Some(match name {
        "down" => [0, -1, 0],
        "up" => [0, 1, 0],
        "north" => [0, 0, -1],
        "south" => [0, 0, 1],
        "west" => [-1, 0, 0],
        "east" => [1, 0, 0],
        _ => return None,
    })
}

pub fn dir_name(v: [i32; 3]) -> &'static str {
    match v {
        [0, -1, 0] => "down",
        [0, 1, 0] => "up",
        [0, 0, -1] => "north",
        [0, 0, 1] => "south",
        [-1, 0, 0] => "west",
        _ => "east",
    }
}

/// Vanilla's `Direction.values()` order, which `getPreferredFacing` walks.
pub const DIRS: [[i32; 3]; 6] = [[0, -1, 0], [0, 1, 0], [0, 0, -1], [0, 0, 1], [-1, 0, 0], [1, 0, 0]];

pub fn axis(v: [i32; 3]) -> usize {
    (0..3).find(|&a| v[a] != 0).unwrap_or(1)
}

fn opposite(v: [i32; 3]) -> [i32; 3] {
    [-v[0], -v[1], -v[2]]
}

/// The direction the player looks along (`getNearestLookingDirection`), from their yaw and pitch in degrees.
pub fn nearest_looking(yaw: f32, pitch: f32) -> [i32; 3] {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());
    let look = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
    let a = (0..3).max_by(|&i, &j| look[i].abs().total_cmp(&look[j].abs())).unwrap_or(1);
    let mut v = [0; 3];
    v[a] = if look[a] < 0.0 { -1 } else { 1 };
    v
}

/// The side whose neighbour has a shaft pointing at the new motor (`getPreferredFacing`); none when none does, or
/// when two sides on different axes do. `shaft[i]` is for `DIRS[i]`.
pub fn preferred(shaft: [bool; 6]) -> Option<[i32; 3]> {
    let mut found: Option<[i32; 3]> = None;
    for (i, side) in DIRS.iter().enumerate() {
        if !shaft[i] {
            continue;
        }
        if found.is_some_and(|f| axis(f) != axis(*side)) {
            return None;
        }
        found = Some(*side);
    }
    found
}

/// The side the new motor's shaft faces: toward a neighbouring shaft, else toward the player (away from them
/// when sneaking, who then also ignores the neighbours).
pub fn placement_facing(look: [i32; 3], sneaking: bool, shaft: [bool; 6]) -> [i32; 3] {
    match preferred(shaft) {
        Some(side) if !sneaking => side,
        _ if sneaking => look,
        _ => opposite(look),
    }
}

/// The turn that takes a model built facing south (+z) to face `to`: an axis and degrees about the block's centre.
pub fn turn_from_south(to: [i32; 3]) -> Option<([f32; 3], f32)> {
    match to {
        [0, 0, 1] => None,
        [0, 0, -1] => Some(([0.0, 1.0, 0.0], 180.0)),
        [1, 0, 0] => Some(([0.0, 1.0, 0.0], 90.0)),
        [-1, 0, 0] => Some(([0.0, 1.0, 0.0], -90.0)),
        [0, 1, 0] => Some(([1.0, 0.0, 0.0], -90.0)),
        _ => Some(([1.0, 0.0, 0.0], 90.0)),
    }
}

/// The turn that takes Create's vertical body (built facing up) to face `to`, an up or down side.
pub fn turn_from_up(to: [i32; 3]) -> Option<([f32; 3], f32)> {
    (to == [0, -1, 0]).then_some(([1.0, 0.0, 0.0], 180.0))
}

/// The face of the block a point on its surface lies on: its axis and sign, from the point relative to the
/// block's corner.
pub fn hit_side(local: [f64; 3]) -> Option<[i32; 3]> {
    const EDGE: f64 = 1e-3;
    for a in 0..3 {
        let sign = if local[a] < EDGE {
            -1
        } else if local[a] > 1.0 - EDGE {
            1
        } else {
            continue;
        };
        let mut v = [0; 3];
        v[a] = sign;
        return Some(v);
    }
    None
}

/// Whether the motor's value box sits on `side` (`isSideActive`): any side but its two ends, and not the underside
/// of a motor lying on its side.
pub fn box_side(facing: [i32; 3], side: [i32; 3]) -> bool {
    axis(side) != axis(facing) && !(axis(facing) != 1 && side == [0, -1, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_setting_stays_within_256_either_way() {
        assert_eq!(scrolled(250, 1, true), 256);
        assert_eq!(scrolled(-250, -1, true), -256);
        assert_eq!(scrolled(16, 3, false), 19);
        assert_eq!(scrolled(1, -2, false), -1);
        assert_eq!(scrolled(1, -1, false), -1);
        assert_eq!(scrolled(-1, 1, false), 1);
        assert_eq!(scrolled(i32::MAX, i32::MAX, true), 256);
        assert_eq!(clamp(1000), 256);
    }

    #[test]
    fn reversing_negates_and_the_negative_end_turns_the_output_around() {
        assert_eq!(apply(16, Op::Reverse), -16);
        assert_eq!(apply(-256, Op::Reverse), 256);
        assert_eq!(apply(16, Op::Add(500)), 256);
        assert_eq!(output(16, 1), 16.0);
        assert_eq!(output(16, -1), -16.0);
        assert_eq!(output(0, -1), 0.0);
    }

    #[test]
    fn the_label_shows_the_size_and_the_way() {
        assert_eq!(label(16), "16 RPM \u{27f2}");
        assert_eq!(label(-32), "32 RPM \u{27f3}");
    }

    #[test]
    fn a_message_round_trips_and_a_bad_one_is_dropped() {
        assert_eq!(decode(&encode([-5, 70, 1234], -16)), Some(([-5, 70, 1234], -16)));
        assert_eq!(decode(&encode([0, 0, 0], 100_000)), Some(([0, 0, 0], 512)));
        assert_eq!(decode(&[0; 13]), None);
        assert_eq!(decode(&[0; 15]), None);
    }

    #[test]
    fn the_reach_is_measured_to_the_block_centre() {
        assert!(in_reach([0.5, 1.5, 0.5], [0, 0, 0]));
        assert!(!in_reach([20.0, 0.0, 0.0], [0, 0, 0]));
    }

    #[test]
    fn changes_wait_for_their_motor_and_expire() {
        push([900, 0, 0], Op::Add(1));
        push([900, 0, 0], Op::Reverse);
        push([900, 0, 0], Op::Reverse); // one per motor
        push([901, 0, 0], Op::Add(2));
        // A level without them: they stay.
        assert!(take(|_| false).is_empty());
        let got = take(|p| p == [900, 0, 0]);
        assert_eq!(got, vec![([900, 0, 0], Op::Add(1)), ([900, 0, 0], Op::Reverse)]);
        // The other one is dropped once it has waited out its passes.
        for _ in 0..PENDING_TTL {
            take(|_| false);
        }
        assert!(take(|_| true).is_empty());
    }

    #[test]
    fn the_player_looks_along_the_biggest_part_of_their_view() {
        assert_eq!(nearest_looking(0.0, 0.0), [0, 0, 1]); // yaw 0: south
        assert_eq!(nearest_looking(90.0, 0.0), [-1, 0, 0]); // west
        assert_eq!(nearest_looking(180.0, 0.0), [0, 0, -1]);
        assert_eq!(nearest_looking(-90.0, 0.0), [1, 0, 0]);
        assert_eq!(nearest_looking(0.0, 90.0), [0, -1, 0]); // straight down
        assert_eq!(nearest_looking(0.0, -90.0), [0, 1, 0]);
    }

    #[test]
    fn a_motor_faces_a_shaft_beside_it_else_the_player() {
        let none = [false; 6];
        // Looking south: the shaft faces the player (north); sneaking, away (south).
        assert_eq!(placement_facing([0, 0, 1], false, none), [0, 0, -1]);
        assert_eq!(placement_facing([0, 0, 1], true, none), [0, 0, 1]);
        // A shaft to the east (DIRS[5]) pulls the shaft toward it, unless sneaking.
        let mut east = none;
        east[5] = true;
        assert_eq!(placement_facing([0, 0, 1], false, east), [1, 0, 0]);
        assert_eq!(placement_facing([0, 0, 1], true, east), [0, 0, 1]);
        // Two shafts on one axis: the later side; on different axes: none of them.
        let mut both = east;
        both[4] = true;
        assert_eq!(preferred(both), Some([1, 0, 0]));
        let mut crossed = east;
        crossed[1] = true;
        assert_eq!(preferred(crossed), None);
        assert_eq!(placement_facing([0, 0, 1], false, crossed), [0, 0, -1]);
    }

    #[test]
    fn facing_names_and_turns_agree() {
        for d in DIRS {
            assert_eq!(dir_vec(dir_name(d)), Some(d));
        }
        assert_eq!(turn_from_south([0, 0, 1]), None);
        assert_eq!(turn_from_south([0, 0, -1]), Some(([0.0, 1.0, 0.0], 180.0)));
        assert_eq!(turn_from_up([0, 1, 0]), None);
        assert_eq!(turn_from_up([0, -1, 0]), Some(([1.0, 0.0, 0.0], 180.0)));
    }

    #[test]
    fn the_value_box_is_on_the_sides_not_the_ends() {
        assert_eq!(hit_side([0.5, 1.0, 0.3]), Some([0, 1, 0]));
        assert_eq!(hit_side([0.0, 0.5, 0.3]), Some([-1, 0, 0]));
        assert_eq!(hit_side([0.5, 0.5, 0.5]), None);
        let north = [0, 0, -1];
        assert!(box_side(north, [1, 0, 0]));
        assert!(box_side(north, [0, 1, 0]));
        assert!(!box_side(north, [0, -1, 0])); // underside of a lying motor
        assert!(!box_side(north, [0, 0, 1])); // the shaft's end
        assert!(!box_side(north, [0, 0, -1])); // the back
        assert!(box_side([0, 1, 0], [0, 0, 1])); // standing: every side
        assert!(!box_side([0, 1, 0], [0, 1, 0]));
    }
}
