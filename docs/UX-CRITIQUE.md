# HOLDFAST — graphics and UX critique

Written after a live session on 2026-08-03 against the `infinite-world` branch,
debug build, driven through the pilot bridge. Every claim below is tied to a
screenshot in `docs/ux-critique/`, to a measured pixel value, or to a line of
source. Where something is taste rather than craft it says so.

Nothing in the source was changed.

**Method.** Six instances, five worlds, sixteen screenshots kept. Contrast
figures are WCAG relative-luminance ratios computed from the PNGs: for each
entity, the median luminance of its brightest fifth of pixels against the median
luminance of the floor in an annulus around it. WCAG's floor for a meaningful
non-text graphic is 3:1.

**Two things I chased and discarded, recorded so nobody repeats them.** (1)
Several overlay screens came back as byte-identical pure black. It was not a
layout bug — it was six concurrent GPU contexts; with three instances the same
screens render fine. (2) The player appeared permanently immobilised at
`(-30.002, 571.591)` with `hold W`/`hold D`/`goto`/`defend` all producing zero
displacement. The instance had already exited on `HOLDFAST_EXIT=900`. That is
the fifth time the instrument, not the game, produced the symptom.

---

## 1. The ten worst problems, worst first

### 1. You cannot find your own character

`spawn_player` (src/player.rs:170) gives the player a mesh, a transform and
nothing else. Every other entity that matters gets a floor ring: allies and
turrets get `Glow::Friend`, elites `Glow::Elite`, bosses `Glow::Boss`, nests and
enemy zones `Glow::Warning`, zones `Glow::Zone`. The one entity whose position
the player must know at every instant is the only one with no marker.

It gets worse, because the duck's body colour is `DUCK_BODY (255,208,62)` and
the gold family is already spoken for four times over: `PlayerShot` is
`ACCENT (255,190,74)`, `Gear` is `GEAR_GOLD (250,194,84)`, the neutral zone ring
is `(0.9,0.75,0.35)`, and the HUD accent is `ACCENT`. **The player is
camouflaged against their own UI language.**

Measured: on the desk the duck reads **2.25:1** against the floor beside it
(`08-density-desk-worse.jpg`). In `15-rooftop.jpg` I could not locate the
character at all, and I was looking for it deliberately with the state file open.
In `16-far-country.jpg` it is behind a book — props are opaque, taller than the
character, and there is no occlusion handling.

**Fix.** A bright ring plus a short vertical pip under the player, in a hue
reserved for nothing else (magenta is free of gameplay meaning; `Neon` uses it
for props only). Dither or silhouette the player when a prop occludes it. Both
are small: the ring mesh and the `Glow` machinery already exist, and the ring
would be one child entity in `spawn_player`.

### 2. Hostiles have no contrast against the ground they walk on

| entity | world | contrast vs floor |
| --- | --- | --- |
| Dust Bunny | Desk | **1.25 : 1** |
| Sugar Ant | Desk | **1.29 : 1** |
| Lamp Moth | Rooftop | **1.06 : 1** |
| tan blob | Rooftop | **1.37 : 1** |
| tan blob | Grid Zero | **2.39 : 1** |

`07-density-desk.jpg` and `08-density-desk-worse.jpg` show eighty-plus enemies
across six archetypes, and they are a single carpet of tan lumps. The palette is
the cause and it is deliberate: `DESK_WOOD (122,80,48)` floor against
`CRUMB_TAN (198,156,92)`, `DUST_GREY (148,142,138)`, `MOTH_WING (180,168,148)`,
`ANT_BODY (78,40,30)`. Every hostile on the desk is a desk-coloured object.

Pillar 4 says the twelve archetypes are "renamed and retinted so knowledge
transfers between runs". `14-grid-zero.jpg` and `15-rooftop.jpg` show the same
tan fluffballs and spiky balls in a neon city and on a checkerboard roof — the
retint either is not happening or is too small to see. Knowledge cannot transfer
between worlds when the player cannot tell the archetypes apart inside one.

**Fix.** Reserve one hue band for hostiles across all five worlds and refuse to
use it for anything else. Darken and desaturate every floor by 25–35% so the
band has somewhere to sit. `14-grid-zero.jpg` is proof this is a choice and not a
renderer limitation: the same meshes and the same lighting read cleanly over a
dark floor.

### 3. Damage numbers are the loudest thing on screen and carry no decisions

`08-density-desk-worse.jpg` has roughly twenty floating numbers in two colours,
overlapping into unreadable smears — "4545", "22 22", "553", "1833". They are
white and gold, the highest-contrast elements in the frame, and they cluster
exactly where the fight is, which is exactly where the player and the enemies
are.

In a game whose first pillar is "aiming is automatic — you never press attack",
per-hit damage is close to pure noise. It buys nothing and it costs the most
valuable pixels on the screen.

**Fix.** Off by default. Keep them for kills only, or aggregate per second per
target, or replace them with a hit flash on the target. This is the cheapest
large improvement available.

### 4. The threat dial — the design's central pillar — is a 22-pixel number in a corner

DESIGN.md gives THREAT a section of its own and calls it "the pacing dial". On
screen it is `THREAT 3.6` in orange with the word `SWARMING` beside it at 13px,
plus `x3.25 rewards` at 14px green, in the top-right corner. No dial, no gauge,
no ramp indicator, no floor marker, no sense of the 0.5–8.0 range or where you
sit in it.

Underneath: `STEP = 0.25` (src/threat.rs:18), so travelling the full range is
**thirty keypresses**, one tap at a time, and the floor rises invisibly at
`MIN_INTENT + elapsed/45 * 0.25`. In `12-results.jpg` the run ended with
"Peak threat 1.0" after I pressed `=` twelve times, because the dial does not
unlock until 75s and I died at 61s — the game's headline mechanic never
appeared, and nothing on screen said so while I was pressing the key.

**Fix.** Make it a real gauge with a visible 0.5–8.0 track, a floor tick that
creeps up so the player watches their options close, and the reward multiplier
rendered as the same length of bar. Add hold-to-repeat and a jump-to-floor key.
Show the locked state on the dial itself, not only as a five-second banner.

### 5. Ring colour is overloaded past the point of meaning

`Glow::spec` (src/art.rs) assigns eighteen glow colours, and the collisions are
structural:

- **Green:** `Friend (120,255,158)` for allies *and* turrets, `ZoneHeld
  (0.4,1.0,0.6)` for a held zone, `Xp (126,232,128)` for XP orbs. Three meanings,
  one hue.
- **Cyan:** `Elite (120,246,255)`, `Plasma (0.35,0.95,1.0)`, `Ally
  (0.45,0.85,1.0)`, and the build cursor. `02-plan-mode.jpg` shows the build
  cursor and an elite's halo as the same cyan ring in the same frame.
- **Red:** `Warning (1.0,0.35,0.2)` for enemy-held ground, `EnemyShot
  DANGER (232,72,66)`, and `Heal HEAL_RED (240,86,96)`. **A healing pickup is the
  same hue as incoming fire.**
- **Cream:** `Boss (255,246,214)` and the player's own weapon FX.
  `15-rooftop.jpg` has a dozen thin cream rings and one of them is the boss.

Rings are also all the same mesh at the same weight, so hierarchy is carried
entirely by hue at low saturation in a dark scene — the worst case for the ~8% of
male players with a red-green deficiency.

**Fix.** Cut to four ring identities and separate them by *weight and pattern*,
not only hue: mine (thick solid), hostile (thin solid), objective (dashed), and
"look at this now" (pulsing). Retire `Glow::Ally` (it is a blue while
`Glow::Friend` is a green, and both mean friendly). Move `Heal` off red.

### 6. The squad and the structures are visually identical

`11-squad-and-turrets.jpg` has four allies and three turrets on screen. Both get
`Mesh3d(art.ring)` with `MeshMaterial3d(art.glow(Glow::Friend))` — literally the
same ring mesh and material (src/allies.rs:577 and :668). Scout (62 HP), Gunner
(74), Bulwark (210) and Medic (86) are four small dark lumps, and so are a Tack
Turret and a Lobber. No role glyph, no size difference, no health bar, no stance
marker.

This is the management layer of a management game and it is unreadable on the
field. It also means the two things the playbook says decide runs — whether your
Bulwark is in the chokepoint and whether your turret ring is intact — cannot be
checked at a glance.

**Fix.** Silhouette differentiation first (a Bulwark should be visibly twice the
mass of a Scout), then a two-state health pip under each, then a small stance
glyph. Turrets get a distinct ring shape from allies.

### 7. Plan mode hides the one thing you go into plan mode for

`02-plan-mode.jpg` and `11-squad-and-turrets.jpg`: while placing a turret the
game shows no range circle for the turret being placed and no range circles for
the turrets already built. There is no range visualisation anywhere in the
codebase — `coverage.rs` is a content-tracking module, not a display. The build
ghost is an untextured red-orange cube that does not resemble what you are
placing.

Three further things go wrong in the same screen:

- The status line renders as world-space text straight across the middle of the
  play area, over the player and over the cursor: *"PLAN - [1] Tack Turret 25
  scrap  ENTER to place, SPACE to resume"* sits on top of the duck in
  `02-plan-mode.jpg`, and *"Blocked - move the cursor."* sits on top of the ghost
  in `11-squad-and-turrets.jpg`.
- The camera does pull back — `BASE_DISTANCE 34.0` to `PLAN_DISTANCE 52.0`, about
  2.3× the visible area — but the extra board arrives carrying no extra
  information. No range rings, no objective markers, no minimap, no wave preview.
  Plan mode gives you more floor and no more to think with.
- `PLANNING / Arrows aim the cursor. WASD still walks…` occupies a large panel at
  the bottom every single time, forever, long after it is needed.

**Fix.** Range rings on placement and on every existing structure while plan
mode is open; a real ghost mesh; move the status line to a fixed slot; pull the
camera back properly; retire the tutorial panel after the third build.

### 8. The infinite world has no landmarks, and you can see its edges

`16-far-country.jpg` is minute seven at 500 units from the landing site: the
same brown desk, the same sticky notes, books, donuts and pencil pots at
different random offsets. Nothing tells you you have travelled. There is no
horizon, no skyline, no biome variation, no structure worth walking towards.
`ZONES 0/22` had been `0/25` and `0/28` in earlier frames — the denominator moves
as chunks stream, so even the objective count is not a stable goal.

And the streaming boundary is visible: in `11-squad-and-turrets.jpg` and
`13-pause.jpg` the floor ends in a staircase of 24-unit chunk squares with pure
black beyond it. The camera is `far: 400.0` with a 42° FOV while
`STREAM_RADIUS = 3` covers 72 units and the fog overlay covers
`(3+0.5)*24 = 84` units — so the void is not even fogged, it is just black.

**Fix (two separate things).** For the edge: an always-present ground plane
under everything plus distance fog to the horizon, so there is never a hard end
of world. For the landmarks: a small number of *large* set-pieces per world —
the monitor, the lamp, the keyboard at true scale, visible from far away — that
give the map a mental geography. This one is the biggest art job on the list and
the one that most changes how the game feels.

### 9. Every state readout is a flat list, so nothing is ever urgent

The HUD is ten absolutely-positioned regions (`grep -c 'PositionType::Absolute'
src/hud.rs` = 10: five corners and edges, a boss bar, a hint banner, a keybind
strip and two more), each a stack of same-weight lines. Consequences visible in
`07-density-desk.jpg`:

- `SCRAP 1715  CORES 86` next to `SQUAD 0/4`, `ZONES 0/25`. The playbook's
  number-one measured failure — "unspent resources are the commonest failure",
  runs ending on hundreds of Scrap — is displayed as two calm grey numbers. By
  `16-far-country.jpg` it was 2864 Scrap and 97 Cores at minute seven with zero
  turrets and zero allies, and the HUD never once escalated.
- `KILLS 454` — a vanity number — has the same weight as `ZONES 0/22`, which is a
  strategic failure.
- The largest element on screen is the elapsed clock, which is the least
  actionable number in the game. `PREP 18s` and `WAVE 9  3s` — the numbers a
  decision hangs on — are 60% smaller beside it.
- `LV 56` is the second-largest number and means almost nothing: levels arrive
  every ~13s and, per the playbook, each one makes the opposition stronger
  (+8.5%) by more than it toughens you (+4%).

**Fix.** Promote by consequence, not by category. Anything affordable and unspent
turns amber and pulses. Anything at zero that should not be (`ZONES 0/n`,
`SQUAD 0/4`) turns amber. Demote the clock and the level to small text. See §2 for
the layout.

### 10. The results screen is a receipt, not a lesson — and the gameplay HUD is still on top of it

`12-results.jpg`. The entire in-game HUD renders behind and around the death
panel: threat, scrap, cores, the weapon chips, the squad panel, the keybind strip,
the still-ticking clock, and — worst — **`ENTER: call early` is on screen at the
same time as `ENTER to run it back`.** One key, two contradictory labels,
simultaneously.

The content is a flat list of seven facts with no hierarchy beyond the survival
time, and it does not say what killed you, does not compare to the previous run,
and shows no curve of the run. The two most motivating lines — `LIFETIME 30 runs
25.9k kills best 8:22` and `Closest: Redline — Hold the threat dial at maximum.
(88%)` — are the smallest and dimmest text on the screen.

`ENTER to run it back` also does not run it back: the log shows
`state GameOver -> Menu`, so it returns to world select and needs a second ENTER.

**Fix.** Hide the HUD on `GameOver`. Name the killer. Draw the threat dial and
the enemy count as a two-line sparkline over the run so the player sees the
moment it went wrong. Promote the achievement-progress line to the top. Make
ENTER actually restart the same world.

---

## 2. What is illegible at a glance, and what the HUD should be

**Reads fine today:** health (top-left bar), the prep/assault phase colour
change, the reward multiplier as a number, the world title in the hint banner.

**Fails:**

| State | How it fails now | What it needs |
| --- | --- | --- |
| Threat | one number + one word, corner, 13–22px | a gauge with the range, the rising floor, and reward as a matching bar |
| Wave | `WAVE 9  3s` at 14px under a huge clock | the biggest timer on screen during prep; a visible composition preview of the incoming assault |
| Territory | `ZONES 0/22`, denominator moves, no direction to any zone | a compass ribbon or edge-of-screen chevrons for the nearest three objectives; a stable count |
| Squad | `SQUAD 0/4 FOLLOW`, four identical lumps on the field | four role slots with a portrait glyph, a health pip and a stance letter; matching glyph over each ally |
| Structures | indistinguishable from allies, no range shown | distinct ring, HP pip, range ring in plan mode |
| Research | four flat price lists, no affordability marking | see below |
| Forts | thin ring, no health, no progress, no name | radial progress arc + `CAPTURING 41%` + a garrison count + fort HP |
| Bosses | a 2px cream bar labelled in grey; boss alive four minutes and unlocatable | a proper boss frame with the `+N` modifier stack, plus an off-screen direction arrow |
| Occlusion | player and units vanish behind props | dither-through for anything friendly |
| Economy | two calm grey numbers | amber when affordable-and-unspent |

**The research tree is not a tree.** `05-research-tree.jpg`: four columns,
eighteen nodes, no edges, no prerequisites, no locked states, no path, no "you
are here". It is four price lists. `77 Cores  4 Skill Points` appears once in
small grey text at the top and no node is marked affordable or not, so with four
points and several nodes wanting one or two the player cannot see what they can
buy. The `[0]` notation on the Endless nodes is never explained. COMMAND has six
nodes to the others' four and hangs 240px below them, breaking the grid.

It also does not stop the game. In `05-research-tree.jpg` there are 25 enemies
with 7 inside 12m and a live boss while I read a wall of translucent text over a
brown desk — text over a moving 3D scene is the hardest reading condition the
game offers, and it chose it for its densest screen.

**The HUD I would build.** One left rail, one bottom bar, nothing else.

- **Left rail, top to bottom:** health, then the threat gauge (range, current,
  floor, reward bar), then the wave state as a single ring timer that changes
  colour between prep and assault. Three things, vertically, in descending order
  of how often you act on them.
- **Bottom bar:** four squad slots with glyph/HP/stance, then the structure
  palette as five slots showing cost and affordability, then Scrap and Cores as
  the last two cells so affordability reads by adjacency.
- **Top edge:** a thin compass ribbon — nearest zone, nearest fort, boss
  direction — as chevrons at the screen edge, distances only.
- **Nothing in the top corners.** Corners are where information goes to be
  ignored; four of the eight current regions are corners.
- **One transient slot**, bottom-centre, for the hint or event line, and a
  persistent "what's new" pill the player can re-open. Right now every hint
  expires in 5s (`HintTone::Tip => 5.0`) and queues behind other hints, so a
  burst of unlocks and enemy reveals trickles past a player who is busy fighting.

**Scaling, which is a shipping blocker for two stated targets.** Every HUD node
and every overlay is fixed pixels: `Val::Px(14.0)` insets, `Val::Px(260.0)` and
`Val::Px(400.0)` panels, `Val::Px(640.0)` for the menu detail panel, and fixed
font sizes throughout. There is no scale factor derived from window size.
`06-research-small-viewport.jpg` is the research tree at 607x569: MIGHT clipped
to a sliver on the left, COMMAND cut in half on the right, the header wrapped so
"close" lands on top of the health bar, the title scrolled off, and the gameplay
HUD interleaved through it. itch.io embeds are commonly 960x600 and a phone in
portrait is narrower than that. **One UI scale factor driven off the smaller
window dimension would fix most of it and should land before any more features.**

---

## 3. The first sixty seconds

What actually happens, from `01-first-30-seconds.jpg` and the log:

- **0:00** `HOLD THE DESK — WASD to move. You attack automatically.` Five seconds,
  bottom-centre. Good line, right length.
- **0:00** The screen: a brown desk, a 24-pixel yellow duck near the middle, a
  giant `0:17` clock at the top, `PREP 23s  ENTER: call early +56%` beside it,
  `THREAT 1.0 LULL / x1.00 rewards / O: OVERCLOCK READY` top-right, `SCRAP 800
  CORES 70` under it, `SQUAD 0/4 / ZONES 0/22 / KILLS 3` bottom-right, `Pencil
  Dart 1` bottom-left, and a seven-item keybind strip along the bottom in the
  lowest-contrast text on the screen. **Six populated regions, before the player has a
  reason to care about any of them.** Two of the named keys (`ENTER: call early`,
  `O: OVERCLOCK READY`) refer to systems that are locked.
- **0:05** `PRESS SPACE TO PLAN — Time slows to a crawl.` The player presses
  SPACE. Build does not unlock until 45s. There is nothing to place, nothing to
  assign, no squad, no zones. Plan mode does what it says and accomplishes
  nothing. **The first thing the game teaches, it teaches forty seconds early**,
  and the lesson the player takes is that the button is pointless.
- **0:10** `NEW: Dust Bunny — Slow and harmless alone. Dangerous in a mass.` The
  Dust Bunny on screen is a tan lump at 1.25:1 against a tan floor. The banner
  names something the player cannot pick out.
- **0:00–0:45** Three enemies, nearest 23m. Nothing to fight, nothing to build,
  nothing to spend. `SCRAP 800` sits there. Held W the whole time, per the
  design's own aim, and nothing happened.
- **0:27** First level-up (`04-level-up.jpg`). The card screen is the best screen
  in the game: three panels, rarity-tinted borders, a number, a name, a one-line
  effect. Two problems — it is modal and blocking, and at one level per ~13s it
  will interrupt about seventy times in a fifteen-minute run. Minor: `R to reroll
  (once per level)` sits below the cards in 14px blue and is easy to miss for the
  first several levels.
- **0:45** `SALVAGE ONLINE — Press B to build turrets with Scrap.` The first real
  decision, three-quarters of a minute in.
- **0:56** (`09-fort-capture.jpg` at 0:56 on a later run) `SKILL POINT EARNED`
  gets a large bottom-centre banner while a fort capture at `-89%` and
  `contested: true` — a five-second window — happens silently on screen.

**Where it loses them.** The whole first forty-five seconds has nothing to do.
There is no threat to react to, no resource to spend, no ground to take, and the
one verb it teaches has no object yet. Meanwhile the screen is already at full
HUD complexity and already advertising two locked systems. A new player's first
minute in a *survival command* game should contain one small crisis they solve
with one new verb.

**Fix.** Move `UNLOCK_BUILD` to about 12s and give the player a reason: a single
small assault from one direction, arriving at ~15s, that a single turret trivially
answers. Delay the plan-mode hint until build unlocks and make plan mode's first
appearance *be* the build tutorial. Start the HUD as three elements — health,
wave, and one hint slot — and fade in each region as its system unlocks. Do not
render `ENTER: call early` or `O: OVERCLOCK READY` before those keys work.

---

## 4. Where the art is doing least work for its cost

The all-procedural constraint is respected below; none of these ask for an asset
file.

**Meshes: the budget is spent where it cannot be seen.** `sphere()` is `ico(2)`
(320 triangles), `sphere_hi()` is `ico(3)` (1280). The player duck is built from
`sphere_hi(0.5)` + `sphere(0.34)` + `sphere_hi(0.31)` + a beak + **eyes of radius
0.075 with highlight spheres of radius 0.032** — roughly 3,500 triangles for
something that renders at 24x27 pixels, where a 0.032-radius highlight is a
fraction of one pixel. Enemies are the same story: a Dust Bunny is
`sphere(0.44)` plus tufts of `sphere(rng.range(0.07, 0.14))`. **All of the
geometry goes into interior detail that dies at play distance, and none goes
into the silhouette, which is the only channel that survives.** Six archetypes
on screen in `07-density-desk.jpg` and every one is a ball.

The single highest-value change to the meshes is to make the twelve archetypes
distinguishable by *outline* at 30 pixels — one is tall and thin, one is wide and
flat, one has a spike above the silhouette, one is a cluster of three — and to
scale rank into size so an elite is visibly bigger and a boss is unmissable. In
`14-grid-zero.jpg` and `15-rooftop.jpg` the bosses ("SIEGE FRAME", "THE
PILEDRIVER", both with near-full bars) are grey slabs *smaller than their own
rings* and read as architecture.

**Lighting: the stated art direction is not on screen.** `palette.rs` opens with
"One warm desk lamp in a dark office". `apply_look` spawns exactly one
`DirectionalLight` and sets a strong blue-tinted ambient (`brightness: 210.0`,
`sun_illuminance: 2100.0` for the desk). The result is flat, even, shadow-soft
lighting with no local focus. `10-fort-nest-lamps.jpg` has two desk-lamp props
in frame and **they emit no light** — measured, the floor 2m from the lamp base
is `(63,52,44)`, no brighter than floor 20m away at `(79,43,20)`. The geometry
for the concept exists; the light does not.

One warm point light per lamp prop, a darker ambient, and a real pool of light on
the floor would give the desk depth, give the player a legible "safe island"
read, and make the existing `LightPools` mechanic visible instead of a number in
a state dump. Also missing: `DirectionalLight` is spawned with no
`CascadeShadowConfig` against a `far: 400.0` camera in a streaming world, which
is asking for shadow popping as chunks load.

**Fog reads as an artifact rather than as fog.** `FogMap::veil` returns exactly
three values — `None`, `DIM_ALPHA`, `1.0` — and `rebuild_overlay` writes one
uniform alpha across all four vertices of each 3-unit quad. So the veil steps in
hard 3m terraces with no gradient. Measured on `01-first-30-seconds.jpg`, a
single scanline at y=425 drops the floor from luma 54 to 35 — a 35% cliff in one
pixel, running the width of the frame. The mesh already carries per-vertex
colour: interpolating alpha from the distance to the sight radius would smooth it
at zero runtime cost.

**Colour: the floors are fighting the entities in three of five worlds.** The
desk's brown-on-brown is the worst (§2). The rooftop's dark-blue/dark-orange
checkerboard is nearly as bad, because the checker's spatial frequency and
contrast are close to the enemies' — a moth reads at **1.06:1** there. Grid Zero
is the model: dark floor, saturated cyan and magenta reserved for architecture,
and everything on top of it pops. Its enemies still read at only 2.39:1, but that
is twice the desk.

**Post-processing.** `Bloom { intensity: 0.22 }`, `TonyMcMapface`, `Msaa::Sample4`
and `Hdr` are all sensible and the emissive rings do glow properly. The bloom is
also the reason a dozen thin cream rings in `15-rooftop.jpg` merge into a haze —
worth a lower bloom on FX rings specifically, or fewer rings.

**Camera.** One fixed overlook pitch for all five worlds, rotatable in 45°
steps. Rooftop is *defined* by height and is shot from above, so you never see
it. Grid Zero has vertical neon architecture and you see the tops of it. This is
partly taste, but a per-world camera pitch (lower for rooftop and Grid Zero,
higher for the desk) is nearly free and would make the worlds feel different for
the first time.

---

## 5. What I would cut

- **Damage numbers.** §1.3. Nothing decides on them.
- **The kills counter.** A vanity number in a slot that could carry an
  affordability warning.
- **The elapsed clock's prominence.** Keep the number, make it 12px.
- **`Glow::Ally`.** A blue that means the same thing as the green
  `Glow::Friend`. One of the two is dead weight and the pair actively confuses.
- **The permanent bottom keybind strip.** Seven bindings in the lowest-contrast
  text on the screen, ungrouped, present forever. Move it to pause (which has
  room and currently has no controls reference at all) and show only the two or
  three keys that are live and unused.
- **The duplicated instructions on the pause screen.** `13-pause.jpg` prints
  `ESC resume / BACKSPACE abandon` at the top and again as `ESC to resume /
  BACKSPACE to abandon the run` 200px lower, in different wording.
- **Five structure types.** Barricade and Shocker do no damage and their value is
  invisible: nothing shows enemy pathing, so a Barricade's effect is unobservable,
  and nothing shows the Shocker's slow aura. Either visualise both or cut to
  three (Turret, Lobber, Generator) until there is a display that makes them
  legible. Cutting is cheaper.
- **The `[0]` Endless research nodes, for now.** Six of eighteen nodes are
  repeatable percentage stacks with unexplained notation, on a screen that
  already cannot show what you can afford. They add cost to the hardest screen
  and reward nothing a player can perceive.
- **`Whisper Campaign` and `Blood Feud` as research nodes.** Inciting a faction
  war is the best idea in the design — "turn the neighbours on each other and
  walk through the middle" — and it is buried as the fifth and sixth entries in
  the fourth column of a price list, at 14/26 Cores plus 2 Skill Points, with no
  map to show which factions border which. The mechanic is not over-featuring;
  its *presentation as a research row* is. It wants a map screen, or it wants
  cutting until it has one.

Not on this list, deliberately: forts, factions, territory, allies and the
threat dial all earn their complexity. Every one of them is under-presented
rather than over-built.

---

## 6. The three best impact-to-effort changes

**1. Give the player a marker, and take the damage numbers away.**
One child entity with a reserved-hue ring in `spawn_player`, plus a flag that
suppresses `FloatingTextEvent` for non-lethal hits. Together these fix the
single worst legibility failure in the build and remove the loudest source of
noise, and neither touches a system. Screenshots that change immediately:
`07-density-desk.jpg`, `08-density-desk-worse.jpg`, `15-rooftop.jpg`.

**2. One UI scale factor, derived from the window's smaller dimension.**
Every panel, inset and font size in `hud.rs` and `screens.rs` is a literal
`Val::Px`. Routing them through a single multiplier turns `06-research-small-viewport.jpg`
from unusable into usable and unblocks both stated ship targets — itch.io embeds
and phones — without any design work. This is mechanical, testable, and the
highest-value hour in the repo.

**3. Darken every floor and reserve one hue for hostiles.**
A palette edit: drop each world's floor 25–35% in luminance and desaturate it,
and retint the twelve archetypes into one reserved band. This moves enemy
contrast from 1.06–1.37:1 to something a player can parse, in five worlds at
once, by editing constants in `palette.rs` and the world `look()` functions.
`14-grid-zero.jpg` already shows what the game looks like when the floor is dark.

Honourable mention, slightly more work but the same order of payoff: **turret and
placement range rings in plan mode**. It is the missing half of the game's
strategic layer and it is one ring mesh, already in `GameArt`, shown
conditionally on `PlanMode::active`.

---

## Appendix: screenshots

All in `docs/ux-critique/`, from the session described above.

| file | what it shows |
| --- | --- |
| `01-first-30-seconds.jpg` | the desk at 0:17, full HUD, three enemies, nothing to do |
| `02-plan-mode.jpg` | plan mode; build cursor cyan next to an elite's cyan halo; status text across the player |
| `03-world-select.jpg` | the menu: text on black, five identical grey chips, no art |
| `04-level-up.jpg` | the card screen — the best screen in the game |
| `05-research-tree.jpg` | four flat price lists over a live fight with a boss |
| `06-research-small-viewport.jpg` | the same screen at 607x569: clipped both sides, header over the health bar |
| `07-density-desk.jpg` | 117 enemies, 20 damage numbers, player unfindable |
| `08-density-desk-worse.jpg` | 80+ enemies, ring soup, contrast measurements taken here |
| `09-fort-capture.jpg` | a fort at `capture -0.89, contested` — a thin ring and no other information |
| `10-fort-nest-lamps.jpg` | two desk lamps emitting no light; fort and nest rings overlapping |
| `11-squad-and-turrets.jpg` | 4 allies + 3 turrets, all identical; visible chunk void beyond the floor |
| `12-results.jpg` | the death screen with the whole gameplay HUD still on top of it |
| `13-pause.jpg` | pause: unpanelled text over the world, instructions printed twice |
| `14-grid-zero.jpg` | the best-looking world, and proof the palette is a choice |
| `15-rooftop.jpg` | the worst readability in the game; player not locatable |
| `16-far-country.jpg` | minute seven, 500 units out — indistinguishable from the start |
