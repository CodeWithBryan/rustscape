"""Original per-return active-varp snapshots driving ordered ambient morph updates."""
import argparse,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def generate(check=False):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    spec=importlib.util.spec_from_file_location('d',ROOT/'oracle/ui/delayed.py');d=importlib.util.module_from_spec(spec);spec.loader.exec_module(d);d.generate(ROOT,v.clean,v.method,True)
    classes=ROOT/'runs/ui-oracle/delayed';run=ROOT/'runs/audio-varps-oracle';run.mkdir(parents=True,exist_ok=True)
    (run/'Prefix.java').write_text('''import java.io.*;import java.util.*;import java.security.*;
public class Prefix{static String hash()throws Exception{ByteArrayOutputStream b=new ByteArrayOutputStream();DataOutputStream d=new DataOutputStream(b);for(int i=0;i<64;i++)d.writeInt(VarpDomain.activeVarps[i]);return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(b.toByteArray()));}
public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){VarpDomain.pendingVarpTimers=new HashTable(16);VarpDomain.activeVarps=new int[3000];VarpDomain.varp=new int[3000];MonotonicClock.now=1000;List<String>polls=new ArrayList<>();for(String command:line.split(";")){String[]c=command.split(",");switch(c[0]){case"V":VarpDomain.set(Integer.parseInt(c[2]),Integer.parseInt(c[1]));break;case"L":VarpDomain.setVarp(Integer.parseInt(c[1]),Integer.parseInt(c[2]));break;case"T":MonotonicClock.now+=Integer.parseInt(c[1]);break;case"P":{List<String>events=new ArrayList<>();for(int id=VarpDomain.poll(true);id!=-1;id=VarpDomain.poll(false))events.add("["+id+",\\\""+hash()+"\\\"]");polls.add(events.toString());break;}default:throw new AssertionError(c[0]);}}System.out.println(polls);}}}
'''.replace('\\\\\\"','\\"'),encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-cp',str(classes),'-d',str(run),str(run/'Prefix.java')],check=True)
    rng=random.Random(53016);cases=[]
    for _ in range(64):
        commands=[]
        for tick in range(32):
            for _ in range(rng.randrange(1,9)):commands.append([rng.choice(['V','V','L']),rng.randrange(64),rng.randrange(-2147483648,2147483648)])
            commands.extend([['T',rng.choice([0,1,499,500,501,1000])],['P']])
        cases.append(dict(commands=commands))
    out=subprocess.run(['java','-cp',str(run)+';'+str(classes),'Prefix'],input='\n'.join(';'.join(','.join(map(str,c))for c in case['commands'])for case in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(out)==len(cases)
    for c,line in zip(cases,out):c['expected']=json.loads(line)
    sources=json.loads((ROOT/'fixtures/ui/delayed.json').read_text())['sources'];data=(json.dumps(dict(sources=sources,bindings='Complete original pending-varp set/setVarp/poll with original intrusive hash iteration and explicit monotonic clock. Each poll return records all64active values BEFORE the next poll, as refreshMagicVarp/updateMulti observes them. Metadata, emitter and hardware services separate.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/audio/varps.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original active-varp prefix corpus differs')
    else:path.write_bytes(data)
    print('Original active-varp prefixes:',len(cases)*32,'poll points;',sum(len(p)for c in cases for p in c['expected']),'returns')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
