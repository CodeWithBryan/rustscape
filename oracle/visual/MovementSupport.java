// I/O/session adapters only; routing, packet writing, movement and facing are extracted.
class Camera {static int originX,originY;}
class Keyboard {static final int KEY_CTRL=0;static boolean[] pressedKeys=new boolean[1];}
class LoginManager {static int mapFlagX,mapFlagY;}
class Protocol {static OutBuffer outboundBuffer;}
class OutBuffer {
    java.io.ByteArrayOutputStream out=new java.io.ByteArrayOutputStream();
    void p1(int v){out.write(v);}void p1isaac(int v){p1(v);}void p1add(int v){p1(v+128);}void p1sub(int v){p1(128-v);}
    void p2(int v){p1(v>>8);p1(v);}void p2add(int v){p1(v>>8);p1(v+128);}
    void ip2(int v){p1(v);p1(v>>8);}void ip2add(int v){p1(v+128);p1(v>>8);}
}
class SeqTypeList {static java.util.Map<Integer,SeqType> sequences=new java.util.HashMap<>();static SeqType get(int id){return sequences.get(id);}}
class Player extends PathingEntity {static int plane;}
class PlayerList {static int selfId;static Player self;static Player[] players=new Player[2048];}
class Npc extends PathingEntity {NpcType type=new NpcType();}
class NpcType {boolean rotationflag=true;}
class JagString {
    String value;JagString(String value){this.value=value;}static JagString parse(String value){return new JagString(value);}
    boolean equalsIgnoreCase(JagString other){return value.equalsIgnoreCase(other.value);}
    static JagString concatenate(JagString[] values){StringBuilder s=new StringBuilder();for(JagString v:values)s.append(v.value);return parse(s.toString());}
}
class LocalizedText {static JagString HIDDEN=JagString.parse("hidden"),EXAMINE=JagString.parse("Examine");}
class Cs1ScriptRunner {static boolean isMenuOpen;}
class MiniMenuEntry {MiniMenuEntry(int index){}}
class PluginRepository {static void DrawMiniMenu(MiniMenuEntry entry){}}
class MiniMap {static int examineCursor=-1;}
class Cross {static int x,y,type,milliseconds;}
class Mouse {static int clickX,clickY;}
class SceneSlot implements Comparable<SceneSlot> {
    int plane,x,y,layer;SceneSlot(int p,int x,int y,int l){plane=p;this.x=x;this.y=y;layer=l;}
    public int compareTo(SceneSlot b){int c=Integer.compare(plane,b.plane);if(c==0)c=Integer.compare(x,b.x);if(c==0)c=Integer.compare(y,b.y);if(c==0)c=Integer.compare(layer,b.layer);return c;}
}
class SceneGraph {
    static boolean levelsHidden;static int getTileHeight(int plane,int x,int z){return 0;}
    static java.util.TreeMap<SceneSlot,Long> objects=new java.util.TreeMap<>();
    static java.util.List<int[]> events=new java.util.ArrayList<>();
    static java.util.Set<Integer> unready=new java.util.HashSet<>();
    static long get(int p,int x,int y,int l){return objects.getOrDefault(new SceneSlot(p,x,y,l),0L);}
    static long getWallKey(int p,int x,int y){return get(p,x,y,0);}
    static long getWallDecorKey(int p,int x,int y){return get(p,x,y,1);}
    static long getSceneryKey(int p,int x,int y){return get(p,x,y,2);}
    static long getGroundDecorKey(int p,int x,int y){return get(p,x,y,3);}
    static void addLocModel(int id,int x,int plane,int angle,int y,int shape,int layer){
        var key=new SceneSlot(plane,x,y,layer);
        if(id<0)objects.remove(key);else objects.put(key,((long)id<<32)|((long)angle<<20)|((long)shape<<14));
        events.add(new int[]{id,x,plane,angle,y,shape,layer});
    }
}
class HashTable {HashTable(int capacity){}void put(Node node,long key){}}
class IntNode extends Node {int value;IntNode(int value){this.value=value;}}
class StringNode extends Node {StringNode(JagString value){}}
class IntUtils {static int clp2(int value){return value<=1?value:Integer.highestOneBit(value-1)<<1;}}
