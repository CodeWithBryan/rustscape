"""Differential corpus for unchanged RT4 Component decoders and layout methods."""
import argparse,hashlib,importlib.util,json,pathlib,random,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];SRC=ROOT/'reference/client/client/src/main/java/rt4';RUN=ROOT/'runs/ui-oracle';FIX=ROOT/'fixtures/ui'
spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
spec=importlib.util.spec_from_file_location('fields',ROOT/'tools/port_component_fields.py');f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)
spec=importlib.util.spec_from_file_location('items',ROOT/'tools/port_client_items.py');items=importlib.util.module_from_spec(spec);spec.loader.exec_module(items)
spec=importlib.util.spec_from_file_location('vm_probe',pathlib.Path(__file__).with_name('vm_probe.py'));vm_probe=importlib.util.module_from_spec(spec);spec.loader.exec_module(vm_probe)
def main(check):
    RUN.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    component=source('Component');fields=f.fields()
    declarations='\n'.join('public '+t+' '+n+(' = '+value if value else '')+';' for t,n,value in fields)
    code='public final class Component {static final JagString EMPTY_STRING=JagString.EMPTY;static final ServerActiveProperties DEFAULT_SERVER_ACTIVE_PROPERTIES=new ServerActiveProperties(0,-1);'+declarations
    for signature in ['public final void decodeIf1(', 'public final void decodeIf3(', 'private Object[] decodeEventHandler(', 'private int[] decodeTriggers(']:code+=v.method(component,signature)
    (RUN/'Component.java').write_text(code+'}',encoding='utf-8')
    b=source('Buffer');code='class Buffer {byte[] data;int offset;Buffer(byte[] d){data=d;}'
    for signature in ['public final int g1()', 'public final byte g1b()', 'public final int g2()', 'public final int g2b()', 'public final int g3()', 'public final int g4()', 'public final JagString gjstr()', 'public final JagString fastgjstr()', 'public final void p1(', 'public final void p2(', 'public final void p2add(', 'public final void ip2(', 'public final void ip2add(', 'public final void p4(', 'public final void ip4(', 'public final void mp4(', 'public final void imp4(']:code+=v.method(b,signature)
    code+='void p1isaac(int n){p1(n);}'
    (RUN/'Buffer.java').write_text(code+'}',encoding='utf-8')
    prop=source('ServerActiveProperties').replace(' extends Node','');(RUN/'ServerActiveProperties.java').write_text(prop)
    layout=source('InterfaceList');(RUN/'InterfaceList.java').write_text('class InterfaceList {static Component gameViewportComponent;static Requests lowPriorityRequests=new Requests();static ServerActiveProperties getServerActiveProperties(Component c){return c.properties;}'+v.method(layout,'public static void calculatePosition(')+v.method(layout,'public static void calculateSize(')+'}')
    (RUN/'ClientScript.java').write_text(source('ClientScript').replace(' extends SecondaryNode',''),encoding='utf-8')
    (RUN/'ClientScriptList.java').write_text('class ClientScriptList {static LruHashTable scripts=new LruHashTable();'+v.method(source('ClientScriptList'),'public static ClientScript get(')+'}',encoding='utf-8')
    obj=source('ObjType');code='class ObjType {static final JagString DEFAULT_NAME=new JagString("null");'+'\n'.join('public '+t+' '+n+(' = '+value if value else '')+';'for t,n,value in items.fields())
    for signature in ['public static class Opcodes','public final void decode(', 'private void decode(', 'public final void generateCertificate(', 'public final void generateLent(']:code+=v.method(obj,signature)
    (RUN/'ObjType.java').write_text(code+'}',encoding='utf-8')
    raster=source('Rasteriser');(RUN/'Rasteriser.java').write_text('class Rasteriser {static int width=36,height=32,alpha,centerX=16,centerY=16;static float brightness=1.0F;static TextureProvider textureProvider=new TextureProvider();static boolean jagged=false,testX=true,lowDetail,opaque,textureHasTransparency;static int[] offsets=new int[32],palette=new int[65536];static{for(int i=0;i<32;i++)offsets[i]=i*36;for(int i=0;i<65536;i++)palette[i]=(i*131586&0xffffff)|1;}'+''.join(v.method(raster,s)for s in ['public static void fillGouraudTriangle(','private static void drawGouraudScanline(','public static void fillTexturedAlphaTriangle(','private static void drawTexturedAlphaScanline('])+'}',encoding='utf-8')
    (RUN/'ColorUtils.java').write_text('class ColorUtils {'+v.method(source('ColorUtils'),'public static int multiplyLightness(')+'}',encoding='utf-8')
    mini=source('MiniMenu');clientprot=source('ClientProt');actions=['OBJ_ACTION_1','OBJ_EQUIP_ACTION','OBJ_IN_COMPONENT_ACTION_4','OBJ_ACTION_4','OBJ_ACTION_5','OBJ_IN_COMPONENT_ACTION_1','OBJ_OPERATE_ACTION','UNKNOWN_48','UNKNOWN_7','UNKNOWN_13','OBJ_EXAMINE_IN_COMPONENT']
    action_code={n:int(value)for n,value in __import__('re').findall(r'public static final int (\w+) = (\d+);',mini)}
    code='class ActionOracle {static byte[] item(int actionCode,int keyInt,int menuArg1,int menuArg2){Protocol.outboundBuffer=new Buffer(new byte[1000]);'
    for name in actions:
        block=v.method(mini,'if (actionCode == '+name+')');lines='\n'.join(line for line in block.splitlines()if 'Protocol.outboundBuffer.'in line);code+=f'if(actionCode=={action_code[name]}){{'+lines+'}'
    code+='return java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset);}static byte[] button(int op,int childId,int componentId){Protocol.outboundBuffer=new Buffer(new byte[1000]);'
    for op in range(1,11):code+=v.method(clientprot,f'if (op == {op})')
    code+='return java.util.Arrays.copyOf(Protocol.outboundBuffer.data,Protocol.outboundBuffer.offset);}}class Protocol{static Buffer outboundBuffer;}'
    (RUN/'ActionOracle.java').write_text(code,encoding='utf-8')
    operation_specs=vm_probe.emit(RUN,source,v.method)
    (RUN/'Support.java').write_text(SUPPORT,encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p) for p in RUN.glob('*.java')]],check=True)
    data=(ROOT/'assets/visual/ui.pack').read_bytes();n=struct.unpack_from('<I',data,8)[0];manifest=json.loads(data[12:12+n]);start=12+n
    inputs=[];cases=[];blob=bytearray()
    for name,(offset,size) in manifest['files'].items():
        if not name.startswith('interface/'):continue
        _,g,c=name.split('/');id=(int(g)<<16)|int(c);raw=data[start+offset:start+offset+size]
        cases.append(dict(id=id,offset=len(blob),size=len(raw)));blob.extend(raw);inputs.append(f'{id}\t{raw.hex()}')
    script_start=len(inputs);scripts=[]
    for name,(offset,size) in manifest['files'].items():
        if not name.startswith('script/'):continue
        raw=data[start+offset:start+offset+size];scripts.append(dict(id=int(name.split('/')[1]),offset=len(blob),size=size));blob.extend(raw);inputs.append('S\t'+raw.hex())
    item_start=len(inputs);item_cases=[]
    for name,(offset,size) in manifest['files'].items():
        if not name.startswith('item/'):continue
        raw=data[start+offset:start+offset+size];id=int(name.split('/')[1]);item_cases.append(dict(id=id,offset=len(blob),size=size));blob.extend(raw);inputs.append(f'I\t{id}\t'+raw.hex())
    layout_start=len(inputs)
    # Layout probes exercise all modes independently of the cached defaults.
    rng=random.Random(530)
    for i in range(512):
        values=[rng.randrange(-200000,200001) for _ in range(4)]+[rng.randrange(-2,7) for _ in range(4)]+[rng.choice([0,2,4,7])]+[rng.randrange(-20,80) for _ in range(2)]+[rng.randrange(1,50) for _ in range(2)]+[rng.randrange(-10000,100001) for _ in range(2)]
        inputs.append('L\t'+','.join(map(str,values)))
    raster_start=len(inputs);raster_cases=[]
    for _ in range(4096):
        points=[[rng.randrange(-80,100),rng.randrange(-80,100)]for _ in range(3)];colors=[rng.randrange(8000,10000)for _ in range(3)];alpha=rng.choice([0,0,0,1,128,255]);case=dict(points=points,colors=colors,alpha=alpha);raster_cases.append(case);inputs.append('R\t'+','.join(map(str,[*[p[1]for p in points],*[p[0]for p in points],*colors,alpha])))
    textured_start=len(inputs);textured_cases=[]
    for _ in range(4096):
        points=[[rng.randrange(-80,100),rng.randrange(-80,100)]for _ in range(3)];lights=[rng.randrange(2,127)for _ in range(3)];view=[[rng.randrange(-150,151),rng.randrange(-150,151),rng.randrange(500,2001)]for _ in range(3)];alpha=rng.choice([0,0,0,1,10,11,128,255]);low=rng.choice([True,False]);opaque=rng.choice([True,False]);case=dict(points=points,lights=lights,view=view,alpha=alpha,low_detail=low,opaque=opaque);textured_cases.append(case);inputs.append('T\t'+','.join(map(str,[*[p[1]for p in points],*[p[0]for p in points],*lights,*[n for p in view for n in p],alpha,int(low),int(opaque)])))
    actions_start=len(inputs);action_cases=[]
    for action in [action_code[n]for n in actions]:
        for id in [-1,0,1,127,128,32767,65535,65536]:
            slot=rng.choice([-1,0,27,255,32768,65535]);component=rng.choice([149<<16,387<<16|28,0x12345678,-1]);action_cases.append(dict(kind='item',action=action,id=id,slot=slot,component=component));inputs.append(f'A\t{action},{id},{slot},{component}')
    for op in range(1,11):
        for child in [-1,0,1,27,499,65535]:
            component=rng.choice([762<<16|73,0x12345678]);action_cases.append(dict(kind='button',action=op,slot=child,component=component));inputs.append(f'B\t{op},{child},{component}')
    result=subprocess.run(['java','-cp',str(RUN),'Support'],input='\n'.join(inputs)+'\n',text=True,encoding='utf-8',capture_output=True,check=True).stdout.splitlines()
    if len(result)!=len(inputs):raise ValueError('oracle output count')
    for c,line in zip(cases,result):
        position,digest,layout=line.split('\t');c.update(position=int(position),sha256=digest,layout=list(map(int,layout.split(','))))
    for c,digest in zip(scripts,result[script_start:item_start]):c['sha256']=digest
    for c,result_line in zip(item_cases,result[item_start:layout_start]):
        pos,digest=result_line.split('\t');c.update(position=int(pos),sha256=digest)
    layouts=[]
    for request,response in zip(inputs[layout_start:raster_start],result[layout_start:raster_start]):layouts.append(dict(input=list(map(int,request[2:].split(','))),expected=list(map(int,response.split(',')))))
    for case,response in zip(raster_cases,result[raster_start:textured_start]):case['sha256']=response
    for case,response in zip(textured_cases,result[textured_start:actions_start]):case['sha256']=response
    for case,response in zip(action_cases,result[actions_start:]):case['hex']=response
    operation_inputs,operations=vm_probe.cases(operation_specs,cases,blob,manifest,data,start)
    operation_result=subprocess.run(['java','-cp',str(RUN),'OperationProbe'],input='\n'.join(operation_inputs)+'\n',text=True,encoding='utf-8',capture_output=True,check=True).stdout.splitlines()
    if len(operations)!=len(operation_result):raise ValueError('operation probe output count')
    for case,response in zip(operations,operation_result):case['sha256']=response
    fixture=json.dumps(dict(sources=sources,components=cases,scripts=scripts,items=item_cases,layouts=layouts,raster=raster_cases,textured=textured_cases,actions=action_cases,operations=operations),separators=(',',':')).encode()+b'\n'
    FIX.mkdir(parents=True,exist_ok=True)
    for name,content in [('oracle.json',fixture),('components.bin',blob)]:
        p=FIX/name
        if check:
            if p.read_bytes()!=content:raise SystemExit(name+' differs from original client')
        else:p.write_bytes(content)
    print(f'RT4 components: {len(cases)}; scripts: {len(scripts)}; items: {len(item_cases)}; layout probes: {len(layouts)}')
    for name in ['delayed','input','text','npc','npc_bas','bridge','chat','audio','sprites','editing','chat_state','stats','item_scripts','player_interaction','entity_menu']:
        spec=importlib.util.spec_from_file_location(name,pathlib.Path(__file__).with_name(name+'.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(ROOT,v.clean,v.method,check)
    for name in ['models','model_geometry','model_animation','model_updates','item_models','cs1','player_bodies','player_forms','sized_routes','world_spots','widget_sprites','widget_draw','widget_style','legacy_widgets','widget_wait','button_clicks','widget_legacy_draw','scrollbars','inventory_scroll','inventory_render','inventory_press','inventory_examine','legacy_buttons','material_alpha','ground_interaction','ground_geometry','targeting']:
        spec=importlib.util.spec_from_file_location(name,pathlib.Path(__file__).with_name(name+'.py'));module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module);module.generate(check=check)
SUPPORT=r'''
import java.io.*;import java.util.*;import java.lang.reflect.*;import java.nio.charset.StandardCharsets;
class JagString {String s;JagString(String s){this.s=s;}static final JagString EMPTY=new JagString("");static JagString decodeString(byte[] d,int n,int off){return new JagString(new String(d,off,n,StandardCharsets.ISO_8859_1));}public int length(){return s.length();}public boolean equalsIgnoreCase(JagString other){return s.equalsIgnoreCase(other.s);}public boolean strEquals(JagString other){return s.equals(other.s);}}
class LocalizedText {static JagString OK=new JagString("Ok"),SELECT=new JagString("Select"),CONTINUE=new JagString("Continue"),DROP=new JagString("Drop"),TAKE=new JagString("Take"),HIDDEN=new JagString("Hidden"),LENT_ITEM_RETURN=new JagString("Discard");}
class Cheat{static boolean qaOpTest=false;}class HookRequest{Object[] arguments;Component source;}class Requests{void addTail(Object o){throw new AssertionError("resize hooks disabled");}}
class LruHashTable{Object get(long id){return null;}void put(Object o,long id){}}
class client{static Js5 js5Archive12=new Js5();}class Js5{byte[] fetchFile(int id,int file){return Support.scriptBytes;}}
class IntUtils{static int clp2(int n){return n;}}class Node{}class IntNode extends Node{int value;IntNode(int value){this.value=value;}}class StringNode extends Node{JagString value;StringNode(JagString value){this.value=value;}}
class HashTable{TreeMap<String,Object> values=new TreeMap<>();HashTable(int n){}void put(Node node,long id){values.putIfAbsent(Long.toString(id),node instanceof IntNode?((IntNode)node).value:((StringNode)node).value);}}
class SoftwareRaster{static int width=36;static int[] pixels;}
class TextureProvider{static boolean low,opaque;int[] getAnimatedPixels(int id,float brightness){int[] pixels=new int[low?4096:16384];for(int i=0;i<pixels.length;i++)pixels[i]=i%7==0?0:(i*131586&0xffffff)|1;return pixels;}int getAverageColor(int id){return 4500;}boolean isLowDetail(int id){return low;}boolean isOpaque(int id){return opaque;}}
public class Support {
 static byte[] scriptBytes;
 static void text(DataOutputStream d,String s)throws Exception {byte[] b=s.getBytes(StandardCharsets.UTF_8);d.writeInt(b.length);d.write(b);}
 static void canonical(DataOutputStream d,Object o)throws Exception {
  if(o==null){d.writeByte(0);return;}if(o instanceof Boolean){d.writeByte((Boolean)o?2:1);return;}
  if(o instanceof Number){d.writeByte(3);d.writeLong(((Number)o).longValue());return;}
  if(o instanceof JagString){d.writeByte(4);text(d,((JagString)o).s);return;}
  if(o instanceof HashTable){d.writeByte(6);var fields=((HashTable)o).values;d.writeInt(fields.size());for(var e:fields.entrySet()){text(d,e.getKey());canonical(d,e.getValue());}return;}
  if(o.getClass().isArray()){d.writeByte(5);int n=Array.getLength(o);d.writeInt(n);for(int i=0;i<n;i++)canonical(d,Array.get(o,i));return;}
  d.writeByte(6);TreeMap<String,Object> fields=new TreeMap<>();for(Field f:o.getClass().getDeclaredFields()){if(Modifier.isStatic(f.getModifiers())||f.getName().equals("dragParent")||f.getName().equals("createdComponents"))continue;f.setAccessible(true);fields.put(f.getName(),f.get(o));}d.writeInt(fields.size());for(var e:fields.entrySet()){text(d,e.getKey());canonical(d,e.getValue());}
 }
 static String layout(Component c,int w,int h)throws Exception {InterfaceList.calculateSize(h,w,c,false);InterfaceList.calculatePosition(c,h,w);return c.x+","+c.y+","+c.width+","+c.height;}
 public static void main(String[]args)throws Exception {BufferedReader r=new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8));String line;while((line=r.readLine())!=null){
  if(line.startsWith("A\t")||line.startsWith("B\t")){int[] x=Arrays.stream(line.substring(2).split(",")).mapToInt(Integer::parseInt).toArray();byte[] raw=line.charAt(0)=='A'?ActionOracle.item(x[0],x[1],x[2],x[3]):ActionOracle.button(x[0],x[1],x[2]);System.out.println(java.util.HexFormat.of().formatHex(raw));continue;}
  if(line.startsWith("T\t")){int[] x=Arrays.stream(line.substring(2).split(",")).mapToInt(Integer::parseInt).toArray();SoftwareRaster.pixels=new int[36*32];Arrays.fill(SoftwareRaster.pixels,0x42657b);Rasteriser.alpha=x[18];TextureProvider.low=x[19]!=0;TextureProvider.opaque=x[20]!=0;Rasteriser.fillTexturedAlphaTriangle(x[0],x[1],x[2],x[3],x[4],x[5],x[6],x[7],x[8],x[9],x[12],x[15],x[10],x[13],x[16],x[11],x[14],x[17],0);ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream d=new DataOutputStream(bytes);for(int p:SoftwareRaster.pixels)d.writeInt(p);System.out.println(java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));continue;}
  if(line.startsWith("R\t")){int[] x=Arrays.stream(line.substring(2).split(",")).mapToInt(Integer::parseInt).toArray();SoftwareRaster.pixels=new int[36*32];Arrays.fill(SoftwareRaster.pixels,0x42657b);Rasteriser.alpha=x[9];Rasteriser.fillGouraudTriangle(x[0],x[1],x[2],x[3],x[4],x[5],x[6],x[7],x[8]);ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream d=new DataOutputStream(bytes);for(int p:SoftwareRaster.pixels)d.writeInt(p);System.out.println(java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));continue;}
  if(line.startsWith("I\t")){String[] x=line.split("\t");ObjType c=new ObjType();c.id=Integer.parseInt(x[1]);Buffer b=new Buffer(java.util.HexFormat.of().parseHex(x[2]));c.decode(b);ByteArrayOutputStream bytes=new ByteArrayOutputStream();canonical(new DataOutputStream(bytes),c);System.out.println(b.offset+"\t"+java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));continue;}
  if(line.startsWith("S\t")){scriptBytes=java.util.HexFormat.of().parseHex(line.substring(2));ClientScript c=ClientScriptList.get(0);ByteArrayOutputStream bytes=new ByteArrayOutputStream();canonical(new DataOutputStream(bytes),c);System.out.println(java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));continue;}
  if(line.startsWith("L\t")){int[] x=Arrays.stream(line.substring(2).split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();c.baseX=x[0];c.baseY=x[1];c.baseWidth=x[2];c.baseHeight=x[3];c.xMode=(byte)x[4];c.yMode=(byte)x[5];c.dynamicWidthValue=(byte)x[6];c.dynamicHeightValue=(byte)x[7];c.type=x[8];c.invMarginX=x[9];c.invMarginY=x[10];c.aspectWidth=x[11];c.aspectHeight=x[12];System.out.println(layout(c,x[13],x[14]));continue;}
  String[] x=line.split("\t");Component c=new Component();c.id=Integer.parseInt(x[0]);byte[] raw=java.util.HexFormat.of().parseHex(x[1]);Buffer b=new Buffer(raw);if(raw[0]==-1)c.decodeIf3(b);else c.decodeIf1(b);
  ByteArrayOutputStream bytes=new ByteArrayOutputStream();canonical(new DataOutputStream(bytes),c);String sha=java.util.HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray()));System.out.println(b.offset+"\t"+sha+"\t"+layout(c,765,503));
 }}
}
'''
if __name__=='__main__':
    ap=argparse.ArgumentParser();ap.add_argument('--check',action='store_true');main(ap.parse_args().check)
