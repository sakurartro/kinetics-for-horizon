// A cart carrying a contraption has a passenger, as in Create (the contraption rides it): it slows like a ridden
// cart (`getSlowdownFactor`), and no mob gets in.
fn kinetics_cart_vehicle(game: &crate::game::Game<'_>, id: u32) -> std::ops::ControlFlow<bool> {
    let dimension = game.dim.id();
    if crate::game::kinetics_contraption::LIVE.lock().unwrap().iter().any(|l| l.cart == Some(id) && l.dimension == dimension) {
        return std::ops::ControlFlow::Break(true);
    }
    std::ops::ControlFlow::Continue(())
}
