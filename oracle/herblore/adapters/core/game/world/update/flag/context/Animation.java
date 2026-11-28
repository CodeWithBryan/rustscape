package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
/** Adapter: an id and the priority it was made with (null without one). */
public class Animation {
    private final int id;
    private final Priority priority;
    public Animation(int id) { this(id, null); }
    public Animation(int id, Priority priority) { this.id = id; this.priority = priority; }
    public int getId() { return id; }
    public Priority getPriority() { return priority; }
}
