package core.game.world.update.flag.context;
import core.game.node.entity.impl.Animator.Priority;
/** Adapter: an id, a delay and a priority, with the pinned two short constructors and create spliced verbatim. */
public class Animation {
    private final int id, delay;
    private final Priority priority;
    /* ANIMATION_ID */
    /* ANIMATION_CREATE */
    /* ANIMATION_PRIORITY */
    public Animation(int id, int delay, Priority priority) { this.id = id; this.delay = delay; this.priority = priority; }
    public int getId() { return id; }
    public int getDelay() { return delay; }
    public Priority getPriority() { return priority; }
}
