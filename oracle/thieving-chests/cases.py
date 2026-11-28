"""Designed chest thieving corpus. Chest rows are read from the pinned source, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/server/Server/src/main/content/global/skill/thieving/ThievableChestPlugin.java"


def constants():
    """Each constant's name, object id, level and respawn, from the pinned enum."""
    text = SOURCE.read_bytes().replace(b"\r", b"").decode("utf-8")
    row = re.search(r"^\t\tTEN_COIN\(.*;$", text, re.M).group()
    out = []
    for match in re.finditer(r"([A-Z_]+)\((\d+), (\d+), [\d.]+, new Item\[\] \{.*?\}, (\d+)\)", row):
        name, node, level, respawn = match.groups()
        out.append(dict(name=name, id=int(node), level=int(level), respawn=int(respawn)))
    if len(out) != 6:
        raise RuntimeError("Chest differs from the 6 constants this corpus expects")
    return out


def state(**changes):
    base = dict(ticks=1000, playerActive=True, nodeActive=True, level=99, lifepoints=10, freeSlots=5)
    base.update(changes)
    return base


def change(**values):
    return dict(op="set", state=values)


def handle(node, option="search for traps"):
    return dict(op="handle", node=node, option=option)


UPDATE = dict(op="update")


def requests():
    chests = constants()
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="chests")])]
    listed = [c["id"] for c in chests]
    probe = sorted({i + d for i in listed for d in (-1, 0, 1)} | {-1, 0, 2574, 65535})
    cases.append(dict(state=state(), ops=[dict(op="forId", node=i) for i in probe]))
    # The plugin for every probed id and option: a search of a scenery that is no chest throws.
    for option in ["open", "search for traps", "Open", "search", ""]:
        cases.append(dict(state=state(), ops=[handle(i, option) for i in probe]))
    for c in chests:
        node, level, wait = c["id"], c["level"], int(c["respawn"] / 0.6)
        respawning = dict(op="respawning", chest=c["name"])
        # The trap of an opened chest by lifepoints.
        for life in [0, 1, 23, 24, 35, 36, 99, 990]:
            cases.append(dict(state=state(lifepoints=life), ops=[handle(node, "open")]))
        # The search's checks, in order.
        for have in sorted({1, level - 1, level, level + 1, 99}):
            for free in [0, 1, 28]:
                cases.append(dict(state=state(level=have, freeSlots=free), ops=[handle(node)] + [UPDATE] * 7))
        # The whole search, then the respawn seen from each tick around its end, by both options and by another
        # constant's chest.
        other = chests[(chests.index(c) + 1) % len(chests)]["id"]
        for active in [True, False]:
            for later in [0, wait - 1, wait, wait + 1]:
                ops = [handle(node)] + [UPDATE] * 6 + [respawning, change(ticks=1000 + later), respawning,
                                                       handle(node, "open"), handle(node), handle(other, "open"), UPDATE]
                cases.append(dict(state=state(nodeActive=active), ops=ops))
        # The tick moving during the pulse, and a player who left.
        for tick in range(6):
            ops = [handle(node)] + [UPDATE] * tick + [change(ticks=1003)] + [UPDATE] * (7 - tick) + [respawning]
            cases.append(dict(state=state(), ops=ops))
            ops = [handle(node)] + [UPDATE] * tick + [change(playerActive=False)] + [UPDATE] * 2
            cases.append(dict(state=state(), ops=ops))
        # A second search while the first pulse runs: the later pulse is the one the driver holds.
        cases.append(dict(state=state(), ops=[handle(node), UPDATE, UPDATE, handle(node)] + [UPDATE] * 7 + [respawning]))
    for number, case in enumerate(cases):
        case["id"] = number
    return cases


def census(out):
    kinds = {}
    for case in out:
        for op, result in zip(case["ops"], case["expected"]):
            key = op["op"] + (":throws" if "throws" in result else "")
            kinds[key] = kinds.get(key, 0) + 1
    return dict(sorted(kinds.items()))
