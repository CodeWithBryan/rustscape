"""Original ordinary type7/8/9 branches and original GL line vertex calls."""
import hashlib,importlib.util,json,pathlib,random,re,shutil,subprocess

def generate(check=False):
 R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-legacy-draw-oracle';O.mkdir(exist_ok=True)
 spec=importlib.util.spec_from_file_location('visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
 def source(name):
  b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
 s=source('JagString');names=['EMPTY','LINE_BREAK','NULL_STRING','PERIOD','PERCENT_SIGN','CLOSE_COLOR_TIMES','WHITE_COLOR_PREFIX','GREEN_COLOR_PREFIX','YELLOW_COLOR_PREFIX','COMMA','CLOSE_COLOR','CLOSE_PAREN_CLOSE_COLOR'];consts=[]
 for name in names:consts.append(re.search(r'public static final JagString '+name+r' = parse\("[^"\n]*"\);',s)[0])
 methods=['public static JagString concatenate(','public static JagString concatenateRange(','public static void copy(','public static JagString parse(','public static JagString parseInt(int','public static JagString parseInt(boolean','public static JagString decodeString(','public static JagString formatIp(','public final int length()','public final int indexOf(JagString needle)','public final int indexOf(JagString needle,','public final JagString substring(int from)','public final JagString substring(int to,','public final JagString compact()']
 (O/'JagString.java').write_text('import java.nio.charset.StandardCharsets;class JagString{byte[]chars;int length;boolean mutable=true;'+''.join(consts)+''.join(v.method(s,m)for m in methods)+'JagString intern(){return this;}String hex(){return java.util.HexFormat.of().formatHex(chars,0,length);}}')
 s=source('Cs1ScriptRunner');ordinary=s[s.index('if (component.type == 7)'):];branches=[v.method(ordinary,'if (component.type == 7)'),v.method(ordinary,'if (component.type == 8 && Protocol.tooltipComponent'),v.method(ordinary,'if (component.type == 9)')]
 placeholders=''.join(re.findall(r'(?:public|private) static final JagString CS1_PLACEHOLDER_\w+ = JagString.parse\("[^"\n]*"\);',s))
 code='import java.nio.charset.StandardCharsets;class Draw{static final int TOOLTIP_DISPLAY_DELAY=50;'+placeholders+'static int run(int i,Component c){Log.add("Run",i);return c.values[i] + c.calls[i]++;}'+v.method(s,'public static JagString interpolate(')+v.method(s,'public static JagString formatItemCount(')
 code+='static void render(Component component,int componentX,int componentY,int clipRight,int clipBottom){int temp1,temp2,temp3,temp4,temp5,memory,objId,cardMemory,i=19;Font font;ObjType objDef;for(int once=0;once<1;once++){'+''.join(branches)+'}}}'
 (O/'Draw.java').write_text(code)
 s=source('StringUtils');(O/'StringUtils.java').write_text('class StringUtils{static JagString ASTERISK=JagString.parse("(Z");'+v.method(s,'public static JagString toString(')+'}')
 s=source('MiniMenu');mini=''.join(re.search(r'public static final JagString '+name+r' = JagString.parse\("[^"\n]*"\);',s)[0]for name in ['COLOR_ITEM_ORANGE','NULL','OPEN_PARENTHESIS'])
 s=source('LocalizedText');constants={name:re.search(r'(?:private|public) static final JagString '+name+r' = JagString.parse\("([^"\n]*)"\);',s)[1]for name in ['ENG_MILLION_SUFFIX','ENG_THOUSAND_SUFFIX']}
 (O/'Constants.java').write_text('class MiniMenu{'+mini+'}class LocalizedText{static JagString MILLION_SHORT=JagString.parse("'+constants['ENG_MILLION_SUFFIX']+'"),THOUSAND_SHORT=JagString.parse("'+constants['ENG_THOUSAND_SUFFIX']+'");}')
 s=source('GlRaster');(O/'GlRaster.java').write_text('class GlRaster{'+v.method(s,'public static void drawLine(')+v.method(s,'public static void drawThickLine(')+'static void fillRect(int x,int y,int w,int h,int c){Log.add("Fill",x,y,w,h,c);}static void drawRect(int x,int y,int w,int h,int c){Log.add("Border",x,y,w,h,c);}}')
 s=source('Protocol');tick=v.method(s,'if (tooltipComponent == null)');tail=s[s.index(tick)+len(tick):];tick+=v.method(tail,'else if (tooltipTimer < Cs1ScriptRunner.TOOLTIP_DISPLAY_DELAY)')
 (O/'Tick.java').write_text('class Tick{static Component tooltipComponent;static int tooltipTimer;static void step(){'+tick+'}}')
 (O/'Support.java').write_text(r'''
 import java.io.*;import java.util.*;
 class Log{static List<String>calls=new ArrayList<>();static void add(String n,Object...a){String s="[\""+n+"\"";for(Object o:a)s+=","+(o instanceof String?"\""+o+"\"":o);calls.add(s+"]");}}
 class Component{static boolean loadFailed;int type,baseWidth,baseHeight,invMarginX,invMarginY,color,halign,width,height,lineWidth;boolean ready,shadowed,lineFlipped;int[]objTypes,objCounts,values=new int[5],calls=new int[5];JagString text;Font getFont(Object o){Log.add("Font");return ready?new Font():null;}}
 class Font{static int factor;int lineHeight;Font(){lineHeight=Probe.height;}int getStringWidth(JagString s){Log.add("Width",s.hex());return s.length()*factor;}void renderLeft(JagString s,int x,int y,int c,int h){Log.add("Left",s.hex(),x,y,c,h);}void renderCenter(JagString s,int x,int y,int c,int h){Log.add("Center",s.hex(),x,y,c,h);}void renderRight(JagString s,int x,int y,int c,int h){Log.add("Right",s.hex(),x,y,c,h);}}
 class Fonts{static Font p12Full;}
 class Sprites{static Object nameIcons;}
 class InterfaceList{static void redraw(Component c){Log.add("Redraw");}}
 class ObjType{int stackable;JagString name;}
 class ObjTypeList{static ObjType get(int id){Log.add("Item",id);ObjType o=new ObjType();o.stackable=id%3;o.name=JagString.parse(id%2==0?"Coins":"Sword");return o;}}
 class Protocol{static Component tooltipComponent;static int tooltipTimer;}
 class Cs1ScriptRunner{static final int TOOLTIP_DISPLAY_DELAY=50;}
 class PrivilegedRequest{int intArg2;Object result;}
 class Player{static PrivilegedRequest lastLogAddress;}
 class PluginRepository{static void ComponentDraw(int i,Component c,int x,int y){Log.add("Plugin",i,x,y);}}
 class GlRenderer{static boolean enabled=true;static int canvasHeight;static GL2 gl=new GL2();static void begin2DNoTexture(){Log.add("Begin2D");}}
 class GL2{static final int GL_LINE_LOOP=2,GL_TRIANGLE_FAN=6;void glBegin(int i){Log.add("Begin",i);}void glColor3ub(byte r,byte g,byte b){Log.add("Color",r&255,g&255,b&255);}void glVertex2f(float x,float y){Log.add("Vertex",Float.floatToRawIntBits(x),Float.floatToRawIntBits(y));}void glEnd(){Log.add("End");}}
 class SoftwareRaster extends GlRaster{}
 class Probe{static int height;static int[] ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}public static void main(String[]args)throws Exception{BufferedReader in=new BufferedReader(new InputStreamReader(System.in));String s;while((s=in.readLine())!=null){String[]p=s.split(";",-1);int[]a=ints(p[0]);Log.calls.clear();if(a[0]==0){Tick.tooltipTimer=a[1];Tick.tooltipComponent=a[2]==0?null:new Component();Tick.step();System.out.println("["+Tick.tooltipTimer+",["+String.join(",",Log.calls)+"]]");continue;}Component c=new Component();c.type=a[0];c.baseWidth=a[1];c.baseHeight=a[2];c.invMarginX=a[3];c.invMarginY=a[4];c.color=a[5];c.halign=a[6];c.width=a[7];c.height=a[8];c.lineWidth=a[9];c.lineFlipped=a[10]!=0;c.shadowed=a[11]!=0;c.ready=a[12]!=0;Component.loadFailed=a[13]!=0;height=a[14];Font.factor=a[15];Fonts.p12Full=new Font();Protocol.tooltipTimer=a[16];Protocol.tooltipComponent=a[17]!=0?c:null;GlRenderer.canvasHeight=a[22];c.values=Arrays.copyOfRange(a,23,28);if(c.type==7){c.objTypes=ints(p[1]);c.objCounts=ints(p[2]);}byte[]text=HexFormat.of().parseHex(p[3]);c.text=JagString.decodeString(text,text.length,0);Draw.render(c,a[18],a[19],a[20],a[21]);System.out.println("["+String.join(",",Log.calls)+"]");}}}
 ''')
 with(O/'compile.log').open('w')as log:p=subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],stdout=log,stderr=subprocess.STDOUT)
 assert p.returncode==0,(O/'compile.log').read_text()
 rng=random.Random(530789);cases=[];commands=[]
 for i in range(1536):
  kind=[7,8,9][i//512];bw=rng.randrange(0,5);bh=rng.randrange(0,5);n=bw*bh
  a=[kind,bw,bh,rng.choice([-115,-10,0,3,115,2147483647]),rng.choice([-12,0,7,2147483647]),rng.randrange(-2147483648,2147483648),rng.randrange(-1,4),rng.randrange(0,600),rng.randrange(0,350),rng.choice([0,1,2,3,7,255]),i%2,int(i%3==0),int(i%13!=0),int(i%5==0),rng.randrange(5,20),rng.randrange(1,10),rng.choice([0,49,50,51]),int(i%7!=0),rng.randrange(-50,800),rng.randrange(-50,600),rng.randrange(100,900),rng.randrange(100,700),rng.randrange(1,2000),*[rng.choice([-2,0,7,999999998,999999999])for _ in range(5)]]
  ids=[rng.choice([0,-1,1,2,3,4,8])for _ in range(n)] or[0];counts=[rng.choice([-2147483648,-1,0,1,100000,1000000,2147483647])for _ in range(n)] or[0]
  text=rng.choice(['','<br>','one<br>','<br>two','one<br><br>three','one%1<br>two%1','%2%5 %dns','caf\xe9 <col=ff0000>red</col>'])
  row=dict(arguments=a,ids=ids,counts=counts,text=text);cases.append(row);commands.append(';'.join([','.join(map(str,a)),','.join(map(str,ids)),','.join(map(str,counts)),text.encode('latin1').hex()]))
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
 rows=[json.loads(s)for s in p.stdout.splitlines()];assert len(rows)==1536
 for c,row in zip(cases,rows):c['expected']=row
 commands=[f'0,{t},{int(selected)}'for t in[-1,0,1,49,50,51,2147483647]for selected in[False,True]]
 p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);assert p.returncode==0,p.stderr
 ticks=[dict(timer=int(c.split(',')[1]),selected=c.endswith(',1'),expected=json.loads(s))for c,s in zip(commands,p.stdout.splitlines())]
 content=json.dumps(dict(scope='Complete unchanged original ordinary type7/8/9 drawing branches, original JagString interpolation/count formatting, original GlRaster thin/thick line methods, and original tooltip timer branch. Font readiness/width/text drawing, item lookup, CS1 expression results, raster boxes and plugin calls are explicit recording services. Does not establish font pixels, native plugins or complete interface input parity.',sources=sources,cases=cases,ticks=ticks),indent=2)+'\n'
 fixture=R/'fixtures/ui/widget-legacy-draw.json'
 if check:assert fixture.read_text(encoding='utf-8')==content,'legacy draw fixture differs'
 else:fixture.write_text(content,encoding='utf-8')
 print('PASS original legacy type7/8/9 draw branches',len(cases),'and tooltip timer',len(ticks),flush=True)

if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');generate(p.parse_args().check)
