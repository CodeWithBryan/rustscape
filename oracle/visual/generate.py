"""Run copied pinned Java methods as an oracle; --check never rewrites fixtures.

Requires the prepared visual cache corpus. Source methods are extracted verbatim,
with only dependency adapters and annotation removal; the reference is never edited.
"""
import argparse
import hashlib
import json
import pathlib
import re
import shutil
import struct
import subprocess
def _recorded(root,name):
    # The packs the JVM fixtures were recorded from: kept under recorded/ once
    # the default packs are rebuilt natively (crates/client/tests/support/packs.rs).
    kept=root/'assets/visual/recorded'/name
    return kept if kept.is_file() else root/'assets/visual'/name

ROOT=pathlib.Path(__file__).resolve().parents[2]
SRC=ROOT/"reference/client/client/src/main/java/rt4"
RUN=ROOT/"runs/visual-oracle"
FIX=ROOT/"fixtures/visual"

def clean(source):
    source=re.sub(r"^\s*(?:package|import org\.openrs2)[^\n]*\n","",source,flags=re.M)
    source=re.sub(r"^\s*@(?:OriginalClass|OriginalMember)[^\n]*\n","",source,flags=re.M)
    return re.sub(r"@(?:OriginalArg|Pc)\([^)]*\)\s*","",source)

def method(source,signature):
    start=source.index(signature)
    opening=source.index("{",start)
    depth=1;end=opening+1
    while depth:
        depth+=(source[end]=="{")-(source[end]=="}")
        end+=1
    return source[start:end]

def generate(build_only=False):
    RUN.mkdir(parents=True,exist_ok=True)
    sources={}
    def source(name):
        raw=(SRC/(name+".java")).read_bytes().replace(b"\r\n",b"\n");sources[name+".java"]=hashlib.sha256(raw).hexdigest()
        return clean(raw.decode())
    raw=source("RawModel")
    fields=raw[raw.index("public final class"):raw.index("private RawModel()")]
    fields=fields.replace("public final class RawModel extends Entity","public final class RawModel extends Entity")
    model=fields+method(raw,"public RawModel(byte[] src)")+"\n"+method(raw,"public RawModel(RawModel[] models, int count)")+"\n"+method(raw,"private int addVertex(")+"\n"+method(raw,"private void decodeNew(")+"\n"+method(raw,"private void decodeOld(")+"\n"+method(raw,"public final void calculateNormals()")+"\n"+method(raw,"public static RawModel create(")+"\n"+method(raw,"public final void recolor(")+"\n"+method(raw,"public final void retexture(")+"\n}"
    model=model[:-1]+method(raw,"public RawModel(int vertexCount,")+method(raw,"public final int addTriangle(")+method(raw,"public final int addOrReuseGroundVertex(")+"GlModel createModel(int a,int c,int x,int y,int z){GlModel g=new GlModel(this,a,c,true);g.createBones();return g;}}"
    (RUN/"RawModel.java").write_text(model)
    for name in ["AnimBase","AnimFrame","MathUtils","PerlinNoise","SynthSound","SynthInstrument","SynthEnvelope","SynthFilter","ShapedTile","VertexNormal","TriangleNormal","ArrayUtils","FluType","FloType","IdkType","Node","LinkedList"]:
        (RUN/(name+".java")).write_text(source(name))
    software=source("SoftwareModel");blend=source("Model")
    pose="""class PoseModel {
    int[] vertexX,vertexY,vertexZ;int[][] boneVertices,boneTriangles;byte[] triangleAlpha;short[] triangleColors;boolean colorsModified;static int originX,originY,originZ;
    PoseModel(RawModel m){vertexX=m.vertexX;vertexY=m.vertexY;vertexZ=m.vertexZ;triangleAlpha=m.triangleAlpha;triangleColors=m.triangleColors;boneVertices=groups(m.vertexBones,m.vertexCount);boneTriangles=m.triangleBones==null?null:groups(m.triangleBones,m.triangleCount);originX=originY=originZ=0;}
    static int[][] groups(int[] labels,int length){if(labels==null)return new int[0][];int[][] out=new int[256][];for(int i=0;i<256;i++){int count=0;for(int j=0;j<length;j++)if(labels[j]==i)count++;out[i]=new int[count];int k=0;for(int j=0;j<length;j++)if(labels[j]==i)out[i][k++]=j;}return out;}
    void transformMaskedBone(int t,int[] labels,int x,int y,int z,boolean a,int mask,int[] parts){throw new UnsupportedOperationException("masked transforms outside corpus");}
    """+method(software,"protected void transformBone(")+"\n"+method(blend,"private void applyAnimationBlend(").replace("private void","public void",1)+"\n}"
    (RUN/"PoseModel.java").write_text(pose)
    gl=source("GlModel")
    fields=gl[gl.index("public final class GlModel"):gl.index("public GlModel()")].replace("extends Model", "").replace("private ","public ")
    methods=["public GlModel()","public GlModel(RawModel model,", "private short findOrCreateVertex(","public static int getDominantAxis(","public static void calculateCylindricalUV(","public static float[] buildTextureTransformMatrix(","public static void calculateSphericalUV(","public static void calculatePlanarUV(","public static int packColorToRGBA(","public final void createBones()", "protected final boolean hasAnimationBones()", "protected final void transformBone(", "protected final void resetAfterAnimation()", "public final void rotateClockwiseAll()", "public final void rotate180All()", "public final void rotateCounterClockwiseAll()", "public final void rotateClockwise()", "public final void rotate180()", "public final void rotateCounterClockwise()", "public final void resize(", "public final void translate("]
    methods += ["private void calculateBounds()", "public final SoftwareIndexedSprite projectShadow(","public final int getMinX()","public final int getMaxX()","public final int getMinZ()","public final int getMaxZ()","public final void rotateY(","protected final void transformShadowBone("]
    glmodel="import java.nio.ByteBuffer;\n"+fields+"\n".join(method(gl,m) for m in methods)+"\n"+method(blend,"public final void applyAnimation(")+"\n"+method(blend,"private void applyAnimationBlend(").replace("private void","public void",1)+"\nvoid transformMaskedBone(int t,int[] labels,int x,int y,int z,boolean a,int mask,int[] parts){throw new UnsupportedOperationException(\"masked transforms outside corpus\");}\n}"
    glmodel=glmodel[:-1]+method(blend,"public final void applyShadowAnimation(")+method(blend,"public final void applyDualAnimation(")+"}"
    (RUN/"GlModel.java").write_text(glmodel)
    shadow_model=source("ShadowModelList")
    character=method(shadow_model,"public static Model getOrCreateShadow(")
    gl_branch=method(character,"if (GlRenderer.enabled)")
    character=character[:character.index("if (GlRenderer.enabled)")]+gl_branch[gl_branch.index("{")+1:-1]+"return shadowModel;}"
    character=re.sub(r"\bModel\b","GlModel",character).replace("(GlModel) SHADOWS.get(hashKey)","null").replace("SHADOWS.put(shadowModel, hashKey);","")
    character=re.sub(r"shadowModel\.copyForAnimation\([^;]+\)","copy(shadowModel)",character).replace("SeqTypeList.getAnimFrameset(frameId >> 16)","new AnimFrameset(animation)")
    (RUN/"CharacterShadowOracle.java").write_text("class CharacterShadowOracle {static AnimFrame animation;static GlModel copy(GlModel source){try{GlModel dest=new GlModel();for(var field:GlModel.class.getDeclaredFields()){if(java.lang.reflect.Modifier.isStatic(field.getModifiers()))continue;field.setAccessible(true);Object value=field.get(source);if(value!=null&&value.getClass().isArray()){int n=java.lang.reflect.Array.getLength(value);Object clone=java.lang.reflect.Array.newInstance(value.getClass().getComponentType(),n);System.arraycopy(value,0,clone,0,n);value=clone;}field.set(dest,value);}dest.bounds=new GlBoundingBox();return dest;}catch(Exception e){throw new RuntimeException(e);}}"+character+"}")
    seq_source=source("SeqType")
    seq_fields=seq_source[seq_source.index("public final class SeqType"):seq_source.index("public void decode(")]
    (RUN/"SeqType.java").write_text(seq_fields+method(seq_source,"public void decode(")+method(seq_source,"private void decode(")+"}")
    loc_source=source("Loc")
    initial=loc_source[loc_source.index("this.seqFrame = 0;"):loc_source.index("if (this.seq.exactmove")]
    randomize=method(loc_source,"if (randomizeAnim && this.seq.replayoff != -1)").replace("Math.random()","random[randomIndex++]")
    (RUN/"LocClock.java").write_text("""class client {static int loop;}
class SoundPlayer {static java.util.List<Integer> events=new java.util.ArrayList<>();static void playSeqSound(int y,SeqType s,int x,boolean self,int frame){events.add(frame);}}
class LocClock {SeqType seq;int seqFrame,seqNextFrame,seqFrameProgress,seqStartLoop;
LocClock(SeqType seq,int loop,double[] random){this.seq=seq;client.loop=loop;boolean randomizeAnim=random!=null;int randomIndex=0;
"""+initial+randomize+"}\n"+method(loc_source,"private void advanceAnimation(").replace("private void","public void",1)+"}")
    (RUN/"Environment.java").write_text(source("Environment"))
    fog_source=source("FogManager");source("GlobalConfig")
    fog_fields=fog_source[fog_source.index("public final class FogManager"):fog_source.index("public static void applyLightPosition()")]
    reset=[]
    for kind,name,value in re.findall(r"(?:public|private) static (boolean|int|float) (\w+)\s*(?:=\s*([^;]+))?;",fog_fields):
        reset.append(name+"="+(value.strip() if value else ("false" if kind=="boolean" else "0"))+";")
    fog_methods=["public static void applyLightPosition()", "public static void setLightParams(", "public static void setFogParams(", "public static void setLightPosition(float", "public static int updateAtmosphere("]
    (RUN/"FogManager.java").write_text(fog_fields+"\n".join(method(fog_source,m) for m in fog_methods)+"\nstatic void resetForOracle(){"+"".join(reset)+"}\n}")
    (RUN/"CollisionMap.java").write_text(source("CollisionMap"))
    path_source=source("PathFinder")
    path_fields=path_source[path_source.index("public class PathFinder"):path_source.index("public static boolean findPath(")]
    (RUN/"PathFinder.java").write_text(path_fields+method(path_source,"public static boolean findPathN(")+"}")
    client_prot=source("ClientProt")
    (RUN/"ClientProt.java").write_text("class ClientProt {static int MOVE_GAMECLICK=215,MOVE_MINIMAPCLICK=39,lastPathLength;static void sendMovePacket(int length,int type){lastPathLength=length;writeMovePacket(length,type);}"+method(client_prot,"public static void sendMovePacket(").replace("sendMovePacket(","writeMovePacket(",1)+"}")
    (RUN/"BasType.java").write_text(source("BasType"))
    npc_type=source("NpcType")
    npc_head=npc_type[:npc_type.index("public final NpcType getMultiNpc()")]
    npc_fields=re.findall(r"(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte|short|JagString|HashTable)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;",npc_head)
    npc_fields="\n".join(f.replace("private ","public ").replace("protected ","public ") for f in npc_fields)
    body=method(npc_type,"public final Model getBodyModel(")
    assembly=body[body.index("RawModel[] rawModels ="):body.index("BasType basType =")]
    assembly+=body[body.index("RawModel rawModel;"):body.index("model = rawModel.createModel")]
    assembly=assembly.replace("client.npcRecolorPalette","NPC_PALETTE")
    (RUN/"NpcDefinitionOracle.java").write_text("class NpcDefinitionOracle {static short[] NPC_PALETTE=new short[256];"+npc_fields+method(npc_type,"public final void decode(")+method(npc_type,"private void decode(")+"RawModel assemble(){int i;"+assembly+"return rawModel;}}class NpcTypeList {static Js5 modelsArchive;}")
    raw_path=RUN/"RawModel.java"
    raw_path.write_text(raw_path.read_text()[:-1]+method(raw,"public final void translate(")+method(raw,"private void invalidate(")+"}")
    entity_source=source("PathingEntity");head=entity_source[:entity_source.index("public final BasType getBasType()")]
    fields=re.findall(r"(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;",head)
    (RUN/"PathingEntity.java").write_text("class PathingEntity {"+"\n".join(f.replace("private ","public ").replace("protected ","public ") for f in fields)+"\nint oracleSize=1;BasType bas;int getSize(){return oracleSize;}BasType getBasType(){return bas;}"+method(entity_source,"public final void move(")+"}")
    npc_source=source("NpcList")
    anim=method(npc_source,"public static void processAnimations(");anim=anim[anim.index("entity.seqStretches = false;"):anim.index("int i;")]
    (RUN/"NpcList.java").write_text("class NpcList {static Npc[] npcs=new Npc[32768];"+method(npc_source,"public static void processMovement(")+method(npc_source,"public static void processFacing(")+"static void advanceMovementAnimation(PathingEntity entity){"+anim+"}}")
    loc_type=source("LocType");loc_list=source("LocTypeList")
    fields=re.findall(r"(?:public|private|protected) (?:static )?(?:final )?(?:int|boolean|byte|short|JagString|HashTable)(?:\[\])* \w+(?:\s*=\s*[^;]+)?;",loc_type)
    (RUN/"LocType.java").write_text("class LocType {"+"\n".join(f.replace("private ","public ").replace("protected ","public ") for f in fields)+method(loc_type,"public void decode(")+method(loc_type,"private void decode(")+method(loc_type,"public void postDecode()")+"static LocType clipping(byte[] data){LocType loc=new LocType();loc.decode(new Buffer(data));"+method(loc_list,"if (loc.breakroutefinding)")+"return loc;}}")
    mini=source("MiniMenu")
    action_names=["LOC_ACTION_1","LOC_ACTION_2","LOC_ACTION_3","LOC_ACTION_4","LOC_ACTION_5","LOC_ACTION_EXAMINE"]
    action_constants="\n".join(re.search(r"public static final int "+name+r" = [^;]+;",mini).group(0) for name in action_names)
    action_branches="\n".join(method(mini,"if (actionCode == "+name+")").replace("PathFinder.findPathToLoc(key, menuArg2, menuArg1);","") for name in action_names)
    menu_start=mini.index("JagString[] locOps = locType.ops;")
    menu_end=mini.index(";",mini.index("add(MiniMap.examineCursor",menu_start))+1
    loc_menu=mini[menu_start:menu_end]
    (RUN/"MenuOracle.java").write_text("class MenuOracle {static int size,defaultCursorId=-1;static boolean DEBUG_OPS=false;static JagString COLOR_CYAN=JagString.parse(\"<col=00ffff>\");static JagString[] ops=new JagString[500],opBases=new JagString[500];static short[] actions=new short[500];static int[] intArgs1=new int[500],intArgs2=new int[500],cursors=new int[500];static long[] keys=new long[500];"+action_constants+method(mini,"public static void add(")+method(mini,"public static void sort()")+"static JagString[] annotateOps(JagString[] ops){return ops;}static void location(LocType locType,long pickKey,int x,int y){int j;"+loc_menu+"}static void action(int actionCode,long key,int menuArg1,int menuArg2){if(actionCode>=2000)actionCode-=2000;int keyInt=(int)key;"+action_branches+"}}")
    changes=source("ChangeLocRequest")
    head=changes[changes.index("public final class ChangeLocRequest"):changes.index("public static void loop()")]
    (RUN/"ChangeLocRequest.java").write_text(head+"\n".join(method(changes,m) for m in ["public static void loop()","public static void push(","public static void init(","public static void flush()"])+"static boolean isLocModelReady(int id,int shape){return !SceneGraph.unready.contains(id);}}")
    raster=source("Rasteriser");colors=source("ColorUtils")
    (RUN/"Rasteriser.java").write_text("class Rasteriser {static VisualMaterials textureProvider;static int[] palette=new int[65536];static float brightness=0.7F;static {calculateBrightness();}"+method(raster,"private static void calculateBrightness()")+method(raster,"public static void fillSpriteTriangle(")+method(raster,"private static void drawSpriteScanline(")+"}")
    shadow=source("Shadow")
    filtering=method(shadow,"public final boolean updateShadowTexture(")
    filtering=filtering[:filtering.index("GL2 gl =")]+"return true;}"
    (RUN/"ShadowFilter.java").write_text("class ShadowFilter {static byte[] pixels=new byte[16384];int pixelHash=-1;"+filtering+"}")
    light=source("Light")
    light_fields=light[light.index("public final class Light"):light.index("public Light(")].replace("private ","public ")
    light_methods=["public Light(","public final void setFlickerParams(","private void computeAttenuation()","public final void updateAnimation(","private void initAnimationPreset()"]
    (RUN/"Light.java").write_text(light_fields+"\n".join(method(light,m).replace("private ","public ") for m in light_methods)+"static void init(){throw new IllegalStateException(\"oracle noise must be injected\");}}")
    (RUN/"LightType.java").write_text(source("LightType"))
    mesh=source("LightMesh")
    (RUN/"LightMesh.java").write_text("class LightVertexCache {Node get(long key){return null;}void put(Node node,long key){}}class LightMesh {int vertexCount;int[] vertexX=new int[1],vertexY=new int[1],vertexZ=new int[1];byte[] colorR=new byte[1],colorG=new byte[1],colorB=new byte[1];LightVertexCache vertexCache=new LightVertexCache();"+method(mesh,"public final int addVertex(")+"}")
    manager=source("LightingManager")
    (RUN/"LightGridOracle.java").write_text("class LightGridOracle {static int lightCount,planeCount,width,length;static Light[] lights;static int[][][] lightGrid;"+method(manager,"public static void initLightGrid()")+method(manager,"public static void buildLightGrid()")+method(manager,"public static void addLight(")+"}")
    scene_lighting=source("SceneGraph")
    placement=scene_lighting[scene_lighting.index("lightObj.y += baseY << 7;"):scene_lighting.index("LightingManager.addLight(lightObj);",scene_lighting.index("lightObj.y += baseY << 7;"))+len("LightingManager.addLight(lightObj);")]+"}"
    placement=placement.replace("LightingManager.addLight(lightObj);","added=true;")
    (RUN/"PointPlacementOracle.java").write_text("class PointPlacementOracle {static boolean added;static void place(Light lightObj,int tileX,int baseY,int ground){int startY,endY;byte[][][] renderFlags=new byte[4][104][104];int[][][] tileHeights=new int[4][104][104];for(var plane:tileHeights)for(var row:plane)java.util.Arrays.fill(row,ground);added=false;"+placement+"}}")
    stamp=source("ShadowManager")
    (RUN/"ShadowStamp.java").write_text("class ShadowStamp {static void markDirtyRegion(int x,int y,int width,int height){}"+"\n".join(method(stamp,m).replace("private ","public ") for m in ["private static void addShadowSprite(","private static void addPixels(","private static void removeShadowSprite(","private static void subtractPixels("])+"}")
    (RUN/"ColorUtils.java").write_text("class ColorUtils {"+method(colors,"public static int multiplyLightness2(")+method(colors,"public static int rgbToHsl(")+method(colors,"public static int packHsl(")+"}")
    appearance=source("PlayerAppearance")
    fields="\n".join(re.search(r"public static final short\[\](?:\[\])? "+name+r" = .*?;",appearance).group(0) for name in ["GAME0_SOURCE_BODY_COLORS","GAME0_SOURCE_SKIN_COLORS","GAME0_DESTINATION_BODY_COLORS","GAME0_DESTINATION_SKIN_COLORS"])
    (RUN/"AppearanceOracle.java").write_text("class AppearanceOracle {"+fields+"""
+    static RawModel assemble(IdkType[] kits,int[] colors){java.util.Arrays.sort(kits,java.util.Comparator.comparingInt(k->new int[]{8,11,4,6,9,7,10}[k.feature%7]));RawModel[] parts=new RawModel[kits.length];for(int i=0;i<kits.length;i++)parts[i]=kits[i].getBodyModel();RawModel combined=new RawModel(parts,parts.length);for(int i=0;i<5;i++){if(colors[i]<GAME0_DESTINATION_BODY_COLORS[i].length)combined.recolor(GAME0_SOURCE_BODY_COLORS[i],GAME0_DESTINATION_BODY_COLORS[i][colors[i]]);if(colors[i]<GAME0_DESTINATION_SKIN_COLORS[i].length)combined.recolor(GAME0_SOURCE_SKIN_COLORS[i],GAME0_DESTINATION_SKIN_COLORS[i][colors[i]]);}return combined;}
+    }""".replace("\n+","\n"))
    scene_source=source("SceneGraph")
    first=scene_source.index("rowWeightedHue[i] = 0;")
    first=scene_source.rfind("for (i = 0; i < 104; i++)",0,first)
    last=scene_source.index("for (i = 1; i < 103; i++)",first)
    (RUN/"FloorBlendOracle.java").write_text("""class FluTypeList {static FluType[] types;static FluType get(int id){return types[id];}}
+    class FloorBlendOracle {static int[][] blend(byte[][] tiles){byte[][][] tileUnderlays={tiles};int currentLevel=0;int[][] underlayColors=new int[104][104];int[] rowWeightedHue=new int[104],rowSaturation=new int[104],rowLightness=new int[104],rowChroma=new int[104],rowCount=new int[104];int i,j,k,overlay,m,n,p,q,z;
+    """.replace("\n+","\n")+scene_source[first:last]+"return underlayColors;}}")
    sprite_source=source("SpriteLoader")
    sprite="class SpriteOracle {static int frames,width,height;static int[] innerHeights,innerWidths,xOffsets,yOffsets,palette;static boolean[] hasAlpha;static byte[][] alpha,pixels;"+method(sprite_source,"public static void decode(byte[] data)")+"}"
    (RUN/"SpriteOracle.java").write_text(sprite)
    for name in ["Support.java","VisualOracle.java","MovementSupport.java"]:shutil.copyfile(pathlib.Path(__file__).parent/name,RUN/name)
    renderer=source("GlRenderer");support=(RUN/"Support.java").read_text()
    support=support.replace("class GlRenderer {static GL2 gl=new GL2();}","class GlRenderer {static GL2 gl=new GL2();static boolean orthographicActive;static float projectionDistance,depthBias,projectionZScale,projectionZOffset;static float[] matrix=new float[16];"+method(renderer,"public static void setDepthBias(")+"}")
    (RUN/"Support.java").write_text(support)
    if build_only:return sources
    blob=_recorded(ROOT,'demo.pack').read_bytes();length=struct.unpack_from("<I",blob,8)[0];manifest=json.loads(blob[12:12+length]);payload=blob[12+length:]
    def get(name):
        off,size=manifest["files"][name];return payload[off:off+size]
    inputs={}
    def save(name):
        filename=name.replace("/","_")+".bin"
        if filename not in inputs:
            inputs[filename]=get(name);(RUN/filename).write_bytes(inputs[filename])
        return filename
    cases=[];requests=[]
    save("materials")
    for near,far,distance in [(0.1,250.0,3000.0/128.0),(50.0,7168.0,3000.0),(50.0/128.0,56.0,3000.0/128.0)]:
        for bias in [0,152.25,302.25,-150,152.25/128.0,302.25/128.0,-150/128.0,0.001]:
            id=f"depth_{near}_{far}_{distance}_{bias}";cases.append(dict(kind="depthbias",id=id,file="materials.bin",near=near,far=far,distance=distance,bias=bias));requests.append(f"depthbias\tmaterials.bin\t{id}\t{near}\t{far}\t{distance}\t{bias}")
    inputs["light-noise.bin"]=(FIX/"light-noise.bin").read_bytes();(RUN/"light-noise.bin").write_bytes(inputs["light-noise.bin"])
    for name in sorted(n for n in manifest["files"] if n.startswith("terrain/")):
        data=get(name);pos=0
        for _ in range(4*64*64):
            while True:
                op=data[pos];pos+=1
                if op==0:break
                if op==1:pos+=1;break
                if op<=49:pos+=1
        index=0
        while pos<len(data):
            op=data[pos];pos+=1
            if op==129:
                for _ in range(4):
                    flag=data[pos];pos+=1
                    if flag==1:pos+=256
            elif op==0:raise ValueError("light oracle expects default atmosphere in current corpus")
            elif op==1:
                count=data[pos];pos+=1
                for _ in range(count):
                    start=pos;radius=data[pos+7];pos+=8+(radius*2+1)*2+2;preset=data[pos]&31;pos+=1
                    if preset==31:pos+=2
                    file=f"light_{name.replace('/','_')}_{index}.bin";index+=1
                    inputs[file]=data[start:pos];(RUN/file).write_bytes(inputs[file]);cases.append(dict(kind="light",file=file));requests.append("light\t"+file)
                    region=list(map(int,name.split('/')[1].split('_')))
                    for ground in [-256,0,1024]:
                        cases.append(dict(kind="lightplace",file=file,region=region,ground=ground));requests.append(f"lightplace\t{file}\t{region[0]*64-3168}\t{region[1]*64-3168}\t{ground}")
            else:raise ValueError("unknown map tail")
    for preset in range(32):
        for phase in range(8):
            data=bytes([phase%4|24])+struct.pack(">3HB",400,500,200,2)+struct.pack(">5H",3,0x105,0x005,0x104,0x203)+struct.pack(">H",(preset*2039+phase*83)&65535)+bytes([preset|phase<<5])+(bytes([0,0]) if preset==31 else b"")
            file=f"light_preset_{preset}_{phase}.bin";inputs[file]=data;(RUN/file).write_bytes(data);cases.append(dict(kind="light",file=file));requests.append("light\t"+file)
    light_files=[c["file"] for c in cases if c["kind"]=="light"]
    for pattern in range(5):
        files=light_files[:78] if pattern==0 else light_files[78+pattern*20:78+pattern*20+80]
        if pattern==4:files=light_files[:78]*4
        id=f"light_grid_{pattern}";cases.append(dict(kind="lightgrid",id=id,file="materials.bin",files=files,pattern=pattern));requests.append(f"lightgrid\tmaterials.bin\t{id}\t{pattern}\t"+"|".join(files))
    for pattern in range(8):
        cases.append(dict(kind="shadow_stamp",file="shadow_filter_1.bin",pattern=pattern));requests.append(f"shadow_stamp\tshadow_filter_1.bin\t{pattern}")
    for name in sorted(n for n in manifest["files"] if n.startswith("model/")):
        file=save(name)
        for light in [[213,213],[-256,160],[0,-384]]:
            id=f"shadow_{file}_{light[0]}_{light[1]}"
            cases.append(dict(kind="shadow",id=id,file=file,light=light));requests.append("\t".join(map(str,["shadow",file,id,*light])))
    for pattern in range(6):
        data=bytes(1 if ((x*17+y*29)%13<pattern or (x//32+y//19)%7==pattern) and x not in (0,257) and y not in (0,257) else 0 for y in range(258) for x in range(258))
        file=f"shadow_filter_{pattern}.bin";inputs[file]=data;(RUN/file).write_bytes(data)
        cases.append(dict(kind="shadow_filter",file=file));requests.append("shadow_filter\t"+file)
    for name in sorted(n for n in manifest["files"] if n.startswith("loc/")):
        file=save(name);cases.append(dict(kind="clipping",file=file));requests.append("clipping\t"+file)
    for index,data in enumerate([bytes([74,27,0]),bytes([17,27,0]),bytes([18,14,3,15,7,0]),bytes([27,17,0])]):
        file=f"clipping_order_{index}.bin";inputs[file]=data;(RUN/file).write_bytes(data);cases.append(dict(kind="clipping",file=file));requests.append("clipping\t"+file)
    for name in sorted(n for n in manifest["files"] if n.startswith("loc/")):
        file=save(name)
        for kind in ["locinfo","locmenu"]:
            cases.append(dict(kind=kind,file=file));requests.append(kind+"\t"+file)
    for action in [42,50,49,46,1001,1004,2042,2050,2049,2046,3001,3004]:
        for npc_id,x,y in [(0,0,0),(65535,16383,16383),(32768,3215,3218),(12345,2600,3300)]:
            id=f"locaction_{action}_{npc_id}_{x}_{y}"
            cases.append(dict(kind="locaction",id=id,file="materials.bin",action=action,location=npc_id,tile=[x,y]))
            requests.append(f"locaction\tmaterials.bin\t{action}\t{npc_id}\t{x}\t{y}")
    change_cases=[]
    for set_loops in [-1,0,1,2,4]:
        for reset in [-1,0,1,2,6]:
            change_cases.append([[0,0,50,50,0,101,0,1,set_loops,reset],*[ [1] for _ in range(8)]])
    change_cases.extend([
        [[0,0,50,50,0,101,0,1,0,-1],[1],[0,0,50,50,0,100,0,0,0,-1],[1]],
        [[2,101,0],[0,0,50,50,0,101,0,1,0,-1],[1],[1],[2,101,1],[1],[1]],
        [[0,0,50,50,0,101,0,1,0,3],[1],[2,100,0],[1],[1],[1],[2,100,1],[1]],
        [[0,0,50,50,0,101,0,1,0,-1],[1],[3],[1],[0,0,50,50,0,-1,0,0,0,-1],[1],[3],[1]],
        [[0,0,50,50,0,101,0,1,3,-1],[0,0,50,50,0,102,0,2,0,-1],[1],[0,0,50,50,0,103,0,3,0,2],[1],[1]],
        [[0,0,50,50,1,200,4,1,0,-1],[0,1,50,50,0,201,2,2,0,3],[0,0,50,50,2,202,10,0,0,2],[1],[1],[1],[3],[1]],
        [[0,0,51,50,0,-1,2,3,0,-1],[1],[0,0,50,50,0,-1,0,0,0,-1],[1],[0,0,50,50,0,100,0,0,0,-1],[1]],
    ])
    for x in [0,1,102,103]:
        change_cases.append([[0,0,x,50,0,101,0,1,0,-1],[1],[1],[3],[1]])
    for i,commands in enumerate(change_cases):
        id=f"locchanges_{i}";cases.append(dict(kind="locchanges",id=id,file="materials.bin",commands=commands))
        requests.append(f"locchanges\tmaterials.bin\t"+";".join(",".join(map(str,c)) for c in commands))
    # Obstacles, narrow diagonal gaps, fully blocked goals, BFS tie order and packet
    # truncation are compared with the original 104-tile ground-click pathfinder.
    for pattern in range(6):
        flags=[]
        for x in range(104):
            for y in range(104):
                value=0xffffff if x==0 or y==0 or x>=99 or y>=99 else 0
                if pattern==1 and x==55 and y not in (31,73):value|=0x100
                if pattern==2 and 48<=x<=57 and 48<=y<=57:value|=0x100
                if pattern==3 and (x*13+y*17)%7==0:value|=0x100
                if pattern==4 and x%4==0 and y<90 and y!=(x*3)%80+8:value|=0x100
                if pattern==5:value|=(1<<((x+y)%8)) if (x*7+y*11)%3==0 else 0
                flags.append(value)
        file=f"pathmap_{pattern}.bin";inputs[file]=struct.pack(">"+"i"*len(flags),*flags);(RUN/file).write_bytes(inputs[file])
        for src,dst in [([50,40],[60,70]),([20,20],[80,80]),([40,40],[52,52]),([50,50],[50,50]),([1,1],[98,98]),([97,97],[99,99])]:
            for approx in [False,True]:
                for run in [False,True]:
                    id=f"route_{pattern}_{src}_{dst}_{int(approx)}_{int(run)}";cases.append(dict(kind="route",id=id,file=file,source=src,destination=dst,approximate=approx,run=run,origin=[3168,3168]))
                    requests.append("\t".join(map(str,["route",file,id,*src,*dst,int(approx),int(run),3168,3168])))
        for shape in [0,1,2,3,4,5,6,7,8,9,10,11,22]:
            for angle in range(4):
                width,length=([2,3] if angle%2==0 else [3,2]) if shape in (10,11,22) else [0,0]
                route_shape=0 if shape in (10,11,22) else shape+1
                blocked=[0,1,4,15][angle]
                id=f"locroute_{pattern}_{shape}_{angle}"
                cases.append(dict(kind="locroute",id=id,file=file,source=[40,40],destination=[52,52],approximate=True,run=False,origin=[3168,3168],size=[width,length],shape=route_shape,angle=angle,blocked=blocked))
                requests.append("\t".join(map(str,["route",file,id,40,40,52,52,1,0,3168,3168,width,length,route_shape,angle,blocked])))
    basfile=save("bas/1426")
    for name in sorted(n for n in manifest["files"] if n.startswith("seq/")): save(name)
    for angle in [0,32,512,1024,1536,2047]:
        for speed in [0,1,2]:
            commands=[[0,4,speed],[4,1,speed],[8,0,speed],[12,6,speed],[160,7,speed],[320,3,speed]]
            id=f"movement_{angle}_{speed}";cases.append(dict(kind="movement",id=id,file=basfile,angle=angle,commands=commands,ticks=420))
            requests.append("\t".join(["movement",basfile,id,str(angle),";".join(",".join(map(str,c)) for c in commands),"420"]))
    commands=[[0,i%8,1] for i in range(14)];id="movement_queue_capacity"
    cases.append(dict(kind="movement",id=id,file=basfile,angle=0,commands=commands,ticks=420))
    requests.append("\t".join(["movement",basfile,id,"0",";".join(",".join(map(str,c)) for c in commands),"420"]))
    for basid,size in [(4,1),(117,1),(62,1),(101,1),(66,2),(86,1)]:
        file=save(f"bas/{basid}")
        for angle in [0,512,1024,2047]:
            commands=[[0,4,1],[4,1,1],[8,0,1],[12,6,1],[160,7,1],[320,3,1]]
            id=f"npc_movement_{basid}_{size}_{angle}"
            cases.append(dict(kind="movement",id=id,file=file,size=size,angle=angle,commands=commands,ticks=420))
            requests.append("\t".join(["movement",file,id,str(angle),";".join(",".join(map(str,c)) for c in commands),"420",str(size)]))
    envfiles=[]
    for flags in range(256):
        data=bytearray([flags])
        if flags&1:data.extend(struct.pack(">I",(0x807f91ff+flags*7919)&0xffffffff))
        for bit,value in [(2,123+flags*251),(4,179+flags*97),(8,307+flags*113)]:
            if flags&bit:data.extend(struct.pack(">H",value&65535))
        if flags&16:data.extend(struct.pack(">hhh",-32768+flags*31,32767-flags*43,-50+flags*7))
        if flags&32:data.extend(struct.pack(">I",(0xf0c8c0a8+flags*379)&0xffffffff))
        if flags&64:data.extend(struct.pack(">H",(flags*271)&65535))
        file=f"environment_{flags}.bin";inputs[file]=bytes(data);(RUN/file).write_bytes(data);envfiles.append(file)
        cases.append(dict(kind="environment",file=file));requests.append("environment\t"+file)
    file="environment_default.bin";inputs[file]=b"";(RUN/file).write_bytes(b"");cases.append(dict(kind="environment",file=file));requests.append("environment\t"+file)
    for view,fade in [(7168,512.0),(3584,256.0),(100,96.0)]:
        files=[envfiles[i] for i in [0,1,18,32,64,127,255]]
        steps=[[0,0,3,1,0]]
        for envindex in range(len(files)):
            for tick in range(75):steps.append([envindex,0 if tick==4 else 1 if tick%4 else 7,(envindex%4)+1,int(tick<50),int(tick==31)])
        steps.extend([[1,1000,4,1,0],[1,0,4,1,0],[0,1,3,1,1]])
        id=f"atmosphere_{view}_{fade}";cases.append(dict(kind="atmosphere",id=id,file=files[0],files=files,steps=steps,view=view,fade=fade))
        requests.append("\t".join(["atmosphere",files[0],id,"|".join(files),";".join(",".join(map(str,step)) for step in steps),str(view),str(fade)]))
    scene=json.loads(get("scene.json"));all_models=[k for k in manifest["files"] if k.startswith("model/")]
    # Every idle/walk frame of all eight NPCs, bind-body assembly, BAS fields,
    # fractional HD poses and footprint/color-specific animated ground shadows.
    import sys
    sys.path.insert(0,str(ROOT/"tools"))
    from prepare_visual import Reader
    for npc in scene["npc_types"]:
        file=save(f"npc/{npc}");raw=get(f"npc/{npc}")
        # BAS opcode 127 is unambiguous in these reviewed definitions, but parse
        # dependencies through the original decoder in the Java oracle as well.
        bas_ids={0:117,1:4,4:4,9:4,41:62,47:101,81:66,100:86}
        basfile=save(f"bas/{bas_ids[npc]}")
        for kind in ["npcdecode","npcmodel"]:
            cases.append(dict(kind=kind,file=file,bas=basfile,id=f"{kind}_{npc}"));requests.append(f"{kind}\t{file}\t{basfile}")
        r=Reader(get(f"bas/{bas_ids[npc]}"));seq_ids=[]
        while (op:=r.u1()):
            if op==1:seq_ids.extend([r.u2(),r.u2()])
            elif op==26:r.take(2)
            else:r.take(2)
        for seq in seq_ids:
            seqfile=save(f"seq/{seq}");r=Reader(get(f"seq/{seq}"))
            while (op:=r.u1())!=1:
                if op in (2,6,7):r.take(2)
                elif op in (5,8,9,10,11):r.take(1)
                elif op in (4,14,15,16):pass
                elif op==3:r.take(r.u1())
                else:raise ValueError(f"NPC sequence {seq}: {op}")
            n=r.u2();delays=[r.u2() for _ in range(n)];lows=[r.u2() for _ in range(n)];highs=[r.u2() for _ in range(n)]
            for i in range(n):
                a=save(f"frame/{highs[i]}/{lows[i]}");j=(i+1)%n;b=save(f"frame/{highs[j]}/{lows[j]}")
                bid=int.from_bytes(inputs[a][:2],"big");base=save(f"base/{bid}")
                for delta in sorted(set([0,delays[i]//2,delays[i]-1])):
                    for tween in [False,True]:
                        id=f"npc_{npc}_{seq}_{i}_{delta}_{tween}"
                        case=dict(kind="npcpose",id=id,file=file,bas=basfile,seq=seqfile,base_id=bid,base=base,first=a,next=b,delta=delta,length=delays[i],tween=tween,angle=(i*128)&2047)
                        cases.append(case);requests.append("\t".join(map(str,["npcpose",file,basfile,seqfile,bid,base,a,b,delta,delays[i],str(tween).lower(),case["angle"]])))
    for name in manifest["files"]:
        if name.startswith(("underlay/","overlay/")):
            file=save(name);kind=name.split("/")[0];cases.append(dict(kind=kind,file=file));requests.append(kind+"\t"+file)
        if name.startswith("kit/"): save(name)
    # Complete Loc clock traces, including randomized starts, long invisible gaps,
    # non-repeating/partial-tail replay and Java loop-counter overflow.
    ticks=list(range(201))+[200,201,1000,1001,10000,10001]
    for name in sorted(n for n in manifest["files"] if n.startswith("seq/")):
        file=save(name)
        for random in [None,[0.0,0.0],[0.97,0.65]]:
            for start in [0,2147483587]:
                id=f"clock_{name.split('/')[1]}_{random}_{start}"
                cases.append(dict(kind="locclock",id=id,file=file,random=random,start=start,ticks=ticks))
                requests.append("\t".join(["locclock",file,id,str(start),"none" if random is None else ",".join(map(str,random)),",".join(map(str,ticks))]))
    underlay_ids=sorted(int(name.split("/")[1])+1 for name in manifest["files"] if name.startswith("underlay/"))
    window=bytes(0 if (x*13+y*7)%17==0 else underlay_ids[(x*x+y*7+x*y)%len(underlay_ids)] for x in range(104) for y in range(104))
    inputs["floor-window.bin"]=window;(RUN/"floor-window.bin").write_bytes(window)
    cases.append(dict(kind="floorblend",file="floor-window.bin"));requests.append("floorblend\tfloor-window.bin")
    for colorids in [[0,0,0,0,0],[1,2,3,4,5],[24,28,28,5,7]]:
        id="appearance_"+"_".join(map(str,colorids));file=save("model/"+str(scene["avatar_models"][0]));cases.append(dict(kind="appearance",id=id,file=file,colors=colorids,kits=scene["avatar_kits"],models=scene["avatar_models"]))
        requests.append("\t".join(["appearance",file,id,",".join(map(str,colorids)),",".join(map(str,scene["avatar_kits"]))]))
    # Both cache model encodings, all avatar body parts, and representative scenery.
    chosen=set(scene["avatar_models"]+[int(k.split("/")[1]) for k in all_models[::37]])
    for id in sorted(chosen):
        name=save(f"model/{id}");cases.append(dict(kind="model",file=name));requests.append("model\t"+name)
    # Every available scene model exercises all four GL texture projections and
    # the actual constructor's face filtering, vertex normals and material colors.
    for name in all_models:
        file=save(name);cases.append(dict(kind="surface",file=file));requests.append("surface\t"+file)
    for name in manifest["files"]:
        if name.startswith("sound/"):
            file=save(name);cases.append(dict(kind="sound",file=file));requests.append("sound\t"+file)
    # Sequence frames and fractional tween samples for idle, walk, run and wave.
    for seq in [808,819,820,821,822,823,824,863,866]:
        data=get(f"seq/{seq}");pos=0
        while data[pos]!=1:
            op=data[pos];pos+=1
            if op in (2,6,7):pos+=2
            elif op in (5,8,9,10,11):pos+=1
            elif op in (4,14,15,16):pass
            elif op==3:pos+=data[pos]+1
            else:raise ValueError(f"sequence {seq}: no frames (opcode {op})")
        n=int.from_bytes(data[pos+1:pos+3],"big");start=pos+3
        delays=struct.unpack_from(">"+"H"*n,data,start);lows=struct.unpack_from(">"+"H"*n,data,start+n*2);highs=struct.unpack_from(">"+"H"*n,data,start+n*4)
        for index in range(n):
            nxt=(index+1)%n;a=save(f"frame/{highs[index]}/{lows[index]}");b=save(f"frame/{highs[nxt]}/{lows[nxt]}")
            baseid=int.from_bytes(inputs[a][:2],"big");base=save(f"base/{baseid}")
            if int.from_bytes(inputs[b][:2],"big")!=baseid:continue
            for modelid in (scene["avatar_models"] if index in set([0,n//2,n-1]) else [])+["combined"]:
                files=[save(f"model/{id}") for id in scene["avatar_models"]] if modelid=="combined" else [save(f"model/{modelid}")]
                file=files[0]
                for delta in (range(max(1,delays[index])) if modelid=="combined" else sorted(set([0,delays[index]//2]))):
                    id=f"{seq}_{index}_{modelid}_{delta}";case=dict(kind="pose",id=id,file=file,base_id=baseid,base=base,first=a,next=b,delta=delta,length=delays[index]);cases.append(case)
                    request="\t".join(map(str,["pose",file,baseid,base,a,b,delta,delays[index],id]))
                    if modelid=="combined":case["files"]=files;request+="\t"+"|".join(files)
                    requests.append(request)
                    if modelid=="combined":
                        case=case.copy();case.update(kind="glpose",id="gl_"+id);cases.append(case)
                        requests.append(request.replace("pose\t","glpose\t",1).replace("\t"+id+"\t","\tgl_"+id+"\t"))
    # Every animated scenery definition: source GL transform order, all quadrants,
    # normal-animation modes and the negative tween delta after the gap optimization.
    for locid,d in sorted(scene["definitions"].items(),key=lambda item:int(item[0])):
        seq=d["animation"]
        if seq in (-1,65535) or not d["models"]:continue
        rawseq=get(f"seq/{seq}")
        if rawseq[0]!=1:raise ValueError("expected first sequence opcode 1")
        n=int.from_bytes(rawseq[1:3],"big")
        delays=struct.unpack_from(">"+"H"*n,rawseq,3);lows=struct.unpack_from(">"+"H"*n,rawseq,3+n*2);highs=struct.unpack_from(">"+"H"*n,rawseq,3+n*4)
        models=d["models"] if d["shapes"] is None else d["models"][:1]
        files=[save(f"model/{model}") for model in models]
        for index in sorted(set([0,n//2,n-1])):
            nxt=(index+1)%n;a=save(f"frame/{highs[index]}/{lows[index]}");b=save(f"frame/{highs[nxt]}/{lows[nxt]}")
            baseid=int.from_bytes(inputs[a][:2],"big");base=save(f"base/{baseid}")
            if int.from_bytes(inputs[b][:2],"big")!=baseid:continue
            for rotation in range(4):
                for normals in [False,True]:
                    for delta in sorted(set([-1,0,delays[index]//2])):
                        id=f"loc_{locid}_{index}_{rotation}_{int(normals)}_{delta}"
                        cases.append(dict(kind="locpose",id=id,file=files[0],files=files,base_id=baseid,base=base,first=a,next=b,delta=delta,length=delays[index],rotation=rotation,normals=normals,resize=d["resize"],offset=d["offset"]))
                        requests.append("\t".join(map(str,["locpose",files[0],baseid,base,a,b,delta,delays[index],id,"|".join(files),rotation,str(normals).lower(),",".join(map(str,d["resize"])),",".join(map(str,d["offset"]))])))
    for x,y in [(556238+3200+i,932731+3200+j) for i,j in [(0,0),(18,22),(63,63),(64,64),(127,127),(32,11)]]:
        file=next(iter(inputs));id=f"{x}_{y}";cases.append(dict(kind="height",id=id,x=x,y=y));requests.append(f"height\t{file}\t{x}\t{y}")
    for shape in range(13):
        for rotation in range(4):
            id=f"{shape}_{rotation}";cases.append(dict(kind="tile",id=id,shape=shape,rotation=rotation));requests.append(f"tile\t{next(iter(inputs))}\t{shape}\t{rotation}\t{id}")
    sprite_ids=[]
    for name in manifest["files"]:
        if name.startswith("texture/"):
            data=get(name)
            if data[:6]==bytes([2,0,39,1,1,0]):
                id=int.from_bytes(data[6:8],"big")
                if id not in sprite_ids:sprite_ids.append(id)
    for id in sprite_ids[:6]:
        file=save(f"sprite/{id}");cases.append(dict(kind="sprite",file=file));requests.append("sprite\t"+file)
    for case,request in list(zip(cases,requests)):
        if case["kind"]!="glpose":continue
        angle=[0,128,512,1024,1536,2047][len(cases)%6];stretched=len(cases)%2==0
        fine=[[0,0],[6976,6464],[13200,128],[64,13000]][len(cases)%4]
        shadow_case=dict(case,kind="character",id="character_"+case["id"],angle=angle,stretched=stretched,fine=fine)
        parts=request.split("\t");parts[0]="character";parts[8]=shadow_case["id"];parts.extend([str(angle),str(stretched).lower(),*map(str,fine)])
        cases.append(shadow_case);requests.append("\t".join(parts))
    (RUN/"requests.tsv").write_text("\n".join(requests)+"\n")
    subprocess.run(["javac","-encoding","UTF-8","-d",str(RUN),*[str(p) for p in RUN.glob("*.java")]],check=True)
    result=subprocess.check_output(["java","-cp",str(RUN),"VisualOracle",str(RUN)],text=True)
    rows=[line.split("\t") for line in result.splitlines()]
    if len(rows)!=len(cases):raise ValueError("oracle case count mismatch")
    for case,row in zip(cases,rows):
        case["expected"]=int(row[2]) if case["kind"]=="height" else row[2]
        if case["kind"]=="sound":case.update(samples=int(row[3]),loop_start=int(row[4]),loop_end=int(row[5]))
        if case["kind"]=="surface":
            name=case["file"]+".surface";inputs[name]=(RUN/name).read_bytes();case["output"]=name
    fixture=dict(format=1,client_commit=json.loads((ROOT/"upstream.lock.json").read_text())["client"]["commit"],sources=sources,inputs={name:hashlib.sha256(data).hexdigest() for name,data in inputs.items()},cases=cases)
    return (json.dumps(fixture,indent=2)+"\n").encode(),inputs

if __name__=="__main__":
    parser=argparse.ArgumentParser();parser.add_argument("--check",action="store_true");args=parser.parse_args()
    fixture,inputs=generate()
    if args.check:
        if (FIX/"oracle.json").read_bytes()!=fixture:raise SystemExit("Visual oracle fixtures changed; inspect source/corpus changes before regeneration")
        for name,data in inputs.items():
            if (FIX/name).read_bytes()!=data:raise SystemExit("Visual oracle input changed: "+name)
    else:
        FIX.mkdir(parents=True,exist_ok=True)
        for name,data in inputs.items():(FIX/name).write_bytes(data)
        (FIX/"oracle.json").write_bytes(fixture)
    print(f"Verified {len(json.loads(fixture)['cases'])} JVM visual/audio cases")
