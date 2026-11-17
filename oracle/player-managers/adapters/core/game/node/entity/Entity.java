package core.game.node.entity;

import core.game.node.Node;
import core.game.node.entity.player.Player;
import core.game.system.timer.TimerManager;
import core.game.world.map.Location;
import core.game.world.update.flag.context.Animation;
import core.game.world.update.flag.context.Graphics;
import java.util.HashMap;
import java.util.Map;
import oracle.Support;

/**
 * Recording entity: index, location and activity from the case, attributes
 * and extensions as maps, the real TimerManager. Every member records its call.
 */
@SuppressWarnings({"unchecked", "rawtypes"})
public class Entity extends Node {
	public final int slot;
	public boolean active;
	public final Map<String, Object> attributes = new HashMap<>();
	public final Map<Class, Object> extensions = new HashMap<>();
	private final TimerManager timers = new TimerManager(this);
	private final Scripts scripts = new Scripts();

	public Entity(int slot, Map spec) {
		super(Support.loc(spec.get("location")));
		this.slot = slot;
		this.index = Support.num(spec, "index", slot + 1);
		this.active = Support.flag(spec, "active", true);
	}

	public static class Scripts {
		public int getDelay() {
			return 0;
		}
	}

	public Scripts getScripts() {
		return scripts;
	}

	public TimerManager getTimers() {
		return timers;
	}

	public Player asPlayer() {
		return (Player) this;
	}

	public String getName() {
		return "entity" + slot;
	}

	public boolean isActive() {
		Support.emit("isActive", Support.label(this));
		return active;
	}

	public void face(Entity entity) {
		Support.emit("face", Support.label(this), Support.label(entity));
	}

	public void lock() {
		Support.emit("lock", Support.label(this));
	}

	public void unlock() {
		Support.emit("unlock", Support.label(this));
	}

	public boolean animate(Animation animation) {
		Support.emit("animate", Support.label(this), animation.getId());
		return true;
	}

	public boolean graphics(Graphics graphics) {
		Support.emit("graphics", Support.label(this), graphics.getId());
		return true;
	}

	public <T> T getAttribute(String key) {
		Support.emit("getAttribute", Support.label(this), key);
		return (T) attributes.get(key);
	}

	public <T> T getAttribute(String key, T fail) {
		Support.emit("getAttribute", Support.label(this), key);
		Object value = attributes.get(key);
		return value != null ? (T) value : fail;
	}

	public void setAttribute(String key, Object value) {
		Support.emit("setAttribute", Support.label(this), key, Support.value(value));
		attributes.put(key, value);
	}

	public void removeAttribute(String key) {
		Support.emit("removeAttribute", Support.label(this), key);
		attributes.remove(key);
	}

	public <T> T getExtension(Class<T> type) {
		Support.emit("getExtension", Support.label(this), type.getSimpleName());
		return (T) extensions.get(type);
	}

	public void addExtension(Class type, Object value) {
		Support.emit("addExtension", Support.label(this), type.getSimpleName(), Support.value(value));
		extensions.put(type, value);
	}

	public void removeExtension(Class type) {
		Support.emit("removeExtension", Support.label(this), type.getSimpleName());
		extensions.remove(type);
	}
}
