"""Original legacy widget expressions/conditions under controlled client services."""
import argparse,hashlib,importlib.util,json,pathlib,random,subprocess
R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-cs1-oracle';O.mkdir(exist_ok=True)
spec=importlib.util.spec_from_file_location('v',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
def generate(check=False):
    sources={}
    def source(name):
        b=(R/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    s=source('Cs1ScriptRunner');(O/'Cs1ScriptRunner.java').write_text('class Cs1ScriptRunner{'+v.method(s,'public static int run(')+v.method(s,'public static boolean isTrue(')+'}')
    (O/'PlayerSkillXpTable.java').write_text(source('PlayerSkillXpTable'))
    s=source('VarpDomain');(O/'VarpDomain.java').write_text('class VarpDomain{static int[]activeVarps=new int[3500],BIT_MASKS=new int[32];static{int m=2;for(int i=0;i<32;i++){BIT_MASKS[i]=m-1;m+=m;}}'+v.method(s,'public static int getVarbit(')+'}')
    (O/'Probe.java').write_text(r'''
    import java.io.*;import java.util.*;
    class Component{int[][]cs1Scripts;int[]cs1ComparisonOpcodes,cs1ComparisonOperands,objTypes,objCounts;}
    class InterfaceList{static Component inventory;static Component getComponent(int id){return id==65538?inventory:null;}}
    class ObjType{boolean members;ObjType(boolean b){members=b;}}class ObjTypeList{static ObjType get(int id){return new ObjType(id==101);}}
    class LoginManager{static boolean mapMembers;}class Player{static int runEnergy,weight;int xFine,yFine,combatLevel;}class PlayerList{static Player self=new Player();}class Camera{static int originX,originY;}
    class VarbitType{int baseVar,startBit,endBit;VarbitType(int b,int s,int e){baseVar=b;startBit=s;endBit=e;}}class VarbitTypeList{static VarbitType get(int id){if(id==0)return new VarbitType(2,2,5);if(id==1)return new VarbitType(0,0,31);throw new IllegalArgumentException();}}
    public class Probe{static int[]ints(String s){if(s.equals("_"))return null;if(s.equals("")||s.equals("-"))return new int[0];return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
    public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){String[]p=line.split("\t",-1);Component c=new Component();if(!p[1].equals("_")){String[]list=p[1].isEmpty()?new String[0]:p[1].split("/",-1);c.cs1Scripts=new int[list.length][];for(int i=0;i<list.length;i++)c.cs1Scripts[i]=ints(list[i]);}c.cs1ComparisonOpcodes=ints(p[2]);c.cs1ComparisonOperands=ints(p[3]);int[]x=ints(p[4]);LoginManager.mapMembers=x[0]!=0;PlayerList.self.combatLevel=x[1];Player.runEnergy=x[2];Player.weight=x[3];PlayerList.self.xFine=x[4];PlayerList.self.yFine=x[5];Camera.originX=x[6];Camera.originY=x[7];System.arraycopy(ints(p[5]),0,PlayerSkillXpTable.boostedLevels,0,25);System.arraycopy(ints(p[6]),0,PlayerSkillXpTable.baseLevels,0,25);System.arraycopy(ints(p[7]),0,PlayerSkillXpTable.experience,0,25);Arrays.fill(VarpDomain.activeVarps,0);int[]vp=ints(p[8]),ids={0,1,2,3499};for(int i=0;i<ids.length;i++)VarpDomain.activeVarps[ids[i]]=vp[i];VarpDomain.activeVarps[261]=vp[0];InterfaceList.inventory=new Component();InterfaceList.inventory.objTypes=ints(p[9]);InterfaceList.inventory.objCounts=ints(p[10]);int result=Cs1ScriptRunner.run(Integer.parseInt(p[0]),c);String active;try{active=Boolean.toString(Cs1ScriptRunner.isTrue(c));}catch(Exception e){active="\"error\"";}System.out.println("["+result+","+active+"]");}}}
    ''')
    c=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],text=True,capture_output=True)
    if c.returncode:raise SystemExit(c.stderr)
    rng=random.Random(530608);cases=[];requests=[]
    def csv(values):return '_'if values is None else ','.join(map(str,values))
    def script():
        result=[]
        for _ in range(rng.randrange(1,13)):
            op=rng.choice(list(range(1,21))+[99]);result.append(op)
            if op in[1,2,3,6]:result.append(rng.choice([-1,0,3,23,24,25]))
            elif op in[4,10]:result += [1,rng.choice([2,3]),rng.choice([-1,100,101,102])]
            elif op in[5,7]:result.append(rng.choice([-1,0,1,2,3499,3500]))
            elif op==13:result += [rng.choice([0,1,2,3499]),rng.choice([-32,-1,0,1,31,32,63])]
            elif op==14:result.append(rng.randrange(2))
            elif op==20:result.append(rng.choice([-2147483648,-1,0,1,2147483647,rng.randrange(-1000,1000)]))
        result.append(0)
        if rng.randrange(12)==0:result=result[:rng.randrange(len(result))]
        return result
    for i in range(2048):
        scripts=None if i%41==0 else [script()for _ in range(rng.randrange(1,5))]
        if i<32:scripts=[[20,-2147483648,16,20,-1,0],[4,1,2,101,0],[10,1,2,101,0],[14,1,0]]
        ops=None if i%17==0 else [rng.randrange(6)for _ in range(rng.randrange(0,5))]
        operands=None if ops is None else [rng.choice([-2,-1,0,1,999999999,rng.randrange(-1000,1000)])for _ in ops]
        if operands and i%19==0:operands=operands[:-1]
        if i%97==0:ops=[];operands=None
        context=[rng.randrange(2)]+[rng.choice([-2147483648,-1,0,1,2147483647,rng.randrange(-100000,100000)])for _ in range(7)]
        boosted=[rng.randrange(-10,128)for _ in range(25)];base=[rng.choice([0,1,99,100,rng.randrange(1,121)])for _ in range(25)];xp=[rng.randrange(-2147483648,2147483648)for _ in range(25)];varps=[rng.randrange(-2147483648,2147483648)for _ in range(4)]
        ids=None if i%23==0 else [rng.choice([0,101,102,103])for _ in range(8)];counts=None if i%13==0 else [rng.choice([-2147483648,-1,0,1,2147483647])for _ in range(rng.choice([3,8,8]))]
        index=rng.choice([-1,0,0,0,1,4]);case=dict(index=index,scripts=scripts,opcodes=ops,operands=operands,context=context,boosted=boosted,base=base,xp=xp,varps=varps,ids=ids,counts=counts);cases.append(case)
        requests.append('\t'.join([str(index),'_'if scripts is None else'/'.join(csv(s)or'-'for s in scripts),csv(ops),csv(operands),csv(context),csv(boosted),csv(base),csv(xp),csv(varps),csv(ids),csv(counts)]))
    for value in [0,1]:
        case=json.loads(json.dumps(cases[0]));case.update(index=0,scripts=[[5,261,0]],opcodes=[1],operands=[1]);case['varps'][0]=value;cases.append(case)
        requests.append('\t'.join(['0','5,261,0','1','1',csv(case['context']),csv(case['boosted']),csv(case['base']),csv(case['xp']),csv(case['varps']),csv(case['ids']),csv(case['counts'])]))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True)
    if p.returncode:raise SystemExit(p.stderr)
    lines=p.stdout.splitlines();assert len(lines)==len(cases)
    for case,line in zip(cases,lines):case['expected']=json.loads(line)
    result=dict(sources=sources,bindings='Complete original Cs1ScriptRunner.run/isTrue with original skill XP table and VarpDomain.getVarbit. Component inventory, member flags and varbit metadata are explicit controlled services; full hierarchy/rendering is separate.',cases=cases)
    fixture=R/'fixtures/ui/cs1.json'
    if check:
        if json.loads(fixture.read_text())!=result:raise SystemExit('CS1 fixture differs')
    else:fixture.write_text(json.dumps(result,separators=(',',':'))+'\n')
    print('Original CS1 execution/conditions',len(cases))
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
