"""Original minimap scenery/indexed blits, terrain passes, colors and pinned metadata."""
import argparse,hashlib,importlib.util,json,pathlib,random,re,struct,subprocess
ROOT=pathlib.Path(__file__).resolve().parents[2]

def generate(check=False):
    spec=importlib.util.spec_from_file_location('visual',ROOT/'oracle/visual/generate.py');v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
    run=ROOT/'runs/minimap-scene-oracle';run.mkdir(parents=True,exist_ok=True);sources={}
    def source(name):
        raw=(ROOT/f'reference/client/client/src/main/java/rt4/{name}.java').read_bytes().replace(b'\r\n',b'\n');sources[name]=hashlib.sha256(raw).hexdigest();return v.clean(raw.decode())
    mini=source('MiniMap');indexed=source('SoftwareIndexedSprite');scene=source('SceneGraph');colors=source('ColorUtils')
    signatures=['public static void blitScaledIndexed(', 'public static void blitScaledIndexedTinted(', 'public final void adjustPalette(', 'public final void flipVertical(', 'public final void trim(', 'public final void renderScaled(', 'public final void renderScaledTinted(']
    (run/'SoftwareIndexedSprite.java').write_text('class SoftwareIndexedSprite {int width,height,innerWidth,innerHeight,xOffset,yOffset;byte[]pixels;int[]pallet;'+''.join(v.method(indexed,s)for s in signatures)+'}',encoding='utf-8')
    tables=''.join(re.search(r'public static final int\[\]\[\] '+n+r' = .*?;',mini).group()for n in ['tileShapeMasks','tileShapeRotations'])
    (run/'MiniMap.java').write_text('class MiniMap{'+tables+v.method(mini,'public static void renderTile(')+v.method(mini,'public static boolean renderScenery(')+v.method(mini,'public static boolean renderMapTileFeatures(')+'}',encoding='utf-8')
    background=mini[mini.index('int[] pixelData = softwareSprite.pixels;'):mini.index('locs = 0;',mini.index('int[] pixelData = softwareSprite.pixels;'))]
    under=scene[scene.index('if (k <= 0) {'):scene.index('s = tileLightness[i][j];',scene.index('if (k <= 0) {'))]
    over=scene[scene.index('texture = overlayType.texture;'):scene.index('setTile(currentLevel, i, j, u, overlayAngle, texture,',scene.index('texture = overlayType.texture;'))]
    code='''import java.io.*;import java.util.*;import java.security.*;import java.nio.*;
class SoftwareRaster{static int[]pixels;static int width=512,clipLeft=0,clipTop=0,clipRight=512,clipBottom=512;}
class PlainTile{int flatColor;}
class ShapedTile{int shape,rotation,textureId,flatColor;}
class Tile{PlainTile plainTile;ShapedTile shapedTile;}
class SceneGraph{static Tile[][][]tiles=new Tile[4][104][104];static byte[][][]renderFlags=new byte[4][104][104];static long key;static int layer;static long getWallKey(int p,int x,int y){return layer==0?key:0;}static long getSceneryKey(int p,int x,int y){return layer==2?key:0;}static long getGroundDecorKey(int p,int x,int y){return layer==3?key:0;}}
class LocType{int mapscene=-1,mapSceneAngleOffset,width=1,length=1;boolean mapSceneRotated;}
class LocTypeList{static LocType value;static LocType get(int id){return value;}}
class MsiType{int spriteId,tintColor;boolean stretchToTile;SoftwareIndexedSprite getSprite(int r){return MsiTypeList.sprites[r];}}
class MsiTypeList{static MsiType value;static SoftwareIndexedSprite[]sprites;static MsiType get(int id){return value;}}
class FloType{int texture,baseColor,secondaryColor;}
class Provider{boolean repeating;int average;boolean isTextureRepeating(int id){return repeating;}int getAverageColor(int id){return average;}}
class Rasteriser{static int[]palette=new int[65536];static Provider textureProvider=new Provider();static{for(int i=0;i<65536;i++)palette[i]=i;}}
class ColorUtils{'''+v.method(colors,'public static int multiplyLightnessSafe(')+v.method(colors,'public static int multiplyLightnessGrayscale(')+'''}
public class Probe{static void renderTile(int[]p,int i,int l,int x,int y){MiniMap.renderTile(p,i,l,x,y);}static int[]background(int plane){SoftwareSprite softwareSprite=new SoftwareSprite();int pixelCount,i,pixelOffset,j;'''+background+'''return softwareSprite.pixels;}
static int[]floor(int k,int q,int hueJitter,int lightnessJitter,FloType overlayType){int level=lightnessJitter>>2<<10,x=hueJitter>>1,r,s,t;int[][]underlayColors={{q}};int i=0,j=0;'''+under+'''int overlayBaseColor,overlayRgb,texture;'''+over+'''return new int[]{r,overlayRgb};}
static class SoftwareSprite{int[]pixels=new int[512*512];}
static String hash(int[]pixels)throws Exception{ByteBuffer b=ByteBuffer.allocate(pixels.length*4);for(int c:pixels)b.putInt(c);return HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(b.array()));}
static int[]ints(String s){return Arrays.stream(s.split(",")).mapToInt(Integer::parseInt).toArray();}
static SoftwareIndexedSprite sprite(int[]n,byte[]pixels,int[]palette){SoftwareIndexedSprite s=new SoftwareIndexedSprite();s.width=n[0];s.height=n[1];s.innerWidth=n[2];s.innerHeight=n[3];s.xOffset=n[4];s.yOffset=n[5];s.pixels=pixels;s.pallet=palette;return s;}
public static void main(String[]args)throws Exception{BufferedReader input=new BufferedReader(new InputStreamReader(System.in));String line;while((line=input.readLine())!=null){String[]p=line.split(";",-1);int[]n=ints(p[1]);SoftwareRaster.pixels=new int[512*512];Arrays.fill(SoftwareRaster.pixels,0x123456);switch(p[0]){
case "F":{SceneGraph.layer=n[2];SceneGraph.key=(1L<<32)|((long)n[3]<<14)|((long)n[4]<<20);if(n[5]==0)SceneGraph.key|=Long.MIN_VALUE;LocTypeList.value=new LocType();MiniMap.renderMapTileFeatures(n[0],n[6],n[1],n[7],0);int base=(103-n[1])*2048+n[0]*4+24624;int[]out=new int[16];for(int i=0;i<16;i++)out[i]=SoftwareRaster.pixels[base+i/4*512+i%4];System.out.println(Arrays.toString(out));break;}
case "S":{SoftwareIndexedSprite s=sprite(n,HexFormat.of().parseHex(p[2]),ints(p[3]));s.adjustPalette(new int[]{n[6],n[7],n[8]}[0],n[7],n[8]);if(n[9]==1){s.innerWidth=s.width;s.innerHeight=s.height;}for(int i=0;i<n[10];i++)s.flipVertical();if(n[11]==1)s.trim();if(n[16]==0)s.renderScaled(n[12],n[13],n[14],n[15]);else s.renderScaledTinted(n[12],n[13],n[14],n[15],n[16]);System.out.println(hash(SoftwareRaster.pixels));break;}
case "B":{int seed=n[0],plane=n[1];for(int p1=0;p1<4;p1++)for(int x1=0;x1<104;x1++)for(int y1=0;y1<104;y1++){int z=(seed+p1*71+x1*17+y1*31)&65535;SceneGraph.renderFlags[p1][x1][y1]=(byte)(z%13==0?8:z%11==0?24:0);Tile tile=new Tile();if(z%7==0){SceneGraph.tiles[p1][x1][y1]=null;continue;}int shape=z%13;if(shape<2){tile.plainTile=new PlainTile();tile.plainTile.flatColor=z%5==0?0:z+1;}else{tile.shapedTile=new ShapedTile();tile.shapedTile.shape=shape;tile.shapedTile.rotation=z&3;tile.shapedTile.textureId=z%5==0?0:z+2;tile.shapedTile.flatColor=z%3==0?0:z+3;}SceneGraph.tiles[p1][x1][y1]=tile;}System.out.println(hash(background(plane)));break;}
case "C":{FloType f=new FloType();f.texture=n[2];f.baseColor=n[3];f.secondaryColor=n[4];Rasteriser.textureProvider.repeating=n[5]!=0;Rasteriser.textureProvider.average=n[6];System.out.println(Arrays.toString(floor(n[0]>=0?1:0,n[0],n[7],n[8],f)));break;}
default:throw new IllegalArgumentException(p[0]);}}}}
'''
    (run/'Probe.java').write_text(code,encoding='utf-8');subprocess.run(['javac','-encoding','UTF-8','-d',str(run),*[str(p)for p in run.glob('*.java')]],check=True)
    rng=random.Random(530);cases=[];requests=[]
    def add(kind,args,**extra):cases.append(dict(kind=kind,args=args,**extra));requests.append(kind+';'+','.join(map(str,args))+(';' + extra['pixels']+';'+','.join(map(str,extra['palette']))if extra else''))
    for layer in [0,2,3]:
        for shape in [0,1,2,3,9,10,22]:
            for angle in range(4):
                for positive in [0,1]:add('F',[rng.randrange(104),rng.randrange(104),layer,shape,angle,positive,0xaabbcc,0xff0000])
    for i in range(512):
        w=rng.randrange(1,33);h=rng.randrange(1,33);cropped=i%2==0;off=[rng.randrange(8),rng.randrange(8)]if cropped else[0,0];inner=[w+off[0]+rng.randrange(8),h+off[1]+rng.randrange(8)]if cropped else[w,h]
        args=[w,h,*inner,*off,rng.randrange(-30,31),rng.randrange(-30,31),rng.randrange(-30,31),int(not cropped),rng.randrange(4),int(i%3==0),rng.randrange(-40,513),rng.randrange(-40,513),rng.randrange(1,129),rng.randrange(1,129),rng.choice([0,0x223344,0xffffff])]
        add('S',args,pixels=bytes(rng.randrange(8)for _ in range(w*h)).hex(),palette=[rng.randrange(0x1000000)for _ in range(8)])
    for _ in range(32):add('B',[rng.randrange(65536),rng.randrange(4)])
    for _ in range(1024):add('C',[rng.choice([-1,rng.randrange(65536)]),0,rng.choice([-1,0]),rng.choice([-1,rng.randrange(65536)]),rng.choice([-1,rng.randrange(65536)]),rng.randrange(2),rng.randrange(65536),rng.randrange(-16,17),rng.randrange(-8,9)])
    output=subprocess.run(['java','-cp',str(run),'Probe'],input='\n'.join(requests)+'\n',text=True,capture_output=True,check=True).stdout.splitlines();assert len(output)==len(cases)
    for c,line in zip(cases,output):c['expected']=json.loads(line)if c['kind']in['F','C']else line
    data=(json.dumps(dict(sources=sources,bindings='Unchanged original indexed sprite transforms/blits, map wall/scenery-feature strokes, first renderMap terrain pass and SceneGraph flat-color expression spans. Synthetic scene/indexed art, identity RGB palette and explicit texture metadata are services; raw cache metadata has a separate corpus.',cases=cases),separators=(',',':'))+'\n').encode();path=ROOT/'fixtures/minimap/scene.json'
    if check:
        if path.read_bytes()!=data:raise SystemExit('Original minimap scene corpus differs')
    else:path.write_bytes(data)
    print(f'Original minimap scene: {len(cases)} indexed/wall/bridge/color cases')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--check',action='store_true');generate(p.parse_args().check)
