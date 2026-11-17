package core.game.node.scenery;
import core.game.node.Node;
import core.game.world.map.Location;
import oracle.Support;
public class Scenery extends Node {
    public int rawType, rawRotation;
    /** Declared target of a case, or the body of a recorded construction; not itself an event. */
    public Scenery(int id, Location l, int type, int rotation, boolean declared) {
        this.rawId = id; this.rawLocation = l; this.rawType = type; this.rawRotation = rotation; this.ref = Support.register(this);
    }
    public Scenery(int id, Location l, int type, int rotation) {
        this(id, l, type, rotation, true);
        Support.emit("scenery.new", ref, id, l, type, rotation);
    }
    public int getType() { return Support.emit("scenery.type", rawType, ref); }
    public int getRotation() { return Support.emit("scenery.rotation", rawRotation, ref); }
    public Scenery transform(int id) {
        Scenery s = new Scenery(id, rawLocation, rawType, rawRotation, true);
        Support.emit("scenery.transform", s.ref, ref, id);
        return s;
    }
}