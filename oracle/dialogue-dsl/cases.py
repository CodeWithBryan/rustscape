"""Scenarios for the dialogue DSL oracle.

cases() yields (case, sweep): `sweep` names the action whose recorded calls each
fail once in turn in the generator's second pass, or None. Every scenario runs
the unmodified DialogueLabeller/DialogueBuilder/SkillDialogueHandler/
SkillDialoguePlugin/ActivateEnchantedGem and BankerDialogueLabellerFile code.
"""

NEEDED_EVENTS = [
    "faceLocation", "dialoguePlayer", "walkReset", "pulseClear", "npcOn", "playerOn", "itemOn", "itemZoom",
    "childPosition", "runScript", "setCloseEvent", "hook", "unhook", "newNpc", "newItem", "questStage",
    "invAmount", "stdout", "log", "debug", "dmessage", "handlerPut", "slayerStarted", "openBank", "pinSettings",
    "collectionBox", "activate", "toggleBank", "removeItem", "apiMessage", "seal", "ironman", "geAwaiting",
    "activated", "usingSecondary", "cb", "create", "getAll", "getName", "input",
]

BANKER = {"kind": "banker"}


def H(b, c=None):
    action = {"op": "handle", "b": b}
    if c is not None:
        action["c"] = c
    return action


def talk(file=0, npc=0):
    return {"op": "talk", "file": file, "npc": npc}


def case(name, actions, sweep=None, **world):
    out = dict(name=name, actions=actions)
    out.update(world)
    return out, sweep


def banker_cases():
    yield case("banker access", [talk(), H(1), H(2), H(1)], 0, files=[BANKER], npcs=[494])
    yield case("banker access choice", [talk(), H(1), H(2), H(1)], 3, files=[BANKER], npcs=[494])
    yield case("banker options shown", [talk(), H(1)], 1, files=[BANKER], npcs=[494])
    yield case("banker what is this place", [talk(), H(1), H(5), H(1), H(1), H(1), H(1), H(1)], None, files=[BANKER], npcs=[494])
    yield case("banker pin", [talk(), H(1), H(3), H(1)], 3, files=[BANKER], npcs=[494])
    yield case("banker collect", [talk(), H(1), H(4), H(1)], 3, files=[BANKER], npcs=[494])
    yield case("banker ge collect", [talk(), H(1), H(1), H(2), H(1)], 1, files=[BANKER], npcs=[494], bank={"ge": True})
    yield case("banker uim", [talk(), H(1)], 0, files=[BANKER], npcs=[494], bank={"uim": True})
    yield case("banker uim female", [talk(), H(1)], None, files=[BANKER], npcs=[494], bank={"uim": True}, male=False)
    yield case("banker uim jade", [talk(), H(1)], 0, files=[BANKER], npcs=[4296], bank={"uim": True})
    yield case("banker sirsal no seal", [talk(), H(1), H(1), H(1), H(1)], 0, files=[BANKER], npcs=[4519])
    yield case("banker sirsal seal", [talk(), H(1)], 0, files=[BANKER], npcs=[4519], bank={"seal": True})
    yield case("banker jade", [talk(), H(1), H(5), H(1), H(1), H(1), H(1)], 1, files=[BANKER], npcs=[4296])
    yield case("banker tutor how to use", [talk(), H(1), H(2), H(1), H(2), H(1)] + [H(1)] * 7 + [H(3), H(1), H(1), H(1), H(4), H(1), H(1), H(1), H(5), H(1)],
               None, files=[BANKER], npcs=[4907])
    yield case("banker tutor goodbye", [talk(), H(1), H(2), H(1), H(5), H(1)], 4, files=[BANKER], npcs=[4907])
    yield case("banker bounty who", [talk(), H(1), H(2), H(1), H(1)], 3, files=[BANKER], npcs=[6538])
    yield case("banker bounty afraid", [talk(), H(1), H(6), H(1), H(1), H(1)], None, files=[BANKER], npcs=[6538])
    yield case("banker bounty second bank", [talk(), H(1)], 1, files=[BANKER], npcs=[6538], bank={"secondBank": True})
    odo = [talk(), H(1), H(2), H(1)] + [H(1)] * 9
    yield case("banker odovacar pays", odo + [H(2), H(1)], len(odo) + 1, files=[BANKER], npcs=[5383], bank={"remove": True})
    yield case("banker odovacar poor", odo + [H(2), H(1), H(1)], len(odo) + 1, files=[BANKER], npcs=[5383])
    yield case("banker odovacar later", odo + [H(3), H(1), H(1)], None, files=[BANKER], npcs=[5383], male=False)
    yield case("banker odovacar refuses", odo + [H(4), H(1), H(1)], None, files=[BANKER], npcs=[5383])
    yield case("banker odovacar access", [talk(), H(1), H(3), H(1)], 3, files=[BANKER], npcs=[5383], bank={"remove": True})
    buy = [talk(), H(1), H(3), H(1), H(1), H(1), H(1), H(1)]
    for result in ["SUCCESS", "ALREADY_ACTIVE", "INTERNAL_FAILURE", "NOT_ENOUGH_MONEY"]:
        yield case("banker buy " + result, buy + [H(2), H(1), H(1)], len(buy) + 1, files=[BANKER], npcs=[494],
                   bank={"secondBank": True, "activation": result})
    yield case("banker buy no", buy + [H(3), H(1), H(1)], None, files=[BANKER], npcs=[494], bank={"secondBank": True})
    yield case("banker switch to secondary", [talk(), H(1), H(3), H(1), H(1)], 4, files=[BANKER], npcs=[494],
               bank={"activated": True})
    yield case("banker switch to primary", [talk(), H(1), H(3), H(1), H(1)], 1, files=[BANKER], npcs=[494],
               bank={"activated": True, "secondary": True, "secondBank": True})
    yield case("banker closed then continue", [talk(), {"op": "close"}, H(1)], None, files=[BANKER], npcs=[494])
    yield case("banker no location", [talk()], None, files=[BANKER], npcs=[494], noLocation=True)
    yield case("banker button out of range", [talk(), H(1), H(9), H(1)], 2, files=[BANKER], npcs=[494])
    yield case("banker opened without npc", [{"op": "open", "file": 0}], 0, files=[BANKER])
    yield case("banker opened by key", [{"op": "openKey", "key": 494}, {"op": "openOther"}], 1, files=[BANKER])


def L(*ops):
    return {"kind": "labeller", "ops": list(ops)}


def opt(goto, option, **kw):
    out = {"goto": goto, "option": option}
    out.update(kw)
    return out


def labeller_cases():
    lines = L(["npc", "HAPPY", ["Hello there."]], ["player", "ASKING", ["Two", "lines"]], ["line", ["Plain", "text"]],
              ["npcPlain", ["A long message that goes on and on until it is split over more than one line by the splitter."]],
              ["playerPlain", ["Bye @name, on @servername as a @g[man,woman]."]], ["npcAs", "ANGRY", 2, ["Another NPC."]],
              ["npcId", 3, ["By id."]], ["item", 0, [], "An item message."], ["item", 0, ["a", "b"], None],
              ["item2", 0, 1, ["Two items."], None], ["item2", 0, 1, [], None])
    run = [talk()] + [H(1)] * 11
    yield case("labeller lines", run, None, files=[lines], items=[[995, 5], [1511, 1]])
    for i in [0, 3, 5, 6, 7, 9]:
        yield case(f"labeller lines step {i}", run[:i + 1], i, files=[lines], items=[[995, 5], [1511, 1]])
    unclosable = L(["npc", "NEUTRAL", ["Locked."], True], ["player", "NEUTRAL", ["Me too."], True], ["line", ["Line."], True],
                   ["item", 0, ["Item."], None, True], ["manual", True, [["send", ["Manual."]]]], ["manual", True, [["emit", "nothing"]]])
    yield case("labeller unclosable", [talk(), {"op": "closeEvent"}, H(1), {"op": "closeEvent"}, H(1), H(1), H(1), H(1),
                                       {"op": "close"}, {"op": "closeEvent"}], 0, files=[unclosable], items=[[995, 1]])
    yield case("labeller unclosable later", [talk(), H(1), H(1), H(1), H(1)], 4, files=[unclosable], items=[[995, 1]],
               attrs={"close_c_": False})
    labels = L(["label", ["start", "alias"], [["npc", "NEUTRAL", ["In start."]], ["goto", "far"]]],
               ["label", ["skipped"]], ["npc", "NEUTRAL", ["Never."]],
               ["label", ["far"]], ["player", "NEUTRAL", ["Far."]], ["goto", "missing"],
               ["label", ["dead"]], ["npc", "NEUTRAL", ["Dead end."]])
    yield case("labeller labels", [talk(), H(1), H(1)], 1, files=[labels])
    loops = L(["exec", [["emit", "first"], ["loadLabel", "a"]]],
              ["label", ["a"]], ["exec", [["emit", "a"], ["goto", "b"]]],
              ["label", ["b"]], ["exec", [["emit", "b"], ["goto", "a"]]])
    yield case("labeller ping pong", [talk()], 0, files=[loops])
    selfjump = L(["label", ["self"]], ["exec", [["emit", "self"], ["goto", "self"]]], ["npc", "NEUTRAL", ["After."]])
    yield case("labeller self jump", [talk(), H(1)], 0, files=[selfjump])
    menu = L(["npc", "NEUTRAL", ["Pick."]],
             ["options", [opt("one", "First", expr="HAPPY"), opt("two", "Second", skip=True),
                          opt("three", "Third", spoken="I say three."), opt("four", "Fourth", **{"if": {"attr": "four"}}),
                          opt("five", "Fifth", **{"if": {"npc": 494}})], None],
             ["label", ["one"]], ["npc", "NEUTRAL", ["One."]], ["goto", "end"],
             ["label", ["two"]], ["npc", "NEUTRAL", ["Two."]], ["goto", "end"],
             ["label", ["three"]], ["npc", "NEUTRAL", ["Three."]],
             ["label", ["four"]], ["npc", "NEUTRAL", ["Four."]],
             ["label", ["five"]], ["npc", "NEUTRAL", ["Five."]],
             ["label", ["end"]], ["afterClose", [["emit", "closed"]]], ["exec", [["emitInput"]]])
    for b in [2, 3, 4, 5, 9]:
        yield case(f"labeller options {b}", [talk(), H(1), H(b), H(1), H(1), H(1)], 2, files=[menu])
    yield case("labeller options with condition", [talk(), H(1), H(5), H(1), H(1)], 1, files=[menu], attrs={"four": True})
    yield case("labeller options bad condition", [talk(), H(1)], None, files=[menu], attrs={"four": 3})
    single = L(["options", [opt("one", "Only"), opt("two", "Hidden", **{"if": False})], "Title"],
               ["label", ["one"]], ["npc", "NEUTRAL", ["Jumped."]])
    yield case("labeller single option", [talk(), H(1)], 0, files=[single])
    none = L(["options", [opt("one", "Hidden", **{"if": False})], "Title", True], ["label", ["one"]], ["npc", "NEUTRAL", ["Never."]])
    yield case("labeller no option", [talk(), H(1), H(1)], 0, files=[none])
    six = L(["options", [opt("a", "A"), opt("b", "B"), opt("c", "C"), opt("d", "D"), opt("e", "E"), opt("f", "F")], None, True])
    yield case("labeller six options", [talk(), H(2), H(1)], 0, files=[six])
    titled = L(["options", [opt("a", "A"), opt("b", "B")], "My title", True], ["label", ["a"]], ["npc", "NEUTRAL", ["A."]],
               ["label", ["b"]], ["npc", "NEUTRAL", ["B."]])
    yield case("labeller titled unclosable options", [talk(), {"op": "closeEvent"}, H(3), {"op": "closeEvent"}, H(1)], 2, files=[titled])
    for kind in ["AMOUNT", "NUMERIC", "STRING_SHORT", "STRING_LONG", "MESSAGE"]:
        flow = L(["npc", "NEUTRAL", ["How many?"]], ["input", kind, "Enter it" if kind != "NUMERIC" else None],
                 ["exec", [["emitInput"]]], ["npc", "NEUTRAL", ["Thanks."]])
        yield case("labeller input " + kind, [talk(), H(1), {"op": "input", "v": 7}, H(1)], 1, files=[flow])
        yield case("labeller input text " + kind, [talk(), H(1), {"op": "input", "v": "abc"}], 2, files=[flow])
    for numeric in [True, False]:
        flow = L(["inputBool", numeric, None], ["exec", [["emitInput"]]], ["player", "NEUTRAL", ["Done."]])
        yield case(f"labeller input numeric {numeric}", [talk(), {"op": "input", "v": 12}, H(1)], 0, files=[flow])
    nested = L(["npc", "NEUTRAL", ["Opening another."]], ["open", {"file": 1, "npc": True}], ["npc", "NEUTRAL", ["Unreached."]])
    other = L(["exec", [["emit", "other"]]], ["player", "NEUTRAL", ["In the other file."]])
    yield case("labeller open stage", [talk(), H(1), H(1)], 1, files=[nested, other])
    bykey = L(["open", {"key": 196608}], ["npc", "NEUTRAL", ["After key."]])
    yield case("labeller open key", [talk()], 0, files=[bykey], skill=True)
    warn = L(["exec", [["openDialogue", {"file": 1}]]], ["npc", "NEUTRAL", ["Clash."]])
    yield case("labeller openDialogue in exec", [talk(), H(1)], 0, files=[warn, other])
    failing = L(["exec", [["setAttr", "x", 1], ["fail"]]], ["npc", "NEUTRAL", ["Unreached."]])
    yield case("labeller exec failure", [talk(), H(1)], None, files=[failing])
    nonpc = L(["npc", "NEUTRAL", ["No NPC."]], ["exec", [["emit", "x"]]], ["player", "NEUTRAL", ["Me."]])
    yield case("labeller without npc", [{"op": "open", "file": 0}, H(1)], 0, files=[nonpc])
    branch = L(["assign", 494], ["exec", [["if", {"attr": "flag"}, [["loadLabel", "yes"]], [["loadLabel", "no"]]]]],
               ["label", ["yes"]], ["npc", "NEUTRAL", ["Yes."]], ["afterClose", [["emit", "after yes"]]], ["goto", "missing"],
               ["label", ["no"]], ["npc", "NEUTRAL", ["No."]], ["exec", [["setAttr", "flag", True]]], ["goto", "yes"])
    yield case("labeller branch no", [talk(), H(1), H(1)], 1, files=[branch])
    yield case("labeller branch yes", [talk(), H(1), H(1), H(1)], 2, files=[branch], attrs={"flag": True})
    jumping = L(["manual", False, [["goto", "later"], ["send", ["Manual."]]]], ["afterClose", [["emit", "not hooked"]]],
                ["npc", "NEUTRAL", ["Skipped."]], ["label", ["later"]], ["npc", "NEUTRAL", ["Later."]], ["afterClose", [["emit", "hooked"]]])
    yield case("labeller after close with jump", [talk(), H(1), H(1)], 0, files=[jumping])
    guarded = L(["exec", [["goto", "a"]]], ["exec", [["emit", "second exec"]]], ["npc", "NEUTRAL", ["Skipped."]], ["label", ["a"]], ["npc", "NEUTRAL", ["A."]])
    yield case("labeller exec after jump", [talk(), H(1)], 0, files=[guarded])
    yield case("labeller after close twice", [talk(), H(1), {"op": "close"}, talk(), H(1), H(1)], 2, files=[branch], attrs={"flag": True})


def B(*clauses, bare=None):
    out = {"kind": "builder", "clauses": list(clauses)}
    if bare is not None:
        out["bare"] = bare
    return out


def clause(when, *chain):
    return {"when": when, "chain": list(chain)}


def builder_cases():
    simple = B(clause({"cond": {"attr": "skip"}}, ["npcl", None, "Skipped clause."]),
               clause("default", ["npcl", "HAPPY", "Hello there, this is a long line that will need to be split by the line splitter."],
                      ["npc", None, ["Two", "lines"]], ["npc", "ANGRY", ["Angry."]], ["playerl", None, "Me."],
                      ["playerl", "ASKING", "Asking?"], ["player", None, ["P1"]], ["player", "SAD", ["P2", "P3"]],
                      ["item", 1511, ["Logs."]], ["iteml", 995, "Coins here."], ["line", ["Plain", "line"]],
                      ["linel", "A plain line split."], ["end"]))
    run = [{"op": "open", "file": 0, "npc": 0}] + [H(1)] * 13
    yield case("builder nodes", run, None, files=[simple])
    for i in [0, 4, 8, 10, 12]:
        yield case(f"builder nodes step {i}", run[:i + 1], i, files=[simple])
    yield case("builder nodes without npc", [{"op": "open", "file": 0}, H(1), H(1)], 1, files=[simple])
    yield case("builder skipped clause", [{"op": "open", "file": 0, "npc": 0}, H(1)], 0, files=[simple], attrs={"skip": True})
    quest = B(clause({"quest": "COOKS_ASSISTANT", "stages": [10, 20]}, ["npcl", None, "Quest stage."], ["end"]),
              clause({"cond": False}, ["npcl", None, "Never."]))
    yield case("builder quest stage", [{"op": "open", "file": 0, "npc": 0}, H(1)], 0, files=[quest], quests={"COOKS_ASSISTANT": 20})
    yield case("builder quest stage miss", [{"op": "open", "file": 0, "npc": 0}], 0, files=[quest])
    menu = B(clause("default", ["npcl", None, "Choose."],
                    ["options", None, "picked", [
                        ["option", "Alpha", None, [["npcl", None, "Alpha chosen."], ["end"]]],
                        ["optionIf", "Beta", {"attr": "beta"}, [["npcl", None, "Beta chosen."], ["endWith", [["emit", "beta end"]]]]],
                        ["playerl", "Gamma said", None, [["npcl", None, "Gamma chosen."]]],
                    ]]))
    for b in [2, 3, 4]:
        yield case(f"builder options {b}", [{"op": "open", "file": 0, "npc": 0}, H(1), H(b), H(1), H(1)], 2, files=[menu], attrs={"beta": True})
    yield case("builder options filtered", [{"op": "open", "file": 0, "npc": 0}, H(1), H(3), H(1), H(1)], 2, files=[menu])
    yield case("builder options bad button", [{"op": "open", "file": 0, "npc": 0}, H(1), H(7)], 2, files=[menu])
    single = B(clause("default", ["options", "Header", None, [
        ["optionIf", "Hidden", False, [["npcl", None, "Hidden."]]],
        ["option", "Shown", None, [["manual", [["emit", "after single"]]], ["npcl", None, "Only shown."], ["end"]]]]]))
    yield case("builder single option", [{"op": "open", "file": 0, "npc": 0}, H(1)], 0, files=[single])
    empty = B(clause("default", ["options", None, None, [["optionIf", "Hidden", False, [["npcl", None, "Hidden."]]]]]))
    yield case("builder no option", [{"op": "open", "file": 0, "npc": 0}], 0, files=[empty])
    six = B(clause("default", ["options", None, None, [["option", t, None, []] for t in "ABCDEF"]]))
    yield case("builder six options", [{"op": "open", "file": 0, "npc": 0}], 0, files=[six])
    loop = B(clause("default", ["npcl", None, "Start."], ["placeholder", "top"],
                    ["builder", "top", [["options", None, None, [
                        ["option", "Again", None, [["npcl", None, "Again."], ["goto", "top"]]],
                        ["option", "Leave", None, [["npcl", None, "Bye."], ["end"]]]]]]]))
    yield case("builder placeholder loop", [{"op": "open", "file": 0, "npc": 0}, H(1), H(2), H(1), H(1), H(3), H(1), H(1)], 2, files=[loop])
    unset = B(clause("default", ["placeholder", "p"], ["npcl", None, "After."]))
    yield case("builder unset placeholder", [{"op": "open", "file": 0, "npc": 0}], 0, files=[unset])
    branch = B(clause("default", ["branch", {"attrInt": "level"}, [
        [1, [["npcl", None, "Level one."], ["end"]]],
        [2, [["npcl", None, "Level two."], ["between", [["emit", "between"], ["setAttr", "seen", True]]], ["npcl", None, "After between."], ["end"]]]]]))
    for level in [0, 1, 2]:
        yield case(f"builder branch {level}", [{"op": "open", "file": 0, "npc": 0}, H(1), H(1), H(1)], 0 if level else None,
                   files=[branch], attrs={"level": level})
    yield case("builder branch wrong type", [{"op": "open", "file": 0, "npc": 0}], None, files=[branch], attrs={"level": True})
    bools = B(clause("default", ["branchBool", "flag", True, [
        [True, [["npcl", None, "True."], ["manual", [["emit", "manual"], ["send", ["Manual line."]]]], ["end"]]],
        [False, [["npcl", None, "False."], ["manualGoto", [["emit", "goto"]], {"plus": 2}], ["npcl", None, "Skipped."],
                 ["npcl", None, "Landed."], ["manualGoto", [["stage", 99]], 1000000]]]]]))
    yield case("builder branch bool default", [{"op": "open", "file": 0, "npc": 0}, H(1), H(1), H(1)], 1, files=[bools])
    yield case("builder branch bool false", [{"op": "open", "file": 0, "npc": 0}, H(1), H(1), H(1), H(1)], 2, files=[bools], attrs={"flag": False})
    yield case("builder bare chain", [{"op": "open", "file": 0, "npc": 0}], None, files=[B(bare=[["npcl", None, "x"]])])
    nomatch = B(clause({"cond": {"attr": "never"}}, ["npcl", None, "x"]))
    yield case("builder no clause matches", [{"op": "open", "file": 0, "npc": 0}, H(1)], 0, files=[nomatch])
    ends = B(clause("default", ["npcl", None, "One."], ["between", [["fail"]]], ["npcl", None, "Two."]))
    yield case("builder between fails", [{"op": "open", "file": 0, "npc": 0}, H(1)], None, files=[ends])
    callbacks = B(clause("default", ["manual", [["npcl", "HAPPY", "From a callback."], ["setAttr", "n", 4]]],
                         ["endWith", [["send", ["Ending."]], ["end"]]]))
    yield case("builder callbacks", [{"op": "open", "file": 0, "npc": 0}, H(1)], 1, files=[callbacks])


ITEMS = [[1925, 1, "Unfired bowl"], [1931, 1, "pot_of_FLOUR"], [1511, 5, "logs"], [995, 100, None], [1617, 1, "Uncut diamond"], [4155, 1, "Enchanted gem"]]


def skill_cases():
    types = {"ONE_OPTION": [0], "MAKE_SET_ONE_OPTION": [2], "TWO_OPTION": [0, 1], "THREE_OPTION": [0, 1, 2],
             "FOUR_OPTION": [0, 1, 2, 4], "FIVE_OPTION": [0, 1, 2, 4, 5]}
    for kind, items in types.items():
        handler = {"type": kind, "items": items}
        yield case("skill " + kind, [{"op": "skill", "handler": handler}, H(5, 0)], 0, items=ITEMS, skill=True, inventory={"1925": 3})
        yield case("skill api " + kind, [{"op": "skill", "handler": handler, "via": "api"}, H(4, 0)], 1, items=ITEMS, skill=True)
        yield case("skill table " + kind, [{"op": "skillTable", "type": kind, "items": items}], 0, items=ITEMS, inventory={"1925": 9, "1931": 2})
    for kind, button in [("ONE_OPTION", 3), ("TWO_OPTION", 3), ("FIVE_OPTION", 6), ("THREE_OPTION", 12), ("FOUR_OPTION", 17)]:
        handler = {"type": kind, "items": types[kind]}
        yield case(f"skill prompt {kind}", [{"op": "skill", "handler": handler}, H(button + 1, 0), {"op": "input", "v": 14}], 1, items=ITEMS, skill=True)
        yield case(f"skill prompt text {kind}", [{"op": "skill", "handler": handler}, H(button + 1, 0), {"op": "input", "v": "27"},
                                                 {"op": "input", "v": "x9"}], 2, items=ITEMS, skill=True)
    yield case("skill getAll override", [{"op": "skill", "handler": {"type": "ONE_OPTION", "items": [0], "getAll": 6}}, H(7, 0)], 1, items=ITEMS, skill=True)
    yield case("skill getName override", [{"op": "skill", "handler": {"type": "FIVE_OPTION", "items": [0, 1, 2, 3, 4], "getName": True}}], 0, items=ITEMS, skill=True)
    yield case("skill create fails", [{"op": "skill", "handler": {"type": "TWO_OPTION", "items": [0, 1], "createFails": True}}, H(9, 0)], 1, items=ITEMS, skill=True)
    yield case("skill null type", [{"op": "skill", "handler": {"type": None, "items": [0]}}, H(5, 0)], 0, items=ITEMS, skill=True)
    yield case("skill too few items", [{"op": "skill", "handler": {"type": "THREE_OPTION", "items": [0]}}], None, items=ITEMS, skill=True)
    yield case("skill five with six", [{"op": "skill", "handler": {"type": "FIVE_OPTION", "items": [0, 1, 2, 4, 5, 0]}}], None, items=ITEMS, skill=True)
    yield case("skill null name", [{"op": "skill", "handler": {"type": "FIVE_OPTION", "items": [0, 1, 2, 3, 4]}}], 0, items=ITEMS, skill=True)
    yield case("skill null name shown", [{"op": "skill", "handler": {"type": "THREE_OPTION", "items": [3, 1, 2]}}], 0, items=ITEMS, skill=True)
    yield case("skill no items", [{"op": "skillTable", "type": "ONE_OPTION", "items": []}], None, items=ITEMS)
    yield case("skill unregistered", [{"op": "skill", "handler": {"type": "ONE_OPTION", "items": [0]}}], 0, items=ITEMS)
    yield case("skill key without handler", [{"op": "openKey", "key": 196608}], 0, items=ITEMS, skill=True)
    yield case("skill handle before open", [{"op": "openKey", "key": 196608}, H(5)], None, items=ITEMS, skill=True)
    yield case("skill for length", [{"op": "forLength", "n": n} for n in range(-1, 7)], None)


def gem_cases():
    yield case("gem not started", [{"op": "gem"}], 0)
    yield case("gem started", [{"op": "gem"}], 0, bank={"slayer": True})
    yield case("gem init", [{"op": "gemInit"}], 0)


def cases():
    for group in [banker_cases, labeller_cases, builder_cases, skill_cases, gem_cases]:
        yield from group()


def groups(rows):
    out = {}
    for row in rows:
        key = row["name"].split(" ")[0]
        out[key] = out.get(key, 0) + 1
    return dict(sorted(out.items()))
