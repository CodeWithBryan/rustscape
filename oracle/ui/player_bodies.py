"""Compare original wearable/identity-kit raw body assembly and BAS transforms."""
import argparse,hashlib,importlib.util,json,pathlib,random,struct,subprocess
def _recorded(root,name):
    # The packs the JVM fixtures were recorded from: kept under recorded/ once
    # the default packs are rebuilt natively (crates/client/tests/support/packs.rs).
    kept=root/'assets/visual/recorded'/name
    return kept if kept.is_file() else root/'assets/visual'/name
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/player-body-oracle';FIX=ROOT/'fixtures/ui/player-bodies.json'
def module(name,path):
 s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
v=module('v',ROOT/'oracle/visual/generate.py');fields=module('f',ROOT/'tools/port_client_items.py');heads=module('h',ROOT/'oracle/ui/model_geometry.py')
def generate(check=False):
    v.RUN=RUN;sources=v.generate(build_only=True)
    def source(name):
     b=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    item=source('ObjType');decl=''.join('public '+t+' '+n+('='+x if x else '')+';'for t,n,x in fields.fields())
    methods=['public static class Opcodes','public final void decode(','private void decode(','public final void generateCertificate(','public final void generateLent(','public final RawModel getBodyModel(']
    (RUN/'ObjType.java').write_text('class ObjType{static final JagString DEFAULT_NAME=new JagString("null");'+decl+''.join(v.method(item,m)for m in methods)+'}'+heads.ITEM_SERVICE)
    p=RUN/'MovementSupport.java';p.write_text(p.read_text().replace('class LocalizedText {','class LocalizedText {static JagString TAKE=new JagString("Take"),DROP=new JagString("Drop"),LENT_ITEM_RETURN=new JagString("Discard");'))
    p=RUN/'Support.java';p.write_text(p.read_text().replace('class IdkTypeList {static Js5 modelsArchive;}','class IdkTypeList {static Js5 modelsArchive;static java.util.Map<Integer,IdkType>kits=new java.util.HashMap<>();static IdkType get(int id){return kits.get(id);}}'))
    p=RUN/'RawModel.java';p.write_text(p.read_text().rstrip()[:-1]+v.method(source('RawModel'),'public final void rotateXYZ(')+'}')
    player=source('PlayerAppearance');body=v.method(player,'public final Model getBodyModel(');body=body[body.index('RawModel[] models ='):body.index('model = combined.createModel')]
    prefix='class PlayerBody{int[]identikit,colors;boolean gender;int basId=-1;int[][]modelTransformCache;static short[]sourceBodyColors=AppearanceOracle.GAME0_SOURCE_BODY_COLORS,sourceSkinColors=AppearanceOracle.GAME0_SOURCE_SKIN_COLORS;static short[][]destinationBodyColors=AppearanceOracle.GAME0_DESTINATION_BODY_COLORS,destinationSkinColors=AppearanceOracle.GAME0_DESTINATION_SKIN_COLORS;RawModel body(int main,int off){int[] activeIdentikit=identikit.clone();if(main>=0)activeIdentikit[5]=main==65535?0:main|0x40000000;if(off>=0)activeIdentikit[3]=off==65535?0:off|0x40000000;int slot,translateX,translateY,translateZ,rotateX,rotateY,rotateZ,cosRotY,sinRotY,cosRotZ,sinRotZ,intermediate;'
    (RUN/'PlayerBody.java').write_text(prefix+body+'return combined;}}class BasTypeList{static java.util.Map<Integer,BasType>types=new java.util.HashMap<>();static BasType get(int id){return types.get(id);}}')
    (RUN/'BodyOracle.java').write_text(r'''
    import java.io.*;import java.nio.file.*;import java.util.*;
    class BodyOracle{
    static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
    static String result(RawModel m)throws Exception{if(m==null)return"null";ByteArrayOutputStream b=new ByteArrayOutputStream();VisualOracle.canonical(new DataOutputStream(b),m);return VisualOracle.digest(b.toByteArray());}
    public static void main(String[]args)throws Exception{Js5 archive=new Js5(Path.of(args[0]));IdkTypeList.modelsArchive=archive;ObjTypeList.modelsArchive=archive;BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\t");switch(p[0]){
    case"K":{IdkType t=new IdkType();t.decode(new Buffer(HexFormat.of().parseHex(p[2])));IdkTypeList.kits.put(Integer.parseInt(p[1]),t);System.out.println(result(t.getBodyModel()));break;}
    case"D":{ObjType t=new ObjType();t.id=Integer.parseInt(p[1]);t.decode(new Buffer(HexFormat.of().parseHex(p[2])));ObjTypeList.items.put(t.id,t);break;}
    case"B":{BasType t=new BasType();t.decode(new Buffer(HexFormat.of().parseHex(p[2])));BasTypeList.types.put(Integer.parseInt(p[1]),t);System.out.println(Arrays.deepToString(t.modelRotateTranslate));break;}
    case"I":System.out.println(result(ObjTypeList.get(Integer.parseInt(p[1])).getBodyModel(p[2].equals("1"))));break;
    case"P":{PlayerBody h=new PlayerBody();h.identikit=ints(p[1]);h.colors=ints(p[2]);h.gender=p[3].equals("1");h.basId=Integer.parseInt(p[4]);System.out.println(result(h.body(Integer.parseInt(p[5]),Integer.parseInt(p[6]))));break;}
    default:throw new AssertionError(p[0]);}}}}
    ''')
    c=subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],text=True,capture_output=True)
    if c.returncode:raise SystemExit(c.stderr)
    path=_recorded(ROOT,'ui-players.pack');pack=heads.pack(path);data=RUN/'data';data.mkdir(exist_ok=True)
    for name,b in pack.items():
     if name.startswith('model/'):(data/('model_'+name[6:]+'.bin')).write_bytes(b)
    cases=[];requests=[];kits=[];items=[];valid_bas=[]
    from sys import path as syspath
    syspath.insert(0,str(ROOT/'tools'))
    from prepare_visual import Reader
    def transforms(data):
     r=Reader(data);result=None
     while(op:=r.u1()):
      if op==1:r.take(4)
      elif 2<=op<=9 or op in [26,30,32,33,35,36] or 38<=op<=45:r.take(2)
      elif op==27:
       if result is None:result=[None]*12
       slot=r.u1();result[slot]=[int.from_bytes(r.take(2),'big',signed=True)for _ in range(6)]
      elif op in [29,31,34,37]:r.take(1)
      else:raise ValueError(op)
     return result
    for name,b in sorted(pack.items()):
     if name.startswith('kit/'):
      id=int(name[4:]);kits.append(id);cases.append(dict(kind='kit',id=id));requests.append(f'K\t{id}\t'+b.hex())
     if name.startswith('item/'):
      id=int(name[5:]);items.append(id);requests.append(f'D\t{id}\t'+b.hex())
    for name,b in sorted(pack.items()):
     if name.startswith('bas/'):
      id=int(name[4:]);cases.append(dict(kind='bas',id=id));requests.append(f'B\t{id}\t'+b.hex());t=transforms(b)
      if t is not None and all(0<=a<2048 for part in t if part for a in part[3:]):valid_bas.append(id)
    for id in items:
     for female in [False,True]:cases.append(dict(kind='item',id=id,female=female));requests.append(f'I\t{id}\t{int(female)}')
    # Synthetic valid BAS rows exercise every slot, rotation order, and wrapping translations.
    rng=random.Random(5301205)
    for id in range(2000,2032):
     b=bytearray()
     for slot in range(12):
      b.extend([27,slot])
      for n in [rng.choice([-32768,-127,0,128,32767])for _ in range(3)]+[rng.randrange(2048)for _ in range(3)]:b.extend(struct.pack('>h',n))
     b.append(0);cases.append(dict(kind='bas',id=id,raw=b.hex()));requests.append(f'B\t{id}\t'+b.hex());valid_bas.append(id)
    # Original kit null/empty/repeated-array behavior is separate from real cache coverage.
    for id,b in [(1000,b'\0'),(1001,b'\2\0\0'),(1002,b'\2\1\0\1\2\0\0')]:
     cases.append(dict(kind='kit',id=id,raw=b.hex()));requests.append(f'K\t{id}\t'+b.hex());pack[f'kit/{id}']=b
    
    for i in range(2048):
     slots=[rng.choice(kits)-0x80000000 if j%3==0 else(rng.choice(items)|0x40000000)if j%3==1 else 0 for j in range(12)];colors=[rng.randrange(16)for _ in range(5)];female=bool(i%2);bas=-1 if i%2==0 else rng.choice(valid_bas)
     overrides=[rng.choice([-1,65535,rng.choice(items)])for _ in range(2)]
     cases.append(dict(kind='player',slots=slots,colors=colors,female=female,bas=bas,overrides=overrides));requests.append('P\t'+','.join(map(str,slots))+'\t'+','.join(map(str,colors))+f'\t{int(female)}\t{bas}\t'+ '\t'.join(map(str,overrides)))
    p=subprocess.run(['java','-Xmx2g','-cp',str(RUN),'BodyOracle',str(data)],input='\n'.join(requests)+'\n',text=True,capture_output=True)
    if p.returncode:raise SystemExit(p.stderr)
    lines=p.stdout.splitlines();assert len(lines)==len(cases),(len(lines),len(cases))
    for c,line in zip(cases,lines):c['expected']=line
    out=dict(sources=sources,packSha256=heads.sha(path.read_bytes()),bindings='Complete original ObjType decode/template/getBodyModel, IdkType and RawModel decode/combine/recolor/retexture/translate/rotateXYZ. Unchanged PlayerAppearance raw body/BAS transform/colour assembly fragment with explicit sequence slot5/3 overrides; complete original BasType decoder. Ready raw assets, no body caches, NPC transform, slot-source masks/overlays, GPU animation or accelerated movement claims.',cases=cases)
    if check:
     if json.loads(FIX.read_text())!=out:raise SystemExit('Player-body fixture differs')
    else:FIX.write_text(json.dumps(out,separators=(',',':'))+'\n')
    print('Original raw bodies:',len(cases),'cases')
    

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
