package oracle;
import java.io.*;
import java.lang.reflect.Field;
import java.util.*;
import org.json.simple.*;
import org.json.simple.parser.JSONParser;
import core.game.node.Node;
import core.game.node.entity.lock.ActionLocks;
import core.game.node.entity.lock.Lock;
import core.game.node.entity.lock.LockElapse;

public class Support {
    static Model current;
    static class Failure extends RuntimeException {Failure(String text){super(text);}}
    static Object field(Object owner,String name){try{Field f=owner.getClass().getDeclaredField(name);f.setAccessible(true);return f.get(owner);}catch(Exception e){throw new RuntimeException(e);}}
    static int number(Map row,String key,int fallback){return row.get(key)instanceof Number?((Number)row.get(key)).intValue():fallback;}
    static String string(Map row,String key,String fallback){Object v=row.get(key);return v==null?fallback:v.toString();}
    static String text(Object value){if(value==null)return null;List list=(List)value;char[]chars=new char[list.size()];for(int i=0;i<chars.length;i++)chars[i]=(char)((Number)list.get(i)).intValue();return new String(chars);}
    static Object chars(String s){if(s==null)return null;List<Integer>out=new ArrayList<>();for(char c:s.toCharArray())out.add((int)c);return out;}
    public static int ticks(){return current.clock();}
    static class Model {
        JSONObject input;ActionLocks actions=new ActionLocks();
        Map<String,Lock>refs=new TreeMap<>();List<Lock>locks=new ArrayList<>();
        Map<String,LockElapse>callbacks=new TreeMap<>();IdentityHashMap<LockElapse,String>events=new IdentityHashMap<>();
        List<Object>trace=new ArrayList<>();Set<Integer>usedHooks=new HashSet<>();int tick,calls;
        Model(JSONObject input){this.input=input;tick=number(input,"tick",0);for(String name:new String[]{"m","t","c","i"}){Lock lock=slot(name);refs.put(name,lock);id(lock);}}
        Lock slot(String kind){switch(kind){case"m":case"movement":return(Lock)field(actions,"movementLock");case"t":case"teleport":return(Lock)field(actions,"teleportLock");case"c":case"component":return(Lock)field(actions,"componentLock");case"i":case"interaction":return(Lock)field(actions,"interactionLock");case"equipment":return actions.getEquipmentLock();default:throw new RuntimeException("unknown slot "+kind);}}
        int id(Lock lock){for(int i=0;i<locks.size();i++)if(locks.get(i)==lock)return i;locks.add(lock);return locks.size()-1;}
        Object nullableId(Lock lock){return lock==null?null:id(lock);}
        Lock ref(Map row,String name,Lock passed){String key=string(row,name,"");return key.equals("passed")?passed:refs.get(key);}
        LockElapse event(Object value){if(value==null)return null;String name=value.toString();return callbacks.computeIfAbsent(name,key->{LockElapse fn=(node,lock)->{trace.add(Arrays.asList("elapse",key,node==null?null:node.id,id(lock)));Map all=(Map)input.get("callbacks");List body=all==null?null:(List)all.get(key);if(body!=null)for(Object a:body)act((Map)a,lock);};events.put(fn,key);return fn;});}
        int clock(){calls++;int call=calls,value=tick;List samples=(List)input.get("samples");if(samples!=null&&call<=samples.size())value=((Number)samples.get(call-1)).intValue();trace.add(Arrays.asList("clock",value));Map hooks=(Map)input.get("hooks");List body=hooks==null?null:(List)hooks.get(Integer.toString(call));if(body!=null&&usedHooks.add(call))for(Object action:body)act((Map)action,null);if(number(input,"failClockAt",-1)==call)throw new Failure("clock");return value;}
        Object act(Map row,Lock passed){
            String op=(String)row.get("op"),kind=string(row,"kind","all");
            switch(op){
                case"tick":tick=number(row,"value",0);return null;
                case"fail":throw new Failure("callback");
                case"new":{Lock lock=new Lock(text(row.get("message")));refs.put(row.get("name").toString(),lock);return id(lock);}
                case"get":{Lock lock=slot(kind);refs.put(row.get("name").toString(),lock);return nullableId(lock);}
                case"equipment":actions.setEquipmentLock(ref(row,"name",passed));return null;
                case"same":return ref(row,"name",passed)==ref(row,"other",passed);
                case"message":ref(row,"name",passed).setMessage(text(row.get("value")));return null;
                case"getMessage":return chars(ref(row,"name",passed).getMessage());
                case"elapse":return id(ref(row,"name",passed).setElapse(event(row.get("event"))));
                case"lockref":{Lock lock=ref(row,"name",passed);if(Boolean.TRUE.equals(row.get("forever")))lock.lock();else lock.lock(number(row,"ticks",0));return null;}
                case"unlockref":ref(row,"name",passed).unlock();return null;
                case"lockedref":return ref(row,"name",passed).isLocked();
                case"lock":if(kind.equals("all")){if(Boolean.TRUE.equals(row.get("forever")))actions.lock();else actions.lock(number(row,"ticks",0));}else if(Boolean.TRUE.equals(row.get("forever")))slot(kind).lock();else{int t=number(row,"ticks",0);switch(kind){case"movement":actions.lockMovement(t);break;case"teleport":actions.lockTeleport(t);break;case"component":actions.lockComponent(t);break;case"interaction":actions.lockInteractions(t);break;default:slot(kind).lock(t);}}return null;
                case"unlock":if(kind.equals("all"))actions.unlock();else{switch(kind){case"movement":actions.unlockMovement();break;case"teleport":actions.unlockTeleport();break;case"component":actions.unlockComponent();break;case"interaction":actions.unlockInteraction();break;default:slot(kind).unlock();}}return null;
                case"locked":if(kind.equals("all"))return Arrays.asList(actions.isMovementLocked(),actions.isInteractionLocked());switch(kind){case"movement":return actions.isMovementLocked();case"teleport":return actions.isTeleportLocked();case"component":return actions.isComponentLocked();case"interaction":return actions.isInteractionLocked();default:return slot(kind).isLocked();}
                case"custom":{Lock lock=row.containsKey("event")?actions.lock(text(row.get("key")),number(row,"ticks",0),event(row.get("event"))):actions.lock(text(row.get("key")),number(row,"ticks",0));if(row.get("name")!=null)refs.put(row.get("name").toString(),lock);return id(lock);}
                case"customLocked":return actions.isLocked(text(row.get("key")));
                case"remove":{String key=text(row.get("key"));if(row.containsKey("event")){Map<String,Lock>map=(Map<String,Lock>)field(actions,"customLocks");Lock lock=map.get(key);if(lock!=null)lock.setElapse(event(row.get("event")));}Node node=row.containsKey("node")&&row.get("node")==null?null:new Node(number(row,"node",1));if(Boolean.FALSE.equals(row.get("cache")))actions.unlock(key,false,node);else actions.unlock(key,node);return null;}
                default:throw new RuntimeException("unknown action "+op);
            }
        }
        Object snapshot(){
            Map<String,Lock>map=(Map<String,Lock>)field(actions,"customLocks");List<String>keys=new ArrayList<>(map.keySet());keys.sort(Comparator.nullsFirst(Comparator.naturalOrder()));
            Map<String,Object>aliases=new TreeMap<>();for(Map.Entry<String,Lock>e:refs.entrySet())aliases.put(e.getKey(),nullableId(e.getValue()));Object equipment=nullableId(actions.getEquipmentLock());
            List<Object>custom=new ArrayList<>();for(String key:keys)custom.add(Arrays.asList(chars(key),id(map.get(key))));
            List<Object>state=new ArrayList<>();for(int i=0;i<locks.size();i++){Lock lock=locks.get(i);Map<String,Object>value=new LinkedHashMap<>();value.put("expiration",field(lock,"expiration"));value.put("message",chars(lock.getMessage()));value.put("event",events.get(lock.getElapseEvent()));state.add(value);}
            Map<String,Object>out=new LinkedHashMap<>();out.put("refs",aliases);out.put("equipment",equipment);out.put("custom",custom);out.put("locks",state);out.put("tick",tick);out.put("calls",calls);return out;
        }
        Object evaluate(){List<Object>steps=new ArrayList<>();for(Object item:(List)input.get("actions")){Object value=null;String error=null;try{value=act((Map)item,null);}catch(NullPointerException e){error="Null";}catch(Failure e){error=e.getMessage();}Map<String,Object>step=new LinkedHashMap<>();step.put("result",value);step.put("error",error);step.put("state",snapshot());step.put("trace",new ArrayList<>(trace));steps.add(step);}return steps;}
    }
    public static void main(String[]args)throws Exception{BufferedReader reader=new BufferedReader(new InputStreamReader(System.in,"UTF-8"));JSONParser parser=new JSONParser();String line;while((line=reader.readLine())!=null){JSONObject row=(JSONObject)parser.parse(line);current=new Model(row);row.put("expected",current.evaluate());System.out.println(JSONValue.toJSONString(row));}}
}
