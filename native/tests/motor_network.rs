// The creative motor in the rotation network (rules: mixins/kinetics/runtime.rs, motor.rs).
use horizon_mod_kinetics::{Kind, Node, Pos, motor_node, solve};

fn shaft(axis: usize) -> Node {
    Node { kind: Kind::Shaft, axis, shafts: [true; 2] }
}

fn run(blocks: Vec<(Pos, Node)>, changed: Pos) -> impl Fn(Pos) -> f32 {
    let speeds = solve(&[changed], |p| blocks.iter().find(|b| b.0 == p).map(|b| b.1));
    move |p| speeds.get(&p).copied().unwrap_or(0.0)
}

#[test]
fn a_motor_turns_the_shaft_at_its_front_at_its_setting() {
    let w = vec![
        ([0, 0, 0], motor_node("east", 48).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], shaft(0)),
    ];
    let s = run(w, [0, 0, 0]);
    assert_eq!(s([0, 0, 0]), 48.0);
    assert_eq!(s([2, 0, 0]), 48.0);
}

#[test]
fn a_motor_facing_the_negative_end_turns_the_other_way_round() {
    let w = vec![([0, 0, 0], motor_node("west", 48).unwrap()), ([-1, 0, 0], shaft(0))];
    let s = run(w, [0, 0, 0]);
    assert_eq!(s([-1, 0, 0]), -48.0);
    // A negative setting on a positive end is the same.
    let w = vec![([0, 0, 0], motor_node("up", -256).unwrap()), ([0, 1, 0], shaft(1))];
    assert_eq!(run(w, [0, 0, 0])([0, 1, 0]), -256.0);
}

#[test]
fn a_motor_has_no_shaft_at_its_back_or_sides() {
    let w = vec![
        ([0, 0, 0], motor_node("east", 16).unwrap()),
        ([-1, 0, 0], shaft(0)),
        ([0, 1, 0], shaft(1)),
    ];
    let s = run(w, [0, 0, 0]);
    assert_eq!(s([-1, 0, 0]), 0.0);
    assert_eq!(s([0, 1, 0]), 0.0);
}

#[test]
fn a_motor_set_to_zero_does_not_fight_another_source() {
    let w = vec![
        ([0, 0, 0], motor_node("east", 0).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], motor_node("west", 16).unwrap()),
    ];
    let s = run(w, [1, 0, 0]);
    assert_eq!(s([1, 0, 0]), -16.0);
}

#[test]
fn two_motors_facing_each_other_lock_the_network_unless_they_agree() {
    let w = vec![
        ([0, 0, 0], motor_node("east", 16).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], motor_node("west", 16).unwrap()),
    ];
    assert_eq!(run(w, [1, 0, 0])([1, 0, 0]), 0.0);
    // One reversed: they push the same way.
    let w = vec![
        ([0, 0, 0], motor_node("east", 16).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], motor_node("west", -16).unwrap()),
    ];
    assert_eq!(run(w, [1, 0, 0])([1, 0, 0]), 16.0);
}

#[test]
fn a_slower_source_turning_the_same_way_is_overpowered() {
    // Create: the faster generator wins; only one turning the other way breaks the network.
    let w = vec![
        ([0, 0, 0], motor_node("east", 32).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], motor_node("west", -16).unwrap()),
    ];
    assert_eq!(run(w, [1, 0, 0])([1, 0, 0]), 32.0);
    let w = vec![
        ([0, 0, 0], motor_node("east", 32).unwrap()),
        ([1, 0, 0], shaft(0)),
        ([2, 0, 0], motor_node("west", 16).unwrap()),
    ];
    assert_eq!(run(w, [1, 0, 0])([1, 0, 0]), 0.0);
}
