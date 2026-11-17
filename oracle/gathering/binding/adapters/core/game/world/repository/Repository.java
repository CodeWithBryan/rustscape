package core.game.world.repository;
import oracle.Support;
public class Repository {
    public static java.util.AbstractList<Object> getPlayers() {
        return new java.util.AbstractList<Object>() {
            public Object get(int i) { throw new UnsupportedOperationException(); }
            public int size() { return Support.emit("repository.players.size", Support.playerCount); }
        };
    }
    public static void sendNews(String text) { Support.emit("repository.sendNews", null, text); }
}