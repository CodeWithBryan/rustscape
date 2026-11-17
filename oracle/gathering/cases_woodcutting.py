"""WoodcuttingListener and WoodcuttingSkillPulse scenarios."""
import math

AXES = [("BRONZE_AXE", 1351, 1, 0), ("IRON_AXE", 1349, 1, 1), ("STEEL_AXE", 1353, 6, 2), ("BLACK_AXE", 1361, 6, 3),
        ("MITHRIL_AXE", 1355, 21, 4), ("ADAMANT_AXE", 1357, 31, 5), ("RUNE_AXE", 1359, 41, 6), ("DRAGON_AXE", 6739, 61, 7),
        ("HATCHET_CLASS1", 14132, 1, 16), ("HATCHET_CLASS2", 14134, 20, 17), ("HATCHET_CLASS3", 14136, 40, 18),
        ("HATCHET_CLASS4", 14138, 60, 19), ("HATCHET_CLASS5", 14140, 80, 20)]
ADZE = 13661
# Designer-side copies of a few chance rows, used only to aim thread-local seeds at a boundary.
TREES = {1276: (1, 64, 200, 32, 100), 1281: (15, 32, 100, 16, 50), 1308: (30, 16, 50, 8, 25), 1307: (45, 8, 25, 4, 12.5),
         1309: (60, 4, 12.5, 2, 6.25), 1306: (75, 2, 6, 1, 3), 9036: (35, 15, 46, 8, 23.5), 2289: (45, 18, 26, 10, 14)}
MISCELLANIA, ETCETERIA, SEERS, ELSEWHERE = [2570, 3850, 0], [2500, 3850, 0], [2700, 3470, 0], [3200, 3200, 0]


def numerator(tree, ordinal, level, black=False):
    _, low, high, mod_low, mod_high = TREES[tree]
    half = 2 if black else 1
    lo, hi = low + ordinal * (mod_low / half), high + ordinal * (mod_high / half)
    return int(math.floor(lo * ((99 - level) / 98.0)) + math.floor(hi * ((level - 1) / 98.0)) + 1)


def cases(h):
    out = []
    op = h.op
    always, never = h.tlr_seed_for_unit(0), h.tlr_seed_for_unit((1 << 53) - 1)

    def node(id=1276, **kw):
        return dict(id=id, location=[3200, 3201, 0], **kw)

    def world(axe=14140, wc=99, static=99, fm=1, weapon=False, **kw):
        # The default hatchet is first in the selection order, which keeps the recorded lookups short.
        base = dict(levels={"8": [wc, static], "11": [fm, 99]}, equipment={"3": axe} if weapon and axe else {},
                    inventory=[] if weapon or not axe else [[axe, 1]])
        for key in ("equipment", "inventory"):
            if key in kw:
                base[key] = kw.pop(key) if key == "inventory" else {**base[key], **kw.pop(key)}
        base.update(kw)
        return base

    def listener(target, ops, **kw):
        out.append(h.scenario("woodcutting", target, ops, **world(**kw)))

    def pulse(target, ops, **kw):
        out.append(h.scenario("woodcuttingPulse", target, ops, **world(**kw)))

    wood = h.rows("content/global/skill/gather/woodcutting/WoodcuttingNode.java", "int full,empty")
    ids = [int(a[0]) for _, a in wood]
    listener(node(), [op("define")])
    # Every row through both bodies with a usable axe, and with the best one.
    for index, tree in enumerate(ids):
        seeds = dict(sharedSeed=index, tlrSeed=str(index * 7919))
        listener(node(tree), [op("define")] + [op("tick")] * 6, **seeds)
        listener(node(tree), [op("handler", state=0), op("handler", state=1), op("handler", state=2)], axe=6739, tlrSeed=always, sharedSeed=index)
        pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4 + [op("pulseStop")], axe=6739, tlrSeed=always, sharedSeed=index)
    # Every hatchet below, at and above its level, held or wielded.
    for name, item, level, ordinal in AXES:
        for static in [level - 1, level, level + 1]:
            for weapon in [False, True]:
                for tree in [1276, 1281]:
                    listener(node(tree), [op("handler", state=0), op("handler", state=1)], axe=item, wc=99, static=static, weapon=weapon, sharedSeed=ordinal)
                    if tree == 1276:
                        pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, axe=item, wc=99, static=static, weapon=weapon, sharedSeed=ordinal)
    # The level gate around every distinct requirement.
    for tree, need in [(1276, 1), (1281, 15), (1308, 30), (9036, 35), (1292, 36), (4142, 40), (1307, 45), (2289, 45), (9034, 50), (21273, 54),
                       (28951, 58), (1309, 60), (1306, 75), (37821, 82)]:
        for wc in [need - 1, need, need + 1]:
            listener(node(tree), [op("handler", state=0)], wc=wc, questStage=21)
            pulse(node(tree), [op("pulseNew"), op("pulseStart"), op("pulsePulse")], wc=wc)
    # Requirement order and messages.
    for place in [MISCELLANIA, ETCETERIA, SEERS]:
        for wc in [1, 99]:
            listener(node(1281), [op("handler", state=0), op("handler", state=1)], wc=wc, location=place, tlrSeed=always)
    listener(node(), [op("handler", state=0), op("handler", state=1)], axe=None)
    listener(node(), [op("handler", state=0), op("handler", state=1)], axe=None, animating=True)
    for stage in [0, 20, 21, 100]:
        for tree in [1292, 1276]:
            listener(node(tree), [op("handler", state=0)], questStage=stage)
    for active in [True, False]:
        for filled in [0, 27, 28]:
            listener(node(1281, active=active), [op("set", fill=filled), op("handler", state=0), op("handler", state=1)], tlrSeed=always)
            pulse(node(1281, active=active), [op("pulseNew"), op("set", fill=filled), op("pulseStart"), op("pulsePulse")], tlrSeed=always)
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=0), op("set", clock=[0, 99]), op("handler", state=0)])
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=0)], hasScript=False)
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=3)], scriptPersist=False)
    listener(node(), [op("set", clock=[7, 101]), op("handler", state=0), op("handler", state=1), op("set", clock=[7, 100]), op("handler", state=1)])
    # Unknown node and missing tool: where the original dereferences null.
    for target in [1, 1342, 0]:
        for animating in [False, True]:
            listener(node(target), [op("handler", state=0), op("handler", state=1), op("set", clock=[0, 100]), op("handler", state=1)], animating=animating)
        pulse(node(target), [op("pulseNew"), op("pulseStart"), op("pulsePulse"), op("pulseStop")])
    pulse(node(), [op("pulseNew"), op("pulsePulse")])
    pulse(node(), [op("pulseNew"), op("pulseStart"), op("pulsePulse")], axe=None)
    # The success roll: exactly on the chance, and one unit below it.
    for tree in sorted(TREES):
        need = TREES[tree][0]
        for name, item, level, ordinal in [AXES[0], AXES[3], AXES[7], AXES[12]]:
            for wc in sorted({need, 99}):
                if wc < need:
                    continue
                value = numerator(tree, ordinal, wc, name == "BLACK_AXE")
                if 0 < value < 256:
                    equal, below = h.tlr_seed_for_ratio(value)
                    for seed in (equal, below):
                        listener(node(tree), [op("handler", state=1)], axe=item, wc=wc, tlrSeed=seed, animating=True)
                        if tree in (1309, 1306):
                            pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, axe=item, wc=wc, tlrSeed=seed, animating=True)
    # Familiar boost moves the level used by the roll, not the requirement.
    for boost in [-2147483648, -1, 0, 1, 2, 2147483647]:
        value = numerator(1309, 0, 60)
        listener(node(1309), [op("handler", state=1)], axe=1351, wc=60, boosts={"8": boost}, tlrSeed=h.tlr_seed_for_ratio(value)[1], animating=True)
    # Instachop bypasses a failed roll and forces felling.
    for flag in [True, False]:
        listener(node(1306), [op("handler", state=1)], wc=75, tlrSeed=never, attributes={"instachop": flag}, animating=True,
                 sharedSeed=h.shared_seed((282, 0), (8, 0)))
    # Sound: every index, with and without neighbours (the player is filtered out).
    for index in range(5):
        for locals_ in [[], [0], [0, 1], [1, 0, 2, 3]]:
            listener(node(1281), [op("handler", state=1)], locals=locals_, sharedSeed=h.shared_seed((5, index)), tlrSeed=never)
            pulse(node(1281), [op("pulseNew"), op("pulseStart"), op("pulsePulse")], locals=locals_, sharedSeed=h.shared_seed((5, index)), tlrSeed=never)
    # Rewards. `animating` removes the sound draw so the designed draws line up.
    quiet = dict(animating=True, tlrSeed=always)
    for nest in [140, 141, 142]:
        for fell in [0, 1, 2, 7]:
            seed = h.shared_seed((282, nest), (8, fell))
            for perk in [[], ["NEST_HUNTER"]]:
                for add in [True, False]:
                    listener(node(1281), [op("handler", state=1)], sharedSeed=seed, perks=perk, addResult=add, **quiet)
            listener(node(1276), [op("handler", state=1)], sharedSeed=seed, **quiet)
            pulse(node(1281), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, sharedSeed=seed, perks=["NEST_HUNTER"], nestAmount=3, **quiet)
    for tree in [1276, 1282, 1315, 2887, 2892, 2023, 1281, 1308, 9036, 1307, 2289, 9034, 21273, 28951, 1309, 1306, 37821]:
        for fell in [0, 1]:
            seed = h.shared_seed((100, 50), (282, 0), (8, fell)) if tree == 2289 else h.shared_seed((282, 0), (8, fell))
            listener(node(tree), [op("handler", state=1), op("handler", state=2)], sharedSeed=seed, playerCount=1000, **quiet)
            if fell == 1:
                pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 5, sharedSeed=seed, playerCount=1000, **quiet)
    for players, cap in [(0, 2000), (1, 2000), (2000, 2000), (4000, 2000), (1, 0), (0, 0)]:
        listener(node(1309), [op("handler", state=1)], sharedSeed=h.shared_seed((282, 0), (8, 1)), playerCount=players, maxPlayers=cap, **quiet)
    for bark in [0, 9, 10, 99]:
        seed = h.shared_seed((100, bark), (282, 1), (8, 0))
        for tree in [2289, 4060]:
            listener(node(tree), [op("handler", state=1)], sharedSeed=seed, **quiet)
            pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, sharedSeed=seed, **quiet)
    for place in [SEERS, ELSEWHERE, MISCELLANIA]:
        for done in [[], ["SEERS_VILLAGE:1"]]:
            for hat in [None, 14631, 14630]:
                for tree in [1276, 1307]:
                    kw = dict(location=place, diaryComplete=done, sharedSeed=h.shared_seed((282, 0), (8, 0)), equipment={} if hat is None else {"0": hat}, **quiet)
                    listener(node(tree), [op("handler", state=1)], **kw)
                    if hat == 14631:
                        pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    listener(node(1281), [op("handler", state=1)], attributes={"stats_manager:logs_chopped": 2147483647}, sharedSeed=h.shared_seed((282, 0), (8, 0)), **quiet)
    # Farming trees: the patch advances and the scenery is never replaced.
    for tree in [8467, 8488, 8444, 8513, 8409]:
        for varbit in [-1, 0, 704]:
            for fell in [0, 1]:
                seed = h.shared_seed((282, 0), (8, fell))
                listener(node(tree), [op("handler", state=1), op("script", index=0, stage=0), op("handler", state=2)], sharedSeed=seed, patchVarbit=varbit, patchState=3, **quiet)
                if varbit != 0:
                    pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 5, sharedSeed=seed, patchVarbit=varbit, patchState=2147483647, **quiet)
    # One-chop branches.
    for tree in [1292, 4142]:
        for wc in [40, 99]:
            listener(node(tree), [op("handler", state=0), op("script", index=0, stage=0), op("handler", state=1), op("script", index=1, stage=5)],
                     wc=wc, questStage=21, tlrSeed=never)
            pulse(node(tree), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 5, wc=wc, tlrSeed=always)
    # Inferno adze: usable only with 92 Firemaking; the two bodies roll differently.
    for fm in [91, 92]:
        for burn in [0, 1, 2, 3]:
            for tree, fire in [(1281, {"1521": 60.0}), (1281, {}), (2289, {})]:
                seed = h.shared_seed((4, burn), (100, 5), (282, 0), (8, 1))
                listener(node(tree), [op("handler", state=1)], axe=ADZE, fm=fm, fireXp=fire, sharedSeed=seed, inventory=[[ADZE, 1], [1351, 1]], **quiet)
        for burn in [0, 24, 25, 99]:
            seed = h.shared_seed((100, burn), (282, 0), (8, 1))
            pulse(node(1281), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, axe=ADZE, fm=fm, sharedSeed=seed, inventory=[[ADZE, 1], [1351, 1]], **quiet)
    # The undercover tree (pulse only).
    for line in [0, 1]:
        pulse(node(10041), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, sharedSeed=h.shared_seed((2, line)), **quiet)
    # Long runs until the tree falls or the inventory fills.
    for run in range(10):
        name, item, level, ordinal = AXES[12] if run % 2 else AXES[run % 8]
        tree = [1276, 1281, 1308, 1307, 1309, 1306][run % 6]
        kw = dict(axe=item, wc=max(TREES[tree][0], 20 + run * 3), sharedSeed=1000 + run, tlrSeed=str(run * 104729 + 1), locals=[0, 1])
        listener(node(tree), [op("define"), op("set", fill=24 + run % 4)] + [op("tick")] * 28, **kw)
        pulse(node(tree), [op("pulseNew"), op("set", fill=24 + run % 4), op("pulseStart")] + [op("pulsePulse")] * 28 + [op("pulseStop")], **kw)
    # A service failure at every position of a full reward-and-fell pass.
    full = h.shared_seed((5, 2), (282, 141), (8, 1))
    for position in range(44):
        listener(node(1281), [op("handler", state=1, failAt=position), op("handler", state=2)], axe=1359, perks=["NEST_HUNTER"], locals=[0, 1],
                 sharedSeed=full, tlrSeed=always)
        pulse(node(1281), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 3 + [op("pulsePulse", failAt=position), op("pulseStop")],
              axe=1359, perks=["NEST_HUNTER"], locals=[0, 1], sharedSeed=h.shared_seed((5, 2), (282, 141), (8, 1)), tlrSeed=always, animating=False)
    for position in range(24):
        listener(node(8467), [op("handler", state=1, failAt=position)], sharedSeed=h.shared_seed((282, 0), (8, 1)), patchVarbit=704, **quiet)
        listener(node(1281), [op("handler", state=0, failAt=position)], tlrSeed=never)
    return out
