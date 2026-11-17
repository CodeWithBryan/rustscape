"""Exact original IfAction/CloseIface bodies and deferred login pulse."""
def requests():
 rows=[]
 def a(op,**kw):return dict(op=op,**kw)
 def add(actions,**kw):rows.append(dict(packet=True,actions=actions,**kw))
 for iface in [0,1,2,3,64,746,999]:
  for opcode in [9,132,155,196]:
   for dialogue in [False,True]:
    add([a('packetState',dialogue=dialogue),a('ifAction',component=iface,iface=762 if iface==0 else 763 if iface==1 else 767 if iface==2 else 99 if iface==3 else iface,opcode=opcode,child=73)])
 for locked,zone,hidden in [(False,False,False),(True,False,False),(False,True,False),(False,False,True)]:
  add([a('packetState',dialogue=True,locked=locked,zone=zone),a('packetComponent',component=0,hidden=hidden),a('ifAction',iface=762,child=73,opcode=155)])
 for value in [None,False,True,0,'bad']:
  for mapvalue in [None,False,True]:
   add([a('attr',key='logging_in',value=value),a('attr',key='worldMap:viewing',value=mapvalue),a('closePacket'),a('pulse',index=0)])
 for allowed in [False,True]:
  for resizable in [False,True]:add([a('packetState',closeAllowed=allowed,resizable=resizable),a('attr',key='worldMap:viewing',value=False),a('closePacket'),a('ifAction',iface=762,child=73,opcode=155)])
 add([a('attr',key='logging_in',value=False),a('attr',player=1,key='logging_in',value=True),a('closePacket'),a('closePacket',player=1),a('pulse',index=0),a('pulse',index=1)])
 for phase in ['packetDebug','dialogue','active','dialogueClose','locks','componentLocked','zone','zoneClick','manager','lookup','hidden','pluginGet','buttonEvent','pluginHandle','managerClose','getAttr','submit','removeAttr','dispatch','resizable','pane','unlock','configureWorld']:
  for at in [1,2]:
   add([a('plugin',name='a',label=7),a('packetComponent',component=3,plugin='a'),a('packetState',dialogue=True),a('ifAction',iface=99,child=5,opcode=155),a('attr',key='logging_in',value=True),a('attr',key='worldMap:viewing',value=True),a('closePacket'),a('pulse',index=0)],failures={phase:at})
 for phase in ['packetDebug','dialogue','active','dialogueClose','locks','componentLocked','zone','zoneClick','manager','lookup','hidden','pluginGet','buttonEvent','managerClose','getAttr','submit','removeAttr','dispatch','resizable']:
  for change in [a('packetState',locked=True,zone=True),a('packetComponent',component=3,plugin='b',hidden=True),a('set',component=3,id=762),a('packetState',visible={}),a('attr',key='worldMap:viewing',value=True)]:
   add([a('plugin',name='a',label=7),a('plugin',name='b',label=7),a('packetComponent',component=3,plugin='a'),a('packetState',dialogue=True),a('ifAction',iface=99,child=5,opcode=155),a('closePacket')],hooks=[dict(event=phase,actions=[change])])
 # Captured plugin is invoked even if the event replaces its component field.
 add([a('plugin',name='a',label=7),a('plugin',name='b',label=7),a('packetComponent',component=3,plugin='a'),a('ifAction',iface=99,child=5,opcode=155)],hooks=[dict(event='buttonEvent',actions=[a('packetComponent',component=3,plugin='b')])])
 # Registry lookup happens after dispatch and can select the component's new ID.
 add([a('plugin',name='a',label=7),a('packetComponent',component=3,plugin='a'),a('ifAction',iface=99,child=73,opcode=155)],hooks=[dict(event='buttonEvent',actions=[a('set',component=3,id=762)])])
 return rows
