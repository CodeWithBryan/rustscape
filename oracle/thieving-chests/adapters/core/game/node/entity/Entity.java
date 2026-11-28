package core.game.node.entity;
import core.game.node.Node;
/** Adapter. */
public abstract class Entity extends Node {
    public abstract void lock(int ticks);
}
