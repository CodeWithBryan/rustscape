"""Original sparse RawModel assembly and complete masked/dual GL calls on cached bodies."""
import hashlib, importlib.util, json, pathlib, random, subprocess

def generate(check=False):
    root=pathlib.Path(__file__).resolve().parents[2]
    spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    sources=v.generate(build_only=True)
    run=root/'runs/players-oracle/parts';run.mkdir(parents=True,exist_ok=True)
    for path in v.RUN.glob('*.java'):
        text=path.read_text(encoding='utf-8')
        if path.name=='GlModel.java':
            old='void transformMaskedBone(int t,int[] labels,int x,int y,int z,boolean a,int mask,int[] parts){throw new UnsupportedOperationException("masked transforms outside corpus");}'
            original=v.clean((v.SRC/'GlModel.java').read_text(encoding='utf-8'))
            assert old in text
            text=text.replace(old,v.method(original,'protected final void transformMaskedBone('))
            model=v.clean((v.SRC/'Model.java').read_text(encoding='utf-8'))
            text=text[:-1]+v.method(model,'public final void applyMaskedAnimation(')+'}'
        (run/path.name).write_text(text,encoding='utf-8')
    code=r'''import java.io.*;import java.nio.file.*;import java.security.*;import java.util.*;
public class PartsProbe{
static int[] ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static String hash(byte[]b)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(b));}
static String raw(RawModel m)throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();DataOutputStream o=new DataOutputStream(b);o.writeInt(m.vertexCount);o.writeInt(m.triangleCount);o.writeInt(m.texturedCount);for(int i=0;i<m.vertexCount;i++){o.writeInt(m.vertexX[i]);o.writeInt(m.vertexY[i]);o.writeInt(m.vertexZ[i]);o.writeInt(m.vertexBones[i]);o.writeShort(m.vertexSources[i]);}for(int i=0;i<m.triangleCount;i++){o.writeInt(m.triangleVertexA[i]);o.writeInt(m.triangleVertexB[i]);o.writeInt(m.triangleVertexC[i]);o.writeShort(m.triangleSources[i]);}for(int i=0;i<m.texturedCount;i++){o.writeInt(m.textureFacesP[i]&65535);o.writeInt(m.textureFacesM[i]&65535);o.writeInt(m.textureFacesN[i]&65535);}return hash(b.toByteArray());}
static String pose(RawModel m,GlModel g,int[]lookup)throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();DataOutputStream o=new DataOutputStream(b);for(int i=0;i<m.vertexCount;i++){o.writeInt(g.vertexX[i]);o.writeInt(g.vertexY[i]);o.writeInt(g.vertexZ[i]);}for(int i=0;i<m.triangleCount;i++){int index=lookup[i];if(index<0)continue;for(int n:new int[]{g.triangleVertexA[index]&65535,g.triangleVertexB[index]&65535,g.triangleVertexC[index]&65535}){o.writeInt(g.normalX[n]);o.writeInt(g.normalY[n]);o.writeInt(g.normalZ[n]);o.writeInt(g.normalMagnitude[n]);}}for(int i=0;i<m.triangleCount;i++){int n=lookup[i];if(n>=0){o.writeShort(g.triangleColors[n]);o.writeByte(g.triangleAlpha[n]);}}o.writeBoolean(GlModel.originValid);return hash(b.toByteArray());}
public static void main(String[]args)throws Exception{Path dir=Path.of(args[0]);Rasteriser.textureProvider=new VisualMaterials(Files.readAllBytes(dir.resolve("materials.bin")));BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1),files=p[0].split(",",-1);RawModel[]parts=new RawModel[files.length];for(int i=0;i<parts.length;i++)if(!files[i].equals("~"))parts[i]=new RawModel(Files.readAllBytes(dir.resolve(files[i])));RawModel m=new RawModel(parts,parts.length);String raw=raw(m);short[]source=m.triangleSources.clone();for(int i=0;i<m.triangleCount;i++)m.triangleSources[i]=(short)i;GlModel g=new GlModel(m,64,850,true);g.createBones();int[]lookup=new int[m.triangleCount];Arrays.fill(lookup,-1);for(int i=0;i<g.triangleCount;i++){int face=g.triangleSources[i]&65535;lookup[face]=i;g.triangleSources[i]=source[face];}AnimBase base=new AnimBase(Integer.parseInt(p[1]),Files.readAllBytes(dir.resolve(p[2])));if(!p[3].equals("~"))System.arraycopy(ints(p[3]),0,base.parts,0,base.parts.length);AnimFrame a=new AnimFrame(Files.readAllBytes(dir.resolve(p[4])),base),b=new AnimFrame(Files.readAllBytes(dir.resolve(p[5])),base),c=new AnimFrame(Files.readAllBytes(dir.resolve(p[8])),base),d=new AnimFrame(Files.readAllBytes(dir.resolve(p[9])),base);GlModel.originValid=Boolean.parseBoolean(p[13]);ArrayList<String>out=new ArrayList<>();for(String command:p[14].split("/")){String[]x=command.split(":");int[]matrix=x[1].equals("~")?null:ints(x[1]);g.applyMaskedAnimation(new AnimFrameset(a),0,Boolean.parseBoolean(x[2])?new AnimFrameset(b):null,0,Integer.parseInt(p[6]),Integer.parseInt(p[7]),Integer.parseInt(x[0]),Boolean.parseBoolean(p[12]),matrix);out.add("\""+pose(m,g,lookup)+"\"");}boolean[]mask=null;if(!p[11].equals("~")){mask=new boolean[256];for(int i=0;i<256;i++)mask[i]=p[11].charAt(i)=='1';}g.applyDualAnimation(new AnimFrameset(a),0,new AnimFrameset(b),0,Integer.parseInt(p[6]),Integer.parseInt(p[7]),new AnimFrameset(c),0,new AnimFrameset(d),0,Integer.parseInt(p[10].split(",")[0]),Integer.parseInt(p[10].split(",")[1]),mask,Boolean.parseBoolean(p[12]));out.add("\""+pose(m,g,lookup)+"\"");System.out.println("{\"raw\":\""+raw+"\",\"poses\":["+String.join(",",out)+"]}");}}}
'''
    (run/'PartsProbe.java').write_text(code,encoding='utf-8')
    p=subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],text=True,capture_output=True);(run/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
    original=json.loads((root/'fixtures/visual/oracle.json').read_text());poses=[c for c in original['cases']if c['kind']=='glpose']
    matrices=json.loads((root/'fixtures/players/masked.json').read_text())['basisCases'];rng=random.Random(5302391);cases=[];inputs=[]
    for i in range(512):
        a=rng.choice(poses);b=rng.choice([c for c in poses if c['base_id']==a['base_id']]);files=[None]*([12,12,34,40][i%4]);positions=sorted(rng.sample(range(len(files)),len(a['files'])))
        for slot,file in zip(positions,a['files']):files[slot]=file
        if i%5==0:files[-1]=next(f for f in files if f is not None)
        base=(root/'fixtures/visual'/a['base']).read_bytes();n=base[0];parts=None if i%3 else[rng.choice([0,1,2,3,4,255,32768,65535])for _ in range(n)]
        overlays=[dict(mask=rng.choice([0,1,2,4,1<<positions[0],32768,65535])&65535,matrix=None if j%3==0 else matrices[(i+j)%512]['expected'],tween=bool((i+j)%2))for j in range(1+i%4)]
        runtime=i%4<2
        if runtime:
            for j,o in enumerate(overlays):o.update(slot=[0,2,5,11][j],mask=1<<[0,2,5,11][j],tween=True)
        mask=None if i%7==0 else[bool(rng.randrange(2))for _ in range(256)]
        c=dict(runtime=runtime,runtimeDelay=i%17,files=files,first=a,second=b,parts=parts,overlays=overlays,mask=mask,normals=bool(i%2),originValid=bool(i%3));cases.append(c)
        inputs.append(';'.join([','.join(f or'~'for f in files),str(a['base_id']),a['base'],'~'if parts is None else','.join(map(str,parts)),a['first'],a['next'],str(a['delta']),str(a['length']),b['first'],b['next'],str(b['delta'])+','+str(b['length']),'~'if mask is None else''.join('1'if n else'0'for n in mask),str(c['normals']).lower(),str(c['originValid']).lower(),'/'.join(str(o['mask'])+':'+('~'if o['matrix']is None else','.join(map(str,o['matrix'])))+':'+str(o['tween']).lower()for o in overlays)]))
    p=subprocess.run(['java','-cp',str(run),'PartsProbe',str(root/'fixtures/visual')],input='\n'.join(inputs)+'\n',text=True,capture_output=True);(run/'stdout.txt').write_text(p.stdout);(run/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(row)for row in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    names={'materials.bin'}
    for c in cases:
        names.update(f for f in c['files']if f is not None)
        for p in[c['first'],c['second']]:names.update([p['base'],p['first'],p['next']])
    out=dict(sources=sources,inputs={n:hashlib.sha256((root/'fixtures/visual'/n).read_bytes()).hexdigest()for n in sorted(names)},scope='Complete unchanged sparse RawModel constructor/addVertex; complete GL masked animation and full dual/tween calls on real cache geometry. Primitive matrix/base-part inputs are authored; original full geometry/normal/material preparation and per-call rounding retained. Triangle IDs are used only during construction to observe and restore original reordered source bits before animation. Cache ownership, malformed input failures and live actor dispatch are separate.',cases=cases)
    content=json.dumps(out,separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/parts.json'
    if check:assert path.read_bytes()==content,'Part pose corpus differs'
    else:path.write_bytes(content)
    print('Original sparse bodies and masked/full poses:',len(cases),sum(len(c['overlays'])+1 for c in cases),'completed animation calls',flush=True)

if __name__=='__main__':
    import sys
    generate('--check'in sys.argv)
