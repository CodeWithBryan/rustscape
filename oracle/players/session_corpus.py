"""Original LoginManager success-header/rebuild gate with explicit socket/UI bindings."""
import hashlib,json,random,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/session';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    login=source('LoginManager');buffer=source('Buffer');packet=source('Packet')
    blocks=method(login,'if (step == 8)')+method(login,'if (step == 9)')
    code=r'''import java.io.*;import java.util.*;
class Socket{byte[]data;int offset,limit;Socket(byte[]b){data=b;}int available(){return limit-offset;}void read(int start,int n,byte[]dest){System.arraycopy(data,offset,dest,start,n);offset+=n;}}
class Buffer{byte[]data=new byte[65536];int offset; BUFFER }
class RandomStub{int getNextKey(){return 0;}}class GlobalConfig{static boolean USE_ISAAC=false;}
class Packet extends Buffer{RandomStub isaac=new RandomStub(); PACKET }
class Protocol{static Socket socket;static Packet inboundBuffer=new Packet();static int opcode,length;static String rebuild="";static void readRebuildPacket(boolean ignored){rebuild=HexFormat.of().formatHex(Arrays.copyOf(inboundBuffer.data,length));}}
class PlayerList{static int selfId;}class MouseRecorder{static boolean enabled;}class LocTypeList{static void setAllowMembers(boolean b){}}class ObjTypeList{static void setAllowMembers(boolean b){}}
class SceneGraph{static int centralZoneX;}class GameShell{static class Link{Object applet;}static Link signLink=new Link();}class ZAP{static void browserControlCall(Object a){}}class UNZAP{static void browserControlCall(Object a){}}
class client{static boolean advertSuppressed=true;static int resets;static void resetGameSessionState(){resets++;}}
public class Probe{
static int step,reply,staffModLevel,blackmarks;static boolean playerUnderage,parentalChatConsent,parentalAdvertConsent,mapQuickChat,playerMember,mapMembers;
static void loop(){ BLOCKS }
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]parts=line.split(";");byte[]wire=HexFormat.of().parseHex(parts[1]);Protocol.socket=new Socket(wire);Protocol.inboundBuffer=new Packet();Protocol.rebuild="";step=8;reply=-3;client.resets=0;List<String>states=new ArrayList<>();for(String n:parts[0].split(",")){Protocol.socket.limit=Integer.parseInt(n);if(step!=0)loop();states.add("["+step+","+reply+","+client.resets+"]");}System.out.println("{\"states\":"+states+",\"fields\":["+staffModLevel+","+blackmarks+","+playerUnderage+","+parentalChatConsent+","+parentalAdvertConsent+","+mapQuickChat+","+MouseRecorder.enabled+","+PlayerList.selfId+","+playerMember+","+mapMembers+"],\"rebuild\":\""+Protocol.rebuild+"\"}");}}
}'''.replace('BUFFER',method(buffer,'public final int g1(')+method(buffer,'public final int g2(')).replace('PACKET',method(packet,'public final int g1isaac(')).replace('BLOCKS',blocks)
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(162530);cases=[]
    for i in range(256):
        fields=bytes(rng.randrange(256) for _ in range(11));payload=bytes(rng.randrange(256)for _ in range(rng.randrange(0,512)))
        wire=fields+bytes([162])+len(payload).to_bytes(2,'big')+payload
        stops=sorted(set([0,1,10,11,12,13,14,max(14,len(wire)-1),len(wire)]))
        cases.append(dict(wire=wire.hex(),stops=stops))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(','.join(map(str,c['stops']))+';'+c['wire']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines()
    assert len(result)==len(cases)
    for c,line in zip(cases,result):c['expected']=json.loads(line)
    path=root/'fixtures/players/session.json';content=json.dumps(dict(sources=hashes,bindings='Unchanged LoginManager step8/9 and Buffer/Packet reads. Binary in-memory socket, ISAAC disabled as pinned, member-archive/browser-ad/reset/rebuild callbacks explicit. This covers the success header and first complete payload gate, not full LoginManager retries or world initialization.',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Client success-header corpus differs')
    else:path.write_bytes(content)
    print(f'Original client login success/rebuild gate: {len(cases)} fragmented streams')
