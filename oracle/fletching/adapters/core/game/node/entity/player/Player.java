package core.game.node.entity.player;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.diary.AchievementDiaryManager;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final AchievementDiaryManager diaries = new AchievementDiaryManager();
    @Override public int getId() { return 0; }
    @Override public Location getLocation() { return W.location(); }
    public boolean animate(Animation animation) { W.emit("animate", animation.getId()); return true; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
}
