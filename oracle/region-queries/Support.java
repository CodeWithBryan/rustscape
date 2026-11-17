package oracle;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.world.map.*;
public class Support{
 static Map input;static Region[]regions;static RegionChunk[]chunks;static int selected;static Location point;
 static List<Object>events;static Map<String,Integer>calls;static int lastKey;static boolean lastProjectile;
 static class Failure extends RuntimeException{}
 static int n(Object o){return((Number)o).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}
 public static void emit(String event,Object...args){List<Object>r=new ArrayList<>();r.add(event);r.addAll(Arrays.asList(args));events.add(r);int at=calls.merge(event,1,Integer::sum);
 for(Object o:(List)input.getOrDefault("hooks",Collections.emptyList())){Map h=(Map)o;if(event.equals(h.get("event"))&&at==v(h,"at",1))for(Object a:(List)h.get("actions"))act((Map)a);}}
 public static Region region(int id){Region r=regions[selected];emit("region",id,r.key);return r;}
 public static Integer[]flags(int key,boolean projectile){Integer[]a=RegionManager.originalFlags(key,projectile);lastKey=key;lastProjectile=projectile;emit("flags",key,projectile);return a;}
 static Map<Integer,Integer[]>flagMap(boolean p){return p?RegionManager.PROJECTILE_FLAGS:RegionManager.CLIPPING_FLAGS;}
 static boolean[][]land(Object o){if(o==null)return null;if(o instanceof Boolean){boolean[][]a=new boolean[8][8];for(boolean[]r:a)Arrays.fill(r,(Boolean)o);return a;}List l=(List)o;boolean[][]a=new boolean[l.size()][];for(int i=0;i<a.length;i++){if(l.get(i)==null)continue;List row=(List)l.get(i);a[i]=new boolean[row.size()];for(int j=0;j<a[i].length;j++)a[i][j]=Boolean.TRUE.equals(row.get(j));}return a;}
 static RegionChunk[][][]array(Object value){if(value==null)return null;RegionChunk[][][]a=new RegionChunk[8][8][4];for(int x=0;x<8;x++)for(int y=0;y<8;y++)Arrays.fill(a[x][y],chunks[n(value)]);return a;}
 static Object query(String q){switch(q){case"clip":return RegionManager.getClippingFlag(point,false);case"projectile":return RegionManager.getClippingFlag(point,true);case"landscape":return RegionManager.isLandscape(point);case"teleport":return RegionManager.isTeleportPermitted(point);case"clipped":return RegionManager.isClipped(point);case"water":return RegionManager.getWaterClipFlag(point);default:throw new IllegalArgumentException(q);}}
 static Object act(Map a){Object value=a.get("value");switch((String)a.get("op")){
 case"clip":case"projectile":case"landscape":case"teleport":case"clipped":case"water":return query((String)a.get("op"));
 case"query":return query((String)a.get("name"));case"fail":throw new Failure();case"select":selected=n(value);break;
 case"has":regions[v(a,"region",0)].has=Boolean.TRUE.equals(value);break;
 case"location":List l=(List)value;point.moveTo(n(l.get(0)),n(l.get(1)),n(l.get(2)));break;
 case"land":chunks[v(a,"chunk",0)].flags.setLandscape(land(value));break;
 case"chunks":regions[v(a,"region",0)].setChunks(array(value));break;
 case"fill":Arrays.fill(RegionManager.originalFlags(lastKey,lastProjectile),n(value));break;
 case"replace":Integer[]f=new Integer[64];Arrays.fill(f,n(value));flagMap(lastProjectile).put(lastKey,f);break;
 default:throw new IllegalArgumentException();}return null;}
 static Object matrix(boolean[][]a){if(a==null)return null;List<Object>out=new ArrayList<>();for(boolean[]r:a){if(r==null){out.add(null);continue;}List<Object>v=new ArrayList<>();for(boolean b:r)v.add(b);out.add(v);}return out;}
 static Object snapshot(Object result,String error){Map<String,Object>r=new TreeMap<>();List<Object>f=new ArrayList<>();for(boolean p:new boolean[]{false,true})for(int key:new TreeSet<>(flagMap(p).keySet()))f.add(Arrays.asList(key,p,new ArrayList<>(Arrays.asList(flagMap(p).get(key)))));
 r.put("result",result);r.put("error",error);r.put("events",new ArrayList<>(events));r.put("selected",selected);r.put("has",Arrays.asList(regions[0].has,regions[1].has));
 r.put("chunks",Arrays.asList(regions[0].raw()==null?null:regions[0].raw()[0][0][0]==chunks[0]?0:1,regions[1].raw()==null?null:regions[1].raw()[0][0][0]==chunks[0]?0:1));
 r.put("landscape",Arrays.asList(matrix(chunks[0].flags.getLandscape()),matrix(chunks[1].flags.getLandscape())));r.put("flags",f);r.put("location",point==null?null:Arrays.asList(point.getX(),point.getY(),point.getZ()));return r;}
 public static void main(String[]args)throws Exception{try(BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=reader.readLine())!=null){input=(Map)new JSONParser().parse(line);Map out=new TreeMap(input);selected=0;events=new ArrayList<>();calls=new HashMap<>();chunks=new RegionChunk[]{new RegionChunk(),new RegionChunk()};regions=new Region[]{new Region(0),new Region(1)};regions[0].has=!Boolean.FALSE.equals(input.get("hasFlags"));regions[1].has=true;regions[0].setChunks(array(0));regions[1].setChunks(array(1));chunks[0].flags.setLandscape(land(input.getOrDefault("landscape",true)));chunks[1].flags.setLandscape(land(false));RegionManager.CLIPPING_FLAGS.clear();RegionManager.PROJECTILE_FLAGS.clear();Object coords=input.getOrDefault("location",Arrays.asList(3207L,3216L,0L));if(coords==null)point=null;else{List l=(List)coords;point=new Location(n(l.get(0)),n(l.get(1)),n(l.get(2)));}
 if(point!=null){int key=(point.getZ()<<22)|((point.getX()>>3)<<11)|(point.getY()>>3);Integer[]f=new Integer[64];Arrays.fill(f,v(input,"flag",0));flagMap(false).put(key,f);}lastKey=0;lastProjectile=false;
 List<Object>states=new ArrayList<>();for(Object a:(List)input.get("actions")){events.clear();Object result=null;String error=null;try{result=act((Map)a);}catch(Failure e){error="Binding";}catch(NullPointerException e){error="Null";}catch(IndexOutOfBoundsException e){error="Index";}states.add(snapshot(result,error));}out.put("expected",states);System.out.println(JSONValue.toJSONString(out));
 }}}
}
