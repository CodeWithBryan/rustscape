package oracle;
import java.util.*;
import core.cache.def.impl.ItemDefinition;
import core.game.node.item.*;
import core.game.node.entity.player.Player;
import core.game.world.map.*;

public class ManagerSupport {
    static TreeMap<Integer,ItemDefinition>defs;
    static int lookup;
    static List<Item>inputs;
    static List<Item[]>arrays;
    static List<List<GroundItem>>captures;
    static Map<GroundItem,Map<String,Boolean>>virtuals;
    static Set<Integer>details;
    public static ItemDefinition definition(int id){return defs.computeIfAbsent(id,ItemDefinition::new);}
    public static int id(GroundItem g){return Support.gid(g);}
    public static RegionChunk chunk(){return Support.chunks[lookup];}
    static void reset(){GroundItemManager.getItems().clear();defs=new TreeMap<>();lookup=0;inputs=new ArrayList<>();arrays=new ArrayList<>();captures=new ArrayList<>();virtuals=new IdentityHashMap<>();details=new HashSet<>();}
    static void rememberFlags(){for(RegionChunk c:Support.chunks)for(var f:c.rawFlags()){boolean found=f==null;for(var old:Support.flags)if(old==f){found=true;break;}if(!found)Support.flags.add(f);}}
    static int n(Map a,String k,int d){return Support.num(Support.get(a,k,d));}
    static Item input(Map a){if(a.containsKey("input"))return a.get("input")==null?null:inputs.get(n(a,"input",0));return new Item(n(a,"id",1),n(a,"amount",1),n(a,"charge",1000));}
    static GroundItem ground(Map a){return a.get("g")==null?null:Support.ground(a);}
    static Location location(Map a){return a.containsKey("locationOf")?Support.grounds.get(n(a,"locationOf",0)).getLocation():Support.loc(a.get("location"));}
    static List<GroundItem> list(Map a){switch((String)a.get("list")){case"manager":return GroundItemManager.getItems();case"chunk":return Support.chunk(a).getItems();case"ai":return Support.player(a).aiItems;default:return captures.get(n(a,"index",0));}}
    static ArrayList<GroundItem> ids(Object raw){if(raw==null)return null;var out=new ArrayList<GroundItem>();for(Object v:(List)raw)out.add(v==null?null:Support.grounds.get(Support.num(v)));return out;}
    static Object act(Map a){switch((String)a.get("op")){
        case"managerCreate":{String ctor=(String)Support.get(a,"ctor","ground");GroundItem g;if(ctor.equals("ground"))g=GroundItemManager.create(ground(a));else{Item item=input(a);Location l=location(a);Player p=Support.player(a);switch(ctor){case"player":g=GroundItemManager.create(item,p);break;case"uid":g=GroundItemManager.create(item,l,n(a,"uid",7),n(a,"absolute",200));break;case"location":g=GroundItemManager.create(item,l);break;default:g=GroundItemManager.create(item,l,p);}}return id(g);}
        case"managerDestroy":{GroundItem g=GroundItemManager.destroy(ground(a));return g==null?null:id(g);}
        case"managerGet":{GroundItem g=GroundItemManager.get(n(a,"id",1),location(a),Support.player(a));return g==null?null:id(g);}
        case"managerIncrease":return id(GroundItemManager.increase(input(a),location(a),Support.player(a)));
        case"managerPulse":GroundItemManager.pulse();return null;
        case"chunkAdd":Support.chunk(a).add(ground(a));return null;
        case"chunkRemove":Support.chunk(a).remove(ground(a));return null;
        case"lookup":lookup=n(a,"value",0);return null;
        case"definition":{int id=n(a,"id",1);if(Boolean.TRUE.equals(a.get("replace"))){ItemDefinition d=new ItemDefinition(id);d.tradeable=false;defs.put(id,d);}ItemDefinition d=definition(id);if(a.containsKey("tradeable"))d.tradeable=a.get("tradeable");if(a.containsKey("plugin"))d.plugin=a.get("plugin")==null?null:new ItemPlugin(n(a,"plugin",0));if(a.containsKey("name")){d.name=(String)a.get("name");details.add(id);}if(a.containsKey("options")){List v=(List)a.get("options");d.options=v==null?null:(String[])v.toArray(new String[0]);details.add(id);}return null;}
        case"itemDefinition":Support.ground(a).definition=a.get("id")==null?null:definition(n(a,"id",1));return null;
        case"setItem":if(a.containsKey("amount"))Support.ground(a).setAmount(n(a,"amount",1));return null;
        case"inputSet":inputs.get(n(a,"input",0)).setAmount(n(a,"amount",1));return null;
        case"moveLocation":{try{var field=Location.class.getDeclaredField("hash");field.setAccessible(true);List v=(List)a.get("value");int x=Support.num(v.get(0)),y=Support.num(v.get(1)),z=Support.num(v.get(2));field.setInt(Support.ground(a).getLocation(),((z&3)<<30)|((x&32767)<<15)|(y&32767));}catch(ReflectiveOperationException ex){throw new RuntimeException(ex);}return null;}
        case"dropper":Support.ground(a).setDropper(Support.player(a));return null;
        case"virtual":{var v=virtuals.computeIfAbsent(Support.ground(a),k->new HashMap<>());for(String key:new String[]{"private","active","auto"})if(a.containsKey(key))v.put(key,(Boolean)a.get(key));return null;}
        case"playerState":{Player p=Support.player(a);if(a.containsKey("location"))p.location=Support.loc(a.get("location"));if(a.containsKey("playing"))p.playing=(Boolean)a.get("playing");if(a.containsKey("artificial"))p.artificial=(Boolean)a.get("artificial");return null;}
        case"setAI":Support.player(a).aiItems=ids(a.get("items"));return null;
        case"captureList":captures.add(list(a));return captures.size()-1;
        case"mutateList":{var list=list(a);switch((String)a.get("kind")){case"clear":list.clear();break;case"add":list.add(ground(a));break;case"remove":return list.remove(ground(a));case"set":list.set(n(a,"slot",0),ground(a));break;}return null;}
        case"input":inputs.add(input(a));return inputs.size()-1;
        case"array":{var values=(List)a.get("items");Item[]out=new Item[values.size()];for(int i=0;i<out.length;i++)out[i]=values.get(i)==null?null:inputs.get(Support.num(values.get(i)));arrays.add(out);return arrays.size()-1;}
        case"arraySet":arrays.get(n(a,"array",0))[n(a,"slot",0)]=a.get("input")==null?null:inputs.get(n(a,"input",0));return null;
        case"managerMany":GroundItemManager.create(a.get("array")==null?null:arrays.get(n(a,"array",0)),Support.loc(a.get("location")),Support.player(a));return null;
        default:throw new IllegalArgumentException((String)a.get("op"));
    }}
    static Object values(List<GroundItem> list){if(list==null)return null;List<Object>v=new ArrayList<>();for(GroundItem g:list)v.add(g==null?null:id(g));return v;}
    static void snapshot(Map state){state.put("manager",values(GroundItemManager.getItems()));List<Object>players=new ArrayList<>();for(Player p:Support.players){Map v=new LinkedHashMap();v.put("location",Support.coord(p.location));v.put("playing",p.playing);v.put("artificial",p.artificial);v.put("ai",values(p.aiItems));players.add(v);}state.put("players",players);List<Object>held=new ArrayList<>();for(var list:captures)held.add(values(list));state.put("captures",held);List<Object>definitions=new ArrayList<>();for(var d:defs.values()){Map v=new LinkedHashMap();v.put("id",d.id);v.put("tradeable",d.tradeable);v.put("plugin",d.plugin==null?null:d.plugin.id);if(details.contains(d.id)){v.put("name",d.name);v.put("options",d.options==null?null:Arrays.asList(d.options));}definitions.add(v);}state.put("definitions",definitions);List<Object>in=new ArrayList<>();for(Item i:inputs)in.add(Arrays.asList(i.getId(),i.getAmount(),i.getCharge()));state.put("inputs",in);List<Object>as=new ArrayList<>();for(Item[]array:arrays){List<Object>out=new ArrayList<>();for(Item i:array)out.add(i==null?null:inputs.indexOf(i));as.add(out);}state.put("arrays",as);}
    static Boolean virtual(GroundItem g,String key){var v=virtuals.get(g);return v==null?null:v.get(key);}
}
class VirtualGround extends GroundItem {
    VirtualGround(Item i){super(i);}VirtualGround(Item i,Location l){super(i,l);}VirtualGround(Item i,Location l,Player p){super(i,l,p);}VirtualGround(Item i,Location l,int d,Player p){super(i,l,d,p);}VirtualGround(Item i,Location l,int uid,int ticks){super(i,l,uid,ticks);}
    public boolean isPrivate(){Boolean value=ManagerSupport.virtual(this,"private");if(value==null)return super.isPrivate();Support.emit("private",ManagerSupport.id(this));return value;}
    public boolean isActive(){Boolean value=ManagerSupport.virtual(this,"active");if(value==null)return super.isActive();Support.emit("active",ManagerSupport.id(this));return value;}
    public boolean isAutoSpawn(){Boolean value=ManagerSupport.virtual(this,"auto");if(value==null)return super.isAutoSpawn();Support.emit("auto",ManagerSupport.id(this));return value;}
    public void respawn(){Support.emit("respawn",ManagerSupport.id(this));}
}
