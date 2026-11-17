"""Run-only probe of unchanged original fixed-rate synthesized PCM playback."""
import hashlib,importlib.util,json,pathlib,random,subprocess,sys
ROOT=pathlib.Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
run=ROOT/'runs/audio-pcm-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
def source(name):
    raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
for name in ['Node','Sound','PcmSound','PcmStream','SoundPcmStream','PcmResampler']:
    (run/(name+'.java')).write_text(source(name),encoding='utf-8')
clip=v.method(source('JavaAudioChannel'),'protected final void write()')
support='''import java.io.*;import java.util.*;import java.lang.reflect.*;import java.security.*;
class GlobalConfig{static int AUDIO_SAMPLE_RATE=22050;}
class AudioChannel{static int sampleRate=22050;static boolean stereo;}
class DataLine{void write(byte[]b,int p,int n){}}
class JavaAudioChannel{int[]samples;byte[]outputBuffer=new byte[1024];DataLine dataLine=new DataLine();'''+clip+'''}
public class Probe{
 static SoundPcmStream stream;static int channels;static Field position,loops;
 static String hash(byte[]b)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(b));}
 static String hash(int[]n)throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();DataOutputStream d=new DataOutputStream(b);for(int x:n)d.writeInt(x);return hash(b.toByteArray());}
 static String state()throws Exception{return stream==null?"null":Arrays.toString(new int[]{position.getInt(stream),loops.getInt(stream),stream.isLinked()?1:0});}
 public static void main(String[]args)throws Exception{
 position=SoundPcmStream.class.getDeclaredField("samplePosition");position.setAccessible(true);loops=SoundPcmStream.class.getDeclaredField("loopCount");loops.setAccessible(true);
 BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);switch(p[0]){
 case"R":{int[]n=Arrays.stream(p[1].split(",")).mapToInt(Integer::parseInt).toArray();AudioChannel.stereo=n[4]!=0;channels=AudioChannel.stereo?2:1;byte[]b=HexFormat.of().parseHex(p[2]);stream=SoundPcmStream.create(new PcmSound(22050,b,n[0],n[1]),n[3]);if(stream!=null){stream.setLoops(n[2]);Node root=new Node();root.nextNode=stream;root.previousNode=stream;stream.nextNode=root;stream.previousNode=root;}break;}
 case"V":if(stream!=null)stream.setVolume(Integer.parseInt(p[1]));break;
 case"T":{int n=Integer.parseInt(p[1]),offset=Integer.parseInt(p[2]),fill=Integer.parseInt(p[3]);int[]b=new int[(n+offset+3)*channels];Arrays.fill(b,fill);if(stream!=null&&stream.isLinked())stream.read(b,offset,n);System.out.println("[\\\""+hash(b)+"\\\","+state()+"]");break;}
 case"C":{JavaAudioChannel a=new JavaAudioChannel();AudioChannel.stereo=Boolean.parseBoolean(p[1]);a.samples=Arrays.stream(p[2].split(",")).mapToInt(Integer::parseInt).toArray();a.write();System.out.println("\\\""+hash(Arrays.copyOf(a.outputBuffer,a.samples.length*2))+"\\\"");break;}
 default:throw new IllegalArgumentException(p[0]);}}}}
'''
# Java string literals above have JSON quotes, not literal backslashes.
support=support.replace('\\\\\\"','\\"')
(run/'Probe.java').write_text(support,encoding='utf-8')
subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
rng=random.Random(53022050);scenarios=[];requests=[]
for i in range(192):
    length=rng.randrange(0,129);pcm=rng.randbytes(length).hex();args=[rng.randrange(length+1),rng.randrange(length+1),rng.choice([-2147483648,-2,-1,0,1,2,100,2147483647]),rng.choice([0,1,127,255]),i%2]
    requests.append('R;'+','.join(map(str,args))+';'+pcm);steps=[]
    for j in range(16):
        volume=rng.choice([0,1,32,127,255]);n=rng.choice([0,1,2,7,16,64,256]);offset=rng.randrange(4);fill=rng.choice([0,-1,2147483647,-2147483648,rng.randrange(-2147483648,2147483648)])
        steps.append(dict(volume=volume,frames=n,offset=offset,fill=fill));requests+=['V;'+str(volume),f'T;{n};{offset};{fill}']
    scenarios.append(dict(pcm=pcm,args=args,steps=steps))
clips=[]
for stereo in [False,True]:
    for _ in range(64):
        n=[rng.choice([-2147483648,-8388609,-8388608,-1,0,1,8388607,8388608,2147483647,rng.randrange(-2147483648,2147483648)])for _ in range(512 if stereo else 256)]
        clips.append(dict(samples=n,stereo=stereo));requests.append('C;'+str(stereo).lower()+';'+','.join(map(str,n)))
output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==192*16+128
it=iter(output)
for c in scenarios:
    for s in c['steps']:s['expected']=json.loads(next(it))
for c in clips:c['expected']=json.loads(next(it))
data=dict(sources=sources,bindings='Complete unchanged SoundPcmStream/PcmSound/PcmStream/Node/PcmResampler. Synthesized PCM rate22050, forward integer step256, centered pan8192, no fades/pingpong; explicit stream linkage, mutable volume, finite/infinite loops, mono/stereo and prefilled buffers. Original JavaAudioChannel.write records clipped16bit output. No music/fractional-step/fade or hardware claim.',streams=scenarios,clips=clips)
path=ROOT/'fixtures/audio/pcm.json';content=(json.dumps(data,separators=(',',':'))+'\n').encode()
if '--check'in sys.argv:
    if path.read_bytes()!=content:raise SystemExit('Original PCM corpus differs')
else:path.write_bytes(content)
print('Original PCM probe:',192*16,'stream observations and',len(clips),'clip buffers')
