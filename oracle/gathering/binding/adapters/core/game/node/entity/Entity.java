package core.game.node.entity;
import core.game.node.Node;
import oracle.Support;
/** What the original Skills reads from its entity. There is no assist session in this corpus. */
public abstract class Entity extends Node {
    public final Properties properties = new Properties();
    public Properties getProperties() { return properties; }
    public <T> T getExtension(Class<T> type) { return null; }
    public core.game.node.entity.player.Player asPlayer() { return (core.game.node.entity.player.Player) this; }
    public void dispatch(Object event) { throw new UnsupportedOperationException(); }
    public core.game.node.entity.skill.Skills getSkills() { return null; }
    public static class Properties {
        private int combat;
        public int getCombatLevel() { return combat; }
        public void setCombatLevel(int v) { Support.emit("properties.setCombatLevel", null, v); combat = v; }
    }
}