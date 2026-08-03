# The playbook

Accumulated strategy knowledge for HOLDFAST, written by everyone who plays it.

**Read this before a run. Append to it after.** Fifty rounds of testing is only
worth more than one round fifty times if what each run learns survives it.

Keep entries short and concrete. A hypothesis, what you did, what happened,
what to try next. Numbers beat adjectives.

---

## What is known

**The longest a character has been kept alive is ~498 seconds** (castellan,
round 5, level 67, 1312 kills, peak threat 8.00 — but this run ended by a
deliberate `quit` while still fully healthy, not death, so **no dossier row
exists for it** — see `castellan` below for why and what the live numbers
were. The last row actually written to `holdfast-runs.tsv` remains **430.7
seconds** (siege, round 4, level 50, 1166 kills, peak threat 3.24, `forts=1`
— a fort captured, joining `warlord`'s two captures earlier the same round).
Up from 374.4s (fortress, round 3, level 34, 331 kills, peak threat 8.00 —
the dial's hard ceiling, reached and held with the player at full HP for the
entire climb). Up from 337.7s (turtle, round 2), up from a starting baseline
of 148s. See `siege`, `fortress` and `castellan` below.
**Fort-holding is no longer the open question either — `castellan` (round
5) took the same "plant the ring at the fort before it flips" idea `siege`
left on the table, and it closed the frontier completely: a fort was taken,
survived at the instant of the flip, lost, retaken repeatedly, and held
through both a researched war and the threat dial's 8.00 ceiling
simultaneously, for well over a hundred seconds of cumulative ownership.**

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

**Faction wars have never once fired, and now do.** Three separate faults, each
enough on its own:

1. `resolve_incitements` ran in `GameSet::Think`, which only runs while playing.
   The request is written by the *research screen*, which is its own state, so
   nothing ever read it and the message expired two frames later. Buying a war
   could only have worked if you closed the screen within about thirty
   milliseconds.
2. **Nothing in the ambient world had a faction.** Allegiance was stamped only by
   forts and nests; the wave director's horde - almost every monster anyone
   meets - had none. So "the two strongest powers nearby" could only find two
   while standing between two nests. Every enemy now belongs to whoever owns the
   ground it stands on, so the regions are real in ordinary play.
3. The purchase was irreversible and the failure was a transient hint. The
   research screen now **refuses to sell a war it cannot start** and says why on
   the node.

Verified: bought at level 10 near the landing site, `wars` read
`SWARM vs BLOOM (44s)`, hint "THE BLOOM TURNS ON THE SWARM. They are not looking
at you." The pilot now reports `war_available` and a `nearby` strength per
faction, so you can see whether a war is buyable before spending on it.

**A zone can now be garrisoned.** Zones decay to neutral when abandoned - that
is deliberate, a flag in open ground is not a place - but only the player and
allies counted as presence, and allies follow you and cap at four. So a second
zone cost you the first. **Turrets now count at half a body**, on zones exactly
as on forts. Two turrets hold a quiet zone; twelve monsters still take it. Nobody
has held more than one zone at a time yet, and now it is possible.

**"Encirclement freezes movement" was never a gameplay bug.** It was reported in
every round since the first, always the same shape: HP falling to nothing over
twenty-plus seconds of literally zero positional change while `kite` or `flee`
was running. The cause: a key put down by `press` is re-pressed every frame and
the steering code was forbidden from releasing it, so `press w` followed by
`kite` left W fighting the S the escape wanted. They cancel exactly, and the
player stands *completely* still for as long as the escape points south.

A steering verb now takes the movement keys back and says so. **The lesson for
whoever plays next: do not mix `press`/`release` with the steering verbs.** Pick
one. And when movement appears to stop, read the `!! REFUSED THIS RUN` line
before concluding anything about crowds.

For the record, so nobody re-litigates it: nothing in the game stuns the player,
and the two things that slow them floor out at 0.58 (a crowd) and 0.15 (a
hazard). The worst case is still three quarters of a unit a second. Zero was
only ever reachable through the bridge.

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
- **A key delivered into a modal screen is now reported.** A `tap 3` meant for
  structure selection is "take card three" if a level-up opened partway through
  the batch, and the turret is silently never selected.
- **Do not `pkill -f "target/debug/holdfast"`.** It kills every other agent's
  instance mid-measurement - this happened three times in round 3. Send
  `pilot.py do $PT "quit"` to your own instance instead.

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

### warlord — 363.9s and 350.4s (three attempts, same round), first fort capture in a live run
Hypothesis: this round's assignment was explicitly the four untouched systems
— zones, allies, forts, and faction war — in that order, not a survival-time
record. Every dossier row up to this point read `zones 0 allies 0 forts 0
wars 0`. Verify each mechanic works, with numbers, even if that means dying on
purpose to confirm a reading.

**Zone capture and decay, confirmed with numbers.** Walking onto a marker and
standing within ~2m for a few seconds captures it with no special command —
`defend`/`goto` both work, presence is presence. Confirmed twice: income went
0.0 → 1.6 scrap/s the instant `owner` flipped to `Player`, and `threat.
from_territory` went 0.0 → 0.2 (effective threat 1.0 → 1.2, reward_mult 1.0 →
1.183 — matches `effective^0.92` exactly). **New this round: zones decay when
abandoned.** Walked ~40m away for about 30 real seconds and watched
`progress` fall from 1.0 to 0.13, then to fully `Neutral` on the next check —
holding ground is not "capture and forget", it needs periodic return trips or
a second zone snowballs the same way in reverse.

**Fort capture, confirmed twice in a live run, including a full capture and a
full loss with exact cause.** First attempt reached "taking 42%" on a BLOOM
fort before dying underlevelled (110.5s, level 7) — enough to confirm the
`[taking N%]` reading is real, not cosmetic. Second attempt did the whole
loop: approached a SWARM fort at level 9-10, stalled at 29-36% for nearly 20
seconds because ~3 SWARM-allegiance monsters in the ring (`garrison: 3`) each
worth 0.34 presence almost exactly cancelled the player's 1.0 — textbook "one
lone defender each stalls it". Opened Plan Mode, placed **two Tack Turrets
inside the ring** (structures count as 0.5 presence each per the
`STRUCTURE_WEIGHT` constant in `forts.rs`), and progress went 36% → 45% → 100%
in about 15 real seconds. Log confirmed: `FORT TAKEN - It works for you now -
and they will come for it.` Reward multiplier jumped 1.36 → 1.76 the instant
ownership flipped, before anything else changed — a held fort pays into the
same reward-multiplier exponent a zone does. **Then it was lost, on camera.**
Later in the same run, pressing Overclock (`O`) spiked threat to ~6.0 for
about 22 real seconds (matches the design doc's own number exactly) and 8+
real seconds after that with the dial still at 3.5-6.0; the SWARM-owned nest
5m from the fort rode that spike and its trickle rate outpaced our rebuild
rate. Garrison climbed to 23 while our structure count had been worn down to
1-2 Barricades; friendly presence (~1.5-2.5) came nowhere close to defenders
(23 × 0.34 ≈ 7.8) plus the loss margin. Log: `FORT LOST: THE SWARM took it
back.` **Net: holding a captured fort is not free — cranking the escalation
dials you'd naturally reach for near a fort you already own can feed the
neighbour's nest faster than your ring can rebuild, and hand the fort straight
back.**

**Allies: recruited repeatedly, `R` truly has no proximity gate.** First
recruit cost 2 Cores, confirmed by economy delta (8→6), and it worked from
open ground with no beacon in sight — corroborates last round's source-read
that `handle_recruit` has no distance check. Ally survival is density-gated,
not random: a Scout recruited into a 60-70-enemy crush died within about 9
real seconds every time; the same recruit, made when nearby density was under
10, was still alive 9+ seconds later at the next check. Never got a clean
test of "can an ally alone capture an uncaptured fort" (question 4) — every
run either already held the only fort in reach or died before a second squad
member could be spared to leave behind on Guard. That specific test is still
open.

**Faction war: bought twice, fired zero times, and the reason is now known
exactly.** Whisper Campaign (14 Cores, Command branch, `RIGHT`×3 `DOWN`×5 from
the tree's default cursor) was bought in two different runs this round —
confirmed by exact Core deltas (15→1, then separately 42→28) — and both times
`raw` state's `wars` array stayed `[]` and `deed:war-incited` never marked
seen, checked repeatedly over several real seconds each time (research pauses
the clock, so there was no rush). Read `resolve_incitements` in
`src/factions.rs`: it only counts monsters that carry an `Allegiance`
component, which is **only ever inserted on fort-garrison, fort-assault, and
nest-trickle spawns** — the generic wave-director horde that makes up nearly
all of a long run's enemy count (Dust Bunny, Clip Crawler, Sugar Ant, Staple
Skitter, Crumb Blob, Lamp Moth, Tack Lobber) carries no faction tag at all,
regardless of how dense it gets. `pick_feuding_pair` requires the **top two**
factions by nearby weight to *both* be non-zero; both of my buys happened
standing inside one faction's own nest cluster (VOID-only, then SWARM-only),
so the second-ranked faction's weight was exactly 0.0 and the function
returned `None` — the Cores were still spent, nothing happened, and the
"NOBODY TO TURN" hint that should explain why was never seen in the log
(likely overwritten within the same tick by a recurring boss-warning hint).
**This is corroborated across the whole round, not just this run**: every
single row in `holdfast-runs.tsv` this round reads `wars 0`, including two
runs explicitly named `diplomat`. Whoever tries next needs to be standing
where **two different factions' actual nests or garrisons** are both within
close range simultaneously (not just two fort *markers* — their fed
monsters) at the moment of purchase — which is also the single most
dangerous kind of terrain in the game (see the VOID/RUST boundary
encirclement death in the log for this round). Bring a ring that can already
tank that density before trying it.

**Threat dial and Overclock, independently reproduced.** `=` raised threat
2.44 → 3.50 with `reward_mult` climbing to 3.46x at full HP, matching the
`effective_threat^0.92` formula already on record. `O` (Overclock) had never
been pressed by name before this round: it spiked threat from 3.50 to 5.92
(settling at 6.00) instantly, `reward_mult` to 8.6-8.8x, `surge: ON` for
almost exactly 22 real seconds before showing `surge: cooling` — matches the
task brief's own stated duration precisely.

**Numbers.** Run 2 (fort capture, died underlevelled while pushing toward it):
`warlord DESK 110.5 7 66 0 0 0 0 0 1.13 137 10791 127 4 0.159`. Run 3 (fort
taken then lost, six-weapon build, no ally alive at death): survived to
**level 43, 467 kills, 350.4s** — second-highest survival time recorded this
round, behind only `fortress`'s 374.4s, and reached with a much smaller
structure count (peaked around 4-5 vs. `fortress`'s 25), suggesting weapon
diversity/levels can substitute for raw structure count in holding a crowd.
Died when Tack Lobbers (ranged, first seen this run) plus two simultaneous
bosses caught the ring down to a single Barricade and took HP from 90% to
dead in about 6 seconds. Result:
`warlord DESK 350.4 43 467 1 0 0 0 0 3.50 137 20277 5928 67 0.492`. An earlier
attempt in between, holding a captured zone/fort combo at lower level, hit
**363.9s, level 24, 339 kills** before an encirclement-freeze death, the
round's second-best time at the point it was set:
`warlord DESK 363.9 24 339 2 0 0 0 0 2.54 312 40968 665 3 0.476`.

**Operational hazard for whoever plays next on a shared machine: other agents'
scripts `pkill -f "target/debug/holdfast"` without any instance filter.** This
killed my pilot process outright three times mid-run this round (once mid
level-up-card-pick, twice mid-command), each time with zero warning beyond
`state.json` silently freezing (`seq`/`wall` stop advancing even though the
process briefly still existed under CPU starvation from a concurrent
`cargo run` rebuild). If a command's result looks frozen for more than a
couple of real seconds, check `ps aux | grep holdfast` before assuming a game
bug — it may simply be dead, and the fix is the exact relaunch command from
the task, then `note strategy=...` again before continuing.

**Takeaway: all four assigned systems now have a real, numbered confirmation
in a live run — zone pay/decay, fort capture/loss with exact cause, ally
recruit/death-by-density, and a fully diagnosed (if not yet triggered) war
mechanic.** The one genuinely open item is question 4, ally-solo fort
capture — set two allies to Guard at an *uncaptured* fort and walk far enough
away that the player's own 1.0 presence can't be contributing, then check
whether `capture` still climbs. The war fix, for whoever tries it: hold a
ring capable of tanking 40+ enemies, walk it to a spot where two factions'
nests are both within ~30-40 units, and buy the node there.

### siege — 430.7s, new record, first fort capture by this instance (round 4)
Hypothesis: the travel-verb fix means a fort is finally reachable in a real
run. Build a modest arsenal near home, travel to the *nearest* fort (weakest
per the distance-scaling rule), clear its garrison from inside the ring, take
it, then see whether it can be held.

**Two early deaths reproduced the encirclement-freeze death spiral exactly,
underlevelled.** Attempt 1 engaged THE STAPLER boss plus a growing crowd
~70-140 units from home with only 3-4 weapons; density hit 16 within 12m and
`kite`/`flee` produced no positional change for 20+ real seconds while HP
free-fell 154→-10 in about 14 seconds. Died **141.5s, level 8, 63 kills**:
`siege DESK 141.5 8 63 0 0 0 0 0 1.30 137 17559 136 4 0.175`. Attempt 2 walked
into a BLOOM nest cluster (one nest at 3-10m) that kept the local crowd
resupplied faster than it could be cleared; same pattern, same boss, same
freeze. Died **124.7s, level 9, 74 kills**:
`siege DESK 124.7 9 74 0 0 0 0 0 1.19 153 11448 193 6 0.222`. **Both deaths
trace to the same root cause as every prior round's freeze reports: once
enemies-within-12m crosses roughly 10-15, `kite`/`flee`/`goto` stop producing
any positional change at all, regardless of build strength** — confirmed
this round at both a weak build (attempts 1-2) and later at an extremely
strong one (attempt 3, see below), so it is not a "you were undergeared"
problem, it is a mechanic (or a bug) that triggers on local density alone.

**Attempt 3 built a real arsenal before engaging, and it changed everything.**
Farmed cautiously (short `kite` bursts, breaking off whenever enemies-within-
12m exceeded ~10) until level 15-20 with 6 weapons (Pencil Dart, Laser
Pointer, Stapler, Ruler Sweep, Coffee Nova, Fan Blast), a full 4/4 squad, and
several hundred Scrap banked. Survived a density spike that would have killed
either earlier attempt outright: **137 simultaneous enemies (later peaking
past 200, with up to 12 elites and 2-3 bosses at once) and 88 enemies within
12m at the low point**, HP cratering to **12 out of 521** at one point and to
single digits again later — and recovered *both* times without dying, purely
by retreating ~50-70 units from the crowd and, in Plan Mode, building a small
turret ring (7 Tack Turrets + 1 Shocker) plus recruiting allies. **HP climbed
from single digits back to full (500+) within about 15-20 real seconds each
time** — the clearest confirmation yet that Plan Mode plus a cheap turret
ring is a genuine emergency brake, not just a planning tool, and that
retreat-and-rebuild beats trying to fight through an active density spike.

**Fort capture mechanics, observed in detail for the first time by this
instance.** Targeted a BLOOM fort at (134, 30.6), 137.5 units from spawn — at
the low end of the distance-scaling range and confirmed weaker than a
89-unit-further sibling fort (which showed `garrison: 19` on approach vs. this
one's baseline 0-4). Standing at 2-8m from the fort core with garrison > 0
held `capture` pinned at exactly **-1.0, "[taking 0%]"** for a sustained
period regardless of how long the ring was held — garrison genuinely stalls
the meter rather than slowly reversing it, matching the design doc. Garrison
on this single fort was observed swinging between **0 and 27** within a
~60-second window, evidently resupplied by four nests planted 4-40 units
away faster than they could be killed at moderate personal DPS. The one time
garrison held at a sustained 0 (after the ring had been fought over for
several minutes and the nests presumably exhausted), `capture` went from
**-1.0 to +1.0 in about 3 real seconds** (-1.0 → 0.461 → 0.605 → 1.0) — a
near step-function unlock once the garrison-stall condition lifts, not a
gradual climb. Also confirmed: **capture decays back toward -1.0 within
under 70 real seconds of leaving the ring uncontested** (was at -0.713 when
we retreated to rebuild HP; was back to -1.0 on return).

**The fort was taken. The player died in the same instant.** The exact tick
`capture` reached 1.0, the log read `FORT TAKEN: It works for you now - and
they will come for it.` and the fort's `owner` field flipped to `YOU` — and a
single 39-damage hit landed in the same tick, taking HP from 20 to -18.
`state` went `Playing → GameOver` one line later. So this run answers
"can a fort be taken" with a clean yes (confirmed independently by `warlord`
earlier the same round, so this instance is not the first — see the record
note above), but leaves "can it be held" exactly as open as before: there
was no surviving tick between the flip and death to observe the newly-owned
fort's own (weaker) gun firing for us, whether wardens stop spawning, or
whether allies/turrets left nearby would have defended it. Final result:
**430.7s, level 50, 1166 kills, forts=1**, holding 4079 Scrap and 8 Cores
unspent:
`siege DESK 430.7 50 1166 5 1 0 1 0 3.24 212 23643 4079 8 0.540`.

**Operational notes.** `goto` and `kite` both still "give up"/overshoot or
freeze in place whenever local density is high, exactly as documented in
prior rounds — this held true even at the strongest build tested to date, so
whoever automates travel next should plan to alternate short bursts with
density checks rather than trusting a single long-duration command near any
fort or nest cluster. Threat only reached 3.24 here (via floor drift over
431 seconds plus one held zone) — the dial itself was never touched
deliberately (a single `MINUS` press was swallowed mid-`LevelUp`), so
`reward_mult` climbing to 2.95-2.95x came entirely from surviving a long time
at a high level, not from pulling the escalation lever on purpose. `defend
x y r` reliably re-approaches and holds within `r` of a point even through
dense crowds once already in range, and is the right verb for standing in a
capture ring (as opposed to `goto`, which is for closing distance from far
away and is the one that stalls).

**Takeaway: build the arsenal first, then go to the fort, not the other way
around** — the same fort, same nests, same boss types that killed two
underleveled attempts outright were survived comfortably by a 6-weapon,
full-squad build, right up to and past the previous all-time density/kill
records. **Fort-taking is now proven twice over in one round; fort-*holding*
is the next frontier** — whoever tries next should assume the moment of
capture is dangerous (both `siege` and one `warlord` capture died at or near
that instant) and should have HP banked well above 50% and a turret/ally
presence already established *at* the ring, not just personal HP, before the
garrison count is allowed to hit zero.

### castellan — record broken: ~498s alive (not dead), level 67, 1312 kills, fort held repeatedly (round 5)
Hypothesis, handed down directly from `siege`: plant the turret ring *at* the
fort before the capture finishes, not fifty metres behind it, so the ring is
already there the instant the garrison hits zero and the meter flips.

**It works, completely, and the "moment of capture is dangerous" problem is
now closed.** Built a 6-weapon arsenal (Pencil Dart, Stapler, Rubber Band,
Tack Mines, Coffee Nova, Ruler Sweep, all levelled to 2-8) plus 3 gear pieces
near spawn first, exactly per `siege`'s recommendation. First fort approach
(a SWARM/RUST-contested one, garrison as low as 1-2) was taken and **the
player survived the flip for the first time ever** — HP dropped 199→143 in
the same tick the fort turned `YOU`, but did not hit zero. It was lost 11
seconds later to a different faction (BLOOM) entirely, because every turret
built so far was 40-95m away, left behind from an earlier camp — a direct,
lived confirmation of the "plant defences at the fort, not behind it" thesis.
Recaptured, built 3-6 Tack Turrets/Shockers **within 1-12m of the fort core
this time**, and from that point on the fort flipped ownership among
YOU/BLOOM/RUST/SWARM at least six more times over the next 150 real seconds
— and the player's HP stayed above 85% of max through essentially all of it,
including through a researched war, a self-triggered Overclock surge, and
finally a deliberate push of the threat dial to its 8.00 hard ceiling.
**A second fort (a different BLOOM one, 30m from the first, after the RUST
one ballooned to garrison 97 and was abandoned) was taken and held the same
way later in the run.** By the time the session ended the build was tanking
**330-580 total enemies, 60-330 within 12m, at threat 6.3-8.0, with the fort
owned**, at HP still 500-650 out of 650-670 max most of the time. This is a
different order of density from `fortress`'s "100+ enemies at threat 8" —
this run added a captured, actively-contested fort into that mix and held
both at once.

**Fort economics, source-verified, not just measured — the number the last
round asked for.** `src/forts.rs::fort_income` pays a held fort **2.4
scrap/s and 0.16 cores/s at baseline** (both × the player's income/core
multiplier stats) — the *same 2.4/s base rate as a single Generator
structure* for the scrap half, plus cores on top that Generators don't pay
at all. `src/threat.rs::effective()` is
`level + streak*0.5 + territory + holdings`, and
`src/forts.rs:705: threat.holdings = held * 0.35` — **holding one fort adds
a flat +0.35 to the effective-threat exponent's base, 75% more than a zone's
+0.2** (`allies.rs:962: threat.territory = held * 0.2`). So a fort out-earns
a zone (2.4 scrap/s + 0.16 cores/s vs. a zone's 1.6 scrap/s + 0.05 cores/s)
and out-costs it too (+0.35 threat floor vs +0.2). **Neither of these two
numbers is exposed by the pilot bridge.** `pilot.rs` reports `from_territory`
as `threat.territory` only (zones), never `threat.holdings` (forts) — so a
fort-holder reading `from_territory: 0.0` and concluding forts don't raise
the floor would be wrong; they do, through a field the bridge simply doesn't
print. Likewise the reported `scrap_per_sec` is `economy.scrap_rate`, which
`structure_income` (Generators) and the zone system both write to — but
`fort_income` calls `economy.gain_scrap`/`gain_cores` directly and **never
touches `scrap_rate` at all**. A held fort's 2.4 scrap/s and 0.16 cores/s are
real, landing in the bank every frame, and completely invisible in the
`scrap_per_sec` telemetry. Whoever next wants to *measure* a fort's income
has to diff `economy.scrap`/`economy.cores` over a controlled window with no
other income source active, not read the rate field.

**Reclaim assaults are categorically harder than ambient waves, and a war
does not relieve them.** The instant a fort flips, up to *four* factions
(not just the two adjacent ones) can enter `MassOnFort` posture
simultaneously — observed SWARM 64%, RUST 40%, BLOOM 40%, VOID 95% all at
once around one fort — pulling from every same-faction nest within ~40
units and producing 60-330 enemies within 12m, far past what ambient waves
alone produce at the same threat/level. Whisper Campaign was bought again
this round (confirmed: `wars: ["SWARM vs RUST (44s)"]` read directly from
`raw` state, not inferred) while three factions were already in `MassOnFort`
against the held fort — **the war fired correctly but the siege did not let
up**; all three postures stayed `MassOnFort` for the entire 44s. A war looks
like a pre-emptive tool to buy a fort assault cheaper before it starts, not
an emergency valve once four factions are already massing on you.

**Nests runaway-feed a nearby fort's garrison an order of magnitude further
than previously measured.** Camped near a RUST fort with 4 same-faction
nests within 20-40 units for about five real minutes and watched its
garrison climb from 1 to **97** — `warlord` (round 4) saw 0→27 off four
nests; this is roughly 4x that, over a longer sit. That fort was abandoned
as uneconomical rather than fought down; a second, cleaner fort nearby
(garrison 1-2, no nests planted) was taken instead. Lesson confirmed at
scale: check `nests_planted` on the fort itself before committing, not just
distance.

**Turrets are anchored to the world position you built them at — walking
away leaves the entire ring behind, dead weight.** This bit twice: after
building a ring, kiting 40-90m away in a level-up chain left every turret
useless (shown at `@40-220m` in the digest from then on) with nothing at the
new position. The fix is procedural, not a card: **rebuild a fresh 3-6
structure ring (Tack Turret ~22-25 scrap, Shocker ~30, Generator ~48, no
discount) at your *current* spot every time you relocate** — cheap enough
that this happened four separate times over the run without ever running
low on Scrap (ended with 19,544 unspent, 111 Cores unspent — the usual
"unspent resources" failure, at a scale nobody has banked before).

**Emergency ally recruitment breaks an active encirclement, reliably,
independent of density.** Twice this run, at HP under 25% inside 90-130
enemies within 12m with `flee`/`kite`/plain `hold W` all producing under a
metre of movement per real second (the classic freeze, reproduced again at
a new high density), recruiting 3-4 allies with repeated `tap R` was
immediately followed — same tick, no separate escape command — by an 8-12
unit position jump and enemies-within-12m dropping by roughly half. This
looks like allies momentarily absorbing/blocking the crush enough to open a
gap, and is worth its own line: **when stuck and dying, spend Cores on
allies before anything else**, not just for their DPS.

**Plan Mode's "WASD still walks" hint is not reliable escape under a
90+-enemy crush.** Holding W/A/S/D while `plan_mode.active` for a full
second moved the player 0.1-1.0 units, an order of magnitude less than the
expected ~1 unit/sec even accounting for the 12% time scale — almost
certainly body-collision blocking against the packed crowd, not the
slowdown itself. Real movement only resumed after exiting Plan Mode
(`SPACE`) and issuing `flee`/`kite` properly, or via the ally-recruit trick
above. Do not trust Plan Mode to double as a retreat.

**Operational: `quit` does not write a dossier row.** `Cmd::Quit` in
`src/pilot.rs` fires `AppExit` directly; the dossier only appends
`OnEnter(AppState::GameOver)` (`src/dossier.rs`), which a clean quit never
enters. This run ended by `quit` while very much alive (HP 525/670, level
67) specifically because it had already answered everything it needed to
and further play was diminishing returns — but that means **no row exists
in `holdfast-runs.tsv` for this run**, unlike every previous entry in this
file. Whoever wants a comparable row for a similarly dominant build should
either let threat/density actually finish the character off, or accept that
`quit`'s numbers have to be cited from the live digest instead (as here:
DESK, ~498s, level 67, 1312 kills, structures 3-9 fluctuating, allies 2-4
fluctuating, forts held repeatedly but not at the exact instant of quit,
peak threat 8.00, furthest 355, explored 42363, scrap 19544, cores 111,
coverage 60.3%). Also confirmed again: a bare `ENTER` sent during `Prep`
outside any modal doubles as "call wave early" — this run triggered two
unintended `WAVE CALLED +18%/+44%` events while trying to confirm a turret
placement, and the pilot's `!! REFUSED` list does not catch this collision
(only LevelUp/Research/PlanMode overloads are tracked).

**Takeaway: the fort-holding frontier is closed.** A fort can be taken,
survived at the instant of the flip, lost, retaken, and held through
Overclock and the threat dial's hard ceiling simultaneously, provided the
turret ring is built *at* the fort — inside its own capture ring — before
or during the capture, not after. The next open ground: nobody has yet
combined a held fort with a *deliberately incited war before the siege
starts* (this run's war fired mid-siege and did nothing for it); nobody has
tried holding two forts from two different factions at once; and the
"holdings" 0.35-per-fort threat contribution plus fort income are both
invisible to the pilot bridge and worth wiring up (`from_territory` should
probably become two fields, and `scrap_per_sec` should sum `fort_income`
too) so the next round doesn't have to grep the source to find them.
