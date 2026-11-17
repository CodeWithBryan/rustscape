"""Verbatim original major/entity tick bodies with observable phase services."""
import argparse,hashlib,importlib.util,json,os,pathlib,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/tick-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run
def requests():
    cases=[]
    phases=['clock','packets','packetTime','disconnects','pulses','listeners','start','run','end','pulseWorld','managers','flush','pre','tickEvent','skills','walking','post','timers','masks']
    def add(actions=None,**kw):cases.append(dict(actions=actions or [dict(op='handle'),dict(op='handle',skip=True)],**kw))
    for entity in [False,True]:
        for location in [None,[0,0,0],[3207,3216,0],[3207,3216,3]]:
            for destination in [None,[0,0,0],[3207,3216,0],[3208,3216,3]]:
                add(doEntity=entity,location=location,walkTo=destination)
    for phase in phases:
        for at in [1,2]:add(doEntity=True,failures={phase:at})
    for failures in [dict(packets=1,exception=1),dict(packets=1,flush=1),dict(packets=1,exception=1,flush=1),dict(flush=1,exception=1),dict(packets=1,exception=1,flush=1,**{})]:add(doEntity=True,failures=failures)
    for clock in [[0,1],[9223372036854775807,-9223372036854775808],[-9223372036854775808,9223372036854775807],[0,2147483648]]:add(doEntity=True,clock=clock)
    for ticks in [-2147483648,-1,0,2147483647]:add(doEntity=True,ticks=ticks)
    for phase in phases:
        for action in [dict(op='set',location=None),dict(op='set',location=[3220,3221,2]),dict(op='mutate',location=[3220,3221,2])]:add(doEntity=True,hooks=[dict(event=phase,actions=[action])])
    add([dict(op='entity'),dict(op='entity')],location=[3207,3216,0],walkTo=[3208,3216,0])
    return cases
def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'));p=ROOT/'reference/server'
    if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock['server']['commit']:raise RuntimeError('Reference pin differs')
    run(['git','-C',p,'diff','--quiet','HEAD','--']);RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
    if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
        if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        jar.write_bytes(data)
    java=[];kt=[]
    def adapter(name,text):
        p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');(kt if p.suffix=='.kt'else java).append(p)
    worker=read(SERVER/'core/worker/MajorUpdateWorker.kt')
    adapter('core/worker/MajorUpdateWorker.kt','''package core.worker
import core.game.world.GameWorld
import core.game.world.repository.Repository
import core.net.packet.PacketProcessor
import core.net.packet.PacketWriteQueue
import core.plugin.Managers
import core.integrations.grafana.Grafana
object System{@JvmStatic fun currentTimeMillis()=oracle.Support.clock()}
class MajorUpdateWorker {val sequence=oracle.Support.Sequence()
'''+c.method(worker,'fun handleTickActions(skipPulseUpdate: Boolean = false)')+'\n}')
    entity=read(SERVER/'core/game/node/entity/Entity.java')
    adapter('core/game/node/entity/Entity.java','''package core.game.node.entity;import core.game.world.GameWorld;import core.game.world.map.Location;import java.util.Objects;
public class Entity{public Location location;public Scripts scripts=new Scripts();public Skills skills=new Skills();public Walking walkingQueue=new Walking();public Timers timers=new Timers();public Masks updateMasks=new Masks();public void dispatch(TickEvent e){oracle.Support.emit("tickEvent",e.tick);}public static class TickEvent{int tick;TickEvent(int t){tick=t;}}public static class Scripts{public void preMovement(){oracle.Support.emit("pre");}public void postMovement(boolean moved){oracle.Support.emit("post",moved);}}public static class Skills{public void pulse(){oracle.Support.emit("skills");}}public static class Walking{public void update(){oracle.Support.walk();}}public static class Timers{public void processTimers(){oracle.Support.emit("timers");}}public static class Masks{public void prepare(Entity e){oracle.Support.emit("masks");}}
'''+c.method(entity,'public void tick()')+'}')
    location=read(SERVER/'core/game/world/map/Location.kt')
    adapter('core/game/world/map/Location.kt','''package core.game.world.map
class Location(x:Int,y:Int,z:Int=0){var hash=LocationHash(x,y,(z%4+4)%4);var x:Int get()=hash.x;set(v){hash=LocationHash(v,y,z)};var y:Int get()=hash.y;set(v){hash=LocationHash(x,v,z)};var z:Int get()=hash.z;set(v){hash=LocationHash(x,y,(v%4+4)%4)};override fun equals(o:Any?)=o is Location&&hash==o.hash;override fun hashCode()=hash.hash;
'''+next(line.strip()for line in location.splitlines()if'fun transform(diffX: Int, diffY: Int, z: Int)'in line)+'''
companion object{@JvmStatic fun create(x:Int,y:Int,z:Int)=Location(x,y,z)};@JvmInline '''+c.method(location,'value class LocationHash(')+'}')
    adapter('core/game/world/GameWorld.java','''package core.game.world;import java.util.*;public class GameWorld{public static oracle.Support.Phase Pulser=new oracle.Support.Phase("pulses");public static List<oracle.Support.Phase>tickListeners=Arrays.asList(new oracle.Support.Phase("listeners"));public static int getTicks(){return oracle.Support.ticks;}public static void pulse(){oracle.Support.emit("pulseWorld");oracle.Support.ticks++;}}''')
    adapter('core/game/world/repository/Repository.java','package core.game.world.repository;public class Repository{public static oracle.Support.Phase disconnectionQueue=new oracle.Support.Phase("disconnects");}')
    adapter('core/net/packet/PacketProcessor.java','package core.net.packet;public class PacketProcessor{public static void processQueue(){oracle.Support.emit("packets");}}')
    adapter('core/net/packet/PacketWriteQueue.java','package core.net.packet;public class PacketWriteQueue{public static void flush(){oracle.Support.emit("flush");}}')
    adapter('core/plugin/Managers.java','package core.plugin;public class Managers{public static void tick(){oracle.Support.emit("managers");}}')
    adapter('core/integrations/grafana/Grafana.kt','''package core.integrations.grafana
object Grafana{var packetProcessTime=0;set(v){oracle.Support.packetTime=v;field=v;oracle.Support.emit("packetTime",v)}}''')
    read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));compile_cp=os.pathsep.join(map(str,[std,jar]))
    run(['java','-Xmx512m','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',compile_cp,'-d',classes,*kt,*java]);run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
    cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
    with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=60)
    if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
    rows=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(rows)==len(cases);data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in rows).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(rows),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Verbatim MajorUpdateWorker.handleTickActions and Entity.tick bodies. Explicit packet/disconnect/pulse/listener/sequence/manager/flush/clock/telemetry and script/event/skill/walking/timer/mask services; sequence start can invoke the real original Entity.tick. Compares phase order, skip flag, location snapshots, moved predicate, wrapping clocks/ticks, catch/finally and exception override paths. Full UpdateSequence/repository lists, concrete services, Player/NPC extensions, native component composition and live world scheduling remain separate.')
    folder=ROOT/'fixtures/tick'
    if check:
        if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Tick fixtures/provenance differ')
        print(f'{len(rows)} original major/entity tick scenarios reproduced.')
    else:folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(rows)} original major/entity tick scenarios.')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
