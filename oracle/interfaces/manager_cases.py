"""Whole original InterfaceManager lifecycle, aliases and failed/reentrant prefixes."""
def requests():
    cases=[]
    def a(op,**kw):return dict(op=op,**kw)
    def add(actions,**kw):cases.append(dict(actions=actions,lifecycle=True,**kw))
    create=[a('create',id=149),a('create',id=14),a('create',id=548)]
    for kind in [None,*range(9)]:
        for op in ['managerOpen','managerCloseComponent','managerSingle','managerOverlay']:
            add([a('definition',id=149,type=kind),*create,a(op,component=0),a('managerClose'),a('managerCloseSingle'),a('managerCloseOverlay')])
    for ident in [14,149,389]:
        for walkable in [False,True]:
            for allowed in [False,True]:
                for event in [False,True]:
                    add([a('create',id=ident),a('definition',id=ident,walkable=walkable),a('component',component=0,closeEvent=4),a('state',opened=0),a('managerWalkable'),a('managerIsOpened'),a('managerHasMain',id=ident),a('managerClose'),a('managerIsOpened')],closeAllowed=allowed,eventClose=event)
    for value in [None,False,0,'',{},[1],42]:
        add([a('managerState',runscript=value),a('managerClose'),a('managerClose')])
    for op in ['managerOpen','managerSingle','managerOverlay','managerCloseComponent']:
        add([a(op,component=None)])
    for op in ['managerClose','managerCloseChat','managerCloseSingle','managerCloseOverlay','managerHasChat','managerIsOpened','managerWalkable']:
        add([a(op)])
    for kind in [None,*range(9)]:
        add([a('create',id=149),a('definition',id=149,type=kind),a('state',chatbox=0),a('managerHasChat'),a('managerCloseChat'),a('managerHasChat')])
    add([a('create',id=137),a('state',chatbox=0),a('managerHasChat'),a('managerCloseChat')])
    add([a('create',id=149),a('create',id=999),a('managerSingle',component=0),a('managerSingle',component=1),a('managerSameClose',component=0,other=1),a('managerCloseSingle')])
    for ident in [333,334]:
        for allowed in [False,True]:
            add([a('create',id=ident),a('managerOverlay',component=0),a('managerOpenId',id=149),a('managerClose'),a('managerCloseOverlay')],closeAllowed=allowed)
    for index in [-1,*range(16)]:
        add([a('create',id=149),a('create',id=182),a('state',tabs=[0]*14+[1]),a('managerViewedTab',index=index),a('managerRemoveTabs',indices=[index]),a('managerCloseTabs')])
    for length in [0,1,3,4,9,15,16]:
        for allowed in [False,True]:
            add([a('create',id=149),a('create',id=182),a('state',tabs=([0]*(length-1)+[1])if length else []),a('managerRemoveTabs',indices=[3,0,3]),a('managerCloseTabs'),a('managerRestoreTabs')],closeAllowed=allowed)
    for familiar in [False,True]:
        add([a('managerRestoreTabs'),a('managerCloseTabs'),a('managerRestoreTabs')],familiar=familiar)
    for mode in [0,2]:
        add([a('create',id=149),a('component',component=0,hidden=True),a('state',tabs=[0]*15),a('managerRestoreTabs')],mode=mode)
    for op in ['managerRestoreTabs','managerCloseTabs','managerViewedTab','managerRemoveTabs','defaults','tab']:
        add([a('create',id=149),a('managerState',tabsNull=True),a(op,index=0,indices=[0],slot=3,component=0)])
    for ident in [149,14,548,751,750,747,999]:
        for window in [None,2]:
            add([*create,a('state',opened=0,chatbox=1,window=window,tabs=[0]),a('managerState',singleTab=0,overlay=1),a('managerGet',id=ident),a('managerGet',id=ident)])
    for field in ['opened','chatbox','singleTab','overlay','window','tabs']:
        acts=[a('create',id=149),a('create',id=149),a('create',id=548),a('state',window=2)]
        acts += [a('managerState',**{field:1})]if field in ['singleTab','overlay']else[a('state',**{field:[1]if field=='tabs'else 1})]
        add([*acts,a('managerGet',id=149)])
    # Hooks mutate the real current fields at explicit external service boundaries.
    for phase in ['closeEvent','listenerClose','send','dispatchClose','runscriptGet','runscriptRemove','script']:
        for change in [a('state',opened=None),a('state',opened=1),a('component',component=0,id=14),a('state',counter=2147483647),a('managerClose'),a('managerOpenId',id=998)]:
            add([*create,a('component',component=0,closeEvent=4),a('state',opened=0),a('managerState',runscript=1),a('managerClose')],hooks=[dict(event=phase,actions=[change])])
    for phase in ['listenerOpen','plugin','dispatch','log','send']:
        for change in [a('state',opened=None),a('state',opened=1),a('managerState',overlay=1),a('managerState',overlay=None),a('managerOpenId',id=998)]:
            add([a('definition',id=149,plugin=3),*create,a('managerOpen',component=0),a('managerOverlay',component=0)],hooks=[dict(event=phase,actions=[change])])
    for phase in ['runscriptGet','runscriptRemove','script','closeEvent','listenerClose','send','dispatchClose','listenerOpen','plugin','dispatch','log','interfaceConfig','extension','weaponCreate','addWeapon','spellbook','familiar','friendsCreate']:
        for at in [1,2]:
            add([a('definition',id=149,plugin=3),*create,a('component',component=0,closeEvent=4,hidden=True),a('state',opened=0,tabs=[0]),a('managerState',runscript=1),a('managerOpen',component=1),a('managerClose'),a('managerRestoreTabs'),a('managerCloseTabs'),a('managerRestoreTabs')],failures={phase:at})
    for phase in ['interfaceConfig','send','listenerClose']:
        for change in [a('state',tabs=[]),a('state',tabs=[1]*15),a('state',tabs=[None]*15),a('managerState',tabsNull=True),a('state',window=2)]:
            add([*create,a('component',component=0,hidden=True),a('state',tabs=[0]*15),a('managerRestoreTabs'),a('managerRemoveTabs',indices=[0,1])],hooks=[dict(event=phase,actions=[change])])
    return cases
