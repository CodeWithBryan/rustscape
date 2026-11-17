"""Unchanged SynthSound.getStart and SoundPcmStream finite mono loops."""
import hashlib,json,pathlib,random,struct,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/audio';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    for name in ['SynthSound','SynthInstrument','SynthEnvelope','SynthFilter','ArrayUtils','Node','Sound','PcmSound','PcmStream','SoundPcmStream']:(run/(name+'.java')).write_text(source(name),encoding='utf-8')
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer {byte[] data;int offset;Buffer(byte[] data){this.data=data;}'+''.join(method(buffer,s)for s in ['public final int g1()', 'public final int g2()', 'public final int g4()', 'public final int gsmart()', 'public final int gsmarts()'])+'}',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class GlobalConfig{static int AUDIO_SAMPLE_RATE=22050;}class AudioChannel{static int sampleRate=22050;static boolean stereo=false;}class Js5{byte[] fetchFile(int a,int b){return null;}}class PcmResampler{byte[] resample(byte[] b){return b;}int scaleRate(int n){return n;}int scalePosition(int n){return n;}}
public class Probe{static String hash(byte[] b)throws Exception{return HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(b));}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){SynthSound s=new SynthSound(new Buffer(HexFormat.of().parseHex(line)));int ticks=s.getStart();PcmSound p=s.toPcmSound();String out=ticks+";"+p.start+";"+p.end+";"+hash(p.samples);for(int loops:new int[]{1,2,3,8}){int n=p.samples.length+(p.samples.length>0&&p.start<p.end?(p.end-p.start)*(loops-1):0);try{int[] mix=new int[n+32];SoundPcmStream stream=SoundPcmStream.create(p,1);if(stream!=null){stream.setLoops(loops-1);stream.read(mix,0,mix.length);}byte[] bytes=new byte[mix.length];for(int i=0;i<mix.length;i++){if((mix[i]&255)!=0)throw new AssertionError("non-integral sample scaling");bytes[i]=(byte)(mix[i]>>8);}out+=";"+hash(bytes);}catch(ArrayIndexOutOfBoundsException e){out+=";error";}}System.out.println(out);}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    data=(root/'assets/visual/ui.pack').read_bytes();n=struct.unpack_from('<I',data,8)[0];manifest=json.loads(data[12:12+n]);start=12+n;names=[k for k in manifest['files']if k.startswith('sound/')];rng=random.Random(530);names=sorted(set([f'sound/{i}'for i in [0,1,2,3,4,5,6,7,8,9,10,127,223,224,380,385,386,387]]+rng.sample(names,16)));cases=[]
    for name in names:
        offset,size=manifest['files'][name];cases.append(dict(name=name,raw=data[start+offset:start+offset+size].hex()))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(c['raw']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for c,line in zip(cases,result):
        ticks,a,b,digest,*loops=line.split(';');c.update(ticks=int(ticks),start=int(a),end=int(b),sha256=digest,loops=loops)
    p=root/'fixtures/ui/audio.json';content=json.dumps(dict(sources=hashes,bindings='22050Hz mono, unit mixer volume, no resampling/fades/ping-pong; finite loops1/2/3/8 plus32 trailing zeros',cases=cases),separators=(',',':')).encode()+b'\n'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI sound corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 queue-trimmed sound and finite loops: {len(cases)} effects')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
