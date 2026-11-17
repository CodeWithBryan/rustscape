import java.nio.file.*;
import java.io.*;
import java.security.*;
import java.util.*;

class Buffer {
    byte[] data; int offset;
    Buffer(byte[] data){this.data=data;}
    int g1(){return data[offset++]&255;}
    byte g1b(){return data[offset++];}
    int g2(){return g1()<<8|g1();}
    short g2b(){return (short)g2();}
    int g3(){return g1()<<16|g1()<<8|g1();}
    int g4(){return g1()<<24|g1()<<16|g1()<<8|g1();}
}
class Js5 {
    final Path dir;
    Js5(Path dir){this.dir=dir;}
    byte[] fetchFile(int group,int file){try{return Files.readAllBytes(dir.resolve("sprite_"+group+".bin"));}catch(IOException e){throw new RuntimeException(e);}}
    boolean isFileReady(int group,int file){return Files.exists(dir.resolve("sprite_"+group+".bin"));}
    boolean isFileReady(int group){return isFileReady(group,0);}
}


public class TextureOracle implements TextureProvider {
    final Path dir; final Js5 sprites;
    final boolean[] lowDetail;
    TextureOracle(Path dir)throws Exception{this.dir=dir;sprites=new Js5(dir);Buffer b=new Buffer(Files.readAllBytes(dir.resolve("materials.bin")));int n=b.g2();boolean[] defined=new boolean[n];for(int i=0;i<n;i++)defined[i]=b.g1()==1;lowDetail=new boolean[n];for(int field=0;field<8;field++)for(int i=0;i<n;i++)if(defined[i]){int v=b.g1();if(field==2)lowDetail[i]=v==1;}}
    Texture graph(int id,boolean[] transpose){try{Buffer b=new Buffer(Files.readAllBytes(dir.resolve("texture_"+id+".bin")));Texture t=new Texture(b);b.g1();transpose[0]=b.g1()==1;return t;}catch(IOException e){throw new RuntimeException(e);}}
    public int[] getPixels(int id){boolean[] tr={false};Texture t=graph(id,tr);int size=isLowDetail(id)?64:128;return t.generateArgbPixels(size,tr[0],size,1.0,sprites,this,false);}
    public boolean isLowDetail(int id){return lowDetail[id];}
    public boolean isTextureLoaded(int id){return true;}
    public boolean isOpaque(int id){throw new UnsupportedOperationException();}
    public void bindTexture(int id){throw new UnsupportedOperationException();}
    public int getAnimationType(int id){throw new UnsupportedOperationException();}
    public int getTextureSpeed(int id){throw new UnsupportedOperationException();}
    public boolean isTextureFlipped(int id){throw new UnsupportedOperationException();}
    public int[] getAnimatedPixels(int id,float progress){throw new UnsupportedOperationException();}
    public int getAverageColor(int id){throw new UnsupportedOperationException();}
    public boolean isTextureRepeating(int id){throw new UnsupportedOperationException();}
    public int getMaterialType(int id){throw new UnsupportedOperationException();}
    public int getTextureBrightness(int id){throw new UnsupportedOperationException();}
    public static void main(String[] args)throws Exception {
        Path dir=Path.of(args[0]);TextureOracle oracle=new TextureOracle(dir);int count=0;
        Path out=dir.resolve("rgba");Files.createDirectories(out);
        java.lang.reflect.Field ops=Texture.class.getDeclaredField("operations");ops.setAccessible(true);
        ArrayList<String> graphs=new ArrayList<>();TreeSet<Integer> spriteIds=new TreeSet<>();
        for(Path p:Files.list(dir).filter(p->p.getFileName().toString().startsWith("texture_")).sorted().toList()){
            int id=Integer.parseInt(p.getFileName().toString().replace("texture_","").replace(".bin",""));
            boolean[] tr={false};Texture t=oracle.graph(id,tr);TextureOp[] nodes=(TextureOp[])ops.get(t);StringJoiner kinds=new StringJoiner(",");
            for(TextureOp node:nodes){kinds.add(node.getClass().getSimpleName().replace("TextureOp",""));if(node.getRequiredSpriteId()>=0)spriteIds.add(node.getRequiredSpriteId());}
            graphs.add(id+"\t"+kinds);
            for(int size:new int[]{64,128}){
                t=oracle.graph(id,tr);
                byte[] rgba=t.generateRgbaBytes(size,size,tr[0],oracle,0.7,oracle.sprites);
                Files.write(out.resolve(id+"_"+size+".rgba"),rgba);if(size==64)Files.write(out.resolve(id+".rgba"),rgba);count++;
                System.out.println(id+"\t"+size+"\t"+HexFormat.of().formatHex(MessageDigest.getInstance("SHA-256").digest(rgba)));
            }
        }
        Files.write(dir.resolve("graphs.tsv"),graphs);Files.write(dir.resolve("sprite_ids.txt"),spriteIds.stream().map(Object::toString).toList());
        System.err.println("Generated "+count+" reference texture images");
    }
}
