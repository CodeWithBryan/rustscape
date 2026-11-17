"""Original non-accelerated BAS decoding and movement/facing selection.

No original methods are rewritten. Cached and synthetic BAS inputs cover
directional runs, slow walks, ID aliases and duplicate/ignored opcodes.
"""
import argparse,hashlib,importlib.util,json,pathlib,random,re,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];SRC=ROOT/'reference/client/client/src/main/java/rt4';RUN=ROOT/'runs/bas-motion-oracle';FIX=ROOT/'fixtures/players/bas-motion.json'
spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)

def generate(check=False):
 RUN.mkdir(parents=True,exist_ok=True);hashes={}
 def source(name):
  raw=(SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
 bas=source('BasType');path=source('PathingEntity');npc=source('NpcList');buffer=source('Buffer')
 def fields(s):return '\n'.join(re.findall(r'(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;',s)).replace('private ','public ').replace('protected ','public ')
 code=r'''import java.io.*;import java.util.*;
class Buffer{byte[]data;int offset;Buffer(byte[]b){data=b;} BUFFER }
class BasType{ BAS DECODE }
class PathingEntity{ FIELDS BasType bas;int getSize(){return size;}BasType getBasType(){return bas;}}
class Player extends PathingEntity{}class Npc extends PathingEntity{NpcType type=new NpcType();}class NpcType{boolean rotationflag=true;}
class PlayerList{static int selfId=17;static Player[]players=new Player[2048];}class Camera{static int originX,originY;}
class SeqType{int looptype,movetype;}class SeqTypeList{static SeqType get(int id){throw new IllegalStateException("primary action unavailable");}}
class NpcList{static Npc[]npcs=new Npc[32768]; PHASES }
public class Probe{
static String snapshot(PathingEntity p){return Arrays.toString(new int[]{p.xFine,p.yFine,p.currentAngle,p.targetAngle,p.movementSeqId,p.movementCatchupTicks,p.standingTurnCounter,p.movementQueueSize});}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]a=line.split(";",-1);Buffer data=new Buffer(HexFormat.of().parseHex(a[1]));BasType b=new BasType();b.decode(data);
if(a[0].equals("D")){System.out.println(Arrays.toString(new int[]{b.idleAnimationId,b.walkAnimation,b.runAnimationId,b.standingCCWTurn,b.standingCWTurn,b.walkFullTurnAnimationId,b.walkCCWTurnAnimationId,b.walkCWTurnAnimationId,b.slowWalkAnimationId,b.slowWalkFullTurnAnimationId,b.slowWalkCCWTurnAnimationId,b.slowWalkCWTurnAnimationId,b.runFullTurnAnimationId,b.runCCWTurnAnimationId,b.runCWTurnAnimationId,b.terrainAlignWidth,b.terrainAlignDepth}));continue;}
int[]c=Arrays.stream(a[2].split(",")).mapToInt(Integer::parseInt).toArray();PathingEntity p=c[0]!=0?new Npc():new Player();if(p instanceof Npc)((Npc)p).type.rotationflag=c[1]!=0;p.bas=b;p.size=c[2];p.currentAngle=c[3];p.turnSpeed=c[4];p.faceEntity=c[5];p.movementCatchupTicks=c[6];p.xFine=c[7];p.yFine=c[8];String[]q=a[3].isEmpty()?new String[0]:a[3].split("/");p.movementQueueSize=q.length;for(int i=0;i<q.length;i++){int[]e=Arrays.stream(q[i].split(",")).mapToInt(Integer::parseInt).toArray();int at=q.length-i-1;p.movementQueueX[at]=e[0];p.movementQueueY[at]=e[1];p.movementQueueSpeed[at]=(byte)e[2];}
List<String>states=new ArrayList<>();for(int i=0;i<Integer.parseInt(a[4]);i++){NpcList.processMovement(p);NpcList.processFacing(p);states.add(snapshot(p));}System.out.println(states);}}}
'''.replace('BUFFER',''.join(v.method(buffer,s)for s in ['public final int g1(', 'public final int g2(', 'public final int g2b('])).replace(' BAS ',fields(bas[:bas.index('public final void postDecode(')])).replace('DECODE',v.method(bas,'public final void decode(')+v.method(bas,'private void decode(')).replace(' FIELDS ',fields(path[:path.index('public final BasType getBasType(')])).replace(' PHASES ',''.join(v.method(npc,s)for s in ['public static void processMovement(', 'public static void processFacing(']))
 (RUN/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),str(RUN/'Probe.java')],check=True)
 pack=ROOT/'assets/visual/ui-players.pack';raw=pack.read_bytes();size=struct.unpack_from('<I',raw,8)[0];manifest=json.loads(raw[12:12+size]);offset=12+size
 decoded=[]
 for name,(start,length)in sorted(manifest['files'].items()):
  if name.startswith('bas/'):decoded.append(dict(id=int(name[4:]),bytes=raw[offset+start:offset+start+length].hex()))
 rng=random.Random(530_7009);synthetic=[]
 for case in range(96):
  data=bytearray()
  for op in [1,2,3,4,5,6,7,8,9,26,38,39,40,41,42,43,44,45,6]:
   data.append(op)
   if op==26:data.extend([rng.randrange(256),rng.randrange(256)])
   else:
    for _ in range(2 if op==1 else 1):data.extend(struct.pack('>H',rng.choice([0,1,2,3,4,5,6,7,65535])))
  data.append(0);synthetic.append(dict(id=-1-case,bytes=data.hex()))
 decoded+=synthetic
 directional=[c for c in decoded if c['id']in [3,116,1205,1422,1423,1424]]
 motions=[]
 for config in synthetic+directional:
  for scenario in range(8 if config['id']<0 else 32):
   size=rng.randrange(1,5);head=[50,50];count=rng.choice([0,1,2,3,4,9]);queue=[];kind=rng.randrange(3)
   for i in range(count):
    head=[head[0]+rng.choice([-1,0,1]),head[1]+rng.choice([-1,0,1])];queue.append([*head,kind if i==0 else rng.randrange(3)])
   fields=[scenario%2,int(scenario%3!=0),size,rng.choice([0,256,512,768,1024,1280,1536,1792,2047]),rng.choice([0,32,64,1024]),-1 if scenario%4 else 8,rng.randrange(4),50*128+size*64,50*128+size*64]
   motions.append(dict(bas=config['bytes'],fields=fields,queue=queue,ticks=24))
 requests=['D;'+c['bytes']for c in decoded]+[';'.join(['M',c['bas'],','.join(map(str,c['fields'])),'/'.join(','.join(map(str,e))for e in c['queue']),str(c['ticks'])])for c in motions]
 result=subprocess.run(['java','-cp',str(RUN),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True);rows=[json.loads(line)for line in result.stdout.splitlines()];assert len(rows)==len(requests)
 for c,row in zip(decoded,rows):c['expected']=row
 for c,row in zip(motions,rows[len(decoded):]):c['expected']=row
 content=json.dumps(dict(sources=hashes,packSha256=hashlib.sha256(raw).hexdigest(),scope='Complete unchanged BasType decode and NpcList.processMovement/processFacing;1431 pinned cached BAS plus96 synthetic non-accelerated configurations. Movement uses explicit queues, size, rotation flags and primitive fields, no primary action or target registry. Covers slow/directional-run ID aliases and duplicated opcode6; animated geometry, acceleration, body transforms, full BAS rendering and authoritative gameplay remain separate.',cachedCount=1431,decoded=decoded,motions=motions),separators=(',',':')).encode()+b'\n'
 if check:
  if FIX.read_bytes()!=content:raise SystemExit('BAS motion fixture differs')
 else:FIX.write_bytes(content)
 print(f'Original BAS decode:{len(decoded)} configs; movement/facing:{len(motions)} scenarios/{sum(c["ticks"]for c in motions)} ticks')
 spec=importlib.util.spec_from_file_location('npc_assets',ROOT/'oracle/players/npc_assets.py');assets=importlib.util.module_from_spec(spec);spec.loader.exec_module(assets)
 assets.generate(ROOT,v,check,roots=[132],folder='bas-assets',include_movement=True)

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');generate(parser.parse_args().check)
