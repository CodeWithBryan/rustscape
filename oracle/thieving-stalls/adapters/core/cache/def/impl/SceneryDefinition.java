package core.cache.def.impl;
import core.game.interaction.OptionHandler;
import oracle.W;
/** Adapter: keeps what a plugin registers, in order, for the driver. */
public final class SceneryDefinition {
    public static boolean setOptionHandler(String name, OptionHandler handler) {
        W.register(name, handler);
        return true;
    }
}
