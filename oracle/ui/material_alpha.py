"""Original specular material opacity under explicit fixed-function GL services."""
import argparse,hashlib,importlib.util,json,pathlib,re,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2];RUN=ROOT/'runs/material-alpha-oracle';FIX=ROOT/'fixtures/ui/material-alpha.json'
def generate(check=False):
    spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    sources={}
    def source(name):
        b=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    original=source('SpecularMaterialRenderer');renderer=source('GlRenderer');source('Js5GlTextureProvider');source('MaterialManager')
    methods=''.join(v.method(original,s)for s in ['private void initDisplayLists()','public final void bind()','public final int getFlags()'])
    methods=methods.replace('@Override','')
    code='class Specular{int displayListBase;int[]cubeMapTextureIds;boolean limitedTextureUnits;Specular(boolean cube,int units){cubeMapTextureIds=cube?new int[]{1,2,3}:null;limitedTextureUnits=units<3;initDisplayLists();}'+methods+'}'
    combine=v.method(renderer,'public static void setTextureCombineAlphaMode(')
    constants=sorted(set(re.findall(r'GL2\.(GL_\w+)',code+combine)))
    declarations=''.join(f'static final int {name}={i+1};'for i,name in enumerate(constants))
    support=SUPPORT.replace('CONSTANTS',declarations).replace('COMBINE_METHOD',combine)
    RUN.mkdir(parents=True,exist_ok=True);(RUN/'AlphaOracle.java').write_text(code+support)
    subprocess.run(['javac','-encoding','UTF-8','-d',str(RUN),str(RUN/'AlphaOracle.java')],check=True)
    cases=[];requests=[]
    for cube in [False,True]:
        for units in [2,3,8]:
            for detail in [False,True]:
                for vertex in [0,1,128,255]:
                    for texture in [0,1,64,127,128,254,255]:
                        cases.append(dict(cube=cube,units=units,detail=detail,vertex=vertex,texture=texture));requests.append(f'{int(cube)},{units},{int(detail)},{vertex},{texture}')
    p=subprocess.run(['java','-cp',str(RUN),'AlphaOracle'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True)
    lines=p.stdout.splitlines();assert len(lines)==len(cases)
    for c,line in zip(cases,lines):c['expected']=json.loads(line)
    out=dict(sources=sources,bindings='Complete original SpecularMaterialRenderer initDisplayLists/bind/getFlags and GlRenderer.setTextureCombineAlphaMode. Explicit constructor capability fields, display-list recording/replay and fixed-function alpha stage evaluator. Cube texture alpha is controlled; this proves restored primary opacity with/without cube maps and high-detail lighting, not specular highlight RGB/cube generation or pixel parity.',cases=cases)
    if check:
        if json.loads(FIX.read_text())!=out:raise SystemExit('Material opacity fixture differs')
    else:FIX.write_text(json.dumps(out,separators=(',',':'))+'\n')
    print('Original specular material opacity:',len(cases),'cases')
SUPPORT=r'''
class Preferences{static boolean highDetailLighting;}
class GlRenderer{static GL2 gl;static int textureCombineAlphaMode,defaultTextureId=1;COMBINE_METHOD}
class GL2{
CONSTANTS
int unit,list=-1,next=1;int[]mode=new int[3],src=new int[3];boolean[]enabled={true,false,false};java.util.Map<Integer,java.util.List<Runnable>>lists=new java.util.HashMap<>();
GL2(){java.util.Arrays.fill(mode,GL_MODULATE);java.util.Arrays.fill(src,GL_TEXTURE);}
void command(Runnable r){if(list==-1)r.run();else lists.get(list).add(r);}
int glGenLists(int n){int id=next;next+=n;return id;}
void glNewList(int id,int ignored){list=id;lists.put(id,new java.util.ArrayList<>());}
void glEndList(){list=-1;}
void glCallList(int id){for(Runnable r:lists.get(id))r.run();}
void glActiveTexture(int t){command(()->unit=t==GL_TEXTURE0?0:t==GL_TEXTURE1?1:2);}
void glTexEnvi(int env,int key,int value){command(()->{if(key==GL_COMBINE_ALPHA)mode[unit]=value;if(key==GL_SRC0_ALPHA)src[unit]=value;});}
void glEnable(int v){command(()->{if(v==GL_TEXTURE_2D||v==GL_TEXTURE_CUBE_MAP)enabled[unit]=true;});}
void glDisable(int v){command(()->{if(v==GL_TEXTURE_2D||v==GL_TEXTURE_CUBE_MAP)enabled[unit]=false;});}
void glMatrixMode(int x){}void glLoadIdentity(){}void glRotatef(float a,float x,float y,float z){}void glTexGeni(int a,int b,int c){}void glBindTexture(int a,int b){}
double alpha(int primary,int texture){double previous=primary;for(int i=0;i<3;i++){if(!enabled[i])continue;double tex=i==0?texture:127;double a=src[i]==GL_PRIMARY_COLOR?primary:src[i]==GL_PREVIOUS?previous:tex;if(mode[i]==GL_REPLACE)previous=a;else if(mode[i]==GL_MODULATE)previous=a*previous/255.0;else throw new AssertionError("Unexpected alpha mode");}return previous;}}
class AlphaOracle{public static void main(String[]args)throws Exception{java.io.BufferedReader r=new java.io.BufferedReader(new java.io.InputStreamReader(System.in));String s;while((s=r.readLine())!=null){int[]p=java.util.Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();GlRenderer.gl=new GL2();GlRenderer.textureCombineAlphaMode=0;Preferences.highDetailLighting=p[2]!=0;Specular m=new Specular(p[0]!=0,p[1]);m.bind();System.out.println("["+m.getFlags()+","+(int)GlRenderer.gl.alpha(p[3],p[4])+"]");}}}
'''
if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
