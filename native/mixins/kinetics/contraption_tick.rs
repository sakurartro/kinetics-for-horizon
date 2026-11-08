// At the end of each level's tick, the contraptions in it (contraption.rs): each marker gets its displays (again,
// after a reload or its chunk coming back: entity ids change, so tagged displays no contraption holds are removed),
// then moves a step toward its target (or waits while its drills break what is in the way, or for good against a
// solid block) and, there, puts its blocks back into the world (Create's
// `Contraption.addBlocksToWorld`; a block whose place is taken drops as an item) and goes.
fn kinetics_contraption_tick(game: &mut crate::game::Game<'_>, dimension: &str) {
    use crate::game::kinetics_contraption as c;
    use horizon_world::nbt::Tag;
    let tagged = |raw: Option<&Tag>, tag: &str| match raw.and_then(|t| t.get("Tags")) {
        Some(Tag::List(l)) => l.iter().any(|t| matches!(t, Tag::String(s) if s.to_string_lossy() == tag)),
        _ => false,
    };
    let mut markers = Vec::new();
    let mut glues = Vec::new();
    let mut pieces = std::collections::HashSet::new();
    for o in &game.others {
        if o.kind == "minecraft:marker" && tagged(game.persist.raw.get(&o.id), c::GLUE_TAG) {
            let corner = |key: &str| match game.persist.raw.get(&o.id).and_then(|t| t.get("data")).and_then(|d| d.get(key)) {
                Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
                _ => None,
            };
            if let (Some(a), Some(b)) = (corner("min"), corner("max")) {
                glues.push((o.id, c::Glue::new(a, b)));
            }
        } else if o.kind == "minecraft:marker" && tagged(game.persist.raw.get(&o.id), c::MARKER_TAG) {
            markers.push(o.id);
        } else if o.kind == "minecraft:block_display" && tagged(game.persist.raw.get(&o.id), c::PIECE_TAG) {
            pieces.insert(o.id);
        }
    }
    let wanted: Vec<c::Request> = {
        let mut all = c::REQUESTS.lock().unwrap();
        let (mine, rest): (Vec<c::Request>, Vec<c::Request>) = all.drain(..).partition(|r| r.dimension == dimension);
        *all = rest;
        mine
    };
    for r in wanted {
        c_assemble(game, &r, &mut glues);
    }
    let mut mine: Vec<c::Live> = {
        let mut all = c::LIVE.lock().unwrap();
        let (mine, rest): (Vec<c::Live>, Vec<c::Live>) = all.drain(..).partition(|l| l.dimension == dimension);
        *all = rest;
        mine
    };
    if markers.is_empty() && pieces.is_empty() && mine.is_empty() {
        return;
    }
    mine.retain(|l| markers.contains(&l.marker));
    for &m in &markers {
        if !mine.iter().any(|l| l.marker == m) {
            mine.push(c::Live { dimension: dimension.to_owned(), marker: m, displays: Vec::new() });
        }
    }
    for id in &pieces {
        if !mine.iter().any(|l| l.displays.contains(id)) {
            game.native_remove_entity(dimension, *id);
        }
    }
    mine.retain_mut(|l| c_run(game, dimension, l, &pieces));
    c::LIVE.lock().unwrap().extend(mine);

    /// Create's `Contraption.assemble`, then `removeBlocksFromWorld`: the snapshot is written into the marker before any
    /// block leaves the world, in the same tick, so a save never sees one without the other. The glue boxes it
    /// used travel in the snapshot and come back where it lands.
    fn c_assemble(
        game: &mut crate::game::Game<'_>,
        r: &crate::game::kinetics_contraption::Request,
        glues: &mut Vec<(u32, crate::game::kinetics_contraption::Glue)>,
    ) {
        use crate::game::{kinetics as k, kinetics_contraption as c};
        use horizon_world::nbt::{JavaString, Tag};
        let from = r.anchor;
        let mut boxes: Vec<c::Glue> = glues.iter().map(|g| g.1).collect();
        boxes.extend(r.extra);
        let found = c::collect(
            from,
            &boxes,
            |p| {
                let facts = game.res.facts().get(game.world.block(p[0], p[1], p[2]));
                let foreign = matches!(game.world.block_entities.get(&(p[0], p[1], p[2])), Some(e) if !matches!(e, BlockEntity::Mod(_)));
                c::movable(&facts.name, facts.hardness, facts.has_class("LiquidBlock"), foreign)
            },
            c::LIMIT,
        );
        let say = |game: &mut crate::game::Game<'_>, text: String| {
            if r.feedback {
                game.native_feedback(Text::literal(text));
            }
        };
        let blocks = match found {
            Ok(b) => b,
            Err(why) => {
                let text = match why {
                    c::Refusal::NothingThere => format!("no block at {from:?} to start from"),
                    c::Refusal::Unmovable(p) => format!("the block at {p:?} can't move"),
                    c::Refusal::TooMany => format!("more than {} blocks", c::LIMIT),
                };
                say(game, text);
                return;
            }
        };
        let registry = game.pack.assets.registry;
        let Some(air) = registry.default_state("minecraft:air") else { return };
        // Each block's place from the anchor, its state as `block_state` takes it, and our block entity's data
        // without its speed (the network is solved again where it lands).
        let mut pieces = Vec::new();
        for p in &blocks {
            let state = game.world.block(p[0], p[1], p[2]);
            let mut piece = Compound::new();
            piece.insert("pos".into(), Tag::IntArray((0..3).map(|i| p[i] - from[i]).collect()));
            // `{Name, Properties}`, as a `block_display` and `state_from_nbt` read it.
            let mut t = Compound::new();
            if let Some(s) = registry.state(state) {
                t.insert("Name".into(), Tag::String(s.name.as_str().into()));
                let props = s.properties.iter().map(|(k, v)| (k.as_str().into(), Tag::String(v.as_str().into()))).collect();
                t.insert("Properties".into(), Tag::Compound(props));
            }
            piece.insert("state".into(), Tag::Compound(t));
            if let Some(BlockEntity::Mod(m)) = game.world.block_entities.get(&(p[0], p[1], p[2])) {
                let mut nbt = m.nbt.clone();
                nbt.remove(&JavaString::from(k::SPEED_KEY));
                piece.insert("nbt".into(), Tag::Compound(nbt));
            }
            pieces.push(Tag::Compound(piece));
        }
        // The glue boxes that hold any of these blocks go with them.
        let mut taken = Vec::new();
        let mut kept = Vec::new();
        for (id, g) in glues.drain(..) {
            if blocks.iter().any(|&p| g.contains(p)) {
                taken.push((id, g));
            } else {
                kept.push((id, g));
            }
        }
        *glues = kept;
        let doubles = |v: [f64; 3]| Tag::List(v.iter().map(|x| Tag::Double(x.to_bits())).collect());
        let mut data = Compound::new();
        data.insert("origin".into(), Tag::IntArray(from.to_vec()));
        data.insert("offset".into(), doubles([0.0; 3]));
        data.insert("target".into(), doubles(r.by.map(f64::from)));
        data.insert("speed".into(), Tag::Double(r.speed.to_bits()));
        if let Some(axis) = r.spin {
            // A bearing's: turns about `axis` through the anchor, its speed read from the bearing each tick.
            data.insert("axis".into(), Tag::IntArray(axis.to_vec()));
            data.insert("angle".into(), Tag::Double(0f64.to_bits()));
            if let Some((p, _, _)) = r.owner {
                data.insert("bearing".into(), Tag::IntArray(p.to_vec()));
            }
        }
        if let Some(h) = r.cart {
            data.insert("heading".into(), Tag::IntArray(h.to_vec()));
        }
        data.insert("pieces".into(), Tag::List(pieces));
        data.insert(
            "glue".into(),
            Tag::List(
                taken
                    .iter()
                    .map(|(_, g)| Tag::IntArray((0..3).map(|i| g.min[i] - from[i]).chain((0..3).map(|i| g.max[i] - from[i])).collect()))
                    .collect(),
            ),
        );
        let mut nbt = Compound::new();
        nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::MARKER_TAG.into())]));
        nbt.insert("data".into(), Tag::Compound(data));
        if game.native_summon(&r.dimension, "minecraft:marker", from.map(f64::from), nbt).is_none() {
            say(game, "the marker couldn't be made here".to_string());
            return;
        }
        for (id, _) in taken {
            game.native_remove_entity(&r.dimension, id);
        }
        // Its displays come at this tick's end. Create's flags: no drops, no shape updates (a torch on it stays put).
        for p in &blocks {
            game.native_set_block(&r.dimension, *p, air, c::REMOVE_FLAGS);
        }
        if let Some((p, key, value)) = r.owner {
            if let Some(BlockEntity::Mod(m)) = game.world.block_entities.get_mut(&(p[0], p[1], p[2])) {
                m.nbt.insert(JavaString::from(key), Tag::Int(value));
            }
        }
        say(game, format!("{} blocks move by {:?}", blocks.len(), r.by));
    }

    /// One contraption's tick: false once it is put back (or its marker can't be read).
    fn c_run(
        game: &mut crate::game::Game<'_>,
        dimension: &str,
        l: &mut crate::game::kinetics_contraption::Live,
        alive: &std::collections::HashSet<u32>,
    ) -> bool {
        use crate::game::kinetics_contraption as c;
        use horizon_world::nbt::Tag;
        let Some(data) = game.persist.raw.get(&l.marker).and_then(|t| t.get("data")).cloned() else {
            return false;
        };
        let ints = |key: &str| match data.get(key) {
            Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
            _ => None,
        };
        let doubles = |key: &str| match data.get(key) {
            Some(Tag::List(v)) if v.len() == 3 => {
                let d = |t: &Tag| if let Tag::Double(b) = t { f64::from_bits(*b) } else { 0.0 };
                Some([d(&v[0]), d(&v[1]), d(&v[2])])
            }
            _ => None,
        };
        let (Some(origin), Some(offset), Some(target), Some(Tag::List(list))) =
            (ints("origin"), doubles("offset"), doubles("target"), data.get("pieces"))
        else {
            return false;
        };
        let rel = |p: &Tag| match p.get("pos") {
            Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
            _ => [0; 3],
        };
        let at = |off: [f64; 3], r: [i32; 3]| [0, 1, 2].map(|i| f64::from(origin[i] + r[i]) + off[i]);
        // Its displays: made again when one is missing.
        if l.displays.is_empty() || l.displays.iter().any(|id| !alive.contains(id)) {
            for id in l.displays.drain(..) {
                game.native_remove_entity(dimension, id);
            }
            for p in list {
                let mut nbt = Compound::new();
                let mut shown = p.get("state").cloned().unwrap_or(Tag::Compound(Compound::new()));
                if matches!(shown.get("Name"), Some(Tag::String(n)) if n.to_string_lossy() == crate::game::kinetics_drill::BLOCK) {
                    if let Tag::Compound(s) = &mut shown {
                        s.insert("Name".into(), Tag::String(crate::game::kinetics_drill::DISPLAY.into()));
                    }
                }
                nbt.insert("block_state".into(), shown);
                nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::PIECE_TAG.into())]));
                // Clients ease each tick's move over the tick.
                nbt.insert("teleport_duration".into(), Tag::Int(1));
                match game.native_summon(dimension, "minecraft:block_display", at(offset, rel(p)), nbt) {
                    Some(id) => l.displays.push(id),
                    // A chunk that isn't loaded: try again next tick.
                    None => {
                        for id in l.displays.drain(..) {
                            game.native_remove_entity(dimension, id);
                        }
                        return true;
                    }
                }
            }
        }
        if let Some(axis) = ints("axis") {
            return c_spin(game, dimension, l, &data, origin, axis, list);
        }
        if offset == target {
            // A cart that got to a rail goes on to the next, else it is put down where it is.
            if data.get("heading").is_some() && c_cart(game, l.marker, origin, offset, &data) {
                return true;
            }
            return c_place(game, dimension, l, origin, offset, list, &data, None);
        }
        let speed = match data.get("speed") {
            Some(Tag::Double(b)) => f64::from_bits(*b),
            _ => c::SPEED,
        };
        let next = c::step(offset, target, speed);
        // Stop at the edge of the loaded world: an entity in a chunk that isn't loaded is never saved.
        if !list.iter().all(|p| {
            let q = at(next, rel(p));
            game.world.chunks.contains_key(&c::chunk_of(q[0], q[2]))
        }) {
            return true;
        }
        // What is in the way (BUR.md phase 3): drills facing the motion break what they enter while it waits
        // (Create's `BlockBreakingMovementBehaviour`), a solid block anything else touches stops it.
        let motion = [0, 1, 2].map(|i| next[i] - offset[i]);
        let pieces: Vec<([f64; 3], bool)> = list
            .iter()
            .map(|p| (at(next, rel(p)), c_facing(p).is_some_and(|f| c::drills_toward(f.map(f64::from), motion))))
            .collect();
        let way = c::way(&pieces, |q| c_in_way(game, q));
        if way.blocked || !way.drill.is_empty() {
            for (i, cell) in way.drill {
                c_drill(game, l.marker, i, &list[i], cell, c::drill_rpm(speed));
            }
            return true;
        }
        game.native_move_entity(dimension, l.marker, at(next, [0; 3]), 0.0, 0.0);
        for (id, p) in l.displays.iter().zip(list) {
            game.native_move_entity(dimension, *id, at(next, rel(p)), 0.0, 0.0);
        }
        if let Some(Tag::Compound(d)) = game.persist.raw.get_mut(&l.marker).and_then(|t| t.get_mut("data")) {
            d.insert("offset".into(), Tag::List(next.iter().map(|x| Tag::Double(x.to_bits())).collect()));
        }
        true
    }

    /// The way a drill piece faces, if the piece is one.
    fn c_facing(p: &horizon_world::nbt::Tag) -> Option<[i32; 3]> {
        use crate::game::{kinetics_drill as d, kinetics_motor as m};
        use horizon_world::nbt::Tag;
        match p.get("state").map(|s| (s.get("Name"), s.get("Properties").and_then(|x| x.get("facing")))) {
            Some((Some(Tag::String(n)), Some(Tag::String(f)))) if n.to_string_lossy() == d::BLOCK => m::dir_vec(&f.to_string_lossy()),
            _ => None,
        }
    }

    /// What the world block at `q` is to a contraption entering it.
    fn c_in_way(game: &crate::game::Game<'_>, q: [i32; 3]) -> crate::game::kinetics_contraption::InWay {
        use crate::game::{kinetics_contraption as c, kinetics_drill as d};
        let f = game.res.facts().get(game.world.block(q[0], q[1], q[2]));
        c::InWay { solid: f.legacy_solid, breakable: d::breakable(&f.name, f.hardness, f.has_class("LiquidBlock")) }
    }

    /// A cart got to the rail under its anchor (BUR.md phase 5): work out the next rail and write the next leg (`target`,
    /// `heading`, `speed`) into the marker. False to stop: no rail goes on, it is too slow, a powered rail is off, or it
    /// is on an assembler with no signal (Create: that one takes the cart apart).
    fn c_cart(game: &mut crate::game::Game<'_>, marker: u32, origin: [i32; 3], offset: [f64; 3], data: &horizon_world::nbt::Tag) -> bool {
        use crate::game::kinetics_contraption as c;
        use horizon_world::nbt::Tag;
        let heading = match data.get("heading") {
            Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
            _ => return false,
        };
        let speed = match data.get("speed") {
            Some(Tag::Double(b)) => f64::from_bits(*b),
            _ => 0.0,
        };
        let at = c::landing(origin, offset, [0, -1, 0]);
        let name = game.res.facts().get(game.world.block(at[0], at[1], at[2])).name.to_string();
        let powered = game.native_block_property(at, "powered").as_deref() == Some("true");
        if name == c::ASSEMBLER && !powered {
            return false;
        }
        let shape = game.native_block_property(at, "shape").unwrap_or_default();
        let Some((next, new_heading)) = c::cart_next(at, &shape, heading, |p| {
            c::is_rail(&game.res.facts().get(game.world.block(p[0], p[1], p[2])).name)
        }) else {
            return false;
        };
        let speed = c::cart_speed(speed, (name == "minecraft:powered_rail").then_some(powered), next[1] - at[1]);
        if speed == 0.0 {
            return false;
        }
        if let Some(Tag::Compound(d)) = game.persist.raw.get_mut(&marker).and_then(|t| t.get_mut("data")) {
            let goal = [0, 1, 2].map(|i| offset[i] + f64::from(next[i] - at[i]));
            d.insert("target".into(), Tag::List(goal.iter().map(|x| Tag::Double(x.to_bits())).collect()));
            d.insert("heading".into(), Tag::IntArray(new_heading.to_vec()));
            d.insert("speed".into(), Tag::Double(speed.to_bits()));
        }
        true
    }

    /// A turning contraption's tick (BUR.md phase 6, Create's `MechanicalBearingBlockEntity`): the bearing's speed turns
    /// it about the axis through its anchor, drills dig where they go and anything else solid holds it back; with the
    /// bearing still it settles on a quarter turn and is put down turned.
    fn c_spin(
        game: &mut crate::game::Game<'_>,
        dimension: &str,
        l: &mut crate::game::kinetics_contraption::Live,
        data: &horizon_world::nbt::Tag,
        origin: [i32; 3],
        axis: [i32; 3],
        list: &[horizon_world::nbt::Tag],
    ) -> bool {
        use crate::game::native_entities::NativeDisplay;
        use crate::game::{kinetics as k, kinetics_contraption as c};
        use horizon_world::nbt::{JavaString, Tag};
        let angle = match data.get("angle") {
            Some(Tag::Double(b)) => f64::from_bits(*b),
            _ => 0.0,
        };
        // The bearing's speed; a bearing that is gone reads as still.
        let rpm = match data.get("bearing") {
            Some(Tag::IntArray(v)) if v.len() == 3 => match game.world.block_entities.get(&(v[0], v[1], v[2])) {
                Some(BlockEntity::Mod(m)) => match m.nbt.get(&JavaString::from(k::SPEED_KEY)) {
                    Some(Tag::Float(b)) => f32::from_bits(*b),
                    _ => 0.0,
                },
                _ => 0.0,
            },
            _ => 0.0,
        };
        let (next, rest) = c::spin_step(angle, rpm);
        if rest {
            let quarters = (next / 90.0).round() as i32;
            return c_place(game, dimension, l, origin, [0.0; 3], list, data, Some((axis, quarters)));
        }
        let centre = origin.map(|v| f64::from(v) + 0.5);
        let rel = |p: &Tag| match p.get("pos") {
            Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
            _ => [0; 3],
        };
        let centre_at = |a: f64, r: [i32; 3]| {
            let v = c::rotate(r.map(f64::from), axis, a);
            [0, 1, 2].map(|i| centre[i] + v[i])
        };
        let mut pieces = Vec::new();
        let mut motions = Vec::new();
        for p in list {
            let (from, to) = (centre_at(angle, rel(p)), centre_at(next, rel(p)));
            let motion = [0, 1, 2].map(|i| to[i] - from[i]);
            let drill = c_facing(p).is_some_and(|f| c::drills_toward(c::rotate(f.map(f64::from), axis, next), motion));
            pieces.push((to.map(|v| v - 0.5), drill));
            motions.push(motion);
        }
        // Stop at the edge of the loaded world, like the straight ones.
        if !pieces.iter().all(|(q, _)| game.world.chunks.contains_key(&c::chunk_of(q[0], q[2]))) {
            return true;
        }
        let way = c::way(&pieces, |q| c_in_way(game, q));
        if way.blocked || !way.drill.is_empty() {
            for (i, cell) in way.drill {
                let m = motions[i];
                c_drill(game, l.marker, i, &list[i], cell, c::drill_rpm((m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt()));
            }
            return true;
        }
        // A display turns about its own corner: put it on the block's centre and shift its corner back by the turn.
        let corner = c::rotate([-0.5; 3], axis, next).map(|v| v as f32);
        let shown = NativeDisplay { translation: corner, left_rotation: c::quaternion(axis, next), interpolation: 1, teleport: 1, ..Default::default() };
        for (id, q) in l.displays.iter().zip(&pieces) {
            game.native_move_entity(dimension, *id, q.0.map(|v| v + 0.5), 0.0, 0.0);
            game.native_set_display(dimension, *id, &shown);
        }
        if let Some(Tag::Compound(d)) = game.persist.raw.get_mut(&l.marker).and_then(|t| t.get_mut("data")) {
            d.insert("angle".into(), Tag::Double(next.to_bits()));
        }
        true
    }

    /// One tick of a moving drill (piece `i`) on `cell`: drill_tick.rs's crack, sound and break, with the breaker
    /// kept in the piece as [progress, next, x, y, z]; a new cell starts it again (Create's `visitNewPosition`).
    fn c_drill(game: &mut crate::game::Game<'_>, marker: u32, i: usize, piece: &horizon_world::nbt::Tag, cell: [i32; 3], rpm: f32) {
        use crate::game::kinetics_drill as d;
        use horizon_world::nbt::Tag;
        let mut b = match piece.get("breaker") {
            Some(Tag::IntArray(v)) if v.len() == 5 && v[2..] == cell[..] => d::Breaker { progress: v[0], next: v[1] },
            _ => d::Breaker::default(),
        };
        let facts = game.res.facts().get(game.world.block(cell[0], cell[1], cell[2]));
        let hardness = d::breakable(&facts.name, facts.hardness, facts.has_class("LiquidBlock")).then_some(facts.hardness);
        let hit = facts.sounds.hit.clone();
        let step = d::step(&mut b, rpm, hardness);
        if let Some(Tag::List(ps)) = game.persist.raw.get_mut(&marker).and_then(|t| t.get_mut("data")).and_then(|x| x.get_mut("pieces")) {
            if let Some(Tag::Compound(pc)) = ps.get_mut(i) {
                pc.insert("breaker".into(), Tag::IntArray(vec![b.progress, b.next, cell[0], cell[1], cell[2]]));
            }
        }
        let crack = match step {
            d::Step::Idle => return,
            d::Step::Reset | d::Step::Broken => -1,
            d::Step::Hit(stage) => stage,
        };
        if step != d::Step::Reset {
            let at = cell.map(|v| f64::from(v) + 0.5);
            // SoundSource 4 is `block`.
            game.with_level(|_, level, _| level.native_play_sound(&hit, at, 4, 0.25, 1.0));
        }
        let id = d::breaker_id(cell);
        game.destroy_block_progress(id, cell, crack);
        if let Some(host) = game.lan.as_ref() {
            host.hub.broadcast_near(
                cell.map(f64::from),
                64.0,
                &horizon_protocol::packets::play::Clientbound::BlockDestruction(horizon_protocol::packets::play::BlockDestruction {
                    id,
                    pos: cell,
                    progress: if (0..10).contains(&crack) { crack as u8 } else { 255 },
                }),
                None,
            );
        }
        if step == d::Step::Broken {
            // An error here is the level's events after the break, which the game itself only reports.
            let _ = game.destroy_block_dropping(cell);
        }
    }

    /// Puts the blocks back where the offset rounds to, then removes the displays and the marker: false.
    fn c_place(
        game: &mut crate::game::Game<'_>,
        dimension: &str,
        l: &mut crate::game::kinetics_contraption::Live,
        origin: [i32; 3],
        offset: [f64; 3],
        list: &[horizon_world::nbt::Tag],
        data: &horizon_world::nbt::Tag,
        spin: Option<([i32; 3], i32)>,
    ) -> bool {
        use crate::game::kinetics_contraption as c;
        use horizon_world::nbt::Tag;
        // A bearing's blocks land turned by the quarter turns it made, and so do their `facing` and `axis`.
        let turned = |v: [i32; 3]| spin.map_or(v, |(axis, q)| c::turn(v, axis, q));
        for p in list {
            let r = match p.get("pos") {
                Some(Tag::IntArray(v)) if v.len() == 3 => turned([v[0], v[1], v[2]]),
                _ => continue,
            };
            let mut st = p.get("state").cloned().unwrap_or(Tag::Compound(Compound::new()));
            if let (Some((axis, q)), Some(Tag::Compound(props))) = (spin, st.get_mut("Properties")) {
                for (key, value) in props.iter_mut() {
                    if let Tag::String(v) = value {
                        if let Some(t) = c::turn_property(&key.to_string_lossy(), &v.to_string_lossy(), axis, q) {
                            *value = Tag::String(t.as_str().into());
                        }
                    }
                }
            }
            let Some(state) = game.state_from_nbt(&st) else { continue };
            let at = c::landing(origin, offset, r);
            let facts = game.res.facts().get(game.world.block(at[0], at[1], at[2]));
            let free = c::movable(&facts.name, facts.hardness, facts.has_class("LiquidBlock"), false) == c::Movable::Skip;
            if free && game.native_set_block(dimension, at, state, c::PLACE_FLAGS) {
                if let (Some(Tag::Compound(saved)), Some(BlockEntity::Mod(m))) =
                    (p.get("nbt"), game.world.block_entities.get_mut(&(at[0], at[1], at[2])))
                {
                    m.nbt.extend(saved.clone());
                }
                continue;
            }
            // shortcut: the item is the block's own id (true for our blocks and most others, not for a wall torch
            // or redstone wire), with no loot table; use the block's loot when Horizon has a helper for it.
            let name = game.res.facts().get(state).name.to_string();
            let mut item = Compound::new();
            item.insert("id".into(), Tag::String(name.as_str().into()));
            item.insert("count".into(), Tag::Int(1));
            let mut nbt = Compound::new();
            nbt.insert("Item".into(), Tag::Compound(item));
            game.native_summon(dimension, "minecraft:item", at.map(|v| f64::from(v) + 0.5), nbt);
        }
        // The glue comes back where the blocks are.
        if let Some(Tag::List(boxes)) = data.get("glue") {
            let at = |v: &[i32], k: usize| {
                let r = turned([v[k], v[k + 1], v[k + 2]]);
                [0, 1, 2].map(|i| c::landing(origin, offset, [0; 3])[i] + r[i])
            };
            for b in boxes {
                if let Tag::IntArray(v) = b {
                    if v.len() == 6 {
                        let g = c::Glue::new(at(v, 0), at(v, 3));
                        let (min, max) = (g.min, g.max);
                        let mut d = Compound::new();
                        d.insert("min".into(), Tag::IntArray(min.to_vec()));
                        d.insert("max".into(), Tag::IntArray(max.to_vec()));
                        let mut nbt = Compound::new();
                        nbt.insert("Tags".into(), Tag::List(vec![Tag::String(c::GLUE_TAG.into())]));
                        nbt.insert("data".into(), Tag::Compound(d));
                        game.native_summon(dimension, "minecraft:marker", min.map(f64::from), nbt);
                    }
                }
            }
        }
        // A bearing can start another turn.
        if let Some(Tag::IntArray(v)) = data.get("bearing") {
            if let (true, Some(BlockEntity::Mod(m))) = (v.len() == 3, game.world.block_entities.get_mut(&(v[0], v[1], v[2]))) {
                m.nbt.insert(horizon_world::nbt::JavaString::from(c::RUNNING_KEY), Tag::Int(0));
            }
        }
        for id in l.displays.drain(..) {
            game.native_remove_entity(dimension, id);
        }
        game.native_remove_entity(dimension, l.marker);
        false
    }
}
