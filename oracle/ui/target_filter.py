"""Original typed getParam methods and the four unmodified target conditions."""
import subprocess
def emit(run,source,method):
    obj=method(source('ObjType'),'public final int getParam(')
    p=run/'Support.java';code=p.read_text();code=code.replace('int getParam(int def,int id){return def;}','HashTable params;'+obj);code=code.replace('class ParamTypeList{static ParamType get(int id){return new ParamType();}}','class ParamTypeList{static int defaultInt;static ParamType get(int id){ParamType p=new ParamType();p.defaultInt=defaultInt;return p;}}');p.write_text(code)
    definitions='''class HashTable{java.util.Map<Long,Object> data=new java.util.HashMap<>();Object get(long id){return data.get(id);}}class IntNode{int value;IntNode(int n){value=n;}}class StringNode{JagString value=new JagString("text");}'''
    definitions+='class NpcType{HashTable params;'+method(source('NpcType'),'public final int getParam(')+'}class LocType{HashTable params;'+method(source('LocType'),'public int getParam(')+'}'
    (run/'TargetDefinitions.java').write_text(definitions)
    mini=source('MiniMenu');blocks={bit:method(mini,'if ((targetMask & '+value+')')for bit,value in [(1,'0x1'),(2,'0x2'),(4,'0x4'),(16,'0x10')]}
    methodbody='''static String filter(int kind,int mask,int param,int def,int table){targetMask=mask;targetParamId=param;targetCursorId=37;targetVerb=new JagString("Cast");targetOpBase=new JagString("Selected");ParamTypeList.defaultInt=def;entries.clear();size=0;HashTable values=table==0?null:new HashTable();if(table>=2){Object value=table==4?new StringNode():new IntNode(table==3?def+1:def);values.data.put(table==5?8L:7L,value);}NpcType npcType=new NpcType();npcType.params=values;LocType locType=new LocType();locType.params=values;ObjType objType=new ObjType();objType.params=values;objType.id=1;Component component=new Component();component.id=149<<16;int npcIndex=13,intArg2=0,x=5,y=6,j=1,slotIndex=2;long key=13,pickKey=7L<<32;JagString npcName=new JagString("Npc");locType.name=new JagString("Loc");ParamType paramType=param==-1?null:ParamTypeList.get(param);try{'''
    for bit,block in blocks.items():methodbody+=f'if(kind=={bit}){{'+block+'}'
    methodbody+='return String.join(",",entries);}catch(ClassCastException e){return "error";}}'
    definitions=(run/'TargetDefinitions.java').read_text().replace('class LocType{','class LocType{JagString name;');(run/'TargetDefinitions.java').write_text(definitions)
    p=run/'MiniMenu.java';code=p.read_text();assert code.endswith('}\n');p.write_text(code[:-2]+methodbody+'}\n')
    (run/'FilterProbe.java').write_text('''import java.io.*;import java.util.*;public class FilterProbe{public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String line;while((line=in.readLine())!=null){int[] a=Arrays.stream(line.split(",")).mapToInt(Integer::parseInt).toArray();System.out.println(MiniMenu.filter(a[0],a[1],a[2],a[3],a[4]));}}}''')
    return [dict(args=[bit,mask,param,default,table])for bit in [1,2,4,16]for mask in [0,1,2,4,16,127]for param in [-1,7]for default in [-2147483648,-1,0,1,2147483647]for table in range(6)]
def observe(run,cases):
    result=subprocess.run(['java','-cp',str(run),'FilterProbe'],input='\n'.join(','.join(map(str,c['args']))for c in cases)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for c,value in zip(cases,result):c['expected']=value
