// The debug command for contraptions (contraption_command.rs), for operators.
fn kinetics_contraption_commands(commands: &mut Vec<NativeCommand>) {
    let c = crate::game::kinetics_contraption::COMMAND;
    commands.push(NativeCommand::op(
        &format!("{c} <from:block_pos> <to:block_pos> <dx:integer> <dy:integer> <dz:integer>"),
        2,
    ));
}
