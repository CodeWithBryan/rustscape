"""Stand-ins for everything the unchanged ForceMovement reaches outside itself.

Each entry is one source file compiled next to the unchanged ForceMovement.java, Pulse.java,
Direction.java, Point.java, ClipMaskSupplier.java, Animation.java, ForceMoveCtx.kt and EntityFlag.kt.
`/* ORIGINAL_* */` markers are filled with original text at generation time (generate.py, `markers`).
Every recorded call goes through oracle.Support.emit, which also injects the failures of the sweep.
Stand-ins for paths the corpus does not drive throw IllegalStateException("not driven").
"""

ADAPTERS = {}

ADAPTERS['core/game/world/map/Location.kt'] = '''package core.game.world.map
import kotlin.math.*
// The original LocationHash packing, constructors, coordinate properties, create, getDistance and getDelta.
class Location(var hash: LocationHash) {
    /* ORIGINAL_LOCATION_CONSTRUCTORS */

    /* ORIGINAL_LOCATION_PROPERTIES */

    companion object {
        /* ORIGINAL_LOCATION_CREATE */
        /* ORIGINAL_LOCATION_DISTANCE */
        /* ORIGINAL_LOCATION_DELTA */
    }

    /* ORIGINAL_LOCATION_INSTANCE_DISTANCE */

    /* ORIGINAL_LOCATION_HASH */
}
'''

ADAPTERS['core/game/world/map/RegionManager.kt'] = '''package core.game.world.map
// Direction.canMove's clipping read; never driven.
object RegionManager {
    @JvmStatic fun getClippingFlag(z: Int, x: Int, y: Int): Int = throw IllegalStateException("not driven")
}
'''

ADAPTERS['core/game/node/entity/impl/Animator.java'] = '''package core.game.node.entity.impl;
// The original Priority constants.
public class Animator {
    /* ORIGINAL_PRIORITY */
}
'''

ADAPTERS['core/cache/def/impl/AnimationDefinition.java'] = '''package core.cache.def.impl;
// Animation.getDuration's definition read; never driven.
public class AnimationDefinition {
    public static AnimationDefinition forId(int id) { throw new IllegalStateException("not driven"); }
    public int getDurationTicks() { throw new IllegalStateException("not driven"); }
}
'''

ADAPTERS['core/game/node/scenery/Scenery.java'] = '''package core.game.node.scenery;
// Animation's object field type; never set.
public class Scenery {}
'''

ADAPTERS['core/game/node/Node.java'] = '''package core.game.node;
// A node of the case: Pulse's isActive check is recorded and answered from the case.
public abstract class Node {
    public boolean isActive() {
        oracle.Support.emit("active", oracle.Support.ix(this));
        return oracle.Support.flag(this, "active", true);
    }
}
'''

ADAPTERS['core/game/node/entity/Entity.java'] = '''package core.game.node.entity;
import core.game.node.Node;
import core.game.node.entity.combat.ImpactHandler;
import core.game.node.entity.impl.WalkingQueue;
import core.game.world.map.Location;
import core.game.world.update.UpdateMasks;
import core.game.world.update.flag.context.Animation;
// The case's entity; every member ForceMovement calls is recorded.
public abstract class Entity extends Node {
    private final ImpactHandler impactHandler = new ImpactHandler(this);
    private final UpdateMasks updateMasks = new UpdateMasks(this);
    private final WalkingQueue walkingQueue = new WalkingQueue(this);
    private final Properties properties = new Properties(this);
    public Location getLocation() {
        oracle.Support.emit("location", oracle.Support.ix(this));
        return oracle.Support.location(this);
    }
    public boolean animate(Animation animation) {
        oracle.Support.emit("animate", oracle.Support.ix(this), oracle.Support.anim(animation));
        return true;
    }
    public ImpactHandler getImpactHandler() { return impactHandler; }
    public UpdateMasks getUpdateMasks() { return updateMasks; }
    public WalkingQueue getWalkingQueue() { return walkingQueue; }
    public Properties getProperties() { return properties; }
    public void lock() { oracle.Support.emit("lock", oracle.Support.ix(this)); }
    public void unlock() { oracle.Support.emit("unlock", oracle.Support.ix(this)); }
}
'''

ADAPTERS['core/game/node/entity/combat/ImpactHandler.java'] = '''package core.game.node.entity.combat;
import core.game.node.entity.Entity;
// setDisabledTicks is recorded.
public class ImpactHandler {
    private final Entity entity;
    public ImpactHandler(Entity entity) { this.entity = entity; }
    public void setDisabledTicks(int ticks) { oracle.Support.emit("disabled_ticks", oracle.Support.ix(entity), ticks); }
}
'''

ADAPTERS['core/game/world/update/UpdateMasks.java'] = '''package core.game.world.update;
import core.game.node.entity.Entity;
import core.game.world.update.flag.EntityFlag;
// register is recorded with the flag and the context's fields.
public class UpdateMasks {
    private final Entity entity;
    public UpdateMasks(Entity entity) { this.entity = entity; }
    public boolean register(EntityFlag flag, Object context) {
        oracle.Support.emit("register_mask", oracle.Support.ix(entity), flag.name(), oracle.Support.context(context));
        return true;
    }
}
'''

ADAPTERS['core/game/node/entity/impl/WalkingQueue.java'] = '''package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
// updateRegion is recorded.
public class WalkingQueue {
    private final Entity entity;
    public WalkingQueue(Entity entity) { this.entity = entity; }
    public boolean updateRegion(Location location, Location destination, boolean forced) {
        oracle.Support.emit("update_region", oracle.Support.ix(entity), oracle.Support.loc(location), oracle.Support.loc(destination), forced);
        return true;
    }
}
'''

ADAPTERS['core/game/node/entity/Properties.java'] = '''package core.game.node.entity;
import core.game.world.map.Location;
// setTeleportLocation is recorded.
public class Properties {
    private final Entity entity;
    public Properties(Entity entity) { this.entity = entity; }
    public void setTeleportLocation(Location location) {
        oracle.Support.emit("teleport_location", oracle.Support.ix(entity), oracle.Support.loc(location));
    }
}
'''

ADAPTERS['core/game/node/entity/player/Player.java'] = '''package core.game.node.entity.player;
public class Player extends core.game.node.entity.Entity {}
'''

ADAPTERS['core/game/node/entity/npc/NPC.java'] = '''package core.game.node.entity.npc;
public class NPC extends core.game.node.entity.Entity {}
'''

ADAPTERS['core/game/world/PulseRunner.java'] = '''package core.game.world;
import core.game.system.task.Pulse;
// submit is recorded with the movement's label.
public class PulseRunner {
    public void submit(Pulse pulse) { oracle.Support.emit("submit", oracle.Support.label(pulse)); }
}
'''

ADAPTERS['core/game/world/GameWorld.kt'] = '''package core.game.world
// The pulse runner ForceMovement submits to.
object GameWorld {
    @JvmStatic
    val Pulser = PulseRunner()
}
'''
