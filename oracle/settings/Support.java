package oracle;
import java.io.*;
import java.lang.reflect.*;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.entity.player.link.Settings;
import core.game.node.entity.player.Player;

/** Only owning-player/container/packet services are adapted. Settings is unchanged. */
public final class Support {
    public static Settings settings;
    public static JSONObject config;
    public static JSONArray trace;
    public static final Map<Integer,Container> containers=new HashMap<>();
    static Map<String,Integer> counts;
    static Set<Integer> fired;
    public static boolean retaliating;
    public static final Properties properties=new Properties();
    public static final Ironman ironman=new Ironman();
    public static final Dispatcher dispatcher=new Dispatcher();
    public static final Session session=new Session();
    public static int integer(Object v){return ((Number)v).intValue();}
    public static int number(String key,int fallback){Object v=config.get(key);return v==null?fallback:integer(v);}
    public static boolean flag(String key){return Boolean.TRUE.equals(config.get(key));}
    public static String bits(double x){return String.format("%016x",Double.doubleToLongBits(x));}
    public static double decimal(Object x){return Double.parseDouble(x.toString());}
    public static final class Failure extends RuntimeException {}
    public static void emit(String event,Object...args){
        JSONArray row=new JSONArray();row.add(event);Collections.addAll(row,args);trace.add(row);
        int n=counts.getOrDefault(event,0)+1;counts.put(event,n);
        JSONArray hooks=(JSONArray)config.getOrDefault("hooks",new JSONArray());
        for(int i=0;i<hooks.size();i++){
            JSONObject h=(JSONObject)hooks.get(i);
            if(!fired.contains(i)&&event.equals(h.get("event"))&&integer(h.getOrDefault("nth",1L))==n){
                fired.add(i);
                for(Object a:(JSONArray)h.get("actions"))act((JSONObject)a);
            }
        }
        JSONObject failure=(JSONObject)config.get("failure");
        if(failure!=null&&event.equals(failure.get("event"))&&integer(failure.getOrDefault("nth",1L))==n)throw new Failure();
    }
    public static final class Properties {
        public boolean isRetaliating(){emit("retaliating");return retaliating;}
        public void setRetaliating(boolean value){emit("setRetaliating",value);retaliating=value;}
    }
    public static final class Ironman {
        public boolean isIronman(){emit("ironman");return flag("ironman");}
    }
    public static final class Session {
        public void write(core.net.packet.IoBuffer b){emit("send",b.opcode,b.bytes);}
    }
    public static final class Dispatcher {
        public void sendRunEnergy(){emit("energy",bits(settings.getRunEnergy()));}
        public void sendMessage(String text){emit("message",text);}
        public void sendString(String text,int id,int child){emit("string",text,id,child);}
    }
    public static final class Container {
        public final int id;public Object[]items;
        Container(int id,JSONArray slots){this.id=id;set(slots);}
        void set(JSONArray slots){items=new Object[slots.size()];for(int i=0;i<items.length;i++)items[i]=slots.get(i)==null?null:new core.game.node.item.Item(integer(slots.get(i)));}
        public core.game.node.item.Item get(int slot){emit("item",id,slot);return(core.game.node.item.Item)items[slot];}
    }
    public static Container container(String kind){emit(kind);return containers.get(number(kind+"Id",kind.equals("inventory")?0:1));}
    public static final class Definition {
        final int id;public Definition(int id){this.id=id;}
        @SuppressWarnings("unchecked") public <T>T getConfiguration(String key,T fallback){
            emit("weight",id);
            JSONObject weights=(JSONObject)config.getOrDefault("weights",new JSONObject());
            if(!weights.containsKey(Integer.toString(id)))return fallback;
            Object raw=weights.get(Integer.toString(id));
            if(raw==null)return null;
            JSONObject row=(JSONObject)raw;String kind=(String)row.getOrDefault("kind","double");Object v=row.get("value");
            if(kind.equals("double"))return(T)Double.valueOf(decimal(v));
            if(kind.equals("long"))return(T)Long.valueOf(v.toString());
            return(T)v;
        }
    }
    static void fields(JSONObject values){
        for(Object key:values.keySet())try{
            Field f=Settings.class.getDeclaredField((String)key);f.setAccessible(true);Object v=values.get(key);
            if(f.getType()==double.class)f.setDouble(settings,decimal(v));
            else if(f.getType()==int.class)f.setInt(settings,integer(v));
            else f.setBoolean(settings,(boolean)v);
        }catch(ReflectiveOperationException e){throw new IllegalArgumentException(e);}
    }
    public static Object act(JSONObject a){
        String op=(String)a.get("op");Object value=a.get("value");int v=value instanceof Number?integer(value):0;
        switch(op){
            case"update":settings.update();break;
            case"chat":settings.updateChatSettings();break;
            case"chatValues":settings.updateChatSettings(integer(a.get("public")),integer(a.get("private")),integer(a.get("trade")));break;
            case"setChat":settings.setChatSettings(integer(a.get("public")),integer(a.get("private")),integer(a.get("trade")));break;
            case"parse":settings.parse((JSONObject)a.get("data"));break;
            case"brightness":settings.setBrightness(v);break;
            case"music":settings.setMusicVolume(v);break;
            case"sound":settings.setSoundEffectVolume(v);break;
            case"area":settings.setAreaSoundVolume(v);break;
            case"public":settings.setPublicChatSetting(v);break;
            case"private":settings.setPrivateChatSetting(v);break;
            case"trade":settings.setTradeSetting(v);break;
            case"clan":settings.setClanChatSetting(v);break;
            case"assist":settings.setAssistSetting(v);break;
            case"attackToggle":settings.toggleAttackStyleIndex(v);break;
            case"attackSet":settings.setAttackStyleIndex(v);break;
            case"specialToggle":settings.toggleSpecialBar();break;
            case"specialSet":settings.setSpecialToggled((boolean)value);break;
            case"specialDrain":return settings.drainSpecial(v);
            case"specialEnergy":settings.setSpecialEnergy(v);break;
            case"retaliate":settings.toggleRetaliating();break;
            case"mouse":settings.toggleMouseButton();break;
            case"effects":settings.toggleChatEffects();break;
            case"split":settings.toggleSplitPrivateChat();break;
            case"aid":settings.toggleAcceptAid();break;
            case"acceptAid":return settings.isAcceptAid();
            case"runToggle":settings.toggleRun();break;
            case"runSet":settings.setRunToggled((boolean)value);break;
            case"energy":settings.updateRunEnergy(decimal(value));break;
            case"energySet":settings.setRunEnergy(decimal(value));break;
            case"weight":settings.updateWeight();break;
            case"state":fields((JSONObject)a.get("fields"));break;
            case"configure":config.putAll((JSONObject)a.get("fields"));break;
            case"retaliation":retaliating=(boolean)value;break;
            case"slots":{int id=integer(a.get("id"));JSONArray slots=(JSONArray)a.get("slots");if(containers.containsKey(id))containers.get(id).set(slots);else containers.put(id,new Container(id,slots));break;}
            default:throw new IllegalArgumentException("Unknown action "+op);
        }
        return null;
    }
    static JSONObject snapshot(){
        JSONObject state=new JSONObject();
        for(Field f:Settings.class.getDeclaredFields())if(!f.getName().equals("player"))try{
            f.setAccessible(true);Object value=f.get(settings);state.put(f.getName(),value instanceof Double?bits((Double)value):value);
        }catch(ReflectiveOperationException e){throw new IllegalStateException(e);}
        JSONObject result=new JSONObject();result.put("fields",state);result.put("retaliating",retaliating);result.put("trace",new JSONArray());((JSONArray)result.get("trace")).addAll(trace);return result;
    }
    static String error(Throwable e){
        if(e instanceof NullPointerException)return"Null";
        if(e instanceof ClassCastException)return"Type";
        if(e instanceof NumberFormatException)return"Number";
        if(e instanceof IndexOutOfBoundsException)return"Index";
        if(e instanceof Failure)return"Binding";
        throw new IllegalStateException(e);
    }
    static JSONObject run(JSONObject request){
        config=new JSONObject();config.putAll((JSONObject)request.getOrDefault("config",new JSONObject()));trace=new JSONArray();counts=new HashMap<>();fired=new HashSet<>();retaliating=flag("retaliating");containers.clear();
        for(int id=0;id<2;id++){JSONArray a=new JSONArray();for(int i=0;i<(id==0?28:14);i++)a.add(null);containers.put(id,new Container(id,a));}
        settings=new Settings(new Player());JSONArray results=new JSONArray();
        for(Object input:(JSONArray)request.get("actions")){
            JSONObject row=new JSONObject();try{row.put("value",act((JSONObject)input));}catch(Throwable e){row.put("error",error(e));}
            row.put("state",snapshot());results.add(row);
        }
        JSONObject result=new JSONObject();result.put("request",request);result.put("results",results);return result;
    }
    public static void main(String[]args)throws Exception{
        BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));String line;
        while((line=reader.readLine())!=null)System.out.println(run((JSONObject)new JSONParser().parse(line)).toJSONString());
    }
}
