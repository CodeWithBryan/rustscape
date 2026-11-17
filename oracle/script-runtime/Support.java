package oracle;
import java.io.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.Node;
import core.game.node.entity.Entity;
import core.game.node.entity.player.Player;
import core.game.node.scenery.Scenery;
import core.game.component.Component;
import core.game.world.GameWorld;
import core.game.world.map.Location;
import core.api.APIKt;

public class Support {
    static int number(Map row,String name){return ((Number)row.get(name)).intValue();}
    static Component component(Object value){return value==null?null:new Component(((Number)value).intValue());}
    static Object evaluate(JSONObject row){
        List<Object>values=new ArrayList<>();String error=null;
        try {
            String op=(String)row.get("op");
            if(op.equals("modal")){
                Player p=new Player();p.interfaceManager.opened=component(row.get("opened"));p.interfaceManager.chatbox=component(row.get("chatbox"));
                values.add(p.hasModalOpen());values.add(p.interfaceManager.isOpened());values.add(p.interfaceManager.hasChatbox());
            } else if(op.equals("clocks")){
                Entity e=new Entity();GameWorld.ticks=number(row,"tick");e.scripts.delay=number(row,"delay");e.clocks=new int[number(row,"length")];
                if(e.clocks.length>0)e.clocks[0]=number(row,"movement");if(e.clocks.length>6)e.clocks[6]=number(row,"stun");
                values.add(e.delayed());values.add(APIKt.isStunned(e));values.add(APIKt.finishedMoving(e));
            } else {
                List xyz=(List)row.get("location");Node n;
                if(op.equals("sceneryCenter")){
                    Scenery s=new Scenery();List dims=(List)row.get("dimensions");s.definition.sizeX=((Number)dims.get(0)).intValue();s.definition.sizeY=((Number)dims.get(1)).intValue();s.direction.value=number(row,"direction");n=s;
                } else {n=new Node();n.size=number(row,"size");}
                n.location=new Location(((Number)xyz.get(0)).intValue(),((Number)xyz.get(1)).intValue(),((Number)xyz.get(2)).intValue());
                Location center=n.getCenterLocation();values.add(Arrays.asList(center.getX(),center.getY(),center.getZ()));
            }
        } catch(ArrayIndexOutOfBoundsException e){error="Index";}
        Map<String,Object>result=new LinkedHashMap<>();result.put("values",values);result.put("error",error);return result;
    }
    public static void main(String[]args)throws Exception{
        BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));JSONParser parser=new JSONParser();String line;
        while((line=reader.readLine())!=null){JSONObject row=(JSONObject)parser.parse(line);row.put("expected",evaluate(row));System.out.println(JSONValue.toJSONString(row));}
    }
}
