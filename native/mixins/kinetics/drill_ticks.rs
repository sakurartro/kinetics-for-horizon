// The drills' block entities tick (the game only ticks those a mod asks for): that is how they dig.
fn kinetics_drill_ticks(block: &str, ticks: &mut bool) {
    if block == crate::game::kinetics_drill::BLOCK || matches!(block, crate::game::kinetics_contraption::PISTON | crate::game::kinetics_contraption::BEARING | crate::game::kinetics_contraption::ASSEMBLER) {
        *ticks = true;
    }
}
