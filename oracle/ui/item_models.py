"""Original ObjType.getModel GL geometry, palette and post-construction resize."""
import argparse,hashlib,importlib.util,json,pathlib,re,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/ui-item-models-oracle';FIX=ROOT/'fixtures/ui/item-models.json'
def module(name,path):
    s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
v=module('v',ROOT/'oracle/visual/generate.py');fields=module('fields',ROOT/'tools/port_client_items.py');heads=module('heads',ROOT/'oracle/ui/model_geometry.py')
def generate(check=False):
    v.RUN=RUN;sources=v.generate(build_only=True)
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    item=source('ObjType');decl=''.join('public '+t+' '+n+('='+value if value else '')+';'for t,n,value in fields.fields())
    methods=['public static class Opcodes','public final void decode(','private void decode(','public final void generateCertificate(','public final void generateLent(','public final ObjType getCountVariant(','public final Model getModel(']
    code='class ObjType{static final JagString DEFAULT_NAME=new JagString("null");'+decl+''.join(v.method(item,m)for m in methods)+'}'
    code=re.sub(r'\bModel\b','GlModel',code).replace('RawModel.create(','RawModel.createTagged(')
    (RUN/'ObjType.java').write_text(code+SERVICE,encoding='utf-8')
    p=RUN/'MovementSupport.java';p.write_text(p.read_text().replace('class LocalizedText {','class LocalizedText {static JagString TAKE=new JagString("Take"),DROP=new JagString("Drop"),LENT_ITEM_RETURN=new JagString("Discard");'))
    p=RUN/'Support.java';p.write_text(p.read_text().replace('class GlRenderer {','class GlRenderer {static boolean enabled=true;'))
    p=RUN/'LocClock.java';p.write_text(p.read_text().replace('class client {','class client {static short[]objRecolorPalette=new short[256];'))
    p=RUN/'SeqType.java';p.write_text(p.read_text().replace('public final class SeqType {','public final class SeqType {GlModel animateEntity(GlModel m,int a,int b,int c){throw new UnsupportedOperationException("Animation independently tested");}'))
    p=RUN/'GlModel.java';g=source('GlModel');p.write_text(p.read_text().rstrip()[:-1]+'public boolean pickable;void uploadBuffers(boolean a,boolean b,boolean c,boolean d,boolean e,boolean f){}'+v.method(g,'public final int getMinY()')+'}')
    p=RUN/'RawModel.java';p.write_text(p.read_text().rstrip()[:-1]+'static RawModel createTagged(Js5 a,int id){RawModel m=create(a,id);if(m!=null){m.triangleSources=new short[m.triangleCount];for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;}return m;}}')
    (RUN/'ItemOracle.java').write_text(SUPPORT,encoding='utf-8')
    compiled=subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),*[str(p)for p in RUN.glob('*.java')]],capture_output=True,text=True)
    if compiled.returncode:raise SystemExit(compiled.stderr)
    ui_path=ROOT/'assets/visual/ui-models.pack';world_path=ROOT/'assets/visual/live/lumbridge-minimap.pack';ui=heads.pack(ui_path);world=heads.pack(world_path);data=RUN/'data';data.mkdir(exist_ok=True)
    for name,raw in ui.items():
        if name.startswith('model/'):(data/('model_'+name[6:]+'.bin')).write_bytes(raw)
    (data/'materials.bin').write_bytes(world['materials']);requests=[];cases=[]
    for name,raw in sorted(ui.items()):
        if name.startswith('item/'):requests.append('D\t'+name[5:]+'\t'+raw.hex())
    ids=sorted(int(n[5:])for n in ui if n.startswith('item/'))
    for id in ids:
        # Both routes occur in Component.getModel and the objId drawing branch.
        for count in [1,10]:cases.append(dict(id=id,count=count));requests.append(f'Q\t{id}\t{count}')
    process=subprocess.run(['java','-Xmx2g','-cp',str(RUN),'ItemOracle',str(data)],input='\n'.join(requests)+'\n',text=True,capture_output=True)
    if process.returncode:raise SystemExit(process.stderr)
    result=process.stdout.splitlines();assert len(result)==len(cases)
    for case,line in zip(cases,result):case['expected']=json.loads(line)
    out=dict(sources=sources,uiSha256=heads.sha(ui_path.read_bytes()),worldSha256=heads.sha(world_path.read_bytes()),palette=[(i*79+1234)&65535 for i in range(256)],bindings='Complete original ObjType raw decode/template/count/getModel methods, GlModel constructor/resize/getMinY, pinned material provider. Class name Model is normalized to GlModel in the wrapper. Metadata/model readiness, no-op GL upload and null animation are explicit services; animation is independently tested. Complete geometry/surface hash includes colors, textures, normals, UVs, original face order and short bounds after GL resize. Cache services retain original result identities per item.',cases=cases)
    if check:
        if json.loads(FIX.read_text())!=out:raise SystemExit('Item widget fixture differs')
    else:FIX.write_text(json.dumps(out,separators=(',',':'))+'\n')
    print('Original item-widget models:',len(cases),'cases')
SERVICE=r'''class ItemModelCache{java.util.Map<Long,Object>m=new java.util.HashMap<>();Object get(long id){return m.get(id);}void put(Object v,long id){m.put(id,v);}}
class ObjTypeList{static Js5 modelsArchive;static ItemModelCache models=new ItemModelCache();static java.util.Map<Integer,ObjType>items=new java.util.HashMap<>();static java.util.Set<Integer>done=new java.util.HashSet<>();static ObjType get(int id){ObjType i=items.computeIfAbsent(id,k->new ObjType());if(done.add(id)){if(i.certtemplate!=-1)i.generateCertificate(get(i.certlink),get(i.certtemplate));if(i.lentTemplate!=-1)i.generateLent(get(i.lentLink),get(i.lentTemplate));}return i;}}
'''
SUPPORT=r'''
import java.io.*;import java.nio.file.*;import java.util.*;
public class ItemOracle{static String digest(ObjType item,GlModel g)throws Exception{ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeInt(g.vertexCount);for(int i=0;i<g.vertexCount;i++){out.writeInt(g.vertexX[i]);out.writeInt(g.vertexY[i]);out.writeInt(g.vertexZ[i]);}
RawModel raw=RawModel.create(ObjTypeList.modelsArchive,item.model);out.writeInt(raw.triangleCount);int[]lookup=new int[raw.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++)lookup[g.triangleSources[i]&65535]=i;
for(int i=0;i<raw.triangleCount;i++){int index=lookup[i];out.writeBoolean(index>=0);if(index<0)continue;out.writeShort(g.triangleColors[index]);out.writeInt(g.triangleTextures[index]);out.writeByte(g.triangleAlpha[index]);for(int n:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){out.writeInt(g.normalX[n]);out.writeInt(g.normalY[n]);out.writeInt(g.normalZ[n]);out.writeInt(g.normalMagnitude[n]);out.writeFloat(g.vertexS[n]);out.writeFloat(g.vertexT[n]);}}
out.writeInt(g.triangleCount);for(int i=0;i<g.triangleCount;i++)out.writeInt(g.triangleSources[i]&65535);return VisualOracle.digest(bytes.toByteArray());}
public static void main(String[]args)throws Exception{ObjTypeList.modelsArchive=new Js5(Path.of(args[0]));Rasteriser.textureProvider=new VisualMaterials(Files.readAllBytes(Path.of(args[0]).resolve("materials.bin")));client.objRecolorPalette=new short[256];for(int i=0;i<256;i++)client.objRecolorPalette[i]=(short)(i*79+1234);BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\t");if(p[0].equals("D")){ObjType t=new ObjType();t.id=Integer.parseInt(p[1]);t.decode(new Buffer(HexFormat.of().parseHex(p[2])));ObjTypeList.items.put(t.id,t);continue;}ObjType t=ObjTypeList.get(Integer.parseInt(p[1])).getCountVariant(Integer.parseInt(p[2]));RawModel raw=RawModel.create(ObjTypeList.modelsArchive,t.model);if(raw==null){System.out.println("null");continue;}
GlModel g=t.getModel(-1,0,null,1,0);System.out.println("["+t.id+","+(t.ambient+64)+","+(t.contrast+768)+","+(-g.getMinY()/2)+",\""+digest(t,g)+"\"]");}}
}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
