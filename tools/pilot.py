#!/usr/bin/env python3
"""Client for the HOLDFAST pilot bridge.

The game exposes a directory with three files: `commands` (append to act),
`state.json` (a situation report, rewritten five times a second) and `log.txt`
(notable events). This wraps that in something terse enough to read at a
glance.

    pilot.py see  <dir>              one-screen digest of the situation
    pilot.py raw  <dir>              the whole report as JSON
    pilot.py do   <dir> "cmd" ...    append command lines and wait them out
                                     (including the steering verbs - `do` does
                                     not return until the game is idle again)
    pilot.py log  <dir> [n]          the last n log lines (default 25)
    pilot.py todo <dir>              content not yet seen this session
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
  SQUAD       R recruits (costs Cores, cap 4). F rallies everyone to you and
                sets Follow. G cycles the squad stance Follow -> Hold -> Guard;
                Hold anchors them where they stand, which is what garrisons a
                fort. Guard sends them to zones only.
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

    # A blocked state first, and loudly. The game pauses on a level-up until a
    # card is taken, and a reader that misses that will sit there sending
    # movement commands into a paused game and conclude the game is broken -
    # which is exactly what happened the first time this ran.
    blocked = {
        "LevelUp": ">>> BLOCKED: press 1, 2 or 3 to take a card. Nothing else works. <<<",
        "SkillTree": ">>> BLOCKED: research is open. ENTER buys, T or ESC closes. <<<",
        "Paused": ">>> BLOCKED: paused. ESC resumes, BACKSPACE abandons the run and returns to world select. <<<",
        "GameOver": ">>> RUN OVER: ENTER starts another. <<<",
        "Menu": ">>> AT THE MENU: left/right pick a world, ENTER deploys. <<<",
    }.get(s["state"])
    if blocked:
        out.append(blocked)

    head = f"[{s['state']}] {s['world']}  t={s['run']['elapsed']:.0f}s  kills={s['run']['kills']}"
    frozen = s.get("game_time_frozen_for")
    if frozen is not None and frozen > 2.0:
        # The run clock stops on any modal screen. A tester measuring a speed
        # across one gets zero movement in zero time and concludes the movement
        # system is dead.
        head += f"  (game time has been stopped for {frozen:.0f}s of real time)"
    fps = s.get("frames_per_sec")
    if fps is not None and fps < 30.0:
        # Worth shouting about: a simulation advancing in big steps makes every
        # measurement taken from it suspect.
        head += f"  !! ONLY {fps:.0f} FPS - measurements from this run are suspect"
    out.append(head)

    # At the menu there is no run, and the resources still hold the last one's
    # values. Reporting them reads as ghost state, which is how it got filed as
    # a bug. Say what is actually true instead.
    if s["state"] == "Menu":
        out.append(
            "No run in progress. The numbers below belong to the previous run "
            "until you deploy."
        )

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

    cov = s.get("coverage")
    if cov:
        out.append(
            f"CONTENT SEEN {cov['fraction']:.0%} ({cov['seen']}) - "
            f"`pilot.py todo <dir>` lists what is left"
        )

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
    # ENTER means two different things and the game never said which. A
    # newcomer pressed it meaning "place the turret" and called a wave early
    # instead, then could not work out why. So say what it does right now.
    can_build = s["unlocks"]["build"]
    if pm["active"]:
        out.append(
            f"PLAN MODE IS ON - cursor ({pm['cursor'][0]:.0f},{pm['cursor'][1]:.0f}), "
            f"placing {pm['selected']}, site is "
            f"{'clear' if pm['valid_site'] else 'BLOCKED - move the cursor'}"
        )
        if not can_build:
            # `valid_site` is about geometry, not permission - a newcomer read
            # "site is clear", pressed ENTER, got nothing, and had no way to
            # learn that building was not unlocked yet.
            out.append("     BUT BUILDING IS NOT UNLOCKED YET - ENTER will do nothing.")
        else:
            out.append("     ENTER places it here. SPACE leaves plan mode.")
        # The game's own refusal, if it gave one. This is the only place it says
        # why a placement failed, so it gets its own line.
        if pm["message"]:
            out.append(f"     the game says: {pm['message']}")
    else:
        out.append(
            "PLAN MODE IS OFF - SPACE turns it on"
            + (" to build." if can_build else ", but building is not unlocked yet.")
        )
        if s["wave"]["phase"].lower().startswith("prep"):
            out.append(
                "     ENTER right now CALLS THE WAVE EARLY "
                f"(+{s['wave']['bonus_if_called_now'] * 100:.0f}% rewards), "
                "it does not build."
            )

    if s["card_offer"]:
        out.append("CARDS ON OFFER (press 1/2/3):")
        for i, c in enumerate(s["card_offer"], 1):
            out.append(f"  {i}. {c['title']} - {c['detail']}")

    if s["hint"].get("headline"):
        out.append(f"HINT {s['hint']['headline']}: {s['hint']['detail']}")

    if s["events"]:
        out.append("SINCE LAST LOOK: " + " | ".join(s["events"]))
    # Sticky, and above everything else in importance: a command the game
    # refused looks exactly like a key that did nothing.
    for pool in (s.get("light_pools") or [])[:2]:
        where = "STANDING IN IT" if pool.get("standing_in_it") else f"{pool['dist']:.0f}m away"
        out.append(
            f"LIGHT POOL {where}, radius {pool['radius']:.0f}, "
            f"x{pool['damage_mult_inside']:.2f} damage and +{pool['regen_inside']:.1f} hp/s "
            f"inside, for +{pool['threat_inside']:.2f} threat while you stand there"
        )
    for hole in (s.get("chasms") or [])[:2]:
        out.append(
            f"CHASM {hole['dist']:.0f}m away, radius {hole['radius']:.0f} "
            f"({hole['to_edge']:.0f}m to the edge) - knockback here kills outright"
        )

    war = s.get("war_available")
    if war:
        out.append(f"A WAR CAN BE INCITED NOW: {war} (Research, Command branch)")
    if s.get("problems"):
        # Timestamped, and labelled as a history, because an unstamped list gets
        # read as a reaction to whatever was typed most recently.
        out.append("!! REFUSED EARLIER IN THIS RUN (with the time it happened):")
        for problem in s["problems"]:
            out.append("     " + problem)

    return "\n".join(out)


def is_blank(path):
    """True when a PNG decodes to a single flat colour.

    Enough PNG to answer one question, which is cheaper than a dependency and
    is the only image inspection this tool ever needs to do.
    """
    import struct
    import zlib

    try:
        raw = open(path, "rb").read()
        pos, idat = 8, b""
        while pos < len(raw):
            length = struct.unpack(">I", raw[pos : pos + 4])[0]
            kind = raw[pos + 4 : pos + 8]
            if kind == b"IDAT":
                idat += raw[pos + 8 : pos + 8 + length]
            pos += 12 + length
        # A few hundred kilobytes is plenty to tell flat from not.
        sample = zlib.decompressobj().decompress(idat, 400_000)
        return len(set(sample)) <= 2
    except Exception:
        return False


# Steering verbs and where their duration sits in the arguments. `goto` and
# `defend` take a position first, so their time is optional and third.
STEER_SECONDS_AT = {
    "roam": 1,
    "chase": 1,
    "flee": 1,
    "kite": 1,
    "goto": 3,
    "defend": 3,
}

# What `goto` and `defend` fall back to when no time is given. `goto` budgets
# itself from the distance, which can be minutes, so waiting on a guess is
# worse than not waiting - the caller is told to sleep instead.
DEFEND_DEFAULT = 20.0


def duration_of(lines):
    """How long the queued commands will take, so `do` can wait them out.

    Steering verbs used to count as zero, so `do "kite 40"` returned at once
    and every caller had to know to sleep afterwards - which nobody did, so
    reports were written about a game state forty seconds in the past.
    """
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
        elif verb in STEER_SECONDS_AT:
            at = STEER_SECONDS_AT[verb]
            try:
                total += float(parts[at])
            except (IndexError, ValueError):
                # `goto` with no limit budgets itself from the distance; the
                # wait loop stops on an empty queue anyway, so lean long.
                total += DEFEND_DEFAULT if verb == "defend" else 90.0
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

    if verb == "todo":
        state = read_state(path)
        cov = (state or {}).get("coverage")
        if not cov:
            print("no coverage data")
            return 1
        missing = cov["missing"]
        print(
            f"COVERAGE {cov['fraction']:.0%}  ({cov['seen']} seen, {len(missing)} to go)"
        )
        groups = {}
        for tag in missing:
            head, _, rest = tag.partition(":")
            groups.setdefault(head, []).append(rest)
        for head in sorted(groups):
            print(f"  {head:8} {', '.join(groups[head])}")
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
        # Wait for the queue to drain AND the last command to finish, so the
        # digest that follows describes the world after the action rather than
        # partway through it. `queued` alone goes to zero as soon as the last
        # command becomes the active one, which is why a `kite 40` used to
        # return immediately.
        # First give the game a moment to notice the append. Without this the
        # very first poll sees an idle pilot - it has not read the file yet -
        # and returns instantly, which is the bug this is here to avoid.
        pickup = time.time() + 2.0
        while time.time() < pickup:
            state = read_state(path)
            if state and (state.get("busy") or state.get("queued", 0) > 0):
                break
            time.sleep(0.15)
        # Then wait for it to go idle again.
        deadline = time.time() + duration_of(lines) + 20.0
        while time.time() < deadline:
            state = read_state(path)
            if state is None:
                break
            if state.get("queued", 0) == 0 and not state.get("busy", False):
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
                if is_blank(out):
                    print(
                        "WARNING: that capture is a single flat colour, which means the\n"
                        "capture failed - not that the game renders blank. It happens on\n"
                        "windows that were resized after creation: the screenshot reads a\n"
                        "stale surface at the original size. Judge this frame by the `see`\n"
                        "digest instead, and do not report it as a rendering bug."
                    )
                return 0
            time.sleep(0.1)
        print("screenshot never appeared")
        return 1

    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main())
