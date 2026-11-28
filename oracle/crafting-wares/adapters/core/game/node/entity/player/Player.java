package core.game.node.entity.player;
import core.game.node.entity.Entity;
import core.game.node.entity.player.link.diary.DiaryType;
import core.game.node.entity.skill.Skills;
import oracle.W;
/** Adapter: the one player of a case. */
public final class Player extends Entity {
    private final Skills skills = new Skills();
    private final AchievementDiaryManager diaries = new AchievementDiaryManager();
    @Override public int getId() { return 0; }
    public Skills getSkills() { return skills; }
    public AchievementDiaryManager getAchievementDiaryManager() { return diaries; }
    /** Adapter: a recorded finishTask. */
    public static final class AchievementDiaryManager {
        public void finishTask(Player player, DiaryType type, int level, int index) {
            W.emit("diary", player == W.player, type.name(), level, index);
        }
    }
}
