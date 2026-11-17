package core.game.node.entity.impl;
import oracle.Support;
public class Animator {
    public enum Priority { LOW, MID, HIGH, VERY_HIGH }
    public boolean isAnimating() { return Support.emit("animator.isAnimating", Support.animating); }
}
