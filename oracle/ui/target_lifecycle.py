"""Original target lifecycle/select branch, with explicit hook mutation adapters.

The callback adapter models ordinary ScriptRunner error reporting/return, not
fatal JVM Errors or a full CS2 interpreter. Rust tests run real CS2 bytecode for
the same mutations, deletions, replacement and failed prefixes.
"""
import json,subprocess

def emit(run,source,method):
    mini=source('MiniMenu');p=run/'MiniMenu.java';code=p.read_text()
    code=code.replace('class MiniMenu {','class MiniMenu {static int targetInterfaceId,targetChildId=-1,defaultCursorId;')
    code=code[:-2]+method(mini,'public static void cancelTargeting(')+method(mini,'public static void startTargeting(')+'static void select(int menuArg1,int menuArg2){Component component;int actionCode=32;'+method(mini,'if (actionCode == UNKNOWN_32)')+'}}\n'
    p.write_text(code)
    p=run/'InterfaceList.java';code=p.read_text().replace('class InterfaceList {','class InterfaceList {static java.util.Map<String,Component> table=new java.util.HashMap<>();static String key(int id,int child){return id+":"+child;}static Component getComponent(int id,int child){return table.get(key(id,child));}static void redraw(Component c){}')
    p.write_text(code)
    (run/'Lifecycle.java').write_text('''import java.util.*;import java.io.*;
class HookRequest{Component source;Object[] arguments;}
class ScriptRunner{static int trace,errors;static void run(HookRequest hook){int phase=(int)hook.arguments[0],mode=(int)hook.arguments[1];trace=trace*10+phase;try{Component c=InterfaceList.getComponent(20,0);if(mode==1){c.optionCircumfix=new JagString("Changed");c.optionBase=new JagString("ChangedBase");}if(mode==2)InterfaceList.table.remove("20:0");if(mode==3){Component replacement=new Component();replacement.id=20;replacement.createdComponentId=0;replacement.if3=true;replacement.type=5;replacement.optionCircumfix=new JagString("Replacement");replacement.optionBase=new JagString("ReplacementBase");InterfaceList.table.put("20:0",replacement);}if(mode==4)throw new IllegalStateException();}catch(Exception e){errors++;}}}
public class Lifecycle{public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){int[] a=Arrays.stream(line.split(",")).mapToInt(Integer::parseInt).toArray();InterfaceList.table.clear();ScriptRunner.trace=ScriptRunner.errors=0;Component old=new Component();old.id=10;old.onUseWith=new Object[]{1,a[0]};InterfaceList.table.put("10:-1",old);Component target=new Component();target.id=20;target.createdComponentId=0;target.if3=a[3]!=0;target.type=5;target.optionBase=new JagString("Base");target.optionSuffix=new JagString("Spell");target.optionCircumfix=new JagString("Cast");target.properties=new ServerActiveProperties(a[5]<<11,7);target.targetCursor=44;target.defaultTargetCursor=45;target.onUse=new Object[]{2,a[1]};InterfaceList.table.put("20:0",target);MiniMenu.isTargeting=a[2]!=0;MiniMenu.targetInterfaceId=10;MiniMenu.targetChildId=-1;MiniMenu.targetMask=86;MiniMenu.targetParamId=-1;MiniMenu.targetCursorId=31;MiniMenu.defaultCursorId=32;MiniMenu.targetVerb=new JagString("Old");MiniMenu.targetOpBase=new JagString("OldText");MiniMenu.itemTargetMode=1;MiniMenu.select(0,20);if(a[4]!=0)MiniMenu.cancelTargeting();Component current=InterfaceList.getComponent(20,0);String state=(MiniMenu.isTargeting?1:0)+","+MiniMenu.targetInterfaceId+","+MiniMenu.targetChildId+","+MiniMenu.targetMask+","+MiniMenu.targetParamId+","+MiniMenu.targetCursorId+","+MiniMenu.defaultCursorId+","+MiniMenu.itemTargetMode+","+ScriptRunner.trace+","+ScriptRunner.errors;System.out.println(state+";"+MiniMenu.hex(MiniMenu.targetVerb)+";"+MiniMenu.hex(MiniMenu.targetOpBase)+";"+(current==null?"-":MiniMenu.hex(current.optionCircumfix)+":"+MiniMenu.hex(current.optionBase)));}}}
''',encoding='utf-8')
    return [dict(args=[cancel,use,active,if3,end,mask])for cancel in range(5)for use in range(5)for active in range(2)for if3 in range(2)for end in range(2)for mask in [0,63]]

def observe(run,cases):
    results=subprocess.run(['java','-cp',str(run),'Lifecycle'],input='\n'.join(','.join(map(str,c['args']))for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(results)==len(cases)
    for case,result in zip(cases,results):case['expected']=result
