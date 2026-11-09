// The turning parts, drawn by the block entities.
fn kinetics_models(models: &mut Vec<String>) {
    use crate::game::kinetics as k;
    // The Create look's models are missing without its pack: the game logs that and draws ours.
    models.extend(k::PART_MODELS.iter().chain(&k::LOOK_MODELS).map(|m| m.to_string()));
    use crate::game::kinetics_casing as c;
    models.extend(c::PART_MODELS.iter().chain(&c::LOOK_MODELS).chain(&c::STATIC_LOOK_MODELS).map(|m| m.to_string()));
    // The clutch's half shaft is ours: Create's `shaft_half` (its look) is the casing's, registered above.
    models.push(crate::game::kinetics_clutch::HALF_MODEL.to_string());
    models.extend(crate::game::kinetics_clutch::STATIC_LOOK_MODELS.iter().map(|m| m.to_string()));
    // The bearing's top (bearing_submit.rs); its half shaft is the casing's, registered above.
    models.push("create:block/bearing/top".to_string());
}
