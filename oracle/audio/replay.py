"""Replay actual Windows audio commands through the pinned original PCM player."""
import argparse,hashlib,json,pathlib,subprocess,sys,wave
ROOT=pathlib.Path(__file__).resolve().parents[2]
p=argparse.ArgumentParser(description=__doc__);p.add_argument('--commands',type=pathlib.Path,required=True);p.add_argument('--wav',type=pathlib.Path,required=True);p.add_argument('--binary',type=pathlib.Path);p.add_argument('--output',type=pathlib.Path);a=p.parse_args()
subprocess.run([sys.executable,str(ROOT/'oracle/audio/pcm.py'),'--check'],cwd=ROOT,check=True)
run=ROOT/'runs/audio-replay-oracle';run.mkdir(parents=True,exist_ok=True)
(run/'Replay.java').write_text('''import java.io.*;import java.util.*;import java.security.*;
public class Replay{
 static Map<Long,SoundPcmStream>streams=new LinkedHashMap<>();static MessageDigest digest;static long frames;
 static void mix(int count){if(count%256!=0)throw new AssertionError("native256-frame blocks required");while(count>0){int[]out=new int[512];for(SoundPcmStream s:streams.values())if(s.isLinked())s.read(out,0,256);JavaAudioChannel c=new JavaAudioChannel();c.samples=out;c.write();digest.update(c.outputBuffer);frames+=256;count-=256;}}
 public static void main(String[]args)throws Exception{AudioChannel.stereo=true;AudioChannel.sampleRate=22050;digest=MessageDigest.getInstance("SHA-256");BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);switch(p[0]){
 case"M":mix(Integer.parseInt(p[1]));break;
 case"P":{long id=Long.parseLong(p[1]);PcmSound pcm=new PcmSound(22050,HexFormat.of().parseHex(p[6]),Integer.parseInt(p[2]),Integer.parseInt(p[3]));SoundPcmStream s=SoundPcmStream.create(pcm,Integer.parseInt(p[5]));if(s!=null){s.setLoops(Integer.parseInt(p[4]));Node root=new Node();root.nextNode=s;root.previousNode=s;s.previousNode=root;s.nextNode=root;streams.put(id,s);}break;}
 case"V":{SoundPcmStream s=streams.get(Long.parseLong(p[1]));if(s!=null)s.setVolume(Integer.parseInt(p[2]));break;}
 case"X":{SoundPcmStream s=streams.get(Long.parseLong(p[1]));if(s!=null)s.unlink();break;}
 default:throw new AssertionError(p[0]);}}System.out.println(frames+";"+HexFormat.of().formatHex(digest.digest()));}}
''',encoding='utf-8')
classes=ROOT/'runs/audio-pcm-oracle';subprocess.run(['javac','-encoding','UTF-8','-cp',str(classes),'-d',str(run),str(run/'Replay.java')],check=True)
commands=json.loads(a.commands.read_text());assert commands['rate']==22050 and commands['channels']==2;cursor=0;requests=[]
for c in commands['commands']:
    assert cursor<=c['frame']<=commands['frames']
    requests.append('M;'+str(c['frame']-cursor));cursor=c['frame']
    if c['op']=='play':requests.append(';'.join(map(str,['P',c['id'],c['start'],c['end'],c['loops'],c['volume'],bytes(v&255 for v in c['samples']).hex()])))
    elif c['op']=='volume':requests.append(f"V;{c['id']};{c['volume']}")
    elif c['op']=='stop':requests.append(f"X;{c['id']}")
    else:raise AssertionError(c['op'])
requests.append('M;'+str(commands['frames']-cursor))
result=subprocess.run(['java','-cp',str(run)+';'+str(classes),'Replay'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.strip();frames,original=result.split(';')
with wave.open(str(a.wav),'rb')as w:
    assert w.getnchannels()==2 and w.getsampwidth()==2 and w.getframerate()==22050 and w.getnframes()==commands['frames']==int(frames);data=w.readframes(w.getnframes())
actual=hashlib.sha256(data).hexdigest();ok=actual==original
def entry(path):return dict(path=str(path.resolve()),bytes=path.stat().st_size,sha256=hashlib.sha256(path.read_bytes()).hexdigest())
report=dict(scope='Actual app-owned Windows submitted PCM replayed with unchanged SoundPcmStream and JavaAudioChannel.write at22050Hz/stereo. Native captured PCM is the explicit sample input; raw cache synthesis is compared separately. Music, fades, pitch, priority scheduling and hardware/device loopback remain outside this claim.',sources=json.loads((ROOT/'fixtures/audio/pcm.json').read_text())['sources'],commands=entry(a.commands),wav=entry(a.wav),binary=entry(a.binary)if a.binary else None,frames=int(frames),commandCount=len(commands['commands']),plays=sum(c['op']=='play'for c in commands['commands']),pcmBytes=len(data),originalSha256=original,nativeSha256=actual,match=ok)
output=a.output or a.wav.with_name('replay-report.json');output.write_text(json.dumps(report,indent=2)+'\n')
print(json.dumps({k:report[k]for k in['frames','commandCount','plays','pcmBytes','originalSha256','nativeSha256','match']}))
if not ok:raise SystemExit('Actual Windows submitted PCM differs from original player')
