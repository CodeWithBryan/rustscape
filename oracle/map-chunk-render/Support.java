package oracle;
import java.util.*;import java.io.*;import org.json.simple.*;import org.json.simple.parser.*;import core.game.world.map.*;import core.game.world.update.MapChunkRenderer;import core.game.node.entity.player.Player;
public class Support {
 static Map input;static Player player;static List<Object>events;static Map<String,Integer>calls;static Set<Integer>hooks;static Map<String,RegionChunk>chunks;static List<RegionChunk>all;static Map<Integer,Region>regions;static List<RegionChunk[][]>viewports;
 public static class Failure extends RuntimeException{}
 static int num(Object x){return((Number)x).intValue();}static Object get(Map m,String k,Object d){return m.containsKey(k)?m.get(k):d;}
 static Location loc(Object v){if(v==null)return null;List a=(List)v;return new Location(num(a.get(0)),num(a.get(1)),num(a.get(2)));}
 public static List<Integer>coord(Location l){return Arrays.asList(l.getX(),l.getY(),l.getZ());}static String key(Location l){return(l.getX()>>3)+","+(l.getY()>>3)+","+l.getZ();}static int rid(Location l){return((l.getX()>>6)<<8)|(l.getY()>>6);}
 static RegionChunk fresh(Location l){RegionChunk c=new RegionChunk(all.size()+1,new Location((l.getX()>>3)<<3,(l.getY()>>3)<<3,l.getZ()));all.add(c);chunks.put(key(l),c);return c;}
 static RegionChunk rawChunk(Location l){RegionChunk c=chunks.get(key(l));return c==null?fresh(l):c;}
 public static RegionChunk chunk(Location l){RegionChunk c=rawChunk(l);emit("chunk",coord(l));return c;}
 public static Region region(Location l){Region r=regions.computeIfAbsent(rid(l),Region::new);emit("region",coord(l));return r;}
 public static void emit(String event,Object...args){List<Object>a=new ArrayList<>();a.add(event);a.addAll(Arrays.asList(args));events.add(a);int n=calls.merge(event,1,Integer::sum);List hs=(List)get(input,"hooks",Collections.emptyList());for(int i=0;i<hs.size();i++){Map h=(Map)hs.get(i);if(event.equals(h.get("event"))&&n==num(get(h,"at",1))&&hooks.add(i))for(Object op:(List)h.get("actions"))act((Map)op);}Map f=(Map)get(input,"failures",Collections.emptyMap());if(f.containsKey(event)&&num(f.get(event))==n)throw new Failure();}
 static Object act(Map a){switch((String)a.get("op")){
 case"render":MapChunkRenderer.render(player);return null;
 case"set":if(a.containsKey("location"))player.rawLocation=loc(a.get("location"));if(a.containsKey("anchor"))player.flags.sceneGraph=loc(a.get("anchor"));if(a.containsKey("name"))player.rawName=(String)a.get("name");return null;
 case"replaceChunk":fresh(loc(a.get("location")));return null;
 case"alias":chunks.put(key(loc(a.get("location"))),rawChunk(loc(a.get("other"))));return null;
 case"rebase":rawChunk(loc(a.get("location"))).currentBase=loc(a.get("base"));return null;
 case"setRow":player.flags.viewport[num(a.get("x"))]=a.get("length")==null?null:new RegionChunk[num(a.get("length"))];return null;
 case"newViewport":player.flags.viewport=new RegionChunk[13][13];viewports.add(player.flags.viewport);return null;
 default:throw new IllegalArgumentException();}}
 static Object viewport(RegionChunk[][]v){List<Object>out=new ArrayList<>();for(RegionChunk[]r:v){if(r==null){out.add(null);continue;}List<Object>row=new ArrayList<>();for(RegionChunk c:r)row.add(c==null?null:c.id);out.add(row);}return out;}
 static Object evaluate(Map data){input=data;player=new Player();player.rawLocation=loc(get(data,"location",Arrays.asList(3207,3216,0)));player.flags.sceneGraph=loc(data.get("anchor"));events=new ArrayList<>();calls=new HashMap<>();hooks=new HashSet<>();chunks=new TreeMap<>();all=new ArrayList<>();regions=new TreeMap<>();viewports=new ArrayList<>();viewports.add(player.flags.viewport);List<Object>results=new ArrayList<>();
 for(Object command:(List)data.get("actions")){Map result=new LinkedHashMap();try{result.put("value",act((Map)command));}catch(Throwable e){while(e.getCause()!=null)e=e.getCause();result.put("error",e instanceof Failure?"Failure":e instanceof NullPointerException?"Null":e instanceof IndexOutOfBoundsException?"Index":e.getClass().getSimpleName());}List<Object>vs=new ArrayList<>();for(RegionChunk[][]v:viewports)vs.add(viewport(v));result.put("viewports",vs);results.add(result);}
 Map out=new LinkedHashMap();out.put("results",results);out.put("events",events);List<Object>cs=new ArrayList<>();for(RegionChunk c:all)cs.add(coord(c.currentBase));out.put("chunks",cs);Map rs=new TreeMap();for(Region r:regions.values()){Map s=new LinkedHashMap();s.put("active",r.active);s.put("viewers",r.viewingPlayers);rs.put(""+r.id,s);}out.put("regions",rs);out.put("location",coord(player.rawLocation));out.put("anchor",player.flags.sceneGraph==null?null:coord(player.flags.sceneGraph));out.put("name",player.rawName);return out;}
 public static void main(String[]args)throws Exception{BufferedReader r=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));JSONParser p=new JSONParser();String line;while((line=r.readLine())!=null){Map input=(Map)p.parse(line);Object expected=evaluate(input);Map out=new LinkedHashMap(input);out.put("expected",expected);System.out.println(JSONValue.toJSONString(out));}}
}
