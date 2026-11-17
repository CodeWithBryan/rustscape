"""Exact original pressed fields, footer, Protocol timer and release statement."""
import hashlib,importlib.util,json,pathlib,random,re,subprocess
R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-inventory-press-oracle';O.mkdir(exist_ok=True)

def generate(check=False):
 spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
 raws={n:(v.SRC/(n+'.java')).read_bytes().replace(b'\r\n',b'\n')for n in['MiniMenu','Protocol']};mini=v.clean(raws['MiniMenu'].decode());protocol=v.clean(raws['Protocol'].decode())
 pattern=r'itemInteractionTick = 0;\s+pressedInventoryComponent = InterfaceList\.getComponent\(menuArg2\);\s+pressedSlotIndex = menuArg1;'
 stamps=re.findall(pattern,mini);assert len(stamps)==13
 assert len({re.sub(r'\s+',' ',s)for s in stamps})==1
 footer=v.method(mini,'if (pressedInventoryComponent != null && itemInteractionTick == 0)')
 timer=v.method(protocol,'if (MiniMenu.pressedInventoryComponent != null)')
 release=re.search(r'MiniMenu\.itemInteractionTick = 10;',protocol)[0]
 constants=dict((n,int(x))for n,x in re.findall(r'public static final int (\w+) = (\d+);',mini));families=[]
 for m in re.finditer(r'if \(actionCode == (\w+)\) \{',mini):
  body=v.method(mini[m.start():],m[0][:-2])
  if re.search(pattern,body):families.append(constants[m[1]])
 assert sorted(families)==[3,5,7,13,23,25,35,40,43,47,48,58,1006],families
 code='''import java.io.*;import java.util.*;
 class Component{int object;Component(int o){object=o;}}
 class InterfaceList{static Component getComponent(int id){Probe.add(0,id);if(Probe.answer==-2)throw new IllegalStateException();Probe.current=Probe.answer;return Probe.answer<0?null:Probe.nodes[Probe.answer];}static void redraw(Component c){Probe.add(1,c.object);if(Probe.fail)throw new IllegalStateException();}}
 class MiniMenu{static Component pressedInventoryComponent;static int pressedSlotIndex,itemInteractionTick;
 static void assign(int menuArg2,int menuArg1){'''+stamps[0]+'''}
 static void footer(){'''+footer+'''}}
 class Protocol{static void tick(){'''+timer+'''}static void release(){'''+release+r'''}}
 public class Probe{static Component[]nodes={new Component(0),new Component(1)};static int answer,current;static boolean fail;static List<String>log=new ArrayList<>();static void add(int...a){log.add(Arrays.toString(a));}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]fields=line.split(";");int[]s=Arrays.stream(fields[0].split(",")).mapToInt(Integer::parseInt).toArray();MiniMenu.pressedInventoryComponent=s[0]<0?null:nodes[s[0]];MiniMenu.itemInteractionTick=s[1];MiniMenu.pressedSlotIndex=s[2];current=s[3];List<String>rows=new ArrayList<>();for(int j=1;j<fields.length;j++){int[]a=Arrays.stream(fields[j].split(",")).mapToInt(Integer::parseInt).toArray();log.clear();fail=a[3]!=0;String error="null";try{switch(a[0]){case 0:answer=a[1];MiniMenu.assign(100,a[2]);MiniMenu.footer();break;case 1:Protocol.tick();break;case 2:Protocol.release();break;case 3:current=a[1];break;case 4:MiniMenu.footer();break;}}catch(Exception e){error="\""+e.getClass().getSimpleName()+"\"";}Component c=MiniMenu.pressedInventoryComponent;rows.add("[["+String.join(",",log)+"],"+(c==null?-1:c.object)+","+MiniMenu.pressedSlotIndex+","+MiniMenu.itemInteractionTick+","+current+","+(c!=null&&current>=0&&c==nodes[current])+","+error+"]");}System.out.println("["+String.join(",",rows)+"]");}}}
 '''
 (O/'Probe.java').write_text(code,encoding='utf-8');p=subprocess.run(['javac','-d',str(O),str(O/'Probe.java')],capture_output=True,text=True);(O/'compile.log').write_text(p.stdout+p.stderr);assert p.returncode==0,p.stderr
 rng=random.Random(5301510);cases=[]
 for i in range(2048):
  seed=[rng.choice([-1,0,1]),rng.choice([-2147483648,-1,0,1,9,10,13,14,15,2147483647]),rng.choice([-1,0,1,65535]),rng.choice([-1,0,1])]
  ops=[]
  if i%4==0:ops=[[0,0,1,0]]+[[1,0,0,0]for _ in range(16)]
  elif i%4==1:ops=[[0,0,1,0],[3,1,0,0],[2,0,0,0]]+[[1,0,0,0]for _ in range(6)]
  else:
   for _ in range(24):
    kind=rng.choice([0,0,1,1,1,1,1,2,3,4]);ops.append([kind,rng.choice([-2,-1,0,1])if kind==0 else rng.choice([-1,0,1]),rng.choice([-1,0,1,65535]),int(rng.randrange(10)==0)])
  cases.append(dict(seed=seed,operations=ops))
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(';'.join(','.join(map(str,a))for a in[c['seed'],*c['operations']])for c in cases)+'\n',capture_output=True,text=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
 rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==len(cases)
 for c,row in zip(cases,rows):c['expected']=row
 out=dict(scope='Exact13 identical post-packet inventory action assignment prefixes, pressed footer, Protocol fifteen-tick expiry and literal release counter10. Scripted component lookup/redraw services, same-numeric-ID distinct object replacement, nulls, failed prefixes and Java counter overflow. Does not establish full MiniMenu action serialization/target cleanup, widget array ownership, dirty rectangle framebuffer or native OS input.',sources={n+'.java':hashlib.sha256(b).hexdigest()for n,b in raws.items()},actionFamilies=sorted(families),cases=cases)
 target=R/'fixtures/ui/inventory-press.json';content=json.dumps(out,separators=(',',':'))+'\n'
 if check:assert target.read_text(encoding='utf-8')==content,'Inventory press corpus differs'
 else:target.write_text(content,encoding='utf-8')
 print('Original pressed inventory lifecycle',len(cases),'sequences',sum(len(c['operations'])for c in cases),'steps')

if __name__=='__main__':
 import sys
 generate('--check'in sys.argv)
