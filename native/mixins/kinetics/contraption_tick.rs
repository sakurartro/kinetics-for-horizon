// At the end of each level's tick, the contraptions in it (contraption.rs): each marker gets its displays (again,
// after a reload or its chunk coming back: entity ids change, so tagged displays no contraption holds are removed),
// then moves a step toward its target (or waits while its drills break what is in the way, or for good against a
// solid block) and, there, puts its blocks back into the world (Create's
// `Contraption.addBlocksToWorld`: what is in its place is broken, or the moved block drops if it can't go in) and goes.
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
            mine.push(c::Live { dimension: dimension.to_owned(), marker: m, displays: Vec::new(), cart: None, lost: 0 });
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
        // The block it is anchored to (piston, bearing, a cart's assembler) and the poles stay (`isAnchoringBlockAt`).
        let anchoring = r.owner.map(|o| o.0).or(r.cart.map(|_| [from[0], from[1] - 1, from[2]]));
        let found = c::collect(
            from,
            &boxes,
            |p| {
                let facts = game.res.facts().get(game.world.block(p[0], p[1], p[2]));
                if Some(p) == anchoring || &*facts.name == c::POLE {
                    return c::Movable::Skip;
                }
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
        if let Some((p, c::EXTENDED_KEY, out)) = r.owner {
            // A piston's: its place, and the offsets of the far end of the poles and of the near one, so that turned
            // the other way it takes the structure back (piston_tick.rs).
            let (far, near) = if out == 1 { (r.by, [0; 3]) } else { ([0; 3], r.by) };
            data.insert("piston".into(), Tag::IntArray(p.to_vec()));
            data.insert("out".into(), doubles(far.map(f64::from)));
            data.insert("in".into(), doubles(near.map(f64::from)));
        }
        if let Some((cart, yaw)) = r.cart {
            // A cart's: the cart it rides, and its first yaw, which it turns from.
            data.insert("cart".into(), Tag::Int(cart as i32));
            data.insert("initial".into(), Tag::Double(yaw.to_bits()));
            data.insert("yaw".into(), Tag::Double(yaw.to_bits()));
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
        // A cart's are made where its marker (its cart) is now, its origin may be far away and unloaded; c_ride puts
        // them in place the same tick.
        let riding = data
            .get("cart")
            .and_then(|_| game.others.iter().find(|o| o.id == l.marker))
            .map(|o| [o.position.x, o.position.y, o.position.z]);
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
                match game.native_summon(dimension, "minecraft:block_display", riding.unwrap_or_else(|| at(offset, rel(p))), nbt) {
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
        if data.get("cart").is_some() {
            return c_ride(game, dimension, l, &data, origin, list);
        }
        if let Some(axis) = ints("axis") {
            return c_spin(game, dimension, l, &data, origin, axis, list);
        }
        if offset == target {
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
                c_drill(game, l.marker, i, &list[i], cell, c::drill_rpm(speed, c::DRILL_MIN));
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

    /// What the world block at `q` is to a contraption entering it. A drill leaves a block with no collision shape
    /// (grass, a torch, a rail) alone (`DrillMovementBehaviour.canBreak`).
    fn c_in_way(game: &crate::game::Game<'_>, q: [i32; 3]) -> crate::game::kinetics_contraption::InWay {
        use crate::game::{kinetics_contraption as c, kinetics_drill as d};
        let f = game.res.facts().get(game.world.block(q[0], q[1], q[2]));
        let breakable = d::breakable(&f.name, f.hardness, f.has_class("LiquidBlock")) && !game.collision_empty(q);
        c::InWay { solid: f.legacy_solid, breakable }
    }

    /// A cart's contraption (BUR.md phase 5, Create's `OrientedContraptionEntity` riding a minecart): it sits on its
    /// cart, the assembler's block under it, and turns to face the way the cart goes; its drills hold the cart still
    /// while they break what they ran into (anything else goes through, as in Create). On an assembler with no
    /// signal it is put down facing that assembler's arrow, and where it is once its cart is gone: false.
    fn c_ride(
        game: &mut crate::game::Game<'_>,
        dimension: &str,
        l: &mut crate::game::kinetics_contraption::Live,
        data: &horizon_world::nbt::Tag,
        origin: [i32; 3],
        list: &[horizon_world::nbt::Tag],
    ) -> bool {
        use crate::game::kinetics_contraption as c;
        use crate::game::native_entities::NativeDisplay;
        use horizon_core::geometry::Vec3;
        use horizon_world::nbt::{JavaString, Tag};
        const UP: [i32; 3] = [0, 1, 0];
        let num = |key: &str| match data.get(key) {
            Some(Tag::Double(b)) => f64::from_bits(*b),
            _ => 0.0,
        };
        let (initial, yaw) = (num("initial"), num("yaw"));
        let rel = |p: &Tag| match p.get("pos") {
            Some(Tag::IntArray(v)) if v.len() == 3 => [v[0], v[1], v[2]],
            _ => [0; 3],
        };
        // c_place lands the blocks from the anchor block (the assembler's place); the snapshot starts a block above it.
        let from = |anchor: [i32; 3]| [0, 1, 2].map(|k| f64::from(anchor[k] + i32::from(k == 1) - origin[k]));
        let marker = game.others.iter().find(|o| o.id == l.marker).map(|o| [o.position.x, o.position.y, o.position.z]);
        let dist = |a: [f64; 3], b: Vec3| (a[0] - b.x).powi(2) + (a[1] - b.y).powi(2) + (a[2] - b.z).powi(2);
        let wanted = l.cart.or(match data.get("cart") {
            Some(Tag::Int(id)) => Some(*id as u32),
            _ => None,
        });
        let mut cart = wanted.and_then(|id| game.carts.iter().position(|k| k.id == id));
        // After a reload ids are new: the cart is the one under the marker, which moves with it.
        if let Some(m) = marker {
            if cart.is_none_or(|i| dist(m, game.carts[i].position) > 4.0) {
                cart = (0..game.carts.len())
                    .filter(|&i| dist(m, game.carts[i].position) <= 4.0)
                    .min_by(|&a, &b| dist(m, game.carts[a].position).total_cmp(&dist(m, game.carts[b].position)));
            }
        }
        let Some(i) = cart else {
            // shortcut: a missing cart (broken, or its chunk not back yet) is waited for 10 ticks before the blocks
            // are put down; Create's contraption is saved inside its cart, so it never waits. Upgrade if one is
            // put down by mistake next to a cart that loads late.
            l.lost += 1;
            return match marker {
                Some(m) if l.lost >= 10 => {
                    let at = c::cart_anchor(m);
                    c_place(game, dimension, l, origin, from(at), list, data, Some((UP, c::cart_quarters(initial, yaw))))
                }
                _ => true,
            };
        };
        l.lost = 0;
        l.cart = Some(game.carts[i].id);
        let (p, v) = (game.carts[i].position, game.carts[i].velocity);
        let (pos, vel) = ([p.x, p.y, p.z], [v.x, v.y, v.z]);
        // On an assembler with no signal (`disassemble`: its yaw becomes the assembler's arrow).
        let under = pos.map(|x| x.floor() as i32);
        if &*game.res.facts().get(game.world.block(under[0], under[1], under[2])).name == c::ASSEMBLER
            && game.native_block_property(under, "powered").as_deref() != Some("true")
        {
            let shape = game.native_block_property(under, "shape").unwrap_or_default();
            let front = c::assembler_yaw(&shape, game.native_block_property(under, "backwards").as_deref() == Some("true"));
            let at = c::cart_anchor(pos);
            return c_place(game, dimension, l, origin, from(at), list, data, Some((UP, c::cart_quarters(initial, front))));
        }
        // Stalled, it keeps the motion the cart had (`motionBeforeStall`) and doesn't turn.
        let stall = match data.get("stall") {
            Some(Tag::List(s)) if s.len() == 3 => Some([0, 1, 2].map(|k| if let Tag::Double(b) = &s[k] { f64::from_bits(*b) } else { 0.0 })),
            _ => None,
        };
        let motion = stall.unwrap_or(vel);
        let size = 2.0 * list.iter().map(|q| { let r = rel(q); f64::from(r[0]).hypot(f64::from(r[2])) }).fold(0.0, f64::max) + 1.0;
        let yaw = if stall.is_some() { yaw } else { c::cart_yaw(yaw, vel, size) };
        let turn = initial - yaw;
        // A block's centre: the anchor block's centre is half a block over the cart, the snapshot a block above that.
        let centre = |r: [i32; 3]| {
            let w = c::rotate(r.map(f64::from), UP, turn);
            [pos[0] + w[0], pos[1] + 1.5 + w[1], pos[2] + w[2]]
        };
        // Each drill facing the motion whose centre is in a block it can break (`visitNewPosition`): it stalls.
        let speed = (motion[0] * motion[0] + motion[1] * motion[1] + motion[2] * motion[2]).sqrt();
        let mut drilling = Vec::new();
        if speed >= 1.0 / 512.0 {
            for (k, q) in list.iter().enumerate() {
                let Some(f) = c_facing(q) else { continue };
                let f = c::rotate(f.map(f64::from), UP, turn);
                if !c::drills_toward(f, motion) {
                    continue;
                }
                // It works a little ahead of its block's centre (`getActiveAreaOffset`).
                let at = centre(rel(q));
                let cell = [0, 1, 2].map(|k| (at[k] + f[k] * c::DRILL_REACH).floor() as i32);
                if c_in_way(game, cell).breakable {
                    drilling.push((k, cell));
                }
            }
        }
        if !drilling.is_empty() {
            game.carts[i].velocity = Vec3::ZERO;
        } else if let Some(s) = stall {
            game.carts[i].velocity = Vec3::new(s[0], s[1], s[2]);
        }
        for &(k, cell) in &drilling {
            c_drill(game, l.marker, k, &list[k], cell, c::drill_rpm(speed, c::CART_DRILL_MIN));
        }
        let shown = NativeDisplay {
            translation: c::rotate([-0.5; 3], UP, turn).map(|x| x as f32),
            left_rotation: c::quaternion(UP, turn),
            interpolation: 1,
            teleport: 1,
            ..Default::default()
        };
        for (id, q) in l.displays.iter().zip(list) {
            game.native_move_entity(dimension, *id, centre(rel(q)), 0.0, 0.0);
            game.native_set_display(dimension, *id, &shown);
        }
        game.native_move_entity(dimension, l.marker, pos, 0.0, 0.0);
        if let Some(Tag::Compound(d)) = game.persist.raw.get_mut(&l.marker).and_then(|t| t.get_mut("data")) {
            d.insert("yaw".into(), Tag::Double(yaw.to_bits()));
            match (drilling.is_empty(), stall) {
                (false, None) => {
                    d.insert("stall".into(), Tag::List(vel.iter().map(|x| Tag::Double(x.to_bits())).collect()));
                }
                (true, Some(_)) => {
                    d.remove(&JavaString::from("stall"));
                }
                _ => {}
            }
        }
        true
    }

    /// A turning contraption's tick (BUR.md phase 6, Create's `MechanicalBearingBlockEntity`): the bearing's speed turns
    /// it about the axis through its anchor, and drills dig where they go, holding it while they do; terrain doesn't
    /// stop it (only a `TranslatingContraption` collides with blocks, `ContraptionCollider.collideBlocks`). With the
    /// bearing still it is put down on the nearest quarter turn.
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
        if !way.drill.is_empty() {
            for (i, cell) in way.drill {
                let m = motions[i];
                c_drill(game, l.marker, i, &list[i], cell, c::drill_rpm((m[0] * m[0] + m[1] * m[1] + m[2] * m[2]).sqrt(), c::DRILL_MIN));
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
        use horizon_core::position::BlockPos;
        use horizon_world::nbt::Tag;
        // A bearing's blocks land turned by the quarter turns it made, and so do their `facing` and `axis`.
        let turned = |v: [i32; 3]| spin.map_or(v, |(axis, q)| c::turn(v, axis, q));
        let registry = game.pack.assets.registry;
        for p in list {
            let r = match p.get("pos") {
                Some(Tag::IntArray(v)) if v.len() == 3 => turned([v[0], v[1], v[2]]),
                _ => continue,
            };
            let Some(mut state) = p.get("state").and_then(|s| game.state_from_nbt(s)) else { continue };
            if let (Some((axis, q)), Some(s)) = (spin, registry.state(state).cloned()) {
                for (key, value) in &s.properties {
                    let Some(t) = c::turn_property(key, value, axis, q) else { continue };
                    // A value the block doesn't take (a furnace turned to face up) keeps the one it had.
                    if let Some(mut now) = registry.state(state).cloned() {
                        now.properties.insert(key.clone(), t);
                        if let Ok(id) = registry.resolve(&now) {
                            state = id;
                        }
                    }
                }
            }
            let at = c::landing(origin, offset, r);
            let here = BlockPos::new(at[0], at[1], at[2]);
            // `addBlocksToWorld`: the moved block drops (its loot) if the place is unbreakable, or it has no collision
            // shape and what is there has one; otherwise what is there is broken, with its drops, and it goes in.
            let facts = game.res.facts().get(game.world.block(at[0], at[1], at[2]));
            let empty = {
                let mut scene = horizon_client::collisions::SceneCollisions { world: &game.world, binding: game.collision, subject: None };
                game.collision
                    .collision_shape_with_context(&mut scene, state, here, &horizon_world::collision_context::CollisionContext::empty(false))
                    .is_ok_and(|shape| shape.is_empty())
            };
            let air = matches!(&*facts.name, "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air");
            if facts.hardness >= 0.0 && !(empty && !game.collision_empty(at)) {
                if !air {
                    // An error here is the level's events after the break, which the game itself only reports.
                    let _ = game.destroy_block_dropping(at);
                }
                if game.native_set_block(dimension, at, state, c::PLACE_FLAGS) {
                    if let (Some(Tag::Compound(saved)), Some(BlockEntity::Mod(m))) =
                        (p.get("nbt"), game.world.block_entities.get_mut(&(at[0], at[1], at[2])))
                    {
                        m.nbt.extend(saved.clone());
                    }
                    continue;
                }
            }
            // `Block.dropResources(state, level, pos, null)`.
            game.with_level(|_, level, _| level.drop_resources_now(here, state));
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
