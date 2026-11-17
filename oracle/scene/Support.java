package oracle;
import java.util.*;import java.io.*;import java.nio.*;import java.security.*;import java.lang.reflect.*;
import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.cache.def.impl.SceneryDefinition;import core.game.node.scenery.*;import core.game.node.item.GroundItem;
import core.game.world.map.*;import core.game.system.task.Pulse;import core.game.world.update.flag.chunk.ObjectUpdateFlag;import core.net.packet.IoBuffer;
public class Support {
 static Map input,samples;static TreeMap<Integer,Region>regions;static TreeMap<Integer,Integer[]>walking,projectile;
 static TreeMap<Integer,List<ObjectUpdateFlag>>updates;static Map<Integer,RegionChunk>chunks;static TreeMap<String,Scenery>nodes;
 static List<Pulse>queue;static List<Object>events;public static core.net.IoSession session;public static boolean moveNear=false;
 static int n(Object v){return((Number)v).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 static Location loc(Object v){List l=(List)v;return new Location(n(l.get(0)),n(l.get(1)),n(l.get(2)));}
 public static Object coords(Location l){return Arrays.asList(l.getX(),l.getY(),l.getZ());}
 static byte[]bytes(String s){byte[]b=new byte[s.length()/2];for(int i=0;i<b.length;i++)b[i]=(byte)Integer.parseInt(s.substring(i*2,i*2+2),16);return b;}
 static String hex(byte[]v){StringBuilder s=new StringBuilder();for(byte b:v)s.append(String.format("%02x",b&255));return s.toString();}
 static Map map(Object...v){Map<String,Object>m=new TreeMap<>();for(int i=0;i<v.length;i+=2)m.put((String)v[i],v[i+1]);return m;}
 public static void emit(String name,Object...args){List<Object>e=new ArrayList<>();e.add(name);Collections.addAll(e,args);events.add(e);}
 public static Region region(int id){return regions.computeIfAbsent(id,k->new Region((k>>8)&255,k&255));}
 public static int archive(String s){int split=s.indexOf('_');int x=Integer.parseInt(s.substring(1,split)),y=Integer.parseInt(s.substring(split+1));if(input.containsKey("cacheRegions")){Map r=(Map)((Map)input.get("cacheRegions")).get(Integer.toString(x<<8|y));return r==null?-1:v(r,s.charAt(0)=='m'?"mapGroup":"landGroup",-1);}if(b(input,"missing")||x<49||x>51||y<49||y>51)return-1;return((x<<8)|y)*2+(s.charAt(0)=='l'?1:0);}
 static byte[]cacheBytes(int group,String kind){for(Object v:((Map)input.get("cacheRegions")).values()){Map r=(Map)v;if(v(r,kind+"Group",-1)==group){String s=(String)r.get(kind);return s==null?null:bytes(s);}}throw new AssertionError("Missing cache group "+group);}
 public static byte[]mapBytes(int group){return input.containsKey("cacheRegions")?cacheBytes(group,"map"):new byte[16384];}
 public static byte[]landBytes(int group){return input.containsKey("cacheRegions")?cacheBytes(group,"land"):bytes(group/2==12850?(String)input.get("land"):"00000000");}
 public static int definitionCount(){return n(input.get("definitionCount"));}
 public static byte[]definitionFile(int id){String s=(String)((Map)input.get("definitionFiles")).get(Integer.toString(id));return s==null?null:bytes(s);}
 public static Integer[]flags(int k,boolean p){return(p?projectile:walking).computeIfAbsent(k,id->{Integer[]a=new Integer[64];Arrays.fill(a,p?0:-1);return a;});}
 public static void border(Location l,boolean p,int mask,boolean add){Region r=region(l.getRegionId());Region.load(r);if(!r.isHasFlags())return;core.game.world.map.build.ChunkFlags f=p?RegionManager.getRegionChunk(l).getProjectileFlags():RegionManager.getRegionChunk(l).getFlags();if(add)f.flag(l.getChunkOffsetX(),l.getChunkOffsetY(),mask);else f.unflag(l.getChunkOffsetX(),l.getChunkOffsetY(),mask);}
 static int key(RegionChunk c){Location l=c.getCurrentBase();return(l.getZ()<<22)|((l.getX()>>3)<<11)|(l.getY()>>3);}
 public static void flag(RegionChunk c,ObjectUpdateFlag f){int k=key(c);chunks.put(k,c);updates.computeIfAbsent(k,v->new ArrayList<>()).add(f);}
 public static void submit(Pulse p){queue.add(p);}
 public static void ground(GroundItem i){emit("ground",i==null?null:i.id);}
 static void tick(){int count=queue.size();for(int i=0;i<count;i++){Pulse p=queue.remove(0);if(!p.update()&&p.isRunning())queue.add(p);}}
 static Object value(Scenery o){if(o==null)return null;Scenery previous=o instanceof Constructed?((Constructed)o).getReplaced():null;return Arrays.asList(o.getId(),o.getType(),o.getRotation(),coords(o.getLocation()),o.isActive(),o.isRenderable(),o instanceof Constructed,o.getCharge(),previous==null?null:Arrays.asList(previous.getId(),coords(previous.getLocation()),previous.getRotation()));}
 static Object aliases(Scenery o){List<String>a=new ArrayList<>();for(String k:nodes.keySet())if(o!=null&&nodes.get(k)==o)a.add(k);return a;}
 static Object snapshot()throws Exception{
  List<Object>rs=new ArrayList<>(),storage=new ArrayList<>();for(int id:regions.keySet()){Region r=regions.get(id);rs.add(Arrays.asList(id,r.isLoaded(),r.isBuild(),r.isHasFlags(),r.getObjectCount()));for(int cx=0;cx<8;cx++)for(int cy=0;cy<8;cy++)for(int z=0;z<4;z++)for(int stat=0;stat<2;stat++){RegionChunk c=r.getChunks()[cx][cy][z];Scenery[][][]a=stat==0?c.getStatObjects():c.getObjects();for(int x=0;x<8;x++)for(int y=0;y<8;y++)for(int s=0;s<4;s++){Scenery o=a[x][y][s];if(o!=null)storage.add(Arrays.asList(id,cx,cy,z,stat,x,y,s,value(o),aliases(o)));}}}
  MessageDigest hash=MessageDigest.getInstance("SHA-256");TreeSet<Integer>keys=new TreeSet<>();keys.addAll(walking.keySet());keys.addAll(projectile.keySet());for(int k:keys)for(int p=0;p<2;p++){Integer[]a=(p==1?projectile:walking).get(k);if(a==null)continue;hash.update(ByteBuffer.allocate(5).putInt(k).put((byte)p).array());for(int i:a)hash.update(ByteBuffer.allocate(4).putInt(i).array());}
  Map<String,Object>held=new TreeMap<>();for(String k:nodes.keySet())held.put(k,value(nodes.get(k)));
  List<Object>pulses=new ArrayList<>();Field passed=Pulse.class.getDeclaredField("ticksPassed");passed.setAccessible(true);for(Pulse p:queue)pulses.add(Arrays.asList(p.getDelay(),passed.get(p),p.isRunning()));
  List<Object>frames=new ArrayList<>();for(int k:updates.keySet()){IoBuffer buffer=core.net.packet.out.UpdateAreaPosition.getChunkUpdateBuffer(new core.game.node.entity.player.Player(),chunks.get(k).getCurrentBase());for(ObjectUpdateFlag f:updates.get(k))f.write(buffer);session.write(buffer);frames.add(Arrays.asList(k,hex(session.bytes)));}updates.clear();
  return map("regions",rs,"storage",storage,"flags",hex(hash.digest()),"held",held,"queue",pulses,"frames",frames,"events",new ArrayList<>(events));
 }
 static void act(Map a){Scenery h=a.get("node")==null?null:nodes.get(a.get("node"));switch((String)a.get("op")){
  case"load":Region.load(region(n(a.get("id"))),b(a,"build"));break;
  case"lookup":{Location l=loc(a.get("location"));nodes.put((String)a.get("name"),RegionManager.getObject(l.getX(),l.getY(),l.getZ(),-1,0));break;}
  case"door":core.game.global.action.DoorActionHandler.handleDoor(new core.game.node.entity.player.Player(),h);break;
  case"tick":for(int i=0;i<n(a.get("count"));i++)tick();break;
  case"remove":SceneryBuilder.remove(h);break;
  case"respawn":SceneryBuilder.remove(h,n(a.get("ticks")));break;
  case"expire":SceneryBuilder.add(h,n(a.get("ticks")));break;
  case"client":SceneryBuilder.replace(h,new Scenery(12348,new Location(3207,3217,0),0,1),n(a.get("ticks")),false);break;
  default:throw new AssertionError(a);
 }}
 static void setup(Map c)throws Throwable{input=c;regions=new TreeMap<>();walking=new TreeMap<>();projectile=new TreeMap<>();updates=new TreeMap<>();chunks=new HashMap<>();nodes=new TreeMap<>();queue=new ArrayList<>();events=new ArrayList<>();session=new core.net.IoSession();SceneryDefinition.getDefinitions().clear();if(input.containsKey("definitionCount")){SceneryDefinition.parse();}else for(String id:new TreeSet<String>(samples.keySet())){int n=Integer.parseInt(id);SceneryDefinition.getDefinitions().put(n,SceneryDefinition.parseDefinition(n,ByteBuffer.wrap(bytes((String)samples.get(id)))));}}
 public static void main(String[]args)throws Throwable{samples=(Map)new JSONParser().parse(new FileReader(args[0]));core.ServerConstants.CONFIG_PATH=new File(args[1]).getParent()+File.separator;core.game.system.config.DoorConfigLoader loader=new core.game.system.config.DoorConfigLoader();events=new ArrayList<>();loader.load();try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));BufferedWriter w=new BufferedWriter(new OutputStreamWriter(System.out,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){JSONObject c=(JSONObject)new JSONParser().parse(line);setup(c);List<Object>out=new ArrayList<>();for(Object a:(List)c.get("actions")){events.clear();act((Map)a);out.add(snapshot());}c.put("expected",out);JSONValue.writeJSONString(c,w);w.newLine();w.flush();}}}
}
