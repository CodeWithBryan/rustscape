package core.game.node;
import core.game.world.map.Location;
import oracle.Support;
/** State fields are named raw* so Kotlin property syntax reaches the recording getters, as it reaches the original getters. */
public abstract class Node {
    public int ref = -2, rawId; public boolean rawActive = true; public Location rawLocation;
    public int getId() { return Support.emit("node.id", rawId, ref); }
    public boolean isActive() { return Support.emit("node.isActive", rawActive, ref); }
    public void setActive(boolean v) { Support.emit("node.setActive", null, ref, v); rawActive = v; }
    public Location getLocation() { return Support.emit("node.location", rawLocation, ref); }
    public core.game.node.scenery.Scenery asScenery() { return (core.game.node.scenery.Scenery) this; }
}