"""Original server widget writers, RT4 readers and delayed model-state application."""
import argparse,hashlib,importlib.util,json,pathlib,random,re,struct,subprocess
def _recorded(root,name):
    # The packs the JVM fixtures were recorded from: kept under recorded/ once
    # the default packs are rebuilt natively (crates/client/tests/support/packs.rs).
    kept=root/'assets/visual/recorded'/name
    return kept if kept.is_file() else root/'assets/visual'/name
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/ui-model-updates-oracle';FIX=ROOT/'fixtures/ui/model-updates.json'
def module(name,path):
    spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
v=module('v',ROOT/'oracle/visual/generate.py');component_fields=module('cf',ROOT/'tools/port_component_fields.py');item_fields=module('it',ROOT/'tools/port_client_items.py')
FIELDS=['modelType','modelId','modelFrameCycle','modelSeqId','seqFrame','seqNextFrame','seqCycle','modelXAngle','modelYAngle','modelZoom','modelXOffset','modelYOffset','modelZOffset','objId','objCount','x','baseX','y','baseY','xMode','yMode']

def generate(check=False):
    RUN.mkdir(parents=True,exist_ok=True);sources={}
    def client(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources['client/'+name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    def server(path):
        raw=(ROOT/'reference/server/Server/src/main'/path).read_bytes().replace(b'\r\n',b'\n');sources['server/'+path]=hashlib.sha256(raw).hexdigest();return re.sub(r'^\s*(?:package |import (?!java\.))[^\n]*\n','',raw.decode(),flags=re.M)
    for path in ['core/net/packet/IoBuffer.java','core/net/packet/PacketHeader.java','core/net/packet/Context.java','core/net/packet/OutgoingPacket.java','core/net/packet/context/DisplayModelContext.java','core/net/packet/context/AnimateInterfaceContext.java','core/net/packet/context/DefaultContext.java','core/net/packet/out/DisplayModel.java','core/net/packet/out/AnimateInterface.java','core/net/packet/out/InterfaceSetAngle.java','core/net/packet/context/ChildPositionContext.java','core/net/packet/out/RepositionChild.java']:
        (RUN/pathlib.Path(path).name).write_text(server(path),encoding='utf-8')
    for name in ['DelayedStateChange','Node','SecondaryNode','SecondaryLinkedList','HashTable','LongNode']:(RUN/(name+'.java')).write_text(client(name),encoding='utf-8')
    client('Component');fields=''.join('public '+t+' '+name+('='+value if value else '')+';'for t,name,value in component_fields.fields()if t in['int','boolean','byte','short'])
    (RUN/'Component.java').write_text('class Component{'+fields+'}',encoding='utf-8')
    obj=client('ObjType');fields=''.join('public '+t+' '+name+('='+value if value else '')+';'for t,name,value in item_fields.fields())
    code='class ObjType{static final JagString DEFAULT_NAME=new JagString("null");'+fields+''.join(v.method(obj,s)for s in['public static class Opcodes','public final void decode(','private void decode(','public final void generateCertificate(','public final void generateLent('])+'}'
    (RUN/'ObjType.java').write_text(code,encoding='utf-8')
    buffer=client('Buffer');methods=['public final int g1()','public final byte g1b()','public final int g2()','public final int g2b()','public final int g3()','public final int g4()','public final JagString gjstr()','public final int g2add()','public final int ig2()','public final int ig2b()','public final int ig2add()','public final int mg4()','public final int img4()','public final int ig4()','public final int g2badd()']
    (RUN/'Buffer.java').write_text('class Buffer{byte[]data;int offset;Buffer(byte[]b){data=b;}'+''.join(v.method(buffer,s)for s in methods)+'}',encoding='utf-8')
    protocol=client('Protocol');names=['IF_SETANIM','IF_SETMODEL','IF_SETNPCHEAD','IF_SETPLAYERHEAD','IF_SETANGLE','IF_SETOBJECT','IF_SETPOSITION'];branches='else '.join(v.method(protocol,'if (opcode == ServerProt.'+name+')')for name in names)
    apply='else '.join(v.method(protocol,'if (type == '+str(n)+')')for n in[4,5,8,9,10,11])
    (RUN/'Protocol.java').write_text('class Protocol{static Buffer inboundBuffer;static int opcode,verifyId;static boolean verifyIdChanged;'+v.method(protocol,'public static boolean setVerifyId(')+'static boolean read(){'+branches+'return false;}static void poll(){for(DelayedStateChange change=DelayedStateChange.poll();change!=null;change=DelayedStateChange.poll()){int type=change.getType(),i=change.getId(),x,dx,modelId;Component component;'+apply+'}}}',encoding='utf-8')
    declarations=''.join('static final int '+name+'='+str(op)+';'for name,op in zip(names,[36,130,73,66,132,50,119]));(RUN/'ServerProt.java').write_text('class ServerProt{'+declarations+'}',encoding='utf-8')
    (RUN/'Probe.java').write_text(SUPPORT.replace('__FIELDS__',','.join('"'+s+'"'for s in FIELDS)),encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],check=True)
    raw=_recorded(ROOT,'ui.pack').read_bytes();n=struct.unpack_from('<I',raw,8)[0];manifest=json.loads(raw[12:12+n]);requests=[]
    for name,(off,size)in manifest['files'].items():
        if name.startswith('item/'):requests.append('D;'+name[5:]+';'+raw[12+n+off:12+n+off+size].hex())
    rng=random.Random(530605);wires=[];pointer=241<<16|2
    for i in range(1536):
        kind=i%6;ptr=pointer if i<768 else rng.randrange(-2147483648,2147483648)
        a=rng.choice([-1,0,1,127,128,32767,32768,65535,65536,rng.randrange(14654)])
        b=rng.choice([-2,-1,0,1,100,512,2000,2147483647]);c=rng.choice([0,1,32,500,1400,32767,65535]);counter=rng.choice([-2147483648,-1,0,1,32767,65535,65536,2147483647])
        args=[kind,ptr,a,b,c,counter];wires.append(dict(args=args));requests.append('W;'+';'.join(map(str,args)))
    position_rng=random.Random(530607)
    for i in range(512):
        ptr=pointer if i<256 else position_rng.randrange(-2147483648,2147483648)
        args=[6,ptr,*[position_rng.choice([-2147483648,-65536,-32769,-32768,-1,0,1,32767,32768,65535,65536,2147483647,position_rng.randrange(-100000,100000)])for _ in range(2)],0,position_rng.randrange(-2147483648,2147483648)]
        wires.append(dict(args=args));requests.append('W;'+';'.join(map(str,args)))
    cases=[]
    for _ in range(128):
        initial=[1,-1,-1,-1,0,1,0,0,0,100,0,0,0,-1,0,0,0,0,0,0,0];config=[rng.randrange(2),rng.choice([0,32,128]),rng.choice([0,64,128]),rng.randrange(-2147483648,2147483648)]
        commands=[]
        for _ in range(64):
            op=rng.choice(['S','S','C','T','P','P','H'])
            commands.append([op,rng.randrange(768)]if op=='S'else[op,rng.choice([4,5,8,9,10])]if op=='C'else[op,rng.choice([0,1,499,500,501,600])]if op=='T'else[op,rng.randrange(-2147483648,2147483648)]if op=='H'else[op])
        commands+=[['T',501],['P']];cases.append(dict(initial=initial,config=config,commands=commands));requests.append('Q;'+','.join(map(str,initial))+';'+','.join(map(str,config))+';'+'/'.join(','.join(map(str,c))for c in commands))
    for i in range(128):
        initial=[1,-1,-1,-1,0,1,0,0,0,100,0,0,0,-1,0]+[position_rng.randrange(-2147483648,2147483648)for _ in range(4)]+[position_rng.randrange(-128,128)for _ in range(2)]
        commands=[['C',11],['S',1536+i],['P'],['T',499],['P'],['T',1],['P'],['S',1536+i],['P'],['S',1536+(i+1)%256],['S',1536+i],['P']]
        for _ in range(32):
            op=position_rng.choice(['S','S','C','T','P','P'])
            commands.append([op,1536+position_rng.randrange(256)]if op=='S'else[op,11]if op=='C'else[op,position_rng.choice([0,1,499,500,501])]if op=='T'else[op])
        commands += [['T',501],['P']]
        cases.append(dict(initial=initial,config=[1,32,32,0],commands=commands));requests.append('Q;'+','.join(map(str,initial))+';1,32,32,0;'+'/'.join(','.join(map(str,c))for c in commands))
    output=subprocess.run(['java','-cp',str(RUN),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(wires)+len(cases)
    for wire,line in zip(wires,output):wire['expected']=json.loads(line)
    for case,line in zip(cases,output[len(wires):]):case['expected']=json.loads(line)
    result=dict(sources=sources,uiSha256=hashlib.sha256(raw).hexdigest(),pointer=pointer,fields=FIELDS,bindings='Complete original DisplayModel/AnimateInterface/InterfaceSetAngle/RepositionChild writers and IoBuffer with controlled counter/session/ISAAC0 services. Complete selected RT4 Protocol reader and delayed apply branches with original intrusive queues; controlled monotonic clock, one component, appearance hash and resolved item lookup. Original raw ObjType decoders/template methods are used. Malformed legacy zero zoom reports arithmetic failure after original partial writes. GPU/hierarchy/complete Player and world integration are separate.',wires=wires,cases=cases)
    if check:
        if json.loads(FIX.read_text(encoding='utf-8'))!=result:raise SystemExit('UI model updates fixture differs')
    else:FIX.write_text(json.dumps(result,separators=(',',':'))+'\n',encoding='utf-8')
    print('Original model-widget wire:',len(wires),'packets;',len(cases),'delayed UI scenarios')

SUPPORT=r'''
import java.io.*;import java.nio.*;import java.util.*;
class ISAACCipher{int getNextValue(){return 0;}}class ByteBufferUtils{static int packGJString2(int a,byte[]b,String c){throw new UnsupportedOperationException();}static String getString(ByteBuffer b){throw new UnsupportedOperationException();}}
class JagString{String value;JagString(String s){value=s;}static JagString decodeString(byte[]b,int n,int o){return new JagString(new String(b,o,n,java.nio.charset.StandardCharsets.ISO_8859_1));}boolean equalsIgnoreCase(JagString s){return value.equalsIgnoreCase(s.value);}}
class LocalizedText{static JagString TAKE=new JagString("Take"),DROP=new JagString("Drop"),HIDDEN=new JagString("Hidden"),LENT_ITEM_RETURN=new JagString("Discard");}
class IntNode extends Node{IntNode(int n){}}class StringNode extends Node{StringNode(JagString s){}}class IntUtils{static int clp2(int n){return n<=1?n:Integer.highestOneBit(n-1)<<1;}}
class MonotonicClock{static long now;static long currentTimeMillis(){return now;}}
class Player{Interfaces interfaces=new Interfaces();Session session=new Session();HeadAppearance appearance=new HeadAppearance();Player getDetails(){return this;}Player getPlayer(){return this;}Interfaces getInterfaceManager(){return interfaces;}Session getSession(){return session;}}
class Interfaces{int count;int getPacketCount(int n){int old=count;count+=n;return old;}}
class Session{IoBuffer last;Session getIsaacPair(){return this;}ISAACCipher getOutput(){return new ISAACCipher();}void write(IoBuffer b){last=b;}}
class HeadAppearance{int value;int getAppearanceHash(){return value;}}class PlayerList{static Player self=new Player();}
class InterfaceList{static Component component;static int redraws;static Component getComponent(int n){return component;}static void redraw(Component c){redraws++;}}
class ObjTypeList{static Map<Integer,ObjType>items=new HashMap<>();static Set<Integer>done=new HashSet<>();static ObjType get(int id){ObjType i=items.computeIfAbsent(id,k->new ObjType());if(done.add(id)){if(i.certtemplate!=-1)i.generateCertificate(get(i.certlink),get(i.certtemplate));if(i.lentTemplate!=-1)i.generateLent(get(i.lentLink),get(i.lentTemplate));}return i;}}
public class Probe{
static String[]fields={__FIELDS__};static List<byte[]>packets=new ArrayList<>();static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static int[]unsigned(byte[]b){int[]out=new int[b.length];for(int i=0;i<b.length;i++)out[i]=b[i]&255;return out;}
static int[]snapshot()throws Exception{int[]out=new int[fields.length+2];for(int i=0;i<fields.length;i++)out[i]=Component.class.getField(fields[i]).getInt(InterfaceList.component);out[fields.length]=Protocol.verifyId;out[fields.length+1]=InterfaceList.redraws;return out;}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);
if(p[0].equals("D")){ObjType item=new ObjType();item.id=Integer.parseInt(p[1]);item.decode(new Buffer(HexFormat.of().parseHex(p[2])));ObjTypeList.items.put(item.id,item);continue;}
if(p[0].equals("W")){int kind=Integer.parseInt(p[1]),ptr=Integer.parseInt(p[2]),a=Integer.parseInt(p[3]),b=Integer.parseInt(p[4]),c=Integer.parseInt(p[5]),count=Integer.parseInt(p[6]);Player player=new Player();player.interfaces.count=count;int hi=ptr>>16,lo=ptr&65535;
switch(kind){case 0:new AnimateInterface().send(new AnimateInterfaceContext(player,a,hi,lo));break;case 1:new DisplayModel().send(new DisplayModelContext(player,DisplayModelContext.ModelType.MODEL,a,0,hi,lo));break;case 2:new DisplayModel().send(new DisplayModelContext(player,a,hi,lo));break;case 3:new DisplayModel().send(new DisplayModelContext(player,hi,lo));break;case 4:new InterfaceSetAngle().send(new DefaultContext(player,a,b,c,hi,lo));break;case 6:new RepositionChild().send(new ChildPositionContext(player,hi,lo,a,b));break;case 5:DisplayModelContext ctx=new DisplayModelContext(player,DisplayModelContext.ModelType.ITEM,a,b,hi,lo);ctx.setZoom(c);new DisplayModel().send(ctx);break;}
IoBuffer buf=player.session.last;byte[]wire=new byte[buf.toByteBuffer().position()+1];wire[0]=(byte)buf.opcode();System.arraycopy(buf.array(),0,wire,1,wire.length-1);packets.add(wire);System.out.println("["+Arrays.toString(unsigned(wire))+","+player.interfaces.count+"]");continue;}
Component c=new Component();int[]initial=ints(p[1]),config=ints(p[2]);for(int i=0;i<fields.length;i++){java.lang.reflect.Field field=Component.class.getField(fields[i]);if(field.getType()==byte.class)field.setByte(c,(byte)initial[i]);else field.setInt(c,initial[i]);}c.if3=config[0]!=0;c.baseWidth=config[1];c.modelViewportWidth=config[2];InterfaceList.component=c;InterfaceList.redraws=0;Protocol.verifyId=0;PlayerList.self.appearance.value=config[3];DelayedStateChange.clear();MonotonicClock.now=1000;List<String>snapshots=new ArrayList<>();int failures=0;
for(String command:p[3].split("/")){String[]a=command.split(",");int n=a.length>1?Integer.parseInt(a[1]):0;switch(a[0]){case"T":MonotonicClock.now+=n;break;case"C":DelayedStateChange.create(n,15794178).pushClient();break;case"H":PlayerList.self.appearance.value=n;break;case"S":byte[]wire=packets.get(n);Protocol.opcode=wire[0]&255;Protocol.inboundBuffer=new Buffer(Arrays.copyOfRange(wire,1,wire.length));try{Protocol.read();}catch(ArithmeticException e){failures++;}break;case"P":Protocol.poll();snapshots.add("["+Arrays.toString(snapshot())+","+failures+"]");break;}}
System.out.println("["+String.join(",",snapshots)+"]");
}}}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
