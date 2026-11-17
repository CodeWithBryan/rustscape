"""Unchanged GL masked bone and player BAS-basis arithmetic, with explicit primitive bindings."""
import hashlib,importlib.util,json,pathlib,random,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/players-oracle/masked';O.mkdir(parents=True,exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    raws={n:(v.SRC/(n+'.java')).read_bytes().replace(b'\r\n',b'\n')for n in['GlModel','PlayerAppearance','NpcType','MathUtils']};texts={n:v.clean(b.decode())for n,b in raws.items()}
    (O/'MathUtils.java').write_text(texts['MathUtils'],encoding='utf-8')
    fields='''class GlBuffer{boolean valid=true;}class GlModel{static int originX,originY,originZ;static boolean originValid;int[]vertexX,vertexY,vertexZ,vertexOffsets;short[]vertexLookup,vertexSources,triangleSources,normalX,normalY,normalZ,triangleColors;byte[]triangleAlpha;int[][]boneVertices,boneTriangles;GlBuffer colorBuffer=new GlBuffer(),normalsBuffer=new GlBuffer();'''
    (O/'GlModel.java').write_text(fields+v.method(texts['GlModel'],'protected final void transformMaskedBone(')+'}',encoding='utf-8')
    block=v.method(texts['PlayerAppearance'],'if (this.modelTransformCache[slotIndex] == null)')
    (O/'Basis.java').write_text('class Basis{int[][]modelTransformCache=new int[1][];int[]run(int[]p){int slotIndex=0,translateX=p[0],translateY=p[1],translateZ=p[2],rotateY=p[3],rotateX=p[4],rotateZ=p[5],cosRotY,sinRotY,cosRotZ,sinRotZ,intermediate;'+block+'return modelTransformCache[0];}}',encoding='utf-8')
    npc_block=v.method(texts['NpcType'],'if (this.modelTransformCache[i] == null)')
    (O/'NpcBasis.java').write_text('class NpcBasis{int[][]modelTransformCache=new int[1][];int[]run(int[]p){int i=0,frame=p[0],tweenFrame=p[1],translateZ=p[2],rotateX=p[3],rotateY=p[4],frameGroupId=p[5],frameDelay,slot,groupId,slotMask;'+npc_block+'return modelTransformCache[0];}}',encoding='utf-8')
    (O/'Probe.java').write_text(r'''
    import java.io.*;import java.util.*;
    public class Probe{
    static int[]arr(String s){return s.equals("~")?null:s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}static short[]shorts(int[]a){if(a==null)return null;short[]o=new short[a.length];for(int i=0;i<a.length;i++)o[i]=(short)a[i];return o;}static byte[]bytes(int[]a){if(a==null)return null;byte[]o=new byte[a.length];for(int i=0;i<a.length;i++)o[i]=(byte)a[i];return o;}static int[][]groups(int[]a){if(a==null)return null;int n=Arrays.stream(a).max().orElse(-1)+1;int[][]out=new int[n][];for(int i=0;i<n;i++){int v=i;out[i]=java.util.stream.IntStream.range(0,a.length).filter(j->a[j]==v).toArray();}return out;}
    static String unsigned(short[]a){if(a==null)return"null";return Arrays.toString(java.util.stream.IntStream.range(0,a.length).map(i->a[i]&65535).toArray());}static String unsigned(byte[]a){if(a==null)return"null";return Arrays.toString(java.util.stream.IntStream.range(0,a.length).map(i->a[i]&255).toArray());}
    static String snapshot(GlModel m){int[][]points=new int[m.vertexX.length][3];for(int i=0;i<points.length;i++)points[i]=new int[]{m.vertexX[i],m.vertexY[i],m.vertexZ[i]};int[][]normals=m.normalX==null?null:new int[m.normalX.length][3];if(normals!=null)for(int i=0;i<normals.length;i++)normals[i]=new int[]{m.normalX[i],m.normalY[i],m.normalZ[i]};return"["+Arrays.deepToString(points)+","+Arrays.toString(new int[]{GlModel.originX,GlModel.originY,GlModel.originZ})+","+GlModel.originValid+","+Arrays.deepToString(normals)+","+unsigned(m.triangleAlpha)+","+unsigned(m.triangleColors)+","+(!m.colorBuffer.valid)+","+(!m.normalsBuffer.valid)+"]";}
    public static void main(String[]unused)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){if(line.startsWith("N;")){System.out.println(Arrays.toString(new NpcBasis().run(arr(line.substring(2)))));continue;}if(line.startsWith("B;")){System.out.println(Arrays.toString(new Basis().run(arr(line.substring(2)))));continue;}String[]f=line.split(";",-1);GlModel m=new GlModel();int[]pos=arr(f[0]),labels=arr(f[1]),faces=arr(f[2]),norm=arr(f[6]),origin=arr(f[9]);m.vertexX=new int[labels.length];m.vertexY=new int[labels.length];m.vertexZ=new int[labels.length];for(int i=0;i<labels.length;i++){m.vertexX[i]=pos[i*3];m.vertexY[i]=pos[i*3+1];m.vertexZ[i]=pos[i*3+2];}m.boneVertices=groups(labels);m.boneTriangles=groups(arr(f[3]));m.vertexSources=shorts(arr(f[4]));m.triangleSources=shorts(arr(f[5]));m.triangleAlpha=bytes(arr(f[7]));m.triangleColors=shorts(arr(f[8]));if(norm!=null){m.normalX=new short[norm.length/3];m.normalY=new short[norm.length/3];m.normalZ=new short[norm.length/3];for(int i=0;i<m.normalX.length;i++){m.normalX[i]=(short)norm[i*3];m.normalY[i]=(short)norm[i*3+1];m.normalZ[i]=(short)norm[i*3+2];}}m.vertexOffsets=new int[labels.length+1];ArrayList<Short>lookup=new ArrayList<>();for(int i=0;i<labels.length;i++){m.vertexOffsets[i]=lookup.size();for(int j=0;j<faces.length;j++)if(faces[j]==i)lookup.add((short)(j+1));}m.vertexOffsets[labels.length]=lookup.size();m.vertexLookup=new short[lookup.size()];for(int i=0;i<lookup.size();i++)m.vertexLookup[i]=lookup.get(i);GlModel.originX=origin[0];GlModel.originY=origin[1];GlModel.originZ=origin[2];GlModel.originValid=origin[3]!=0;ArrayList<String>out=new ArrayList<>();for(String command:f[10].split("/",-1)){int[]c=arr(command);int n=c[6];int[]lab=Arrays.copyOfRange(c,7,7+n);int[]matrix=c[7+n]==0?null:Arrays.copyOfRange(c,8+n,23+n);m.transformMaskedBone(c[0],lab,c[1],c[2],c[3],c[4]!=0,c[5],matrix);out.add(snapshot(m));}System.out.println("["+String.join(",",out)+"]");}}}
    ''',encoding='utf-8')
    p=subprocess.run(['javac','-d',str(O),*[str(p)for p in O.glob('*.java')]],text=True,capture_output=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
    rng=random.Random(53012014);small=[-32768,-16384,-129,-1,0,1,127,128,255,16383,32767];coords=[-2147483648,-100000,-17,-1,0,1,16,10000,2147483647]
    bases=[dict(transform=[rng.choice(small)for _ in range(3)]+[rng.randrange(2048)for _ in range(3)])for _ in range(512)]
    for i in range(32):bases[i]['transform'][3:]=[0,0,0]
    p=subprocess.run(['java','-cp',str(O),'Probe'],input=''.join('B;'+','.join(map(str,c['transform']))+'\n'for c in bases),text=True,capture_output=True);(O/'basis-stdout.txt').write_text(p.stdout);(O/'basis-stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    for c,row in zip(bases,p.stdout.splitlines()):c['expected']=json.loads(row)
    p=subprocess.run(['java','-cp',str(O),'Probe'],input=''.join('N;'+','.join(map(str,c['transform']))+'\n'for c in bases),text=True,capture_output=True);(O/'npc-basis-stdout.txt').write_text(p.stdout);(O/'npc-basis-stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    for c,row in zip(bases,p.stdout.splitlines()):c['npcExpected']=json.loads(row)
    assert all('expected'in c and 'npcExpected'in c for c in bases)

    cases=[]
    for i in range(2048):
     c=dict(vertices=[[rng.choice(coords)if i%9==0 else rng.randrange(-8000,8000)for _ in range(3)]for _ in range(8)],vertexLabels=[rng.randrange(4)for _ in range(8)],faces=[[rng.randrange(8)for _ in range(3)]for _ in range(3)],faceLabels=None if i%11==0 else[rng.randrange(3)for _ in range(3)],vertexSources=None if i%7==0 else[rng.choice([0,1,2,4,8,16,255,32768,65535])for _ in range(8)],faceSources=None if i%5==0 else[rng.choice([0,1,2,4,8,65535])for _ in range(3)],normals=None if i%13==0 else[[rng.choice(small)for _ in range(3)]for _ in range(9)],alpha=None if i%17==0 else[rng.randrange(256)for _ in range(3)],colors=[rng.randrange(65536)for _ in range(3)],origin=[rng.choice(coords)for _ in range(3)]+[rng.randrange(2)],commands=[])
     for step in range(8):
      kind=[0,1,2,3,5,7,4,0][(i+step)%8];matrix=None if rng.randrange(3)==0 else rng.choice(bases)['expected'];xyz=[rng.randrange(2048)for _ in range(3)]if kind==2 else[rng.choice(small)for _ in range(3)]
      c['commands'].append(dict(kind=kind,xyz=xyz,normals=bool(rng.randrange(2)),mask=rng.choice([0,1,2,4,8,16,255,32768,65535]),labels=[rng.randrange(6)for _ in range(rng.randrange(5))],matrix=matrix))
     cases.append(c)
    def arr(a):return '~'if a is None else ','.join(map(str,a))
    def flat(a):return arr(None if a is None else[n for row in a for n in row])
    inputs=[]
    for c in cases:
     commands=[]
     for cmd in c['commands']:commands.append(arr([cmd['kind'],*cmd['xyz'],int(cmd['normals']),cmd['mask'],len(cmd['labels']),*cmd['labels'],int(cmd['matrix']is not None),*(cmd['matrix']or[])]))
     inputs.append(';'.join([flat(c['vertices']),arr(c['vertexLabels']),flat(c['faces']),arr(c['faceLabels']),arr(c['vertexSources']),arr(c['faceSources']),flat(c['normals']),arr(c['alpha']),arr(c['colors']),arr(c['origin']),'/'.join(commands)]))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(x)for x in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    out=dict(scope='Complete unchanged GlModel.transformMaskedBone and exact PlayerAppearance and NpcType BAS matrix-cache construction branches. Explicit primitive model/normal/index arrays and GPU invalidation records; valid topology/trig/matrix lengths. Full pose blending, raw assembly, cache/static identity and actual actor rendering remain separate.',sources={n+'.java':hashlib.sha256(b).hexdigest()for n,b in raws.items()},basisCases=bases,cases=cases)
    content=json.dumps(out,separators=(',',':'))+'\n';target=R/'fixtures/players/masked.json'
    if check:assert target.read_text(encoding='utf-8')==content,'Masked GL corpus differs'
    else:target.write_text(content,encoding='utf-8');print('Original masked GL',len(cases),'sequences',len(cases)*8,'steps and',len(bases),'BAS matrices',flush=True)

if __name__=='__main__':
    import sys
    generate('--check'in sys.argv)
