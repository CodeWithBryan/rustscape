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
    public Inventory getInventory() { return inventory; }
    public QuestRepository getQuestRepository() { return quests; }
    /** Adapter: the declared stage of a quest. */
    public static final class QuestRepository {
        public int getStage(content.data.Quests quest) { return W.questStage(quest.name()); }
    }
    private final QuestRepository quests = new QuestRepository();
    /** Adapter: the amount of an item's id in the declared inventory. */
    public static final class Inventory {
        public int getAmount(core.game.node.item.Item item) { return W.amount(item.getId()); }
    }
    private final Inventory inventory = new Inventory();
}
