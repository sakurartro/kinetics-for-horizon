// Create's goggle overlay (`GoggleOverlayRenderer`): while goggles are on the head, the stress lines (goggles.rs) or
// gauge information of the kinetic block under the crosshair, in a tooltip box beside it; an overstressed network's
// warning shows without goggles too.
fn kinetics_goggles_hud(game: &mut crate::game::Game<'_>, layer: &str, batch: &mut HudBatch) {
    use crate::game::{kinetics as k, kinetics_goggles as g, kinetics_stress as st};
    use horizon_world::nbt::{JavaString, Tag};
    if layer != "minecraft:crosshair" || !game.native_window() {
        return;
    }
    let goggles = game.armor[0].as_ref().is_some_and(|s| &*s.item == g::GOGGLES);
    let Some((distance, hit)) = game.native_view_hit() else { return };
    if distance > 5.0 {
        return;
    }
    let pos = [hit.block_x, hit.block_y, hit.block_z];
    let block = game.native_block(pos);
    let read = |key: &str| match game.world.block_entities.get(&(pos[0], pos[1], pos[2])) {
        Some(BlockEntity::Mod(be)) => match be.nbt.get(&JavaString::from(key)) {
            Some(Tag::Float(bits)) => Some(f32::from_bits(*bits)),
            _ => None,
        },
        _ => None,
    };
    let (speed, theoretical, overstressed) = if block == "kinetics:source" {
        (k::SOURCE_SPEED, k::SOURCE_SPEED, false)
    } else if k::has_block_entity(&block) || crate::game::kinetics_casing::has_block_entity(&block) {
        let speed = g::pick(read(k::SPEED_KEY).unwrap_or(0.0), read(crate::game::kinetics_clutch::SPEED_POS_KEY));
        let net = st::read(&read);
        (speed, net.map_or(speed, |n| n.theoretical_speed), net.is_some_and(|n| n.stress.overstressed()))
    } else {
        return;
    };
    let mut lines: Vec<g::Line> = Vec::new();
    if goggles && block == crate::game::kinetics_stressometer::BLOCK {
        use crate::game::kinetics_stressometer as sm;
        let net = st::read(&read);
        let (capacity, stress) = net.map_or((0.0, 0.0), |n| (n.stress.capacity, n.stress.stress));
        let level = st::Level::of(stress / if capacity == 0.0 { 1.0 } else { capacity }) as usize;
        lines.extend(sm::tooltip(capacity, stress, net.map_or(0.0, |n| n.theoretical_speed), level, st::DISABLE_STRESS).unwrap_or_default());
    } else if goggles && block == crate::game::kinetics_speedometer::BLOCK {
        // `SpeedLevel.getFormattedSpeedText`: an overstressed gauge shows the theoretical speed dark gray (Create also
        // strikes it through).
        for (i, (text, rgb)) in crate::game::kinetics_speedometer::tooltip(if overstressed { theoretical } else { speed }).into_iter().enumerate() {
            lines.push(vec![(text, if overstressed && i == 2 { 0x555555 } else { rgb })]);
        }
    } else if goggles && block == "kinetics:source" {
        lines.push(vec![(g::label(speed), 0xFF_FFFF)]);
    } else if goggles {
        lines = g::stress_lines(st::impact(&block), st::capacity(&block), theoretical, crate::game::kinetics_stressometer::number);
    }
    if overstressed {
        if !lines.is_empty() {
            lines.push(Vec::new());
        }
        lines.extend(g::overstressed_lines());
    }
    if lines.is_empty() {
        return;
    }
    // A vanilla tooltip box (`GuiGraphics.renderTooltip`) at Create's anchor: centre + (20, 0), text 12 px right
    // and up of it, kept on the screen, 2 px more under the title.
    let width = lines.iter().map(|l| l.iter().map(|(t, _)| game.native_text_width(t)).sum::<f32>()).fold(0.0, f32::max);
    let h = 10.0 * lines.len() as f32 - 2.0 + if lines.len() > 1 { 2.0 } else { 0.0 };
    let x = (batch.width / 2.0 + 32.0).min(batch.width - width - 20.0);
    let y = (batch.height / 2.0 - 12.0).min(batch.height - h - 20.0);
    let (bw, bh) = (width + 6.0, h + 6.0);
    // `TooltipRenderUtil`: background 0xF0100010 (Create fades it to 0.75), border fading 0x505000FF -> 0x5028007F.
    game.native_fill(batch, [x - 3.0, y - 4.0, bw, bh + 2.0], 0xB4_100010);
    game.native_fill(batch, [x - 4.0, y - 3.0, bw + 2.0, bh], 0xB4_100010);
    game.native_fill(batch, [x - 3.0, y - 3.0, bw, 1.0], 0x50_5000FF);
    game.native_fill(batch, [x - 3.0, y + h + 2.0, bw, 1.0], 0x50_28007F);
    for side in [x - 3.0, x + width + 2.0] {
        game.native_fill(batch, [side, y - 2.0, 1.0, h + 4.0], 0x50_3C00BF);
    }
    for (i, line) in lines.iter().enumerate() {
        let mut at = x;
        let ly = y + 10.0 * i as f32 + if i > 0 { 2.0 } else { 0.0 };
        for (text, rgb) in line {
            game.native_text(batch, text, [at, ly], *rgb, true);
            at += game.native_text_width(text);
        }
    }
}
