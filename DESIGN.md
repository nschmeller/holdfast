# DESK FREE-FOR-ALL — Design

A keyboard-only 3D **survival command** game. You are a small hero holding
ground against an endless escalation. The skill being tested is *decision-making
under compounding pressure*, not reflexes.

## Pillars

1. **Command, don't twitch.** Aiming is automatic. Nothing requires frame-perfect
   dodging. Every meaningful decision can be made with time slowed to a crawl.
2. **You own the throttle.** The run is endless and always escalating, but the
   player chooses how fast. Every point of speed is paid for in reward.
3. **Ground is worth something.** Territory, structures and squad positioning
   turn the arena into a board you shape, not just a space you kite around.
4. **One vocabulary, four worlds.** The same twelve threat archetypes appear
   everywhere, renamed and retinted, so knowledge transfers between runs.

## The three clocks

| Clock | Length | What it does |
| --- | --- | --- |
| **Trickle** | continuous | Light, constant pressure. Never zero, never overwhelming. |
| **Wave cycle** | ~55s | Prep window → Assault. The main rhythm of play. |
| **Boss cycle** | ~115s | A named boss. Repeats forever with stacking `+N` modifiers. |

## THREAT — the pacing dial

A single number, `0.5` to `8.0`, set by the player with `-` and `=`.

- **Raises**: spawn rate, enemy HP/damage, elite frequency.
- **Also raises**: XP, Scrap, Cores, drop count and gear rarity — superlinearly
  (`reward = threat^0.92`), so overreaching genuinely pays.
- **Floor rises with elapsed time.** Turtling is a delaying action, never a
  strategy. By minute 15 the dial cannot go below ~5.

Three further levers stack on top:

- **Call the wave early** (`Enter` during prep) — every unused prep second
  converts to a reward bonus for that wave. The core risk/reward beat.
- **Overclock** (`O`) — 22s surge, +2.5 threat, ×1.6 rewards, 80s cooldown.
- **Territory** — each held zone adds threat *and* pays income. Holding more
  ground makes the game harder on purpose.

## PLAN mode — the anti-reflex valve

`Space` drops time to 12%. Everything tactical happens here, at leisure:

- Move the build cursor (`arrows` / `WASD`), place structures (`1`–`5`, `Enter`)
- Set squad stances, assign allies to zones (`Tab` cycles, `F` rallies)
- Read enemy composition and incoming wave contents

Plan mode is unlimited and free. The game never punishes thinking — it only
punishes thinking *wrong*.

## Economy

| Resource | Earned from | Spent on |
| --- | --- | --- |
| **XP** | kills, zone income | Levels → upgrade cards (`1`/`2`/`3`) |
| **Scrap** | kills, generators, zones | Turrets, barricades, repairs |
| **Cores** | elites, bosses, zone capture | Recruiting and promoting allies, research |

## Territory

Capture zones are scattered across every arena. Standing in one — or leaving an
ally there — captures it over ~8s. A held zone:

- pays Scrap and XP income
- projects a stat aura to friendlies inside it
- adds `+0.2` threat (holding ground draws attention)
- becomes a target: enemies actively siege zones and can flip them back

## Squad

Four ally archetypes, recruited with Cores, capped by Beacons:

| Ally | Role |
| --- | --- |
| **Scout** | Cheap, fast, reveals and harasses |
| **Gunner** | Ranged sustained damage |
| **Bulwark** | Soaks and body-blocks; holds a chokepoint |
| **Medic** | Heals the player, allies and structures |

Stances: **Follow**, **Hold position**, **Guard zone**. Set in plan mode.

## Structures

| Structure | Cost | Role |
| --- | --- | --- |
| **Tack Turret** | Scrap | Rapid single-target |
| **Lobber** | Scrap | Arcing splash, hits crowds |
| **Shocker** | Scrap | Slow/stun aura, no damage |
| **Barricade** | Scrap | No gun; reshapes enemy pathing |
| **Generator** | Scrap | Scrap income, fragile, high-value target |

All structures have HP, can be repaired, and are legitimate enemy targets.

## Progression that never caps

- **Levels**: unlimited. Past the card pool, levels grant stacking Refinements.
- **Weapons**: 8 levels each, then a Mastery evolution.
- **Research tree**: 4 branches, ending in repeatable Endless nodes with rising
  costs.
- **Bosses**: cycle forever, gaining `+N` and a new modifier each rotation.
- **Enemy power**: `(1 + minutes × 0.42)^1.18 × threat_power`. Compounding, so
  minute 30 is categorically different from minute 15.

## Controls (laptop-native, no mouse)

| Key | Action |
| --- | --- |
| `WASD` / `arrows` | Move (or move build cursor in plan mode) |
| `Space` | **PLAN mode** (time to 12%) |
| `-` / `=` | Threat down / up |
| `Enter` | Call wave early / confirm placement |
| `1`–`5` | Select structure, pick upgrade card |
| `Q` / `E` | Rotate camera |
| `B` | Build palette |
| `R` | Recruit / interact |
| `F` | Rally squad to you |
| `G` | Cycle squad stance |
| `T` | Research tree |
| `O` | Overclock |
| `X` | Demolish under cursor |
| `Esc` | Pause |
