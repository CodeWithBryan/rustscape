package oracle;
import java.util.*;import java.io.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.scenery.*;import core.game.world.map.*;
public class Support {
 static TreeMap<String,Scenery> nodes;static TreeMap<String,RegionChunk> chunks;static IdentityHashMap<Location,Integer> locations;static int generated;public static Integer failId;
 public static class Failure extends RuntimeException {}
 static int n(Object o){return ((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 static Location loc(Object value){List l=(List)value;return new Location(n(l.get(0)),n(l.get(1)),n(l.get(2)));}
 static Object coords(Location l){return Arrays.asList(l.getX(),l.getY(),l.getZ());}
 static int locationId(Location l){return locations.computeIfAbsent(l,k->locations.size());}
 static String label(Scenery o){if(o==null)return null;for(String k:nodes.keySet())if(nodes.get(k)==o)return k;String k="g"+generated++;nodes.put(k,o);return k;}
 static Map<String,Object> map(Object...p){Map<String,Object>m=new TreeMap<>();for(int i=0;i<p.length;i+=2)m.put((String)p[i],p[i+1]);return m;}
 static List<Object> cells(Scenery[][][] a){List<Object>r=new ArrayList<>();for(int x=0;x<8;x++)for(int y=0;y<8;y++)for(int i=0;i<4;i++)if(a[x][y][i]!=null)r.add(Arrays.asList(x,y,i,label(a[x][y][i])));return r;}
 static Object act(Map a){String op=(String)a.get("op");RegionChunk c=chunks.get(a.getOrDefault("chunk","main"));Scenery o=a.get("node")==null?null:nodes.get(a.get("node"));switch(op){
 case"static":c.addStatDyn(o,v(a,"x",7),v(a,"y",1));return null;
 case"add":return c.add(o);
 case"index":return c.getIndex(v(a,"x",7),v(a,"y",1),v(a,"id",-1),v(a,"shape",-1));
 case"copy":chunks.put((String)a.get("name"),c.copy());return null;
 case"rebase":c.rebaseObjects();return null;
 case"base":c.setBase(loc(a.get("value")));return null;
 case"currentBase":c.setCurrentBase(loc(a.get("value")));return null;
 case"resetFlags":c.resetClippingFlags();return null;
 case"rotation":c.setRotation(n(a.get("value")));return null;
 case"rotatePosition":{int[] r=RegionChunk.getRotatedPosition(v(a,"x",0),v(a,"y",0),v(a,"sx",1),v(a,"sy",1),v(a,"rotation",0),v(a,"chunkRotation",0));return Arrays.asList(r[0],r[1]);}
 case"cell":(b(a,"static")?c.getStatObjects():c.getObjects())[v(a,"x",7)][v(a,"y",1)][v(a,"slot",0)]=o;return null;
 case"active":o.setActive(b(a,"value"));return null;case"renderable":o.setRenderable(b(a,"value"));return null;case"charge":o.setCharge(n(a.get("value")));return null;case"rotate":o.setRotation(n(a.get("value")));return null;case"location":o.setLocation(loc(a.get("value")));return null;
 case"failId":failId=a.get("value")==null?null:n(a.get("value"));return null;
 default:throw new IllegalArgumentException(op);
 }}
 static Map<String,Object> snap(){Map<String,Object>cs=new TreeMap<>();for(String k:chunks.keySet()){RegionChunk c=chunks.get(k);cs.put(k,map("static",cells(c.getStatObjects()),"dynamic",cells(c.getObjects()),"base",coords(c.getBase()),"baseRef",locationId(c.getBase()),"currentBase",coords(c.getCurrentBase()),"currentRef",locationId(c.getCurrentBase()),"rotation",c.getRotation(),"flags",Arrays.asList(c.getFlags().baseX,c.getFlags().baseY,c.getFlags().baseZ),"projectile",Arrays.asList(c.getProjectileFlags().baseX,c.getProjectileFlags().baseY,c.getProjectileFlags().baseZ)));}
 Map<String,Object>ns=new TreeMap<>();for(String k:nodes.keySet()){Scenery o=nodes.get(k);ns.put(k,o==null?null:map("id",o.getId(),"shape",o.getType(),"rotation",o.getRotation(),"direction",o.getDirection().toInteger(),"location",coords(o.getLocation()),"locationRef",locationId(o.getLocation()),"active",o.isActive(),"renderable",o.isRenderable(),"constructed",o instanceof Constructed,"charge",o.getCharge()));}return map("chunks",cs,"nodes",ns);}
 static void setup(Map input){nodes=new TreeMap<>();chunks=new TreeMap<>();locations=new IdentityHashMap<>();generated=0;failId=null;core.cache.def.impl.SceneryDefinition.defs.clear();Map<String,Map> rows=new TreeMap<>((Map)input.get("nodes"));for(String k:rows.keySet()){Map r=rows.get(k);if(r==null){nodes.put(k,null);continue;}int id=n(r.get("id"));Location l=loc(r.get("location"));Scenery o=b(r,"constructed")?new Constructed(id,l,v(r,"shape",0),v(r,"rotation",0)):new Scenery(id,l,v(r,"shape",0),v(r,"rotation",0));o.setActive(!Boolean.FALSE.equals(r.get("active")));o.setRenderable(!Boolean.FALSE.equals(r.get("renderable")));o.setCharge(v(r,"charge",1000));nodes.put(k,o);}chunks.put("main",new RegionChunk(loc(input.getOrDefault("base",Arrays.asList(3200,3216,0))),v(input,"rotation",0)));}
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){JSONObject c=(JSONObject)new JSONParser().parse(line);setup(c);List<Object>out=new ArrayList<>();for(Object a:(List)c.get("actions")){Object result=null;String error=null;try{result=act((Map)a);}catch(Throwable e){error=e instanceof Failure?"Binding":e instanceof ArrayIndexOutOfBoundsException?"Index":e instanceof NullPointerException?"Null":e instanceof IllegalStateException?"State":e.getClass().getSimpleName();}Map<String,Object>s=snap();s.put("result",result);s.put("error",error);out.add(s);}c.put("expected",out);System.out.println(JSONValue.toJSONString(c));}}}
}
