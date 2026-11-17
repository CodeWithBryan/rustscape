"""Original immutable JagString character concatenation and substring methods."""
import hashlib,json,pathlib,random,subprocess

def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4/JagString.java'
    raw=src.read_bytes().replace(b'\r\n',b'\n');jag=clean(raw.decode())
    run=root/'runs/ui-oracle/editing';run.mkdir(parents=True,exist_ok=True)
    code='''import java.io.*;import java.util.*;
class JagString{byte[] chars;int length;JagString(){}JagString(byte[] b){chars=b;length=b.length;}static void copy(byte[] a,int x,byte[] b,int y,int n){System.arraycopy(a,x,b,y,n);}
'''+method(jag,'public final JagString concatChar(')+method(jag,'public final JagString substring(int to, int from)')+'''}
public class Probe{public static void main(String[] args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[] x=line.split(";",-1);JagString s=new JagString(HexFormat.of().parseHex(x[1]));try{JagString out=x[0].equals("C")?s.concatChar(Integer.parseInt(x[2])):s.substring(Integer.parseInt(x[3]),Integer.parseInt(x[2]));System.out.println(HexFormat.of().formatHex(out.chars));}catch(RuntimeException e){System.out.println("error");}}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),str(run/'Probe.java')],check=True)
    rng=random.Random(530);cases=[]
    for c in list(range(-128,513))+[-2147483648,2147483647,10000,65535]:
        cases.append(dict(op='C',text=bytes(rng.randrange(256)for _ in range(rng.randrange(40))).hex(),args=[c]))
    for _ in range(1024):
        text=bytes(rng.randrange(256)for _ in range(rng.randrange(40)));a=rng.randrange(-2,len(text)+3);b=rng.randrange(-2,len(text)+3)
        cases.append(dict(op='S',text=text.hex(),args=[a,b]))
    inputs=[';'.join([c['op'],c['text'],*map(str,c['args'])])for c in cases]
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for c,line in zip(cases,result):c['expected']=line
    content=json.dumps(dict(sources={'JagString':hashlib.sha256(raw).hexdigest()},bindings='Exact-sized byte-string storage; malformed substring bounds produce an explicit native error',cases=cases),separators=(',',':')).encode()+b'\n'
    p=root/'fixtures/ui/editing.json'
    if check:
        if p.read_bytes()!=content:raise SystemExit('UI editing corpus differs')
    else:p.write_bytes(content)
    print(f'RT4 immutable text editing: {len(cases)} cases')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
