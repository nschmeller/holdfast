# The playbook

Accumulated strategy knowledge for HOLDFAST, written by everyone who plays it.

**Read this before a run. Append to it after.** Fifty rounds of testing is only
worth more than one round fifty times if what each run learns survives it.

Keep entries short and concrete. A hypothesis, what you did, what happened,
what to try next. Numbers beat adjectives.

---

## What is known

**The record, as of the last round played, is 337.7 seconds** (turtle,
level 14, 220 kills) — up from a starting baseline of 148s. See below.

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

### Open questions nobody has answered

- Does holding territory pay for the threat floor it raises? Territory
  unlocked in every run played so far, but zone markers were never actually
  stood on to test capture — `zones` reads 0 in every dossier row to date.
- Can allies take a fort without the player present? The mechanic says yes;
  nobody has tried it. Allies unlocked this round (turtle run) but were never
  recruited — squad stayed 0/4 the whole session.
- Is a faction war actually pressure relief, or does it just kill things you
  wanted the XP from? Still untested — Research was reached for the first
  time this round but its nodes were bought blind (see below) and Whisper
  Campaign/Blood Feud specifically were not identified or targeted.
- What does maximum threat do to the economy over a long run? Threat has now
  been observed climbing organically past 2.38 without ever being touched by
  hand — the dial itself unlocked at t=318s in the best run, but the run
  ended less than 20 seconds later. Nobody has still ever pressed `-`/`=`.
- Is knockback beside a chasm strong enough to build around? Still untested —
  no chasm was encountered in five runs across two rounds.
- Do light pools pay for the attention they draw? Still untested.
- **New question: does a turret ring plateau or genuinely hold indefinitely?**
  The best run this round built a 5-structure ring that visibly stabilized HP
  (climbing from 63 to 132 over ~80 seconds while kills piled up) — but by
  wave 6 the ring had been worn down to 1 turret and the position collapsed,
  forcing a flee that ended in death near an unrelated fort cluster. Was that
  an inevitable consequence of threat scaling outpacing structure HP, or would
  a bigger/earlier ring (started with more Scrap saved up front, or built with
  Overtuned taken before the swarm arrived rather than during it) have held
  past that point? Untested.

---

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
