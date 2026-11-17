package oracle;
import java.io.*;import java.util.*;import java.security.*;import java.lang.reflect.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.world.map.*;import core.game.world.map.path.*;
public class Support {
 static Map input;static long probes;static MessageDigest digest;static List<Object> prefix,suffix;static Location start,end;
 static int n(Object v){return ((Number)v).intValue();}static int v(Map m,String k,int d){return m.get(k)==null?d:n(m.get(k));}static boolean b(Map m,String k){return Boolean.TRUE.equals(m.get(k));}
 static int[] a(Map m,String k,int[] fallback){List a=(List)m.get(k);if(a==null)return fallback;int[] r=new int[a.size()];for(int i=0;i<r.length;i++)r[i]=n(a.get(i));return r;}
 static Location location(Map m,String k,int[] fallback){if(m.containsKey(k)&&m.get(k)==null)return null;int[] a=a(m,k,fallback);return Location.create(a[0],a[1],a[2]);}
 static void integer(MessageDigest d,int x){d.update((byte)(x>>>24));d.update((byte)(x>>>16));d.update((byte)(x>>>8));d.update((byte)x);}
 static String hex(byte[] data){StringBuilder b=new StringBuilder();for(byte v:data)b.append(String.format("%02x",v&255));return b.toString();}
 public static int clip(int z,int x,int y,boolean global){probes++;List<Object> event=Arrays.asList(global?1:0,z,x,y);for(Object q:event)integer(digest,n(q));if(prefix.size()<24)prefix.add(event);suffix.add(event);if(suffix.size()>24)suffix.remove(0);
  if(probes==v(input,"failAt",-1))throw new IllegalStateException("oracle clipping failure");
  if(probes==v(input,"mutateAt",-1)){if(input.containsKey("mutateStart")){int[] p=a(input,"mutateStart",null);start.setX(p[0]);start.setY(p[1]);start.setZ(p[2]);}if(input.containsKey("mutateEnd")){int[] p=a(input,"mutateEnd",null);end.setX(p[0]);end.setY(p[1]);end.setZ(p[2]);}}
  int flag=v(input,global?"worldDefault":"default",0);for(Object row:(List)input.getOrDefault(global?"worldFlags":"flags",Collections.emptyList())){List f=(List)row;if(n(f.get(0))==x&&n(f.get(1))==y&&(f.size()<4||n(f.get(3))==z))flag=n(f.get(2));}return flag;
 }
 static Object state(SmartPathfinder finder)throws Exception{Map<String,Object> out=new TreeMap<>();for(String name:new String[]{"curX","curY","dstX","dstY","writePathPosition","foundPath"}){Field f=SmartPathfinder.class.getDeclaredField(name);f.setAccessible(true);out.put(name,f.get(finder));}for(String name:new String[]{"queueX","queueY","via","cost"}){MessageDigest d=MessageDigest.getInstance("SHA-256");Field f=SmartPathfinder.class.getDeclaredField(name);f.setAccessible(true);Object data=f.get(finder);if(data instanceof int[])for(int v:(int[])data)integer(d,v);else for(int[] row:(int[][])data)for(int v:row)integer(d,v);out.put(name,hex(d.digest()));}return out;}
 static Object coords(Location l){return l==null?null:Arrays.asList(l.getX(),l.getY(),l.getZ());}
 public static void main(String[] args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);probes=0;digest=MessageDigest.getInstance("SHA-256");prefix=new ArrayList<>();suffix=new ArrayList<>();start=location(input,"start",new int[]{3207,3216,0});end=location(input,"end",new int[]{3210,3218,0});Map<String,Object> out=new TreeMap<>();String error=null;Object result=null;SmartPathfinder finder=null;
  int[] from=a(input,"from",new int[]{3207,3216,0}),to=a(input,"to",new int[]{3208,3216}),mover=a(input,"mover",new int[]{1,1}),size=a(input,"size",new int[]{1,1});ClipMaskSupplier cms=(z,x,y)->clip(z,x,y,false);
  try{String op=(String)input.getOrDefault("op","find");switch(op){
   case "door":result=Pathfinder.canDoorInteract(from[0],from[1],mover[0],to[0],to[1],v(input,"kind",0),v(input,"rotation",0),from[2],cms);break;
   case "decoration":result=Pathfinder.canDecorationInteract(from[0],from[1],mover[0],to[0],to[1],v(input,"rotation",0),v(input,"kind",0),from[2],cms);break;
   case "standing":result=Pathfinder.isStandingIn(from[0],from[1],mover[0],mover[1],to[0],to[1],size[0],size[1]);break;
   case "rectangle":result=Pathfinder.canInteract(from[0],from[1],mover[0],to[0],to[1],size[0],size[1],v(input,"walkingFlag",0),from[2],cms);break;
   case "sized":result=Pathfinder.canInteractSized(from[0],from[1],mover[0],mover[1],to[0],to[1],size[0],size[1],v(input,"walkingFlag",0),from[2]);break;
   case "find":finder=new SmartPathfinder();core.game.world.GameWorld.INSTANCE.getSettings().setSmartpathfinder_bfs(b(input,"bfs"));core.ServerConstants.MAX_PATHFIND_DISTANCE=v(input,"maxDistance",25);Path path=finder.find(start,mover[0],end,size[0],size[1],v(input,"rotation",0),v(input,"kind",0),v(input,"walkingFlag",0),b(input,"near"),Boolean.TRUE.equals(input.get("nullSupplier"))?null:cms);List<Object> points=new ArrayList<>();for(Point p:path.getPoints())points.add(Arrays.asList(p.getX(),p.getY()));result=Arrays.asList(path.isSuccessful(),path.isMoveNear(),points);break;
   default:throw new IllegalArgumentException(op);
  }}catch(NullPointerException ex){error="Null";}catch(IndexOutOfBoundsException ex){error="Index";}catch(IllegalStateException ex){error="Binding";}
  out.put("result",result);out.put("error",error);out.put("probes",probes);out.put("probeHash",hex(digest.digest()));out.put("prefix",prefix);out.put("suffix",suffix);out.put("start",coords(start));out.put("end",coords(end));out.put("state",finder==null?null:state(finder));input.put("expected",out);System.out.println(JSONValue.toJSONString(input));
 }}}
}
