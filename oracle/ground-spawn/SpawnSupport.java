package oracle;
import java.util.*;
import core.game.system.config.GroundSpawnLoader.GroundSpawn;
import core.game.system.task.Pulse;
import core.game.world.PulseRunner;

public class SpawnSupport {
    static int max,count,current;
    static List<GroundSpawn>spawns;
    static List<PulseRunner>runners;
    static List<Pulse>pulses;
    static void reset(){max=2000;count=0;current=0;spawns=new ArrayList<>();pulses=new ArrayList<>();runners=new ArrayList<>();runners.add(new PulseRunner());runners.add(new PulseRunner());}
    public static int maximum(){int v=max;Support.emit("maximum",v);return v;}
    public static int population(){int v=count;Support.emit("population",v);return v;}
    public static PulseRunner pulser(){int id=current;PulseRunner r=runners.get(id);Support.emit("pulser",id);return r;}
    static void remember(){if(runners==null)return;for(var r:runners)for(Pulse p:r.getCurrentPulses())if(!pulses.contains(p))pulses.add(p);}
    static GroundSpawn spawn(Map a){return spawns.get(ManagerSupport.n(a,"spawn",0));}
    static Object act(Map a){switch((String)a.get("op")){
        case"spawnNew":{GroundSpawn s=new GroundSpawn(ManagerSupport.n(a,"rate",3),ManagerSupport.input(a),ManagerSupport.location(a));spawns.add(s);return Support.gid(s);}
        case"spawnInit":return Support.gid(spawn(a).init());
        case"spawnRate":spawn(a).setRespawnRate(ManagerSupport.n(a,"minimum",1),ManagerSupport.n(a,"maximum",2));return null;
        case"spawnDuration":return spawn(a).getRespawnDuration();
        case"spawnRespawn":spawn(a).respawn();remember();return null;
        case"spawnConfig":if(a.containsKey("maximum"))max=ManagerSupport.n(a,"maximum",2000);if(a.containsKey("population"))count=ManagerSupport.n(a,"population",0);return null;
        case"spawnRunner":current=ManagerSupport.n(a,"index",0);return null;
        case"spawnWorld":runners.get(ManagerSupport.n(a,"index",0)).updateAll();return null;
        case"spawnPulse":return pulses.get(ManagerSupport.n(a,"pulse",0)).pulse();
        default:throw new IllegalArgumentException((String)a.get("op"));
    }}
    static void snapshot(Map state){remember();List<Object>s=new ArrayList<>();for(var spawn:spawns)s.add(Arrays.asList(Support.gid(spawn),spawn.getRespawnRate()));state.put("spawns",s);List<Object>ps=new ArrayList<>();try{var ticks=Pulse.class.getDeclaredField("ticksPassed");ticks.setAccessible(true);var checks=Pulse.class.getDeclaredField("checks");checks.setAccessible(true);for(Pulse p:pulses)ps.add(Arrays.asList(p.getDelay(),ticks.getInt(p),p.isRunning(),checks.get(p)==null,p.getClass().getName()));}catch(ReflectiveOperationException e){throw new RuntimeException(e);}state.put("spawnPulses",ps);List<Object>qs=new ArrayList<>();for(var r:runners){List<Object>q=new ArrayList<>();for(Pulse p:r.getCurrentPulses())q.add(pulses.indexOf(p));qs.add(q);}state.put("spawnQueues",qs);state.put("population",Arrays.asList(max,count,current));}
}
