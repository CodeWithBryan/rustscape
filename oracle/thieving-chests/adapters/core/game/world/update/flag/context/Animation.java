package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
/** Adapter: the id and the priority (MID when none is given, as in the original). */
public class Animation {
    private final int id; private final Priority priority;
    public Animation(int id) { this(id, Priority.MID); }
    public Animation(int id, Priority priority) { this.id = id; this.priority = priority; }
    public static Animation create(int id) { return new Animation(id, Priority.MID); }
    public int getId() { return id; }
    public Priority getPriority() { return priority; }
}
