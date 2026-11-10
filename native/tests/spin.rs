// The phase for `BlockEntityDraw::spin` (runtime.rs): the game turns a part by `game_time * degrees_per_tick + phase`.
use horizon_mod_kinetics::{angle, spin_args, spin_phase};

/// Degrees a part drawn with `(dpt, phase)` shows at tick `t`.
fn shown((dpt, phase): (f32, f32), t: i64) -> f64 {
    (t as f64 * f64::from(dpt) + f64::from(phase)).rem_euclid(360.0)
}

fn near(a: f64, b: f64) -> bool {
    let d = (a - b).rem_euclid(360.0);
    d < 1e-3 || 360.0 - d < 1e-3
}

#[test]
fn a_steady_part_turns_as_the_old_angle_plus_its_tooth_offset() {
    let a = spin_args([0, 0, 0], 0, 16.0, 22.5, 1000);
    for t in [1000, 1013, 99_999] {
        assert!(near(shown(a, t), f64::from(angle(16.0, t as f64)) + 22.5));
    }
    // Asked again with nothing changed it gives the same phase.
    assert_eq!(spin_args([0, 0, 0], 0, 16.0, 22.5, 1007), a);
}

#[test]
fn a_new_speed_carries_on_from_the_current_angle() {
    let before = spin_args([1, 0, 0], 0, 16.0, 0.0, 500);
    let after = spin_args([1, 0, 0], 0, -32.0, 0.0, 777);
    assert!(near(shown(before, 777), shown(after, 777)));
    // And turns the other way from there.
    assert!(near(shown(after, 778) - shown(after, 777), -32.0 * 0.3));
}

#[test]
fn from_and_to_a_standstill_a_part_is_at_its_offset() {
    spin_args([2, 0, 0], 0, 16.0, 11.25, 10);
    let stopped = spin_args([2, 0, 0], 0, 0.0, 11.25, 50);
    assert!(near(shown(stopped, 51), 11.25));
    let restarted = spin_args([2, 0, 0], 0, 8.0, 11.25, 90);
    assert!(near(shown(restarted, 90), 11.25));
}

#[test]
fn cogs_that_change_speed_in_the_same_tick_keep_meshing() {
    // Neighbours turn opposite ways, 22.5 degrees apart: the sum of their angles stays what it was.
    let (a0, b0) = (spin_args([3, 0, 0], 0, 16.0, 22.5, 0), spin_args([4, 0, 0], 0, -16.0, 0.0, 0));
    let (a1, b1) = (spin_args([3, 0, 0], 0, 40.0, 22.5, 300), spin_args([4, 0, 0], 0, -40.0, 0.0, 300));
    for t in [300, 301, 1234] {
        assert!(near(shown(a1, t) + shown(b1, t), shown(a0, 0) + shown(b0, 0)));
    }
    assert!(spin_phase(None, 5.0, 3.0, 0.0) == 3.0);
}
