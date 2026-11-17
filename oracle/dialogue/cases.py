"""Scenario corpus for the dialogue oracle. Pure data: no JVM, no fixtures.

Append new scenarios at the end only: initial.lock.json pins the first rows.
"""
END = 1000000

def a(op, **v):
    return dict(op=op, **v)

def f(name, value, **v):
    return a("field", f=name, v=value, **v)

def hook(event, *actions, at=1):
    return dict(event=event, at=at, actions=list(actions))

def proto(script=None, ids=(7,), **v):
    return dict(ids=list(ids), script=script or {}, **v)

SUBSTITUTIONS = [
    "", "plain", "@name", "@name@name", "x@servernamey", "@Name @NAME", "@g[he,she]", "@g[he,she] and @g[his,her]!",
    "@g[,]", "@g[a]", "@g[a,b", "@g[a,b]c]", "@g[a]b,c]", "@g[@g[x,y],z]", "@g[$1,$2]", "@g[$2,$1]", "@g[$0,x]",
    "@g[\\$1,b\\\\]", "@g[a\\,b]", "@g[$,x]", "@g[$x,y]", "@g[${a},y]", "@g[${,y]", "@g[$12,y]", "@g[$3,y]",
    "@g[$10,$20]", "@g[$00,y]", "@g[$9,$9]", "@G[a,b]", "@g [a,b]", "@g[a\nb,c\nd]", "@g[é,Σ] Ω",
    "a@g[b,c]d@g[e,f]g@g[h", "@g[one,two],@g[three,four]]", "@g[x,y]@name@servername", "@g[@name,@servername]",
    [64, 103, 91, 0xD83D, 0xDE00, 44, 0xD800, 93], "@g[a,b]@g[$1,$2]", "@g[,b]@g[a,]", "$1 \\ @g[x,y] $",
]

def substitution_cases(add):
    for msg in SUBSTITUTIONS:
        add([a("subst", msg=msg), f("male", False), a("subst", msg=msg)])
    for name in ["@servername", "@g[x,y]", "@name", "", "$1", [0xD800], "Zoë"]:
        add([f("username", name), a("subst", msg="Hi @name, @g[sir,madam] of @servername."), f("male", False),
             a("subst", msg="@name"), a("subst", msg="@g[@name,b]")])
    for server in ["@name", "@g[x,y]", "", "Wörld"]:
        add([f("server", server), a("subst", msg="@servername @name @g[a,b]")])
    add([f("username", None), a("subst", msg="x"), a("subst", msg="@name")])
    add([f("server", None), a("subst", msg="x")])
    add([f("noSettings", True), a("subst", msg="x")])
    add([a("subst", msg=None), a("subst", msg="x", nullPlayer=True), a("subst", msg=None, nullPlayer=True)])
    add([a("subst", msg="@g[a,b]", p=1), f("male", False, p=1), a("subst", msg="@g[a,b] @name", p=1)])
    for phase in ["username", "settings", "male"]:
        add([a("subst", msg="@name @g[a,b]")], failures={phase: 1})

def line_set(count):
    return ["line %d @name" % i for i in range(count)]

def send_cases(add):
    add([a("exprs")])
    for name in ["", "a", "gnome", "slayer-master", "éΣ", [0xD83D, 0xDE00], "x" * 70, None]:
        add([a("keyOf", name=name)])
    for count in range(7):
        add([a("dialogue", lines=line_set(count))])
        for hide in [False, True]:
            add([a("plain", hide=hide, lines=line_set(count))])
    add([a("dialogue", lines=None), a("dialogue", lines=[None, "b"]), a("dialogue", lines=["a"], d=2)])
    # Every sendDialogues overload.
    for count in [0, 1, 2, 3, 4, 5]:
        for npc in [-2, -1, 0, 1, 9000]:
            add([a("say", form="nIh", npc=npc, anim=-1, hide=False, lines=line_set(count)),
                 a("say", form="nIh", npc=npc, anim=9827, hide=True, lines=line_set(count))])
    for npc in [-1, 3, 8591, 8592]:
        for expr in [None, "HAPPY", "OLD_NORMAL", "GRAAHK_NOD"]:
            add([a("say", form="nF", npc=npc, expr=expr, lines=["a", "b"]),
                 a("say", form="nFh", npc=npc, expr=expr, hide=True, lines=["a"]),
                 a("say", form="nI", npc=npc, anim=-1 if expr is None else 591, lines=["a", "b", "c"])])
    for entity in ["p0", "p1", "n0", "n1", "n2", None]:
        for expr in [None, "ANGRY"]:
            add([a("say", form="eF", e=entity, expr=expr, lines=["one"]),
                 a("say", form="eI", e=entity, anim=-1 if expr is None else 9785, lines=["one", "two"]),
                 a("say", form="eFh", e=entity, expr=expr, hide=True, lines=["one"]),
                 a("say", form="eFh", e=entity, expr=expr, hide=False, lines=["one", "two", "three", "four"]),
                 a("say", form="eIh", e=entity, anim=-1 if expr is None else 9785, hide=True, lines=["one"])])
    add([f("nodeId", 12), f("nodeId", -1, p=1), a("say", form="eFh", e="p0", hide=False, lines=["a"]),
         a("say", form="eIh", e="p1", anim=5, hide=True, lines=["a"]), a("say", form="eFh", e="p1", hide=False, lines=["a"], d=1)])
    add([f("shown", 2, n=0), f("shown", None, n=1), f("npcId", -1, n=2), a("say", form="eF", e="n0", lines=["a"]),
         a("say", form="eF", e="n1", lines=["a"]), a("say", form="eF", e="n2", lines=["a"]), a("say", form="eFh", e="n2", lines=["a"])])
    add([f("npcName", "Hans", id=0), f("npcName", None, id=1), f("npcMissing", None, id=2), a("say", form="nI", npc=0, lines=["a"]),
         a("say", form="nI", npc=1, lines=["a"]), a("say", form="nI", npc=2, lines=["a"])])
    add([a("say", form="nI", npc=4, lines=["a", None, "c"]), a("say", form="nI", npc=-1, lines=[None]),
         a("say", form="nI", npc=4, lines=["@g[$3,x]", "after"]), a("say", form="nI", npc=4, lines=["ok @g[a,b]", "@g[$,x]", "never"])])
    for d in [1, 2]:
        add([a("say", form="nI", npc=4, lines=["a"], d=d), a("say", form="eF", e="p0", lines=["a"], d=d),
             a("say", form="eF", e="n0", lines=["a"], d=d), a("say", form="nI", npc=4, lines=[], d=d)])
    add([f("username", None), a("say", form="nI", npc=-1, lines=["a"]), a("say", form="nI", npc=2, lines=["a"])])
    add([f("username", "@g[lord,lady]"), f("male", False), f("server", "@name"), a("say", form="nI", npc=-1, lines=["@name of @servername"])])
    # Options.
    for count in range(8):
        add([a("options", title="Pick", lines=["o%d" % i for i in range(count)])])
    add([a("options", title=None, lines=["a", "b"]), a("options", title="", lines=["a", "b", "c"]),
         a("options", title="T", lines=["a", None, "c"]), a("options", title="T", lines=[None, "b"]),
         a("options", title=None, lines=["a"]), a("options", title="T", lines=["a", "b"], d=2)])
    # Items.
    items = [[995, 1000], [4151, 1], [-1, 0], [7, -3]]
    for count in range(6):
        add([a("item", id=995, lines=line_set(count)), a("item", item=1, lines=line_set(count)),
             a("double", ids=[1, 2], lines=line_set(count)), a("double", items=[0, 3], lines=line_set(count))], items=items)
    add([a("item", item=None, lines=["a"]), a("double", items=[None, 1], lines=["a"]), a("double", items=[1, None], lines=["a"]),
         a("item", item=0, lines=["a", None]), a("item", id=5, lines=["a"], d=2)], items=items)
    for zoom in [0, 1, 2, 3, -1, -3, 2147483647, -2147483648, 5000]:
        add([f("def", [zoom, -7, 2047], id=995), f("def", [zoom, 1, 2], id=7), a("item", item=0, lines=["a"]),
             a("double", items=[0, 3], lines=["a", "b"])], items=items)
    add([a("item", item=0, lines=["a"]), a("double", items=[0, 1], lines=["a"])], items=items,
        hooks=[hook("itemOn", f("def", [10, 20, 30], id=995), f("itemId", 4151, item=0), f("itemAmount", 9, item=0)),
               hook("itemOn", f("def", [40, 50, 60], id=4151), at=2)])
    # Destroy.
    long = " ".join("word%d" % i for i in range(40))
    destroy = [None, "", "Short.", "x" * 200, "y" * 201, " " * 201, "a " * 100, "a " * 101, " " + "b " * 110, "c  " * 80,
               long, long + "  ", "  " + long, "z" * 150 + " " + "q" * 60, "é " * 120, 5]
    for message in destroy:
        add([f("destroyMessage", message, id=4151), a("destroy", id=4151, message="unused"), a("destroy", id=5, message=None)])
    add([f("defName", None, id=9), a("destroy", id=9), a("destroy", id=9, d=2)])
    # Titled plain forms.
    for count in [0, 1, 4, 11, 13]:
        add([a("blue", title="Title", lines=line_set(count)), a("scroll", title="Title", lines=line_set(count))])
    add([a("blue", title=None, lines=["a", None]), a("scroll", title=None, lines=[None]), a("blue", title="t", lines=[], d=2), a("scroll", title="t", lines=[], d=2)])
    add([a("input", string=True, args=["Enter amount:"]), a("input", string=False, args=["Enter amount:"]), a("input", string=True, args=[]),
         a("input", string=False, args=["a", 5, None, True]), a("long", args=["Prompt"]), a("long", args=[]), a("message", to="Zezima"),
         a("message", to=None), a("input", string=True, args=["x"], d=2), a("long", args=["x"], d=2), a("message", to="x", d=2)])

def failure_cases(add):
    """A recording failure at each service call the subject actually makes."""
    items = [[995, 1000], [4151, 1]]
    common = ["dispatch", "text", "manager", "chatbox", "open"]
    speech = common + ["hidden", "anim", "reposition", "username", "settings", "male", "id"]
    subjects = [
        (a("say", form="nI", npc=4, anim=-1, lines=["a @name", "b"]), speech + ["itemOn", "npcOn", "angle", "npcDef"]),
        (a("say", form="eF", e="p0", lines=["a @g[x,y]", "b"]), speech + ["playerOn"]),
        (a("say", form="eFh", e="n1", expr="HAPPY", hide=True, lines=["a"]), speech + ["itemOn", "npcOn", "angle", "npcDef", "npcId"]),
        (a("say", form="eF", e="n1", lines=["a"]), ["shownNpc", "npcId"]),
        (a("say", form="nI", npc=4, lines=[]), ["stderr"]),
        (a("item", id=995, lines=["a", "b"]), common + ["hidden", "itemOn", "angle", "reposition", "itemDef", "newItem"]),
        (a("double", items=[0, 1], lines=["a"]), common + ["hidden", "itemOn", "angle", "reposition", "itemDef"]),
        (a("options", title="T", lines=["a", "b", "c"]), common),
        (a("plain", hide=True, lines=["a", "b"]), common + ["hidden", "id"]),
        (a("destroy", id=995), common + ["itemOn", "itemDef"]),
        (a("scroll", title="T", lines=["a"]), common),
        (a("blue", title="T", lines=["a", "b"]), common),
        (a("input", string=True, args=["p"]), ["dispatch", "runScript"]),
    ]
    for subject, phases in subjects:
        for phase in phases:
            for at in [1, 2, 3, 5, 9]:
                add([subject, a("dialogue", lines=["after"])], items=items, failures={phase: at})

def late_read_cases(add):
    items = [[995, 1000], [4151, 1]]
    say = a("say", form="nI", npc=4, lines=["first @name", "second @servername", "third @g[a,b]"])
    for at in [1, 2, 4, 9, 11, 12]:
        add([say, a("dialogue", lines=["x"])], hooks=[hook("dispatch", f("dispatch", 1), at=at)])
        add([say, a("dialogue", lines=["x"])], hooks=[hook("dispatch", f("dispatch", None), at=at)])
    for event in ["text", "hidden", "anim", "npcOn", "username", "settings", "male", "npcDef"]:
        for at in [1, 2]:
            add([say], hooks=[hook(event, a("line", i=1, v="CHANGED @name"), a("line", i=2, v=None), f("username", "Late"),
                              f("server", "Late world"), f("male", False), f("npcName", "Late npc", id=4), at=at)])
    add([say], hooks=[hook("manager", f("manager", 1)), hook("chatbox", f("componentId", 999, c=0))])
    add([say], hooks=[hook("manager", f("manager", None))])
    add([say], hooks=[hook("chatbox", f("chatbox", None))])
    add([say], hooks=[hook("manager", f("chatbox", 2))])
    add([say, a("say", form="nI", npc=4, lines=["again"])], hooks=[hook("open", f("chatbox", 1)), hook("open", f("chatbox", None), at=2)])
    add([a("say", form="eF", e="p0", lines=["a", "b"])], hooks=[hook("playerOn", f("username", None))])
    add([a("say", form="eF", e="n0", lines=["a"])], hooks=[hook("shownNpc", f("npcId", 77, n=0), f("shown", 1, n=0))])
    add([a("options", title="T", lines=["a", "b", "c"])], hooks=[hook("text", a("line", i=2, v="late"), a("line", i=0, v="ignored")), hook("text", a("line", i=1, v=None), at=2)])
    add([a("item", id=995, lines=["a", "b"])], hooks=[hook("text", a("line", i=0, v="late"), a("line", i=1, v="later"))])
    add([a("blue", title="T", lines=["a", "b", "c"]), a("scroll", title="T", lines=["a", "b"])],
        hooks=[hook("text", a("line", i=1, v="late")), hook("text", a("line", i=1, v="late too"), at=17)])
    add([a("plain", hide=True, lines=["a", "b"])], hooks=[hook("open", f("componentId", 555, c=3)), hook("hidden", a("line", i=9, v="x"))])
    add([a("dialogue", lines=["a", "b"])], hooks=[hook("text", a("line", i=9, v="x"))])
    add([a("double", items=[0, 1], lines=["a"])], items=items, hooks=[hook("itemOn", f("itemId", 5, item=1), f("def", [9, 9, 9], id=4151))])
    # Reentrant interpreter calls from a service.
    add([say, a("close")], hooks=[hook("text", a("dialogue", lines=["nested"]), at=2)])
    add([a("say", form="nI", npc=4, lines=["a"])], hooks=[hook("open", a("say", form="eF", e="p0", lines=["inner"]))])
    add([a("options", title="T", lines=["a", "b"])], hooks=[hook("text", a("options", title="Inner", lines=["x", "y", "z"]))])

A = {"0": [["npc", ["Hello @name."]], ["stage", 1]], "1": [["player", ["Hi."]], ["stage", 2]],
     "2": [["options", ["Yes", "No", "Maybe"]], ["stage", 3]],
     "3": [["btn", 1, [["npcE", "HAPPY", ["Great!"]], ["stage", END]]], ["btn", 2, [["playerE", "SAD", ["Bye."]], ["finish"]]], ["btn", 3, [["end"]]]],
     "-1": [["line", ["It is over."]], ["stage", END]]}

def registry_cases(add):
    protos = [proto(A, ids=[7, 8]), proto(A, ids=[8, 9], cls="B"), proto(A, ids=[], mode="reflect"), proto(A, ids=[65541, -5], cls="B")]
    add([a("contains", id=7), a("add", id=7, proto=0), a("contains", id=7), a("add", id=7, proto=1), a("add", id=7, proto=0),
         a("add", id=65543, proto=2), a("add", id=65543, proto=1), a("add", id=-1, proto=1), a("add", id=-1, proto=2), a("contains", id=-1)], protos=protos)
    add([a("init", proto=0), a("init", proto=1), a("contains", id=8), a("contains", id=9), a("init", proto=3), a("init", proto=3),
         a("init", proto=2), a("contains", id=65541), a("contains", id=-5)], protos=protos)
    add([a("add", id=5), a("contains", id=5), a("open", key=5), a("add", id=5, proto=0), a("add", id=6, proto=0), a("add", id=6)], protos=protos)
    add([a("init", proto=0)], protos=protos, failures={"getIds": 1})
    add([a("simple"), a("simple"), a("contains", id=70099)])

def open_cases(add):
    for mode in ["override", "reflect", "null", "throw", "noctor"]:
        for args in [[], [dict(npc=0)], ["text"], [None], [True], [5, dict(npc=1)], [dict(file=0)]]:
            add([a("init", proto=0), a("open", key=7, args=args), a("handle", c=241, b=1), a("close"), a("open", key=8, args=args)],
                protos=[proto(A, mode=mode)], files=[dict(script={})])
    add([a("open", key=7), a("open", key=7, args=[dict(npc=0)]), a("openName", name="gnome"), a("openName", name=None),
         a("open", key=7, d=2), a("open", key=7, args=[dict(npc=2)], d=2)])
    for npc_field in ["noQueue", "noPulses"]:
        add([f(npc_field, True, n=0), a("open", key=7, args=[dict(npc=0)]), a("init", proto=0), a("open", key=7, args=[dict(npc=0)])], protos=[proto(A)])
    for phase in ["dialoguePlayer", "walkingQueue", "walkReset", "pulseManager", "pulseClear", "trace", "isDebug", "message", "newInstance",
                  "interpreter", "setAttribute", "event", "pluginHandle", "log", "getIds"]:
        for at in [1, 2, 3]:
            add([a("init", proto=0), f("debug", True), a("open", key=7, args=[dict(npc=0)]), a("open", key=7), a("handle", b=1)],
                protos=[proto(A)], failures={phase: at})
            add([a("init", proto=0), a("open", key=7, args=["x"])], protos=[proto(A, mode="reflect")], failures={phase: at})
    add([a("init", proto=0), a("init", proto=1), f("debug", True), a("open", key=7), a("open", key=9), f("debug", False), a("open", key=8)],
        protos=[proto(A, ids=[7]), proto(A, ids=[8, 9], cls="B")])
    # Content-defined open, including a refusal and an immediate end.
    for ret in [True, False]:
        add([a("init", proto=0), a("open", key=7, args=["a", "b"]), a("handle", b=1)],
            protos=[proto(A, open=[["line", ["Opened."]], ["stage", 2]], openRet=ret)])
    add([a("init", proto=0), a("open", key=7), a("handle", b=1)], protos=[proto({"0": [["end"]]})])
    add([a("init", proto=0), a("open", key=7), a("handle", b=1)], protos=[proto({"0": [["stage", END]]})])
    add([a("init", proto=0), a("open", key=7)], protos=[proto({"0": [["fail"]]})])
    # A second open overwrites without closing; a failed newInstance keeps the old dialogue.
    add([a("init", proto=0), a("init", proto=1), a("init", proto=2), a("open", key=7), a("open", key=8), a("handle", b=1), a("open", key=9), a("handle", b=1), a("close")],
        protos=[proto(A, ids=[7]), proto(A, ids=[8], cls="B"), proto(A, ids=[9], mode="throw")])
    add([a("init", proto=0), a("init", proto=1), a("open", key=7), a("open", key=8), a("handle", b=1)],
        protos=[proto(A, ids=[7]), proto(A, ids=[8], mode="null")])
    # The player's current interpreter can differ from the one that opens.
    add([a("init", proto=0), f("interpreter", 1), a("open", key=7), a("handle", b=1), a("handle", b=1, d=1), f("interpreter", None), a("open", key=7), a("handle", b=1)],
        protos=[proto(A)])
    add([a("init", proto=0), a("open", key=7, p=1), a("handle", b=1, p=1), a("handle", b=1, p=0), a("open", key=7, p=0, d=1)], protos=[proto(A)])
    # SimpleDialoguePlugin.
    add([a("simple"), a("open", key=70099, args=["One line."]), a("handle", c=210, b=1), a("handle", c=210, b=1)])
    add([a("simple"), a("open", key=70099, args=["a", "b", "c", "d"]), a("close"), a("open", key=70099, args=["a", "b", "c", "d", "e"]), a("handle", b=1)])
    add([a("simple"), a("open", key=70099), a("open", key=70099, args=[5]), a("open", key=70099, args=["a", None]), a("handle", b=1),
         a("open", key=70099, args=[dict(npc=0)]), a("open", key=70099, args=["a", 5])])
    add([a("simple"), f("interpreter", None), a("open", key=70099, args=["a"]), f("interpreter", 1), a("open", key=70099, args=["a"]), a("handle", b=1), a("handle", b=1, d=1)])
    add([a("simple"), a("open", key=70099, args=["a", "b"]), a("handle", b=1)], hooks=[hook("text", a("line", i=1, v="ignored: SimpleDialoguePlugin copies its arguments"))])

def stage_cases(add):
    base = [a("init", proto=0), a("open", key=7, args=[dict(npc=1)])]
    for path in [[1, 1, 1, 1, 1], [1, 1, 1, 2, 1, 1, 1], [1, 1, 1, 3, 1], [1, 1, 1, 4, 5], [0, 0, 0, 0], [2, 3, 2, 2], [1, 1, 5, 1, 1]]:
        add(base + [a("handle", c=228, b=b) for b in path], protos=[proto(A)])
    add(base + [a("handle", b=-2147483648), a("handle", b=2147483647)], protos=[proto({"*": [["getInc"], ["next"], ["inc"]]})])
    add(base + [a("pm", inst=0, ops=[["stage", 2147483647], ["next"], ["getInc"], ["inc"], ["finish"], ["finished"], ["pirate"]]), f("male", False),
                a("pm", inst=0, ops=[["pirate"], ["close"], ["finished"], ["end"]]), a("pm", proto=0, ops=[["pirate"]]), a("pm", proto=0, ops=[["end"], ["next"], ["close"]])],
        protos=[proto(A)])
    # END_DIALOGUE handling, close events and vetoes.
    for setup in [[], [f("closeEvent", True)], [f("veto", True)], [f("chatbox", None)], [f("manager", None)], [f("interpreter", 1)], [f("interpreter", None)]]:
        add(base + [a("pm", inst=0, ops=[["stage", END]])] + setup + [a("handle", b=1), a("handle", b=1), a("close")], protos=[proto(A)])
        add(base + setup + [a("addAction", action=1), a("addAction"), a("close"), a("close"), a("handle", b=1)], protos=[proto(A)])
    add(base + [a("handle", b=1), a("close"), a("close")], protos=[proto(A, close="veto")])
    add(base + [a("handle", b=1)], protos=[proto({"0": [["stage", END]], "*": [["end"]]}, close="veto")])
    for phase in ["pluginClose", "closeChatbox", "open", "event", "manager", "chatbox", "closeEvent", "setAttribute", "interpreter", "pluginHandle"]:
        for at in [1, 2, 3, 4]:
            add(base + [a("pm", inst=0, ops=[["stage", END]]), a("handle", b=1), a("close")], protos=[proto({})], failures={phase: at})
            add(base + [a("handle", b=1), a("close"), a("handle", b=1)], protos=[proto({"0": [["player", ["x"]]], "*": [["end"]]})], failures={phase: at})
    # Content that fails, closes, reopens or swaps itself.
    add(base + [a("handle", b=1), a("handle", b=1)], protos=[proto({"0": [["npc", ["a"]], ["stage", 1]], "1": [["stage", 5], ["fail"]]})])
    add([a("init", proto=0), a("init", proto=1), a("open", key=7), a("handle", b=1), a("handle", b=1), a("handle", b=1)],
        protos=[proto({"0": [["stage", 1]], "1": [["i", a("close")], ["i", a("open", key=8)], ["npcId", 3, ["still here"]]]}, ids=[7]),
                proto({"0": [["player", ["second plugin"]], ["stage", 1]], "1": [["end"]]}, ids=[8], cls="B")])
    add(base + [a("handle", b=1), a("handle", b=1)], protos=[proto({"0": [["stage", 1]], "1": [["i", a("setDialogue")], ["npcId", 3, ["orphan"]], ["end"]]})])
    add(base + [a("new", proto=0), a("setDialogue", inst=1), a("handle", b=1), a("setDialogue"), a("handle", b=1), a("new", proto=0, nullPlayer=True),
                a("setDialogue", inst=2), a("handle", b=1), a("pm", inst=2, ops=[["npc", ["x"]]]), a("pm", inst=2, ops=[["close"]]), a("pm", inst=2, ops=[["end"]])],
        protos=[proto({"*": [["npc", ["a"]]]})])
    add(base + [a("handle", b=1)], protos=[proto({"0": [["stage", 1]], "*": [["npc", ["a"]]]})], hooks=[hook("event", a("close"), at=1)])
    add(base + [a("handle", b=1)], protos=[proto({"0": [["stage", 1]], "*": [["npc", ["a"]]]})], hooks=[hook("event", a("setDialogue"), at=1)])
    add(base + [a("handle", b=1), a("handle", b=1)], protos=[proto({"*": [["npc", ["a"]], ["next"]]})], hooks=[hook("open", a("close"), at=2)])
    # Helper methods of the plugin base class.
    helpers = [["npc", ["a"]], ["npc", ["a", "b", "c", "d", "e"]], ["npcId", 12, ["a"]], ["npcId", -1, ["a"]], ["npcE", None, ["a"]], ["npcE", "LAUGH", ["a", "b"]],
               ["player", ["a"]], ["playerE", "NEUTRAL", ["a"]], ["npcl", "HAPPY", "word " * 40], ["playerl", None, "short"], ["npcl", "SAD", None],
               ["playerl", "SAD", "x" * 54 + " a b"], ["options", ["a", "b"]], ["options", ["a"]], ["line", ["a", "b"]], ["line", None], ["line", []],
               ["normal", "p1", "HAPPY", ["a"]], ["normal", "n2", None, ["a"]], ["normal", None, None, ["a"]], ["npcl", None, "<b> abcdef " * 9]]
    for npcs in [[1, 9000, 2], [8592, 8591, 0]]:
        for args in [[], [dict(npc=0)], [dict(npc=1)]]:
            for op in helpers:
                add([a("init", proto=0), a("open", key=7, args=args), a("pm", inst=0, ops=[op]), a("pm", proto=0, ops=[op])], npcs=npcs,
                    protos=[proto({}, ids=[9001, 3])])
    add([a("init", proto=0), a("open", key=7), a("pm", inst=0, ops=[["npc", ["a"]]]), a("pm", inst=0, ops=[["npcE", "HAPPY", ["a"]]]),
         a("pm", inst=0, ops=[["setNpc", 2], ["npc", ["a"]], ["setNpc", None], ["npcId", 4, ["b"]]])], protos=[proto({}, ids=[])])
    for phase in ["getIds", "npcId", "shownNpc", "dispatch", "interpreter", "username"]:
        for at in [1, 2, 3]:
            add(base + [a("pm", inst=0, ops=[["npc", ["a"]], ["player", ["b"]]]), a("pm", inst=0, ops=[["setNpc", None], ["npc", ["a"]], ["npcE", "SAD", ["b"]]])],
                protos=[proto({})], failures={phase: at})

def topic_cases(add):
    topics = [dict(text="Stage five", stage=5), dict(text="A file", file=0), dict(text="Other", expr="HAPPY"), dict(text="Skip", stage=6, skip=True),
              dict(text="Hidden", stage=7, **{"if": False}), dict(text="Shown", stage=8, **{"if": True}), dict(text="File skip", file=1, skip=True),
              dict(text="Stage five", stage=9, **{"if": False}), dict(text="End", stage=END), dict(text="Hidden file", file=1, **{"if": False}),
              dict(text="Stage five", stage=11)]
    files = [dict(script={"0": [["npc", ["In file zero."]], ["stage", 1]], "*": [["playerE", "HAPPY", ["file zero again"]]]}),
             dict(script={"*": [["npcl", "NEUTRAL", "In file one."]]}, shadow=True)]
    script = {"*": [["npc", ["plugin stage"]]]}
    selections = [[], [0], [1], [2], [3], [4], [5], [6], [4, 9], [0, 1], [0, 1, 2], [0, 1, 2, 3], [0, 1, 2, 3, 5], [0, 1, 2, 3, 5, 6], [4, 0], [7, 5], [4, 7, 10], [3, 6], [8, 0], [9, 4, 1]]
    for selection in selections:
        for with_npc in [False, True]:
            base = [a("init", proto=0), a("open", key=7, args=[dict(npc=0)] if with_npc else [])]
            add(base + [a("pm", inst=0, ops=[["topics", selection]])] + [a("handle", c=228, b=b) for b in [2, 1]], protos=[proto(script)], files=files, topics=topics)
            add(base + [a("pm", inst=0, ops=[["topics", selection]]), a("handle", b=3), a("handle", b=1)], protos=[proto(script)], files=files, topics=topics)
            add([a("openFile", file=0, args=[dict(npc=0)] if with_npc else []), a("fm", file=0, ops=[["topics", selection]]), a("handle", b=2), a("handle", b=1)],
                files=files, topics=topics)
            add([a("openFile", file=1), a("fm", file=1, ops=[["topicsT", selection, "Ask about"]]), a("handle", b=3), a("handle", b=1), a("handle", b=1)],
                files=files, topics=topics)
    base = [a("init", proto=0), a("open", key=7, args=[dict(npc=0)])]
    for button in [-1, 0, 1, 2, 3, 4, 5, 6, 7, 2147483647]:
        add(base + [a("pm", inst=0, ops=[["topics", [0, 3, 1, 6, 8]]]), a("handle", c=234, b=button), a("handle", b=1), a("handle", b=1)],
            protos=[proto(script)], files=files, topics=topics)
    # Stale active topics: the plugin form follows the first active entry.
    add(base + [a("pm", inst=0, ops=[["topics", [8, 0]], ["topics", [1]]]), a("handle", b=1), a("pm", inst=0, ops=[["topics", [4]], ["topics", [0]], ["topics", []]])],
        protos=[proto(script)], files=files, topics=topics)
    add(base + [a("pm", inst=0, ops=[["topics", [0, 1]]]), a("close"), a("pm", inst=0, ops=[["topics", [0, 1]]]), a("openFile", file=0), a("open", key=7)],
        protos=[proto(script)], files=files, topics=topics)
    add([a("new", proto=0, nullPlayer=True), a("pm", inst=0, ops=[["topics", [4]]]), a("pm", inst=0, ops=[["topics", [0]]]), a("pm", proto=0, ops=[["topics", [4, 0]]])],
        protos=[proto(script)], files=files, topics=topics)
    add(base + [a("pm", inst=0, ops=[["i", a("setDialogue")], ["topics", [1]]]), a("pm", inst=0, ops=[["topics", [0]]])], protos=[proto(script)], files=files, topics=topics)
    for phase in ["dispatch", "text", "open", "interpreter", "event", "fileHandle", "pluginHandle", "setAttribute"]:
        for at in [1, 2, 4]:
            add(base + [a("pm", inst=0, ops=[["topics", [0, 3, 1, 6]]]), a("handle", b=2), a("handle", b=3), a("handle", b=1)], protos=[proto(script)],
                files=files, topics=topics, failures={phase: at})
            add(base + [a("pm", inst=0, ops=[["topics", [1]]]), a("handle", b=1)], protos=[proto(script)], files=files, topics=topics, failures={phase: at})

def file_cases(add):
    conversation = {"0": [["npc", ["File hello."]], ["stage", 1]], "1": [["options", ["One", "Two"]], ["stage", 2]],
                    "2": [["btn", 1, [["player", ["One."]], ["stage", END]]], ["btn", 2, [["returnAt", 2]]]]}
    for args in [[], [dict(npc=0)], [dict(npc=1)], ["x"], [None]]:
        add([a("openFile", file=0, args=args), a("handle", b=1), a("handle", b=1), a("handle", b=1), a("handle", b=1)], files=[dict(script=conversation)])
        add([a("openFile", file=0, args=args), a("handle", b=1), a("handle", b=2), a("handle", b=1), a("handle", b=1), a("close")], files=[dict(script=conversation)])
    add([a("openFile", file=None), a("handle", b=1), a("close"), a("openFile", file=None, args=[dict(npc=2)]), a("handle", b=1)])
    add([a("openFile", file=0, d=2), f("interpreter", None), a("openFile", file=0), f("interpreter", 1), a("openFile", file=0), a("handle", b=1), a("handle", b=1, d=1)],
        files=[dict(script=conversation)])
    helpers = [["npc", ["a"]], ["npc", ["a", "b", "c", "d", "e"]], ["npcId", 12, ["a"]], ["npcE", None, ["a"]], ["npcE", "LAUGH", ["a", "b"]], ["npcIdE", 3, "SAD", ["a"]],
               ["npcIdE", -1, None, ["a"]], ["npcT", 3, "A title", "HAPPY", ["a"]], ["npcT", 3, "A title", None, []], ["npcT", -1, "Player title", None, ["a", "b"]],
               ["player", ["a"]], ["playerE", "NEUTRAL", ["a"]], ["npcl", "HAPPY", "word " * 40], ["npclId", 5, None, "short"], ["npclT", 5, "Long title", "SAD", "word " * 30],
               ["npcl1", "plain npcl"], ["playerl", "SAD", "x" * 54 + " a b"], ["playerl1", "plain playerl"], ["npcl", "SAD", None], ["playerl1", None],
               ["options", ["a", "b"]], ["options", ["a"]], ["optionsT", ["a", "b", "c"], "Custom title"], ["line", ["a", "b"]], ["line", []],
               ["normal", "p1", "HAPPY", ["a"]], ["normal", "n2", None, ["a"]], ["normal", None, None, ["a"]], ["cur"], ["end"], ["endFile"], ["returnAt", 4],
               ["abandon"], ["stage", 9], ["stage", END]]
    for npcs in [[1, 9000, 2], [8592, 8591, 0]]:
        for args in [[], [dict(npc=0)]]:
            for op in helpers:
                add([a("fm", file=0, ops=[op]), a("openFile", file=0, args=args), a("fm", file=0, ops=[op]), a("handle", b=1)], npcs=npcs, files=[dict(script={})])
    for spec in [dict(shadow=True), dict(current=END), dict(current=3), dict(shadow=True, current=END)]:
        add([a("openFile", file=0), a("fm", file=0, ops=[["cur"], ["stage", 4], ["cur"]]), a("handle", b=1), a("fm", file=0, ops=[["stage", END], ["cur"]]), a("handle", b=1), a("handle", b=1)],
            files=[dict(script={"*": [["player", ["x"]]]}, **spec)])
    add([a("fm", file=0, ops=[["load", 0, 1, 0]]), a("fm", file=0, ops=[["load", None, 1, 0]]), a("fm", file=0, ops=[["load", 1, None, None]]),
         a("fm", file=0, ops=[["load", 1, None, 2]]), a("fm", file=0, ops=[["npc", ["a"]], ["player", ["b"]], ["line", ["c"]]]), a("fm", file=0, ops=[["end"], ["endFile"]])],
        files=[dict(script={})])
    # Plugins that divert to files.
    files = [dict(script=conversation), dict(script={"*": [["npcE", "HAPPY", ["second file"]], ["endFile"]]}), dict(script={"0": [["abandon"]]})]
    loader = {"0": [["loadFile", 0], ["i", a("handle", b=1)]], "2": [["npc", ["returned at two"]], ["stage", END]], "*": [["npc", ["plugin again"]], ["stage", END]]}
    add([a("init", proto=0), a("open", key=7, args=[dict(npc=1)]), a("handle", b=1), a("handle", b=2), a("handle", b=1), a("handle", b=1)], protos=[proto(loader)], files=files)
    add([a("init", proto=0), a("open", key=7), a("pm", inst=0, ops=[["loadFile", None], ["loadFile", 1]]), a("handle", b=1), a("handle", b=1),
         a("pm", inst=0, ops=[["loadFile", 2]]), a("handle", b=1), a("handle", b=1), a("pm", inst=0, ops=[["setFile", 0], ["close"]]), a("pm", proto=0, ops=[["loadFile", 0]])],
        protos=[proto(loader)], files=files)
    for options in [[], [["Cook", 0]], [["Cook", 0], [None, 1]], [["a", 0], ["b", 1], ["c", 2], ["d", None], ["e", 0]], [["a", 0]] * 6, [["only", None]]]:
        ops = [["addOption", name, file] for name, file in options] + [["choices"]]
        add([a("init", proto=0), a("open", key=7), a("pm", inst=0, ops=ops), a("handle", b=1), a("handle", b=2)],
            protos=[proto({"1001": [["btn", 1, [["loadFile", 1]]], ["npc", ["choice handled"]]], "*": [["npc", ["plugin"]]]})], files=files)
    # Kotlin spreads copy the array a DialogueFile helper receives; the Java plugin helpers do not.
    late = [hook("text", a("line", i=1, v="mutated"), at=2)]
    add([a("init", proto=0), a("open", key=7, args=[dict(npc=0)]), a("pm", inst=0, ops=[["npc", ["a", "b", "c"]]])], protos=[proto({})], hooks=late)
    add([a("openFile", file=0, args=[dict(npc=0)]), a("fm", file=0, ops=[["npc", ["a", "b", "c"]]])], files=[dict(script={})], hooks=late)
    add([a("init", proto=0), a("open", key=7), a("pm", inst=0, ops=[["options", ["a", "b", "c"]]])], protos=[proto({})], hooks=[hook("text", a("line", i=2, v="mutated"))])
    add([a("openFile", file=0), a("fm", file=0, ops=[["options", ["a", "b", "c"]]])], files=[dict(script={})], hooks=[hook("text", a("line", i=2, v="mutated"))])
    add([a("openFile", file=0), a("fm", file=0, ops=[["line", ["a", "b", "c"]]])], files=[dict(script={})], hooks=[hook("text", a("line", i=2, v="mutated"))])
    for phase in ["fileHandle", "interpreter", "setAttribute", "event", "dispatch", "open", "npcId", "id"]:
        for at in [1, 2, 3, 5]:
            add([a("openFile", file=0, args=[dict(npc=0)]), a("handle", b=1), a("handle", b=1), a("fm", file=0, ops=[["npcT", 3, "T", None, ["a"]]]), a("close")],
                files=[dict(script=conversation)], failures={phase: at})

def close_cases(add):
    add([a("close"), a("close", d=2), a("addAction", action=0), a("addAction"), a("addAction", action=2), a("close"), a("runAction", index=0, b=4),
         a("runAction", index=1), a("runAction", index=3), a("runAction", index=2, b=-1, p=1)])
    add([a("addAction", action=0), a("runAction")], failures={"action": 1})

def requests():
    rows = []
    def add(actions, **v):
        rows.append(dict(actions=actions, **v))
    substitution_cases(add)
    send_cases(add)
    failure_cases(add)
    late_read_cases(add)
    registry_cases(add)
    open_cases(add)
    stage_cases(add)
    topic_cases(add)
    file_cases(add)
    close_cases(add)
    return rows
