# HANDOFF

Living state for whoever picks this up next, including a future version of me
after a context reset. Update it when the situation changes, not at the end.

**Read this first, then `git log --oneline -12`, then the task list.**

---

## What this is

HOLDFAST: a keyboard-only 3D survival command game in Rust on **Bevy 0.19**,
targeting itch.io (WASM) plus iOS and Android. Zero non-Bevy dependencies -
the PRNG, the meshes and the audio are all generated in-crate on purpose, so
that the wasm build needs no JS shims and there are no asset files to load.

`DESIGN.md` has the pillars. `README.md` is the public face.

## Environment gotchas

- **`cargo` is not on `PATH` in fresh shells.** Prefix every command with
  `export PATH="$HOME/.cargo/bin:$PATH"`.
- **The shell is zsh.** `${PIPESTATUS[0]}` is empty; use `$pipestatus[1]` or
  `setopt pipefail`. `cmd | tail` hides a non-zero exit - check it explicitly.
  This has caused a build failure to be reported as a success once already.
- **Iterate in debug.** The user watches the window while work happens and has
  said stutters are fine. Release builds are for shipping only.
- **The backend sometimes reports no monitors at all.** Seen for a whole
  session on macOS: `Query<&Monitor>` stays empty, so nothing can be tiled and
  the window silently never moves - which looks exactly like broken placement
  arithmetic and cost an hour of chasing the wrong thing. Pass
  `HOLDFAST_SCREEN=-520,-1440,2560,1440` (the DELL's rectangle) and placement
  works regardless. Do this by default when launching for the user.
- **Windows must open on the external monitor.** In the entity-sorted monitor
  list, index **0** is the 2560x1440 DELL at `[-520, -1440]`; index 1 is the
  laptop's own 3024x1964 panel, which is also the *primary*. So pass
  `HOLDFAST_MONITOR=0`, and never trust query order without the sort - the
  `PrimaryMonitor` component puts the laptop in a different archetype and
  reverses them.

## Verifying without a human at the keyboard

Two harnesses, both inert unless their env vars are set.

**`src/devtools.rs`** - scripted one-shots.
`HOLDFAST_ARENA`, `HOLDFAST_AUTOSTART`, `HOLDFAST_SHOT=out.png@12`, `HOLDFAST_EXIT=20`,
`HOLDFAST_SPEED=4`, `HOLDFAST_UNLOCK=1`, `HOLDFAST_AUTOPICK=1`, `HOLDFAST_MONITOR=0`,
`HOLDFAST_MONITOR_NAME=DELL`, `HOLDFAST_TILE=0:2`, `HOLDFAST_RES=960x600`.

**`src/pilot.rs`** - a live command channel, so an agent can actually play.
Set `HOLDFAST_PILOT=<dir>` and the game polls `<dir>/commands`, injects the keys
into `ButtonInput<KeyCode>`, rewrites `<dir>/state.json` five times a second
and appends events to `<dir>/log.txt`. Drive it with `tools/pilot.py`:

    python3 tools/pilot.py see  <dir>
    python3 tools/pilot.py do   <dir> "hold W 1.2" "tap SPACE"
    python3 tools/pilot.py shot <dir> out.png
    python3 tools/pilot.py keys

Launch two side by side on the external monitor:

    S=-520,-1440,2560,1440
    HOLDFAST_SCREEN=$S HOLDFAST_PILOT=$PT/a HOLDFAST_TILE=0:2 ./target/debug/holdfast &
    HOLDFAST_SCREEN=$S HOLDFAST_PILOT=$PT/b HOLDFAST_TILE=1:2 ./target/debug/holdfast &

Screenshots occasionally come back solid black at exactly 56997 bytes. That is
a capture race, not a rendering bug - retry and the same scene appears.

## The playtest loop

Two agent definitions in `.claude/agents/`:

- **`playtester`** (fast, cheap model) drives one live instance through the
  pilot bridge in a given persona and leaves everything it saw in its instance
  directory: `FINDINGS.md`, `log.txt`, `stdout.log`, screenshots.
- **`playtest-evaluator`** (heavy model, no hurry) reads *all* of that
  afterwards, verifies each claim against the source, de-duplicates across
  testers, and returns one ranked brief.

Do not read a pile of raw findings yourself - that is what the evaluator is
for. Give each playtester its own instance directory, its own window slot, and
a distinct persona; a cautious first-timer and a min-maxer break different
things. Cheap models confabulate, so the evaluator's verification pass is
load-bearing, not ceremony.

## Git

**One branch. Linear history. No merge commits.** The repo is configured for
it - `merge.ff = only`, `pull.rebase = true`, and a `pre-merge-commit` hook in
`.githooks` that refuses outright, because an explicit `--no-ff` overrides
config but not a hook. If you do use a branch, land it with:

    git rebase main <branch> && git checkout main && git merge --ff-only <branch>

`core.hooksPath` is set to `.githooks`, so the hook is version-controlled and
survives a fresh clone as soon as that config is set again.

## Definition of done for any change

    export PATH="$HOME/.cargo/bin:$PATH"
    cargo fmt
    cargo clippy --all-targets -- -D warnings   # must print nothing
    cargo test                                  # 378 tests today

Lints are deliberately brutal: `pedantic` + `nursery` + `cargo` + `style`,
`unsafe_code = "forbid"`. Every `allow` in `Cargo.toml` carries a written
reason. Do not add one without a reason, and do not silence a lint that is
pointing at a real problem.

## Where things stand

Done: the core loop, five worlds, ten weapons, gear, the research tree, allies,
territory, turrets, the threat dial and wave cycle, onboarding, procedural
audio and FX, the third-person overlook camera, both test harnesses.

Also done since: infinite chunked worlds, fog of war, forts and nests and
seeders, four monster factions with regional territory and player-incited
wars, save and resume, the web/iOS/Android builds, achievements and lifetime
stats, and the optional LLM tactician.

What is left:

1. **Touch controls.** iOS and Android build, but a keyboard-only game does not
   survive a touchscreen. See `mobile/README.md` - the input layer is already
   shaped for it, the design is not done.
2. **Wrapper projects.** No Xcode project and no Gradle project exist, so
   neither mobile build has ever run on a device.
3. **The native model bridges.** `holdfast_set_model_bridge` is written and
   tested; the Swift and Kotlin sides that would call it are not.
4. **A longer balance pass.** The level-linked difficulty curve is new and has
   only been checked over short runs.

## Forts, spawners and the faction war (the spec, in full)

Stated by the user across two conversations. All of it is in scope.

- **Enemy forts are scattered through the world.** A fort is a standing
  structure with health and a garrison, not a spawn point.
- A fort periodically **sends out assaults** at the player when they are near
  enough to be worth attacking.
- A fort also sends out a **seeder**: a special monster that travels some
  distance away and then **transforms into a spawner**. This is how enemy
  territory grows.
- **Spawners trickle out monsters continuously.** They come from two places:
  seeded into the world at generation time, and planted by seeders.
- **Forts can be captured by the player *and by their allies*.** Allies must be
  able to contribute to a capture and finish one on their own - the user called
  this out specifically.
- A **captured fort works identically for its new owner**: friendly assaults,
  friendly seeders, friendly spawners. The other side then tries to retake it,
  so ground changes hands repeatedly.
- The enemy AI is a **faction director**, not per-monster wandering. Monsters
  choose between focusing the player, retaking a lost fort, defending a
  threatened one, and splitting to hit several objectives at once - and they
  **coordinate**. Massing on a fort they can actually take should beat
  harassing a player they cannot kill, and the reverse when the player is soft.

Design seam: forts and spawners are created from messages carrying a position
and an owner, so *world generation decides placement*. That keeps the whole
system independent of whether the world is today's fixed arena or the infinite
chunked one, and means it does not have to be built twice.

## Monster factions

Not one undifferentiated horde. Several **factions**, each holding **regions**
of the map, assigned deterministically from the world seed so the same rule
serves both the fixed arenas and the infinite world.

- Monster factions are **neutral to one another** by default, and hostile only
  to the player.
- **Late-stage skill-tree nodes let the player set two factions at war** for a
  period, after which they revert to neutral. This is the payoff for reading
  the map: turn the neighbours on each other and walk through the middle.
- Each faction has its own colour, name and **temperament**, which changes how
  its forts and its war director behave - how aggressively it expands with
  seeders, how heavily it garrisons, how readily it masses on a fort versus
  hunting the player.

Forts, spawners and seeders are therefore owned by a *faction*, not by "the
enemy". `Faction::Player` is one of them, which is what makes a captured fort
work identically for its new owner without a second code path.

## The `infinite-world` branch

`git checkout infinite-world` - one WIP commit, does not compile into the app
because nothing is wired into `lib.rs` yet.

- `src/world.rs`: `CHUNK_SIZE=24`, `STREAM_RADIUS=3`, `UNLOAD_RADIUS=5`,
  `WorldSeed`, `ChunkManager`, `LightPools`, `Chasms`, `chunk_rng` keyed on
  `(seed, IVec2)` so a chunk regenerates identically forever.
- `src/fog.rs`: `FOG_CELL=3.0`, `SIGHT_RADIUS=21.0`, `FogMap` with explored and
  visible sets, `FogOccluded { require_sight }`, one regenerated overlay mesh.

What remains, and why it was parked: `environments/mod.rs` still exposes the
fixed-arena `SceneData`. It needs to become `ChunkContent { props, lights,
hazards, zones, light_pools, chasms, forts, spawners }` plus
`EnvKind::generate_chunk(coord, rng)` and `EnvKind::chunk_floor(coord, seed)`,
and each of the five world modules needs a chunk generator built from its
existing prop constructors (already made `pub(super)` for this). Then
`enemy.rs` spawns relative to the player rather than the arena, `player.rs`
loses the bounds clamp and gains chasm handling, `combat.rs`'s `EnemyGrid`
centres on the player, `weapons.rs` and `allies.rs` swap `Spotlight` for
`LightPools`, and the environment tests get rewritten.

## Things learned the hard way

- `spawn_enemy` once forgot to insert `Actor`, so enemies rendered and never
  moved. The user spotted it before the tests did. Movement lives in
  `integrate_actors`; anything that should move needs that component.
- `GameSet::Reap` runs strictly last and is the *only* place entities despawn.
  Everything else marks `Doomed` with `try_insert`. Despawning mid-frame
  panics the moment two systems condemn the same entity.
- Bevy panics B0001/B0002 mean two conflicting queries or a `Res` and `ResMut`
  of the same type in one system. Fix with `ParamSet` or a single `ResMut`.
- Tests that drive time must **not** add `TimePlugin` - it overwrites manual
  clock advances from the wall clock. Use `init_resource::<Time>()` and
  `advance_by`.
- `MeshBuilder` collides with a Bevy prelude trait; ours is `MeshWeld`.
- BSD `sed` has no `\b`. Use `perl -pi -e`.
