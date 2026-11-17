"""Original client skill reducer, XP lookup and stat/misc transmit branches."""
import hashlib,json,pathlib,random,struct,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/stats';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    (run/'PlayerSkillXpTable.java').write_text(source('PlayerSkillXpTable'),encoding='utf-8')
    protocol=source('Protocol');interface=source('InterfaceList');buffer=source('Buffer')
    (run/'Buffer.java').write_text('class Buffer{byte[] data;int offset;Buffer(byte[] d){data=d;}'+''.join(method(buffer,s)for s in ['public final int g1(', 'public final int g1add(', 'public final int img4(', 'public final int g2b(', 'public final int g2('])+'}',encoding='utf-8')
    update=''.join(method(protocol,'if (opcode == ServerProt.'+name+')')for name in ['UPDATE_STAT','UPDATE_RUNENERGY','UPDATE_RUNWEIGHT'])
    notify=method(interface,'if (component.onStatTransmit != null && PlayerSkillXpTable.updatedStatsWriterIndex > component.updatedStatsReaderIndex)')+method(interface,'if (miscTransmitAt > component.lastTransmitTimer && component.onMiscTransmit != null)')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class InterfaceList{static int transmitTimer=1,miscTransmitAt=0;static void redrawActiveInterfaces(){}}class PluginRepository{static void OnXPUpdate(int skill,int xp){}}class Player{static int runEnergy,weight;}class ServerProt{static int UPDATE_STAT=38,UPDATE_RUNENERGY=234,UPDATE_RUNWEIGHT=159;}
class Component{int updatedStatsReaderIndex,lastTransmitTimer=-1;Object[] onStatTransmit=new Object[]{1},onMiscTransmit=new Object[]{2};int[] statTriggers;}class HookRequest{Component source;Object[] arguments;}class Requests{int count;void addTail(Object o){count++;}}
public class Probe{static Buffer inboundBuffer;static int opcode,miscTransmitAt;static Requests lowPriorityRequests=new Requests();static boolean update(int op,byte[] data){opcode=op;inboundBuffer=new Buffer(data);'''+update+'''return false;}static void notify(Component component){HookRequest request,request2;int j,k,skill;miscTransmitAt=InterfaceList.miscTransmitAt;'''+notify+'''component.lastTransmitTimer=InterfaceList.transmitTimer++;}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;Component component=null;while((line=r.readLine())!=null){String[] x=line.split(";",-1);switch(x[0]){case "R":Arrays.fill(PlayerSkillXpTable.boostedLevels,0);Arrays.fill(PlayerSkillXpTable.baseLevels,0);Arrays.fill(PlayerSkillXpTable.experience,0);Arrays.fill(PlayerSkillXpTable.updatedStats,0);PlayerSkillXpTable.updatedStatsWriterIndex=Integer.parseInt(x[1]);InterfaceList.transmitTimer=Integer.parseInt(x[2]);InterfaceList.miscTransmitAt=0;Player.runEnergy=Player.weight=0;component=new Component();component.updatedStatsReaderIndex=Integer.parseInt(x[3]);if(!x[4].equals("null"))component.statTriggers=x[4].isEmpty()?new int[0]:Arrays.stream(x[4].split(",")).mapToInt(Integer::parseInt).toArray();continue;case "Q":lowPriorityRequests.count=0;notify(component);System.out.println(lowPriorityRequests.count);continue;case "D":System.out.println(Arrays.toString(PlayerSkillXpTable.boostedLevels)+";"+Arrays.toString(PlayerSkillXpTable.baseLevels)+";"+Arrays.toString(PlayerSkillXpTable.experience)+";"+Arrays.toString(PlayerSkillXpTable.updatedStats)+";"+PlayerSkillXpTable.updatedStatsWriterIndex+","+component.updatedStatsReaderIndex+","+Player.runEnergy+","+Player.weight+","+InterfaceList.miscTransmitAt+","+InterfaceList.transmitTimer+","+component.lastTransmitTimer);continue;case "L":System.out.println(Arrays.toString(PlayerSkillXpTable.xpLevelLookup));continue;default:update(Integer.parseInt(x[0]),HexFormat.of().parseHex(x[1]));}}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];inputs=['L'];thresholds=[];total=0
    for level in range(1,100):total+=int(2**(level/7)*300+level);thresholds.append(total//4)
    for _ in range(96):
        writer=rng.choice([0,0,2147483640,-2147483648]);reader=rng.choice([0,writer]);timer=rng.choice([1,2147483640,-2147483648]);triggers=rng.choice([None,[],[0,3,5],[24],[rng.randrange(25)]]);steps=[];inputs.append(';'.join(map(str,['R',writer,timer,reader,'null'if triggers is None else','.join(map(str,triggers))])))
        for _ in range(rng.randrange(80,150)):
            if rng.randrange(5)==0:steps.append(None);inputs.append('Q');continue
            op=rng.choice([38,38,38,234,159])
            if op==38:
                boosted=rng.randrange(256);xp=rng.choice([-2147483648,-1,0,2147483647,rng.randrange(30000000),rng.choice(thresholds)+rng.choice([-1,0,1])]);skill=rng.randrange(25);data=bytes([(boosted+128)&255])+struct.pack('>i',xp)[2:]+struct.pack('>i',xp)[:2]+bytes([skill])
            elif op==234:data=bytes([rng.randrange(256)])
            else:data=struct.pack('>h',rng.randrange(-32768,32768))
            steps.append(dict(op=op,data=data.hex()));inputs.append(str(op)+';'+data.hex())
        cases.append(dict(writer=writer,reader=reader,timer=timer,triggers=triggers,steps=steps));inputs.append('D')
    result=iter(subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines());levels=json.loads(next(result))
    for c in cases:
        c['hooks']=[int(next(result))for step in c['steps']if step is None];fields=next(result).split(';');c['expected']=[json.loads(s)for s in fields[:4]]+[list(map(int,fields[4].split(',')))]
    assert next(result,None)is None
    p=root/'fixtures/ui/stats.json';content=json.dumps(dict(sources=hashes,bindings='Controlled packet inputs and transmit clock; redraw and optional plugin XP callbacks are explicit no-op adapters; other event-source ordering and server skill rules are excluded',thresholds=levels,cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI skill state corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 client skill/misc updates: {len(cases)} scenarios')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
