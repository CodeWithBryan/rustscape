"""Original SceneGraph floor render-level classification; no renderer adapter."""
import hashlib,json,pathlib,random,subprocess

def generate(root,clean,method,check=False):
    source=root/'reference/client/client/src/main/java/rt4/SceneGraph.java'
    raw=source.read_bytes().replace(b'\r\n',b'\n')
    run=root/'runs/bridge-oracle';run.mkdir(exist_ok=True)
    code='class SceneGraph {static byte[][][] renderFlags=new byte[4][104][104];'+method(clean(raw.decode()),'public static int getRenderLevel(')+'}'
    (run/'SceneGraph.java').write_text(code,encoding='utf-8')
    (run/'Probe.java').write_text('import java.io.*;public class Probe{public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in));String line;while((line=reader.readLine())!=null){String[]p=line.split(",");int plane=Integer.parseInt(p[0]);SceneGraph.renderFlags[plane][50][50]=(byte)Integer.parseInt(p[1]);SceneGraph.renderFlags[1][50][50]=(byte)Integer.parseInt(p[2]);System.out.println(SceneGraph.getRenderLevel(50,50,plane));}}}',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'SceneGraph.java'),str(run/'Probe.java')],check=True)
    cases=[]
    for plane in range(4):
        for flags in range(16):
            for bridge in range(4):
                cases.append(dict(plane=plane,flags=flags,bridge=flags if plane==1 else bridge))
    rng=random.Random(53028043427)
    for index in range(256):
        plane=index%4;flags=rng.randrange(256);cases.append(dict(plane=plane,flags=flags,bridge=flags if plane==1 else rng.randrange(256)))
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(f"{c['plane']},{c['flags']},{c['bridge']}"for c in cases)+'\n',text=True,capture_output=True,check=True)
    rows=result.stdout.splitlines();assert len(rows)==len(cases)
    for case,row in zip(cases,rows):case['expected']=int(row)
    out=dict(sources={'SceneGraph.java':hashlib.sha256(raw).hexdigest()},scope='Unchanged original SceneGraph.getRenderLevel and explicit renderFlags array inputs. Flags8 force ground before flag2 bridge lowering. Plane1 flag aliases are the same actual cell. This does not establish full scene tile linkage, scenery replacement, roof occlusion or GPU rendering.',cases=cases)
    fixture=root/'fixtures/ui/bridge.json'
    if check:
        if json.loads(fixture.read_text())!=out:raise SystemExit('Bridge render-level fixture differs')
    else:fixture.write_text(json.dumps(out,separators=(',',':'))+'\n')
    print('Original bridge/floor render levels:',len(cases),'observations',flush=True)

if __name__=='__main__':
    import argparse,importlib.util
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('bridge_visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--check',action='store_true');generate(root,v.clean,v.method,parser.parse_args().check)
