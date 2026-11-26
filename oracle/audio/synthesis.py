"""Original synthesized PCM for every prepared location ambient effect."""
import argparse,hashlib,importlib.util,json,pathlib,struct,subprocess
def _recorded(root,name):
    # The packs the JVM fixtures were recorded from: kept under recorded/ once
    # the default packs are rebuilt natively (crates/client/tests/support/packs.rs).
    kept=root/'assets/visual/recorded'/name
    return kept if kept.is_file() else root/'assets/visual'/name
ROOT=pathlib.Path(__file__).resolve().parents[2]
def generate(check=False,*,ids=None,name='synthesis',bindings=None):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    spec=importlib.util.spec_from_file_location('a',ROOT/'oracle/ui/audio.py');a=importlib.util.module_from_spec(spec);spec.loader.exec_module(a);a.generate(ROOT,v.clean,v.method,True)
    run=ROOT/f'runs/audio-{name}-oracle';run.mkdir(parents=True,exist_ok=True)
    code='''import java.io.*;import java.util.*;
public class Probe{static String hash(byte[]b)throws Exception{return HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(b));}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){byte[]raw=HexFormat.of().parseHex(line);PcmSound p=new SynthSound(new Buffer(raw)).toPcmSound();SynthSound trimmed=new SynthSound(new Buffer(raw));int ticks=trimmed.getStart();PcmSound q=trimmed.toPcmSound();System.out.println("["+p.samples.length+","+p.start+","+p.end+",\\\""+hash(p.samples)+"\\\","+ticks+","+q.samples.length+","+q.start+","+q.end+",\\\""+hash(q.samples)+"\\\"]");}}}
'''.replace('\\\\\\"','\\"')
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-cp',str(ROOT/'runs/ui-oracle/audio'),'-d',str(run),str(run/'Probe.java')],check=True)
    if ids is None:
        metadata=json.loads((ROOT/'fixtures/audio/metadata.json').read_text());ids=set()
        for c in metadata['cases']:
            n=c['expected'];ids.update(v for v in [n[0]]+(n[4]or[])if v>=0)
    pack=_recorded(ROOT,'ui.pack').read_bytes();n=struct.unpack_from('<I',pack,8)[0];manifest=json.loads(pack[12:12+n]);blob=pack[12+n:];cases=[];missing=[]
    for id in sorted(ids):
        key=f'sound/{id}'
        if key not in manifest['files']:missing.append(id);continue
        start,size=manifest['files'][key];cases.append(dict(id=id,raw=blob[start:start+size].hex()))
    out=subprocess.run(['java','-cp',str(run)+';'+str(ROOT/'runs/ui-oracle/audio'),'Probe'],input='\n'.join(c['raw']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(out)==len(cases)
    for c,line in zip(cases,out):c['expected']=json.loads(line)
    if bindings is None:bindings='All distinct prepared LocType primary/secondary sound IDs, original complete SynthSound/instrument/filter/envelope synthesis before/after getStart. Global sample rate22050. Missing raw sound groups listed explicitly; no music or hardware-output claim.'
    sources=json.loads((ROOT/'fixtures/ui/audio.json').read_text())['sources'];data=(json.dumps(dict(sources=sources,bindings=bindings,cases=cases,missing=missing),separators=(',',':'))+'\n').encode();path=ROOT/f'fixtures/audio/{name}.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original ambient synthesis corpus differs')
    else:path.write_bytes(data)
    print('Original ambient synthesis:',len(cases),'effects;',len(missing),'missing groups')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
