"""Pinned SpotAnimType decode/constructModel, SeqType poses and actor attachment."""
import argparse,hashlib,importlib.util,json,pathlib,random,re,shutil,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/players-spot-oracle';FIX=ROOT/'fixtures/players/spots'
ROOTS=[92,93,111,184,369,457,503,1000]
def module(name,path):
 s=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
def generate(check=False):
 v=module('spot_visual',ROOT/'oracle/visual/generate.py');sources=v.generate(build_only=True);RUN.mkdir(parents=True,exist_ok=True);lib=RUN/'lib';lib.mkdir(exist_ok=True)
 for p in v.RUN.glob('*.java'):shutil.copyfile(p,lib/p.name)
 def source(name):
  b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
 spot=source('SpotAnimType');seq=source('SeqType');frames=source('AnimFrameset');gl=source('GlModel');entity=source('PathingEntity');npc=source('Npc');player=source('Player')
 (lib/'SpotAnimType.java').write_text(re.sub(r'\bModel\b','GlModel',spot).replace('SeqTypeList','SpotSeqTypeList'),encoding='utf-8')
 p=lib/'SeqType.java';s=p.read_text();s=s.rstrip()[:-1]+re.sub(r'\bModel\b','GlModel',v.method(seq,'public Model animateSpotAnim(')).replace('SeqTypeList','SpotSeqTypeList')+'}';p.write_text(s,encoding='utf-8')
 p=lib/'Support.java';s=p.read_text();old='class AnimFrameset {AnimFrame[] frames;AnimFrameset(AnimFrame frame){frames=new AnimFrame[]{frame};}}';assert old in s
 s=s.replace(old,'class AnimFrameset {AnimFrame[] frames;AnimFrameset(AnimFrame... frames){this.frames=frames;}'+v.method(frames,'public final boolean isAlphaTransformed(')+v.method(frames,'public final boolean isColorTransformed(')+'}');p.write_text(s,encoding='utf-8')
 p=lib/'GlModel.java';s=p.read_text().rstrip()[:-1]+v.method(gl,'public final void rotateX(')+v.method(gl,'public final void rotateZ(')+'GlModel copyForAnimation(boolean a,boolean c,boolean n){return CharacterShadowOracle.copy(this);}}';p.write_text(s,encoding='utf-8')
 # Face provenance is diagnostic metadata only, as in the existing GL oracle.
 p=lib/'RawModel.java';s=p.read_text();s=s.replace('GlModel createModel(int a,int c,int x,int y,int z){','GlModel createModel(int a,int c,int x,int y,int z){this.triangleSources=new short[this.triangleCount];for(int i=0;i<this.triangleCount;i++)this.triangleSources[i]=(short)i;');p.write_text(s,encoding='utf-8')
 alignment=v.method(entity,'protected final void alignToTerrain(').replace('Model model','GlModel model').replace('SceneGraph.getTileHeight','SpotGround.height').replace('Player.plane','SpotGround.plane')
 attach=[]
 for name,code,marker,var in [('Npc',npc,'if (this.spotAnimId != -1 && this.spotAnimFrame != -1)','model'),('Player',player,'if (!this.lowDetail && this.spotAnimId != -1 && this.spotAnimFrame != -1)','shadowModel')]:
  body=v.method(code,marker).replace('PathingEntity.terrain','SpotEntity.terrain');attach.append('GlModel attach'+name+'(){GlModel '+var+'=null;'+body+'return '+var+';}')
 (lib/'SpotBindings.java').write_text('''
class SpotModels {java.util.Map<Integer,GlModel>values=new java.util.HashMap<>();Object get(int id){return values.get(id);}void put(Object value,int id){values.put(id,(GlModel)value);}}
class SpotAnimTypeList {static Js5 modelsArchive;static SpotModels models=new SpotModels();static java.util.Map<Integer,SpotAnimType>types=new java.util.HashMap<>();static SpotAnimType get(int id){return types.get(id);}}
class SpotSeqTypeList {static java.util.Map<Integer,SeqType>types=new java.util.HashMap<>();static java.util.Map<Integer,AnimFrameset>groups=new java.util.HashMap<>();static SeqType get(int id){return types.get(id);}static AnimFrameset getAnimFrameset(int id){return groups.get(id);}}
class SpotGround {static int plane,index;static int[]heights;static java.util.List<int[]>calls=new java.util.ArrayList<>();static int height(int p,int x,int y){calls.add(new int[]{p,x,y});return heights[index++];}}
class SpotEntity {static int terrainPitchAngle,terrainRollAngle,terrainYOffset;int xFine,yFine,tileHeight;BasType bas=new BasType();BasType getBasType(){return bas;}
'''+alignment+'}\nclass SpotAttachment {int spotAnimId,spotAnimFrame,spotAnimNextFrame,spotAnimDelayClock,spotAnimY;boolean lowDetail;'+''.join(attach)+'}',encoding='utf-8')
 (lib/'SpotProbe.java').write_text(pathlib.Path(__file__).with_name('SpotProbe.java').read_text().replace('SeqTypeList','SpotSeqTypeList'),encoding='utf-8')
 subprocess.run(['javac','-encoding','UTF-8','-d',str(lib),*[str(p)for p in lib.glob('*.java')]],check=True)
 p=module('spot_prepare',ROOT/'tools/prepare_visual.py');cache=p.Cache(ROOT/'assets/cache');inputs={};cases=[];requests=[]
 def save(name,data):
  file=name.replace('/','_')+'.bin';inputs[file]=data;(RUN/file).write_bytes(data);return file
 save('materials',cache.file(26,0));raw={}
 for group,files in cache.index(21)[1].items():
  for id in files:raw[group*256+id]=cache.file(21,group,id)
 rng=random.Random(530021)
 decoded=[dict(id=id,raw=b.hex())for id,b in sorted(raw.items())]
 for i in range(256):
  b=bytearray()
  for _ in range(rng.randrange(1,18)):
   op=rng.choice([1,2,4,5,6,7,8,9,40,41,254]);b.append(op)
   if op in[1,2,4,5,6]:b.extend(struct.pack('>H',rng.choice([0,1,90,128,180,255,270,65535])))
   elif op in[7,8]:b.append(rng.randrange(256))
   elif op in[40,41]:
    n=rng.randrange(4);b.append(n)
    for _ in range(n):b.extend(struct.pack('>HH',rng.randrange(65536),rng.randrange(65536)))
  b.append(0);decoded.append(dict(id=-i-1,raw=b.hex()))
 requests=['D;'+c['raw']for c in decoded]
 result=subprocess.run(['java','-cp',str(lib),'SpotProbe',str(RUN)],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True)
 for c,line in zip(decoded,result.stdout.splitlines()):c['expected']=json.loads(line)
 assert len(result.stdout.splitlines())==len(decoded)
 definitions={c['id']:c['expected']['definition']for c in decoded if c['id']>=0};requests=[]
 cues=[]
 for id in ROOTS:
  d=definitions[id];file=save(f'spot/{id}',raw[id]);save(f'model/{d["model"]}',cache.file(7,d['model']));sid=d['sequence'];sfile=save(f'seq/{sid}',cache.file(20,sid>>7,sid&127));r=p.Reader(inputs[sfile])
  # Earlier metadata can precede the complete main frame table.
  while (op:=r.u1())!=1:
   if op in [2,6,7]:r.take(2)
   elif op in [5,8,9,10,11]:r.take(1)
   elif op in [4,14,15,16]:pass
   elif op==3:r.take(r.u1())
   elif op==12:r.take(r.u1()*4)
   else:raise ValueError((sid,op))
  n=r.u2();delays=[r.u2()for _ in range(n)];lows=[r.u2()for _ in range(n)];highs=[r.u2()for _ in range(n)]
  cues.append(dict(spot=id,sequence=sid,file=sfile))
  for frame in sorted({0,n//2,n-1}):
   nxt=frame+1 if frame+1<n else -1;framespec=[]
   for index in[frame,nxt]:
    if index<0:framespec+=['-',-1];continue
    f=save(f'frame/{highs[index]}/{lows[index]}',cache.file(0,highs[index],lows[index]));bid=int.from_bytes(inputs[f][:2],'big');save(f'base/{bid}',cache.file(1,bid));framespec +=[f,bid]
   for delta in sorted({0,delays[frame]//2,delays[frame]-1}):
    for tween in[False,True]:
     for attach in['Npc','Player']:
      height=rng.choice([0,50,100,-100]);alignment=[rng.randrange(2048),rng.randrange(2048),rng.randrange(-120,121)];case=dict(id=f'{id}_{frame}_{delta}_{tween}_{attach}',type=id,file=file,seq=sfile,frame=frame,next=nxt,first=framespec[0],first_base=framespec[1],second=framespec[2],second_base=framespec[3],delta=delta,tween=tween,attach=attach,height=height,terrain=alignment);cases.append(case);requests.append(';'.join(map(str,['P',id,file,sfile,frame,nxt,*framespec,delta,str(tween).lower(),attach,height,*alignment])))
  # Exercise the same real body with all supported/ignored orientation values,
  # nonuniform resize and terrain flag on the unchanged construction methods.
  for angle in[0,90,180,270,360,65535]:
   variant=raw[id][:-1]+bytes([4])+struct.pack('>H',192)+bytes([5])+struct.pack('>H',96)+bytes([6])+struct.pack('>H',angle)+bytes([9,0]);vf=save(f'spot/{id}_variant_{angle}',variant)
   case=dict(id=f'{id}_variant_{angle}',type=id,file=vf,seq=sfile,frame=-1,next=-1,first='-',first_base=-1,second='-',second_base=-1,delta=0,tween=False,attach='Npc',height=100,terrain=[64,2000,-33]);cases.append(case);requests.append(';'.join(map(str,['P',id,vf,sfile,-1,-1,'-',-1,'-',-1,0,'false','Npc',100,64,2000,-33])))
 for c in cues:requests.append('S;'+c['file'])
 alignments=[]
 for i in range(512):
  c=dict(dimensions=[rng.choice([0,4,32,64,128,1020]),rng.choice([0,4,32,64,128,1020])],yaw=rng.randrange(2048),fine=[rng.randrange(-10000,20000),rng.randrange(-10000,20000)],tile_height=rng.randrange(-2000,1000),heights=[rng.randrange(-3000,3000)for _ in range(4)]);alignments.append(c);requests.append('A;'+','.join(map(str,[*c['dimensions'],c['yaw'],*c['fine'],c['tile_height'],*c['heights']])))
 primitives=[]
 for resize in[[128,128],[128,0],[0,128],[129,128],[65535,255],[0,0]]:
  for angle in[0,90,180,270,360,65535]:
   b=bytes([4])+struct.pack('>H',resize[0])+bytes([5])+struct.pack('>H',resize[1])+bytes([6])+struct.pack('>H',angle)+bytes([0]);vertices=[[2147483647,-2147483648,-1],[-2147483648,1,2147483647],[16777216,-16777217,0],[127,128,-129]];primitives.append(dict(raw=b.hex(),vertices=vertices));requests.append('T;'+b.hex()+';'+','.join(map(str,[v for row in vertices for v in row])))
 (RUN/'requests.tsv').write_text('\n'.join(requests)+'\n')
 result=subprocess.run(['java','-cp',str(lib),'SpotProbe',str(RUN)],input='\n'.join(requests)+'\n',text=True,capture_output=True)
 if result.returncode:raise RuntimeError('Spot request '+str(len(result.stdout.splitlines()))+': '+result.stderr)
 lines=iter(result.stdout.splitlines())
 for c in cases:c['expected']=next(lines)
 for c in cues:c['expected']=json.loads(next(lines))
 for c in alignments:c['expected']=json.loads(next(lines))
 for c in primitives:c['expected']=json.loads(next(lines))
 assert next(lines,None)is None
 data=json.dumps(dict(sources=sources,scope='Unchanged SpotAnimType decode/constructModel, SeqType.animateSpotAnim and original GL geometry/normals/colors/alpha, plus unchanged Npc/Player spot attachment branches and PathingEntity terrain sampling. Original SeqType sound cue observations for selected cached graphics. Explicit immutable model/sequence/frame cache services, diagnostic face provenance and clone adapter. No particles, floor graphics/projectiles, device rendering or authoritative effects claim.',roots=ROOTS,decoded=decoded,cases=cases,cues=cues,alignments=alignments,primitives=primitives,inputs={n:hashlib.sha256(b).hexdigest()for n,b in inputs.items()}),separators=(',',':')).encode()+b'\n'
 if check:
  assert(FIX/'oracle.json').read_bytes()==data,'Spot fixture differs'
  for n,b in inputs.items():assert(FIX/n).read_bytes()==b,n
 else:
  FIX.mkdir(parents=True,exist_ok=True)
  for n,b in inputs.items():(FIX/n).write_bytes(b)
  (FIX/'oracle.json').write_bytes(data)
 print('Original spot graphics:',len(raw),'cached +256 synthetic definitions;',len(cases),'GL poses/attachments;',len(alignments),'terrain samples;',len(primitives),'extreme transforms',flush=True)
 synth=module('spot_synthesis',ROOT/'oracle/audio/synthesis.py');synth.generate(check,ids={2574},name='spot-synthesis',bindings='Real sound2574 referenced by cached spot457/sequence2789/frame7, original SynthSound/instrument/filter/envelope synthesis before and after getStart at22050Hz. Sequence cue provenance is recorded separately by the original SeqType decoder. No music, combat or device loopback claim.')
if __name__=='__main__':
 a=argparse.ArgumentParser(description=__doc__);a.add_argument('--check',action='store_true');generate(a.parse_args().check)
