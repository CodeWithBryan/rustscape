"""Compile the complete pinned modern interaction registry without source edits."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,itertools,urllib.request
H=pathlib.Path(__file__).resolve().parent;R=H.parents[1];O=R/'runs/interaction-listeners-oracle';S=R/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',R/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
def a(op,**v):return dict(op=op,**v)
def requests():
 rows=[]
 def add(actions,**v):rows.append(dict(actions=actions,**v))
 for kind,method,locked,movement,instant,zone,result in itertools.product(range(5),range(4),[False,True],[False,True],[False,True],[False,True],[False,True]):
  setup=[]
  if method==1:setup=[a('add',kind=kind)]
  if method==2:setup=[a('generic',kind=kind)]
  if method==3:setup=[a('metadata',kind=kind)]
  add(setup+[a('run',kind=kind),a('pulse')],locked=locked,movementLocked=movement,instant=instant,zone=zone,result=result)
 for kind,method,flipped,instant,zone,result in itertools.product(range(5),range(5),[False,True],[False,True],[False,True],[False,True]):
  setup=[]
  if method==1:setup=[a('use',used=2 if flipped else 1,with_=1 if flipped else 2,kind=kind)]
  if method==2:setup=[a('any',with_=[2],kind=kind)]
  if method==3:setup=[a('wildcard',kind=kind)]
  if method==4:setup=[a('use',used=-1,with_=1,kind=4)]
  add(setup+[a('runUse',kind=kind,withNode=3 if method==4 else 2),a('pulse')],instant=instant,zone=zone,result=result)
 for equip,result,replace in itertools.product([False,True],[False,True],[False,True]):
  add([a('equip',equip=equip),a('runEquip',equip=equip)],result=result,hooks=[dict(event='dispatchOn',actions=[a('equip',equip=equip,function=1)])] if replace else [])
 for opts in [['Use','use'],['a:b','a:b'],['I','i'],['Σ','σ'],['İ','i\u0307'],['one','two']]:
  for locale in ['ROOT','tr']:
   add([a('add',options=opts),a('add',options=opts,function=1),a('get',option=opts[0])],locale=locale)
 for kind in [-1,0,2,4,5]:
  add([a('add',kind=kind),a('add',kind=kind)])
  add([a('generic',kind=kind),a('generic',kind=kind)])
  add([a('use',kind=kind),a('use',used=2,with_=1,kind=kind)])
 for arrays in [False,True]:
  add([a('use'),a('useArrays' if arrays else 'use'),a('getUse')])
 for opt in ['use','Use','I','Σ']:
  add([a('metadata',generic=True,options=[opt]),a('run',option=opt,kind=1),a('pulse')])
 for dest in ['specific','id','generic']:
  for kind in range(5):
   add([a('destination',mode=dest,kind=kind),a('add',kind=kind),a('run',kind=kind),a('pulse')])
 for phase in ['scripts','removeWeak','locks','interactionLocked','setOption','dispatchOn','manager','movement','runPulse','zone','zoneOn','location','face','invokeOn','id','predicate','dispatchUse','invokeUse','zoneUse','install']:
  add([a('add'),a('run'),a('pulse')],failures={phase:1})
  add([a('use'),a('runUse'),a('pulse')],failures={phase:1})
  add([a('metadata'),a('run'),a('pulse')],failures={phase:1})
 for phase in ['removeWeak','dispatchOn','manager','movement','zoneOn','face','invokeOn']:
  add([a('add'),a('run'),a('pulse'),a('pulse')],hooks=[dict(event=phase,actions=[a('addArrays',function=1),a('field',name='manager',value=1),a('field',name='nodeId',value=99)])])
 for result in [False,True]:
  add([a('wildcard'),a('getUse')],predicate=result,hooks=[dict(event='predicate',actions=[a('wildcard',function=1)])])
 add([a('metadata',options=['use']),a('run'),a('pulse')],hooks=[dict(event='dispatchOn',actions=[a('metadata',function=1,distance=7,persist=True)])])
 add([a('generic',options=['USE']),a('metadata'),a('run'),a('pulse')])
 add([a('useMetadata',with_=[2,3]),a('useMetadata',with_=[]),a('useMetadata',with_=7)])
 for kind in range(5):
  add([a('useIds',kind=kind,with_=[2,3,2]),a('getUse',kind=kind,with_=3)])
  add([a('addArrays',ids=[123,124,123],kind=kind),a('get',id=124,kind=kind)])
  add([a('use',used=2,with_=1,kind=kind),a('useIds',kind=kind,with_=[3,1,4]),a('getUse',used=1,with_=3,kind=kind)])
  add([a('useArrays',kind=kind,used=[1,2],with_=[2,3]),a('getUse',used=2,with_=3,kind=kind)])
 for opt in ['use','Use','I','i','Σ','σ','İ','i\u0307']:
  for locale in ['ROOT','tr']:
   add([a('generic',options=[opt]),a('getGeneric',option=opt),a('getGeneric',option=opt.lower())],locale=locale)
   for mode in ['specific','id','generic']:
    add([a('destination',mode=mode,options=[opt]),a('getDest',mode=mode,option=opt),a('getDest',mode=mode,option=opt.lower())],locale=locale)
 for use in [False,True]:
  cls='oracle.UseHandler'if use else'oracle.OnHandler'
  add([a('isInstant',use=use),a('instant',**{'class':cls}),a('isInstant',use=use),a('instant',**{'class':cls,'enabled':False}),a('isInstant',use=use)])
 for equip in [False,True]:add([a('getEquip',equip=equip),a('equip',equip=equip),a('getEquip',equip=equip)])
 for kind in [1,2,3,4]:add([a('use',used=2,with_=1,kind=kind),a('runUse',usedNode=2,withNode=1,kind=kind),a('pulse')])
 for phase in ['id','dispatchUse','invokeUse','location','face']:
  add([a('use',kind=0,used=2,with_=1),a('runUse',kind=0),a('pulse')],hooks=[dict(event=phase,actions=[a('field',name='nodeId',value=77,node=1),a('field',name='nodeId',value=88,node=2)])])
 for phase in ['scripts','removeWeak','setOption','dispatchOn','install','manager','movement']:
  add([a('metadata'),a('run'),a('pulse')],hooks=[dict(event=phase,actions=[a('field',name='scripts',value=1),a('field',name='manager',value=1),a('metadata',function=1,distance=-1,persist=True)])])
 add([a('use'),a('runUse'),a('useArrays',function=1),a('pulse')])
 for phase in ['zoneUse','dispatchUse']:
  add([a('use'),a('runUse'),a('pulse')],hooks=[dict(event=phase,actions=[a('useArrays',function=1)])])
 for mode in ['specific','id','generic']:
  add([a('destination',mode=mode,id=2 if mode=='id' else 123),a('add'),a('run'),a('pulse')],hooks=[dict(event='manager',actions=[a('destination',mode=mode,id=2 if mode=='id' else 123,function=1)])])
 return rows
def generate(check):
 lock=json.loads(c.read(R/'upstream.lock.json'));assert c.run(['git','-C',R/'reference/server','rev-parse','HEAD']).stdout.decode().strip()==lock['server']['commit'];c.run(['git','-C',R/'reference/server','diff','--quiet','HEAD','--'])
 O.mkdir(exist_ok=True);classes=O/'classes';classes.mkdir(exist_ok=True);kt=[]
 def adapter(name,text):
  p=O/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');kt.append(p)
 for name in ['InteractionListeners.kt','InteractionListener.kt','IntType.kt','Script.kt']:
  p=S/'core/game/interaction'/name;c.read(p);kt.append(p)
 adapter('core/api/ContentInterface.kt','package core.api\ninterface ContentInterface')
 adapter('content/global/handlers/item/SpadeDigListener.kt','package content.global.handlers.item\nobject SpadeDigListener{fun registerListener(l:core.game.world.map.Location,m:(core.game.node.entity.player.Player)->Unit){}}')
 adapter('core/game/world/map/Location.kt','package core.game.world.map\ndata class Location(val tag:Int)')
 adapter('core/game/interaction/Option.kt','package core.game.interaction\nclass Option(val name:String,val index:Int)')
 adapter('core/game/event/Events.kt','package core.game.event\nclass InteractionEvent(val node:core.game.node.Node,val option:String)\nclass UseWithEvent(val used:Int,val with:Int)')
 adapter('core/game/node/Node.kt','''package core.game.node
open class Node(val tag:Int,var rawId:Int){open val id:Int get(){oracle.Model.emit("id",tag);return rawId};open val location:core.game.world.map.Location get(){oracle.Model.emit("location",tag);return core.game.world.map.Location(tag)};fun asItem()=this as core.game.node.item.Item}''')
 adapter('core/game/node/item/Item.kt','package core.game.node.item\nclass Item(tag:Int,id:Int):core.game.node.Node(tag,id)')
 adapter('core/game/node/entity/Entity.kt','package core.game.node.entity\nopen class Entity(tag:Int,id:Int):core.game.node.Node(tag,id)')
 adapter('core/game/node/entity/player/Player.kt','''package core.game.node.entity.player
import oracle.Model
class Player(tag:Int):core.game.node.entity.Entity(tag,tag){var scriptsId=0;var managerId=0;var zoneId=0;val scripts:oracle.Scripts get(){Model.emit("scripts",tag);return oracle.Scripts(scriptsId)};val locks:oracle.Locks get(){Model.emit("locks",tag);return oracle.Locks()};val pulseManager:oracle.Manager get(){Model.emit("manager",tag);return oracle.Manager(managerId)};val zoneMonitor:oracle.Zone get(){Model.emit("zone",tag);return oracle.Zone(zoneId)};fun setAttribute(k:String,v:String){Model.emit("setOption",tag,Model.units(v));Model.option=v};fun dispatch(e:core.game.event.InteractionEvent){Model.emit("dispatchOn",tag,e.node.tag,Model.units(e.option))};fun dispatch(e:core.game.event.UseWithEvent){Model.emit("dispatchUse",tag,e.used,e.with)};fun faceLocation(l:core.game.world.map.Location){Model.emit("face",tag,l.tag)}}''')
 adapter('core/game/interaction/MovementPulse.kt','''package core.game.interaction
import core.game.node.Node
import core.game.node.entity.Entity
import core.game.node.entity.player.Player
import core.game.world.map.Location
enum class DestinationFlag{ENTITY,ITEM,OBJECT}
abstract class MovementPulse(val player:Player,val target:Node,val flag:DestinationFlag?,val dest:((Entity,Node)->Location)?=null){init{oracle.Model.emit("movement",player.tag,target.tag,flag?.name,oracle.Model.functionId(dest));oracle.Model.pulses.add(this)};abstract fun pulse():Boolean}''')
 p=H/'Oracle.kt';c.read(p);kt.append(p)
 jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jsonjar=R/'runs/interfaces-oracle/json-simple-1.1.1.jar';assert hashlib.sha256(jsonjar.read_bytes()).hexdigest()==json.loads(c.read(R/'oracle/auth/json.lock.json'))['sha256']
 reflect=json.loads(c.read(R/'oracle/input-prompt/reflect.lock.json'));assert '<artifactId>kotlin-reflect</artifactId>\n      <version>'+reflect['version']+'</version>'in c.read(R/'reference/server/Server/pom.xml')
 reflectjar=O/('kotlin-reflect-'+reflect['version']+'.jar')
 if not reflectjar.exists()or reflectjar.stat().st_size!=reflect['size']or hashlib.sha256(reflectjar.read_bytes()).hexdigest()!=reflect['sha256']:
  cached=R/'runs/input-prompt-oracle'/reflectjar.name;data=cached.read_bytes()if cached.exists()else urllib.request.urlopen(reflect['url'],timeout=30).read();assert len(data)==reflect['size']and hashlib.sha256(data).hexdigest()==reflect['sha256'];reflectjar.write_bytes(data)
 cp=os.pathsep.join(map(str,[classes,std,jsonjar,reflectjar]))
 c.run(['java','-Xmx768m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',cp,'-d',classes,*kt])
 rows=requests();data=''.join(json.dumps(row,ensure_ascii=True)+'\n' for row in rows).encode();exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java') if c.JAVA_HOME else 'java'
 result=subprocess.run(list(map(str,[exe,'-Xmx768m','-Dfile.encoding=UTF-8','-cp',cp,'oracle.OracleKt'])),cwd=R,input=data,capture_output=True,timeout=180);assert result.returncode==0,result.stderr.decode(errors='replace')
 out=[json.loads(line)for line in result.stdout.splitlines()];assert len(out)==len(rows)
 fixture=''.join(json.dumps(row,sort_keys=True,separators=(',',':'))+'\n'for row in out).encode();c.read(pathlib.Path(__file__))
 initial=json.loads(c.read(H/'initial.lock.json'));assert hashlib.sha256(b''.join(fixture.splitlines(keepends=True)[:initial['caseCount']])).hexdigest()==initial['sha256']
 provenance=dict(format=1,upstream=lock,initialBaseline=initial,declaredReflectionArtifact=reflect,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(fixture).hexdigest(),scope='Complete unchanged InteractionListeners, InteractionListener, IntType and Script source. Entity/script/lock/event/zone/movement/callback bodies are mutable recording services; scheduled pulse bodies can be invoked independently. Not full ScriptProcessor/movement/world/content runtime or natural banking proof. Private maps are sorted observations; array identity text normalized only in snapshots.')
 folder=R/'fixtures/interaction-listeners';folder.mkdir(exist_ok=True)
 if check:assert(folder/'oracle.jsonl').read_bytes()==fixture and json.loads((folder/'provenance.json').read_text(encoding='utf-8'))==provenance
 else:(folder/'oracle.jsonl').write_bytes(fixture);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2)+'\n',encoding='utf-8')
 (O/'runtime.json').write_text(json.dumps(dict(java=str(exe),classpath=cp),indent=2)+'\n',encoding='utf-8')
 print(len(rows),'original interaction listener cases reproduced'if check else'original interaction listener cases generated',flush=True)
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);v=p.parse_args();c.JAVA_HOME=v.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(v.check)
