"""Scenarios of the binding corpus: listeners and the fishing pulse over the original Skills, Container and Item.

Only inputs are declared here; levels, experience, slots and every recorded call come from the JVM.
"""
STACKABLE = [313, 314, 10087, 3150]
TUTORIAL = {"tutorial:complete": True}


def cases(h):
    out = []
    op = h.op
    always = h.tlr_seed_for_unit(0)

    def world(tool=None, wield=None, levels=None, inventory=(), tutorial=True, **kw):
        held = ([[tool, 1]] if tool else []) + [list(x) for x in inventory]
        base = dict(levels={str(k): v for k, v in (levels or {}).items()}, inventory=held, equipment={"3": wield} if wield else {},
                    stackable=STACKABLE, attributes=dict(TUTORIAL) if tutorial else {})
        base["attributes"].update(kw.pop("attributes", {}))
        base.update(kw)
        return base

    def add(subject, target, ops, **kw):
        out.append(h.scenario(subject, dict(id=target, location=[3200, 3201, 0]), ops, **world(**kw)))

    ticks = lambda n: [op("define")] + [op("tick")] * n

    def catching(*more):
        """Smallest shared seed whose first double is below 0.15 and whose next nextInt(bound) draws are in the wanted sets."""
        seed = 0
        while True:
            r = h.JavaRandom(seed)
            if r.next_double() < 0.15 and all(r.next_int(bound) in wanted for bound, wanted in more):
                return seed
            seed += 1
    # ---- woodcutting ----------------------------------------------------
    trees = [(1276, 1), (1281, 15), (1308, 30), (1307, 45), (1309, 60), (1306, 75)]
    for index, (tree, need) in enumerate(trees):
        for tool, wield in [(1351, None), (None, 1359), (6739, None)]:
            for level in sorted({need, 99}):
                add("woodcutting", tree, ticks(14), tool=tool, wield=wield, levels={8: [level, level]}, tlrSeed=str(index * 7919 + level), sharedSeed=index)
        add("woodcutting", tree, ticks(10), tool=1351, levels={8: [need - 1, need - 1]}) if need > 1 else None
        add("woodcutting", tree, ticks(10), levels={8: [99, 99]})
    # Level-ups: one log crosses a threshold; the tutorial cap holds stored experience at 175.
    for tutorial in [True, False]:
        for xp in [0, 57, 58, 60, 150, 170, 276]:
            add("woodcutting", 1276, ticks(12), tool=1351, levels={8: [1, 1]}, tlrSeed=always, tutorial=tutorial, xp={"8": xp})
        add("woodcutting", 1281, ticks(40), tool=6739, levels={8: [15, 15]}, tlrSeed=always, tutorial=tutorial, xp={"8": 2740})
    add("woodcutting", 1276, ticks(12), tool=1351, levels={8: [5, 1]}, tlrSeed=always, xp={"8": 60})
    add("woodcutting", 1276, ticks(12), tool=1351, levels={8: [99, 99]}, tlrSeed=always, xp={"8": 199999990})
    add("woodcutting", 1276, ticks(12), tool=1351, levels={8: [99, 99]}, tlrSeed=always, xp={"8": 200000000})
    # Boosted and drained dynamic levels against the static requirement.
    for dynamic, static in [(20, 1), (1, 20), (14, 15), (15, 14), (99, 1)]:
        add("woodcutting", 1281, ticks(8), tool=1351, levels={8: [dynamic, static]}, tlrSeed=always)
    # A filling inventory: slot order, the last slot and the refusal.
    for filled in [0, 25, 26, 27, 28]:
        add("woodcutting", 1276, [op("set", fill=filled)] + ticks(16), tool=1351, levels={8: [99, 99]}, tlrSeed=always)
    add("woodcutting", 1276, ticks(8) + [op("set", take=1351)] + [op("tick")] * 6, tool=1351, levels={8: [99, 99]}, tlrSeed=always)
    add("woodcutting", 1276, ticks(4) + [op("set", equip=[3, None])] + [op("tick")] * 4, wield=1359, levels={8: [99, 99]}, tlrSeed=always)
    add("woodcutting", 1276, ticks(4) + [op("set", equip=[3, 6739])] + [op("tick")] * 6, levels={8: [99, 99]}, tlrSeed=always)
    add("woodcutting", 1276, ticks(90), tool=1351, inventory=[(1511, 1), (1, 1), (1521, 1)], levels={8: [50, 50]}, tlrSeed="12345", sharedSeed=9)
    add("woodcutting", 1276, ticks(30), tool=1351, levels={8: [99, 99]}, tlrSeed=always, perks=["NEST_HUNTER"], sharedSeed=0)
    # ---- mining ---------------------------------------------------------
    rocks, seen = [], set()
    for name, a in h.rows("content/global/skill/gather/mining/MiningNode.java", "public static List<WeightedChanceItem>"):
        if a[2] not in seen and len(rocks) < 12:
            seen.add(a[2])
            rocks.append(int(a[0]))
    for index, rock in enumerate(rocks):
        for tool, wield in [(1265, None), (None, 1275)]:
            add("mining", rock, ticks(24), tool=tool, wield=wield, levels={14: [99, 99]}, mathSeed=index, sharedSeed=index + 3, tlrSeed=str(index))
        add("mining", rock, ticks(10), tool=1265, levels={14: [1, 1]}, mathSeed=index)
        add("mining", rock, [op("set", fill=27)] + ticks(40), tool=1275, levels={14: [99, 99]}, mathSeed=index + 20, tutorial=False)
    add("mining", rocks[0], ticks(160), tool=1275, levels={14: [1, 1]}, mathSeed=77)
    add("mining", rocks[0], ticks(10), levels={14: [99, 99]})
    # The Star Sprite bonus makes addItemOrDrop place two unstackable ores one by one; with one slot left the second is dropped.
    for index in range(8):
        for filled in [0, 27]:
            add("mining", rocks[index % 3], [op("set", fill=filled)] + ticks(70), tool=1275, levels={14: [99, 99]}, timers=["StarBonus"],
                mathSeed=index + 100, sharedSeed=index)
    # Bracelet of clay (its last charge removes it from the equipment) and a ring whose name is read.
    for rock in rocks[:4]:
        for charges in [1, 2]:
            add("mining", rock, ticks(60), tool=1275, levels={14: [99, 99]}, equipment={"9": 11074, "12": 2572},
                attributes={"jewellery-charges:bracelet-of-clay": charges}, mathSeed=rock, sharedSeed=charges)
    # The legacy pulses: the two-argument experience hand-off and their own inventory calls.
    cycle = [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 14 + [op("pulseStop")]
    for index, (tree, need) in enumerate(trees):
        add("woodcuttingPulse", tree, cycle, tool=6739, levels={8: [99, 99]}, tlrSeed=always, sharedSeed=index, xp={"8": 13034400})
        add("woodcuttingPulse", tree, [op("pulseNew"), op("set", fill=27)] + cycle[1:], wield=1359, levels={8: [need, need]}, tlrSeed=str(index + 5))
    for index, rock in enumerate(rocks):
        add("miningPulse", rock, cycle, tool=1275, levels={14: [99, 99]}, mathSeed=index + 40, sharedSeed=index, timers=["StarBonus"])
        add("miningPulse", rock, [op("pulseNew"), op("set", fill=27)] + cycle[1:], wield=1275, levels={14: [99, 99]}, mathSeed=index + 60,
            equipment={"3": 1275, "9": 11074, "12": 2572}, attributes={"jewellery-charges:bracelet-of-clay": 1})
    # ---- fishing --------------------------------------------------------
    spots = [(316, "net", 303, None), (316, "bait", 307, 313), (309, "lure", 309, 314), (309, "bait", 307, 313), (312, "cage", 301, None),
             (312, "harpoon", 311, None), (313, "net", 305, None), (313, "harpoon", 311, None), (1177, "fish", 3157, 3150)]
    for index, (npc, word, tool, bait) in enumerate(spots):
        held = [(bait, 6)] if bait else []
        for level in [1, 40, 99]:
            add("fishing", npc, ticks(40), tool=tool, inventory=held, levels={10: [level, level]}, usedOption=word, sharedSeed=index * 13 + level)
        add("fishing", npc, [op("set", fill=26)] + ticks(40), tool=tool, inventory=held, levels={10: [99, 99]}, usedOption=word, sharedSeed=index + 400)
        add("fishing", npc, ticks(6), inventory=held, levels={10: [99, 99]}, usedOption=word)
        add("fishingPulse", npc, [op("pulseNew", option=name) for name in [OPTION[(npc, word)]]] + [op("pulseStart")] + [op("pulsePulse")] * 30,
            tool=tool, inventory=held, levels={10: [99, 99]}, sharedSeed=index + 800, hasTask=index % 2 == 0)
    # The stripy feather is consumed before the feather; one of each.
    add("fishing", 309, ticks(60), tool=309, inventory=[(314, 3), (10087, 2)], levels={10: [99, 99]}, usedOption="lure", sharedSeed=5)
    add("fishing", 309, ticks(30), tool=309, inventory=[(10087, 1)], levels={10: [99, 99]}, usedOption="lure", sharedSeed=6)
    # Barehand fishing pays Strength as well; the barb-tail harpoon may be wielded.
    add("fishing", 312, ticks(60), levels={10: [99, 99], 2: [99, 99]}, usedOption="harpoon", attributes={"barbtraining:fist": 1}, sharedSeed=21)
    add("fishing", 313, ticks(60), levels={10: [99, 99], 2: [76, 76]}, usedOption="harpoon", attributes={"barbtraining:fist": 2}, sharedSeed=22)
    add("fishing", 312, ticks(40), wield=10129, levels={10: [99, 99]}, usedOption="harpoon", sharedSeed=23)
    # GREAT_AIM doubles a catch into the last slots; the refused second fish goes to the ground service.
    for filled in [24, 26, 27]:
        add("fishing", 316, [op("set", fill=filled)] + ticks(120), tool=303, levels={10: [99, 99]}, usedOption="net", perks=["GREAT_AIM"], sharedSeed=filled)
        add("fishingPulse", 316, [op("pulseNew", option="SMALL_NET"), op("set", fill=filled), op("pulseStart")] + [op("pulsePulse")] * 120, tool=303,
            levels={10: [99, 99]}, perks=["GREAT_AIM"], sharedSeed=filled + 50)
    # Gloves stay a required service; the reply comes from the real equipment.
    add("fishing", 313, ticks(60), tool=311, levels={10: [99, 99]}, usedOption="harpoon", equipment={"9": 12861}, sharedSeed=31)
    # Level-ups in fishing: a catch crosses one or several thresholds, with and without the tutorial cap.
    for tutorial in [True, False]:
        for xp in [72, 73, 80, 170, 400, 199999995]:
            add("fishing", 316, ticks(40), tool=303, levels={10: [1, 1]}, usedOption="net", sharedSeed=catching(), tutorial=tutorial, xp={"10": xp})
            add("fishingPulse", 316, [op("pulseNew", option="SMALL_NET"), op("pulseStart")] + [op("pulsePulse")] * 20, tool=303, levels={10: [1, 1]},
                sharedSeed=catching(), tutorial=tutorial, xp={"10": xp})
    add("fishing", 316, ticks(40), tool=303, levels={10: [7, 1]}, usedOption="net", sharedSeed=catching(), xp={"10": 80})
    # The second fish of GREAT_AIM meets a full inventory and goes to the ground-item service.
    add("fishing", 316, [op("set", fill=27)] + ticks(12), tool=303, levels={10: [99, 99]}, usedOption="net", perks=["GREAT_AIM"], sharedSeed=catching((20, (1,))))
    add("fishing", 316, [op("set", fill=26)] + ticks(12), tool=303, levels={10: [99, 99]}, usedOption="net", perks=["GREAT_AIM"], sharedSeed=catching((20, (1,))))
    add("fishingPulse", 316, [op("pulseNew", option="SMALL_NET"), op("set", fill=27), op("pulseStart")] + [op("pulsePulse")] * 4, tool=303,
        levels={10: [99, 99]}, perks=["GREAT_AIM"], sharedSeed=catching((100, (0, 1, 2, 3, 4, 5))))
    # The 200-tick experience log.
    add("fishing", 316, [op("set", ticks=350)] + ticks(30), tool=303, levels={10: [99, 99]}, usedOption="net", sharedSeed=46)
    # Injected failures of the remaining required services, with the bound state left as it was.
    for at in range(0, 46):
        add("fishing", 316, [op("handler", state=0, failAt=at)], tool=303, levels={10: [1, 1]}, usedOption="net", perks=["GREAT_AIM"], xp={"10": 80},
            sharedSeed=catching((20, (1,))))
    for at in range(0, 30):
        add("woodcutting", 1276, [op("handler", state=0), op("handler", state=1, failAt=at)], tool=1351, levels={8: [1, 1]}, tlrSeed=always, xp={"8": 60})
    return [case for case in out if case is not None]


OPTION = {(316, "net"): "SMALL_NET", (316, "bait"): "BAIT", (309, "lure"): "LURE", (309, "bait"): "PIKE_BAIT", (312, "cage"): "LOBSTER_CAGE",
          (312, "harpoon"): "HARPOON", (313, "net"): "BIG_NET", (313, "harpoon"): "SHARK_HARPOON", (1177, "fish"): "KARAMBWAN_VES"}