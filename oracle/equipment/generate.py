"""Complete unchanged EquipmentContainer with explicit player/content receivers."""
import argparse,copy,hashlib,importlib.util,json,os,pathlib,re,subprocess
R=pathlib.Path(__file__).resolve().parents[2];H=pathlib.Path(__file__).resolve().parent;O=R/'runs/equipment-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('inventory',R/'oracle/inventory-listener/generate.py');base=importlib.util.module_from_spec(spec);spec.loader.exec_module(base);c=base.c

def requests():
 rows=[]
 defs={'0':{},'1':dict(slot=3,bonuses=[0,0,0,0,0,1,2,3,4,5,6,7,8,9,10],requirements={'2':1}), '2':dict(slot=5), '3':dict(slot=3,two=True,speed=7),'4':dict(slot=13,stack=True),'5':dict(slot=1), '6':dict(slot=-1),'7':dict(slot=3,equipment='plugin'),'8':dict(slot=5,equipment='plugin'),'11283':dict(slot=5)}
 def add(actions=None,**kw):
  row=dict(definitions=defs,items={'new':dict(id=1),'old':dict(id=7),'shield':dict(id=8),'two':dict(id=3),'arrows':dict(id=4,amount=30),'cape':dict(id=5)},inventory={'0':'new'},equipment={},event={},actions=actions or [dict(op='equip',item='new')],hooks=[],allowed=True,interaction=True,plugin=None,extension=True,dfs=False,special=False,fine=False,grand=False,main=False);row.update(kw);rows.append(copy.deepcopy(row))
 for op in ['equip','explicit','one','three','array','list','all','absent','shift','refresh','update','bonuses','stats']:
  for fire in [False,True]:
   for occupied in [False,True]:add([dict(op=op,item='new',fire=fire)],equipment={'3':'old','5':'shield'}if occupied else {},event={'1':'cape','3':'new','5':'shield'})
 for slot in [-2,-1,0,1,2,3,4,5,7,9,10,12,13,14,2147483647]:
  for allowed in [False,True]:
   d=copy.deepcopy(defs);d['1']['slot']=slot;add(definitions=d,allowed=allowed)
 for ident in [1,2,3,4,5,6]:
  for origin in [False,True]:
   for fire in [False,True]:
    add([dict(op='explicit',item='new',fromInventory=origin,fire=fire,inventorySlot=0)],items={'new':dict(id=ident),'old':dict(id=7),'shield':dict(id=8),'cape':dict(id=5)},equipment={'3':'old','5':'shield'})
 for plugin in [None,False,True,1,'bad']:
  for interact in [False,True]:
   for full in [False,True]:
    add(equipment={'3':'old','5':'shield'},plugin=plugin,interaction=interact,inventory={str(i):'new' for i in range(28)}if full else {'0':'new'})
 for old in [0,1,100,2147483640,2147483647]:
  for amount in [-1,0,1,30,2147483647]:
   for origin in [False,True]:
    add([dict(op='explicit',item='new',fromInventory=origin,inventorySlot=0,fire=False)],items={'new':dict(id=4,amount=amount,charge=15),'old':dict(id=4,amount=old,charge=900)},equipment={'13':'old'})
 for extension in [False,True]:
  for dfs in [False,True]:
   for special in [False,True]:
    for op in ['refresh','update']:add([dict(op=op)],equipment={'3':'old'},event={'1':'cape','3':'new','5':'shield'},extension=extension,dfs=dfs,special=special,main=True)
 for amount in [0,1,19,20,65535]:
  for fine in [False,True]:
   for grand in [False,True]:
    add([dict(op='bonuses')],equipment={'3':'new','5':'dfs'},items={'new':dict(id=1),'dfs':dict(id=11283,charge=amount)},fine=fine,grand=grand,main=True)
 for values in [[],[3],[-1]*15,[2147483647]*15,[-2147483648]*15,[1]*16]:
  d=copy.deepcopy(defs);d['1']['bonuses']=values;d['11283']['bonuses']=values
  for fine in [False,True]:add([dict(op='bonuses')],definitions=d,equipment={'3':'new','5':'dfs'},items={'new':dict(id=1),'dfs':dict(id=11283,charge=65535)},fine=fine,grand=True,main=True)
 for field,value in [('slot',True),('two',3),('speed',False),('bonuses',False),('requirements',False),('equipment',True)]:
  d=copy.deepcopy(defs);d['1'][field]=value;d['7'][field]=value
  for op in ['equip','refresh','bonuses']:add([dict(op=op,item='new')],definitions=d,equipment={'3':'old'})
 for when in ['inventory','equipment','requirements','dispatch','plugin','interaction','packet','properties','attackSpeed','extension','weapon','attribute','removeAttribute','combatPulse','clearHandler','settings','special','varp','appearance','animations','updateAppearance','weight','setBonuses','interfaces','hasMain','cape','defence','text']:
  for op in ['equip','refresh','update']:
   add([dict(op=op,item='new')],equipment={'3':'old','5':'shield'},event={'1':'cape','3':'new','5':'shield'},dfs=True,main=True,hooks=[dict(when=when,action='fail')])
 for when in ['inventory','requirements','plugin','interaction','packet','appearance','properties','weight','text']:
  for action in ['clearInventory','clearEquipment','amount','index','event']:
   add(equipment={'3':'old','5':'shield'},hooks=[dict(when=when,action=action,item='new',value=3)])
 # Cached definition/Node name and original input slot are distinct from copies.
 add(items={'new':dict(id=1,index=3),'old':dict(id=7)},equipment={})
 add(items={'new':dict(id=1,index=3,name=None),'old':dict(id=7)},equipment={})
 for op in ['equip','explicit','one','refresh','update']:
  add([dict(op=op,item=None)],nullPlayer=True)
 for op in ['equip','explicit','refresh','update','bonuses']:
  add([dict(op=op,item='new')],items={'new':dict(id=6,definition=1),'old':dict(id=7,definition=3)},equipment={'3':'old'},event={'3':'new'})
 for slot in [3,5,13]:
  d=copy.deepcopy(defs);d['1'].update(slot=slot,two=True)
  add(definitions=d,equipment={str(slot):'old','5':'old'})
 for when in ['packet','plugin','interaction','properties','text','setBonuses']:
  for action in ['replaceArray','clearBonuses']:
   add([dict(op='refresh'),dict(op='equip',item='new'),dict(op='bonuses')],equipment={'3':'old','5':'shield'},main=True,hooks=[dict(when=when,action=action)])
 for when in ['packet','properties','appearance','weight','text']:
  add([dict(op='update'),dict(op='refresh')],equipment={'3':'old','5':'shield'},event={'3':'new','5':'shield'},hooks=[dict(when=when,action='reenter')])
 for op in ['equip','explicit','refresh','update','bonuses','stats']:
  for main in [False,True]:
   add([dict(op=op,item='new')],detachedEquipment=True,ownEquipment={'3':'two'},items={'new':dict(id=2),'old':dict(id=7),'shield':dict(id=8),'two':dict(id=3),'cape':dict(id=5)},equipment={'3':'old','5':'shield'},event={'1':'cape','3':'new','5':'shield'},main=main)
 return rows

def generate(check):
 baseline=R/'fixtures/inventory/oracle.jsonl';before=hashlib.sha256(baseline.read_bytes()).hexdigest();base.generate(True);assert hashlib.sha256(baseline.read_bytes()).hexdigest()==before
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);java=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 source=c.read(R/'runs/inventory-oracle/adapters/core/cache/def/impl/ItemDefinition.java');original=c.read(S/'core/cache/def/impl/ItemDefinition.java')
 source=source[:-1]+'public boolean hasRequirement(core.game.node.entity.player.Player p,boolean a,boolean b){return oracle.EquipmentOracle.requirement(this,p,a,b); }'+c.method(original,'public int getRequirement(')+'}'
 adapter('core/cache/def/impl/ItemDefinition.java',source)
 config=c.read(S/'core/game/system/config/ItemConfigParser.kt');keys=['HIGH_ALCHEMY','LOW_ALCHEMY','EQUIP_SLOT','TWO_HANDED','BONUS','ATTACK_SPEED','REQUIREMENTS']
 adapter('core/game/system/config/ItemConfigParser.java','package core.game.system.config;public class ItemConfigParser{'+''.join('public static final String '+k+'='+re.search(r'const val '+k+r' = ("[^"]+")',config).group(1)+';'for k in keys)+'}')
 adapters={
 'core/net/packet/PacketRepository.java':'package core.net.packet;public class PacketRepository{public static void send(Class<?>t,core.net.packet.context.ContainerContext c){oracle.EquipmentOracle.packet(c);}}',
 'core/net/packet/out/WeightUpdate.java':'package core.net.packet.out;public class WeightUpdate{}',
 'core/game/node/entity/skill/Skills.java':'package core.game.node.entity.skill;public class Skills{public static final int STRENGTH=2;}',
 'org/rs09/consts/Components.java':'package org.rs09.consts;public class Components{public static final int EQUIP_SCREEN2_667=667;}',
 'org/jetbrains/annotations/Nullable.java':'package org.jetbrains.annotations;public @interface Nullable{}',
 'core/plugin/Plugin.java':'package core.plugin;public class Plugin<T>{public Object fireEvent(String n,Object...a){return oracle.EquipmentOracle.plugin(n,a);}}',
 'core/game/interaction/InteractionListeners.java':'package core.game.interaction;public class InteractionListeners{public static boolean run(int id,core.game.node.entity.player.Player p,core.game.node.item.Item i,boolean e){return oracle.EquipmentOracle.interaction(id,p,i,e);}}',
 'core/game/node/entity/combat/equipment/WeaponInterface.java':'package core.game.node.entity.combat.equipment;public class WeaponInterface{public void updateInterface(){oracle.EquipmentOracle.hit("weapon");}}',
 'content/global/skill/skillcapeperks/SkillcapePerks.java':'package content.global.skill.skillcapeperks;public class SkillcapePerks{public static final String FINE_ATTUNEMENT="fine",GRAND_BULLWARK="grand";public static boolean isActive(String k,core.game.node.entity.player.Player p){oracle.EquipmentOracle.hit("perk",k);return Boolean.TRUE.equals(oracle.EquipmentOracle.row.get(k));}}',
 'content/global/skill/skillcapeperks/SkillcapeEquipmentPlugin.java':'package content.global.skill.skillcapeperks;public class SkillcapeEquipmentPlugin{public static void updateCapePerks(core.game.node.entity.player.Player p,core.game.container.Container c,int s){oracle.EquipmentOracle.hit("cape",s);}}',
 'core/api/ContentAPIKt.java':'package core.api;public class ContentAPIKt{public static void setVarp(core.game.node.entity.player.Player p,int i,int v){oracle.EquipmentOracle.hit("varp",i,v);}public static void log(Object o,int l,String s){throw new AssertionError(s);}}',
 }
 for n,t in adapters.items():adapter(n,t)
 adapter('core/game/node/entity/player/Player.java',c.read(H/'Player.java'))
 for n in ['core/game/container/impl/EquipmentContainer.java']:
  p=S/n;c.read(p);java.append(p)
 java.append(H/'EquipmentOracle.java');c.read(H/'EquipmentOracle.java')
 cp=os.pathsep.join(map(str,[classes,R/'runs/inventory-listener-oracle/classes',R/'runs/inventory-oracle/classes',R/'runs/inventory-oracle/json-simple-1.1.1.jar']))
 c.run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();data=''.join(json.dumps(row)+'\n'for row in cases).encode();(O/'input.jsonl').write_bytes(data)
 exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java';p=subprocess.run([str(exe),'-Dfile.encoding=UTF-8','-cp',cp,'oracle.EquipmentOracle'],cwd=R,input=data,capture_output=True,timeout=120);(O/'stderr.log').write_bytes(p.stderr);assert p.returncode==0,p.stderr.decode(errors='replace')
 rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);assert all({k:v for k,v in row.items()if k!='expected'}==case for row,case in zip(rows,cases))
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in rows).encode();c.read(pathlib.Path(__file__))
 provenance=dict(format=1,upstream=json.loads(c.read(R/'upstream.lock.json')),sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),unchangedInventoryBaseline=before,scope='Complete unchanged EquipmentContainer including private EquipmentListener and bonus/stat methods, unchanged Container/Event/Item and packet writers. Definition.hasRequirement and player/content/packet-dispatch/Settings/appearance/weapon/combat/perk services are explicit recording adapters. Original getRequirement retained verbatim. Compares overload dispatch, live aliases, partial mutations/failures, requirements and hook results, slots, stacks, callbacks, exact container bytes and bonus narrowing. Does not prove full Player/equipment services, accepted live Equip or profile/content integration.')
 folder=R/'fixtures/equipment';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text())==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n')
 print(len(rows),'complete-original EquipmentContainer scenarios reproduced'if check else'complete-original EquipmentContainer scenarios generated')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
