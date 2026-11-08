# Kinetics for Horizon

PRE PRE Alpha(Very raw). An unofficial mod for [Horizon](https://github.com/horizon-rust/horizon) (a Minecraft 26.3 client in Rust) that ports the **kinetic core of Create**: rotation you can build with, and nothing else.

Not affiliated with the Create team.

## Scope

This is the rotation core, not the whole of Create. Included: shafts, cogwheels, casings, gearboxes, clutch and gearshift, chain drives, brackets, girders, water wheels, hand crank, creative motor, rotation source, rotation speed controller, flywheel, cuckoo clocks, stress, gauges, wrench and goggles, and a first, untested set of moving structures: mechanical drill, super glue, mechanical piston, mechanical bearing and cart assembler (see BUR.md). Not included: belts, fluids, trains, chests and other blocks with contents on a moving structure, collision of players with it, crafting machines, the rest of Create's content. Those need systems Horizon's mod API doesn't have yet, and the core had to work first.

Rotation rules follow Create: speed travels along shafts, meshing cogwheels reverse direction, large↔small gives ×2/×½, a conflict stops the network.

## Requirements

- Horizon with its launcher, on **Minecraft 26.3**.
- The native part is pinned to Horizon commit `c5c8a33` (hook points API 153, needs 129 and 151). Older Nightlies, such as 2026-11-01 (API 80), won't work. If your Horizon differs, the launcher asks before building ("Build anyway").
- Rust is not needed to play; the launcher builds the game for you. The first build takes 20–30 minutes. Building the mod files yourself (below) does need Rust and `cargo-horizon-mod`.
- **Tested only on Arch Linux** (Omarchy). Other distros, Windows and macOS are untested; the paths below for them come from reading the code, not from running it.
- **Alpha:** A LOT OF BUGS

## Install the mod

The mod has two files: `kinetics.hmod` (blocks, models, recipes) and `kinetics-0.1.0.hnmod` (the Rust code that makes things turn). You need both.

1. Build them (or take them from Releases, if there is one):
   ```sh
   cd content && cargo horizon-mod build   # -> content/target/horizon-mod/kinetics.hmod
   python3 native/package.py               # -> native/target/kinetics-0.1.0.hnmod
   ```
2. Copy them into your Horizon game folder. On Linux it is `~/.local/share/horizon/` (full paths below; on other systems it is the folder with `saves/` and `options.txt`):
   ```sh
   cp content/target/horizon-mod/kinetics.hmod ~/.local/share/horizon/horizon-mods/kinetics.hmod
   cp native/target/kinetics-0.1.0.hnmod ~/.local/share/horizon/horizon-native-mods/kinetics-0.1.0.hnmod
   ```
   Other systems (**UNTESTED**, paths taken from the launcher/tool code, not from running it; same file names, same two subfolders):

   | OS | Game folder |
   |---|---|
   | Windows | `%APPDATA%\horizon\` (copy to `horizon-mods\` and `horizon-native-mods\`) |
   | macOS | `~/Library/Application Support/horizon/` |

   Building the files on Windows/macOS is also untested; use `python` instead of `python3` on Windows if needed. If your game folder is elsewhere, `HORIZON_HOME` overrides it for `tools/create_look.py`.
3. In the launcher, press **Build** on the Mods tab (native mods are always on). Native mods run with full access to your computer, so read the source first.
4. Start the game. Kinetics has its own creative tab.

## Create-style look (optional resource pack)

By default Kinetics uses plain vanilla textures. Create's models and textures are All Rights Reserved, so they are not in this repo or the mod. To get the Create look **for your own game**, build it from your own copy of Create:

1. Download the **Create 1.21.1 NeoForge** jar from [Modrinth](https://modrinth.com/mod/create/versions?g=1.21.1&l=neoforge).
2. Run (Python 3):
   ```sh
   python3 tools/create_look.py ~/Downloads/create-1.21.1-<version>.jar
   ```
   Options: `--out` (zip path), `--mod-out` (hmod path), `--content` (content folder).
3. It writes two files into the game folder:
   - `~/.local/share/horizon/resourcepacks/kinetics-create-look.zip`: Create models for blocks in the world.
   - `~/.local/share/horizon/kinetics-create-look.hmod`: a personal copy of the mod that also fixes inventory and creative-tab icons.
4. Use it:
   - World look only: enable the zip in **Resource Packs**.
   - World and icons: move `~/.local/share/horizon/kinetics-create-look.hmod` to `~/.local/share/horizon/horizon-mods/` and delete `kinetics.hmod` there (only one mod with id `kinetics`). Keep the `.hnmod`. You don't need the zip then.
5. To go back: disable the pack and restore the normal `kinetics.hmod`.

**Never upload, release or commit the generated files**: they contain Create's assets.

## Development

```sh
cd native && cargo test   # rotation logic and rules
```

| Path | What |
|---|---|
| `content/` | Content mod: blocks, models, items, loot, recipes |
| `native/mixins/kinetics/` | Native bundle: block entities, network solver (`runtime.rs`), rendering |
| `native/tests/` | Tests for the rules |
| `tools/create_look.py` | Local Create-look builder |

## License

Code: MIT (`LICENSE`). Rotation rules are ported from Create (MIT, Copyright (c) The Create Team / The Creators of Create); the notice is in `native/mixins/kinetics/runtime.rs`. No Create assets are included.
