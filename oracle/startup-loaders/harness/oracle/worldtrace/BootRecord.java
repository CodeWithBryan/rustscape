package oracle.worldtrace;

import core.cache.def.impl.ItemDefinition;
import core.cache.def.impl.NPCDefinition;
import core.cache.def.impl.SceneryDefinition;
import core.game.node.entity.player.info.login.LoginConfiguration;
import core.game.system.SystemManager;
import core.game.system.task.Pulse;
import core.game.system.timer.TimerRegistry;
import core.game.world.GameWorld;
import core.plugin.ClassScanner;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.TreeSet;
import java.util.stream.Collectors;

/**
 * The part of WorldTrace.boot() that follows its GameWorld.prompt(false) call, copied statement for statement
 * (WorldTrace.boot cannot be reused: it calls prompt itself). The startup-loaders generator only accepts a run
 * whose resulting boot record is byte-identical to the world-trace fixture's, which also checks this copy.
 */
final class BootRecord {
    static Map<String, Object> after(long before) throws Throwable {
        Map<String, Object> out = new LinkedHashMap<>();
        Map<String, Object> listeners = new LinkedHashMap<>();
        listeners.put("login", WorldTrace.names(GameWorld.getLoginListeners()));
        listeners.put("loginPlugins", WorldTrace.names(LoginConfiguration.getLoginPlugins()));
        listeners.put("logout", WorldTrace.names(GameWorld.getLogoutListeners()));
        listeners.put("persistPlayer", WorldTrace.names((Collection<?>) WorldTrace.staticField(Class.forName("core.game.node.entity.player.info.login.PlayerSaver"), "contentHooks")));
        listeners.put("persistWorld", WorldTrace.names(GameWorld.getWorldPersists()));
        listeners.put("shutdown", WorldTrace.names(GameWorld.getShutdownListeners()));
        listeners.put("startup", WorldTrace.names(GameWorld.getStartupListeners()));
        listeners.put("tick", WorldTrace.names(GameWorld.getTickListeners()));
        listeners.put("timers", WorldTrace.names(TimerRegistry.INSTANCE.getAutoTimers()));
        out.put("listeners", listeners);
        Map<String, Object> scan = new LinkedHashMap<>();
        scan.put("contentInterfaces", new ArrayList<Object>(ClassScanner.scanResults.getClassesImplementing("core.api.ContentInterface").getNames()));
        scan.put("initializable", new ArrayList<Object>(ClassScanner.scanResults.getClassesWithAnnotation("core.plugin.Initializable").getNames()));
        scan.put("timers", new ArrayList<Object>(ClassScanner.scanResults.getSubclasses("core.game.system.timer.RSTimer").getNames()));
        out.put("scan", scan);
        Map<String, Object> tables = new LinkedHashMap<>();
        tables.put("contentInterfaces", ClassScanner.INSTANCE.getAmountLoaded());
        tables.put("itemDefinitions", ItemDefinition.getDefinitions().size());
        tables.put("legacyPlugins", ClassScanner.INSTANCE.getNumPlugins());
        tables.put("npcDefinitions", NPCDefinition.getDefinitions().size());
        tables.put("sceneryDefinitions", SceneryDefinition.getDefinitions().size());
        tables.put("timerTypes", new ArrayList<Object>(new TreeSet<>(TimerRegistry.INSTANCE.getTimerMap().keySet())));
        List<Object> pulses = new ArrayList<>();
        for (Pulse pulse : GameWorld.getPulser().getCurrentPulses()) pulses.add(pulse.getClass().getName());
        tables.put("pulseClasses", pulses);
        out.put("tables", tables);
        out.put("systemState", String.valueOf(SystemManager.state()));
        List<Object> threads = new ArrayList<>();
        for (Thread thread : new TreeMap<>(Thread.getAllStackTraces().keySet().stream()
                .filter(t -> t != Thread.currentThread() && t.getThreadGroup() != null && !t.getThreadGroup().getName().equals("system")
                    && !t.getThreadGroup().getName().equals("InnocuousThreadGroup"))
                .collect(Collectors.toMap(t -> t.getName() + "#" + t.getId(), t -> t))).values()) {
            threads.add(thread.getName().replaceAll("\\d+$", "N") + (thread.isDaemon() ? " (daemon)" : ""));
        }
        java.util.Collections.sort((List) threads);
        out.put("threads", threads);
        out.put("registries", WorldTrace.registries());
        out.put("_err", WorldTrace.errLines - before);
        return out;
    }
}
