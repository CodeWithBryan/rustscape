"""Unchanged Player.decodeAppearance with controlled equipment/palette bindings."""
import hashlib,json,random,subprocess
def generate(root,clean,method,check=False):
    import generate as inputs
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/appearance';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    player=source('Player');buffer=source('Buffer')
    methods=''.join(method(buffer,s)for s in ['public final int g1(', 'public final byte g1b(', 'public final int g2(', 'public final int g4(', 'public final long g8('])
    code=r'''import java.io.*;import java.util.*;
class Buffer{byte[]data;int offset;Buffer(byte[]b){data=b;} BUFFER }
class Parent{int size=1,xFine,yFine;int[]movementQueueX=new int[10],movementQueueY=new int[10];int getSize(){return size;}void setSize(int n){size=n;}}
class JagString{long value;JagString(long n){value=n;}JagString toTitleCase(){return this;}}class Base37{static JagString decode37(long n){return new JagString(n);}}
class Equipment{static int[]objIds=new int[32768];static{for(int i=0;i<objIds.length;i++)objIds[i]=1000+i;}}class ObjType{int team;}class ObjTypeList{static ObjType get(int id){ObjType t=new ObjType();t.team=id%7;return t;}}
class PlayerAppearance{static short[][]destinationBodyColors={new short[5],new short[6],new short[7],new short[8],new short[9]};int npcId=-1;int[]colors,identikit;boolean female;int bas;void set(int[] c,int id,boolean f,int[]k,int b){colors=c;npcId=id;female=f;identikit=k;bas=b;}}
class AreaSoundManager{static void remove(Player p){}}class ParticleSystem{void update(){}}
class Player extends Parent{int titleIndex,skullHeadIcon=-1,prayerHeadIcon=-1,team,basTypeId,combatLevel,skill,combatLevelWithSummoning,combatRange=-1,soundRadius,idleSound,crawlSound,walkSound,runSound;JagString username;PlayerAppearance appearance;ParticleSystem particleSystem;static void updateAreaSound(Player p){} APPEARANCE }
public class Probe{static String arr(int[]a){return Arrays.toString(a);}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){Player p=new Player();Buffer b=new Buffer(HexFormat.of().parseHex(line));p.decodeAppearance(b);System.out.println(p.size+","+p.titleIndex+","+p.skullHeadIcon+","+p.prayerHeadIcon+","+p.team+","+p.basTypeId+","+p.username.value+","+p.combatLevel+","+p.skill+","+p.combatLevelWithSummoning+","+p.combatRange+","+p.soundRadius+","+(p.appearance.female?1:0)+","+p.appearance.npcId+","+b.offset+";"+arr(p.appearance.identikit)+";"+arr(p.appearance.colors)+";"+(p.soundRadius==0?"null":arr(new int[]{p.idleSound,p.crawlSound,p.walkSound,p.runSound})));}}}
'''.replace('BUFFER',methods).replace('APPEARANCE',method(player,'public final void decodeAppearance('))
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530);cases=[dict(wire=(inputs.appearance(rng)+bytes(rng.randrange(256)for _ in range(rng.randrange(20)))).hex())for _ in range(1536)]
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(c['wire']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for case,line in zip(cases,result):
        scalars,kits,colors,sounds=line.split(';');case['expected']=[list(map(int,scalars.split(','))),json.loads(kits),json.loads(colors),json.loads(sounds)]
    p=root/'fixtures/players/appearance.json';content=json.dumps(dict(sources=hashes,bindings='Original full decodeAppearance; equipment index i maps to item1000+i with team=id%7; palette lengths5/6/7/8/9; Base37 callback records raw name37; area sound/particles/appearance-set model cache are explicit bindings. Actor fine-coordinate/NPC-size behavior is excluded.',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('Player appearance corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 full appearance reader: {len(cases)} cases')
