package content.global.activity.ttrail;
import core.game.node.entity.player.Player;
import oracle.W;
/** Adapter: the declared answer. */
public final class TreasureTrailManager {
    private static final TreasureTrailManager INSTANCE = new TreasureTrailManager();
    public static TreasureTrailManager getInstance(Player player) { return INSTANCE; }
    public boolean hasClue() { W.emit("hasClue"); return W.flag("hasClue"); }
}
