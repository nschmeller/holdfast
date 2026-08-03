#!/usr/bin/env python3
"""Client for the HOLDFAST pilot bridge.

The game exposes a directory with three files: `commands` (append to act),
`state.json` (a situation report, rewritten five times a second) and `log.txt`
(notable events). This wraps that in something terse enough to read at a
glance.

    pilot.py see  <dir>              one-screen digest of the situation
    pilot.py raw  <dir>              the whole report as JSON
    pilot.py do   <dir> "cmd" ...    append command lines and wait them out
    pilot.py log  <dir> [n]          the last n log lines (default 25)
    pilot.py shot <dir> <file.png>   screenshot, waits for the file to appear
    pilot.py keys                    the game's controls

Commands understood by `do`: tap/press/release/hold/wait/shot/note/quit.
See `src/pilot.rs` for the grammar.
"""

import json
import os
import sys
import time

CONTROLS = """\
HOLDFAST CONTROLS (keyboard only)
  MENU        left/right pick a world, ENTER deploys
  MOVE        W A S D
  DASH        SHIFT
  AIM/NUDGE   arrow keys (weapons fire themselves; you never press attack)
  PLAN MODE   SPACE toggles. Time drops to 12%, free and untimed.
                arrows move the build cursor, 1-5 pick a structure,
                ENTER places it, O cycles ally recruitment
  THREAT      MINUS lowers the dial, EQUAL raises it. Higher = harder and
                far richer. O triggers an Overclock surge when ready.
  CALL WAVE   ENTER during prep starts the assault early for a bonus
  SQUAD       F cycles stance (Follow/Hold/Guard), G guards here, R regroups
  LEVEL UP    1 2 3 pick a card, R rerolls once
  RESEARCH    T opens the tree, arrows navigate, ENTER buys, T or ESC closes
  CAMERA      Q and E rotate the overlook
  PAUSE       ESC
"""


def read_state(path):
    """Read the report, retrying briefly: it is replaced by rename, so a read
    can land in the gap between unlink and link."""
    target = os.path.join(path, "state.json")
    for _ in range(40):
        try:
            with open(target, encoding="utf-8") as handle:
                return json.load(handle)
        except (FileNotFoundError, json.JSONDecodeError):
            time.sleep(0.05)
    return None


def bar(fraction, width=10):
    filled = max(0, min(width, round(fraction * width)))
    return "#" * filled + "." * (width - filled)


def digest(s):
    if s is None:
        return "no report yet - is the game running?"

    out = []
    head = f"[{s['state']}] {s['world']}  t={s['run']['elapsed']:.0f}s  kills={s['run']['kills']}"
    out.append(head)

    p = s["player"]
    if p.get("hp") is not None:
        hp = f"{p['hp']:.0f}/{p['max_hp']:.0f}"
        out.append(
            f"HP {hp} [{bar(p['hp'] / max(p['max_hp'], 1))}]  "
            f"pos ({p['pos'][0]:.0f},{p['pos'][1]:.0f})  "
            f"dash={'ready' if p.get('dash_ready') else 'cooling'}"
        )
    out.append(
        f"LVL {p['level']} [{bar(p['xp_fraction'])}]  "
        f"unspent levels={p['pending_levels']}  skill points={p['skill_points']}"
    )

    t = s["threat"]
    out.append(
        f"THREAT {t['level']:.2f} (aiming for {t['intent']:.2f}, floor {t['floor']:.2f})  "
        f"reward x{t['reward_mult']:.2f}  "
        f"surge={'ON' if t['surging'] else ('ready' if t['surge_ready'] else 'cooling')}"
    )

    w = s["wave"]
    out.append(
        f"WAVE {w['number']} {w['phase']} {w['timer']:.0f}s left  "
        f"budget={w['budget']:.0f}  "
        f"call-now bonus +{w['bonus_if_called_now'] * 100:.0f}%"
    )

    e = s["enemies"]
    kinds = ", ".join(f"{k['count']}x {k['kind']}" for k in e["by_kind"][:6]) or "none"
    near = "-" if e["nearest_dist"] is None else f"{e['nearest_dist']:.0f}m"
    out.append(
        f"ENEMIES {e['total']} ({e['elites']} elite, {e['bosses']} boss)  "
        f"{e['within_12m']} within 12m  nearest {near}"
    )
    out.append(f"        {kinds}")

    f = s.get("fog")
    if f:
        out.append(
            f"EXPLORED {f['explored_area']:.0f} sq units  ({f['cells_in_sight']} cells in sight)"
        )

    ec = s["economy"]
    out.append(f"SCRAP {ec['scrap']:.0f} (+{ec['scrap_per_sec']:.1f}/s)  CORES {ec['cores']:.0f}")

    weapons = ", ".join(f"{x['name']} L{x['level']}" for x in s["weapons"]) or "none"
    out.append(f"WEAPONS {weapons}")

    gear = ", ".join(f"{k}={v}" for k, v in s["gear"].items() if v) or "none"
    out.append(f"GEAR {gear}")

    sq = s["squad"]
    members = ", ".join(f"{m['kind']} L{m['level']}" for m in sq["members"]) or "none"
    out.append(f"SQUAD {sq['count']}/{sq['cap']} {sq['stance']}: {members}")

    turrets = ", ".join(f"{x['kind']}@{x['dist']:.0f}m" for x in s["turrets"]) or "none"
    out.append(f"TURRETS {turrets}")

    zones = (
        ", ".join(
            f"{z['owner']}{'*' if z['contested'] else ''}@{z['dist']:.0f}m" for z in s["zones"][:5]
        )
        or "none"
    )
    out.append(f"ZONES {zones}")

    forts = s.get("forts") or []
    if forts:
        out.append(
            "FORTS "
            + ", ".join(
                f"{f['owner']}@{f['dist']:.0f}m"
                + (f" [taking {(f['capture'] + 1) / 2 * 100:.0f}%]" if f["contested"] else "")
                for f in forts[:5]
            )
        )
    nests = s.get("nests") or []
    if nests:
        out.append("NESTS " + ", ".join(f"{n['owner']}@{n['dist']:.0f}m" for n in nests[:6]))
    factions = s.get("factions") or []
    if factions:
        out.append(
            "FACTIONS "
            + ", ".join(f"{f['faction']}:{f['posture']}({f['commitment']:.0%})" for f in factions)
        )
    if s.get("wars"):
        out.append("WARS " + "; ".join(s["wars"]))

    u = s["unlocks"]
    online = ", ".join(k for k, v in u.items() if v) or "nothing yet"
    locked = ", ".join(k for k, v in u.items() if not v)
    out.append(f"UNLOCKED {online}" + (f"   (still locked: {locked})" if locked else ""))

    pm = s["plan_mode"]
    if pm["active"]:
        out.append(
            f"PLAN MODE cursor ({pm['cursor'][0]:.0f},{pm['cursor'][1]:.0f}) "
            f"placing {pm['selected']} "
            f"{'OK' if pm['valid_site'] else 'BLOCKED'}"
            + (f"  msg: {pm['message']}" if pm["message"] else "")
        )

    if s["card_offer"]:
        out.append("CARDS ON OFFER (press 1/2/3):")
        for i, c in enumerate(s["card_offer"], 1):
            out.append(f"  {i}. {c['title']} - {c['detail']}")

    if s["hint"].get("headline"):
        out.append(f"HINT {s['hint']['headline']}: {s['hint']['detail']}")

    if s["events"]:
        out.append("SINCE LAST LOOK: " + " | ".join(s["events"]))

    return "\n".join(out)


def duration_of(lines):
    """How long the queued commands will take, so `do` can wait them out."""
    total = 0.0
    for line in lines:
        parts = line.split("#")[0].split()
        if not parts:
            continue
        verb = parts[0].lower()
        if verb in ("hold", "wait") and len(parts) >= 2:
            try:
                total += float(parts[-1])
            except ValueError:
                pass
        elif verb in ("tap", "shot", "screenshot"):
            total += 0.1
    return total


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    verb = sys.argv[1]

    if verb == "keys":
        print(CONTROLS)
        return 0

    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    path = sys.argv[2]

    if verb == "see":
        print(digest(read_state(path)))
        return 0

    if verb == "raw":
        print(json.dumps(read_state(path), indent=1))
        return 0

    if verb == "log":
        count = int(sys.argv[3]) if len(sys.argv) > 3 else 25
        try:
            with open(os.path.join(path, "log.txt"), encoding="utf-8") as handle:
                print("".join(handle.readlines()[-count:]), end="")
        except FileNotFoundError:
            print("no log yet")
        return 0

    if verb == "do":
        lines = sys.argv[3:]
        if not lines:
            print("nothing to do")
            return 2
        with open(os.path.join(path, "commands"), "a", encoding="utf-8") as handle:
            for line in lines:
                handle.write(line.rstrip("\n") + "\n")
            handle.flush()
        # Wait for the queue to drain so the digest that follows describes the
        # world after the action rather than during it.
        time.sleep(duration_of(lines) + 0.45)
        for _ in range(60):
            state = read_state(path)
            if state is None or state.get("queued", 0) == 0:
                break
            time.sleep(0.25)
        print(digest(read_state(path)))
        return 0

    if verb == "shot":
        if len(sys.argv) < 4:
            print("shot needs an output path")
            return 2
        out = os.path.abspath(sys.argv[3])
        if os.path.exists(out):
            os.remove(out)
        with open(os.path.join(path, "commands"), "a", encoding="utf-8") as handle:
            handle.write(f"shot {out}\n")
        for _ in range(80):
            # The file appears the moment the GPU readback lands; give the
            # writer a moment to finish before anyone opens it.
            if os.path.exists(out) and os.path.getsize(out) > 0:
                time.sleep(0.3)
                print(out)
                return 0
            time.sleep(0.1)
        print("screenshot never appeared")
        return 1

    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
