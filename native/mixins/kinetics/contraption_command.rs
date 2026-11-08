// `/kinetics_contraption <from> <to> <dx> <dy> <dz>` (BUR.md phase 2, a debug command until the piston and the glue
// come): the blocks glued in the box from..to that touch its corner `from` become a contraption, which moves by
// (dx, dy, dz) and is put back there. It only asks (contraption_tick.rs assembles, like for the piston).
fn kinetics_contraption_command(
    game: &mut crate::game::Game<'_>,
    player: &NativeServerPlayer,
    command: &str,
    ran: &mut bool,
) {
    use crate::game::kinetics_contraption as c;
    let mut words = command.split_whitespace();
    if *ran || words.next() != Some(c::COMMAND) {
        return;
    }
    *ran = true;
    let base = player.position.map(|v| v.floor() as i32);
    let args: Vec<&str> = words.collect();
    let nums: Option<Vec<i32>> = (args.len() == 9)
        .then(|| args.iter().enumerate().map(|(i, t)| if i < 6 { c::coord(t, base[i % 3]) } else { t.parse().ok() }).collect())
        .flatten();
    let Some(n) = nums else {
        game.native_feedback(Text::literal(format!("/{} <from> <to> <dx> <dy> <dz>", c::COMMAND)));
        return;
    };
    let dimension = game.dim.id().to_owned();
    if dimension != player.dimension || !game.native_authoritative() {
        return;
    }
    let (from, to, by) = ([n[0], n[1], n[2]], [n[3], n[4], n[5]], [n[6], n[7], n[8]]);
    c::REQUESTS.lock().unwrap().push(c::Request {
        dimension,
        anchor: from,
        by,
        speed: c::SPEED,
        extra: Some(c::Glue::new(from, to)),
        feedback: true,
        owner: None,
        spin: None,
        cart: None,
    });
}
