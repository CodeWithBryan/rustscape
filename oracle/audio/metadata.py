"""Real prepared location audio fields and original one-level hasAreaSound lookup."""
import argparse,hashlib,importlib.util,json,pathlib,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]
def generate(check=False,pack_file=None):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources=v.generate(build_only=True)
    run=ROOT/'runs/audio-metadata-oracle';run.mkdir(parents=True,exist_ok=True)
    raw=(ROOT/'reference/client/client/src/main/java/rt4/LocType.java').read_bytes().replace(b'\r\n',b'\n');method=v.method(v.clean(raw.decode()),'public boolean hasAreaSound()')
    original=(v.RUN/'LocType.java').read_text();(run/'LocType.java').write_text(original[:-1]+method+'}',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class LocTypeList{static Map<Integer,LocType>types=new HashMap<>();static LocType get(int id){return types.getOrDefault(id,new LocType());}}
public class Probe{public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));List<Integer>ids=new ArrayList<>();String line;while((line=r.readLine())!=null){String[]p=line.split(";");int id=Integer.parseInt(p[0]);LocType d=LocType.clipping(HexFormat.of().parseHex(p[1]));d.postDecode();LocTypeList.types.put(id,d);ids.add(id);}for(int id:ids){LocType d=LocTypeList.types.get(id);System.out.println("["+d.bgsound+","+d.bgsoundrange*128+","+d.bgsoundmin+","+d.bgsoundmax+","+Arrays.toString(d.bgsounds)+","+d.hasAreaSound()+"]");}}}
''',encoding='utf-8')
    files=[str(p)for p in v.RUN.glob('*.java')if p.name!='LocType.java']+[str(run/'LocType.java'),str(run/'Probe.java')];subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*files],check=True)
    pack=(pack_file or ROOT/'assets/visual/live/lumbridge-minimap.pack').read_bytes();n=struct.unpack_from('<I',pack,8)[0];manifest=json.loads(pack[12:12+n]);blob=pack[12+n:];cases=[]
    for name,(start,size)in sorted(manifest['files'].items()):
        if name.startswith('loc/'):cases.append(dict(id=int(name[4:]),raw=blob[start:start+size].hex()))
    out=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(str(c['id'])+';'+c['raw']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(out)==len(cases)
    for c,line in zip(cases,out):c['expected']=json.loads(line)
    data=(json.dumps(dict(sources=sources,bindings='Complete original LocType decoders/postDecode and unchanged hasAreaSound method. All raw prepared location roots/morph dependencies loaded before one-level child lookup; missing child metadata uses original default fields. Global cache/geometry services are explicit visual oracle adapters; no whole-scene playback claim.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/audio/metadata.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original location audio metadata differs')
    else:path.write_bytes(data)
    print('Original location audio metadata:',len(cases),'records;',sum(c['expected'][-1]for c in cases),'audible roots')
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--pack',type=pathlib.Path);a=p.parse_args();generate(a.check,a.pack)
