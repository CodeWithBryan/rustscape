"""MiningListener, MiningSkillPulse and ProspectListener scenarios."""
import functools

PICKS = [("BRONZE_PICKAXE", 1265, 1), ("IRON_PICKAXE", 1267, 1), ("STEEL_PICKAXE", 1269, 6), ("MITHRIL_PICKAXE", 1273, 21),
         ("ADAMANT_PICKAXE", 1271, 31), ("RUNE_PICKAXE", 1275, 41), ("PICKAXE_CLASS1", 14122, 1), ("PICKAXE_CLASS2", 14124, 20),
         ("PICKAXE_CLASS3", 14126, 40), ("PICKAXE_CLASS4", 14128, 60), ("PICKAXE_CLASS5", 14130, 80)]
ADZE = 13661
MASK = (1 << 64) - 1
CREST, AWAY = [2735, 9690, 0], [3200, 3200, 0]


def cases(h):
    out = []
    op = h.op

    @functools.lru_cache(maxsize=None)
    def math_seed(kind):
        """Seeds for Math.random's generator by the first draws: host small/large, client large/small, star hit."""
        seed = 0
        while True:
            r = h.JavaRandom(seed)
            a, b, c = r.next_double(), r.next_double(), r.next_double()
            if kind == "hit" and a < 0.02 and b > 0.9 and int(c * 6) != 3:
                return seed
            if kind == "miss" and a > 0.9 and b < 0.02:
                return seed
            if kind == "star" and a < 0.02 and b > 0.9 and int(c * 6) == 3:
                return seed
            if kind == "essenceStar" and int(a * 6) == 3:
                return seed
            seed += 1

    @functools.lru_cache(maxsize=None)
    def gem_seed(index):
        """Thread-local seed whose next nextInt(4) is `index`."""
        seed = 0
        while True:
            z = (seed + 0x9E3779B97F4A7C15) & MASK
            z = ((z ^ (z >> 33)) * 0xFF51AFD7ED558CCD) & MASK
            z = (((z ^ (z >> 33)) * 0xC4CEB9FE1A85EC53) & MASK) >> 32
            if z & 3 == index:
                return str(seed)
            seed += 1

    hit, miss, star = math_seed("hit"), math_seed("miss"), math_seed("star")

    def node(id=2092, **kw):
        kw.setdefault("location", [3300, 3301, 0])
        return dict(id=id, **kw)

    def world(pick=14130, mining=99, static=99, fm=1, weapon=False, **kw):
        # The default pickaxe is first in the selection order, which keeps the recorded lookups short.
        base = dict(levels={"14": [mining, static], "11": [fm, 99]},
                    equipment={"3": pick} if weapon and pick else {}, inventory=[] if weapon or not pick else [[pick, 1]])
        for key in ("equipment", "inventory"):
            if key in kw:
                base[key] = kw.pop(key) if key == "inventory" else {**base[key], **kw.pop(key)}
        base.update(kw)
        return base

    def listener(target, ops, **kw):
        out.append(h.scenario("mining", target, ops, **world(**kw)))

    def pulse(target, ops, **kw):
        out.append(h.scenario("miningPulse", target, ops, **world(**kw)))

    def prospect(target, ops, **kw):
        out.append(h.scenario("prospect", target, ops, **world(**kw)))

    mine = h.rows("content/global/skill/gather/mining/MiningNode.java", "public static List<WeightedChanceItem>")
    ids = [int(a[0]) for _, a in mine]
    listener(node(), [op("define")])
    prospect(node(), [op("define")])
    # Every row through the listener, the pulse and prospecting.
    for index, rock in enumerate(ids):
        seeds = dict(sharedSeed=index, mathSeed=hit, shuffleSeed=index, tlrSeed=str(index * 7919))
        listener(node(rock), [op("handler", state=0), op("handler", state=1), op("handler", state=2)], **seeds)
        pulse(node(rock), [op("pulseNew"), op("pulseStart")], **seeds)
        prospect(node(rock), [op("on"), op("runPulse", index=0)])
    for index, rock in enumerate([2090, 2094, 2108, 4027, 33220, 2092, 2100, 2096, 2098, 2102, 2104, 2106, 23567, 2491, 10946, 10947, 29746, 6669]):
        listener(node(rock), [op("define")] + [op("tick")] * 10, sharedSeed=index, mathSeed=index, tlrSeed=str(index))
        pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4 + [op("pulseStop")], sharedSeed=index, mathSeed=hit, shuffleSeed=index)
    # Empty rocks, unknown objects and the null dereferences.
    for rock in [450, 452, 21296, 11552, 1, 0, -1, 2099, 2098]:
        prospect(node(rock), [op("on")])
        for at in [[2735, 9695, 0], [2737, 9689, 0], [2740, 9684, 0], [2737, 9683, 0], [2735, 9695, 1], [2736, 9695, 0]]:
            pulse(node(rock, location=at), [op("pulseNew"), op("pulseStart"), op("pulsePulse")], mathSeed=hit)
        listener(node(rock), [op("handler", state=0), op("handler", state=1)], mathSeed=hit)
    pulse(node(), [op("pulseNew"), op("pulsePulse")])
    pulse(node(), [op("pulseNew"), op("pulseStart"), op("pulsePulse"), op("pulseStop")], pick=None)
    listener(node(), [op("handler", state=0), op("handler", state=1)], pick=None)
    listener(node(), [op("set", clock=[0, 100]), op("handler", state=0), op("handler", state=1)])
    # Every pickaxe below, at and above its level; essence delay depends on the pickaxe.
    for name, item, level in PICKS + [("INFERNO_ADZE2", ADZE, 61)]:
        for static in [level - 1, level, level + 1]:
            for weapon in [False, True]:
                for rock in [2092, 2491]:
                    for fm in ([91, 92] if item == ADZE else [1]):
                        for coin in ([0, 1] if item == ADZE and rock == 2491 else [0]):
                            kw = dict(pick=item, static=static, weapon=weapon, fm=fm, mathSeed=miss, sharedSeed=h.shared_seed((2, coin)))
                            if item == ADZE:
                                kw["inventory"] = [[ADZE, 1], [1265, 1]] if not weapon else [[1265, 1]]
                            listener(node(rock), [op("handler", state=0), op("handler", state=1), op("handler", state=2)], **kw)
                            if not weapon:
                                pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Level gates and requirement messages.
    for rock, need in [(2090, 1), (4027, 10), (2092, 15), (2100, 20), (2096, 30), (10946, 35), (2098, 40), (23567, 40), (10947, 45), (29746, 46),
                       (2102, 55), (2104, 70), (2106, 85)]:
        for level in [need - 1, need, need + 1]:
            listener(node(rock), [op("handler", state=0), op("handler", state=1)], mining=level, mathSeed=hit)
            pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, mining=level, mathSeed=hit)
            for boost in [-2147483648, -5, -2, -1, 0, 1]:
                listener(node(rock), [op("handler", state=1)], mining=level, boosts={"14": boost}, mathSeed=hit)
    for rock in [23567, 2491, 2092, 2108]:
        for active in [True, False]:
            for filled in [26, 27, 28]:
                listener(node(rock, active=active), [op("set", fill=filled), op("handler", state=0), op("handler", state=1), op("handler", state=2)], mathSeed=hit)
                pulse(node(rock, active=active), [op("pulseNew"), op("set", fill=filled), op("pulseStart")] + [op("pulsePulse")] * 4, mathSeed=hit)
    # Forced success and failure of the two Math.random draws, with the precision perk.
    for seed in [hit, miss, 0, 1, 2, 3]:
        for perk in [[], ["PRECISION_MINER"]]:
            for rock in [2090, 2106, 2491]:
                listener(node(rock), [op("handler", state=1)], mathSeed=seed, perks=perk)
                pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, mathSeed=seed, perks=perk)
    # Clay and the bracelet of clay.
    for charges in [None, 28, 2, 1, 0, -5, -2147483648]:
        for removed in [True, False]:
            for hands in [11074, 11073, None]:
                kw = dict(mathSeed=hit, addResult=removed, equipment={} if hands is None else {"9": hands},
                          attributes={} if charges is None else {"jewellery-charges:bracelet-of-clay": charges})
                listener(node(2108), [op("handler", state=1)], **kw)
                pulse(node(2108), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Gold: the Family Crest area (listener) and the perfect-gold rock (pulse).
    for rock in [2098, 2099, 2092]:
        for at in [CREST, AWAY, [2733, 9683, 0], [2741, 9695, 3], [2732, 9690, 0], [2742, 9690, 0], [2735, 9682, 0], [2735, 9696, 0]]:
            listener(node(rock), [op("handler", state=1)], mathSeed=hit, location=at)
    # Sandstone and granite sizes.
    for rock, sizes in [(10946, 4), (10947, 3)]:
        for size in range(sizes):
            kw = dict(mathSeed=hit, sharedSeed=h.shared_seed((sizes, size)))
            listener(node(rock), [op("handler", state=1)], **kw)
            pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Essence: pure essence from level 30, and the full-inventory stop.
    for level in [1, 29, 30, 31]:
        for filled in [1, 26, 27]:
            listener(node(2491), [op("set", fill=filled), op("handler", state=0), op("handler", state=1), op("handler", state=2)], mining=level)
            pulse(node(2491), [op("pulseNew"), op("set", fill=filled), op("pulseStart")] + [op("pulsePulse")] * 7, mining=level, mathSeed=hit)
    listener(node(2491), [op("handler", state=1)], timers=["StarBonus"], mathSeed=math_seed("essenceStar"), diaryLevel=2, armour=2)
    # Gem rocks: the weight draw, the shuffled shared table and both messages.
    for value in [0, 1, 4, 5, 9, 10, 15, 16, 30, 31, 60, 61, 127]:
        for shuffle in [0, 1, 2]:
            kw = dict(mathSeed=hit, sharedSeed=h.shared_seed((128, value)), shuffleSeed=shuffle)
            listener(node(23567), [op("handler", state=1), op("set", nodeActive=True, mathSeed=hit), op("handler", state=2)], **kw)
            pulse(node(23567), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Varrock armour tiers and thresholds (they differ between the two bodies).
    for rock in [2090, 2098, 2104, 2106]:
        for diary in [-1, 0]:
            for armour in [-1, 0, 1, 2]:
                for draw in [0, 1, 2, 3, 4, 10, 11]:
                    kw = dict(mathSeed=hit, sharedSeed=h.shared_seed((100, draw)), diaryLevel=diary, armour=armour)
                    listener(node(rock), [op("handler", state=1)], **kw)
                    if draw in (10, 11) and armour in (-1, 2):
                        pulse(node(rock), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Shooting-star bonus ore.
    for seed in [hit, star]:
        for timers in [[], ["StarBonus"]]:
            listener(node(2096), [op("handler", state=1)], mathSeed=seed, timers=timers, diaryLevel=1, armour=2, sharedSeed=h.shared_seed((100, 0)))
            pulse(node(2096), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, mathSeed=seed, timers=timers)
    # Bonus gem: jewellery changes the chance; each gem; a full inventory drops it.
    for ring in [None, 2572, 14652, 1635]:
        for neck in [None, 1704, 1705, 1712, 1713, 1714]:
            chance = 282
            if ring in (2572, 14652):
                chance = int(chance / 1.5)
            if neck is not None and 1705 <= neck <= 1713:
                chance = int(chance / 1.5)
            for draw in [0, 1]:
                gear = {**({} if ring is None else {"12": ring}), **({} if neck is None else {"2": neck})}
                kw = dict(mathSeed=hit, sharedSeed=h.shared_seed((chance, draw)), equipment=gear, tlrSeed=gem_seed((chance + draw) % 4))
                listener(node(2092), [op("handler", state=1)], **kw)
                if neck in (None, 1705):
                    pulse(node(2092), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    for index in range(4):
        for filled in [26, 27]:
            kw = dict(mathSeed=hit, sharedSeed=h.shared_seed((282, 1)), tlrSeed=gem_seed(index))
            listener(node(2096), [op("set", fill=filled), op("handler", state=1)], **kw)
            pulse(node(2096), [op("pulseNew"), op("set", fill=filled), op("pulseStart")] + [op("pulsePulse")] * 4, **kw)
    # Respawn duration feeds the replacement.
    for players, cap in [(0, 2000), (1, 2000), (1000, 2000), (2000, 2000), (4000, 2000), (1, 0), (0, 0)]:
        listener(node(2106), [op("handler", state=1)], mathSeed=hit, playerCount=players, maxPlayers=cap)
    # Long runs.
    for run in range(10):
        name, item, level = PICKS[10] if run % 2 else PICKS[run % 6]
        rock = [2090, 2092, 2096, 2491, 23567, 2108][run % 6]
        kw = dict(pick=item, mining=max(41, 30 + run * 3), sharedSeed=2000 + run, mathSeed=3000 + run, shuffleSeed=run, tlrSeed=str(run * 15485863 + 5),
                  diaryLevel=run % 3 - 1, armour=run % 4 - 1, timers=["StarBonus"] if run % 5 == 0 else [])
        listener(node(rock), [op("define"), op("set", fill=20 + run % 8)] + [op("tick")] * 28, **kw)
        pulse(node(rock), [op("pulseNew"), op("set", fill=20 + run % 8), op("pulseStart")] + [op("pulsePulse")] * 28 + [op("pulseStop")], **kw)
    # A service failure at every position of a full pass.
    for position in range(52):
        kw = dict(mathSeed=star, sharedSeed=h.shared_seed((100, 0), (188, 1)), tlrSeed=gem_seed(2), timers=["StarBonus"], diaryLevel=0, armour=0,
                  equipment={"9": 11074, "12": 2572}, attributes={"jewellery-charges:bracelet-of-clay": 1})
        listener(node(2108), [op("handler", state=1, failAt=position), op("handler", state=2)], **kw)
        pulse(node(2108), [op("pulseNew"), op("pulseStart")] + [op("pulsePulse")] * 3 + [op("pulsePulse", failAt=position), op("pulseStop")], **kw)
    for position in range(40):
        listener(node(23567), [op("handler", state=1, failAt=position)], mathSeed=hit, sharedSeed=h.shared_seed((128, 61)), shuffleSeed=4)
        listener(node(2092), [op("handler", state=0, failAt=position)])
    return out
