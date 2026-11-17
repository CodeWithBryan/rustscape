"""Compile pinned player/communication classes unmodified, with explicit world/I/O adapters."""
import argparse, hashlib, importlib.util, json, os, pathlib, subprocess, urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2]
HERE=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('login_compiler',ROOT/'oracle/login/generate.py')
compiler=importlib.util.module_from_spec(spec);spec.loader.exec_module(compiler)
read=compiler.read;run=compiler.run
SERVER=ROOT/'reference/server/Server/src/main'
RUN=ROOT/'runs/player-oracle'

# Only metadata access is supported. World, packets, management and missing
# StringUtils methods deliberately throw instead of silently emulating gameplay.
ADAPTERS={
'MetadataSupport.kt':'''import core.game.node.entity.player.info.*
import core.auth.UserAccountInfo
data class MetadataInfo(val username:String,val displayMode:Int,val windowMode:Int,val screenWidth:Int,val screenHeight:Int,val isaacPair:List<Int>,val opcode:Int)
class MetadataSession {var clientInfo:ClientInfo?=null;var isaacPair:List<Int>?=null;var associatedUsername:String?=null;var details:PlayerDetails?=null;var response:Int?=null;var opcode:Int?=null}
object MetadataLogin {@JvmStatic fun proceedWith(session:MetadataSession,details:PlayerDetails,opcode:Int){session.details=details;session.opcode=opcode;session.response=8}}
''',
'core/auth/Clock.kt':'package core.auth\nobject System {@JvmStatic fun currentTimeMillis():Long = oracle.Clock.now()}',
'oracle/Clock.java':'package oracle; public final class Clock {public static long value;public static int reads;public static long now(){reads++;return value;}}',
'core/game/node/entity/player/info/System.java':'package core.game.node.entity.player.info; public final class System {public static long currentTimeMillis(){return oracle.Clock.now();}}',
'core/net/IoSession.java':'package core.net; public class IoSession {public String address;public IoSession(String address){this.address=address;}public String getAddress(){return address;}}',
'core/tools/Log.java':'package core.tools; public enum Log {ERR}',
'core/tools/StringUtils.java':'package core.tools; public class StringUtils {public static String formatDisplayName(String s){throw new UnsupportedOperationException();}public static String[] splitIntoLine(String s,int n){throw new UnsupportedOperationException();}}',
'core/api/ContentAPIKt.java':'package core.api; public class ContentAPIKt {public static void log(Class<?> c,core.tools.Log l,String s){throw new UnsupportedOperationException();}public static void setVarp(core.game.node.entity.player.Player p,int id,int v){throw new UnsupportedOperationException();}}',
'core/game/system/task/Pulse.java':'package core.game.system.task; public abstract class Pulse {public Pulse(int ticks,core.game.node.entity.player.Player... p){throw new UnsupportedOperationException();}public abstract boolean pulse();public void stop(){throw new UnsupportedOperationException();}}',
'core/game/system/communication/ClanRepository.java':'package core.game.system.communication; public class ClanRepository {public String getOwner(){throw new UnsupportedOperationException();}public static ClanRepository get(String s,boolean... b){throw new UnsupportedOperationException();}public void rank(String s,ClanRank r){throw new UnsupportedOperationException();}}',
'core/game/node/entity/player/Player.java':'''package core.game.node.entity.player;
public class Player {
 public core.game.node.entity.player.info.PlayerDetails getDetails(){throw new UnsupportedOperationException();}
 public String getName(){throw new UnsupportedOperationException();}
 public core.game.system.communication.CommunicationInfo getCommunication(){return getDetails().getCommunication();}
 public boolean isAdmin(){throw new UnsupportedOperationException();}
 public void sendMessage(String s){throw new UnsupportedOperationException();}public void sendMessages(String[] s){throw new UnsupportedOperationException();}
 public <T>T getAttribute(String s,T value){throw new UnsupportedOperationException();}
 public Player getPacketDispatch(){throw new UnsupportedOperationException();}
 public Settings getSettings(){throw new UnsupportedOperationException();}
 public static class Settings {public int getPrivateChatSetting(){throw new UnsupportedOperationException();}}
}''',
'core/game/world/repository/Repository.java':'package core.game.world.repository;public class Repository {public static core.game.node.entity.player.Player getPlayerByName(String s){throw new UnsupportedOperationException();}}',
'core/net/amsc/WorldCommunicator.java':'package core.net.amsc;public class WorldCommunicator {public static boolean isEnabled(){throw new UnsupportedOperationException();}}',
'core/net/amsc/MSPacketRepository.java':'package core.net.amsc;public class MSPacketRepository {public static void sendContactUpdate(String a,String b,boolean c,boolean d,Object e){throw new UnsupportedOperationException();}}',
'core/net/packet/PacketRepository.java':'package core.net.packet;public class PacketRepository {public static void send(Class<?> c,Object o){throw new UnsupportedOperationException();}}',
'core/net/packet/context/ContactContext.java':'package core.net.packet.context;public class ContactContext {public ContactContext(core.game.node.entity.player.Player p,String s,int i){throw new UnsupportedOperationException();}}',
'core/net/packet/out/ContactPackets.java':'package core.net.packet.out;public class ContactPackets {}',
'core/worker/ManagementEvents.java':'package core.worker;public class ManagementEvents {public static void publish(Object o){throw new UnsupportedOperationException();}}',
'proto/management/PrivateMessage.java':'package proto.management;public class PrivateMessage {public static Builder newBuilder(){throw new UnsupportedOperationException();}public static class Builder {public void setSender(String s){}public void setRank(int n){}public void setMessage(String s){}public void setReceiver(String s){}public PrivateMessage build(){throw new UnsupportedOperationException();}}}',
'core/game/world/GameWorld.java':'''package core.game.world;public class GameWorld {
 public static core.storage.AccountStorageProvider storage;
 public static core.storage.AccountStorageProvider getAccountStorage(){return storage;}
 public static Settings getSettings(){throw new UnsupportedOperationException();}public static Pulser getPulser(){throw new UnsupportedOperationException();}
 public static class Settings {public boolean isDevMode(){throw new UnsupportedOperationException();}public int getWorldId(){throw new UnsupportedOperationException();}}
 public static class Pulser {public void submit(core.game.system.task.Pulse p){throw new UnsupportedOperationException();}}
}''',
'core/game/system/mysql/SQLTable.java':'''package core.game.system.mysql;
import java.util.*;
public class SQLTable {
 public Map<String,String> values=new LinkedHashMap<>();public List<Object> trace=new ArrayList<>();public String fail="";
 public Column getColumn(String name){return new Column(name);}
 public class Column {String name;Column(String name){this.name=name;}
  public Object getValue(){trace.add(Arrays.asList("read",name));if(fail.equals("read:"+name))throw new RuntimeException();return values.get(name);}
  public void updateValue(Object value){trace.add(Arrays.asList("write",name,value));if(fail.equals("write:"+name))throw new RuntimeException();values.put(name,(String)value);}
 }
}'''
}

def requests():
    cases=[]
    for text in ['',',',',,,','1','1,2','1,0,8,9','1,0,0,9','9,9,9,9','-1,10,8,9','1,0,8,9,5','1,0,8,9,bad',' 1,0,8,9','+1,-0,3,9','١,０,８,９','2147483648','1,,8,9','1,0,8,9,,,']:
        cases.append(dict(kind='requirements',text=text))
    for ordinal in range(-2,12):
        for i in range(4):
            tokens=['1','0','8','9'];tokens[i]=str(ordinal);cases.append(dict(kind='requirements',text=','.join(tokens)))
    names=['alice','bob','Aa','BB','AaAa','AaBB','BBAa','BBBB','é','Ω','😀','']
    contacts=['','~',',','{},','{alice,1}','{alice,1}~{alice,8}','{{al}ic{e},+3,ignored}','{,1}','{alice,1,}','{alice,}','alice','{alice,bad}','{alice,-1}','{alice,10}','{alice,１}','{alice,1}~~{bob,2}','{alice,1}~']
    contacts+=['~'.join('{%s,%s}'%(name,i%10) for i,name in enumerate(names)), '~'.join('{user%s,%s}'%(i,i%10) for i in range(260))]
    colliders=['']
    for _ in range(6):colliders=[prefix+suffix for prefix in colliders for suffix in ['Aa','BB']]
    contacts+=['~'.join('{%s,%s}'%(name,i%10) for i,name in enumerate(colliders)), '~'.join('{%s,%s}'%(name,i%10) for i,name in enumerate(colliders+['user'+str(i) for i in range(160)]+list(reversed(colliders))))]
    for text in contacts:cases.append(dict(kind='contacts',text=text))
    base=dict(contacts='{old,8}',blocked='first,,second,',clanName='old clan',currentClan='old owner',clanReqs='1,0,8,9')
    for reqs in ['1,0,8,9','1,2,0,4','1,2,bad,4','-1,10,8,9',',','','1,2']:
        for text in ['', '{alice,1}~{bob,2}', '{alice,1}~{bob,bad}']:
            row=dict(base,contacts=text,blocked='',clanName='new clan',currentClan='new owner',clanReqs=reqs)
            cases.append(dict(kind='communication',actions=[dict(op='account',**base),dict(op='account',**row),dict(op='account',**row),dict(op='save_columns')]))
            cases.append(dict(kind='communication',actions=[dict(op='account',**base),dict(op='columns',values=row),dict(op='save_columns')]))
    for interned in [False,True]:
        for reqs in [None,'','1,2,0,9','1,2,bad,9','1,2,3,9,broken']:
            cases.append(dict(kind='communication',actions=[dict(op='columns',values=dict(base,clanReqs=reqs),interned=interned),dict(op='save_columns')]))
    for fail in ['read:contacts','read:blocked','read:clanName','read:currentClan','read:clanReqs','write:blocked','write:contacts','write:clanName','write:currentClan','write:clanReqs']:
        cases.append(dict(kind='communication',actions=[dict(op='account',**base),dict(op='columns',values=base,fail=fail),dict(op='save_columns',fail=fail)]))
    cases.append(dict(kind='communication',actions=[dict(op='put',key='key',username='value',rank=3,world=-2147483648),dict(op='block',value=None),dict(op='block',value=''),dict(op='save_columns'),dict(op='rank',key='key',value=None),dict(op='save_columns')]))
    for slot in range(4):
        cases.append(dict(kind='communication',actions=[dict(op='account',**base),dict(op='requirement',slot=slot,value=None),dict(op='save_columns')]))
    for mode in [-2147483648,-1,0,1,2,3,2147483647]:
        for window in [-1,0,1,2,3]:cases.append(dict(kind='client',display=mode,window=window,width=-1,height=2147483647))
    for ordinal in [-2147483648,-1,0,1,2,3,2147483647]:cases.append(dict(kind='rights',ordinal=ordinal))
    cases.append(dict(kind='ranks'))
    for mac in [None,'','01:ab:CD','01-ab-CD','::::','Ω:😀']:
        cases.append(dict(kind='uid',actions=[dict(op='new',ip='127.0.0.1',computer='host',mac='a:b',serial='secret'),dict(op='translate',empty=mac is None,ip='new',computer=None,mac=mac,serial=None)]))
    cases.append(dict(kind='uid',actions=[dict(op='new',ip=None,computer=None,mac=None,serial=None)]))
    for now in [-2**63,-1,0,1000,2**63-1]:
        for delta in [-1,0,1,86400000*1001,86400000*1001-1]:
            until=(now+delta+2**63)%2**64-2**63
            cases.append(dict(kind='details',now=now,actions=[dict(op='set',field=7,value=until),dict(op='set',field=8,value=until),dict(op='flags')]))
    for parsed in [False,True]:
        for ban in [999,1000,1001]:
            for fail in ['', 'illegal_state','storage']:
                cases.append(dict(kind='details',now=1000,actions=[dict(op='parse'),dict(op='set',field=8,value=ban),dict(op='parsed',value=parsed),dict(op='save',fail=fail)]))
    for field,value in [(1,'password'),(2,-2147483648),(3,-1),(3,0),(3,1),(3,2),(3,3),(4,2147483647),(14,-2**63),(15,2**63-1)]:
        cases.append(dict(kind='details',now=1000,actions=[dict(op='set',field=field,value=value),dict(op='flags')]))
    for slot in range(4):cases.append(dict(kind='details',now=1000,actions=[dict(op='parse'),dict(op='parsed',value=True),dict(op='requirement',slot=slot,value=None),dict(op='save')]))
    cases.append(dict(kind='details',now=1000,actions=[dict(op='parse'),dict(op='parsed',value=True),dict(op='clan',value=None),dict(op='save')]))
    cases.append(dict(kind='details',now=1000,actions=[dict(op='uid',ip='uid ip',computer='computer',mac='a:b',serial='serial'),dict(op='session',value='session ip'),dict(op='session',value=None),dict(op='clear_session')]))
    for reqs in ['1,0,8,9','1,2,bad,9','-1,10,8,9','1,2','']:
        for text in ['', '{alice,1}~{bob,2}', '{alice,1}~{bob,bad}']:
            cases.append(dict(kind='metadata',now=1000,username='packet name',values=dict(base,contacts=text,clanReqs=reqs)))
    return cases

def generate(check):
    lock=json.loads(read(ROOT/'upstream.lock.json'))
    for part in ['client','server']:
        path=ROOT/'reference'/part
        if run(['git','-C',path,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
        run(['git','-C',path,'diff','--quiet','HEAD','--'])
    jars=compiler.artifacts();RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True)
    artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'));json_jar=RUN/'json-simple-1.1.1.jar'
    if not json_jar.exists() or hashlib.sha256(json_jar.read_bytes()).hexdigest()!=artifact['sha256']:
        with urllib.request.urlopen(artifact['url'],timeout=60) as response:data=response.read()
        if len(data)!=artifact['size'] or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
        json_jar.write_bytes(data)
    paths={}
    for name,text in ADAPTERS.items():
        path=RUN/'adapters'/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_text(text,encoding='utf-8');paths[name]=path
    run(['javac','--release','11','-d',classes,paths['oracle/Clock.java']])
    stdlib=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));annotations=next(p for p in jars if p.name.startswith('annotations-'))
    classpath=os.pathsep.join(map(str,[classes,stdlib,annotations,json_jar]))
    kotlin=[SERVER/'core/auth/UserAccountInfo.kt',SERVER/'core/storage/AccountStorageProvider.kt',paths['core/auth/Clock.kt']]
    for path in kotlin[:2]:read(path)
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,*kotlin])
    java=[path for name,path in paths.items() if name.endswith('.java') and name!='oracle/Clock.java']
    for name in ['CommunicationInfo','Contact','ClanRank']:
        path=SERVER/'core/game/system/communication'/(name+'.java');read(path);java.append(path)
    for name in ['PlayerDetails','UIDInfo','Rights','ClientInfo']:
        path=SERVER/'core/game/node/entity/player/info'/(name+'.java');read(path);java.append(path)
    run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,*java])
    event=read(SERVER/'core/net/event/LoginReadEvent.kt')
    suffix=event[event.index('val details = PlayerDetails(info.username)'):event.index('\n        } catch (e: Exception)')]
    gate=RUN/'MetadataGate.kt'
    gate.write_text('import core.game.node.entity.player.info.*\nimport core.auth.UserAccountInfo\nimport MetadataLogin as Login\nobject MetadataGate {@JvmStatic fun prepare(session:MetadataSession,info:MetadataInfo,accountInfo:UserAccountInfo){try{\n'+suffix+'\n}catch(e:Exception){session.response=0}}}',encoding='utf-8')
    run(['java','-cp',os.pathsep.join(map(str,jars)),'org.jetbrains.kotlin.cli.jvm.K2JVMCompiler','-no-stdlib','-no-reflect','-jvm-target','11','-classpath',classpath,'-d',classes,paths['MetadataSupport.kt'],gate])
    read(HERE/'PlayerOracle.java')
    run(['javac','--release','11','-encoding','UTF-8','-cp',classpath,'-d',classes,HERE/'PlayerOracle.java'])
    command=['java','-Dfile.encoding=UTF-8','-cp',classpath,'PlayerOracle']
    if compiler.JAVA_HOME:command[0]=str(compiler.JAVA_HOME/'bin'/('java.exe' if os.name=='nt' else 'java'))
    result=subprocess.run(command,cwd=ROOT,input=''.join(json.dumps(case,ensure_ascii=True)+'\n' for case in requests()).encode(),capture_output=True)
    if result.returncode:raise RuntimeError(result.stderr.decode(errors='replace'))
    output=[json.loads(line) for line in result.stdout.splitlines() if line]
    if len(output)!=len(requests()):raise RuntimeError('Missing player oracle cases')
    data=''.join(json.dumps(case,ensure_ascii=True,sort_keys=True,separators=(',',':'))+'\n' for case in output).encode()
    read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
    provenance=dict(format=1,upstream=lock,sources=dict(sorted(compiler.SOURCES.items())),caseCount=len(output),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Unmodified complete original player metadata and communication classes, exercised for account/legacy SQLTable data parsing, serialization, partial mutations, detail save/delegates, UID data, client modes and rank enum data, plus verbatim LoginReadEvent metadata suffix. Explicit clocks, column/storage/session and throwing world adapters. Login.proceedWith is a recording boundary returning offline. Social actions, world entry, clan repositories, lootshare pulses, Rights player attributes and UID debug output remain unsupported.')
    folder=ROOT/'fixtures/player'
    if check:
        if (folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text())!=provenance:raise RuntimeError('Player oracle/provenance differs; review before regeneration')
        print(f'{len(output)} original player metadata cases reproduced.')
    else:
        folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,indent=2,sort_keys=True)+'\n',encoding='utf-8');print(f'Generated {len(output)} original player metadata cases.')
if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');parser.add_argument('--java-home',type=pathlib.Path)
    args=parser.parse_args();compiler.JAVA_HOME=args.java_home or (pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME']) if 'RS09_ORACLE_JAVA_HOME' in os.environ else None);generate(args.check)
