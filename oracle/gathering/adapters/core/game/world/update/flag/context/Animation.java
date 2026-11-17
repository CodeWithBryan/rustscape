package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator;
public class Animation {
    public final int id; public final Animator.Priority priority;
    public Animation(int id) { this(id, null); }
    public Animation(int id, Animator.Priority priority) { this.id = id; this.priority = priority; }
    public int getId() { return id; }
}
