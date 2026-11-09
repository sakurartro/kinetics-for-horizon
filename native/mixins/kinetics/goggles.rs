// Kinetics: the engineer's goggles' rules, with no game types: the bundle adds this file to the game as
// `game::kinetics_goggles`, and `cargo test` runs its tests (native/Cargo.toml `[[test]] goggles`).
//
// Ported from Create's `content/equipment/goggles/GogglesItem.java` (`isWearingGoggles`: the helmet slot holds the
// goggles) and `GoggleOverlayRenderer.java` (looking at a kinetic block shows its speed),
// https://github.com/Creators-of-Create/Create, branch mc1.21.1/dev, commit 4148fd8.
//
// Copyright (c) The Create Team / The Creators of Create
// Licensed under the MIT License; the full notice is in runtime.rs.
//
// Stress lines: `KineticBlockEntity.addToTooltip` / `addToGoggleTooltip` / `addStressImpactStats` and
// `GeneratingKineticBlockEntity.addToGoggleTooltip`, with the values of stress.rs.
//
// What differs from Create: English only; a generator's speed is its block's speed; no fade-in or goggles icon.

pub const GOGGLES: &str = "kinetics:goggles";

/// The speed to show for a block entity's two saved speeds (a clutch or gearshift has a second half): the one
/// that turns faster, so a half that still turns is not hidden by one that stopped.
pub fn pick(a: f32, b: Option<f32>) -> f32 {
    match b {
        Some(b) if b.abs() > a.abs() => b,
        _ => a,
    }
}

/// The overlay line: the size in RPM and the way (negative: clockwise), like the motor's value box.
pub fn label(speed: f32) -> String {
    format!("{} RPM {}", speed.abs().round() as i32, if speed < 0.0 { "\u{27f3}" } else { "\u{27f2}" })
}

const GOLD: u32 = 0xFFAA00;
const WHITE: u32 = 0xFFFFFF;
const GRAY: u32 = 0xAAAAAA;
const DARK_GRAY: u32 = 0x555555;
const AQUA: u32 = 0x55FFFF;

/// One line of coloured pieces (text, 0xRRGGBB).
pub type Line = Vec<(String, u32)>;

fn line(text: &str, rgb: u32) -> Line {
    vec![(text.to_string(), rgb)]
}

/// `LangBuilder.forGoggles`: a header is indented by 4 spaces, a value line by 5.
const H: &str = "    ";

/// The stress lines for a block: its base `impact` and `capacity` (su per RPM, stress.rs) and the speed the network
/// `theoretical`ly gives it, as Create's goggles show them (`addToGoggleTooltip`). `number` is
/// `kinetics_stressometer::number` (`CreateLang.number`). Empty for a block with neither, as in Create.
pub fn stress_lines(impact: f32, capacity: f32, theoretical: f32, number: fn(f32) -> String) -> Vec<Line> {
    let mut out = Vec::new();
    let value = |v: f32| vec![(format!("{H} {}su", number(v)), AQUA), (" at current speed".to_string(), DARK_GRAY)];
    if impact != 0.0 {
        out.push(line(&format!("{H}Kinetic Stats:"), WHITE));
        out.push(line(&format!("{H}Kinetic Stress Impact:"), GRAY));
        out.push(value(impact * theoretical.abs()));
    }
    if capacity != 0.0 {
        if !out.is_empty() {
            out.push(Vec::new());
        }
        out.push(line(&format!("{H}Generator Stats:"), WHITE));
        out.push(line(&format!("{H}Kinetic Stress Capacity:"), GRAY));
        out.push(value(capacity * theoretical.abs()));
    }
    out
}

/// Create's overstressed warning (`KineticBlockEntity.addToTooltip`, key `gui.contraptions.network_overstressed`),
/// shown with or without goggles: gold title, then gray text with the underscored words white.
pub fn overstressed_lines() -> Vec<Line> {
    let p = |t: &str, rgb| (t.to_string(), rgb);
    vec![
        line(&format!("{H}Overstressed"), GOLD),
        vec![p(H, GRAY), p("It appears that this contraption is ", GRAY), p("overstressed", WHITE), p(".", GRAY)],
        vec![p(H, GRAY), p("Add more sources or ", GRAY), p("slow down", WHITE), p(" the", GRAY)],
        vec![p(H, GRAY), p("components with a high ", GRAY), p("stress impact", WHITE), p(".", GRAY)],
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_the_faster_half() {
        assert_eq!(pick(16.0, None), 16.0);
        assert_eq!(pick(0.0, Some(-32.0)), -32.0);
        assert_eq!(pick(8.0, Some(-4.0)), 8.0);
    }

    #[test]
    fn label_shows_size_and_way() {
        assert_eq!(label(16.0), "16 RPM \u{27f2}");
        assert_eq!(label(-256.0), "256 RPM \u{27f3}");
        assert_eq!(label(7.6), "8 RPM \u{27f2}");
        assert_eq!(label(0.0), "0 RPM \u{27f2}");
    }

    #[test]
    fn stress_lines_follow_create() {
        let n: fn(f32) -> String = |v| format!("{v}");
        let text = |l: &Line| l.iter().map(|p| p.0.as_str()).collect::<String>();
        assert!(stress_lines(0.0, 0.0, 16.0, n).is_empty());
        let l = stress_lines(4.0, 0.0, -16.0, n);
        assert_eq!(text(&l[2]), "     64su at current speed");
        assert_eq!((l[0][0].1, l[2][0].1, l[2][1].1), (WHITE, AQUA, DARK_GRAY));
        let g = stress_lines(0.0, 8.0, 16.0, n);
        assert_eq!((text(&g[0]).as_str(), text(&g[2]).as_str()), ("    Generator Stats:", "     128su at current speed"));
        assert_eq!(text(&overstressed_lines()[1]), "    It appears that this contraption is overstressed.");
    }
}
