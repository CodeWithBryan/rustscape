"""Every cached SpriteLoader frame, alpha plane and decoded sprite pixels."""
import hashlib,json,pathlib,re,struct,subprocess
def generate(root,clean,method,check=False):
    src=root/'reference/client/client/src/main/java/rt4';run=root/'runs/ui-oracle/sprites';run.mkdir(parents=True,exist_ok=True);hashes={}
    def source(name):
        raw=(src/(name+'.java')).read_bytes().replace(b'\r\n',b'\n');hashes[name]=hashlib.sha256(raw).hexdigest();return clean(raw.decode())
    sprite=source('SpriteLoader');fields=''.join(re.findall(r'public static (?:int|byte|boolean)(?:\[\])* \w+;',sprite))
    (run/'SpriteLoader.java').write_text('class SpriteLoader {'+fields+''.join(method(sprite,s)for s in ['public static void decode(byte[] data)', 'public static Sprite[] createAlphaSprites()', 'public static void clear()'])+'}',encoding='utf-8')
    buffer=source('Buffer');(run/'Buffer.java').write_text('class Buffer{byte[] data;int offset;Buffer(byte[] data){this.data=data;}'+''.join(method(buffer,s)for s in ['public final int g1()', 'public final byte g1b()', 'public final int g2()', 'public final int g3()'])+'}',encoding='utf-8')
    (run/'Probe.java').write_text('''import java.io.*;import java.util.*;
class GlRenderer{static boolean enabled=false;}class Sprite{int[] pixels;boolean alpha;}class SoftwareSprite extends Sprite{SoftwareSprite(int a,int b,int c,int d,int w,int h,int[] pixels){this.pixels=pixels;}}class SoftwareAlphaSprite extends SoftwareSprite{SoftwareAlphaSprite(int a,int b,int c,int d,int w,int h,int[] pixels){super(a,b,c,d,w,h,pixels);alpha=true;}}class GlSprite extends SoftwareSprite{GlSprite(int a,int b,int c,int d,int w,int h,int[] p){super(a,b,c,d,w,h,p);}}class GlAlphaSprite extends SoftwareAlphaSprite{GlAlphaSprite(int a,int b,int c,int d,int w,int h,int[] p){super(a,b,c,d,w,h,p);}}
public class Probe{public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in));String line;while((line=r.readLine())!=null){SpriteLoader.decode(HexFormat.of().parseHex(line));ByteArrayOutputStream bytes=new ByteArrayOutputStream();DataOutputStream out=new DataOutputStream(bytes);out.writeInt(SpriteLoader.width);out.writeInt(SpriteLoader.height);out.writeInt(SpriteLoader.frames);out.writeInt(SpriteLoader.palette.length);for(int p:SpriteLoader.palette)out.writeInt(p);for(int i=0;i<SpriteLoader.frames;i++){out.writeInt(SpriteLoader.xOffsets[i]);out.writeInt(SpriteLoader.yOffsets[i]);out.writeInt(SpriteLoader.innerWidths[i]);out.writeInt(SpriteLoader.innerHeights[i]);out.writeBoolean(SpriteLoader.hasAlpha[i]);out.writeInt(SpriteLoader.pixels[i].length);out.write(SpriteLoader.pixels[i]);out.write(SpriteLoader.alpha[i]);}Sprite[] sprites=SpriteLoader.createAlphaSprites();for(Sprite s:sprites)for(int p:s.pixels){out.writeByte(p>>16);out.writeByte(p>>8);out.writeByte(p);out.writeByte(s.alpha?p>>>24:p==0?0:255);}System.out.println(HexFormat.of().formatHex(java.security.MessageDigest.getInstance("SHA-256").digest(bytes.toByteArray())));}}}
''',encoding='utf-8')
    subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    data=(root/'assets/visual/demo.pack').read_bytes();n=struct.unpack_from('<I',data,8)[0];manifest=json.loads(data[12:12+n]);start=12+n;blob=bytearray();cases=[];inputs=[]
    for name,(offset,size)in manifest['files'].items():
        if name.startswith('sprite/'):
            raw=data[start+offset:start+offset+size];cases.append(dict(name=name,offset=len(blob),size=size));blob.extend(raw);inputs.append(raw.hex())
    result=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(inputs)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(result)==len(cases)
    for c,line in zip(cases,result):c['sha256']=line
    content=json.dumps(dict(sources=hashes,bindings='Software sprite constructors retain original decoded pixels; no GL upload or widget rendering',cases=cases),separators=(',',':')).encode()+b'\n'
    for name,data in [('sprites.json',content),('sprites.bin',blob)]:
        p=root/'fixtures/ui'/name
        if check:
            if p.read_bytes()!=data:raise SystemExit(name+' differs')
        else:p.write_bytes(data)
    print(f'RT4 complete sprite sheets: {len(cases)}')
if __name__=='__main__':
    import importlib.util,sys
    root=pathlib.Path(__file__).resolve().parents[2];spec=importlib.util.spec_from_file_location('visual',root/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v);generate(root,v.clean,v.method,'--check'in sys.argv)
