# Dead-code audit

A systematic sweep for one bug class: **state that is written but never read, and
enum variants or config that exist but are never wired up.** Source reading only;
the game was never launched.

Audited at `df36ee7` plus the uncommitted working tree, 2026-08-03.
`src/pilot.rs` changed under the audit mid-way (the `Population` entity-count
block landed); findings below are against the tree as of the second pass.

## How this was found, and why the toolchain misses it

Two independent passes, deliberately overlapping.

**1. A compiler-verified pass.** The crate was copied to a scratch directory,
`lib.rs` was concatenated into `main.rs` so it became the crate root, every
`pub` was rewritten to `pub(crate)`, and it was built as a **bin-only crate with
no test targets**. Under those conditions rustc's own `dead_code` lint reports
every never-read field, never-constructed variant, and never-called function in
*production* code. It found **47 items**. That is the entire reason this class
survives `cargo clippy --all-targets -- -D warnings`: in a lib crate `pub`
exempts an item from `dead_code`, and `--all-targets` counts a use from a
`#[cfg(test)]` module as a use.

**2. A grep pass over every struct field.** For each of ~700 fields, every
`.field` occurrence was classified as write (followed by `=`, `+=`, …) or read.
This pass is not redundant: rustc's `dead_code` **does not** flag a field whose
only write goes through a smart-pointer deref, which in Bevy means almost every
gameplay write. `ResMut<T>` and `Mut<T>` writes hide the field from the lint
entirely — that is how `Intent.dash`, `ChunkManager.generated`,
`SaveSlot.last_note` and `WindowPlaced.asked` are invisible to pass 1.

Plus targeted sweeps: enum-variant site counts, `ALL` array completeness,
components never named in a query, resources never taken as a system param,
message types with no `MessageReader`, `HOLDFAST_*` variables against
`DevConfig` field reads, and the coverage checklist against its producers.

Reproduce pass 1 with `tools/deadcheck.sh` if you decide to keep it — the
scratch crate is at
`/private/tmp/claude-501/.../scratchpad/deadcheck` for this session only.

## Status of the seven calibration instances

| # | Item | Status |
|---|---|---|
| 1 | `DeathEvent::by_player` | **Fixed** — now `credited`, read in `pickups.rs`; the story is written into the doc comment at `common.rs:202`. |
| 2 | `Fort::pulse` | **Fixed** — read at `forts.rs:1757`. |
| 3 | `RankAura` | **STILL DEAD.** See finding 12. The halo was added beside it; the component was never removed or wired. |
| 4 | `HazardKind::Shock` | **Fixed** — placed by `grid.rs:147`, stun wired at `combat.rs:786`, coverage writer at `player.rs:395`. |
| 5 | `Progression::skill_points` | **Fixed** — spent at `screens.rs:688`. |
| 6 | `RunClock::best_streak` | **Fixed** — assigned in `note_kill` at `threat.rs:209`. |
| 7 | `Glow::Friend` | **Fixed for `Friend`. The class is not fixed.** See finding 0 — the guard test that the fix installed does not test what its name says, and five more registries have the identical shape. |

---

# Finding 0 — the guard against #7 is a tautology, and five registries share the shape

**Severity: highest. This is not a dead-field bug; it is the reason one will
recur.** Confidence: confirmed by reading the test and every registry.

`src/art.rs:396`, `every_glow_variant_is_in_all`, is cited at `art.rs:57` as the
thing that "makes the omission impossible":

```rust
let mut seen = std::collections::HashSet::new();
for g in Glow::ALL {
    assert!(seen.insert(g), "{g:?} listed twice in Glow::ALL");
}
assert_eq!(seen.len(), Glow::ALL.len(), "Glow::ALL has duplicates, ...");
```

It iterates `ALL`, and then asserts a property of `ALL` against itself. **The
enum's variant set is never consulted.** Add a 19th `Glow` variant and forget
`ALL`: the array is still `[Self; 18]` with 18 valid entries, so it compiles;
this test still passes; and `art.glow(new_variant)` still hits
`.expect("glow material registered at startup")` at `art.rs:173`, which takes
the schedule down and silently disables recruiting and building — bug #7,
verbatim. `every_glow_has_a_colour` (`art.rs:413`) also walks `ALL` and proves
only that `ALL` is walkable.

Nor is `[Self; 18]` a compile-time guard: appending a variant does not change
the array's length or contents. What *does* fire is the exhaustive `match` in
`Glow::spec()` (`art.rs:84`) — the compiler makes you give the new variant a
colour, and then lets you forget the list. That is exactly the sequence that
produced #7.

The same construction — a hand-written `ALL`, a registry built by iterating it,
a lookup by discriminant or key — appears in **six** places, five of which
panic:

| Registry | Filled from | Looked up by | On a missing variant |
|---|---|---|---|
| `GameArt.glows` (`HashMap`) | `Glow::ALL` — `art.rs:243` | `art.rs:173` | **panic** `expect` |
| `GameArt.enemies` (`Vec`) | `EnemyKind::ALL` — `art.rs:264` | `art.rs:178` `[kind as usize]` | **panic** index |
| `GameArt.allies` (`Vec`) | `AllyKind::ALL` — `art.rs:269` | index by discriminant | **panic** index |
| `GameArt.turrets` (`Vec`) | `TurretKind::ALL` — `art.rs:273` | index by discriminant | **panic** index |
| `SoundBank.clips` (`Vec`) | `Sfx::ALL` — `audio.rs:451` | `audio.rs:103` `[sfx as usize]` | **panic** index |
| `GameArt.banners` | `Faction::ALL` — `art.rs:333` | `art.rs:161` | safe, `unwrap_or_else` |

All eleven `ALL` arrays are complete **today** (verified mechanically:
`AllyKind` 4/4, `TurretKind` 5/5, `Glow` 18/18, `Sfx` 27/27, `EnemyKind` 12/12,
`EnvKind` 5/5, `Faction` 5/5, `StatBoost` 18/18, `Branch` 4/4, `GearSlot` 3/3,
`WeaponKind` 10/10). The exposure is entirely future.

The existing order checks — `EnvKind` at `environments/mod.rs:640`, `Faction` at
`factions.rs:691` — catch a variant *inserted in the middle* (discriminants
shift) but not one *appended*, which is the natural thing to do.

**How to make it impossible** (no new dependency, in keeping with the
zero-dependency rule): tie `ALL` to an exhaustive `match`, so appending a
variant is a **compile error**, not a test failure.

```rust
impl Glow {
    /// Position in `ALL`. Exhaustive on purpose: a new variant cannot compile
    /// until it is given a slot, and `all_is_complete` then refuses to pass
    /// until `ALL` has a slot for it to occupy.
    const fn slot(self) -> usize {
        match self {
            Self::Xp => 0,
            // ...
            Self::Warning => 17,
        }
    }
}

#[test]
fn all_is_complete() {
    for (i, g) in Glow::ALL.iter().enumerate() {
        assert_eq!(g.slot(), i, "{g:?} is in the wrong slot");
    }
    // Every slot is claimed, so no variant is missing from ALL.
    let mut claimed = vec![false; Glow::ALL.len()];
    for g in Glow::ALL { claimed[g.slot()] = true; }
    assert!(claimed.iter().all(|c| *c));
}
```

Add a variant → `slot()` is non-exhaustive → build fails → you assign an index →
the index is out of range of `ALL` → the test fails until `ALL` grows. Worth
doing for `Glow`, `Sfx`, `EnemyKind`, `AllyKind` and `TurretKind`, i.e. the five
that panic. The alternative shape — a `const fn after(self) -> Option<Self>`
successor chain that `ALL` is built from — is equally airtight and removes the
hand-written array altogether.

`HazardKind` deserves an `ALL` of its own: `coverage::expected()` at
`coverage.rs:130` hand-writes `[Scald, Sticky, Shock, Font]`, the only list in
that function not derived from an enum's `ALL`. Adding a fifth hazard silently
drops it from the coverage checklist.

---

# Tier 1 — player-visible mechanical consequence

## 1. The Highlighter's mastery does nothing

- **Written:** `src/weapons.rs:483` — `shot.burn = if mastered { dmg * 0.3 } else { 0.0 };`
- **Carried:** `src/combat.rs:564` — `burn: s.burn` into `Projectile.burn` (`combat.rs:298`)
- **Should read it:** nothing does. `Projectile.burn` has **zero** read sites
  (compiler-confirmed: `combat.rs:297 fields slow and burn are never read`).
  `StatusEffects::apply_burn` (`enemy.rs:328`) is never called
  (compiler-confirmed). The burn damage tick at `enemy.rs:1090-1103` is complete
  and correct, and never receives a `burn_dps`.
- **Consequence:** the level-8 payoff advertised on the upgrade card as
  *"MASTERY: the beam burns everything it crosses"* (`weapons.rs:205`) is a
  no-op. A player who takes the Highlighter to mastery gets nothing for it.
  Every other mastery works — `CoffeeNova`'s at `weapons.rs:512` spawns a real
  `SpawnHazard`, which is what the Highlighter should probably do too.
- **Confidence: confirmed.** Every use site of `Projectile.burn`,
  `SpawnShot.burn` and `apply_burn` read.
- **Also dead in the same subsystem, lower stakes:** `Projectile.slow` /
  `SpawnShot.slow`. No weapon ever sets `slow` to anything but the `0.0` default
  (`combat.rs:345`), so nothing is lost — but the field pair invites the same
  mistake again. `Hazard.slow` is a separate, working field
  (`combat.rs:783`, `player.rs:409`).

## 2. Four coverage checklist items have no producer — third recurrence

- **Written:** `src/coverage.rs:126-129` — `expected()` emits
  `faction:{tag}` for all four `Faction::MONSTERS`.
- **Should write the tag:** nothing does. There is **no `Seen` message anywhere
  whose tag begins with `faction:`**. Every other category has a producer:
  `world:` `coverage.rs:241`, `weapon:` `weapons.rs:338`, `enemy:`
  `pickups.rs:129`, `turret:` `allies.rs:641`, `ally:` `allies.rs:583`,
  `hazard:` `player.rs:395`, and all nineteen `deed:` tags.
- **Consequence:** the checklist has 63 entries (5+10+12+5+4+**4**+4+19). Four
  can never be marked, so the coverage sweep is **permanently capped at 59/63 =
  93.7%**, and `missing()` permanently reports four faction tags that no
  playtester can ever clear. This is load-bearing on your playtest loop: the
  coverage readout is what turns "go and see everything" into a task with an
  answer, and it is lying about the last 6%.
- **This is the third instance of the identical bug**, and the code says so.
  `player.rs:391` — *"All four hazard items were on the coverage checklist with
  no writer anywhere, so the sweep could never pass 94%."* `coverage.rs:246` —
  *"Ten of the nineteen deeds had no writer at all and coverage could never pass
  41%."* Both were fixed; the faction row was never noticed.
- **The guard that should have caught it points the wrong way.**
  `the_checklist_covers_every_enum_the_game_has` (`coverage.rs:287`) asserts the
  *checklist* has four faction entries (`coverage.rs:310-313`). It never asks
  whether anything can mark them.
- **Confidence: confirmed.** Every `Seen` write site in the crate enumerated.
- **Make it impossible:** route emission through typed constructors
  (`Seen::faction(f)`, `Seen::deed(Deed::Dash)`) and derive `expected()` from the
  same source, so a checklist entry without a producer cannot be spelled. Short
  of that, a test that scans the source for each tag prefix — this crate already
  generates all its content in-crate, so a source-reading test is in keeping.
- Natural writer: `factions.rs` already knows when the player first meets a
  faction (`NearbyPowers`, `faction_at`); a `Seen::of("faction", f.tag())` on
  first contact or first kill is the obvious reading of "exercised".

## 3. `Sfx::WaveStart` is synthesized and never played

- **Written:** declared `audio.rs:44`, listed in `ALL` `audio.rs:75`,
  gain-balanced at 0.4 `audio.rs:88`, and given a full synth voice at
  `audio.rs:441`.
- **Should play it:** nothing. It is the **only one of 27 `Sfx` variants with no
  emission site outside `audio.rs`** (all 26 others were checked individually).
  `tick_waves` (`threat.rs:371-405`) raises no `SfxEvent` at all — it sets
  `cycle.announce = 2.5` for the visual banner and nothing else.
- **Consequence:** the assault beginning — the loudest recurring beat in the
  game loop, and the one the player must react to — has a banner and no sound.
  The clip exists and is mixed; it just never fires.
- **Confidence: confirmed.**

## 4. Ally and structure descriptions are never shown

- **Written:** `AllyKind::blurb()` `allies.rs:132` (4 lines), `TurretKind::blurb()`
  `allies.rs:312` (5 lines), `recruit_hint()` `progress.rs:1131`.
- **Should read them:** the recruit panel and the build menu. Neither is called
  anywhere in production (compiler-confirmed ×3). `WeaponKind::blurb` *is* used
  (`progress.rs:893`, upgrade cards) — allies and turrets are the odd ones out.
  `recruit_hint`'s own doc says *"Recruit costs shown on the squad panel."*
- **Consequence:** the player is never told what a structure does. The build
  menu shows names and Scrap costs (`hud.rs:777`, `command.rs:244`) and nothing
  else, so the three non-obvious ones are undiscoverable except by buying them:
  *"Shocker — No damage. Slows everything in a wide radius."* / *"Barricade — No
  gun. Reshapes where the enemy can walk."* / *"Generator — Pays Scrap every
  second. Fragile. Guard it."* Same for choosing between Scout, Gunner, Bulwark
  and Medic.
- **Confidence: confirmed.**
- **Note on false confidence:** `allies.rs:1215` and `allies.rs:1298` assert
  `!kind.blurb().is_empty()` for every variant. The tests keep dead content
  alive and read like coverage. They are why the compiler's `--all-targets`
  build stays quiet, and why pass 1 had to exclude test targets.

## 5. Per-world enemy tinting is never applied

- **Written:** `src/enemy.rs:813` — `commands.entity(id).insert(EnvTint(kind.tint(env)));`
  with the comment *"Tint marker consumed by the presentation pass."*
- **Should read it:** a presentation pass. **`EnvTint` is never named in any
  query, filter or system param** (compiler-confirmed: `enemy.rs:846 field 0 is
  never read`; my ECS sweep: `queries=0`). `EnemyKind::tint(env)`
  (`enemy.rs:234`) is called from this one line and nowhere else, so the whole
  12-kind × 5-world tint table is dead data.
- **Consequence:** every enemy in every world renders with the same
  `art.solid` material (`enemy.rs:787`). The module's opening premise —
  `enemy.rs:4`, *"The same twelve archetypes appear in every environment,
  renamed and retinted"* — is half-implemented. Renamed yes (`EnemyKind::name`
  is wired). Retinted no. A Dust Bunny and a Thorn Sprite are visually
  identical.
- **Confidence: confirmed.**

## 6. `fx::clear_fx` is never registered, so the FX budget leaks across runs

- **Written:** `src/fx.rs:244-258`, documented *"Clear every transient effect,
  used when a run ends."* It resets `FxCounts.particles` and `.floaters` to zero.
- **Should call it:** `FxPlugin::build` (`fx.rs:51-61`) does not, and nothing
  else does (compiler-confirmed). `FxCounts` is never reset anywhere — its only
  decrements are in `tick_particles:132` and `tick_floaters:215`, which fire
  only when a particle's own life expires.
- **Consequence:** particles and floaters carry `RunEntity`, so they are
  despawned wholesale by `start_run` (`screens.rs:385`) and by `gameover_input`
  (`screens.rs:965`) — while `FxCounts` keeps counting them. Every entity alive
  at the instant a run is cleared leaks one count, permanently, for the process.
  Once `counts.floaters` reaches `MAX_FLOATERS` (44) damage numbers stop
  appearing; at `MAX_PARTICLES` (420) all particle effects stop. The ceilings are
  low and `gameover_input` is a **single Enter keypress** for "restart in place",
  well inside the 0.85 s life of a death burst.
- **Confidence: confirmed mechanism; the rate depends on timing.** A human who
  reads the results screen for a second leaks little, because
  `tick_particles`/`tick_floaters` run unconditionally in `PostUpdate` and drain
  naturally. An agent driving the bridge through many fast restarts in one
  process would silently lose all feedback — worth knowing before the next
  "FX look broken" report, which would be the instrument again.

## 7. No "new weapon" banner

- **Written:** nothing. `Unlocks.seen_weapons: HashSet<u8>` (`onboarding.rs:45`)
  has **zero writes, zero reads and zero initialisers** other than
  `Default::default()`.
- **Should use it:** the sibling field `seen_enemies` (`onboarding.rs:44`) is
  documented *"Kinds already introduced, so each gets exactly one 'new threat'
  banner"* and is fully wired. `seen_weapons` is its unimplemented twin.
- **Consequence:** a new monster kind gets a one-time introduction banner; a new
  weapon does not. The asymmetry is invisible in code and obvious in play.
- **Confidence: confirmed** (compiler + grep, both directions).

## 8. Five named world hazards are never announced, and the gust period is ignored

- **Written:** `Gust.label` — `"USB FAN"`, `"CANOPY WIND"`, `"DOWNDRAFT"`,
  `"GRAV SHEAR"`, `"MANA SURGE"` at `desk.rs:41`, `forest.rs:54`,
  `rooftop.rs:52`, `grid.rs:51`, `arcane.rs:53`.
- **Should read it:** the HUD. Nothing does in production (compiler-confirmed;
  one test reads it). There is no gust indicator or warning of any kind.
- **Consequence:** each world's signature environmental hazard has a
  world-specific name that no player ever sees, and no telegraph.
- **`Gust.interval` is worse than dead — it is silently overridden.** Written in
  all five worlds and never read; `tick_gust` (`environments/mod.rs:571-586`)
  drives the cycle from `duration`/`cooldown`. Every world sets the two to
  *different* values: 12/10, 15/12, 13/11, 10/9, 14/12. The intended period is
  ignored and each gust cycles 10–20% faster than the world says it should.
- **Confidence: confirmed.**

## 9. `Objective` knows which fort it is for, and nothing reads it

- **Written:** `forts.rs:1557-1562` sets `kind: TakeFort | DefendFort` and
  `fort: plan.focus`; `forts.rs:1567`, `:1577`, `:1627` set `HuntPlayer`.
- **`Objective.fort` has zero read sites** (compiler-confirmed:
  `forts.rs:315 field fort is never read`).
- **`Objective.kind` has exactly one read**, `forts.rs:1540`, and only to decide
  whether to refresh `pos` toward the player.
- **The consumer reads only `pos`:** `enemy.rs:878` —
  `let target = objective.map_or(player_pos, |o| o.pos);`
- **Consequence:** `TakeFort` and `DefendFort` are behaviourally identical — a
  monster sent to defend its own fort behaves exactly like one sent to storm
  yours: both walk to a point. Nothing garrisons, holds a ring, or prefers
  targets differently. And because `fort` is unread, a monster committed to a
  fort keeps walking to a stale `focus_pos` for the rest of its `review` window
  (up to 8 s) after the fort is destroyed or changes hands. The struct's own
  doc — *"a faction can besiege a fort it wants while ignoring a player it cannot
  catch"* — is delivered entirely by `pos`; `kind` and `fort` are decoration.
- **Confidence: confirmed.**
- Related, minor: `Posture::Split` (`forts.rs:627`) is set by `decide()` and no
  production code branches on it — the only comparison is a test at
  `forts.rs:2321`. Its distinctness is carried by `commitment: 0.45`, which *is*
  read, so nothing is broken. Reads as a tag with no reader.

---

# Tier 2 — dead plumbing, no current player-visible effect

## 10. `Earned` is a registered message with no reader

- `stats.rs:436` declares it, `stats.rs:448` registers it with `add_message`,
  `stats.rs:485` writes it. **No `MessageReader<Earned>` exists anywhere** — it
  is the only one of the 18 registered message types with no reader (all 18
  checked). Compiler-confirmed independently: `stats.rs:436 field 0 is never read`.
- No consequence today: `check_achievements` pushes a `HintQueue` banner two
  lines later (`stats.rs:487`), which is the player-visible feedback. `Earned`
  is a second channel nobody listens on.
- **Confidence: confirmed.**

## 11. The helper written to stop hazard spawning from drifting is unused, and it has drifted

- `environments/mod.rs:529`, `spawn_hazard_entity`, whose doc reads: *"Shared
  with `world`, which streams them in per chunk, **so the two cannot drift
  apart** in how a hazard is assembled."*
- **It is never called** (compiler-confirmed), and `world.rs:440-475` is a
  hand-copied duplicate that is the one actually used.
- **They have already drifted.** `world.rs` adds `ChunkEntity(coord)` and
  `FogOccluded::default()` and derives the pulse phase from the chunk
  coordinate; `mod.rs` adds neither component and takes `phase` as an argument.
  A third copy exists at `combat.rs:580-615` for `SpawnHazard`, with its own
  copy of the four-arm `HazardKind → Glow` match (`combat.rs:586`,
  `world.rs:441`, `mod.rs:535` — three copies).
- No consequence today; the live copy is the more complete one. The comment is
  actively false, and any future hazard change made in the "shared" helper will
  do nothing.
- **Confidence: confirmed.**

## 12. `RankAura` — calibration item #3, still dead

- Inserted at `enemy.rs:795` (elites, `pal::ELITE_TRIM`) and `enemy.rs:801`
  (bosses, `pal::BOSS_TRIM`). **Never queried by anything**
  (compiler: `enemy.rs:842 field color is never read`; ECS sweep: `queries=0`).
- The visual it was for now exists independently: a glowing halo child spawned
  at `enemy.rs:816-834` using `Glow::Elite` / `Glow::Boss`. `RankAura` looks like
  what was left behind when the halo replaced it.
- No consequence — delete it. Worth flagging only because it is on your own
  list of seven and reads as fixed.
- **Confidence: confirmed.**

## 13. Three lifetime-stat keys are declared and never recorded; five more are recorded and never surfaced

Cross-checked every `stat::` constant against every write and read site.

**Declared, never recorded, and no reader** (compiler-confirmed unused):
- `stat::CORES` `stats.rs:45` — no reference anywhere in the crate.
- `stat::ZONES_HELD` `stats.rs:46` — no reference anywhere in the crate.
- `stat::SCRAP` `stats.rs:44` — referenced only from tests (`stats.rs:661,665`).

**Recorded, saved to disk, and surfaced nowhere** — no achievement uses them and
the only lifetime display (`screens.rs:912-914`) shows runs, kills and best time:
- `stat::TOTAL_TIME` — written `stats.rs:524` only
- `stat::LEVELS` — written `stats.rs:527` only
- `stat::ELITES` — written `pickups.rs:148`; read only from a test
- `stat::FORTS_LOST` — written `forts.rs:962` only
- `stat::SURGES` — written `command.rs:309` only

**Also:** `faction_kill_key()` (`stats.rs:608`) builds a `killed_{tag}` ledger
key and is called only from a test (`stats.rs:820`). Per-faction kill counts are
never recorded. `Ledger::iter()` (`stats.rs:85`) — the only way to enumerate the
ledger — is never called, so nothing could display them anyway.

Good news from the same cross-check: **every achievement's key has a writer.**
No achievement is unearnable.

- **Confidence: confirmed** for each key individually.

## 14. The player's name is never shown

- `Identity::display_name()` (`stats.rs:352`) is implemented, tested
  (`stats.rs:799`), and never called in production (compiler-confirmed).
- `provider()` — documented *"for the profile screen"* — is called only from a
  `Debug` impl (`stats.rs:395`). There is no profile screen.
- The `Identity` trait's whole presentation surface is unwired; only `report()`
  is used (`stats.rs:484`).
- **Confidence: confirmed.**

## 15. Registered materials and enum variants nothing uses

- **`Glow::Screen`** (`art.rs:66`, colour `art.rs:88`) and **`Glow::Neon`**
  (`art.rs:68`, colour `art.rs:90`) — in `ALL`, so a material is built for each
  at startup, and applied to nothing. Only two of the 18 are in this state.
  (The `palette::SCREEN_GLOW` *colour* is used widely; it is the emissive
  material that is dead.)
- **`Surface::Glow(Glow)`** (`environments/mod.rs:199`) — **never constructed**
  (compiler-confirmed). `world.rs:396` handles it and no world ever asks for it;
  the other four `Surface` variants are all used.
- **`GameArt` handles built and never used** (compiler-confirmed,
  `art.rs:113/133/144/147/154/155/156`): `unlit` (material), `mine`, `shard`,
  `shadow`, `arrow`, `unit_cube`, `unit_sphere` (meshes). `shadow` is a
  contact-shadow disc (`art.rs:324`) that nothing places; `arrow` is a direction
  indicator nothing draws.
- **Consequence:** none, cosmetic/dead. A little startup work and a little
  memory.
- **Confidence: confirmed.**

## 16. `ArenaBounds` — a whole resource that only its tests still use

- `arena.rs:15`. **Never constructed outside `#[cfg(test)]`, never registered as
  a resource, never taken as a system param** (compiler + ECS sweep agree).
- Its five methods — `clamp`, `contains`, `edge_distance`, `perimeter_point`,
  `diagonal` (`arena.rs:31-65`) — are all unused, as is
  `ColliderShape::bounding_radius` (`arena.rs:95`).
- Seven test constructions across `arena.rs:541-611` keep roughly 90 lines of
  test code alive for a concept the infinite world replaced. The module doc still
  advertises it: *"Arena mechanics shared by every environment: **bounds**, solid
  props, hazards."*
- **Consequence:** none. Notable mainly because it means some of the 461 passing
  tests are testing something the game no longer contains.
- **Confidence: confirmed.**

## 17. `Hazard.life` and five `Hazard` constructors

- `Hazard.life: Option<f32>` (`arena.rs:273`) is set at all three construction
  sites (`combat.rs:601` `Some(h.life)`, `world.rs:456` `None`,
  `mod.rs:548` `None`) and **read nowhere** (compiler-confirmed). Expiry is
  handled by `Ephemeral::new(h.life)` (`combat.rs:604`), which works.
- `Hazard::scald`, `Hazard::sticky`, `with_life`, `enemies_only`, `player_only`
  (`arena.rs:279-317`) are all unused in production; the three real construction
  sites build the struct literally. `arena.rs:638-647` tests them.
- **Consequence:** none. `life` is a redundant second copy of an already-correct
  mechanism, and the builder API's `enemies_only`/`player_only` semantics are
  reimplemented inline at each site.
- **Confidence: confirmed.**

## 18. Fields written through `ResMut`/`Mut` and never read

These are invisible to `dead_code` (see method note above); all were found by
the grep pass and each read/write site was then checked by hand.

| Field | Written | Reads | Consequence |
|---|---|---|---|
| `Intent.dash` `player.rs:113` | `player.rs:229` | **none** | The struct's doc promises input and movement are separable *"so the two can be re-sourced (touch, gamepad)"*. `move_dir` honours that; `dash` does not — the dash is triggered from the local bool at `player.rs:230`. A future touch or gamepad source that sets `Intent.dash` would silently not dash. |
| `ChunkManager.generated` `world.rs:112` | `world.rs:119`, `world.rs:321` | **none** | Doc says *"Total chunks generated this run, purely for the stats ledger."* It never reaches the ledger; there is no `stat::` key for it. |
| `SaveSlot.last_note` `save.rs:348` | `save.rs:647` | **none** | A note built with `format!` on every save and shown nowhere. `build_menu` (`screens.rs:128`) takes `Res<SaveSlot>` and reads other fields. |
| `WindowPlaced.asked` `devtools.rs:259` | `devtools.rs:401`, `:442` | **none** | Bookkeeping in the window-placement retry loop; the sibling `warned` is read at `devtools.rs:314`. Harness-only. |
| `Research.cursor` `progress.rs:351` | initialiser only | **none** | A stale duplicate of `screens.rs:29 ResearchCursor`, which is the real one. Writing `research.cursor` would silently no-op. |
| `ChunkCtx.coord` `mod.rs:313` | 8 construction sites | **none** | Generators use `min` only. |
| `LightSpec.shadows` `mod.rs:270` | `mod.rs:366`, hardcoded `false` | **none** | Doubly dead: no world can request shadows, and `world.rs:430` hardcodes `shadow_maps_enabled: false` anyway. |
| `Temperament.blurb` `factions.rs:137` | 5 sites, `factions.rs:98-122` | **none** | Five one-line faction characterisations (*"Digs in. Slow to expand, miserable to evict."*) never shown. A test at `factions.rs:653` asserts they are non-empty. |

All: **confidence confirmed**, no player-visible consequence today.

---

# Tier 3 — unused helpers and superseded APIs

Compiler-verified as never called in production. No consequence beyond noise,
but two carry doc comments that are now false.

**Doc comments that lie:**
- `EnemyGrid::best_target` (`combat.rs:194`) — *"Turrets and the laser use this"*.
  They use `best_visible_target` (`combat.rs:203`). `EnemyGrid::nearest`
  (`combat.rs:153`) is likewise superseded by `nearest_visible`.
- `weapons::friendly_damageable()` (`weapons.rs:724`) — *"Structures and allies
  both need…"*. Neither calls it; the struct is built inline at `allies.rs:565`,
  `allies.rs:657` and `player.rs:180`.

**Plain unused:**
`Conquests::count` `forts.rs:469` · `Diplomacy::war_remaining` `factions.rs:255`
(`active_wars()` supersedes it) · `RunClock::streak` `threat.rs:222` ·
`RunClock::stage` `threat.rs:234` (self-described as *"purely cosmetic"*) ·
`WaveCycle::label` `threat.rs:364` (the HUD re-implements the same
`Prep`/`Assault` match inline at `hud.rs:497`) · `Coverage::iter`
`coverage.rs:76` · `ChunkManager::loaded_count` `world.rs:128` ·
`StatusEffects::apply_burn` `enemy.rs:328` (see finding 1) ·
`fx::despawn_all` `fx.rs:261` · `fx::on_exit_menu_clear` `fx.rs:267` (an empty
function body, `pub`, never called — with `const _: Option<AppState> = None;`
below it whose comment claims to keep `AppState` in scope *"for the plugin's run
conditions"*; `FxPlugin` has no run conditions) · `common::flat` `common.rs:263`
· `common::Hover` `common.rs:135` (a component never constructed; never queried
either) · `meshgen::MeshWeld::shape` `meshgen.rs:80` · `meshgen::at_scaled`
`meshgen.rs:105` · `meshgen::boxed` `meshgen.rs:124` · `palette::with_alpha`
`palette.rs:118` · `Rng::shuffle` `rng.rs:102` · `progress::recruit_hint`
`progress.rs:1131` (see finding 4).

`ChunkManager::loaded_count` is worth wiring rather than deleting: the
`population` block just added to the pilot snapshot exists to diagnose silent
accumulation, and loaded chunks are a candidate it does not yet report.

---

# Negative results

Sweeps that came back clean, recorded so nobody repeats them:

- **`HOLDFAST_*` environment variables.** All 21 are parsed into `DevConfig` or
  read directly, and **every one of the 14 `DevConfig` fields is read and acted
  on**. Nothing documented in the `devtools.rs` table is inert.
  (`HOLDFAST_LABEL`, `pilot.rs:841`, works but is absent from that table —
  a doc gap, not dead code.)
- **All eleven `ALL` arrays are complete today.** See finding 0 for why that is
  not reassuring.
- **Every achievement key has a writer.** No achievement is unearnable.
- **17 of 18 registered message types have a reader.** Only `Earned` does not.
- **Only three components are never queried:** `EnvTint`, `RankAura`, `Hover`.
- **Only one resource is never a system param:** `ArenaBounds`.
- **No enum variant outside those named above is constructed-but-unmatched.** All
  35 enums had their variants counted by site; `ShotVisual`, `StatBoost`,
  `DamageSource`, `HintTone`, `CardKind`, `ZoneOwner`, `Rank`, `Behavior`,
  `Phase` and `EnvKind` are all fully wired.
- **All nineteen `deed:` coverage tags have producers**, and all four `hazard:`
  tags. The earlier fixes hold.

---

# Recommendations, most valuable first

1. **Fix finding 0 first.** Replace `every_glow_variant_is_in_all` with a real
   completeness check, and give the same treatment to `Sfx`, `EnemyKind`,
   `AllyKind` and `TurretKind`. Until then the bug that took the whole schedule
   down is one appended variant away in five places, and the test named after it
   cannot see it.
2. **Adopt the bin-only `pub(crate)` shadow build as a check.** It found 47
   items that `cargo clippy --all-targets -- -D warnings` cannot see, and it is
   two facts about rustc: `pub` exempts items from `dead_code` in a lib crate,
   and `--all-targets` lets a test count as a use. Two ways to have it
   permanently:
   - *A script.* `tools/deadcheck.sh` copies `src/`, concatenates `lib.rs` into
     `main.rs`, rewrites `pub` → `pub(crate)`, and runs `cargo build` with a
     shared `CARGO_TARGET_DIR`. ~2.5 min warm, no source changes.
   - *Better: make it the real build.* `tests/simulation.rs` is 258 lines and
     touches only `holdfast::threat::*`. Move it into `threat.rs` as a
     `#[cfg(test)]` module, then mark everything `pub(crate)` except `run()` and
     the two FFI entry points. Plain `cargo clippy -- -D warnings` then catches
     this entire class forever, with no extra tooling. This is the single change
     that would have prevented most of the seven.
   - Either way, **run it without `--all-targets`.** Findings 4 and 18 are alive
     only because tests use them.
3. **Invert the coverage checklist.** Make emission and `expected()` come from
   one source so a tag without a producer cannot be written down. Finding 2 is
   the third time this exact bug has shipped.
4. **A review rule the shadow build enforces:** a field added in a commit must be
   read in the same commit. That is the whole class, stated in one line, and once
   check 2 is in place the build refuses to land a violation.
5. **Decide, don't just delete.** Findings 1, 3, 4, 5, 7, 8 and 9 are unfinished
   features with the data already in place, not litter — the burn subsystem, the
   wave-start clip, the ally and structure blurbs, the per-world tint table, the
   gust names, and the attack/defend distinction are each one small system away
   from working. Findings 10–18 and Tier 3 are litter and can go.
