"""Compare real raw kit/item/NPC/player head assembly with the pinned client."""
import argparse,hashlib,importlib.util,json,pathlib,random,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/ui-model-geometry-oracle';FIX=ROOT/'fixtures/ui/model-geometry.json'
spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
spec=importlib.util.spec_from_file_location('f',ROOT/'tools/port_client_items.py');f=importlib.util.module_from_spec(spec);spec.loader.exec_module(f)

def pack(path):
    raw=path.read_bytes();n=struct.unpack_from('<I',raw,8)[0];m=json.loads(raw[12:12+n]);return{key:raw[12+n+off:12+n+off+size]for key,(off,size)in m['files'].items()}
def sha(b):return hashlib.sha256(b).hexdigest()
def generate(check=False):
    v.RUN=RUN;sources=v.generate(build_only=True)
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=sha(raw);return v.clean(raw.decode())
    npc=source('NpcType');body=v.method(npc,'public final Model getHeadModel(');body=body[body.index('RawModel[] rawModels ='):body.index('model = rawModel.createModel')].replace('client.npcRecolorPalette','NPC_PALETTE')
    p=RUN/'NpcDefinitionOracle.java';text=p.read_text(encoding='utf-8');text=text.replace('class NpcTypeList', 'class NpcTypeList',1);anchor='}class NpcTypeList';assert anchor in text;text=text.replace(anchor,'RawModel head(){if(headmodels==null)return null;'+body+'return rawModel;}}class NpcTypeList');p.write_text(text,encoding='utf-8')
    item=source('ObjType');fields=''.join('public '+t+' '+n+('='+value if value else '')+';'for t,n,value in f.fields())
    code='class ObjType{static final JagString DEFAULT_NAME=new JagString("null");'+fields
    for m in ['public static class Opcodes','public final void decode(', 'private void decode(', 'public final RawModel getHeadModel(', 'public final void generateCertificate(', 'public final void generateLent(']:code+=v.method(item,m)
    (RUN/'ObjType.java').write_text(code+'}'+ITEM_SERVICE,encoding='utf-8')
    p=RUN/'MovementSupport.java';text=p.read_text(encoding='utf-8').replace('class LocalizedText {','class LocalizedText {static JagString TAKE=new JagString("Take"),DROP=new JagString("Drop"),LENT_ITEM_RETURN=new JagString("Discard");');p.write_text(text,encoding='utf-8')
    p=RUN/'Support.java';text=p.read_text(encoding='utf-8').replace('class IdkTypeList {static Js5 modelsArchive;}','class IdkTypeList {static Js5 modelsArchive;static java.util.Map<Integer,IdkType>kits=new java.util.HashMap<>();static IdkType get(int id){return kits.get(id);}}');p.write_text(text,encoding='utf-8')
    player=source('PlayerAppearance');head=v.method(player,'public final Model getHeadModel(');head=head[head.index('RawModel[] models ='):head.index('model = combined.createModel')]
    partial=v.method(player,'public final Model getPartialHeadModel(');partial=partial[partial.index('RawModel[] models ='):partial.index('model = rawModel.createModel')]
    (RUN/'PlayerHead.java').write_text('class PlayerHead{int[]identikit,colors;boolean gender;static short[]sourceBodyColors=AppearanceOracle.GAME0_SOURCE_BODY_COLORS,sourceSkinColors=AppearanceOracle.GAME0_SOURCE_SKIN_COLORS;static short[][]destinationBodyColors=AppearanceOracle.GAME0_DESTINATION_BODY_COLORS,destinationSkinColors=AppearanceOracle.GAME0_DESTINATION_SKIN_COLORS;RawModel head(){int slotId;'+head+'return combined;}RawModel partial(int jawId,int beardId,int hairId){'+partial+'return rawModel;}}',encoding='utf-8')
    (RUN/'HeadOracle.java').write_text(SUPPORT,encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],check=True)
    ui_path=ROOT/'assets/visual/ui-models.pack';world_path=ROOT/'assets/visual/live/lumbridge-minimap.pack';ui=pack(ui_path);world=pack(world_path);data=RUN/'data';data.mkdir(exist_ok=True)
    for name,raw in ui.items():
        if name.startswith('model/'):(data/('model_'+name[6:]+'.bin')).write_bytes(raw)
    cases=[];requests=[];kit_ids=[];item_ids=[]
    for name,raw in sorted(ui.items()):
        if name.startswith('kit/'):
            id=int(name[4:]);kit_ids.append(id);cases.append(dict(kind='kit',id=id));requests.append(f'K\t{id}\t'+raw.hex())
    for name,raw in sorted(ui.items()):
        if name.startswith('item/'):
            id=int(name[5:]);item_ids.append(id)
            for female in[False,True]:cases.append(dict(kind='item',id=id,female=female));requests.append(f'I\t{id}\t{int(female)}\t'+raw.hex())
    for name,raw in sorted(world.items()):
        if name.startswith('npc/'):
            id=int(name[4:]);cases.append(dict(kind='npc',id=id));requests.append(f'N\t{id}\t'+raw.hex())
    rng=random.Random(530603)
    for i in range(256):
        slots=[rng.choice(kit_ids)-0x80000000 if j%3==0 else (rng.choice(item_ids)|0x40000000)if j%3==1 else 0 for j in range(12)];colors=[rng.randrange(16)for _ in range(5)];female=bool(i%2)
        cases.append(dict(kind='player',slots=slots,colors=colors,female=female));requests.append('P\t'+','.join(map(str,slots))+'\t'+','.join(map(str,colors))+'\t'+str(int(female)))
    for i in range(128):
        ids=[rng.choice(kit_ids)for _ in range(3)];colors=[rng.randrange(16)for _ in range(5)];cases.append(dict(kind='partial',ids=ids,colors=colors));requests.append('H\t'+','.join(map(str,ids))+'\t'+','.join(map(str,colors)))
    result=subprocess.run(['java','-Xmx2g','-cp',str(RUN),'HeadOracle',str(data)],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for case,line in zip(cases,result):case['expected']=line
    output=dict(sources=sources,uiSha256=sha(ui_path.read_bytes()),worldSha256=sha(world_path.read_bytes()),palette=[(i*79+1234)&65535 for i in range(256)],bindings='Original complete IdkType and raw ObjType decode/getHeadModel; unchanged resolved non-morph NPC raw-head assembly and PlayerAppearance head/partial raw assembly fragments, with complete original RawModel decoding/combination/recolor/retexture. Metadata ready services and palette are explicit. Item templates, morph choice, caches/animation and GPU output are separate contracts.',cases=cases)
    if check:
        if json.loads(FIX.read_text(encoding='utf-8'))!=output:raise SystemExit('Head geometry fixture differs')
    else:FIX.write_text(json.dumps(output,separators=(',',':'))+'\n',encoding='utf-8')
    print('Original head geometry:',len(cases),'cases;',sum(c['expected']not in['null','morph']for c in cases),'assembled models')

ITEM_SERVICE=r'''class ObjTypeList{
static Js5 modelsArchive;static java.util.Map<Integer,ObjType>items=new java.util.HashMap<>();static java.util.Set<Integer>resolved=new java.util.HashSet<>();
static ObjType get(int id){ObjType item=items.get(id);if(resolved.add(id)){if(item.certtemplate!=-1)item.generateCertificate(get(item.certlink),get(item.certtemplate));if(item.lentTemplate!=-1)item.generateLent(get(item.lentLink),get(item.lentTemplate));}return item;}}
'''
SUPPORT=r'''
import java.io.*;import java.nio.file.*;import java.util.*;
public class HeadOracle{
static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static String result(RawModel m)throws Exception{if(m==null)return"null";ByteArrayOutputStream b=new ByteArrayOutputStream();VisualOracle.canonical(new DataOutputStream(b),m);return VisualOracle.digest(b.toByteArray());}
public static void main(String[]args)throws Exception{Js5 archive=new Js5(Path.of(args[0]));IdkTypeList.modelsArchive=archive;ObjTypeList.modelsArchive=archive;NpcTypeList.modelsArchive=archive;for(int i=0;i<256;i++)NpcDefinitionOracle.NPC_PALETTE[i]=(short)(i*79+1234);BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\t");switch(p[0]){
case"K":{IdkType k=new IdkType();k.decode(new Buffer(HexFormat.of().parseHex(p[2])));IdkTypeList.kits.put(Integer.parseInt(p[1]),k);System.out.println(result(k.getHeadModel()));break;}
case"I":{ObjType i=new ObjType();i.decode(new Buffer(HexFormat.of().parseHex(p[3])));ObjTypeList.items.put(Integer.parseInt(p[1]),i);System.out.println(result(i.getHeadModel(p[2].equals("1"))));break;}
case"N":{NpcDefinitionOracle n=new NpcDefinitionOracle();n.decode(new Buffer(HexFormat.of().parseHex(p[2])));System.out.println(n.multiNpcs!=null?"morph":result(n.head()));break;}
case"P":{PlayerHead h=new PlayerHead();h.identikit=ints(p[1]);h.colors=ints(p[2]);h.gender=p[3].equals("1");System.out.println(result(h.head()));break;}
case"H":{PlayerHead h=new PlayerHead();h.colors=ints(p[2]);int[]ids=ints(p[1]);System.out.println(result(h.partial(ids[0],ids[1],ids[2])));break;}
default:throw new AssertionError(p[0]);}}}}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
