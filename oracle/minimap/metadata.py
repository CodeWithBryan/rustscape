"""All prepared location minimap metadata and real floor/mapscene cache records."""
import argparse,hashlib,importlib.util,json,pathlib,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False,pack_file=None):
    spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources=v.generate(build_only=True)
    spec=importlib.util.spec_from_file_location('p',ROOT/'tools/prepare_visual.py');p=importlib.util.module_from_spec(spec);spec.loader.exec_module(p);cache=p.Cache(ROOT/'assets/cache')
    raw=(ROOT/'reference/client/client/src/main/java/rt4/MsiType.java').read_bytes().replace(b'\r\n',b'\n');sources['MsiType']=hashlib.sha256(raw).hexdigest();msi=v.clean(raw.decode())
    run=ROOT/'runs/minimap-metadata-oracle';run.mkdir(parents=True,exist_ok=True)
    code='''import java.io.*;import java.util.*;
class MsiDefinition{int spriteId,tintColor;boolean stretchToTile=false;'''+v.method(msi,'public final void decode(')+v.method(msi,'private void decodeOpcode(')+'''}
public class Probe{public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);byte[]data=HexFormat.of().parseHex(p[1]);switch(p[0]){
case "L":{LocType d=LocType.clipping(data);d.postDecode();System.out.println("["+d.mapfunction+","+d.mapscene+","+d.mapSceneRotated+","+d.mapSceneAngleOffset+","+d.width+","+d.length+","+d.interactable+","+d.multiLocVarbit+","+d.multiLocVarp+","+(d.multiLocs==null?"null":Arrays.toString(d.multiLocs))+"]");break;}
case "O":{FloType d=new FloType();d.decode(new Buffer(data),0);System.out.println("["+d.baseColor+","+d.secondaryColor+","+d.texture+","+d.textureScale+","+d.blendTexture+","+d.waterColor+","+d.waterOpacity+"]");break;}
case "M":{MsiDefinition d=new MsiDefinition();d.decode(new Buffer(data),0);System.out.println("["+d.spriteId+","+d.tintColor+","+d.stretchToTile+"]");break;}
}}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in v.RUN.glob('*.java')],str(run/'Probe.java')],check=True)
    pack=(pack_file or ROOT/'assets/visual/live/lumbridge.pack').read_bytes();n=struct.unpack_from('<I',pack,8)[0];manifest=json.loads(pack[12:12+n]);blob=pack[12+n:];cases=[]
    for name,(start,size)in sorted(manifest['files'].items()):
        if name.startswith('loc/'):cases.append(dict(kind='L',id=int(name[4:]),raw=blob[start:start+size].hex()))
    for group,kind in [(4,'O'),(34,'M')]:
        for id,data in sorted(cache.group(2,group).items()):cases.append(dict(kind=kind,id=id,raw=data.hex()))
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(c['kind']+';'+c['raw']for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=json.loads(line)
    data=(json.dumps(dict(sources=sources,bindings='Every raw location definition in the live Lumbridge pack, all original overlay and mapscene records in pinned cache2. Complete original decoders/postDecode; geometry/cache-template services supplied by visual oracle.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/minimap/metadata.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap metadata differs')
    else:path.write_bytes(data)
    print(f'Original minimap metadata: {len(cases)} raw location/overlay/mapscene records')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');p.add_argument('--pack',type=pathlib.Path);a=p.parse_args();generate(a.check,a.pack)
