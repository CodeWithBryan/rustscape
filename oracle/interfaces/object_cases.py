"""Object identity/capture cases appended without changing the original238 rows."""
import copy,itertools
def requests():
 out=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**kw):row=dict(identity=True,actions=actions);row.update(kw);out.append(copy.deepcopy(row))
 objects=[a('objectPlugin',alias='a',label=1),a('objectPlugin',alias='b',label=1),a('objectEvent',alias='ea',label=7,result=True),a('objectEvent',alias='eb',label=7,result=False)]
 base=objects+[a('assignDefinitionPlugin',id=149,alias='a'),a('create',id=149),a('assignClose',component=0,alias='ea')]
 add(base+[a('objectAlias',alias='c',**{'from':'a'}),a('callbackSame',a='a',b='b'),a('callbackSame',a='a',b='c'),a('newInstance',alias='a'),a('fireDefault',alias='a'),a('fireDefault',alias='a',identifier='event'),a('getters',component=0),a('open',component=0),a('close',component=0)])
 add(base+[a('assignDefinitionPlugin',id=149,alias='b'),a('aliasPlugin',component=0,alias='cached'),a('callbackSame',a='a',b='cached'),a('assignPlugin',component=0,alias=None),a('aliasPlugin',component=0,alias='late'),a('callbackSame',a='b',b='late'),a('open',component=0),a('removeDefinition',id=149),a('assignDefinitionPlugin',id=149,alias='a'),a('getters',component=0)])
 add(base+[a('aliasClose',component=0,alias='old'),a('assignClose',component=0,alias='eb'),a('callbackSame',a='old',b='ea'),a('callbackSame',a='old',b='eb'),a('close',component=0),a('eventResult',alias='eb',value=True),a('close',component=0)])
 for mode,kind in itertools.product([-1,1,2],[None,*range(9)]):
  add(base+[a('definition',id=149,type=kind),a('open',component=0),a('close',component=0)],mode=mode)
 for phase,action in itertools.product(['objectPlugin','objectClose','listenerOpen','listenerClose','send'],[a('assignPlugin',component=0,alias='b'),a('assignClose',component=0,alias='eb'),a('eventResult',alias='ea',value=False),a('removeAlias',alias='a'),a('component',component=0,id=762),a('definition',id=149,type=2,tab=4)]):
  add(base+[a('open',component=0),a('close',component=0),a('open',component=0),a('close',component=0)],hooks=[dict(event=phase,actions=[action])])
 for phase in ['objectPlugin','objectClose','listenerOpen','listenerClose','send','setAttr','getAttr']:
  for at in [1,2]:add(base+[a('open',component=0),a('close',component=0),a('guard',component=0),a('attribute',value=False),a('close',component=0)],failures={phase:at})
 for value,missing in itertools.product([None,False,True,0,'bad'],[False,True]):
  add([a('create',id=149),a('guard',component=None if missing else 0),a('attribute',value=value),a('close',component=0),a('aliasClose',component=0,alias='g'),a('getters',component=0)])
 for phase,action in itertools.product(['setAttr','getAttr'],[a('attribute',value=False),a('attribute',value=True),a('assignClose',component=0,alias='ea')]):
  add(base+[a('guard',component=0),a('close',component=0)],hooks=[dict(event=phase,actions=[action])])
 for op in ['assignPlugin','assignClose','assignDefinitionPlugin','eventResult','fireDefault','newInstance']:
  for alias in [None,'a','ea']:
   add(base+[a(op,component=0,id=149,alias=alias,value=False)])
 for phase,action in [('objectPlugin',a('open',component=0)),('objectClose',a('close',component=0))]:
  add(base+[a('open',component=0),a('close',component=0)],hooks=[dict(event=phase,actions=[action])])
 return out
