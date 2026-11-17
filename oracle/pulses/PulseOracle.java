package oracle;
import core.game.system.task.Pulse;
import core.game.world.PulseRunner;
import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.node.entity.impl.*;
import core.ServerConstants;
import java.util.*;
import java.io.*;
import java.lang.reflect.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;

public final class PulseOracle {
    public static JSONObject input;
    public static Player player;
    public static PulseRunner runner;
    public static TreeMap<String,Pulse> pulses=new TreeMap<>();
    public static IdentityHashMap<Pulse,String> names=new IdentityHashMap<>();
    public static IdentityHashMap<Pulse,JSONObject> configs=new IdentityHashMap<>();
    static IdentityHashMap<Node[],Integer> arrayIds=new IdentityHashMap<>();
    static TreeMap<String,Node[]> arrays=new TreeMap<>();
    public static TreeMap<Integer,Node> nodes=new TreeMap<>();
    public static ArrayList<Object> events=new ArrayList<>();
    static HashMap<String,Integer> calls=new HashMap<>();
    public static int[] hashes;
    public static long now,step;
    public static int clockIndex,botTotal,otherTotal,unhandled;
    public static boolean logging,db,dead;
    public static HashMap<String,Integer> lengths=new HashMap<>(),counts=new HashMap<>();
    public static class Failure extends RuntimeException {}
    public static int number(JSONObject o,String k,int d){Object n=o.get(k);return n instanceof Number?((Number)n).intValue():d;}
    public static boolean flag(JSONObject o,String k,boolean d){Object v=o.get(k);return v==null?d:(Boolean)v;}
    public static String name(Pulse p){if(p==null)return null;String name=names.get(p);if(name==null){name="u"+unhandled++;names.put(p,name);pulses.put(name,p);}return name;}
    public static int enumHash(int index){return hashes[index];}
    public static void hook(String who,String event,Object... args){
        ArrayList<Object> trace=new ArrayList<>();trace.add(who);trace.add(event);trace.addAll(Arrays.asList(args));events.add(trace);
        String key=who+":"+event;int count=calls.getOrDefault(key,0)+1;calls.put(key,count);
        for(Object raw:(JSONArray)input.getOrDefault("scripts",new JSONArray())){
            JSONObject rule=(JSONObject)raw;
            if(who.equals(rule.get("who"))&&event.equals(rule.get("event"))&&number(rule,"at",1)==count)
                for(Object a:(JSONArray)rule.get("actions"))action((JSONObject)a);
        }
    }
    public static JSONObject config(Pulse p){return configs.getOrDefault(p,new JSONObject());}
    public static void pulseHook(Pulse p,String event,Object... args){hook(name(p),event,args);}
    public static boolean nodeActive(Node node){hook("node:"+node.id,"active");return node.active;}
    public static long clock(){JSONArray times=(JSONArray)input.getOrDefault("times",new JSONArray());long value=clockIndex<times.size()?((Number)times.get(clockIndex)).longValue():now;clockIndex++;now+=step;hook("clock","sample",value);return value;}
    public static void log(int level,String message){hook("log",level==1?"Warn":"Error",message);}
    public static void length(String n,int value){hook("metrics","length",n,value);if(db)lengths.put(n,lengths.getOrDefault(n,0)+value);}
    public static void count(String n){hook("metrics","count",n);if(db)counts.put(n,counts.getOrDefault(n,0)+1);}
    public static void botTotal(int value){hook("metrics","bots",value);botTotal=value;}
    public static void otherTotal(int value){hook("metrics","others",value);otherTotal=value;}
    static String error(Throwable e){if(Boolean.getBoolean("rs09.oracle.debug"))e.printStackTrace(new PrintStream(new FileOutputStream(FileDescriptor.err)));if(e instanceof NullPointerException)return "Null";if(e instanceof IndexOutOfBoundsException)return "Index";if(e instanceof ConcurrentModificationException)return "ConcurrentModification";if(e instanceof Failure)return "Binding";throw new RuntimeException(e);}
    static Object field(Object object,Class<?> type,String key){try{Field f=type.getDeclaredField(key);f.setAccessible(true);return f.get(object);}catch(Exception e){throw new RuntimeException(e);}}
    @SuppressWarnings("unchecked")
    static Map<PulseType,Pulse> slots(){return(Map<PulseType,Pulse>)field(player.getPulseManager(),PulseManager.class,"currentPulses");}
    static PulseType kind(Object value){if(value==null)return null;return PulseType.valueOf(value.toString());}
    static Node[] nodeArray(Object value){if(value==null)return null;JSONArray a=(JSONArray)value;Node[] result=new Node[a.size()];for(int i=0;i<a.size();i++)result[i]=a.get(i)==null?null:nodes.get(((Number)a.get(i)).intValue());return result;}
    public static Object action(JSONObject a){
        Pulse p=a.get("pulse")==null?null:pulses.get((String)a.get("pulse"));int value=number(a,"value",0);PulseManager manager=player.getPulseManager();
        switch((String)a.get("op")){
        case "update":return p.update();
        case "body":return p.pulse();
        case "runBase":p.run();break;
        case "start":p.start();break;
        case "stop":p.stop();break;
        case "restart":p.restart();break;
        case "running":p.running=(Boolean)a.get("value");break;
        case "delay":p.setDelay(value);break;
        case "ticks":p.setTicksPassed(value);break;
        case "inactive":return p.hasInactiveNode();
        case "isRunning":return p.isRunning();
        case "removeFor":return p.removeFor((String)a.get("text"));
        case "setNode":p.addNodeCheck(number(a,"index",0),a.get("node")==null?null:nodes.get(number(a,"node",0)));break;
        case "getNode":{Node n=p.getNodeCheck(number(a,"index",0));return n==null?null:n.id;}
        case "replaceChecks":{Node[] checks=a.containsKey("array")?arrays.get((String)a.get("array")):nodeArray(a.get("value"));try{Field f=Pulse.class.getDeclaredField("checks");f.setAccessible(true);f.set(p,checks);}catch(Exception e){throw new RuntimeException(e);}break;}
        case "node":nodes.get(number(a,"node",0)).active=(Boolean)a.get("value");break;
        case "managerRun":manager.run(p,kind(a.getOrDefault("kind","STANDARD")));break;
        case "clear":manager.clear();break;
        case "clearKind":return manager.clear(kind(a.get("kind")));
        case "current":return name(manager.getCurrent());
        case "has":return manager.hasPulseRunning();
        case "moving":return manager.isMovingPulse();
        case "dead":dead=(Boolean)a.get("value");break;
        case "cancelDeath":PulseManager.cancelDeathTask(player);break;
        case "unhandled":return name(manager.runUnhandledAction(player,kind(a.getOrDefault("kind","STANDARD"))));
        case "mapPut":slots().put(kind(a.get("kind")),p);break;
        case "submit":runner.submit(p);break;
        case "world":runner.updateAll();break;
        case "logging":ServerConstants.GRAFANA_LOGGING=(Boolean)a.get("value");break;
        case "db":db=(Boolean)a.get("value");break;
        case "clock":now=((Number)a.get("value")).longValue();break;
        case "step":step=((Number)a.get("value")).longValue();break;
        case "fail":throw new Failure();
        default:throw new IllegalArgumentException("Unknown action "+a);
        }return null;
    }
    static Object snapshot(Object value,String error){
        TreeMap<String,Object> states=new TreeMap<>();
        for(Map.Entry<String,Pulse> entry:new ArrayList<>(pulses.entrySet())){
            Pulse p=entry.getValue();if(p==null){states.put(entry.getKey(),null);continue;}
            Node[] checks=(Node[])field(p,Pulse.class,"checks");ArrayList<Object> values=null;Integer ref=null;
            if(checks!=null){ref=arrayIds.computeIfAbsent(checks,k->arrayIds.size());values=new ArrayList<>();for(Node n:checks)values.add(n==null?null:n.id);}
            states.put(entry.getKey(),Arrays.asList(p.getClass().getName(),p.running,p.getDelay(),p.getTicksPassed(),ref,values));
        }
        ArrayList<Object> current=new ArrayList<>();for(Map.Entry<PulseType,Pulse> e:slots().entrySet())current.add(Arrays.asList(e.getKey()==null?null:e.getKey().toString(),name(e.getValue())));
        ArrayList<Object> queued=new ArrayList<>();for(Pulse p:runner.getCurrentPulses())queued.add(name(p));
        Map<String,Object> out=new TreeMap<>();out.put("result",value);out.put("error",error);out.put("pulses",states);out.put("map",current);out.put("current",name(player.getPulseManager().getCurrent()));out.put("queue",queued);out.put("events",new ArrayList<>(events));out.put("clock",now);out.put("clockIndex",clockIndex);out.put("totals",Arrays.asList(botTotal,otherTotal));out.put("lengths",new TreeMap<>(lengths));out.put("counts",new TreeMap<>(counts));out.put("logging",ServerConstants.GRAFANA_LOGGING);out.put("db",db);return out;
    }
    static Object evaluate(JSONObject request){
        input=request;events.clear();calls.clear();pulses.clear();names.clear();configs.clear();arrayIds.clear();arrays.clear();nodes.clear();lengths.clear();counts.clear();unhandled=0;clockIndex=0;
        now=((Number)request.getOrDefault("clock",0L)).longValue();step=((Number)request.getOrDefault("step",0L)).longValue();botTotal=number(request,"botTotal",17);otherTotal=number(request,"otherTotal",19);db=flag(request,"db",true);dead=false;ServerConstants.GRAFANA_LOGGING=flag(request,"logging",true);
        JSONArray rawHashes=(JSONArray)request.get("hashes");hashes=new int[4];for(int i=0;i<4;i++)hashes[i]=((Number)rawHashes.get(i)).intValue();
        player=new Player();nodes.put(0,player);nodes.put(1,new Node(1,true));nodes.put(2,new Node(2,false));runner=new PulseRunner();
        JSONObject rawArrays=(JSONObject)request.getOrDefault("arrays",new JSONObject());for(Map.Entry<String,Object> e:new TreeMap<String,Object>(rawArrays).entrySet()){Node[] a=nodeArray(e.getValue());arrays.put(e.getKey(),a);if(a!=null)arrayIds.put(a,arrayIds.size());}
        JSONObject raw=(JSONObject)request.get("pulses");for(Map.Entry<String,Object> e:new TreeMap<String,Object>(raw).entrySet()){
            if(e.getValue()==null){pulses.put(e.getKey(),null);continue;}JSONObject d=(JSONObject)e.getValue();int delay=number(d,"delay",1);Node[] checks=d.containsKey("array")?arrays.get((String)d.get("array")):nodeArray(d.get("checks"));
            Pulse p;switch((String)d.getOrDefault("class","base")){
            case "movement":p=new core.game.interaction.MovementPulse(delay,checks);break;
            case "combat":p=new core.game.node.entity.combat.CombatPulse(delay,checks);break;
            case "script":p=new ScriptAPIPulse(delay,checks);break;
            case "bot":p=new core.game.bots.GeneralBotCreator.BotScriptPulse(delay,checks,d.get("scriptNull")==Boolean.TRUE?null:new core.game.bots.GeneralBotCreator.Script());break;
            default:p=d.get("ctor")==null?new TestPulse(delay,checks):d.get("ctor").equals("default")?new TestPulse():new TestPulse(delay);
            }
            p.running=flag(d,"running",true);p.setTicksPassed(number(d,"ticks",0));pulses.put(e.getKey(),p);names.put(p,e.getKey());configs.put(p,d);
        }
        ArrayList<Object> result=new ArrayList<>();for(Object action:(JSONArray)request.get("actions")){events.clear();Object value=null;String error=null;try{value=action((JSONObject)action);}catch(Throwable e){error=error(e);}result.add(snapshot(value,error));}return result;
    }
    public static void main(String[]args)throws Exception{
        PrintStream stderr=System.err;System.setErr(new PrintStream(new ByteArrayOutputStream()){@Override public void println(Object x){if(x instanceof Throwable)events.add(Arrays.asList("exception",error((Throwable)x)));}});
        try{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,java.nio.charset.StandardCharsets.UTF_8));String line;while((line=reader.readLine())!=null){JSONObject request=(JSONObject)new JSONParser().parse(line);request.put("expected",evaluate(request));System.out.println(request.toJSONString());}}
        catch(Throwable error){System.setErr(stderr);throw error;}
    }
}
