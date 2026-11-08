// Kinetics: shafts and cogwheels for Horizon. The rotation network's rules and
// its solver, with no game types: this file is both the crate's library (its
// tests) and the module the bundle adds to the game as `game::kinetics`.
//
// The connection rules (`modifier`) are ported from Create's
// `content/kinetics/RotationPropagator.java` (getRotationSpeedModifier,
// isLargeToLargeGear, isLargeToSmallCog) and the neighbour offsets from
// `KineticBlockEntity.addPropagationLocations` /
// `SimpleKineticBlockEntity.addPropagationLocations`,
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
// Unlike Create, which updates the network incrementally and breaks a block on
// a conflict, the whole connected network is solved again from its sources on
// every change, and a conflicting network stands still (speed 0 everywhere).

use std::collections::{HashMap, HashSet, VecDeque};

/// The test source's fixed speed, in RPM (positive: counter-clockwise seen
/// from the positive end of its axis).
pub const SOURCE_SPEED: f32 = 16.0;
/// The block entity's saved key for its speed.
pub const SPEED_KEY: &str = "speed";
/// The creative motor's saved setting, in RPM (signed; absent: 16).
pub const RPM_KEY: &str = "rpm";
// A network larger than this stands still (speed 0 everywhere) instead of being solved in part; raise it if real builds hit it.
pub const MAX_NODES: usize = 4096;

pub type Pos = [i32; 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Shaft,
    Cog,
    LargeCog,
    Source,
    /// Two shaft halves along the axis; the flag is `powered` (cut). `kind_of` says unpowered.
    Clutch(bool),
    /// As the clutch, but powered turns the far half the other way.
    Gearshift(bool),
    /// The gearbox: `axis` is the one without a shaft; the other two have a shaft out of both sides.
    Gearbox,
    /// The hand crank: its shaft is at one end of its axis only, `toward` (+1 / -1) the end, and it gives `rpm`
    /// (signed along the axis, 0 while nobody turns it). `kind_of` can only name it: the facing and the speed are the block's.
    Crank { toward: i8, rpm: i16 },
    /// The creative motor: its shaft is at one end of its axis only, `toward` (+1 / -1) the end, and it gives its
    /// setting `rpm` (signed, -256 to 256). `kind_of` can only name it: the facing and the setting are the block's.
    Motor { toward: i8, rpm: i16 },
    /// The rotation speed controller: a shaft through its block on a horizontal axis, linked to the large cogwheel
    /// above it (`controller_partner`); `rpm` is its saved target. `kind_of` can only name it.
    Controller { rpm: i16 },
    /// An encased chain drive or adjustable chain gearshift: chained to its neighbours along the axis `conn` (3:
    /// none), `ends` saying whether a chain leaves toward its negative / positive side. `signal` is the gearshift's
    /// redstone signal (0 for a plain drive). `kind_of` can only name it: the rest is the block's state.
    Chain { signal: u8, conn: u8, ends: [bool; 2] },
    /// Create's `PoweredShaftBlock`: a shaft (both ends) that is also a source, the output of an engine. It gives
    /// `rpm` (signed along the axis, 0: no source, just a shaft); `kind_of` can only name it, the speed is the block entity's.
    PoweredShaft { rpm: i16 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub kind: Kind,
    /// 0 = x, 1 = y, 2 = z.
    pub axis: usize,
    /// Whether a shaft pokes out of the end on the negative / positive side of the axis: every kind has both,
    /// except a casing's cogwheel (`bottom_shaft`, `top_shaft`), whose shafts are there only when set.
    pub shafts: [bool; 2],
}

/// The kind of block `name` (`kinetics:shaft`), none for any other block.
pub fn kind_of(name: &str) -> Option<Kind> {
    match name.strip_prefix("kinetics:")? {
        "shaft" | "speedometer" | "stressometer" | "andesite_encased_shaft" | "brass_encased_shaft" | "metal_girder_encased_shaft" | "flywheel" => Some(Kind::Shaft),
        "cogwheel" | "andesite_encased_cogwheel" | "brass_encased_cogwheel" => Some(Kind::Cog),
        "large_cogwheel" | "andesite_encased_large_cogwheel" | "brass_encased_large_cogwheel" => Some(Kind::LargeCog),
        "source" => Some(Kind::Source),
        "clutch" => Some(Kind::Clutch(false)),
        "gearshift" => Some(Kind::Gearshift(false)),
        "gearbox" => Some(Kind::Gearbox),
        // The cuckoo clocks and the drill are consumers with a shaft at the back, like a crank at rest.
        "hand_crank" | "cuckoo_clock" | "mysterious_cuckoo_clock" | "mechanical_drill" | "mechanical_piston" | "mechanical_bearing" => Some(Kind::Crank { toward: 1, rpm: 0 }),
        "creative_motor" => Some(Kind::Motor { toward: 1, rpm: 16 }),
        "rotation_speed_controller" => Some(Kind::Controller { rpm: 16 }),
        "encased_chain_drive" | "adjustable_chain_gearshift" => Some(Kind::Chain { signal: 0, conn: 3, ends: [false; 2] }),
        // The water wheels are shafts that generate: the speed is their block entity's (settle.rs).
        "powered_shaft" | "water_wheel" | "large_water_wheel" => Some(Kind::PoweredShaft { rpm: 0 }),
        _ => None,
    }
}

/// The block state property `axis` as an index.
pub fn axis_of(value: &str) -> Option<usize> {
    match value {
        "x" => Some(0),
        "y" => Some(1),
        "z" => Some(2),
        _ => None,
    }
}

/// The node for a block name and its `axis` property.
pub fn node(name: &str, axis: Option<&str>) -> Option<Node> {
    node_ends(name, axis, None, None)
}

/// `node` with the block state properties `bottom_shaft` and `top_shaft`, which a casing's cogwheel has.
pub fn node_ends(
    name: &str,
    axis: Option<&str>,
    bottom: Option<&str>,
    top: Option<&str>,
) -> Option<Node> {
    let kind = kind_of(name)?;
    let cased_cog = kind != Kind::Shaft && name.contains("_encased_");
    let end = |p: Option<&str>| !cased_cog || p == Some("true");
    Some(Node {
        kind,
        axis: axis_of(axis?)?,
        shafts: [end(bottom), end(top)],
    })
}

/// The node for a block name, its `axis` and its `powered` property (a clutch or gearshift's).
pub fn node_split(name: &str, axis: Option<&str>, powered: Option<&str>) -> Option<Node> {
    let mut n = node(name, axis)?;
    let on = powered == Some("true");
    n.kind = match n.kind {
        Kind::Clutch(_) => Kind::Clutch(on),
        Kind::Gearshift(_) => Kind::Gearshift(on),
        k => k,
    };
    Some(n)
}

/// Create's `hasShaftTowards`: whether `n` has a shaft out of its end on the positive (or negative) side of `axis`.
pub fn has_shaft_toward(n: Node, axis: usize, positive: bool) -> bool {
    if n.kind == Kind::Gearbox {
        // The gearbox's `axis` is the one without a shaft; the other two have one out of both sides.
        return n.axis != axis;
    }
    n.axis == axis && n.shafts[usize::from(positive)]
}

/// Which ends (negative, positive) of a cogwheel on `axis` at `p` get a shaft when it is cased: those whose
/// neighbour along the axis has a shaft toward it (`EncasedCogwheelBlock.handleEncasing`).
pub fn shaft_ends(p: Pos, axis: usize, get: impl Fn(Pos) -> Option<Node>) -> [bool; 2] {
    [false, true].map(|positive| {
        let mut q = p;
        q[axis] += if positive { 1 } else { -1 };
        get(q).is_some_and(|n| has_shaft_toward(n, axis, !positive))
    })
}

/// How a gearbox's face on `axis` (the positive side if `positive`) turns, against the gearbox's own speed
/// (the speed the solver stores for it): the two faces of an axis turn opposite ways, and the other axis
/// the other way round, whichever face is the input (`RotationPropagator.getAxisModifier`).
pub fn gearbox_sign(box_axis: usize, axis: usize, positive: bool) -> f32 {
    let first = (0..3).find(|&a| a != box_axis).unwrap_or(0);
    let s = if positive { 1.0 } else { -1.0 };
    if axis == first { s } else { -s }
}

fn face_sign(n: Node, axis: usize, positive: bool) -> f32 {
    if n.kind == Kind::Gearbox {
        gearbox_sign(n.axis, axis, positive)
    } else {
        1.0
    }
}

/// The creative motor at an axis, from its block state's `facing` and its saved setting.
pub fn motor_node(facing: &str, rpm: i32) -> Option<Node> {
    let (axis, toward) = match facing {
        "down" => (1, -1),
        "up" => (1, 1),
        "north" => (2, -1),
        "south" => (2, 1),
        "west" => (0, -1),
        "east" => (0, 1),
        _ => return None,
    };
    let rpm = rpm.clamp(-256, 256) as i16;
    // Its shaft is at the `toward` end only.
    Some(Node { kind: Kind::Motor { toward, rpm }, axis, shafts: [toward < 0, toward > 0] })
}

/// The rotation speed controller on `axis` (`x` or `z`) with its saved target.
pub fn controller_node(axis: &str, rpm: i32) -> Option<Node> {
    let axis = axis_of(axis).filter(|&a| a != 1)?;
    Some(Node { kind: Kind::Controller { rpm: rpm.clamp(-256, 256) as i16 }, axis, shafts: [true; 2] })
}

/// The other end of the controller link (`isLargeCogToSpeedController`): for a controller the large cogwheel on top
/// of it, for a large cogwheel on a horizontal axis the controller below it; either way the axes must differ.
fn controller_partner(p: Pos, n: Node, get: &impl Fn(Pos) -> Option<Node>) -> Option<Pos> {
    let q = match n.kind {
        Kind::Controller { .. } => add(p, [0, 1, 0]),
        Kind::LargeCog if n.axis != 1 => add(p, [0, -1, 0]),
        _ => return None,
    };
    let m = get(q)?;
    let (cog, ctrl) = if large(n) { (n, m) } else { (m, n) };
    (large(cog) && matches!(ctrl.kind, Kind::Controller { .. }) && cog.axis != 1 && cog.axis != ctrl.axis).then_some(q)
}

/// The speed the controller link hands from one side to the other (`SpeedControllerBlockEntity.getConveyedSpeed`):
/// the target, but a side already turning the target's way at least that fast keeps going; nothing without input
/// or target. shortcut: Create's pick of the side that powers the controller is reduced to "the side reached first".
pub fn conveyed(target: i32, input: f32) -> f32 {
    let t = target as f32;
    if t == 0.0 || input == 0.0 {
        return 0.0;
    }
    if (t < 0.0) == (input < 0.0) { if t < 0.0 { t.min(input) } else { t.max(input) } } else { t }
}

/// The hand crank at an axis, from its block state's `facing` (the way the handle points) and the speed
/// `turn` gave it (`HandCrankBlockEntity.getGeneratedSpeed`: `convertToDirection`, positive along a positive facing).
pub fn crank_node(facing: &str, turn: i32) -> Option<Node> {
    let (axis, out) = match facing {
        "down" => (1, -1),
        "up" => (1, 1),
        "north" => (2, -1),
        "south" => (2, 1),
        "west" => (0, -1),
        "east" => (0, 1),
        _ => return None,
    };
    let rpm = (turn.clamp(-256, 256) * out) as i16;
    // Its shaft is at the end opposite the handle.
    let toward = -out as i8;
    Some(Node { kind: Kind::Crank { toward, rpm }, axis, shafts: [toward < 0, toward > 0] })
}

/// The axis a chain drive is chained along, from its `axis` and `axis_along_first` (`ChainDriveBlock.getConnectionAxis`).
pub fn chain_axis(axis: usize, along_first: bool) -> usize {
    match (along_first, axis) {
        (true, 0) => 1,
        (true, _) => 0,
        (false, 2) => 1,
        (false, _) => 2,
    }
}

/// The node for a chain drive or gearshift from its `axis`, `part` (`none` / `start` / `middle` / `end`),
/// `axis_along_first` properties and its redstone `signal`.
pub fn chain_node(name: &str, axis: Option<&str>, part: Option<&str>, first: Option<&str>, signal: i32) -> Option<Node> {
    let mut n = node(name, axis)?;
    if !matches!(n.kind, Kind::Chain { .. }) {
        return None;
    }
    // A positive neighbour is chained to by a middle or start part, a negative one by a middle or end.
    let ends = match part {
        Some("middle") => [true, true],
        Some("start") => [false, true],
        Some("end") => [true, false],
        _ => [false, false],
    };
    let conn = if ends == [false, false] { 3 } else { chain_axis(n.axis, first == Some("true")) as u8 };
    n.kind = Kind::Chain { signal: signal.clamp(0, 15) as u8, conn, ends };
    Some(n)
}

/// A chain gearshift's speed factor at a redstone `signal` (`ChainGearshiftBlockEntity.getModifierForSignal`).
pub fn chain_factor(signal: u8) -> f32 {
    if signal == 0 { 1.0 } else { 1.0 + (f32::from(signal) + 1.0) / 16.0 }
}

/// The powered shaft on `axis` ("x"/"y"/"z") giving `rpm`, its generator's speed (`PoweredShaftBlockEntity.generatedSpeed`).
pub fn powered_shaft_node(axis: Option<&str>, rpm: i32) -> Option<Node> {
    let rpm = rpm.clamp(-256, 256) as i16;
    Some(Node { kind: Kind::PoweredShaft { rpm }, axis: axis_of(axis?)?, shafts: [true; 2] })
}

/// The speed a node gives its network; a motor set to 0 or a crank at rest gives none, so it doesn't fight another
/// source.
pub fn generated(n: Node) -> Option<f32> {
    match n.kind {
        Kind::Source => Some(SOURCE_SPEED),
        Kind::Motor { toward, rpm } => {
            let v = f32::from(rpm) * f32::from(toward);
            (v != 0.0).then_some(v)
        }
        Kind::Crank { rpm, .. } | Kind::PoweredShaft { rpm } => (rpm != 0).then_some(f32::from(rpm)),
        _ => None,
    }
}

fn small(n: Node) -> bool {
    n.kind == Kind::Cog
}

fn large(n: Node) -> bool {
    n.kind == Kind::LargeCog
}

/// Every position a node may connect to: the six faces and the twelve edge
/// diagonals (Create checks the diagonals for cogwheels only; `modifier`
/// answers 0 for everything else there).
pub const OFFSETS: [Pos; 18] = [
    [1, 0, 0],
    [-1, 0, 0],
    [0, 1, 0],
    [0, -1, 0],
    [0, 0, 1],
    [0, 0, -1],
    [1, 1, 0],
    [1, -1, 0],
    [-1, 1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [1, 0, -1],
    [-1, 0, 1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, 1, -1],
    [0, -1, 1],
    [0, -1, -1],
];

/// How `to`'s speed follows `from`'s when `to` sits at `from + diff`:
/// 1 along a shaft, -1 between two cogwheels, -2 / -0.5 between a large and a
/// small one, 0 when they don't connect. `getRotationSpeedModifier`.
pub fn modifier(from: Node, to: Node, diff: Pos) -> f32 {
    let nonzero: Vec<usize> = (0..3).filter(|&a| diff[a] != 0).collect();
    // Axis <-> axis: face neighbours along both shafts, each with a shaft toward the other
    // (`hasShaftTowards`); a gearbox turns the direction by its face (`getAxisModifier`).
    if nonzero.len() == 1 {
        let a = nonzero[0];
        if diff[a].abs() == 1
            && has_shaft_toward(from, a, diff[a] > 0)
            && has_shaft_toward(to, a, diff[a] < 0)
        {
            return face_sign(from, a, diff[a] > 0) * face_sign(to, a, diff[a] < 0);
        }
    }
    // Chain drives chained side by side (`areBlocksConnected`): `from`'s speed over `to`'s modifier, times its own.
    if let (Kind::Chain { signal: s, conn, ends }, Kind::Chain { signal: t, conn: other, .. }) = (from.kind, to.kind) {
        let linked = conn < 3
            && conn == other
            && nonzero == [conn as usize]
            && diff[conn as usize].abs() == 1
            && ends[usize::from(diff[conn as usize] > 0)];
        return if linked { chain_factor(s) / chain_factor(t) } else { 0.0 };
    }
    // Large <-> large, axes crossed (`isLargeToLargeGear`).
    if large(from) && large(to) && from.axis != to.axis {
        let third = 3 - from.axis - to.axis;
        if diff[from.axis] != 0 && diff[to.axis] != 0 && diff[third] == 0 {
            return if (diff[from.axis] > 0) ^ (diff[to.axis] > 0) {
                -1.0
            } else {
                1.0
            };
        }
        return 0.0;
    }
    // Large <-> small: same axis, one diagonal step around it (`isLargeToSmallCog`).
    let large_to_small = |l: Node, s: Node| {
        l.axis == s.axis && diff[l.axis] == 0 && (0..3).all(|a| a == l.axis || diff[a].abs() == 1)
    };
    if large(from) && small(to) && large_to_small(from, to) {
        return -2.0;
    }
    if small(from) && large(to) && large_to_small(to, from) {
        return -0.5;
    }
    // Small <-> small: face neighbours beside each other, same axis.
    if small(from)
        && small(to)
        && nonzero.len() == 1
        && diff[nonzero[0]].abs() == 1
        && nonzero[0] != from.axis
        && from.axis == to.axis
    {
        return -1.0;
    }
    0.0
}

fn add(p: Pos, d: Pos) -> Pos {
    [p[0] + d[0], p[1] + d[1], p[2] + d[2]]
}

/// A solver vertex: a position and, for a split block (clutch, gearshift), which of its two halves
/// (0 toward -axis, 1 toward +axis); 0 for every other block.
pub type Vertex = (Pos, u8);

/// Whether blocks of `kind` are two shaft halves with a switchable link between them.
pub fn is_split(kind: Kind) -> bool {
    matches!(kind, Kind::Clutch(_) | Kind::Gearshift(_))
}

/// The factor from one half of a split block to the other: a powered clutch cuts, a powered
/// gearshift reverses (`ClutchBlockEntity` / `GearshiftBlockEntity.getRotationSpeedModifier`).
pub fn through(kind: Kind) -> f32 {
    match kind {
        Kind::Clutch(true) => 0.0,
        Kind::Gearshift(true) => -1.0,
        _ => 1.0,
    }
}

/// The half of `m` that a neighbour stepping `d` to reach it meets.
fn entry_half(m: Node, d: Pos) -> u8 {
    u8::from(is_split(m.kind) && d[m.axis] < 0)
}

/// The connected neighbours of the vertex `v` (node `n`), each with the factor from
/// `v`'s speed to its own.
fn links(v: Vertex, n: Node, get: &impl Fn(Pos) -> Option<Node>) -> Vec<(Vertex, f32)> {
    let (p, half) = v;
    let mut out = Vec::new();
    // A split half touches only the neighbour past its own face, and its other half.
    let mut unit = [0; 3];
    let dirs: &[Pos] = if is_split(n.kind) {
        let f = through(n.kind);
        if f != 0.0 {
            out.push(((p, 1 - half), f));
        }
        unit[n.axis] = if half == 0 { -1 } else { 1 };
        std::slice::from_ref(&unit)
    } else {
        &OFFSETS
    };
    for &d in dirs {
        let q = add(p, d);
        let Some(m) = get(q) else { continue };
        let f = modifier(n, m, d);
        if f != 0.0 {
            out.push(((q, entry_half(m, d)), f));
        } else {
            // A one-sided rule (Create checks both ways): use the inverse.
            let back = modifier(m, n, [-d[0], -d[1], -d[2]]);
            if back != 0.0 {
                out.push(((q, entry_half(m, d)), 1.0 / back));
            }
        }
    }
    out
}

/// Solves the networks touching `changed` (positions whose block changed,
/// kinetic or not any more) and gives every node in them its speed. A network
/// with no source, or whose speeds disagree somewhere (two sources fighting,
/// a loop of gears that can't turn), stands still: speed 0 for all its nodes.
/// A split block's speed is its -axis half's; `solve_halves` has both.
pub fn solve(changed: &[Pos], get: impl Fn(Pos) -> Option<Node>) -> HashMap<Pos, f32> {
    let mut out = HashMap::new();
    for ((p, half), v) in solve_halves(changed, get) {
        if half == 0 {
            out.insert(p, v);
        } else {
            out.entry(p).or_insert(v);
        }
    }
    out
}

/// `solve` with a speed for each half of a split block: `(pos, 0)` toward -axis, `(pos, 1)` toward +axis.
pub fn solve_halves(changed: &[Pos], get: impl Fn(Pos) -> Option<Node>) -> HashMap<Vertex, f32> {
    solve_groups(changed, get).0
}

/// `solve_halves` and also the solved networks, each as its list of vertices (for the stress of `stress.rs`).
pub fn solve_groups(changed: &[Pos], get: impl Fn(Pos) -> Option<Node>) -> (HashMap<Vertex, f32>, Vec<Vec<Vertex>>) {
    let mut out = HashMap::new();
    let mut groups = Vec::new();
    let mut starts: Vec<Pos> = Vec::new();
    for &c in changed {
        starts.push(c);
        starts.extend(OFFSETS.iter().map(|&d| add(c, d)));
    }
    for s in starts {
        let Some(n) = get(s) else { continue };
        for h in 0..if is_split(n.kind) { 2 } else { 1 } {
            let start = (s, h);
            if out.contains_key(&start) {
                continue;
            }
            // Gather the network.
            let mut nodes: Vec<(Vertex, Node)> = vec![(start, n)];
            let mut seen: HashSet<Vertex> = HashSet::from([start]);
            let mut i = 0;
            while i < nodes.len() && nodes.len() < MAX_NODES {
                let (v, n) = nodes[i];
                i += 1;
                let across = controller_partner(v.0, n, &get).map(|q| (q, 0));
                for (q, _) in links(v, n, &get).into_iter().chain(across.map(|q| (q, 1.0))) {
                    if seen.insert(q) {
                        nodes.push((q, get(q.0).expect("linked node exists")));
                    }
                }
            }
            let capped = nodes.len() >= MAX_NODES && i < nodes.len();
            // Spread the sources' speed.
            let mut speed: HashMap<Vertex, f32> = HashMap::new();
            let mut queue: VecDeque<Vertex> = VecDeque::new();
            for &(v, n) in &nodes {
                if let Some(g) = generated(n) {
                    speed.insert(v, g);
                    queue.push_back(v);
                }
            }
            let mut locked = capped;
            while let Some(v) = queue.pop_front() {
                let n = get(v.0).expect("queued node exists");
                let sv = speed[&v];
                // The controller link hands a speed from one side to the other: it fills a side that has no speed
                // yet, and a side with one (a second source) must agree with it either way round, else the network locks.
                if let Some(q) = controller_partner(v.0, n, &get).map(|q| (q, 0)).filter(|q| seen.contains(q)) {
                    let target = match (n.kind, get(q.0).map(|m| m.kind)) {
                        (Kind::Controller { rpm }, _) | (_, Some(Kind::Controller { rpm })) => i32::from(rpm),
                        _ => 0,
                    };
                    match speed.get(&q) {
                        None => {
                            speed.insert(q, conveyed(target, sv));
                            queue.push_back(q);
                        }
                        Some(&other) => {
                            let agrees = |a: f32, b: f32| (a - b).abs() <= 1e-4 * b.abs().max(1.0);
                            if !agrees(conveyed(target, sv), other) && !agrees(conveyed(target, other), sv) {
                                locked = true;
                            }
                        }
                    }
                }
                for (q, f) in links(v, n, &get) {
                    if !seen.contains(&q) {
                        continue; // past the cap
                    }
                    let want = sv * f;
                    match speed.get(&q) {
                        Some(&have) if (have - want).abs() > 1e-4 * want.abs().max(1.0) => {
                            locked = true;
                        }
                        Some(_) => {}
                        None => {
                            speed.insert(q, want);
                            queue.push_back(q);
                        }
                    }
                }
                if locked {
                    break;
                }
            }
            groups.push(nodes.iter().map(|&(v, _)| v).collect());
            for (v, _) in nodes {
                let sv = if locked {
                    0.0
                } else {
                    speed.get(&v).copied().unwrap_or(0.0)
                };
                out.insert(v, sv + 0.0); // no -0.0: its bits would read as a change
            }
        }
    }
    (out, groups)
}

/// The angle in degrees a part turning at `rpm` has reached at `ticks` (game
/// time plus the partial tick): a full turn per minute per RPM, 20 ticks a second.
pub fn angle(rpm: f32, ticks: f64) -> f32 {
    ((ticks * rpm as f64 * 360.0 / 1200.0) % 360.0) as f32
}

// Positions whose block changed (`native_block_changed`), solved at the end of
// their level's tick (`native_level_tick_post`). Both run on the server thread.
static DIRTY: std::sync::Mutex<Vec<(&'static str, Pos)>> = std::sync::Mutex::new(Vec::new());

/// Marks `pos` in `dimension` (`minecraft:overworld`) to be solved again.
pub fn mark(dimension: &'static str, pos: Pos) {
    DIRTY
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push((dimension, pos));
}

/// The positions marked in `dimension` since the last call.
pub fn take(dimension: &str) -> Vec<Pos> {
    let mut dirty = DIRTY.lock().unwrap_or_else(|e| e.into_inner());
    let mut out = Vec::new();
    dirty.retain(|&(d, p)| {
        if d == dimension {
            out.push(p);
            false
        } else {
            true
        }
    });
    out
}

/// Whether block `name` gets a block entity: every part that turns. The source
/// is solved from its block alone.
pub fn has_block_entity(name: &str) -> bool {
    matches!(
        kind_of(name),
        Some(Kind::Shaft | Kind::PoweredShaft { .. } | Kind::Cog | Kind::LargeCog | Kind::Controller { .. } | Kind::Motor { .. } | Kind::Crank { .. } | Kind::Gearbox | Kind::Clutch(_) | Kind::Gearshift(_) | Kind::Chain { .. })
    )
}

/// The standalone models the block entities draw, built along the y axis.
pub const PART_MODELS: [&str; 3] = [
    "kinetics:block/shaft_part",
    "kinetics:block/cogwheel_part",
    "kinetics:block/large_cogwheel_part",
];

/// Create's own models: they exist only while the pack from `tools/create_look.py` is on (the game lays a
/// mod's files over a resource pack's, so the pack can't replace `PART_MODELS`); drawn first, ours the fallback.
pub const LOOK_MODELS: [&str; 3] = [
    "create:block/shaft",
    "create:block/cogwheel",
    "create:block/large_cogwheel",
];

pub fn look_model(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Shaft | Kind::PoweredShaft { .. } | Kind::Controller { .. } => Some(LOOK_MODELS[0]),
        Kind::Cog => Some(LOOK_MODELS[1]),
        Kind::LargeCog => Some(LOOK_MODELS[2]),
        _ => None,
    }
}

pub fn part_model(kind: Kind) -> Option<&'static str> {
    match kind {
        Kind::Shaft | Kind::PoweredShaft { .. } | Kind::Controller { .. } => Some(PART_MODELS[0]),
        Kind::Cog => Some(PART_MODELS[1]),
        Kind::LargeCog => Some(PART_MODELS[2]),
        _ => None,
    }
}

/// The light a part is drawn with, packed as `draw.light` wants it (sky light * 16 in bits 0..8, block light *
/// 16 in bits 8..16): the brightest `(block, sky)` of the samples. A solid block's own spot is dark (the
/// log-based cogwheels drew black), so the caller samples it and its six neighbours.
pub fn part_light(samples: impl IntoIterator<Item = (u8, u8)>) -> u32 {
    let (block, sky) = samples
        .into_iter()
        .fold((0u8, 0u8), |(b, s), (b2, s2)| (b.max(b2), s.max(s2)));
    (u32::from(sky) * 16) | ((u32::from(block) * 16) << 8)
}

pub fn axis_vector(axis: usize) -> [f32; 3] {
    let mut v = [0.0; 3];
    v[axis] = 1.0;
    v
}

/// The turn that lays a model built along y along `axis`: y to x is -90
/// degrees about z, y to z is 90 degrees about x.
pub fn lay_along(axis: usize) -> Option<([f32; 3], f32)> {
    match axis {
        0 => Some(([0.0, 0.0, 1.0], -90.0)),
        2 => Some(([1.0, 0.0, 0.0], 90.0)),
        _ => None,
    }
}

/// Half a tooth (22.5 degrees, 8 teeth) on every other small cogwheel, so
/// neighbours' teeth fall between each other (Create offsets them too).
pub fn tooth_offset(kind: Kind, axis: usize, pos: Pos) -> f32 {
    let sum: i32 = (0..3).filter(|&a| a != axis).map(|a| pos[a]).sum();
    if kind == Kind::Cog && sum.rem_euclid(2) == 0 {
        22.5
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(blocks: &[(Pos, Kind, usize)]) -> impl Fn(Pos) -> Option<Node> + '_ {
        move |p| {
            blocks.iter().find(|b| b.0 == p).map(|b| Node {
                kind: b.1,
                axis: b.2,
                shafts: [true; 2],
            })
        }
    }

    const X: usize = 0;
    const Y: usize = 1;
    const Z: usize = 2;
    use Kind::*;

    #[test]
    fn shafts_carry_the_source_speed_along_their_axis() {
        let w = [
            ([0, 0, 0], Source, X),
            ([1, 0, 0], Shaft, X),
            ([2, 0, 0], Shaft, X),
            ([3, 0, 0], Shaft, X),
            // Beside the line but on another axis: not connected.
            ([1, 1, 0], Shaft, Y),
        ];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[3, 0, 0]], 16.0);
        assert_eq!(s[&[1, 0, 0]], 16.0);
        assert_eq!(s.get(&[1, 1, 0]).copied().unwrap_or(0.0), 0.0);
    }

    #[test]
    fn meshing_cogwheels_turn_opposite_ways() {
        // Source, shaft, cog, cog beside it, cog beside that one.
        let w = [
            ([0, 0, 0], Source, Y),
            ([0, 1, 0], Shaft, Y),
            ([0, 2, 0], Cog, Y),
            ([1, 2, 0], Cog, Y),
            ([1, 2, 1], Cog, Y),
        ];
        let s = solve(&[[0, 2, 0]], world(&w));
        assert_eq!(s[&[0, 2, 0]], 16.0);
        assert_eq!(s[&[1, 2, 0]], -16.0);
        assert_eq!(s[&[1, 2, 1]], 16.0);
    }

    #[test]
    fn cogwheels_stacked_on_their_axis_turn_together() {
        let w = [
            ([0, 0, 0], Source, Z),
            ([0, 0, 1], Cog, Z),
            ([0, 0, 2], Cog, Z),
        ];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[0, 0, 2]], 16.0);
    }

    #[test]
    fn a_large_cogwheel_doubles_and_halves() {
        // Large at the origin (axis y), small one diagonal step around it.
        let w = [
            ([0, -1, 0], Source, Y),
            ([0, 0, 0], LargeCog, Y),
            ([1, 0, 1], Cog, Y),
        ];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[1, 0, 1]], -32.0);
        // Driven from the small side instead: half, reversed.
        let w = [
            ([1, -1, 1], Source, Y),
            ([1, 0, 1], Cog, Y),
            ([0, 0, 0], LargeCog, Y),
        ];
        let s = solve(&[[1, 0, 1]], world(&w));
        assert_eq!(s[&[0, 0, 0]], -8.0);
    }

    #[test]
    fn a_powered_shaft_is_a_source_that_is_a_shaft_too() {
        let run = |rpm: i32| {
            let shaft = Node { kind: Shaft, axis: X, shafts: [true; 2] };
            let w = [([1, 0, 0], powered_shaft_node(Some("x"), rpm).unwrap()), ([0, 0, 0], shaft), ([2, 0, 0], shaft)];
            solve(&[[1, 0, 0]], |q| w.iter().find(|b| b.0 == q).map(|b| b.1))
        };
        // Both ends carry it, in its own sign; the setting is clamped.
        let s = run(-64);
        assert_eq!((s[&[0, 0, 0]], s[&[1, 0, 0]], s[&[2, 0, 0]]), (-64.0, -64.0, -64.0));
        assert_eq!(run(999)[&[2, 0, 0]], 256.0);
        // At 0 it is a plain shaft: no source, nothing turns.
        assert_eq!(run(0).get(&[2, 0, 0]).copied().unwrap_or(0.0), 0.0);
        assert_eq!(powered_shaft_node(Some("w"), 5), None);
        assert_eq!(kind_of("kinetics:powered_shaft"), Some(PoweredShaft { rpm: 0 }));
        assert!(has_block_entity("kinetics:powered_shaft"));
    }

    #[test]
    fn a_network_past_the_cap_stands_still() {
        let line = |len: i32| {
            move |p: Pos| {
                (p[1] == 0 && p[2] == 0 && (0..len).contains(&p[0]))
                    .then_some(Node { kind: if p[0] == 0 { Source } else { Shaft }, axis: X, shafts: [true; 2] })
            }
        };
        let ok = solve(&[[0, 0, 0]], line(MAX_NODES as i32 - 1));
        assert_eq!(ok[&[5, 0, 0]], 16.0);
        let big = solve(&[[0, 0, 0]], line(MAX_NODES as i32 + 10));
        assert_eq!(big[&[5, 0, 0]], 0.0);
    }

    #[test]
    fn no_source_means_no_speed() {
        let w = [([0, 0, 0], Shaft, X), ([1, 0, 0], Shaft, X)];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[0, 0, 0]], 0.0);
        assert_eq!(s[&[1, 0, 0]], 0.0);
    }

    #[test]
    fn a_consistent_loop_of_four_cogwheels_turns() {
        let w = [
            ([0, -1, 0], Source, Y),
            ([0, 0, 0], Cog, Y),
            ([1, 0, 0], Cog, Y),
            ([1, 0, 1], Cog, Y),
            ([0, 0, 1], Cog, Y),
        ];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[1, 0, 1]], 16.0);
        assert_eq!(s[&[0, 0, 1]], -16.0);
    }

    #[test]
    fn a_loop_whose_directions_conflict_locks_the_whole_network() {
        // Source 1 drives cog A; A drives cog B the other way; B's shaft
        // reaches source 2, which turns the same way as source 1: B can't
        // turn both ways, so nothing turns.
        let w = [
            ([0, 0, 0], Source, Y),
            ([0, 1, 0], Cog, Y),
            ([1, 1, 0], Cog, Y),
            ([1, 0, 0], Source, Y),
            ([0, 2, 0], Shaft, Y),
        ];
        let s = solve(&[[0, 1, 0]], world(&w));
        for p in [[0, 0, 0], [0, 1, 0], [1, 1, 0], [1, 0, 0], [0, 2, 0]] {
            assert_eq!(s[&p], 0.0, "{p:?}");
        }
    }

    #[test]
    fn breaking_a_shaft_stops_what_was_past_it() {
        // [1,0,0] was broken: the change is solved from its neighbours.
        let w = [
            ([0, 0, 0], Source, X),
            ([2, 0, 0], Shaft, X),
            ([3, 0, 0], Shaft, X),
        ];
        let s = solve(&[[1, 0, 0]], world(&w));
        assert_eq!(s[&[0, 0, 0]], 16.0);
        assert_eq!(s[&[2, 0, 0]], 0.0);
        assert_eq!(s[&[3, 0, 0]], 0.0);
    }

    #[test]
    fn large_cogwheels_with_crossed_axes_mesh() {
        let w = [
            ([0, -1, 0], Source, Y),
            ([0, 0, 0], LargeCog, Y),
            ([1, 1, 0], LargeCog, X),
        ];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[1, 1, 0]].abs(), 16.0);
    }

    fn half_speeds(w: &[(Pos, Kind, usize)], changed: Pos) -> HashMap<Vertex, f32> {
        solve_halves(&[changed], world(w))
    }

    #[test]
    fn an_unpowered_clutch_or_gearshift_passes_the_speed_on() {
        for split in [Clutch(false), Gearshift(false)] {
            let w = [
                ([0, 0, 0], Source, X),
                ([1, 0, 0], split, X),
                ([2, 0, 0], Shaft, X),
            ];
            let s = half_speeds(&w, [1, 0, 0]);
            assert_eq!(s[&([1, 0, 0], 0)], 16.0);
            assert_eq!(s[&([1, 0, 0], 1)], 16.0);
            assert_eq!(s[&([2, 0, 0], 0)], 16.0);
        }
    }

    #[test]
    fn a_powered_clutch_cuts_what_is_past_it() {
        let w = [
            ([0, 0, 0], Source, X),
            ([1, 0, 0], Clutch(true), X),
            ([2, 0, 0], Shaft, X),
        ];
        let s = half_speeds(&w, [1, 0, 0]);
        assert_eq!(s[&([1, 0, 0], 0)], 16.0);
        assert_eq!(s[&([1, 0, 0], 1)], 0.0);
        assert_eq!(s[&([2, 0, 0], 0)], 0.0);
        assert_eq!(s[&([0, 0, 0], 0)], 16.0);
    }

    #[test]
    fn a_powered_gearshift_reverses_what_is_past_it_from_either_side() {
        let w = [
            ([0, 0, 0], Source, X),
            ([1, 0, 0], Gearshift(true), X),
            ([2, 0, 0], Shaft, X),
        ];
        let s = half_speeds(&w, [1, 0, 0]);
        assert_eq!(s[&([1, 0, 0], 0)], 16.0);
        assert_eq!(s[&([1, 0, 0], 1)], -16.0);
        assert_eq!(s[&([2, 0, 0], 0)], -16.0);
        // The source on the +x side: the -x half is the far one.
        let w = [
            ([3, 0, 0], Source, X),
            ([2, 0, 0], Gearshift(true), X),
            ([1, 0, 0], Shaft, X),
        ];
        let s = half_speeds(&w, [2, 0, 0]);
        assert_eq!(s[&([2, 0, 0], 1)], 16.0);
        assert_eq!(s[&([2, 0, 0], 0)], -16.0);
        assert_eq!(s[&([1, 0, 0], 0)], -16.0);
    }

    #[test]
    fn a_powered_clutch_keeps_two_networks_apart_and_a_powered_gearshift_makes_them_fight() {
        let between = |split| {
            [
                ([0, 0, 0], Source, X),
                ([1, 0, 0], split, X),
                ([2, 0, 0], Source, X),
            ]
        };
        let s = half_speeds(&between(Clutch(true)), [1, 0, 0]);
        assert_eq!(s[&([0, 0, 0], 0)], 16.0);
        assert_eq!(s[&([2, 0, 0], 0)], 16.0);
        let s = half_speeds(&between(Gearshift(true)), [1, 0, 0]);
        assert_eq!(s[&([0, 0, 0], 0)], 0.0);
        assert_eq!(s[&([2, 0, 0], 0)], 0.0);
        assert_eq!(s[&([1, 0, 0], 1)], 0.0);
    }

    #[test]
    fn a_split_block_joins_only_along_its_axis() {
        let w = [
            ([0, 0, 0], Source, Y),
            ([0, 1, 0], Shaft, Y),
            ([0, 2, 0], Clutch(false), X),
            ([1, 2, 0], Shaft, X),
        ];
        let s = half_speeds(&w, [0, 2, 0]);
        assert_eq!(s[&([0, 2, 0], 0)], 0.0);
        assert_eq!(s[&([1, 2, 0], 0)], 0.0);
    }

    #[test]
    fn a_powered_gearshift_in_a_cogwheel_train_reverses_the_far_side() {
        let w = [
            ([0, 0, 0], Source, Y),
            ([0, 1, 0], Gearshift(true), Y),
            ([0, 2, 0], Cog, Y),
            ([1, 2, 0], Cog, Y),
        ];
        let s = half_speeds(&w, [0, 1, 0]);
        assert_eq!(s[&([0, 2, 0], 0)], -16.0);
        assert_eq!(s[&([1, 2, 0], 0)], 16.0);
    }

    #[test]
    fn split_blocks_are_named_and_powered_from_properties() {
        assert_eq!(
            node_split("kinetics:clutch", Some("x"), Some("true")),
            Some(Node { kind: Clutch(true), axis: 0, shafts: [true; 2] })
        );
        assert_eq!(
            node_split("kinetics:gearshift", Some("y"), Some("false")),
            Some(Node { kind: Gearshift(false), axis: 1, shafts: [true; 2] })
        );
        assert!(has_block_entity("kinetics:clutch") && has_block_entity("kinetics:gearshift"));
    }

    #[test]
    fn a_crank_turns_the_shaft_at_its_back_and_nothing_else() {
        // Handle up, shaft down into the wall: the crank at the origin gives +32 along y.
        let crank = |facing, turn| (crank_node(facing, turn).unwrap(), [0, 0, 0]);
        let (c, p) = crank("up", 32);
        assert_eq!(c.kind, Kind::Crank { toward: -1, rpm: 32 });
        let w = |q: Pos| {
            if q == p {
                Some(c)
            } else if q == [0, -1, 0] || q == [0, 1, 0] || q == [1, 0, 0] {
                Some(Node { kind: Shaft, axis: if q[0] == 1 { X } else { Y }, shafts: [true; 2] })
            } else {
                None
            }
        };
        let s = solve(&[p], w);
        assert_eq!(s[&[0, -1, 0]], 32.0);
        // The handle's side and a shaft beside it get nothing.
        assert_eq!(s[&[0, 1, 0]], 0.0);
        assert_eq!(s.get(&[1, 0, 0]).copied().unwrap_or(0.0), 0.0);
    }

    #[test]
    fn a_crank_follows_its_facing_and_rests_without_locking() {
        // Facing down, the speed flips along y; north and west too (negative facing).
        assert_eq!(crank_node("down", 32).unwrap().kind, Kind::Crank { toward: 1, rpm: -32 });
        assert_eq!(crank_node("east", -32).unwrap().kind, Kind::Crank { toward: -1, rpm: -32 });
        assert_eq!(crank_node("north", 32).unwrap().axis, Z);
        assert_eq!(crank_node("sideways", 32), None);
        // At rest it is no source: the source behind it drives it.
        let w = [
            ([0, 0, 0], Source, X),
            ([1, 0, 0], Shaft, X),
        ];
        // Facing east, its shaft end is toward -x: beside the shaft at x = 1.
        let rest = crank_node("east", 0).unwrap();
        assert_eq!(rest.kind, Kind::Crank { toward: -1, rpm: 0 });
        let get = |p: Pos| if p == [2, 0, 0] { Some(rest) } else { world(&w)(p) };
        assert_eq!(solve(&[[1, 0, 0]], get)[&[2, 0, 0]], 16.0);
        // Facing west, the handle side is toward the shaft: not connected.
        let back = crank_node("west", 0).unwrap();
        let get = |p: Pos| if p == [2, 0, 0] { Some(back) } else { world(&w)(p) };
        assert_eq!(solve(&[[1, 0, 0]], get).get(&[2, 0, 0]).copied().unwrap_or(0.0), 0.0);
    }

    #[test]
    fn angle_turns_once_a_minute_per_rpm() {
        assert_eq!(angle(1.0, 1200.0), 0.0);
        assert_eq!(angle(1.0, 300.0), 90.0);
        assert_eq!(angle(-16.0, 0.0), 0.0);
    }

    #[test]
    fn a_part_takes_the_brightest_light_around_it() {
        // Inside a solid block (0, 0); a neighbour has block light 5 and full sky light.
        assert_eq!(
            part_light([(0, 0), (5, 15), (2, 3)]),
            (15 * 16) | ((5 * 16) << 8)
        );
        assert_eq!(part_light([]), 0);
    }

    #[test]
    fn marks_are_taken_once_per_dimension() {
        mark("test:a", [1, 2, 3]);
        mark("test:b", [4, 5, 6]);
        assert_eq!(take("test:a"), vec![[1, 2, 3]]);
        assert!(take("test:a").is_empty());
        assert_eq!(take("test:b"), vec![[4, 5, 6]]);
    }

    fn cased(kind: Kind, axis: usize, shafts: [bool; 2]) -> Node {
        Node { kind, axis, shafts }
    }

    fn nodes(blocks: &[(Pos, Node)]) -> impl Fn(Pos) -> Option<Node> + '_ {
        move |p| blocks.iter().find(|b| b.0 == p).map(|b| b.1)
    }

    #[test]
    fn a_cased_cogwheel_carries_speed_only_through_the_ends_with_a_shaft() {
        let w = [
            ([0, 0, 0], cased(Source, Y, [true; 2])),
            ([0, 1, 0], cased(Shaft, Y, [true; 2])),
            ([0, 2, 0], cased(Cog, Y, [true, false])),
            ([0, 3, 0], cased(Shaft, Y, [true; 2])),
        ];
        let s = solve(&[[0, 2, 0]], nodes(&w));
        assert_eq!(s[&[0, 2, 0]], 16.0);
        assert_eq!(s[&[0, 3, 0]], 0.0);
        // No shaft at all: the shaft below doesn't reach it.
        let w = [
            ([0, 0, 0], cased(Source, Y, [true; 2])),
            ([0, 1, 0], cased(Shaft, Y, [true; 2])),
            ([0, 2, 0], cased(Cog, Y, [false; 2])),
        ];
        let s = solve(&[[0, 2, 0]], nodes(&w));
        assert_eq!(s[&[0, 2, 0]], 0.0);
    }

    #[test]
    fn a_cased_cogwheel_meshes_with_its_neighbours_like_a_bare_one() {
        let w = [
            ([0, 0, 0], cased(Source, Y, [true; 2])),
            ([0, 1, 0], cased(Shaft, Y, [true; 2])),
            ([0, 2, 0], cased(Cog, Y, [true, false])),
            ([1, 2, 0], cased(Cog, Y, [false; 2])),
        ];
        let s = solve(&[[0, 2, 0]], nodes(&w));
        assert_eq!(s[&[1, 2, 0]], -16.0);
        // A cased large cogwheel with no shafts is driven by a small one diagonally beside it.
        let w = [
            ([0, 0, 0], cased(Source, Y, [true; 2])),
            ([0, 1, 0], cased(Cog, Y, [true; 2])),
            ([1, 1, 1], cased(LargeCog, Y, [false; 2])),
        ];
        let s = solve(&[[0, 1, 0]], nodes(&w));
        assert_eq!(s[&[1, 1, 1]], -8.0);
    }

    #[test]
    fn casing_a_cogwheel_puts_shafts_where_neighbours_have_one_toward_it() {
        let w = [
            ([0, 0, 0], cased(Shaft, Y, [true; 2])),
            // Above: a shaft on another axis does not count.
            ([0, 2, 0], cased(Shaft, X, [true; 2])),
        ];
        assert_eq!(shaft_ends([0, 1, 0], Y, nodes(&w)), [true, false]);
        // A cased cogwheel without a shaft toward it doesn't count either.
        let w = [([0, 2, 0], cased(Cog, Y, [false, true]))];
        assert_eq!(shaft_ends([0, 1, 0], Y, nodes(&w)), [false, false]);
        let w = [([0, 2, 0], cased(Cog, Y, [true, false]))];
        assert_eq!(shaft_ends([0, 1, 0], Y, nodes(&w)), [false, true]);
    }

    #[test]
    fn cased_blocks_are_kinetic_parts_and_cogwheels_read_their_shaft_properties() {
        let ends = |n: Option<Node>| n.map(|n| (n.kind, n.shafts));
        let n = node_ends("kinetics:andesite_encased_cogwheel", Some("y"), Some("false"), Some("true"));
        assert_eq!(ends(n), Some((Kind::Cog, [false, true])));
        let n = node_ends("kinetics:andesite_encased_large_cogwheel", Some("x"), None, None);
        assert_eq!(ends(n), Some((Kind::LargeCog, [false, false])));
        let n = node_ends("kinetics:andesite_encased_shaft", Some("z"), None, None);
        assert_eq!(ends(n), Some((Kind::Shaft, [true, true])));
        let n = node_ends("kinetics:cogwheel", Some("z"), None, None);
        assert_eq!(ends(n), Some((Kind::Cog, [true, true])));
        assert!(has_block_entity("kinetics:andesite_encased_shaft"));
    }

    #[test]
    fn block_names_and_axes() {
        assert_eq!(
            node("kinetics:cogwheel", Some("z")),
            Some(Node {
                kind: Kind::Cog,
                axis: 2,
                shafts: [true; 2]
            })
        );
        assert_eq!(node("minecraft:stone", Some("y")), None);
        assert_eq!(node("kinetics:shaft", None), None);
    }
    #[test]
    fn a_flywheel_is_a_shaft_for_the_network() {
        let n = node("kinetics:flywheel", Some("x")).unwrap();
        assert_eq!((n.kind, n.axis, n.shafts), (Kind::Shaft, 0, [true; 2]));
        assert!(has_block_entity("kinetics:flywheel"));
    }
    #[test]
    fn a_controller_sets_the_cogwheel_above_it_from_either_side() {
        let c = |t| Controller { rpm: t };
        // Controller on x with a large cogwheel on z above it; the shaft side is powered.
        let w = [([0, 0, 0], Source, X), ([1, 0, 0], c(64), X), ([1, 1, 0], LargeCog, Z), ([2, 0, 0], Shaft, X), ([2, 2, 0], Cog, Z)];
        let s = solve(&[[0, 0, 0]], world(&w));
        assert_eq!(s[&[1, 1, 0]], 64.0);
        assert_eq!(s[&[2, 0, 0]], 16.0);
        assert_eq!(s[&[2, 2, 0]], -128.0); // the cog meshes with the large one
        // The cogwheel side powered: the controller and its shaft get the target.
        let w = [([1, 1, 1], Source, Z), ([1, 1, 0], Shaft, Z), ([1, 1, -1], LargeCog, Z), ([1, 0, -1], c(-32), X), ([2, 0, -1], Shaft, X)];
        let s = solve(&[[1, 1, 1]], world(&w));
        assert_eq!(s[&[1, 0, -1]], -32.0);
        assert_eq!(s[&[2, 0, -1]], -32.0);
        // Target 0 stops the other side; a cogwheel on the controller's own axis isn't linked.
        let w = [([0, 0, 0], Source, X), ([1, 0, 0], c(0), X), ([1, 1, 0], LargeCog, Z)];
        assert_eq!(solve(&[[0, 0, 0]], world(&w))[&[1, 1, 0]], 0.0);
        let w = [([0, 0, 0], Source, X), ([1, 0, 0], c(64), X), ([1, 1, 0], LargeCog, X)];
        assert_eq!(solve(&[[0, 0, 0]], world(&w))[&[1, 1, 0]], 0.0);
    }

    #[test]
    fn sources_on_both_sides_of_a_controller_must_agree() {
        // Shaft side 16 -> target 64 on the cogwheel; a motor on the cogwheel side agrees at 64 and locks at 48.
        let net = |rpm: i16| {
            let nd = |kind, axis| Node { kind, axis, shafts: [true; 2] };
            let motor = Node { kind: Motor { toward: 1, rpm }, axis: Z, shafts: [false, true] };
            let w = [
                ([0, 0, 0], nd(Source, X)),
                ([1, 0, 0], nd(Controller { rpm: 64 }, X)),
                ([1, 1, 0], nd(LargeCog, Z)),
                ([1, 1, -1], motor),
            ];
            solve(&[[0, 0, 0]], |p| w.iter().find(|b| b.0 == p).map(|b| b.1))
        };
        assert_eq!(net(64)[&[1, 1, 0]], 64.0);
        assert_eq!(net(48)[&[1, 1, 0]], 0.0);
    }

    #[test]
    fn the_conveyed_speed_is_the_target_unless_the_input_is_already_past_it() {
        assert_eq!(conveyed(64, 16.0), 64.0);
        assert_eq!(conveyed(16, 64.0), 64.0);
        assert_eq!(conveyed(-16, -64.0), -64.0);
        assert_eq!(conveyed(-16, 64.0), -16.0);
        assert_eq!(conveyed(0, 64.0), 0.0);
        assert_eq!(conveyed(64, 0.0), 0.0);
        assert_eq!(controller_node("y", 5), None);
        assert_eq!(controller_node("z", 999).map(|n| n.kind), Some(Controller { rpm: 256 }));
    }
}
