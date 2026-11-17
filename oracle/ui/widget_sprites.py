"""Draft original Component.getSprite preparation; existing source/fixtures untouched."""
import argparse,pathlib,json,hashlib,re,subprocess,struct,random,importlib.util

def generate(check=False):
    R=pathlib.Path(__file__).resolve().parents[2];O=R/'runs/ui-widget-sprite-oracle';O.mkdir(exist_ok=True)
    spec=importlib.util.spec_from_file_location('widget_visual',R/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);sources={}
    def src(name):
     b=(v.SRC/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');sources[name+'.java']=hashlib.sha256(b).hexdigest();return v.clean(b.decode())
    b=src('Buffer');(O/'Buffer.java').write_text('class Buffer {byte[]data;int offset;Buffer(byte[]b){data=b;}'+''.join(v.method(b,'public final '+sig)for sig in ['int g1()','byte g1b()','int g2()','int g3()'])+'}',encoding='utf-8')
    s=src('SpriteLoader');fields=''.join(re.findall(r'public static (?:int|byte|boolean)(?:\[\])* \w+;',s));(O/'SpriteLoader.java').write_text('class SpriteLoader{'+fields+''.join(v.method(s,sig)for sig in ['public static boolean decode(Js5 js5,','public static void decode(byte[] data)','public static SoftwareSprite loadSoftwareSprite(','public static SoftwareSprite createSingleSoftwareSprite()','public static void clear()'])+'}',encoding='utf-8')
    s=src('SoftwareSprite');(O/'SoftwareSprite.java').write_text('class SoftwareSprite extends Sprite {int[]pixels;'+''.join(v.method(s,sig)for sig in ['public SoftwareSprite(int innerWidth,','public static SoftwareSprite loadSoftwareAlphaSprite(','public static SoftwareSprite createSoftwareAlphaSprite()','public final void flipVertical()','public final void flipHorizontal()','public final void pad(','public final void drawOutline(','public final void drawShadow('])+'}',encoding='utf-8')
    s=src('SoftwareAlphaSprite');(O/'SoftwareAlphaSprite.java').write_text('class SoftwareAlphaSprite extends SoftwareSprite {'+v.method(s,'public SoftwareAlphaSprite(int innerWidth,')+'}',encoding='utf-8')
    s=src('Component');(O/'Component.java').write_text('class Component{static boolean loadFailed;static Cache sprites=new Cache();int spriteId,activeSpriteId,outlineThickness,shadowColor;boolean vFlip,hFlip,hasAlpha;'+v.method(s,'public final Sprite getSprite(')+'}',encoding='utf-8')
    # Retain the unchanged original GL upload conversion before the actual GL service.
    methods=[]
    for name,signature,boundary in [('GlSprite','protected void uploadPixels(','ByteBuffer buffer'),('GlAlphaSprite','protected final void uploadPixels(','ByteBuffer buf')]:
     s=src(name);m=v.method(s,signature);m=m[:m.index(boundary)];m=m[:m.rfind('\n')]+ '\n return rgba;}';m=m.replace('protected final void','public byte[]').replace('protected void','public byte[]');methods.append('class Upload'+name+' {int width,height,powerOfTwoWidth,powerOfTwoHeight;'+m+'}')
    (O/'Upload.java').write_text('\n'.join(methods),encoding='utf-8')
    resize=v.method(src('GlSprite'),'public final void renderResized(')
    resize=resize[:resize.index('GL2 gl')];resize=resize[:resize.rfind('\n')]+ '\n return new int[]{x,y,destWidth,destHeight};}'
    resize=resize.replace('public final void','public int[]').replace('return;','return null;')
    (O/'Resize.java').write_text('class Resize{int width,height,innerWidth,innerHeight,xOffset,yOffset;'+resize+'}',encoding='utf-8')

    (O/'Support.java').write_text('''
    class Sprite {int innerWidth,innerHeight,xOffset,yOffset,width,height;}
    class GlRenderer {static boolean enabled=false;static void begin2DReplace(){}}
    class GlSprite extends Sprite {GlSprite(SoftwareSprite s){throw new AssertionError("GPU adapter is separate");}}
    class GlAlphaSprite extends Sprite {GlAlphaSprite(SoftwareSprite s){throw new AssertionError("GPU adapter is separate");}}
    class Cache {long key;Object value;Object get(long k){key=k;return null;}void put(Object v,long k){key=k;value=v;}}
    class Js5 {byte[]data;byte[]fetchFile(int id,int group){return data;}}
    class InterfaceList {static Js5 spriteProvider=new Js5();}

    ''',encoding='utf-8')
    (O/'IntUtils.java').write_text('class IntUtils{'+v.method(src('IntUtils'),'public static int clp2(')+'}',encoding='utf-8')
    (O/'Probe.java').write_text('import java.io.*;import java.util.*;import java.security.*;\npublic class Probe {\n static String sha(byte[]b)throws Exception{return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(b));}\n public static void main(String[]args)throws Exception{\n  BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String l;\n  while((l=r.readLine())!=null){\n   String[]p=l.split(";");InterfaceList.spriteProvider.data=HexFormat.of().parseHex(p[0]);\n   int[]a=Arrays.stream(p[1].split(",")).mapToInt(Integer::parseInt).toArray();\n   Component c=new Component();c.spriteId=a[0];c.activeSpriteId=a[1];c.hasAlpha=a[2]!=0;c.hFlip=a[3]!=0;c.vFlip=a[4]!=0;c.outlineThickness=a[5];c.shadowColor=a[6];Component.sprites=new Cache();\n   SoftwareSprite s=(SoftwareSprite)c.getSprite(a[7]!=0);if(s==null){System.out.println("null");continue;}\n   boolean alpha=s instanceof SoftwareAlphaSprite;byte[]rgba;int stride;\n   if(alpha){UploadGlAlphaSprite u=new UploadGlAlphaSprite();u.width=s.width;u.height=s.height;rgba=u.uploadPixels(s.pixels);stride=u.powerOfTwoWidth;}\n   else{UploadGlSprite u=new UploadGlSprite();u.width=s.width;u.height=s.height;rgba=u.uploadPixels(s.pixels);stride=u.powerOfTwoWidth;}\n   byte[]trim=new byte[s.width*s.height*4];for(int y=0;y<s.height;y++)System.arraycopy(rgba,y*stride*4,trim,y*s.width*4,s.width*4);\n   ByteArrayOutputStream buf=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(buf);for(int value:s.pixels)out.writeInt(value);\n   int[]rect=Arrays.stream(p[2].split(",")).mapToInt(Integer::parseInt).toArray();Resize resize=new Resize();resize.width=s.width;resize.height=s.height;resize.innerWidth=s.innerWidth;resize.innerHeight=s.innerHeight;resize.xOffset=s.xOffset;resize.yOffset=s.yOffset;int[]placed=resize.renderResized(rect[0],rect[1],rect[2],rect[3]);\n   System.out.println(Long.toUnsignedString(Component.sprites.key)+";"+s.innerWidth+","+s.innerHeight+";"+s.xOffset+","+s.yOffset+";"+s.width+","+s.height+";"+alpha+";"+sha(buf.toByteArray())+";"+sha(trim)+";"+(placed==null?"null":Arrays.toString(placed))+";"+sha(rgba));\n  }\n }\n}\n',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(O),*[str(p)for p in O.glob('*.java')]],check=True)
    meta=json.loads((R/'fixtures/ui/sprites.json').read_text());blob=(R/'fixtures/ui/sprites.bin').read_bytes();rng=random.Random(530005);cases=[];inputs=[]
    for i,row in enumerate(meta['cases']):
     raw=blob[row['offset']:row['offset']+row['size']]
     for alpha in [False,True]:
      opt=dict(has_alpha=alpha,h_flip=False,v_flip=False,outline=0,shadow=0)
      cases.append(dict(asset=row,rawSha256=hashlib.sha256(raw).hexdigest(),options=opt,ids=[17,29],active=False));inputs.append(raw)
     if i%13==0:
      for flags in range(16):
       opt=dict(has_alpha=bool(flags&1),h_flip=bool(flags&2),v_flip=bool(flags&4),outline=rng.choice([0,1,2,3,7]),shadow=rng.choice([0,1,0x302020,-1,0x7fffffff]))
       cases.append(dict(asset=row,rawSha256=hashlib.sha256(raw).hexdigest(),options=opt,ids=[17,29],active=bool(flags&8)));inputs.append(raw)
    # Small authored raw sheets exercise the same unchanged complete original loader.
    def sheet(inner,offset,size,indices,alpha,columns):
     w,h=size;flags=(1 if columns else 0)|(2 if alpha is not None else 0)
     def plane(data):return bytes(data[y*w+x]for x in range(w)for y in range(h))if columns else bytes(data)
     pixels=bytes([flags])+plane(indices)+(plane(alpha)if alpha is not None else b'')
     palette=bytes([0,0,1,255,128,32])
     meta=struct.pack('>HHBHHHH',*inner,2,*offset,*size)
     return pixels+palette+meta+struct.pack('>H',1)
    for i in range(512):
     inner=[rng.randrange(1,25),rng.randrange(1,25)];size=[rng.randrange(1,inner[0]+1),rng.randrange(1,inner[1]+1)];offset=[rng.randrange(inner[a]-size[a]+1)for a in range(2)];count=size[0]*size[1];raw=sheet(inner,offset,size,[rng.randrange(3)for _ in range(count)],[rng.choice([0,1,127,128,254,255])for _ in range(count)]if i%3 else None,bool(i&1))
     opt=dict(has_alpha=bool(i&2),h_flip=bool(i&4),v_flip=bool(i&8),outline=rng.choice([-1,0,1,2,3,255,4096]),shadow=rng.choice([0,1,-1,-2147483648,2147483647,rng.randrange(0x1000000)]))
     ids=[rng.choice([-2147483648,-2,-1,0,1,65535,2147483647]),rng.choice([-2147483648,-2,-1,0,1,65535,2147483647])]
     cases.append(dict(raw=raw.hex(),rawSha256=hashlib.sha256(raw).hexdigest(),options=opt,ids=ids,active=bool(i&16)));inputs.append(raw)
    commands=[]
    for i,c in enumerate(cases):c['rect']=[-7+i%31,-13+i%23,[0,1,15,32,53,101,420][i%7],[0,1,17,33,71,127,312][i%7]]
    for c,raw in zip(cases,inputs):
     o=c['options'];commands.append(raw.hex()+';'+','.join(map(str,[*c['ids'],int(o['has_alpha']),int(o['h_flip']),int(o['v_flip']),o['outline'],o['shadow'],int(c['active'])]))+';'+','.join(map(str,c['rect'])))
    p=subprocess.run(['java','-cp',str(O),'Probe'],input='\n'.join(commands)+'\n',text=True,capture_output=True);(O/'stdout.txt').write_text(p.stdout);(O/'stderr.txt').write_text(p.stderr);assert p.returncode==0,p.stderr
    rows=[]
    for l in p.stdout.splitlines():
     if l=='null':rows.append(None);continue
     a=l.split(';');rows.append(dict(key=a[0],inner=list(map(int,a[1].split(','))),offset=list(map(int,a[2].split(','))),size=list(map(int,a[3].split(','))),alpha=a[4]=='true',pixelsSha256=a[5],rgbaSha256=a[6],resized=json.loads(a[7]),backingRgbaSha256=a[8]))
    assert len(rows)==len(cases)
    for c,row in zip(cases,rows):c['expected']=row
    content=json.dumps(dict(scope='Complete unchanged Component.getSprite, original SpriteLoader/SoftwareSprite alpha mode and flip/pad/outline/shadow plus original GL upload pixel conversion prefix. Cache is an empty recording service for each call; original resized GL integer bounds and power-of-two upload bytes are compared. Full soft-reference/GC and complete GPU-frame fidelity remain open.',sources=sources,sourceSpritesSha256=hashlib.sha256(blob).hexdigest(),cases=cases),indent=2)+'\n'
    fixture=R/'fixtures/ui/widget-sprites.json'
    if check:assert fixture.read_text()==content,'widget sprite fixture differs'
    else:fixture.write_text(content,encoding='utf-8')
    print('PASS original widget image preparation',len(cases),'cases')

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');generate(parser.parse_args().check)
