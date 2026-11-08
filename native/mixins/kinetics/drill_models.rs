// The drill's body and head, baked beside the block models (with Create's, when the look pack is on).
fn kinetics_drill_models(models: &mut Vec<String>) {
    use crate::game::kinetics_drill as d;
    models.extend(d::MODELS.iter().chain(&d::LOOK_MODELS).map(|m| m.to_string()));
}
