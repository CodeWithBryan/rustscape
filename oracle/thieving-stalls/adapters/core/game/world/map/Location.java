package core.game.world.map;
import oracle.W;
/** Adapter: the declared region. */
public final class Location {
    public boolean isInRegion(int region) { W.emit("inRegion", region); return W.number("region") == region; }
}
