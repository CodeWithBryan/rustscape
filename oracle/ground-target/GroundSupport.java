package oracle;
import java.util.*;import core.game.node.item.*;import core.game.world.map.Location;import core.game.interaction.Option;
public class GroundSupport{
 static void captureErrors(){System.setErr(new java.io.PrintStream(new java.io.OutputStream(){StringBuilder line=new StringBuilder();public void write(int b){if(b==10){String text=line.toString();line.setLength(0);if(text.startsWith("java.lang.NullPointerException"))Support.emit("exception","Null");}else if(b!=13)line.append((char)b);}}));}
 static GroundItem ground;
 static Location loc(Object o){if(o==null)return null;List a=(List)o;return Location.create(Support.n(a.get(0)),Support.n(a.get(1)),Support.n(a.get(2)));}
 static void replace(Map a){Location l=a.containsKey("location")?loc(a.get("location")):Location.create(3211,3217,0);Item i=new Item(1);
  ground=Boolean.TRUE.equals(a.get("spawn"))?new GroundSpawnAdapter(i,l):new GroundItem(i,l,Support.v(a,"decay",200),null);
  ground.size=Support.v(a,"size",1);ground.setRemoved(Boolean.TRUE.equals(a.get("removed")));if(a.containsKey("decay"))Support.field(ground,"decayTime",Support.n(a.get("decay")));}
 static void reset(Map input){replace((Map)input.get("ground"));Support.option=new Option("Take",2).setHandler(Support.handler(input));}
 static void act(Map a){switch((String)a.get("op")){
  case"groundReplace":replace(a);break;
  case"groundSet":if(a.containsKey("location"))ground.location=loc(a.get("location"));if(a.containsKey("size"))ground.size=Support.n(a.get("size"));if(a.containsKey("removed"))ground.setRemoved(Boolean.TRUE.equals(a.get("removed")));if(a.containsKey("decay"))Support.field(ground,"decayTime",Support.n(a.get("decay")));break;
  default:throw new IllegalArgumentException((String)a.get("op"));}}
 static void snapshot(Map o){o.put("ground",Arrays.asList(Support.coord(ground.getLocation()),ground.size(),ground.isRemoved(),ground.getDecayTime(),ground instanceof GroundSpawnAdapter));Option opt=Support.option;o.put("option",Arrays.asList(opt.getName(),opt.getIndex(),opt.getHandler()==null?null:Support.field(opt.getHandler(),"token")));}
}
