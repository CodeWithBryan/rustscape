"""Original ordinary rectangle/text selection and byte-string formatting."""
import hashlib,importlib.util,json,pathlib,random,re,subprocess

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-widget-style-oracle';O.mkdir(parents=True,exist_ok=True)
    spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def source(name):
     b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    s=source('JagString');names=['EMPTY','NULL_STRING','PERIOD','PERCENT_SIGN','CLOSE_COLOR_TIMES','WHITE_COLOR_PREFIX','GREEN_COLOR_PREFIX','YELLOW_COLOR_PREFIX','COMMA','CLOSE_COLOR','CLOSE_PAREN_CLOSE_COLOR'];consts=[]
    for name in names:consts.append(re.search(r'public static final JagString '+name+r' = parse\("[^"\n]*"\);',s)[0])
    methods=['public static JagString concatenate(','public static JagString concatenateRange(','public static void copy(','public static JagString parse(','public static JagString parseInt(int','public static JagString parseInt(boolean','public static JagString decodeString(','public static JagString formatIp(','public final int length()','public final int indexOf(JagString needle)','public final int indexOf(JagString needle,','public final JagString substring(int from)','public final JagString substring(int to,','public final JagString compact()']
    (O/'JagString.java').write_text('import java.nio.charset.StandardCharsets;class JagString{byte[]chars;int length;boolean mutable=true;'+''.join(consts)+''.join(v.method(s,m)for m in methods)+'JagString intern(){return this;}String hex(){return java.util.HexFormat.of().formatHex(chars,0,length);}}',encoding='utf-8')
    s=source('Cs1ScriptRunner');rect=v.method(s,'if (component.type == 3)');text=v.method(s,'if (component.type == 4)')
    placeholders=''.join(re.findall(r'(?:public|private) static final JagString CS1_PLACEHOLDER_\w+ = JagString.parse\("[^"\n]*"\);',s))
    code='import java.nio.charset.StandardCharsets;class Draw{static Component pleaseWaitComponent;'+placeholders+'static boolean isTrue(Component c){Log.add("Active");return c.active;}static int run(int i,Component c){Log.add("Run",i);return c.values[i];}'+v.method(s,'public static JagString interpolate(')+v.method(s,'public static JagString formatItemCount(')
    code+='static void render(Component component,int alpha){int componentX=7,componentY=11,temp1,temp2,i=19;Font font;'+rect+'else '+text+'}}'
    (O/'Draw.java').write_text(code,encoding='utf-8')
    s=source('StringUtils');(O/'StringUtils.java').write_text('class StringUtils{static JagString ASTERISK=JagString.parse("(Z");'+v.method(s,'public static JagString toString(')+'}',encoding='utf-8')
    s=source('MiniMenu');mini=''.join(re.search(r'public static final JagString '+name+r' = JagString.parse\("[^"\n]*"\);',s)[0]for name in ['COLOR_ITEM_ORANGE','NULL','OPEN_PARENTHESIS'])
    s=source('LocalizedText');constants={name:re.search(r'(?:private|public) static final JagString '+name+r' = JagString.parse\("([^"\n]*)"\);',s)[1]for name in ['ENG_PLEASEWAIT','ENG_MILLION_SUFFIX','ENG_THOUSAND_SUFFIX']}
    (O/'Constants.java').write_text('class MiniMenu{'+mini+'}class LocalizedText{static JagString PLEASEWAIT=JagString.parse("'+constants['ENG_PLEASEWAIT']+'"),MILLION_SHORT=JagString.parse("'+constants['ENG_MILLION_SUFFIX']+'"),THOUSAND_SHORT=JagString.parse("'+constants['ENG_THOUSAND_SUFFIX']+'");}',encoding='utf-8')
    (O/'Support.java').write_text(r'''
    import java.io.*;import java.util.*;import java.nio.charset.StandardCharsets;
    class Log {static List<String>calls=new ArrayList<>();static void add(String name,Object...args){String s="[\""+name+"\"";for(Object o:args)s+=","+(o instanceof String?"\""+o+"\"":o);calls.add(s+"]");}}
    class Component {static boolean loadFailed;int type,color,activeColor,overColor,activeOverColor,objId,objCount,width,height,halign,valign,vpadding;boolean if3,active,filled,shadowed,ready;JagString text,activeText;int[]values;Font getFont(Object icons){Log.add("Font");return ready?new Font():null;}}
    class Sprites{static Object nameIcons;}
    class InterfaceList{static Component hoveredComponent;static void redraw(Component c){Log.add("Redraw");}}
    class ObjType{JagString name;int stackable;}
    class ObjTypeList{static ObjType item;static ObjType get(int id){Log.add("Item",id);return item;}}
    class PrivilegedRequest{int intArg2;Object result;}
    class Player{static PrivilegedRequest lastLogAddress;}
    class GlRenderer{static boolean enabled=true;}
    class GlRaster{static void fillRect(int x,int y,int w,int h,int color){Log.add("Fill",x,y,w,h,color);}static void drawRect(int x,int y,int w,int h,int color){Log.add("Border",x,y,w,h,color);}static void fillRectAlpha(int x,int y,int w,int h,int color,int alpha){Log.add("FillAlpha",x,y,w,h,color,alpha);}static void drawRectAlpha(int x,int y,int w,int h,int color,int alpha){Log.add("BorderAlpha",x,y,w,h,color,alpha);}}
    class SoftwareRaster extends GlRaster{}
    class PluginRepository{static void ComponentDraw(int i,Component c,int x,int y){Log.add("Plugin",i,x,y);}}
    class Font{void drawInterfaceText(JagString s,int x,int y,int w,int h,int color,int shadow,int ha,int va,int pad){Log.add("Text",s.hex(),x,y,w,h,color,shadow,ha,va,pad);}}
    class Probe{
    static JagString string(String hex){if(hex.equals("_"))return null;byte[]b=HexFormat.of().parseHex(hex);return JagString.decodeString(b,b.length,0);}
    public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8));String line;while((line=in.readLine())!=null){String[]p=line.split(";",-1);int[]a=Arrays.stream(p[0].split(",")).mapToInt(Integer::parseInt).toArray();Component c=new Component();c.type=a[0];c.if3=a[1]!=0;c.active=a[2]!=0;InterfaceList.hoveredComponent=a[3]!=0?c:null;Draw.pleaseWaitComponent=a[4]!=0?c:null;c.ready=a[5]!=0;Component.loadFailed=a[6]!=0;c.filled=a[7]!=0;int alpha=a[8];c.shadowed=a[9]!=0;c.color=a[10];c.activeColor=a[11];c.overColor=a[12];c.activeOverColor=a[13];c.objId=a[14];c.objCount=a[15];ObjTypeList.item=new ObjType();ObjTypeList.item.stackable=a[16];c.width=a[17];c.height=a[18];c.halign=a[19];c.valign=a[20];c.vpadding=a[21];Player.lastLogAddress=null;if(a[23]!=0){Player.lastLogAddress=new PrivilegedRequest();Player.lastLogAddress.intArg2=a[22];Player.lastLogAddress.result=p[4].equals("_")?null:new String(HexFormat.of().parseHex(p[4]),StandardCharsets.UTF_8);}c.values=Arrays.copyOfRange(a,24,29);c.text=string(p[1]);c.activeText=string(p[2]);ObjTypeList.item.name=string(p[3]);Log.calls.clear();Draw.render(c,alpha);System.out.println("["+String.join(",",Log.calls)+"]");}}
    }
    ''',encoding='utf-8')
    with(O/'compile.log').open('w')as log:p=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],stdout=log,stderr=subprocess.STDOUT)
    assert p.returncode==0,(O/'compile.log').read_text()
    rng=random.Random(530034);cases=[];commands=[]
    for i in range(2048):
     values=[rng.choice([-2147483648,-1,0,1,999999998,999999999,2147483647,rng.randrange(-50000,50000)])for _ in range(5)]
     count=rng.choice([-2147483648,-1,0,1,999,1000,99999,100000,999999,1000000,2147483647])
     a=[3 if i<768 else 4,int(bool(i&1)),int(bool(i&2)),int(bool(i&4)),int(bool(i&8)),int(i%37!=0),int(bool(i&16)),int(bool(i&32)),rng.choice([-1,0,1,127,128,255,256,257,2147483647]),int(bool(i&64)),*[rng.choice([0,-1,rng.randrange(-2147483648,2147483648)])for _ in range(4)],rng.choice([-1,0,123]),count,rng.randrange(3),rng.randrange(0,530),rng.randrange(0,350),rng.randrange(4),rng.randrange(4),rng.randrange(25),rng.randrange(-2147483648,2147483648),int(i%3!=0),*values]
     text=rng.choice(['plain','base %1 / %2 / %3 / %4 / %5','%dns - %1 %1 %%1','<col=ff0000>caf\xe9 %5</col>','%3%1%2%4%5 %dns %dns','unused %6 %dnS %'])
     active=rng.choice(['','selected %2','active %1 %dns','active',' %3'])
     item=rng.choice([None,'Coins','Sword','item %1'])
     dns=rng.choice([None,'localhost','h\xe9st','user\0host','HOST%1','wide\u03bb'])
     c=dict(arguments=a,text=text,activeText=active,itemName=item,dnsResult=dns);cases.append(c)
     def hextext(s):return '_'if s is None else s.encode('latin1').hex()
     commands.append(','.join(map(str,a))+';'+hextext(text)+';'+hextext(active)+';'+hextext(item)+';'+('_'if dns is None else dns.encode('utf-8').hex()))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Complete unchanged original type3/type4 drawing branches, original count formatting/interpolation and JagString operations under recording font, item, CS1-expression, DNS-result, raster and plugin services. Context/services and full GPU pixels remain separate.',sources=sources,cases=cases),indent=2)+'\n'
    fixture=R/'fixtures/ui/widget-style.json'
    if check:assert fixture.read_text(encoding='utf-8')==content,'widget style fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS original rectangle/text branches',len(cases),'cases',flush=True)

if __name__=="__main__":
    import argparse
    parser=argparse.ArgumentParser();parser.add_argument("--check",action="store_true");generate(parser.parse_args().check)
