package core.game.component;
/** Adapter: a recorded put. */
public final class ComponentDefinition {
    public static void put(int id, ComponentPlugin plugin) { oracle.W.component(id, plugin); }
}
