package oracle;
import java.io.*;import java.util.*;import org.json.simple.*;import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.Player;import core.game.node.entity.player.link.*;
public class Support{
 public static IronmanManager manager;static Map input;static List<Object>events;
 public static class Failure extends RuntimeException{}
 static IronmanMode mode(Object v){return v==null?null:IronmanMode.values()[((Number)v).intValue()];}
 public static void message(String text){events.add(Arrays.asList("message",text));if(input.containsKey("messageMode"))manager.setMode(mode(input.get("messageMode")));if(Boolean.TRUE.equals(input.get("failMessage")))throw new Failure();}
 static Object act(Map a,List<IronmanManager>aliases){IronmanManager m=Boolean.TRUE.equals(a.get("alias"))?aliases.get(0):manager;switch((String)a.get("op")){
 case"set":m.setMode(mode(a.get("mode")));return null;case"is":return m.isIronman();case"default":return m.checkRestriction();case"check":return m.checkRestriction(mode(a.get("minimum")));case"owner":return m.getPlayer()!=null;case"capture":aliases.add(manager);return aliases.get(0)==manager;case"icons":List<Object>r=new ArrayList<>();for(IronmanMode v:IronmanMode.values())r.add(v.getIcon());return r;default:throw new AssertionError();}}
 public static void main(String[]args)throws Exception{try(BufferedReader r=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8))){String line;while((line=r.readLine())!=null){input=(Map)new JSONParser().parse(line);Map output=new TreeMap(input);Player player=Boolean.TRUE.equals(input.get("player"))?new Player():null;manager=new IronmanManager(player);List<IronmanManager>aliases=new ArrayList<>();List<Object>states=new ArrayList<>();
 for(Object value:(List)input.get("actions")){events=new ArrayList<>();Object result=null;String error=null;try{result=act((Map)value,aliases);}catch(NullPointerException e){error="Null";}catch(Failure e){error="Binding";}Map<String,Object>s=new TreeMap<>();s.put("result",result);s.put("error",error);s.put("mode",manager.getMode()==null?null:manager.getMode().ordinal());s.put("isIronman",manager.isIronman());s.put("player",manager.getPlayer()!=null);s.put("samePlayer",manager.getPlayer()==player);s.put("aliases",aliases.size());s.put("events",new ArrayList<>(events));states.add(s);}output.put("expected",states);System.out.println(JSONValue.toJSONString(output));}}}
}
