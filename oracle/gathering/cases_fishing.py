"""Fish/FishingOption/FishingSpot rows and FishingListener / FishingPulse scenarios.

Rows, ids and option names are discovered from the pinned Kotlin sources. The chance arithmetic below is a
designer-side copy used only to aim shared-generator seeds at an outcome; the recorded results are the JVM's.
"""
import functools
import pathlib
import re

MAIN = pathlib.Path(__file__).resolve().parents[2] / "reference/server/Server/src/main"
FISHING, STRENGTH = 10, 2
STACKABLE = [313, 314, 10087, 3150]
# Declared attribute state is keyed without the "/save:" prefix, as GameAttributes stores it.
FIST = "barbtraining:fist"
NAMES = {
    "301": "Lobster pot", "303": "Small fishing net", "305": "Big fishing net", "307": "Fishing rod", "309": "Fly fishing rod",
    "311": "Harpoon", "313": "Fishing bait", "314": "Feather", "10087": "Stripy feather", "10129": "Barb-tail harpoon",
    "13431": "Crayfish cage", "3157": "Karambwan vessel", "1585": "Oily fishing rod", "3150": "Raw karambwanji",
    "13435": "Raw crayfish", "317": "Raw shrimps", "327": "Raw sardine", "345": "Raw herring", "321": "Raw anchovies",
    "353": "Raw mackerel", "335": "Raw trout", "341": "Raw cod", "349": "Raw pike", "3379": "Slimy eel", "331": "Raw salmon",
    "5004": "Frog spawn", "359": "Raw tuna", "10138": "Raw rainbow fish", "5001": "Raw cave eel", "377": "Raw lobster",
    "363": "Raw bass", "371": "Raw swordfish", "2148": "Raw lava eel", "7944": "Raw monkfish", "3142": "Raw karambwan",
    "383": "Raw shark", "395": "Raw sea turtle", "389": "Raw manta ray", "401": "Seaweed", "405": "Casket", "407": "Oyster",
    "11328": "Leaping trout", "11330": "Leaping salmon", "11332": "Leaping sturgeon", "7989": "Big bass",
    "7991": "Big swordfish", "7993": "Big shark", "12860": "Swordfish gloves", "12861": "Shark gloves",
}


def text(path):
    return (MAIN / path).read_bytes().replace(b"\r", b"").decode("utf-8")


def const(value):
    return int(re.fullmatch(r"\(?(?:Items|NPCs)\.[A-Z_0-9]*_(\d+)\)?", value.strip()).group(1))


def tables(h):
    fish = {}
    for name, a in h.rows("content/global/skill/fishing/Fish.kt", "companion object"):
        fish[name] = dict(id=const(a[0]), level=int(a[1]), low=float(a[3]), high=float(a[4]))
    body = text("content/global/skill/fishing/FishingOption.kt")
    body = body[body.index("{") + 1 : body.index("companion object")]
    options = {}
    for m in re.finditer(r'^\s*([A-Z_]+)\((Items\.[A-Z_0-9]+), (\d+), Animation\(\d+\), (null|intArrayOf\(([^"]*)\)), "([a-z]+)", (.*)\)[,;]\s*$', body, re.M):
        name, tool, level, bait, baits, option, caught = m.groups()
        options[name] = dict(tool=const(tool), level=int(level), bait=[] if bait == "null" else [const(b) for b in baits.split(",")],
                             option=option, fish=[f.strip()[5:] for f in caught.split(",")])
    body = text("content/global/skill/fishing/FishingSpot.kt")
    body = body[body.index("{") + 1 : body.index("companion object")]
    spots = {}
    for name, ids, opts in re.findall(r"([A-Z_]+)\(\s*intArrayOf\(([^)]*)\),([^)]*)\)", body):
        spots[name] = dict(ids=[const(i) for i in ids.split(",") if i.strip()], options=[o.strip()[14:] for o in opts.split(",") if o.strip()])
    return fish, options, spots


def cases(h):
    out = []
    op = h.op
    h.NAMES.update(NAMES)
    FISH, OPTIONS, SPOTS = tables(h)
    spot_of = {}
    for name, spot in SPOTS.items():
        for npc in spot["ids"]:
            spot_of[npc] = name

    def chance(fish, level):
        f = FISH[fish]
        return (level - 1.0) * ((f["high"] - f["low"]) / 98.0) + f["low"]

    @functools.lru_cache(maxsize=None)
    def seed(*draws):
        """Smallest shared seed whose first draws satisfy the plan: ('i', bound, wanted values) or ('d', below, limit)."""
        value = 0
        while True:
            r = h.JavaRandom(value)
            for draw in draws:
                if draw[0] == "i":
                    if r.next_int(draw[1]) not in draw[2]:
                        break
                elif (r.next_double() < draw[2]) != draw[1]:
                    break
            else:
                return value
            value += 1

    def ordinary(bound=100):
        return ("i", bound, tuple(v for v in range(bound) if v not in (0, 50, 90)))

    def plan(option, level, catch, stripy=False, special=None):
        """Draws that make `option.rollFish` return `catch` (None: nothing) at the given level."""
        draws = []
        if option == "BIG_NET":
            if special is not None:
                return [("i", 100, (special,))]
            draws.append(ordinary())
        for name in OPTIONS[option]["fish"]:
            if FISH[name]["level"] > level:
                continue
            if option == "LURE" and stripy != (name == "RAINBOW_FISH"):
                continue
            draws.append(("d", name == catch, chance(name, level)))
            if name == catch:
                break
        return draws

    def node(id=316, **kw):
        return dict(id=id, location=[3200, 3201, 0], **kw)

    def world(option="SMALL_NET", level=99, strength=99, tool=True, bait=50, extra=(), **kw):
        o = OPTIONS[option]
        inventory = ([[o["tool"], 1]] if tool else []) + ([[o["bait"][0], bait]] if o["bait"] and bait else []) + [list(x) for x in extra]
        base = dict(levels={str(FISHING): [level, level], str(STRENGTH): [strength, strength]}, inventory=inventory,
                    stackable=STACKABLE, usedOption=o["option"])
        base.update(kw)
        return base

    def spot_for(option):
        """The first spot id offering the option under its own name."""
        for name, spot in SPOTS.items():
            for candidate in spot["options"]:
                if OPTIONS[candidate]["option"] == OPTIONS[option]["option"]:
                    if candidate == option:
                        return spot["ids"][0]
                    break
        return None

    reachable = [name for name in OPTIONS if spot_for(name) is not None]

    def listener(target, ops, **kw):
        out.append(h.scenario("fishing", target, ops, **world(**kw)))

    def pulse(target, ops, **kw):
        out.append(h.scenario("fishingPulse", target, ops, **world(**kw)))

    # ---- tables ---------------------------------------------------------
    out.append(dict(kind="fishingCounts"))
    out += [dict(kind="fish", index=i) for i in range(len(FISH))]
    out += [dict(kind="fishingOption", index=i) for i in range(len(OPTIONS))]
    out += [dict(kind="fishingSpot", index=i) for i in range(len(SPOTS))]
    ids = sorted(spot_of)
    keys = sorted({k for i in ids for k in (i - 1, i, i + 1)} | {-2147483648, -1, 0, 2147483647})
    out.append(dict(kind="fishingLookup", table="spot", keys=keys))
    items = sorted({f["id"] for f in FISH.values()})
    out.append(dict(kind="fishingLookup", table="fish", keys=sorted({k for i in items for k in (i - 1, i, i + 1)} | {-1, 0, 7989})))
    out.append(dict(kind="fishingLookup", table="option", keys=sorted({o["option"] for o in OPTIONS.values()}) + ["", "Net", "NET", "nets", "INVALID"]))
    levels = [-2147483648, -1, 0, 1, 2, 50, 98, 99, 100, 120, 2147483647]
    out.append(dict(kind="fishingLookup", table="chance", keys=[[name, level] for name in FISH for level in levels]))
    spoken = sorted({o["option"] for o in OPTIONS.values()}) + ["Net", "NET", "nEt", "BAIT", "Harpoon", "CAGE", "Lure", "FISH", "", "INVALID", " net", "neİt", "İ"]
    out.append(dict(kind="fishingLookup", table="spotOption", keys=[[name, word] for name in SPOTS for word in spoken]))

    # ---- listener -------------------------------------------------------
    listener(node(), [op("define")])
    # Every spot id through every option it offers, driven by the modelled tick loop.
    for index, npc in enumerate(ids):
        for option in SPOTS[spot_of[npc]]["options"]:
            listener(node(npc), [op("define")] + [op("tick")] * 7, option=option, sharedSeed=index * 31 + 7)
    # Each reachable option: nothing, then each of its fish, from state 0 and from a later state.
    for option in reachable:
        target, level = node(spot_for(option)), 99
        for stripy in ([False, True] if option == "LURE" else [False]):
            extra = [(10087, 5)] if stripy else []
            catches = [None] + [f for f in OPTIONS[option]["fish"] if option != "LURE" or stripy == (f == "RAINBOW_FISH")]
            for catch in catches:
                draws = plan(option, level, catch, stripy)
                listener(target, [op("handler", state=0), op("handler", state=1), op("handler", state=2)], option=option, extra=extra,
                         sharedSeed=seed(*draws))
        for special in ([0, 50, 90] if option == "BIG_NET" else []):
            listener(target, [op("handler", state=0), op("handler", state=1)], option=option, sharedSeed=seed(("i", 100, (special,))))
        # The option's own level gate, and the fish level gates inside rollFish.
        gates = sorted({OPTIONS[option]["level"]} | {FISH[f]["level"] for f in OPTIONS[option]["fish"]})
        for gate in gates:
            for value in [gate - 1, gate, gate + 1]:
                listener(target, [op("handler", state=0), op("handler", state=1)], option=option, level=value, sharedSeed=gate)
        # Requirement order and texts.
        listener(target, [op("handler", state=0), op("handler", state=1)], option=option, tool=False)
        listener(target, [op("handler", state=0), op("handler", state=1)], option=option, bait=0)
        listener(target, [op("handler", state=0)], option=option, tool=False, bait=0, level=1)
        for filled in [26, 27, 28]:
            listener(target, [op("set", fill=filled), op("handler", state=0), op("handler", state=1), op("handler", state=2)], option=option, sharedSeed=3)
        listener(target, [op("handler", state=0), op("set", fill=28), op("handler", state=1)], option=option, sharedSeed=seed(*plan(option, 99, OPTIONS[option]["fish"][0])) if option != "LURE" else 5)
        # The bait runs out between two catches.
        if OPTIONS[option]["bait"]:
            listener(target, [op("define")] + [op("tick")] * 30, option=option, bait=1, sharedSeed=11)
            listener(target, [op("handler", state=0), op("set", take=OPTIONS[option]["bait"][0]), op("handler", state=1)], option=option, bait=1,
                     sharedSeed=seed(*plan(option, 99, OPTIONS[option]["fish"][0])) if option != "LURE" else seed(("d", True, 0.2)))
    # Tool names decide the missing-tool text: "net" anywhere and in any case, otherwise the lowered name.
    for name in ["Small fishing net", "NET", "Magnet", "Cabinet key", "Big fishing Net", "Fly fishing rod", "NÉT", "neţ", "İstanbul rod", ""]:
        listener(node(), [op("handler", state=0)], tool=False, names={"303": name})
        listener(node(309), [op("handler", state=0)], option="LURE", tool=False, names={"309": name})
    # Bait names decide " left." / "s left.".
    for bait, plain in [("Fishing bait", "Fishing bait"), ("Fishing Bait", "Fishing bait"), ("Feather", "Fishing bait"), ("fishing bait", "fishing bait"), ("", "")]:
        listener(node(), [op("handler", state=0)], option="BAIT", usedOption="bait", bait=0, names={"313": plain} if bait == plain else {"313": plain, "314": bait})
        listener(node(309), [op("handler", state=0)], option="LURE", bait=0, names={"314": bait, "313": plain})
    # LURE asks for every bait until one is held; either feather satisfies hasBait.
    for held in [[], [(314, 2)], [(10087, 2)], [(314, 1), (10087, 1)]]:
        listener(node(309), [op("define")] + [op("tick")] * 12, option="LURE", bait=0, extra=held, sharedSeed=len(held) + 40)
    # The used option: case, unknown words, and options of another spot.
    for word in ["net", "Net", "NET", "bait", "BAIT", "cage", "harpoon", "lure", "fish", "", "INVALID", "nets", "İ"]:
        listener(node(), [op("handler", state=0), op("handler", state=1)], usedOption=word, extra=[(307, 1), (313, 5)])
    listener(node(), [op("handler", state=0)], usedOption=None)
    # Not an NPC, an NPC that is no fishing spot, signed extremes.
    for target in [node(kind="scenery"), node(0), node(1), node(315 - 6), node(-1), node(2147483647), node(8000)]:
        listener(target, [op("handler", state=0), op("handler", state=1)])
    # Movement, the skilling clock, an inactive or distant spot, another plane.
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=0), op("set", clock=[0, 99]), op("handler", state=0)])
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=0)], hasScript=False)
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=3)], scriptPersist=False)
    listener(node(), [op("set", clock=[7, 101]), op("handler", state=0), op("handler", state=1), op("set", clock=[7, 100]), op("handler", state=1)])
    listener(node(active=False), [op("handler", state=0), op("handler", state=1)])
    listener(node(), [op("handler", state=0), op("set", nodeActive=False), op("handler", state=1), op("handler", state=2)], sharedSeed=seed(("d", True, 0.3)))
    for place in [[3200, 3200, 0], [3201, 3202, 0], [3202, 3201, 0], [3200, 3203, 0], [3198, 3201, 0], [3200, 3201, 1], [3200, 3201, 3],
                  [-2147483648, 3201, 0], [3200, 2147483647, 0], [2147483647, -2147483648, 0]]:
        listener(node(), [op("handler", state=0), op("handler", state=1)], location=place, sharedSeed=seed(("d", True, 0.3)))
    listener(node(), [op("handler", state=0), op("set", npcLocation=[3210, 3210, 0]), op("handler", state=1), op("handler", state=2)], sharedSeed=seed(("d", True, 0.3)))
    # hasSpaceFor / removeBait refusals reached without the state-0 check.
    listener(node(), [op("set", fill=28), op("handler", state=1)], sharedSeed=seed(("d", True, 0.3)))
    listener(node(), [op("set", fill=28), op("handler", state=1)], sharedSeed=seed(("d", True, 0.3)), scriptPersist=False)
    listener(node(), [op("handler", state=1)], option="BAIT", bait=0, sharedSeed=seed(("d", True, 0.1)))
    listener(node(309), [op("handler", state=1)], option="LURE", bait=0, sharedSeed=seed(("d", True, 0.2)))
    listener(node(309), [op("handler", state=1), op("handler", state=2)], option="LURE", bait=1, extra=[(10087, 1)], sharedSeed=seed(*plan("LURE", 99, "RAINBOW_FISH", True)))
    # Familiar boost changes the chance level, never the fish gate; int addition wraps.
    for boost in [-2147483648, -100, -1, 0, 1, 50, 2147483647]:
        for level in [1, 20, 99, 2147483647]:
            listener(node(), [op("handler", state=1)], level=level, boosts={str(FISHING): boost}, sharedSeed=boost & 0xFFFF)
    # The big fish: only for the three mapped fish, with the development-mode chance.
    for option, fish, npc in [("BIG_NET", "BASS", 313), ("HARPOON", "SWORDFISH", 312), ("SHARK_HARPOON", "SHARK", 313), ("HARPOON", "TUNA", 312)]:
        for settings, dev in [(False, False), (True, False), (True, True)]:
            bound = 10 if settings and dev else 5000
            for hit in [1, 0, 2]:
                draws = plan(option, 99, fish)
                if fish != "TUNA":
                    draws = draws + [("i", bound, (hit,))]
                elif hit != 1:
                    continue
                listener(node(npc), [op("handler", state=1)], option=option, settings=settings, devMode=dev, sharedSeed=seed(*draws))
    listener(node(313), [op("handler", state=1)], option="BIG_NET", settings=True, devMode=True, names={"363": "RAW Raw bass raw"},
             sharedSeed=seed(*(plan("BIG_NET", 99, "BASS") + [("i", 10, (1,))])))
    # Catch texts: names with "raw " and "big " anywhere, upper case, non-ASCII lowering.
    for name in ["Raw shrimps", "RAW SHRIMPS", "Big raw big shrimps", "raw raw ", "İRAW É", "Strawberry", ""]:
        listener(node(), [op("handler", state=1)], names={"317": name}, sharedSeed=seed(("d", True, 0.4)))
    for option, npc in [("BIG_NET", 313)]:
        for special in [0, 50, 90]:
            listener(node(npc), [op("handler", state=1), op("handler", state=2)], option=option, sharedSeed=seed(("i", 100, (special,))))
    # GREAT_AIM: roll(20) after the catch.
    for hit in [1, 0, 19]:
        listener(node(), [op("handler", state=1), op("handler", state=2)], perks=["GREAT_AIM"], sharedSeed=seed(("d", True, 0.4), ("i", 20, (hit,))))
        listener(node(313), [op("handler", state=1)], option="SHARK_HARPOON", perks=["GREAT_AIM"], settings=True, devMode=True,
                 sharedSeed=seed(*(plan("SHARK_HARPOON", 99, "SHARK") + [("i", 10, (1,)), ("i", 20, (hit,))])))
    listener(node(), [op("define")] + [op("tick")] * 60, perks=["GREAT_AIM"], sharedSeed=1234, attributes={"stats_manager:fish_caught": 2147483646})
    # Fist of Guthix gloves: only the matching pair pays and uses a charge.
    for option, fish, npc in [("HARPOON", "SWORDFISH", 312), ("HARPOON", "TUNA", 312), ("SHARK_HARPOON", "SHARK", 313)]:
        for gloves in [None, 12860, 12861]:
            listener(node(npc), [op("handler", state=1)], option=option, equipment={"9": gloves} if gloves else {},
                     sharedSeed=seed(*(plan(option, 99, fish) + ([("i", 5000, (0,))] if fish != "TUNA" else []))))
    # Barehand fishing: unlock attribute, any harpoon held, the two level pairs, per-fish requirements.
    for option, npc, pair in [("HARPOON", 312, (55, 35)), ("SHARK_HARPOON", 313, (96, 76))]:
        for fist in [None, 0, 1, 2, -1, 2147483647]:
            attributes = {} if fist is None else {FIST: fist}
            listener(node(npc), [op("handler", state=0), op("handler", state=1)], option=option, tool=False, attributes=attributes, sharedSeed=seed(("d", True, 0.05)))
        for extra, equipment in [([(311, 1)], {}), ([(10129, 1)], {}), ([], {"3": 10129}), ([], {"3": 311})]:
            listener(node(npc), [op("handler", state=0), op("handler", state=1)], option=option, tool=False, extra=extra, equipment=equipment,
                     attributes={FIST: 1}, sharedSeed=seed(("d", True, 0.05)))
        for level in [pair[0] - 1, pair[0]]:
            for strength in [pair[1] - 1, pair[1]]:
                listener(node(npc), [op("handler", state=0), op("handler", state=1)], option=option, tool=False, level=level, strength=strength,
                         attributes={FIST: 1}, sharedSeed=seed(("d", True, 0.05)))
    for level, strength in [(55, 35), (69, 99), (70, 49), (70, 50), (99, 99)]:
        for catch in ["TUNA", "SWORDFISH", None]:
            if catch == "SWORDFISH" and level < 50:
                continue
            for fist in [1, 2]:
                listener(node(312), [op("handler", state=0), op("handler", state=1), op("handler", state=2)], option="HARPOON", tool=False, level=level,
                         strength=strength, attributes={FIST: fist}, sharedSeed=seed(*(plan("HARPOON", level, catch) + ([("i", 5000, (0,))] if catch == "SWORDFISH" else []))))
    listener(node(313), [op("handler", state=0), op("handler", state=1)], option="SHARK_HARPOON", tool=False, attributes={FIST: 1},
             sharedSeed=seed(*(plan("SHARK_HARPOON", 99, "SHARK") + [("i", 5000, (0,))])))
    listener(node(312), [op("handler", state=0), op("handler", state=1)], option="HARPOON", tool=False, attributes={FIST: "1"})
    # The barehand loop animation waits for the animation clock; the skilling clock decides which branch asks.
    for end in [99, 100, 101]:
        for skilling in [100, 101]:
            listener(node(312), [op("set", clock=[1, end]), op("set", clock=[7, skilling]), op("handler", state=1)], option="HARPOON", tool=False,
                     attributes={FIST: 2}, sharedSeed=seed(("d", False, 0.9), ("d", False, 0.9)))
    listener(node(), [op("set", clock=[1, 100]), op("handler", state=1), op("set", clock=[1, 99]), op("handler", state=1)], sharedSeed=9)
    # A foraging familiar walks beside the player and acts on every catch attempt.
    for familiar in ["forager", "other"]:
        for step in [[0, 1], [1, -1], [-1, 0], [2147483647, -2147483648]]:
            listener(node(), [op("handler", state=0), op("handler", state=1)], familiar=familiar, direction=step, sharedSeed=seed(("d", True, 0.4)))
        listener(node(), [op("handler", state=0)], familiar=familiar, hasFamiliar=False)
        listener(node(312), [op("handler", state=0), op("handler", state=1)], option="HARPOON", tool=False, attributes={FIST: 1}, familiar=familiar,
                 sharedSeed=seed(("d", True, 0.05)))
    listener(node(), [op("handler", state=0)], hasFamiliar=True)
    listener(node(), [op("handler", state=0)], familiar="forager", location=[2147483647, 3200, 3], direction=[1, 1])
    # Long runs until the inventory fills or the bait is gone.
    for index, (option, npc) in enumerate([("SMALL_NET", 316), ("BAIT", 316), ("LURE", 309), ("LOBSTER_CAGE", 312), ("BIG_NET", 313), ("KARAMBWAN_VES", 1177)]):
        listener(node(npc), [op("define")] + [op("tick")] * 220, option=option, bait=12, sharedSeed=900 + index)
        listener(node(npc), [op("set", fill=25), op("define")] + [op("tick")] * 80, option=option, sharedSeed=950 + index, perks=["GREAT_AIM"])
    # Injected failures at every position of two full passes.
    full = dict(option="SHARK_HARPOON", tool=False, attributes={FIST: 1}, familiar="forager", perks=["GREAT_AIM"], settings=True, devMode=True,
                equipment={"9": 12861}, sharedSeed=seed(*(plan("SHARK_HARPOON", 99, "SHARK") + [("i", 10, (1,)), ("i", 20, (1,))])))
    for at in range(0, 64):
        listener(node(313), [op("handler", state=0, failAt=at)], **full)
    plain = dict(option="LURE", extra=[(10087, 3)], sharedSeed=seed(*plan("LURE", 99, "RAINBOW_FISH", True)))
    for at in range(0, 40):
        listener(node(309), [op("handler", state=0, failAt=at)], **plain)

    # ---- pulse ----------------------------------------------------------
    run = [op("pulseStart")] + [op("pulsePulse")] * 4 + [op("pulseStop")]
    for index, option in enumerate(OPTIONS):
        npc = spot_for(option) or 316
        pulse(node(npc), [op("pulseNew", option=option)] + run, option=option, sharedSeed=index + 60)
        for stripy in ([False, True] if option == "LURE" else [False]):
            extra = [(10087, 5)] if stripy else []
            catches = [None] + [f for f in OPTIONS[option]["fish"] if option != "LURE" or stripy == (f == "RAINBOW_FISH")]
            for catch in catches:
                for task in [False, True]:
                    pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse"), op("pulsePulse"), op("pulsePulse")], option=option,
                          extra=extra, hasTask=task, sharedSeed=seed(*plan(option, 99, catch, stripy)))
        for special in ([0, 50, 90] if option == "BIG_NET" else []):
            pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], option=option, hasTask=True,
                  sharedSeed=seed(("i", 100, (special,))))
        for value in [OPTIONS[option]["level"] - 1, OPTIONS[option]["level"]]:
            pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse")], option=option, level=value)
        pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse")], option=option, tool=False)
        pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse")], option=option, bait=0)
        for filled in [27, 28]:
            pulse(node(npc), [op("pulseNew", option=option), op("set", fill=filled), op("pulseStart"), op("pulsePulse"), op("pulsePulse"), op("pulsePulse")],
                  option=option, sharedSeed=seed(*plan(option, 99, OPTIONS[option]["fish"][0])) if option != "LURE" else 8)
    # A null option, and members that dereference it.
    for member in [op("pulseStart"), op("pulsePulse"), op("pulseCheck"), op("pulseAnimate"), op("pulseReward"), op("pulseMessage", type=0),
                   op("pulseMessage", type=1), op("pulseMessage", type=2), op("pulseStop")]:
        pulse(node(), [op("pulseNew"), member, op("set", delay=5), member])
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseMessage", type=2), op("pulseMessage", type=0), op("pulseMessage", type=3)])
    # The spot: the same Location object, an equal new one, a moved one, inactive, invisible.
    for change in [dict(npcLocation=[3200, 3201, 0]), dict(npcLocation=[3200, 3202, 0]), dict(nodeActive=False), dict(npcInvisible=True)]:
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("set", **change), op("pulsePulse"), op("pulsePulse")])
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("set", **change), op("pulseStart")])
    pulse(node(active=False, invisible=True), [op("pulseNew", option="SMALL_NET"), op("pulseStart")])
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart")], location=[3300, 3300, 2])
    # Barb-tail harpoon stands in for the harpoon in the pulse as well.
    for option, npc in [("HARPOON", 312), ("SHARK_HARPOON", 313), ("BARB_HARPOON", 312), ("LOBSTER_CAGE", 312)]:
        for extra, equipment in [([(10129, 1)], {}), ([], {"3": 10129}), ([], {"3": 311}), ([], {})]:
            pulse(node(npc), [op("pulseNew", option=option), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], option=option, tool=False, extra=extra,
                  equipment=equipment, sharedSeed=seed(("d", True, 0.05)))
    # GREAT_AIM in the pulse: random(100) <= 5.
    for value in [0, 5, 6, 99]:
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], perks=["GREAT_AIM"],
              sharedSeed=seed(("d", True, 0.4), ("i", 100, (value,))))
    # The counter attribute wraps; a value of the wrong type fails the cast.
    for held in [0, 41, 2147483647, -1]:
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], attributes={"stats_manager:fish_caught": held},
              sharedSeed=seed(("d", True, 0.4)))
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], attributes={"stats_manager:fish_caught": "x"},
          sharedSeed=seed(("d", True, 0.4)))
    # The last free slot: the catch fills the inventory, the dialogue names lobsters only for a lobster.
    for option, npc in [("SMALL_NET", 316), ("LOBSTER_CAGE", 312), ("BAIT", 316)]:
        pulse(node(npc), [op("pulseNew", option=option), op("set", fill=27), op("pulseStart"), op("pulsePulse"), op("pulsePulse"), op("pulsePulse")],
              option=option, sharedSeed=seed(*plan(option, 99, OPTIONS[option]["fish"][0])))
    # Space and bait refusals inside reward, reached directly.
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("set", delay=5), op("set", fill=28), op("pulseReward")], sharedSeed=seed(("d", True, 0.4)))
    pulse(node(), [op("pulseNew", option="BAIT"), op("set", delay=5), op("pulseReward")], option="BAIT", bait=0, sharedSeed=seed(("d", True, 0.1)))
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseReward"), op("pulseReward")], sharedSeed=seed(("d", True, 0.4)))
    for delay in [-1, 0, 1, 2, 5]:
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("set", delay=delay), op("pulseReward")], sharedSeed=seed(("d", False, 0.9), ("d", False, 0.9)))
    # Familiar boost in the pulse roll.
    for boost in [-2147483648, -1, 1, 2147483647]:
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], boosts={str(FISHING): boost}, sharedSeed=boost & 0xFFFF)
    # Familiars: a forager walks from its own location in start and acts in reward.
    for familiar in ["forager", "other"]:
        for step in [[0, 1], [-1, -1], [2147483647, 2147483647]]:
            pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("pulsePulse"), op("pulsePulse")], familiar=familiar, direction=step,
                  familiarLocation=[3190, 3190, 1], sharedSeed=seed(("d", True, 0.4)))
        pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart")], familiar=familiar, hasFamiliar=False)
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart")], hasFamiliar=True)
    pulse(node(), [op("pulseNew", option="SMALL_NET"), op("pulseStart"), op("set", hasFamiliar=True), op("pulsePulse"), op("pulsePulse")], sharedSeed=4)
    # isBareHanded / checkFish and the submitted catch pulse, through the public animate member.
    pairs = [(54, 35), (55, 34), (55, 35), (69, 50), (70, 49), (70, 50), (95, 76), (96, 75), (96, 76), (99, 99)]
    for option, npc in [("HARPOON", 324), ("SHARK_HARPOON", 313), ("HARPOON", 312), ("BARB_HARPOON", 324), ("SMALL_NET", 324)]:
        for level, strength in pairs:
            pulse(node(npc), [op("pulseNew", option=option), op("pulseAnimate")] + [op("runPulse", index=0)] * 7, option=option, tool=False, level=level,
                  strength=strength, sharedSeed=level)
    for extra, equipment in [([(311, 1)], {}), ([], {"3": 311}), ([(10129, 1)], {}), ([], {"3": 10129})]:
        pulse(node(324), [op("pulseNew", option="HARPOON"), op("pulseAnimate"), op("runPulse", index=0)], option="HARPOON", tool=False, extra=extra, equipment=equipment)
    pulse(node(324), [op("pulseNew", option="HARPOON"), op("pulseAnimate"), op("pulseAnimate")] + [op("runPulse", index=0), op("runPulse", index=1)] * 6
          + [op("set", fill=28), op("runPulse", index=1)], option="HARPOON", tool=False, sharedSeed=77)
    pulse(node(324), [op("pulseNew", option="HARPOON"), op("pulseAnimate")] + [op("runPulse", index=0)] * 5 + [op("set", fill=28), op("runPulse", index=0)],
          option="HARPOON", tool=False, level=60, strength=40)
    # Long pulse runs.
    for index, (option, npc) in enumerate([("SMALL_NET", 316), ("LURE", 309), ("LOBSTER_CAGE", 312), ("BIG_NET", 313)]):
        pulse(node(npc), [op("pulseNew", option=option), op("pulseStart")] + [op("pulsePulse")] * 120, option=option, bait=9, hasTask=True, sharedSeed=700 + index)
    # Injected failures at every position of a full pulse pass.
    full = dict(option="LURE", extra=[(10087, 3)], hasTask=True, perks=["GREAT_AIM"], familiar="forager",
                sharedSeed=seed(*(plan("LURE", 99, "RAINBOW_FISH", True) + [("i", 100, (3,))])))
    for at in range(0, 12):
        pulse(node(309), [op("pulseNew", option="LURE"), op("pulseStart", failAt=at)], **full)
    for at in range(0, 48):
        pulse(node(309), [op("pulseNew", option="LURE"), op("pulseStart"), op("pulsePulse"), op("pulsePulse", failAt=at)], **full)
    for at in range(0, 22):
        pulse(node(324), [op("pulseNew", option="HARPOON"), op("pulseAnimate")] + [op("runPulse", index=0)] * 5 + [op("runPulse", index=0, failAt=at)],
              option="HARPOON", tool=False, level=80, strength=60)
    return out
