package content.global.skill.slayer;
import core.game.node.entity.player.Player;
import oracle.W;
/** Adapter: the declared ring flag. */
public final class SlayerManager {
    public static final class Flags {
        public boolean isRingUnlocked() { W.emit("ringUnlocked"); return W.flag("ring"); }
    }
    public final Flags flags = new Flags();
    public static SlayerManager getInstance(Player player) { return new SlayerManager(); }
}
