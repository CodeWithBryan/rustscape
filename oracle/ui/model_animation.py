"""Unchanged SeqType.animateEntity dispatch, including secondary-frame lookup quirks."""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/ui-model-animation-oracle';FIX=ROOT/'fixtures/ui/model-animation.json'
spec=importlib.util.spec_from_file_location('v',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
def generate(check=False):
    RUN.mkdir(parents=True,exist_ok=True);raw=(ROOT/'reference/client/client/src/main/java/rt4/SeqType.java').read_bytes().replace(b'\r\n',b'\n')
    method=v.method(v.clean(raw.decode()),'public Model animateEntity(')
    (RUN/'SeqType.java').write_text('class SeqType{int[]frames,frameDelay,frameset;boolean tween,hasModelTransforms;static boolean applyTweening;'+method+'}',encoding='utf-8')
    (RUN/'Support.java').write_text(SUPPORT,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),str(RUN/'SeqType.java'),str(RUN/'Support.java')],check=True)
    rng=random.Random(530604);cases=[];requests=[]
    for _ in range(2048):
        n=rng.randrange(1,9);frames=[rng.randrange(17)<<16|rng.randrange(40)for _ in range(n)];delays=[rng.randrange(1,30)for _ in range(n)]
        secondary=None if rng.randrange(3)==0 else[rng.choice([65535,rng.randrange(17)<<16|rng.randrange(40)])for _ in range(rng.randrange(n+10))]
        args=[rng.randrange(n),rng.choice([-1,*range(n),n+2]),rng.randrange(-3,40),rng.randrange(2),rng.randrange(2),rng.randrange(2),rng.randrange(3)]
        cases.append(dict(frames=frames,delays=delays,secondary=secondary,args=args));requests.append(';'.join([','.join(map(str,args)),','.join(map(str,frames)),','.join(map(str,delays)),'null'if secondary is None else','.join(map(str,secondary))]))
    result=subprocess.run(['java','-cp',str(RUN),'Support'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for case,line in zip(cases,result):case['expected']=json.loads(line)
    output=dict(sources={'SeqType':hashlib.sha256(raw).hexdigest()},bindings='Complete unchanged SeqType.animateEntity. Controlled frameset readiness and alpha/color flags; Model spy records copy flags and ordered applyAnimation calls. The original overwrites nextFrame with its low frame index before looking up secondary-next metadata. Actual bone geometry and GPU are separate, with their own original-source corpora.',cases=cases)
    if check:
        if json.loads(FIX.read_text(encoding='utf-8'))!=output:raise SystemExit('Widget entity-animation fixture differs')
    else:FIX.write_text(json.dumps(output,separators=(',',':'))+'\n',encoding='utf-8')
    print('Original UI entity-animation dispatch:',len(cases),'cases')
SUPPORT=r'''
import java.io.*;import java.util.*;
class AnimFrameset{int group;AnimFrameset(int g){group=g;}boolean isAlphaTransformed(int i){Support.event(1,group,i);return(group+i)%3==0;}boolean isColorTransformed(int i){Support.event(2,group,i);return(group*7+i)%5==0;}}
class SeqTypeList{static int mode;static AnimFrameset getAnimFrameset(int id){Support.event(0,id);return mode==2||mode==1&&id%7==0?null:new AnimFrameset(id);}}
class Model{Model copyForEntity(boolean a,boolean b,boolean c){Support.event(3,a?1:0,b?1:0,c?1:0);return new Model();}void applyAnimation(AnimFrameset a,int af,AnimFrameset b,int bf,int delta,int length,boolean normals){Support.event(4,a.group,af,b==null?-1:b.group,bf,delta,length,normals?1:0);}}
public class Support{static List<int[]>events;static void event(int...v){events.add(v);}static int[]ints(String s){return s.isEmpty()?new int[0]:Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split(";",-1);int[]x=ints(p[0]);SeqType s=new SeqType();s.frames=ints(p[1]);s.frameDelay=ints(p[2]);s.frameset=p[3].equals("null")?null:ints(p[3]);s.tween=x[3]!=0;SeqType.applyTweening=x[4]!=0;s.hasModelTransforms=x[5]!=0;SeqTypeList.mode=x[6];events=new ArrayList<>();s.animateEntity(new Model(),x[1],x[2],x[0]);StringJoiner out=new StringJoiner(",","[","]");for(int[]e:events)out.add(Arrays.toString(e));System.out.println(out);}}}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
