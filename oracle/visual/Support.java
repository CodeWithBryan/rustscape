// Deliberately narrow adapters for compiling the pinned decoding/math methods.
// The methods under test are copied verbatim at generation time, with annotations removed.
class Entity {}
class AnimFrameset {AnimFrame[] frames;AnimFrameset(AnimFrame frame){frames=new AnimFrame[]{frame};}}
class GlobalConfig {static int AUDIO_SAMPLE_RATE=22050;static boolean USE_TWEENING=true;static int TILE_DISTANCE=56,VIEW_DISTANCE=TILE_DISTANCE*128;static float VIEW_FADE_DISTANCE=((float)TILE_DISTANCE/28.0f)*256.0f;}
class GlBoundingBox {boolean valid;short minX,maxX,minY,maxY,minZ,maxZ;int cylinderRadius,sphereRadius;}
class SoftwareIndexedSprite {int width,height,innerWidth,innerHeight,xOffset,yOffset;byte[] pixels;SoftwareIndexedSprite(int w,int h,int unused){width=innerWidth=w;height=innerHeight=h;pixels=new byte[w*h];}void clear(){java.util.Arrays.fill(pixels,(byte)0);}}
class GlBuffer {boolean valid;}
class GlVertexBufferObject {}
class GlModelRecoloring {}
class Preferences {static boolean highDetailLighting=true,fogEnabled=true;}
// Only records arguments at the original FogManager's GPU boundary.
class GL2 {
    static final int GL_LIGHT0=0,GL_LIGHT1=1,GL_POSITION=2,GL_LIGHT_MODEL_AMBIENT=3,GL_DIFFUSE=4,GL_FOG_MODE=5,GL_LINEAR=6,GL_FOG_DENSITY=7,GL_FOG_HINT=8,GL_FASTEST=9,GL_FOG_START=10,GL_FOG_END=11,GL_FOG_COLOR=12;
    float[][] lights=new float[3][3];float[] fog=new float[3],range=new float[2];
    void glLightModelfv(int p,float[] v,int off){System.arraycopy(v,off,lights[0],0,3);}
    void glLightfv(int id,int p,float[] v,int off){if(p==GL_DIFFUSE)System.arraycopy(v,off,lights[id+1],0,3);}
    void glFogi(int p,int v){}void glHint(int p,int v){}
    void glFogf(int p,float v){if(p==GL_FOG_START)range[0]=v;if(p==GL_FOG_END)range[1]=v;}
    void glFogfv(int p,float[] v,int off){System.arraycopy(v,off,fog,0,3);}
    static final int GL_PROJECTION=13,GL_MODELVIEW=14;
    void glMatrixMode(int mode){}void glLoadMatrixf(float[] matrix,int offset){}
}
class GlRenderer {static GL2 gl=new GL2();}
class VisualMaterials {
    int[][] fields;
    VisualMaterials(byte[] data){Buffer b=new Buffer(data);int n=b.g2();boolean[] defined=new boolean[n];for(int i=0;i<n;i++)defined[i]=b.g1()==1;fields=new int[n][8];for(int f=0;f<8;f++)for(int i=0;i<n;i++)if(defined[i])fields[i][f]=b.g1();}
    boolean isTextureFlipped(int id){return fields[id][3]==1;}
    boolean isOpaque(int id){return fields[id][1]==1;}
    int getMaterialType(int id){return fields[id][6];}
    int getAnimationType(int id){return fields[id][7];}
    int getTextureBrightness(int id){return fields[id][5];}
    int getTextureSpeed(int id){return fields[id][4];}
}
class IdkTypeList {static Js5 modelsArchive;}
class Js5 {
    java.nio.file.Path dir;
    Js5(java.nio.file.Path dir){this.dir=dir;}
    byte[] fetchFile(int group,int file){try{return java.nio.file.Files.readAllBytes(dir.resolve("model_"+group+".bin"));}catch(java.io.IOException e){throw new RuntimeException(e);}}
    boolean isFileReady(int group,int file){return true;}
}
class PcmSound {byte[] samples;int start,end;PcmSound(int rate,byte[] samples,int start,int end){this.samples=samples;this.start=start;this.end=end;}}
class Buffer {
    byte[] data;int offset;
    Buffer(byte[] data){this.data=data;}
    Buffer(int size){this.data=new byte[size];}
    int g1(){return data[offset++]&255;}
    byte g1b(){return data[offset++];}
    int g2(){return (g1()<<8)|g1();}
    int g2b(){return (short)g2();}
    int g3(){return (g1()<<16)|(g1()<<8)|g1();}
    int g4(){return (g1()<<24)|(g1()<<16)|(g1()<<8)|g1();}
    JagString gjstr(){int start=offset;while(data[offset++]!=0){}return JagString.parse(new String(data,start,offset-start-1,java.nio.charset.StandardCharsets.ISO_8859_1));}
    int gsmart(){return (data[offset]&255)<128?g1()-64:g2()-49152;}
    int gsmarts(){return (data[offset]&255)<128?g1():g2()-32768;}
}
