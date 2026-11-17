"""Every enum row, every lookup key and the respawn-duration arithmetic."""


def cases(rows):
    out = []
    wood = rows("content/global/skill/gather/woodcutting/WoodcuttingNode.java", "int full,empty")
    mine = rows("content/global/skill/gather/mining/MiningNode.java", "public static List<WeightedChanceItem>")
    res = rows("content/global/skill/gather/SkillingResource.java", "private static final Map")
    tools = rows("content/data/skill/SkillingTool.java", "private final int id")
    for kind, table in [("woodcuttingNode", wood), ("miningNode", mine), ("resource", res), ("tool", tools)]:
        out += [dict(kind=kind, index=index) for index in range(len(table))]

    def ids(table, *columns):
        found = set()
        for _, a in table:
            for column in columns:
                text = a[column]
                if text.lstrip("-").isdigit():
                    found.add(int(text))
        return found

    probes = {-2147483648, -1, 0, 1, 1275, 1277, 2147483647}
    for name, table, columns in [("woodcutting", wood, (0, 1)), ("mining", mine, (0, 1)), ("resource", res, (0, 9))]:
        keys = sorted(ids(table, *columns) | {i + d for i in ids(table, *columns) for d in (-1, 1)} | probes)
        for start in range(0, len(keys), 64):
            out.append(dict(kind="lookup", table=name, ids=keys[start : start + 64]))
    tool_ids = sorted({int(a[0].rsplit("_", 1)[-1]) for _, a in tools} | probes | {1350, 1352, 13660, 13662})
    out.append(dict(kind="lookup", table="tool", ids=tool_ids))
    # One row per distinct respawn range, across the population and cap boundaries.
    for name, table, column in [("woodcutting", wood, None), ("mining", mine, None), ("resource", res, 3)]:
        seen = {}
        for row, a in table:
            key = a[2] if column is None else a[column]
            seen.setdefault(key, row)
        for row in seen.values():
            for players, cap in [(0, 2000), (1, 2000), (2, 2000), (500, 2000), (1000, 2000), (1999, 2000), (2000, 2000), (2001, 2000),
                                 (4000, 2000), (1, 0), (0, 0), (1, 1), (3, 7), (1, -5), (2147483647, 1), (1, 2147483647)]:
                out.append(dict(kind="respawn", table=name, name=row, playerCount=players, maxPlayers=cap))
    return out
