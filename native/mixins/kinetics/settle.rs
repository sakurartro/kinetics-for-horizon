// Solves the networks touched in `dimension` this tick and stores each part's speed.
fn kinetics_settle(game: &mut crate::game::Game<'_>, dimension: &str) {
    use crate::game::kinetics as k;
    use crate::game::kinetics_clutch as c;
    use crate::game::kinetics_stress as st;
    use horizon_world::nbt::{JavaString, Tag};
    let mut changed = k::take(dimension);
    // A clutch or gearshift that was placed, or whose neighbour changed, follows its redstone signal.
    // Like Create's `detachKinetics`, one that flips cuts its network for this tick; the flip marks it
    // again (block changed), so the network joins again, in its new state, at the next tick's end.
    let mut detached: Vec<[i32; 3]> = Vec::new();
    let mut checks = c::take(dimension);
    checks.extend(changed.iter().copied());
    for pos in checks {
        let name = game.native_block(pos);
        if !c::is_split(&name) || detached.contains(&pos) {
            continue;
        }
        let powered = game.native_block_property(pos, "powered").as_deref() == Some("true");
        let Some(flip) = c::flip(&name, powered, game.native_neighbor_signal(dimension, pos)) else {
            continue;
        };
        // Flag 2 is UPDATE_CLIENTS: nothing around it needs to hear.
        let value = if flip.powered { "true" } else { "false" };
        if game.native_set_block_property_with(pos, "powered", value, 2) {
            changed.push(pos);
            if flip.detach {
                detached.push(pos);
            }
        }
    }
    if changed.is_empty() {
        return;
    }
    // The lookup reads the world, and a block entity can only be reached mutably: share the game through a cell.
    let cell = std::cell::RefCell::new(&mut *game);
    let get = |p: [i32; 3]| -> Option<k::Node> {
        if detached.contains(&p) {
            return None;
        }
        let name = cell.borrow().native_block(p);
        let nm = name.strip_prefix("kinetics:").unwrap_or("");
        if nm == "clutch" || nm == "gearshift" {
            let g = cell.borrow();
            return k::node_split(
                &name,
                g.native_block_property(p, "axis").as_deref(),
                g.native_block_property(p, "powered").as_deref(),
            );
        }
        if crate::game::kinetics_chain::is_chain(&name) {
            // A chain drive or gearshift: its chain, and the signal its block entity saved.
            let key = JavaString::from(crate::game::kinetics_chain::SIGNAL_KEY);
            let signal = match cell.borrow_mut().world.block_entities.get_mut(&(p[0], p[1], p[2])) {
                Some(BlockEntity::Mod(m)) => match m.nbt.get(&key) {
                    Some(Tag::Int(v)) => *v,
                    _ => 0,
                },
                _ => 0,
            };
            let g = cell.borrow();
            return k::chain_node(
                &name,
                g.native_block_property(p, "axis").as_deref(),
                g.native_block_property(p, "part").as_deref(),
                g.native_block_property(p, crate::game::kinetics_chain::FIRST).as_deref(),
                signal,
            );
        }
        if name == crate::game::kinetics_contraption::PISTON {
            // The poles are at its back, so the shaft comes in at its side.
            let facing = cell.borrow().native_block_property(p, "facing")?;
            return k::crank_node(crate::game::kinetics_contraption::piston_input(&facing), 0);
        }
        if crate::game::kinetics_clock::is_clock(&name) || name == crate::game::kinetics_drill::BLOCK || name == crate::game::kinetics_contraption::BEARING {
            // A cuckoo clock or a drill: a shaft at its back, giving no speed.
            return k::crank_node(&cell.borrow().native_block_property(p, "facing")?, 0);
        }
        if name == crate::game::kinetics_crank::BLOCK {
            // A hand crank: its facing, and the speed its turner gives it.
            let turn = crate::game::kinetics_crank::speed(dimension, p);
            return k::crank_node(&cell.borrow().native_block_property(p, "facing")?, turn);
        }
        if crate::game::kinetics_water_wheel::is_wheel(&name) {
            // A water wheel: a generator on its axis, at the speed its flow score (saved in its block entity) gives.
            use crate::game::kinetics_water_wheel as w;
            let score = match cell.borrow_mut().world.block_entities.get_mut(&(p[0], p[1], p[2])) {
                Some(BlockEntity::Mod(m)) => match m.nbt.get(&JavaString::from(w::FLOW_KEY)) {
                    Some(Tag::Int(v)) => *v,
                    _ => 0,
                },
                _ => 0,
            };
            let g = cell.borrow();
            let axis = w::axis_index(&g.native_block_property(p, if name == w::LARGE { "axis" } else { "facing" })?)?;
            return k::powered_shaft_node(Some(["x", "y", "z"][axis]), w::generated(score, w::size(&name)));
        }
        if name == "kinetics:powered_shaft" {
            // A powered shaft: its generator's speed, saved in its block entity (0 until set).
            let rpm = match cell.borrow_mut().world.block_entities.get_mut(&(p[0], p[1], p[2])) {
                Some(BlockEntity::Mod(m)) => match m.nbt.get(&JavaString::from(k::RPM_KEY)) {
                    Some(Tag::Int(v)) => *v,
                    _ => 0,
                },
                _ => 0,
            };
            return k::powered_shaft_node(cell.borrow().native_block_property(p, "axis").as_deref(), rpm);
        }
        if name == crate::game::kinetics_stressometer::BLOCK {
            // A stressometer: a shaft along the axis its facing and `axis_along_first` give.
            use crate::game::kinetics_stressometer as sm;
            let g = cell.borrow();
            let facing = sm::dir_vec(&g.native_block_property(p, "facing")?)?;
            let along = g.native_block_property(p, "axis_along_first").as_deref() == Some("true");
            return k::node(&name, Some(["x", "y", "z"][sm::rotation_axis(facing, along)]));
        }
        if name == crate::game::kinetics_speedometer::BLOCK {
            // A speedometer: a shaft along the axis its facing and `axis_along_first` give.
            use crate::game::kinetics_speedometer as sp;
            let g = cell.borrow();
            let facing = sp::dir_vec(&g.native_block_property(p, "facing")?)?;
            let along = g.native_block_property(p, "axis_along_first").as_deref() == Some("true");
            return k::node(&name, Some(["x", "y", "z"][sp::rotation_axis(facing, along)]));
        }
        let controller = name == crate::game::kinetics_speed_controller::BLOCK;
        if name != crate::game::kinetics_motor::BLOCK && !controller {
            let g = cell.borrow();
            return k::node_ends(
                &name,
                g.native_block_property(p, "axis").as_deref(),
                g.native_block_property(p, "bottom_shaft").as_deref(),
                g.native_block_property(p, "top_shaft").as_deref(),
            );
        }
        // A motor's end (or a controller's axis) and its setting (saved in its block entity, 16 until set).
        let rpm = match cell.borrow_mut().world.block_entities.get_mut(&(p[0], p[1], p[2])) {
            Some(BlockEntity::Mod(m)) => match m.nbt.get(&JavaString::from(k::RPM_KEY)) {
                Some(Tag::Int(v)) => *v,
                _ => crate::game::kinetics_motor::DEFAULT_RPM,
            },
            _ => crate::game::kinetics_motor::DEFAULT_RPM,
        };
        if controller {
            return k::controller_node(&cell.borrow().native_block_property(p, "axis")?, rpm);
        }
        let facing = cell.borrow().native_block_property(p, "facing")?;
        k::motor_node(&facing, rpm)
    };
    let (mut speeds, groups) = k::solve_groups(&changed, &get);
    // Each network's capacity and stress; one that is overstressed stops (speed 0 everywhere), keeping the speed
    // it would have as `theoretical_speed` (Create: `KineticNetwork.updateNetwork`, `KineticBlockEntity.getSpeed`).
    let mut facts: Vec<([i32; 3], [(&'static str, f32); 4])> = Vec::new();
    for group in &groups {
        let ns = st::evaluate(
            group,
            |v| speeds[&v],
            |p| get(p).and_then(k::generated),
            |p| st::impact(&cell.borrow().native_block(p)),
            |p| st::capacity(&cell.borrow().native_block(p)),
        );
        for &v in group {
            if !facts.iter().any(|f| f.0 == v.0) {
                facts.push((v.0, ns.entries(speeds[&v] + 0.0)));
            }
        }
        if ns.overstressed() {
            for v in group {
                speeds.insert(*v, 0.0);
            }
        }
    }
    for &p in &detached {
        speeds.insert((p, 0), 0.0);
        speeds.insert((p, 1), 0.0);
    }
    let mut moved: Vec<[i32; 3]> = Vec::new();
    for ((pos, half), speed) in speeds {
        let key = JavaString::from(if half == 0 { k::SPEED_KEY } else { c::SPEED_POS_KEY });
        let tag = Tag::Float(speed.to_bits());
        let wrote = game.native_with_mod_block_entity(pos, |be| {
            if be.nbt.get(&key) == Some(&tag) {
                return false;
            }
            be.nbt.insert(key.clone(), tag.clone());
            true
        });
        if wrote == Some(true) && !moved.contains(&pos) {
            moved.push(pos);
        }
    }
    for (pos, entries) in facts {
        let wrote = game.native_with_mod_block_entity(pos, |be| {
            let mut changed = false;
            for (name, value) in entries {
                let (key, tag) = (JavaString::from(name), Tag::Float(value.to_bits()));
                if be.nbt.get(&key) != Some(&tag) {
                    be.nbt.insert(key, tag);
                    changed = true;
                }
            }
            changed
        });
        if wrote == Some(true) && !moved.contains(&pos) {
            moved.push(pos);
        }
    }
    for pos in moved {
        game.native_block_entity_sync(pos);
        // Both gauges' comparators follow them (`GaugeBlockEntity.setChanged` -> `updateNeighbourForOutputSignal`).
        let b = game.native_block(pos);
        if b == crate::game::kinetics_stressometer::BLOCK || b == crate::game::kinetics_speedometer::BLOCK {
            game.native_update_output_neighbors(dimension, pos);
        }
    }
}
