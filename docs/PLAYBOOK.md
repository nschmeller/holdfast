# The playbook

Accumulated strategy knowledge for HOLDFAST, written by everyone who plays it.

**Read this before a run. Append to it after.** Fifty rounds of testing is only
worth more than one round fifty times if what each run learns survives it.

Keep entries short and concrete. A hypothesis, what you did, what happened,
what to try next. Numbers beat adjectives.

---

## What is known

**The record, as of the last round played, is 374.4 seconds** (fortress,
level 34, 331 kills, peak threat 8.00 — the dial's hard ceiling, reached and
held with the player at full HP for the entire climb). Up from 337.7s
(turtle, round 2), up from a starting baseline of 148s. See `fortress` below.

### Confirmed by measurement

- **Kiting beats roaming by a wide margin.** Weapons fire themselves, so the
  whole craft of fighting is standing where you are hitting and they are not.
  Mindless roaming dies at ~35s pre-fix and ~190s post-fix. Deliberate kiting,
  played carefully, now measures at 172.4s (see `kite-and-explore` below) —
  and a reckless version of the same strategy measured at 82.6s, *worse* than
  mindless roaming. Kiting is not automatically safe; where and when you kite
  matters more than the verb itself.
- **A held position with a turret ring beats free-roaming kiting by a wide
  margin once structures scale with the difficulty curve.** 337.7s vs 172.4s
  in the same session, same weapon draws available. See `turtle` below.
- **Unspent resources are the commonest failure.** Runs routinely end holding
  hundreds of Scrap and dozens of Cores. If it is banked, the system it buys
  went unused. Confirmed again this round: even a disciplined turtle run
  ended holding 198 Scrap and 5 Cores, banked seconds before death because a
  crisis interrupted the spending rhythm. Bank as little as possible; spend
  the moment you can afford the next tier.
- **Nothing happens within 130 units of the landing point.** Forts, nests,
  seeders and factions are all further out. A run that never travels cannot
  touch half the game.
- **Levelling settles to roughly one level per 13 seconds** after level 2 and
  stays there. Each one is a modal card screen, so a fifteen-minute run is
  about seventy interruptions.
- **Every level makes the opposition stronger** (`level_power`, +8.5%/level)
  by more than it makes you tougher (`constitution`, +4%/level). Levels are not
  optional — the game blocks until you take one — so levelling is a net
  defensive loss before the card is applied. Weapon and health cards have to
  make up the difference.
- **Turret kills do not pay the player.** A/B'd directly this round: standing
  back on `defend` while a Tack Turret killed ~30 enemies produced zero Scrap
  gain and zero XP — level and Scrap stayed frozen for over 20 seconds despite
  the kill counter climbing. Switching to `chase` to land kills personally
  jumped Scrap from 14 to 40 and triggered an immediate level-up within a few
  seconds. A turret ring clears ambient pressure and can hold ground, but it
  does not fund itself or level the player up — you still have to fight
  yourself to afford the next turret.
- **Once truly encircled, movement commands stop working.** Reproduced twice,
  independently, in two different runs: with ~10+ enemies within melee range,
  issuing `kite`, `flee` or `goto` produces *no positional change at all* for
  several real game-seconds (confirmed by identical coordinates across
  consecutive state reads), during which the player absorbs a full,
  unmitigated damage tick roughly every 0.4-0.7s. Both near-deaths and one
  actual death trace back to this. The escape has to happen before the
  encirclement, not after — once surrounded, disengagement verbs cannot buy
  the distance back. Whether this is knockback/stagger-lock from repeated
  hits, a pathing failure with no clear route out, or something else was not
  determined; worth a focused look by whoever can watch it happen rather than
  read it from a state dump.
- **Level-up chains are a real hazard while dense.** Rapid kills from AoE
  weapons (Ruler Sweep, Clip Orbit) can chain 2-3 level-ups within a handful
  of seconds. Each one blocks input. If a crowd is closing in when the chain
  starts, by the time you are done picking cards you can be already
  surrounded with no movement command active — chain the card pick and the
  next movement command into the same batch of inputs; do not resolve the
  card in isolation and issue movement afterward, the gap between is where
  the damage lands.
- **The threat-dial reward formula is exactly `reward_mult = effective_threat
  ^ 0.92`**, where `effective_threat = threat.level + 0.2 * zones_held` (each
  held zone adds a flat 0.2 to the exponent's base, on top of raising the
  floor). Verified numerically at four separate points this round: threat
  2.194 + 1 zone (eff. 2.394) → reward 2.245 (calc: 2.394^0.92 = 2.24); threat
  8.0 + 1 zone (eff. 8.2) → reward 6.93 (calc: 8.2^0.92 = 6.93); threat 8.0 +
  0 zones (eff. 8.0) → reward 6.774 (calc: 8.0^0.92 = 6.77). This is the
  game's central paid-for-danger claim and it had never been checked against
  the actual numbers before this round — it holds exactly.
- **The threat dial has a hard ceiling: 8.00.** Pressing `=` repeatedly stops
  raising it there and the game fires a "Redline: hold the threat dial at
  maximum" achievement hint. Nobody had reached it before this round (prior
  best was 2.38, reached by organic floor drift, not the dial).
- **A turret/generator ring genuinely funds itself and then some, once
  Generators are in the mix.** `structure_income` pays a flat 2.4 scrap/s per
  live Generator (not scaled by threat, unlike Generator HP/damage which do
  scale via `defence_scale`), and it stacks linearly — four Generators plus
  one held zone plus the Logistics/Supply-Lines-style income cards pushed
  income from 0 to 27.9 scrap/s over the course of one run. Past a certain
  ring size, income growth outpaces the *player's own throughput* for
  spending it (see `fortress` below): the bottleneck stops being "do you have
  scrap" and becomes "can you physically place structures fast enough".
- **Research works and is cheap relative to what it buys.** Whisper Campaign
  cost 14 Cores (out of a pool that regenerates from elite/zone kills) and
  its `boost` field grants a permanent income multiplier *in addition to*
  triggering the 45-second war side-effect — income jumped 16.1→19.4 scrap/s
  the instant it was bought, before any war could have taken effect. Whether
  the war itself actually started could not be confirmed this round: the
  dossier's `wars` field reads active wars *at the moment the run ends*, and
  a 45s (or even a rank-scaled 110s Blood Feud) war will have long expired by
  the time a fortress run ends 130+ seconds later. A zero in that column is
  not evidence the war never happened.
- **The `defend`/`kite` encirclement-freeze failure reproduces at far higher
  scale and is fatal, fast.** At threat 8.0 with a worn-down ring (down to
  2-3 structures) and roughly 80-90 enemies within 12m, the death spiral from
  full HP to 0 took about 14 real seconds once it started, in a near-metronomic
  ~20-35 damage tick every 0.4-0.5s that no `defend` radius or threat-dial
  pull-down could interrupt in time. The `-` key presses issued the moment
  the drop started did not register before death (see `fortress` below) —
  either they were swallowed by a `LevelUp` state opening mid-batch, or the
  reaction window is simply narrower than one `pilot.py do` round-trip.
  Whatever the fortress build, the moment ~90 enemies are inside 12m at max
  threat is already too late to save the run by de-escalating.

### Open questions nobody has answered

- Does holding territory pay for the threat floor it raises? Partially
  answered: standing near a zone marker for ~8s captures it automatically
  (no deliberate "hold still" action needed beyond `defend`-ing nearby) and
  it does pay — 1.6 scrap/s plus a flat +0.2 to the reward exponent's base,
  confirmed this round. What is still unknown is whether the *floor* it
  raises (`threat.territory = held * 0.2`) is worth it at scale — with one
  zone the answer was clearly yes; nobody has held two or more at once.
- Can allies take a fort without the player present? Still completely
  untested — no run has gone near a fort yet (see below). The recruit
  mechanic itself is now understood, though: despite the HUD hint reading
  "Press R near a beacon to recruit", `handle_recruit` in `src/allies.rs` has
  *no proximity check at all* — `R` recruits from anywhere in the world the
  instant `allies` is unlocked and Cores/squad-cap allow it. The "near a
  beacon" text appears to be vestigial or forward-looking, not a real gate.
- Is a faction war actually pressure relief, or does it just kill things you
  wanted the XP from? Still functionally untested. Whisper Campaign was
  bought for the first time this round (14 Cores, Command branch, 5 `RIGHT`
  + 5 `DOWN` from the tree's default cursor position) and its income
  side-effect fired instantly, but the war itself lasts only 45s and the
  dossier only records wars active *at death* — by the time this run ended
  (120+ seconds later) any war would already have expired. Whoever tries
  next should buy it, then immediately check `raw` state's `wars` array
  (not wait for death) to see the actual pairing and timer.
- **Answered: what does maximum threat do to the economy over a long run?**
  See "confirmed by measurement" above — `reward_mult = effective_threat^0.92`
  exactly, verified at threat 2.2 through the dial's hard ceiling of 8.0. A
  fortress able to tank threat 8 earns roughly 6.8-6.9x the base reward per
  kill, for free, the whole time it holds.
- Is knockback beside a chasm strong enough to build around? Still untested —
  no chasm was encountered in six runs across three rounds.
- Do light pools pay for the attention they draw? Still untested.
- **Does a turret ring plateau or genuinely hold indefinitely?** Better
  answered but not fully settled: a 25-structure ring at threat 8.0 held the
  *player* at full HP through 100+ simultaneous enemies for a sustained
  stretch (multiple structure/ally wipe-and-rebuild cycles, all while HP
  never dropped), but once density crossed roughly 90 enemies within 12m
  with the ring worn down to 2-3 structures, the position collapsed to death
  in about 14 seconds flat with no recovery window. So: a big-enough ring
  *does* hold indefinitely against moderate-to-high threat, but there appears
  to be a genuine density ceiling near ~90-within-12m at max threat where
  rebuild rate cannot keep up with attrition rate no matter how much scrap is
  banked, because building costs real actions/time, not just currency.
  Untested: whether a ring built *even bigger, even earlier* (before threat
  8 rather than reactively during it), or one that adds Barricades
  specifically to choke the approach lanes down to fewer than 90-fits-within,
  pushes that ceiling higher, or whether it's a hard wall regardless.

---

## Fixed after round 3 — read this before planning anything

**The reason nobody has ever reached a fort was `goto`.** It had a flat
thirty-second safety net. Real travel is around five units a second once
crowds, scenery and level-up screens are counted, so the verb covered about
150 units and then quietly released the keys. Forts start at 130 units and
cluster past 200. Every run that was told to walk out to the war stopped dead
in the empty middle, and *nothing in the report said so* — the command drained
from the queue and the position simply stopped changing. Three rounds of
`zones=0 forts=0 wars=0` came from that one line.

`goto` now budgets from the distance, says how far short it stopped if it runs
out, and sidesteps when it stops making ground. **Walking to (200,30) from the
landing site now works and takes about 35 seconds.** Eight forts sit inside 95
units of there.

**The fort chain has been verified end to end by hand** — travel out, clear the
ring, hold it, meter reaches +1, fort flips to YOU, and the losing faction
comes back for it. It is now the most valuable untried strategy in the game and
nobody has done it in a real run.

What a fort is now, since all of this is new:

- **Three emplaced guns**, firing in rotation round the wall, reaching 15 units
  — twice the 7.5-unit capture ring, so there is no standing at the edge and
  waiting it out. About 14 damage a second. They are the fort; there is nothing
  to snipe off first. **Bring armour, regen and health, or do not go.**
- **Wardens** while contested: elites sent specifically to drive you off.
- **Contest speeds the assault timer up**, so eleven seconds in a ring is a
  siege.
- **Forts get tougher the further from home they stand**, up to a ceiling near
  double. The first one you meet is the lesson.
- **Garrison monsters stall a capture rather than reverse it** while you are
  standing there. Three of them stop a lone player outright; a squad or a
  cleared ring makes it quick. So: kill the nests, thin the ring, then hold.
- **A fort you take is a much weaker thing**: one gun at a third the damage, no
  wardens, no assaults. It pays Cores and Scrap and raises the threat floor.
- **Turrets count as presence at half a body.** This is the interaction that
  makes holding one possible — four turrets in the ring turn an eight-monster
  reclaim into a hold. Nobody has tried building on a captured fort.
- The pilot reports `garrison` per fort, so a meter that will not move tells you
  why.

Other operational fixes:

- **Arrow keys answer to any spelling** — `UP`, `ArrowUp`, `arrow_up` all work.
- **A command the game refused is now sticky** in the report, under
  `!! REFUSED THIS RUN`. It used to appear for one snapshot, 200ms, and vanish —
  so a mistyped key was indistinguishable from a dead key.
- **The dossier's `furthest` column is now this run's travel**, not a lifetime
  personal best. Every row before this one printed the same 312.
- **`best streak` counts kills** with under three seconds between them. It used
  to be unassigned, so it read 0 after thousands of kills.
- **Card offers are weighted**, not a uniform shuffle: levelling a weapon is
  worth four times a stat point. `Refinement` was unreachable and now appears.
- **The threat dial unlocks at 75s**, not 300s.
- **Research costs skill points as well as Cores** on the repeatable nodes (1)
  and on Whisper Campaign and Blood Feud (2). Skill points arrive every third
  level and previously bought nothing at all.

## Fixed since round 2, so old measurements are stale

- **Distant kills now send their loot home.** A strategist measured that a
  turret ring "pays the owner nothing". The orbs existed - forty metres away,
  and the whole point of a turtle build is not going there. Pickups outside the
  magnet radius now drift towards the player slowly, so a defended position
  funds itself. Any measurement of turtle economy before this is void.
- **`kite` and `flee` steer out of the gap in the ring**, not away from the
  single nearest enemy. The old behaviour is why "movement produces no
  positional change for several seconds" inside a crush: the nearest enemy
  changes every frame and the bearing flipped with it. It now sums repulsion
  from everything nearby and commits to the result.
- **A `tap` holds for a tenth of a second.** One frame raced the state
  transition into the level-up screen, which is why card selection looked
  intermittent for three rounds.

## Strategies tried

*(Append below. Newest last.)*

### kite-and-explore — 82.6s and 172.4s (two attempts, same round)
Hypothesis: never stop moving, never build, travel outward continuously; the
cheapest strategy to execute and the baseline everything else should be
measured against.

First attempt sprinted straight to ~140 units out during the opening Prep
window (`goto 140 0`), landing directly in a cluster of 2 forts and several
nests before any levels or gear existed. Density spiralled from 10 to 40
enemies in under a minute; kiting the *nearest* enemy does nothing when 10+
are converging from every direction, and level-up screens kept freezing the
player in place while the crowd closed in. Died at **82.6s, level 3, 42
kills** — worse than the pre-round mindless-roaming baseline of ~190s. Result:
`kite-and-explore DESK 82.6 3 42 0 0 0 0 0 1.01 312 11925 47 2 0.079`.

Second attempt farmed the sparse enemies near the landing zone first (moved
only 60 units out, fought there through two waves before pushing further),
took defensive/AoE cards (Ruler Sweep, Coffee Nova, Clip Orbit) instead of
whatever was offered, and built one Tack Turret during a Prep lull instead of
banking the Scrap. Survived to **level 8, 106 kills, 172.4s** — more than
double the first attempt on identical mechanics, just played less
recklessly. Died the same way both times: a level-up chain (3 in ~15 real
seconds) froze the player while a dense cluster (Staple Skitters + a boss)
closed the distance, and no movement command could open it back up once
surrounded. Result:
`kite-and-explore DESK 172.4 8 106 1 0 0 0 0 1.46 312 13437 75 6 0.206`.

**Takeaway: "kite constantly" is not itself the variable that matters — where
and when you engage is.** The same verb produced a 2x difference in survival
time purely from not diving into a fort/nest cluster underlevelled. This
should reset the framing for anyone using kite-and-explore as a baseline:
compare against 172.4s, not 82.6s.

### turtle — 210.0s and 337.7s (two attempts, same round)
Hypothesis: build a ring of turrets and hold one spot. Blocked in earlier
rounds because structures did not scale with the difficulty curve; that is
now fixed, so this was a genuine first attempt.

First attempt settled ~50 units out, placed a single Tack Turret, then
alternated between `defend` (to bank Scrap passively) and `chase`/`kite` (to
actually earn Scrap and XP — see the turret-kills-don't-pay finding above).
HP crashed to 15/170 once at t=157s when a boss + 2 elites closed in while
`defend` was giving ground indefinitely instead of holding a line, forcing a
flee that abandoned the turret and wandered into unknown territory. Died to a
55-enemy wave near an undiscovered nest cluster at **210.0s, level 7, 94
kills**, having never actually rebuilt the ring. Result:
`turtle DESK 210.0 7 94 0 0 0 0 0 1.67 312 12951 131 6 0.254`.

Second attempt was the disciplined version: picked a spot ~65 units out with
no visible fort/nest nearby, farmed personally (not on `defend`) until Scrap
allowed a real cluster, then placed 4-5 structures within 1-3m of each other
(2x Tack Turret, a Shocker, a Lobber) around a single held point and stayed
on `defend` with a tight 10-12 unit radius from then on, rebuilding whenever
scrap allowed. The ring visibly worked: HP *climbed* from 63/154 to 132/178
over about 80 real seconds while kills piled up (113 → 198), because the
turrets were clearing faster than the wave could build density. Reached
**level 14, research unlocked at t=246s (first time anyone has seen it),
threat dial unlocked at t=318s (first time anyone has unlocked it, though it
was never touched — died 20s later)**. The ring was ground down from 5
structures to 1 by wave 6 as threat climbed past 2.0, and the same
encirclement-freezes-movement failure that ended the first attempt forced a
flee that ran straight into a fresh BLOOM fort/nest cluster and died to a
92-enemy wave. Final: **337.7s, level 14, 220 kills, peak threat 2.38**.
Result: `turtle DESK 337.7 14 220 1 0 0 0 0 2.38 312 12357 198 5 0.381`.

**Takeaway: turtle is the strongest strategy measured so far by a wide
margin** (337.7s vs 172.4s best kite run), but it is not a steady state — it
buys a long plateau, not indefinite survival, because turret HP does not
seem to scale fast enough to outlast threat forever, and the player still has
to personally fight (not just `defend`) to fund the ring in the first place.
Whoever plays next should try funding a *bigger* ring *earlier* (bank Scrap
specifically for it in the first 60-90s rather than trickling one turret at a
time) and see whether that changes the wave-6-collapse pattern, and should
actually stand on a zone marker and open Research deliberately (not blind)
once the ring is stable enough to afford the idle time.

### fort-and-feud — untested
Take a fort, let it plant nests for you, then use research to set the two
neighbouring factions at war and hold the ground while they fight each other.
The most ambitious combination the systems currently allow. Still nobody's
reached a fort in a state stable enough to try capturing it — the closest any
run has come is passing within ~20-30m of fort markers while fleeing.

### fortress — 374.4s, new record (round 3)
Hypothesis: round 2's turtle proved a ring works but only tried one turret
and never touched Research or the threat dial. Build a *real* ring — several
structures, mixed kinds, Shocker + Barricade included — spend continuously
instead of banking, and deliberately pull both of the never-tested levers
(Research at 240s, the threat dial at 300s) once the ring is stable. Aim past
337.7s.

**What was built.** Landed, farmed the immediate area for ~90s (picking up a
zone at 3-5m from spawn almost by accident — it captures itself if you stand
near it, no deliberate action needed), then used `plan-mode` to place a
Tack/Tack/Lobber/Shocker/Barricade/Generator core ring at radius 3, then a
second ring at radius 6-7 (adding a 2nd Generator plus more Tack/Lobber/
Shocker), then, once the scrap income snowballed, a third ring at radius 10
with mostly Tack Turrets. Peak: **25 structures simultaneously**, squad
4/4 (Scout/Gunner/Bulwark/Medic), 6 weapons, income peaking at **27.9
scrap/s** (up from 0) thanks to 4 live Generators + 1 held zone + Logistics/
Supply-Lines-style cards. Scrap banked, at various points, well past 1000,
2000, and finally over 8000 — the economy compounded faster than the build
loop (cursor movement + placement, each structure taking real keystrokes)
could spend it, which is a *new* shape of the "unspent resources" failure:
not laziness, a throughput ceiling.

**Threat dial, pulled by hand for the first time ever.** Unlocked at t=304s.
Pressed `=` in bursts of 5. Threat climbed 2.19 → 3.50 → 4.75 → 6.00 → 7.25 →
8.00 over about 30 game-seconds, and **the dial hard-caps at 8.00** (fires a
"Redline: hold the threat dial at maximum" achievement hint the instant it's
reached — first time anyone has seen that state). Reward multiplier climbed
in lockstep: 1.0x → 3.33x → 4.35x → 5.36x → 6.34x → 6.93x, and the exact
formula came out to `reward_mult = effective_threat^0.92` where
`effective_threat = threat.level + 0.2 * zones_held` — checked against four
separate data points and it matches to two decimal places every time. **HP
stayed pegged at max (rising from ~290 to 374 via level-ups) through the
entire climb to threat 8.0** — the ring ate 100+ simultaneous enemies (peak
146, 8-16 elites, 2-3 bosses concurrently) without the player taking a single
point of damage for roughly 15-20 real seconds at the ceiling.

**Research, opened and bought for the first time.** `T`, then 3x `RIGHT` to
reach the Command branch, 5x `DOWN` to reach Whisper Campaign (14 Cores).
Bought it. Income jumped 16.1 → 19.4 scrap/s *immediately* — the node's own
`StatBoost::Income` side-effect fires regardless of the war, so it is not a
pure "spend cores, get a war" button, it is "spend cores, get a permanent
income bump AND (unconfirmed this run, see open questions) a 45s war between
the two strongest nearby factions."

**How it ended.** Once the ring had been repeatedly worn down (structures and
allies dying and being rebuilt several times over — squad dropped to 1/4 more
than once, structure count cycled between 25 and as low as 2), a wave at
max threat pushed density past ~90 enemies within 12m with the ring reduced
to 2 structures. HP went from full to dead in **14 real seconds flat**,
taking a near-metronomic 20-35 damage tick roughly every 0.4-0.5s — the same
encirclement pattern the round-2 turtle run hit, reproduced at an order of
magnitude more enemies. Pressing `-` repeatedly the moment the drop started
did not visibly lower threat before death; whether that's the same
level-up-swallows-keystrokes issue documented below or a genuinely too-narrow
reaction window was not determined. Died at **374.4s, level 34, 331 kills,
peak threat 8.00**, holding 8918 Scrap and 40 Cores unspent (structures=2,
allies=1 *at death*, both far below their peaks — the dossier only samples
the end state). Result:
`fortress DESK 374.4 34 331 2 1 0 0 0 8.00 312 2286 8918 40 0.540`.

**Operational notes for whoever drives next.** `pilot.py`'s arrow keys must
be sent as `UP`/`DOWN`/`LEFT`/`RIGHT` — `hold ArrowUp 1.0` parses to nothing
(`key_from_name` only recognises the plain names) and silently fails to move
the plan-mode cursor at all; this cost several wasted turns before it was
caught by explicitly diffing `plan_mode.cursor` before/after a move. The
cursor itself moves at a calibrated **17 units/sec of real time**, unaffected
by plan-mode's 12% slowdown (it reads `Time<Real>`), so a placement at radius
r from the player needs a hold of `r/17` seconds. Also: a `tap 1`/`2`/`3`
meant for structure selection, chained blindly through a `LevelUp` state that
opens mid-batch, gets silently reinterpreted as a card pick instead (and vice
versa) — always confirm `state == "Playing"` before trusting a structure-kind
keystroke landed, or you will burn a level-up choice you didn't mean to make
and place the wrong turret kind.

**Takeaway: the ring is not just the strongest strategy measured, it
*answers the game's central design bet* — a fortress that can tank threat 8
gets paid ~6.8x per kill, for free, the entire time it holds, and it can hold
a genuinely enormous crowd (100+) at full player HP.** It is not immortal:
there is a real density ceiling (~90 within 12m at max threat) past which
rebuild rate cannot keep up with attrition and the collapse-to-death window
is under 15 seconds with no observed recovery. Never travelled past ~20m from
spawn the entire run (furthest/explored numbers are unchanged from every
prior round) — forts, nests, and the fort-and-feud combination remain
completely untouched. Next: (1) build the *third* ring layer with Barricades
specifically, to choke approach lanes rather than just adding raw DPS, and
see if that raises the ~90-enemy collapse threshold; (2) buy Whisper Campaign
or Blood Feud and immediately check `raw` state's `wars` array rather than
waiting for death, to finally learn whether the war actually fires; (3) once
the ring is proven stable at threat 8, walk it — or a smaller satellite
ring — the 130+ units out to an actual fort and see whether a held zone's
"allies can capture without you present" claim holds up.
