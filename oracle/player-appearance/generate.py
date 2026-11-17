"""Compile complete pinned Appearance/BodyPart/Gender with actual containers/animations."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/player-appearance-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('inventory',R/'oracle/inventory-listener/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c
spec=importlib.util.spec_from_file_location('animator',R/'oracle/animator/generate.py');animator=importlib.util.module_from_spec(spec);spec.loader.exec_module(animator)
def requests():
 rows=[]
 defs={'0':{},'1':dict(equip=123,walk_anim=901,render_anim=1200),'2':dict(equip=2,remove_sleeves=True),'3':dict(equip=3,remove_head=True,remove_beard=True),'4':dict(equip=4,hat=True),'4041':dict(equip=41),'4042':dict(equip=42),'12842':dict(equip=128)}
 def add(actions=None,**kw):
  row=dict(definitions=defs,npcs={'1':dict(size=3,render=222,stand=100,walk=101,turn=0,turn180=0,cw=102,ccw=-1,melee=400,defence=401,death=402)},items={'weapon':dict(id=1),'chest':dict(id=2),'hat':dict(id=3),'otherHat':dict(id=4),'hood':dict(id=4041)},equipment={},actions=actions or [dict(op='prepare')],hooks=[],style=0,override=None,animate=True,nullOwner=False);row.update(kw);rows.append(copy.deepcopy(row))
 for gender in [0,1]:
  for slot in [0,1,2,3,4,5,7,9,10,12,13]:
   for ident in [1,2,3,4,4041,4042]:add([dict(op='gender',value=gender),dict(op='prepare'),dict(op='prepare')],items={'item':dict(id=ident)},equipment={str(slot):'item'})
 for hat in [None,'hat','otherHat']:
  for hood in [False,True]:
   for ride in [False,True]:
    for look in [0,1,45,51]:
     add([dict(op='part',index=0,look=look,color=17),dict(op='riding',value=ride),dict(op='prepare')],equipment={**({'0':hat}if hat else{}),**({'1':'hood'}if hood else{}),'4':'chest','3':'weapon'})
 for gender in [0,1,-128,127,128,-129,'+１',' 1',None,True]:
  values=[dict(look=i+1,color=i+20)for i in range(7)]
  add([dict(op='capture',name='before',kind='parts'),dict(op='parse',value=dict(gender=gender,appearance_cache=values)),dict(op='prepare')])
 for index in [0,3,6]:
  for field,value in [('look',None),('look','bad'),('look',True),('look',2147483648),('color','bad'),('color',None)]:
   values=[dict(look=i+60,color=2)for i in range(7)];values[index][field]=value;add([dict(op='parse',value=dict(gender=1,appearance_cache=values)),dict(op='prepare')])
 for v in [None,False,[],{},dict(gender=1),dict(gender=1,appearance_cache=None),dict(gender=1,appearance_cache=[]),dict(gender=1,appearance_cache=[None]*7),dict(gender=1,appearance_cache=[False]*7)]:add([dict(op='parse',value=v)])
 for value in [None,0,1]:
  add([dict(op='capture',name='old',kind='parts'),dict(op='gender',value=value),dict(op='retainedPart',name='old',index=0,look=123,color=78),dict(op='copy',target='B',source='A'),dict(op='part',index=0,look=222),dict(op='copy',source='A'),dict(op='prepare',target='B')])
 for defaults in ['gender','animations','attacks']:
  add([dict(op='default',kind=defaults,index=0,value=777),dict(op='defaultAnimations'),dict(op='gender',value=0),dict(op='transform',value=-1)])
 add([dict(op='defaultPart',index=0,look=88,color=99),dict(op='gender',value=0),dict(op='gender',target='B',value=0),dict(op='defaultNull',index=2),dict(op='gender',value=1),dict(op='gender',value=0)])
 for slot in [-1,0,3,8,11,13,14]:
  for value in [-2147483648,-1,0,2147483647]:add([dict(op='drawClothes',index=slot,value=value),dict(op='drawItem',index=slot,item='weapon'),dict(op='clear',index=slot)])
 for ident in [1,12842]:
  for key,val in [('walk_anim',None),('walk_anim',False),('walk_anim',-1),('stand_anim',False),('render_anim',True)]:
   d=copy.deepcopy(defs);d[str(ident)][key]=val;add([dict(op='riding',value=True),dict(op='setAnimations')],definitions=d,items={'weapon':dict(id=ident)},equipment={'3':'weapon'})
 for ident in [-1,0,1]:
  for style in [-2147483648,-1,0,1,3,4,2147483647]:add([dict(op='transform',value=ident),dict(op='transform',value=-1)],style=style,equipment={'3':'weapon'})
 for animations in [[],[None],[456],[456,457],False,7]:
  d=copy.deepcopy(defs);d['1']['attack_anims']=animations;add([dict(op='transform',value=-1)],definitions=d,equipment={'3':'weapon'})
 for combat in [None,False,333]:
  npc=dict(size=2,render=201,stand=100,walk=101,turn=102,turn180=103,cw=104,ccw=105,melee=combat,defence=combat,death=combat);add([dict(op='transform',value=1),dict(op='configAnimation',value=999),dict(op='transform',value=-1)],npcs={'1':npc})
 for value in [None,0,999,False,'bad']:
  add([dict(op='render'),dict(op='setRender',value=88),dict(op='render')],override=value)
 for ride in [False,True]:
  for redirect in [False,True]:add([dict(op='cart',value=ride),dict(op='copter')],redirect=redirect,equipment={'3':'weapon'})
 for when in ['equipment','render','prepared','setPrepared','npc','size','settings','style','properties','combat','sync','appearance','animate','attribute']:
  for op in [dict(op='prepare'),dict(op='transform',value=1),dict(op='cart',value=True),dict(op='copter'),dict(op='render')]:add([op],hooks=[dict(when=when,action='fail')],equipment={'3':'weapon'},override=999)
 for when in ['equipment','render','setPrepared','properties','size','sync','appearance','attribute']:
  for action in ['clearEquipment','redirect','part','override','prepared','reenter']:
   add([dict(op='prepare'),dict(op='transform',value=1),dict(op='copter'),dict(op='render')],hooks=[dict(when=when,action=action)],override=999,equipment={'3':'weapon','4':'chest','0':'otherHat'})
 for op in [dict(op='prepare'),dict(op='transform',value=1),dict(op='transform',value=-1),dict(op='cart',value=True),dict(op='copter'),dict(op='render')]:add([op],nullOwner=True)
 add([dict(op='icons',skull=7,head=9),dict(op='transform',value=1),dict(op='riding',value=True),dict(op='drawClothes',index=12,value=88),dict(op='copy',target='B',source='A'),dict(op='part',look=33),dict(op='getter',name='skin'),dict(op='getter',name='wrists')])
 for name in ['hair','beard','torso','arms','wrists','skin','legs','feet']:
  add([dict(op='getter',name=name),dict(op='animation',index=6,value=444),dict(op='icons',skull=-2147483648,head=2147483647),dict(op='prepare',player=1)])
 for part in range(7):add([dict(op='nullPart',index=part),dict(op='getter',name=['hair','beard','torso','arms','wrists','legs','feet'][part]),dict(op='prepare'),dict(op='parse',value=dict(gender=1,appearance_cache=[dict(look=4,color=5)]*7))])
 for kind,index in [('animations',6),('body',13)]:add([dict(op='capture',kind=kind,name='held'),dict(op='heldSet',name='held',index=index,value=2147483647),dict(op='defaultAnimations'),dict(op='copy',target='B',source='A')])
 add([dict(op='gender',value=None),dict(op='parse',value=dict(gender='bad',appearance_cache=[])),dict(op='parse',value=dict(gender=0,appearance_cache=[]))])
 for item in [None,dict(id=1,definition=None)]:add([dict(op='drawItem',item='nil',index=-1),dict(op='prepare')],items={'nil':item},equipment={'4':'nil'})
 add([dict(op='defaultNull',index=2),dict(op='construct',name='C')])
 add([dict(op='defaultPart',index=0,look=88,color=99),dict(op='construct',name='C'),dict(op='copy',target='B',source='C'),dict(op='part',target='C',look=42,color=23),dict(op='prepare',target='B')])
 return rows

def generate(check):
 animator.c.JAVA_HOME=c.JAVA_HOME;animator.generate(True);base.generate(True);c.SOURCES.update(animator.c.SOURCES)
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 source=c.read(R/'runs/inventory-oracle/adapters/core/cache/def/impl/ItemDefinition.java');original=c.read(S/'core/cache/def/impl/ItemDefinition.java');source=source[:-1]+c.method(original,'public int getEquipId()')+c.method(original,'public int getRenderAnimationId()')+'}'
 adapter('core/cache/def/impl/ItemDefinition.java',source)
 import re
 config=c.read(S/'core/game/system/config/ItemConfigParser.kt');keys=['HIGH_ALCHEMY','LOW_ALCHEMY','REMOVE_SLEEVES','REMOVE_HEAD','REMOVE_BEARD','IS_HAT','STAND_ANIM','STAND_TURN_ANIM','WALK_ANIM','RUN_ANIM','TURN180_ANIM','TURN90CW_ANIM','TURN90CCW_ANIM','ATTACK_ANIMS','RENDER_ANIM_ID']
 adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{'+''.join('public static final String '+k+'='+re.search(r'const val '+k+r' = ("[^"]+")',config).group(1)+';'for k in keys)+'}')
 npc=c.read(S/'core/cache/def/impl/NPCDefinition.java');methods=['public int getSize()','public int getStandAnimation()','public int getWalkAnimation()','public int getTurnAnimation()','public int getTurn180Animation()','public int getTurnCWAnimation()','public int getTurnCCWAnimation()','public int getRenderAnimationId()','public Animation getCombatAnimation(']
 adapter('core/cache/def/impl/NPCDefinition.java','package core.cache.def.impl;import core.game.world.update.flag.context.Animation;import core.game.system.config.NPCConfigParser;public class NPCDefinition extends core.cache.def.Definition<core.game.node.Node>{public int size=1,standAnimation=-1,walkAnimation=-1,turnAnimation=-1,turn180Animation=-1,turnCWAnimation=-1,turnCCWAnimation=-1,renderAnimationId;public static NPCDefinition forId(int id){return oracle.AppearanceOracle.npc(id);}'+''.join(c.method(npc,m)for m in methods)+'}')
 adapter('core/game/system/config/NPCConfigParser.java','package core.game.system.config;public class NPCConfigParser{public static final String MELEE_ANIMATION="melee_animation",MAGIC_ANIMATION="magic_animation",RANGE_ANIMATION="range_animation",DEFENCE_ANIMATION="defence_animation",DEATH_ANIMATION="death_animation";}')
 weapon=c.read(S/'core/game/node/entity/combat/equipment/WeaponInterface.java');line=next(l for l in weapon.splitlines()if 'public static final Animation[] DEFAULT_ANIMS ='in l)
 adapter('core/game/node/entity/combat/equipment/WeaponInterface.java','package core.game.node.entity.combat.equipment;import core.game.world.update.flag.context.Animation;import core.game.node.entity.impl.Animator.Priority;public class WeaponInterface{'+line+'}')
 equipment=c.read(S/'core/game/container/impl/EquipmentContainer.java');constants=''.join(re.findall(r'public static final int SLOT_[^;]+;',equipment))
 adapter('core/game/container/impl/EquipmentContainer.java','package core.game.container.impl;public class EquipmentContainer{'+constants+'}')
 adapter('core/game/node/entity/player/Player.java',c.read(H/'Player.java'))
 for name in ['Appearance','BodyPart','Gender']:p=S/f'core/game/node/entity/player/link/appearance/{name}.java';c.read(p);java.append(p)
 java.append(H/'AppearanceOracle.java');c.read(H/'AppearanceOracle.java')
 # Container/Item/Animation are the complete unchanged original classes from
 # freshly reproduced independent assemblies. Only player/world lookups adapt.
 cp=os.pathsep.join(map(str,[classes,R/'runs/inventory-oracle/classes',R/'runs/animator-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 data=''.join(json.dumps(row)+'\n'for row in requests()).encode();(O/'input.jsonl').write_bytes(data);exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 p=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.AppearanceOracle'],cwd=R,input=data,capture_output=True,timeout=120);(O/'stderr.log').write_bytes(p.stderr);assert p.returncode==0,p.stderr.decode(errors='replace')
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(requests());assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,requests()))
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),scope='Complete unchanged Appearance/BodyPart/Gender with actual original Container/Item/Animation and verbatim definition getters/WeaponInterface defaults. Player/render/Settings/properties/attribute/animation/world services are explicit recording adapters. Full API state, aliases, defaults, body preparation, NPC/human transitions, nullable attacks, parse mutation/error order, reentrant callbacks and failed prefixes compare. Full Player/render writer/gameplay bindings and remaining properties/profile/content remain separate.')
 folder=R/'fixtures/player-appearance';folder.mkdir(exist_ok=True)
 if check:assert (folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text())==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
 print(len(rows),'complete-original Appearance scenarios reproduced'if check else'complete-original Appearance scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
