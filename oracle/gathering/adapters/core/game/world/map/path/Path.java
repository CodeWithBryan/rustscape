package core.game.world.map.path;
import core.game.node.entity.Entity;
import oracle.Support;
public class Path {
    private final int id;
    Path(int id) { this.id = id; }
    public void walk(Entity entity) { Support.emit("path.walk", null, id, entity.ref); }
}