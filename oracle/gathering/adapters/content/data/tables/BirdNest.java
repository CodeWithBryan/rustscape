package content.data.tables;
import core.game.node.entity.player.Player;
import core.game.node.item.Item;
import oracle.Support;
public class BirdNest {
    private final int id, amount;
    private BirdNest(int id, int amount) { this.id = id; this.amount = amount; }
    /** The reply is the nest item the service hands back: [id, amount]. */
    public static BirdNest getRandomNest(boolean wyson) { Support.emit("birdNest.getRandomNest", Support.list(Support.nestId, Support.nestAmount), wyson); return new BirdNest(Support.nestId, Support.nestAmount); }
    public Item getNest() { return new Item(id, amount); }
    public static void drop(Player p) { Support.emit("birdNest.drop", null, p.ref); }
}
