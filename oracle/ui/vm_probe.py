"""Unchanged ScriptRunner component-operation branches with childless UI bindings."""
import hashlib,json,random,re,subprocess

def emit(run,source,method):
    runner=source('ScriptRunner')
    names={name:int(value)for name,value in re.findall(r'public static final int (\w+) = (\d+);',runner)}
    specs={
        1000:('setPosition',4,0),1001:('setSize',4,0),1003:('setHidden',1,0),1004:('setAspect',2,0),1005:('setNoClickThrough',1,0),
        1100:('setScrollPos',2,0),1101:('setRGB',1,0),1102:('setFilled',1,0),1103:('setTrans',1,0),1104:('setLineWid',1,0),1105:('setSprite',1,0),1106:('set2DAngle',1,0),1107:('setSpriteTiling',1,0),1108:('setModel',1,0),1109:('set3DRotation',6,0),1110:('setAnimation',1,0),1111:('setModelOrthog',1,0),1112:('setText',0,1),1113:('setFont',1,0),1114:('setTextAlignment',3,0),1115:('setTextAntiMacro',1,0),1116:('setOutlineThickness',1,0),1117:('setShadowColor',1,0),1118:('setVFlip',1,0),1119:('setHFlip',1,0),1120:('setScrollMax',2,0),1121:(None,2,0),1122:('setAlpha',1,0),1123:('set3DViewDistance',1,0),
        1200:('setItem',2,0),1205:('setItemNoNum',2,0),1300:('setContextMenuOption',1,1),1301:('setDraggable',2,0),1302:('setDragRenderBehaviour',1,0),1303:('setDragDeadZone',1,0),1304:('setDragDeadTime',1,0),1305:('setOptionBase',0,1),1306:('setTargetVerb',0,1),1307:('setNoOptions',0,0),1308:(None,2,0),1309:(None,2,0),
    }
    # Getter outputs remain on the original stacks and are observed separately.
    for op,name in [(1500,'getX'),(1501,'getY'),(1502,'getWidth'),(1503,'getHeight'),(1504,'getHidden'),(1505,'getLayer'),(1600,'getScrollX'),(1601,'getScrollY'),(1602,'getText'),(1603,'getScrollMaxH'),(1604,'getScrollMaxV'),(1605,'get3DDistance'),(1606,'getRotateX'),(1607,'getRotateZ'),(1608,'getRotateY'),(1609,'getAlpha'),(1610,'getModelXOffset'),(1611,'getModelYOffset'),(1612,'getSpriteId'),(1700,'getItemId'),(1701,'getItemAmt'),(1702,'getChildId'),(1800,'getTargetMask'),(1801,'getOption'),(1802,'getOptionBase')]:specs[op]=(name,1 if op==1801 else 0,0)
    blocks=[]
    for op,(name,_,_)in specs.items():
        if op in [1200,1205]:
            if op==1205:continue
            signature='if (opcode == Cs2Opcodes.setItem || opcode == Cs2Opcodes.setItemNoNum)'
        else:signature=f'if (opcode == Cs2Opcodes.{name})'if name else f'if (opcode == {op})'
        block=method(runner,signature)
        block=block.replace('continue;','return;')
        blocks.append(block)
    constants=''.join(f'static final int {name}={value};'for name,value in names.items())
    code='''import java.io.*;import java.util.*;import java.nio.charset.StandardCharsets;
public class OperationProbe {
 static final JagString EMPTY_STRING=JagString.EMPTY;static Component component;static Map<Integer,byte[]> itemBytes=new HashMap<>();static int[] intStack=new int[1000];static JagString[] stringStack=new JagString[1000];static int isp,ssp;
 static void apply(int opcode){int int1=0,int2=0,int3=0;JagString str1=null;boolean boolValue=false;
 '''+''.join(blocks)+'''throw new IllegalArgumentException("Unsupported probe "+opcode);}
 static class Cs2Opcodes {'''+constants+'''}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8));String line;while((line=r.readLine())!=null){String[] x=line.split("\\t",-1);
 if(x[0].equals("I")){itemBytes.put(Integer.parseInt(x[1]),HexFormat.of().parseHex(x[2]));continue;}
 component=new Component();component.id=Integer.parseInt(x[0]);Buffer b=new Buffer(HexFormat.of().parseHex(x[1]));if(b.data[0]==-1)component.decodeIf3(b);else component.decodeIf1(b);int opcode=Integer.parseInt(x[2]);isp=0;ssp=0;
 if(!x[3].isEmpty())for(String n:x[3].split(","))intStack[isp++]=Integer.parseInt(n);
 if(Integer.parseInt(x[5])>0)for(String s:x[4].split(",",-1))stringStack[ssp++]=new JagString(new String(HexFormat.of().parseHex(s),StandardCharsets.ISO_8859_1));
 apply(opcode);ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream d=new DataOutputStream(bytes);Support.canonical(d,component);Support.canonical(d,Arrays.copyOf(intStack,isp));Support.canonical(d,Arrays.copyOf(stringStack,ssp));d.writeInt(component.dragParent==null?-1:component.dragParent.id);d.writeInt(component.dragParent==null?-1:component.dragParent.createdComponentId);
 System.out.println(HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));
 }} }
class ObjTypeList{static ObjType get(int id){ObjType c=new ObjType();c.id=id;byte[] raw=OperationProbe.itemBytes.get(id);if(raw!=null)c.decode(new Buffer(raw));return c;}}
'''
    callbacks=sorted(set(re.findall(r'DelayedStateChange\.(\w+)\(',code)))
    code+='class DelayedStateChange{'+''.join(f'static void {name}(int id){{}}'for name in callbacks)+'}'
    (run/'OperationProbe.java').write_text(code,encoding='utf-8')
    # Data-only methods are copied from Component; parent/layout invalidation is a binding.
    p=run/'Component.java';s=p.read_text();s=s[:-1]+'Component dragParent;Component[] createdComponents;'+method(source('Component'),'public final void setOp(')+method(source('Component'),'public final void setDragTarget(')+'}';p.write_text(s,encoding='utf-8')
    p=run/'InterfaceList.java';s=p.read_text();s=s[:-1]+'''static Component getParent(Component c){return null;}static Component getComponent(int id,int child){return OperationProbe.component.id==id&&child==-1?OperationProbe.component:null;}static void redraw(Component c){}static void layoutComponent(Component c,boolean resize){}'''+method(source('InterfaceList'),'public static void update(')+'}class GameShell{static int canvasWidth=765,canvasHeight=503;}';p.write_text(s,encoding='utf-8')
    return specs

def cases(specs,component_cases,raw,manifest,pack_data,pack_start):
    rng=random.Random(530);by_id={c['id']:c for c in component_cases};inputs=[];out=[]
    for id in [995,1277,4151]:
        start,size=manifest['files'][f'item/{id}'];inputs.append(f'I\t{id}\t'+pack_data[pack_start+start:pack_start+start+size].hex())
    for op,(_,ni,ns)in specs.items():
        for i in range(8):
            cid=rng.choice([149<<16,620<<16|22,762<<16|61,762<<16|14,387<<16|28]);c=by_id[cid];ints=[rng.choice([-32768,-1,0,1,2,31,128,32767,65535])for _ in range(ni)];strings=[rng.choice(['Choose','A<col=ff0000> test','', 'Caf\xe9'])for _ in range(ns)]
            if op==1004:ints=[rng.randrange(1,1025),rng.randrange(1,1025)]
            if op in [1200,1205]:ints=[rng.choice([995,1277,4151,-1]),rng.choice([-1,0,1,127,1000000])]
            if op==1301:ints=[cid,-1]
            if op in [1300,1309,1801]:ints[0]=rng.randrange(0,12)
            raw_component=raw[c['offset']:c['offset']+c['size']];string_field=','.join(s.encode('latin1').hex() for s in strings)
            # One empty string needs a marker, since the transport's empty column means zero strings.
            if ns and string_field=='':string_field='00'
            # Source JagString can store a NUL in a constructed string; native script strings cannot.
            if ns and string_field=='00':strings=[''];string_field=''
            out.append(dict(component=cid,opcode=op,ints=ints,strings=strings,result='string'if op in [1602,1801,1802]else'int'if op>=1500 else None))
            inputs.append(f'{cid}\t{raw_component.hex()}\t{op}\t'+','.join(map(str,ints))+'\t'+string_field+'\t'+str(ns))
    return inputs,out
