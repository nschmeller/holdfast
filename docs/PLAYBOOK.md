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
were. The highest row actually written to `holdfast-runs.tsv` is now **511.7
seconds** (marshal, round 8, level 60, 944 kills, peak threat 3.74,
`forts=1`, also a deliberate healthy `quit` — since round 5's fix, `quit`
writes a row too). Up from 430.7s (siege, round 4, level 50, 1166 kills,
peak threat 3.24, `forts=1` — a fort captured, joining `warlord`'s two
captures earlier the same round). Up from 374.4s (fortress, round 3, level
34, 331 kills, peak threat 8.00 — the dial's hard ceiling, reached and held
with the player at full HP for the entire climb). Up from 337.7s (turtle,
round 2), up from a starting baseline of 148s. See `siege`, `fortress`,
`castellan` and `marshal` below.
**Fort-holding is no longer the open question either — `castellan` (round
5) took the same "plant the ring at the fort before it flips" idea `siege`
left on the table, and it closed the frontier completely: a fort was taken,
survived at the instant of the flip, lost, retaken repeatedly, and held
through both a researched war and the threat dial's 8.00 ceiling
simultaneously, for well over a hundred seconds of cumulative ownership.**
**Holding two forts at once is answered: yes, after the round-8 reclaim fix.
`viceroy` (round 9) held *three* forts simultaneously** — `threat.from_forts`
read `0.35`, `0.70` and `1.05` in turn as each one flipped, exactly `0.35×n`,
and `economy.scrap_per_sec_from_forts` matched `2.4×n` at every step (`2.4`,
`4.8`, `7.2`). Marshal's "no" was correct for the pre-fix build; the fix
(reclaim presses 42s then regroups 34s, instead of committing forever) is
what makes it possible now. The durability split is stark and now measured
precisely: a fort defended by 4+ Tack Turrets and nothing else survived
80+ real seconds fully unattended (player 100+ units away) before finally
reverting; a fort "defended" by 3 allies on `Hold` and zero turrets reverted
in roughly 20-35 seconds. Turrets outlast allies as unattended garrisons by
a wide margin. See `viceroy` below for the full numbers and a new technique
— building the ring from *outside* the fort's own capture radius — that
made repeat captures survivable for the first time this dossier.

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
- **Answered, mostly: is knockback beside a chasm strong enough to build
  around?** Yes on the mechanics, yes on early live measurement, unconfirmed
  on the specific "does it kill a boss" claim. See `sapper` below: fighting
  0.5-8m from a chasm rim held HP at/near max for entire multi-wave stretches
  (including a boss present) at 0.5-1.2 kills/sec, and the source code shows
  no enemy kind is exempt from knockback and the player cannot fall in
  (`Actor::avoids_chasms` defaults `true` and is only ever set `false` for
  `Enemy`). Nobody has yet watched a specific boss actually go over the edge
  on camera — the enemy-count telemetry can't distinguish a fall-kill from a
  DPS-kill, so that specific sub-claim is still open for whoever plays next.
- **Answered: do light pools pay for the attention they draw?** Yes, but the
  mechanism is not the one anyone expected. See `sapper` below: kills/sec
  inside a pool was not clearly higher than outside (1.0-1.55/s inside vs.
  2.0/s in one outside control, muddied by weapon-level confounds), but HP
  stayed pinned at or near 100% every window recorded inside a pool, while an
  outside-control window at comparable density lost 31% of max HP in 6
  seconds. Source dig explains it: `player_regen` in `src/player.rs` adds a
  flat **+1.4 HP/s** while standing in any `LightPools::contains` area, on top
  of the 1.25x damage multiplier the pilot already reports — and that regen
  bonus appears nowhere in the in-game hints or the pilot's `raw` output, only
  in a source comment. Also checked and found absent: no code anywhere
  (`enemy.rs`, `director`) actually pulls enemy aggro toward light pools —
  the "draws attention" framing does not correspond to any targeting-weight
  system found in this codebase; enemies already beeline the player
  regardless of light.
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

**Fixed after round 9:**

- **Bodies take a fort; turrets keep it.** Round 9 captured a fort from 20 metres
  away by placing turrets on its ground from outside its 15-unit gun range — the
  build cursor reaches 26 — bypassing the guns, the wardens and the contest
  urgency, which is the entire difficulty of a fort. Structures no longer count
  towards *taking* one, only towards keeping one. **So there are exactly two ways
  to capture: stand in the ring yourself, or send allies.**
- **A conquered fort is remembered across chunk unloading.** Forts carry
  `ChunkEntity` and were despawned at 120 units, then rebuilt from the world seed
  enemy-owned — so walking away from a siege you had won silently undid it, and an
  empire wider than 120 units was impossible by construction. Note honestly: the
  fix is unit-tested and the mechanism was read directly, but nobody has yet
  watched it work in play, because a fort left undefended is legitimately
  reclaimed within a minute.
- **`garrison` now reports whoever opposes the fort's current owner.** It counted
  only the owner's own loyalists, so it read zero for every fort the player held
  while the rivals pushing it back were invisible. A falling fort had no reported
  cause.
- **A steering verb's clock stops while the game does.** A `flee 20` issued just
  before a level-up used to burn its twenty seconds doing nothing and report that
  it had fled — which cost round 9 two deaths outright.

**Fixed after round 8 — two of these change how the whole game plays:**

- **Monsters more than 165 units away are now released.** Nothing was ever culled,
  so every monster a run passed followed it forever and travel dragged an
  accumulating horde. That is the attrition spiral that ended nearly every run in
  the dossier, and it made the game's own advice - go out past 130 units - into a
  trap. Bosses are exempt. **Wide exploration should now be viable; it was not
  before, and any measurement of travel from before round 8 is void.**
- **A reclaim now lifts.** Up to four factions committed to a held fort's ring
  permanently, because a player-held fort is always the most attractive prize on
  the board. A faction that presses for 42 seconds without progress regroups for
  34. **Holding a fort unattended should now be possible; nobody has managed more
  than 90 seconds, and that was why.**
- **Standing in the light draws elites 2.2x faster.** The pool's +0.45 threat was
  measured as about half of what standing in one is worth, so it now has a cost
  that mounts the longer you stay.
- The digest reports `frames_per_sec` (it shouts under 30) and
  **`game_time_frozen_for`** - every `GameSet` is gated on `AppState::Playing`, so
  the run clock genuinely stops on a card screen. Two `raw` reads 74 wall-seconds
  apart can return an identical `t`. A speed measured across that gap is zero
  movement in zero time, which looks exactly like a dead movement system; it cost
  round 8 six measurements. **Check that line before believing a zero.**
- `Stance::Hold` garrisons allies at a fort. `Guard` only targets zones.

**Fixed after round 5:**

- **Light pools and chasms are finally visible.** They were never reported by the
  bridge, which is why five rounds called them untested. `raw` now lists both,
  nearest first: pools with `standing_in_it` and `damage_mult_inside`, chasms with
  `to_edge`, since knockback beside a hole is worth far more than knockback in the
  open. On the default desk seed there is **a chasm 36 units from the landing site
  and a light pool at 91.** They were always right there. Nobody has used either.

- **A run that ends by `quit` now writes a dossier row.** Round 5's run - the
  best yet, ended deliberately while healthy - left no row at all, because the
  dossier only appended on `GameOver`. Both endings write exactly one row now.
- **A held fort's books are visible.** `raw` reports `threat.from_forts` (0.35
  per fort, against 0.2 for a zone) and `economy.scrap_per_sec_from_forts`. Fort
  income used to be missing from `scrap_per_sec` entirely.
- The three numbers a fort pays are named constants in `src/forts.rs` now:
  `FORT_SCRAP` 2.4/s, `FORT_CORES` 0.16/s, `FORT_THREAT` 0.35.
- Plan mode's hint no longer implies walking is at full speed in there. It is at
  plan pace like everything else - which means escaping in plan mode is exactly
  as effective as escaping in real time, since the monsters are slowed too. It
  just takes longer in wall-clock.

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

### diplomat — 502.6s, new record for a run that actually died; war/territory/allies, not forts (round 4)
Assignment: the three systems nobody had exercised that are not forts —
faction wars, territory at scale, allies — deliberately leaving forts alone
as scenery for the parallel `siege`/`castellan` instance. Five attempts on
the pre-fix binary (all died in the 175-360s range without a war ever
firing — see below), then the coordinator's three-fault fix landed mid-session
and a fresh instance on the patched binary answered every question cleanly.

**Before the fix, the war genuinely never fired, confirmed the hard way.**
Two separate purchase attempts (Whisper Campaign, 14 Cores) on the pre-fix
binary both left `wars: []` and produced *no* hint at all — not even
"NOBODY TO TURN" — because the `InciteRequest` message was written while the
research screen (`AppState::SkillTree`) was open and `resolve_incitements`
only runs in `GameSet::Think`, gated to `AppState::Playing`; by the time the
screen closed, several real seconds and many SkillTree-only Update frames
later, Bevy's message double-buffer had already dropped it. Buying and
closing in the same input batch didn't help — the batch itself spans enough
frames while paused for the message to expire before `Playing` resumes.
This is the same fault the coordinator's notice independently diagnosed;
recorded here as an independent before/after confirmation, not a duplicate
report.

**After the fix: is a war pressure relief or an XP thief? Pressure relief,
overwhelmingly, measured continuously through one full 45-second window.**
Bought Whisper Campaign at level 22 (14 Cores, HP 290/342, 40 enemies within
12m) standing at a BLOOM/RUST boundary with `war_available: "BLOOM vs RUST"`
already showing in the digest before purchase. Immediately after buying
(still paused in `SkillTree`, `wars: ["RUST vs BLOOM (45s)"]` read from
`raw` the instant the purchase completed — no waiting, no ambiguity) and
closing the screen, hint fired: "THE BLOOM TURNS ON THE RUST: They are not
looking at you." Sampled every ~5-10s from there:

| t (s since buy) | enemies within 12m | HP / max | RUST posture | BLOOM posture |
|---|---|---|---|---|
| 0  | 48-52 | 284/342 (83%) | HuntPlayer(0%) | HuntPlayer(0%) |
| 9  | 18-20 | 293-302/357 (83-85%) | MassOnFort(40%) | MassOnFort(40%) |
| 20 | 0 | 320/357 (90%) | HuntPlayer(0%) | MassOnFort(40%) |
| 23 | 0-3 | 357-372/357-372 (100%) | HuntPlayer(0%) | MassOnFort(40%) |
| war end (45s) | 4 | 401/439 (91%) | HuntPlayer(0%) | HuntPlayer(0%), reverted |

Kills went 287 → 308 → 345 → 442 → 461 over the same window (both factions
were killing each other, not just standing down), HP never dropped again
until the war had fully expired and the player wandered back into ambient
density, and both factions' `posture` flipped to `MassOnFort` simultaneously
at commitment 0.4 each — they redirected at each other, not just away from
the player. **Buy it the moment `war_available` shows a pair you can see,
even mid-fight; the 14 Cores paid for themselves in the first 10 seconds and
the rest was free HP regen with the enemy AI doing the work.** The `nearby`
field the coordinator added is real and moves with the fight: BLOOM's went
91.5 → 79.7 → 27.6 → 25-33 across the same window, tracking its forces
being pulled away and thinned by RUST, not just relocated.

**Opening Research is a free panic button, not just a menu.** `AppState::SkillTree`
sits outside the `GameSet::*` chain entirely, so *no* damage, no enemy
movement, and no HP/enemy-count field changes at all while it's open —
confirmed directly: two `raw` reads taken ~15 real seconds apart while
paused in SkillTree (deliberately, to test this) returned byte-identical
`t`, `hp`, and `enemies.total`. At HP 34/394 with 71 enemies inside 12m and
no cores to spend, opening research anyway still buys unlimited real-world
thinking time with zero in-game cost — worth doing purely to plan an escape,
independent of whether you can afford anything on the tree.

**Hold four zones at once with turrets, not allies or the player — the
`from_territory` field confirms it holds even from 90m away.** Travelled a
loop placing one Tack Turret directly on each of four zone markers (25
scrap each, no discount used); all four flipped to `Player` and stayed there
after the player moved on. Once 90-100m from every one of them, `raw` state's
`zones` array (capped to the 5 nearest) stopped listing any of the four —
but `threat.from_territory` still read exactly `0.8` (= 4 × 0.2), proving
the zone-hold and its threat contribution are tracked globally, not by
render/query distance. `effective_threat` went from 1.358 (no territory) to
2.158 (4 zones held): `reward_mult` 1.32 → 2.03, a **54% reward increase**
for 100 Scrap spent once and never revisited, against a `spawn_mult`
increase from 1.43 to 2.00 (+40%) and `power_mult` from 1.10 to 1.32 (+20%)
— both computed directly from the same `effective()` this reward multiplier
reads, since neither is exposed by the pilot bridge either. A fifth zone was
captured shortly after by a lone Guard-stance ally (see below), for
`from_territory: 1.0` and `reward_mult` reading `2.117` at threat 1.459,
matching the formula to three decimal places again. **The zones=5 row this
produced (`diplomat DESK 241.3 23 194 4 2 5 0 0 2.87 ...`) is the first time
this game has recorded more than one zone held at once in the dossier.**

**Allies hold zones solo and travel there unassisted, but die there alone
at high threat without a turret backup.** Recruited a mixed 4-unit squad
(Scout/Gunner/Bulwark/Medic or similar) several times this session; `G`
pressed twice (Follow→Hold→Guard) sends every ally to a *different*
currently-unowned zone (`handle_recruit`'s target list is filtered to
`owner != Player` and indexed by ally slot, so a 4-ally squad claims up to
four distinct zones with one keypress, no manual routing). Watched it
succeed clean at threat ~1.4-1.6 (a lone ally captured and held a zone 17-46m
away while the player fought elsewhere, `scrap_per_sec` jumping 1.6/s per
zone exactly on schedule) and fail slowly at threat 2.2-2.6 in a later run:
a Drill-Sergeant-boosted Bulwark with 885 max HP, sent alone to Guard a
distant zone, still bled down from full to dead (885→638→312→142→0, squad
count ticking 4→3→2→1→0) over about 90 real seconds of ambient pressure with
no player support nearby — a turret at the same threat, by contrast, has
been shown elsewhere this round to hold indefinitely once built. **Allies are
a legitimate zone-capture tool (fast, no build action, no scrap) but not a
zone-*holding* tool once threat passes roughly 2; use them to flip a zone,
then backfill with a turret if you want it to stick.**

**Fatal pattern, twice this session before the fix and once after: leveling
up mid-`do`-batch swallows a keystroke meant for something else, most
dangerously the ones that would have opened an escape.** Confirmed via the
sticky `!! REFUSED THIS RUN` list every time: `Digit3`/`KeyT`/`Enter` sent
while `LEVELUP` was actually open got read as level-up keys, several calls
in a row, while HP kept dropping in the background because the level-up
screen itself does *not* pause damage the way SkillTree does (a level-up in
a crowd is not free time; only Research is). Two of this session's three
deaths trace to exactly this: an intended card-pick that was actually a
build/research keystroke arriving one tap late, followed a few seconds later
by a death that had already been decided before the correct card was ever
seen. Lesson: after any `do` call that might straddle a level-up, always
check `state` before trusting the next keystroke lands where intended — the
digest can look identical (`[LevelUp] ... CARDS ON OFFER`) whether it's the
same level-up still open or the game already moved on and reopened another.

**Fast batched `DOWN` taps into the research tree can silently under-count.**
Attempted to buy Blood Feud (Command row 6) via one batched
`RIGHT×3 DOWN×6 ENTER`; the purchase went through (26→22 Cores spent ≈4,
`wars` stayed `[]`) but landed on row 2 (Supply Lines, ~4 Cores) instead —
at least 4 of the 6 `DOWN` taps were dropped somewhere in the batch despite
the earlier `RIGHT×3` in the same call having worked correctly one screen
prior. Cross-check the digest (or at minimum the Cores delta) after any
multi-tap tree navigation before trusting the cursor landed where the RIGHT/DOWN
count says it should have; a silent under-buy costs Cores without giving you
the node, or the war, you paid for.

**Light pools and chasms exist in the fixed `desk` arena (this is not an
infinite-world-only feature) but are invisible to the pilot bridge.**
`src/environments/desk.rs` rolls a light pool next to the desk lamp prop at
an 11% feature chance (`c.pool(...)`) and a chasm in the gap between two
desks at 9% (`c.chasm(...)`) — both exist in every "desk" run, not just the
WIP infinite-world branch. Neither `LightPool` nor `Chasm` is reported
anywhere in `state.json`; `grep -n "pool\|chasm" src/pilot.rs -i` returns
nothing. Screenshot capture failed on every attempt this session (flat
56997-byte PNGs, the known capture race), so there was no way — visual or
telemetric — to confirm proximity to either across five full runs and one
502-second run touching five separate zone clusters. **This remains
completely untested, and will stay untestable for a keyboard-only,
screenshot-unreliable driver until the pilot bridge reports a distance to
the nearest light pool and chasm the way it already does for zones, forts
and nests.**

**Fort contamination is easy by accident even when explicitly avoiding
forts.** Wandering (via `flee`/`goto`, never deliberately) put the player
within a fort's capture ring three separate times this session purely
because "away from the crowd" and "toward the nearest fort" pointed the same
direction; one of these triggered a `[taking 45%]` contest and a warden
assault that took HP from 366/394 to a near-death 41/449 in under 20
seconds — a direct, unwanted replay of `castellan`'s "reclaim assaults are
categorically harder" finding, encountered by someone who was actively
trying not to go there. **If forts are explicitly out of scope, check the
`FORTS` distance line before every `flee`/`goto` call, not just before
`goto`-ing somewhere on purpose** — the steering verbs do not know or care
that a fort is a hazard you'd rather not touch.

**The final run: 502.6s, level 42, 871 kills, peak threat 3.30 — the highest
dossier row (a run that actually died) recorded so far**, beating `siege`'s
374/430s marks. Reached via disciplined level-1 farming near spawn with
constant retreat at the first sign of density (never letting `within_12m`
exceed ~20-25 before disengaging), landing every level-up card decisively,
and treating both the 45s Whisper Campaign window and the free SkillTree
pause as scheduled rest stops rather than emergency-only tools. Zeroed out
on zones/allies/forts/structures at the moment of death only because the
run's final ~120 seconds were spent purely on survival after the last ally
died and cores ran short of a second war purchase — the peak state, sampled
mid-run, held 5 zones, 4 turrets, and a 4-ally squad simultaneously. Died to
the same pattern as every other run this round: HP crossed roughly 30% while
`within_12m` was already past 40, and the gap between "still fine" and "dead"
was under 15 real seconds with no recovery window once it started, even at
threat 3.3 (a much lower threat than `fortress`'s 8.0 collapse) — the density
ceiling that kills a build is a function of raw enemy count near the player,
not threat level directly, and 300+ ambient enemies at threat 3.3 is already
past it.

**Takeaway for whoever plays next:** the war/territory/allies frontier is
now closed to the same degree the fort frontier was closed by `castellan` —
buy a war the instant `war_available` shows a pair, garrison zones with
turrets rather than allies for anything meant to last, and use allies as a
capture tool, not a holding one. Open ground: nobody has combined an incited
war with a *held territory ring* (does BLOOM/RUST fighting each other reduce
pressure on turrets the way it does on the player?); nobody has bought
Blood Feud successfully yet (every attempt this session either lacked the
26 Cores or landed on the wrong node — its 110s window against a
`nearby`-tracked fight is still an open measurement); and light pools and
chasms remain the only two systems in the entire game nobody has ever
observed, for lack of a bridge-side distance field rather than for lack of
trying.

### sapper — 214.9s, first live look at chasms and light pools (round 6)
Assignment: terrain as a weapon. The bridge now reports `light_pools`
(`standing_in_it`, `damage_mult_inside`) and `chasms` (`to_edge`), fixed
specifically so this round could look at what every prior round could only
theorize about. Three questions: is knockback beside a chasm a build, do
light pools pay for the attention they draw, and does stacking both with the
threat dial compound into the strongest position in the game.

**Source-code findings, checked before spending any playtime, because they
answer "does this even work" faster than combat does.** `grep -rn
"\.by_player" src/*.rs` returns nothing outside the two write-sites in
`combat.rs`/`enemy.rs` — `DeathEvent.by_player` (set when a falling enemy was
hit by the player within the last 2.5s) is computed and stored but **never
read anywhere**, meaning `handle_deaths` in `pickups.rs` pays full XP/Scrap/
Core reward for a chasm death whether the player pushed the enemy in or it
simply wandered in on its own. The in-source comment on `enemy_fall_off`
claims self-walked chasm kills were deliberately made to *not* pay, citing a
past exploit that "reached 16.6 kills a second" — but nothing in the current
code enforces that; if the guard exists it isn't here. Separately,
`enemy_think`'s obstacle-avoidance only steers around `ObstacleField` (props),
which chasms are never added to, and `Enemy`'s `Actor::avoids_chasms` is
explicitly `false` ("deliberately careless") — so enemies beeline straight
through/into a chasm if it sits between them and the player, with zero
special-casing. The player's own `Actor::avoids_chasms` is the type default,
`true`, and nothing ever overrides it for `Player` — the player cannot be
knocked into a hole, full stop, so testing at the very rim (down to 0.5m to
the visible edge) carries no self-death risk. And knockback force itself
(`apply_damage` in `combat.rs`) is `knockback_force * stats.knockback` where
`stats` is the *player's* `PlayerStats` for every non-player target — there is
no per-enemy-kind knockback resistance anywhere, so mechanically a boss is
exactly as shoveable as a Dust Bunny, just proportionally harder to move
per-hit because nothing in the impulse system scales by body size either.
Catalogued weapon knockback forces for whoever wants to build around this:
Fan Blast `34 + level*4` (explicitly commented "shove things off the edge" —
this is the designed chasm weapon), Coffee Nova `18`, Ruler Sweep `14`,
Stapler `~11.2`, Pencil Dart/Rubber Band `~8` (baseline multiplier), Clip
Orbit `7`, Laser Pointer `4` (weakest, a precision weapon not meant to push).

**Live chasm test.** Farmed to a 5-6 weapon build (Pencil Dart, Stapler, Tack
Mines, Coffee Nova, Fan Blast, Heavy Hands' +25% knockback card) then used
`kite` — not `goto`, which kept stalling/giving up short even at modest
density, see operational notes below — to settle 4-8m from a chasm (down to
0.5m from the visible edge at closest). Held that ground with `defend` through
several waves and a full encounter with THE STAPLER boss. Measured across
four ~10-20s windows: kills/sec ranged **0.49 to 1.19**, and **HP stayed at
or within 3% of max in every single window** (168-210 HP band, out of a
similar max) even as local density climbed to 20-28 enemies within 12m and 1
boss present concurrently. This is a genuinely strong result — no other
strategy in the playbook has held that HP-flat a line at that density this
early (level 9-14) without a turret ring. **What I could not confirm: whether
the boss itself ever actually fell in**, as opposed to being ground down by
DPS — THE STAPLER remained alive (count still 1) at every check across an
80-second stretch near the pit, and the aggregate `enemies.total`/`by_kind`
telemetry can't distinguish a fall-kill from a melee-kill for a specific
entity. Given the source-level proof above (uniform knockback, no boss
exemption), the mechanical claim stands, but nobody has watched it happen to
a named boss yet — that needs a slower, more deliberate lure-to-the-lip test.

**Live light pool test.** Found via `goto`/`kite` toward the reported pool
coordinates; confirmed the desk arena's terrain is chunk-generated on the
fly (`src/environments/desk.rs`: 9% chasm / 11% pool chance *per chunk*, not
two fixed instances) — new pool/chasm pairs kept appearing as I explored,
including one only ~12 units apart at ~65-90 units from spawn, much closer
than the ~36m/~91m pair visible at spawn. Measured kills/sec standing in a
pool across three windows: 1.25, 1.55, 1.0 per second, versus **2.0/s** in one
outside-the-pool control window at comparable density — so the pool did *not*
clearly raise kill rate (confounded by weapon levels changing between
windows, so take this as suggestive, not clean). The much cleaner signal was
survivability: **HP held flat at 100% of max in every window recorded while
standing in the pool** (across density climbing 26→51 enemies), while the one
control window taken ~11-12m outside it saw HP fall from 149 to 103 (**-31%
in 6 seconds**) at similar-or-lower density. Source dig explains the gap:
`player_regen` in `src/player.rs` adds a flat **+1.4 HP/s** while
`LightPools::contains(player.pos)` is true, on top of the 1.25x damage
multiplier the pilot already surfaces — and this regen bonus is in neither
the in-game hint text nor the pilot's `raw` output, only a source comment.
Also checked: `grep -rn LightPools src/enemy.rs` returns nothing — no code
anywhere pulls enemy aggro toward a pool. The "draws attention" framing in
the design doc doesn't correspond to any targeting-weight system that exists
in this codebase; enemies already beeline the player regardless of light, so
a pool's cost is exactly its price of admission (walking there), not some
extra lure effect.

**The compound: pool + rising threat dial.** Stood in a pool and tapped
`EQUAL` (not `=` — the pilot rejects the literal glyph, `tap MINUS`/`tap
EQUAL` are the actual key names) repeatedly, pushing threat 1.0 → 1.7 while
density climbed to 116 total enemies, 37 within 12m, 3 elites + 1 boss
concurrently. Held HP at 85-95% of max for a genuinely long stretch (roughly
60-90 seconds) at that density and threat — reward_mult climbing to 1.6-1.7x
on top of the pool's own multiplier and its hidden regen. **It is not free,
though, and the reason is instructive rather than surprising**: this
particular pool happened to sit 2-9m from a cluster of 5-6 active BLOOM
nests, and once threat and density stacked high enough, that nest cluster
fed faster than the (mid-tier, mostly L1-L2) build could clear even with the
pool's bonuses — HP cratered from 85% to 30% in about 15 seconds. `flee`
resolved it cleanly (full separation and +65 HP recovered in 8 seconds), so
the position itself wasn't a trap, but it shows **a light pool amplifies
whatever position you're already in — it doesn't fix a bad one** (nests
nearby are still nests nearby). The run ultimately died about a minute later,
away from the pool entirely, to the exact same encirclement-freeze pattern
documented every round since the first: `within_12m` crossed into the
30s, HP went 156→-3 in under 45 real seconds with one `LevelUp` gap in the
middle, and the final `flee` (HP 61→-3) landed 2.5 seconds too late once the
tick rate hit 14-23 damage every 0.4-0.6s. The terrain compound raises the
*ceiling* of survivable density substantially over open ground — it does not
repeal the encirclement ceiling once local density crosses somewhere in the
low-to-mid 30s within 12m.

**Numbers.** Died **214.9s, level 19, 288 kills, best streak 204, peak
threat 1.78** (dial pushed to ~1.7 plus streak/floor drift), zones/forts/
allies/wars all 0 (never attempted — the assignment was terrain, not
territory), 397 Scrap and 8 Cores unspent at death. Result:
`sapper DESK 214.9 19 288 0 0 0 0 0 1.78 116 12996 397 8 0.238`.

**Operational notes for whoever drives next.** (1) `tools/pilot.py`'s `do`
only waits out `hold`/`wait`/`tap`/`shot` durations (`duration_of` in
`pilot.py` only sums those four verbs) — a `defend x y r 60` or `kite 20`
returns from `do` almost immediately after the command is queued, *not*
after 60/20 seconds of play. Follow every steering verb with a real
(bash-side) `sleep` if you actually want it to run its course before the
next check; otherwise consecutive `do` calls just re-read the same instant.
(2) The threat dial keys must be sent as `tap MINUS` / `tap EQUAL` (`PLUS`
also maps to Equal) — a literal `tap "="` or `tap "-"` is rejected with
`rejected: unknown key`. (3) This instance's `holdfast` process was
**silently killed twice this session with zero error output or crash log** —
`ps aux` simply stopped listing it between one command and the next, no
`AppExit`, no panic in `stdout.log`. This happened while 4-5 other agents'
`holdfast`/`cargo` processes were alive concurrently on the same machine
(physical memory was down to ~1.5GB free of 24GB at the time) — almost
certainly the OS silently reclaiming a backgrounded/occluded GUI process
under memory pressure, not a game bug. Relaunching with the identical env
recovered cleanly and reproduced the *exact same* chasm/pool coordinates
(the desk seed is deterministic), so no measurement was lost, just time.
Whoever configures concurrent rounds on shared hardware should expect this
above 4 simultaneous instances and budget for at least one silent relaunch.

**Takeaway: terrain is a real, working build, not scenery.** A chasm rim
sustains combat at flat-100%-HP density levels that open ground cannot, and a
light pool's real value is hidden regen, not the advertised damage bonus.
Both compound with the threat dial for a genuinely higher ceiling than either
alone — but neither is a substitute for reading local density, since the
same encirclement-freeze wall that ends every other strategy in this file
still ends this one once `within_12m` climbs into the 30s. Next: (1) a
patient, deliberate test that lures a *named* boss to the very lip and
confirms on camera (or via a tighter enemy-count diff) that it actually falls
rather than dies to DPS; (2) build a turret ring *at* a chasm rim the way
`castellan` built one at a fort, and see whether turret presence plus
knockback pushes the ~90-within-12m collapse ceiling higher than either
system alone; (3) deliberately seek out one of the closer chasm/pool pairs
(they exist, ~12 units apart was found this round) and fight from the
single tile that is simultaneously in the pool and at the rim, rather than
treating them as two separate stops the way this round did for lack of time.

### tourist — world tour, all five worlds played for the first time in one session (round 6)
Assignment: every previous round (eleven runs, five rounds) played DESK and
DESK only. Tour all five worlds, drive `pilot.py todo`'s coverage checklist
down, and answer the one question nobody could: is a world mechanically
different to play, or only differently painted?

**Coverage went 0% → 51% (32/63) in one continuous session, touching every
world.** `EnvKind::COUNT=5`: Desk ("THE DESK"), Forest ("THE UNDERGROWTH",
short name `WILD`), Rooftop ("BLOCK 9 ROOFTOP", `CITY`), Grid ("GRID ZERO"),
Arcane ("THE ARCANE SANCTUM"). All five were deployed into, fought in for
several minutes each, and left via the menu rather than by dying, so a single
process's `Coverage` resource (in-memory, per-run-session, not per-world)
accumulated across all of them. Final death was in a second Forest visit:
**306.5s, level 30, 317 kills, peak threat 2.20**, killed by THE STOMPER
(BossHolePunch's Forest name) landing three shockwave hits in under a second
immediately followed by wave 6 arriving at 11 HP — a clean, specific cause,
not the usual undifferentiated crowd-freeze. Result:
`tourist-worldtour-forest2 WILD 306.5 30 317 1 1 0 0 0 2.20 389 40086 1034 9 0.508`.

**Answer to the central question: the weapon/monster/turret layer is a pure
reskin; the terrain/hazard layer is not.** Checked both claims against source,
not just play:

- `WeaponKind::name(env)`, `EnemyKind::name(env)` and the turret equivalent in
  `allies.rs` are honest lookup tables indexed `[kind][env]` — ten weapons,
  twelve monsters and five turrets each get five real names (Pencil Dart ->
  Rivet Gun -> Mote Bolt etc.) with **identical underlying stats** (`base()`
  in `weapons.rs` takes no `EnvKind` parameter at all). A Rooftop Rivet Gun
  and a Desk Pencil Dart are the same weapon in every number that matters.
  This is deliberate and it works — confirmed by pulling every weapon
  archetype across the five worlds this session (Pencil/Rivet/Pulse/Mote/
  Thorn Dart, Coffee/Steam/Plasma/Mana Nova, etc.) and never seeing a stat
  discrepancy.
- **The terrain each world generates is genuinely, numerically different**,
  confirmed by reading all five `environments/*.rs` hazard/pool blocks:
  light pools are **1.25x damage in Desk/Forest/Rooftop but 1.30x in Grid and
  Arcane**, with radius varying 5.5-6.5 (`c.pool(p, radius, damage_bonus)` -
  grid.rs:70 uses `(5.5, 0.3)`, arcane.rs:64 uses `(6.5, 0.3)`, the other three
  use `0.25`). Grid's signature hazard, "plasma conduits" (grid.rs:129-155),
  is a **fundamentally different shape and threat level from every other
  world's terrain hazard**: a line (not a circle, built from overlapping
  discs along a rolled direction), 26 DPS (the highest of any world's terrain
  hazard), pulsing on a 4.5-6.5s period at 40% uptime — you have to read a
  rhythm, not just avoid a spot. Rooftop's steam vents are circular, 18 DPS,
  35% duty. Forest's mud is pure slow (0.38-0.52x, **zero damage**) with a
  rare separate 5-DPS stinging pool — matches its "mud slows everything"
  quirk exactly. **Arcane's ley hub is the most mechanically distinct hazard
  in the entire game**: `HazardKind::Font`, **negative DPS** (-7 on the
  spokes, -14 at the hub) which the damage pipeline reads as *healing*, and
  the source comment says outright it heals "friend or foe" — matching the
  quirk text ("Ley lines heal whoever holds them. So the enemy wants them
  too.") precisely. I did not personally stand in one this session (16%
  per-chunk chance, never rolled one in range) but the mechanic is real and
  sourced, not aspirational text.

**Two concrete mis-themed bugs, precisely the class the user originally
flagged.**
1. `onboarding.rs:164`, `reset_onboarding`: the very first hint of every run,
   in every world, is the hardcoded string `"HOLD THE DESK"` / `"WASD to
   move."` — confirmed live in Forest, Rooftop, Grid and Arcane, all showing
   `HINT HOLD THE DESK` in the digest despite the world clearly being none of
   those things. Every other onboarding hint (`SALVAGE ONLINE`, `TERRITORY
   CONTESTED`, etc.) is generic enough not to leak, but this one names the
   wrong world outright, on line one of every run that isn't Desk.
2. `WeaponKind::blurb()` (`weapons.rs:144`) is a single flat string per
   archetype, **not indexed by `EnvKind` the way `name()` is** — so
   ClipOrbit's card always reads "Paperclips orbit you and shred whatever
   touches them" even when the weapon is drawn as "Hornet Ring" (Forest),
   "Bolt Orbit" (Grid) or "Orbiting Sigils" (Arcane); CoffeeNova is always
   "Scalding ring..." even as "Mana Nova" in Arcane; RubberBand is always
   "Ricochets off walls and props" even as "Whip Vine" in a forest with no
   walls. Confirmed live: picked up "Orbiting Sigils" in Arcane and the card
   text still said paperclips. This is the *exact* "pencil darts in the
   forest" failure mode, just relocated from the weapon's name (which is
   fixed) to its description (which isn't).

**A related, non-bug but notable gap: gear is not reskinned at all.**
`GEAR_NAMES` in `progress.rs` (Paper Crown, Tape Wrap, Lucky Clip, etc.) has
no `EnvKind` dimension whatsoever — a "Tape Wrap" body piece with desk
flavour dropped in Forest, Rooftop, Grid and Arcane alike this session. The
task brief only called out monsters/weapons/research as themed-per-world, so
this isn't a violation of spec, but it's the one visible-to-the-player system
the reskin pass never reached, and it shows: gear is the one card-offer
screen every run that still says something desk-specific regardless of
world.

**The coverage checklist itself cannot reach 100%, and the reason is in the
source, not bad luck.** `grep -rn "Seen::of(\"hazard\"\|coverage::Seen(format!(\"hazard\"" src/*.rs`
returns nothing — of the four `hazard:*` entries in `coverage::expected()`
(`Scald`, `Sticky`, `Shock`, `Font`), **none has a writer anywhere in the
codebase**. `coverage.rs`'s own `note_milestones` comment records that a
prior fix already found and patched ten dead deed-writers ("the checklist
was lying to whoever used it") but the hazard category was evidently missed
in that pass — it is still exactly as dead now as the deeds were before that
fix. Worse for `Shock` specifically: `grep -n "HazardKind::Shock"
src/environments/*.rs` returns **zero matches** — none of the five world
generators ever places one, so it is unreachable twice over (no writer, and
nothing to trigger it even if there were one). **Maximum achievable coverage
given the current source is 59/63 = 93.65%, not 100%** — worth fixing
(wire a `Seen::of("hazard", ...)` into whatever system already applies
`HazardKind` dot-damage/slow to the player, `combat.rs` almost certainly) so
the next round's checklist stops lying about four items nobody can ever
check off.

**Operational notes for whoever plays multiple worlds next.**
- **`Coverage` is a Bevy `Resource`, held in the running process's memory —
  killing and relaunching the binary resets it to 0%, even though the world
  select carousel and everything else look identical.** This cost real time
  this session: an early instance had to be killed (see below) and its
  Forest coverage was gone the moment the replacement launched. The fix:
  **never quit between worlds** — from `GameOver`, press `ESC` to return
  straight to `Menu` (no restart); from mid-run, `ESC` (pause) then
  `BACKSPACE` ("abandon the run") does the same. Both keep the same process
  alive and its `Coverage` resource intact, confirmed directly (persisted
  8%→100%+ across four world switches with no drop).
- **Two of this session's early `holdfast` processes looked hung** — `see`
  returned state stuck ~50s behind wall-clock, no new `log.txt` lines,
  `queued` never draining. Killed both by PID (my own, never by binary
  name) and relaunched. Given `sapper`'s report the same round of processes
  being silently OS-killed under <2GB free memory, and this machine running
  4-5 concurrent `holdfast`/`cargo` processes throughout, the likelier
  explanation is severe frame-time starvation under contention rather than a
  confirmed game hang — a `quit` issued to the *second* stuck instance did
  eventually log `quit requested` and exit cleanly once system load dropped,
  which a genuine deadlock would not do.
- **`tools/pilot.py`'s `digest()` crashed mid-session** with
  `KeyError: 'regen_inside'` on every `see`/`do` call once a light-pool entry
  was rendered — a concurrent, uncommitted edit to `src/pilot.rs`/
  `tools/pilot.py` (visible in `git diff`, presumably in response to
  `sapper`'s finding this same round that light-pool regen is undocumented)
  added new fields to the digest formatter that the already-running binary's
  JSON doesn't emit yet. `raw` and `todo` don't call `digest()` and kept
  working throughout. Worked around by writing a tiny local stand-in
  (`dostub.py`, appends to `commands` and polls `queued==0` without ever
  calling the crashing formatter) rather than editing the shared
  `tools/pilot.py` out from under whoever was mid-edit on it.
- Recruiting allies (`R`) picks a **random** kind from what's missing/
  available, not a chosen one — four `tap R` this session produced two
  Sprites (Scout) before a Slinger (Gunner) ever showed, so getting a
  specific ally kind for coverage may take several tries.
- Level-up chains under heavy passive kill throughput (a maxed 6-weapon
  loadout plus 3 allies plus a turret, by level 20+) can back up several
  levels deep; resolving them is instant per card (no time cost) once you
  stop trying to interleave movement commands with them — batch `tap 1`
  five times in a row if the queue is backed up, then move.

**Takeaway: the theming assignment is half-done.** Names and colours are
correctly, completely reskinned per world with identical mechanics
underneath (by design, and it holds up under inspection). The environment
layer — hazards, pools, terrain shape — is not skin at all; it has real,
sourced numeric differences that change how a world has to be played (Grid's
lethal rhythm-hazard vs. Forest's harmless-but-omnipresent mud vs. Arcane's
inverted healing-hazard). But two visible text surfaces (the opening hint,
every weapon's card blurb) leak Desk-specific flavour into every other world,
and the coverage checklist itself cannot be completed as shipped because a
whole category (hazards) has no tracking hook at all. Next: fix the two
blurb/hint leaks and the hazard `Seen` gap first (cheap, mechanical, and they
were found by grep, not luck); then actually stand in an Arcane ley line
mid-fight and confirm live that the "heals the enemy too" claim changes how a
fight there should be played, which nobody has done yet.

### demolition — 283.9s and 334.7s (two attempts, same round), terrain re-tested against the credited-kill fix, and the pool+chasm compound finally measured live (round 7)
Assignment: three things changed since `sapper` (round 6) — a chasm death only
pays if `DeathEvent.credited` is true, a light pool now costs +0.45 threat while
you stand in it, and `pilot.py do` finally blocks for the full duration of a
steering verb. Re-answer, with numbers: is knockback into a chasm still a build
once the free farm is closed, can a boss actually be shoved in, does a taxed
light pool still pay, and what does the compound (pool + chasm + high dial)
actually measure at.

**Source read first.** `enemy.rs`'s `enemy_fall_off` sets `credited:
enemy.pushed_recently > 0.0`, and `pushed_recently` is set to `2.5` (seconds) by
`apply_damage` in `combat.rs` on **any** `DamageEvent` that reaches an `Enemy`
component — player weapon, ally, turret, hazard tick, or a rival faction's hit
during a war, not specifically a knockback hit. So the fix's real shape is
"was this thing touched by damage in the last 2.5s", not "did the player's
shove cause this fall" — a trash mob that takes a Pencil Dart tick and then
wanders into a hole on its own pays in full, same as one Fan Blast puts over
the rim directly. The old zero-engagement farm (stand across a hole, touch
nothing, watch 16.6 kills/s) is dead either way, because nothing is credited
without at least one hit landing first, but "is this a *knockback* build"
should really be read as "is fighting adjacent to a chasm a build", which is a
fair question, just a different one than the assignment's phrasing assumed.
Confirmed live and unaffected by any of this: `Player`'s `Actor::avoids_chasms`
stays the type default `true` (never overridden), so standing at the very rim
still carries no self-death risk — this was re-verified by literally standing
2-9m from the visible edge for a cumulative 90+ seconds this round without so
much as a stagger.

**Chasm-only, no pool (`demolition`, first attempt).** Built to a 5-weapon
Fan Blast/Coffee Nova/Clip Orbit loadout by level 8-14, then `goto`'d 33m to a
chasm and settled 5-9m out (1-6m to the visible edge) with `defend`. Across
four measurement windows between t=140s and t=165s (threat climbing 1.28→1.45,
reward 1.47→1.58, density 21→41 enemies with 8→25 within 12m): kills went
55→94, **39 kills in ~25s = 1.56/s average**, and **HP was reported at 100% of
max at every single check** (176/176 → 197/199), never once below the cap.
This is the same flat-HP signature `sapper` found last round, now reproduced
under the tightened credit rule — so whatever is happening at the rim (some
kills by knock, most probably by ordinary DPS on things that also happen to be
near the hole) is not gated away by the fix. It then **collapsed**: over the
next 28s density spiked to a boss + 3 elites + ~40 trash with 22-23 within
12m, and HP went 197→31 (an 84% loss) before a `flee` + a Thicker Shell card
heal pulled it back from 7/210. This is the same density ceiling `sapper` and
`fortress` both hit — terrain raises how much density you can eat at 100% HP,
it does not repeal the point past which it stops mattering.

**Finding a genuine overlap spot.** Desk generates a chasm 9% and a pool 11%
per 24-unit chunk (`environments/desk.rs`), independently, so most pairs
sampled this round (about 15, across two runs and 300+ units of travel) sat
35-140m apart — nowhere near close enough to occupy at once. Exactly **one**
pair was found at **13.6m centre-to-centre** (pool at (85.4,-4.3) r=6, chasm at
(79.6,7.9) r=3.5) — close enough that standing just inside the pool's rim
nearest the chasm puts you 4-9m from the chasm's edge, both bonuses live at
once. This is rare, not guaranteed — worth flagging for whoever wants to
reproduce it: it takes deliberate searching, not "walk to the first chasm you
see".

**The compound (`demolition-2`, second attempt), pool+chasm+dial together.**
Stood in the 13.6m-pair spot from level 8 onward, pushed the dial with 5x `tap
EQUAL` to threat 2.44 (reward 2.66-2.76x — the dial's `level` snaps to the
target immediately, it does not ramp like the organic `floor` does, worth
noting for anyone timing a push), then ran 14 back-to-back `defend` windows
(each cut short by a level-up, since kill throughput was high) from t=125s to
t=218s. **Kills climbed 59→209 (150 kills in 93s of session time ≈ 1.6/s
average, individual windows 1.14-2.75/s), and HP was literally 100% of max at
every single one of the ~20 checks taken** — 154/154 up through 216/216 as max
HP grew from cards, never once below the cap, while density grew from 23 to 59
total enemies (2-20 within 12m) **and a boss (THE STAPLER) stood inside 13m of
the fight continuously for at least 51 seconds without dying.** No prior
strategy in this dossier has held flat-100%-HP that long at that density —
`sapper`'s best was "within 3%", this is exactly the cap, for the better part
of a minute.

**Boss-into-chasm: still not confirmed, and now better understood why.** THE
STAPLER survived 51+ seconds standing 6-13m from the chasm under continuous
6-weapon fire without falling (`enemies.bosses` stayed 1 throughout). Read
against source: Fan Blast's `aim` locks onto `grid.nearest_visible`, which at
that density is almost always a trash mob, not the boss specifically — only
Laser Pointer ("snaps to the biggest threat") reliably targets it, and this
build didn't carry one. Coffee Nova and Ruler Sweep hit *everything* in their
(small, 5-7 unit) radius/arc regardless of target-lock, so they touch the boss
whenever it closes to melee range, but their range is short enough that the
boss spent most of its 51+ seconds outside it. Separately, in the open-ground
half of the same run (no chasm within 45m), **a second boss (THE HOLE PUNCH)
did die**, sometime between two checks 14s apart, to nothing but sustained
6-weapon DPS in a 145-enemy melee — proving this build can and does kill a
boss on damage alone within a comparable timeframe, which undercuts any claim
that the chasm made the difference in either case. The mechanical claim from
round 6 stands (uniform knockback, no boss exemption, chasm entry is
independent of remaining HP) but two rounds running now, nobody has watched a
boss actually go over an edge — it needs either a target-locking weapon
(Laser Pointer) deliberately kept in the loadout, or a screenshot to confirm
on camera, and screenshots are still the known 56997-byte blank capture in
this environment (confirmed again this round, both attempts) so the visual
route is closed for now too.

**Does a taxed light pool still pay? Yes, unambiguously.** `threat.from_light`
reports the flat `+0.45` exactly as documented every time the digest was
checked while `standing_in_it`, and it folds straight into `reward_mult` — the
2.66-2.76x reward multiplier through the compound section above has that
+0.45 baked in for free the entire time, on top of the regen and damage bonus.
The tax is real (reward would have been lower without it) but it is a
strictly better trade than not standing there, exactly as `sapper` concluded
before the cost existed, just with smaller margins now.

**Open-ground baseline, same run, stronger build.** After the compound
section, walked 45+ units off any chasm or pool (confirmed via `raw`) and ran
the same `defend`-and-measure loop from t=278s to t=325s with an *even
stronger* build (Fan Blast now level 8/mastered vs. level 4-6 during the
compound section). Kills climbed 234→304 (70 in 47s ≈ 1.5/s, comparable rate),
but **HP fell from 226 (100%) to a floor of 139-163 (58-72% of max) and
stayed there**, under admittedly much higher density (106-162 total enemies,
up to 49 within 12m, 2 bosses concurrently, vs. 23-59 total at the compound
spot) — the comparison isn't perfectly clean (density and time both differ),
but a *stronger* build losing 30-40% of its HP pool where the *weaker* build
held 100% flat is a real, sourced-in-the-data gap, not noise.

**Both runs died the identical way, and it was never the terrain.** Neither
death happened near a chasm or pool. `demolition` died fleeing in the open to
THE HOLE PUNCH's shockwave ring (5 hits, 11-28 damage each, in 3.3s, hint
literally warned "stand close or far, not mid" seconds beforehand).
`demolition-2` died fleeing a 145-162-enemy open-ground crowd: HP 137→-16 in
20 seconds of continuous `flee`, in ~20-21 damage ticks roughly every 1-1.5s —
the exact uninterruptible-attrition signature every round since the first has
logged. Terrain raised the density ceiling substantially; it did not move the
ceiling's existence.

**Numbers.** `demolition`: died 283.9s, level 15, 153 kills, peak threat 2.08,
449 Scrap/12 Cores unspent, 0 structures/allies/zones/forts/wars (terrain was
the whole assignment this round, territory wasn't touched).
`demolition DESK 283.9 15 153 0 0 0 0 0 2.08 204 21744 449 12 0.270`.
`demolition-2`: died 334.7s, level 27, 304 kills, peak threat 2.44, 963
Scrap/15 Cores unspent (worse than the first attempt — kill-rate outpaced
spending rhythm even harder at this power level).
`demolition-2 DESK 334.7 27 304 0 0 0 0 0 2.44 148 18495 963 15 0.317`.

**Operational notes.** (1) `pilot.py do "kite/flee/defend/goto N"` genuinely
blocks for the full duration now, confirmed directly — no more stale-state
reports from returning early. (2) The dial's `level` snaps to the `EQUAL`/
`MINUS` target on the very next tick; only the organic `floor` ramps slowly.
(3) Screenshot capture is still the known flat-56997-byte failure on this
machine, both attempts, so any future "watch it fall on camera" test needs
either a fixed capture path or a different verification method entirely.

**Takeaway: fighting next to a chasm, inside a light pool, at a pushed dial is
still the strongest measured position in this dossier** — 100%-flat HP for
50+ continuous seconds at 2.4-2.7x reward against a boss and rising density is
a result nothing else here has matched — but it is a **rare, found** position
(one 13.6m pool/chasm pair out of ~15 sampled), not something you can always
walk to, and the boss-into-a-hole sub-claim is now *twice* unconfirmed live
despite being mechanically airtight in source. Next: repeat the compound test
carrying Laser Pointer specifically so a target-locking weapon is guaranteed to
threaten the boss while it's near the rim, and fix or route around the
screenshot capture bug so a fall can finally be confirmed on camera instead of
inferred from a count going quiet.

### assayer — falsification pass on three balance changes (round 8)
Assignment: not exploration — verify or disprove three specific claims (crowd
floor 0.58→0.68, first boss 115s→168s with a 9s warning, light pools now cost
+0.45 threat) with numbers, adversarially. Two runs, both on the patched
binary: `assayer` (died 369.8s, level 29, 410 kills, peak threat 4.46,
`assayer DESK 369.8 29 410 0 0 0 0 0 4.46 271 23940 1079 22 0.381`) and
`assayer-2` (ended by `quit` mid-LevelUp, level 23, 254 kills, peak threat
2.32, `assayer DESK 244.4 23 254 0 0 0 0 0 2.32 247 19962 821 16 0.381`).

**Claim 1 (crowd floor): confirmed true, but the first six attempts to measure
it produced a false negative that is worth its own warning.** Source:
`CROWD_FLOOR = 0.68` in `src/player.rs`, predicting `8.4 * 0.68 = 5.712` u/s
against an Ant's 4.97 ceiling. My first measurements — reading position with
`pilot.py raw` before and after a separate `pilot.py do "flee N"` call —
repeatedly showed **zero or near-zero net displacement** (0.02-1.7 units over
4-13 "elapsed" seconds) even in open ground with no chasm/fort nearby, which
looked exactly like the pre-fix "crowd traps you" bug reproducing. It wasn't.
Two compounding measurement faults, both self-inflicted: (1) issuing a card
`tap` and the next `flee` as **separate, sequential `Bash` tool invocations**
adds real wall-clock round-trip latency between "the move command finished"
and "I read the position" — during that idle gap, with no key held, the
character is stationary by design, and that dead time was getting folded into
the speed calculation as if the movement command had been running the whole
time; (2) this build's AoE weapons (Ruler Sweep/Clip Orbit/Coffee Nova) kill
fast enough in a 40+ enemy crowd to chain a level-up every 5-10 real seconds,
and `AppState::LevelUp` gates **every** `GameSet` including `GameSet::Input`
where `tick_threat` lives (`lib.rs`: all sets `run_if(in_state(Playing))`) —
so the reported `t=` elapsed clock genuinely **freezes** the instant a card
screen opens, and a `flee` issued right before that froze at zero elapsed,
zero displacement, indistinguishable from a dead steering system. Confirmed
directly: two `raw` reads 74 wall-seconds apart while parked in one long
LevelUp chain returned byte-identical `t`. (This also means the "level-up
doesn't pause damage" claim in a much earlier round's `diplomat` entry needs a
caveat: `LevelUp` **is** a full simulation pause exactly like `SkillTree` —
the damage that round saw bracketing a level-up happened in the `Playing`
segments either side of it, not during it.)

Fixed the measurement (one Python process: read state, append to `commands`,
poll for `busy`-pickup then idle exactly like `pilot.py do`, read state again —
no inter-call latency, no ambiguity about which segment was `Playing`) and
re-ran it three times, at rising density, all in the open, no chasm/fort
within 25m:

| density (within 12m / total) | flee speed | outcome |
|---|---|---|
| 37 / 79-91 | **5.62 u/s** | within-12m 37→0 in 4.4s |
| 36 / 90 | 4.43 u/s | within-12m 36→7 in 5.3s |
| 53 / 134 | **5.77 u/s** | within-12m 53→4 in 5.3s |

All three land at or slightly above the predicted 5.71 u/s floor and clear
local density to near-zero inside about five seconds — including the
task's explicit "100+ enemies, Ants and Moths" case (134 total, including
Sugar Ant and Lamp Moth by name, both fast kinds). **The crowd floor fix
works exactly as claimed. A crowd, even a genuinely enormous one, can no
longer trap the player**, and the earlier zero-displacement readings were the
harness, not the game — a fifth confirmation of the HANDOFF.md lesson that a
system which looks broken from the bridge is usually the instrument.

**Is wading now too cheap? No — it still costs real HP, measured directly.**
Standing on `defend` while density built from 90→136 enemies (40→54 within
12m) at threat 4.46 cost **258→123 HP, a 52% loss, in about 12 seconds**
before I fled. In the second run's open-ground control (below), 60-73 within
12m produced repeated single-digit-second drops of 12-16% of max HP. The
floor prevents a permanent cage; it does not make crowds free, and a build
that stops moving in one still bleeds.

**One genuine, still-live danger the fix does not touch: stopping right after
a successful flee.** The `assayer` run's death (369.8s) traces exactly here —
`flee` performed correctly (53→4 within 12m in 5.3s, HP cost only ~8 points
during the escape itself), but afterward I issued no further movement while
reading state at 33% HP with Tack Lobbers (ranged elites) and a boss still in
range; HP went 115→-23 over the following 11 real seconds to ranged fire, not
melee density. The floor fixes melee encirclement; it has nothing to say
about standing still at low HP within range of ranged attackers.

**Claim 2 (first boss 168s, 9s warning): confirmed exactly, live.** Source:
`FIRST_BOSS = 168.0`, `BOSS_INTERVAL = 115.0`, `BOSS_WARNING = 9.0` in
`src/enemy.rs`. Live: THE STAPLER was absent at elapsed t=167.2 and present
(`enemies.bosses: 1`) at t=169.0. Backing out the warning's timestamp through
every LevelUp pause gap in between (the `wall` clock the pilot reports keeps
running in real time through a pause; `elapsed` does not, so the gap between
them at any instant is exactly the cumulative pause time) put the "SOMETHING
IS COMING" hint at **elapsed≈159.2s** — 8.8s before landing, matching the
design to within rounding. The *second* boss confirmed the interval
independently: warned at t=274s, predicting a landing at 283s (168+115),
which is exactly `BOSS_INTERVAL` later. **The early game is now
survivable at this encounter**: the build meeting THE STAPLER at t≈168s was
level 8-9 with 5-6 weapons (Pencil Dart, Stapler, Laser Pointer, Ruler Sweep,
Clip Orbit) — precisely the "only about four weapons deep" profile the
pre-fix dossier recorded dying to this exact boss at 125-141s — and this run
survived the encounter itself outright, dying 200 seconds later to an
unrelated dense/ranged situation. **The 9s warning is real reaction time**,
confirmed by the second boss's announce-to-land gap holding at 9s exactly
again. Caveat found in the same window, not a fault in the boss timer: my
build had wandered into a fort/nest/chasm cluster right as the warning fired
(HP crashed 152→77 in that stretch from the *fort*, not the boss) — the
timer gives honest notice, but 9 seconds does not protect against an
unrelated hazard already unfolding when it fires.

**Claim 3 (light pools cost +0.45 threat): confirmed live, and the "is it now
too strong" question has a real, if uncomfortable, answer.** `threat.
from_light` read exactly `0.45` every single time sampled while
`standing_in_it`. Ran a same-run, same-build, same-threat-dial A/B:

- **In the pool** (level 6→22 build climbing through it), across 8+ separate
  windows at threat 1.05-2.24 (reward 1.06x-2.65x) and density climbing from
  16 to **54 within 12m / 114 total** (including a boss): HP was **pinned at
  literally 100% of max in every single window** — the highest-density window
  (41→54 within 12m) had HP go **93%→100%, a net increase**, while getting
  hit continuously (nearest enemy 1.1-1.5m throughout).
- **In the open**, same run, same weapons, same threat (2.24), **higher**
  density (53-73 within 12m, up to 131 total): HP repeatedly dropped —
  226→189 (-16%), 219→193 (-12%), 210→194 (-8%) inside single-digit-second
  windows, none of which the pool comparison ever showed at equal-or-lower
  density.
- **But the pool is not build-independent.** A separate, deliberately
  underpowered attempt (level 13, six low-level weapons, threat pushed to
  4.59 — well past what the build could clear) died **inside the pool**, HP
  178→-3 in under 10 real seconds at only 46-54 total enemies. Standing in
  light does not rescue a matchup your weapons cannot win.

**Verdict: the tax is real and it is doing about half its job.** It correctly
gates the pool behind "your build has to already be coping" — an
underlevelled character still dies there exactly as in the open, so it is not
a universal safety blanket. But for any build capable of holding its own at
all, the pool remains close to strictly dominant: it doesn't just reduce
damage taken, it let HP climb while under sustained fire at the highest local
density recorded this round for any strategy. A `+0.45` addition to the
reward exponent's base is not enough to make "don't stand in the pool" a
live decision once a run is past its shakiest early levels — whoever tunes
this next should either raise the tax further or add a second cost (e.g. an
explicit aggro pull, since `enemy.rs` still has no code that weights
targeting toward light at all, confirmed again this round by grep) so the
pool is a trade a strong build still has to weigh, not a corner it parks in.

**Operational notes for whoever measures timing/position next.** (1) Do not
issue two commands that depend on each other's outcome as separate parallel
tool calls — the task's own instructions call this out and I violated it
early this round, which is very likely what produced several of the "stuck"
readings before I caught it. (2) For any speed/HP-rate measurement, write a
single script that appends to `commands` and polls `busy`→idle in one
process; two separate `pilot.py do`/`pilot.py raw` invocations have enough
their own overhead to contaminate a 3-5 second window. (3) `goto` still gives
up short fairly often near any density ("ran out of time working round
something in the way") — pass a third argument or reissue; this happened four
times this round, always recovered on retry. (4) `frames_per_sec` never
dropped below ~55 in any reading this round despite six concurrent instances
on the shared machine — no measurement here is stutter-suspect. (5) `quit`
sent while a `LevelUp` card was open still wrote a dossier row correctly, but
the process itself lingered several seconds past the "quit requested" log
line before actually exiting — not investigated further, did not block
anything.

Next: whoever plays after this should (a) push the light-pool question the
other direction — try an even *stronger* build in a pool at the threat
dial's 8.00 ceiling and see whether HP can be held at 100% indefinitely, which
would make the "raise the tax further" recommendation above urgent rather
than optional; (b) the ranged-death-after-a-successful-flee pattern found
here (Tack Lobbers finishing a run that had already escaped melee range)
is a new failure mode nobody has named before and is worth a dedicated look —
does `flee`/`kite` account for ranged attackers' effective range at all, or
only melee proximity?

### marshal — the multi-fort empire, eight attempts to the first capture, then two forts ~189 units apart (round 8)
Assignment: hold two forts at once and find the ceiling. Read the source first
(`src/forts.rs`) to get the capture arithmetic exactly, since every prior round
that touched forts had to reverse-engineer it live: presence is `player 1.0 +
allies*0.7 + structures*0.5` inside `FORT_CAPTURE_RADIUS=7.5`, weighed against
`defenders = garrison*0.34` (only same-faction monsters count as defenders;
everyone else is a "rival" that pressures a *player-held* fort but doesn't
help a capture). While the player holds it, `net` cannot go negative unless
`pressure - friendly > LOSS_MARGIN(1.0)` — otherwise it settles back to fully
held on its own, exactly as the assignment promised. Also found in source and
worth logging precisely: `Stance::Guard`'s auto-anchor (`squad_input` in
`src/command.rs`) only ever targets `Zone`s, never forts — the assignment's
claim that "G sends them to guard a spot" is true only for `Stance::Hold`
(press `G` once more to cycle Follow→Hold; it anchors each ally at its
*current* position, which is why walking them to the fort first, then
pressing `G`, works and pressing `G` from anywhere else does not).

**Seven straight deaths before the first capture, and the reason was never
the fort — it was the approach.** Attempts 1-7 (levels 7 to 25, five
different regions, tactics ranging from aggressive wide `kite`-leveling to
disciplined compact farming) all died to ambient density between 60 and 260
enemies *before or immediately upon* reaching a fort's ring, never to a
siege that had actually started going well. The pattern that broke it in
attempt 8: **farm in one fixed spot with `defend x y r` (not `kite`, which
explores continuously), then make exactly one long `goto` straight to a
clean fort (garrison ≤2) with no detours.** `defend` at a single point held
density flat at 5-30 enemies from level 1 to level 10 for over 300 real
seconds — kiting the same stretch produced 60-150+ within the first two
minutes every single time. `EXPLORED sq units` correlated almost exactly with
ambient `ENEMIES total` across every attempt: `tick_nests` in `src/forts.rs`
spawns from *any* nest within `ASSAULT_RANGE=66` of the player's current
position regardless of the *fort's* distance to the player, so wide travel
activates every nest cluster passed near and none of them ever go back to
sleep — a genuine, previously-undocumented cost of exploring, not just of
standing near a fort.

**The capture itself, once reached with a real build, is fast and survives
being swarmed** — the previous rounds' "eleven seconds is a siege" undersold
it once density gets extreme. First capture: approached a clean VOID fort
(garrison 0) at level 11 with 6 mid-leveled weapons, immediately built 5
Tack/Shocker turrets via Plan Mode (`SPACE`, arrows, `1`/`3`, `ENTER` — the
12% time scale gave real breathing room mid-build, confirmed directly: HP
that was falling in real time climbed the instant Plan Mode opened), then
recruited a full 4-ally squad. Progress went 7%→29%→46%→61%→87%→**captured**
over about 90 real seconds while density oscillated 60-90 enemies within
12m, HP never dropping below 25% and mostly staying above 70% once the ring
was actually in place. `hints.push_once("fort-first", "FORT TAKEN"...)` fires
its headline text only once *per run* — the underlying capture (stats,
economy, coverage) fires every time, so a second silent capture reads only
in `raw`'s `forts` array (`owner: "YOU"`, `capture: 1.0`), not in the digest
hints.

**Q1 (can two forts be held at once) — answered, and the answer is no, not
passively.** Captured a second fort **188.97 units away** (VOID again;
`(-153.788,-11.695)` to `(-135.406,-199.735)`, computed from `raw` positions)
using the same ring-then-squad recipe, this time from a level-57, 745-max-HP
character that no longer needed to be careful (307 enemies, 65 within 12m,
HP never left 90%+). Set the squad to `Hold` at fort 1 before leaving. Both
forts were reclaimed by their original owner (VOID both times) within
roughly 30-90 real seconds of the player's presence leaving the capture
radius — confirmed three separate times across the run (fort 1 lost while
travelling to fort 2; fort 2 lost while travelling back to recapture fort 1;
fort 1 recaptured a second time and *still* only holds while the player is
physically in the ring). The turret-and-ally ring (1-5 turrets + up to 4
allies, `0.5`/`0.7` presence each = 2.5-5.3 total) is real defensive value —
it visibly slows the loss and the fort settles back toward full-held the
instant the player returns — but it was never once enough on its own against
the `MassOnFort` response a capture provokes. `threat.holdings` /
`economy.scrap_per_sec_from_forts` in `raw` confirmed this precisely without
having to guess: it read `0.35`/`~2.9-3.5` (one fort's worth) at every single
check taken this run, **never `0.70`** — the two captures never overlapped
in wall-clock time despite both nominally being "held" moments apart.
**Whoever tries next should build a much bigger ring before ever leaving
fort 1** (I had scrap surplus in the thousands unspent every time — 8-12
turrets costs under 300 Scrap and was never tried) and/or split allies 2-and-2
between forts rather than 4-at-one, since a 4-ally/0-turret fort held less
than a 3-turret/2-ally one did in this run's informal comparison.

**Q2 (what does the empire pay) — confirmed exactly, numbers from `raw`,
not inferred.** `threat.holdings` is `held * 0.35` and never higher than
`0.35` this run because holdings never overlapped (see above) — so the
"does it compound" question is still open, just for a different reason than
expected: the *ceiling* was never approached, the *floor* (staying in one
place long enough for two forts to coexist) was the actual blocker.
`economy.scrap_per_sec_from_forts` tracked `FORT_SCRAP(2.4) * income_mult`
precisely — read `2.4`, `2.88`, `2.9`, `3.456` across the run as Logistics/
income cards stacked, always matching the held-count × base × multiplier
exactly, confirming last round's source-reading and finally giving it a live
number.

**Q4 (does an ally squad capture a fort alone) — still untested**, and this
round found the mechanic (`Hold` stance, not `Guard`) without ever trying the
from-scratch version: sending a squad at an *enemy-owned* fort with the
player elsewhere. Next round should walk allies to a low-garrison fort,
switch to `Hold`, then leave completely (not just retreat to the ring's edge)
and watch `raw`'s `capture` value climb on its own.

**MassOnFort at scale, confirmed exactly as the assignment described.**
`raw`'s `factions` array read `SWARM:MassOnFort(64%) RUST:MassOnFort(40%)
VOID:MassOnFort(95%)` simultaneously, multiple times, the instant a capture
completed — three factions massing on one fort at once, not two. The
posture cleared back to `HuntPlayer(0%)` for all three within about 15-20
seconds each time contest ended (fort settled either way), consistent with
`WarRoom`'s 4-second review interval plus a few review cycles to disengage.

**Operational, for whoever plays next.** (1) **`raw`'s `forts` array is only
the 8 nearest to the player** (`write_war` in `src/pilot.rs` does
`.take(8)`) — once you are more than ~80-100 units from a held fort with
denser ground between, it silently drops off the list. There is currently no
way to check a distant held fort's status without physically returning to
it or reasoning from the `log.txt` hint stream (`FORT LOST: THE <faction>
took it back.` fires every loss, unlike the capture headline). Whoever wants
to measure exact hold duration next should watch the log, not poll `raw`.
(2) **`O` is Overclock, everywhere, not just outside Plan Mode** — reached
for it once meaning to cycle something else and it fired a 22-second threat
surge unintentionally (harmless this run since the build could tank it, but
worth flagging: it is not gated by any state, doesn't share a cooldown
warning in the digest until it's already firing). (3) Building in Plan Mode
during an active siege is not just safe, it is a *reversal* — HP climbing
the instant `SPACE` was pressed, repeatedly and reproducibly, because the
12% time scale slows the incoming damage far more than it slows placing a
turret. (4) A single `defend x y r seconds` call that spans a level-up still
gets interrupted exactly like `kite`/`flee` (the level-up screen eats the
rest of the batch) — this cost real turns every single attempt; batch the
card tap and the next steering verb together as the playbook has said since
round 3, and it still isn't automatic.

**Numbers.** Eight lives total this session (`marshal-multifort` through
`-8`); the first seven all ended in death with `forts=0` (193-341s, levels
7-25, 67-373 kills — see `holdfast-runs.tsv`, not narrated individually here
since none reached the assignment). The eighth ended by a deliberate `quit`
while fully healthy, which still wrote a dossier row:
`marshal-multifort-8 DESK 511.7 60 944 3 4 0 1 0 3.74 243 19953 6137 37 0.667`
— level 60, 944 kills, 3 structures/4 allies live at the moment of quit, a
fort actively held (`forts=1`), peak threat 3.74, 6137 Scrap and 37 Cores
unspent (the usual failure, at a new scale — an 8-12 turret ring on fort 1
alone would have cost under 5% of the banked Scrap).

**Takeaway: the fort-empire frontier moves from "can you take one" (closed
since round 5) to "can you hold more than one at a time" — and this round's
answer is that presence is genuinely a zero-sum resource in this game by
design, not by bug.** A fort not actively occupied by the player, however
heavily garrisoned with turrets and allies, loses a contested tug-of-war
against a determined three-faction siege within about a minute. That is
consistent with `LOSS_MARGIN` and `DEFENDER_WEIGHT` as written — it is not a
missing feature, it is the intended cost of expansion. The empire's ceiling
in this run wasn't threat, distance, or Scrap; it was the number of things
one player can be near at once. Next: a much bigger single ring (8-12
structures, tested — not just assumed — against the density this round
measured), the ally-alone capture from Q4, and a genuine attempt to hold
*three* forts by pre-building rings at all three before capturing any of
them, since Scrap was never the bottleneck once the first fort's income
came online.

### viceroy — three forts held at once, a build-from-outside-the-ring technique, and a hard measurement of what wide exploration still costs (round 9)

Assignment: verify the two round-8 fixes (reclaim lifts, monsters >165 units
released) actually changed the picture — hold two forts at once, test whether
an ally squad can capture a fort alone, and re-measure whether wide
exploration is viable. Answer to all three: **yes on two forts (in fact
three), unresolved-but-informative on ally-alone, and no on wide exploration
being free — it is measurably better than pre-fix, but still expensive.**

**Eight lives this round, six of them killed by the same mechanism: a
`LevelUp` screen opening between two keys of an already-issued batch.** This
was worse than any previous round's account of it. This build's weapon set
(Ruler Sweep, Clip Orbit, Coffee Nova, Fan Blast) chains kills fast enough
under 20-60 concurrent enemies that a level-up can open in the ~1-2 real
seconds between one `pilot.py do` call finishing and the next one's first key
being delivered — and once open, every subsequent key in that *next* batch
(`SPACE`, a digit, `ENTER`, `flee`, `goto`) gets swallowed as a `LevelUp`
input or silently dropped, while the character stands still and takes full,
unmitigated damage. Confirmed directly and repeatedly: `goto`/`flee`/`kite`
issued right after a card pick sometimes produced *zero* positional change
and a full HP crash (139→29 in one four-second window; 158→8 in another) with
the `!! REFUSED` log showing the exact keys that got redirected. The single
fix that worked: never issue more than a card-pick plus one steering verb per
batch, and re-check state after *every* card, not just the ones you expect.
Even doing this, it cost two outright deaths (`disciplined-single-fort-siege-v3`
died 261.6s/level 11/96 kills to a card-swallowed `kite` at 154→31→22→9 HP in
under 15 seconds; `fort-siege-v5` died 249.2s/level 13/129 kills the same way
at a fort's ring, garrison having grown from 1 to 6 between scouting it and
arriving).

**Q3 (is wide exploration viable now) — better, not free.** With the
`FORGET_DISTANCE=165` fix live, local density (within 12m) genuinely resets
to near-zero on a single `kite`/`flee` call even from 20-50 within 12m,
confirmed a dozen times this round — the crowd-floor and forget-distance
fixes are doing their job. But the **total ambient count still climbs hard
with travel**, fix or no fix: one life went 20 (t=100s) → 82 (t=191s) → 145
(t=249s, death) total enemies over about 150 real seconds and roughly 250
units of net travel; another went 21 (t=139s) → 176 (t=248s) over 100
seconds. The mechanism is the same one `marshal` named in round 8
(`tick_nests` spawns from any nest within 66 units of the player's *current*
position, not the fort's), but it is sharply worse specifically **inside a
cluster of enemy forts** — a region with 4-6 forts within 30-90 units of each
other (seen for VOID and RUST territory both) stacks that many forts' nests
and seeders on top of each other, and simply walking through one is what
turned 40 enemies into 150+ inside thirty seconds twice this round. **The
fix makes kiting through open ground cheap; it does not make walking into a
faction's fort cluster cheap, and the digest's `FORTS`/`NESTS` lines are the
warning to read before committing to a direction.**

**Q1 (hold two forts) — yes, decisively, and it went to three.** Approached
an isolated `garrison: 2` SWARM fort at level 6 (five weapons, 0 allies —
allies unlock on a **timer, 2:45 elapsed, not a level threshold**, confirmed
directly: `HINT SQUAD LOCKED: Comes online at 2:45`, contradicting every
previous round's assumption it was level-gated), stood ~20 units outside its
7.5-unit capture radius, opened Plan Mode, and walked the cursor onto the
fort itself (`FORT_CAPTURE_RADIUS` is small enough that the plan cursor's
26-unit leash reaches it easily from well outside). Placed 4 Tack Turrets
directly on the fort's ground (its own collider blocks the exact centre — a
small cursor nudge finds clear ground still inside the ring) and fled. With
**zero player presence in the capture radius at any point**, `capture`
climbed 0%→30%→68%→79%→"FORT TAKEN" over about 20 real seconds while the
player stood 15-25m away taking near-zero damage. Repeated the exact
technique on a second fort (garrison 1) 33 units away: same result, same
zero-presence capture. With both held, `raw`'s `threat.from_forts` read
**exactly `0.70`** (`0.35 × 2`) and `economy.scrap_per_sec_from_forts` read
**exactly `4.8`** (`2.4 × 2`) — the first time either number has been seen
above `0.35`/`2.4` in this dossier. Continuing the same life, the player
then walked (not planned — just arrived) into a *third* fort's ring and
captured it through ordinary presence over about 30 seconds; with all three
held, `threat.from_forts` read **`1.05`** and `scrap_per_sec_from_forts` read
**`7.2`**, both exactly `n × constant` for `n=3` — confirming the assignment's
own prediction ("Three forts is +1.05 threat") to the decimal. `reward_mult`
peaked at **2.98** (effective threat 3.28: 1.56 level + 1.05 forts + 0.2
territory + 0.45 for standing in a light pool at the same moment — every
additive threat source stacking simultaneously, confirmed live for the first
time).

**Durability, measured precisely instead of estimated.** Fort 1 (4 Tack
Turrets, no allies, player elsewhere) held from capture (~t=141s) until
between t=221s (`capture: 0.745`, still slowly eroding) and t=241s (fully
reverted, `owner: SWARM`, `garrison: 12`) — **roughly 80-100 seconds
unattended** against a real `MassOnFort` reclaim, a large step up from
round 8's 30-90s for a similarly-sized reactive ring, consistent with
building the ring *before* the reclaim starts rather than during it. Fort 2
(3 Tack Turrets) was still holding at that same check (`capture: 0.771`,
`contested: false`) — at least as durable. Fort 3 was different: captured by
ordinary player+ally presence (3 allies in `Follow`, no turrets), then the
squad was switched to `Hold` at that exact spot and the player left — this
fort reverted in roughly **20-35 seconds**, far faster than either
turret-defended fort. All three eventually fell back to their original
owners by t≈250s, but never simultaneously — the ring-defended forts clearly
outlasted the ally-defended one.

**Q2 (can allies alone capture/hold a fort) — the capture side is now
answered by proxy, the hold side got a real but imperfect data point.**
Structures alone (0 allies, 0 player-in-ring) captured two forts outright
this round, which settles the design's core claim ("allies have to be able
to contribute to a capture and finish one on their own") for structures at
least — presence-by-proxy works exactly as the source promises. For
allies specifically: the one clean-ish test (3 allies on `Hold`, 0 turrets,
after the player had already stood in the ring during the capture itself)
held for only 20-35 seconds, well under either turret-only fort's duration.
That is not a clean "allies alone from scratch" test — the fort was already
captured before the squad order was given — so whoever plays next should
still do the from-scratch version marshal proposed: walk a full 4-ally squad
to a full-garrison *enemy-owned* fort, set `Hold`, and leave before capture
starts, watching `capture` climb (or not) with zero turrets and zero player
presence ever in the radius.

**Numbers.** Eight lives this round (see `holdfast-runs.tsv` for
`multi-fort-empire-...`, `fort-ring-then-...-v2`, `disciplined-single-fort-
siege-v3`/`v4`, `fort-siege-v5` through `v7`); the first six died between
203s and 372s to the LevelUp-swallow mechanism above, none reaching a fort.
The seventh (`fort-siege-v6-plan-mode-ring-from-outside-radius`) got the
technique working for the first time but died at 254.5s/level 14/132 kills
before the fort finished flipping. The eighth
(`fort-siege-v7-early-plan-mode-ring-at-distance`) is the one described
above: ended by deliberate `quit` at t=250.9s, level 19, 211 kills, 3
forts held simultaneously at peak (`0` at the row itself, since all three had
reverted again by the time of the `quit` — see below), peak reward
`x2.98`, 923 Scrap / 43 Cores unspent —
`fort-siege-v7-early-plan-mode-ring-at-distance DESK 250.9 19 211 3 1 1 0 0
2.24 264 19962 923 43 0.667`. **Operational note on that `quit`:** issued
while a `LevelUp` was open, it logged `quit requested` immediately but the
process then sat with `state.json`'s `seq` apparently frozen (same `seq`,
same `wall`, three consecutive reads) for several real seconds, matching
round 8's lingering-process note exactly — but this time it resolved on its
own: the process exited cleanly, the dossier row was written correctly, and
it no longer appeared in `ps aux` afterward. Not a permanent hang, just a
slow one; worth another few seconds' patience before assuming the run is
stuck.

**Takeaway.** The two round-8 fixes did exactly what they were meant to:
forts can now be held in the plural, and travel is not an automatic death
sentence. The frontier has moved again — it is no longer "can you hold two
forts" (yes) but "can you reach a fort at all without either the density
spiral or the LevelUp-swallow mechanism killing you first," and separately,
"does a *pure* ally garrison (no turrets, no player, from a completely
uncaptured fort) ever hold." Next: the from-scratch ally-Hold test above;
carrying Bulk Salvage/Overtuned into a life dedicated to nothing but building
15-20 Tack Turrets at a *fourth* fort to see if the 80-100s unattended
duration this round measured scales further with ring size; and a serious
look at whether the pilot bridge can detect "a LevelUp opened mid-batch" and
either retry the swallowed keys or report it inline, since it is now the
single largest cause of death in this dossier, bigger than any monster.

### auditor — falsification pass on the two round-9 fixes, plus the round-8 ranged-death gap (round 9)

Assignment: verify-or-break `FORGET_DISTANCE=165` and the reclaim-lift
(`SIEGE_PATIENCE=42`/`SIEGE_REGROUP=34`), and settle whether ranged fire is
the real killer post-crowd-floor-fix. Six lives. Verdict on all three:
**both fixes work as intended and neither has become an exploit or a
trivialisation; the round-8 ranged-death gap has a sharper answer than
expected — the dominant ranged killer in a first fort siege is not the Tack
Lobber, it is the fort's own emplaced gun, and it is unavoidable by design.**

**Claim 1 (165-unit forget) — confirmed clean by direct A/B, and it does not
break a fight.** Source: `forget_the_left_behind` in `src/enemy.rs` despawns
any non-boss enemy past 165 units every frame, decrementing `director.alive`
and writing no `Record` — so nothing it touches can inflate kills or reverse
the counter, confirmed by code and never once seen to regress a kill total
live (338→356→372→411→416→…, monotonic every check this round). Measured the
exploit question directly: at t=406.3s (238 enemies total, 41 within 12m, HP
220/250) issued one `goto` 198.6 units in a straight line, arriving at
t=428.6s (22.3 elapsed seconds later, ≈8.9 u/s — essentially full move speed,
the crowd-floor was not even needed because nothing could keep pace) with
enemies down to 177 and only +18 kills credited: `(238−177)−18 = 43` monsters
silently released, paying nothing, in that one flight. Returning to
approximately the same spot afterward showed the *local* crowd was genuinely
gone (2 within 12m vs. 41 before fleeing) even while the *global* total kept
climbing from continued wide travel (238→177→230→248→253) — so fleeing shreds
the specific mob chasing you, it does not zero out the ambient pressure the
run has already earned by exploring, which is exactly the design's intended
half of the fix. On "does it break a fight": both the player's weapons (max
range ≈20-70 depending on level/area stats) and the worst ranged monster
(Tack Lobber, 20 units) are far under half of 165, so nothing that is
actually exchanging hits with the player can be anywhere near the forget
radius — the only way to trigger it is genuinely outrunning a mob over
150-200 units, which cost ~22 real/game seconds of committed, undistracted
movement here. That is a real cost, not a free reset, and it is the design's
intended cost ("losing ground should be a decision made minutes ago"), paid
in time and distance rather than HP. **No exploit found, no interrupted
fight found.**

**Claim 2 (reclaim lifts) — confirmed live, with a captured-and-lost fort
watched start to finish, and it answers "too easy to keep" with a clean no.**
Capturing one clean (garrison 0) fort took five deaths first — three of them
(`auditor-3`, `auditor-4`, `auditor-5-retreat-and-let-turrets-work`, 251.9s/
296.9s/316.5s, levels 15/20/16) died at 44-81% capture progress despite full
HP moments earlier, every time from the same two-part mechanism: the fort's
gun (see Claim 3) plus a `LevelUp` chain silently cancelling an in-progress
Plan Mode turret build (confirmed directly: a batch of `SPACE, ENTER, RIGHT,
RIGHT, ENTER, …, SPACE` that crossed a level-up left only 1 of 3 intended
turrets built and dumped the player back into full-speed combat mid-ring —
independently rediscovered the same session as `viceroy`'s "six of eight
lives" finding above). The fix: build **one turret per `do` call**, check
state after every single key, and resolve any interrupting card before
placing the next turret — confirmed Plan Mode genuinely survives a resolved
LevelUp (`PLAN MODE IS ON` persisted across a level-up-and-card-pick in this
run), so the fault is specifically an *un-resolved* card eating the batch,
not Plan Mode itself. With that discipline, `auditor-6` captured a VOID fort
at level 16/full HP (192/192) with 5 Tack Turrets + a 3-ally `Hold` squad,
never dropping below full HP during the build. Directly confirmed via `raw`,
twice, 110 elapsed seconds apart (t=263.0 and t=373.6): `owner: YOU,
capture: 1.0, contested: false`, while `factions` simultaneously read
`VOID:MassOnFort(0.95) RUST:MassOnFort(0.4)` continuously across that whole
window — three-faction commitment sustained for well over a minute produced
*no* contest at all, because the ring was still killing attackers before they
reached the 7.5-unit capture radius. The fort held roughly **130 elapsed
seconds** total (captured ≈t=255s, first sign of `contested: true` and
`garrison: 14` on VOID's side by ≈t=385-395s, cross-referenced against
`FORT LOST` firing at wall=4336.04, ~9-15 seconds after the last confirmed
full-HP `Playing` tick) — longer than `marshal`'s round-8 baseline of 30-90s
for a similarly-sized ring, and in the same range as `viceroy`'s round-9
80-100s for a *pre-built* ring. Once it actually flipped, the loss took only
**~9-11 seconds** end to end (three ~6-8 damage ticks then `FORT LOST`),
matching the original "eleven seconds is a siege" design exactly on the way
out as well as the way in. **Verdict: a held fort is measurably harder to
lose than before round 8/9 (longer uncontested windows under real
multi-faction pressure), but it is not "too easy" or unlosable — a ring far
stronger than the hypothetical "two turrets" (5 turrets + Overtuned + 3
allies) still fell to a determined siege once contact was actually made.**
On "does the lull read as disinterest": not confirmed either way this
round — my own sampling was too coarse (two reads 110s apart, both catching
`MassOnFort` at high commitment) to see whether SWARM/VOID/RUST/BLOOM's
posture actually cycled to `HuntPlayer` and back per the 42s/34s clock in
between; whoever samples next should poll `raw`'s `factions` array every
~5-10 real seconds through a full hold to catch the toggle directly.

**Claim 3 (is ranged fire the real killer, and what can a player do) — yes,
and the source of it is not what round 8 expected.** `src/forts.rs`'s
`fort_guns` fires `GUN_DAMAGE=8.0 * fort.strength` per shot, one muzzle
rotating every `GUN_CADENCE(1.7)/guns(3) ≈ 0.567s`, for an enemy fort — about
14.1 dps aggregate, unavoidable by any movement speed because *capturing
requires standing inside `FORT_CAPTURE_RADIUS=7.5`*, itself well inside the
gun's own `GUN_RANGE=15`. Every death this round at a fort (three separate
lives, three separate forts) showed the exact same log signature: a
metronomic ~8-damage tick every 0.4-0.6 real/game seconds, occasionally
punctuated by larger melee hits, dropping 150-250 HP to zero in 10-20
seconds once the fort's posture flipped to `Defend`. This is *not* new to
round 8/9 — `GUN_DAMAGE`, `DEFENDER_WEIGHT`, `LOSS_MARGIN` and
`CONTEST_URGENCY` are unchanged since `c174444` ("forts defend themselves"),
confirmed by `git log -p`, so it predates and is independent of both changes
this round was asked to audit. Round 8's Tack-Lobber-after-a-successful-flee
death is a *different, smaller* threat (max 20-unit range, ~5-11 damage per
hit, genuinely dodgeable by kiting) from the fort gun (15-unit range, ~14
dps aggregate, *not* dodgeable once you are committed to a capture, because
the objective itself requires standing in range). **What a player should do
about it, and whether that option is actually available:** yes, it is
available and it worked — bring HP/armour/regen cards *before* approaching
(this round's successful life had taken Second Wind, Thicker Shell and
Plating over the preceding levels), bring a full ally squad to split
incoming fire, and build the turret ring one placement at a time rather than
batching, since a batched build is the single most common way this round's
testers (both `auditor` and `viceroy`) died with the ring half-finished. The
option exists and is exactly what the source comments describe
("bring armour, regen and health, or do not go") — the failure mode is
executing it carelessly (batching keys through a level-up), not the
mechanic being unfair.

**Numbers.** `holdfast-runs.tsv`: `auditor` 277.9s/L16/181k/2.05,
`auditor-2-forgetdist-reclaim` 160.9s/L9/80k/1.41, `auditor-3-careful-siege`
251.9s/L15/159k/1.90, `auditor-4-armored-siege` 316.5s/L20/198k/2.26,
`auditor-5-retreat-and-let-turrets-work` 296.9s/L16/172k/2.15, and the best
of the round, `auditor-6-final-siege-attempt` (ended by `quit`, though the
process took its usual several extra real seconds to actually close —
matching `viceroy`'s and round 8's notes on this): **477.3s, level 32, 416
kills, peak threat 3.15, 2 allies alive, 0 structures alive, 3669 Scrap / 61
Cores unspent** (`auditor-6-final-siege-attempt DESK 477.3 32 416 0 2 0 0 0
3.15 282 39501 3669 61 0.667`) — a fort was captured and later reclaimed by
VOID before the row was written, so the dossier's `forts` column reads 0
despite ~130 seconds of successful holding earlier in the same life; the
`forts` column is a snapshot at death/quit, not a lifetime count, and should
not be read as "never captured one" without checking the narrative.

**Next:** the chunk-unload question this round did not get to test cleanly
but should be checked directly — `src/world.rs`'s `stream_chunks` fully
`try_despawn`s every `ChunkEntity` (forts and nests both carry one) once its
chunk exceeds `UNLOAD_RADIUS(5)*CHUNK_SIZE(24)=120` units from the player,
and `generate_chunk` reassigns a fort's owner deterministically from the
world seed with **zero persistence of a player capture** when the chunk
reloads. If that is right, a captured fort whose chunk unloads (travelling
~120+ units away, which `marshal`'s two-forts-189-units-apart and this
round's own wide travel both did) would silently revert to its original
hostile owner *regardless* of the reclaim-lift fix or ring strength — a
different, cheaper way to lose a fort than a siege, and one that would mean
some of the "reclaimed within 30-90s" measurements in earlier rounds may have
been chunk-unload resets rather than genuine `MassOnFort` victories. Confirm
by capturing a fort, walking dead straight along one axis until it drops out
of `raw`'s `forts` list entirely, then returning to check whether `owner`
and `capture` came back as `YOU`/`1.0` or reset to the original hostile
faction at `-1.0`.
