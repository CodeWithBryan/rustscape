"""Run unchanged server ChunkFlags against bounded sparse flag-store adapters."""
import argparse,hashlib,importlib.util,json,os,pathlib,random,subprocess,urllib.request
ROOT=pathlib.Path(__file__).resolve().parents[2];HERE=pathlib.Path(__file__).resolve().parent;RUN=ROOT/'runs/clipping-oracle';SERVER=ROOT/'reference/server/Server/src/main'
spec=importlib.util.spec_from_file_location('compiler',ROOT/'oracle/login/generate.py');c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c);read,run=c.read,c.run

def requests():
 cases=[];rng=random.Random(530)
 def op(name,**kw):return dict(op=name,**kw)
 def add(actions,**kw):cases.append(dict(actions=actions,**kw))
 for shape in [-1,0,1,2,3,4]:
  for rotation in [-1,0,1,2,3,4]:
   for projectile in [False,True]:
    for clipped in [False,True]:
     for x,y in [(0,0),(0,7),(7,0),(7,7),(3,4),(-1,8),(8,-1)]:
      add([op('door',x=x,y=y,shape=shape,rotation=rotation,clipped=clipped),op('door',x=x,y=y,shape=shape,rotation=rotation,clipped=clipped,add=False),op('get',x=x,y=y)],projectile=projectile)
 for projectile in [False,True]:
  for x,y in [(0,0),(-1,8),(8,-1),(8,0),(-1,7),(7,8)]:
   for mask in [-2**31,-1,0,1,0x82,0x1ffffff]:
    add([op('get',x=x,y=y),op('addFlag',x=x,y=y,mask=mask),op('removeFlag',x=x,y=y,mask=mask),op('flag',x=x,y=y,mask=mask),op('flag',x=x,y=y,mask=mask,add=False),op('clear',x=x,y=y),op('invalidate',x=x,y=y)],projectile=projectile)
 for sx in [-1,0,1,2,10]:
  for sy in [-1,0,1,2,10]:
   for clipped in [False,True]:
    for projectile in [False,True]:
     add([op('solid',x=7,y=7,sx=sx,sy=sy,clipped=clipped),op('solid',x=7,y=7,sx=sx,sy=sy,clipped=clipped,add=False)],projectile=projectile)
 for event in ['flags','border']:
  for action in [op('fail'),op('invalidate',x=0,y=0),op('flag',x=1,y=1,mask=-1),op('door',x=2,y=3,shape=2,rotation=1,clipped=True),op('members',value=True)]:
   add([op('door',x=0,y=0,shape=2,rotation=3,clipped=True),op('tile',x=7,y=7),op('tile',x=7,y=7,add=False)],hooks=[dict(event=event,at=2,actions=[action])])
 for base in [[-1,-1,3],[2**31-1,-2**31,2**31-1],[-16384,16383,-1],[3207,3217,4]]:
  add([op('door',x=0,y=7,shape=0,rotation=1,clipped=True),op('empty',x=0,y=0),op('solidTile',x=7,y=7),op('members',value=True),op('landscape',value=None)],base=base)
 for _ in range(50):
  actions=[]
  for _ in range(20):
   name=rng.choice(['get','flag','addFlag','removeFlag','clear','invalidate','door','solid','tile','empty','solidTile','members'])
   actions.append(op(name,x=rng.randrange(-2,10),y=rng.randrange(-2,10),mask=rng.choice([-1,0,1,128,256,2147483647]),rotation=rng.randrange(-1,5),shape=rng.randrange(-1,5),clipped=bool(rng.randrange(2)),add=bool(rng.randrange(2)),sx=rng.randrange(-1,5),sy=rng.randrange(-1,5),value=bool(rng.randrange(2))))
  add(actions,projectile=bool(rng.randrange(2)))
 return cases

def generate(check):
 lock=json.loads(read(ROOT/'upstream.lock.json'))
 for part in ['server','client']:
  p=ROOT/'reference'/part
  if run(['git','-C',p,'rev-parse','HEAD']).stdout.decode().strip()!=lock[part]['commit']:raise RuntimeError('Reference pin differs')
  run(['git','-C',p,'diff','--quiet','HEAD','--'])
 RUN.mkdir(parents=True,exist_ok=True);classes=RUN/'classes';classes.mkdir(exist_ok=True);jars=c.artifacts();std=next(p for p in jars if p.name.startswith('kotlin-stdlib-'));jar=RUN/'json-simple-1.1.1.jar';artifact=json.loads(read(ROOT/'oracle/auth/json.lock.json'))
 if not jar.exists()or hashlib.sha256(jar.read_bytes()).hexdigest()!=artifact['sha256']:
  with urllib.request.urlopen(artifact['url'],timeout=60)as r:data=r.read()
  if len(data)!=artifact['size']or hashlib.sha256(data).hexdigest()!=artifact['sha256']:raise RuntimeError('JSON artifact differs')
  jar.write_bytes(data)
 java=[]
 def adapter(name,text):
  p=RUN/'adapters'/name;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text,encoding='utf-8');java.append(p)
 adapter('core/game/world/map/RegionChunk.java','package core.game.world.map;public class RegionChunk{public static final int SIZE=8;}')
 adapter('core/game/world/map/RegionManager.java','package core.game.world.map;public class RegionManager{public static Integer[] getFlags(int key,boolean p){return oracle.Support.flags(key,p);}public static void addClippingFlag(int z,int x,int y,boolean p,int mask){oracle.Support.border(z,x,y,p,mask,true);}public static void removeClippingFlag(int z,int x,int y,boolean p,int mask){oracle.Support.border(z,x,y,p,mask,false);}}')
 p=SERVER/'core/game/world/map/build/ChunkFlags.java';read(p);java.append(p);read(HERE/'Support.java');java.append(HERE/'Support.java');cp=os.pathsep.join(map(str,[classes,std,jar]));run(['javac','--release','11','-encoding','UTF-8','-cp',cp,'-d',classes,*java])
 cases=requests();file=RUN/'input.jsonl';file.write_text(''.join(json.dumps(x)+'\n'for x in cases),encoding='utf-8');exe=c.JAVA_HOME/'bin'/('java.exe'if os.name=='nt'else'java')if c.JAVA_HOME else'java'
 with file.open('rb')as f:p=subprocess.run(list(map(str,[exe,'-Dfile.encoding=UTF-8','-cp',cp,'oracle.Support'])),cwd=ROOT,stdin=f,capture_output=True,timeout=120)
 if p.returncode:raise RuntimeError(p.stderr.decode(errors='replace'))
 out=[json.loads(line)for line in p.stdout.splitlines()if line];assert len(out)==len(cases)
 data=''.join(json.dumps(x,sort_keys=True,separators=(',',':'))+'\n'for x in out).encode();read(pathlib.Path(__file__));read(ROOT/'oracle/login/generate.py')
 provenance=dict(format=1,upstream=lock,sources=dict(sorted(c.SOURCES.items())),caseCount=len(cases),fixtureSha256=hashlib.sha256(data).hexdigest(),sourceHashAlgorithm='sha256, CR removed',scope='Complete unchanged server ChunkFlags.java with explicit sparse getFlags and border-region routing adapters. Tests compare all fields, flags, movement/projectile defaults, call ordering and partial callback failures. Actual RegionManager/region loading, LandscapeParser and scene integration are separate.')
 folder=ROOT/'fixtures/clipping'
 if check:
  if(folder/'oracle.jsonl').read_bytes()!=data or json.loads((folder/'provenance.json').read_text(encoding='utf-8'))!=provenance:raise RuntimeError('Clipping fixtures/provenance differ')
  print(f'{len(out)} original server-clipping scenarios reproduced.')
 else:
  folder.mkdir(parents=True,exist_ok=True);(folder/'oracle.jsonl').write_bytes(data);(folder/'provenance.json').write_text(json.dumps(provenance,sort_keys=True,indent=2)+'\n',encoding='utf-8');print(f'Generated {len(out)} original server-clipping scenarios.')
if __name__=='__main__':
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--java-home',type=pathlib.Path);a=p.parse_args();c.JAVA_HOME=a.java_home or(pathlib.Path(os.environ['RS09_ORACLE_JAVA_HOME'])if'RS09_ORACLE_JAVA_HOME'in os.environ else None);generate(a.check)
