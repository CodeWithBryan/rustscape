package core.game.interaction;
import core.plugin.Plugin;
import oracle.W;
/** Adapter: the type constants, the allowed nodes and a recorded addHandler. Its run and its pulses are not here. */
public abstract class UseWithHandler implements Plugin<Object> {
    public static final int ITEM_TYPE = 0;
    public static final int NPC_TYPE = 1;
    public static final int OBJECT_TYPE = 2;
    public static final int PLAYER_TYPE = 3;
    public final int[] allowedNodes;
    public UseWithHandler(int... allowedNodes) { this.allowedNodes = allowedNodes; }
    public static void addHandler(int id, int type, UseWithHandler handler) { W.addHandler(id, type, handler); }
    public abstract boolean handle(NodeUsageEvent event);
    @Override public Object fireEvent(String identifier, Object... args) { return null; }
}
