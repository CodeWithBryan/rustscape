package oracle;
import java.util.*;
import core.game.node.item.*;
import core.game.node.entity.player.Player;
import core.game.global.action.PickupHandler;
import content.global.handlers.item.PickupPlugin;
import content.data.GodType;
import content.global.skill.runecrafting.RunePouch;
public class PickupSupport {
    public static boolean initializing,permitted,pluginResult;
    public static Inventory[]inventories;
    public static PickupPlugin handler;
    public static class Inventory {
        public int id;boolean space=true,add=true;List<Object>added=new ArrayList<>();
        Inventory(int i){id=i;}
        public boolean hasSpaceFor(Item i){boolean v=space;Support.emit("space",id,Arrays.asList(i.getId(),i.getAmount(),i.getCharge()),v);return v;}
        public boolean add(Item i){boolean v=add;if(v)added.add(Arrays.asList(i.getId(),i.getAmount(),i.getCharge()));Support.emit("addInventory",id,Arrays.asList(i.getId(),i.getAmount(),i.getCharge()),v);return v;}
    }
    static void reset(){
        inventories=new Inventory[]{new Inventory(0),new Inventory(1)};permitted=true;pluginResult=true;handler=new PickupPlugin();
        initializing=true;
        int[]capes={2412,2413,2414};for(int i=0;i<3;i++)GodType.values()[i].cape=new Item(capes[i]);
        int[]pouches={5509,5510,5512,5514};for(int i=0;i<4;i++){RunePouch.values()[i].pouch=new Item(pouches[i]);RunePouch.values()[i].decayed=new Item(pouches[i]+1);}
        initializing=false;
    }
    static Object act(Map a){switch((String)a.get("op")){
        case"pickupSet":{
            Player p=Support.player(a);
            if(a.containsKey("deadline")){Object v=a.get("deadline");p.pickupAttributes.put("droppedItem:"+ManagerSupport.n(a,"id",1),v instanceof Number?((Number)v).intValue():v);}
            if(a.containsKey("pickup"))p.pickupAttributes.put("pickup",a.get("pickup"));
            if(Boolean.TRUE.equals(a.get("removePickup")))p.pickupAttributes.remove("pickup");
            if(a.containsKey("ironman"))p.ironman=(Boolean)a.get("ironman");
            if(a.containsKey("name"))p.rawName((String)a.get("name"));
            if(a.containsKey("inventory"))p.inventoryId=ManagerSupport.n(a,"inventory",0);
            if(a.containsKey("owned")){p.owned.clear();for(Object i:(List)a.get("owned"))p.owned.add(Support.num(i));}
            if(a.containsKey("permitted"))permitted=(Boolean)a.get("permitted");
            if(a.containsKey("pluginResult"))pluginResult=(Boolean)a.get("pluginResult");return null;}
        case"inventorySet":{Inventory i=inventories[ManagerSupport.n(a,"inventory",0)];if(a.containsKey("space"))i.space=(Boolean)a.get("space");if(a.containsKey("add"))i.add=(Boolean)a.get("add");return null;}
        case"capeSet":GodType.values()[ManagerSupport.n(a,"index",0)].cape.setId(ManagerSupport.n(a,"id",1));return null;
        case"pouchSet":{RunePouch p=RunePouch.values()[ManagerSupport.n(a,"index",0)];(Boolean.TRUE.equals(a.get("decayed"))?p.decayed:p.pouch).setId(ManagerSupport.n(a,"id",1));return null;}
        case"take":return PickupHandler.take(Support.player(a),ManagerSupport.ground(a));
        case"canTake":return PickupHandler.canTake(Support.player(a),ManagerSupport.ground(a),ManagerSupport.n(a,"type",0));
        case"pickupHandle":return handler.handle(Support.player(a),ManagerSupport.ground(a),"ignored");
        case"pickupDestination":return Support.coord(handler.getDestination(null,null));
        case"pickupRegister":try{return handler.newInstance(null)==handler;}catch(Throwable e){throw new RuntimeException(e);}
        case"fail":throw new Support.Failure();
        default:throw new IllegalArgumentException((String)a.get("op"));
    }}
    static void snapshot(Map s){
        List<Object>players=new ArrayList<>();for(Player p:Support.players){Map v=new TreeMap();v.put("attributes",new TreeMap(p.pickupAttributes));v.put("inventory",p.inventoryId);v.put("ironman",p.ironman);v.put("owned",new TreeSet(p.owned));v.put("name",p.rawName());players.add(v);}s.put("pickupPlayers",players);
        List<Object>inv=new ArrayList<>();for(Inventory i:inventories)inv.add(Arrays.asList(i.space,i.add,new ArrayList<>(i.added)));s.put("pickupInventories",inv);
        List<Object>capes=new ArrayList<>();for(GodType g:GodType.values())capes.add(g.cape.getId());List<Object>pouches=new ArrayList<>();for(RunePouch p:RunePouch.values())pouches.add(Arrays.asList(p.pouch.getId(),p.decayed.getId()));s.put("pickupConfig",Arrays.asList(permitted,pluginResult,capes,pouches));
    }
}
