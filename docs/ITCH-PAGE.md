# itch.io store page

Copy and settings for publishing HOLDFAST. Paste into the itch.io project form.
Everything here is draft-quality prose written to be edited, not a final word.

## Upload settings

| Field | Value |
|---|---|
| File | `holdfast-web.zip` (from `bash tools/build-web.sh`) |
| Kind of project | HTML |
| This file will be played in the browser | **ticked** |
| Viewport | **1280 × 720** |
| Fullscreen button | ticked |
| Mobile friendly | **unticked** — keyboard only, no touch controls yet |
| Automatically start on page load | unticked (audio needs a click first) |
| Genre | Strategy |
| Tags | `roguelite`, `survivors-like`, `strategy`, `tower-defense`, `keyboard-only`, `procedural-generation`, `no-assets`, `bevy`, `rust` |
| Pricing | Free, donations welcome |

## Title

**HOLDFAST**

## Tagline

*(itch calls this the "short description or tagline", 120 characters or so)*

> You choose how hard it gets. Hold ground on a desk that never ends.

## Description

> **You own the throttle.**
>
> HOLDFAST is a survival command game. You do not aim and you do not press
> attack — your weapons fire themselves. What you decide is *how much trouble to
> be in*, and almost every system in the game is a lever that makes it worse in
> exchange for making it richer.
>
> Turn the threat dial up and everything pays more. Call the next wave in early
> for a bonus. Hold territory and it feeds you, loudly. Take a fort and it works
> for you — and the faction that lost it will come for it. Set two monster
> factions at war and walk through the middle while they are busy.
>
> The world is generated as you walk and it does not end. Five places to fight —
> a desk, an undergrowth, a rooftop, a circuit grid, an arcane sanctum — each
> with its own monsters, weapons and hazards under the same rules, so what you
> learn in one applies in all of them.
>
> Plan mode is free and untimed. Time drops to a crawl and you can think as long
> as you like. This is a strategy game with opportunities to get into the action,
> not a test of reflexes.
>
> **Controls are keyboard only.** `WASD` moves, `SPACE` plans, `T` researches,
> `-` and `=` set the pace. Full list below.

## Controls

> | | |
> |---|---|
> | `WASD` | Move — you attack automatically |
> | `SHIFT` | Dash |
> | `SPACE` / `B` | Plan mode. Time drops to 12%, free and untimed |
> | `arrows` | Aim the build cursor in plan mode |
> | `1`–`5` then `ENTER` | Pick and place a structure |
> | `ENTER` in prep | Call the wave early for a reward bonus |
> | `-` / `=` | Lower and raise the threat dial |
> | `O` | Overclock — a hard threat spike, on a cooldown |
> | `R` | Recruit a squadmate (costs Cores) |
> | `F` / `G` | Rally the squad / cycle its stance |
> | `T` | Research tree |
> | `M` | Mute |
> | `1` `2` `3` | Choose a card on level-up (`R` rerolls once) |
> | `ESC` | Pause |

## Screenshots to upload

In `dist/press/`. Ordered so the first is the one itch shows largest.

1. `01-fort-siege.png` — a fort with its capture ring, mid-siege
2. `02-plan-mode.png` — plan mode with the build cursor and a turret ring
3. `03-squad.png` — a squad and turrets, showing the green friendly rings
4. `04-levelup.png` — a card screen
5. `05-research.png` — the research tree
6. `06-worlds.png` — a world other than the desk

## Cover image

`dist/press/cover.png`, 630 × 500. itch crops this to a variety of shapes, so
the important content sits in the middle.

## What is deliberately not claimed

Worth keeping honest on the page, because players notice:

- No touch support. It says keyboard only, and means it.
- No controller support.
- The run is endless; there is no ending to spoil or promise.
