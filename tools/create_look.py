#!/usr/bin/env python3
"""Builds local-only Create-look files from a Create jar you downloaded yourself.

    python3 tools/create_look.py ~/Downloads/create-1.21.1-6.0.10.jar

Create's 1.21.1 (NeoForge) block and item models and their referenced textures still load in Horizon 26.3.
The resource pack is for the native in-world renderer. A private local .hmod copy points Kinetics item
definitions at Create's item models, so they also show in inventory and creative menus. Horizon layers
content mods after resource packs, so a resource-pack-only item override cannot replace Kinetics' definitions.
If a Create item model needs an unsupported NeoForge loader, the local .hmod keeps Kinetics' existing icon.
Create's assets are All Rights Reserved: both outputs are for your own game; never share or commit them.
Default outputs: resourcepacks/kinetics-create-look.zip and kinetics-create-look.hmod in the Horizon game folder.
"""
import argparse, json, os, pathlib, re, sys, zipfile


def game_home():
    """The Horizon game folder, as the launcher finds it: HORIZON_HOME, else `horizon` in the platform's data folder."""
    if os.environ.get("HORIZON_HOME"):
        return pathlib.Path(os.environ["HORIZON_HOME"])
    if sys.platform == "win32":
        base = pathlib.Path(os.environ.get("APPDATA") or pathlib.Path.home() / "AppData/Roaming")
    elif sys.platform == "darwin":
        base = pathlib.Path.home() / "Library/Application Support"
    else:
        base = pathlib.Path(os.environ.get("XDG_DATA_HOME") or pathlib.Path.home() / ".local/share")
    return base / "horizon"

# Create models the Kinetics mod draws first (runtime.rs LOOK_MODELS), all built along the y axis but shaft_half (casing.rs, clutch.rs), along z.
MODELS = ["shaft", "cogwheel", "large_cogwheel"]
# The andesite casing's (casing.rs LOOK_MODELS, STATIC_LOOK_MODELS): the cased parts' turning parts, and the static
# models drawn over the placeholders; the cogwheels' have a variant per shaft end.
MODELS += ["cogwheel_shaftless", "large_cogwheel_shaftless", "shaft_half", "andesite_casing", "encased_shaft/block_andesite"]
MODELS += [f"andesite_encased_{big}cogwheel{ends}" for big in ("", "large_")
           for ends in ("", "_top", "_bottom", "_top_bottom")]
MODELS += ["hand_crank/block", "hand_crank/handle"]  # crank.rs LOOK_MODELS
# The cuckoo clocks (clock.rs LOOK_MODELS).
MODELS += [f"cuckoo_clock/{m}" for m in ("block", "hour_hand", "minute_hand", "left_door", "right_door", "pig", "creeper")]
MODELS += ["mechanical_drill/block", "mechanical_drill/head"]  # drill.rs LOOK_MODELS (body built up, head south)
MODELS += ["bearing/top"]  # bearing_submit.rs (its half shaft is the casing's)
MODELS += ["rotation_speed_controller/bracket"]  # submit.rs, under a large cogwheel
MODELS += ["water_wheel/block"]  # water_wheel.rs LOOK_FRAME (the wheels themselves are OBJ models, which the game does not draw)
# The gearbox's and the split blocks' static models (gearbox.rs STATIC_LOOK, clutch.rs STATIC_LOOK_MODELS).
MODELS += ["gearbox/block", "clutch/block", "clutch/block_powered", "gearshift/block", "gearshift/block_powered"]
# The brass and copper casings (casing.rs); copper_casing is only a cube, native code doesn't ask for it yet.
MODELS += ["brass_casing", "copper_casing", "encased_shaft/block_brass"]
MODELS += [f"brass_encased_{big}cogwheel{ends}" for big in ("", "large_")
           for ends in ("", "_top", "_bottom", "_top_bottom")]
# The chain drives' static models (chain.rs static_look); only the gearshift has a _powered one in Create.
_CHAIN = ["single", "middle_horizontal", "middle_vertical", "end_horizontal", "end_vertical"]
MODELS += [f"encased_chain_drive/{c}" for c in _CHAIN]
MODELS += [f"adjustable_chain_gearshift_{c}{p}" for c in _CHAIN for p in ("", "_powered")]
# The brackets (bracket.rs): the datagen-ed, material-suffixed files (the plain ones have unresolved texture variables).
MODELS += [f"bracket/{t}/{k}_{m}" for t in ("shaft", "cog") for k in ("ground", "wall") for m in ("wooden", "metal")]
# The gauges (stressometer.rs / speedometer.rs LOOK_MODELS): the two bodies, the dial and each head.
MODELS += ["gauge/base", "gauge/base_wall", "gauge/dial", "gauge/stressometer/head", "gauge/speedometer/head"]
PACK_FORMAT = [97, 1]  # Minecraft 26.3 (horizon-core version.rs RESOURCE_PACK_FORMAT)


def ref(name):
    """'create:block/x' or 'block/x' -> (namespace, path)."""
    ns, _, path = name.rpartition(":")
    return (ns or "minecraft"), path


def local_manifest(source):
    """Add Create's asset namespace to a private Kinetics content-mod copy."""
    text = (source / "horizon-mod.toml").read_text()
    match = re.search(r"(?m)^namespaces\s*=\s*\[(.*?)\]\s*$", text)
    namespaces = set(re.findall(r'"([a-z0-9_.-]+)"', match.group(1))) if match else set()
    if match and "create" in namespaces:
        return text
    namespaces.add("create")
    encoded = "namespaces = " + json.dumps(sorted(namespaces))
    if match:
        return text[:match.start()] + encoded + text[match.end():]
    return text.rstrip() + "\n" + encoded + "\n"


def item_names(source):
    """Kinetics' explicit items plus blocks which create an item by default."""
    names = {p.stem for p in (source / "assets/kinetics/items").glob("*.json")}
    blocks = source / "data/kinetics/horizon/block"
    if blocks.is_dir():
        for path in blocks.glob("*.json"):
            block = json.loads(path.read_bytes())
            if block.get("item", True) is not False:
                names.add(path.stem)
    return names


def build_local_mod(source, out, extracted_assets, create_items):
    """Write a private .hmod copy with its item definitions pointed at Create models."""
    if not (source / "horizon-mod.toml").is_file():
        sys.exit(f"{source}: no horizon-mod.toml (expected the Kinetics content-mod folder)")

    entries = {}
    for folder in ("assets", "data"):
        root = source / folder
        if root.is_dir():
            for path in root.rglob("*"):
                if path.is_file():
                    entries[path.relative_to(source).as_posix()] = path.read_bytes()

    # Horizon 26.3 does not recognize this vanilla component yet. It is only
    # excluded from the private local copy so the rest of Kinetics can load.
    goggles = "data/kinetics/horizon/item/goggles.json"
    if goggles in entries:
        item = json.loads(entries[goggles])
        item.get("components", {}).pop("!minecraft:dyed_color", None)
        entries[goggles] = (json.dumps(item, indent=2) + "\n").encode()

    replaced = []
    for item_model in sorted(create_items):
        rel = f"assets/kinetics/items/{item_model}.json"
        item = json.loads(entries[rel]) if rel in entries else {
            "model": {"type": "minecraft:model", "model": f"create:item/{item_model}"}
        }
        item.setdefault("model", {})["model"] = f"create:item/{item_model}"
        entries[rel] = (json.dumps(item, indent=2) + "\n").encode()
        replaced.append(item_model)

    # A moving contraption shows its blocks as block displays: only blockstate models, none of what block entities draw
    # over them. So the casings' placeholders become Create's cubes (the static look drawn over them is the same), and
    # the drill's display block (contraption_tick.rs) Create's whole drill, its item model, built facing south.
    for casing in ("andesite_casing", "brass_casing", "copper_casing"):
        entries[f"assets/kinetics/models/block/{casing}.json"] = json.dumps({"parent": f"create:block/{casing}"}).encode()
    if "mechanical_drill" in create_items:
        turns = {"south": {}, "north": {"y": 180}, "east": {"y": 270}, "west": {"y": 90}, "up": {"x": 90}, "down": {"x": 270}}
        entries["assets/kinetics/blockstates/drill_display.json"] = json.dumps({"variants": {
            f"facing={f}": {"model": "create:block/mechanical_drill/item", **r} for f, r in turns.items()}}, indent=2).encode()

    entries.update(extracted_assets)
    out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(out, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("horizon-mod.toml", local_manifest(source).encode())
        for path in sorted(entries):
            z.writestr(path, entries[path])
    return replaced


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("jar", type=pathlib.Path, help="create-1.21.1-*.jar")
    ap.add_argument("--out", type=pathlib.Path,
                    default=game_home() / "resourcepacks/kinetics-create-look.zip")
    ap.add_argument("--content", type=pathlib.Path, default=pathlib.Path(__file__).resolve().parents[1] / "content",
                    help="Kinetics content-mod source folder (default: ../content beside this script)")
    ap.add_argument("--mod-out", type=pathlib.Path,
                    default=game_home() / "kinetics-create-look.hmod",
                    help="private local .hmod output (default: Horizon game folder, outside horizon-mods)")
    a = ap.parse_args()

    try:
        jar = zipfile.ZipFile(a.jar)
    except (OSError, zipfile.BadZipFile) as e:
        sys.exit(f"{a.jar}: not a jar ({e})")
    names = set(jar.namelist())
    toml = jar.read("META-INF/neoforge.mods.toml").decode() if "META-INF/neoforge.mods.toml" in names else ""
    if not re.search(r'modId\s*=\s*"create"', toml):
        sys.exit(f"{a.jar}: not Create for NeoForge (no modId \"create\" in META-INF/neoforge.mods.toml)")
    if "1.21.1" not in a.jar.name:
        print(f"warning: {a.jar.name} doesn't say 1.21.1; other versions' models may differ", file=sys.stderr)

    files = {}  # path in the pack -> bytes
    unsupported_models = set()

    def model(name):
        ns, path = ref(name)
        if ns != "create":
            return True  # vanilla's own (block/block): the game has it
        entry = f"assets/create/models/{path}.json"
        if entry in files:
            return entry not in unsupported_models
        if entry not in names:
            sys.exit(f"{a.jar.name} has no {entry}: another Create version?")
        data = jar.read(entry)
        m = json.loads(data)
        if path == "block/gearbox/block":  # the core stops 0.95 short of the box edge: lying on its side, that gap is see-through
            core = m["elements"][1]
            core["from"][0] = core["from"][2] = 0.01
            core["to"][0] = core["to"][2] = 15.99
            for face in core["faces"].values():
                face["uv"] = [0, 2, 16, 14]
            data = json.dumps(m, indent=2).encode()
        files[entry] = data
        supported = True
        if "loader" in m:
            print(f"warning: {entry} uses NeoForge loader {m['loader']!r}, which Horizon may not draw", file=sys.stderr)
            unsupported_models.add(entry)
            supported = False
        for tex in m.get("textures", {}).values():
            if isinstance(tex, str) and not tex.startswith("#"):
                texture(tex)
        if "parent" in m:
            supported = model(m["parent"]) and supported
        if not supported:
            unsupported_models.add(entry)
        return supported

    def texture(name):
        ns, path = ref(name)
        if ns != "create":
            return
        for ext in (".png", ".png.mcmeta"):
            entry = f"assets/create/textures/{path}{ext}"
            if entry in names:
                files[entry] = jar.read(entry)
        if f"assets/create/textures/{path}.png" not in names:
            print(f"warning: {a.jar.name} has no texture {name}", file=sys.stderr)

    def particle(name):
        """The particle texture of Create's model `name`, looked up its parents, pulled into the pack."""
        textures, parent = {}, name
        while parent and ref(parent)[0] == "create":
            m = json.loads(jar.read(f"assets/create/models/{ref(parent)[1]}.json"))
            textures = {**m.get("textures", {}), **textures}
            parent = m.get("parent")
        tex = textures.get("particle")
        while tex and tex.startswith("#"):
            tex = textures.get(tex[1:])
        if not tex:
            sys.exit(f"{a.jar.name}: {name} has no particle texture: another Create version?")
        texture(tex)
        return tex

    def blockstate(block):
        return json.loads(jar.read(f"assets/create/blockstates/{block}.json"))

    def matched(block, fixed=()):
        """Kinetics' variants of `block` with Create's model and turns where Create has the same state (the Create
        properties Kinetics lacks taken from `fixed`), Kinetics' placeholder where Create has none."""
        props = lambda key: dict(p.split("=") for p in key.split(",") if p)
        ours = json.loads((a.content / f"assets/kinetics/blockstates/{block}.json").read_bytes())["variants"]
        theirs = [(props(k), v) for k, v in blockstate(block)["variants"].items()]
        out = {}
        for key, v in ours.items():
            want = {**dict(fixed), **props(key)}
            out[key] = next((c for p, c in theirs if p == want), v)
            if out[key] is not v:
                model(out[key]["model"])
        return {"variants": out}

    all_models = MODELS + ["creative_motor/block", "creative_motor/block_vertical"]
    for name in all_models:
        model(f"create:block/{name}")
    # The piston's models for its blockstate (the piston's own code and blockstate are on their way).
    for v in blockstate("mechanical_piston")["variants"].values():
        model(v["model"])

    # The local .hmod's own Kinetics files, laid over Kinetics' (the pack can't replace a mod's files).
    local = {}
    # Blocks no native code draws Create's look over: Create's blockstates, on Kinetics' state properties.
    local_states = {
        "rotation_speed_controller": matched("rotation_speed_controller"),
        "mechanical_bearing": matched("mechanical_bearing"),
        # Create's rail types but the powered rail, and its ascending shapes, aren't Kinetics': those keep theirs.
        "cart_assembler": matched("cart_assembler", {"rail_type": "powered_rail", "waterlogged": "false"}),
        "metal_girder_encased_shaft": blockstate("metal_girder_encased_shaft"),  # the same axis, top and bottom
        # Kinetics' pole is a chain, on an axis: Create's facings up, south and east.
        "piston_extension_pole": {"variants": {
            "axis=y": {"model": "create:block/piston_extension_pole"},
            "axis=z": {"model": "create:block/piston_extension_pole", "x": 90},
            "axis=x": {"model": "create:block/piston_extension_pole", "x": 90, "y": 90}}},
        # Kinetics' girder has one `beam` (none, x, z, xz) for Create's `x` and `z`.
        "metal_girder": {"multipart": [
            {"when": {"beam": "none"}, "apply": {"model": "create:block/metal_girder/block_pole"}},
            {"when": {"beam": "x|xz"}, "apply": {"model": "create:block/metal_girder/block_x"}},
            {"when": {"beam": "z|xz"}, "apply": {"model": "create:block/metal_girder/block_z"}},
            {"when": {"beam": "x|z", "top": "true"}, "apply": {"model": "create:block/metal_girder/block_top"}},
            {"when": {"beam": "x|z", "bottom": "true"}, "apply": {"model": "create:block/metal_girder/block_bottom"}},
            {"when": {"beam": "xz"}, "apply": {"model": "create:block/metal_girder/block_cross"}}]},
    }
    for block, state in local_states.items():
        for part in state.get("multipart", []):
            model(part["apply"]["model"])
        local[f"assets/kinetics/blockstates/{block}.json"] = json.dumps(state, indent=2).encode()
    # Placeholders native code draws Create's model over (or in place of): only their break particles, Create's.
    # The cased shafts' and chain drives' walls showed through Create's recessed cores.
    placeholders = {
        "shaft_block": "shaft", "cogwheel_block": "cogwheel", "large_cogwheel_block": "large_cogwheel",
        "crank_block": "hand_crank/block", "clock_block": "cuckoo_clock/block", "drill_block": "mechanical_drill/block",
        "motor_block": "creative_motor/block", "flywheel_block": "flywheel/block",
        "andesite_encased_shaft_block": "encased_shaft/block_andesite",
        "brass_encased_shaft_block": "encased_shaft/block_brass",
        "chain_drive_block": "encased_chain_drive/single",
        "adjustable_chain_gearshift_block": "adjustable_chain_gearshift_single",
        "adjustable_chain_gearshift_powered_block": "adjustable_chain_gearshift_single_powered",
    }
    for ours, theirs in placeholders.items():
        local[f"assets/kinetics/models/block/{ours}.json"] = json.dumps(
            {"textures": {"particle": particle(f"create:block/{theirs}")}}).encode()
    items = item_names(a.content)
    if not (a.content / "assets/kinetics/items").is_dir():
        sys.exit(f"{a.content}: missing Kinetics item definitions; pass --content with the content-mod folder")
    create_items = []
    for item_name in sorted(items):
        item_model = f"item/{item_name}"
        if f"assets/create/models/{item_model}.json" not in names:
            print(f"warning: no Create item model for kinetics:{item_name}; keeping its Kinetics icon", file=sys.stderr)
            continue
        if not model(f"create:{item_model}"):
            print(f"warning: Create item model for kinetics:{item_name} uses an unsupported loader; keeping its Kinetics icon", file=sys.stderr)
            continue
        create_items.append(item_name)
    if "wrench" in create_items:
        # Create's wrench is its handle and its gear (WrenchItemRenderer turns the gear; here it stands still).
        model("create:item/wrench/gear")
        local["assets/kinetics/items/wrench.json"] = json.dumps({"model": {"type": "minecraft:composite", "models": [
            {"type": "minecraft:model", "model": f"create:item/wrench/{m}"} for m in ("item", "gear")]}}, indent=2).encode()
    files["pack.mcmeta"] = json.dumps({"pack": {
        "description": "Kinetics with Create's look (from your own Create jar; don't share)",
        "min_format": PACK_FORMAT, "max_format": PACK_FORMAT}}, indent=2).encode()

    a.out.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(a.out, "w", zipfile.ZIP_DEFLATED) as z:
        for path in sorted(files):
            z.writestr(path, files[path])
    replaced = build_local_mod(
        a.content,
        a.mod_out,
        {**{p: b for p, b in files.items() if p.startswith("assets/")}, **local},
        create_items,
    )
    print(f"{a.out}: {len(files)} files. Turn it on in Options > Resource Packs for the world look.")
    print(f"{a.mod_out}: local-only Kinetics .hmod; Create item models: {', '.join(replaced)}")
    print("Use this .hmod instead of the regular Kinetics content .hmod; keep the Kinetics .hnmod installed.")


if __name__ == "__main__":
    main()
