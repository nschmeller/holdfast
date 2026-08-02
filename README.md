# HOLDFAST

**Five small worlds. One rule: hold your ground.**

A keyboard-only 3D survival command roguelite. You are one small unit holding
ground against an escalation that never stops — and you decide how fast it
escalates.

Built in Rust with [Bevy 0.19](https://bevy.org). Ships to the web, to desktop,
and to iOS from one codebase.

---

## What makes it different

**It is a command game, not a twitch game.** Every weapon aims and fires itself.
Nothing requires a frame-perfect dodge. Press `Space` and time drops to 12% so
you can build, reposition your squad and read the board at leisure — for as long
as you like, at no cost. The game never punishes thinking, only thinking wrong.

**You own the throttle.** A single THREAT dial, `0.5` to `8.0`, drives spawn
rate and enemy power — and multiplies XP, Scrap, Cores and gear rarity
superlinearly. Push it and get rich. Back off and fall behind, because the dial's
floor rises with the clock. Turtling is a delaying action, never a strategy.

**Ground is worth something.** Capture zones pay income and project auras. Build
turrets, barricades and generators. Recruit a squad and give it stances. The
enemy sieges all of it.

**One vocabulary, five worlds.** The same twelve threat archetypes appear
everywhere, renamed and retinted, so what you learn on a desk still applies in
the Sanctum.

## The five worlds

| World | | Plays like |
| --- | --- | --- |
| **THE DESK** | 2AM, one lamp, and the stationery has opinions | Tight and cluttered; a USB fan sweeps a lane |
| **THE UNDERGROWTH** | You are four inches tall and the moss is hostile | Wide and open; mud slows everything |
| **BLOCK 9 ROOFTOP** | Neon, rust, and something in the vents | Long sightlines; steam vents on a timer |
| **GRID ZERO** | A test platform in hard vacuum | Almost no cover; your barricades *are* the terrain |
| **THE ARCANE SANCTUM** | A broken sanctum where the wards still hold | Ley lines heal whoever holds them — including the enemy |

## Controls

Everything is on the keyboard. There is no mouse input at all.

| Key | Action |
| --- | --- |
| `WASD` / arrows | Move |
| `Space` | **PLAN mode** — time to 12%, arrows drive the build cursor |
| `1`–`5` | Pick a structure (plan mode) / pick an upgrade card |
| `Enter` | Place structure / call the wave in early / confirm |
| `-` `=` | Threat dial down / up |
| `O` | Overclock — 22s surge, +2.5 threat, ×1.6 rewards |
| `B` | Build palette |
| `R` | Recruit |
| `F` | Rally squad |
| `G` | Cycle squad stance |
| `T` | Research tree |
| `Q` / `E` | Rotate camera |
| `Esc` | Pause |

## No assets

There is not a single texture, model, font file or sound file in this repository.

- **Meshes** are welded at startup from Bevy primitives with per-vertex colour,
  so one material covers nearly the whole scene and GPU instancing stays
  effective at several hundred entities.
- **Floors** are generated as per-cell quad grids — wood grain, moss, tar paper,
  hex plating and inscribed flagstones, all from a value-noise function.
- **Sound** is synthesized into WAV buffers at startup from oscillators and
  filtered noise.
- **Randomness** is a hand-rolled xoshiro256++, so there is no `getrandom`
  dependency and therefore no wasm JS shim.

The practical result is a web build that downloads only the engine and the game
logic, and an iOS build that needs no resource bundle.

## Building

```sh
cargo run --release              # desktop
./scripts/build-web.sh           # wasm bundle for itch.io, into dist/
./scripts/build-ios.sh           # iOS static libraries
```

### Development harness

The game is driven entirely by keypresses, which makes it awkward to verify
without a human. These environment variables make it scriptable:

```sh
HOLDFAST_ARENA=arcane      # start in a named world
HOLDFAST_AUTOSTART=1       # skip the menu
HOLDFAST_UNLOCK=1          # every subsystem online immediately
HOLDFAST_SPEED=4           # run the simulation at 4x
HOLDFAST_SHOT=out.png@12   # screenshot after 12 seconds
HOLDFAST_EXIT=15           # quit after 15 seconds
```

```sh
cargo run --features debug-names   # real names in ECS conflict panics
```

## Testing and lints

```sh
cargo test
cargo clippy --all-targets -- -D warnings
```

Every Clippy group except `restriction` is enabled at warn level, plus a set of
stricter rustc lints. The handful of allows in `Cargo.toml` each carry a comment
explaining which Bevy idiom they exist to accommodate.

## Layout

```
src/
  main.rs          app wiring, states, system ordering
  arena.rs         bounds, collision, hazards  — knows nothing about any world
  environments/    the five worlds, as pure data
  models.rs        every creature, projectile and pickup mesh
  meshgen.rs       primitive welding and procedural floors
  threat.rs        the pacing dial and the wave cycle
  enemy.rs         twelve archetypes and the spawn director
  weapons.rs       ten self-firing weapons
  allies.rs        squad, structures, territory
  command.rs       plan mode and every keybinding
  progress.rs      levels, cards, research, gear
  onboarding.rs    staged unlocks and contextual hints
```

## Licence

MIT OR Apache-2.0.
