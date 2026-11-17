"""Complete original SoundPlayer queue with explicit synth/cache/mixer/MIDI bindings."""
import hashlib,json,random,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4/SoundPlayer.java';raw=src.read_bytes().replace(b'\r\n',b'\n');code=clean(raw.decode()).replace('Math.random()','Probe.random')
    protocol=(src.parent/'Protocol.java').read_bytes().replace(b'\r\n',b'\n');buffer=(src.parent/'Buffer.java').read_bytes().replace(b'\r\n',b'\n')
    synth=method(clean(protocol.decode()),'if (opcode == ServerProt.SYNTH_SOUND)')
    run=root/'runs/players-oracle/sound';run.mkdir(parents=True,exist_ok=True);(run/'SoundPlayer.java').write_text(code,encoding='utf-8')
    support=r'''import java.io.*;import java.util.*;
class Preferences{static int soundEffectVolume=127,ambientSoundsVolume=127,musicVolume=0;}
class SeqType{int[][]soundeffect;}
class PcmSound{int id;PcmSound(int i){id=i;}PcmSound resample(Object a){return this;}}
class SynthSound{int id;SynthSound(int i){id=i;}int getStart(){return id%6;}PcmSound toPcmSound(){return new PcmSound(id);}static SynthSound create(Object a,int id,int ignored){return id%11==0?null:new SynthSound(id);}}
class SoundPcmStream{int id,volume,loops;static SoundPcmStream create(PcmSound s,int v){SoundPcmStream p=new SoundPcmStream();p.id=s.id;p.volume=v;return p;}void setLoops(int n){loops=n;}}
class Mixer{void addSubStream(SoundPcmStream s){Probe.played.add(Arrays.toString(new int[]{s.id,s.loops+1,s.volume}));}}
class Player{int xFine,yFine;}class PlayerList{static Player self=new Player();}
class MidiPlayer{static boolean jingle=false;static boolean isPlaying(){return false;}static void playImmediate(Object a,int b,int c){}}class MusicPlayer{static int groupId=-1;}
class client{static Object js5Archive4,resampler,js5Archive6;static Mixer soundStream=new Mixer();}
class Outbound{void p1isaac(int a){}void p4(int a){}}class Protocol{static Outbound outboundBuffer=new Outbound();}class ClientProt{static int SOUND_SONGEND;}
class Buffer{byte[]data;int offset;Buffer(byte[]b){data=b;} BUFFER }class ServerProt{static int SYNTH_SOUND=172;}
public class Probe{static Buffer inboundBuffer;static int opcode;static boolean synth(byte[]b){inboundBuffer=new Buffer(b);opcode=172; SYNTH return false;}static double random;static List<String>played=new ArrayList<>();static String snapshot(){List<String>q=new ArrayList<>();for(int i=0;i<SoundPlayer.size;i++)q.add("["+SoundPlayer.ids[i]+","+SoundPlayer.loops[i]+","+SoundPlayer.delays[i]+","+SoundPlayer.positions[i]+","+(SoundPlayer.sounds[i]==null?"null":SoundPlayer.sounds[i].getStart())+"]");return "{\"entries\":"+q+",\"played\":"+played+"}";}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]a=line.split(";");switch(a[0]){
case"R":SoundPlayer.size=0;played.clear();break;
case"V":Preferences.soundEffectVolume=Integer.parseInt(a[1]);Preferences.ambientSoundsVolume=Integer.parseInt(a[2]);PlayerList.self.xFine=Integer.parseInt(a[3]);PlayerList.self.yFine=Integer.parseInt(a[4]);break;
case"Q":SoundPlayer.play(Integer.parseInt(a[1]),Integer.parseInt(a[2]),Integer.parseInt(a[3]));break;
case"B":synth(HexFormat.of().parseHex(a[1]));break;
case"S":{SeqType s=new SeqType();String[]v=a[1].split(",");int[]sounds=new int[v.length];for(int i=0;i<v.length;i++)sounds[i]=Integer.parseInt(v[i]);s.soundeffect=new int[][]{sounds};random=Double.parseDouble(a[6]);SoundPlayer.playSeqSound(Integer.parseInt(a[3]),s,Integer.parseInt(a[2]),Boolean.parseBoolean(a[4]),Integer.parseInt(a[5]));break;}
case"T":SoundPlayer.loop();System.out.println(snapshot());played.clear();break;
}}}}
'''.replace('BUFFER',method(clean(buffer.decode()),'public final int g1(')+method(clean(buffer.decode()),'public final int g2(')).replace('SYNTH return',synth+' return')
    (run/'Probe.java').write_text(support,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'SoundPlayer.java'),str(run/'Probe.java')],check=True)
    rng=random.Random(53050);cases=[];inputs=[]
    for scenario in range(32):
        inputs.append('R');commands=[]
        for tick in range(128):
            c=['V',rng.choice([0,1,32,127,255]),rng.choice([0,1,64,127,255]),rng.randrange(104)*128+64,rng.randrange(104)*128+64];commands.append(c);inputs.append(';'.join(map(str,c)))
            for _ in range(rng.randrange(1,7)if tick%11 else 70):
                kind=rng.randrange(3)
                if kind==0:
                    wire=rng.choice([65535,rng.randrange(100)]).to_bytes(2,'big')+bytes([rng.randrange(8)])+rng.randrange(15).to_bytes(2,'big');c=['B',wire.hex()];commands.append(c);inputs.append(';'.join(c))
                elif kind==1:
                    c=['Q',rng.randrange(8),rng.randrange(-1,101),rng.randrange(-11,9)];commands.append(c);inputs.append(';'.join(map(str,c)))
                else:
                    sounds=[(rng.randrange(100)<<8)|(rng.randrange(8)<<5)|rng.randrange(5)]+[rng.randrange(100)for _ in range(rng.randrange(4))]
                    c=['S',sounds,rng.randrange(104)*128+64,rng.randrange(104)*128+64,bool(rng.randrange(2)),rng.choice([0,0,0,1,5]),rng.random()];commands.append(c);inputs.append(';'.join(map(str,['S',','.join(map(str,sounds)),c[2],c[3],str(c[4]).lower(),c[5],c[6]])))
            commands.append(['T']);inputs.append('T')
        cases.append(dict(commands=commands))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==4096
    it=iter(result)
    for c in cases:c['expected']=[json.loads(next(it))for _ in range(128)]
    path=root/'fixtures/players/sound.json';content=json.dumps(dict(sources={'SoundPlayer':hashlib.sha256(raw).hexdigest(),'Protocol':hashlib.sha256(protocol).hexdigest(),'Buffer':hashlib.sha256(buffer).hexdigest()},bindings='Unchanged full SoundPlayer and Protocol.SYNTH_SOUND/Buffer reads; Math.random replaced by explicit Probe.random; synth readiness missing for id%11==0, start trim=id%6, mixer records id/loops/volume. MIDI callbacks disabled explicitly. PCM synthesis/finite loops compared separately; no hardware-output claim.',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Sound queue corpus differs')
    else:path.write_bytes(content)
    print(f'Original complete finite-effect/sequence sound queue: {len(cases)} scenarios / {len(result)} ticks')
