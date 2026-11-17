"""Verbatim base NPC factory/init/initConfig with observable external entity services."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request,re
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/npc-init-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
 cases=[]
 def a(kind,**kw):return dict(op=kind,**kw)
 def add(actions=None,**kw):cases.append(dict(actions=actions or[a('config'),a('init')],**kw))
 for level in [-2147483648,-3,0,1,2,5,100,2147483647]:add(defs=[dict(id=0,level=level)])
 for key in ['attack_level','strength_level','defence_level','range_level','magic_level','lifepoints','aggressive','bonuses','attack_speed','force_talk','movement_radius','death_gfx']:
  for value in [None,0,1,-1,'bad',True,[],[1,2,3]]:add(defs=[dict(id=0,level=7,config={key:value})])
 for node in [0,2]:
  for hp in [None,1,40,'bad']:add([a('config',node=node)],hp=hp,defs=[dict(id=node,level=20)])
 animations={k:dict(animation=i)for i,k in enumerate(['melee_animation','magic_animation','range_animation','defence_animation','death_animation'],100)}
 for absent in [None,*animations]:
  config={k:v for k,v in animations.items()if k!=absent};config.update(dict(start_gfx=1,projectile=2,end_gfx=3,start_height=100,prj_height=125,end_height=255));add(defs=[dict(id=0,config=config)])
 for children in [None,[],[-1],[1,-1],[1,2,3]]:
  for active in [False,True]:add([a('init'),a('init')],defs=[dict(id=0,level=10,children=children)],active=active)
 for properties in [None,0,1]:
  for skills in [None,0,1]:add(nodes=[dict(node=0,props=properties,skills=skills)],defs=[dict(id=0,config=animations)])
 for phase in ['properties','skills','combat','static','maximum','current','animation','getAnimation','bonuses','speed','entity','spawn','repository','move','active','renderable','interact','configure','defaultBehavior','base','created']:
  for at in [1,2]:add(defs=[dict(id=0,level=10,config=animations,children=[1,2])],failures={phase:at})
 for phase in ['properties','skills','static','animation','getAnimation','spawn','repository','move','active','interact','configure','defaultBehavior','base']:
  for action in [a('set',node=0,definition=None),a('set',node=0,props=1,skills=1),a('set',node=0,index=77,size=4),a('def',id=0,config={'attack_level':20,'melee_animation':dict(animation=999)})]:add(defs=[dict(id=0,level=10,config=animations,children=[1,2])],hooks=[dict(event=phase,actions=[action])])
 for phase in ['base','interact']:
  for children in [None,[],[None],[None,None,None]]:add([a('init')],defs=[dict(id=0,children=[1,2])],hooks=[dict(event=phase,at=1 if phase=='base'else 2,actions=[a('set',node=0,children=children)])])
 for prototype in [None,1]:
  for constructed in [None,1]:
   for direction in [None,0,6,7]:add([a('factory',id=8,location=[3207,3216,2],direction=direction,args=[1,2,3])],prototype=prototype,constructed=constructed)
 for direction in [None,0,6,7]:add([a('factory',id=8,location=[3207,3216,2],direction=direction,args=[1,2,3])],prototype=1)
 for phase in ['prototype','construct']:
  add([a('factory',id=8,location=None,direction=6,args=[])],prototype=1,failures={phase:1})
 return cases
def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
 if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
 run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[];kt=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
 source=read(SERVER/'core/game/node/entity/npc/NPC.java');methods='\n'.join(c.method(source,s)for s in ['public static NPC create(int id, Location location, Direction direction, Object... objects)','public void init()','public void initConfig()']);adapter('core/game/node/entity/npc/NPC.java',read(HERE/'NPC.java').replace('/* ORIGINAL_METHODS */',methods))
 definition=read(SERVER/'core/cache/def/impl/NPCDefinition.java');base=read(SERVER/'core/cache/def/Definition.java')
 adapter('core/cache/def/impl/NPCDefinition.java','package core.cache.def.impl;import java.util.*;import core.game.system.config.NPCConfigParser;import core.game.world.update.flag.context.*;public class NPCDefinition{public int id,level,size=1;public int[]childNPCIds;public Map<String,Object>handlers=new HashMap<>();public Graphics[]combatGraphics=new Graphics[3];public NPCDefinition(int id){this.id=id;}public int getCombatLevel(){return level;}public Map<String,Object>getHandlers(){return handlers;}'+c.method(base,'public <V> V getConfiguration(String key)')+c.method(base,'public <V> V getConfiguration(String key, V fail)')+c.method(definition,'public Animation getCombatAnimation(int index)')+c.method(definition,'public void initCombatGraphics(Map<String, Object> config)')+'}')
 constants=re.findall(r'const val (\w+) = "([^"]*)"',read(SERVER/'core/game/system/config/NPCConfigParser.kt'));adapter('core/game/system/config/NPCConfigParser.java','package core.game.system.config;public class NPCConfigParser{'+''.join(f'public static final String {k}="{v}";'for k,v in constants)+'}')
 animator=read(SERVER/'core/game/node/entity/impl/Animator.java')
 adapter('core/game/node/entity/impl/Animator.java','package core.game.node.entity.impl;public class Animator{'+c.method(animator,'public static enum Priority')+'}')
 adapter('core/cache/def/impl/AnimationDefinition.java','package core.cache.def.impl;public class AnimationDefinition{public static AnimationDefinition forId(int id){throw new UnsupportedOperationException();}public int getDurationTicks(){throw new UnsupportedOperationException();}}')
 adapter('core/game/node/scenery/Scenery.java','package core.game.node.scenery;public class Scenery{}')
 adapter('core/game/node/entity/Entity.java','package core.game.node.entity;import core.game.world.map.Location;public class Entity{public Location location;public boolean active;public Entity(Location l){location=l;}public Location getLocation(){return location;}public void init(){active=true;oracle.Support.emit("entity",((core.game.node.entity.npc.NPC)this).slot);}}')
 adapter('core/game/node/entity/npc/AbstractNPC.java','package core.game.node.entity.npc;import core.game.world.map.*;public class AbstractNPC extends NPC{public AbstractNPC(int id,Location l){super(id,l);walks=true;}public static AbstractNPC forId(int id){return oracle.Support.prototype(id);}public AbstractNPC construct(int id,Location l,Object...args){return oracle.Support.construct(this,id,l,args);}}')
 adapter('content/global/skill/summoning/familiar/Familiar.java','package content.global.skill.summoning.familiar;public class Familiar extends core.game.node.entity.npc.NPC{public Familiar(int id,core.game.world.map.Location l){super(id,l);}}')
 adapter('core/game/node/entity/skill/Skills.java','''package core.game.node.entity.skill;public class Skills{public static final int ATTACK=0,DEFENCE=1,STRENGTH=2,HITPOINTS=3,RANGE=4,MAGIC=6;public int slot;public int[]statics=new int[24],currents=new int[24];public Skills(int s){slot=s;}public void setStaticLevel(int id,int level){statics[id]=level;oracle.Support.emit("static",slot,id,level);}public void setLevel(int id,int level){currents[id]=level;oracle.Support.emit("current",slot,id,level);}public int getMaximumLifepoints(){int result=statics[3];oracle.Support.emit("maximum",slot);return result;}}''')
 adapter('core/game/world/repository/Repository.java','package core.game.world.repository;import core.game.node.entity.npc.NPC;public class Repository{public static Repository getNpcs(){return new Repository();}public boolean add(NPC n){oracle.Support.add(n);return true;}public static void addRenderableNPC(NPC n){oracle.Support.render(n);}}')
 # NPC.java imports this class in the original; preserve the same simple name.
 p=RUN/'adapters/core/game/node/entity/npc/NPC.java';p.write_text(p.read_text(encoding='utf-8').replace('import java.util.*;','import java.util.*;import core.game.world.repository.Repository;'),encoding='utf-8')
 for name in ['core/game/world/map/Direction.java','core/game/world/update/flag/context/Animation.java','core/game/world/update/flag/context/Graphics.java']:
  p=SERVER/name;read(p);java.append(p)
 adapter('core/game/world/map/Point.java','package core.game.world.map;public class Point{public Point(int x,int y){}}')
 adapter('core/game/world/map/path/ClipMaskSupplier.java','package core.game.world.map.path;public interface ClipMaskSupplier{int getClippingFlag(int z,int x,int y);}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static void move(core.game.node.entity.npc.NPC n,Location a,Location b){oracle.Support.emit("move",n.slot,oracle.Support.coord(a),oracle.Support.coord(b));}public static int getClippingFlag(int z,int x,int y){throw new UnsupportedOperationException();}public static Chunk getRegionChunk(Location l){throw new UnsupportedOperationException();}public static class Chunk{public void flag(Object o){throw new UnsupportedOperationException();}}}')
 adapter('core/game/world/update/flag/chunk/GraphicUpdateFlag.java','package core.game.world.update.flag.chunk;public class GraphicUpdateFlag{public GraphicUpdateFlag(core.game.world.update.flag.context.Graphics g,core.game.world.map.Location l){throw new UnsupportedOperationException();}}')
 location=read(SERVER/'core/game/world/map/Location.kt');adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;val region:Region get()=Region(this);companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)};@JvmInline '+c.method(location,'value class LocationHash(')+'}\nclass Region(val l:Location){val isActive:Boolean get()=oracle.Support.active(l)}')
 read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with p.open('rb')as f:r=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=90)
 if r.returncode:raise RuntimeError(r.stderr.decode(errors='replace'))
 rows=[json.loads(l)for l in r.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim NPC.create/init/initConfig bodies and selected Definition/NPCDefinition methods, complete Animation/Graphics/Direction and original packed Location. Explicit constructors/prototypes, entity/properties/skills, familiar attributes, metadata lookup, repository, regions, configure/behavior/interact services. State, pointer capture, partial errors and callback mutations are compared; does not establish complete NPC/Entity/skills/properties/content/init/world integration.')
 folder=ROOT/'fixtures/npc-init';folder.mkdir(parents=True,exist_ok=True)
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('NPC init fixtures/provenance differ')
  print(f'{len(rows)} original NPC initialization scenarios reproduced.')
 else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n');print(f'Generated {len(rows)} original NPC initialization scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
