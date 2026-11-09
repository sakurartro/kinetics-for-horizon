// The crank's turning part, baked beside the block models (with Create's, when the look pack is on).
fn kinetics_crank_models(models: &mut Vec<String>) {
    use crate::game::kinetics_crank as c;
    models.extend(std::iter::once(c::PART_MODEL).chain(c::LOOK_MODELS).map(|m| m.to_string()));
}
