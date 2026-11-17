package oracle;
import java.util.*;
import java.lang.reflect.*;
import core.game.system.timer.TimerRegistry;
/** Oracle-only fresh-world isolation: HashMap.clear retains capacity across cases. */
public final class RegistryReset {
    public static void reset()throws Exception {
        Map<?,?> map=TimerRegistry.INSTANCE.getTimerMap();map.clear();
        Field table=HashMap.class.getDeclaredField("table");table.setAccessible(true);table.set(map,null);
        Field threshold=HashMap.class.getDeclaredField("threshold");threshold.setAccessible(true);threshold.setInt(map,0);
        TimerRegistry.INSTANCE.getAutoTimers().clear();
    }
}
