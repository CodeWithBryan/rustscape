package core.game.node.entity.impl;
import core.game.node.entity.Entity;
import core.game.world.map.Location;
import oracle.Support;
public class Projectile {
    public static Projectile create(Entity source, Entity victim, int id, int startHeight, int endHeight, int startDelay, int speed) {
        Support.emit("projectile.create", null, Support.ref(source), victim == null ? null : (Object) Support.ref(victim), id, startHeight, endHeight, startDelay, speed);
        return new Projectile();
    }
    public Projectile transform(Entity source, Location l, boolean npc, int baseSpeed, int modifiedSpeed) {
        Support.emit("projectile.transform", null, Support.ref(source), l, npc, baseSpeed, modifiedSpeed);
        return this;
    }
    public void send() { Support.emit("projectile.send", null); }
}
