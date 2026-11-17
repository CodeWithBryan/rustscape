"""Compile unchanged NPCSpawner.kt, checking full pinned data and partial failures."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/npc-spawns-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
    cases=[dict(pinned=True)]
    def add(text,**kw):cases.append(dict(text=text,**kw))
    def data(loc='3207,3216,0,1,6',id=47):return json.dumps([dict(npc_id=id,loc_data='{'+loc+'}')])
    for direction in range(-1,10):add(data(f'3207,3216,0,1,{direction}'),booths=[47])
    for flag in ['0','1','2','true','01','+1','\t1\n','\u00a01\u00a0']:add(data('3207,3216,0,'+flag+',0'))
    for loc in ['','-','--','{3207,3216,0,1,6}-{}','3207,3216,0,1,6-3208,3216,0,0,7','3207,3216,0,1,6,ignored','-1,3216,0,1,0','3207,3216,-1,1,0','16384,65535,5,1,0','3207,3216,0','3207,3216,0,1,','3207,3216,0,,0','\u20003207,3216,0,1,0','\t3207\r, 3216,0,1,0','3207,3216,0,1,2147483648','{32{07},3216,0,1,0}']:add(data(loc))
    for id in [None,1,1.0,True,'+47','٤٧',' 47','2147483648',{},[]]:add(data(id=id))
    for text in ['null','{}','[null]','[3]','[{}]','[{"npc_id":47}]','[{"npc_id":47,"loc_data":null}]','[,{"npc_id":47 "loc_data":"3207,3216,0,1,6"},]','[{"npc_id":47,"loc_data":""}]','[']:add(text)
    text=json.dumps([dict(npc_id=47,loc_data='3207,3216,0,1,6-3208,3216,0,0,7'),dict(npc_id=1,loc_data='3209,3216,0,1,2')])
    for phase in ['create','walks','direction','attribute','booth','init','log']:
        for at in [1,2,3]:add(text,booths=[47],failures={phase:at})
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'));ref=ROOT/'reference/server'
    if run(['git','-C',ref,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
    run(['git','-C',ref,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    folder=ROOT/'fixtures/npc-spawns';folder.mkdir(parents=True,exist_ok=True);config=folder/'npc_spawns.json'
    raw=run(['git','-C',ref,'show',lock['server']['commit']+':Server/data/configs/npc_spawns.json']).stdout
    if check:
        if config.read_bytes()!=raw:raise RuntimeError('Pinned NPC spawn data differs')
    else:config.write_bytes(raw)
    java=[];kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
    for name in ['core/game/system/config/NPCSpawner.kt','core/game/world/map/Direction.java']:
        p=SERVER/name;read(p);(kt if p.suffix=='.kt'else java).append(p)
    adapter('core/ServerConstants.java','package core;public class ServerConstants{public static String CONFIG_PATH;}')
    adapter('core/tools/SystemLogger.java','package core.tools;public class SystemLogger{}')
    adapter('core/tools/Log.java','package core.tools;public enum Log{FINE;}')
    adapter('core/api/ContentAPI.kt','package core.api\nfun log(c:Class<*>,l:core.tools.Log,s:String){oracle.Support.emit("log",s)}')
    adapter('core/game/node/entity/npc/NPC.java','''package core.game.node.entity.npc;import core.game.world.map.*;import java.util.*;public class NPC{public int id,slot;public Location location;public boolean walks,initialized;private Direction direction=Direction.SOUTH;public Map<String,Object>attributes=new TreeMap<>();public NPC definition=this;public static NPC create(int id,Location l){NPC n=new NPC();n.id=id;n.location=l;n.slot=oracle.Support.npcs.size();oracle.Support.npcs.add(n);oracle.Support.emit("create",n.slot,id,oracle.Support.coord(l));return n;}public void setWalks(boolean b){walks=b;oracle.Support.emit("walks",slot,b);}public boolean isWalks(){return walks;}public void setDirection(Direction d){direction=d;oracle.Support.emit("direction",slot,d.ordinal());}public Direction getDirection(){return direction;}public void setAttribute(String k,Object v){attributes.put(k,v);oracle.Support.emit("attribute",slot,k,v);}public NPC getDefinition(){return this;}public boolean getConfiguration(String k,boolean fallback){return oracle.Support.booth(this);}public void init(){initialized=true;oracle.Support.emit("init",slot);}}''')
    adapter('core/game/world/map/path/ClipMaskSupplier.java','package core.game.world.map.path;public interface ClipMaskSupplier{int getClippingFlag(int z,int x,int y);}')
    adapter('core/game/world/map/Point.java','package core.game.world.map;public class Point{public Point(int x,int y){}}')
    adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static int getClippingFlag(int z,int x,int y){throw new UnsupportedOperationException();}}')
    location=read(SERVER/'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt','package core.game.world.map\nclass Location(x:Int,y:Int,z:Int=0){val hash=LocationHash(x,y,(z%4+4)%4);val x:Int get()=hash.x;val y:Int get()=hash.y;val z:Int get()=hash.z;companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)};@JvmInline '+c.method(location,'value class LocationHash(')+'}')
    read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();p=RUN/'input.jsonl';p.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with p.open('rb')as f:result=subprocess.run(list(map(str,[exe,'-Xmx1g','-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support',RUN/'config',config])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    rows=[json.loads(l)for l in result.stdout.splitlines()if l];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),configSha256=hashlib.sha256(raw).hexdigest(),pinnedConfigRows=len(json.loads(raw)),pinnedNpcCount=len(rows[0]['expected']['npcs']),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged NPCSpawner.load and Direction, original packed Location. Complete pinned npc_spawns.json plus grammar/coercion/trim/ordinal/failure cases. NPC factory/definition/init, attributes and logging are explicit observable services. Does not claim NPC subclasses/content, complete initialization or world repository integration.')
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('NPC-spawns fixtures/provenance differ')
        print(f'{len(rows)} original NPC-spawn scenarios reproduced, {provenance["pinnedNpcCount"]} pinned spawns.')
    else:(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original NPC-spawn scenarios, {provenance["pinnedNpcCount"]} pinned spawns.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
