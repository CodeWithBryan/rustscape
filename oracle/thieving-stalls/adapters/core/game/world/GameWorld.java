package core.game.world;
import oracle.W;
/** Adapter: the declared tick. */
public final class GameWorld {
    public static int getTicks() { W.emit("ticks"); return W.number("ticks"); }
}
