package core.game.node.entity.player.link.diary;
import core.game.node.entity.player.Player;
import oracle.W;
/** Adapter: completion is declared case state; finishTask is recorded. */
public final class AchievementDiaryManager {
    public boolean hasCompletedTask(DiaryType type, int level, int index) { return W.flag("taskDone"); }
    public void finishTask(Player player, DiaryType type, int level, int index) { W.emit("finishTask", type.name(), level, index); }
}
