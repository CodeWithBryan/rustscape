"""Designed pickable door corpus. Door rows are read from the pinned source, not typed here."""
import pathlib
import re

ROOT = pathlib.Path(__file__).resolve().parents[2]
SOURCE = ROOT / "reference/server/Server/src/main/content/global/skill/thieving/PickableDoorHandler.java"


def doors():
    """Each pickable door's tile and level, and the scenery ids, from the pinned source."""
    text = SOURCE.read_bytes().replace(b"\r", b"").decode("utf-8")
    out = []
    for match in re.finditer(r"pickableDoors\.add\(new PickableDoor\(new Location\[\]\{Location\.create\((\d+), (\d+), (\d+)\)\}, (\d+), ([\d.]+)(.*?)\)\);", text):
        x, y, z, level, _, rest = match.groups()
        flags = [f.strip() == "true" for f in rest.split(",")[1:]]
        out.append(dict(at=[int(x), int(y), int(z)], level=int(level), lockpick=bool(flags[:1] and flags[0]), flipped=len(flags) > 1 and flags[1]))
    if len(out) != 12:
        raise RuntimeError("PickableDoorHandler differs from the 12 doors this corpus expects")
    ids = [int(i) for i in re.search(r"DOORS = new int\[\]\{([^}]*)\}", text).group(1).split(",")]
    if len(ids) != 28:
        raise RuntimeError("PickableDoorHandler differs from the 28 ids this corpus expects")
    return out, ids


def state(**changes):
    base = dict(player=[0, 0, 0], level=99, lockpick=True, hasAction=True, destination=[1, 2, 0], draws=[0])
    base.update(changes)
    return base


def handle(at, rotation, option, node=2550):
    return dict(op="handle", node=node, at=at, rotation=rotation, option=option)


# Where the player stands relative to the door: each logical direction, its ties and the door's own tile.
OFFSETS = [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (-1, 1), (1, -1), (-1, -1), (2, 1), (-2, 1), (1, 2), (1, -2), (-2, -1)]


def requests():
    pickable, ids = doors()
    cases = [dict(state=state(), ops=[dict(op="registrations"), dict(op="doors")])]
    # A scenery that is no listed door, by option; and every listed tile on another plane.
    strangers = [[0, 0, 0], [2672, 3308, 1], [2672, 3309, 0], [2671, 3308, 0]]
    for option in ["open", "pick-lock", "Open", "pick lock", "close", ""]:
        cases.append(dict(state=state(), ops=[handle(at, 0, option) for at in strangers]
                          + [handle(pickable[0]["at"], 0, option, node) for node in [ids[0], ids[-1], 1, -1]]))
    for door in pickable:
        at = door["at"]
        # Which side is inside: every rotation by every offset, for both options.
        for rotation in range(8):
            for option in ["open", "pick-lock"]:
                ops = []
                for dx, dy in OFFSETS:
                    ops.append(dict(op="set", state=dict(player=[at[0] + dx, at[1] + dy, at[2]])))
                    ops.append(handle(at, rotation, option))
                cases.append(dict(state=state(draws=[4]), ops=ops))
        # The pick's checks in order, from a locked side: east of the door is outside at rotation 0 and inside at
        # rotation 2, which is the locked side of a flipped door.
        rotation, stand = 2 if door["flipped"] else 0, [at[0] + 1, at[1], at[2]]
        for level in sorted({1, door["level"] - 1, door["level"], door["level"] + 1, 99}):
            for lockpick in [True, False]:
                for first in [0, 3, 4, 11]:
                    for second in [4, 5]:
                        for third in [0, 1]:
                            cases.append(dict(state=state(player=stand, level=level, lockpick=lockpick, draws=[first, second, third]),
                                              ops=[handle(at, rotation, "pick-lock")]))
    # The destination: only scenery whose definition has the pick-lock action asks DoorActionHandler.
    for has in [True, False]:
        for destination in [[1, 2, 0], [3044, 3956, 0]]:
            cases.append(dict(state=state(hasAction=has, destination=destination),
                              ops=[dict(op="destination", node=node, at=[5, 6, 0], rotation=r) for node in [ids[0], 1] for r in [0, 3]]))
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
