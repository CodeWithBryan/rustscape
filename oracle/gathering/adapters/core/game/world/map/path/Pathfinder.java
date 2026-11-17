package core.game.world.map.path;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
import oracle.Support;
/** Recording stand-in: a found path is an owner token. */
public class Pathfinder {
    public static Path find(Entity mover, Location destination) { int id = Support.paths++; Support.emit("pathfinder.find", id, mover.ref, destination); return new Path(id); }
    public static Path find(Location start, Location destination) { int id = Support.paths++; Support.emit("pathfinder.findFrom", id, start, destination); return new Path(id); }
}