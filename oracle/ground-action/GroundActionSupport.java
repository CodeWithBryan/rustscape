package oracle;
import java.util.*;import core.game.interaction.*;import core.net.packet.PacketProcessor;import core.net.packet.in.Packet;
public class GroundActionSupport{
 public static boolean plugin,ground,item;public static InteractPlugin[]interactions;static Option[]options;
 static void reset(){plugin=ground=item=false;interactions=new InteractPlugin[]{new InteractPlugin(0),new InteractPlugin(1)};options=new Option[]{new Option("Take",2,0),new Option("Use",0,1),Option.NULL};interactions[0].options[2]=options[0];interactions[1].options[0]=options[1];}
 static Object act(Map a){switch((String)a.get("op")){
  case"fail":throw new Support.Failure();
  case"groundProcess":Packet.GroundItemAction p=new Packet.GroundItemAction();p.player=Support.player(a);p.x=ManagerSupport.n(a,"x",3207);p.y=ManagerSupport.n(a,"y",3216);p.id=ManagerSupport.n(a,"id",1);p.optIndex=ManagerSupport.n(a,"index",2);PacketProcessor.handle(p);return null;
  case"groundActionSet":if(a.containsKey("plugin"))plugin=Boolean.TRUE.equals(a.get("plugin"));if(a.containsKey("ground"))ground=Boolean.TRUE.equals(a.get("ground"));if(a.containsKey("item"))item=Boolean.TRUE.equals(a.get("item"));
   if(a.containsKey("interaction"))Support.ground(a).interactionKey=a.get("interaction")==null?null:Support.num(a.get("interaction"));
   if(a.containsKey("options")){List values=(List)a.get("options");Option[]array=new Option[values.size()];for(int i=0;i<array.length;i++)array[i]=values.get(i)==null?null:options[Support.num(values.get(i))];interactions[ManagerSupport.n(a,"table",0)].options=array;}return null;
  default:throw new IllegalArgumentException((String)a.get("op"));}}
 static void snapshot(Map s){List<Object>tables=new ArrayList<>(),receivers=new ArrayList<>();for(InteractPlugin p:interactions){List<Object>array=new ArrayList<>();for(Option o:p.options)array.add(o==null?null:o.key);tables.add(array);}for(var g:Support.grounds)receivers.add(g.interactionKey);s.put("groundAction",Arrays.asList(plugin,ground,item,tables,receivers));}
}
