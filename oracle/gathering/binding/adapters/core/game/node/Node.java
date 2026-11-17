package core.game.node;
import core.game.interaction.*;
import core.game.world.map.Location;
import oracle.Support;
/** The first corpus's recording node, with the fields and the pinned members the original Item needs. */
public abstract class Node {
    public int ref = -2, rawId; public boolean rawActive = true; public Location rawLocation;
    protected String name; protected int index; protected Location location; protected InteractPlugin interactPlugin; protected DestinationFlag destinationFlag;
    public Node() {}
    public int getId() { return Support.emit("node.id", rawId, ref); }
    public boolean isActive() { return Support.emit("node.isActive", rawActive, ref); }
    public void setActive(boolean v) { Support.emit("node.setActive", null, ref, v); rawActive = v; }
    public Location getLocation() { return Support.emit("node.location", rawLocation, ref); }
    public core.game.node.scenery.Scenery asScenery() { return (core.game.node.scenery.Scenery) this; }
    /* NODE_METHODS */
}