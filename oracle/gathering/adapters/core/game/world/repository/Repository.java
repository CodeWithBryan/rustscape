package core.game.world.repository;
import oracle.Support;
public class Repository {
    /** Only size() is used by the slice; the count is a declared world input. */
    public static java.util.AbstractList<Object> getPlayers() {
        return new java.util.AbstractList<Object>() {
            public Object get(int i) { throw new UnsupportedOperationException(); }
            public int size() { return Support.emit("repository.players.size", Support.playerCount); }
        };
    }
}
