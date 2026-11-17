"""Unchanged JavaMouseWheel classes with real headless AWT wheel events."""
import hashlib,json,random,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/players-oracle/wheel';run.mkdir(parents=True,exist_ok=True);hashes={}
    for name in ['MouseWheel','JavaMouseWheel']:
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();code=clean(raw.decode()).replace('import static rt4.MathUtils.clamp;','');(run/(name+'.java')).write_text(code,encoding='utf-8')
    probe='''import java.io.*;import java.awt.*;import java.awt.event.*;public class Probe{public static void main(String[]args)throws Exception{JavaMouseWheel w=new JavaMouseWheel();Component target=new Canvas();BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]a=line.split(";");Integer out=null;if(a[0].equals("R"))w=new JavaMouseWheel();else if(a[0].equals("M"))w.mouseWheelMoved(new MouseWheelEvent(target,MouseEvent.MOUSE_WHEEL,0,0,0,0,0,false,MouseWheelEvent.WHEEL_UNIT_SCROLL,3,Integer.parseInt(a[1])));else out=w.getRotation();System.out.println("["+w.currentRotation+","+w.previousRotation+","+out+"]");}}}'''
    (run/'Probe.java').write_text(probe,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(53045);commands=[]
    for scenario in range(32):
        commands.append(['R'])
        for i in range(128):commands.append(['P']if i%4==3 else ['M',rng.choice([0,-1,1,-3,3,-2**31,2**31-1,rng.randrange(-1000,1001)])])
    lines=subprocess.run(['java','-Djava.awt.headless=true','-cp',str(run),'Probe'],input='\n'.join(';'.join(map(str,c))for c in commands)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(lines)==len(commands)
    content=json.dumps(dict(sources=hashes,bindings='Complete unchanged MouseWheel/JavaMouseWheel classes (only unused clamp import removed), real headless AWT Canvas/MouseWheelEvent objects, controlled rotation/clock inputs. Integer wrapping, previousRotation and destructive getRotation observations; DOM device-unit conversion is a separate explicit platform policy.',cases=[dict(command=c,expected=json.loads(line))for c,line in zip(commands,lines)]),separators=(',',':')).encode()+b'\n';path=root/'fixtures/players/wheel.json'
    if check:
        if path.read_bytes()!=content:raise SystemExit('Wheel corpus differs')
    else:path.write_bytes(content)
    print(f'Original JavaMouseWheel/AWT observations: {len(commands)} commands')
